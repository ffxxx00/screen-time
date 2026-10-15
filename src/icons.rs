use base64::Engine;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    Graphics::Gdi::*,
    System::{
        Diagnostics::ToolHelp::*,
        Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION},
    },
    UI::{Shell::*, WindowsAndMessaging::*},
};

pub fn process_path(pid: u32) -> Option<PathBuf> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buffer = vec![0u16; 32768];
        let mut length = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length);
        CloseHandle(handle);
        if ok == 0 {
            None
        } else {
            Some(PathBuf::from(std::ffi::OsString::from_wide(
                &buffer[..length as usize],
            )))
        }
    }
}

pub fn running_paths() -> BTreeMap<String, PathBuf> {
    let mut paths = BTreeMap::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return paths;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of_val(&entry) as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            if let Some(path) = process_path(entry.th32ProcessID) {
                if let Some(name) = path.file_name() {
                    paths.insert(name.to_string_lossy().to_lowercase(), path);
                }
            }
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
    }
    paths
}

pub fn icon_url(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let rgba = unsafe {
        let mut info: SHFILEINFOW = std::mem::zeroed();
        if SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        ) == 0
        {
            return None;
        }
        let dc = CreateCompatibleDC(ptr::null_mut());
        if dc.is_null() {
            DestroyIcon(info.hIcon);
            return None;
        }
        let mut bitmap: BITMAPINFO = std::mem::zeroed();
        bitmap.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bitmap.bmiHeader.biWidth = 32;
        bitmap.bmiHeader.biHeight = -32;
        bitmap.bmiHeader.biPlanes = 1;
        bitmap.bmiHeader.biBitCount = 32;
        bitmap.bmiHeader.biCompression = BI_RGB;
        let mut bits = ptr::null_mut();
        let dib = CreateDIBSection(dc, &bitmap, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        if dib.is_null() {
            DeleteDC(dc);
            DestroyIcon(info.hIcon);
            return None;
        }
        let previous = SelectObject(dc, dib);
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, 32 * 32 * 4);
        pixels.fill(0);
        let black_ok = DrawIconEx(dc, 0, 0, info.hIcon, 32, 32, 0, ptr::null_mut(), DI_NORMAL);
        GdiFlush();
        let black = pixels.to_vec();
        pixels.fill(255);
        let white_ok = DrawIconEx(dc, 0, 0, info.hIcon, 32, 32, 0, ptr::null_mut(), DI_NORMAL);
        GdiFlush();
        let mut rgba = vec![0; pixels.len()];
        for i in (0..pixels.len()).step_by(4) {
            let alpha = 255 - pixels[i].saturating_sub(black[i]);
            for (out, source) in [(0, 2), (1, 1), (2, 0)] {
                rgba[i + out] = if alpha == 0 {
                    0
                } else {
                    ((black[i + source] as u32 * 255 / alpha as u32).min(255)) as u8
                };
            }
            rgba[i + 3] = alpha;
        }
        SelectObject(dc, previous);
        DeleteObject(dib);
        DeleteDC(dc);
        DestroyIcon(info.hIcon);
        if black_ok == 0 || white_ok == 0 {
            return None;
        }
        rgba
    };
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 32, 32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
    }
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

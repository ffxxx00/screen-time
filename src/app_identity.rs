//! Best-effort hosted application identity. Queries run outside the tracking loop.
use crate::{history, icons, AppEvent};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{mpsc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use windows::{
    core::{BSTR, PCWSTR, PROPVARIANT, VARIANT},
    Win32::{
        Foundation::{HANDLE, HWND},
        Storage::Packaging::Appx::GetApplicationUserModelId,
        System::{Com::*, Wmi::*},
        UI::Shell::{PropertiesSystem::*, SHLoadIndirectString},
    },
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME},
    System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    UI::{Shell::CommandLineToArgvW, WindowsAndMessaging::GetWindowThreadProcessId},
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<crate::native_identity::Evidence>,
}
#[derive(Clone, Debug)]
pub struct Resolved {
    pub id: String,
    pub metadata: Metadata,
}
#[derive(Clone)]
struct Request {
    hwnd: usize,
    pid: u32,
    path: PathBuf,
    generation: u64,
    stamp: u64,
}
type Cache = BTreeMap<(u32, u64, usize), (Instant, Resolved)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
pub fn cached(hwnd: usize, pid: u32) -> Option<Resolved> {
    let stamp = process_stamp(pid)?;
    CACHE
        .get()?
        .lock()
        .ok()?
        .get(&(pid, stamp, hwnd))
        .filter(|(at, _)| at.elapsed() < Duration::from_secs(30))
        .map(|(_, result)| result.clone())
}
static REQUESTS: OnceLock<mpsc::Sender<Request>> = OnceLock::new();
pub fn request(hwnd: usize, pid: u32, path: &Path, generation: u64) {
    let Some(stamp) = process_stamp(pid) else {
        return;
    };
    if let Some(sender) = REQUESTS.get() {
        let _ = sender.send(Request {
            hwnd,
            pid,
            path: path.into(),
            generation,
            stamp,
        });
    }
}
pub fn start(proxy: tao::event_loop::EventLoopProxy<AppEvent>) {
    let (sender, receiver) = mpsc::channel::<Request>();
    if REQUESTS.set(sender).is_err() {
        return;
    }
    std::thread::spawn(move || {
        let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        let service = if initialized { wmi_service() } else { None };
        CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
        while let Ok(mut request) = receiver.recv() {
            // Keep only the newest foreground request rather than accumulating work.
            for newer in receiver.try_iter() {
                request = newer;
            }
            let stamp = request.stamp;
            if process_stamp(request.pid) != Some(stamp) {
                continue;
            }
            let key = (request.pid, stamp, request.hwnd);
            let cached = cached(request.hwnd, request.pid);
            let result = cached.unwrap_or_else(|| resolve(&request, service.as_ref(), initialized));
            let mut current_pid = 0;
            unsafe {
                GetWindowThreadProcessId(request.hwnd as _, &mut current_pid);
            }
            if current_pid != request.pid || process_stamp(request.pid) != Some(stamp) {
                continue;
            }
            if let Ok(mut cache) = CACHE.get().unwrap().lock() {
                if cache.len() >= 256 {
                    cache.clear();
                }
                cache.insert(key, (Instant::now(), result.clone()));
            }
            let _ = proxy.send_event(AppEvent::ApplicationResolved {
                generation: request.generation,
                hwnd: request.hwnd,
                path: request.path,
                result,
            });
        }
        drop(service);
        if initialized {
            unsafe { CoUninitialize() };
        }
    });
}
fn process_stamp(pid: u32) -> Option<u64> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut kernel = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut user = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
        CloseHandle(handle);
        (ok != 0)
            .then_some(((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64)
    }
}
fn wmi_service() -> Option<IWbemServices> {
    unsafe {
        let locator: IWbemLocator =
            CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).ok()?;
        let service = locator
            .ConnectServer(
                &BSTR::from("ROOT\\CIMV2"),
                &BSTR::new(),
                &BSTR::new(),
                &BSTR::new(),
                0,
                &BSTR::new(),
                None,
            )
            .ok()?;
        CoSetProxyBlanket(
            &service,
            10,
            0,
            PCWSTR::null(),
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )
        .ok()?;
        Some(service)
    }
}
fn command_line(service: &IWbemServices, pid: u32) -> Option<String> {
    unsafe {
        let query = BSTR::from(format!(
            "SELECT CommandLine FROM Win32_Process WHERE ProcessId={pid}"
        ));
        let enumeration = service
            .ExecQuery(
                &BSTR::from("WQL"),
                &query,
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None,
            )
            .ok()?;
        let mut objects = [None];
        let mut returned = 0;
        enumeration
            .Next(1500, &mut objects, &mut returned)
            .ok()
            .ok()?;
        let object = objects[0].take()?;
        let mut value = VARIANT::default();
        object
            .Get(windows::core::w!("CommandLine"), 0, &mut value, None, None)
            .ok()?;
        BSTR::try_from(&value).ok().map(|value| value.to_string())
    }
}
fn property(store: &IPropertyStore, pid: u32) -> Option<String> {
    unsafe {
        let key = PROPERTYKEY {
            fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
            pid,
        };
        let value: PROPVARIANT = store.GetValue(&key).ok()?;
        let value = BSTR::try_from(&value).ok()?.to_string();
        (!value.is_empty()).then_some(value)
    }
}
fn windows_identity(hwnd: usize, pid: u32) -> Option<(String, Option<String>)> {
    unsafe {
        if let Ok(store) = SHGetPropertyStoreForWindow::<_, IPropertyStore>(HWND(hwnd as _)) {
            if let Some(id) = property(&store, 5) {
                let name = property(&store, 4).and_then(|resource| {
                    let source: Vec<u16> = resource.encode_utf16().chain(Some(0)).collect();
                    let mut output = [0u16; 1024];
                    SHLoadIndirectString(PCWSTR(source.as_ptr()), &mut output, None).ok()?;
                    Some(String::from_utf16_lossy(
                        &output[..output.iter().position(|c| *c == 0).unwrap_or(output.len())],
                    ))
                });
                return Some((id.clone(), name.or_else(|| shell_name(&id))));
            }
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut length = 1024;
        let mut id = vec![0u16; length as usize];
        let result = GetApplicationUserModelId(
            HANDLE(handle),
            &mut length,
            windows::core::PWSTR(id.as_mut_ptr()),
        );
        CloseHandle(handle);
        (result.0 == 0 && length > 1 && length <= 1024).then(|| {
            let id = String::from_utf16_lossy(&id[..length as usize - 1]);
            let name = shell_name(&id);
            (id, name)
        })
    }
}
fn shell_name(id: &str) -> Option<String> {
    unsafe {
        let parsing: Vec<u16> = format!("shell:AppsFolder\\{id}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let item: windows::Win32::UI::Shell::IShellItem =
            windows::Win32::UI::Shell::SHCreateItemFromParsingName(PCWSTR(parsing.as_ptr()), None)
                .ok()?;
        let name = item
            .GetDisplayName(windows::Win32::UI::Shell::SIGDN_NORMALDISPLAY)
            .ok()?;
        let result = name.to_string().ok();
        CoTaskMemFree(Some(name.0.cast()));
        result
    }
}
fn resolve(request: &Request, service: Option<&IWbemServices>, com: bool) -> Resolved {
    let version = crate::native_identity::version_strings(&request.path);
    let runtime = version
        .get("OriginalFilename")
        .map(Path::new)
        .filter(|path| is_host(path))
        .unwrap_or(&request.path);
    let fallback = history::executable_name(&history::identity(&request.path));
    let mut result = Resolved {
        id: history::identity(&request.path),
        metadata: Metadata {
            name: fallback.clone(),
            icon: None,
            native: None,
        },
    };
    if let Some((id, name)) = (com && is_host(runtime))
        .then(|| windows_identity(request.hwnd, request.pid))
        .flatten()
    {
        result.id = format!("app:windows:{id}");
        // An AUMID is an identity, not necessarily a friendly display name.
        if let Some(name) = name.filter(|name| !name.trim().is_empty()) {
            result.metadata.name = name;
        }
    }
    if is_java(runtime) {
        if let Some(java) = service
            .and_then(|service| command_line(service, request.pid))
            .and_then(|command| java_identity(&split_command(&command)))
        {
            if result.id.starts_with("path:") {
                result = java;
            } else if result.metadata.name.eq_ignore_ascii_case(&fallback) {
                result.metadata.name = java.metadata.name;
            }
        }
    }
    if !is_host(runtime) {
        if let Some(name) = version
            .get("FileDescription")
            .or_else(|| version.get("ProductName"))
        {
            result.metadata.name = name.clone();
        }
        if let Some(native) = crate::native_identity::identify(&request.path) {
            result.id = native.0;
            result.metadata.native = Some(native.1);
        }
    }
    if result.id.starts_with("app:") {
        result.metadata.icon = icons::window_icon_url(request.hwnd);
    }
    readable_java_name(&result.id, &mut result.metadata);
    result
}

// Presentation is separate from identity. Never infer a brand from a package.
fn readable_java_name(id: &str, metadata: &mut Metadata) {
    let main = id
        .strip_prefix("app:java:class:")
        .map(str::to_owned)
        .or_else(|| {
            let fields: Vec<String> =
                serde_json::from_str(id.strip_prefix("app:java:manifest:")?).ok()?;
            (fields.len() == 3).then(|| fields[2].clone())
        });
    let main = main.or_else(|| id.strip_prefix("app:java:module:").map(str::to_owned));
    if let Some(main) = main {
        if metadata.name == main || metadata.name.is_empty() {
            metadata.name = readable_java_target(&main);
        }
    }
}
fn readable_java_target(main: &str) -> String {
    let parts: Vec<_> = main
        .split(['.', '$', '/'])
        .filter(|s| !s.is_empty())
        .collect();
    let generic = |word: &str| {
        matches!(
            word.to_ascii_lowercase().as_str(),
            "main"
                | "app"
                | "application"
                | "entrypoint"
                | "bootstrap"
                | "launcher"
                | "client"
                | "server"
                | "ui"
                | "com"
                | "org"
                | "net"
                | "io"
        )
    };
    let selected = parts
        .iter()
        .rev()
        .find(|word| !generic(word))
        .copied()
        .unwrap_or(main);
    let characters: Vec<_> = selected.chars().collect();
    let mut result = String::new();
    for (i, &c) in characters.iter().enumerate() {
        if c == '_' || c == '-' {
            result.push(' ');
            continue;
        }
        if i > 0
            && c.is_uppercase()
            && (characters[i - 1].is_lowercase()
                || (characters[i - 1].is_uppercase()
                    && characters.get(i + 1).is_some_and(|n| n.is_lowercase())))
        {
            result.push(' ');
        }
        if result.is_empty() {
            result.extend(c.to_uppercase());
        } else {
            result.push(c);
        }
    }
    result.trim().to_string()
}
pub fn is_host(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        matches!(
            name.to_string_lossy().to_ascii_lowercase().as_str(),
            "java.exe"
                | "javaw.exe"
                | "python.exe"
                | "pythonw.exe"
                | "node.exe"
                | "electron.exe"
                | "rundll32.exe"
                | "dllhost.exe"
                | "applicationframehost.exe"
                | "wwahost.exe"
        )
    })
}
fn is_java(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        matches!(
            name.to_string_lossy().to_ascii_lowercase().as_str(),
            "java.exe" | "javaw.exe"
        )
    })
}
fn split_command(command: &str) -> Vec<String> {
    unsafe {
        let wide: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
        let mut count = 0;
        let argv = CommandLineToArgvW(wide.as_ptr(), &mut count);
        if argv.is_null() {
            return Vec::new();
        }
        let mut result = Vec::new();
        for i in 0..count {
            let value = *argv.add(i as usize);
            let mut len = 0;
            while *value.add(len) != 0 {
                len += 1;
            }
            result.push(String::from_utf16_lossy(std::slice::from_raw_parts(
                value, len,
            )));
        }
        windows_sys::Win32::Foundation::LocalFree(argv.cast());
        result
    }
}
#[derive(Debug, PartialEq)]
enum JavaTarget {
    Class(String, Option<String>),
    Jar(String),
    Module(String),
}
fn java_target(args: &[String]) -> Option<JavaTarget> {
    let mut i = 1;
    let mut classpath = None;
    while i < args.len() {
        let arg = &args[i];
        if arg == "-jar" {
            return args.get(i + 1).cloned().map(JavaTarget::Jar);
        }
        if arg == "-m" || arg == "--module" {
            return args
                .get(i + 1)
                .map(|value| JavaTarget::Module(value.clone()));
        }
        if let Some(value) = arg.strip_prefix("--module=") {
            return Some(JavaTarget::Module(value.into()));
        }
        if matches!(arg.as_str(), "-cp" | "-classpath" | "--class-path") {
            classpath = args.get(i + 1).cloned();
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--class-path=") {
            classpath = Some(value.into());
            i += 1;
            continue;
        }
        if matches!(
            arg.as_str(),
            "-p" | "--module-path"
                | "--upgrade-module-path"
                | "--add-modules"
                | "--limit-modules"
                | "--add-reads"
                | "--add-exports"
                | "--add-opens"
                | "--patch-module"
                | "--enable-native-access"
                | "--source"
        ) {
            i += 2;
            continue;
        }
        // Argfiles need the launch working directory; never guess it from the Java binary.
        if arg.starts_with('@')
            || matches!(
                arg.as_str(),
                "--" | "-h" | "--help" | "-version" | "--version"
            )
        {
            return None;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        if arg.ends_with(".java") {
            return None;
        }
        if arg
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '$'))
        {
            return Some(JavaTarget::Class(arg.clone(), classpath));
        }
        return None;
    }
    None
}
fn manifest(path: &Path) -> Option<BTreeMap<String, String>> {
    if !path.is_absolute() {
        return None;
    }
    let file = File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut entry = archive.by_name("META-INF/MANIFEST.MF").ok()?;
    if entry.size() > 65536 {
        return None;
    }
    let mut bytes = Vec::new();
    entry.by_ref().take(65537).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 65536 {
        return None;
    }
    parse_manifest(std::str::from_utf8(&bytes).ok()?)
}
fn parse_manifest(text: &str) -> Option<BTreeMap<String, String>> {
    let mut lines = Vec::<String>::new();
    for line in text.lines() {
        if line.is_empty() {
            break;
        }
        if let Some(continuation) = line.strip_prefix(' ') {
            lines.last_mut()?.push_str(continuation);
        } else {
            lines.push(line.into());
        }
    }
    Some(
        lines
            .into_iter()
            .filter_map(|line| {
                line.split_once(": ")
                    .map(|(key, value)| (key.to_ascii_lowercase(), value.trim().to_string()))
            })
            .collect(),
    )
}
fn from_manifest(data: &BTreeMap<String, String>) -> Option<Resolved> {
    let main = data.get("main-class")?;
    let title = manifest_title(data);
    let vendor = data
        .get("implementation-vendor-id")
        .or_else(|| data.get("implementation-vendor"));
    let key = serde_json::to_string(&(
        vendor.map(String::as_str).unwrap_or(""),
        title.unwrap_or(""),
        main,
    ))
    .ok()?;
    Some(Resolved {
        id: format!("app:java:manifest:{key}"),
        metadata: Metadata {
            name: title
                .map(str::to_owned)
                .unwrap_or_else(|| readable_java_target(main)),
            icon: None,
            native: None,
        },
    })
}
fn manifest_title(data: &BTreeMap<String, String>) -> Option<&str> {
    [
        "implementation-title",
        "specification-title",
        "javafx-application-name",
        "bundle-name",
    ]
    .iter()
    .filter_map(|key| data.get(*key))
    .map(|s| s.trim())
    .find(|s| {
        !s.is_empty() && s.len() <= 256 && !s.starts_with('%') && !s.chars().any(char::is_control)
    })
}
fn classpath_metadata(classpath: &str, main: &str) -> Option<BTreeMap<String, String>> {
    let entry = format!("{}.class", main.replace('.', "/"));
    // Only the archive/directory containing the entry class can name the app;
    // dependency manifests often declare unrelated library product names.
    for path in classpath
        .split(';')
        .take(64)
        .map(Path::new)
        .filter(|p| p.is_absolute())
    {
        if path.is_dir() && path.join(&entry).is_file() {
            let manifest = path.join("META-INF/MANIFEST.MF");
            let mut text = String::new();
            let Ok(file) = File::open(manifest) else {
                continue;
            };
            if file.take(65537).read_to_string(&mut text).is_err() {
                continue;
            }
            if text.len() <= 65536 {
                return parse_manifest(&text);
            }
        } else if path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("jar"))
        {
            let Some(mut archive) = File::open(path)
                .ok()
                .and_then(|file| zip::ZipArchive::new(file).ok())
            else {
                continue;
            };
            if archive.by_name(&entry).is_err() {
                continue;
            }
            return manifest(path);
        }
    }
    None
}
fn java_identity(args: &[String]) -> Option<Resolved> {
    let mut result = java_target_identity(args)?;
    // A small allowlist of application-name properties, only in JVM options.
    // Never retain arbitrary arguments or use the window/document title.
    let mut options = args.iter().skip(1);
    while let Some(argument) = options.next() {
        if !argument.starts_with('-') {
            break;
        }
        if matches!(
            argument.as_str(),
            "-cp"
                | "-classpath"
                | "--class-path"
                | "-p"
                | "--module-path"
                | "--upgrade-module-path"
                | "--add-modules"
                | "--limit-modules"
                | "--add-reads"
                | "--add-exports"
                | "--add-opens"
                | "--patch-module"
                | "--enable-native-access"
                | "--source"
        ) {
            options.next();
            continue;
        }
        if matches!(argument.as_str(), "-jar" | "-m" | "--module")
            || argument.starts_with("--module=")
        {
            break;
        }
        if let Some(name) = [
            "-Djava.awt.application.name=",
            "-Dapple.awt.application.name=",
            "-Dapplication.name=",
            "-Dapp.name=",
        ]
        .iter()
        .find_map(|prefix| argument.strip_prefix(prefix))
        {
            if !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control) {
                result.metadata.name = name.trim().into();
            }
        }
    }
    Some(result)
}
fn java_target_identity(args: &[String]) -> Option<Resolved> {
    match java_target(args)? {
        JavaTarget::Jar(jar) => {
            let path = Path::new(&jar);
            if let Some(result) = manifest(path).as_ref().and_then(from_manifest) {
                return Some(result);
            }
            // No semantic metadata: keep different JARs distinct, irrespective of the JVM location.
            if !path.is_absolute() {
                return None;
            }
            Some(Resolved {
                id: format!("app:java:jar:{}", history::identity(path)),
                metadata: Metadata {
                    name: path.file_stem()?.to_string_lossy().into_owned(),
                    icon: None,
                    native: None,
                },
            })
        }
        JavaTarget::Class(main, classpath) => {
            let data = classpath
                .as_deref()
                .and_then(|paths| classpath_metadata(paths, &main));
            if let Some(data) = data
                .as_ref()
                .filter(|data| data.get("main-class") == Some(&main))
            {
                if let Some(result) = from_manifest(data) {
                    return Some(result);
                }
            }
            Some(Resolved {
                id: format!("app:java:class:{main}"),
                metadata: Metadata {
                    name: data
                        .as_ref()
                        .and_then(manifest_title)
                        .map(str::to_owned)
                        .unwrap_or_else(|| readable_java_target(&main)),
                    icon: None,
                    native: None,
                },
            })
        }
        JavaTarget::Module(module) => {
            let name = module.split('/').next()?.to_string();
            if name.is_empty() {
                return None;
            }
            Some(Resolved {
                id: format!("app:java:module:{name}"),
                metadata: Metadata {
                    name: readable_java_target(&name),
                    icon: None,
                    native: None,
                },
            })
        }
    }
}

pub fn load_catalog(csv: &Path) -> BTreeMap<String, Metadata> {
    let cached = std::fs::read(csv.with_extension("apps.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<BTreeMap<String, Metadata>>(&bytes).ok());
    let mut result = cached.unwrap_or_default();
    if let Ok(mut reader) = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(csv)
    {
        for row in reader.records().flatten() {
            if row.len() == 7 && history::parse(&row).is_some() {
                let metadata = result.entry(row[3].to_string()).or_default();
                // Keep a richer name learned after these historical rows were
                // written; a technical class/executable must not downgrade it.
                if metadata.name.is_empty() || !technical_name(&row[3], &row[6]) {
                    metadata.name = row[6].to_string();
                }
                if let Some(digest) = row[3].strip_prefix("app:native:") {
                    let evidence = metadata.native.get_or_insert_with(Default::default);
                    evidence.fingerprints.insert(digest.to_string());
                    if !row[4].is_empty() {
                        evidence.paths.insert(history::identity(Path::new(&row[4])));
                    }
                }
            }
        }
    }
    for (id, metadata) in &mut result {
        readable_java_name(id, metadata);
    }
    result
}
fn technical_name(id: &str, name: &str) -> bool {
    if name.to_ascii_lowercase().ends_with(".exe") {
        return true;
    }
    let mut metadata = Metadata {
        name: name.into(),
        ..Default::default()
    };
    readable_java_name(id, &mut metadata);
    metadata.name != name
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write_test_jar(path: &Path, manifest: &str, class: &str) {
        use std::io::Write;
        let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
        archive
            .start_file("META-INF/MANIFEST.MF", zip::write::FileOptions::default())
            .unwrap();
        archive.write_all(manifest.as_bytes()).unwrap();
        archive
            .start_file(
                format!("{}.class", class.replace('.', "/")),
                zip::write::FileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"synthetic class, not executed").unwrap();
        archive.finish().unwrap();
    }
    #[test]
    fn names_come_from_owning_archive_without_main_class_not_dependency() {
        let fixture = crate::test_support::Fixture::new("");
        let library = fixture.root.join("dependency.jar");
        let program = fixture.root.join("program.jar");
        write_test_jar(
            &library,
            "Implementation-Title: Wrong Library\nMain-Class: org.vendor.PaintStudio\n\n",
            "org.library.Helper",
        );
        write_test_jar(
            &program,
            "Implementation-Title: Vendor Paint Studio\n\n",
            "org.vendor.PaintStudio",
        );
        let command = vec![
            "javaw.exe".into(),
            "-cp".into(),
            format!("{};{}", library.display(), program.display()),
            "org.vendor.PaintStudio".into(),
        ];
        let result = java_identity(&command).unwrap();
        assert_eq!(result.metadata.name, "Vendor Paint Studio");
        assert_eq!(result.id, "app:java:class:org.vendor.PaintStudio");
        // A JVM update and a library with tempting metadata do not name the app.
        let mut updated = command.clone();
        updated[0] = r"D:\OtherJava\java.exe".into();
        assert_eq!(java_identity(&updated).unwrap().id, result.id);
    }
    #[test]
    fn directory_classpath_and_missing_manifest_are_supported() {
        let fixture = crate::test_support::Fixture::new("");
        let classes = fixture.root.join("classes");
        std::fs::create_dir_all(classes.join("org/vendor")).unwrap();
        std::fs::write(classes.join("org/vendor/DiagramEditor.class"), b"synthetic").unwrap();
        let command = vec![
            "java.exe".into(),
            "-cp".into(),
            classes.display().to_string(),
            "org.vendor.DiagramEditor".into(),
        ];
        assert_eq!(
            java_identity(&command).unwrap().metadata.name,
            "Diagram Editor"
        );
        std::fs::create_dir(classes.join("META-INF")).unwrap();
        std::fs::write(
            classes.join("META-INF/MANIFEST.MF"),
            "Bundle-Name: Diagram Workshop\n\n",
        )
        .unwrap();
        assert_eq!(
            java_identity(&command).unwrap().metadata.name,
            "Diagram Workshop"
        );
    }
    #[test]
    fn jvm_declared_name_is_bounded_and_application_arguments_are_not_names() {
        let app = java_identity(&args(&[
            "java",
            "-cp",
            r"C:\Apps\tool.jar",
            "-Dapplication.name=Example Workshop",
            "org.example.Main",
            "-Dapp.name=secret document",
        ]))
        .unwrap();
        assert_eq!(app.metadata.name, "Example Workshop");
        assert!(!app.id.contains("secret"));
        assert_eq!(
            java_identity(&args(&[
                "java",
                "org.example.DiagramEditor",
                "-Dapp.name=secret"
            ]))
            .unwrap()
            .metadata
            .name,
            "Diagram Editor"
        );
        assert_eq!(
            java_identity(&args(&[
                "java",
                "-Dapp.name=bad\nname",
                "org.example.DiagramEditor"
            ]))
            .unwrap()
            .metadata
            .name,
            "Diagram Editor"
        );
    }
    #[test]
    fn generic_names_and_cached_metadata_have_no_application_exceptions() {
        for (target, expected) in [
            ("org.example.PhotoEditor", "Photo Editor"),
            ("org.other.NetworkHTTPMonitor", "Network HTTP Monitor"),
            ("org.example.Workshop.EntryPoint", "Workshop"),
            ("org.example.plot_studio.Main", "Plot studio"),
        ] {
            let mut metadata = Metadata {
                name: target.into(),
                ..Default::default()
            };
            readable_java_name(&format!("app:java:class:{target}"), &mut metadata);
            assert_eq!(metadata.name, expected);
        }
        let fixture = crate::test_support::Fixture::new("");
        let id = "app:java:class:org.example.Main";
        let catalog = BTreeMap::from([(
            id.to_string(),
            Metadata {
                name: "Example Workshop".into(),
                ..Default::default()
            },
        )]);
        std::fs::write(
            fixture.csv.with_extension("apps.json"),
            serde_json::to_vec(&catalog).unwrap(),
        )
        .unwrap();
        let row = history::encode_named(
            &chrono::Local::now(),
            id,
            Some(Path::new(r"C:\Java\javaw.exe")),
            60.0,
            Some("org.example.Main"),
        );
        std::fs::write(&fixture.csv, &row).unwrap();
        assert_eq!(load_catalog(&fixture.csv)[id].name, "Example Workshop");
        assert_eq!(std::fs::read(&fixture.csv).unwrap(), row);
    }
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }
    #[test]
    fn native_window_identity_and_icon_are_independent_of_executable() {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
            let class: Vec<u16> = format!("ScreenTimeIdentityTest{}", std::process::id())
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let instance =
                windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                hIcon: LoadIconW(std::ptr::null_mut(), IDI_INFORMATION),
                ..std::mem::zeroed()
            };
            assert_ne!(RegisterClassW(&wc), 0);
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            {
                let store: IPropertyStore = SHGetPropertyStoreForWindow(HWND(hwnd)).unwrap();
                let id_key = PROPERTYKEY {
                    fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
                    pid: 5,
                };
                let name_key = PROPERTYKEY { pid: 4, ..id_key };
                store
                    .SetValue(&id_key, &PROPVARIANT::from("Example.Editor"))
                    .unwrap();
                store
                    .SetValue(&name_key, &PROPVARIANT::from("Example Editor"))
                    .unwrap();
                let native = resolve(
                    &Request {
                        hwnd: hwnd as usize,
                        pid: std::process::id(),
                        path: PathBuf::from(r"C:\Apps\browser.exe"),
                        generation: 0,
                        stamp: process_stamp(std::process::id()).unwrap(),
                    },
                    None,
                    true,
                );
                assert_eq!(
                    native.id,
                    history::identity(Path::new(r"C:\Apps\browser.exe"))
                );
                let (id, name) = windows_identity(hwnd as usize, std::process::id()).unwrap();
                assert_eq!(id, "Example.Editor");
                assert_eq!(name.as_deref(), Some("Example Editor"));
                assert!(icons::window_icon_url(hwnd as usize)
                    .unwrap()
                    .starts_with("data:image/png;base64,"));
            }
            DestroyWindow(hwnd);
            UnregisterClassW(class.as_ptr(), instance);
            CoUninitialize();
        }
    }
    #[test]
    #[ignore = "Requires a synthetic Java process supplied by SCREEN_TIME_TEST_JAVA_PID"]
    fn native_java_process_resolves_manifest_without_a_game_rule() {
        let pid = std::env::var("SCREEN_TIME_TEST_JAVA_PID")
            .unwrap()
            .parse()
            .unwrap();
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        }
        {
            let service = wmi_service().unwrap();
            let path = icons::process_path(pid).unwrap();
            assert!(is_java(&path));
            let result = resolve(
                &Request {
                    hwnd: 0,
                    pid,
                    path,
                    generation: 0,
                    stamp: process_stamp(pid).unwrap(),
                },
                Some(&service),
                true,
            );
            assert_eq!(result.metadata.name, "Example Hosted Tool");
            assert!(result.id.starts_with("app:java:manifest:"));
        }
        unsafe {
            CoUninitialize();
        }
    }
    #[test]
    fn java_runtime_and_version_do_not_define_application() {
        let one = java_identity(&args(&[
            r"C:\Java17\javaw.exe",
            "-Xmx2G",
            "-Dexample=ignored",
            "com.example.Editor",
            "--token",
            "private",
        ]))
        .unwrap();
        let two = java_identity(&args(&[
            r"D:\Java21\javaw.exe",
            "-Xmx4G",
            "com.example.Editor",
        ]))
        .unwrap();
        assert_eq!(one.id, two.id);
        assert!(!one.id.contains("private"));
        assert_ne!(
            one.id,
            java_identity(&args(&["javaw.exe", "com.example.Game"]))
                .unwrap()
                .id
        );
    }
    #[test]
    fn options_quotes_modules_and_unsupported_argfiles() {
        let command = r#""C:\Program Files\Java\javaw.exe" --add-opens java.base/java.lang=ALL-UNNAMED -cp "C:\Apps\a.jar;C:\Apps\b.jar" com.example.Main --accessToken secret"#;
        assert_eq!(
            java_target(&split_command(command)),
            Some(JavaTarget::Class(
                "com.example.Main".into(),
                Some(r"C:\Apps\a.jar;C:\Apps\b.jar".into())
            ))
        );
        assert_eq!(
            java_target(&args(&["java", "--module=org.example/com.example.Main"])),
            Some(JavaTarget::Module("org.example/com.example.Main".into()))
        );
        assert!(java_target(&args(&["java", "@args.txt"])).is_none());
        assert!(java_identity(&args(&["java", "-jar", "relative.jar"])).is_none());
    }
    #[test]
    fn manifest_continuation_versions_and_individual_sections() {
        let first=parse_manifest("Manifest-Version: 1.0\r\nImplementation-Title: Example \r\n Editor\r\nImplementation-Vendor: Example Org\r\nImplementation-Version: 1.0\r\nMain-Class: com.example.Main\r\n\r\nName: untrusted.class\r\nImplementation-Title: Wrong\r\n").unwrap();
        let mut second = first.clone();
        second.insert("implementation-version".into(), "2.0".into());
        assert_eq!(
            from_manifest(&first).unwrap().id,
            from_manifest(&second).unwrap().id
        );
        assert_eq!(
            from_manifest(&first).unwrap().metadata.name,
            "Example Editor"
        );
        second.insert("implementation-title".into(), "Other App".into());
        assert_ne!(
            from_manifest(&first).unwrap().id,
            from_manifest(&second).unwrap().id
        );
    }
    #[test]
    fn jar_manifest_is_read_without_extracting_and_distinguishes_versions() {
        use std::io::Write;
        let f = crate::test_support::Fixture::new("");
        let jar = f.csv.with_extension("jar");
        let mut archive = zip::ZipWriter::new(File::create(&jar).unwrap());
        archive
            .start_file("META-INF/MANIFEST.MF", zip::write::FileOptions::default())
            .unwrap();
        archive.write_all(b"Manifest-Version: 1.0\nMain-Class: org.example.Tool\nImplementation-Title: Example Tool\n\n").unwrap();
        archive.finish().unwrap();
        let app = java_identity(&vec![
            "java.exe".into(),
            "-jar".into(),
            jar.to_string_lossy().into_owned(),
        ])
        .unwrap();
        assert_eq!(app.metadata.name, "Example Tool");
        assert!(!app.id.contains("java.exe"));
        std::fs::remove_file(jar).unwrap();
    }
    #[test]
    fn catalog_names_recover_from_csv_without_sidecar() {
        let f = crate::test_support::Fixture::new("");
        let id = "app:java:class:org.example.Editor";
        let row = history::encode_named(
            &chrono::Local::now(),
            id,
            Some(Path::new(r"C:\Java\javaw.exe")),
            0.125,
            Some("Editor"),
        );
        std::fs::write(&f.csv, row).unwrap();
        assert_eq!(load_catalog(&f.csv)[id].name, "Editor");
    }
    #[test]
    #[ignore = "Requires local WMI service; native integration check"]
    fn native_wmi_reads_only_current_process_command() {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        }
        {
            let service = wmi_service().expect("local WMI service");
            let command =
                command_line(&service, std::process::id()).expect("own process command line");
            assert!(!split_command(&command).is_empty());
            assert!(process_stamp(std::process::id()).is_some());
        }
        unsafe {
            CoUninitialize();
        }
    }
}

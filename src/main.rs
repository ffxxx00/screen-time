#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(not(target_os = "windows"))]
compile_error!("Screen Time está diseñado únicamente para Windows 10/11.");

mod app_identity;
mod coverage;
mod daily_summary;
mod data_management;
mod diagnostics;
mod durability;
mod grouping;
mod history;
mod icons;
mod lifecycle;
mod native_identity;
#[cfg(test)]
mod test_support;

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, NaiveDateTime};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    ptr,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};
use tao::{
    dpi::{LogicalSize, PhysicalPosition},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    platform::windows::WindowExtWindows,
    window::{Window, WindowBuilder, WindowId},
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, MouseButton, TrayIconBuilder, TrayIconEvent,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, HWND, LPARAM, LRESULT, WPARAM},
    System::Threading::{CreateMutexW, GetCurrentProcess},
    UI::{
        Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
        Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
        WindowsAndMessaging::{
            CallNextHookEx, GetClassNameW, GetWindowThreadProcessId, SetWindowsHookExW,
            UnhookWindowsHookEx, EVENT_SYSTEM_FOREGROUND, HC_ACTION, WH_KEYBOARD_LL, WH_MOUSE_LL,
            WINEVENT_OUTOFCONTEXT, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
            WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_SYSKEYDOWN,
            WM_SYSKEYUP, WM_XBUTTONDOWN,
        },
    },
};

const IDLE_LIMIT_MS: u32 = 120_000;
const OPEN_ID: &str = "open_dashboard";
const EXIT_ID: &str = "exit_tracker";
const CHART_JS: &str = include_str!("../assets/chart.umd.min.js");

#[link(name = "psapi")]
extern "system" {
    fn EmptyWorkingSet(process: HANDLE) -> i32;
}

fn trim_background_memory() {
    // WebView2 is already dropped. Let Windows reclaim resident pages retained
    // by the allocator and DLLs while the tracker waits in the tray.
    unsafe {
        EmptyWorkingSet(GetCurrentProcess());
    }
}

#[derive(Debug)]
enum AppEvent {
    ApplicationResolved {
        generation: u64,
        hwnd: usize,
        path: PathBuf,
        result: app_identity::Resolved,
    },
    Foreground(usize),
    Input,
    IdleDeadline,
    Checkpoint,
    UiLoadTimeout,
    SystemState {
        paused: bool,
        at: Instant,
        idle: u32,
        ack: mpsc::Sender<()>,
    },
    OpenDashboard,
    DashboardReady,
    SettingsReady,
    IconsLoaded {
        window_id: WindowId,
        generation: u64,
        entries: Vec<(String, Option<PathBuf>, Option<String>)>,
    },
    RefreshDashboard,
    TogglePause,
    OpenDataFolder,
    ExportHistory,
    ClearHistory,
    SaveIconColors(serde_json::Value),
    SetIdlePause(bool),
    SetDarkMode(bool),
    SetAutostart(bool),
    SelectDate(NaiveDate),
    OpenSettings,
    CloseSettings,
    Exit(&'static str),
}

struct Dashboard {
    webview: wry::WebView,
    window: Window,
    id: WindowId,
    geometry: WindowGeometry,
    settings_path: PathBuf,
    settings_key: &'static str,
    icon_cache: BTreeMap<PathBuf, Option<String>>,
    unresolved_icons: std::collections::BTreeSet<String>,
    icons_loading: bool,
    icon_generation: u64,
    icons_enabled: bool,
    ready: bool,
    opened_at: Instant,
    geometry_dirty_at: Option<Instant>,
}

fn close_dashboard(dashboard: &mut Option<Dashboard>) {
    if let Some(mut current) = dashboard.take() {
        current.geometry.capture(&current.window);
        if let Err(error) = save_preference(
            &current.settings_path,
            current.settings_key,
            current.geometry.to_json(),
        ) {
            show_message(&error);
        }
        let Dashboard {
            webview, window, ..
        } = current;
        drop(webview);
        drop(window);
    }
}

#[derive(Clone, Copy)]
struct WindowGeometry {
    width: f64,
    height: f64,
    maximized: bool,
    position: Option<(i32, i32)>,
}
impl WindowGeometry {
    fn load(path: &Path, key: &str, width: f64, height: f64) -> Self {
        let data = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .unwrap_or_default();
        let dimension = |name: &str, fallback: f64| {
            data[key][name]
                .as_f64()
                .filter(|v| v.is_finite() && *v >= 100.0 && *v <= 16000.0)
                .unwrap_or(fallback)
        };
        Self {
            width: dimension("width", width),
            height: dimension("height", height),
            maximized: data[key]["maximized"].as_bool().unwrap_or(false),
            position: data[key]["x"]
                .as_i64()
                .zip(data[key]["y"].as_i64())
                .and_then(|(x, y)| Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?))),
        }
    }
    fn capture(&mut self, window: &Window) {
        if window.is_minimized() {
            return;
        }
        self.maximized = window.is_maximized();
        if !self.maximized {
            if let Ok(position) = window.outer_position() {
                self.position = Some((position.x, position.y));
            }
            let size = window.inner_size().to_logical::<f64>(window.scale_factor());
            if size.width > 0.0 && size.height > 0.0 {
                self.width = size.width;
                self.height = size.height;
            }
        }
    }
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"width": self.width, "height": self.height, "maximized": self.maximized, "x": self.position.map(|p| p.0), "y": self.position.map(|p| p.1)})
    }
    fn restore(&self, window: &Window) {
        if let Some((x, y)) = self.position {
            window.set_outer_position(PhysicalPosition::new(x, y));
        }
        fit_window_to_work_area(window, self.position.is_none());
        if self.maximized {
            window.set_maximized(true);
        }
    }
}

// Windows may place a newly created window partly outside the monitor work area,
// especially when a saved size was recorded on a larger display.
fn fit_window_to_work_area(window: &Window, center: bool) {
    use windows_sys::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::{GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER},
    };
    if window.is_maximized() || window.is_minimized() {
        return;
    }
    unsafe {
        let hwnd = window.hwnd() as HWND;
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        let mut rect: RECT = std::mem::zeroed();
        if monitor.is_null()
            || GetMonitorInfoW(monitor, &mut info) == 0
            || GetWindowRect(hwnd, &mut rect) == 0
        {
            return;
        }
        let work = info.rcWork;
        if let Some((x, y, width, height)) = fit_rect(rect, work, center) {
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
    }
}

fn fit_rect(
    rect: windows_sys::Win32::Foundation::RECT,
    work: windows_sys::Win32::Foundation::RECT,
    center: bool,
) -> Option<(i32, i32, i32, i32)> {
    let width = (rect.right - rect.left).min(work.right - work.left);
    let height = (rect.bottom - rect.top).min(work.bottom - work.top);
    if width <= 0 || height <= 0 {
        return None;
    }
    Some((
        if center {
            work.left + (work.right - work.left - width) / 2
        } else {
            rect.left.clamp(work.left, work.right - width)
        },
        if center {
            work.top + (work.bottom - work.top - height) / 2
        } else {
            rect.top.clamp(work.top, work.bottom - height)
        },
        width,
        height,
    ))
}

fn save_preference(path: &Path, key: &str, value: serde_json::Value) -> Result<(), String> {
    let mut data = match fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)
            .map_err(|e| format!("Configuración inválida: {e}"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(format!("No se pudo leer la configuración: {e}")),
    };
    let object = data
        .as_object_mut()
        .ok_or("La configuración debe ser un objeto JSON.")?;
    object.insert(key.into(), value);
    durability::atomic_write(path, data.to_string().as_bytes())
        .map_err(|e| format!("No se pudo guardar la configuración: {e}"))
}

fn startup_entry(enabled: Option<bool>) -> Result<bool, String> {
    use windows_sys::Win32::System::Registry::*;
    let wide = |text: &str| text.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let key = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let name = wide("Screen Time");
    let result = unsafe {
        match enabled {
            Some(true) => {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                let command = wide(&format!("\"{}\"", exe.display()));
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    key.as_ptr(),
                    name.as_ptr(),
                    REG_SZ,
                    command.as_ptr().cast(),
                    (command.len() * 2) as u32,
                )
            }
            Some(false) => RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()),
            None => {
                let mut length = 0;
                RegGetValueW(
                    HKEY_CURRENT_USER,
                    key.as_ptr(),
                    name.as_ptr(),
                    RRF_RT_REG_SZ,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut length,
                )
            }
        }
    };
    if result == 2 && enabled != Some(true) {
        return Ok(false);
    }
    if result != 0 {
        return Err(format!(
            "No se pudo acceder al inicio de Windows (error {result})."
        ));
    }
    Ok(enabled.unwrap_or(true))
}

static EVENT_PROXY: OnceLock<EventLoopProxy<AppEvent>> = OnceLock::new();
static LAST_INPUT_EVENT_MS: AtomicU64 = AtomicU64::new(0);

struct SingleInstance(HANDLE);

impl SingleInstance {
    fn acquire() -> Result<Self, String> {
        let name: Vec<u16> = "Local\\Screen Time.Daemon\0".encode_utf16().collect();
        let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err("No se pudo crear el mutex de instancia única.".into());
        }
        const ERROR_ALREADY_EXISTS: u32 = 183;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe { CloseHandle(handle) };
            return Err("Screen Time ya está ejecutándose.".into());
        }
        Ok(Self(handle))
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

#[derive(Clone)]
struct Segment {
    since: Instant,
    wall_start: DateTime<Local>,
}

struct Tracker {
    csv_path: PathBuf,
    process: Option<String>,
    segment: Option<Segment>,
    foreground_hwnd: HWND,
    pause_when_idle: bool,
    dark_mode: bool,
    process_paths: BTreeMap<String, PathBuf>,
    applications: BTreeMap<String, app_identity::Metadata>,
    identity_generation: u64,
    grouping: grouping::Grouping,
    journal: durability::Journal,
    pending_rows: VecDeque<Vec<u8>>,
    system_paused: bool,
    manual_paused: bool,
    save_error: Option<String>,
    daily_summary: Option<daily_summary::DailySummary>,
    coverage: coverage::Coverage,
}

impl Tracker {
    fn new(csv_path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = csv_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear el directorio de datos: {e}"))?;
        }
        data_management::finish_reset(&csv_path)?;
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&csv_path)
            .map_err(|e| format!("No se pudo abrir {}: {e}", csv_path.display()))?;
        let pause_when_idle = fs::read(csv_path.with_extension("settings.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| {
                value
                    .get("pause_when_idle")
                    .and_then(|value| value.as_bool())
            })
            .unwrap_or(false);
        let manual_paused = fs::read(csv_path.with_extension("settings.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| value.get("manual_paused").and_then(|value| value.as_bool()))
            .unwrap_or(false);
        let dark_mode = fs::read(csv_path.with_extension("settings.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| value.get("dark_mode").and_then(|value| value.as_bool()))
            .unwrap_or(false);
        let process_paths = fs::read(csv_path.with_extension("paths.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let journal = durability::Journal::open(&csv_path)?;
        let applications = app_identity::load_catalog(&csv_path);
        let mut grouping = grouping::Grouping::load(&csv_path);
        grouping.catalog(&applications);
        let coverage = coverage::Coverage::load(&csv_path).unwrap_or_else(|error| {
            diagnostics::record(&format!("coverage_load_error: {error}"));
            coverage::Coverage::unavailable(&csv_path)
        });
        Ok(Self {
            journal,
            pending_rows: VecDeque::new(),
            system_paused: false,
            manual_paused,
            save_error: None,
            daily_summary: None,
            coverage,
            csv_path,
            process_paths,
            applications,
            identity_generation: 0,
            grouping,
            process: None,
            segment: None,
            foreground_hwnd: ptr::null_mut(),
            pause_when_idle,
            dark_mode,
        })
    }

    fn foreground(&mut self, hwnd: HWND) {
        if self.system_paused || self.manual_paused {
            return;
        }
        if let Some(at) = lifecycle::pause_at() {
            self.flush_until(self.accounting_end(at, idle_ms()));
            return;
        }
        self.mark_observed();
        if hwnd == self.foreground_hwnd && self.process.is_some() {
            return;
        }
        let now = Instant::now();
        let idle = idle_ms();
        if self.pause_when_idle && idle >= IDLE_LIMIT_MS {
            self.flush_until(idle_cutoff(now, idle));
        } else {
            self.flush_until(now);
        }
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
        }
        // Explorer owns both file windows and the desktop/taskbar. Only the former is app use.
        let mut class = [0u16; 128];
        let length = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
        let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
        let path = if is_shell_surface(&class) {
            None
        } else {
            icons::process_path(pid)
        };
        let cached_identity = path.as_ref().and_then(|path| {
            app_identity::cached(hwnd as usize, pid).map(|result| (path.clone(), result))
        });
        self.identity_generation = self.identity_generation.wrapping_add(1);
        if let Some(path) = &path {
            app_identity::request(hwnd as usize, pid, path, self.identity_generation);
        }
        self.process = path.as_deref().map(history::identity);
        if let Some(path) = &path {
            self.grouping.observe(&history::identity(path), path);
        }
        if let (Some(name), Some(path)) = (&self.process, path) {
            let key = name.to_lowercase();
            if self.process_paths.get(&key) != Some(&path) {
                self.process_paths.insert(key, path);
                self.save_process_paths();
            }
        }
        self.foreground_hwnd = hwnd;
        self.segment = if (!self.pause_when_idle || idle < IDLE_LIMIT_MS) && self.process.is_some()
        {
            Some(Segment {
                since: now,
                wall_start: Local::now(),
            })
        } else {
            None
        };
        if let Some((path, result)) = cached_identity {
            self.application_resolved(self.identity_generation, hwnd as usize, path, result);
        }
    }

    fn application_resolved(
        &mut self,
        generation: u64,
        hwnd: usize,
        path: PathBuf,
        mut result: app_identity::Resolved,
    ) {
        if generation != self.identity_generation
            || hwnd != self.foreground_hwnd as usize
            || self.manual_paused
        {
            return;
        }
        native_identity::reconcile(&mut result, &self.applications);
        if result.id == history::identity(&path)
            && result.metadata.name == history::executable_name(&result.id)
        {
            return;
        }
        if self.process.as_deref() == Some(history::identity(&path).as_str()) {
            self.process = Some(result.id.clone());
        }
        self.grouping.observe(&result.id, &path);
        let key = result.id.to_lowercase();
        if self.process_paths.get(&key) != Some(&path) {
            self.process_paths.insert(key, path);
            self.save_process_paths();
        }
        let mut metadata = result.metadata;
        if metadata.icon.is_none() {
            metadata.icon = self
                .applications
                .get(&result.id)
                .and_then(|old| old.icon.clone());
        }
        if self.applications.get(&result.id) != Some(&metadata) {
            self.applications.insert(result.id, metadata);
            self.grouping.catalog(&self.applications);
            if let Ok(bytes) = serde_json::to_vec(&self.applications) {
                if let Err(error) =
                    durability::atomic_write(&self.csv_path.with_extension("apps.json"), &bytes)
                {
                    diagnostics::record(&format!("application_metadata_save_error: {error}"));
                }
            }
        }
    }

    fn save_process_paths(&self) {
        if let Ok(bytes) = serde_json::to_vec(&self.process_paths) {
            if let Err(error) =
                durability::atomic_write(&self.csv_path.with_extension("paths.json"), &bytes)
            {
                show_message(&format!(
                    "No se pudieron guardar las rutas de iconos: {error}"
                ));
            }
        }
    }

    fn input(&mut self) {
        if self.manual_paused || self.system_paused || lifecycle::pause_at().is_some() {
            return;
        }
        // Recover focus changes missed while a native modal loop was active.
        let hwnd = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
        if hwnd != self.foreground_hwnd || self.process.is_none() {
            self.foreground(hwnd);
        }
        let now = Instant::now();
        if self.segment.is_none() {
            if self.process.is_some() {
                self.segment = Some(Segment {
                    since: now,
                    wall_start: Local::now(),
                });
            }
        }
    }

    fn idle_deadline(&mut self) -> Option<u32> {
        if self.manual_paused || self.system_paused || !self.pause_when_idle {
            return None;
        }
        let elapsed = idle_ms();
        if elapsed >= IDLE_LIMIT_MS {
            self.flush_until(idle_cutoff(Instant::now(), elapsed));
            None
        } else {
            Some(IDLE_LIMIT_MS - elapsed)
        }
    }

    fn checkpoint(&mut self) {
        self.retry_pending();
        if self.system_paused || self.manual_paused {
            return;
        }
        if let Some(at) = lifecycle::pause_at() {
            self.flush_until(self.accounting_end(at, idle_ms()));
            return;
        }
        self.mark_observed();
        let now = Instant::now();
        let wall_start = Local::now();
        let idle = idle_ms();
        if self.pause_when_idle && idle >= IDLE_LIMIT_MS {
            self.flush_until(idle_cutoff(now, idle));
        } else {
            self.flush_until(now);
            if self.process.is_some() {
                self.segment = Some(Segment {
                    since: now,
                    wall_start,
                });
            }
        }
    }

    fn mark_observed(&mut self) {
        if !self.coverage.is_available() {
            return;
        }
        if let Err(error) = self.coverage.mark(Local::now().date_naive()) {
            diagnostics::record(&format!("coverage_write_error: {error}"));
        }
    }

    fn set_idle_pause(&mut self, enabled: bool) -> Result<(), String> {
        self.checkpoint();
        save_preference(
            &self.csv_path.with_extension("settings.json"),
            "pause_when_idle",
            enabled.into(),
        )?;
        self.pause_when_idle = enabled;
        // A policy change applies from this instant, never retrospectively to history.
        self.segment = if !self.manual_paused
            && !self.system_paused
            && self.process.is_some()
            && (!enabled || idle_ms() < IDLE_LIMIT_MS)
        {
            Some(Segment {
                since: Instant::now(),
                wall_start: Local::now(),
            })
        } else {
            None
        };
        Ok(())
    }

    fn toggle_pause(&mut self) -> Result<(), String> {
        if self.manual_paused
            && self
                .csv_path
                .with_extension("reset.json")
                .try_exists()
                .map_err(|error| error.to_string())?
        {
            return Err("El borrado está pendiente. Volvé a pulsar Borrar historial o reiniciá la aplicación para completarlo antes de reanudar.".into());
        }
        save_preference(
            &self.csv_path.with_extension("settings.json"),
            "manual_paused",
            (!self.manual_paused).into(),
        )?;
        if self.manual_paused {
            self.manual_paused = false;
            self.foreground_hwnd = ptr::null_mut();
            let hwnd =
                unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
            self.foreground(hwnd);
        } else {
            self.flush_until(self.accounting_end(Instant::now(), idle_ms()));
            self.manual_paused = true;
            self.foreground_hwnd = ptr::null_mut();
        }
        Ok(())
    }

    fn clear_history(&mut self) -> Result<(), String> {
        if !self.manual_paused {
            self.toggle_pause()?;
        }
        self.retry_pending();
        if let Some(error) = &self.save_error {
            return Err(format!(
                "No se borró el historial porque hay un guardado pendiente: {error}"
            ));
        }
        data_management::reset(&self.csv_path)?;
        self.daily_summary = None;
        self.coverage = coverage::Coverage::load(&self.csv_path)?;
        self.process_paths.clear();
        self.applications.clear();
        self.grouping = grouping::Grouping::default();
        self.identity_generation = self.identity_generation.wrapping_add(1);
        self.process = None;
        self.journal = durability::Journal::open(&self.csv_path)?;
        Ok(())
    }

    fn accounting_end(&self, now: Instant, idle: u32) -> Instant {
        let end = if self.pause_when_idle {
            idle_cutoff(now, idle)
        } else {
            now
        };
        lifecycle::pause_at().map(|at| end.min(at)).unwrap_or(end)
    }

    fn flush_until(&mut self, end: Instant) {
        if let (Some(process), Some(segment)) = (self.process.as_ref(), self.segment.take()) {
            let seconds = end.saturating_duration_since(segment.since).as_secs_f64();
            if seconds > 0.0 {
                let path = self.process_paths.get(&process.to_lowercase());
                self.pending_rows.push_back(history::encode_named(
                    &segment.wall_start,
                    process,
                    path.map(PathBuf::as_path),
                    seconds,
                    self.applications
                        .get(process)
                        .map(|metadata| metadata.name.as_str()),
                ));
            }
        }
        self.retry_pending();
    }

    fn retry_pending(&mut self) {
        while let Some(row) = self.pending_rows.front() {
            // Observe outside edits before our own append so the daily index can
            // safely hash only bytes committed by this process on the next sync.
            let index_current = self
                .daily_summary
                .as_mut()
                .is_some_and(|summary| summary.sync(&self.csv_path).is_ok());
            match self.journal.append(row) {
                Ok(()) => {
                    if index_current {
                        if let Some(summary) = self.daily_summary.as_mut() {
                            summary.note_committed_append(row.len());
                        }
                    }
                    self.pending_rows.pop_front();
                    self.save_error = None;
                }
                Err(error) => {
                    self.save_error = Some(error);
                    return;
                }
            }
        }
    }

    fn system_state(&mut self, paused: bool, at: Instant, idle: u32) {
        self.system_state_with_foreground(paused, at, idle, || unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow()
        });
    }

    fn system_state_with_foreground(
        &mut self,
        paused: bool,
        at: Instant,
        idle: u32,
        foreground: impl FnOnce() -> HWND,
    ) {
        if paused {
            self.flush_until(self.accounting_end(at, idle));
            self.system_paused = true;
            self.foreground_hwnd = ptr::null_mut();
        } else if self.system_paused {
            lifecycle::clear_pause();
            self.system_paused = false;
            self.foreground_hwnd = ptr::null_mut();
            self.foreground(foreground());
        }
    }

    #[cfg(test)]
    fn totals_at(&self, end: Instant) -> Result<Vec<(String, f64)>, String> {
        let mut totals: BTreeMap<String, f64> =
            load_chart_data(&self.csv_path)?.into_iter().collect();
        if let (Some(name), Some(segment)) = (&self.process, &self.segment) {
            *totals.entry(name.clone()).or_default() +=
                end.saturating_duration_since(segment.since).as_secs_f64() / 60.0;
        }
        let mut rows: Vec<_> = totals.into_iter().collect();
        rows.sort_by(|a, b| b.1.total_cmp(&a.1));
        Ok(rows)
    }

    fn set_dark_mode(&mut self, enabled: bool) -> Result<(), String> {
        save_preference(
            &self.csv_path.with_extension("settings.json"),
            "dark_mode",
            enabled.into(),
        )?;
        self.dark_mode = enabled;
        Ok(())
    }

    fn snapshot(&mut self, date: NaiveDate) -> Result<serde_json::Value, String> {
        if let Some(error) = &self.save_error {
            return Err(format!("Guardado pendiente; se reintentará: {error}"));
        }
        let idle = idle_ms();
        let end = self.accounting_end(Instant::now(), idle);
        if self.daily_summary.is_none() {
            self.daily_summary = Some(daily_summary::DailySummary::load(&self.csv_path)?);
        }
        let summary = self.daily_summary.as_mut().unwrap();
        summary.sync(&self.csv_path)?;
        let today = Local::now().date_naive();
        let mut usage = summary.day(date);
        let mut first_date = summary.first_date().unwrap_or(today);
        if let Some(first_observed) = self.coverage.first_date() {
            first_date = first_date.min(first_observed);
        }
        if let (Some(name), Some(segment)) = (&self.process, &self.segment) {
            first_date = first_date.min(segment.wall_start.date_naive());
            add_daily_segment(
                &mut usage,
                date,
                segment.wall_start.naive_local(),
                name,
                end.saturating_duration_since(segment.since).as_secs_f64(),
            );
        }
        let usage = self.grouping.group(usage);
        let week_start = date
            .checked_sub_days(Days::new(date.weekday().num_days_from_monday() as u64))
            .unwrap();
        let week_end = week_start.checked_add_days(Days::new(6)).unwrap();
        let mut week_apps: BTreeMap<String, [f64; 7]> = BTreeMap::new();
        let mut week_days = Vec::with_capacity(7);
        let mut daily_totals = [0.0; 7];
        let mut measured_days = [false; 7];
        for (index, total) in daily_totals.iter_mut().enumerate() {
            let day = week_start
                .checked_add_days(Days::new(index as u64))
                .unwrap();
            week_days.push(day.to_string());
            if day > today {
                continue;
            }
            let day_usage = if day == date {
                usage.clone()
            } else {
                let mut day_usage = summary.day(day);
                if let (Some(name), Some(segment)) = (&self.process, &self.segment) {
                    add_daily_segment(
                        &mut day_usage,
                        day,
                        segment.wall_start.naive_local(),
                        name,
                        end.saturating_duration_since(segment.since).as_secs_f64(),
                    );
                }
                self.grouping.group(day_usage)
            };
            measured_days[index] = self.coverage.contains(day) || !day_usage.is_empty();
            for (name, hours) in day_usage {
                let minutes = hours.iter().sum::<f64>() / 60.0;
                week_apps.entry(name).or_insert([0.0; 7])[index] += minutes;
                *total += minutes;
            }
        }
        let mut week_rows: Vec<_> = week_apps.into_iter().collect();
        week_rows.sort_by(|a, b| b.1.iter().sum::<f64>().total_cmp(&a.1.iter().sum::<f64>()));
        let average_start = week_start.max(first_date);
        let average_end = week_end.min(today);
        let average_days = if average_end >= average_start {
            (average_end - average_start).num_days() as usize + 1
        } else {
            0
        };
        let week_total = daily_totals.iter().sum::<f64>();
        let previous_start = week_start.checked_sub_days(Days::new(7)).unwrap();
        let previous_end = week_start.pred_opt().unwrap();
        let previous_average_start = previous_start.max(first_date);
        let previous_days = if previous_end >= previous_average_start {
            (previous_end - previous_average_start).num_days() as usize + 1
        } else {
            0
        };
        let previous_total: f64 = (0..7)
            .map(|i| {
                let day = previous_start.checked_add_days(Days::new(i)).unwrap();
                let mut day_usage = summary.day(day);
                if let (Some(name), Some(segment)) = (&self.process, &self.segment) {
                    add_daily_segment(
                        &mut day_usage,
                        day,
                        segment.wall_start.naive_local(),
                        name,
                        end.saturating_duration_since(segment.since).as_secs_f64(),
                    );
                }
                day_usage
                    .values()
                    .map(|hours| hours.iter().sum::<f64>() / 60.0)
                    .sum::<f64>()
            })
            .sum();
        let mut rows: Vec<_> = usage.into_iter().collect();
        rows.sort_by(|a, b| b.1.iter().sum::<f64>().total_cmp(&a.1.iter().sum::<f64>()));
        let mut applications = history::applications(
            rows.iter()
                .map(|row| row.0.as_str())
                .chain(week_rows.iter().map(|row| row.0.as_str())),
        );
        if let Some(values) = applications.as_object_mut() {
            for (id, metadata) in self.grouping.presentation(&self.applications) {
                if let Some(value) = values.get_mut(&id) {
                    value["name"] = metadata.name.into();
                    if !metadata.name.to_ascii_lowercase().ends_with(".exe") {
                        value["displayName"] = metadata.name.into();
                    }
                }
            }
        }
        Ok(serde_json::json!({
            "applications": applications,
            "labels": rows.iter().map(|row| &row.0).collect::<Vec<_>>(),
            "minutes": rows.iter().map(|row| row.1.iter().sum::<f64>() / 60.0).collect::<Vec<_>>(),
            "hourly": rows.iter().map(|row| row.1.map(|seconds| seconds / 60.0)).collect::<Vec<_>>(),
            "week": {
                "start": week_start.to_string(), "end": week_end.to_string(), "days": week_days,
                "dailyTotals": daily_totals, "totalMinutes": week_total,
                "measuredDays": measured_days,
                "averageMinutes": if average_days > 0 { week_total / average_days as f64 } else { 0.0 },
                "averageDays": average_days,
                "previousAverageMinutes": if previous_days > 0 { Some(previous_total / previous_days as f64) } else { None },
                "labels": week_rows.iter().map(|row| &row.0).collect::<Vec<_>>(),
                "minutes": week_rows.iter().map(|row| row.1.iter().sum::<f64>()).collect::<Vec<_>>(),
                "daily": week_rows.iter().map(|row| row.1).collect::<Vec<_>>()
            },
            "date": date.to_string(), "today": today.to_string(), "firstDate": first_date.to_string(),
            "measured": self.coverage.contains(date) || !rows.is_empty(),
            "trackingState": if self.manual_paused { "manual" } else if self.system_paused || lifecycle::pause_at().is_some() { "system" } else if self.pause_when_idle && idle >= IDLE_LIMIT_MS { "idle" } else if self.segment.is_some() { "active" } else { "waiting" },
            "pauseWhenIdle": self.pause_when_idle,
            "darkMode": self.dark_mode,
            "autostart": startup_entry(None)?,
            "updated": Local::now().format("%H:%M:%S").to_string()
        }))
    }
}

// CSV timestamps are local wall time. Clip to the selected day, then intersect
// each hour so a segment crossing midnight contributes to both calendar days.
fn add_daily_segment(
    usage: &mut BTreeMap<String, [f64; 24]>,
    date: NaiveDate,
    start: NaiveDateTime,
    name: &str,
    seconds: f64,
) {
    if !seconds.is_finite() || seconds <= 0.0 {
        return;
    }
    let midnight = date.and_hms_opt(0, 0, 0).unwrap();
    let distance = start.signed_duration_since(midnight);
    let offset = distance
        .num_microseconds()
        .map(|value| value as f64 / 1_000_000.0)
        .unwrap_or_else(|| distance.num_seconds() as f64);
    let begin = offset.max(0.0);
    let finish = (offset + seconds).min(86400.0);
    if finish <= begin {
        return;
    }
    let hours = usage.entry(name.to_owned()).or_insert([0.0; 24]);
    for (hour, total) in hours.iter_mut().enumerate() {
        *total +=
            (finish.min((hour + 1) as f64 * 3600.0) - begin.max(hour as f64 * 3600.0)).max(0.0);
    }
}

fn data_file() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(Path::to_path_buf))
        })
        .ok_or_else(|| "No se pudo determinar el directorio local de datos.".to_string())?;
    Ok(base.join("Screen Time").join("app_usage.csv"))
}

fn is_shell_surface(class: &str) -> bool {
    matches!(
        class,
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

fn idle_ms() -> u32 {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return 0;
    }
    let tick = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount() };
    tick.wrapping_sub(info.dwTime)
}

fn idle_cutoff(now: Instant, idle: u32) -> Instant {
    now.checked_sub(Duration::from_millis(
        idle.saturating_sub(IDLE_LIMIT_MS) as u64
    ))
    .unwrap_or(now)
}

unsafe extern "system" fn foreground_callback(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND {
        if let Some(proxy) = EVENT_PROXY.get() {
            let _ = proxy.send_event(AppEvent::Foreground(hwnd as usize));
        }
    }
}

unsafe extern "system" fn keyboard_callback(code: i32, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32
        && matches!(
            wparam as u32,
            WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP
        )
    {
        notify_input();
    }
    CallNextHookEx(ptr::null_mut(), code, wparam, _lparam)
}

unsafe extern "system" fn mouse_callback(code: i32, wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    let message = wparam as u32;
    if code == HC_ACTION as i32
        && matches!(
            message,
            WM_MOUSEMOVE
                | WM_MOUSEWHEEL
                | WM_MOUSEHWHEEL
                | WM_LBUTTONDOWN
                | WM_RBUTTONDOWN
                | WM_MBUTTONDOWN
                | WM_XBUTTONDOWN
        )
    {
        notify_input();
    }
    CallNextHookEx(ptr::null_mut(), code, wparam, _lparam)
}

fn notify_input() {
    let now = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
    let previous = LAST_INPUT_EVENT_MS.load(Ordering::Relaxed);
    if now.saturating_sub(previous) < 300 {
        return;
    }
    LAST_INPUT_EVENT_MS.store(now, Ordering::Relaxed);
    if let Some(proxy) = EVENT_PROXY.get() {
        let _ = proxy.send_event(AppEvent::Input);
    }
}

fn start_deadline_worker(proxy: EventLoopProxy<AppEvent>) -> mpsc::Sender<u32> {
    let (tx, rx) = mpsc::channel::<u32>();
    thread::Builder::new()
        .name("idle-deadline".into())
        .spawn(move || {
            while let Ok(milliseconds) = rx.recv() {
                match rx.recv_timeout(Duration::from_millis(milliseconds as u64)) {
                    Ok(next) => {
                        // The latest activity or focus notification restarts the one-shot deadline.
                        let mut deadline = next;
                        loop {
                            match rx.recv_timeout(Duration::from_millis(deadline as u64)) {
                                Ok(next) => deadline = next,
                                Err(mpsc::RecvTimeoutError::Timeout) => {
                                    let _ = proxy.send_event(AppEvent::IdleDeadline);
                                    break;
                                }
                                Err(mpsc::RecvTimeoutError::Disconnected) => return,
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        let _ = proxy.send_event(AppEvent::IdleDeadline);
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
        })
        .expect("No se pudo iniciar el temporizador de inactividad");
    tx
}

#[cfg(test)]
fn load_chart_data(path: &Path) -> Result<Vec<(String, f64)>, String> {
    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|e| format!("No se pudo leer el CSV: {e}"))?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(file);
    let mut totals = BTreeMap::<String, f64>::new();
    for row in reader.records() {
        let row = row.map_err(|e| format!("El CSV contiene un registro inválido: {e}"))?;
        if let Some(usage) = history::parse(&row) {
            *totals.entry(usage.identity).or_default() += usage.seconds;
        }
    }
    let mut entries: Vec<_> = totals
        .into_iter()
        .map(|(name, seconds)| (name, seconds / 60.0))
        .collect();
    entries.sort_by(|a, b| b.1.total_cmp(&a.1));
    Ok(entries)
}

fn dashboard_html(dark_mode: bool) -> String {
    include_str!("../assets/dashboard.html")
        .replace("__DARK__", if dark_mode { "dark" } else { "light" })
        .replace("__CSS__", include_str!("../assets/dashboard.css"))
        .replace("__VIEW_STATE_JS__", include_str!("../assets/view-state.js"))
        .replace("__CHART_JS__", CHART_JS)
        .replace("__DASHBOARD_JS__", include_str!("../assets/dashboard.js"))
}
fn handle_ipc(body: &str) {
    let Some(proxy) = EVENT_PROXY.get() else {
        return;
    };
    let event = match body {
        "toggle-pause" => Some(AppEvent::TogglePause),
        "open-data-folder" => Some(AppEvent::OpenDataFolder),
        "export-history" => Some(AppEvent::ExportHistory),
        "clear-history" => Some(AppEvent::ClearHistory),
        "refresh" => Some(AppEvent::RefreshDashboard),
        "dashboard-ready" => Some(AppEvent::DashboardReady),
        "settings-ready" => Some(AppEvent::SettingsReady),
        "open-settings" => Some(AppEvent::OpenSettings),
        "close-settings" => Some(AppEvent::CloseSettings),
        _ => serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|message| match message.get("command")?.as_str()? {
                "save-icon-colors" => {
                    let colors = message.get("colors")?.as_array()?;
                    if colors.len() > 512
                        || !colors.iter().all(|entry| {
                            let Some(pair) = entry.as_array() else {
                                return false;
                            };
                            pair.len() == 2
                                && pair[0].as_str().is_some_and(|name| name.len() <= 32768)
                                && pair[1].as_str().is_some_and(|color| {
                                    color.len() == 7
                                        && color.starts_with('#')
                                        && color[1..].bytes().all(|c| c.is_ascii_hexdigit())
                                })
                        })
                    {
                        return None;
                    }
                    Some(AppEvent::SaveIconColors(serde_json::Value::Array(
                        colors.clone(),
                    )))
                }
                "set-idle-pause" => {
                    Some(AppEvent::SetIdlePause(message.get("enabled")?.as_bool()?))
                }
                "set-dark-mode" => Some(AppEvent::SetDarkMode(message.get("enabled")?.as_bool()?)),
                "set-autostart" => Some(AppEvent::SetAutostart(message.get("enabled")?.as_bool()?)),
                "select-date" => {
                    NaiveDate::parse_from_str(message.get("date")?.as_str()?, "%Y-%m-%d")
                        .ok()
                        .map(AppEvent::SelectDate)
                }
                _ => None,
            }),
    };
    if let Some(event) = event {
        let _ = proxy.send_event(event);
    }
}
fn make_icon() -> Icon {
    Icon::from_rgba(include_bytes!("../assets/app-icon.rgba").to_vec(), 32, 32)
        .expect("icono RGBA válido")
}

fn window_icon() -> tao::window::Icon {
    tao::window::Icon::from_rgba(include_bytes!("../assets/app-icon.rgba").to_vec(), 32, 32)
        .expect("icono de ventana válido")
}

fn open_folder(folder: &Path) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(folder)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("No se pudo abrir la carpeta: {error}"))
}

fn confirm_clear_history() -> bool {
    let message: Vec<u16> = "¿Borrar todo el historial de esta instalación? Se eliminan el historial, las fechas de seguimiento y las ubicaciones de aplicaciones guardadas. Se conservan preferencias, exportaciones y copias de seguridad del instalador. El seguimiento quedará pausado. Esta acción no se puede deshacer.\0".encode_utf16().collect();
    let title: Vec<u16> = "Borrar historial\0".encode_utf16().collect();
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            0x04 | 0x30 | 0x100,
        ) == 6
    }
}

fn show_message(message: &str) {
    let wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    let title: Vec<u16> = "Screen Time\0".encode_utf16().collect();
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            ptr::null_mut(),
            wide.as_ptr(),
            title.as_ptr(),
            0x10,
        );
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        if let Some(location) = info.location() {
            diagnostics::record(&format!("panic at {}:{}", location.file(), location.line()));
        } else {
            diagnostics::record("panic");
        }
    }));
    if let Err(error) = run() {
        diagnostics::record("startup_error");
        show_message(&error);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_geometry_roundtrips_and_accepts_older_preferences() {
        let fixture = super::test_support::Fixture::new("");
        let path = fixture.root.join("settings.json");
        let old = super::WindowGeometry::load(&path, "dashboard_window", 780.0, 820.0);
        assert_eq!(old.position, None);
        let geometry = super::WindowGeometry {
            width: 700.0,
            height: 600.0,
            maximized: true,
            position: Some((-1300, 42)),
        };
        super::save_preference(&path, "dashboard_window", geometry.to_json()).unwrap();
        let loaded = super::WindowGeometry::load(&path, "dashboard_window", 780.0, 820.0);
        assert_eq!(loaded.position, Some((-1300, 42)));
        assert_eq!(
            (loaded.width, loaded.height, loaded.maximized),
            (700.0, 600.0, true)
        );
    }
    #[test]
    fn window_fits_work_area_including_negative_monitor_coordinates() {
        use windows_sys::Win32::Foundation::RECT;
        let work = RECT {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1040,
        };
        let rect = RECT {
            left: -500,
            top: 300,
            right: 300,
            bottom: 1200,
        };
        assert_eq!(
            super::fit_rect(rect, work, false),
            Some((-800, 140, 800, 900))
        );
        assert_eq!(
            super::fit_rect(rect, work, true),
            Some((-1360, 70, 800, 900))
        );
        let oversized = RECT {
            left: -2000,
            top: -50,
            right: 1000,
            bottom: 1500,
        };
        assert_eq!(
            super::fit_rect(oversized, work, true),
            Some((-1920, 0, 1920, 1040))
        );
    }
    use super::*;

    struct TestFile(PathBuf);
    impl TestFile {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "horarios-regression-{}-{name}.csv",
                std::process::id()
            ));
            fs::write(&path, "2026-09-24 10:00:00,example.exe,60\n").unwrap();
            Self(path)
        }
    }
    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    fn segment(start: Instant) -> Segment {
        Segment {
            since: start,
            wall_start: Local::now(),
        }
    }

    #[test]
    fn snapshot_unifies_legacy_path_and_profile_for_day_and_week() {
        use chrono::TimeZone;
        let fixture = test_support::Fixture::new("");
        let date = Local::now().date_naive();
        let start = Local
            .from_local_datetime(&date.and_hms_opt(10, 0, 0).unwrap())
            .single()
            .unwrap();
        let path = Path::new(r"C:\Apps\browser.exe");
        let mut bytes = format!("{} 10:00:00,Browser.exe,60\n", date).into_bytes();
        bytes.extend(history::encode(
            &start,
            &history::identity(path),
            Some(path),
            120.0,
        ));
        bytes.extend(history::encode_named(
            &start,
            "app:windows:Browser.Profile1",
            Some(path),
            180.0,
            Some("Personal profile"),
        ));
        fs::write(&fixture.csv, &bytes).unwrap();
        let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
        let snapshot = tracker.snapshot(date).unwrap();
        assert_eq!(snapshot["labels"].as_array().unwrap().len(), 1);
        assert_eq!(snapshot["minutes"][0], 6.0);
        assert_eq!(snapshot["hourly"][0][10], 6.0);
        assert_eq!(snapshot["week"]["labels"].as_array().unwrap().len(), 1);
        assert_eq!(snapshot["week"]["minutes"][0], 6.0);
        assert_eq!(snapshot["week"]["totalMinutes"], 6.0);
        assert_eq!(fs::read(&fixture.csv).unwrap(), bytes);
        assert_eq!(
            snapshot["applications"][history::identity(path)]["name"],
            "browser.exe"
        );
    }
    #[test]
    fn updates_and_java_names_are_repaired_in_day_week_and_rebuilt_index() {
        use chrono::TimeZone;
        let fixture = test_support::Fixture::new("");
        let date = Local::now().date_naive();
        let start = Local
            .from_local_datetime(&date.and_hms_opt(10, 0, 0).unwrap())
            .single()
            .unwrap();
        let rows = [
            (
                "app:native:one",
                r"C:\Program Files\WindowsApps\Example.Chat_1.2.3.0_x64__publisher\app\chat.exe",
                "chat.exe",
            ),
            (
                "app:native:two",
                r"C:\Program Files\WindowsApps\Example.Chat_2.0.0.0_x64__publisher\app\chat.exe",
                "chat.exe",
            ),
            (
                "app:native:three",
                r"C:\Local\Discord\app-1.0.1\Discord.exe",
                "discord.exe",
            ),
            (
                "app:native:four",
                r"C:\Local\Discord\app-1.0.2\Discord.exe",
                "discord.exe",
            ),
            (
                "app:java:class:org.example.PaintStudio",
                r"C:\Java\javaw.exe",
                "org.example.PaintStudio",
            ),
            (
                "app:java:manifest:[\"\",\"\",\"org.example.ChartStudio\"]",
                r"C:\Java\javaw.exe",
                "org.example.ChartStudio",
            ),
        ];
        let mut bytes = Vec::new();
        for (id, path, name) in rows {
            bytes.extend(history::encode_named(
                &start,
                id,
                Some(Path::new(path)),
                60.0,
                Some(name),
            ));
        }
        fs::write(&fixture.csv, &bytes).unwrap();
        for rebuild in [false, true] {
            if rebuild {
                fs::remove_file(fixture.csv.with_extension("daily.json")).unwrap();
            }
            let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
            let snapshot = tracker.snapshot(date).unwrap();
            assert_eq!(snapshot["labels"].as_array().unwrap().len(), 4);
            assert_eq!(snapshot["week"]["labels"].as_array().unwrap().len(), 4);
            assert_eq!(snapshot["minutes"][0], 2.0);
            assert_eq!(snapshot["minutes"][1], 2.0);
            assert_eq!(snapshot["week"]["totalMinutes"], 6.0);
            assert_eq!(
                snapshot["applications"][rows[4].0]["displayName"],
                "Paint Studio"
            );
            assert_eq!(
                snapshot["applications"][rows[5].0]["displayName"],
                "Chart Studio"
            );
            assert_eq!(fs::read(&fixture.csv).unwrap(), bytes);
        }
    }
    #[test]
    fn native_identity_survives_move_update_restart_and_groups_history() {
        use chrono::TimeZone;
        let fixture = test_support::Fixture::new("");
        let date = Local::now().date_naive();
        let start = Local
            .from_local_datetime(&date.and_hms_opt(10, 0, 0).unwrap())
            .single()
            .unwrap();
        let old = fixture.root.join("program.exe");
        let moved = fixture.root.join("moved.exe");
        fs::write(&old, b"synthetic executable version one").unwrap();
        let bytes = history::encode(&start, &history::identity(&old), Some(&old), 60.0);
        fs::write(&fixture.csv, &bytes).unwrap();
        let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
        let resolve = |tracker: &mut Tracker, path: &Path| {
            let (id, evidence) = native_identity::identify(path).unwrap();
            tracker.process = Some(history::identity(path));
            tracker.application_resolved(
                0,
                0,
                path.into(),
                app_identity::Resolved {
                    id,
                    metadata: app_identity::Metadata {
                        name: "Program".into(),
                        icon: None,
                        native: Some(evidence),
                    },
                },
            );
            tracker.process.clone().unwrap()
        };
        let id = resolve(&mut tracker, &old);
        fs::rename(&old, &moved).unwrap();
        let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
        assert_eq!(resolve(&mut tracker, &moved), id);
        fs::write(&moved, b"synthetic updated version two, different content").unwrap();
        assert_eq!(resolve(&mut tracker, &moved), id);
        let mut combined = bytes;
        combined.extend(history::encode_named(
            &start,
            &id,
            Some(&moved),
            120.0,
            Some("Program"),
        ));
        fs::write(&fixture.csv, &combined).unwrap();
        let mut rebuilt = Tracker::new(fixture.csv.clone()).unwrap();
        let snapshot = rebuilt.snapshot(date).unwrap();
        assert_eq!(snapshot["labels"], serde_json::json!([id]));
        assert_eq!(snapshot["minutes"][0], 3.0);
        assert_eq!(snapshot["week"]["minutes"][0], 3.0);
        assert_eq!(
            rebuilt.applications[&id]
                .native
                .as_ref()
                .unwrap()
                .fingerprints
                .len(),
            2
        );
        assert_eq!(fs::read(&fixture.csv).unwrap(), combined);
        rebuilt.clear_history().unwrap();
        assert!(rebuilt.applications.is_empty());
        assert!(!fixture.csv.with_extension("apps.json").exists());
    }

    #[test]
    fn resolved_application_preserves_segment_and_rejects_stale_results() {
        let fixture = test_support::Fixture::new("");
        let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
        let path = PathBuf::from(r"C:\Java17\javaw.exe");
        tracker.process = Some(history::identity(&path));
        tracker.identity_generation = 2;
        let start = Instant::now();
        tracker.segment = Some(segment(start));
        let result = app_identity::Resolved {
            id: "app:java:class:org.example.Editor".into(),
            metadata: app_identity::Metadata {
                name: "Example Editor".into(),
                icon: None,
                native: None,
            },
        };
        tracker.application_resolved(1, 0, path.clone(), result.clone());
        assert_eq!(
            tracker.process.as_deref(),
            Some(history::identity(&path).as_str())
        );
        tracker.application_resolved(2, 0, path, result.clone());
        assert_eq!(tracker.process.as_deref(), Some(result.id.as_str()));
        tracker.flush_until(start + Duration::from_millis(750));
        let rows = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(&fixture.csv)
            .unwrap()
            .records()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(history::parse(&rows[0]).unwrap().seconds, 0.75);
        assert_eq!(&rows[0][6], "Example Editor");
        let rebuilt = Tracker::new(fixture.csv.clone()).unwrap();
        assert_eq!(rebuilt.applications[&result.id].name, "Example Editor");
        tracker.clear_history().unwrap();
        tracker.application_resolved(2, 0, PathBuf::from("ignored"), result);
        assert!(tracker.applications.is_empty());
        assert!(!fixture.csv.with_extension("apps.json").exists());
    }

    #[test]
    fn desktop_and_taskbar_are_excluded_but_file_explorer_is_counted() {
        for class in [
            "Progman",
            "WorkerW",
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
        ] {
            assert!(is_shell_surface(class));
        }
        for class in ["CabinetWClass", "ExploreWClass", "Chrome_WidgetWin_1"] {
            assert!(!is_shell_surface(class));
        }
    }

    #[test]
    fn previous_week_average_uses_calendar_days_and_missing_history_is_null() {
        let file = TestFile::new("previous-week");
        fs::write(
            &file.0,
            "2026-09-14 10:00:00,example.exe,4200\n2026-09-21 10:00:00,example.exe,8400\n",
        )
        .unwrap();
        let mut tracker = Tracker::new(file.0.clone()).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
        let snapshot = tracker.snapshot(date).unwrap();
        assert_eq!(snapshot["week"]["previousAverageMinutes"], 10.0);
        assert_eq!(snapshot["week"]["averageMinutes"], 20.0);
        let first = tracker
            .snapshot(date.checked_sub_days(Days::new(7)).unwrap())
            .unwrap();
        assert!(first["week"]["previousAverageMinutes"].is_null());
    }

    #[test]
    fn short_switches_preserve_fractional_seconds_and_legacy_records() {
        let file = TestFile::new("fractions");
        let mut tracker = Tracker::new(file.0.clone()).unwrap();
        tracker.process = Some("example.exe".into());
        let start = Instant::now();
        tracker.segment = Some(segment(start));
        tracker.flush_until(start + Duration::from_millis(250));
        tracker.segment = Some(segment(start));
        tracker.flush_until(start + Duration::from_millis(750));
        assert!((load_chart_data(&file.0).unwrap()[0].1 * 60.0 - 61.0).abs() < 0.000001);
    }

    #[test]
    fn minute_checkpoint_saves_elapsed_time_and_starts_next_segment() {
        let file = TestFile::new("minute-checkpoint");
        let mut tracker = Tracker::new(file.0.clone()).unwrap();
        tracker.process = Some("example.exe".into());
        tracker.segment = Some(segment(Instant::now() - Duration::from_secs(60)));
        tracker.checkpoint();
        let recorded = load_chart_data(&file.0).unwrap()[0].1 * 60.0;
        assert!((120.0..122.0).contains(&recorded));
        assert!(tracker.segment.is_some());
        assert!(tracker.pending_rows.is_empty());
    }

    #[test]
    fn foreground_change_saves_active_segment_before_minute_checkpoint() {
        let file = TestFile::new("foreground-flush");
        let mut tracker = Tracker::new(file.0.clone()).unwrap();
        tracker.process = Some("example.exe".into());
        tracker.foreground_hwnd = 1usize as HWND;
        tracker.segment = Some(segment(Instant::now() - Duration::from_secs(10)));
        tracker.foreground(ptr::null_mut());
        let recorded = load_chart_data(&file.0).unwrap()[0].1 * 60.0;
        assert!((70.0..72.0).contains(&recorded));
        assert!(tracker.segment.is_none());
        assert!(tracker.pending_rows.is_empty());
    }

    #[test]
    fn live_snapshots_include_active_time_without_double_counting_or_writes() {
        let file = TestFile::new("snapshot");
        let mut tracker = Tracker::new(file.0.clone()).unwrap();
        tracker.process = Some("example.exe".into());
        let start = Instant::now();
        tracker.segment = Some(segment(start));
        let original = fs::read(&file.0).unwrap();
        for _ in 0..2 {
            assert!(
                (tracker
                    .totals_at(start + Duration::from_millis(30250))
                    .unwrap()[0]
                    .1
                    * 60.0
                    - 90.25)
                    .abs()
                    < 0.000001
            );
        }
        assert_eq!(fs::read(&file.0).unwrap(), original);
        tracker.flush_until(start + Duration::from_millis(30250));
        assert!(
            (tracker.totals_at(start + Duration::from_secs(40)).unwrap()[0].1 * 60.0 - 90.25).abs()
                < 0.000001
        );
    }

    #[test]
    fn idle_cutoff_excludes_time_after_two_minutes() {
        let start = Instant::now();
        let end = idle_cutoff(start + Duration::from_secs(600), 600_000);
        assert_eq!(end.duration_since(start), Duration::from_secs(120));
    }

    #[test]
    fn ten_minutes_waiting_count_in_full_unless_idle_pause_is_enabled() {
        for (pause, expected_seconds, name) in
            [(false, 600.0, "waiting"), (true, 120.0, "optional-idle")]
        {
            let file = TestFile::new(name);
            let mut tracker = Tracker::new(file.0.clone()).unwrap();
            assert!(!tracker.pause_when_idle, "Waiting must count by default");
            tracker.pause_when_idle = pause;
            tracker.process = Some("MundoGaturro.exe".into());
            let start = Instant::now();
            tracker.segment = Some(segment(start));
            let end = tracker.accounting_end(start + Duration::from_secs(600), 600_000);
            let live = tracker.totals_at(end).unwrap();
            assert!(
                (live
                    .iter()
                    .find(|row| row.0 == "MundoGaturro.exe")
                    .unwrap()
                    .1
                    * 60.0
                    - expected_seconds)
                    .abs()
                    < 0.000001
            );
            tracker.flush_until(end);
            let saved = load_chart_data(&file.0).unwrap();
            assert!(
                (saved
                    .iter()
                    .find(|row| row.0 == "MundoGaturro.exe")
                    .unwrap()
                    .1
                    * 60.0
                    - expected_seconds)
                    .abs()
                    < 0.000001
            );
        }
    }

    #[test]
    fn cached_snapshot_adds_live_segment_without_persisting_it() {
        let f = test_support::Fixture::new(&format!(
            "{} 00:00:00,example.exe,60\n",
            Local::now().date_naive()
        ));
        let mut tracker = Tracker::new(f.csv.clone()).unwrap();
        tracker.process = Some("example.exe".into());
        tracker.segment = Some(Segment {
            since: Instant::now() - Duration::from_secs(30),
            wall_start: Local::now(),
        });
        let before = fs::read(&f.csv).unwrap();
        let data = tracker.snapshot(Local::now().date_naive()).unwrap();
        let seconds = data["minutes"][0].as_f64().unwrap() * 60.0;
        assert!((90.0..92.0).contains(&seconds));
        assert_eq!(fs::read(&f.csv).unwrap(), before);
        let saved = daily_summary::DailySummary::load(&f.csv).unwrap();
        assert_eq!(
            saved.day(Local::now().date_naive())["example.exe"]
                .iter()
                .sum::<f64>(),
            60.0
        );
    }

    #[test]
    fn weekly_snapshot_uses_seven_days_and_ranks_apps_across_the_week() {
        let today = Local::now().date_naive();
        let monday = today
            .checked_sub_days(Days::new(today.weekday().num_days_from_monday() as u64 + 7))
            .unwrap();
        let tuesday = monday.checked_add_days(Days::new(1)).unwrap();
        let sunday = monday.checked_add_days(Days::new(6)).unwrap();
        let fixture = test_support::Fixture::new(&format!(
            "{monday} 10:00:00,game.exe,3600\n{tuesday} 10:00:00,work.exe,7200\n{sunday} 10:00:00,game.exe,1800\n"
        ));
        let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
        let data = tracker.snapshot(monday).unwrap();
        let week = &data["week"];
        assert_eq!(week["start"], monday.to_string());
        assert_eq!(week["end"], sunday.to_string());
        assert_eq!(week["averageDays"], 7);
        assert_eq!(week["totalMinutes"], 210.0);
        assert_eq!(week["averageMinutes"], 30.0);
        assert_eq!(
            week["measuredDays"],
            serde_json::json!([true, true, false, false, false, false, true])
        );
        assert_eq!(week["labels"][0], "work.exe");
        assert_eq!(week["dailyTotals"][0], 60.0);
        assert_eq!(week["dailyTotals"][1], 120.0);
        assert_eq!(week["dailyTotals"][6], 30.0);
    }

    #[test]
    fn installed_days_without_data_count_as_zero_while_coverage_remains_distinct() {
        let today = Local::now().date_naive();
        let monday = today
            .checked_sub_days(Days::new(today.weekday().num_days_from_monday() as u64 + 7))
            .unwrap();
        let tuesday = monday.checked_add_days(Days::new(1)).unwrap();
        let wednesday = monday.checked_add_days(Days::new(2)).unwrap();
        let friday = monday.checked_add_days(Days::new(4)).unwrap();
        let fixture = test_support::Fixture::new(&format!("{wednesday} 10:00:00,work.exe,3600\n"));
        fs::write(
            fixture.csv.with_extension("coverage.json"),
            format!(r#"{{"days":[],"startDate":"{monday}"}}"#),
        )
        .unwrap();
        let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
        tracker.coverage.mark(friday).unwrap();
        drop(tracker);
        let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
        let zero = tracker.snapshot(friday).unwrap();
        assert_eq!(zero["measured"], true);
        assert!(zero["labels"].as_array().unwrap().is_empty());
        let missing = tracker.snapshot(tuesday).unwrap();
        assert_eq!(missing["measured"], false);
        assert_eq!(missing["firstDate"], monday.to_string());
        assert_eq!(
            missing["week"]["measuredDays"],
            serde_json::json!([false, false, true, false, true, false, false])
        );
        assert_eq!(missing["week"]["averageDays"], 7);
        assert!((missing["week"]["averageMinutes"].as_f64().unwrap() - 60.0 / 7.0).abs() < 1e-9);
    }

    #[test]
    fn unreadable_coverage_does_not_block_csv_tracking_or_get_overwritten() {
        let fixture = test_support::Fixture::new("");
        let path = fixture.csv.with_extension("coverage.json");
        fs::write(&path, b"invalid json").unwrap();
        let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
        tracker.mark_observed();
        assert_eq!(fs::read(&path).unwrap(), b"invalid json");
        assert_eq!(
            tracker.snapshot(Local::now().date_naive()).unwrap()["measured"],
            false
        );
    }

    #[test]
    fn existing_coverage_gets_a_persistent_start_date() {
        let fixture = test_support::Fixture::new("");
        let today = Local::now().date_naive();
        let path = fixture.csv.with_extension("coverage.json");
        fs::write(&path, format!(r#"{{"days":["{today}"]}}"#)).unwrap();
        let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
        tracker.coverage.mark(today).unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["startDate"], today.to_string());
        assert_eq!(saved["days"], serde_json::json!([today.to_string()]));
    }

    #[test]
    fn weekly_snapshot_divides_first_partial_week_by_calendar_days_and_adds_live_time_once() {
        let today = Local::now().date_naive();
        let monday = today
            .checked_sub_days(Days::new(today.weekday().num_days_from_monday() as u64 + 7))
            .unwrap();
        let wednesday = monday.checked_add_days(Days::new(2)).unwrap();
        let fixture = test_support::Fixture::new(&format!("{wednesday} 10:00:00,game.exe,300\n"));
        let mut tracker = Tracker::new(fixture.csv.clone()).unwrap();
        let week = tracker.snapshot(wednesday).unwrap()["week"].clone();
        assert_eq!(week["averageDays"], 5);
        assert_eq!(week["averageMinutes"], 1.0);

        let current_monday = today
            .checked_sub_days(Days::new(today.weekday().num_days_from_monday() as u64))
            .unwrap();
        let active = test_support::Fixture::new(&format!(
            "{current_monday} 00:00:00,game.exe,60\n{today} 00:00:00,game.exe,60\n"
        ));
        let mut tracker = Tracker::new(active.csv.clone()).unwrap();
        tracker.process = Some("game.exe".into());
        tracker.segment = Some(Segment {
            since: Instant::now() - Duration::from_secs(30),
            wall_start: Local::now(),
        });
        let before = fs::read(&active.csv).unwrap();
        let first = tracker.snapshot(today).unwrap();
        let second = tracker.snapshot(today).unwrap();
        for data in [&first, &second] {
            let total = data["week"]["totalMinutes"].as_f64().unwrap();
            assert!((2.5..2.54).contains(&total));
            assert_eq!(
                data["week"]["averageDays"],
                today.weekday().num_days_from_monday() as u64 + 1
            );
        }
        assert_eq!(fs::read(&active.csv).unwrap(), before);
    }

    #[test]
    fn manual_pause_flushes_once_ignores_activity_and_survives_restart() {
        let f = test_support::Fixture::new("");
        let mut tracker = Tracker::new(f.csv.clone()).unwrap();
        tracker.process = Some("synthetic.exe".into());
        tracker.segment = Some(Segment {
            since: Instant::now() - Duration::from_secs(10),
            wall_start: Local::now(),
        });
        tracker.toggle_pause().unwrap();
        let saved = fs::read(&f.csv).unwrap();
        assert!(!saved.is_empty());
        tracker.foreground(ptr::null_mut());
        tracker.input();
        tracker.checkpoint();
        tracker.set_idle_pause(false).unwrap();
        assert!(tracker.manual_paused && tracker.segment.is_none());
        assert_eq!(fs::read(&f.csv).unwrap(), saved);
        assert_eq!(
            tracker.snapshot(Local::now().date_naive()).unwrap()["trackingState"],
            "manual"
        );
        let mut restarted = Tracker::new(f.csv.clone()).unwrap();
        assert!(restarted.manual_paused);
        restarted.toggle_pause().unwrap();
        assert!(!restarted.manual_paused);
        assert!(!Tracker::new(f.csv.clone()).unwrap().manual_paused);
    }

    #[test]
    fn manual_pause_is_not_cleared_by_system_resume() {
        let f = test_support::Fixture::new("");
        let mut tracker = Tracker::new(f.csv.clone()).unwrap();
        tracker.toggle_pause().unwrap();
        tracker.system_state_with_foreground(true, Instant::now(), 0, || ptr::null_mut());
        tracker.system_state_with_foreground(false, Instant::now(), 0, || ptr::null_mut());
        assert!(tracker.manual_paused && !tracker.system_paused && tracker.segment.is_none());
    }

    #[test]
    fn clearing_history_discards_cached_totals_and_keeps_tracking_paused() {
        let f = test_support::Fixture::new("2026-09-28 10:00:00,synthetic.exe,30\n");
        let mut tracker = Tracker::new(f.csv.clone()).unwrap();
        tracker.snapshot(Local::now().date_naive()).unwrap();
        tracker.clear_history().unwrap();
        assert!(tracker.manual_paused && tracker.segment.is_none());
        assert!(fs::read(&f.csv).unwrap().is_empty());
        let data = tracker.snapshot(Local::now().date_naive()).unwrap();
        assert_eq!(data["week"]["totalMinutes"], 0.0);
        assert_eq!(data["labels"], serde_json::json!([]));
        assert!(Tracker::new(f.csv.clone()).unwrap().manual_paused);
    }

    #[test]
    fn system_pause_flushes_segment_and_ignores_new_activity() {
        let f = test_support::Fixture::new("");
        let mut tracker = Tracker::new(f.csv.clone()).unwrap();
        tracker.process = Some("example.exe".into());
        let now = Instant::now();
        tracker.segment = Some(Segment {
            since: now - Duration::from_secs(10),
            wall_start: Local::now(),
        });
        tracker.system_state(true, now, 0);
        tracker.input();
        tracker.foreground(ptr::null_mut());
        tracker.checkpoint();
        assert!(tracker.system_paused && tracker.segment.is_none());
        assert_eq!(load_chart_data(&f.csv).unwrap()[0].1 * 60.0, 10.0);
    }

    #[test]
    fn simulated_suspend_does_not_count_sleep_and_resumes_from_zero() {
        let f = test_support::Fixture::new("");
        let mut tracker = Tracker::new(f.csv.clone()).unwrap();
        let now = Instant::now();
        tracker.process = Some("example.exe".into());
        tracker.segment = Some(Segment {
            since: now - Duration::from_secs(600),
            wall_start: Local::now() - chrono::Duration::minutes(10),
        });
        let suspend_at = now - Duration::from_secs(300);
        tracker.system_state_with_foreground(true, suspend_at, 0, || ptr::null_mut());
        tracker.system_state_with_foreground(true, now, 0, || ptr::null_mut());
        tracker.checkpoint();
        assert!(tracker.system_paused && tracker.segment.is_none());
        assert_eq!(load_chart_data(&f.csv).unwrap()[0].1 * 60.0, 300.0);

        tracker.system_state_with_foreground(false, now, 0, || ptr::null_mut());
        assert!(!tracker.system_paused);
        tracker.process = Some("example.exe".into());
        tracker.segment = Some(Segment {
            since: now,
            wall_start: Local::now(),
        });
        tracker.flush_until(now + Duration::from_secs(120));
        assert_eq!(load_chart_data(&f.csv).unwrap()[0].1 * 60.0, 420.0);
    }
}

fn run() -> Result<(), String> {
    if std::env::args().any(|arg| arg == "--quit") {
        let _ = lifecycle::request_exit();
        return Ok(());
    }
    let _instance = match SingleInstance::acquire() {
        Ok(instance) => instance,
        Err(error) => {
            if lifecycle::request_dashboard() {
                return Ok(());
            }
            return Err(error);
        }
    };
    let csv_path = data_file()?;
    diagnostics::record(&format!("start version={}", env!("CARGO_PKG_VERSION")));
    let mut tracker = Tracker::new(csv_path.clone())?;
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    EVENT_PROXY
        .set(proxy.clone())
        .map_err(|_| "No se pudo inicializar el canal de eventos.".to_string())?;

    app_identity::start(proxy.clone());
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            ptr::null_mut(),
            Some(foreground_callback),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    if hook.is_null() {
        return Err("SetWinEventHook no pudo registrar EVENT_SYSTEM_FOREGROUND.".into());
    }
    let keyboard_hook =
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_callback), ptr::null_mut(), 0) };
    let mouse_hook =
        unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_callback), ptr::null_mut(), 0) };
    if keyboard_hook.is_null() || mouse_hook.is_null() {
        unsafe {
            UnhookWinEvent(hook);
            if !keyboard_hook.is_null() {
                UnhookWindowsHookEx(keyboard_hook);
            }
            if !mouse_hook.is_null() {
                UnhookWindowsHookEx(mouse_hook);
            }
        }
        return Err("No se pudieron registrar los hooks de actividad de teclado/ratón.".into());
    }

    let deadline_sender = start_deadline_worker(proxy.clone());

    let open_item = MenuItem::with_id(OPEN_ID, "Abrir", true, None);
    let pause_item = MenuItem::with_id(
        "toggle_pause",
        if tracker.manual_paused {
            "Reanudar seguimiento"
        } else {
            "Pausar seguimiento"
        },
        true,
        None,
    );
    let exit_item = MenuItem::with_id(EXIT_ID, "Salir", true, None);
    let menu = Menu::new();
    menu.append(&open_item)
        .map_err(|e| format!("No se pudo crear el menú: {e}"))?;
    menu.append(&pause_item).map_err(|e| e.to_string())?;
    menu.append(&exit_item)
        .map_err(|e| format!("No se pudo crear el menú: {e}"))?;
    let open_id = open_item.id().clone();
    let exit_id = exit_item.id().clone();
    let pause_id = pause_item.id().clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some(proxy) = EVENT_PROXY.get() {
            let app_event = if event.id == open_id {
                Some(AppEvent::OpenDashboard)
            } else if event.id == pause_id {
                Some(AppEvent::TogglePause)
            } else if event.id == exit_id {
                Some(AppEvent::Exit("tray"))
            } else {
                None
            };
            if let Some(app_event) = app_event {
                let _ = proxy.send_event(app_event);
            }
        }
    }));
    TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
        if let TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        } = event
        {
            if let Some(proxy) = EVENT_PROXY.get() {
                let _ = proxy.send_event(AppEvent::OpenDashboard);
            }
        }
    }));
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Screen Time")
        .with_icon(make_icon())
        .build()
        .map_err(|e| format!("No se pudo crear el icono de bandeja: {e}"))?;

    let mut dashboard: Option<Dashboard> = None;
    let mut settings: Option<Dashboard> = None;
    let mut selected_date: Option<NaiveDate> = None;
    tracker.system_paused = lifecycle::session_blocked();
    let mut monitor = Some(lifecycle::Monitor::start()?);
    let startup_hwnd =
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    tracker.foreground(startup_hwnd);
    if let Some(first_deadline) = tracker.idle_deadline() {
        let _ = deadline_sender.send(first_deadline);
    }

    if std::env::args().any(|arg| arg == "--dashboard") {
        let _ = proxy.send_event(AppEvent::OpenDashboard);
    } else {
        trim_background_memory();
    }

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(AppEvent::Foreground(hwnd)) => {
                tracker.foreground(hwnd as HWND);
                if let Some(remaining) = tracker.idle_deadline() { let _ = deadline_sender.send(remaining); }
            }
            Event::UserEvent(AppEvent::Input) => {
                if tracker.system_paused { if let Some(monitor) = &monitor { monitor.note_input(); } }
                tracker.input();
                if tracker.pause_when_idle { let _ = deadline_sender.send(IDLE_LIMIT_MS); }
            }
            Event::UserEvent(AppEvent::Checkpoint) => {
                tracker.checkpoint();
            }
            Event::UserEvent(AppEvent::UiLoadTimeout) => {
                for current in dashboard.iter_mut().chain(settings.iter_mut()) {
                    if !current.ready && current.opened_at.elapsed() >= Duration::from_secs(10) {
                        current.ready = true;
                        current.window.set_visible(true);
                        show_message("La ventana no terminó de cargar. Cerrala y volvé a abrirla. Si se repite, repará Microsoft Edge WebView2 Runtime.");
                    }
                }
            }
            Event::UserEvent(AppEvent::SystemState { paused, at, idle, ack }) => {
                tracker.system_state(paused, at, idle);
                let _ = ack.send(());
                if let Some(remaining) = tracker.idle_deadline() { let _ = deadline_sender.send(remaining); }
            }
            Event::UserEvent(AppEvent::IdleDeadline) => {
                if let Some(remaining) = tracker.idle_deadline() { let _ = deadline_sender.send(remaining); }
            }
            Event::UserEvent(AppEvent::OpenDashboard) => {
                if dashboard.is_some() {
                    if let Some(current) = dashboard.as_ref().filter(|current| current.ready) { current.window.set_minimized(false); fit_window_to_work_area(&current.window, false); current.window.set_visible(true); current.window.set_focus(); }
                } else {
                    tracker.checkpoint();
                    let html = dashboard_html(tracker.dark_mode);
                    {
                            let result = (|| -> Result<_, String> {
                                let settings_path = csv_path.with_extension("settings.json");
                                let geometry = WindowGeometry::load(&settings_path, "dashboard_window", 780.0, 820.0);
                                let window = WindowBuilder::new().with_visible(false).with_window_icon(Some(window_icon())).with_title("Screen Time")
                                    .with_inner_size(LogicalSize::new(geometry.width, geometry.height))
                                    .with_min_inner_size(LogicalSize::new(360.0, 480.0)).build(target)
                                    .map_err(|e| format!("No se pudo crear el dashboard: {e}"))?;
                                geometry.restore(&window);
                                let saved_colors = fs::read(&settings_path).ok()
                                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                                    .and_then(|settings| settings.get("icon_colors").cloned()).unwrap_or_else(|| serde_json::json!([]));
                                let webview = wry::WebViewBuilder::new().with_html(html)
                                    .with_initialization_script(&format!("window.savedIconColors={saved_colors};"))
                                    .with_ipc_handler(|request| handle_ipc(request.body()))
                                    .build(&window)
                                    .map_err(|e| format!("No se pudo inicializar WebView2: {e}"))?;
                                let id = window.id();
                                Ok(Dashboard { webview, window, id, geometry, settings_path, settings_key: "dashboard_window", icon_cache: BTreeMap::new(), unresolved_icons: Default::default(), icons_loading: false, icon_generation: 0, icons_enabled: false, ready: false, opened_at: Instant::now(), geometry_dirty_at: None })
                            })();
                            match result {
                                Ok(created) => { dashboard = Some(created); if let Some(monitor) = &monitor { if let Err(error) = monitor.arm_ui_timeout() { show_message(&error); } } let _ = proxy.send_event(AppEvent::RefreshDashboard); }
                                Err(e) => show_message(&e),
                            }
                    }
                }
            }
            Event::UserEvent(AppEvent::DashboardReady) => {
                if let Some(current) = dashboard.as_mut() {
                    current.icons_enabled = true;
                    if !current.ready {
                        current.ready = true;
                        current.window.set_visible(true);
                        current.window.set_focus();
                        let _ = current.webview.evaluate_script("window.dispatchEvent(new Event('resize'));");
                        let _ = proxy.send_event(AppEvent::RefreshDashboard);
                    }
                }
            }
            Event::UserEvent(AppEvent::SettingsReady) => {
                if let Some(current) = settings.as_mut() {
                    if !current.ready {
                        current.ready = true; current.window.set_visible(true); current.window.set_focus(); }
                }
            }
            Event::UserEvent(AppEvent::IconsLoaded { window_id, generation, entries }) => {
                if let Some(current) = dashboard.as_mut().filter(|current| current.id == window_id && current.icon_generation == generation) {
                    current.icons_loading = false;
                    let mut paths_changed = false;
                    for (name, path, icon) in entries {
                        if let Some(path) = path {
                            if tracker.process_paths.get(&name) != Some(&path) {
                                tracker.process_paths.insert(name.clone(), path.clone());
                                paths_changed = true;
                            }
                            current.icon_cache.insert(path, icon);
                        } else {
                            current.unresolved_icons.insert(name);
                        }
                    }
                    if paths_changed { tracker.save_process_paths(); }
                    let _ = proxy.send_event(AppEvent::RefreshDashboard);
                }
            }
            Event::UserEvent(AppEvent::TogglePause) => {
                if let Err(error) = tracker.toggle_pause() { show_message(&error); }
                pause_item.set_text(if tracker.manual_paused { "Reanudar seguimiento" } else { "Pausar seguimiento" });
                if let Some(remaining) = tracker.idle_deadline() { let _ = deadline_sender.send(remaining); }
                let _ = proxy.send_event(AppEvent::RefreshDashboard);
            }
            Event::UserEvent(AppEvent::OpenDataFolder) => {
                if let Some(folder) = csv_path.parent() { if let Err(error) = open_folder(folder) { show_message(&error); } }
            }
            Event::UserEvent(AppEvent::ExportHistory) => {
                tracker.checkpoint();
                let result = if let Some(error) = &tracker.save_error { Err(format!("No se exportó: hay un guardado pendiente. {error}")) } else { data_management::export(&csv_path).and_then(|folder| open_folder(&folder)) };
                if let Err(error) = result { show_message(&error); }
            }
            Event::UserEvent(AppEvent::ClearHistory) => {
                if confirm_clear_history() {
                    let result = tracker.clear_history();
                    let message = match result {
                        Ok(()) => {
                            selected_date = None;
                            if let Some(current) = dashboard.as_mut() { current.icon_generation += 1; current.icons_loading = false; current.icon_cache.clear(); current.unresolved_icons.clear(); }
                            "Historial borrado. El seguimiento quedó pausado. Podés reanudarlo desde el dashboard o la bandeja. Se conservan exportaciones y copias de seguridad.".to_string()
                        }
                        Err(error) => {
                            let message = format!("No se pudo completar el borrado: {error}");
                            show_message(&message);
                            message
                        }
                    };
                    pause_item.set_text(if tracker.manual_paused { "Reanudar seguimiento" } else { "Pausar seguimiento" });
                    if let Some(current) = settings.as_ref() {
                        let message = serde_json::to_string(&message).unwrap_or_default();
                        let _ = current.webview.evaluate_script(&format!("document.getElementById('status').textContent={message};"));
                    }
                    let _ = proxy.send_event(AppEvent::RefreshDashboard);
                }
            }
            Event::UserEvent(AppEvent::ApplicationResolved {generation, hwnd, path, result}) => {
                tracker.application_resolved(generation, hwnd, path, result);
                let _ = proxy.send_event(AppEvent::RefreshDashboard);
            }
            Event::UserEvent(AppEvent::SaveIconColors(colors)) => {
                if let Err(error) = save_preference(&csv_path.with_extension("settings.json"), "icon_colors", colors) {
                    diagnostics::record(&format!("icon_colors_save_error: {error}"));
                }
            }
            Event::UserEvent(AppEvent::SetIdlePause(enabled)) => {
                if let Err(error) = tracker.set_idle_pause(enabled) { show_message(&error); }
                if let Some(remaining) = tracker.idle_deadline() { let _ = deadline_sender.send(remaining); }
                let _ = proxy.send_event(AppEvent::RefreshDashboard);
            }
            Event::UserEvent(AppEvent::SetDarkMode(enabled)) => {
                if let Err(error) = tracker.set_dark_mode(enabled) { show_message(&error); }
                let _ = proxy.send_event(AppEvent::RefreshDashboard);
            }
            Event::UserEvent(AppEvent::SetAutostart(enabled)) => {
                if let Err(error) = startup_entry(Some(enabled)) { show_message(&error); }
                let _ = proxy.send_event(AppEvent::RefreshDashboard);
            }
            Event::UserEvent(AppEvent::SelectDate(date)) => {
                selected_date = if date >= Local::now().date_naive() { None } else { Some(date) };
                let _ = proxy.send_event(AppEvent::RefreshDashboard);
            }
            Event::UserEvent(AppEvent::CloseSettings) => { close_dashboard(&mut settings); if dashboard.is_none() { tracker.daily_summary = None; trim_background_memory(); } }
            Event::UserEvent(AppEvent::OpenSettings) => {
                if let Some(current) = settings.as_ref() { current.window.set_focus(); }
                else {
                    let result = (|| -> Result<Dashboard, String> {
                        let settings_path = csv_path.with_extension("settings.json");
                        let geometry = WindowGeometry::load(&settings_path, "settings_window", 460.0, 560.0);
                        let window = WindowBuilder::new().with_visible(false).with_window_icon(Some(window_icon())).with_title("Configuración — Screen Time")
                            .with_inner_size(LogicalSize::new(geometry.width, geometry.height))
                            .with_min_inner_size(LogicalSize::new(340.0, 340.0)).build(target).map_err(|e| e.to_string())?;
                        geometry.restore(&window);
                        let html = include_str!("../assets/settings.html")
                            .replace("__CSS__", include_str!("../assets/dashboard.css"))
        .replace("__VIEW_STATE_JS__", include_str!("../assets/view-state.js"))
                            .replace("__DARK__", if tracker.dark_mode { "dark" } else { "light" });
                        let webview = wry::WebViewBuilder::new().with_html(html)
                            .with_ipc_handler(|request| handle_ipc(request.body())).build(&window).map_err(|e| e.to_string())?;
                        let id = window.id();
                        Ok(Dashboard { webview, window, id, geometry, settings_path, settings_key: "settings_window", icon_cache: BTreeMap::new(), unresolved_icons: Default::default(), icons_loading: false, icon_generation: 0, icons_enabled: false, ready: false, opened_at: Instant::now(), geometry_dirty_at: None })
                    })();
                    match result { Ok(created) => { settings = Some(created); if let Some(monitor) = &monitor { if let Err(error) = monitor.arm_ui_timeout() { show_message(&error); } } }, Err(error) => show_message(&error) }
                }
                let _ = proxy.send_event(AppEvent::RefreshDashboard);
            }
            Event::UserEvent(AppEvent::RefreshDashboard) => {
                if let Some(current) = settings.as_ref() {
                    let preferences = serde_json::json!({
                        "darkMode": tracker.dark_mode, "pauseWhenIdle": tracker.pause_when_idle,
                        "autostart": startup_entry(None).unwrap_or(false), "dataLocation": csv_path.parent().map(|path| path.display().to_string())
                    });
                    let _ = current.webview.evaluate_script(&format!("window.updateUsage({preferences});"));
                }
                if dashboard.is_none() { return; }
                let mut snapshot = tracker.snapshot(selected_date.unwrap_or_else(|| Local::now().date_naive()));
                if let (Ok(data), Some(current)) = (&mut snapshot, dashboard.as_mut()) {
                    let mut images = serde_json::Map::new();
                    let mut missing = Vec::new();
                    let presentation = tracker.grouping.presentation(&tracker.applications);
                    for labels in [data["labels"].as_array(), data["week"]["labels"].as_array()].into_iter().flatten() {
                        for label in labels {
                            if let Some(name) = label.as_str() {
                                if let Some(icon) = presentation.get(name).and_then(|metadata| metadata.icon) {
                                    images.insert(name.into(), icon.into());
                                    continue;
                                }
                                let key = name.to_lowercase();
                                let path = tracker.process_paths.get(&key);
                                if let Some(Some(url)) = path.and_then(|path| current.icon_cache.get(path)) {
                                    images.insert(name.into(), url.clone().into());
                                } else if !current.icons_loading && !current.unresolved_icons.contains(&key)
                                    && path.is_none_or(|path| !current.icon_cache.contains_key(path)) {
                                    missing.push((key.clone(), history::executable_path(&key).or_else(|| path.cloned())));
                                }
                            }
                        }
                    }
                    data["icons"] = images.into();
                    if current.icons_enabled && !missing.is_empty() {
                        current.icons_loading = true;
                        let window_id = current.id;
                        let generation = current.icon_generation;
                        let icon_proxy = proxy.clone();
                        std::thread::spawn(move || {
                            let running = if missing.iter().any(|(_, path)| path.is_none()) { icons::running_paths() } else { BTreeMap::new() };
                            let entries = missing.into_iter().map(|(name, known_path)| {
                                let path = known_path.or_else(|| running.get(&name).cloned());
                                let icon = path.as_deref().and_then(icons::icon_url);
                                (name, path, icon)
                            }).collect();
                            let _ = icon_proxy.send_event(AppEvent::IconsLoaded { window_id, generation, entries });
                        });
                    }
                }
                for current in dashboard.iter() {
                    match &snapshot {
                        Ok(data) => { let _ = current.webview.evaluate_script(&format!("window.updateUsage({data});")); }
                        Err(error) => {
                            let message = serde_json::to_string(&error).unwrap_or_default();
                            let _ = current.webview.evaluate_script(&format!("window.showUsageError({message});"));
                        }
                    }
                }
            }
            Event::UserEvent(AppEvent::Exit(reason)) => {
                tracker.idle_deadline();
                tracker.flush_until(Instant::now());
                close_dashboard(&mut settings);
                close_dashboard(&mut dashboard);
                diagnostics::record(&format!("exit reason={reason}"));
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent { window_id, event: WindowEvent::CloseRequested, .. } => {
                if settings.as_ref().map(|current| current.id) == Some(window_id) { close_dashboard(&mut settings); }
                if dashboard.as_ref().map(|current| current.id) == Some(window_id) { close_dashboard(&mut settings); close_dashboard(&mut dashboard); tracker.daily_summary = None; trim_background_memory(); }
                else if dashboard.is_none() && settings.is_none() { tracker.daily_summary = None; trim_background_memory(); }
            }
            Event::WindowEvent { window_id, event: WindowEvent::Resized(_) | WindowEvent::Moved(_), .. } => {
                for current in dashboard.iter_mut().chain(settings.iter_mut()) {
                    if current.id == window_id && current.ready { current.geometry.capture(&current.window); current.geometry_dirty_at = Some(Instant::now()); }
                }
            }
            Event::LoopDestroyed => {
                drop(monitor.take());
                close_dashboard(&mut settings);
                close_dashboard(&mut dashboard);
                tracker.flush_until(tracker.accounting_end(Instant::now(), idle_ms()));
                unsafe { UnhookWinEvent(hook); UnhookWindowsHookEx(keyboard_hook); UnhookWindowsHookEx(mouse_hook); }
                diagnostics::record("event_loop_ended");
            }
            _ => {}
        }
        if !matches!(*control_flow, ControlFlow::Exit) {
            for current in dashboard.iter_mut().chain(settings.iter_mut()) {
                if let Some(changed) = current.geometry_dirty_at {
                    let deadline = changed + Duration::from_secs(1);
                    if Instant::now() >= deadline {
                        if let Err(error) = save_preference(&current.settings_path, current.settings_key, current.geometry.to_json()) {
                            diagnostics::record(&format!("window_geometry_save_error: {error}"));
                        }
                        current.geometry_dirty_at = None;
                    } else if !matches!(*control_flow, ControlFlow::WaitUntil(existing) if existing <= deadline) {
                        *control_flow = ControlFlow::WaitUntil(deadline);
                    }
                }
            }
        }
    });
}

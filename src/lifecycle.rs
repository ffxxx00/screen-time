use crate::{AppEvent, EVENT_PROXY};
use std::{
    ptr,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{
        LibraryLoader::GetModuleHandleW,
        Power::*,
        RemoteDesktop::*,
        SystemServices::{GUID_LIDSWITCH_STATE_CHANGE, GUID_SESSION_DISPLAY_STATUS},
    },
    UI::WindowsAndMessaging::*,
};
const INPUT: u32 = WM_APP + 10;
const SHOW: u32 = WM_APP + 9;
const QUIT: u32 = WM_APP + 11;
const CLASS: &str = "Screen Time.Lifecycle";
const CHECKPOINT_TIMER: usize = 1;
const UI_TIMEOUT_TIMER: usize = 2;
const CHECKPOINT_INTERVAL_MS: u32 = 60_000;
const UI_TIMEOUT_MS: u32 = 12_000;
static PAUSE_AT: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
pub fn pause_at() -> Option<Instant> {
    *PAUSE_AT.lock().unwrap()
}
pub fn clear_pause() {
    *PAUSE_AT.lock().unwrap() = None;
}
fn same_guid(a: windows_sys::core::GUID, b: windows_sys::core::GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

unsafe extern "system" fn find_lifecycle_window(hwnd: HWND, result: LPARAM) -> BOOL {
    let mut name = [0u16; 64];
    let length = GetClassNameW(hwnd, name.as_mut_ptr(), name.len() as i32);
    if length > 0
        && name[..length as usize]
            .iter()
            .copied()
            .eq(CLASS.encode_utf16())
    {
        *(result as *mut HWND) = hwnd;
        return 0;
    }
    1
}

pub fn request_dashboard() -> bool {
    post_to_lifecycle(SHOW)
}
pub fn request_exit() -> bool {
    post_to_lifecycle(QUIT)
}
fn post_to_lifecycle(message: u32) -> bool {
    unsafe {
        let mut hwnd: HWND = ptr::null_mut();
        EnumWindows(
            Some(find_lifecycle_window),
            (&mut hwnd as *mut HWND) as isize,
        );
        !hwnd.is_null() && PostMessageW(hwnd, message, 0, 0) != 0
    }
}
pub fn session_blocked() -> bool {
    unsafe {
        let mut buffer = ptr::null_mut();
        let mut bytes = 0;
        if WTSQuerySessionInformationW(
            ptr::null_mut(),
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        ) == 0
        {
            return true;
        }
        let blocked = if bytes as usize >= std::mem::size_of::<WTSINFOEXW>() {
            let info = &*(buffer as *const WTSINFOEXW);
            let state = info.Data.WTSInfoExLevel1;
            info.Level != 1
                || state.SessionState != WTSActive
                || state.SessionFlags != WTS_SESSIONSTATE_UNLOCK as i32
        } else {
            true
        };
        WTSFreeMemory(buffer.cast());
        blocked
    }
}
#[derive(Default)]
struct State {
    session: bool,
    suspended: bool,
    ending: bool,
    lid_closed: bool,
    display_off: bool,
}
impl State {
    fn paused(&self) -> bool {
        self.session || self.suspended || self.ending || self.lid_closed || self.display_off
    }

    fn session_event(&mut self, event: u32, blocked: bool) -> bool {
        match event {
            WTS_SESSION_LOCK
            | WTS_SESSION_LOGOFF
            | WTS_CONSOLE_DISCONNECT
            | WTS_REMOTE_DISCONNECT => self.session = true,
            WTS_SESSION_UNLOCK | WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT | WTS_SESSION_LOGON => {
                self.session = blocked;
                if !blocked {
                    self.suspended = false;
                }
            }
            _ => return false,
        }
        true
    }

    fn power_event(&mut self, event: u32, blocked: bool) -> bool {
        match event {
            PBT_APMSUSPEND => self.suspended = true,
            PBT_APMRESUMEAUTOMATIC => {
                self.suspended = true;
                self.session = blocked;
            }
            PBT_APMRESUMESUSPEND | PBT_APMRESUMECRITICAL => {
                self.suspended = false;
                self.session = blocked;
            }
            _ => return false,
        }
        true
    }

    fn display_event(&mut self, lid: bool, value: u32) {
        if lid {
            self.lid_closed = value == 0;
        } else {
            self.display_off = value == 0;
        }
    }

    fn input(&mut self, blocked: bool) -> bool {
        if !self.suspended || blocked {
            return false;
        }
        self.suspended = false;
        self.session = false;
        true
    }

    fn notify(&self) {
        if let Some(proxy) = EVENT_PROXY.get() {
            let paused = self.paused();
            let at = Instant::now();
            if paused {
                PAUSE_AT.lock().unwrap().get_or_insert(at);
            }
            let (ack, wait) = mpsc::channel();
            let _ = proxy.send_event(AppEvent::SystemState {
                paused,
                at,
                idle: crate::idle_ms(),
                ack,
            });
            // Runs on a dedicated message thread so shutdown/suspend can wait for
            // the main thread's durable checkpoint without blocking its event loop.
            let _ = wait.recv_timeout(Duration::from_millis(1800));
        }
    }
}
unsafe extern "system" fn wndproc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    if message == WM_NCCREATE {
        let create = &*(l as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        return DefWindowProcW(hwnd, message, w, l);
    }
    if state.is_null() {
        return DefWindowProcW(hwnd, message, w, l);
    }
    let state = &mut *state;
    match message {
        WM_TIMER if w == CHECKPOINT_TIMER => {
            if let Some(proxy) = EVENT_PROXY.get() {
                let _ = proxy.send_event(AppEvent::Checkpoint);
            }
            0
        }
        WM_TIMER if w == UI_TIMEOUT_TIMER => {
            KillTimer(hwnd, UI_TIMEOUT_TIMER);
            if let Some(proxy) = EVENT_PROXY.get() {
                let _ = proxy.send_event(AppEvent::UiLoadTimeout);
            }
            0
        }
        INPUT => {
            if state.suspended && state.input(session_blocked()) {
                state.notify();
            }
            0
        }
        SHOW => {
            if let Some(proxy) = EVENT_PROXY.get() {
                let _ = proxy.send_event(AppEvent::OpenDashboard);
            }
            0
        }
        QUIT => {
            if let Some(proxy) = EVENT_PROXY.get() {
                let _ = proxy.send_event(AppEvent::Exit("external_request"));
            }
            0
        }
        WM_WTSSESSION_CHANGE => {
            let blocked = if matches!(
                w as u32,
                WTS_SESSION_UNLOCK | WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT | WTS_SESSION_LOGON
            ) {
                session_blocked()
            } else {
                true
            };
            if state.session_event(w as u32, blocked) {
                state.notify();
            }
            0
        }
        WM_POWERBROADCAST if w as u32 == PBT_POWERSETTINGCHANGE && l != 0 => {
            let setting = &*(l as *const POWERBROADCAST_SETTING);
            if setting.DataLength == 4 {
                let guid = setting.PowerSetting;
                let lid = same_guid(guid, GUID_LIDSWITCH_STATE_CHANGE);
                if lid || same_guid(guid, GUID_SESSION_DISPLAY_STATUS) {
                    let value = ptr::read_unaligned(setting.Data.as_ptr().cast::<u32>());
                    state.display_event(lid, value);
                    state.notify();
                }
            }
            1
        }
        WM_POWERBROADCAST => {
            let blocked = if matches!(
                w as u32,
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND | PBT_APMRESUMECRITICAL
            ) {
                session_blocked()
            } else {
                true
            };
            if state.power_event(w as u32, blocked) {
                state.notify();
            }
            1
        }
        WM_QUERYENDSESSION => {
            state.ending = true;
            state.notify();
            1
        }
        WM_ENDSESSION => {
            state.ending = w != 0;
            state.notify();
            if w != 0 {
                if let Some(proxy) = EVENT_PROXY.get() {
                    let _ = proxy.send_event(AppEvent::Exit("windows_session"));
                }
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, CHECKPOINT_TIMER);
            KillTimer(hwnd, UI_TIMEOUT_TIMER);
            WTSUnRegisterSessionNotification(hwnd);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, w, l),
    }
}
pub struct Monitor {
    hwnd: usize,
    thread: Option<thread::JoinHandle<()>>,
}
impl Monitor {
    pub fn arm_ui_timeout(&self) -> Result<(), String> {
        if unsafe { SetTimer(self.hwnd as HWND, UI_TIMEOUT_TIMER, UI_TIMEOUT_MS, None) } == 0 {
            return Err("No se pudo activar el tiempo límite de carga de WebView2".into());
        }
        Ok(())
    }
    pub fn note_input(&self) {
        unsafe {
            PostMessageW(self.hwnd as HWND, INPUT, 0, 0);
        }
    }
    pub fn start() -> Result<Self, String> {
        let (ready, receive) = mpsc::channel::<Result<usize, String>>();
        let thread = thread::Builder::new()
            .name("windows-session".into())
            .stack_size(256 * 1024)
            .spawn(move || unsafe {
                let instance = GetModuleHandleW(ptr::null());
                let class = wide(CLASS);
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(wndproc),
                    hInstance: instance,
                    lpszClassName: class.as_ptr(),
                    ..std::mem::zeroed()
                };
                if RegisterClassW(&wc) == 0 {
                    let _ = ready.send(Err("No se pudo registrar la ventana de sesión".into()));
                    return;
                }
                let mut state = Box::new(State {
                    session: session_blocked(),
                    suspended: false,
                    ..State::default()
                });
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    class.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    instance,
                    (&mut *state as *mut State).cast(),
                );
                if hwnd.is_null() {
                    let _ = ready.send(Err("No se pudo crear la ventana de sesión".into()));
                    return;
                }
                if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) == 0
                    || SetTimer(hwnd, CHECKPOINT_TIMER, CHECKPOINT_INTERVAL_MS, None) == 0
                {
                    let _ = ready.send(Err(
                        "No se pudieron registrar las notificaciones de sesión o recuperación"
                            .into(),
                    ));
                    DestroyWindow(hwnd);
                    return;
                }
                let registrations =
                    [GUID_LIDSWITCH_STATE_CHANGE, GUID_SESSION_DISPLAY_STATUS].map(|guid| {
                        RegisterPowerSettingNotification(hwnd, &guid, DEVICE_NOTIFY_WINDOW_HANDLE)
                    });
                if registrations.iter().any(|handle| *handle == 0) {
                    for handle in registrations {
                        if handle != 0 {
                            UnregisterPowerSettingNotification(handle);
                        }
                    }
                    let _ = ready.send(Err(
                        "No se pudieron registrar los eventos de tapa y pantalla".into(),
                    ));
                    DestroyWindow(hwnd);
                    return;
                }
                let _ = ready.send(Ok(hwnd as usize));
                state.notify();
                let mut message: MSG = std::mem::zeroed();
                while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                for handle in registrations {
                    UnregisterPowerSettingNotification(handle);
                }
            })
            .map_err(|e| e.to_string())?;
        let hwnd = receive.recv().map_err(|e| e.to_string())??;
        Ok(Self {
            hwnd,
            thread: Some(thread),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lid_and_display_pause_independently_of_suspend_and_input() {
        let mut state = State::default();
        state.display_event(true, 0);
        assert!(state.paused());
        assert!(!state.input(false));
        state.display_event(false, 0);
        state.display_event(true, 1);
        assert!(state.paused());
        state.display_event(false, 2); // Dimmed still counts as visible.
        assert!(!state.paused());
        state.session_event(WTS_SESSION_LOCK, true);
        state.display_event(true, 0);
        state.display_event(true, 1);
        assert!(state.paused());
        state.session_event(WTS_SESSION_UNLOCK, false);
        assert!(!state.paused());
    }

    #[test]
    fn checkpoint_interval_is_one_minute_and_ui_timeout_is_independent() {
        assert_eq!(CHECKPOINT_INTERVAL_MS, 60_000);
        assert!(UI_TIMEOUT_MS < CHECKPOINT_INTERVAL_MS);
        assert_ne!(CHECKPOINT_TIMER, UI_TIMEOUT_TIMER);
    }

    #[test]
    fn simulated_lock_disconnect_and_reconnect() {
        let mut state = State::default();
        assert!(state.session_event(WTS_SESSION_LOCK, true));
        assert!(state.paused());
        assert!(state.session_event(WTS_SESSION_UNLOCK, true));
        assert!(state.paused());
        assert!(state.session_event(WTS_SESSION_UNLOCK, false));
        assert!(!state.paused());
        for (disconnect, connect) in [
            (WTS_CONSOLE_DISCONNECT, WTS_CONSOLE_CONNECT),
            (WTS_REMOTE_DISCONNECT, WTS_REMOTE_CONNECT),
            (WTS_SESSION_LOGOFF, WTS_SESSION_LOGON),
        ] {
            assert!(state.session_event(disconnect, true));
            assert!(state.paused());
            assert!(state.session_event(connect, false));
            assert!(!state.paused());
        }
        assert!(!state.session_event(0, false));
        assert!(!state.paused());
    }

    #[test]
    fn simulated_suspend_and_automatic_wake_need_user_presence() {
        let mut state = State::default();
        assert!(state.power_event(PBT_APMSUSPEND, false));
        assert!(state.paused());
        assert!(state.power_event(PBT_APMRESUMEAUTOMATIC, true));
        assert!(!state.input(true));
        assert!(state.paused());
        assert!(state.session_event(WTS_SESSION_UNLOCK, false));
        assert!(!state.paused());

        assert!(state.power_event(PBT_APMSUSPEND, false));
        assert!(state.power_event(PBT_APMRESUMEAUTOMATIC, false));
        assert!(state.paused());
        assert!(state.input(false));
        assert!(!state.paused());
        assert!(!state.input(false));
        assert!(!state.power_event(0, false));
    }

    #[test]
    fn simulated_resume_and_cancelled_shutdown() {
        let mut state = State::default();
        state.ending = true; // WM_QUERYENDSESSION
        assert!(state.paused());
        state.ending = false; // WM_ENDSESSION with wParam = 0
        assert!(!state.paused());
        assert!(state.power_event(PBT_APMSUSPEND, false));
        assert!(state.power_event(PBT_APMRESUMESUSPEND, true));
        assert!(state.paused());
        assert!(state.session_event(WTS_SESSION_UNLOCK, false));
        assert!(!state.paused());
        assert!(state.power_event(PBT_APMSUSPEND, false));
        assert!(state.power_event(PBT_APMRESUMECRITICAL, false));
        assert!(!state.paused());
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        unsafe {
            PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

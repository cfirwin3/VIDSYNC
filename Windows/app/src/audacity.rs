//! Audacity 4.0 timecode from the desktop accessibility layer.
//!
//! On Linux, Audacity 4.0 is a Qt application that publishes its playback
//! position as an AT-SPI accessible name (`Playback position:00 h 00 m 11.037
//! s`). On Windows the same Qt accessibility tree is exposed through UI
//! Automation (UIA); the accessible Name (or the Value pattern) carries the
//! same text.
//!
//! Either way a background thread discovers the Audacity accessible tree,
//! caches the node that holds the timecode, and polls it about thirty times a
//! second. The UI only reads a small snapshot, so a slow or missing Audacity
//! never stalls the video.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const POLL: Duration = Duration::from_millis(33);
pub const REDISCOVER: Duration = Duration::from_millis(1500);
pub const MOVEMENT_WINDOW: Duration = Duration::from_millis(220);

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub connected: bool,
    pub position: Option<f64>,
    /// True while the timecode has changed recently, i.e. Audacity is running.
    pub moving: bool,
    /// True when Audacity's timecode display uses the requested
    /// `00h00m00.000s` style rather than a bare colon clock.
    pub format_ok: bool,
    pub message: String,
}

pub struct AudacityLink {
    state: Arc<Mutex<Snapshot>>,
    stop: Arc<AtomicBool>,
}

impl AudacityLink {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(Snapshot {
            connected: false,
            position: None,
            moving: false,
            format_ok: false,
            message: "looking for Audacity…".to_string(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_state = Arc::clone(&state);
        let worker_stop = Arc::clone(&stop);
        thread::spawn(move || platform::poll_loop(worker_state, worker_stop));
        AudacityLink { state, stop }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for AudacityLink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn set_state(state: &Mutex<Snapshot>, next: Snapshot) {
    if let Ok(mut current) = state.lock() {
        *current = next;
    }
}

fn disconnected(message: impl Into<String>) -> Snapshot {
    Snapshot {
        connected: false,
        position: None,
        moving: false,
        format_ok: false,
        message: message.into(),
    }
}

/// Shared movement tracking: a reading that changes within the movement window
/// marks Audacity as playing. Both accessibility backends feed this.
struct Tracker {
    last_position: Option<f64>,
    last_change: Option<Instant>,
}

impl Tracker {
    fn new() -> Self {
        Tracker {
            last_position: None,
            last_change: None,
        }
    }

    fn reset(&mut self) {
        self.last_position = None;
        self.last_change = None;
    }

    fn observe(&mut self, position: f64) -> bool {
        let changed = self
            .last_position
            .map(|previous| (position - previous).abs() > 0.0005)
            .unwrap_or(false);
        if changed {
            self.last_change = Some(Instant::now());
        }
        self.last_position = Some(position);
        self.last_change
            .map(|at| at.elapsed() < MOVEMENT_WINDOW)
            .unwrap_or(false)
    }
}

fn live_state(position: f64, moving: bool, raw: &str) -> Snapshot {
    let format_ok = timecode_uses_hms(raw);
    Snapshot {
        connected: true,
        position: Some(position),
        moving,
        format_ok,
        message: if format_ok {
            "timecode live".to_string()
        } else {
            "set Audacity to 00h00m00.000s".to_string()
        },
    }
}

// ---------------------------------------------------------------- Linux

#[cfg(target_os = "linux")]
mod platform {
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{OwnedObjectPath, OwnedValue};

    use super::{
        disconnected, live_state, set_state, Snapshot, Tracker, POLL, REDISCOVER,
    };

    struct TimecodeNode {
        conn: Connection,
        bus: String,
        path: String,
    }

    impl TimecodeNode {
        fn read(&self) -> Result<String, Box<dyn Error>> {
            let props = Proxy::new(
                &self.conn,
                self.bus.as_str(),
                self.path.as_str(),
                "org.freedesktop.DBus.Properties",
            )?;
            let value: OwnedValue = props.call("Get", &("org.a11y.atspi.Accessible", "Name"))?;
            Ok(String::try_from(value)?)
        }
    }

    pub fn poll_loop(state: Arc<Mutex<Snapshot>>, stop: Arc<AtomicBool>) {
        let mut node: Option<TimecodeNode> = None;
        let mut next_discover = Instant::now();
        let mut tracker = Tracker::new();

        while !stop.load(Ordering::Relaxed) {
            if Instant::now() >= next_discover {
                match discover() {
                    Ok(found) => {
                        node = Some(found);
                        next_discover = Instant::now() + REDISCOVER * 8;
                    }
                    Err(message) => {
                        node = None;
                        tracker.reset();
                        next_discover = Instant::now() + Duration::from_secs(1);
                        set_state(&state, disconnected(message));
                    }
                }
            }

            if let Some(current) = node.as_ref() {
                match current.read() {
                    Ok(raw) => {
                        if let Some(position) = super::parse_timecode(&raw) {
                            let moving = tracker.observe(position);
                            set_state(&state, live_state(position, moving, &raw));
                        } else {
                            // The node was found but its text is not a timecode
                            // we understand. Re-discover soon in case Audacity
                            // swapped the timecode display.
                            node = None;
                            tracker.reset();
                            set_state(&state, disconnected("timecode format not recognised"));
                        }
                    }
                    Err(_) => {
                        node = None;
                        tracker.reset();
                        set_state(&state, disconnected("Audacity closed"));
                    }
                }
            }

            thread::sleep(POLL);
        }
    }

    fn discover() -> Result<TimecodeNode, String> {
        discover_inner().map_err(|e| friendly(e.to_string()))
    }

    fn friendly(message: String) -> String {
        if message.contains("org.a11y") || message.contains("Accessibility") {
            "accessibility bus unavailable".to_string()
        } else if message.contains("not found") || message.contains("No such") {
            "Audacity not found".to_string()
        } else {
            "waiting for Audacity".to_string()
        }
    }

    fn discover_inner() -> Result<TimecodeNode, Box<dyn Error>> {
        let session = Connection::session()?;

        // Make sure the desktop accessibility bridge is active. Audacity only
        // builds its accessible tree while this is switched on.
        if let Ok(props) = Proxy::new(
            &session,
            "org.a11y.Bus",
            "/org/a11y/bus",
            "org.freedesktop.DBus.Properties",
        ) {
            let enabled = zbus::zvariant::Value::Bool(true);
            let _: Result<(), _> = props.call("Set", &("org.a11y.Status", "IsEnabled", enabled));
        }

        let bus = Proxy::new(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus")?;
        let address: String = bus.call("GetAddress", &())?;
        let conn = zbus::blocking::connection::Builder::address(address.as_str())?.build()?;

        let root = Proxy::new(
            &conn,
            "org.a11y.atspi.Registry",
            "/org/a11y/atspi/accessible/root",
            "org.a11y.atspi.Accessible",
        )?;
        let apps: Vec<(String, OwnedObjectPath)> = root.call("GetChildren", &())?;

        for (app_bus, app_path) in apps {
            let name = get_string(&conn, &app_bus, app_path.as_str(), "Name").unwrap_or_default();
            if !name.to_ascii_lowercase().contains("audacity") {
                continue;
            }
            let count = get_i32(&conn, &app_bus, app_path.as_str(), "ChildCount").unwrap_or(0);
            if count <= 0 {
                continue;
            }
            if let Some(path) = find_timecode(&conn, &app_bus, app_path.as_str(), 0)? {
                return Ok(TimecodeNode {
                    conn,
                    bus: app_bus,
                    path,
                });
            }
        }

        Err("Audacity 4.0 timecode was not found".into())
    }

    fn find_timecode(
        conn: &Connection,
        bus: &str,
        path: &str,
        depth: usize,
    ) -> Result<Option<String>, Box<dyn Error>> {
        if depth > 14 {
            return Ok(None);
        }
        let accessible = Proxy::new(conn, bus, path, "org.a11y.atspi.Accessible")?;
        let name = get_string(conn, bus, path, "Name").unwrap_or_default();
        if super::looks_like_timecode(&name) {
            return Ok(Some(path.to_string()));
        }

        let count = get_i32(conn, bus, path, "ChildCount").unwrap_or(0);
        if count <= 0 {
            return Ok(None);
        }
        let children: Vec<(String, OwnedObjectPath)> = accessible.call("GetChildren", &())?;
        for (_, child) in children {
            if let Some(found) = find_timecode(conn, bus, child.as_str(), depth + 1)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    fn get_prop(
        conn: &Connection,
        bus: &str,
        path: &str,
        property: &str,
    ) -> Result<OwnedValue, Box<dyn Error>> {
        let props = Proxy::new(conn, bus, path, "org.freedesktop.DBus.Properties")?;
        Ok(props.call("Get", &("org.a11y.atspi.Accessible", property))?)
    }

    fn get_string(
        conn: &Connection,
        bus: &str,
        path: &str,
        property: &str,
    ) -> Result<String, Box<dyn Error>> {
        Ok(String::try_from(get_prop(conn, bus, path, property)?)?)
    }

    fn get_i32(
        conn: &Connection,
        bus: &str,
        path: &str,
        property: &str,
    ) -> Result<i32, Box<dyn Error>> {
        Ok(i32::try_from(get_prop(conn, bus, path, property)?)?)
    }
}

// ---------------------------------------------------------------- Windows

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use windows_sys::core::{BSTR, GUID, HRESULT};
    use windows_sys::Win32::Foundation::{
        CloseHandle, SysFreeString, SysStringLen, FALSE, HWND, LPARAM, TRUE,
    };
    use windows_sys::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, WNDENUMPROC,
    };

    use super::{
        disconnected, live_state, set_state, Snapshot, Tracker, POLL, REDISCOVER,
    };

    const CLSID_CUIAUTOMATION: GUID =
        GUID::from_u128(0xFF48DBA4_60EF_4201_AA87_54103EEF594E);
    const IID_IUIAUTOMATION: GUID =
        GUID::from_u128(0x30CBE57D_D9D0_452A_AB13_7AC5AC4825EE);
    const IID_IUIAUTOMATION_VALUE_PATTERN: GUID =
        GUID::from_u128(0xA94CD8B1_0844_4CD6_9D2D_640537AB39E9);

    // UI Automation vtable positions (after the three IUnknown methods).
    // IUIAutomation order: CompareElements(3), CompareRuntimeIds(4),
    // GetRootElement(5), ElementFromHandle(6), ElementFromPoint(7),
    // GetFocusedElement(8), the four BuildCache methods (9-12),
    // CreateCacheRequest(13), CreateTrueCondition(14).
    const UIA_GET_ROOT_ELEMENT: usize = 5;
    const UIA_ELEMENT_FROM_HANDLE: usize = 6;
    const UIA_CREATE_TRUE_CONDITION: usize = 14;

    // IUIAutomationElement order: SetFocus(3), GetRuntimeId(4), FindFirst(5),
    // FindAll(6), the two build-cache finders (7-8), BuildUpdatedCache(9),
    // GetCurrentPropertyValue/Ex (10-11), GetCachedPropertyValue/Ex (12-13),
    // GetCurrentPatternAs (14), GetCachedPatternAs (15), GetCurrentPattern(16),
    // GetCachedPattern(17), GetCachedParent(18), GetCachedChildren(19), then
    // the current property getters starting at ProcessId(20). Name is 23;
    // IsOffscreen comes after ControlType/LocalizedControlType/Name (23),
    // AcceleratorKey (24), AccessKey (25), HasKeyboardFocus (26),
    // IsKeyboardFocusable (27), IsEnabled (28), AutomationId (29), ClassName
    // (30), HelpText (31), Culture (32), IsControlElement (33),
    // IsContentElement (34), IsPassword (35), NativeWindowHandle (36),
    // ItemType (37) and IsOffscreen (38).
    const ELEMENT_FIND_ALL: usize = 6;
    const ELEMENT_GET_CURRENT_NAME: usize = 23;
    const ELEMENT_GET_CURRENT_IS_OFFSCREEN: usize = 38;
    const ELEMENT_GET_CURRENT_PATTERN: usize = 16;

    const ARRAY_GET_LENGTH: usize = 3;
    const ARRAY_GET_ELEMENT: usize = 4;

    const VALUE_GET_CURRENT_VALUE: usize = 4;

    const TREE_SCOPE_DESCENDANTS: i32 = 4;
    const UIA_VALUE_PATTERN_ID: i32 = 10002;

    // ------------------------------------------------------------ COM glue
    //
    // windows-sys carries the plain Win32 calls but not the UI Automation COM
    // interfaces, so the few vtables we need are called by index. Keeping the
    // wrappers tiny makes the method numbers above the only ABI detail.

    unsafe fn vtable_slot(this: *mut c_void, index: usize) -> *mut c_void {
        let table = *(this as *mut *mut *mut c_void);
        *table.add(index)
    }

    macro_rules! call {
        ($this:expr, $index:expr, $sig:ty $(, $arg:expr)*) => {{
            let f: $sig = std::mem::transmute(vtable_slot($this, $index));
            f($this $(, $arg)*)
        }};
    }

    unsafe fn release(this: *mut c_void) {
        if !this.is_null() {
            let f: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(vtable_slot(this, 2));
            f(this);
        }
    }

    /// An owned COM pointer that releases itself on drop.
    struct Com(*mut c_void);

    impl Com {
        fn new(ptr: *mut c_void) -> Self {
            Com(ptr)
        }

        fn raw(&self) -> *mut c_void {
            self.0
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            unsafe { release(self.0) };
        }
    }

    fn bstr_to_string(value: BSTR) -> Option<String> {
        if value.is_null() {
            return None;
        }
        unsafe {
            let len = SysStringLen(value) as usize;
            let text = if len == 0 {
                String::new()
            } else {
                String::from_utf16_lossy(std::slice::from_raw_parts(value, len))
            };
            SysFreeString(value);
            Some(text)
        }
    }

    unsafe fn current_name(element: *mut c_void) -> Option<String> {
        let mut value: BSTR = std::ptr::null();
        let hr: HRESULT = call!(
            element,
            ELEMENT_GET_CURRENT_NAME,
            unsafe extern "system" fn(*mut c_void, *mut BSTR) -> HRESULT,
            &mut value
        );
        if hr >= 0 {
            bstr_to_string(value)
        } else {
            None
        }
    }

    unsafe fn current_value(element: *mut c_void) -> Option<String> {
        let mut unknown: *mut c_void = std::ptr::null_mut();
        let hr: HRESULT = call!(
            element,
            ELEMENT_GET_CURRENT_PATTERN,
            unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> HRESULT,
            UIA_VALUE_PATTERN_ID,
            &mut unknown
        );
        if hr < 0 || unknown.is_null() {
            return None;
        }
        let unknown = Com::new(unknown);

        // QueryInterface for the typed value-pattern interface.
        let mut pattern: *mut c_void = std::ptr::null_mut();
        let hr: HRESULT = call!(
            unknown.raw(),
            0,
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
            &IID_IUIAUTOMATION_VALUE_PATTERN,
            &mut pattern
        );
        if hr < 0 || pattern.is_null() {
            return None;
        }
        let pattern = Com::new(pattern);
        let mut value: BSTR = std::ptr::null();
        let hr: HRESULT = call!(
            pattern.raw(),
            VALUE_GET_CURRENT_VALUE,
            unsafe extern "system" fn(*mut c_void, *mut BSTR) -> HRESULT,
            &mut value
        );
        if hr >= 0 {
            bstr_to_string(value)
        } else {
            None
        }
    }

    unsafe fn is_offscreen(element: *mut c_void) -> bool {
        let mut value: i32 = 0;
        let hr: HRESULT = call!(
            element,
            ELEMENT_GET_CURRENT_IS_OFFSCREEN,
            unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
            &mut value
        );
        hr >= 0 && value != 0
    }

    // ------------------------------------------------------------ discovery

    struct WindowSearch {
        process_id: u32,
        found: HWND,
    }

    unsafe extern "system" fn window_callback(window: HWND, lparam: LPARAM) -> i32 {
        let search = &mut *(lparam as *mut WindowSearch);
        let mut owner = 0u32;
        GetWindowThreadProcessId(window, &mut owner);
        if owner == search.process_id
            && IsWindowVisible(window) != FALSE
            && has_title(window)
        {
            search.found = window;
            return FALSE; // stop enumerating
        }
        TRUE
    }

    unsafe fn has_title(window: HWND) -> bool {
        let mut buffer = [0u16; 96];
        let len = GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32);
        len > 0
    }

    fn process_is_audacity(pid: u32) -> bool {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
            if handle.is_null() {
                return false;
            }
            let mut buffer = [0u16; 1024];
            let mut len = buffer.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut len);
            CloseHandle(handle);
            if ok == FALSE || len == 0 {
                return false;
            }
            String::from_utf16_lossy(&buffer[..len as usize])
                .to_ascii_lowercase()
                .contains("audacity")
        }
    }

    struct ProcessSearch {
        found: HWND,
    }

    unsafe extern "system" fn audacity_window_callback(window: HWND, lparam: LPARAM) -> i32 {
        let search = &mut *(lparam as *mut ProcessSearch);
        if IsWindowVisible(window) == FALSE || !has_title(window) {
            return TRUE;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, &mut pid);
        if pid != 0 && process_is_audacity(pid) {
            search.found = window;
            return FALSE;
        }
        TRUE
    }

    fn audacity_window() -> Option<HWND> {
        let mut search = ProcessSearch {
            found: std::ptr::null_mut(),
        };
        let callback: WNDENUMPROC = Some(audacity_window_callback);
        unsafe {
            EnumWindows(callback, &mut search as *mut _ as LPARAM);
        }
        if search.found.is_null() {
            None
        } else {
            Some(search.found)
        }
    }

    /// Walk Audacity's UIA subtree once and keep the element whose accessible
    /// name or value is a timecode. Discovery is cheap enough to repeat every
    /// twelve seconds, but far too expensive to do thirty times a second.
    fn discover() -> Result<Com, String> {
        unsafe {
            let window = audacity_window().ok_or_else(|| "Audacity not found".to_string())?;

            let mut automation: *mut c_void = std::ptr::null_mut();
            let hr: HRESULT = CoCreateInstance(
                &CLSID_CUIAUTOMATION,
                std::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_IUIAUTOMATION,
                &mut automation,
            );
            if hr < 0 || automation.is_null() {
                return Err("accessibility unavailable".to_string());
            }
            let automation = Com::new(automation);

            let mut root: *mut c_void = std::ptr::null_mut();
            let hr: HRESULT = call!(
                automation.raw(),
                UIA_ELEMENT_FROM_HANDLE,
                unsafe extern "system" fn(*mut c_void, HWND, *mut *mut c_void) -> HRESULT,
                window,
                &mut root
            );
            if hr < 0 || root.is_null() {
                return Err("Audacity not found".to_string());
            }
            let root = Com::new(root);

            let mut condition: *mut c_void = std::ptr::null_mut();
            let hr: HRESULT = call!(
                automation.raw(),
                UIA_CREATE_TRUE_CONDITION,
                unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
                &mut condition
            );
            if hr < 0 || condition.is_null() {
                return Err("accessibility unavailable".to_string());
            }
            let condition = Com::new(condition);

            let mut array: *mut c_void = std::ptr::null_mut();
            let hr: HRESULT = call!(
                root.raw(),
                ELEMENT_FIND_ALL,
                unsafe extern "system" fn(*mut c_void, i32, *mut c_void, *mut *mut c_void) -> HRESULT,
                TREE_SCOPE_DESCENDANTS,
                condition.raw(),
                &mut array
            );
            if hr < 0 || array.is_null() {
                return Err("Audacity timecode was not found".to_string());
            }
            let array = Com::new(array);

            let mut count: i32 = 0;
            let hr: HRESULT = call!(
                array.raw(),
                ARRAY_GET_LENGTH,
                unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
                &mut count
            );
            if hr < 0 {
                return Err("Audacity timecode was not found".to_string());
            }
            let count = count.clamp(0, 20_000);

            for index in 0..count {
                let mut element: *mut c_void = std::ptr::null_mut();
                let hr: HRESULT = call!(
                    array.raw(),
                    ARRAY_GET_ELEMENT,
                    unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> HRESULT,
                    index,
                    &mut element
                );
                if hr < 0 || element.is_null() {
                    continue;
                }
                let element = Com::new(element);

                let name = current_name(element.raw()).unwrap_or_default();
                if super::looks_like_timecode(&name) {
                    return Ok(element);
                }

                // Qt can publish a short label as the name ("Playback
                // position") and put the changing clock in the Value pattern.
                let lower = name.to_ascii_lowercase();
                let might_hold_value = name.trim().is_empty()
                    || lower.contains("position")
                    || lower.contains("timecode")
                    || lower.contains("playback");
                if might_hold_value && !is_offscreen(element.raw()) {
                    if let Some(value) = current_value(element.raw()) {
                        if super::looks_like_timecode(&value) {
                            return Ok(element);
                        }
                    }
                }
            }
        }

        Err("Audacity timecode was not found".to_string())
    }

    fn read(node: &Com) -> Option<String> {
        unsafe {
            if let Some(name) = current_name(node.raw()) {
                if super::parse_timecode(&name).is_some() {
                    return Some(name);
                }
            }
            current_value(node.raw()).filter(|value| super::parse_timecode(value).is_some())
        }
    }

    pub fn poll_loop(state: Arc<Mutex<Snapshot>>, stop: Arc<AtomicBool>) {
        // UI Automation is COM; initialise this worker thread before any UIA
        // call and balance it when the thread ends.
        let com_ready = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) >= 0 };

        let mut node: Option<Com> = None;
        let mut next_discover = Instant::now();
        let mut tracker = Tracker::new();

        while !stop.load(Ordering::Relaxed) {
            if Instant::now() >= next_discover {
                match discover() {
                    Ok(found) => {
                        node = Some(found);
                        next_discover = Instant::now() + REDISCOVER * 8;
                    }
                    Err(message) => {
                        node = None;
                        tracker.reset();
                        next_discover = Instant::now() + Duration::from_secs(1);
                        set_state(&state, disconnected(message));
                    }
                }
            }

            if let Some(current) = node.as_ref() {
                match read(current) {
                    Some(raw) => {
                        if let Some(position) = super::parse_timecode(&raw) {
                            let moving = tracker.observe(position);
                            set_state(&state, live_state(position, moving, &raw));
                        } else {
                            node = None;
                            tracker.reset();
                            set_state(&state, disconnected("timecode format not recognised"));
                        }
                    }
                    None => {
                        node = None;
                        tracker.reset();
                        set_state(&state, disconnected("Audacity closed"));
                    }
                }
            }

            thread::sleep(POLL);
        }

        drop(node);
        if com_ready {
            unsafe { CoUninitialize() };
        }
    }

    // Keep the root getter referenced so the vtable index stays documented even
    // if discovery switches to walking from the desktop root later.
    #[allow(dead_code)]
    fn root_element(automation: *mut c_void) -> Option<Com> {
        unsafe {
            let mut root: *mut c_void = std::ptr::null_mut();
            let hr: HRESULT = call!(
                automation,
                UIA_GET_ROOT_ELEMENT,
                unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
                &mut root
            );
            if hr >= 0 && !root.is_null() {
                Some(Com::new(root))
            } else {
                None
            }
        }
    }

    // The window-search helper is only used by the process-title fallback.
    #[allow(dead_code)]
    fn window_by_pid(pid: u32) -> Option<HWND> {
        let mut search = WindowSearch {
            process_id: pid,
            found: std::ptr::null_mut(),
        };
        let callback: WNDENUMPROC = Some(window_callback);
        unsafe {
            EnumWindows(callback, &mut search as *mut _ as LPARAM);
        }
        if search.found.is_null() {
            None
        } else {
            Some(search.found)
        }
    }
}

// ---------------------------------------------------------------- other

#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use super::{disconnected, set_state, Snapshot, POLL};

    pub fn poll_loop(state: Arc<Mutex<Snapshot>>, stop: Arc<AtomicBool>) {
        set_state(&state, disconnected("accessibility unavailable"));
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(1).max(POLL));
        }
    }
}

// ---------------------------------------------------------------- parsing

/// Audacity 4.0's timecode can be shown as `00h00m00.000s` or as a colon
/// clock. The person asked for the dot to turn green only for the hour /
/// minute / second display, so that choice is reported separately from the
/// value itself.
pub fn timecode_uses_hms(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    let clock = lower.rsplit(':').next().unwrap_or(lower.as_str());
    clock.contains('h') && clock.contains('m') && clock.contains('s')
}

/// Recognise the accessible name that carries Audacity's position. The
/// English label is checked first; the fallback accepts a short name that is a
/// clock, so a translated Audacity still works.
pub fn looks_like_timecode(name: &str) -> bool {
    let text = name.trim();
    if text.is_empty() || text.len() > 96 || parse_timecode(text).is_none() {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    lower.contains("position")
        || lower.contains("timecode")
        || lower.contains(':')
        || timecode_uses_hms(text)
}

/// Parse the text Audacity exposes for its playback position.
///
/// The display can be `00h00m00.000s`, `00 h 00 m 11.037 s`, or a colon clock
/// such as `00:00:11.037`; all describe the same hours:minutes:seconds.mmm
/// value. The readable prefix is removed before the number is read.
pub fn parse_timecode(raw: &str) -> Option<f64> {
    let mut text = raw.trim();
    let first_lower = text.to_ascii_lowercase();
    if first_lower.contains('h') && first_lower.contains('m') && first_lower.contains('s') {
        // An accessible name may be labelled (`Playback position:` in any
        // language) and may use spaces around the units. Remove the label and
        // anything before the first digit, but never touch a colon clock such
        // as `00:00:11.037`.
        if let Some((prefix, rest)) = text.split_once(':') {
            let prefix = prefix.to_ascii_lowercase();
            let rest_lower = rest.to_ascii_lowercase();
            if prefix.contains("position")
                || prefix.contains("timecode")
                || (rest_lower.contains('h')
                    && rest_lower.contains('m')
                    && rest_lower.contains('s'))
            {
                text = rest;
            }
        }
        if !text.starts_with(|c: char| c.is_ascii_digit()) {
            if let Some(index) = text.find(|c: char| c.is_ascii_digit()) {
                text = &text[index..];
            }
        }
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains('h') || lower.contains('m') || lower.contains('s') {
        let mut total = 0.0;
        let mut start = 0usize;
        let mut parts = 0;
        let mut seconds = 0.0;
        for (i, ch) in lower.char_indices() {
            let unit = match ch {
                'h' => 'h',
                'm' => 'm',
                's' => 's',
                _ => continue,
            };
            let number: f64 = text[start..i].trim().parse().ok()?;
            if !number.is_finite() || number < 0.0 {
                return None;
            }
            match unit {
                'h' => total += number * 3600.0,
                'm' => total += number * 60.0,
                's' => seconds = number,
                _ => {}
            }
            parts += 1;
            start = i + ch.len_utf8();
        }
        if parts == 0 {
            return None;
        }
        total += seconds;
        return Some(total.max(0.0));
    }

    // Fall back to the colon clock used by the rest of the interface.
    parse_clock(text)
}

fn parse_clock(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for (i, raw) in parts.iter().enumerate() {
        let value: f64 = raw.trim().parse().ok()?;
        if !value.is_finite() || value < 0.0 {
            return None;
        }
        total += value
            * match parts.len() - i {
                1 => 1.0,
                2 => 60.0,
                3 => 3600.0,
                _ => return None,
            };
    }
    if total.is_finite() {
        Some(total)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_audacity_spaced_timecode() {
        assert_eq!(
            parse_timecode("Playback position:00 h 00 m 11.037 s"),
            Some(11.037)
        );
        assert_eq!(
            parse_timecode("Playback position:01 h 02 m 03.004 s"),
            Some(3723.004)
        );
    }

    #[test]
    fn reads_compact_timecode() {
        assert_eq!(parse_timecode("00h00m00.000s"), Some(0.0));
        assert_eq!(parse_timecode("02h03m04.500s"), Some(7384.5));
        assert_eq!(parse_timecode("00:01:02.250"), Some(62.25));
    }

    #[test]
    fn rejects_text_without_a_clock() {
        assert_eq!(parse_timecode("Playback position:"), None);
        assert_eq!(parse_timecode("Audacity"), None);
    }

    #[test]
    fn finds_a_timecode_even_when_the_label_is_translated() {
        assert!(looks_like_timecode("Playback position:00 h 00 m 11.037 s"));
        assert!(looks_like_timecode("Reproducción:00 h 00 m 11.037 s"));
        assert!(looks_like_timecode("00:00:11.037"));
        assert!(!looks_like_timecode("Playback position:"));
        assert!(!looks_like_timecode("Timeline"));
    }

    #[test]
    fn recognises_the_hour_minute_second_display() {
        assert!(timecode_uses_hms("Playback position:00 h 00 m 11.037 s"));
        assert!(timecode_uses_hms("00h00m11.037s"));
        assert!(!timecode_uses_hms("00:00:11.037"));
    }
}

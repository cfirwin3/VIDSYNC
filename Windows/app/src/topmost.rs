//! Keep the player window above ordinary windows.
//!
//! Macroquad does not expose an "always on top" switch, so this small helper
//! asks the desktop directly: `_NET_WM_STATE_ABOVE` through X11 on Linux, and
//! `SetWindowPos(HWND_TOPMOST)` through Win32 on Windows. It finds this
//! application's own top-level window and leaves every other window untouched.
//! On a session without the needed desktop API the request is simply ignored
//! and the rest of the player keeps working.

#[cfg(target_os = "linux")]
mod platform {
    use std::os::raw::{c_long, c_uchar};
    use std::ptr;
    use x11_dl::xlib;

    const WM_CLASS: &[u8] = b"vidsync";

    unsafe fn class_matches(xlib: &xlib::Xlib, display: *mut xlib::Display, window: u64) -> bool {
        let atom = (xlib.XInternAtom)(
            display,
            b"WM_CLASS\0".as_ptr() as *const std::os::raw::c_char,
            xlib::False,
        );
        if atom == 0 {
            return false;
        }
        let mut actual_type: u64 = 0;
        let mut actual_format: i32 = 0;
        let mut count: u64 = 0;
        let mut after: u64 = 0;
        let mut prop: *mut c_uchar = ptr::null_mut();
        let status = (xlib.XGetWindowProperty)(
            display,
            window,
            atom,
            0,
            1024,
            xlib::False,
            xlib::AnyPropertyType as u64,
            &mut actual_type,
            &mut actual_format,
            &mut count,
            &mut after,
            &mut prop,
        );
        if status != 0 || prop.is_null() {
            return false;
        }
        let bytes = std::slice::from_raw_parts(prop, count as usize);
        let matched = bytes
            .split(|b| *b == 0)
            .any(|part| part.eq_ignore_ascii_case(WM_CLASS));
        (xlib.XFree)(prop as *mut _);
        matched
    }

    unsafe fn find_window(
        xlib: &xlib::Xlib,
        display: *mut xlib::Display,
        parent: u64,
        depth: u32,
    ) -> Option<u64> {
        if depth == 0 {
            return None;
        }
        if class_matches(xlib, display, parent) {
            return Some(parent);
        }
        let mut root: u64 = 0;
        let mut parent_return: u64 = 0;
        let mut children: *mut u64 = ptr::null_mut();
        let mut count: u32 = 0;
        if (xlib.XQueryTree)(
            display,
            parent,
            &mut root,
            &mut parent_return,
            &mut children,
            &mut count,
        ) == 0
            || children.is_null()
        {
            return None;
        }
        let mut found = None;
        for i in 0..count as usize {
            let child = *children.add(i);
            if let Some(window) = find_window(xlib, display, child, depth - 1) {
                found = Some(window);
                break;
            }
        }
        (xlib.XFree)(children as *mut _);
        found
    }

    /// Add or remove `_NET_WM_STATE_ABOVE` on the player window.
    pub fn apply(above: bool) -> bool {
        unsafe {
            let Ok(xlib) = xlib::Xlib::open() else {
                return false;
            };
            let display = (xlib.XOpenDisplay)(ptr::null());
            if display.is_null() {
                return false;
            }
            let root = (xlib.XDefaultRootWindow)(display);
            let Some(window) = find_window(&xlib, display, root, 5) else {
                (xlib.XCloseDisplay)(display);
                return false;
            };

            let state = (xlib.XInternAtom)(
                display,
                b"_NET_WM_STATE\0".as_ptr() as *const std::os::raw::c_char,
                xlib::False,
            );
            let state_above = (xlib.XInternAtom)(
                display,
                b"_NET_WM_STATE_ABOVE\0".as_ptr() as *const std::os::raw::c_char,
                xlib::False,
            );

            // Change the property as well as sending the client message. Some
            // window managers honour one path and ignore the other.
            let mut atoms = [state_above];
            (xlib.XChangeProperty)(
                display,
                window,
                state,
                4, // XA_ATOM
                32,
                xlib::PropModeReplace,
                atoms.as_mut_ptr() as *mut c_uchar,
                if above { 1 } else { 0 },
            );

            let mut event: xlib::XEvent = std::mem::zeroed();
            event.client_message.type_ = xlib::ClientMessage;
            event.client_message.send_event = xlib::True;
            event.client_message.display = display;
            event.client_message.window = window;
            event.client_message.message_type = state;
            event.client_message.format = 32;
            event
                .client_message
                .data
                .set_long(0, if above { 1 } else { 0 });
            event.client_message.data.set_long(1, state_above as c_long);
            event.client_message.data.set_long(2, 0);
            event.client_message.data.set_long(3, 1); // source: normal application
            (xlib.XSendEvent)(
                display,
                root,
                xlib::False,
                (xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask) as c_long,
                &mut event,
            );
            if above {
                (xlib.XRaiseWindow)(display, window);
            }
            (xlib.XFlush)(display);
            (xlib.XCloseDisplay)(display);
            true
        }
    }
}

#[cfg(windows)]
mod platform {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, FALSE, TRUE};
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, SetWindowPos,
        HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WNDENUMPROC,
    };

    struct Search {
        process_id: u32,
        found: HWND,
    }

    unsafe extern "system" fn callback(window: HWND, lparam: LPARAM) -> i32 {
        let search = &mut *(lparam as *mut Search);
        let mut owner = 0u32;
        GetWindowThreadProcessId(window, &mut owner);
        if owner != search.process_id || IsWindowVisible(window) == FALSE {
            return TRUE;
        }

        // miniquad also creates a hidden message window; the visible main
        // window is the one carrying the application title.
        let mut title = [0u16; 64];
        let len = GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32);
        if len > 0 && String::from_utf16_lossy(&title[..len as usize]).eq_ignore_ascii_case("VIDSYNC")
        {
            search.found = window;
            return FALSE;
        }
        TRUE
    }

    fn main_window() -> Option<HWND> {
        let mut search = Search {
            process_id: unsafe { GetCurrentProcessId() },
            found: std::ptr::null_mut(),
        };
        let callback: WNDENUMPROC = Some(callback);
        unsafe {
            EnumWindows(callback, &mut search as *mut _ as LPARAM);
        }
        if search.found.is_null() {
            None
        } else {
            Some(search.found)
        }
    }

    /// Add or remove the Win32 topmost flag on the player window.
    pub fn apply(above: bool) -> bool {
        let Some(window) = main_window() else {
            return false;
        };
        let insert_after = if above { HWND_TOPMOST } else { HWND_NOTOPMOST };
        unsafe {
            SetWindowPos(
                window,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            ) != 0
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    pub fn apply(_above: bool) -> bool {
        false
    }
}

pub use platform::apply;

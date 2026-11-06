//! Keeps the main window above other windows ("置顶"), like flomo's pin button.

use gpui_kit::Window;

#[cfg(target_os = "macos")]
pub fn set_pinned(window: &Window, pinned: bool) -> bool {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    const NS_NORMAL_WINDOW_LEVEL: isize = 0;
    const NS_FLOATING_WINDOW_LEVEL: isize = 3;

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return false;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return false;
    };
    // SAFETY: the handle comes from the live GPUI window on the main thread; `window`
    // returns the NSWindow hosting the view and `setLevel:` takes an NSInteger.
    unsafe {
        let view = appkit.ns_view.as_ptr() as *mut AnyObject;
        let ns_window: *mut AnyObject = msg_send![view, window];
        if ns_window.is_null() {
            return false;
        }
        let level = if pinned { NS_FLOATING_WINDOW_LEVEL } else { NS_NORMAL_WINDOW_LEVEL };
        let _: () = msg_send![ns_window, setLevel: level];
    }
    true
}

#[cfg(not(target_os = "macos"))]
pub fn set_pinned(_: &Window, _: bool) -> bool {
    false
}

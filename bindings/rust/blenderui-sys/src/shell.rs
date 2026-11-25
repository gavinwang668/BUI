//! The desktop shell (`kit/include/BUI_kit.h`, "desktop shell"): tray icons, notifications,
//! taskbar progress, recent files. UI thread only.

use std::os::raw::{c_char, c_double, c_float, c_int, c_void};

use crate::BKWindow;

/// A tray icon (`BKTray`).
#[repr(C)]
pub struct BKTray {
    _private: [u8; 0],
}

/// A menu item (`BKMenuSpec`): NULL or "-" text is a separator; `checked` -1 none, 0/1.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BKMenuSpec {
    pub text: *const c_char,
    pub icon: c_int,
    pub shortcut: *const c_char,
    pub checked: c_int,
    pub enabled: c_int,
    /// What `BK_EV_CALL` hands back (retained through `BK_set_ref_hooks` while set).
    pub ref_: *mut c_void,
    pub sub: *const BKMenuSpec,
    pub nsub: c_int,
    pub fn_: Option<unsafe extern "C" fn(data: *mut c_void)>,
    pub data: *mut c_void,
}

/// The one listener of every shell event (`BKShellCallbacks`): `BK_EV_CLICK` (a = button),
/// `BK_EV_DOUBLE`, `BK_EV_CALL` (ref, a = item id), `BK_EV_ACTIVATE` (tray NULL, a = notification).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BKShellCallbacks {
    pub data: *mut c_void,
    pub emit: Option<
        unsafe extern "C" fn(data: *mut c_void, tray: *mut BKTray, kind: c_int, ref_: *mut c_void, a: c_double,
                             b: c_double),
    >,
}

// What this build and system have (`BK_shell_features`).
pub const BK_SHELL_TRAY: c_int = 1;
pub const BK_SHELL_NOTIFY: c_int = 2;
pub const BK_SHELL_PROGRESS: c_int = 4;
pub const BK_SHELL_RECENT: c_int = 8;

// Taskbar progress states (`BK_PROGRESS_*`).
pub const BK_PROGRESS_NONE: c_int = 0;
pub const BK_PROGRESS_NORMAL: c_int = 1;
pub const BK_PROGRESS_INDETERMINATE: c_int = 2;
pub const BK_PROGRESS_PAUSED: c_int = 3;
pub const BK_PROGRESS_ERROR: c_int = 4;

unsafe extern "C" {
    pub fn BK_shell_features() -> c_int;
    pub fn BK_shell_set_app_id(id: *const c_char);
    pub fn BK_shell_set_callbacks(cb: *const BKShellCallbacks);
    pub fn BK_tray_new(icon: *const c_char, tooltip: *const c_char) -> *mut BKTray;
    pub fn BK_tray_free(tray: *mut BKTray);
    pub fn BK_tray_set_icon(tray: *mut BKTray, icon: *const c_char) -> c_int;
    pub fn BK_tray_set_tooltip(tray: *mut BKTray, tooltip: *const c_char) -> c_int;
    pub fn BK_tray_set_menu(tray: *mut BKTray, items: *const BKMenuSpec, n: c_int) -> c_int;
    pub fn BK_notify(title: *const c_char, body: *const c_char, icon: *const c_char) -> c_int;
    pub fn BK_window_set_progress(win: *mut BKWindow, state: c_int, value: c_float) -> c_int;
    pub fn BK_shell_add_recent(path: *const c_char) -> c_int;
}

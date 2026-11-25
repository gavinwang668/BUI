//! The desktop shell: tray icons with menus, notifications, the taskbar's progress and the
//! system's recent documents (the C kit's `BK_tray_*`, `BK_notify`, `BK_window_set_progress`,
//! `BK_shell_add_recent`; kit/intern/shell).
//!
//! ```ignore
//! let tray = Tray::new(Some("BLENDER"), "My app").expect("no tray here");
//! tray.set_menu(&[
//!     MenuItem::action(1, "Open"),
//!     MenuItem::check(2, "Mute", false),
//!     MenuItem::Separator,
//!     MenuItem::action(3, "Quit"),
//! ]);
//! cx.on_shell_event(|event, cx| match event {
//!     ShellEvent::MenuItem { id: 3, .. } => cx.quit(),
//!     ShellEvent::NotificationClicked(_) => { /* bring a window forward */ }
//!     _ => {}
//! });
//! shell::notify("Done", "The export finished", None);
//! window.set_taskbar_progress(TaskbarProgress::Normal(0.4));
//! ```
//!
//! Windows: Shell_NotifyIcon (notifications are the tray icon's balloon, shown as a toast),
//! ITaskbarList3, SHAddToRecentDocs. Linux: StatusNotifierItem + dbusmenu, desktop notifications
//! and the launcher entry over D-Bus (libdbus-1 loaded at run time), GTK's recently-used.xbel.
//! macOS: NSStatusItem, UNUserNotificationCenter (bundled apps), the Dock icon, NSDocumentController.
//! What a system lacks, [`features`] leaves out, and its calls do nothing (`None` / `false`).

use std::borrow::Cow;
use std::cell::Cell;
use std::ffi::CString;
use std::os::raw::{c_char, c_double, c_int, c_void};
use std::path::Path;
use std::ptr::NonNull;

use crate::sys;
use crate::window::{run_hooks, with_running_app, App, Window};

/// What this build and system have ([`features`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ShellFeatures(c_int);

impl ShellFeatures {
    /// Tray icons ([`Tray`]).
    pub const fn tray(self) -> bool {
        self.0 & sys::BK_SHELL_TRAY != 0
    }
    /// Notifications ([`notify`]).
    pub const fn notifications(self) -> bool {
        self.0 & sys::BK_SHELL_NOTIFY != 0
    }
    /// The taskbar's progress ([`Window::set_taskbar_progress`]).
    pub const fn progress(self) -> bool {
        self.0 & sys::BK_SHELL_PROGRESS != 0
    }
    /// Recent documents ([`add_recent_document`]).
    pub const fn recent_documents(self) -> bool {
        self.0 & sys::BK_SHELL_RECENT != 0
    }
}

/// What this build (the `shell` feature) and this system have: Linux needs libdbus-1 and a
/// session bus for notifications and progress, and a tray host (StatusNotifierWatcher) for trays.
pub fn features() -> ShellFeatures {
    // SAFETY: no arguments; asks the system once.
    ShellFeatures(unsafe { sys::BK_shell_features() })
}

/// `s` as a NUL-terminated string for the call (cut at an inner NUL): on the stack when short.
fn with_c<R>(s: &str, f: impl FnOnce(*const c_char) -> R) -> R {
    let s = s.split('\0').next().unwrap_or_default().as_bytes();
    let mut stack = [0u8; 256];
    if s.len() < stack.len() {
        stack[..s.len()].copy_from_slice(s);
        f(stack.as_ptr().cast())
    } else {
        // no inner NUL is left: the conversion cannot fail
        let owned = CString::new(s).unwrap_or_default();
        f(owned.as_ptr())
    }
}

fn with_opt_c<R>(s: Option<&str>, f: impl FnOnce(*const c_char) -> R) -> R {
    match s {
        Some(s) => with_c(s, f),
        None => f(std::ptr::null()),
    }
}

/// Who the app is to the shell: Windows' AppUserModelID, the desktop file's id on Linux
/// (`"org.example.App"` for `org.example.App.desktop`). Set it before the first tray or
/// notification; the default is the executable's name.
pub fn set_app_id(id: &str) {
    // SAFETY: a NUL-terminated string for the call (the kit copies it).
    with_c(id, |p| unsafe { sys::BK_shell_set_app_id(p) });
}

/// Shows a desktop notification; `icon` is an image file or a Blender icon's name (`None`: the
/// app's). Its id, which [`ShellEvent::NotificationClicked`] reports; `None` when it can't show.
pub fn notify(title: &str, body: &str, icon: Option<&str>) -> Option<NotificationId> {
    // SAFETY: NUL-terminated strings for the call (copied).
    let id = with_c(title, |t| with_c(body, |b| with_opt_c(icon, |i| unsafe { sys::BK_notify(t, b, i) })));
    (id > 0).then_some(NotificationId(id as u32))
}

/// Adds a file to the system's recent documents (Windows' jump list and Explorer, macOS'
/// Open Recent, GTK's recent files). False when the file does not exist or there is no list.
pub fn add_recent_document(path: impl AsRef<Path>) -> bool {
    let Some(path) = path.as_ref().to_str() else { return false };
    // SAFETY: a NUL-terminated string for the call.
    with_c(path, |p| unsafe { sys::BK_shell_add_recent(p) } != 0)
}

/// A notification ([`notify`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NotificationId(pub u32);

/// A tray icon ([`Tray::id`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TrayId(usize);

/// What the shell reports ([`App::on_shell_event`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellEvent {
    /// A tray icon was clicked: `button` 1 left, 2 middle, 3 right (a right click on an icon
    /// with a menu opens the menu instead).
    TrayClicked { tray: TrayId, button: u8 },
    /// A tray icon was double-clicked (Windows).
    TrayDoubleClicked { tray: TrayId },
    /// A tray menu item was chosen: its [`MenuItem`] id.
    MenuItem { tray: TrayId, id: u32 },
    /// A notification was clicked.
    NotificationClicked(NotificationId),
}

/// The taskbar button's progress ([`Window::set_taskbar_progress`]); values 0.0 ..= 1.0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaskbarProgress {
    Hidden,
    Normal(f32),
    /// Busy, with no value.
    Indeterminate,
    Paused(f32),
    Error(f32),
}

impl TaskbarProgress {
    const fn raw(self) -> (c_int, f32) {
        match self {
            Self::Hidden => (sys::BK_PROGRESS_NONE, 0.0),
            Self::Normal(v) => (sys::BK_PROGRESS_NORMAL, v),
            Self::Indeterminate => (sys::BK_PROGRESS_INDETERMINATE, 0.0),
            Self::Paused(v) => (sys::BK_PROGRESS_PAUSED, v),
            Self::Error(v) => (sys::BK_PROGRESS_ERROR, v),
        }
    }
}

impl Window {
    /// Shows `progress` on the window's taskbar button (Windows: ITaskbarList3; macOS: the Dock
    /// icon; Linux: the app's launcher entry). False off screen or without one.
    pub fn set_taskbar_progress(&self, progress: TaskbarProgress) -> bool {
        let Some(native) = self.native() else { return false };
        let (state, value) = progress.raw();
        // SAFETY: a live native window; the kit clamps the value.
        unsafe { sys::BK_window_set_progress(native, state, value) != 0 }
    }
}

/// A tray menu's item ([`Tray::set_menu`]). `id`s come back in [`ShellEvent::MenuItem`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuItem<'a> {
    Action { id: u32, label: &'a str, enabled: bool },
    Check { id: u32, label: &'a str, checked: bool, enabled: bool },
    /// Borrowed (`&[..]`) or owned (`vec![..]`) items.
    Submenu { label: &'a str, items: Cow<'a, [MenuItem<'a>]> },
    Separator,
}

impl<'a> MenuItem<'a> {
    pub const fn action(id: u32, label: &'a str) -> Self {
        Self::Action { id, label, enabled: true }
    }
    pub const fn check(id: u32, label: &'a str, checked: bool) -> Self {
        Self::Check { id, label, checked, enabled: true }
    }
    pub fn submenu(label: &'a str, items: impl Into<Cow<'a, [MenuItem<'a>]>>) -> Self {
        Self::Submenu { label, items: items.into() }
    }
    /// The same item, greyed out (actions and checks).
    #[must_use]
    pub fn disabled(self) -> Self {
        match self {
            Self::Action { id, label, .. } => Self::Action { id, label, enabled: false },
            Self::Check { id, label, checked, .. } => Self::Check { id, label, checked, enabled: false },
            other => other,
        }
    }
}

/// The items of every level, a submenu's after its parent's level (one array, so the pointers
/// between levels hold), and their labels in one buffer: two allocations per menu.
struct Specs {
    specs: Vec<sys::BKMenuSpec>,
    text: Vec<u8>,
}

impl Specs {
    fn new(items: &[MenuItem<'_>]) -> Self {
        fn sizes(items: &[MenuItem<'_>], n: &mut usize, bytes: &mut usize) {
            for it in items {
                *n += 1;
                match it {
                    MenuItem::Action { label, .. } | MenuItem::Check { label, .. } => *bytes += label.len() + 1,
                    MenuItem::Submenu { label, items } => {
                        *bytes += label.len() + 1;
                        sizes(items, n, bytes);
                    }
                    MenuItem::Separator => {}
                }
            }
        }
        let (mut n, mut bytes) = (0, 0);
        sizes(items, &mut n, &mut bytes);
        let mut s = Self { specs: Vec::with_capacity(n), text: Vec::with_capacity(bytes) };
        s.place(items);
        s
    }

    /// `label` NUL-terminated in the buffer; its capacity was reserved, so it never moves.
    fn label(&mut self, label: &str) -> *const c_char {
        let at = self.text.len();
        self.text.extend(label.split('\0').next().unwrap_or_default().bytes());
        self.text.push(0);
        // SAFETY: `at` is inside the buffer, which is not reallocated (capacity reserved).
        unsafe { self.text.as_ptr().add(at).cast() }
    }

    /// Places one level, then each submenu's; returns where the level starts.
    fn place(&mut self, items: &[MenuItem<'_>]) -> usize {
        let start = self.specs.len();
        for it in items {
            let mut spec = sys::BKMenuSpec {
                text: std::ptr::null(),
                icon: 0,
                shortcut: std::ptr::null(),
                checked: -1,
                enabled: 1,
                ref_: std::ptr::null_mut(),
                sub: std::ptr::null(),
                nsub: 0,
                fn_: None,
                data: std::ptr::null_mut(),
            };
            match it {
                &MenuItem::Action { id, label, enabled } => {
                    spec.text = self.label(label);
                    spec.enabled = c_int::from(enabled);
                    spec.ref_ = id as usize as *mut c_void; // the id, not a pointer: no ref hooks
                }
                &MenuItem::Check { id, label, checked, enabled } => {
                    spec.text = self.label(label);
                    spec.checked = c_int::from(checked);
                    spec.enabled = c_int::from(enabled);
                    spec.ref_ = id as usize as *mut c_void;
                }
                MenuItem::Submenu { label, items } => {
                    spec.text = self.label(label);
                    spec.nsub = items.len() as c_int;
                }
                MenuItem::Separator => {}
            }
            self.specs.push(spec);
        }
        for (i, it) in items.iter().enumerate() {
            if let MenuItem::Submenu { items, .. } = it {
                let sub = self.place(items);
                // SAFETY: `sub` is inside `specs`, which is not reallocated (capacity reserved).
                self.specs[start + i].sub = unsafe { self.specs.as_ptr().add(sub) };
            }
        }
        start
    }
}

/// A tray icon (Windows' notification area, the menu bar's status items, a Linux panel's tray).
/// UI thread only (`!Send`); removed when dropped.
pub struct Tray {
    ptr: NonNull<sys::BKTray>,
}

impl Tray {
    /// `icon`: an image file or a Blender icon's name (`"BLENDER"`), `None` for the app's own.
    /// `None` when this system has no tray ([`features`]).
    pub fn new(icon: Option<&str>, tooltip: &str) -> Option<Self> {
        // SAFETY: NUL-terminated strings for the call (copied).
        let ptr = with_opt_c(icon, |i| with_c(tooltip, |t| unsafe { sys::BK_tray_new(i, t) }));
        NonNull::new(ptr).map(|ptr| Self { ptr })
    }

    /// What its events carry.
    pub fn id(&self) -> TrayId {
        TrayId(self.ptr.as_ptr() as usize)
    }

    /// False when `icon` is no image or icon BUI reads (the old one stays).
    pub fn set_icon(&self, icon: Option<&str>) -> bool {
        // SAFETY: a live tray (all methods below likewise).
        with_opt_c(icon, |i| unsafe { sys::BK_tray_set_icon(self.ptr.as_ptr(), i) } != 0)
    }

    pub fn set_tooltip(&self, tooltip: &str) -> bool {
        with_c(tooltip, |t| unsafe { sys::BK_tray_set_tooltip(self.ptr.as_ptr(), t) } != 0)
    }

    /// Its menu (copied; empty removes it), opened by a right click.
    pub fn set_menu(&self, items: &[MenuItem<'_>]) -> bool {
        let specs = Specs::new(items);
        // SAFETY: `items.len()` top-level items at the front of `specs`, alive for the call.
        unsafe { sys::BK_tray_set_menu(self.ptr.as_ptr(), specs.specs.as_ptr(), items.len() as c_int) != 0 }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: ours.
        unsafe { sys::BK_tray_free(self.ptr.as_ptr()) };
    }
}

impl std::fmt::Debug for Tray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Tray").field(&self.id()).finish()
    }
}

// ==========================================================================
// Events

thread_local! {
    /// Events that came while a callback held the app: handled on its next turn.
    static PENDING: Cell<Vec<ShellEvent>> = const { Cell::new(Vec::new()) };
}

unsafe extern "C" fn on_shell(_data: *mut c_void, tray: *mut sys::BKTray, kind: c_int, ref_: *mut c_void,
                              a: c_double, _b: c_double) {
    let tray = TrayId(tray as usize);
    let event = match kind {
        sys::BK_EV_CLICK => ShellEvent::TrayClicked { tray, button: a as u8 },
        sys::BK_EV_DOUBLE => ShellEvent::TrayDoubleClicked { tray },
        sys::BK_EV_CALL => ShellEvent::MenuItem { tray, id: ref_ as usize as u32 },
        sys::BK_EV_ACTIVATE => ShellEvent::NotificationClicked(NotificationId(a as u32)),
        _ => return,
    };
    let ran = with_running_app(|app| {
        drain(app);
        dispatch(app, &event);
    });
    if !ran {
        PENDING.with(|p| {
            let mut v = p.take();
            v.push(event);
            p.set(v);
        });
        // SAFETY: posts a message; the loop's wake drains the app (App::drain)
        unsafe { sys::BK_window_wake() };
    }
}

fn dispatch(app: &mut App, event: &ShellEvent) {
    run_hooks(app, |a| &mut a.shell_hooks, |h, app| h(event, app));
}

/// The events that waited for the app (App::drain).
pub(crate) fn drain(app: &mut App) {
    let pending = PENDING.with(Cell::take);
    for event in &pending {
        dispatch(app, event);
    }
}

impl App {
    /// Call `f` with every desktop shell event: tray clicks, tray menu items, notification clicks.
    pub fn on_shell_event(&mut self, f: impl FnMut(&ShellEvent, &mut App) + 'static) {
        if self.shell_hooks.is_empty() {
            let cb = sys::BKShellCallbacks { data: std::ptr::null_mut(), emit: Some(on_shell) };
            // SAFETY: the kit copies the struct; `on_shell` lives for the program.
            unsafe { sys::BK_shell_set_callbacks(&cb) };
        }
        self.shell_hooks.push(Box::new(f));
    }
}

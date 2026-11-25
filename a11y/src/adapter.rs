//! The platform adapter of one window: serves the kit's tree to the platform's accessibility
//! API and queues what screen readers ask for. Windows: UI Automation (accesskit_windows).
//! Elsewhere there is no adapter yet (`Adapter::new` gives none), so the kit builds no tree.
//!
//! The request queue and the activation handler are AccessKit's platform-neutral handlers: the
//! macOS and Linux adapters take them as they are (until they come, nothing there uses them).
#![cfg_attr(not(windows), allow(dead_code, clippy::missing_const_for_fn))]

use std::collections::VecDeque;
use std::ffi::{CString, c_void};
use std::sync::{Arc, Mutex, PoisonError};

use crate::node::{Action, Node, Request, Shape, validate};
use crate::tree::Tree;

/// `BuiA11yHost`: how the adapter reaches the window that owns it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Host {
    pub data: *mut c_void,
    /// The whole tree now (a reader attached): sets the nodes, their count and the focused id.
    /// Returns 0 when it has none. Called on the window's thread.
    pub tree: Option<unsafe extern "C" fn(*mut c_void, *mut *const Node, *mut i32, *mut u64) -> i32>,
    /// A request is waiting for `bui_a11y_take`. Called from any thread.
    pub wake: Option<unsafe extern "C" fn(*mut c_void)>,
}

/// A request as a reader made it, kept until the window's thread takes it.
struct Pending {
    id: u64,
    action: Action,
    num: f64,
    text: Option<Box<str>>,
}

/// The host's wake callback, callable from the reader's thread.
#[derive(Clone, Copy)]
struct Waker {
    wake: unsafe extern "C" fn(*mut c_void),
    data: usize,
}

/// The requests waiting for the window's thread (readers call from theirs), and the host's wake
/// while the adapter lives: a reader may hold on to the window's elements after it is gone, and
/// its requests then go nowhere.
struct Queue(Mutex<(VecDeque<Pending>, Option<Waker>)>);

impl Queue {
    const fn new(waker: Option<Waker>) -> Self {
        Self(Mutex::new((VecDeque::new(), waker)))
    }
    /// Queue `p` and wake the host, unless the adapter is gone (under the lock: it can't go meanwhile).
    #[expect(clippy::significant_drop_tightening, reason = "the wake must not race the adapter's drop")]
    fn push(&self, p: Pending) {
        let mut q = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(w) = q.1 {
            q.0.push_back(p);
            // SAFETY: the host promises `wake` may be called from any thread with its data, until
            // the adapter is freed (`close`).
            unsafe { (w.wake)(w.data as *mut c_void) };
        }
    }
    fn pop(&self) -> Option<Pending> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).0.pop_front()
    }
    /// The adapter goes: no more requests, no more wakes.
    fn close(&self) {
        let mut q = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        q.0.clear();
        q.1 = None;
    }
}

/// The requests a reader makes, as the kit's [`Request`]s (unknown ones are dropped).
struct Requests {
    queue: Arc<Queue>,
}

impl accesskit::ActionHandler for Requests {
    fn do_action(&mut self, r: accesskit::ActionRequest) {
        let Some(action) = Action::from_accesskit(r.action) else { return };
        let (num, text) = match r.data {
            Some(accesskit::ActionData::NumericValue(v)) => (v, None),
            Some(accesskit::ActionData::Value(s)) => (0.0, Some(s)),
            _ => (0.0, None),
        };
        self.queue.push(Pending { id: r.target_node.0, action, num, text });
    }
}

/// The whole tree from the host, for an adapter that starts. It holds no borrow (a pointer to the
/// adapter's [`Tree`]), so the answer to `WM_GETOBJECT` can be made after the adapter's borrow
/// ends: making it may send the window another `WM_GETOBJECT`.
struct Activation {
    host: Host,
    tree: *mut Tree,
}

impl accesskit::ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        let f = self.host.tree?;
        let (mut nodes, mut n, mut focus) = (std::ptr::null(), 0i32, 0u64);
        // SAFETY: the host's callback with its data and three out pointers.
        if unsafe { f(self.host.data, &raw mut nodes, &raw mut n, &raw mut focus) } == 0 || nodes.is_null() || n <= 0 {
            return None;
        }
        // SAFETY: the host gives `n` nodes, valid until its next call.
        let nodes = unsafe { std::slice::from_raw_parts(nodes, n as usize) };
        // SAFETY: strings checked right here.
        unsafe { validate(nodes, Shape::Tree) }.ok()?;
        // SAFETY: the adapter's tree, which `wm_getobject` lent for this call.
        let tree = unsafe { &mut *self.tree };
        tree.reset();
        // SAFETY: validated above.
        Some(unsafe { tree.update(nodes, focus, true) })
    }
}

/// One window's adapter (`BuiA11y`).
pub struct Adapter {
    #[cfg(windows)]
    inner: accesskit_windows::Adapter,
    host: Host,
    tree: Tree,
    queue: Arc<Queue>,
    /// A reader asked for the tree: updates go out from then on.
    active: bool,
    /// The text of the request `take` returned last (its `text` points here).
    taken: Option<CString>,
}

impl Drop for Adapter {
    fn drop(&mut self) {
        self.queue.close(); // the window and its host go with the adapter: readers' requests too
    }
}

/// Events the platform must be told about, raised once the adapter is no longer borrowed (raising
/// them may call back into it).
#[must_use]
pub struct Events(#[cfg(windows)] Option<accesskit_windows::QueuedEvents>);

impl Events {
    pub fn raise(self) {
        #[cfg(windows)]
        if let Some(e) = self.0 {
            e.raise();
        }
    }
}

impl Adapter {
    /// The adapter of the native window `window` (an `HWND` on Windows), or `None` where this
    /// platform has none yet.
    ///
    /// # Safety
    /// `window` is the live native window the adapter serves, and outlives it.
    #[must_use]
    pub unsafe fn new(window: *mut c_void, host: Host) -> Option<Box<Self>> {
        if window.is_null() {
            return None;
        }
        let queue = Arc::new(Queue::new(host.wake.map(|wake| Waker { wake, data: host.data as usize })));
        #[cfg(windows)]
        {
            let requests = Requests { queue: Arc::clone(&queue) };
            let hwnd = accesskit_windows::HWND(window);
            // SAFETY: a plain query of the live window.
            let focused = unsafe { GetForegroundWindow() } == window;
            Some(Box::new(Self {
                inner: accesskit_windows::Adapter::new(hwnd, focused, requests),
                host,
                tree: Tree::new(),
                queue,
                active: false,
                taken: None,
            }))
        }
        #[cfg(not(windows))]
        {
            let _ = (host, queue);
            None
        }
    }

    /// Whether a reader asked for the tree.
    #[must_use]
    pub const fn active(&self) -> bool {
        self.active
    }

    /// Send the tree as it is now (only what changed). Nothing before a reader asked.
    ///
    /// # Safety
    /// The node strings are null or NUL-terminated and valid for the call.
    pub unsafe fn update(&mut self, nodes: &[Node], focus: u64) -> Result<Events, crate::node::Invalid> {
        // SAFETY: the caller's contract.
        unsafe { validate(nodes, Shape::Tree) }?;
        if !self.active {
            return Ok(Events(#[cfg(windows)] None));
        }
        let tree = &mut self.tree;
        #[cfg(windows)]
        {
            // SAFETY: validated above.
            let ev = self.inner.update_if_active(|| unsafe { tree.update(nodes, focus, false) });
            Ok(Events(ev))
        }
        #[cfg(not(windows))]
        {
            let _ = (tree, focus);
            Ok(Events())
        }
    }

    /// The window got or lost the keyboard focus.
    pub fn window_focus(&mut self, focused: bool) -> Events {
        #[cfg(windows)]
        {
            Events(self.inner.update_window_focus_state(focused))
        }
        #[cfg(not(windows))]
        {
            let _ = focused;
            Events()
        }
    }

    /// The next waiting request, its text valid until the next call.
    pub fn take(&mut self) -> Option<Request> {
        let p = self.queue.pop()?;
        self.taken = p.text.map(|t| CString::new(t.replace('\0', "")).unwrap_or_default());
        Some(Request {
            id: p.id,
            action: p.action as i32,
            num: p.num,
            text: self.taken.as_deref().map_or(std::ptr::null(), std::ffi::CStr::as_ptr),
        })
    }

    /// `WM_GETOBJECT`: when the adapter answers it, the answer (the UIA provider), to turn into
    /// the `LRESULT` once the adapter is no longer borrowed (that may send another `WM_GETOBJECT`).
    #[cfg(windows)]
    pub fn wm_getobject(&mut self, wparam: usize, lparam: isize)
        -> Option<impl Into<accesskit_windows::LRESULT> + use<>> {
        let mut act = Activation { host: self.host, tree: &raw mut self.tree };
        let asked = self.inner.handle_wm_getobject(
            accesskit_windows::WPARAM(wparam),
            accesskit_windows::LPARAM(lparam),
            &mut act,
        )?;
        self.active = true;
        Some(asked)
    }
}

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetForegroundWindow() -> *mut c_void;
}

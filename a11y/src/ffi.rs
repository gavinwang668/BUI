//! The C ABI the kit calls (declared in include/bui_a11y.h). Every pointer is checked here.

use std::ffi::c_void;

use crate::adapter::{Adapter, Events, Host};
use crate::node::{Invalid, Node, Request, Shape, validate};

/// The nodes behind `nodes`/`n`, or why there are none.
const unsafe fn slice<'a>(nodes: *const Node, n: i32) -> Result<&'a [Node], Invalid> {
    match (nodes.is_null(), n) {
        (_, 0) => Ok(&[]),
        (false, 1..) => {
            // SAFETY: the caller gives `n` nodes.
            Ok(unsafe { std::slice::from_raw_parts(nodes, n as usize) })
        }
        _ => Err(Invalid::Array),
    }
}

const fn code(r: Result<(), Invalid>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => e as i32,
    }
}

/// Check a flat tree: 0 when it is valid, else a `BUI_A11Y_INVALID_*` code. `subtree` nonzero: a
/// host's subtree (any number of top nodes, widgets allowed), else a whole tree.
///
/// # Safety
/// `nodes` points at `n` nodes whose strings are null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bui_a11y_validate(nodes: *const Node, n: i32, subtree: i32) -> i32 {
    let shape = if subtree != 0 { Shape::Subtree } else { Shape::Tree };
    // SAFETY: the caller's contract.
    code(unsafe { slice(nodes, n) }.and_then(|s| unsafe { validate(s, shape) }))
}

/// The adapter of a native window (`HWND` on Windows), or NULL where this platform has none.
///
/// # Safety
/// `window` is a live native window that outlives the adapter; `host` is null or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bui_a11y_new(window: *mut c_void, host: *const Host) -> *mut Adapter {
    if host.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: the caller's contract; the host is copied.
    unsafe { Adapter::new(window, *host) }.map_or(std::ptr::null_mut(), Box::into_raw)
}

/// Free an adapter (NULL: nothing).
///
/// # Safety
/// `a` is null or from `bui_a11y_new`, and not used after.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bui_a11y_free(a: *mut Adapter) {
    if !a.is_null() {
        // SAFETY: the caller's contract.
        drop(unsafe { Box::from_raw(a) });
    }
}

/// 1 once a screen reader asked for the tree (updates go out from then on).
///
/// # Safety
/// `a` is null or a live adapter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bui_a11y_active(a: *const Adapter) -> i32 {
    // SAFETY: the caller's contract.
    i32::from(unsafe { a.as_ref() }.is_some_and(Adapter::active))
}

/// Send the window's tree as it is now (the adapter sends only what changed): 0, or the
/// `BUI_A11Y_INVALID_*` code of a bad tree. Call it on the window's thread.
///
/// # Safety
/// `a` is null or a live adapter; `nodes` points at `n` nodes whose strings are null or
/// NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bui_a11y_update(a: *mut Adapter, nodes: *const Node, n: i32, focus: u64) -> i32 {
    // SAFETY: the caller's contract.
    let Some(adapter) = (unsafe { a.as_mut() }) else { return 0 };
    // SAFETY: the caller's contract.
    let events = unsafe { slice(nodes, n) }.and_then(|s| unsafe { adapter.update(s, focus) });
    // the adapter's borrow has ended: raising may ask it again (WM_GETOBJECT)
    match events {
        Ok(e) => {
            e.raise();
            0
        }
        Err(e) => e as i32,
    }
}

/// The window got (1) or lost (0) the keyboard focus.
///
/// # Safety
/// `a` is null or a live adapter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bui_a11y_window_focus(a: *mut Adapter, focused: i32) {
    // SAFETY: the caller's contract.
    let events: Option<Events> = unsafe { a.as_mut() }.map(|a| a.window_focus(focused != 0));
    if let Some(e) = events {
        e.raise();
    }
}

/// The next request a reader made, into `*r_req` (its text valid until the next call): 1, or 0
/// when none waits. Call it on the window's thread after the host's `wake`.
///
/// # Safety
/// `a` is null or a live adapter; `r_req` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bui_a11y_take(a: *mut Adapter, r_req: *mut Request) -> i32 {
    // SAFETY: the caller's contract.
    let (Some(adapter), false) = (unsafe { a.as_mut() }, r_req.is_null()) else { return 0 };
    match adapter.take() {
        Some(r) => {
            // SAFETY: checked non-null; the caller's contract.
            unsafe { r_req.write(r) };
            1
        }
        None => 0,
    }
}

/// Windows: `WM_GETOBJECT` for the adapter's window. Returns 1 and sets `*r_result` when the
/// adapter answers it, else 0 (pass the message to `DefWindowProc`).
///
/// # Safety
/// `a` is null or a live adapter; `r_result` is writable. Call it from the window procedure.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bui_a11y_wm_getobject(a: *mut Adapter, wparam: usize, lparam: isize, r_result: *mut isize) -> i32 {
    // SAFETY: the caller's contract; the borrow ends with this statement.
    let asked = unsafe { a.as_mut() }.and_then(|a| a.wm_getobject(wparam, lparam));
    match (asked, r_result.is_null()) {
        (Some(asked), false) => {
            // UIA is called here, with the adapter free again (it may send another WM_GETOBJECT)
            let r: accesskit_windows::LRESULT = asked.into();
            // SAFETY: checked non-null.
            unsafe { r_result.write(r.0) };
            1
        }
        _ => 0,
    }
}

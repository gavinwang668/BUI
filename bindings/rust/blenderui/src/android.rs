//! Android: the entry point of a Rust app ([`android_main!`](crate::android_main)).
//!
//! The NativeActivity shell (platform/android/bui_android.c) calls `bui_app_main`; the macro
//! defines it, so an app's code has no `unsafe` and nothing Android-specific. Before the app runs:
//!
//! - **JNI**: the activity's Java VM and object are given to `ndk-context`, which crates that call
//!   into Java read (cpal lists sound devices through it; without it they panic).
//! - **Logs**: stdout and stderr go to logcat (tag `bui`), panics included, so `println!` and
//!   `eprintln!` are seen with `adb logcat -s bui`.
//! - **Folders**: the kit already pointed HOME, XDG_CACHE_HOME and TMPDIR at the app's own
//!   folders (BK_android_attach), so `dirs::cache_dir()` and `std::env::temp_dir()` work.

use std::ffi::{CString, c_char, c_int, c_void};
use std::io::{BufRead, BufReader};
use std::os::fd::IntoRawFd;
use std::sync::Once;

use crate::sys;

/// The head of NDK's `ANativeActivity` (android/native_activity.h): the fields read here.
#[repr(C)]
struct ActivityHead {
    callbacks: *mut c_void,
    vm: *mut c_void,
    env: *mut c_void,
    clazz: *mut c_void,
}

/// logcat's priority of an info line (android/log.h `ANDROID_LOG_INFO`).
const LOG_INFO: c_int = 4;

unsafe extern "C" {
    fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    fn dup2(old: c_int, new: c_int) -> c_int;
}

/// Runs the app `main` inside the running activity: what `bui_app_main` does. Called by
/// [`android_main!`](crate::android_main); an app doesn't call it.
pub fn run(main: fn()) {
    logs_to_logcat();
    // SAFETY: the shell attached the activity (BK_android_attach) before it called bui_app_main.
    let activity = unsafe { sys::BK_android_activity() }.cast::<ActivityHead>();
    if activity.is_null() {
        main();
        return;
    }
    // SAFETY: a live ANativeActivity: its VM and object stay valid until bui_app_main returns, and
    // the context is released below, before they go (a relaunch sets it again).
    unsafe { ndk_context::initialize_android_context((*activity).vm, (*activity).clazz) };
    main();
    // SAFETY: set above, on this thread.
    unsafe { ndk_context::release_android_context() };
}

/// Sends stdout and stderr to logcat, a line at a time, once per process.
fn logs_to_logcat() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let Ok((reader, writer)) = std::io::pipe() else { return };
        let fd = writer.into_raw_fd(); // kept open: it is stdout and stderr from now on
        // SAFETY: plain descriptors; 1 and 2 now write into the pipe.
        unsafe {
            dup2(fd, 1);
            dup2(fd, 2);
        }
        let _ = std::thread::Builder::new().name("bui-logcat".into()).spawn(move || {
            let tag = c"bui";
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                if let Ok(text) = CString::new(line) {
                    // SAFETY: two C strings that live through the call.
                    unsafe { __android_log_write(LOG_INFO, tag.as_ptr(), text.as_ptr()) };
                }
            }
        });
    });
}

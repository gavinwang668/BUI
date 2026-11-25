//! Node.js / Bun bindings (napi-rs): BUI driven from JavaScript, GPUIX style.
//!
//! BUI runs on its own UI thread: the JavaScript thread is never blocked,
//! and an idle UI costs no CPU. JavaScript keeps a retained element tree and
//! sends only what changed: per window and per tick, one `Int32Array` of ops
//! plus a string table. The UI thread applies them to its mirror of the tree
//! (`blenderui-jsview`, shared with the QuickJS runtime) and re-renders;
//! events come back through a thread-safe function.
//!
//! The JavaScript side lives in `bindings/node` (`@blender-ui/native`, with
//! `@blender-ui/react` on top).
//!
//! SPDX-License-Identifier: GPL-2.0-or-later

use std::sync::{Arc, mpsc};

use blenderui::prelude::*;
use blenderui_jsview as view;
use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
    ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi::{Env, JsFunction, JsObject};
use napi_derive::napi;

mod motion;

type Tsfn = ThreadsafeFunction<BuiEvent, ErrorStrategy::Fatal>;

// ==========================================================================
// Values crossing the boundary

/// An event from a window (`kind`: click, mousedown, mouseup, mousemove,
/// mouseenter, mouseleave, wheel, keydown, keyup, change, input, pinch, rotate, pan, frame,
/// close); the fields as in `blenderui_jsview::Event`.
#[napi(object)]
#[derive(Clone, Default)]
pub struct BuiEvent {
    pub kind: String,
    pub window: u32,
    /// The node whose listener fired.
    pub target: u32,
    pub x: f64,
    pub y: f64,
    /// 0 left, 1 middle, 2 right (DOM numbering).
    pub button: u32,
    pub click_count: u32,
    pub buttons: u32,
    pub direction: Option<String>,
    pub dx: f64,
    pub dy: f64,
    pub delta: f64,
    pub delta_x: f64,
    pub delta_y: f64,
    pub phase: Option<String>,
    pub key: Option<String>,
    pub key_char: Option<String>,
    pub value: Option<String>,
    pub checked: bool,
    pub number: f64,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// `motion` (`target`: the motion, `button`: the event bit, `number`: a
    /// value target's value): a plain object's values.
    pub values: Option<Vec<f64>>,
}

impl From<view::Event> for BuiEvent {
    fn from(e: view::Event) -> BuiEvent {
        BuiEvent {
            kind: e.kind, window: e.window, target: e.target, x: e.x, y: e.y, button: e.button,
            click_count: e.click_count, buttons: e.buttons, direction: e.direction, dx: e.dx, dy: e.dy, delta: e.delta,
            delta_x: e.delta_x, delta_y: e.delta_y, phase: e.phase.map(str::to_owned), key: e.key, key_char: e.key_char,
            value: e.value,
            checked: e.checked, number: e.number, shift: e.shift, ctrl: e.ctrl, alt: e.alt, values: e.values,
        }
    }
}

#[napi(object)]
pub struct AppOptions {
    /// Draw on the GPU (Direct3D 11 on Windows, OpenGL 3.3 on Linux).
    pub gpu: Option<bool>,
    /// GPU text from glyph outlines (Slug) or a glyph atlas.
    pub slug: Option<bool>,
    /// No native windows (tests, screenshots, CI).
    pub headless: Option<bool>,
    /// Blender's "dark" (default) or "light" theme.
    pub theme: Option<String>,
}

#[napi(object)]
#[derive(Clone)]
pub struct WindowConfig {
    pub title: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub resizable: Option<bool>,
    pub gpu: Option<bool>,
    pub transparent: Option<bool>,
    pub always_on_top: Option<bool>,
    pub frame: Option<bool>,
    pub layer_shell: Option<LayerShellConfig>,
}

#[napi(object)]
#[derive(Clone)]
pub struct LayerShellConfig {
    pub layer: Option<String>,
    pub anchor: Option<Vec<String>>,
    pub exclusive_zone: Option<f64>,
    pub keyboard: Option<String>,
}

/// A tray menu item (Electron's `MenuItem` options): `type` "normal" (default), "checkbox",
/// "separator" or "submenu".
#[napi(object)]
pub struct TrayMenuItem {
    /// What the `traymenu` event reports (chosen by JavaScript).
    pub id: u32,
    pub label: Option<String>,
    #[napi(js_name = "type")]
    pub kind: Option<String>,
    pub checked: Option<bool>,
    pub enabled: Option<bool>,
    pub submenu: Option<Vec<TrayMenuItem>>,
}

/// What this build and system have of the desktop shell.
#[napi(object)]
pub struct ShellFeatures {
    pub tray: bool,
    pub notifications: bool,
    pub progress: bool,
    pub recent_documents: bool,
}

fn tray_items(items: Vec<TrayMenuItem>) -> Result<Vec<view::shell::TrayItem>> {
    items.into_iter().map(|it| {
        let kind = match it.kind.as_deref() {
            None => view::shell::TrayItemKind::Normal,
            Some(k) => view::shell::TrayItemKind::parse(k)
                .ok_or_else(|| Error::from_reason(format!("unknown menu item type {k:?}")))?,
        };
        Ok(view::shell::TrayItem {
            id: it.id, label: it.label.unwrap_or_default(), kind, checked: it.checked.unwrap_or(false),
            enabled: it.enabled.unwrap_or(true), submenu: tray_items(it.submenu.unwrap_or_default())?,
        })
    }).collect()
}

#[napi(object)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[napi(object)]
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    /// RGBA, row-major.
    pub data: Buffer,
}

// ==========================================================================
// The UI thread

type Job = Box<dyn FnOnce(&mut App) + Send>;

/// How jobs reach the UI thread: the native loop's sender, or (headless) a
/// channel the UI thread blocks on.
#[derive(Clone)]
enum Link {
    Native(UiSender),
    Headless(mpsc::Sender<Job>),
}

impl Link {
    fn send(&self, f: impl FnOnce(&mut App) + Send + 'static) -> bool {
        match self {
            Link::Native(s) => s.send(f),
            Link::Headless(tx) => tx.send(Box::new(f)).is_ok(),
        }
    }
}

/// The headless loop ends after the job that set this.
struct Quit;

/// Events to JavaScript, from the UI thread.
fn sink(tsfn: &Tsfn) -> view::Sink {
    let t = tsfn.clone();
    Arc::new(move |e: view::Event| {
        t.call(BuiEvent::from(e), ThreadsafeFunctionCallMode::NonBlocking);
    })
}

// ==========================================================================
// The JavaScript class

/// A BUI application: owns the UI thread.
#[napi]
pub struct NativeApp {
    link: Link,
    headless: bool,
}

impl NativeApp {
    /// Run `f` on the UI thread and wait for its answer (None: the thread is gone).
    fn ask<T: Send + 'static>(&self, f: impl FnOnce(&mut App) -> T + Send + 'static) -> Option<T> {
        let (tx, rx) = mpsc::sync_channel(1);
        if !self.link.send(move |cx| {
            let _ = tx.send(f(cx));
        }) {
            return None;
        }
        rx.recv().ok()
    }
}

#[napi]
impl NativeApp {
    #[napi(constructor)]
    pub fn new(options: Option<AppOptions>,
               #[napi(ts_arg_type = "(event: BuiEvent) => void")] on_event: JsFunction) -> Result<Self> {
        let tsfn: Tsfn = on_event.create_threadsafe_function(0, |ctx: ThreadSafeCallContext<BuiEvent>| {
            Ok(vec![ctx.value])
        })?;
        let (gpu, slug, headless) = match &options {
            Some(o) => (o.gpu, o.slug, o.headless.unwrap_or(false)),
            None => (None, None, false),
        };
        let theme = options.as_ref().and_then(|o| o.theme.clone());
        // macOS: AppKit only runs on the main thread, which here is the
        // JavaScript thread (libuv's loop), while BUI runs on its own
        // "bui-ui" thread; the Cocoa host refuses windows there, so stay
        // headless instead of starting a native loop that would end at once.
        let asked_headless = headless;
        let headless = headless || !blenderui::native_windows() || cfg!(target_os = "macos");
        // Headless without being asked is silent otherwise: nothing on screen, nothing in the log.
        if headless && !asked_headless {
            if cfg!(target_os = "macos") {
                eprintln!("blenderui: Node windows are not available on macOS yet; running headless (no window is shown).");
            } else {
                eprintln!("blenderui: no window system is available (X11 needs DISPLAY and libX11, Wayland needs \
                           WAYLAND_DISPLAY and XDG_RUNTIME_DIR); running headless, so no window is shown. Pass \
                           `headless: true` to silence this.");
            }
        }
        let (ltx, lrx) = mpsc::channel::<Link>();
        std::thread::Builder::new().name("bui-ui".into()).spawn(move || {
            let mut a = if headless { Application::headless() } else { Application::new().keep_alive(true) };
            if let Some(g) = gpu {
                a = a.gpu(g);
            }
            if let Some(s) = slug {
                a = a.slug(s);
            }
            if let Some(t) = &theme {
                a = a.theme(t);
            }
            let events = tsfn.clone();
            a.run(move |cx| {
                cx.set_global(events.clone());
                view::init(cx, sink(&events));
                if headless {
                    let (tx, rx) = mpsc::channel::<Job>();
                    let _ = ltx.send(Link::Headless(tx));
                    // jobs until every sender is gone
                    while let Ok(job) = rx.recv() {
                        job(cx);
                        cx.run_pending();
                        if cx.try_global::<Quit>().is_some() {
                            break;
                        }
                    }
                } else {
                    let _ = ltx.send(Link::Native(cx.ui_sender()));
                }
            });
            // the loop ended: let the runtime exit
            drop(tsfn);
        }).map_err(|e| Error::from_reason(format!("starting the UI thread: {e}")))?;
        let link = lrx.recv().map_err(|_| Error::from_reason("the UI thread failed to start"))?;
        Ok(NativeApp { link, headless })
    }

    /// Whether windows are off-screen.
    #[napi(getter)]
    pub fn headless(&self) -> bool {
        self.headless
    }

    /// Open window `id` (chosen by JS) with an empty `<body>` as node 0.
    #[napi]
    pub fn open_window(&self, id: u32, config: Option<WindowConfig>) {
        let c = config.map(|c| view::WindowConfig {
            title: c.title, width: c.width, height: c.height, resizable: c.resizable, gpu: c.gpu,
            transparent: c.transparent, always_on_top: c.always_on_top, frame: c.frame,
            layer_shell: c.layer_shell.map(|l| view::LayerShellConfig {
                layer: l.layer, anchor: l.anchor, exclusive_zone: l.exclusive_zone, keyboard: l.keyboard,
            }),
        }).unwrap_or_default();
        self.link.send(move |cx| {
            let s = sink(cx.global::<Tsfn>());
            view::open_window(cx, id, c, s);
        });
    }

    /// Apply a batch of ops to window `window` and re-render it: right away,
    /// or (`animated`, during requestAnimationFrame) on the next vsync.
    /// `motion`: the Motion props its `MOTION` ops set (`[which, value, ...]`).
    #[napi]
    pub fn commit(&self, window: u32, ops: Int32Array, strings: Vec<String>, animated: Option<bool>,
                  motion: Option<JsObject>) -> Result<()> {
        let ops: Vec<i32> = ops.to_vec();
        let animated = animated.unwrap_or(false);
        let (props, bad) = motion::props(motion)?;
        self.link.send(move |cx| view::commit(cx, window, &ops, &strings, props, animated));
        bad.map_or(Ok(()), Err) // the batch is applied; the prop that couldn't be read is reported
    }

    /// Ask for a `frame` event when window `window` next renders (vsync).
    #[napi]
    pub fn request_frame(&self, window: u32) {
        self.link.send(move |cx| view::request_frame(cx, window));
    }

    /// Picture window `window` for a view transition to what the next commit renders.
    #[napi]
    pub fn start_view_transition(&self, window: u32) {
        self.link.send(move |cx| view::start_view_transition(cx, window));
    }

    #[napi]
    pub fn close_window(&self, window: u32) {
        self.link.send(move |cx| view::close_window(cx, window));
    }

    /// GPU text: Slug (glyph outlines) or the glyph atlas.
    #[napi]
    pub fn set_slug(&self, on: bool) {
        self.link.send(move |cx| {
            blenderui::set_slug(on);
            cx.notify_all();
        });
    }

    /// Blender's "dark" or "light" theme for every window (they redraw).
    #[napi]
    pub fn set_theme(&self, name: String) -> bool {
        let ok = matches!(name.to_ascii_lowercase().as_str(), "dark" | "light" | "blender dark" | "blender light");
        if ok {
            self.link.send(move |cx| {
                blenderui::set_theme(&name);
                cx.notify_all();
            });
        }
        ok
    }

    /// Switch to a theme loaded with `loadTheme`: resolves with whether the name is known.
    #[napi(ts_return_type = "Promise<boolean>")]
    pub fn set_custom_theme(&self, env: Env, name: String) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |cx| {
            let ok = blenderui::set_theme(&name);
            cx.notify_all();
            deferred.resolve(move |_| Ok(ok));
        });
        Ok(promise)
    }

    /// Register the themes of a theme file's text (`file` false) or of the file at that path
    /// (`file` true): resolves with how many were added, rejects with where the mistake is.
    #[napi(ts_return_type = "Promise<number>")]
    pub fn load_theme(&self, env: Env, source: String, file: bool) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |_cx| {
            let r = if file { blenderui::load_theme_file(&source) } else { blenderui::load_theme(&source) };
            match r {
                Ok(n) => deferred.resolve(move |_| Ok(n as u32)),
                Err(e) => deferred.reject(Error::from_reason(e)),
            }
        });
        Ok(promise)
    }

    /// The names of the loaded themes.
    #[napi(ts_return_type = "Promise<string[]>")]
    pub fn themes(&self, env: Env) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |_cx| {
            let names = blenderui::themes();
            deferred.resolve(move |_| Ok(names));
        });
        Ok(promise)
    }

    /// One color of the active theme (`rgba` 0..1): resolves with whether the key exists.
    #[napi(ts_return_type = "Promise<boolean>")]
    pub fn set_theme_color(&self, env: Env, key: String, r: f64, g: f64, b: f64, a: f64) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |cx| {
            let ok = blenderui::set_theme_color(&key, [r as f32, g as f32, b as f32, a as f32]);
            cx.notify_all();
            deferred.resolve(move |_| Ok(ok));
        });
        Ok(promise)
    }

    /// One number of the active theme (roundness, border width, ...): resolves with whether the key exists.
    #[napi(ts_return_type = "Promise<boolean>")]
    pub fn set_theme_value(&self, env: Env, key: String, value: f64) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |cx| {
            let ok = blenderui::set_theme_value(&key, value);
            cx.notify_all();
            deferred.resolve(move |_| Ok(ok));
        });
        Ok(promise)
    }

    /// Tray icon `id` (chosen by JS): resolves false where the system has no tray.
    #[napi(ts_return_type = "Promise<boolean>")]
    pub fn tray_new(&self, env: Env, id: u32, icon: Option<String>, tooltip: String) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |cx| {
            let ok = view::shell::tray_new(cx, id, icon.as_deref(), &tooltip);
            deferred.resolve(move |_| Ok(ok));
        });
        Ok(promise)
    }

    /// An image file or a Blender icon's name; null: the app's icon.
    #[napi]
    pub fn tray_set_icon(&self, id: u32, icon: Option<String>) {
        self.link.send(move |cx| {
            view::shell::tray_set_icon(cx, id, icon.as_deref());
        });
    }

    #[napi]
    pub fn tray_set_tooltip(&self, id: u32, tooltip: String) {
        self.link.send(move |cx| {
            view::shell::tray_set_tooltip(cx, id, &tooltip);
        });
    }

    /// Its menu; chosen items come back as `traymenu` events with their `id`.
    #[napi]
    pub fn tray_set_menu(&self, id: u32, items: Vec<TrayMenuItem>) -> Result<()> {
        let items = tray_items(items)?;
        self.link.send(move |cx| {
            view::shell::tray_set_menu(cx, id, &items);
        });
        Ok(())
    }

    #[napi]
    pub fn tray_free(&self, id: u32) {
        self.link.send(move |cx| view::shell::tray_free(cx, id));
    }

    /// A desktop notification: resolves with its id (its clicks are `notification` events), or
    /// null when it can't show.
    #[napi(ts_return_type = "Promise<number | null>")]
    pub fn notify(&self, env: Env, title: String, body: String, icon: Option<String>) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |cx| {
            let id = view::shell::notify(cx, &title, &body, icon.as_deref());
            deferred.resolve(move |_| Ok(id));
        });
        Ok(promise)
    }

    /// Window `window`'s taskbar progress (`mode`: normal, indeterminate, paused, error, none).
    #[napi]
    pub fn set_progress(&self, window: u32, mode: String, value: f64) {
        self.link.send(move |cx| {
            view::shell::set_progress(cx, window, &mode, value);
        });
    }

    /// Adds a file to the system's recent documents: resolves with whether it was added.
    #[napi(ts_return_type = "Promise<boolean>")]
    pub fn add_recent_document(&self, env: Env, path: String) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |_| {
            let ok = blenderui::shell::add_recent_document(&path);
            deferred.resolve(move |_| Ok(ok));
        });
        Ok(promise)
    }

    /// Windows' AppUserModelID, the desktop file's id on Linux.
    #[napi]
    pub fn set_app_id(&self, id: String) {
        self.link.send(move |_| blenderui::shell::set_app_id(&id));
    }

    /// What this build and system have of the desktop shell.
    #[napi(ts_return_type = "Promise<ShellFeatures>")]
    pub fn shell_features(&self, env: Env) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |_| {
            let [tray, notifications, progress, recent_documents] = view::shell::features();
            deferred.resolve(move |_| Ok(ShellFeatures { tray, notifications, progress, recent_documents }));
        });
        Ok(promise)
    }

    /// End the UI thread (closes every window).
    #[napi]
    pub fn quit(&self) {
        self.link.send(|cx| {
            view::close_all(cx);
            cx.set_global(Quit);
            cx.quit();
        });
    }

    /// Render window `window` and resolve with its pixels: `{ width, height,
    /// data }` (RGBA bytes, device pixels).
    #[napi(ts_return_type = "Promise<{ width: number, height: number, data: Buffer }>")]
    pub fn screenshot(&self, env: Env, window: u32) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |cx| match view::screenshot(cx, window) {
            Some((width, height, rgba)) => {
                deferred.resolve(move |_| Ok(Screenshot { width, height, data: rgba.into() }));
            }
            None => deferred.reject(Error::from_reason("no such window")),
        });
        Ok(promise)
    }

    /// Bounds of node `node` (logical pixels), or null.
    #[napi(ts_return_type = "Promise<Rect | null>")]
    pub fn bounds(&self, env: Env, window: u32, node: u32) -> Result<JsObject> {
        let (deferred, promise) = env.create_deferred()?;
        self.link.send(move |cx| {
            let r = view::bounds(cx, window, node);
            deferred.resolve(move |_| Ok(r.map(|b| Rect { x: b.x, y: b.y, width: b.width, height: b.height })));
        });
        Ok(promise)
    }

    /// Click at (x, y) in window `window` (tests; logical pixels).
    #[napi]
    pub fn simulate_click(&self, window: u32, x: f64, y: f64) {
        self.link.send(move |cx| view::simulate_click(cx, window, x, y));
    }

    /// The pointer in window `window` (tests; logical pixels): `kind` 0 moves it, 1 presses the left button, 2 lets go.
    #[napi]
    pub fn simulate_pointer(&self, window: u32, kind: u32, x: f64, y: f64) {
        self.link.send(move |cx| view::simulate_pointer(cx, window, kind, x, y));
    }

    /// A touch screen contact in window `window` (tests; logical pixels): `phase` 0 down, 1 move, 2 up, 3 cancel.
    #[napi]
    pub fn simulate_touch(&self, window: u32, id: i32, phase: u32, x: f64, y: f64) {
        self.link.send(move |cx| view::simulate_touch(cx, window, id, phase, x, y));
    }

    /// A trackpad gesture in window `window` (tests; logical pixels): `kind` 0 pinches by `amount`
    /// (the scale change), 1 turns by `amount` degrees.
    #[napi]
    pub fn simulate_gesture(&self, window: u32, kind: u32, x: f64, y: f64, amount: f64) {
        self.link.send(move |cx| view::simulate_gesture(cx, window, kind, x, y, amount));
    }
}

// ==========================================================================
// Measuring (benchmarks): the same counters as bench/common/measure.rs.

/// CPU cycles used by every thread of this process (Windows; otherwise null).
/// Unlike tick-based CPU time, exact for threads that wake on each vsync.
#[napi]
pub fn process_cpu_cycles() -> Option<f64> {
    view::process_cpu_cycles()
}

/// The time-stamp counter (x86-64; otherwise null): divide cycle counts by
/// its rate, measured against the wall clock, to get CPU time.
#[napi]
pub fn timestamp_counter() -> Option<f64> {
    view::timestamp_counter()
}

//! Shared by every demo: the one place that opens a window, declares a GPU shader with its
//! software fallback, and keeps the clock. Demos hold their own state in their `Render` struct.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use blenderui::GpuTarget;
use blenderui::prelude::*;

pub mod canvas;
pub mod color;
pub mod control;
pub mod ease;
pub mod menu;
pub mod pen;
pub mod story;
pub mod text;

/// Set for a scripted (headless) run: the clock is then BUI's virtual one, so a storyboard's frames
/// land at the same moments however long the software renderer takes over each.
static VIRTUAL: AtomicBool = AtomicBool::new(false);
/// The virtual clock, in microseconds ([`advance`] moves it).
static VIRTUAL_US: AtomicU64 = AtomicU64::new(0);

/// Seconds since the first call (the shaders' clock), or since a scripted run began.
#[must_use]
pub fn now() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    if VIRTUAL.load(Relaxed) {
        return VIRTUAL_US.load(Relaxed) as f32 / 1e6;
    }
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// Moves BUI's virtual clock (CSS and Motion animations) and [`now`] on by `d` together.
pub fn advance(cx: &mut App, d: Duration) {
    cx.advance_clock(d);
    VIRTUAL_US.fetch_add(d.as_micros() as u64, Relaxed);
}

/// A listener that updates view `V` through `view` (a `Copy` handle: see `WeakEntity::listener`) and
/// renders it again: for elements built where there is no `cx` (a screen's editor factories).
pub fn act<V: 'static, E: ?Sized + 'static>(view: WeakEntity<V>, f: impl Fn(&mut V, &E) + 'static)
                                           -> impl Fn(&E, &mut Window, &mut App) + 'static {
    view.listener(move |this, e, _, cx| {
        f(this, e);
        cx.notify();
    })
}

/// Element id `id` in the `k`th area showing an editor (`0`: the first keeps `id` itself): ids are
/// one per page, and an editor shown in two areas builds its elements twice.
#[must_use]
pub fn area_id(id: &str, k: u32) -> String {
    if k == 0 { id.to_string() } else { format!("{id}-{k}") }
}

/// Keys come to the page without a click: focuses `#root` once a first frame exists, unless one of
/// Blender's text fields is being typed in. Call at the top of `render`; `frames` counts the frames
/// rendered.
pub fn focus_root(w: &mut Window, frames: &mut u32) {
    if w.focused_id().is_none() && *frames > 0 && !w.text_field_focused() {
        w.focus_id("root");
    }
    *frames += 1;
}

/// A GPU pixel shader and the flat image drawn where there is no GPU (software renderer).
pub struct Fx {
    effect: Rc<ShaderEffect>,
    fallback: Arc<Image>,
}

impl Fx {
    /// `glsl` defines `vec4 effect(vec2 p)`; `floats` is how many `U[i]` it reads.
    ///
    /// # Panics
    /// When the declaration itself is invalid (the compile log arrives once drawn, see [`Fx::error`]).
    #[must_use]
    pub fn new(name: &str, glsl: &str, floats: usize, fallback_rgb: [u8; 3]) -> Self {
        Self::sampling(name, glsl, 0, floats, fallback_rgb)
    }

    /// [`Fx::new`] for a shader that also samples `inputs` textures (`input_color` / `input_raw`).
    ///
    /// # Panics
    /// As [`Fx::new`].
    #[must_use]
    pub fn sampling(name: &str, glsl: &str, inputs: usize, floats: usize, fallback_rgb: [u8; 3]) -> Self {
        let effect = ShaderEffect::new(name, glsl, inputs, floats).unwrap_or_else(|e| panic!("{e}"));
        let px: Vec<u8> = (0..16 * 16).flat_map(|_| [fallback_rgb[0], fallback_rgb[1], fallback_rgb[2], 255]).collect();
        Self { effect: Rc::new(effect), fallback: Arc::new(Image::from_rgba(16, 16, &px).expect("fallback image")) }
    }

    /// The shader itself (off-screen drawing with `GpuTarget`, or a pass inside another canvas).
    #[must_use]
    pub const fn effect(&self) -> &Rc<ShaderEffect> {
        &self.effect
    }

    /// The GPU compile log of the last attempt ("" when fine).
    #[must_use]
    pub fn error(&self) -> String {
        self.effect.error()
    }

    /// A canvas running the shader; `floats` is evaluated per frame with the box's own geometry
    /// (`origin`, `size` in logical pixels) so a demo can turn window coordinates into box UV.
    pub fn canvas<F: AsRef<[f32]>>(&self, floats: impl Fn(Bounds<f32>) -> F + 'static) -> Div {
        self.canvas_after(Vec::new(), |_, _, _| {}, floats)
    }

    /// [`Fx::canvas`] sampling `inputs`, with `pass` recorded first each frame (given the same
    /// floats): an `effect_to` into a render target this shader then reads.
    pub fn canvas_after<F: AsRef<[f32]>>(&self, inputs: Vec<i32>, pass: impl Fn(Bounds<f32>, &mut Painter, &[f32]) + 'static,
                                         floats: impl Fn(Bounds<f32>) -> F + 'static) -> Div {
        let (e, fb) = (self.effect.clone(), self.fallback.clone());
        canvas(move |b, p| {
            let f = floats(b);
            pass(b, p, f.as_ref());
            p.effect(b, &e, &inputs, f.as_ref(), Some(&fb));
        })
    }
}

/// A point in window coordinates as 0..1 UV inside `b`, y up (GLSL convention).
#[must_use]
pub fn uv_in(b: Bounds<f32>, x: f32, y: f32) -> [f32; 2] {
    [(x - b.origin.x) / b.size.width.max(1.0), 1.0 - (y - b.origin.y) / b.size.height.max(1.0)]
}

/// What drives a headless run: it gets the app and the window, and can click, step the virtual
/// clock (`advance_clock`) and draw.
pub type Script = Box<dyn FnOnce(&mut App, WindowId)>;

/// The value after `flag` on the command line (`--screenshot out.png`).
#[must_use]
pub fn arg(flag: &str) -> Option<String> {
    std::env::args().skip_while(|a| a != flag).nth(1)
}

/// Opens one GPU window and runs `make`'s view in it. With `--screenshot out.png` it renders one
/// frame off-screen instead (software renderer: shaders show their fallback) and saves it.
pub fn run<V: Render + 'static>(w: f32, h: f32, title: &str, make: impl FnOnce() -> V + 'static) {
    run_with(w, h, title, make, None);
}

/// [`run`], or with a `script` the same view in a headless window (software renderer, virtual
/// clock: see [`now`]) that the script drives instead. `--screenshot out.png` is a script too.
///
/// # Panics
/// When the window cannot be opened.
pub fn run_with<V: Render + 'static>(w: f32, h: f32, title: &str, make: impl FnOnce() -> V + 'static,
                                     script: Option<Script>) {
    let script = script.or_else(|| arg("--screenshot").map(|path| -> Script {
        Box::new(move |cx, win| {
            let (pw, ph, pixels) = cx.draw(win);
            save_png(&path, pw, ph, &pixels);
        })
    }));
    if script.is_some() {
        VIRTUAL.store(true, Relaxed);
        // a software frame can take seconds: let the motion clock take each step whole
        blenderui::motion::lag_smoothing(0.0, 0.0);
    }
    let title = title.to_string();
    let app = if script.is_some() { Application::headless() } else { Application::new() };
    app.run(move |cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(w), px(h)),
            })),
            titlebar: Some(TitlebarOptions { title: Some(title.into()) }),
            gpu: Some(script.is_none()),
            ..Default::default()
        };
        let win = cx.open_window(opts, |_, cx| cx.new(|_| make())).unwrap();
        if let Some(script) = script {
            script(cx, win.id);
        }
    });
}

/// The window size from `--size WxH`, else `default`.
#[must_use]
pub fn size_arg(default: (f32, f32)) -> (f32, f32) {
    arg("--size").and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
        .unwrap_or(default)
}

/// Draws frames off-screen on the GPU (`draw` records one from its floats) and saves each as
/// `dir/name.png`; hands back the target for more (timing). `None` without a GPU or on a shader
/// error (reported).
pub fn gpu_stills<F: AsRef<[f32]>>(dir: &str, w: usize, h: usize, stills: impl IntoIterator<Item = (&'static str, F)>,
                                   draw: impl Fn(&mut GpuTarget, &[f32]) -> bool) -> Option<GpuTarget> {
    let Some(mut gpu) = GpuTarget::new(w, h) else {
        eprintln!("no GPU device");
        return None;
    };
    for (name, floats) in stills {
        if !draw(&mut gpu, floats.as_ref()) {
            eprintln!("{name}: the shader did not run");
            return None;
        }
        save_png(&format!("{dir}/{name}.png"), w, h, gpu.pixels());
    }
    Some(gpu)
}

/// Writes `0x00RRGGBB` pixels (a window's or a `GpuTarget`'s) as an RGB PNG.
///
/// # Panics
/// When the file cannot be written.
pub fn save_png(path: &str, w: usize, h: usize, pixels: &[u32]) {
    let rgb: Vec<u8> = pixels.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().expect("png").write_image_data(&rgb).expect("png");
}

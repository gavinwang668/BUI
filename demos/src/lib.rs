//! Shared by every demo: the one place that opens a window, declares a GPU shader with its
//! software fallback, and keeps the clock. Demos hold their own state in their `Render` struct.

use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use blenderui::GpuTarget;
use blenderui::prelude::*;

pub mod canvas;
pub mod menu;

/// Seconds since the first call (the shaders' clock).
#[must_use]
pub fn now() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
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
    let shot = arg("--screenshot").map(|path| -> Script {
        Box::new(move |cx, win| {
            let (pw, ph, pixels) = cx.draw(win);
            save_png(&path, pw, ph, &pixels);
        })
    });
    run_with(w, h, title, make, shot);
}

/// [`run`], or with a `script` the same view in a headless window (software renderer, virtual
/// clock) that the script drives instead.
///
/// # Panics
/// When the window cannot be opened.
pub fn run_with<V: Render + 'static>(w: f32, h: f32, title: &str, make: impl FnOnce() -> V + 'static,
                                     script: Option<Script>) {
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

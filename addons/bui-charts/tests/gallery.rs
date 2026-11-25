//! Renders every example of the gallery (every chart kind, `examples/gallery`) headless: each one
//! builds, lays out and paints something besides its background, and a pointer move over it does not
//! panic.
#![allow(clippy::wildcard_imports, clippy::needless_pass_by_value, dead_code)]

#[macro_use]
#[path = "../examples/gallery/registry.rs"]
mod registry;

#[path = "../examples/gallery/bar_gauge.rs"]
mod bar_gauge;
#[path = "../examples/gallery/candlestick.rs"]
mod candlestick;
#[path = "../examples/gallery/category.rs"]
mod category;
#[path = "../examples/gallery/dashboard.rs"]
mod dashboard;
#[path = "../examples/gallery/dashboards.rs"]
mod dashboards;
#[path = "../examples/gallery/diagram.rs"]
mod diagram;
#[path = "../examples/gallery/distribution.rs"]
mod distribution;
#[path = "../examples/gallery/earth.rs"]
mod earth;
#[path = "../examples/gallery/finance.rs"]
mod finance;
#[path = "../examples/gallery/flame_graph.rs"]
mod flame_graph;
#[path = "../examples/gallery/flow.rs"]
mod flow;
#[path = "../examples/gallery/fractal.rs"]
mod fractal;
#[path = "../examples/gallery/gauge.rs"]
mod gauge;
#[path = "../examples/gallery/heatmap.rs"]
mod heatmap;
#[path = "../examples/gallery/histogram.rs"]
mod histogram;
#[path = "../examples/gallery/hybrid.rs"]
mod hybrid;
#[path = "../examples/gallery/matrix.rs"]
mod matrix;
#[path = "../examples/gallery/node_graph.rs"]
mod node_graph;
#[path = "../examples/gallery/quality.rs"]
mod quality;
#[path = "../examples/gallery/science.rs"]
mod science;
#[path = "../examples/gallery/stat.rs"]
mod stat;
#[path = "../examples/gallery/state_timeline.rs"]
mod state_timeline;
#[path = "../examples/gallery/status_history.rs"]
mod status_history;
#[path = "../examples/gallery/time_series.rs"]
mod time_series;
#[path = "../examples/gallery/traces.rs"]
mod traces;
#[path = "../examples/gallery/trend.rs"]
mod trend;
#[path = "../examples/gallery/xy.rs"]
mod xy;

use blenderui::prelude::*;
pub use registry::page;
use registry::{CHARTS, Example};

/// One example on its page, or on `backdrop` (the screenshots draw each one on black and on white).
struct One(&'static Example, Option<Rgba>);

impl Render for One {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (example, backdrop) = (self.0, self.1.unwrap_or_else(|| page().0));
        bui_panels::ui::scope(window, cx, || {
            div().class("absolute inset-0 flex flex-col p-2").bg(backdrop).text_color(page().1).child((example.render)())
        })
    }
}

/// Pixels that are not the page background.
fn drawn(pixels: &[u32], background: u32) -> usize {
    pixels.iter().filter(|p| **p != background).count()
}

#[test]
fn every_example_renders() {
    Application::headless().run(|cx| {
        blenderui::set_theme("dark");
        let background = {
            let [r, g, b] = [page().0.r, page().0.g, page().0.b].map(|v| (v * 255.).round() as u32);
            (r << 16) | (g << 8) | b
        };
        let mut blank = Vec::new();
        let mut count = 0;
        for chart in CHARTS {
            for example in chart.examples {
                let options = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: size(px(900.), px(520.)),
                    })),
                    ..Default::default()
                };
                let win = cx.open_window(options, move |_, cx| cx.new(|_| One(example, None))).unwrap();
                cx.draw(win.id);
                cx.simulate_mouse_move(win.id, 450., 260.);
                let (_, _, pixels) = cx.draw(win.id);
                // the title and description are not drawn here: anything is the chart's
                if drawn(&pixels, background) < 400 {
                    blank.push(format!("{} / {} ({} pixels)", chart.name, example.title, drawn(&pixels, background)));
                    if let Ok(dir) = std::env::var("CHART_DUMP") {
                        let mut out = b"P6
900 520
255
".to_vec();
                        for p in &pixels {
                            out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
                        }
                        std::fs::write(format!("{dir}/blank-{}.ppm", blank.len()), out).unwrap();
                    }
                }
                cx.close_window(win.id);
                count += 1;
            }
        }
        assert!(count > 300, "{count} examples");
        assert!(blank.is_empty(), "nothing drawn by: {blank:?}");
    });
}

/// `name` as a file name: lowercase words joined by dashes.
fn slug(name: &str) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    words.join("-")
}

/// Straight RGBA from the same frame drawn on black and on white: where the backdrop shows through,
/// the two differ by the backdrop's share (1 - alpha), and the colour on black is colour x alpha.
fn unmatte(on_black: &[u32], on_white: &[u32]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(on_black.len() * 4);
    for (&b, &w) in on_black.iter().zip(on_white) {
        let channel = |p: u32, s: u32| f32::from(((p >> s) & 255) as u8);
        let shown: f32 = [16, 8, 0].iter().map(|&s| channel(w, s) - channel(b, s)).sum::<f32>() / (3. * 255.);
        let alpha = (1. - shown).clamp(0., 1.);
        for s in [16, 8, 0] {
            let c = if alpha > 0. { (channel(b, s) / alpha).min(255.) } else { 0. };
            rgba.push(c.round() as u8);
        }
        rgba.push((alpha * 255.).round() as u8);
    }
    rgba
}

/// `rgba` (w x h) with what is drawn moved to the middle: every screenshot frames its chart alike.
fn centred(rgba: &[u8], w: usize, h: usize) -> Vec<u8> {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if rgba[(y * w + x) * 4 + 3] != 0 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    if x0 >= x1 {
        return rgba.to_vec(); // nothing drawn
    }
    let (dx, dy) = ((w - (x1 - x0)) / 2, (h - (y1 - y0)) / 2);
    let mut out = vec![0; rgba.len()];
    for y in y0..y1 {
        let (from, to) = ((y * w + x0) * 4, ((y - y0 + dy) * w + dx) * 4);
        out[to..to + (x1 - x0) * 4].copy_from_slice(&rgba[from..from + (x1 - x0) * 4]);
    }
    out
}

/// Every example as a transparent PNG, all the same size (16:9) with the chart in the middle, into
/// `CHART_SHOTS` (a folder):
/// `CHART_SHOTS=out BUI_UI_SCALE=2 cargo test --test gallery screenshots -- --ignored` for 2560 x 1440.
#[test]
#[ignore = "writes files: run it by hand with CHART_SHOTS"]
fn screenshots() {
    let dir = std::env::var("CHART_SHOTS").expect("CHART_SHOTS: the folder to write to");
    // CHART_SHOTS_FROM / CHART_SHOTS_TO: the examples to take (1-based, inclusive), to do part of the set
    let bound = |name: &str, default: usize| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default);
    let (from, to) = (bound("CHART_SHOTS_FROM", 1), bound("CHART_SHOTS_TO", usize::MAX));
    std::fs::create_dir_all(&dir).unwrap();
    Application::headless().run(move |cx| {
        blenderui::set_theme("dark");
        let mut n = 0;
        for chart in CHARTS {
            for example in chart.examples {
                n += 1;
                if n < from || n > to {
                    continue;
                }
                println!("{n:03} {} / {}", chart.name, example.title);
                let mut frame = |backdrop: u32| {
                    let options = WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: point(px(0.), px(0.)),
                            size: size(px(1280.), px(720.)),
                        })),
                        ..Default::default()
                    };
                    let win = cx.open_window(options, move |_, cx| cx.new(|_| One(example, Some(rgb(backdrop))))).unwrap();
                    cx.draw(win.id);
                    let shot = cx.draw(win.id);
                    cx.close_window(win.id);
                    shot
                };
                let (w, h, black) = frame(0x000000);
                let (_, _, white) = frame(0xffffff);
                let path = format!("{dir}/{n:03}-{}-{}.png", slug(chart.name), slug(example.title));
                let mut png = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&path).unwrap()), w as u32, h as u32);
                png.set_color(png::ColorType::Rgba);
                png.set_depth(png::BitDepth::Eight);
                png.write_header().unwrap().write_image_data(&centred(&unmatte(&black, &white), w, h)).unwrap();
            }
        }
        println!("{n} screenshots in {dir}");
    });
}

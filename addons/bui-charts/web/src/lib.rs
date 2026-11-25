//! The gallery's examples in a web page, one at a time: the live render of the website's chart pages
//! (`/charts`). The examples are the gallery's own modules
//! (`examples/gallery`, by `#[path]`, as `tests/gallery.rs` takes them), and `registry::CHARTS` names them.
//!
//! Besides `blenderui_web::export!`'s functions, the page calls:
//!
//! * `bui_charts_catalog()` / `bui_charts_catalog_len()`: every chart and its examples as JSON,
//!   `[{"name": .., "examples": [{"title": .., "description": ..}]}]`, valid for the module's life;
//! * `bui_charts_show(name, len, example)`: show a chart's example (the name is UTF-8 written with
//!   `bui_alloc`, freed here; an unknown name does nothing, the index is clamped);
//! * `bui_charts_set_dark(dark)`: Blender Dark or Blender Light;
//! * `bui_charts_reseed(seed)`: new random data for the examples that draw it, then mount the example again.
#![allow(clippy::wildcard_imports, clippy::needless_pass_by_value, dead_code)]

#[macro_use]
#[path = "../../examples/gallery/registry.rs"]
mod registry;

#[path = "../../examples/gallery/bar_gauge.rs"]
mod bar_gauge;
#[path = "../../examples/gallery/candlestick.rs"]
mod candlestick;
#[path = "../../examples/gallery/category.rs"]
mod category;
#[path = "../../examples/gallery/dashboard.rs"]
mod dashboard;
#[path = "../../examples/gallery/dashboards.rs"]
mod dashboards;
#[path = "../../examples/gallery/diagram.rs"]
mod diagram;
#[path = "../../examples/gallery/distribution.rs"]
mod distribution;
#[path = "../../examples/gallery/earth.rs"]
mod earth;
#[path = "../../examples/gallery/finance.rs"]
mod finance;
#[path = "../../examples/gallery/flame_graph.rs"]
mod flame_graph;
#[path = "../../examples/gallery/flow.rs"]
mod flow;
#[path = "../../examples/gallery/fractal.rs"]
mod fractal;
#[path = "../../examples/gallery/gauge.rs"]
mod gauge;
#[path = "../../examples/gallery/heatmap.rs"]
mod heatmap;
#[path = "../../examples/gallery/histogram.rs"]
mod histogram;
#[path = "../../examples/gallery/hybrid.rs"]
mod hybrid;
#[path = "../../examples/gallery/matrix.rs"]
mod matrix;
#[path = "../../examples/gallery/node_graph.rs"]
mod node_graph;
#[path = "../../examples/gallery/quality.rs"]
mod quality;
#[path = "../../examples/gallery/science.rs"]
mod science;
#[path = "../../examples/gallery/stat.rs"]
mod stat;
#[path = "../../examples/gallery/state_timeline.rs"]
mod state_timeline;
#[path = "../../examples/gallery/status_history.rs"]
mod status_history;
#[path = "../../examples/gallery/time_series.rs"]
mod time_series;
#[path = "../../examples/gallery/traces.rs"]
mod traces;
#[path = "../../examples/gallery/trend.rs"]
mod trend;
#[path = "../../examples/gallery/xy.rs"]
mod xy;

use std::cell::Cell;
use std::sync::LazyLock;

use blenderui::hooks::{Signal, keyed};
use blenderui::prelude::*;
use registry::{CHARTS, Layout, page};

/// What the stage shows. `nonce` changes on a reseed, so the example mounts again with fresh data.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
struct Pick {
    chart: usize,
    example: usize,
    nonce: u32,
    dark: bool,
}

thread_local! {
    /// The pick, a global signal: writing it re-renders the stage. Set once the app starts.
    static PICK: Cell<Option<Signal<Pick>>> = const { Cell::new(None) };
}

fn update(change: impl FnOnce(&mut Pick)) {
    if let Some(pick) = PICK.get() {
        pick.update(change);
    }
}

/// An example's render, run where its element is made: inside [`keyed`], so its hooks follow the pick.
struct Mount(fn() -> AnyElement);

impl IntoElement for Mount {
    fn into_any_element(self) -> AnyElement {
        (self.0)()
    }
}

struct Stage(Signal<Pick>);

impl Render for Stage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pick = self.0.get();
        let chart = &CHARTS[pick.chart];
        let example = &chart.examples[pick.example];
        let (background, foreground, _) = page();
        let mounted = keyed((pick.chart, pick.example, pick.nonce), Mount(example.render));
        // the dashboards hold bui-panels workspaces, which take the window and app from here
        bui_panels::ui::scope(window, cx, || {
            let stage = div().id("stage").bg(background).text_color(foreground);
            match chart.layout {
                Layout::Page => stage.class("absolute inset-0").child(mounted),
                Layout::Grid(_) => stage.class("absolute inset-0 flex flex-col p-3 overflow-y-scroll").child(mounted),
            }
        })
    }
}

blenderui_web::export!(|cx: &mut App, options: WindowOptions| {
    let pick = Signal::new(Pick::default());
    PICK.set(Some(pick));
    cx.open_window(options, move |_, cx| cx.new(|_| Stage(pick))).unwrap();
});

static CATALOG: LazyLock<String> = LazyLock::new(|| {
    let charts = CHARTS.iter().map(|chart| {
        let examples = chart.examples.iter().map(|e| serde_json::json!({ "title": e.title, "description": e.description }));
        serde_json::json!({ "name": chart.name, "examples": examples.collect::<Vec<_>>() })
    });
    serde_json::Value::Array(charts.collect()).to_string()
});

/// The catalog's UTF-8 JSON (see the crate docs), `bui_charts_catalog_len()` bytes long.
#[unsafe(no_mangle)]
pub extern "C" fn bui_charts_catalog() -> *const u8 {
    CATALOG.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn bui_charts_catalog_len() -> usize {
    CATALOG.len()
}

/// Shows example `example` of the chart named by the `len` UTF-8 bytes at `name` (written with
/// `bui_alloc`, freed here).
///
/// # Safety
/// `name` and `len` must be what `bui_alloc` returned and was given, and not freed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bui_charts_show(name: *mut u8, len: usize, example: u32) {
    // SAFETY: by the contract above
    let found = CHARTS.iter().position(|chart| chart.name.as_bytes() == unsafe { std::slice::from_raw_parts(name, len) });
    // SAFETY: the page wrote this buffer with `bui_alloc` and passes it back once
    unsafe { blenderui_web::free_buffer(name, len) };
    let Some(chart) = found else { return };
    let example = (example as usize).min(CHARTS[chart].examples.len() - 1);
    update(|pick| {
        pick.chart = chart;
        pick.example = example;
    });
}

/// Blender Dark (nonzero) or Blender Light.
#[unsafe(no_mangle)]
pub extern "C" fn bui_charts_set_dark(dark: i32) {
    let dark = dark != 0;
    blenderui::set_theme(if dark { "dark" } else { "light" });
    // the stage's page colours come from the theme: render it again
    update(|pick| pick.dark = dark);
}

/// New random data for the examples that draw it (the finance charts), and the example mounted again.
#[unsafe(no_mangle)]
pub extern "C" fn bui_charts_reseed(seed: u32) {
    finance::reseed(u64::from(seed));
    update(|pick| pick.nonce = pick.nonce.wrapping_add(1));
}

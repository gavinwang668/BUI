//! Velvet Menu: a Persona-style main menu for a tech app, on BUI. A GPU shader is the water
//! (caustics, light shafts, a room far behind, rising motes); over it, a heavy italic menu tilted
//! across the screen. The item under the pointer (or the arrow keys) snaps forward, black on a red
//! slash; Enter / click / Confirm stamps it and slides in its sub-menu, which follows the arrows;
//! C / Escape / Close puts it away. Laid out on a 1280 x 720 canvas in `rem`, sized to the window
//! (see `bui_demos::canvas`), so it stays sharp at any size.
//!
//! Motion ("energetic"): selection snaps in 180 ms with an overshoot, the red slash 160 ms behind
//! it and the pink sliver 40 ms after that; the menu flies in 35 ms per item on first light, the
//! counter counts up; the water, the shards and the motes keep moving underneath.
//!
//! `cargo run --release --bin velvet`
//! `-- --storyboard DIR` drives it headlessly (keys, hover, clicks) and saves frames along BUI's
//! virtual clock (`-- --size 1600x600` for another window size); `-- --frames DIR` renders the
//! water off-screen on the GPU.

use std::sync::OnceLock;
use std::time::Duration;

use blenderui::prelude::*;
use bui_demos::canvas::{rem, rem_css, Fit};
use bui_demos::menu::{MenuState, ENTRIES};
use bui_demos::{arg, gpu_stills, now, run_with, save_png, Fx, Script};

/// U[0] = time, U[1] = seconds since the last confirm (< 0: none).
const WATER: &str = r"
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

vec4 effect(vec2 p) {
    float t = U[0]; float pulse = U[1];
    vec2 uv = p / box_size;
    vec2 q = vec2(uv.x * box_size.x / box_size.y, uv.y);
    // depth: shallow cyan under the surface, deep blue below
    vec3 c = mix(vec3(0.32, 0.95, 1.0), vec3(0.04, 0.55, 0.96), smoothstep(0.0, 0.5, uv.y));
    c = mix(c, vec3(0.0, 0.13, 0.72), smoothstep(0.45, 1.0, uv.y));
    // the room far behind the water: pale pillars and a rail, swaying with the refraction
    float sway = 0.012 * sin(uv.y * 14.0 + t * 0.9);
    float pillar = smoothstep(0.035, 0.0, abs(fract((q.x + sway) * 1.6 + 0.15) - 0.5) - 0.06);
    float rail = smoothstep(0.01, 0.0, abs(uv.y - 0.56 - 0.05 * sin(q.x * 1.7) + sway));
    c = mix(c, c * 1.18 + 0.05, (0.3 * pillar + 0.25 * rail) * smoothstep(0.15, 0.6, uv.y));
    // caustics, brightest near the surface
    vec2 z = q * 5.5 + vec2(t * 0.15, t * 0.1);
    float k = 0.0;
    for (int i = 0; i < 3; i++) {
        z = vec2(z.x + sin(z.y + t * 0.6), z.y + cos(z.x - t * 0.5));
        k += 1.0 / (1.0 + 30.0 * abs(sin(z.x) * sin(z.y)));
    }
    c += vec3(0.55, 0.95, 1.0) * pow(k / 3.0, 2.0) * (1.0 - smoothstep(0.0, 0.75, uv.y)) * 0.9;
    // light shafts from the surface
    float shaft = (0.5 + 0.5 * sin(q.x * 8.0 + q.y * 3.0 + 2.0 * sin(t * 0.25))) * (0.5 + 0.5 * sin(q.x * 21.0 - q.y * 4.0 + t * 0.35));
    c += vec3(0.6, 0.95, 1.0) * pow(shaft, 3.0) * 0.22 * (1.0 - uv.y);
    // motes rising
    vec2 g = q * vec2(16.0, 9.0) + vec2(0.0, t * 0.5);
    vec2 id = floor(g);
    vec2 f = fract(g) - 0.5 - (vec2(hash(id + 1.3), hash(id + 2.1)) - 0.5) * 0.6;
    c += step(0.86, hash(id)) * smoothstep(0.07, 0.0, length(f)) * 0.4;
    // a confirm lights the water for a moment
    if (pulse >= 0.0) c += vec3(0.25, 0.3, 0.35) * exp(-pulse * 7.0);
    c += (hash(p + fract(t) * 17.0) - 0.5) * 0.025;
    return vec4(clamp(c, 0.0, 1.0), 1.0);
}
";

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; position: relative; overflow: hidden; background: #0730c8; user-select: none;
       font-family: Inter, system-ui, sans-serif; -webkit-font-smoothing: antialiased; }
.water { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
.ui { left: 0; top: 0; }
.abs { position: absolute; }

/* -- the left: an ink slash, drifting shards, MENU running down the edge -- */
.ink { left: 0; top: 0; width: 520px; height: 100%; background: #f4fbff;
       clip-path: polygon(0 0, 62% 0, 38% 46%, 52% 100%, 0 100%);
       animation: inkin 420ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.ink2 { left: 0; top: 0; width: 520px; height: 100%; background: #3fd8ff;
        clip-path: polygon(30% 52%, 44% 50%, 56% 100%, 40% 100%); opacity: 0.75;
        animation: inkin 420ms cubic-bezier(0.2, 0, 0, 1) 80ms backwards; }
.menuword { left: -150px; top: 250px; font-size: 230px; font-weight: 900; letter-spacing: -14px; color: #050505;
            transform: rotate(90deg); white-space: nowrap; animation: wordin 520ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.bar { left: 0; top: 0; width: 38px; height: 280px; background: #7a7a7a; }
.shard { width: 34px; height: 46px; clip-path: polygon(50% 0, 100% 60%, 30% 100%);
         animation: drift 6s ease-in-out infinite alternate; }

/* -- the counter -- */
.wallet { left: 112px; top: 38px; width: 178px; height: 64px; border: 2px solid #000; background: rgba(255,255,255,0.92);
          padding: 6px 12px; box-sizing: border-box; animation: drop 360ms cubic-bezier(0.2, 0, 0, 1) 120ms backwards; }
.yen { font-size: 28px; font-weight: 300; color: #000; letter-spacing: -1px; font-variant-numeric: tabular-nums; line-height: 1.1; }
.wallet small { display: block; font-size: 10px; font-weight: 700; font-style: italic; color: #000; }

/* -- the menu: heavy italics, tilted, each where it sits in the stack -- */
.item { padding: 0 10px; font-weight: 900; color: #8af4ff; letter-spacing: -3px; white-space: nowrap; line-height: 1;
        transform: rotate(-14deg) skewX(-12deg); transform-origin: left center; cursor: pointer;
        transition-property: transform, color, opacity; transition-duration: 180ms;
        transition-timing-function: cubic-bezier(0.34, 1.56, 0.64, 1);
        animation: flyin 340ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.item span { position: relative; display: inline-block; }
.item.on { color: #050505; transform: rotate(-14deg) skewX(-12deg) scale(1.2) translateX(-8px); }
.dim .item { opacity: 0.4; transform: rotate(-14deg) skewX(-12deg) translateX(-30px); }
.dim .item.on { opacity: 1; transform: rotate(-14deg) skewX(-12deg) scale(1.2) translateX(-38px); }
.slash { position: absolute; left: -26px; top: 18%; height: 70%; width: calc(100% + 70px); background: #ff1530;
         clip-path: polygon(0 62%, 100% 0, 93% 100%, 2% 100%); transform-origin: left center;
         animation: slash 160ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.sliver { position: absolute; left: -12px; top: 6%; height: 80%; width: calc(100% + 92px); background: #ff8fd0;
          clip-path: polygon(93% 12%, 100% 4%, 97% 96%, 94.5% 96%); transform-origin: left center;
          animation: slash 160ms cubic-bezier(0.2, 0, 0, 1) 40ms backwards; }
/* on the label, not the item: swapping the item's own animation would replay its entrance */
.stamp { animation: stamp 260ms cubic-bezier(0.2, 0, 0, 1); }

/* -- the sub-menu a confirm slides in -- */
.panel { z-index: 30; right: 70px; top: 120px; width: 400px; padding: 18px 26px 22px; background: #f4fbff;
         box-shadow: 10px 10px 0 #050505; transform: skewX(-8deg);
         animation: panelin 300ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.panel h2 { margin: 0 0 10px; font-size: 34px; font-weight: 900; font-style: italic; color: #050505; letter-spacing: -2px;
            border-bottom: 5px solid #ff1530; padding-bottom: 4px; }
.row { display: flex; justify-content: space-between; padding: 8px 10px; font-size: 16px; font-weight: 700; color: #082a6e;
       transition-property: background-color, color; transition-duration: 80ms;
       animation: rowin 220ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.row:hover { background: #050505; color: #8af4ff; }
.row b { font-weight: 400; color: #5576b0; font-variant-numeric: tabular-nums; }
.out { animation: panelout 150ms cubic-bezier(0.3, 0, 1, 1) forwards; pointer-events: none; }

/* -- the hint, bottom right -- */
.hint { z-index: 30; right: 0; bottom: 74px; width: 330px; color: #fff; }
.hint .what { font-size: 30px; font-weight: 700; font-style: italic; letter-spacing: -1px; padding-left: 14px; }
.hint .kind { font-size: 12px; font-weight: 700; padding-left: 14px; border-bottom: 2px solid #fff; }
.keys { z-index: 30; right: 30px; bottom: 22px; display: flex; gap: 10px; color: #fff; font-size: 24px; font-weight: 700; font-style: italic; }
.key { display: flex; align-items: center; gap: 6px; padding: 4px 6px; cursor: pointer;
       transition-property: transform; transition-duration: 120ms; }
.key:active { transform: scale(0.96); }
.cap { width: 22px; height: 22px; border: 2px solid #fff; border-radius: 3px; font-size: 12px; font-style: normal;
       display: flex; align-items: center; justify-content: center; }

/* -- motion -- */
@keyframes inkin { from { opacity: 0; transform: translateX(-60px); } to { opacity: 1; transform: translateX(0); } }
@keyframes wordin { from { opacity: 0; transform: rotate(90deg) translateX(80px); } to { opacity: 1; transform: rotate(90deg) translateX(0); } }
@keyframes drop { from { opacity: 0; transform: translateY(-16px); } to { opacity: 1; transform: translateY(0); } }
@keyframes drift { from { transform: translate(0, 0) rotate(0deg); } to { transform: translate(14px, -22px) rotate(24deg); } }
@keyframes flyin { from { opacity: 0; transform: rotate(-14deg) skewX(-12deg) translateX(140px); }
                   to { opacity: 1; transform: rotate(-14deg) skewX(-12deg) translateX(0); } }
@keyframes slash { 0% { transform: scaleX(0); } 70% { transform: scaleX(1.08); } 100% { transform: scaleX(1); } }
@keyframes stamp { 0% { transform: scale(1); } 40% { transform: scale(1.18); } 100% { transform: scale(1); } }
@keyframes panelin { from { opacity: 0; transform: skewX(-8deg) translateX(120px); } to { opacity: 1; transform: skewX(-8deg) translateX(0); } }
@keyframes rowin { from { opacity: 0; transform: translateX(40px); } to { opacity: 1; transform: translateX(0); } }
@keyframes panelout { to { opacity: 0; transform: skewX(-8deg) translateX(60px); } }
";

/// Where each entry sits on the design canvas: x, y, font size (design px).
const AT: [(f32, f32, f32); 9] = [
    (500.0, 168.0, 62.0), (585.0, 232.0, 48.0), (562.0, 270.0, 56.0), (510.0, 318.0, 62.0), (590.0, 374.0, 50.0),
    (548.0, 418.0, 54.0), (540.0, 470.0, 50.0), (520.0, 522.0, 56.0), (556.0, 574.0, 50.0),
];

/// The counter in the top-left box, counted up on first light.
const REQUESTS: f32 = 73_797.0;
const DESIGN: (f32, f32) = (1280.0, 720.0);

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

struct Velvet {
    water: Fx,
    menu: MenuState,
    started: f32,
    /// Frames rendered: the page takes keyboard focus once one exists.
    frames: u32,
}

impl Velvet {
    fn panel(&self, i: usize, leaving: bool) -> Div {
        let entry = &ENTRIES[i];
        let mut p = div().id(format!("panel{i}-{}", self.menu.confirms)).class(if leaving { "abs panel out" } else { "abs panel" })
            .child(h2().child(entry.name));
        for (k, (name, value)) in entry.rows.iter().enumerate() {
            p = p.child(div().class("row").css("animation-delay", format!("{}ms", 120 + k * 40))
                .child(span().child(*name)).child(b().child(*value)));
        }
        p
    }
}

/// Places an absolutely positioned element (design px).
fn at(d: Div, x: f32, y: f32) -> Div {
    d.class("abs").css("left", rem(x)).css("top", rem(y))
}

impl Render for Velvet {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        // keys come to the page without a click: focus it once its first frame exists
        if w.focused_id().is_none() && self.frames > 0 {
            w.focus_id("root");
        }
        self.frames += 1;
        let fit = Fit::new(w, DESIGN);

        let confirmed = self.menu.confirmed;
        let water = self.water.canvas(move |_| {
            let t = now();
            [t, confirmed.map_or(-1.0, |c| t - c)]
        }).class("water");
        // the counter counts up on first light (ease-out cubic)
        let k = ((now() - self.started - 0.3) / 0.9).clamp(0.0, 1.0);
        let n = (REQUESTS * (1.0 - (1.0 - k).powi(3))).round() as u32;
        let requests = format!("{},{:03}", n / 1000, n % 1000);

        let mut left = div()
            .child(div().class("abs bar"))
            .child(div().class("abs ink")).child(div().class("abs ink2"));
        for (i, (x, y, color, delay)) in [(250.0, 120.0, "#ff6fd8", 0), (330.0, 300.0, "#3fd8ff", 1500), (180.0, 470.0, "#9b6bff", 800),
                                          (420.0, 560.0, "#ff6fd8", 2300), (90.0, 610.0, "#3fd8ff", 400)].into_iter().enumerate() {
            left = left.child(at(div().class("shard"), x, y).css("background", color)
                .css("animation-delay", format!("-{delay}ms")).css("transform", format!("rotate({}deg)", i * 40)));
        }
        left = left.child(div().class("abs menuword").child("MENU"));

        let m = &self.menu;
        let mut menu = at(div(), fit.ox, fit.oy).css("width", rem(DESIGN.0)).css("height", rem(DESIGN.1))
            .class(if m.open.is_some() { "dim" } else { "" });
        for (i, entry) in ENTRIES.iter().enumerate() {
            let on = i == m.sel;
            let (x, y, size) = AT[i];
            let mut d = at(div().id(format!("m{i}")), x, y).class(if on { "item on" } else { "item" })
                .css("font-size", rem(size)).css("animation-delay", format!("{}ms", 200 + i * 35))
                .css("z-index", if on { "20" } else { "10" });
            if on {
                // keyed by the pick, so the slash snaps in again on each selection
                d = d.child(div().id(format!("slash{}", m.picks)).class("slash"))
                    .child(div().id(format!("sliver{}", m.picks)).class("sliver"));
            }
            // the stamp is on the label, keyed by the confirm so a second Enter stamps again
            let label = if on && m.open == Some(i) { span().id(format!("stamp{}", m.confirms)).class("stamp") } else { span() };
            menu = menu.child(d.child(label.child(entry.name))
                .on_hover(cx.listener(move |this, over: &bool, _, cx| if *over && this.menu.select(i) { cx.notify() }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.menu.select(i);
                    this.menu.confirm();
                    cx.notify();
                })));
        }
        let mut ui = div().class("abs ui").css("width", rem(fit.w)).css("height", rem(fit.h))
            .child(left)
            .child(div().class("abs wallet").child(div().class("yen").child(requests)).child(small().child("requests today")))
            .child(menu);
        if let Some(i) = m.closing {
            ui = ui.child(self.panel(i, true));
        }
        if let Some(i) = m.open {
            ui = ui.child(self.panel(i, false));
        }
        ui = ui
            .child(div().class("abs hint").child(div().class("what").child(m.hint())).child(div().class("kind").child("Command")))
            .child(div().class("abs keys")
                .child(div().id("confirm").class("key").child(div().class("cap").child("⏎")).child("Confirm")
                    .on_click(cx.listener(|this, _, _, cx| if this.menu.confirm() { cx.notify() })))
                .child(div().id("close").class("key").child(div().class("cap").child("C")).child("Close")
                    .on_click(cx.listener(|this, _, _, cx| if this.menu.close() { cx.notify() }))));
        html().css("font-size", fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root").attr("tabindex", "0")
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| if this.menu.key(&e.keystroke.key) { cx.notify() }))
                .child(water).child(ui))
    }
}

/// Drives the menu headlessly with no click first: the arrow keys and Enter straight away, a
/// hover, Enter again, a click into another sub-menu, Close.
fn storyboard(dir: String) -> Script {
    Box::new(move |cx, win| {
        let shot = |cx: &mut App, name: &str| {
            let (w, h, px) = cx.draw(win);
            save_png(&format!("{dir}/{name}.png"), w, h, &px);
        };
        let centre = |cx: &mut App, id: &str| {
            let b = cx.element_bounds(win, id).unwrap_or_else(|| panic!("no #{id}"));
            (b.origin.x + b.size.width / 2.0, b.origin.y + b.size.height / 2.0)
        };
        let play = |cx: &mut App, name: &str, steps: &[u64]| {
            let mut at = 0;
            for &ms in steps {
                cx.advance_clock(Duration::from_millis(ms - at));
                at = ms;
                shot(cx, &format!("{name}-{ms:04}ms"));
            }
        };
        play(cx, "00-enter", &[0, 150, 300, 450, 700, 1200]);
        // no click: the pointer only comes into the window (the kit takes keys once it has)
        cx.simulate_mouse_move(win, 5.0, 5.0);
        cx.simulate_key(win, "", "Down");
        play(cx, "01-down", &[0, 60, 120, 200, 400]);
        cx.simulate_key(win, "", "Return");
        play(cx, "02-return", &[0, 80, 300, 600]);
        cx.simulate_key(win, "", "Down");
        play(cx, "03-down-while-open", &[0, 300]);
        let (x, y) = centre(cx, "m6");
        cx.simulate_click(win, x, y);
        play(cx, "04-click", &[0, 80, 600]);
        let (x, y) = centre(cx, "close");
        cx.simulate_click(win, x, y);
        play(cx, "05-close", &[0, 80, 300]);
    })
}

fn water() -> Fx {
    Fx::new("water", WATER, 2, [14, 110, 230])
}

fn main() {
    if let Some(dir) = arg("--frames") {
        let fx = water();
        let stills = [("water", [4.0f32, -1.0]), ("water-confirm", [4.1, 0.05])];
        if gpu_stills(&dir, 1280, 720, stills, |g, f| g.draw(fx.effect(), &[], f)).is_none() {
            eprintln!("water shader: {:?}", fx.error());
        }
        return;
    }
    let (w, h) = arg("--size").and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
        .unwrap_or(DESIGN);
    let make = || Velvet { water: water(), menu: MenuState::default(), started: now(), frames: 0 };
    run_with(w, h, "BUI · Velvet Menu", make, arg("--storyboard").map(storyboard));
}

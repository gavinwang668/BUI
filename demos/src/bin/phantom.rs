//! Phantom: a Persona 5-style main menu for a tech app, on BUI. A GPU shader paints the scene (a
//! red room, a black-and-white starburst spinning out on the right, a white star behind the menu,
//! a black blot, halftone, a red brush slash); over it, ransom-note type: every letter its own
//! font, size, tilt and box. The selected command's letters pop onto red, white and cyan blocks;
//! Enter / click / Confirm opens its sub-menu, C / Escape / Close puts it away.
//!
//! Motion ("energetic"): a selection re-pops its letters one after another (18 ms apart, 200 ms
//! each, overshooting), the pointer shard snaps in behind; a confirm flashes the scene and slams
//! the card in; the starburst keeps turning outward underneath.
//!
//! `cargo run --release --bin phantom`
//! `-- --storyboard DIR` drives it headlessly and saves frames along BUI's virtual clock
//! (`-- --size 1920x1080` for another window size); `-- --frames DIR` renders the scene on the GPU.

use std::sync::OnceLock;
use std::time::Duration;

use blenderui::prelude::*;
use bui_demos::canvas::{rem, rem_css, Fit};
use bui_demos::menu::{MenuState, ENTRIES};
use bui_demos::{arg, gpu_stills, now, run_with, save_png, Fx, Script};

/// U[0] = time, U[1] = seconds since the last confirm (< 0: none).
const SCENE: &str = r"
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p); vec2 f = fract(p); f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}
// a five-point jagged star's radius factor at angle a (a triangle wave around the circle)
float jag(float a, float n, float depth) { return 1.0 + depth * abs(fract(a * n / 6.2831853) - 0.5) * 2.0; }
// distance to the segment a-b
float seg(vec2 p, vec2 a, vec2 b) { vec2 pa = p - a; vec2 ba = b - a; return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0)); }

vec4 effect(vec2 p) {
    float t = U[0]; float pulse = U[1];
    vec2 uv = p / box_size;
    float asp = box_size.x / box_size.y;
    vec2 q = vec2(uv.x * asp, uv.y);
    // the red room: a deep red wall, window frames, grime
    vec3 col = mix(vec3(0.45, 0.0, 0.03), vec3(0.86, 0.04, 0.09), 0.55 * smoothstep(0.0, 0.7, uv.x) + 0.45 * vnoise(q * 2.5 + t * 0.04));
    float fx = abs(fract(q.x * 1.3 + 0.15 * uv.y) - 0.5);
    float fy = abs(fract(uv.y * 2.2 + 0.1) - 0.5);
    col *= 1.0 - 0.5 * max(smoothstep(0.43, 0.46, fx), smoothstep(0.44, 0.47, fy) * step(fx, 0.43));
    col *= 0.78 + 0.22 * vnoise(q * 24.0);
    // the starburst: black and white jagged rings turning outward, on the right
    vec2 d = q - vec2(asp * 0.86, 0.6);
    float r = length(d); float a = atan2(d.y, d.x) + t * 0.05;
    float rings = fract(r * 15.0 / jag(a, 5.0, 0.6) - t * 0.25);
    vec3 burst = vec3(step(0.5, rings) * 0.88 + 0.05);
    float edge = q.x - asp * 0.6 - 0.06 * sin(uv.y * 11.0) - 0.04 * (fract(uv.y * 5.0) - 0.5);
    col = mix(col, burst, smoothstep(-0.002, 0.002, edge));
    // the white star behind the menu, inked
    vec2 s = q - vec2(asp * 0.44, 0.47);
    float sr = length(s) / (0.3 * jag(atan2(s.y, s.x) + 0.3, 5.0, 0.75));
    col = mix(col, vec3(0.02), smoothstep(1.02, 1.0, sr));
    col = mix(col, vec3(0.93), smoothstep(0.985, 0.965, sr));
    // a black blot over the middle, ragged
    vec2 b = (q - vec2(asp * 0.64, 0.66)) / vec2(0.3, 0.42);
    float blot = length(b) + 0.12 * vnoise(q * 9.0) - 0.06;
    col = mix(col, vec3(0.02), smoothstep(1.0, 0.98, blot));
    // halftone, white on the black, top of the blot
    vec2 cell = fract(q * 46.0) - 0.5;
    float tone = smoothstep(0.38, 0.05, length(q - vec2(asp * 0.66, 0.32)));
    col = mix(col, vec3(0.9), step(length(cell), 0.28 * tone) * step(blot, 1.0));
    // the red brush slash
    float stroke = min(seg(q, vec2(asp * 0.56, 0.14), vec2(asp * 0.52, 0.52)), seg(q, vec2(asp * 0.52, 0.52), vec2(asp * 0.7, 0.6)));
    float w = 0.045 + 0.012 * vnoise(q * 30.0);
    col = mix(col, vec3(0.02), smoothstep(w + 0.008, w + 0.004, stroke));
    col = mix(col, vec3(0.78, 0.02, 0.04), smoothstep(w, w - 0.004, stroke));
    // a confirm: the scene flashes
    if (pulse >= 0.0) col = mix(col, vec3(1.0), 0.5 * exp(-pulse * 10.0));
    return vec4(col, 1.0);
}
";

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; position: relative; overflow: hidden; background: #8a0010; user-select: none;
       font-family: Inter, system-ui, sans-serif; -webkit-font-smoothing: antialiased; }
.scene { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
.abs { position: absolute; }
.ui { left: 0; top: 0; }

/* -- ransom-note letters: each one its own font, size, tilt and box -- */
.g { display: inline-block; line-height: 1; padding: 1px 3px; margin: 0 -1px; color: #fff;
     text-shadow: 2px 2px 0 #000, -1px -1px 0 #000, 1px -1px 0 #000, -1px 1px 0 #000; }
.g.ink { background: #050505; color: #fff; text-shadow: none; }
.g.paper { background: #f2f2f2; color: #050505; text-shadow: none; }
.g.blood { background: #e0001b; color: #fff; text-shadow: none; }
.g.sky { background: #3fe8ff; color: #050505; text-shadow: none; }
.g.pop { animation: pop 200ms cubic-bezier(0.2, 0, 0, 1) backwards; }

/* -- the menu -- */
.item { cursor: pointer; white-space: nowrap; transform: rotate(-3deg); transform-origin: right center;
        animation: drop 320ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.item .strip { padding: 2px 8px 4px; background: #050505;
               clip-path: polygon(2% 8%, 100% 0, 97% 100%, 0 92%); }
.item.on .strip { background: transparent; clip-path: none; }
.item.on { transform: rotate(-6deg) scale(1.22); transition-property: transform; transition-duration: 180ms;
           transition-timing-function: cubic-bezier(0.34, 1.56, 0.64, 1); }
.shard { position: absolute; right: -46px; top: -6px; width: 52px; height: 46px; background: #3fe8ff;
         clip-path: polygon(0 40%, 100% 0, 30% 100%); animation: shard 180ms cubic-bezier(0.2, 0, 0, 1) 60ms backwards; }

/* -- the labels: tilted, inked, stuck on -- */
.label { white-space: nowrap; transform-origin: left center; animation: slam 260ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.card { background: #f2f2f2; color: #050505; padding: 6px 16px; box-shadow: 6px 6px 0 #050505; font-weight: 800; }
.card.black { background: #050505; color: #fff; box-shadow: 6px 6px 0 #f2f2f2; text-align: center; }
.card small { display: block; font-size: 13px; letter-spacing: 1px; font-weight: 700; }
.hint { background: #050505; color: #fff; padding: 2px 10px; font-size: 22px; font-style: italic; font-weight: 700;
        border: 2px solid #fff; transform: rotate(-4deg); animation: slam 200ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.keys { right: 26px; bottom: 18px; display: flex; gap: 10px; }
.key { display: flex; align-items: center; gap: 6px; padding: 4px 10px; background: #f2f2f2; color: #050505;
       border: 3px solid #050505; font-size: 20px; font-weight: 800; font-style: italic; cursor: pointer; transform: rotate(-3deg);
       transition-property: transform; transition-duration: 120ms; }
.key:active { transform: rotate(-3deg) scale(0.96); }
.cap { width: 22px; height: 22px; border: 2px solid #050505; font-size: 12px; font-style: normal;
       display: flex; align-items: center; justify-content: center; }

/* -- the sub-menu a confirm slams in -- */
.panel { z-index: 30; right: 40px; top: 330px; width: 360px; padding: 16px 22px 20px; background: #050505;
         border: 4px solid #f2f2f2; box-shadow: 10px 10px 0 #e0001b; transform: rotate(-4deg);
         animation: panelin 260ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.panel .head { margin-bottom: 10px; }
.row { display: flex; justify-content: space-between; gap: 16px; padding: 6px 10px; font-size: 16px; font-weight: 800; color: #fff;
       transition-property: background-color; transition-duration: 80ms;
       animation: rowin 200ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.row:hover { background: #e0001b; }
.row b { font-weight: 600; color: #bdbdbd; font-style: italic; font-variant-numeric: tabular-nums; }
.out { animation: panelout 140ms cubic-bezier(0.3, 0, 1, 1) forwards; pointer-events: none; }

/* -- motion -- */
@keyframes pop { 0% { opacity: 0; transform: scale(0.4) rotate(-20deg); } 60% { opacity: 1; transform: scale(1.25) rotate(4deg); }
                 100% { opacity: 1; transform: scale(1) rotate(0deg); } }
@keyframes drop { from { opacity: 0; transform: rotate(-3deg) translateY(-24px); } to { opacity: 1; transform: rotate(-3deg) translateY(0); } }
@keyframes shard { from { opacity: 0; transform: translateX(-30px) scale(0.3); } to { opacity: 1; transform: translateX(0) scale(1); } }
@keyframes slam { from { opacity: 0; transform: scale(1.6); } to { opacity: 1; transform: scale(1); } }
@keyframes panelin { 0% { opacity: 0; transform: rotate(-4deg) translateX(140px) scale(1.1); }
                     70% { opacity: 1; transform: rotate(-4deg) translateX(-8px) scale(0.98); }
                     100% { opacity: 1; transform: rotate(-4deg) translateX(0) scale(1); } }
@keyframes rowin { from { opacity: 0; transform: translateX(36px); } to { opacity: 1; transform: translateX(0); } }
@keyframes panelout { to { opacity: 0; transform: rotate(-4deg) translateX(60px); } }
";

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

const DESIGN: (f32, f32) = (1280.0, 720.0);
/// Where each command's right edge sits (x, y) and its type size, on the design canvas.
const AT: [(f32, f32, f32); 9] = [
    (720.0, 128.0, 40.0), (735.0, 176.0, 36.0), (712.0, 218.0, 38.0), (690.0, 260.0, 40.0), (668.0, 304.0, 36.0),
    (650.0, 344.0, 38.0), (700.0, 386.0, 36.0), (680.0, 430.0, 38.0), (650.0, 474.0, 36.0),
];
const FONTS: [&str; 7] = ["Inter", "Georgia", "Impact", "Consolas", "Times New Roman", "Arial Black", "Courier New"];

/// A small deterministic hash, so each letter keeps its look from frame to frame.
const fn mix(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

/// How loud a ransom word is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Voice {
    /// White letters, now and then inked or on paper.
    Plain,
    /// Every letter on a block: red, white, cyan, black. Each pops in, keyed by `pick`.
    Loud { pick: u32 },
}

/// `text` as a ransom note: each letter its own font, size, tilt, case and box (`seed` fixes them).
fn ransom(text: &str, seed: u32, size: f32, voice: Voice) -> Div {
    let mut out = div().css("display", "inline-block");
    for (k, ch) in text.chars().enumerate() {
        if ch == ' ' {
            out = out.child(span().css("display", "inline-block").css("width", rem(size * 0.3)));
            continue;
        }
        let h = mix(seed, k as u32 + 1);
        let font = FONTS[(h % FONTS.len() as u32) as usize];
        let scale = 0.82 + f32::from((h >> 3) as u8 % 41) / 100.0;
        let tilt = f32::from((h >> 11) as u8 % 19) - 9.0;
        let lower = (h >> 17).is_multiple_of(3);
        let shown: String = if lower { ch.to_lowercase().collect() } else { ch.to_uppercase().collect() };
        let class = match voice {
            Voice::Plain => match (h >> 20) % 7 { 0 => "g ink", 1 => "g paper", _ => "g" },
            Voice::Loud { .. } => ["g blood", "g paper", "g sky", "g ink", "g paper"][((h >> 20) % 5) as usize],
        };
        let mut g = span().class(class).css("font-family", font).css("font-size", rem(size * scale))
            .css("transform", format!("rotate({tilt}deg)")).css("font-weight", if (h >> 24).is_multiple_of(2) { "900" } else { "700" });
        if let Voice::Loud { pick } = voice {
            g = g.id(format!("g{seed}-{pick}-{k}")).class("pop").css("animation-delay", format!("{}ms", k * 18));
        }
        out = out.child(g.child(shown));
    }
    out
}

/// Places an absolutely positioned element (design px).
fn at(d: Div, x: f32, y: f32) -> Div {
    d.class("abs").css("left", rem(x)).css("top", rem(y))
}

struct Phantom {
    scene: Fx,
    menu: MenuState,
    /// Frames rendered: the page takes keyboard focus once one exists.
    frames: u32,
}

impl Phantom {
    fn panel(&self, i: usize, leaving: bool) -> Div {
        let entry = &ENTRIES[i];
        let mut p = div().id(format!("panel{i}-{}", self.menu.confirms)).class(if leaving { "abs panel out" } else { "abs panel" })
            .child(div().class("head").child(ransom(entry.name, 900 + i as u32, 30.0, Voice::Loud { pick: self.menu.confirms })));
        for (k, (name, value)) in entry.rows.iter().enumerate() {
            p = p.child(div().class("row").css("animation-delay", format!("{}ms", 110 + k * 40))
                .child(span().child(*name)).child(b().child(*value)));
        }
        p
    }
}

impl Render for Phantom {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        if w.focused_id().is_none() && self.frames > 0 {
            w.focus_id("root");
        }
        self.frames += 1;
        let fit = Fit::new(w, DESIGN);
        let m = &self.menu;

        let confirmed = m.confirmed;
        let scene = self.scene.canvas(move |_| {
            let t = now();
            [t, confirmed.map_or(-1.0, |c| t - c)]
        }).class("scene");

        // the commands, right-aligned down the white star, each a ransom note on a black strip
        let mut menu = at(div(), fit.ox, fit.oy).css("width", rem(DESIGN.0)).css("height", rem(DESIGN.1));
        for (i, entry) in ENTRIES.iter().enumerate() {
            let on = i == m.sel;
            let (right, y, size) = AT[i];
            let voice = if on { Voice::Loud { pick: m.picks } } else { Voice::Plain };
            let mut d = div().id(format!("m{i}")).class(if on { "abs item on" } else { "abs item" })
                .css("right", rem(DESIGN.0 - right)).css("top", rem(y)).css("animation-delay", format!("{}ms", 150 + i * 30))
                .css("z-index", if on { "20" } else { "10" })
                .child(div().class("strip").child(ransom(entry.name, 100 + i as u32, size, voice)));
            if on {
                d = d.child(div().id(format!("shard{}", m.picks)).class("shard"));
            }
            menu = menu.child(d
                .on_hover(cx.listener(move |this, over: &bool, _, cx| if *over && this.menu.select(i) { cx.notify() }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.menu.select(i);
                    this.menu.confirm();
                    cx.notify();
                })));
        }
        // the labels around it
        let (ox, oy) = (fit.ox, fit.oy);
        let label = |x: f32, y: f32, tilt: f32, delay: usize| {
            at(div().class("label"), ox + x, oy + y).css("transform", format!("rotate({tilt}deg)"))
                .css("animation-delay", format!("{delay}ms"))
        };
        let top = div()
            .child(label(790.0, 22.0, -10.0, 300).child(ransom("Don't break", 7, 30.0, Voice::Plain)))
            .child(label(1000.0, 46.0, -4.0, 360).child(div().class("card").child(ransom("PROD", 8, 44.0, Voice::Plain))));
        let right = fit.w - DESIGN.0 + 950.0 - ox;
        let cards = div()
            .child(label(right, 186.0, -3.0, 420).child(div().class("card").css("font-size", rem(20.0)).child("Acme Cloud")))
            .child(label(right - 14.0, 232.0, -5.0, 470).child(div().class("card black")
                .child(ransom("MENU: MAIN", 9, 36.0, Voice::Plain)).child(small().child("select a command"))));
        let command = div()
            .child(label(400.0, 520.0, -4.0, 480).child(ransom("COMMAND", 11, 76.0, Voice::Plain)))
            .child(at(div().id(format!("hint{}", m.sel)).class("hint"), ox + 560.0, oy + 622.0).child(m.hint()))
            .child(label(250.0, 500.0, -6.0, 520).child(ransom("152 builds", 12, 26.0, Voice::Plain)))
            .child(label(150.0, 622.0, -6.0, 560).child(ransom("$86,683 MRR", 13, 42.0, Voice::Plain)));

        let mut ui = div().class("abs ui").css("width", rem(fit.w)).css("height", rem(fit.h))
            .child(top).child(cards).child(menu).child(command);
        if let Some(i) = m.closing {
            ui = ui.child(self.panel(i, true));
        }
        if let Some(i) = m.open {
            ui = ui.child(self.panel(i, false));
        }
        ui = ui.child(div().class("abs keys")
            .child(div().id("close").class("key").child(div().class("cap").child("C")).child("Close")
                .on_click(cx.listener(|this, _, _, cx| if this.menu.close() { cx.notify() })))
            .child(div().id("confirm").class("key").child(div().class("cap").child("⏎")).child("Confirm")
                .on_click(cx.listener(|this, _, _, cx| if this.menu.confirm() { cx.notify() }))));
        html().css("font-size", fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root").attr("tabindex", "0")
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| if this.menu.key(&e.keystroke.key) { cx.notify() }))
                .child(scene).child(ui))
    }
}

/// Drives the menu headlessly: first light, the arrow keys and Enter, ↓ with the card open, a
/// click into another command, Close.
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
        play(cx, "00-enter", &[0, 150, 300, 500, 900]);
        // no click: the pointer only comes into the window (the kit takes keys once it has)
        cx.simulate_mouse_move(win, 5.0, 5.0);
        cx.simulate_key(win, "", "Down");
        play(cx, "01-down", &[0, 40, 90, 160, 400]);
        cx.simulate_key(win, "", "Return");
        play(cx, "02-return", &[0, 80, 160, 500]);
        cx.simulate_key(win, "", "Down");
        play(cx, "03-down-while-open", &[0, 400]);
        let (x, y) = centre(cx, "m5");
        cx.simulate_click(win, x, y);
        play(cx, "04-click", &[0, 500]);
        let (x, y) = centre(cx, "close");
        cx.simulate_click(win, x, y);
        play(cx, "05-close", &[0, 300]);
    })
}

fn scene() -> Fx {
    Fx::new("phantom", SCENE, 2, [140, 0, 16])
}

fn main() {
    if let Some(dir) = arg("--frames") {
        let fx = scene();
        let stills = [("phantom", [4.0f32, -1.0]), ("phantom-confirm", [4.05, 0.05])];
        if gpu_stills(&dir, 1280, 720, stills, |g, f| g.draw(fx.effect(), &[], f)).is_none() {
            eprintln!("phantom shader: {:?}", fx.error());
        }
        return;
    }
    let (w, h) = arg("--size").and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
        .unwrap_or(DESIGN);
    let make = || Phantom { scene: scene(), menu: MenuState::default(), frames: 0 };
    run_with(w, h, "BUI · Phantom", make, arg("--storyboard").map(storyboard));
}

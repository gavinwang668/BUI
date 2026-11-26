//! ASCII Field: a dark engineer's portfolio over a background that answers the pointer, ported
//! from a portfolio site's hero (its WebGL "ice flow" pass and its canvas glyph field).
//!
//! The background is one GPU shader: a slow FBM scene (three layers drifting at their own speeds
//! and angles, cross-warped, vignetted, grained, in a near-black green) is sampled once per 13 px
//! cell and drawn as a 5 x 5 bitmap glyph picked by the cell's brightness. Near the pointer a halo
//! lifts the brightness, so the dim field turns into bright code around it (except in the middle,
//! where the headline sits); it eases in when the pointer arrives and out when it leaves the
//! window. Scanlines and a vignette finish it.
//!
//! Over it: cards that lift 4 px under the pointer with a 220 px green glow that follows it, and a
//! call-to-action band filled with a second, DOM-free glyph field (`Painter` text): random
//! characters that morph about once a second and brighten within 180 px of the pointer.
//!
//! Motion: the hero and the cards rise once on load (0.7 s, 90 ms apart); hover lift and glow are
//! 200 ms transitions; the glow halo follows at a frame-rate independent 0.12-per-30fps rate.
//!
//! `cargo run --release --bin ascii`
//! `-- --storyboard DIR` drives the pointer over it headlessly; `-- --frames DIR` renders the
//! background on the GPU with the pointer in a few places.

use std::sync::OnceLock;

use blenderui::prelude::*;
use bui_demos::canvas::{at, rem, rem_css, Fit};
use bui_demos::ease::approach;
use bui_demos::{arg, gpu_stills, now, run_with, story::story, Fx, Script};

/// The glow of a card, the pointer at (`x`, `y`) in it (design px): only the card it is over draws one.
fn card_glow(x: f32, y: f32) -> Div {
    let glow = div().class("glow");
    if (0.0..CARD_W).contains(&x) && (0.0..CARD_H).contains(&y) {
        glow.css("background", format!("radial-gradient({} circle at {} {}, rgba(0,255,65,0.09), transparent 70%)",
                                       rem(220.0), rem(x), rem(y)))
    } else {
        glow
    }
}

/// U[0] time, U[1..2] pointer (logical px in the box), U[3] glow 0..1, U[4] the box's logical width.
const FIELD: &str = r"
float hash21(vec2 p) { vec3 p3 = fract(vec3(p.xyx) * 0.1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
float noise(vec2 p) {
    vec2 i = floor(p); vec2 f = fract(p);
    float a = hash21(i); float b = hash21(i + vec2(1.0, 0.0));
    float c = hash21(i + vec2(0.0, 1.0)); float d = hash21(i + vec2(1.0, 1.0));
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}
float fbm(vec2 p) {
    float v = 0.0; float a = 0.5;
    for (int i = 0; i < 5; i++) { v += a * noise(p); p *= 2.05; a *= 0.5; }
    return v;
}
vec3 palette(float t) {
    vec3 base = vec3(0.008, 0.016, 0.011);
    vec3 mid = vec3(0.028, 0.082, 0.05);
    vec3 peak = vec3(0.05, 0.14, 0.085);
    float s = clamp(t, 0.0, 1.0);
    return mix(base, mix(mid, peak, smoothstep(0.52, 1.0, s)), smoothstep(0.0, 0.68, s));
}
// the scene at uv (0..1), pix (cell centre, logical px) for the grain
vec3 scene(vec2 uv, vec2 pix, float t) {
    vec2 ar = vec2(box_size.x / box_size.y, 1.0);
    vec2 p = (uv - 0.5) * ar;
    float l1 = fbm(p * 1.7 + vec2(t * 0.032, t * 0.012));
    float l2 = fbm(p * 2.3 + vec2(-t * 0.018, t * 0.024) + vec2(3.7, 1.4));
    float warp = fbm(p * 1.4 + vec2(l1 * 1.8, l2 * 1.4) + t * 0.015);
    float mixed = l1 * 0.42 + l2 * 0.32 + warp * 0.26;
    mixed *= 1.0 - smoothstep(0.42, 0.92, length(p));
    mixed = clamp(mixed + (hash21(pix + vec2(t * 47.0)) - 0.5) * 0.022, 0.0, 1.0);
    return palette(mixed);
}
// bit a of the 25-bit glyph n (exact in a float: n < 2^25 and even past 2^24)
float bit(float n, float a) { return mod(floor(n / exp2(a)), 2.0); }
float character(float n, vec2 p) {
    p = floor(p * vec2(4.0, 4.0) + 2.5);
    if (p.x < 0.0 || p.x > 4.0 || p.y < 0.0 || p.y > 4.0) return 0.0;
    return bit(n, p.x + 5.0 * p.y);
}
vec4 effect(vec2 p) {
    float s = box_size.x / max(U[4], 1.0);
    float cell = 13.0 * s;
    vec2 uv = p / box_size;
    vec2 ar = vec2(box_size.x / box_size.y, 1.0);
    vec2 centre = (floor(p / cell) + 0.5) * cell;
    float gray = dot(scene(centre / box_size, centre / s, U[0]), vec3(0.299, 0.587, 0.114));
    float boost = 0.0;
    if (U[3] > 0.001) {
        vec2 m = vec2(U[1], U[2]) * s / box_size;
        float halo = smoothstep(0.32, 0.0, length((uv - m) * ar));
        float fade = smoothstep(0.18, 0.42, length((uv - 0.5) * ar));
        boost = halo * U[3] * fade;
        gray = clamp(gray + boost * 0.9, 0.0, 1.0);
    }
    float n = 0.0;
    if (gray > 0.05) n = 4194304.0;
    if (gray > 0.15) n = 131200.0;
    if (gray > 0.25) n = 4329604.0;
    if (gray > 0.35) n = 14815374.0;
    if (gray > 0.45) n = 4357252.0;
    if (gray > 0.55) n = 15255086.0;
    if (gray > 0.65) n = 4532014.0;
    if (gray > 0.75) n = 11512810.0;
    if (gray > 0.85) n = 27141542.0;
    float ch = character(n, mod(p, cell) / cell * 2.0 - 1.0);
    vec3 green = vec3(0.180, 0.490, 0.310);
    vec3 col = green * (0.55 + gray * 1.35) * ch * gray * 1.5;
    col += green * boost * 0.45;
    float scan = pow(sin(p.y / s * 3.14159 * 2.0) * 0.5 + 0.5, 1.5) * 0.07 * 0.15;
    col -= scan;
    col *= clamp(1.0 - length(uv - 0.5) * 0.28 * 1.2, 0.0, 1.0);
    return vec4(max(col, vec3(0.0)), 1.0);
}
";

const DESIGN: (f32, f32) = (1280.0, 800.0);
/// The call-to-action band's glyph field: cells across and down, and the band (design px).
const COLS: usize = 56;
const ROWS: usize = 9;
const BAND: (f32, f32, f32, f32) = (80.0, 586.0, 1120.0, 180.0);
const GLYPHS: &[u8] = b" .:-=+*x?#@%/\\|<>{}[]()0123abcXYZ";
/// The field's knobs, as the site's band sets them.
const DENSITY: f32 = 0.18;
const MORPH_HZ: f32 = 1.1;
const OPACITY: f32 = 0.35;
/// Within this many design px of the pointer a glyph brightens.
const REACH: f32 = 180.0;
/// The halo's follow rate: 0.12 of the way per frame at 30 fps.
const GLOW_RATE: f32 = 3.835;

const CARDS: [(&str, &str, &str); 3] = [
    ("01", "Agent systems", "Tool-using agents with evals, tracing and a rollback plan, shipped to production instead of a demo day."),
    ("02", "Realtime products", "Sync engines, presence and collaborative editing that stay under 50 ms on a bad hotel Wi-Fi."),
    ("03", "Performance audits", "Two weeks, one report, a ranked list of fixes. Typical result: a 40% faster first load."),
];
const CARD_Y: f32 = 352.0;
const CARD_W: f32 = 352.0;
const CARD_H: f32 = 190.0;
const CARD_GAP: f32 = 32.0;

const CSS: &str = r"
html { height: 100%; background: #000; }
body { margin: 0; height: 100%; position: relative; overflow: hidden; background: #000; color: #d7e4dc; user-select: none;
       font-family: Inter, 'Segoe UI', system-ui, sans-serif; -webkit-font-smoothing: antialiased; }
.bg { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
.abs { position: absolute; }
.mono { font-family: 'JetBrains Mono', Consolas, monospace; }

.nav { left: 80px; top: 34px; width: 1120px; height: 44px; display: flex; align-items: center; justify-content: space-between;
       animation: rise 700ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.logo { font-size: 15px; font-weight: 700; letter-spacing: 0.5px; color: #e8f5ec; }
.logo b { color: #00ff41; font-weight: 700; }
.links { display: flex; gap: 6px; }
.link { height: 40px; padding: 0 14px; display: flex; align-items: center; font-size: 13px; color: #8da597; border-radius: 10px;
        cursor: pointer; transition-property: color, background-color; transition-duration: 150ms; }
.link:hover { color: #e8f5ec; background: rgba(0,255,65,0.06); }

.hero { left: 80px; top: 120px; width: 1120px; text-align: center; }
.eyebrow { font-size: 12px; letter-spacing: 2px; color: #00ff41; animation: rise 700ms cubic-bezier(0.2, 0, 0, 1) 80ms backwards; }
.hero h1 { margin: 16px auto 0; width: 760px; font-size: 58px; line-height: 1.02; font-weight: 700; letter-spacing: -2px; color: #f2fbf5;
           text-wrap: balance; animation: rise 700ms cubic-bezier(0.2, 0, 0, 1) 160ms backwards; }
.hero p { margin: 18px auto 0; width: 560px; font-size: 16px; line-height: 1.5; color: #8da597; text-wrap: pretty;
          animation: rise 700ms cubic-bezier(0.2, 0, 0, 1) 240ms backwards; }

.card { width: 352px; height: 190px; box-sizing: border-box; padding: 24px; border-radius: 18px; overflow: hidden; cursor: pointer;
        background: rgba(10,14,12,0.78); backdrop-filter: blur(6px);
        box-shadow: 0 0 0 1px rgba(255,255,255,0.08), 0 10px 30px rgba(0,0,0,0.45);
        transition-property: transform, box-shadow; transition-duration: 200ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1);
        animation: rise 700ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.card:hover { transform: translateY(-4px); box-shadow: 0 0 0 1px rgba(0,255,65,0.22), 0 20px 52px rgba(0,0,0,0.55); }
.card:active { transform: translateY(-4px) scale(0.96); }
/* the reference's `.hire__card:before`: a 220 px green circle at the pointer, fading out */
.glow { position: absolute; left: 0; top: 0; width: 352px; height: 190px; opacity: 0;
        transition-property: opacity; transition-duration: 200ms; }
.card:hover .glow { opacity: 1; }
.card .num { position: relative; font-size: 12px; color: #00ff41; letter-spacing: 1px; }
.card h3 { position: relative; margin: 14px 0 8px; font-size: 21px; font-weight: 650; color: #f2fbf5; letter-spacing: -0.4px; }
.card p { position: relative; margin: 0; font-size: 13.5px; line-height: 1.5; color: #8da597; text-wrap: pretty; }

/* no overflow clip: the field keeps to its cells, and cutting its hundreds of glyphs at the rounded
   corners every frame would double the page's cost */
.band { left: 80px; top: 586px; width: 1120px; height: 180px; border-radius: 22px; background: #050806;
        box-shadow: 0 0 0 1px rgba(0,255,65,0.14), 0 24px 60px rgba(0,0,0,0.6);
        animation: rise 700ms cubic-bezier(0.2, 0, 0, 1) 420ms backwards; }
.field { position: absolute; left: 0; top: 0; width: 1120px; height: 180px; }
.cta { position: absolute; left: 0; top: 0; width: 100%; height: 100%; display: flex; align-items: center; justify-content: space-between;
       padding: 0 48px; box-sizing: border-box; }
.cta h2 { margin: 0; font-size: 30px; font-weight: 700; letter-spacing: -0.8px; color: #f2fbf5; text-wrap: balance; }
.cta small { display: block; margin-top: 6px; font-size: 13px; color: #8da597; }
.btn { height: 48px; padding: 0 22px; border-radius: 12px; display: flex; align-items: center; gap: 10px; cursor: pointer;
       background: #00ff41; color: #021006; font-size: 14px; font-weight: 700;
       box-shadow: 0 0 0 1px rgba(0,255,65,0.5), 0 8px 30px rgba(0,255,65,0.25);
       transition-property: transform, box-shadow; transition-duration: 150ms; }
.btn:hover { box-shadow: 0 0 0 1px rgba(0,255,65,0.8), 0 10px 40px rgba(0,255,65,0.45); }
.btn:active { transform: scale(0.96); }

@keyframes rise { from { opacity: 0; transform: translateY(18px); } to { opacity: 1; transform: translateY(0); } }
";

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

struct Ascii {
    field: Fx,
    /// The pointer in window px, and whether it is in the window (the halo's target).
    pointer: (f32, f32),
    inside: bool,
    /// The halo, easing towards `inside`.
    glow: f32,
    last: f32,
    /// The band's glyphs: an index into `GLYPHS`, or -1 for none.
    glyphs: [i8; COLS * ROWS],
    morphed: f32,
    seed: u32,
}

impl Ascii {
    fn new() -> Self {
        let mut a = Self { field: field(), pointer: (-1e4, -1e4), inside: false, glow: 0.0, last: now(), glyphs: [-1; COLS * ROWS],
                           morphed: now(), seed: 0x9e37_79b9 };
        for i in 0..a.glyphs.len() {
            a.glyphs[i] = if a.rand() < DENSITY { a.pick() } else { -1 };
        }
        a
    }

    /// xorshift32: 0..1.
    fn rand(&mut self) -> f32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn pick(&mut self) -> i8 {
        ((self.rand() * GLYPHS.len() as f32) as usize).min(GLYPHS.len() - 1) as i8
    }

    /// The field's morph: a few cells appear, change or go out (the glyph field step of the reference).
    fn morph(&mut self) {
        let n = ((self.glyphs.len() as f32 * 0.004) as usize).max(4);
        for _ in 0..n {
            let i = ((self.rand() * self.glyphs.len() as f32) as usize).min(self.glyphs.len() - 1);
            self.glyphs[i] = if self.glyphs[i] < 0 {
                if self.rand() < DENSITY { self.pick() } else { -1 }
            } else if self.rand() < 0.18 {
                -1
            } else {
                self.pick()
            };
        }
    }
}

/// The band's glyph field painted into `b`: each glyph's alpha rises near `pointer` (window px).
fn paint_field(b: Bounds<f32>, p: &mut Painter, glyphs: &[i8; COLS * ROWS], pointer: (f32, f32), reach: f32) {
    let cell = b.size.width / COLS as f32;
    let reach2 = reach * reach;
    for (i, &g) in glyphs.iter().enumerate() {
        if g < 0 {
            continue;
        }
        let (col, row) = (i % COLS, i / COLS);
        // clear of the copy and the button (the reference's exclusion ellipses), fading in past them
        let e = clear_of_copy((col as f32 + 0.5) / COLS as f32, (row as f32 + 0.5) / ROWS as f32);
        if e < 0.78 || ((col == 0 || col == COLS - 1) && (row == 0 || row == ROWS - 1)) {
            continue;
        }
        let edge = ((e - 0.78) / 0.32).min(1.0);
        let (x, y) = (b.origin.x + col as f32 * cell, b.origin.y + row as f32 * cell);
        let (dx, dy) = (x + cell * 0.5 - pointer.0, y + cell * 0.5 - pointer.1);
        let d2 = dx * dx + dy * dy;
        let a = edge * if d2 < reach2 { (OPACITY + (1.0 - d2 / reach2) * 0.65).min(1.0) } else { OPACITY };
        let k = g as usize;
        // SAFETY of the slice: GLYPHS is ASCII, so every byte is a char boundary
        let s = std::str::from_utf8(&GLYPHS[k..=k]).unwrap_or(" ");
        p.text(x, y, s, cell * 0.85, Rgba { r: 0.0, g: 1.0, b: 0.255, a });
    }
}

/// How far (u, v) in the band (0..1) is outside the ellipses round its heading and its button:
/// below 1 inside one.
fn clear_of_copy(u: f32, v: f32) -> f32 {
    let ellipse = |cx: f32, cy: f32, rx: f32, ry: f32| (((u - cx) / rx).powi(2) + ((v - cy) / ry).powi(2)).sqrt();
    ellipse(0.27, 0.5, 0.29, 0.5).min(ellipse(0.885, 0.5, 0.12, 0.42))
}

impl Render for Ascii {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        let fit = Fit::new(w, DESIGN);
        let t = now();
        self.glow = approach(self.glow, if self.inside { 1.0 } else { 0.0 }, GLOW_RATE, (t - self.last).clamp(0.0, 0.1));
        self.last = t;
        if t - self.morphed > 1.0 / MORPH_HZ {
            self.morphed = t;
            self.morph();
        }

        let (glow, pointer) = (self.glow, self.pointer);
        let bg = self.field.canvas(move |b| {
            [now(), pointer.0 - b.origin.x, pointer.1 - b.origin.y, glow, b.size.width]
        }).class("bg");

        // the pointer in design px, for the card glows
        let (dx, dy) = (pointer.0 / fit.scale - fit.ox, pointer.1 / fit.scale - fit.oy);
        let mut page = at(div(), fit.ox, fit.oy).css("width", rem(DESIGN.0)).css("height", rem(DESIGN.1))
            .child(at(div().class("nav"), 80.0, 34.0)
                .child(div().class("logo mono").child("kai").child(b().child(".")).child("vance"))
                .child(div().class("links")
                    .child(div().class("link").child("Work")).child(div().class("link").child("Writing"))
                    .child(div().class("link").child("Stack")).child(div().class("link").child("Contact"))))
            .child(at(div().class("hero"), 80.0, 120.0)
                .child(div().class("eyebrow mono").child("> AI FULLSTACK ENGINEER · PORTO · REMOTE"))
                .child(h1().child("I build agent systems that actually ship."))
                .child(p().child("Twelve years of product engineering. Now: agents, realtime sync and the boring infrastructure that keeps both alive at 3 a.m.")));
        for (i, (num, title, body)) in CARDS.iter().enumerate() {
            let x = 80.0 + i as f32 * (CARD_W + CARD_GAP);
            page = page.child(at(div().id(format!("card{i}")).class("card"), x, CARD_Y)
                .css("animation-delay", format!("{}ms", 100 + i * 90))
                .child(card_glow(dx - x, dy - CARD_Y))
                .child(div().class("num mono").child(*num))
                .child(h3().child(*title))
                .child(p().child(*body)));
        }
        let glyphs = self.glyphs;
        let reach = REACH * fit.scale;
        page = page.child(at(div().id("band").class("band"), BAND.0, BAND.1)
            .child(canvas(move |b, p| paint_field(b, p, &glyphs, pointer, reach)).class("field"))
            .child(div().class("cta")
                .child(div().child(h2().child("Have something hard? Let's build it.")).child(small().child("Two slots open for Q1 · replies within a day")))
                .child(div().id("cta").class("btn").child("Start a project").child(span().child("→")))));

        html().css("font-size", fit.root_font())
            .child(head().child(style().child(css())))
            .child(body()
                .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, _| {
                    this.pointer = (e.position.x.0, e.position.y.0);
                    this.inside = true;
                }))
                .on_mouse_leave(cx.listener(|this, _: &MouseMoveEvent, _, _| this.inside = false))
                .child(bg).child(page))
    }
}

fn field() -> Fx {
    Fx::new("ascii-field", FIELD, 5, [2, 6, 4])
}

/// The pointer over a card, across the band, then out of the window.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        s.play("00-enter", &[0, 200, 500, 1000]);
        let (x, y) = s.centre("card1");
        s.cx.simulate_mouse_move(s.win, x - 60.0, y - 30.0);
        s.play("01-card", &[0, 250]);
        let (x, y) = s.centre("band");
        s.cx.simulate_mouse_move(s.win, x - 200.0, y);
        s.play("02-band", &[0, 1200]);
        s.hover("cta");
        s.play("03-cta", &[0, 200]);
    })
}

fn main() {
    if let Some(dir) = arg("--frames") {
        let fx = field();
        let stills = [("rest", [6.0f32, 640.0, 400.0, 0.0, 1280.0]), ("glow-left", [6.0, 260.0, 300.0, 1.0, 1280.0]),
                      ("glow-right", [9.0, 1050.0, 620.0, 1.0, 1280.0]), ("glow-centre", [9.0, 640.0, 400.0, 1.0, 1280.0])];
        if gpu_stills(&dir, 1280, 800, stills, |g, f| g.draw(fx.effect(), &[], f)).is_none() {
            eprintln!("field shader: {:?}", fx.error());
        }
        return;
    }
    run_with(DESIGN.0, DESIGN.1, "BUI · ASCII Field", Ascii::new, arg("--storyboard").map(storyboard));
}

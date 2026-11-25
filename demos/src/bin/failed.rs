//! Windows Failed: a Y2K blue collage on BUI. A GPU shader paints the poster wall (ragged blue and
//! white panels, flowing ink, scanlines, grain). No UI: the first click on the wall pops a
//! cascade of "Windows Failed" dialogs, with a screen shake and a shader glitch; every later click
//! pops one more. Every ✕ and OK works: the warning closes, and another one opens somewhere else.
//! From the first click, kernel panics (a terminal, sticker captions, a star) pop up at random
//! every few seconds.
//!
//! Motion ("energetic"): pops of 180-280 ms with an overshoot, the burst staggered in under half a
//! second (windows 60 ms apart, then stickers 35 ms apart, then the star); hard shadows land 40 ms
//! after their windows; the replaced burst exits in 160 ms, accelerating; the ink, the spinning
//! star and the cursor keep the wall alive.
//!
//! `cargo run --release --bin failed`
//! `-- --storyboard DIR` clicks through it headlessly and saves frames along BUI's virtual clock;
//! `-- --frames DIR` renders the background shader off-screen on the GPU.

use std::time::Duration;

use blenderui::prelude::*;
use bui_demos::{arg, gpu_stills, now, run_with, save_png, Fx, Script};

/// U[0] = time, U[1] = seconds since the last click (< 0: none).
const WALL: &str = r"
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p); vec2 f = fract(p); f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}
float fbm(vec2 p) { return 0.5 * vnoise(p) + 0.25 * vnoise(p * 2.03 + 1.7) + 0.125 * vnoise(p * 4.1 + 9.2) + 0.0625 * vnoise(p * 8.3 + 3.1); }

vec4 effect(vec2 p) {
    float t = U[0]; float g = U[1];
    vec2 uv = p / box_size;
    // a click tears the wall: horizontal slices jump sideways for a quarter second, easing out
    if (g >= 0.0 && g < 0.28) {
        float k = 1.0 - g / 0.28;
        float band = floor(uv.y * 28.0);
        float r = hash(vec2(band, floor(t * 40.0)));
        uv.x += step(0.55, r) * (r - 0.55) * 0.5 * k * k;
    }
    // poster panels: ragged columns, some white, some blue, black seams
    float cx = uv.x * 6.0 + 0.35 * vnoise(vec2(uv.y * 2.0, 3.0));
    float col = floor(cx);
    float blueCol = step(0.5, hash(vec2(col, 7.0)));
    float seam = step(0.465, abs(fract(cx) - 0.5));
    float bar = step(0.985, fract(uv.y * 3.0 + hash(vec2(col, 1.0))));
    // ink: a slow, domain-warped flow, with a glossy streak inside it
    vec2 q = uv * vec2(box_size.x / box_size.y, 1.0) * 2.2;
    float w = fbm(q * 1.3 + vec2(t * 0.04, -t * 0.03));
    float n = fbm(q + 1.6 * w + vec2(0.0, t * 0.05) + col * 0.37);
    float ink = smoothstep(0.5, 0.53, n);
    float gloss = smoothstep(0.6, 0.63, n) * (1.0 - smoothstep(0.64, 0.7, n));
    vec3 white = vec3(0.95, 0.95, 0.97); vec3 blue = vec3(0.06, 0.06, 0.95); vec3 deep = vec3(0.01, 0.01, 0.3);
    vec3 c = blueCol < 0.5 ? mix(white, blue, ink) : mix(blue, deep, ink);
    c = mix(c, vec3(0.6, 0.65, 1.0), gloss * 0.85);
    c *= 1.0 - 0.95 * max(seam, bar);
    // xerox print: halftone in the blue, scanlines, grain, crushed contrast
    vec2 cell = fract(p / 5.0) - 0.5;
    c *= 1.0 - 0.18 * blueCol * step(length(cell), 0.25);
    c *= 0.92 + 0.08 * sin(p.y * 1.5708);
    c += (hash(p + fract(t * 7.0) * 50.0) - 0.5) * 0.09;
    c = clamp((c - 0.06) * 1.15, 0.0, 1.0);
    // the first frames of a click flash inverted
    if (g >= 0.0 && g < 0.05) c = vec3(1.0) - c;
    return vec4(c, 1.0);
}
";

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; position: relative; overflow: hidden; background: #1010f0; cursor: pointer; user-select: none;
       font-family: Inter, system-ui, sans-serif; -webkit-font-smoothing: antialiased; }
.wall { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
.type { position: absolute; color: #000; font-weight: 900; letter-spacing: -4px; line-height: 0.9; white-space: nowrap; }
.stage { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
.place { position: absolute; }

/* -- a Windows 9x dialog, as stuck on a poster: thick ink outline, hard offset shadow -- */
.win { position: absolute; width: 380px; }
.shade { position: absolute; left: 7px; top: 7px; width: 380px; height: 152px; background: #000;
         animation: shade 200ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.box { position: relative; height: 148px; border: 2px solid #000; background: #cdcdcd;
       box-shadow: inset 2px 2px 0 #fff, inset -2px -2px 0 #7b7b7b;
       animation: pop 260ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.title { display: flex; align-items: center; height: 34px; margin: 3px; padding: 0 6px 0 12px; background: #1414ff;
         color: #fff; font-family: Consolas, monospace; font-weight: 700; font-size: 21px; letter-spacing: 0.5px;
         animation: flicker 180ms steps(3) backwards; }
.title span { flex: 1; }
.x { position: relative; width: 22px; height: 22px; background: #fff; border: 2px solid #000; color: #000; font-size: 15px;
     line-height: 20px; text-align: center; font-family: Consolas, monospace; font-weight: 700; cursor: pointer;
     transition-property: background-color, color, transform; transition-duration: 80ms; transition-timing-function: ease-out; }
.x::after { content: ''; position: absolute; left: -9px; top: -9px; right: -9px; bottom: -9px; } /* a 40 px hit area */
.x:hover { background: #000; color: #fff; }
.x:active { transform: scale(0.96); }
/* a closed warning: quick, accelerating, a little shrink; it stays invisible until the next action */
.closing { animation: close 140ms cubic-bezier(0.3, 0, 1, 1) forwards; pointer-events: none; }
@keyframes close { to { opacity: 0; transform: scale(0.92); } }
.msg { padding: 18px 18px 0 22px; font-size: 22px; color: #111; letter-spacing: -0.3px; }
.ok { display: flex; align-items: center; justify-content: center; margin: 14px 18px 0 auto; width: 92px; height: 26px;
      background: #cdcdcd; color: #111; font-size: 14px; cursor: pointer;
      box-shadow: inset 2px 2px 0 #fff, inset -2px -2px 0 #7b7b7b, 0 0 0 1px #000; }
.ok:active { box-shadow: inset -2px -2px 0 #fff, inset 2px 2px 0 #7b7b7b, 0 0 0 1px #000; padding-top: 2px; }

/* -- sticker captions and stars -- */
.sticker { padding: 7px 16px 9px; background: #1414ff; color: #fff; font-size: 21px; letter-spacing: 0.2px;
           border: 2px solid #fff; box-shadow: 0 0 0 2px #000, 4px 4px 0 2px #000; white-space: nowrap;
           animation: snap 200ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.spin { animation: spin 14s linear infinite; }
.star { width: 70px; height: 70px; background: #1414ff;
        clip-path: polygon(50% 0%, 61% 35%, 98% 35%, 68% 57%, 79% 91%, 50% 70%, 21% 91%, 32% 57%, 2% 35%, 39% 35%);
        animation: star 280ms cubic-bezier(0.2, 0, 0, 1) backwards; }

/* -- the kernel panic billboard -- */
.term { width: 440px; padding: 16px 18px; background: #050505;
        border: 6px solid #f2f2f2; box-shadow: 0 0 0 2px #000, 8px 8px 0 2px #000;
        color: #eaeaea; font-family: Consolas, monospace; font-size: 10.5px; line-height: 1.45;
        animation: crt 320ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.term div { animation: line 90ms ease-out backwards; white-space: nowrap; overflow: hidden; }
.cursor { display: inline-block; width: 7px; height: 12px; background: #eaeaea; animation: blink 1s steps(2) infinite; }

/* -- motion. Entrances decelerate; the overshoot is in the keyframes, so every pop lands the same.
   `backwards` fill: a finished pop drops its transform and is a plain box again (no layer) -- */
@keyframes pop { 0% { opacity: 0; transform: translate(-16px, 22px) scale(0.5); }
                 55% { opacity: 1; transform: translate(2px, -3px) scale(1.07); }
                 78% { transform: translate(0, 0) scale(0.98); }
                 100% { opacity: 1; transform: translate(0, 0) scale(1); } }
@keyframes shade { 0% { opacity: 0; transform: translate(-7px, -7px); } 100% { opacity: 1; transform: translate(0, 0); } }
@keyframes flicker { 0% { opacity: 0; } 50% { opacity: 1; } 75% { opacity: 0.35; } 100% { opacity: 1; } }
@keyframes snap { 0% { opacity: 0; transform: scale(0.3) rotate(-14deg); }
                  60% { opacity: 1; transform: scale(1.12) rotate(3deg); }
                  100% { opacity: 1; transform: scale(1) rotate(0deg); } }
@keyframes star { 0% { opacity: 0; transform: scale(0) rotate(-120deg); }
                  65% { opacity: 1; transform: scale(1.2) rotate(10deg); }
                  100% { opacity: 1; transform: scale(1) rotate(0deg); } }
@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
@keyframes crt { 0% { opacity: 0; transform: scale(1.15, 0.02); }
                 45% { opacity: 1; transform: scale(1.02, 0.06); }
                 100% { opacity: 1; transform: scale(1, 1); } }
@keyframes line { from { opacity: 0; transform: translateX(-6px); } to { opacity: 1; transform: translateX(0); } }
@keyframes blink { from { opacity: 1; } to { opacity: 0; } }
/* two names for one shake, alternated per click, so each click restarts it */
@keyframes shakeA { 0% { transform: translate(0, 0); } 20% { transform: translate(-7px, 3px); } 45% { transform: translate(6px, -2px); }
                    70% { transform: translate(-3px, 1px); } 100% { transform: translate(0, 0); } }
@keyframes shakeB { 0% { transform: translate(0, 0); } 20% { transform: translate(-7px, 3px); } 45% { transform: translate(6px, -2px); }
                    70% { transform: translate(-3px, 1px); } 100% { transform: translate(0, 0); } }
.shake0 { animation: shakeA 320ms ease-in-out; } .shake1 { animation: shakeB 320ms ease-in-out; }
/* the panic a new one replaces: out faster than it came in, accelerating, falling a little */
.out { animation: out 160ms cubic-bezier(0.3, 0, 1, 1) forwards; }
@keyframes out { to { opacity: 0; transform: translateY(12px) scale(0.97); } }
";

const MESSAGES: [&str; 5] = ["Wake. Up. You.", "Still loading you.", "Fatal: too much.", "Retry? Never.", "Not responding."];
const STICKERS: [&str; 10] = ["wake up.", "ctrl+z yea", "re==L00d", "// ignore", "overheat.", "한숨 (ah!)",
                              "404 feelings", "sudo stay", "brb rebooting", "not responding"];
const TILTS: [f32; 6] = [-6.0, 4.0, -3.0, 7.0, -8.0, 5.0];
const KERNEL: [&str; 12] = [
    "BUG: unable to handle kernel paging request at ffff88014b4a0000",
    "IP: __memmove+0x24/0x1a0",
    "PGD 21f4067 P4D 21f4067 PUD 21f7067",
    "PMD 10059b063 PTE 800000014b4a0161",
    "Oops: 0003 [#1] SMP",
    "Modules linked in: coretemp kvm_intel kvm irqbypass snd_hda_intel",
    "  crct10dif_pclmul crc32_pclmul ghash_clmulni_intel iTCO_wdt",
    "CPU: 2 PID: 494 Comm: loadkeys Not tainted 4.11.8-1-ARCH",
    "task: ffff880151149600 task.stack: ffffc90000c3c000",
    "RIP: 0010:__memmove+0x24/0x1a0",
    "RSP: 0018:ffffc90000c3fe18 EFLAGS: 00010282",
    "Kernel panic - not syncing: Fatal exception",
];
const WINDOWS: usize = 5;

/// Places an absolutely positioned element.
fn at(d: Div, x: f32, y: f32) -> Div {
    d.css("left", format!("{x}px")).css("top", format!("{y}px"))
}

/// Starts an element's entrance `ms` after it appears (each element keeps its own clock).
fn after(d: Div, ms: usize) -> Div {
    d.css("animation-delay", format!("{ms}ms"))
}

/// A cheap, deterministic scatter in 0..1 (the golden-ratio sequence), so nothing repeats a spot.
fn scatter(k: u32, phase: f32) -> f32 {
    (k as f32 * 0.618_034 + phase).fract()
}

/// One warning dialog. `id` keys its elements, so each keeps its own animation clock.
struct Warning {
    id: u32,
    x: f32,
    y: f32,
    /// Entrance delay (a cascade staggers its windows).
    delay: usize,
    /// Whether the body shows a message (a cascade's back windows show only their title bars).
    message: bool,
    /// Closed: playing its exit, gone at the next action.
    closing: bool,
}

impl Warning {
    fn view(&self, cx: &mut Context<Failed>) -> Div {
        let (id, delay) = (self.id, self.delay);
        // ✕ and OK both work... and open another warning. They stop the click there, or it would
        // reach the wall and start a cascade too.
        let dismiss = |d: Div| d.attr("role", "button").on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            this.close(id, cx);
        }));
        let mut b = after(div().class("box"), delay)
            .child(after(div().class("title"), delay)
                .child(span().child("Windows Failed"))
                .child(dismiss(div().id(format!("x{id}")).class("x").attr("aria-label", "Close").child("✕"))));
        if self.message {
            b = b.child(div().class("msg").child(MESSAGES[id as usize % MESSAGES.len()]))
                .child(dismiss(div().id(format!("ok{id}")).class("ok").child("OK")));
        }
        // a z-index per warning, rising: BUI then paints each warning's subtree (its text too) as one
        // run, in order. Without it a lower warning's text drew over the boxes of the ones above it
        at(div().id(format!("w{id}")).class(if self.closing { "win closing" } else { "win" }), self.x, self.y)
            .css("z-index", format!("{}", id + 1))
            .child(after(div().class("shade"), delay + 40))
            .child(b)
    }
}

/// A kernel panic: the terminal, sticker captions over it and a star, popping up at a random
/// place and tilt from time to time. `n` keys its elements and seeds it.
struct Panic {
    n: u32,
    x: f32,
    y: f32,
    tilt: f32,
}

impl Panic {
    fn new(n: u32) -> Self {
        Self { n, x: 20.0 + scatter(n, 0.2) * 560.0, y: 40.0 + scatter(n * 7, 0.65) * 470.0, tilt: -8.0 + scatter(n * 3, 0.4) * 13.0 }
    }

    /// `leaving`: the panic a new one replaced, on its way out (invisible until the next).
    fn view(&self, leaving: bool) -> Div {
        let (n, x, y) = (self.n, self.x, self.y);
        let mut term = div().class("term");
        for (i, l) in KERNEL.iter().enumerate() {
            term = term.child(after(div(), 200 + i * 22).child(*l));
        }
        term = term.child(after(div(), 200 + KERNEL.len() * 22).child(span().class("cursor")));
        let mut out = div().id(format!("p{n}")).class(if leaving { "stage out" } else { "stage" })
            .child(at(div().id(format!("p{n}t")).class("place"), x, y).css("transform", format!("rotate({}deg)", self.tilt))
                .child(term));
        // captions land on the terminal once it is on, then the star on its corner
        let spots = [(250.0, 150.0), (420.0, 160.0), (300.0, 222.0)];
        for (k, (dx, dy)) in spots.into_iter().enumerate() {
            let s = n as usize * 3 + k;
            let tilt = TILTS[s % TILTS.len()];
            out = out.child(at(div().id(format!("p{n}s{k}")).class("place"), x + dx, y + dy)
                .css("transform", format!("rotate({tilt}deg)"))
                .child(after(div().class("sticker"), 420 + k * 35).child(STICKERS[s % STICKERS.len()])));
        }
        out.child(at(div().id(format!("p{n}star")).class("place"), x + 230.0, y - 30.0)
            .child(div().class("spin").child(after(div().class("star"), 560))))
    }
}

struct Failed {
    wall: Fx,
    /// In paint order: the newest on top.
    warnings: Vec<Warning>,
    panic: Option<Panic>,
    leaving: Option<Panic>,
    /// When the next panic pops up (`now()`); none until the first click.
    next_panic: Option<f32>,
    /// Cascades opened (each one's position comes from it).
    cascades: u32,
    /// Warnings opened, ever: the next warning's id.
    opened: u32,
    clicked: Option<f32>,
}

impl Failed {
    /// Every action starts here: what was closing by now has finished its exit, and goes.
    fn act(&mut self) {
        self.warnings.retain(|w| !w.closing);
        self.clicked = Some(now());
    }

    fn open(&mut self, x: f32, y: f32, delay: usize, message: bool) {
        self.warnings.push(Warning { id: self.opened, x, y, delay, message, closing: false });
        self.opened += 1;
    }

    /// A panic replaces the last one; the next comes 2.5 to 6 s later.
    fn panic(&mut self) {
        let n = self.panic.as_ref().map_or(1, |p| p.n + 1);
        self.leaving = self.panic.replace(Panic::new(n));
        self.next_panic = Some(now() + 2.5 + 3.5 * scatter(n, 0.9));
    }

    /// A click on the wall. The first is a cascade of five, and starts the panics; every later one
    /// is one more warning, somewhere else.
    fn wall_click(&mut self, cx: &mut Context<Self>) {
        if self.cascades > 0 {
            self.act();
            self.spawn();
            cx.notify();
            return;
        }
        self.act();
        self.cascades += 1;
        let k = self.cascades;
        let (x, y) = (150.0 + scatter(k, 0.0) * 300.0, 80.0 + scatter(k, 0.3) * 170.0);
        for i in 0..WINDOWS {
            self.open(x + i as f32 * 24.0, y + i as f32 * 30.0, i * 60, i + 1 == WINDOWS);
        }
        if self.next_panic.is_none() {
            self.panic();
        }
        cx.notify();
    }

    /// ✕ or OK: that warning closes, and another one opens somewhere else.
    fn close(&mut self, id: u32, cx: &mut Context<Self>) {
        self.act();
        if let Some(w) = self.warnings.iter_mut().find(|w| w.id == id) {
            w.closing = true;
        }
        self.spawn();
        cx.notify();
    }

    /// One warning, popping up at a spot the last ones didn't use.
    fn spawn(&mut self) {
        let k = self.opened;
        self.open(60.0 + scatter(k, 0.11) * 560.0, 40.0 + scatter(k * 3, 0.47) * 420.0, 0, true);
    }
}

impl Render for Failed {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        if self.next_panic.is_some_and(|t| now() >= t) {
            self.panic();
        }
        let clicked = self.clicked;
        let wall = self.wall.canvas(move |_| {
            let t = now();
            [t, clicked.map_or(-1.0, |c| t - c)]
        }).class("wall");
        let words = [("IOI", 860.0, 30.0, 150.0, 0.0), ("DaLE", 880.0, 236.0, 92.0, 4.0), ("NEVATI", 690.0, 410.0, 118.0, -3.0),
                     ("UPLINK", 705.0, 535.0, 96.0, 2.0), ("ERRO", 760.0, 650.0, 130.0, -1.0)];
        // the shake restarts on every action: its two identical keyframes alternate
        let mut stage = div().class(if self.opened == 0 { "stage".to_string() } else { format!("stage shake{}", self.opened % 2) });
        for (word, x, y, size, tilt) in words {
            stage = stage.child(at(div().class("type"), x, y)
                .css("font-size", format!("{size}px")).css("transform", format!("rotate({tilt}deg)")).child(word));
        }
        if let Some(p) = &self.leaving {
            stage = stage.child(p.view(true));
        }
        if let Some(p) = &self.panic {
            stage = stage.child(p.view(false));
        }
        for warning in &self.warnings {
            stage = stage.child(warning.view(cx));
        }
        // no UI: a click anywhere on the wall starts it (and starts another cascade)
        html()
            .child(head().child(style().child(CSS)))
            .child(body().id("wall").child(wall).child(stage)
                .on_click(cx.listener(|this, _, _, cx| this.wall_click(cx))))
    }
}

/// Clicks through the demo in a headless window and saves frames along the virtual clock: a
/// cascade (and the first panic), closing warnings with ✕ and OK (each opens another), then
/// more clicks on the wall (one warning each).
fn storyboard(dir: String) -> Script {
    Box::new(move |cx, win| {
        let shot = |cx: &mut App, name: &str| {
            let (w, h, px) = cx.draw(win);
            save_png(&format!("{dir}/{name}.png"), w, h, &px);
        };
        let click = |cx: &mut App, id: &str| {
            let b = cx.element_bounds(win, id).unwrap_or_else(|| panic!("no #{id}"));
            cx.simulate_click(win, b.origin.x + b.size.width / 2.0, b.origin.y + b.size.height / 2.0);
        };
        let play = |cx: &mut App, name: &str, steps: &[u64]| {
            let mut at = 0;
            for &ms in steps {
                cx.advance_clock(Duration::from_millis(ms - at));
                at = ms;
                shot(cx, &format!("{name}-{ms:04}ms"));
            }
        };
        let wall = |cx: &mut App| cx.simulate_click(win, 1060.0, 790.0);
        shot(cx, "00-idle");
        wall(cx);
        play(cx, "01-cascade", &[0, 40, 90, 150, 220, 300, 380, 460, 600, 1000]);
        // close the front warning (id 4) with ✕: it goes, warning 5 pops up elsewhere
        click(cx, "x4");
        play(cx, "02-close", &[0, 60, 120, 200, 400]);
        // OK works the same, and neither starts a cascade
        for id in 5..=8 {
            click(cx, &format!("{}{id}", if id % 2 == 1 { "ok" } else { "x" }));
            cx.advance_clock(Duration::from_millis(500));
        }
        shot(cx, "03-after-closing-5");
        // later clicks on the wall: one warning each, no cascade
        wall(cx);
        play(cx, "04-wall-again", &[0, 80, 400]);
        wall(cx);
        wall(cx);
        cx.advance_clock(Duration::from_millis(500));
        shot(cx, "05-wall-three-times");
    })
}

fn wall() -> Fx {
    Fx::new("wall", WALL, 2, [16, 16, 240])
}

fn main() {
    if let Some(dir) = arg("--frames") {
        let fx = wall();
        let stills = [("wall", [3.0f32, -1.0]), ("wall-glitch", [3.1, 0.12])];
        if gpu_stills(&dir, 1100, 800, stills, |g, f| g.draw(fx.effect(), &[], f)).is_none() {
            eprintln!("wall shader: {:?}", fx.error());
        }
        return;
    }
    let make = || Failed { wall: wall(), warnings: Vec::new(), panic: None, leaving: None, next_panic: None, cascades: 0, opened: 0, clicked: None };
    run_with(1100.0, 800.0, "BUI · Windows Failed", make, arg("--storyboard").map(storyboard));
}

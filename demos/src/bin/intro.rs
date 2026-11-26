//! Intro: a designer's portfolio that opens as a terminal, then a glass carousel of their
//! products further down the page, ported from a portfolio site's opening and projects section.
//!
//! The intro: a liquid-metal frame (a GPU shader: chrome bands flowing under a slow warp, a
//! little chromatic split) fades in out of a blur on a dark cover; inside it a terminal prints
//! its prompt, types `npx start @maraoki` a key at a time (45 to 115 ms apart) and offers
//! "Press return ⏎ to run". Return (or a click) runs it: the install transcript prints line by
//! line, the text blurs away, the cover fades, and the hero (name, copy, buttons, stats that count
//! up) rises in. Escape skips it.
//!
//! The projects: scrolling the page (wheel, or drag it) brings in the section's pill, a small
//! metal icon and the carousel: rounded glass panels in a loop, the active one centred and large,
//! its neighbours smaller, fainter and blurred by distance. It follows a drag under the finger and
//! springs to the nearest panel on release (a swipe past 40 px steps), steps with ← / →, Shift +
//! wheel and the buttons, and advances by itself every 3.5 s unless the pointer is on it. The
//! caption swaps with a 300 ms fade-up, title then summary 40 ms apart, and counts "3 / 10".
//!
//! Motion: GSAP's curves from the reference (`expo.out` entrances, `power2.in` exits, `power4.out`
//! rise); the carousel position is one spring `MotionValue`, so every step interrupts cleanly; the
//! page scroll chases its target with an exponential follow (smooth, frame-rate independent).
//!
//! `cargo run --release --bin intro`
//! `-- --storyboard DIR` drives it headlessly (typing, Return, scroll, keys, a drag, the auto
//! advance) and saves frames along BUI's virtual clock; `-- --frames DIR` renders the metal.

use std::sync::OnceLock;

use blenderui::motion::MotionValue;
use blenderui::prelude::*;
use blenderui::vars;
use bui_demos::canvas::{rem, rem_css, Fit};
use bui_demos::ease::{approach, ease_out, expo_out, progress};
use bui_demos::{arg, focus_root, gpu_stills, now, run_with, story::story, Fx, Script};

/// U[0] time, U[1] appear 0..1 (scale .94 -> 1, blur -> sharp, fade), U[2] corner radius and
/// U[3] the box's width, both logical px.
const METAL: &str = r"
float chrome(float x) {
    float f = fract(x);
    // the reference's chrome ramp: bright, mid, bright, dark, bright, mid, bright
    return 0.72 + 0.25 * cos(6.2831 * 3.0 * f + 0.35 * sin(6.2831 * f)) + 0.05 * cos(6.2831 * f);
}
vec4 effect(vec2 p) {
    float s = box_size.x / max(U[3], 1.0);
    float a = clamp(U[1], 0.0, 1.0);
    float sc = mix(0.94, 1.0, a);
    vec2 half = box_size * 0.5 * sc;
    float r = U[2] * s * sc;
    vec2 d = abs(p - box_size * 0.5) - half + r;
    float sd = length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - r;
    float mask = clamp(0.5 - sd, 0.0, 1.0);
    vec2 c = (p - box_size * 0.5) / (box_size.y * sc);
    float t = U[0];
    vec2 w = c;
    w += 0.13 * vec2(sin(c.y * 3.1 + t * 0.55), cos(c.x * 2.7 - t * 0.45));
    w += 0.05 * vec2(sin(c.y * 7.3 - t * 0.9 + c.x * 2.0), cos(c.x * 6.1 + t * 0.7 - c.y));
    float stripe = dot(w, vec2(0.342, 0.94)) * 2.2 + 0.35 * length(w) + 0.05 * t;
    float blur = 1.0 - a;
    vec3 col = vec3(chrome(stripe + 0.035), chrome(stripe), chrome(stripe - 0.035));
    col = mix(col, vec3(0.8), blur * 0.85);
    col += pow(max(col.g - 0.88, 0.0) * 8.0, 2.0) * 0.12;
    col *= vec3(0.97, 0.98, 1.0) * (0.78 + 0.22 * (1.0 - length(c) * 1.1));
    float alpha = mask * a;
    return vec4(col * alpha, alpha);
}
";

const DESIGN: (f32, f32) = (1280.0, 800.0);
/// The page's height (design px): the hero, then the projects.
const PAGE_H: f32 = 1660.0;
/// The hero's metal frame (design px).
const FRAME: (f32, f32, f32, f32) = (80.0, 110.0, 600.0, 560.0);
const FRAME_R: f32 = 28.0;
const CMD: &str = "npx start @maraoki";
/// The transcript: (mark, label, text). An empty mark is a plain dim line.
const LINES: [(&str, &str, &str); 9] = [
    ("", "", "Need to install the following packages:"),
    ("", "", "  @maraoki/start@2026.11.26"),
    ("", "", "Ok to proceed? (y) y"),
    ("", "", " "),
    ("✔", "designer", "Northwind · Kestrel Labs · Halcyon"),
    ("✔", "shipped", "Murmur · Tidepool · Lumen · Glyphfall"),
    ("✔", "skills", "motion-kit · shader-lab · type-tools +4"),
    ("✔", "status", "open for app builds & consulting"),
    ("➜", "ready", "maraoki.design"),
];
/// One product in the carousel.
struct Project {
    name: &'static str,
    /// Live (else still building).
    launched: bool,
    platforms: &'static str,
    summary: &'static str,
    /// The icon: its gradient and the mark on it.
    colors: (&'static str, &'static str),
    mark: &'static str,
}

const fn project(name: &'static str, launched: bool, platforms: &'static str, summary: &'static str,
                 colors: (&'static str, &'static str), mark: &'static str) -> Project {
    Project { name, launched, platforms, summary, colors, mark }
}

const PROJECTS: [Project; 10] = [
    project("Murmur", true, "Mac, Windows, iPhone", "Talk, and clean text lands in whatever app you're in.", ("#ff7a59", "#ff2d6f"), "M"),
    project("Tidepool", true, "iPhone, Apple Watch", "A calm sleep and feeding log for new parents.", ("#5ee0d0", "#2f7cf6"), "T"),
    project("Lumen", true, "Web, iPhone", "A portfolio network for people who make things with AI.", ("#ffd84d", "#ff8a00"), "L"),
    project("Ledgerly", false, "Web, self-hosted", "Open-source invoicing that matches the paid tools line for line.", ("#9be15d", "#00b386"), "£"),
    project("Orbit", false, "Mac", "A launcher that remembers what you were doing an hour ago.", ("#a18cff", "#5b3df5"), "O"),
    project("Reel", false, "Mac (Apple Silicon)", "A screen recorder that makes product demos look edited.", ("#ff6b6b", "#7a1fa2"), "R"),
    project("Remote", false, "iPhone, Mac", "Your phone as a trackpad, keyboard and presenter clicker.", ("#3a3d46", "#0d0f14"), "◐"),
    project("Glyphfall", true, "Browser game", "A falling-letters word game you can finish on a coffee break.", ("#00d2ff", "#3a47d5"), "G"),
    project("Folio", false, "Web, CLI", "Turn a folder of notes into a printed zine.", ("#f7f2e8", "#d9cbb0"), "F"),
    project("Common Ground", true, "White paper", "An open protocol for sharing design tokens between tools.", ("#43e97b", "#38a1f7"), "C"),
];
/// The carousel: panel size, distance between neighbours, and where its row sits (design px).
const PANEL: f32 = 300.0;
const STEP: f32 = 300.0;
const GAP: f32 = 30.0;
const ROW_Y: f32 = 1060.0;
/// Seconds between auto-advances, and how far a swipe goes before it steps.
const ADVANCE: f32 = 3.5;
const SWIPE: f32 = 40.0;
/// Stats: label, value, prefix, suffix, zero-padded to two.
const STATS: [(&str, u32, &str, &str, bool); 4] = [
    ("Years designing products", 12, "", "+", false),
    ("Live projects", 5, "", "", true),
    ("Talks given", 14, "", "", true),
    ("Murmur on launch day", 3, "#", "", false),
];

const CSS: &str = r"
html { height: 100%; background: #0b0b0c; }
body { margin: 0; height: 100%; position: relative; overflow: hidden; background: #f3f3f1; color: #111214; user-select: none;
       font-family: Inter, 'Segoe UI', system-ui, sans-serif; -webkit-font-smoothing: antialiased; }
.abs { position: absolute; }
.mono { font-family: 'JetBrains Mono', Consolas, monospace; }
.page { position: absolute; left: 0; top: 0; width: 1280px; height: 1660px; }

/* -- the header -- */
.headbar { left: 0; top: 0; width: 100%; height: 88px; background: rgba(243,243,241,0.82); backdrop-filter: blur(14px);
           box-shadow: 0 1px 0 rgba(0,0,0,0.06); }
.head { left: 80px; top: 28px; width: 1120px; height: 44px; display: flex; align-items: center; justify-content: space-between; }
.brand { font-size: 15px; font-weight: 700; letter-spacing: -0.2px; }
.nav { display: flex; gap: 4px; }
.nav div { height: 40px; padding: 0 14px; display: flex; align-items: center; font-size: 14px; color: #6b6d73; border-radius: 10px;
           cursor: pointer; transition-property: color, background-color; transition-duration: 150ms; }
.nav div:hover { color: #111214; background: rgba(0,0,0,0.05); }

/* -- the hero -- */
.metal { position: absolute; }
.copy { left: 740px; top: 132px; width: 460px; }
.face { width: 64px; height: 64px; border-radius: 32px; background: linear-gradient(140deg, #ffb48a 0%, #ff5f7a 55%, #6f4cff 100%);
        display: flex; align-items: center; justify-content: center; color: #fff; font-weight: 700; font-size: 20px;
        box-shadow: 0 0 0 1px rgba(0,0,0,0.1), 0 10px 24px rgba(255,95,122,0.35); }
.copy h1 { margin: 22px 0 0; font-size: 46px; line-height: 1.04; font-weight: 700; letter-spacing: -1.6px; text-wrap: balance; }
.copy p { margin: 16px 0 0; font-size: 16px; line-height: 1.55; color: #6b6d73; text-wrap: pretty; }
.ctas { margin-top: 26px; display: flex; align-items: center; gap: 22px; }
.btn { height: 44px; padding: 0 20px; border-radius: 12px; display: flex; align-items: center; font-size: 14px; font-weight: 600; cursor: pointer;
       background: linear-gradient(155deg, #fbfbfc 0%, #b9bcc4 18%, #f4f5f7 34%, #858892 52%, #e6e7eb 68%, #a3a6ae 84%, #f7f7f9 100%);
       box-shadow: 0 0 0 1px rgba(0,0,0,0.12), 0 6px 18px rgba(0,0,0,0.12), inset 0 1px 0 rgba(255,255,255,0.8);
       transition-property: transform, box-shadow; transition-duration: 150ms; }
.btn:hover { box-shadow: 0 0 0 1px rgba(0,0,0,0.16), 0 10px 26px rgba(0,0,0,0.18), inset 0 1px 0 rgba(255,255,255,0.8); }
.btn:active { transform: scale(0.96); }
.more { height: 44px; display: flex; align-items: center; font-size: 14px; color: #6b6d73; cursor: pointer;
        transition-property: color; transition-duration: 150ms; }
.more:hover { color: #111214; }
.stats { left: 740px; top: 520px; width: 460px; }
.stat { display: flex; justify-content: space-between; padding: 7px 0; font-size: 14px; border-bottom: 1px solid rgba(0,0,0,0.07); }
.stat span { color: #6b6d73; }
.stat b { font-weight: 600; font-variant-numeric: tabular-nums; }
.pre { opacity: 0; }
.rise { animation: rise 900ms cubic-bezier(0.25, 1, 0.5, 1) backwards; }

/* -- the intro: a dark cover, the terminal inside the metal -- */
.cover { left: 0; top: 0; width: 100%; height: 100%; background: #0b0b0c; }
.cover.out { animation: fade 900ms cubic-bezier(0.5, 0, 0.75, 0) 200ms forwards; }
.term { color: #fff; mix-blend-mode: difference; font-size: 14px; line-height: 1.7; white-space: pre-wrap; }
.term.out { animation: blurout 280ms cubic-bezier(0.5, 0, 0.75, 0) forwards; }
.term.in { animation: termin 900ms cubic-bezier(0.16, 1, 0.3, 1) 500ms backwards; }
.dots { display: flex; gap: 8px; margin-bottom: 16px; }
.dots i { width: 8px; height: 8px; border-radius: 4px; background: rgba(255,255,255,0.4); }
.dim { opacity: 0.6; }
.faint { opacity: 0.5; }
.caret { display: inline-block; width: 8px; height: 16px; margin-left: 1px; background: #fff; vertical-align: -3px;
         animation: caret 1s steps(1) infinite; }
.lab { display: inline-block; width: 84px; opacity: 0.6; }
.hint { height: 48px; border-radius: 13px; padding: 0 8px 0 16px; box-sizing: border-box; display: flex; align-items: center;
        justify-content: space-between; background: rgba(255,255,255,0.82); backdrop-filter: blur(12px); font-size: 13px; color: #6b6d73;
        animation: hintin 1100ms cubic-bezier(0.16, 1, 0.3, 1) 150ms backwards; }
.hint.out { animation: blurout 250ms cubic-bezier(0.5, 0, 0.75, 0) 40ms forwards; }
.hint kbd { padding: 3px 8px; border-radius: 6px; background: #e8e8e6; color: #111214; font-family: 'JetBrains Mono', Consolas, monospace; font-size: 12px; }
.run { height: 34px; padding: 0 14px; border-radius: 8px; display: flex; align-items: center; gap: 8px; background: #111214; color: #fff;
       font-size: 13px; font-weight: 600; cursor: pointer; animation: breathe 1300ms ease-in-out 1350ms infinite alternate; }
.run:active { transform: scale(0.96); }
.skip { right: 24px; bottom: 20px; height: 40px; padding: 0 14px; display: flex; align-items: center; font-size: 12px; color: rgba(255,255,255,0.45);
        cursor: pointer; transition-property: color; transition-duration: 150ms; }
.skip:hover { color: rgba(255,255,255,0.85); }

/* -- the projects -- */
.pill { display: flex; align-items: center; gap: 8px; font-size: 14px; color: #3b3d42; }
.pill b { padding: 3px 8px; border-radius: 12px; background: #e4e4e1; font-weight: 500; font-variant-numeric: tabular-nums; }
.lead { font-size: 34px; font-weight: 700; letter-spacing: -1px; text-align: center; text-wrap: balance; }
.sub { font-size: 15px; color: #6b6d73; text-align: center; }
.row { position: relative; width: 1280px; height: 380px; }
/* every layer rounds itself: cheaper than a rounded `overflow: hidden` cutting what moving panels hold, each frame */
.panel { position: absolute; width: 300px; height: 300px; border-radius: 68px; cursor: pointer;
         box-shadow: 0 30px 60px rgba(20,20,30,0.16), 0 0 0 1px rgba(0,0,0,0.06); }
.art { position: absolute; left: 0; top: 0; width: 300px; height: 300px; border-radius: 68px; display: flex; align-items: center; justify-content: center; }
.mark { width: 150px; height: 150px; border-radius: 44px; display: flex; align-items: center; justify-content: center;
        font-size: 84px; font-weight: 800; color: #fff; letter-spacing: -3px;
        background: rgba(255,255,255,0.16); box-shadow: inset 0 1px 0 rgba(255,255,255,0.55), 0 12px 30px rgba(0,0,0,0.18); }
.sheen { position: absolute; left: 0; top: 0; width: 300px; height: 300px; border-radius: 68px;
         background: linear-gradient(170deg, rgba(255,255,255,0.5) 0%, rgba(255,255,255,0) 55%); }
.rim { position: absolute; left: 0; top: 0; width: 300px; height: 300px; border-radius: 68px;
       box-shadow: inset 0 1px 1px rgba(255,255,255,0.7), inset 0 -10px 30px rgba(0,0,0,0.18); }
.veil { position: absolute; left: 0; top: 0; width: 300px; height: 300px; border-radius: 68px; background: #f3f3f1; }
.caption { width: 800px; text-align: center; }
.caption h3 { margin: 0; font-size: 26px; font-weight: 700; letter-spacing: -0.6px; }
.caption p { margin: 8px auto 0; width: 720px; font-size: 15px; line-height: 1.5; color: #6b6d73; text-wrap: balance; }
.badge { display: inline-block; margin-left: 10px; vertical-align: 4px; padding: 3px 9px; border-radius: 10px; background: #fff1c7; color: #8a5a00; font-size: 12px; font-weight: 600; letter-spacing: 0; }
.ctl { width: 400px; display: flex; align-items: center; justify-content: center; gap: 18px; }
.arrow { width: 44px; height: 44px; border-radius: 22px; display: flex; align-items: center; justify-content: center; cursor: pointer;
         background: #fff; font-size: 18px; color: #111214; box-shadow: 0 0 0 1px rgba(0,0,0,0.08), 0 4px 12px rgba(0,0,0,0.06);
         transition-property: transform, box-shadow; transition-duration: 150ms; }
.arrow:hover { box-shadow: 0 0 0 1px rgba(0,0,0,0.14), 0 8px 20px rgba(0,0,0,0.1); }
.arrow:active { transform: scale(0.96); }
.count { width: 70px; text-align: center; font-size: 14px; color: #6b6d73; font-variant-numeric: tabular-nums; }

@keyframes rise { from { opacity: 0; transform: translateY(14px); } to { opacity: 1; transform: translateY(0); } }
@keyframes termin { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
@keyframes hintin { from { opacity: 0; transform: translateY(12px); filter: blur(6px); } to { opacity: 1; transform: translateY(0); filter: blur(0px); } }
@keyframes blurout { to { opacity: 0; transform: translateY(-6px); filter: blur(6px); } }
@keyframes fade { to { opacity: 0; } }
@keyframes caret { 50% { opacity: 0; } }
@keyframes breathe { from { transform: scale(1); } to { transform: scale(1.04); } }
";

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

/// Where the intro is, each phase with when it began (seconds on [`now`]).
#[derive(Clone, Copy, PartialEq)]
enum Intro {
    Typing,
    Ready(f32),
    Running(f32),
    Leaving(f32),
    Done(f32),
}

/// When each line of the transcript prints, seconds into the run (the reference's schedule).
fn line_at(n: usize) -> f32 {
    if n < 4 { n as f32 * 0.12 } else { 0.6 + (n - 4) as f32 * 0.22 }
}

/// Seconds after the start at which the command has `k` characters typed: from 1 s, 45 to 115 ms
/// a key (a fixed pseudo-random jitter, so every run types alike).
fn typed_at(k: usize) -> f32 {
    1.0 + (0..k).map(|i| 0.045 + 0.07 * ((i as f32 * 12.9898).sin() * 43_758.547).fract().abs()).sum::<f32>()
}

/// A press that may become a drag: the pointer (along the drag) and the value at the press. It
/// takes the pointer only once it has moved past `DRAG_THRESHOLD`, so a click on a button or a
/// panel under it still reaches that.
#[derive(Clone, Copy)]
struct Grab {
    from: f32,
    start: f32,
    held: bool,
}

impl Grab {
    const fn new(from: f32, start: f32) -> Self {
        Self { from, start, held: false }
    }

    /// The pointer moved to `at`: how far from the press (design px, `k` window px each), taking
    /// the pointer the first time it passes the threshold. `None` until then.
    fn moved(&mut self, at: f32, k: f32, w: &mut Window, cx: &mut App) -> Option<f32> {
        let d = (at - self.from) / k;
        if !self.held && d.abs() * k > DRAG_THRESHOLD {
            self.held = true;
            w.set_pointer_capture(cx);
        }
        self.held.then_some(d)
    }
}

/// The run lasts until the last line, then holds 0.6 s; the exit takes 1.1 s.
const RUN: f32 = 0.6 + 4.0 * 0.22 + 0.6;
const LEAVE: f32 = 1.1;

struct Portfolio {
    metal: Fx,
    started: f32,
    intro: Intro,
    /// The page scroll (design px): where it is heading, and where it is.
    scroll: f32,
    shown: f32,
    last: f32,
    /// A page drag: the pointer y and the target at the press; the last move (y, time) for a fling.
    page_grab: Option<Grab>,
    page_move: (f32, f32, f32),
    /// The carousel: its position in panels (a spring), a drag (pointer x, position at the press),
    /// whether the pointer is over it, the last step ([`now`]) and how many steps so far.
    pos: MotionValue,
    grab: Option<Grab>,
    hover: bool,
    stepped: f32,
    steps: u32,
    /// The panel the caption shows and when it began to (its swap plays from then).
    caption: (usize, f32),
    /// Shift + wheel steps at most every 220 ms.
    wheeled: f32,
    frames: u32,
}

impl Portfolio {
    fn new() -> Self {
        let t = now();
        Self { metal: metal(), started: t, intro: Intro::Typing, scroll: 0.0, shown: 0.0, last: t, page_grab: None,
               page_move: (0.0, 0.0, t), pos: MotionValue::new(0.0), grab: None, hover: false, stepped: t, steps: 0,
               caption: (0, -1.0), wheeled: 0.0, frames: 0 }
    }

    const fn done(&self) -> bool {
        matches!(self.intro, Intro::Done(_))
    }

    /// Return / click: finishes the typing if it is still going, then runs the transcript.
    fn run(&mut self) -> bool {
        match self.intro {
            Intro::Typing | Intro::Ready(_) => {
                self.intro = Intro::Running(now());
                true
            }
            _ => false,
        }
    }

    /// Escape: straight to the page.
    fn skip(&mut self) -> bool {
        if matches!(self.intro, Intro::Leaving(_) | Intro::Done(_)) {
            return false;
        }
        self.intro = Intro::Leaving(now());
        true
    }

    /// The active panel (0..10).
    fn active(&self) -> usize {
        self.pos.get().round().rem_euclid(PROJECTS.len() as f32) as usize
    }

    /// Springs the carousel `by` panels on from where it is heading.
    fn step(&mut self, by: f32) {
        let to = self.pos.get().round() + by;
        self.spring_to(to);
    }

    fn spring_to(&mut self, to: f32) {
        drop(self.pos.animate(to, vars! { type: "spring", stiffness: 210, damping: 26 }));
        self.stepped = now();
        self.steps += 1;
    }

    fn max_scroll(fit: &Fit) -> f32 {
        (PAGE_H - fit.h).max(0.0)
    }

    fn scroll_by(&mut self, dy: f32) {
        if self.done() {
            self.scroll += dy;
        }
    }

    fn key(&mut self, key: &str) -> bool {
        match key {
            "Return" | "KP_Enter" => self.run(),
            "Escape" => self.skip(),
            "Right" if self.done() => {
                self.step(1.0);
                true
            }
            "Left" if self.done() => {
                self.step(-1.0);
                true
            }
            "Down" | "Next" | "space" => {
                self.scroll_by(if key == "Down" { 120.0 } else { 520.0 });
                true
            }
            "Up" | "Prior" => {
                self.scroll_by(if key == "Up" { -120.0 } else { -520.0 });
                true
            }
            _ => false,
        }
    }

    /// The terminal inside the metal frame, as far as the intro has got.
    fn terminal(&self, t: f32, cx: &mut Context<Self>) -> Div {
        let leaving = matches!(self.intro, Intro::Leaving(_));
        let typed = match self.intro {
            Intro::Typing => (0..=CMD.len()).take_while(|&k| typed_at(k) <= t - self.started).last().unwrap_or(0),
            _ => CMD.len(),
        };
        let mut out = div();
        if let Intro::Running(r) | Intro::Leaving(r) = self.intro {
            let since = if leaving { f32::INFINITY } else { t - r };
            for (n, (mark, lab, text)) in LINES.iter().enumerate() {
                if line_at(n) > since {
                    break;
                }
                out = out.child(if mark.is_empty() {
                    div().class("dim").child(*text)
                } else {
                    div().child(format!("{mark} ")).child(span().class("lab").child(*lab)).child(*text)
                });
            }
        }
        let caret = if leaving || matches!(self.intro, Intro::Running(_)) { div() } else { span().class("caret") };
        let text = div().id("term").class(if leaving { "term mono out" } else { "term mono in" })
            .child(div().class("dots").child(i()).child(i()).child(i()))
            .child(div().class("dim").child("mara@oki ~ zsh"))
            .child(div().css("margin-top", rem(8.0)).child(span().class("faint").child("~ $ ")).child(&CMD[..typed]).child(caret))
            .child(out);
        let pad = 56.0;
        let mut term = div().class("abs").css("left", rem(FRAME.0 + pad)).css("top", rem(FRAME.1 + pad))
            .css("width", rem(FRAME.2 - 2.0 * pad)).css("height", rem(FRAME.3 - 2.0 * pad)).child(text);
        if let Intro::Ready(_) | Intro::Leaving(_) = self.intro {
            term = term.child(div().id("hint").class(if leaving { "abs hint out" } else { "abs hint" })
                .css("left", "0").css("bottom", "0").css("width", "100%")
                .child(span().child("Press ").child(kbd().child("return")).child(" to run"))
                .child(div().id("run").class("run").child("Run").child(span().child("⏎"))
                    .on_click(cx.listener(|this, _, _, cx| if this.run() { cx.notify() }))));
        }
        term.on_click(cx.listener(|this, _, _, cx| if this.run() { cx.notify() }))
    }

    /// The hero's copy and stats; hidden until the intro is done, then risen in 50 ms apart.
    fn hero(&self, t: f32) -> (Div, Div) {
        let done = match self.intro {
            Intro::Done(d) => Some(d),
            _ => None,
        };
        let item = |d: Div, k: usize| match done {
            Some(_) => d.class("rise").css("animation-delay", format!("{}ms", k * 50)),
            None => d.class("pre"),
        };
        let copy = div().class("abs copy")
            .child(item(div().class("face").child("MO"), 0))
            .child(item(h1().child("Hello, I'm Mara. I design and build apps."), 1))
            .child(item(p().child("Fourteen years leading product design for studios and startups. Founder of Murmur and Tidepool; speaker at a few conferences you've probably skipped."), 2))
            .child(item(div().class("ctas").child(div().id("contact").class("btn").child("Contact"))
                .child(div().class("more").child("See the work →")), 3));
        let mut stats = div().class("abs stats");
        for (k, (label, n, prefix, suffix, pad)) in STATS.iter().enumerate() {
            let shown = done.map_or(0, |d| (*n as f32 * ease_out(progress(t - d - 0.2 - k as f32 * 0.05, 0.0, 1.4))).round() as u32);
            let v = if *pad { format!("{prefix}{shown:02}{suffix}") } else { format!("{prefix}{shown}{suffix}") };
            stats = stats.child(item(div().class("stat").child(span().child(*label)).child(b().child(v)), 4 + k));
        }
        (copy, stats)
    }

    /// The projects section: pill, icon, lead, the carousel and its caption, each revealed by the
    /// scroll (`shown`, design px) as it comes up from the bottom of a `view_h` tall window.
    fn projects(&self, t: f32, view_h: f32, cx: &mut Context<Self>) -> Div {
        // a full-width strip at `y`, centring `d`, faded and lifted in as it comes into view
        let reveal = |d: Div, y: f32| {
            let k = ease_out(progress(self.shown + view_h - y, 0.0, 220.0));
            div().class("abs").css("left", "0").css("top", rem(y)).css("width", rem(DESIGN.0)).css("display", "flex")
                .css("justify-content", "center")
                .css("opacity", format!("{k}")).css("transform", format!("translateY({})", rem(30.0 * (1.0 - k)))).child(d)
        };
        let icon = self.metal.canvas(move |b| [t, 1.0, 24.0, b.size.width]).css("width", rem(96.0)).css("height", rem(96.0));
        let mut sec = div()
            .child(reveal(div().child(icon), 770.0))
            .child(reveal(div().class("pill").child(b().child("01")).child("Launched & building"), 890.0))
            .child(reveal(div().class("lead").child("I build my own products, end to end."), 936.0))
            .child(reveal(div().class("sub").child("5 are live on the App Store and the web, 5 more are in the works."), 986.0));

        // the row: the panels within 3.5 of the position, nearest on top
        let pos = self.pos.get();
        let n = PROJECTS.len() as f32;
        let mut near: [(f32, usize); 10] = std::array::from_fn(|i| {
            let d = (i as f32 - pos + n * 0.5).rem_euclid(n) - n * 0.5;
            (d, i)
        });
        near.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
        let mut row = div().id("row").class("row");
        for &(d, i) in &near {
            let a = d.abs();
            if a > 3.5 {
                continue;
            }
            let Project { name, colors: (c1, c2), mark, .. } = PROJECTS[i];
            let scale = 1.0 - a.min(3.0) * 0.17;
            // neighbours sit a gap apart at their own scale: the integral of the scale, plus gaps
            let x = 640.0 + d.signum() * (PANEL * (a - 0.085 * a * a) + GAP * a) - PANEL / 2.0;
            let mut panel = div().class("panel").css("left", rem(x)).css("top", rem(40.0))
                .css("transform", format!("scale({scale})")).css("opacity", format!("{}", (1.0 - a * 0.26).max(0.0)))
                .child(div().class("art").css("background", format!("linear-gradient(145deg, {c1} 0%, {c2} 100%)"))
                    .child(div().class("mark").child(mark)))
                .child(div().class("sheen")).child(div().class("rim"));
            // depth: a veil of the page's colour, thicker the further out (a `filter: blur()` a
            // panel would raster every moving panel again each frame: ten times the cost)
            if a > 0.05 {
                panel = panel.child(div().class("veil").css("opacity", format!("{}", a.min(3.0) * 0.2)));
            }
            let by = d.round();
            row = row.child(panel.id(format!("p-{name}")).on_click(cx.listener(move |this, _, _, cx| {
                if by != 0.0 {
                    this.step(by);
                    cx.notify();
                }
            })));
        }
        row = row
            .on_hover(cx.listener(|this, over: &bool, _, _| this.hover = *over))
            .on_pointer_down(cx.listener(|this, e: &PointerEvent, _, cx| {
                this.grab = Some(Grab::new(e.position.x.0, this.pos.get()));
                cx.stop_propagation();
            }))
            .on_pointer_move(cx.listener(|this, e: &PointerEvent, w, cx| {
                let k = Fit::new(w, DESIGN).scale;
                if let Some(g) = this.grab.as_mut()
                    && let Some(dx) = g.moved(e.position.x.0, k, w, cx)
                {
                    // the panels follow the finger
                    let p0 = g.start;
                    this.pos.stop();
                    this.pos.set(p0 - dx / STEP);
                    cx.notify();
                }
            }))
            .on_pointer_up(cx.listener(|this, e: &PointerEvent, w, cx| {
                if let Some(g) = this.grab.take()
                    && g.held
                {
                    let dx = (e.position.x.0 - g.from) / Fit::new(w, DESIGN).scale;
                    let p0 = g.start.round();
                    let to = if dx.abs() > SWIPE { p0 - dx.signum() * (dx.abs() / STEP).ceil().min(3.0) } else { p0 };
                    this.spring_to(to);
                    w.release_pointer_capture(cx);
                    cx.notify();
                }
            }))
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                if e.modifiers.shift {
                    let t = now();
                    if t - this.wheeled > 0.22 {
                        this.wheeled = t;
                        this.step(-e.delta.signum());
                        cx.notify();
                    }
                    cx.stop_propagation();
                }
            }));
        sec = sec.child(reveal(row, ROW_Y - 40.0));

        // the caption of the active panel
        let active = self.active();
        let Project { name, launched, platforms, summary, .. } = PROJECTS[active];
        // the swap: a 5 px fade-up (the reference's cubic-bezier(0.16, 1, 0.3, 1), an expo-out),
        // the summary 40 ms behind the title
        let swap = |d: Div, delay: f32| {
            let k = expo_out(progress(t - self.caption.1 - delay, 0.0, 0.3));
            // settled: no styles left (a settled opacity layer over a swapped child does not repaint)
            if k >= 1.0 {
                return d;
            }
            d.css("opacity", format!("{k}")).css("transform", format!("translateY({})", rem(5.0 * (1.0 - k))))
        };
        let mut title = swap(h3(), 0.0).child(name);
        if !launched {
            title = title.child(span().class("badge").child("building"));
        }
        sec = sec
            .child(reveal(div().class("caption")
                .child(title)
                .child(swap(p(), 0.04).child(format!("{summary} · {platforms}"))),
                ROW_Y + 380.0))
            .child(reveal(div().class("ctl")
                .child(div().id("prev").class("arrow").child("←").on_click(cx.listener(|this, _, _, cx| { this.step(-1.0); cx.notify(); })))
                .child(div().class("count").child(format!("{} / {}", active + 1, PROJECTS.len())))
                .child(div().id("next").class("arrow").child("→").on_click(cx.listener(|this, _, _, cx| { this.step(1.0); cx.notify(); }))),
                ROW_Y + 486.0));
        sec
    }
}

impl Render for Portfolio {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        focus_root(w, &mut self.frames);
        let fit = Fit::new(w, DESIGN);
        let t = now();

        // the intro's clock: typed -> ready, run -> leave -> done
        self.intro = match self.intro {
            Intro::Typing if typed_at(CMD.len()) <= t - self.started => Intro::Ready(t),
            Intro::Running(r) if t - r > RUN => Intro::Leaving(t),
            Intro::Leaving(l) if t - l > LEAVE => Intro::Done(t),
            i => i,
        };
        let active = self.active();
        if active != self.caption.0 {
            self.caption = (active, t);
        }
        // the scroll: clamped, followed; the carousel: advances while left alone
        let dt = (t - self.last).clamp(0.0, 0.1);
        self.last = t;
        self.scroll = self.scroll.clamp(0.0, Self::max_scroll(&fit));
        self.shown = approach(self.shown, self.scroll, 9.0, dt);
        if self.done() && !self.hover && self.grab.is_none() && t - self.stepped > ADVANCE && self.shown + fit.h > ROW_Y {
            self.step(1.0);
        }

        let appear = expo_out(progress(t - self.started, 0.0, 1.4));
        let frame = self.metal.canvas(move |b| [t, appear, FRAME_R, b.size.width]).class("metal")
            .css("left", rem(FRAME.0)).css("top", rem(FRAME.1)).css("width", rem(FRAME.2)).css("height", rem(FRAME.3));
        let (copy, stats) = self.hero(t);
        // the hero drifts up slower than the page and settles back as it leaves
        let drift = self.shown * 0.25;
        let hero = div().class("abs").css("left", "0").css("top", rem(drift)).css("width", rem(DESIGN.0))
            .css("height", rem(780.0)).child(frame).child(copy).child(stats);
        let page = div().class("page").css("transform", format!("translateY({})", rem(-self.shown)))
            .child(hero).child(self.projects(t, fit.h, cx));

        let done = self.done();
        let header = div().class("abs head").css("left", rem(80.0 + fit.ox))
            .child(div().class(if done { "brand rise" } else { "brand pre" }).child("mara oki"))
            .child(div().class(if done { "nav rise" } else { "nav pre" }).css("animation-delay", "100ms")
                .child(div().child("Work")).child(div().child("About")).child(div().child("Contact")));
        let mut ui = div().class("abs").css("left", rem(fit.ox)).css("top", "0").css("width", rem(DESIGN.0)).css("height", rem(fit.h))
            .child(page);
        // the intro: the cover, and the metal and terminal over it
        if !done {
            let leaving = matches!(self.intro, Intro::Leaving(_));
            let cover_metal = self.metal.canvas(move |b| [t, appear, FRAME_R, b.size.width]).class("metal")
                .css("left", rem(FRAME.0)).css("top", rem(FRAME.1)).css("width", rem(FRAME.2)).css("height", rem(FRAME.3));
            ui = ui.child(div().class(if leaving { "abs cover out" } else { "abs cover" }).css("left", rem(-fit.ox))
                .css("width", rem(fit.w)).css("height", rem(fit.h)))
                .child(cover_metal)
                .child(self.terminal(t, cx));
            if !leaving {
                ui = ui.child(div().id("skip").class("abs skip").child("Skip intro · esc")
                    .on_click(cx.listener(|this, _, _, cx| if this.skip() { cx.notify() })));
            }
        }

        html().css("font-size", fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root").attr("tabindex", "0")
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| if this.key(&e.keystroke.key) { cx.notify() }))
                .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, w, cx| {
                    let k = Fit::new(w, DESIGN).scale;
                    this.scroll_by(-e.pixels.map_or(e.delta * 90.0, |p| p / k));
                    cx.notify();
                }))
                .on_pointer_down(cx.listener(|this, e: &PointerEvent, _, _| {
                    if this.done() {
                        let t = now();
                        this.page_grab = Some(Grab::new(e.position.y.0, this.scroll));
                        this.page_move = (e.position.y.0, 0.0, t);
                    }
                }))
                .on_pointer_move(cx.listener(|this, e: &PointerEvent, w, cx| {
                    let k = Fit::new(w, DESIGN).scale;
                    let (y, t) = (e.position.y.0, now());
                    if let Some(g) = this.page_grab.as_mut()
                        && let Some(dy) = g.moved(y, k, w, cx)
                    {
                        let s0 = g.start;
                        let (ly, _, lt) = this.page_move;
                        this.page_move = (y, (ly - y) / k / (t - lt).max(1e-3), t);
                        this.scroll = s0 - dy;
                        this.shown = this.scroll;
                        cx.notify();
                    }
                }))
                .on_pointer_up(cx.listener(|this, _: &PointerEvent, w, cx| {
                    if this.page_grab.take().is_some_and(|g| g.held) {
                        // a fling: carry on by a third of a second's worth of the last speed
                        this.scroll += this.page_move.1 * 0.3;
                        w.release_pointer_capture(cx);
                        cx.notify();
                    }
                }))
                .child(ui).child(div().class("abs headbar").css("opacity", format!("{}", progress(self.shown, 0.0, 60.0)))).child(header))
    }
}

fn metal() -> Fx {
    Fx::new("liquid-metal", METAL, 4, [196, 199, 206])
}

/// Types, runs, scrolls down to the carousel, steps it with keys, a button and a drag, waits for
/// the auto-advance.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        s.play("00-intro", &[0, 300, 700, 1300, 1800, 2600, 3400]);
        s.cx.simulate_mouse_move(s.win, 5.0, 5.0);
        s.key("Return");
        s.play("01-run", &[0, 300, 800, 1500, 2200, 2400, 2700, 3300, 3800, 4600]);
        for _ in 0..8 {
            s.cx.simulate_scroll(s.win, 640.0, 400.0, -2);
            s.tick(60);
        }
        s.play("02-scroll", &[0, 400, 1200]);
        s.key("Right");
        s.play("03-right", &[0, 120, 600]);
        s.click("next");
        s.play("04-next", &[0, 600]);
        let (x, y) = s.centre("row");
        s.cx.simulate_drag(s.win, (x + 150.0, y), (x - 200.0, y));
        s.play("05-drag", &[0, 150, 700]);
        s.cx.simulate_mouse_move(s.win, 5.0, 5.0);
        s.play("06-auto", &[0, 3700, 4200]);
    })
}

fn main() {
    if let Some(dir) = arg("--frames") {
        let fx = metal();
        let stills = [("metal-appear", [0.0f32, 0.3, 28.0, 600.0]), ("metal", [3.0, 1.0, 28.0, 600.0]), ("metal-later", [9.0, 1.0, 28.0, 600.0])];
        if gpu_stills(&dir, 600, 560, stills, |g, f| g.draw(fx.effect(), &[], f)).is_none() {
            eprintln!("metal shader: {:?}", fx.error());
        }
        return;
    }
    run_with(DESIGN.0, DESIGN.1, "BUI · Intro", Portfolio::new, arg("--storyboard").map(storyboard));
}

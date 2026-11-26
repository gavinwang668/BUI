//! Lagoon: a skinned desktop MP3 player of the early 2000s, on BUI. Three docked windows in a
//! glossy aqua skin: the player (LCD clock, a live spectrum analyzer painted with BUI's `Painter`,
//! a character-stepped marquee, kbps / kHz, volume, balance, seek, transport), the equalizer
//! (ON / AUTO, a preamp and ten bands, the curve they make, presets) and the playlist.
//!
//! Everything works: Play runs the clock and the analyzer (its bars follow the EQ, the preamp and
//! the volume, peaks hold then fall); click the analyzer for the oscilloscope, the clock for the
//! time left. Drag (or wheel) any slider, click the seek bar to jump. PRESETS slides every band to
//! the next preset; AUTO picks one per track. Click a row to select it, double-click (or Enter) to
//! play it; +FILE adds a track, -FILE removes the selection, SHUF shuffles, SORT sorts, LOAD
//! resets the list. EQ / PL fold their windows away and the ones below slide up. Keys: Z X C V B
//! (prev, play, pause, stop, next), Space, ↑ / ↓ in the list, Enter.
//!
//! Motion: the windows drop in on first light (80 ms apart); folding a window is a 260 ms
//! transform + opacity transition; presets ease out over 420 ms; presses scale to 0.96.
//!
//! `cargo run --release --bin amp`, `-- --storyboard DIR` for headless frames.

use std::sync::OnceLock;

use blenderui::prelude::*;
use bui_demos::canvas::{rem, rem_css, Fit};
use bui_demos::control::{draggable, Axis, Dial};
use bui_demos::ease::{ease_out, lerp, progress};
use bui_demos::{arg, focus_root, now, run_with, story::story, Script};

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; position: relative; overflow: hidden; user-select: none;
       background: linear-gradient(160deg, #134b5f 0%, #0b2a3a 55%, #071822 100%);
       font-family: Tahoma, 'Segoe UI', sans-serif; font-size: 11px; color: #0f5a63; -webkit-font-smoothing: antialiased; }
.abs { position: absolute; }
.app { position: absolute; width: 560px; height: 700px; }
.mono { font-family: Consolas, 'Courier New', monospace; }
.num { font-variant-numeric: tabular-nums; }

/* docked windows: a slot moves (folding), the window inside drops in once */
.slot { position: absolute; left: 20px; width: 520px;
        transition-property: transform, opacity; transition-duration: 260ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.slot.gone { opacity: 0; transform: translateY(-10px) scale(0.97); pointer-events: none; }
.win { position: relative; width: 520px; border-radius: 10px;
       background: linear-gradient(180deg, #f0fdfd 0%, #c4ecef 40%, #98d8de 100%);
       box-shadow: 0 18px 44px rgba(0,0,0,0.5), 0 0 0 1px #3d939b, inset 0 1px 0 #fff;
       animation: drop 520ms cubic-bezier(0.2, 0, 0, 1) backwards; }
@keyframes drop { from { opacity: 0; transform: translateY(-18px); } to { opacity: 1; transform: translateY(0); } }
.tbar { position: absolute; left: 4px; top: 4px; width: 512px; height: 20px; border-radius: 6px;
        background: linear-gradient(180deg, #8fe0e6 0%, #43b4be 50%, #2b9aa5 52%, #5cc6cf 100%);
        box-shadow: inset 0 1px 0 rgba(255,255,255,0.7), 0 0 0 1px #2f8a93;
        color: #fff; font-weight: 700; font-size: 10px; letter-spacing: 4px; text-align: center; line-height: 20px; }
.tbar .dots { position: absolute; right: 6px; top: 4px; display: flex; gap: 4px; }
.tbar .dots div { width: 12px; height: 12px; border-radius: 6px; background: linear-gradient(180deg, #e8fbfc, #7ccdd4);
                  box-shadow: 0 0 0 1px #2f8a93; }
.tbar .grip { position: absolute; left: 8px; top: 6px; width: 60px; height: 8px;
              background: repeating-linear-gradient(180deg, rgba(255,255,255,0.6) 0px, rgba(255,255,255,0.6) 1px, transparent 1px, transparent 3px); }

/* the LCD-ish panels */
/* the LCD's glass clips what it shows: the analyzer, the curve, the scrolling marquee */
.lcd { position: absolute; overflow: hidden; border-radius: 6px; background: linear-gradient(180deg, #f2feff 0%, #c7f0f4 100%);
       box-shadow: inset 0 1px 3px rgba(0,60,70,0.35), 0 0 0 1px #6cb9c0; }
.clutter { position: absolute; left: 10px; top: 32px; width: 14px; display: flex; flex-direction: column; gap: 3px;
           font-size: 9px; font-weight: 700; color: #2a8c95; text-align: center; }
.time { position: absolute; left: 34px; top: 0px; font-size: 40px; font-weight: 700; color: #0d4f58; letter-spacing: 1px; cursor: pointer;
        text-shadow: 0 0 6px rgba(13,79,88,0.3); }
.state { position: absolute; left: 10px; top: 14px; width: 14px; height: 14px; background: #0d4f58; }
.st-play { clip-path: polygon(10% 0, 100% 50%, 10% 100%); background: #1c9b3a; }
.st-pause { clip-path: polygon(0 0, 38% 0, 38% 100%, 0 100%, 0 0, 62% 0, 100% 0, 100% 100%, 62% 100%, 62% 0); }
.st-stop { transform: scale(0.8); }
.marq { font-size: 13px; color: #0d4f58; line-height: 24px; padding-left: 8px; white-space: pre; }
.tag { font-size: 13px; font-weight: 700; color: #0d4f58; text-align: center; line-height: 20px; }
.lbl { position: absolute; font-size: 9px; font-weight: 700; color: #2a7d86; letter-spacing: 0.5px; }
.led { position: absolute; font-size: 9px; font-weight: 700; color: #8cc3c8; letter-spacing: 0.5px;
       transition-property: color; transition-duration: 160ms; }
.led.lit { color: #0a7f8b; }

/* buttons: glossy aqua keys */
.key { position: absolute; display: flex; align-items: center; justify-content: center; flex-direction: column; border-radius: 6px; cursor: pointer;
       background: linear-gradient(180deg, #ffffff 0%, #d4f3f5 48%, #a9e1e6 52%, #cdf0f2 100%);
       box-shadow: 0 1px 2px rgba(0,50,60,0.35), 0 0 0 1px #5aaab2, inset 0 1px 0 #fff;
       font-size: 9px; font-weight: 700; color: #11606a; letter-spacing: 0.5px; line-height: 1.15;
       transition-property: transform, background-color, box-shadow; transition-duration: 120ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.key:hover { background-color: #e9fbfc; }
.key:active { transform: scale(0.96); }
.key.lit { box-shadow: 0 1px 2px rgba(0,50,60,0.35), 0 0 0 1px #2b9aa5, inset 0 0 6px rgba(40,190,200,0.6); }
.key .dot { width: 6px; height: 6px; border-radius: 3px; background: #9cc9cd; margin-bottom: 2px;
            transition-property: background-color, box-shadow; transition-duration: 160ms; }
.key.lit .dot { background: #31e05a; box-shadow: 0 0 5px #31e05a; }
.glyph { background: #11606a; }
.tri { width: 12px; height: 12px; clip-path: polygon(12% 0, 100% 50%, 12% 100%); }
.bars2 { width: 11px; height: 12px; clip-path: polygon(0 0, 36% 0, 36% 100%, 0 100%, 0 0, 64% 0, 100% 0, 100% 100%, 64% 100%, 64% 0); }
.sq { width: 11px; height: 11px; border-radius: 1px; }
.skip { display: flex; align-items: center; }
.skip .tri { width: 9px; height: 11px; }
.skip .bar { width: 3px; height: 11px; }
.eject { width: 12px; height: 8px; clip-path: polygon(50% 0, 100% 100%, 0 100%); }

/* sliders */
.hs { position: absolute; height: 20px; cursor: pointer; }
.groove { position: absolute; left: 0; top: 7px; width: 100%; height: 6px; border-radius: 3px; background: #7fbfc5;
          box-shadow: inset 0 1px 2px rgba(0,50,60,0.5); }
.hs .fill { position: absolute; left: 0; top: 7px; height: 6px; border-radius: 3px; background: linear-gradient(180deg, #9ff0a8, #2fb64d); }
.thumb { position: absolute; top: 3px; width: 26px; height: 14px; margin-left: -13px; border-radius: 7px;
         background: linear-gradient(180deg, #ffffff 0%, #c9eef1 50%, #8fd3d9 52%, #d7f4f6 100%);
         box-shadow: 0 1px 3px rgba(0,50,60,0.5), 0 0 0 1px #4a9ea6;
         transition-property: transform; transition-duration: 120ms; }
.hs:hover .thumb, .vs:hover .orb { transform: scale(1.08); }
.grab .thumb, .grab .orb { transform: scale(1.15); }

.vs { position: absolute; width: 28px; height: 104px; cursor: pointer; }
.vs .groove { left: 11px; top: 4px; width: 6px; height: 96px; }
.vs .mid { position: absolute; left: 6px; top: 51px; width: 16px; height: 2px; background: rgba(15,90,99,0.35); }
.vs .lvl { position: absolute; left: 11px; width: 6px; border-radius: 3px; }
.orb { position: absolute; left: 3px; width: 22px; height: 22px; margin-top: -11px; border-radius: 11px;
       background: linear-gradient(180deg, #e6fbff 0%, #6fd0f0 45%, #1f93c9 55%, #6fd6f2 100%);
       box-shadow: 0 2px 4px rgba(0,40,70,0.5), 0 0 0 1px #1d7ea8, inset 0 1px 0 #fff;
       transition-property: transform; transition-duration: 120ms; }
.db { position: absolute; left: -10px; width: 48px; text-align: center; font-size: 9px; font-weight: 700; color: #0d4f58;
      opacity: 0; transition-property: opacity; transition-duration: 120ms; }
.grab .db { opacity: 1; }
.band { position: absolute; top: 192px; width: 40px; text-align: center; font-size: 9px; font-weight: 700; color: #2a7d86; }
.toast { position: absolute; right: 8px; top: 4px; font-size: 10px; font-weight: 700; color: #0d4f58; letter-spacing: 1px; }

/* playlist */
.list { left: 10px; top: 30px; width: 500px; height: 150px; overflow-y: auto; background: linear-gradient(180deg, #ffffff, #effcfd); }
.row { display: flex; height: 21px; line-height: 21px; padding: 0 8px; font-size: 12px; color: #1d6e78; cursor: pointer;
       transition-property: background-color, color; transition-duration: 100ms; }
.row:hover { background: rgba(120,210,218,0.22); }
.row.sel { background: linear-gradient(180deg, #bdeef2, #8fd8de); color: #0b4a52; }
.row.cur { font-weight: 700; color: #0f4fb8; }
.row .t { flex: 1; white-space: nowrap; overflow: hidden; }
.row .d { width: 44px; text-align: right; }
.newrow { animation: rowin 300ms cubic-bezier(0.2, 0, 0, 1); }
@keyframes rowin { from { opacity: 0; transform: translateX(-12px); } to { opacity: 1; transform: translateX(0); } }
";

const DESIGN: (f32, f32) = (560.0, 700.0);
/// The three windows: top (design px) and height.
const MAIN: (f32, f32) = (16.0, 204.0);
const EQW: (f32, f32) = (226.0, 210.0);
const PLW: (f32, f32) = (442.0, 236.0);
/// Window x on the design canvas.
const WIN_X: f32 = 20.0;
/// A slider in the player window: its id, x, y, width, and whether it fills up to the thumb.
#[derive(Clone, Copy)]
struct Fader {
    id: &'static str,
    x: f32,
    y: f32,
    w: f32,
    fill: bool,
}
const SEEK: Fader = Fader { id: "seek", x: 24.0, y: 130.0, w: 472.0, fill: true };
const VOLUME: Fader = Fader { id: "volume", x: 232.0, y: 92.0, w: 150.0, fill: true };
const BALANCE: Fader = Fader { id: "balance", x: 392.0, y: 92.0, w: 70.0, fill: false };
/// Equalizer sliders: the preamp's x, the first band's, the step, the top, the travel.
const EQ_PRE_X: f32 = 22.0;
const EQ_BAND_X: f32 = 100.0;
const EQ_STEP: f32 = 41.0;
const EQ_TOP: f32 = 84.0;
const EQ_TRAVEL: f32 = 96.0;
const BANDS: [&str; 10] = ["60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K"];
/// Analyzer bars.
const BARS: usize = 24;

/// Artist, title, seconds.
const LIBRARY: [(&str, &str, u32); 9] = [
    ("Dune Static", "Low Tide Signal", 240), ("Velvet Ohm", "Glass Arcade", 211),
    ("Kiln & Koi", "Paper Lanterns (Short One)", 240), ("Northbound Tram", "Bad Signal", 225),
    ("Mono Lisa", "Pastel Riot", 198), ("Halcyon Drive", "CRT Sunset", 252),
    ("Saltwater Choir", "Dial-Up Hymn", 233), ("Neon Orchard", "Floppy Heart", 187), ("Quiet Arcade", "Insert Coin", 205),
];
const START_LIST: [usize; 6] = [0, 1, 2, 3, 4, 5];

/// Name, then preamp and the ten bands (0..1, 0.5 flat).
const PRESETS: [(&str, [f32; 11]); 6] = [
    ("FLAT", [0.5; 11]),
    ("ROCK", [0.55, 0.72, 0.64, 0.54, 0.42, 0.4, 0.5, 0.62, 0.7, 0.72, 0.72]),
    ("POP", [0.52, 0.42, 0.55, 0.66, 0.7, 0.66, 0.52, 0.44, 0.42, 0.42, 0.44]),
    ("BASS", [0.5, 0.86, 0.8, 0.7, 0.56, 0.46, 0.42, 0.4, 0.4, 0.4, 0.4]),
    ("VOCAL", [0.5, 0.36, 0.42, 0.56, 0.7, 0.74, 0.68, 0.54, 0.46, 0.42, 0.4]),
    ("DANCE", [0.56, 0.8, 0.7, 0.5, 0.44, 0.48, 0.6, 0.7, 0.66, 0.6, 0.58]),
];
/// How long a preset takes to slide in (seconds).
const PRESET_SLIDE: f32 = 0.42;

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Play {
    Stopped,
    Playing,
    Paused,
}

/// The eleven equalizer dials (`band::<0>` is the preamp), as the function pointers `draggable` takes.
fn band<const I: usize>(a: &mut Amp) -> &mut Dial {
    &mut a.eq[I]
}
const BAND_DIALS: [fn(&mut Amp) -> &mut Dial; 11] = [
    band::<0>, band::<1>, band::<2>, band::<3>, band::<4>, band::<5>, band::<6>, band::<7>, band::<8>, band::<9>, band::<10>,
];

fn mmss(secs: u32) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// A cheap repeatable noise in 0..1.
fn hash(a: f32, b: f32) -> f32 {
    ((a * 127.1 + b * 311.7).sin() * 43758.547).fract().abs()
}

#[expect(clippy::struct_excessive_bools, reason = "each is one independent switch on the skin")]
struct Amp {
    play: Play,
    /// The library index playing, and seconds into it.
    cur: usize,
    pos: f32,
    last: f32,
    /// The playlist (library indices), the selected row, and rows added (keys their slide-in).
    list: Vec<usize>,
    sel: usize,
    added: Option<usize>,
    seek: Dial,
    volume: Dial,
    balance: Dial,
    eq: [Dial; 11],
    eq_on: bool,
    auto: bool,
    preset: usize,
    /// A preset sliding in: when it began and where every band started.
    slide: Option<(f32, [f32; 11])>,
    show_eq: bool,
    show_pl: bool,
    remaining: bool,
    scope: bool,
    shuffle: bool,
    repeat: bool,
    /// The analyzer: bar levels and their peaks (with the time each peak still holds).
    bars: [f32; BARS],
    peaks: [f32; BARS],
    hold: [f32; BARS],
    fit: Fit,
    frames: u32,
}

impl Amp {
    const fn len(&self) -> f32 {
        LIBRARY[self.cur].2 as f32
    }

    fn start(&mut self, lib: usize) {
        self.cur = lib;
        self.pos = 0.0;
        self.play = Play::Playing;
        if self.auto {
            self.apply_preset(1 + lib % (PRESETS.len() - 1));
        }
    }

    /// The row of the playing track in the list, if it is in it.
    fn cur_row(&self) -> Option<usize> {
        self.list.iter().position(|&i| i == self.cur)
    }

    fn skip(&mut self, forward: bool) {
        if self.list.is_empty() {
            return;
        }
        let n = self.list.len();
        let at = self.cur_row().unwrap_or(0);
        let step = if self.shuffle { 1 + (self.pos as usize) % (n.max(2) - 1) } else { 1 };
        let row = if forward { (at + step) % n } else { (at + n - step % n) % n };
        let keep = self.play;
        self.start(self.list[row]);
        if keep != Play::Playing {
            self.play = keep;
        }
    }

    fn apply_preset(&mut self, p: usize) {
        self.preset = p;
        self.slide = Some((now(), self.eq.map(|d| d.value)));
    }

    fn eq_gain(&self, f: f32) -> f32 {
        if !self.eq_on {
            return 1.0;
        }
        let x = f * 9.0;
        let (i, k) = (x.floor() as usize, x.fract());
        let v = lerp(self.eq[1 + i.min(9)].value, self.eq[1 + (i + 1).min(9)].value, k);
        (1.0 + (v - 0.5) * 1.6) * (1.0 + (self.eq[0].value - 0.5) * 0.8)
    }

    /// Advances the clock, a sliding preset and the analyzer by the time since the last frame.
    fn tick(&mut self) {
        let t = now();
        let dt = (t - self.last).clamp(0.0, 0.25);
        self.last = t;
        if self.play == Play::Playing && !self.seek.dragging() {
            self.pos += dt;
            if self.pos >= self.len() {
                if self.repeat { self.pos = 0.0 } else { self.skip(true) }
            }
        }
        if !self.seek.dragging() {
            self.seek.value = self.pos / self.len();
        }
        if let Some((at, from)) = self.slide {
            let k = ease_out(progress(t, at, at + PRESET_SLIDE));
            let to = PRESETS[self.preset].1;
            for (i, d) in self.eq.iter_mut().enumerate() {
                d.value = lerp(from[i], to[i], k);
            }
            if k >= 1.0 {
                self.slide = None;
            }
        }
        let on = self.play == Play::Playing;
        let kick = (-((t % 0.5) / 0.11)).exp();
        let snare = (-(((t + 0.25) % 1.0) / 0.09)).exp();
        for i in 0..BARS {
            let f = i as f32 / (BARS - 1) as f32;
            let jitter = hash((t * 18.0).floor(), i as f32);
            let tone = 0.5 * (1.0 - 0.55 * f) * (0.55 + 0.45 * ((t * (2.0 + i as f32 * 0.37)).sin() * (t * 1.1 + i as f32).cos()).abs());
            let level = tone + kick * 0.55 * (1.0 - f).powi(2) + snare * 0.25 * (1.0 - (f - 0.45).abs() * 2.0).max(0.0) + jitter * 0.18 * f;
            let target = if on { (level * self.eq_gain(f) * (0.35 + 0.8 * self.volume.value)).clamp(0.0, 1.0) } else { 0.0 };
            // bars jump up and fall at a steady rate; peaks hold, then fall slowly
            self.bars[i] = target.max(self.bars[i] - dt * 2.4);
            if self.bars[i] >= self.peaks[i] {
                self.peaks[i] = self.bars[i];
                self.hold[i] = 0.35;
            } else if self.hold[i] > 0.0 {
                self.hold[i] -= dt;
            } else {
                self.peaks[i] = (self.peaks[i] - dt * 0.7).max(0.0);
            }
        }
    }

    fn key(&mut self, key: &str) -> bool {
        match key {
            "z" | "Z" => self.skip(false),
            "x" | "X" => self.press_play(),
            "c" | "C" => self.pause(),
            "v" | "V" => self.stop(),
            "b" | "B" => self.skip(true),
            "space" => {
                if self.play == Play::Playing { self.pause() } else { self.press_play() }
            }
            "Up" => self.sel = self.sel.saturating_sub(1),
            "Down" => self.sel = (self.sel + 1).min(self.list.len().saturating_sub(1)),
            "Return" | "KP_Enter" => {
                if let Some(&lib) = self.list.get(self.sel) {
                    self.start(lib);
                }
            }
            _ => return false,
        }
        true
    }

    fn press_play(&mut self) {
        match self.play {
            Play::Paused => self.play = Play::Playing,
            Play::Playing => self.pos = 0.0,
            Play::Stopped => {
                let lib = self.list.get(self.sel).copied().unwrap_or(self.cur);
                self.start(lib);
            }
        }
    }

    const fn pause(&mut self) {
        self.play = match self.play {
            Play::Playing => Play::Paused,
            Play::Paused => Play::Playing,
            Play::Stopped => Play::Stopped,
        };
    }

    const fn stop(&mut self) {
        self.play = Play::Stopped;
        self.pos = 0.0;
    }

    /// A window point in the coordinates of the window whose top is `top`.
    fn in_window(&self, p: Point<Pixels>, top: f32) -> (f32, f32) {
        let (x, y) = self.fit.design(p);
        (x - WIN_X, y - top)
    }

    fn chrome(title: &'static str, h: f32) -> Div {
        div().class("win").css("height", rem(h))
            .child(div().class("tbar").child(div().class("grip")).child(title)
                .child(div().class("dots").child(div()).child(div()).child(div())))
    }

    fn key_btn(id: &'static str, (x, y, w, h): (f32, f32, f32, f32), lit: bool) -> Div {
        div().id(id).class(if lit { "key lit" } else { "key" })
            .css("left", rem(x)).css("top", rem(y)).css("width", rem(w)).css("height", rem(h))
    }

    /// The slider `f` showing `d`, dragging the dial `dial` picks.
    fn hslider(&self, cx: &mut Context<Self>, f: Fader, d: Dial, dial: fn(&mut Self) -> &mut Dial, changed: fn(&mut Self)) -> Div {
        let pct = format!("{}%", d.value * 100.0);
        let mut s = draggable(div().id(f.id).class(if d.dragging() { "hs grab" } else { "hs" }), cx, Axis::Right,
                              f.w * self.fit.scale, dial, changed)
            .css("left", rem(f.x)).css("top", rem(f.y)).css("width", rem(f.w))
            .child(div().class("groove"));
        if f.fill {
            s = s.child(div().class("fill").css("width", pct.clone()));
        }
        s.child(div().class("thumb").css("left", pct))
    }

    fn main_win(&self, cx: &mut Context<Self>, t: f32) -> Div {
        let shown = if self.remaining { format!("-{}", mmss((self.len() - self.pos).max(0.0) as u32)) } else { mmss(self.pos as u32) };
        let blink = self.play == Play::Paused && (t * 2.0).fract() > 0.5;
        let state = match self.play {
            Play::Playing => "state st-play",
            Play::Paused => "state st-pause",
            Play::Stopped => "state st-stop",
        };
        let (bars, peaks, scope) = (self.bars, self.peaks, self.scope);
        let level = bars.iter().sum::<f32>() / BARS as f32;
        let analyzer = canvas(move |b, p| paint_analyzer(b, p, &bars, &peaks, scope, level, t))
            .id("viz").class("abs").css("left", rem(8.0)).css("top", rem(50.0)).css("width", rem(180.0)).css("height", rem(36.0))
            .on_click(cx.listener(|this, _, _, cx| {
                this.scope = !this.scope;
                cx.notify();
            }));
        let lcd = div().class("lcd").css("left", rem(28.0)).css("top", rem(30.0)).css("width", rem(196.0)).css("height", rem(92.0))
            .child(div().class(state))
            .child(div().id("time").class("time mono num").css("opacity", if blink { "0.15" } else { "1" }).child(shown)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.remaining = !this.remaining;
                    cx.notify();
                })))
            .child(analyzer);
        // the marquee steps a character at a time, as the bitmap fonts did
        let (artist, title, secs) = LIBRARY[self.cur];
        let row = self.cur_row().map_or(0, |r| r + 1);
        let text = format!("{row}. {artist} - {title} ({}) *** ", mmss(secs));
        let n = text.chars().count();
        let k = (t / 0.2) as usize % n;
        let marquee: String = text.chars().cycle().skip(k).take(36).collect();
        let transport = [("prev", "skip rev"), ("play", "tri"), ("pause", "bars2"), ("stop", "sq"), ("next", "skip")];
        let mut win = Self::chrome("L A G O O N", MAIN.1)
            .child(div().class("clutter").children(["O", "A", "I", "D", "V"].map(|c| div().child(c))))
            .child(lcd)
            .child(div().class("lcd").css("left", rem(232.0)).css("top", rem(30.0)).css("width", rem(278.0)).css("height", rem(24.0))
                .child(div().class("marq mono").child(marquee)))
            .child(div().class("lcd tag num").css("left", rem(232.0)).css("top", rem(62.0)).css("width", rem(36.0)).css("height", rem(20.0)).child("128"))
            .child(div().class("lbl").css("left", rem(272.0)).css("top", rem(67.0)).child("kbps"))
            .child(div().class("lcd tag num").css("left", rem(300.0)).css("top", rem(62.0)).css("width", rem(26.0)).css("height", rem(20.0)).child("44"))
            .child(div().class("lbl").css("left", rem(330.0)).css("top", rem(67.0)).child("kHz"))
            .child(div().class("led").css("left", rem(366.0)).css("top", rem(67.0)).child("mono"))
            .child(div().class(if self.play == Play::Stopped { "led" } else { "led lit" }).css("left", rem(398.0)).css("top", rem(67.0)).child("stereo"))
            .child(self.hslider(cx, VOLUME, self.volume, |a| &mut a.volume, |_| {}))
            .child(self.hslider(cx, BALANCE, self.balance, |a| &mut a.balance, |a| {
                // a detent at the centre
                if (a.balance.value - 0.5).abs() < 0.05 {
                    a.balance.value = 0.5;
                }
            }))
            .child(Self::key_btn("eqb", (470.0, 60.0, 40.0, 24.0), self.show_eq).child(div().class("dot")).child("EQ")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.show_eq = !this.show_eq;
                    cx.notify();
                })))
            .child(Self::key_btn("plb", (470.0, 88.0, 40.0, 24.0), self.show_pl).child(div().class("dot")).child("PL")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.show_pl = !this.show_pl;
                    cx.notify();
                })))
            .child(self.hslider(cx, SEEK, self.seek, |a| &mut a.seek, |a| a.pos = a.seek.value * a.len()));
        for (i, (id, glyph)) in transport.into_iter().enumerate() {
            let g = if id == "prev" || id == "next" {
                let (tri, bar) = (div().class("glyph tri"), div().class("glyph bar"));
                if id == "prev" { div().class("skip").child(bar).child(tri.css("transform", "scaleX(-1)")) } else { div().class("skip").child(tri).child(bar) }
            } else {
                div().class(format!("glyph {glyph}"))
            };
            let lit = matches!((id, self.play), ("play", Play::Playing) | ("pause", Play::Paused) | ("stop", Play::Stopped));
            win = win.child(Self::key_btn(id, (10.0 + i as f32 * 46.0, 158.0, 44.0, 36.0), lit).child(g)
                .on_click(cx.listener(move |this, _, _, cx| {
                    match id {
                        "prev" => this.skip(false),
                        "play" => this.press_play(),
                        "pause" => this.pause(),
                        "stop" => this.stop(),
                        _ => this.skip(true),
                    }
                    cx.notify();
                })));
        }
        win.child(Self::key_btn("eject", (246.0, 158.0, 40.0, 36.0), false).child(div().class("glyph eject"))
                .child(div().class("glyph").css("width", rem(12.0)).css("height", rem(2.0)).css("margin-top", rem(2.0)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.show_pl = true;
                    this.added = None;
                    this.list = START_LIST.to_vec();
                    cx.notify();
                })))
            .child(Self::key_btn("shuffle", (330.0, 160.0, 72.0, 32.0), self.shuffle).child(div().class("dot")).child("SHUFFLE")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.shuffle = !this.shuffle;
                    cx.notify();
                })))
            .child(Self::key_btn("repeat", (408.0, 160.0, 56.0, 32.0), self.repeat).child(div().class("dot")).child("REPEAT")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.repeat = !this.repeat;
                    cx.notify();
                })))
            .child(div().class("abs").css("left", rem(472.0)).css("top", rem(158.0)).css("width", rem(36.0)).css("height", rem(36.0))
                .css("border-radius", "50%").css("background", "linear-gradient(180deg, #e6fbff 0%, #6fd0f0 45%, #1f93c9 55%, #6fd6f2 100%)")
                .css("box-shadow", "0 2px 4px rgba(0,40,70,0.5), 0 0 0 1px #1d7ea8, inset 0 1px 0 #fff"))
    }

    fn eq_win(&self, cx: &mut Context<Self>) -> Div {
        let vals = self.eq.map(|d| d.value);
        let (on, preset_at) = (self.eq_on, self.slide.map(|s| s.0));
        let preset = PRESETS[self.preset].0;
        let curve = canvas(move |b, p| paint_curve(b, p, &vals, on))
            .class("abs").css("left", rem(0.0)).css("top", rem(0.0)).css("width", rem(220.0)).css("height", rem(44.0));
        let fade = preset_at.map_or(0.0, |at| 1.0 - progress(now(), at + PRESET_SLIDE + 0.6, at + PRESET_SLIDE + 1.0));
        let mut win = Self::chrome("E Q U A L I Z E R", EQW.1)
            .child(Self::key_btn("on", (10.0, 32.0, 56.0, 24.0), self.eq_on).child(div().class("dot")).child("ON")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.eq_on = !this.eq_on;
                    cx.notify();
                })))
            .child(Self::key_btn("auto", (72.0, 32.0, 62.0, 24.0), self.auto).child(div().class("dot")).child("AUTO")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.auto = !this.auto;
                    cx.notify();
                })))
            .child(div().class("lcd").css("left", rem(150.0)).css("top", rem(28.0)).css("width", rem(220.0)).css("height", rem(44.0))
                .child(curve).child(div().class("toast").css("opacity", format!("{fade}")).child(preset)))
            .child(Self::key_btn("presets", (400.0, 32.0, 110.0, 24.0), false).child("PRESETS")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.apply_preset((this.preset + 1) % PRESETS.len());
                    cx.notify();
                })))
            .child(div().class("lbl").css("left", rem(52.0)).css("top", rem(EQ_TOP)).child("+12 db"))
            .child(div().class("lbl").css("left", rem(58.0)).css("top", rem(EQ_TOP + 46.0)).child("0 db"))
            .child(div().class("lbl").css("left", rem(54.0)).css("top", rem(EQ_TOP + 92.0)).child("-12 db"));
        for (i, dial) in BAND_DIALS.into_iter().enumerate() {
            let d = self.eq[i];
            let x = if i == 0 { EQ_PRE_X } else { EQ_BAND_X + (i - 1) as f32 * EQ_STEP };
            // the level: from the 0 dB line to the thumb, green under, orange over
            let y = 4.0 + (1.0 - d.value) * EQ_TRAVEL;
            let (top, h) = if d.value >= 0.5 { (y, 52.0 - y) } else { (52.0, y - 52.0) };
            let colour = if d.value >= 0.5 { "linear-gradient(180deg, #ffb347, #9ff0a8)" } else { "linear-gradient(180deg, #9ff0a8, #2fb6c4)" };
            let db = (d.value - 0.5) * 24.0;
            win = win.child(draggable(div().id(format!("band{i}")).class(if d.dragging() { "vs grab" } else { "vs" }), cx, Axis::Up,
                                      EQ_TRAVEL * self.fit.scale, dial, |a| a.slide = None)
                .css("left", rem(x)).css("top", rem(EQ_TOP))
                .child(div().class("groove")).child(div().class("mid"))
                .child(div().class("lvl").css("top", rem(top)).css("height", rem(h.max(0.0))).css("background", colour))
                .child(div().class("orb").css("top", rem(y)))
                .child(div().class("db num").css("top", rem(y - 26.0)).child(format!("{db:+.1}"))))
                .child(div().class("band").css("left", rem(x - 6.0)).child(if i == 0 { "PRE" } else { BANDS[i - 1] }));
        }
        win
    }

    fn pl_win(&self, cx: &mut Context<Self>) -> Div {
        let mut list = div().class("abs lcd list");
        for (row, &lib) in self.list.iter().enumerate() {
            let (artist, title, secs) = LIBRARY[lib];
            let mut class = String::from("row mono");
            if row == self.sel {
                class.push_str(" sel");
            }
            if lib == self.cur {
                class.push_str(" cur");
            }
            if self.added == Some(lib) {
                class.push_str(" newrow");
            }
            list = list.child(div().id(format!("r{lib}")).class(class)
                .child(div().class("t").child(format!("{}. {artist} - {title}", row + 1)))
                .child(div().class("d num").child(mmss(secs)))
                .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                    this.sel = row;
                    if e.click_count() >= 2 {
                        this.start(lib);
                    }
                    cx.notify();
                })));
        }
        let total: u32 = self.list.iter().map(|&i| LIBRARY[i].2).sum();
        let buttons: [(&'static str, &str, &str); 4] = [("add", "+", "FILE"), ("rem", "-", "FILE"), ("shuf", "SHUF", "LIST"), ("sort", "SORT", "A-Z")];
        let mut win = Self::chrome("P L A Y L I S T", PLW.1).child(list);
        for (i, (id, a, b)) in buttons.into_iter().enumerate() {
            win = win.child(Self::key_btn(id, (10.0 + i as f32 * 50.0, 188.0, 46.0, 40.0), false).child(a).child(b)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit(id);
                    cx.notify();
                })));
        }
        win.child(div().class("lcd tag mono num").css("left", rem(232.0)).css("top", rem(194.0)).css("width", rem(150.0)).css("height", rem(28.0))
                .css("line-height", rem(28.0)).child(format!("{}/{}", mmss(self.pos as u32), mmss(total))))
            .child(Self::key_btn("load", (430.0, 188.0, 80.0, 40.0), false).child("LOAD").child("LIST")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit("load");
                    cx.notify();
                })))
    }

    /// The playlist buttons. The selection stays on its track through a shuffle or a sort.
    fn edit(&mut self, what: &str) {
        let selected = self.list.get(self.sel).copied();
        match what {
            "add" => {
                if let Some(lib) = (0..LIBRARY.len()).find(|i| !self.list.contains(i)) {
                    self.list.push(lib);
                    self.added = Some(lib);
                    self.sel = self.list.len() - 1;
                }
            }
            "rem" => {
                if self.sel < self.list.len() && self.list[self.sel] != self.cur {
                    self.list.remove(self.sel);
                    self.sel = self.sel.min(self.list.len().saturating_sub(1));
                }
            }
            "shuf" => {
                let n = self.list.len();
                for i in (1..n).rev() {
                    let j = (hash(now(), i as f32) * (i + 1) as f32) as usize % (i + 1);
                    self.list.swap(i, j);
                }
            }
            "sort" => self.list.sort_by_key(|&i| LIBRARY[i].1),
            _ => {
                self.list = START_LIST.to_vec();
                self.added = None;
            }
        }
        if what != "add" && what != "rem" {
            self.sel = selected.and_then(|lib| self.list.iter().position(|&i| i == lib)).unwrap_or(0);
        }
    }
}

/// The analyzer's segments, bottom to top.
const SEGMENT_COLOURS: [u32; 4] = [0x18a0aa, 0x55cf7a, 0xe8d84a, 0xf08a2a];

/// The analyzer: bars in segments (teal to orange up the bar) with peak caps, or the scope.
fn paint_analyzer(b: Bounds<f32>, p: &mut Painter, bars: &[f32; BARS], peaks: &[f32; BARS], scope: bool, level: f32, t: f32) {
    let (x0, y0, w, h) = (b.origin.x, b.origin.y, b.size.width, b.size.height);
    // the dotted floor grid of the old skins
    let mut gy = y0 + 3.0;
    while gy < y0 + h {
        let mut gx = x0;
        while gx < x0 + w {
            p.fill(Bounds { origin: point(gx, gy), size: size(1.0, 1.0) }, rgba(0x0f5a6330));
            gx += 4.0;
        }
        gy += 4.0;
    }
    if scope {
        let pts: [(f32, f32); 72] = std::array::from_fn(|i| {
            let u = i as f32 / 71.0;
            let v = (u * 19.0 + t * 9.0).sin() * 0.6 + (u * 47.0 - t * 13.0).sin() * 0.3 + (u * 7.0 + t * 3.0).sin() * 0.3;
            (x0 + u * w, y0 + h * 0.5 - v * level * h * 0.9)
        });
        p.line(&pts, rgb(0x13a5b0), 1.6);
        return;
    }
    let bw = w / BARS as f32;
    let seg = 3.0;
    let rows = (h / seg) as usize;
    for i in 0..BARS {
        let x = x0 + i as f32 * bw;
        let lit = (bars[i] * rows as f32).round() as usize;
        for r in 0..lit {
            let c = SEGMENT_COLOURS[(r * SEGMENT_COLOURS.len() / rows.max(1)).min(SEGMENT_COLOURS.len() - 1)];
            p.fill(Bounds { origin: point(x + 0.5, y0 + h - (r + 1) as f32 * seg + 1.0), size: size(bw - 1.5, seg - 1.0) }, rgb(c));
        }
        if peaks[i] > 0.02 {
            p.fill(Bounds { origin: point(x + 0.5, y0 + h - peaks[i] * h - 1.0), size: size(bw - 1.5, 1.4) }, rgb(0x0d4f58));
        }
    }
}

/// The equalizer's curve: a Catmull-Rom spline through the ten bands, coloured by gain, over the
/// preamp's line.
fn paint_curve(b: Bounds<f32>, p: &mut Painter, vals: &[f32; 11], on: bool) {
    let (x0, y0, w, h) = (b.origin.x, b.origin.y, b.size.width, b.size.height);
    let pad = 6.0;
    let y_of = |v: f32| y0 + pad + (1.0 - v) * (h - 2.0 * pad);
    p.line(&[(x0 + pad, y_of(0.5)), (x0 + w - pad, y_of(0.5))], rgba(0x0f5a6340), 1.0);
    p.line(&[(x0 + pad, y_of(vals[0])), (x0 + w - pad, y_of(vals[0]))], rgba(0x2fb6c470), 1.0);
    let band = |i: isize| vals[1 + i.clamp(0, 9) as usize];
    let pts: [(f32, f32, f32); 46] = std::array::from_fn(|k| {
        let u = k as f32 / 45.0 * 9.0;
        let i = u.floor() as isize;
        let f = u - i as f32;
        let (a, bb, c, d) = (band(i - 1), band(i), band(i + 1), band(i + 2));
        // Catmull-Rom
        let v = 0.5 * (2.0 * bb + (-a + c) * f + (2.0 * a - 5.0 * bb + 4.0 * c - d) * f * f + (-a + 3.0 * bb - 3.0 * c + d) * f * f * f);
        (x0 + pad + u / 9.0 * (w - 2.0 * pad), y_of(v.clamp(0.0, 1.0)), v)
    });
    for s in pts.windows(2) {
        let v = f32::midpoint(s[0].2, s[1].2);
        let c = if !on { 0x8cc3c8 } else if v > 0.62 { 0xf08a2a } else if v > 0.5 { 0xd8c83a } else if v > 0.4 { 0x3cbf6a } else { 0x18a0aa };
        p.line(&[(s[0].0, s[0].1), (s[1].0, s[1].1)], rgb(c), 2.0);
    }
}

impl Render for Amp {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // the analyzer and the clock move while anything plays or settles
        let peaks_settling = self.peaks.iter().any(|&p| p > 0.0);
        if self.play != Play::Stopped || self.slide.is_some() || peaks_settling || self.frames < 90 {
            w.request_animation_frame();
        }
        focus_root(w, &mut self.frames);
        self.fit = Fit::new(w, DESIGN);
        self.tick();
        let t = now();
        let fold = if self.show_eq { 0.0 } else { EQW.1 + 6.0 };
        let app = div().class("app").css("left", rem(self.fit.ox)).css("top", rem(self.fit.oy))
            .on_mouse_down_capture(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, _| {
                // a press on the seek bar jumps there; the drag carries on from it
                let (x, y) = this.in_window(e.position, MAIN.0);
                if (SEEK.x..SEEK.x + SEEK.w).contains(&x) && (SEEK.y..SEEK.y + 20.0).contains(&y) {
                    this.seek.set((x - SEEK.x) / SEEK.w);
                    this.pos = this.seek.value * this.len();
                }
            }))
            .child(div().class("slot").css("top", rem(MAIN.0)).child(self.main_win(cx, t)))
            .child(div().class(if self.show_eq { "slot" } else { "slot gone" }).css("top", rem(EQW.0))
                .child(self.eq_win(cx).css("animation-delay", "80ms")))
            .child(div().class(if self.show_pl { "slot" } else { "slot gone" }).css("top", rem(PLW.0))
                .css("transform", format!("translateY(-{}rem)", fold / 16.0))
                .child(self.pl_win(cx).css("animation-delay", "160ms")));
        html().css("font-size", self.fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root").attr("tabindex", "0")
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| if this.key(&e.keystroke.key) { cx.notify() }))
                .child(app))
    }
}

/// Every control in turn.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        s.play("00-enter", &[0, 150, 300, 700]);
        s.click("play");
        s.play("01-play", &[0, 500, 1500]);
        s.click("presets");
        s.play("02-preset", &[0, 120, 250, 500, 1600]);
        let b = s.cx.element_bounds(s.win, "band3").expect("band3");
        let x = b.origin.x + b.size.width / 2.0;
        s.cx.simulate_drag(s.win, (x, b.origin.y + 50.0), (x, b.origin.y + 10.0));
        s.play("03-band-drag", &[0, 400]);
        s.click("viz");
        s.play("04-scope", &[0, 300]);
        s.click("viz");
        s.click("time");
        let b = s.cx.element_bounds(s.win, "seek").expect("seek");
        s.cx.simulate_click(s.win, b.origin.x + b.size.width * 0.6, b.origin.y + b.size.height / 2.0);
        s.play("05-seek-remaining", &[0, 300]);
        let b = s.cx.element_bounds(s.win, "volume").expect("volume");
        let y = b.origin.y + b.size.height / 2.0;
        s.cx.simulate_drag(s.win, (b.origin.x + b.size.width * 0.7, y), (b.origin.x + b.size.width * 0.3, y));
        s.play("06-volume", &[0]);
        s.click("r3");
        s.cx.simulate_click(s.win, x, y);
        s.play("07-dblclick-row", &[0, 400]);
        s.click("add");
        s.play("08-add", &[0, 100, 400]);
        s.click("sort");
        s.play("09-sort", &[0]);
        s.click("eqb");
        s.play("10-fold-eq", &[0, 100, 300]);
        s.click("eqb");
        s.play("11-unfold-eq", &[0, 300]);
        s.click("pause");
        s.play("12-pause", &[0, 300, 800]);
        s.cx.simulate_key(s.win, "v", "v");
        s.play("13-stop", &[0, 600, 1500]);
    })
}

fn main() {
    let make = || Amp {
        play: Play::Stopped,
        cur: 2,
        pos: 3.0,
        last: now(),
        list: START_LIST.to_vec(),
        sel: 2,
        added: None,
        seek: Dial::new(0.0),
        volume: Dial::new(0.75),
        balance: Dial::new(0.5),
        eq: PRESETS[3].1.map(Dial::new),
        eq_on: true,
        auto: false,
        preset: 3,
        slide: None,
        show_eq: true,
        show_pl: true,
        remaining: false,
        scope: false,
        shuffle: false,
        repeat: false,
        bars: [0.0; BARS],
        peaks: [0.0; BARS],
        hold: [0.0; BARS],
        fit: Fit { scale: 1.0, w: DESIGN.0, h: DESIGN.1, ox: 0.0, oy: 0.0 },
        frames: 0,
    };
    run_with(DESIGN.0, DESIGN.1, "Lagoon", make, arg("--storyboard").map(storyboard));
}

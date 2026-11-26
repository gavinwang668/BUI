//! Clock: a soft-UI clock app on BUI, two phones of it side by side, in two looks that cross-fade
//! into each other: Soft Light (pale blue-grey, extruded and pressed-in surfaces, navy type, a
//! red second hand) and Night Knob (slate, an orange glow, a glossy knob). Every surface is
//! layered `box-shadow` pairs (light from the top left, shade to the bottom right; inset when
//! pressed) whose colours are CSS variables, so a theme switch transitions every shadow, fill
//! and colour at once while the painted parts (hands, ticks, arcs) mix the same way.
//!
//! Each phone has four tabs: Clock (a live analog face with a sweeping second hand, and the
//! next two alarms), Alarms (switches whose knobs spring across), Stopwatch (a 60-tick ring that
//! fills with the seconds, Start / Stop, Lap / Reset, laps dropping in), and Timer (drag the
//! knob up or down, or turn the wheel over it, to set the minutes; the glowing arc follows,
//! − / + step a minute, the power button runs it and the arc drains, ringing out at zero).
//!
//! Motion: phones and their parts rise in once, 100 ms apart; a tab's view fades up 8 px; the
//! switch knobs travel on an overshooting curve (interruptible transitions, so a quick double
//! flip just turns around); presses sink to `scale(0.96)` and an inset shadow. Gear (on either
//! phone), the side labels or T switch the look; Space starts / stops the stopwatch.
//!
//! `cargo run --release --bin clock`
//! `-- --storyboard DIR` clicks, drags and scrolls through every tab and both looks headlessly
//! and saves frames along BUI's virtual clock.

use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use blenderui::prelude::*;
use bui_demos::canvas::{rem, rem_css, Fit};
use bui_demos::control::{draggable, Axis, Dial};
use bui_demos::pen::{mix, Pen};
use bui_demos::ease::approach;
use bui_demos::{arg, focus_root, now, run_with, size_arg, story::story, Script};

const DESIGN: (f32, f32) = (1240.0, 860.0);
/// The timer knob: degrees its arc starts at (clockwise from 12) and how far a full hour turns.
const ARC_FROM: f32 = -135.0;
const ARC_SWEEP: f32 = 270.0;
/// Pointer travel (design px) that turns the timer knob through the whole hour.
const KNOB_TRAVEL: f32 = 300.0;

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; overflow: hidden; user-select: none;
       font-family: 'Segoe UI', Inter, system-ui, sans-serif; -webkit-font-smoothing: antialiased; }
.app { position: absolute; left: 0; top: 0; width: 100%; height: 100%;
       --bg: #dfe7f2; --card: #e6edf6; --hi: rgba(255,255,255,0.95); --lo: rgba(150,166,190,0.55);
       --ink: #2a3a58; --mute: #8b9ab0; --accent: #3e63f2; --deep: #2c3d60; --nub: #f4f7fb; --red: #ec4d5c;
       --k1: #f3f6fb; --k2: #d3dce9; --gloss: rgba(255,255,255,0.9);
       background: linear-gradient(160deg, #e9eff7 0%, #d5dfec 60%, #c4d1e2 100%); color: var(--ink);
       transition-property: background-color, color; transition-duration: 500ms; }
.app.dark { --bg: #1d242d; --card: #2b3440; --hi: rgba(255,255,255,0.06); --lo: rgba(0,0,0,0.5);
            --ink: #eef2f7; --mute: #7f8b99; --accent: #ff7a1a; --deep: #ff7a1a; --nub: #47525f; --red: #ff7a1a;
            --k1: #5b6878; --k2: #1f262f; --gloss: rgba(200,220,255,0.32);
            background: linear-gradient(160deg, #27303b 0%, #1d242d 55%, #161b22 100%); }
.abs { position: absolute; }
.neu { background: var(--card); box-shadow: -6px -6px 14px var(--hi), 8px 8px 18px var(--lo);
       transition-property: background-color, box-shadow, transform; transition-duration: 500ms, 500ms, 160ms;
       transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.pit { background: var(--card); box-shadow: inset 5px 5px 12px var(--lo), inset -5px -5px 12px var(--hi);
       transition-property: background-color, box-shadow; transition-duration: 500ms; }

.side { top: 0; height: 860px; width: 60px; display: flex; align-items: center; justify-content: center; cursor: pointer; }
.side span { display: block; transform: rotate(-90deg); white-space: nowrap; font-size: 15px; letter-spacing: 1px;
             color: var(--mute); opacity: 0.55; transition-property: opacity, color; transition-duration: 200ms; }
.side:hover span { opacity: 1; }
.side.on span { opacity: 1; color: var(--accent); }

.neu.phone, .neu.face { box-shadow: -14px -14px 34px var(--hi), 18px 18px 40px var(--lo); }
.phone { width: 380px; height: 770px; border-radius: 52px; animation: rise 700ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.title { left: 36px; top: 40px; font-size: 36px; font-weight: 700; letter-spacing: -0.5px; }
.sub { left: 38px; top: 88px; font-size: 14px; color: var(--mute); font-variant-numeric: tabular-nums; }
.round { width: 56px; height: 56px; border-radius: 28px; cursor: pointer; }
.round:active, .btn:active, .tab:active { transform: scale(0.96); box-shadow: inset 4px 4px 10px var(--lo), inset -4px -4px 10px var(--hi); }
.gear { left: 296px; top: 32px; }
.icon { position: absolute; left: 16px; top: 16px; width: 24px; height: 24px; }

.view { left: 0; top: 0; width: 380px; height: 680px; animation: swap 320ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.face { left: 40px; top: 130px; width: 300px; height: 300px; border-radius: 150px; }
.inner { left: 75px; top: 75px; width: 150px; height: 150px; border-radius: 75px;
         box-shadow: -6px -6px 14px var(--hi), 10px 10px 22px var(--lo); }
.paint { left: 0; top: 0; width: 300px; height: 300px; }
.read { left: 0; top: 122px; width: 300px; text-align: center; font-size: 40px; font-weight: 700;
        font-variant-numeric: tabular-nums; letter-spacing: -0.5px; }
.read small { display: block; font-size: 13px; font-weight: 400; color: var(--mute); letter-spacing: 0.5px; margin-top: 2px; }

.row { left: 34px; width: 312px; height: 76px; border-radius: 24px; animation: rise 500ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.row .t { position: absolute; left: 24px; top: 10px; font-size: 28px; font-weight: 600; font-variant-numeric: tabular-nums;
          transition-property: opacity; transition-duration: 300ms; }
.row .t span { font-size: 15px; font-weight: 400; color: var(--mute); margin-left: 6px; }
.row .lbl { position: absolute; left: 25px; top: 50px; font-size: 11px; color: var(--mute); letter-spacing: 0.4px; }
.row.off .t { opacity: 0.45; }
.tog { position: absolute; right: 18px; top: 15px; width: 96px; height: 46px; border-radius: 23px; cursor: pointer;
       background: var(--card); box-shadow: inset 3px 3px 8px var(--lo), inset -3px -3px 8px var(--hi);
       transition-property: background-color, box-shadow; transition-duration: 300ms; }
.nub { position: absolute; left: 4px; top: 4px; width: 38px; height: 38px; border-radius: 19px; background: var(--nub);
       box-shadow: 3px 3px 8px var(--lo), -2px -2px 6px var(--hi);
       transition-property: transform, background-color; transition-duration: 420ms, 300ms;
       transition-timing-function: cubic-bezier(0.34, 1.56, 0.64, 1), cubic-bezier(0.2, 0, 0, 1); }
.tog.on .nub { transform: translateX(50px); background: var(--deep); }

.btn { top: 462px; width: 148px; height: 58px; border-radius: 20px; font-size: 18px; font-weight: 600; text-align: center;
       line-height: 58px; cursor: pointer; }
.btn.warn { color: var(--red); }
.lap { left: 34px; width: 312px; height: 56px; border-radius: 20px; animation: lapin 380ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.lap b { position: absolute; left: 22px; top: 13px; font-size: 22px; font-weight: 700; font-variant-numeric: tabular-nums; }
.lap span.n { position: absolute; left: 50px; top: 20px; font-size: 14px; color: var(--mute); }
.lap .split { position: absolute; right: 22px; top: 16px; font-style: normal; font-size: 18px; font-weight: 600; font-variant-numeric: tabular-nums; }

.big { left: 0; top: 118px; width: 380px; text-align: center; font-size: 68px; font-weight: 700; letter-spacing: -2px;
       color: var(--accent); font-variant-numeric: tabular-nums; transition-property: color; transition-duration: 500ms; }
.hint { left: 0; top: 204px; width: 380px; text-align: center; font-size: 13px; color: var(--mute); }
.dial { left: 30px; top: 236px; width: 320px; height: 320px; }
.knob { left: 80px; top: 80px; width: 160px; height: 160px; border-radius: 80px; cursor: grab;
        background: linear-gradient(150deg, var(--k1) 0%, var(--k2) 100%);
        box-shadow: 0 26px 40px var(--lo), -8px -8px 20px var(--hi), inset 0 2px 2px rgba(255,255,255,0.18);
        transition-property: box-shadow, transform; transition-duration: 200ms; }
.knob.grab { cursor: grabbing; transform: scale(0.98); }
.gloss { left: 24px; top: 6px; width: 112px; height: 66px; background: radial-gradient(ellipse, var(--gloss) 0%, transparent 70%); }
.rot { left: 0; top: 0; width: 160px; height: 160px; }
.pip { left: 74px; top: 14px; width: 12px; height: 12px; border-radius: 6px; background: var(--accent);
       box-shadow: 0 0 10px var(--accent); }
.ring { left: 20px; top: 20px; width: 280px; height: 280px; border-radius: 140px; border: 3px solid var(--accent);
        box-sizing: border-box; opacity: 0; animation: ping 900ms cubic-bezier(0.2, 0, 0, 1) 3; }
.small { width: 48px; height: 48px; border-radius: 24px; }
.small .icon { left: 12px; top: 12px; }

.tabs { left: 30px; top: 680px; width: 320px; height: 64px; display: flex; justify-content: space-between; }
.tab { position: relative; width: 64px; height: 64px; border-radius: 22px; cursor: pointer; }
.tab.on { box-shadow: inset 5px 5px 12px var(--lo), inset -5px -5px 12px var(--hi); }
.tab .icon { left: 20px; top: 20px; }

@keyframes rise { from { opacity: 0; transform: translateY(24px); } to { opacity: 1; transform: translateY(0); } }
@keyframes swap { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
@keyframes lapin { from { opacity: 0; transform: translateY(-14px); } to { opacity: 1; transform: translateY(0); } }
@keyframes ping { 0% { opacity: 0.9; transform: scale(1); } 100% { opacity: 0; transform: scale(1.3); } }
";

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

/// C's `struct tm` (and room for the fields glibc adds after it).
#[repr(C)]
#[derive(Default)]
struct Tm {
    sec: i32,
    min: i32,
    hour: i32,
    _rest: [i32; 6],
    _ext: [i64; 3],
}

#[cfg(windows)]
unsafe extern "C" {
    fn _localtime64_s(tm: *mut Tm, time: *const i64) -> i32;
}
#[cfg(not(windows))]
unsafe extern "C" {
    fn localtime_r(time: *const i64, tm: *mut Tm) -> *mut Tm;
}

/// Seconds since local midnight, with the milliseconds (the C library knows the time zone).
fn day_seconds() -> f32 {
    // the wall clock when the app started, run on by `now()` (virtual in a headless run)
    static START: OnceLock<Duration> = OnceLock::new();
    let start = *START.get_or_init(|| SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default());
    let d = start + Duration::from_secs_f32(now());
    let secs = d.as_secs().cast_signed();
    let mut tm = Tm::default();
    // SAFETY: both pointers are to live locals; the library fills `tm`.
    #[cfg(windows)]
    unsafe {
        _localtime64_s(&raw mut tm, &raw const secs);
    }
    // SAFETY: as above.
    #[cfg(not(windows))]
    unsafe {
        localtime_r(&raw const secs, &raw mut tm);
    }
    (tm.hour * 3600 + tm.min * 60 + tm.sec) as f32 + d.subsec_millis() as f32 / 1000.0
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Clock,
    Alarms,
    Watch,
    Timer,
}

const TABS: [Tab; 4] = [Tab::Clock, Tab::Alarms, Tab::Watch, Tab::Timer];

/// The little line icons, painted so they take the theme's colour.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Icon {
    Tab(Tab),
    Gear,
    Power,
    Minus,
    Plus,
}

struct Alarm {
    mins: u16,
    label: &'static str,
    on: bool,
}

#[derive(Default)]
struct Watch {
    /// When it was started (`now()`), while running.
    since: Option<f32>,
    /// Seconds banked before the current run.
    banked: f32,
    laps: Vec<f32>,
}

impl Watch {
    fn elapsed(&self, now: f32) -> f32 {
        self.banked + self.since.map_or(0.0, |s| now - s)
    }
}

struct Clock {
    dark: bool,
    /// 0 light .. 1 dark: the painted parts' mix, chasing `dark` at the CSS transition's pace.
    theme: f32,
    last: f32,
    tabs: [Tab; 2],
    alarms: [Alarm; 4],
    watch: Watch,
    /// The timer: the knob's value is the time left as a fraction of an hour.
    timer: Dial,
    /// When the running timer ends (`now()`).
    ends: Option<f32>,
    /// Times it has rung out: keys the ring that pulses.
    rang: u32,
    frames: u32,
}

/// The painted colours at theme mix `k`.
#[derive(Clone, Copy)]
struct Pal {
    ink: Rgba,
    mute: Rgba,
    accent: Rgba,
    hand: Rgba,
    minute: Rgba,
    card: Rgba,
    track: Rgba,
}

impl Pal {
    fn at(k: f32) -> Self {
        Self {
            ink: mix(rgb(0x2a3a58), rgb(0xeef2f7), k),
            mute: mix(rgb(0xa3b0c2), rgb(0x56616e), k),
            accent: mix(rgb(0x3e63f2), rgb(0xff7a1a), k),
            hand: mix(rgb(0xec4d5c), rgb(0xff7a1a), k),
            minute: mix(rgb(0x6b7a92), rgb(0xaab4c2), k),
            card: mix(rgb(0xe6edf6), rgb(0x2b3440), k),
            track: mix(rgb(0xd2dbe7), rgb(0x1f262f), k),
        }
    }
}

/// A 24-unit line icon in `color`; `solid` fills its body (the active tab).
fn icon(kind: Icon, color: Rgba, solid: Option<Rgba>) -> Div {
    canvas(move |b, p| {
        let mut pen = Pen::new(b, p, 24.0);
        let w = 1.8;
        let clear = Rgba::TRANSPARENT;
        let body = |pen: &mut Pen, c: (f32, f32), r: f32| match solid {
            Some(_) => pen.disc(c, r, color, clear, 0.0),
            None => pen.disc(c, r, clear, color, w),
        };
        // the hands go in the card's colour on a solid body
        let hands = solid.unwrap_or(color);
        match kind {
            Icon::Tab(Tab::Clock) => {
                body(&mut pen, (12.0, 12.0), 9.5);
                pen.line([(12.0, 6.5), (12.0, 12.0), (16.0, 12.0)], hands, w);
            }
            Icon::Tab(Tab::Alarms) => {
                body(&mut pen, (12.0, 13.0), 8.5);
                pen.line([(12.0, 9.0), (12.0, 13.0), (15.0, 15.0)], hands, w);
                pen.line([(3.5, 6.5), (7.0, 3.5)], color, w);
                pen.line([(20.5, 6.5), (17.0, 3.5)], color, w);
            }
            Icon::Tab(Tab::Watch) => {
                body(&mut pen, (12.0, 13.5), 8.5);
                pen.line([(12.0, 5.0), (12.0, 2.5)], color, w);
                pen.line([(9.5, 2.0), (14.5, 2.0)], color, w);
                pen.line([(12.0, 13.5), (15.0, 10.5)], hands, w);
            }
            Icon::Tab(Tab::Timer) => {
                if solid.is_some() {
                    pen.disc((12.0, 12.0), 9.5, color, clear, 0.0);
                } else {
                    pen.arc((12.0, 12.0), 9.5, (30.0, 360.0), color, w);
                }
                pen.line([(12.0, 12.0), (16.5, 7.5)], hands, w);
            }
            Icon::Gear => {
                pen.disc((12.0, 12.0), 3.2, clear, color, w);
                pen.disc((12.0, 12.0), 7.0, clear, color, w);
                for i in 0..8 {
                    pen.ray((12.0, 12.0), i as f32 * 45.0, 7.0, 10.0, color, 2.6);
                }
            }
            Icon::Power => {
                pen.arc((12.0, 13.0), 8.0, (35.0, 325.0), color, w);
                pen.line([(12.0, 3.0), (12.0, 11.0)], color, w);
            }
            Icon::Minus => pen.line([(6.0, 12.0), (18.0, 12.0)], color, w),
            Icon::Plus => {
                pen.line([(6.0, 12.0), (18.0, 12.0)], color, w);
                pen.line([(12.0, 6.0), (12.0, 18.0)], color, w);
            }
        }
    })
    .class("icon")
}

/// `mm:ss.cs` (or `h:mm:ss` past an hour).
fn stopwatch(secs: f32) -> String {
    let cs = (secs * 100.0) as u32;
    let (m, s, c) = (cs / 6000, cs / 100 % 60, cs % 100);
    if m >= 60 { format!("{}:{:02}:{s:02}", m / 60, m % 60) } else { format!("{m:02}:{s:02}.{c:02}") }
}

impl Clock {
    fn new() -> Self {
        Self {
            dark: false,
            theme: 0.0,
            last: now(),
            tabs: [Tab::Clock, Tab::Watch],
            alarms: [
                Alarm { mins: 5 * 60, label: "Run club", on: true },
                Alarm { mins: 6 * 60, label: "Wake up", on: false },
                Alarm { mins: 7 * 60 + 30, label: "Stand-up", on: true },
                Alarm { mins: 22 * 60 + 45, label: "Wind down", on: false },
            ],
            watch: Watch::default(),
            timer: Dial::new(25.0 / 60.0),
            ends: None,
            rang: 0,
            frames: 0,
        }
    }

    fn toggle_watch(&mut self) {
        let now = now();
        match self.watch.since.take() {
            Some(s) => self.watch.banked += now - s,
            None => self.watch.since = Some(now),
        }
    }

    /// Lap while running, reset while stopped.
    fn lap_or_reset(&mut self) {
        if self.watch.since.is_some() {
            let e = self.watch.elapsed(now());
            self.watch.laps.push(e);
        } else {
            self.watch = Watch::default();
        }
    }

    fn run_timer(&mut self) {
        if self.ends.take().is_none() && self.timer.value > 0.0 {
            self.ends = Some(now() + self.timer.value * 3600.0);
        }
    }

    fn step_timer(&mut self, minutes: f32) {
        self.ends = None;
        self.timer.set(((self.timer.value * 60.0).round() + minutes) / 60.0);
    }

    /// The knob turned: whole minutes, and a running timer pauses.
    fn timer_turned(&mut self) {
        self.ends = None;
        self.timer.value = (self.timer.value * 60.0).round() / 60.0;
    }

    const fn set_theme(&mut self, dark: bool) {
        self.dark = dark;
    }

    /// The minutes until the next alarm that is on, from local time.
    fn next_alarm(&self, day: f32) -> Option<u32> {
        let now = (day / 60.0) as u32;
        self.alarms.iter().filter(|a| a.on).map(|a| (u32::from(a.mins) + 1440 - now) % 1440).min()
    }

    fn alarm_row(&self, card: usize, k: usize, top: f32, delay: usize, cx: &mut Context<Self>) -> Div {
        let a = &self.alarms[k];
        let (h, m) = (a.mins / 60, a.mins % 60);
        let (h12, ampm) = (if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "am" } else { "pm" });
        div().class(if a.on { "abs neu row" } else { "abs neu row off" }).css("top", rem(top))
            .css("animation-delay", format!("{}ms", 120 + delay * 70))
            .child(div().class("t").child(format!("{h12}:{m:02}")).child(span().child(ampm)))
            .child(div().class("lbl").child(a.label))
            .child(div().id(format!("tog{card}-{k}")).class(if a.on { "tog on" } else { "tog" }).child(div().class("nub"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.alarms[k].on = !this.alarms[k].on;
                    cx.notify();
                })))
    }

    fn clock_view(&self, card: usize, day: f32, pal: Pal, cx: &mut Context<Self>) -> Div {
        let face = canvas(move |b, p| {
            let mut pen = Pen::new(b, p, 300.0);
            let c = (150.0, 150.0);
            for q in 0..4 {
                pen.ray(c, q as f32 * 90.0, 124.0, 140.0, pal.mute, 2.4);
            }
            let (h, m, s) = (day / 3600.0 % 12.0, day / 60.0 % 60.0, day % 60.0);
            pen.ray(c, h * 30.0, -6.0, 52.0, pal.ink, 4.0);
            pen.ray(c, m * 6.0, -8.0, 84.0, pal.minute, 3.0);
            pen.ray(c, s * 6.0, -18.0, 118.0, pal.hand, 2.2);
            pen.disc(c, 4.5, pal.hand, Rgba::TRANSPARENT, 0.0);
        }).class("abs paint");
        let mut v = div().class("abs view")
            .child(div().class("abs neu face").child(div().class("abs neu inner")).child(face));
        for (i, k) in [0, 1].into_iter().enumerate() {
            v = v.child(self.alarm_row(card, k, 470.0 + i as f32 * 92.0, i, cx));
        }
        v
    }

    fn alarms_view(&self, card: usize, cx: &mut Context<Self>) -> Div {
        let mut v = div().class("abs view");
        for k in 0..self.alarms.len() {
            v = v.child(self.alarm_row(card, k, 140.0 + k as f32 * 92.0, k, cx));
        }
        v
    }

    fn watch_view(&self, card: usize, now: f32, pal: Pal, cx: &mut Context<Self>) -> Div {
        let e = self.watch.elapsed(now);
        let ring = canvas(move |b, p| {
            let mut pen = Pen::new(b, p, 300.0);
            let lit = e % 60.0;
            for i in 0..60 {
                let on = (i as f32) < lit;
                let long = i % 5 == 0;
                pen.ray((150.0, 150.0), i as f32 * 6.0, if long { 116.0 } else { 121.0 }, 134.0,
                        if on { pal.accent } else { pal.mute }, if long { 2.4 } else { 1.6 });
            }
            // the second's sweep, a dot running round
            pen.ray((150.0, 150.0), lit * 6.0, 140.0, 141.0, pal.accent, 6.0);
        }).class("abs paint");
        let running = self.watch.since.is_some();
        let mut v = div().class("abs view")
            .child(div().class("abs neu face").child(ring)
                .child(div().class("abs read").child(stopwatch(e))
                    .child(small().child(if running { "running" } else if e > 0.0 { "paused" } else { "ready" }))))
            .child(div().id(format!("start{card}")).class("abs neu btn").css("left", rem(34.0))
                .child(if running { "Stop" } else { "Start" })
                .on_click(cx.listener(|this, _, _, cx| { this.toggle_watch(); cx.notify(); })))
            .child(div().id(format!("lap{card}")).class(if running { "abs neu btn" } else { "abs neu btn warn" })
                .css("left", rem(198.0)).child(if running { "Lap" } else { "Reset" })
                .on_click(cx.listener(|this, _, _, cx| { this.lap_or_reset(); cx.notify(); })));
        let laps = &self.watch.laps;
        for (row, i) in (0..laps.len()).rev().take(2).enumerate() {
            let split = laps[i] - if i > 0 { laps[i - 1] } else { 0.0 };
            v = v.child(div().id(format!("lap{card}-{i}")).class("abs neu lap").css("top", rem(540.0 + row as f32 * 68.0))
                .child(b().child(format!("{}", i + 1))).child(span().class("n").child("Lap")).child(span().class("split").child(stopwatch(split))));
        }
        v
    }

    fn timer_view(&self, card: usize, pal: Pal, scale: f32, cx: &mut Context<Self>) -> Div {
        let v = self.timer.value;
        let secs = (v * 3600.0).round() as u32;
        let arc = canvas(move |b, p| {
            let mut pen = Pen::new(b, p, 320.0);
            let c = (160.0, 160.0);
            pen.arc(c, 136.0, (ARC_FROM, ARC_FROM + ARC_SWEEP), pal.track, 5.0);
            for i in 0..=12 {
                pen.ray(c, ARC_FROM + i as f32 * ARC_SWEEP / 12.0, 150.0, if i % 3 == 0 { 158.0 } else { 154.0 }, pal.mute, 1.6);
            }
            pen.glow_arc(c, 136.0, (ARC_FROM, ARC_FROM + ARC_SWEEP * v), pal.accent, 4.0, 3);
        }).class("abs paint").css("width", rem(320.0)).css("height", rem(320.0));
        let knob = div().id(format!("knob{card}")).class(if self.timer.dragging() { "abs knob grab" } else { "abs knob" })
            .child(div().class("abs gloss"))
            .child(div().class("abs rot").css("transform", format!("rotate({}deg)", ARC_FROM + ARC_SWEEP * v))
                .child(div().class("abs pip")));
        let knob = draggable(knob, cx, Axis::Up, KNOB_TRAVEL * scale, |this: &mut Self| &mut this.timer, Self::timer_turned);
        let mut dial = div().class("abs dial").child(arc).child(knob);
        if self.rang > 0 {
            dial = dial.child(div().id(format!("ring{}", self.rang)).class("abs ring"));
        }
        let running = self.ends.is_some();
        let small_btn = |id: String, kind: Icon, x: f32, y: f32| {
            div().id(id).class("abs neu round small").css("left", rem(x)).css("top", rem(y)).child(icon(kind, pal.ink, None))
        };
        div().class("abs view")
            .child(div().class("abs big").child(format!("{:02}:{:02}", secs / 60, secs % 60)))
            .child(div().class("abs hint").child(if running { "counting down · drag to pause" } else { "drag the knob · scroll to fine-tune" }))
            .child(dial)
            .child(small_btn(format!("minus{card}"), Icon::Minus, 34.0, 250.0)
                .on_click(cx.listener(|this, _, _, cx| { this.step_timer(-1.0); cx.notify(); })))
            .child(small_btn(format!("plus{card}"), Icon::Plus, 298.0, 250.0)
                .on_click(cx.listener(|this, _, _, cx| { this.step_timer(1.0); cx.notify(); })))
            .child(div().id(format!("power{card}")).class("abs neu round").css("left", rem(162.0)).css("top", rem(586.0))
                .child(icon(Icon::Power, if running { pal.accent } else { pal.ink }, None))
                .on_click(cx.listener(|this, _, _, cx| { this.run_timer(); cx.notify(); })))
    }

    fn phone(&self, card: usize, (x, y): (f32, f32), (day, now): (f32, f32), pal: Pal, scale: f32, cx: &mut Context<Self>) -> Div {
        let tab = self.tabs[card];
        let (title, sub) = match tab {
            Tab::Clock => ("Clock", self.next_alarm(day).map_or_else(|| "No alarms set".to_string(),
                                                                      |m| format!("Next alarm in {}h {:02}m", m / 60, m % 60))),
            Tab::Alarms => ("Alarms", format!("{} of {} on", self.alarms.iter().filter(|a| a.on).count(), self.alarms.len())),
            Tab::Watch => ("Stopwatch", format!("{} laps", self.watch.laps.len())),
            Tab::Timer => ("Timer", "Night knob · one hour dial".to_string()),
        };
        let view = match tab {
            Tab::Clock => self.clock_view(card, day, pal, cx),
            Tab::Alarms => self.alarms_view(card, cx),
            Tab::Watch => self.watch_view(card, now, pal, cx),
            Tab::Timer => self.timer_view(card, pal, scale, cx),
        }.id(format!("view{card}-{}", tab as u8));
        let mut tabs = div().class("abs tabs");
        for (i, t) in TABS.into_iter().enumerate() {
            let on = t == tab;
            tabs = tabs.child(div().id(format!("tab{card}-{i}")).class(if on { "neu tab on" } else { "neu tab" })
                .child(icon(Icon::Tab(t), if on { pal.accent } else { pal.mute }, on.then_some(pal.card)))
                .on_click(cx.listener(move |this, _, _, cx| if this.tabs[card] != t {
                    this.tabs[card] = t;
                    cx.notify();
                })));
        }
        div().class("abs neu phone").css("left", rem(x)).css("top", rem(y)).css("animation-delay", format!("{}ms", card * 100))
            .child(div().class("abs title").child(title))
            .child(div().class("abs sub").child(sub))
            .child(view)
            .child(div().id(format!("gear{card}")).class("abs neu round gear").child(icon(Icon::Gear, pal.mute, None))
                .on_click(cx.listener(|this, _, _, cx| { this.set_theme(!this.dark); cx.notify(); })))
            .child(tabs)
    }
}

impl Render for Clock {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        focus_root(w, &mut self.frames);
        let now = now();
        let dt = (now - self.last).clamp(0.0, 0.1);
        self.last = now;
        self.theme = approach(self.theme, if self.dark { 1.0 } else { 0.0 }, 9.0, dt);
        if let Some(end) = self.ends {
            if now >= end {
                self.ends = None;
                self.timer.value = 0.0;
                self.rang += 1;
            } else {
                self.timer.value = (end - now) / 3600.0;
            }
        }
        let pal = Pal::at(self.theme);
        let day = day_seconds();
        let fit = Fit::new(w, DESIGN);
        let side = |id: &'static str, text: &'static str, x: f32, on: bool| {
            div().id(id).class(if on { "abs side on" } else { "abs side" }).css("left", rem(x)).child(span().child(text))
        };
        let stage = div().class("abs").css("left", rem(fit.ox)).css("top", rem(fit.oy))
            .css("width", rem(DESIGN.0)).css("height", rem(DESIGN.1))
            .child(side("light", "Soft Light Style", 70.0, !self.dark)
                .on_click(cx.listener(|this, _, _, cx| { this.set_theme(false); cx.notify(); })))
            .child(side("dark", "Night Knob Style", 1110.0, self.dark)
                .on_click(cx.listener(|this, _, _, cx| { this.set_theme(true); cx.notify(); })))
            .child(self.phone(0, (190.0, 40.0), (day, now), pal, fit.scale, cx))
            .child(self.phone(1, (670.0, 70.0), (day, now), pal, fit.scale, cx));
        html().css("font-size", fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root").attr("tabindex", "0")
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                    match e.keystroke.key.as_str() {
                        "t" | "T" => this.set_theme(!this.dark),
                        "space" => this.toggle_watch(),
                        _ => return,
                    }
                    cx.notify();
                }))
                .child(div().class(if self.dark { "app dark" } else { "app" }).child(stage)))
    }
}

/// Clicks, drags and scrolls through both looks and every tab, saving frames as it goes.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        s.play("00-enter", &[0, 200, 400, 900]);
        s.click("tog0-1");
        s.play("01-toggle", &[0, 120, 240, 500]);
        s.click("start1");
        s.play("02-stopwatch", &[0, 1500, 3200]);
        s.click("lap1");
        s.play("03-lap", &[0, 120, 400, 2000]);
        s.click("lap1");
        s.play("04-lap2", &[0, 400]);
        s.click("tab0-3");
        s.play("05-timer-tab", &[0, 150, 400]);
        let (x, y) = s.centre("knob0");
        s.cx.simulate_drag(s.win, (x, y), (x, y - 120.0));
        s.play("06-knob-drag", &[0, 300]);
        s.cx.simulate_scroll(s.win, x, y, -3);
        s.play("07-knob-wheel", &[0]);
        s.click("gear0");
        s.play("08-dark", &[0, 150, 300, 600]);
        s.click("power0");
        s.play("09-run", &[0, 2000]);
        for _ in 0..60 {
            s.click("minus0");
        }
        s.click("plus0");
        s.click("power0");
        s.play("10-ring", &[0, 30000, 60200, 60600, 61400]);
        s.click("tab1-1");
        s.play("11-alarms", &[0, 400]);
        s.click("tog1-2");
        s.play("12-toggle-dark", &[0, 500]);
        s.click("tab1-0");
        s.click("light");
        s.play("13-light", &[0, 250, 700]);
    })
}

fn main() {
    let (w, h) = size_arg(DESIGN);
    run_with(w, h, "BUI · Clock", Clock::new, arg("--storyboard").map(storyboard));
}

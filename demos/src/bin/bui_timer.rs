//! BUI Timer: a clock app built from Blender's own UI, for Blender users who want to see what it can
//! do outside Blender. Everything is real: Blender's screen (areas you split, join, swap, maximize
//! and switch the editor of, with workspaces on top that remember their layouts), its header,
//! toolbar and N sidebar regions, its panels, and its widgets doing what they do in Blender
//! (number fields you drag to scrub or double-click to type `25*60` into, sliders, checkboxes, the
//! color picker, dropdowns, progress bars).
//!
//! Editors (any area can show any of them):
//! - World Clock: analog faces (`Painter`) for a list of cities by fixed UTC offset, your home zone
//!   a number field; add from a dropdown, rename and re-zone in the sidebar.
//! - Alarms: hour and minute fields to scrub, weekday checkboxes, a toggle each; one rings in the
//!   top bar (snooze, dismiss), as a desktop notification and a chime.
//! - Timers: as many as you like, each a ring and a progress bar; presets, a duration field that
//!   takes math (`2*60+30`), labels; the soonest shows on the taskbar button.
//! - Stopwatch: centiseconds, laps in a table with the best and worst marked, laps as CSV.
//! - Pomodoro: work / break sliders, cycles, and Focus: Blender's Ctrl+Alt+Space on its own area.
//! - Settings: chime, volume, 24-hour time, seconds, the accent colour (it recolours the app).
//!
//! Keys (as Blender's: single keys, quiet while a text field is typed in): Space start / stop the
//! stopwatch, L lap, R reset it; P start / pause the pomodoro; T a new timer from the form;
//! Ctrl+PageUp / Ctrl+PageDown workspaces; Esc dismisses a ringing alarm and leaves focus;
//! N toggles an area's sidebar (over the area), Ctrl+Space maximizes it.
//!
//! `cargo run --release --bin bui_timer`; `-- --storyboard DIR` drives it headlessly (the clock is
//! then a fixed morning, and nothing reaches the desktop: no notification, tray or sound).

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use blenderui::prelude::*;
use blenderui::screen::{AreaRef, AreaSplit, Editor, Editors, Region, Screen, ScreenEvent};
use blenderui::shell::{self, TaskbarProgress, Tray};
use blenderui::Sound;
use bui_demos::pen::Pen;
use bui_demos::story::{story, Story};
use bui_demos::{arg, focus_root, now, run_with, size_arg, Script};
use bui_demos::act;
use bui_demos::color::parse_hex;

const CSS: &str = r"
html, body { height: 100%; }
body { display: flex; flex-direction: column; font-size: 13px; color: #e6e6e6; -webkit-font-smoothing: antialiased;
       --accent: #4772b3; }
.top { height: 30px; flex-shrink: 0; display: flex; align-items: center; gap: 2px; padding: 0 8px; background: #232323;
       border-bottom: 1px solid #161616; }
.brand { font-weight: 700; margin-right: 10px; color: #f0f0f0; letter-spacing: 0.2px; }
.tab { padding: 4px 12px; border-radius: 4px; color: #b8b8b8; cursor: pointer;
       transition-property: background-color, color; transition-duration: 120ms; }
.tab:hover { background: #3a3a3a; color: #fff; }
.tab.on { background: #4a4a4a; color: #fff; }
.spacer { flex: 1; }
.ring { display: flex; align-items: center; gap: 8px; padding: 2px 4px 2px 10px; border-radius: 4px; background: #6b2d2d;
        animation: ringing 900ms ease-in-out infinite alternate; }
.ring b { font-variant-numeric: tabular-nums; }
.screen { flex: 1; min-height: 0; }
.status { height: 22px; flex-shrink: 0; display: flex; align-items: center; gap: 14px; padding: 0 10px; background: #232323;
          border-top: 1px solid #161616; color: #9a9a9a; font-size: 11px; }
.status b { color: #d6d6d6; font-weight: 600; font-variant-numeric: tabular-nums; }
kbd { font-size: 10px; }

/* regions */
.hdr { height: 100%; display: flex; align-items: center; gap: 4px; padding: 0 6px; background: #2b2b2b; }
.hdr .title { font-weight: 600; margin-right: 6px; }
.hdr .sep { width: 1px; height: 16px; background: #444; margin: 0 4px; }
.side { height: 100%; overflow-y: auto; background: #2b2b2b; padding: 4px; box-sizing: border-box; }
.side details { margin-bottom: 4px; }
.side .row { display: flex; align-items: center; gap: 6px; margin: 5px 4px; }
.side .row > span { width: 72px; color: #a8a8a8; flex-shrink: 0; }
.side .days { display: flex; flex-wrap: wrap; gap: 4px 10px; margin: 4px; }
.side .days label, .row label { display: flex; align-items: center; gap: 4px; }
.side .hint { color: #8a8a8a; font-size: 11px; margin: 6px 4px; text-wrap: pretty; }
.main { height: 100%; overflow-y: auto; box-sizing: border-box; padding: 12px; background: #303030; }
.empty { color: #8a8a8a; text-align: center; margin-top: 40px; }

/* world clock */
.clocks { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 10px; }
.city { padding: 10px; border-radius: 8px; background: #3a3a3a; cursor: pointer; text-align: center;
        box-shadow: 0 1px 2px rgba(0,0,0,0.3); transition-property: background-color, box-shadow; transition-duration: 120ms; }
.city:hover { background: #424242; }
.city.sel { box-shadow: 0 0 0 2px var(--accent), 0 1px 2px rgba(0,0,0,0.3); }
.city .face { width: 120px; height: 120px; margin: 0 auto 6px; }
.city .name { font-weight: 600; text-wrap: balance; }
.city .t { font-size: 20px; font-weight: 300; font-variant-numeric: tabular-nums; }
.city .d { color: #9a9a9a; font-size: 11px; font-variant-numeric: tabular-nums; }
.city.night { background: #2c3140; }

/* alarms */
.alarm { display: flex; align-items: center; gap: 12px; padding: 8px 12px; margin-bottom: 6px; border-radius: 6px;
         background: #3a3a3a; cursor: pointer; transition-property: background-color, opacity; transition-duration: 120ms; }
.alarm:hover { background: #424242; }
.alarm.sel { box-shadow: inset 3px 0 0 var(--accent); background: #444; }
.alarm.off { opacity: 0.55; }
.alarm .when { font-size: 26px; font-weight: 300; width: 110px; font-variant-numeric: tabular-nums; }
.alarm .when small { font-size: 12px; margin-left: 3px; color: #a8a8a8; }
.alarm .what { flex: 1; min-width: 0; }
.alarm .what div:first-child { font-weight: 600; }
.alarm .what div + div { color: #9a9a9a; font-size: 11px; }
.next { color: #a8a8a8; margin: 0 2px 10px; font-variant-numeric: tabular-nums; }

/* timers */
.timers { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 10px; }
.timer { padding: 10px; border-radius: 8px; background: #3a3a3a; text-align: center; cursor: pointer;
         transition-property: box-shadow, background-color; transition-duration: 120ms; }
.timer.sel { box-shadow: 0 0 0 2px var(--accent); }
.timer.done { animation: done 700ms ease-in-out infinite alternate; }
.timer .dial { width: 140px; height: 140px; margin: 0 auto; position: relative; }
.timer .left { position: absolute; left: 0; right: 0; top: 50px; font-size: 24px; font-weight: 300; font-variant-numeric: tabular-nums; }
.timer .lbl { position: absolute; left: 0; right: 0; top: 84px; color: #a8a8a8; font-size: 11px; }
.timer .btns { display: flex; gap: 4px; justify-content: center; margin-top: 8px; }
.timer progress { width: 100%; margin-top: 8px; }
.new { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; margin-bottom: 12px; padding: 8px; border-radius: 6px; background: #2b2b2b; }
.new .read { width: 64px; text-align: right; font-variant-numeric: tabular-nums; color: #cfcfcf; }

/* stopwatch */
.sw { display: flex; gap: 16px; align-items: flex-start; flex-wrap: wrap; }
.sw .dial { width: 200px; height: 200px; flex-shrink: 0; }
.sw .big { font-size: 44px; font-weight: 200; font-variant-numeric: tabular-nums; letter-spacing: -1px; }
.sw .ctl { display: flex; gap: 6px; margin: 8px 0 12px; }
.laps { width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }
.laps td, .laps th { padding: 4px 8px; text-align: right; }
.laps th { color: #9a9a9a; font-weight: 400; }
.laps td:first-child, .laps th:first-child { text-align: left; }
.laps tr.best td { color: #7fd17f; }
.laps tr.worst td { color: #e88080; }
.laps tr.fresh { animation: lapin 260ms cubic-bezier(0.2, 0, 0, 1); }

/* pomodoro */
.pomo { display: flex; flex-direction: column; align-items: center; gap: 8px; }
.pomo .dial { width: 170px; height: 170px; position: relative; flex-shrink: 0; }
.pomo .phase { position: absolute; left: 0; right: 0; top: 52px; text-align: center; text-transform: uppercase; letter-spacing: 2px; font-size: 11px; color: #b8b8b8; }
.pomo .left { position: absolute; left: 0; right: 0; top: 68px; text-align: center; font-size: 32px; font-weight: 200; font-variant-numeric: tabular-nums; }
.pomo .dots { display: flex; gap: 6px; }
.pomo .dot { width: 10px; height: 10px; border-radius: 5px; background: #4a4a4a; transition-property: background-color; transition-duration: 200ms; }
.pomo .dot.on { background: var(--accent); }
.pomo .ctl { display: flex; flex-wrap: wrap; justify-content: center; gap: 6px; }
.pomo.focus { background: #1c1c1c; }
.pomo .stats { width: 100%; max-width: 260px; color: #a8a8a8; font-size: 11px; font-variant-numeric: tabular-nums; }
.pomo .stats progress { width: 100%; }

/* settings */
.set { max-width: 420px; }
.set .row { display: flex; align-items: center; gap: 8px; margin: 8px 0; }
.set .row > span { width: 110px; color: #a8a8a8; flex-shrink: 0; }
.swatch-preview { height: 6px; border-radius: 3px; background: var(--accent); margin: 10px 0; }

@keyframes ringing { from { background-color: #6b2d2d; } to { background-color: #9a3a3a; } }
@keyframes done { from { background-color: #3a3a3a; } to { background-color: #5a3434; } }
@keyframes lapin { from { opacity: 0; transform: translateY(-6px); } to { opacity: 1; transform: translateY(0); } }
";

/// Fixed-offset places (no daylight time: the offsets are the standard ones).
const PLACES: [(&str, f32); 22] = [
    ("Honolulu", -10.0), ("Anchorage", -9.0), ("Vancouver", -8.0), ("Denver", -7.0), ("Chicago", -6.0),
    ("New York", -5.0), ("Halifax", -4.0), ("São Paulo", -3.0), ("Reykjavík", 0.0), ("Lisbon", 0.0),
    ("Berlin", 1.0), ("Cairo", 2.0), ("Nairobi", 3.0), ("Dubai", 4.0), ("Mumbai", 5.5), ("Kathmandu", 5.75),
    ("Dhaka", 6.0), ("Bangkok", 7.0), ("Singapore", 8.0), ("Tokyo", 9.0), ("Sydney", 10.0), ("Auckland", 12.0),
];
const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const SOUNDS: [&str; 4] = ["Chime", "Bell", "Digital", "Silent"];
const PRESETS: [(&str, f32); 6] = [("1 min", 60.0), ("3 min", 180.0), ("5 min", 300.0), ("10 min", 600.0), ("25 min", 1500.0), ("1 h", 3600.0)];
const WORKSPACES: [&str; 4] = ["Timers", "Clocks", "Focus", "Settings"];

/// The screen each workspace starts with (the user's own changes are kept per workspace).
fn workspace_layout(ws: usize) -> AreaSplit {
    match ws {
        0 => AreaSplit::v(0.6, "Timers", AreaSplit::h(0.52, "Stopwatch", "Pomodoro")),
        1 => AreaSplit::v(0.58, "World Clock", "Alarms"),
        2 => AreaSplit::v(0.62, "Pomodoro", "Timers"),
        _ => AreaSplit::v(0.5, "Settings", "Alarms"),
    }
}

// ---------------------------------------------------------------------------------------------
// The model

#[derive(Clone)]
struct City {
    id: u32,
    name: String,
    offset: f32,
}

#[derive(Clone)]
struct Alarm {
    id: u32,
    label: String,
    hour: u32,
    minute: u32,
    days: [bool; 7],
    on: bool,
    /// The day number (since 1970, home time) it last rang, so it rings once a day.
    rang: i64,
    /// Snoozed until this wall second.
    snooze_until: Option<f64>,
}

#[derive(Clone)]
struct Timer {
    id: u32,
    label: String,
    total: f32,
    /// Seconds run before the current run, and when the current run began (`now()`).
    before: f32,
    since: Option<f32>,
    done: bool,
}

impl Timer {
    fn run(&self, t: f32) -> f32 {
        self.before + self.since.map_or(0.0, |s| t - s)
    }

    fn left(&self, t: f32) -> f32 {
        (self.total - self.run(t)).max(0.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Work,
    Short,
    Long,
}

impl Phase {
    const fn name(self) -> &'static str {
        match self {
            Self::Work => "Focus",
            Self::Short => "Short break",
            Self::Long => "Long break",
        }
    }
}

#[derive(Clone)]
struct Pomodoro {
    work: f32,
    short: f32,
    long: f32,
    cycles: u32,
    auto: bool,
    phase: Phase,
    done_cycles: u32,
    before: f32,
    since: Option<f32>,
    /// Seconds of work today, and the daily goal (minutes).
    focused: f32,
    goal: f32,
}

impl Pomodoro {
    fn length(&self) -> f32 {
        60.0 * match self.phase {
            Phase::Work => self.work,
            Phase::Short => self.short,
            Phase::Long => self.long,
        }
    }

    fn run(&self, t: f32) -> f32 {
        self.before + self.since.map_or(0.0, |s| t - s)
    }
}

#[derive(Clone, Default)]
struct Stopwatch {
    before: f32,
    since: Option<f32>,
    /// Each lap's total at its end.
    laps: Vec<f32>,
}

impl Stopwatch {
    fn run(&self, t: f32) -> f32 {
        self.before + self.since.map_or(0.0, |s| t - s)
    }
}

#[derive(Clone)]
struct Settings {
    sound: usize,
    volume: f64,
    h24: bool,
    seconds: bool,
    accent: String,
    home: f32,
}

/// Everything the editors draw: cloned into them each render (an editor's factory owns its copy).
#[derive(Clone)]
struct State {
    cities: Vec<City>,
    city_sel: Option<u32>,
    alarms: Vec<Alarm>,
    alarm_sel: Option<u32>,
    timers: Vec<Timer>,
    timer_sel: Option<u32>,
    /// The new-timer form.
    form_secs: f32,
    form_label: String,
    sw: Stopwatch,
    pomo: Pomodoro,
    set: Settings,
    next_id: u32,
    /// The UTC second `now()` 0 is.
    base: f64,
    /// The alarm ringing, and since when.
    ringing: Option<(u32, f32)>,
    /// A focused area (Pomodoro's Focus): restored on Esc.
    focus: Option<AreaRef>,
}

impl State {
    fn wall(&self, t: f32) -> f64 {
        self.base + f64::from(t)
    }

    /// Seconds into the day at UTC offset `offset` hours, and the day number.
    fn local(&self, t: f32, offset: f32) -> (f64, i64) {
        let s = self.wall(t) + f64::from(offset) * 3600.0;
        (s.rem_euclid(86_400.0), s.div_euclid(86_400.0) as i64)
    }

    const fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    fn accent(&self) -> Rgba {
        rgb(parse_hex(&self.set.accent).unwrap_or(0x4772b3))
    }

    /// The next alarm to ring, as (seconds from now, alarm).
    fn next_alarm(&self, t: f32) -> Option<(f64, &Alarm)> {
        let (sec, day) = self.local(t, self.set.home);
        self.alarms.iter().filter(|a| a.on && a.days.iter().any(|d| *d)).filter_map(|a| {
            let at = f64::from(a.hour * 3600 + a.minute * 60);
            (0..8).find_map(|k| {
                let wd = weekday(day + k);
                let delta = at - sec + 86_400.0 * k as f64;
                (a.days[wd] && delta > 0.0 && !(k == 0 && a.rang == day)).then_some((delta, a))
            })
        }).min_by(|x, y| x.0.total_cmp(&y.0))
    }
}

/// Monday 0 .. Sunday 6 of a day number (1970-01-01 was a Thursday).
const fn weekday(day: i64) -> usize {
    (day + 3).rem_euclid(7) as usize
}

fn two(v: u32) -> String {
    format!("{v:02}")
}

/// `h:mm:ss` / `m:ss` for a count down, rounded up (a timer reads 0:01 until it is done).
fn clock_left(s: f32) -> String {
    let s = s.ceil().max(0.0) as u32;
    if s >= 3600 { format!("{}:{}:{}", s / 3600, two(s / 60 % 60), two(s % 60)) } else { format!("{}:{}", s / 60, two(s % 60)) }
}

/// `mm:ss.cc` (and hours when there are any) for a count up.
fn clock_up(s: f32) -> String {
    let cs = (s.max(0.0) * 100.0) as u32;
    let (h, m, sec, c) = (cs / 360_000, cs / 6000 % 60, cs / 100 % 60, cs % 100);
    if h > 0 { format!("{h}:{}:{}.{}", two(m), two(sec), two(c)) } else { format!("{}:{}.{}", two(m), two(sec), two(c)) }
}

/// A time of day for people: `14:05` or `2:05 PM` (with seconds when asked).
fn time_of_day(sec: f64, set: &Settings) -> (String, &'static str) {
    let s = sec as u32;
    let (h, m, ss) = (s / 3600, s / 60 % 60, s % 60);
    let tail = if set.seconds { format!(":{}", two(ss)) } else { String::new() };
    if set.h24 {
        (format!("{}:{}{tail}", two(h), two(m)), "")
    } else {
        let h12 = if h % 12 == 0 { 12 } else { h % 12 };
        (format!("{h12}:{}{tail}", two(m)), if h < 12 { "AM" } else { "PM" })
    }
}

fn offset_text(o: f32) -> String {
    let m = (o * 60.0).round() as i32;
    let (sign, m) = if m < 0 { ('−', -m) } else { ('+', m) };
    if m % 60 == 0 { format!("UTC{sign}{}", m / 60) } else { format!("UTC{sign}{}:{}", m / 60, two((m % 60) as u32)) }
}

// ---------------------------------------------------------------------------------------------
// Sound: chimes made at start (a WAV each in the temp folder), played through BUI's `Sound`

fn chime_path(kind: usize) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("bui_timer_{}.wav", SOUNDS[kind].to_lowercase()))
}

/// Writes the chimes once: sine partials with an exponential decay, 16-bit mono 22 kHz.
fn write_chimes() {
    const RATE: u32 = 22_050;
    for kind in 0..3 {
        let path = chime_path(kind);
        if path.exists() {
            continue;
        }
        let notes: &[(f32, f32)] = match kind {
            0 => &[(880.0, 0.0), (1318.5, 0.18), (1760.0, 0.36)],
            1 => &[(523.25, 0.0), (1046.5, 0.0), (1568.0, 0.0)],
            _ => &[(2000.0, 0.0), (2000.0, 0.12), (2000.0, 0.24), (2000.0, 0.36)],
        };
        let len = (RATE as f32 * 1.4) as usize;
        let mut pcm: Vec<u8> = Vec::with_capacity(len * 2);
        for i in 0..len {
            let t = i as f32 / RATE as f32;
            let v: f32 = notes.iter().map(|&(f, at)| {
                let dt = t - at;
                if dt < 0.0 { 0.0 } else if kind == 2 { if dt < 0.07 { (dt * f * std::f32::consts::TAU).sin().signum() * 0.25 } else { 0.0 } }
                else { (dt * f * std::f32::consts::TAU).sin() * (-dt * 4.0).exp() * 0.3 }
            }).sum();
            pcm.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32_000.0) as i16).to_le_bytes());
        }
        let mut wav = Vec::with_capacity(44 + pcm.len());
        let n = pcm.len() as u32;
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + n).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        for v in [1u16, 1] {
            wav.extend_from_slice(&v.to_le_bytes());
        }
        for v in [RATE, RATE * 2] {
            wav.extend_from_slice(&v.to_le_bytes());
        }
        for v in [2u16, 16] {
            wav.extend_from_slice(&v.to_le_bytes());
        }
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&n.to_le_bytes());
        wav.extend_from_slice(&pcm);
        let _ = std::fs::write(&path, wav);
    }
}

// ---------------------------------------------------------------------------------------------
// The app

struct TimerApp {
    st: State,
    ws: usize,
    /// Each workspace's screen as the user left it.
    saved: [Option<String>; WORKSPACES.len()],
    /// A storyboard: nothing reaches the desktop.
    headless: bool,
    tray: Option<Tray>,
    sound: Option<Sound>,
    /// What the desktop shows already (the tray tooltip, the taskbar), so it is told only changes.
    shown_tip: String,
    shown_bar: Option<(u8, u32)>,
    /// The accent the theme has (see `theme_accent`).
    themed: String,
    frames: u32,
}

impl TimerApp {
    fn new(headless: bool) -> Self {
        // a storyboard runs on a fixed morning: 2026-11-26 08:58:40 UTC
        let base = if headless {
            1_795_683_520.0
        } else {
            SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64()) - f64::from(now())
        };
        let mut st = State {
            cities: Vec::new(),
            city_sel: None,
            alarms: Vec::new(),
            alarm_sel: None,
            timers: Vec::new(),
            timer_sel: None,
            form_secs: 300.0,
            form_label: String::from("Tea"),
            sw: Stopwatch::default(),
            pomo: Pomodoro { work: 25.0, short: 5.0, long: 15.0, cycles: 4, auto: true, phase: Phase::Work, done_cycles: 0,
                             before: 0.0, since: None, focused: 0.0, goal: 120.0 },
            set: Settings { sound: 0, volume: 70.0, h24: true, seconds: true, accent: String::from("#4772b3"), home: 1.0 },
            next_id: 0,
            base,
            ringing: None,
            focus: None,
        };
        for (name, offset) in [("Home", 1.0), ("New York", -5.0), ("Mumbai", 5.5), ("Tokyo", 9.0)] {
            let id = st.id();
            st.cities.push(City { id, name: name.into(), offset });
        }
        st.city_sel = st.cities.first().map(|c| c.id);
        for (label, hour, minute, days, on) in [("Wake up", 7, 0, [true, true, true, true, true, false, false], true),
                                                ("Stand-up", 9, 30, [true, true, true, true, true, false, false], true),
                                                ("Long run", 8, 0, [false, false, false, false, false, true, false], false)] {
            let id = st.id();
            st.alarms.push(Alarm { id, label: label.into(), hour, minute, days, on, rang: -1, snooze_until: None });
        }
        st.alarm_sel = st.alarms.first().map(|a| a.id);
        if !headless {
            write_chimes();
        }
        let tray = (!headless).then(|| Tray::new(Some("TIME"), "BUI Timer")).flatten();
        Self { st, ws: 0, saved: Default::default(), headless, tray, sound: None, shown_tip: String::new(),
                   shown_bar: None, themed: String::new(), frames: 0 }
    }

    // -- actions ------------------------------------------------------------------------------

    fn chime(&mut self) {
        let kind = self.st.set.sound;
        if self.headless || kind >= 3 {
            return;
        }
        self.sound = Sound::new(&chime_path(kind).to_string_lossy());
        if let Some(s) = &self.sound {
            s.set_volume(self.st.set.volume / 100.0);
        }
    }

    fn tell(&self, title: &str, body: &str) {
        if !self.headless {
            let _ = shell::notify(title, body, Some("TIME"));
        }
    }

    fn add_timer(&mut self) {
        let id = self.st.id();
        let label = if self.st.form_label.trim().is_empty() { format!("Timer {}", self.st.timers.len() + 1) } else { self.st.form_label.clone() };
        self.st.timers.push(Timer { id, label, total: self.st.form_secs.max(1.0), before: 0.0, since: Some(now()), done: false });
        self.st.timer_sel = Some(id);
    }

    fn timer(&mut self, id: u32) -> Option<&mut Timer> {
        self.st.timers.iter_mut().find(|x| x.id == id)
    }

    fn toggle_timer(&mut self, id: u32) {
        let t = now();
        if let Some(x) = self.timer(id) {
            match x.since.take() {
                Some(s) => x.before += t - s,
                None if x.done => *x = Timer { before: 0.0, since: Some(t), done: false, ..x.clone() },
                None => x.since = Some(t),
            }
        }
    }

    fn reset_timer(&mut self, id: u32) {
        if let Some(x) = self.timer(id) {
            *x = Timer { before: 0.0, since: None, done: false, ..x.clone() };
        }
    }

    fn sw_toggle(&mut self) {
        let t = now();
        let sw = &mut self.st.sw;
        match sw.since.take() {
            Some(s) => sw.before += t - s,
            None => sw.since = Some(t),
        }
    }

    fn sw_lap(&mut self) {
        let t = now();
        if self.st.sw.since.is_some() {
            let total = self.st.sw.run(t);
            self.st.sw.laps.push(total);
        }
    }

    fn sw_reset(&mut self) {
        self.st.sw = Stopwatch::default();
    }

    fn pomo_toggle(&mut self) {
        let t = now();
        let p = &mut self.st.pomo;
        match p.since.take() {
            Some(s) => p.before += t - s,
            None => p.since = Some(t),
        }
    }

    /// The phase is over (or skipped): the next one, running when `auto`.
    fn pomo_next(&mut self, finished: bool) {
        let t = now();
        let p = &mut self.st.pomo;
        if p.phase == Phase::Work {
            if finished {
                p.focused += p.length();
            }
            p.done_cycles += 1;
            p.phase = if p.done_cycles.is_multiple_of(p.cycles) { Phase::Long } else { Phase::Short };
        } else {
            p.phase = Phase::Work;
        }
        p.before = 0.0;
        p.since = (p.auto && finished).then_some(t);
    }

    fn workspace(&mut self, ws: usize) {
        self.ws = ws % WORKSPACES.len();
        self.leave_focus();
    }

    fn leave_focus(&mut self) {
        if let Some(a) = self.st.focus.take() {
            a.restore();
        }
    }

    fn key(&mut self, e: &KeyDownEvent) -> bool {
        let k = &e.keystroke;
        let ctrl = k.modifiers.control;
        match k.key.to_ascii_lowercase().as_str() {
            "space" if !ctrl => self.sw_toggle(),
            "l" if !ctrl => self.sw_lap(),
            "r" if !ctrl => self.sw_reset(),
            "p" if !ctrl => self.pomo_toggle(),
            "t" if !ctrl => self.add_timer(),
            "prior" if ctrl => self.workspace(self.ws + WORKSPACES.len() - 1),
            "next" if ctrl => self.workspace(self.ws + 1),
            "escape" => {
                if self.st.ringing.take().is_none() {
                    self.leave_focus();
                }
            }
            _ => return false,
        }
        true
    }

    /// Time moves things: timers end, alarms ring, the pomodoro turns. Before each render.
    fn tick(&mut self, t: f32) {
        let mut ended: Vec<String> = Vec::new();
        for x in &mut self.st.timers {
            if x.since.is_some() && x.left(t) <= 0.0 {
                x.before = x.total;
                x.since = None;
                x.done = true;
                ended.push(x.label.clone());
            }
        }
        for label in ended {
            self.chime();
            self.tell("Timer done", &label);
        }
        if self.st.pomo.since.is_some() && self.st.pomo.run(t) >= self.st.pomo.length() {
            let was = self.st.pomo.phase;
            self.pomo_next(true);
            self.chime();
            self.tell(&format!("{} over", was.name()), &format!("Next: {}", self.st.pomo.phase.name()));
        }
        let (sec, day) = self.st.local(t, self.st.set.home);
        let wall = self.st.wall(t);
        let wd = weekday(day);
        let mut ring = None;
        for a in &mut self.st.alarms {
            let due = f64::from(a.hour * 3600 + a.minute * 60);
            let snoozed = a.snooze_until.is_some_and(|s| wall >= s);
            if (a.on && a.days[wd] && a.rang != day && sec >= due && sec < due + 60.0) || snoozed {
                a.rang = day;
                a.snooze_until = None;
                ring = Some((a.id, a.label.clone()));
            }
        }
        if let Some((id, label)) = ring {
            self.st.ringing = Some((id, t));
            self.chime();
            self.tell("Alarm", &label);
        }
    }

    /// The desktop: the tray's tooltip and the taskbar button follow the soonest timer.
    fn desktop(&mut self, w: &Window, t: f32) {
        if self.headless {
            return;
        }
        let soonest = self.st.timers.iter().filter(|x| x.since.is_some()).min_by(|a, b| a.left(t).total_cmp(&b.left(t)));
        let tip = soonest.map_or_else(|| String::from("BUI Timer"), |x| format!("{}: {} left", x.label, clock_left(x.left(t))));
        if tip != self.shown_tip {
            if let Some(tray) = &self.tray {
                tray.set_tooltip(&tip);
            }
            self.shown_tip = tip;
        }
        let bar = soonest.map(|x| (1, (x.run(t) / x.total * 100.0) as u32));
        if bar != self.shown_bar {
            let p = bar.map_or(TaskbarProgress::Hidden, |(_, v)| TaskbarProgress::Normal(v as f32 / 100.0));
            w.set_taskbar_progress(p);
            self.shown_bar = bar;
        }
    }

    /// The accent reaches Blender's own widgets through the theme (`accent-color` doesn't: it
    /// parses, and no widget reads it): slider and progress fills, and the selected states.
    fn theme_accent(&mut self) {
        if self.themed == self.st.set.accent {
            return;
        }
        let c = self.st.accent();
        for key in ["slider.fill", "progress_bar.fill", "primary.background", "checkbox.background.selected"] {
            blenderui::set_theme_color(key, [c.r, c.g, c.b, 1.0]);
        }
        self.themed.clone_from(&self.st.set.accent);
    }

}

type Weak = WeakEntity<TimerApp>;



// ---------------------------------------------------------------------------------------------
// Drawing

fn face(st: &State, offset: f32, t: f32, size: f32) -> Div {
    let (sec, _) = st.local(t, offset);
    let accent = st.accent();
    let night = !(6.0 * 3600.0..20.0 * 3600.0).contains(&sec);
    canvas(move |b, p| {
        let mut pen = Pen::new(b, p, 100.0);
        let c = (50.0, 50.0);
        let (bg, ink) = if night { (rgb(0x1f2330), rgb(0xd8dcf0)) } else { (rgb(0xe9e9e9), rgb(0x262626)) };
        pen.disc(c, 48.0, bg, rgb(0x1a1a1a), 1.0);
        for k in 0..60 {
            let long = k % 5 == 0;
            pen.ray(c, k as f32 * 6.0, if long { 39.0 } else { 42.0 }, 45.0, Rgba { a: if long { 0.9 } else { 0.35 }, ..ink }, if long { 1.6 } else { 0.8 });
        }
        let s = sec as f32;
        pen.ray(c, (s / 3600.0 % 12.0) * 30.0, -4.0, 24.0, ink, 3.4);
        pen.ray(c, s / 60.0 % 60.0 * 6.0, -4.0, 34.0, ink, 2.4);
        pen.ray(c, s.floor() % 60.0 * 6.0, -8.0, 38.0, accent, 1.2);
        pen.disc(c, 2.6, accent, Rgba::TRANSPARENT, 0.0);
    }).class("face").css("width", format!("{size}px")).css("height", format!("{size}px"))
}

/// A ring that empties as `frac` (0..1) is used: a track and the rest, glowing in the accent.
fn ring_dial(frac: f32, color: Rgba, size: f32) -> Div {
    canvas(move |b, p| {
        let mut pen = Pen::new(b, p, 100.0);
        pen.arc((50.0, 50.0), 44.0, (0.0, 360.0), rgb(0x262626), 6.0);
        let to = 360.0 * (1.0 - frac.clamp(0.0, 1.0));
        if to > 0.5 {
            pen.glow_arc((50.0, 50.0), 44.0, (0.0, to), color, 6.0, 2);
        }
    }).css("position", "absolute").css("left", "0").css("top", "0").css("width", format!("{size}px")).css("height", format!("{size}px"))
}

// ---------------------------------------------------------------------------------------------
// The editors: each builds from its own copy of the state, and talks back through `weak`

fn world_clock(st: &State, weak: Weak, t: f32, n: u32) -> Editor {
    let id = |s: &str| format!("wc{n}-{s}");
    let mut grid = div().class("clocks");
    for c in &st.cities {
        let (sec, day) = st.local(t, c.offset);
        let (home_sec, home_day) = st.local(t, st.set.home);
        let (time, ampm) = time_of_day(sec, &st.set);
        let diff = (c.offset - st.set.home) * 1.0;
        let rel = match day - home_day {
            0 => "Today",
            1 => "Tomorrow",
            _ => "Yesterday",
        };
        let _ = home_sec;
        let cid = c.id;
        let night = !(6.0 * 3600.0..20.0 * 3600.0).contains(&sec);
        grid = grid.child(div().id(id(&format!("city{cid}")))
            .class(match (st.city_sel == Some(cid), night) {
                (true, true) => "city sel night",
                (true, false) => "city sel",
                (false, true) => "city night",
                (false, false) => "city",
            })
            .on_click(act(weak, move |this, _: &ClickEvent| this.st.city_sel = Some(cid)))
            .child(face(st, c.offset, t, 120.0))
            .child(div().class("name").child(c.name.clone()))
            .child(div().class("t").child(format!("{time} {ampm}")))
            .child(div().class("d").child(format!("{rel} · {}{}h · {}", if diff >= 0.0 { "+" } else { "−" }, fmt_hours(diff.abs()), offset_text(c.offset)))));
    }
    let mut add = Dropdown().id(id("add")).on_change(act(weak, |this, e: &ChangeEvent| {
        let k = e.number as usize;
        if k > 0 && let Some(&(name, offset)) = PLACES.get(k - 1) {
            let id = this.st.id();
            this.st.cities.push(City { id, name: name.into(), offset });
            this.st.city_sel = Some(id);
        }
    })).child(option().child("Add City…"));
    for (name, o) in PLACES {
        add = add.child(option().child(format!("{name} ({})", offset_text(o))));
    }
    let header = div().class("hdr").child(span().class("title").child("World Clock"))
        .child(add)
        .child(div().class("sep"))
        .child(span().child("Home"))
        .child(NumberField().id(id("home")).min(-12.0).max(14.0).step(0.25).value(format!("{}", st.set.home))
            .on_change(act(weak, |this, e: &ChangeEvent| this.st.set.home = e.number as f32)));
    let sel = st.cities.iter().find(|c| Some(c.id) == st.city_sel).cloned();
    let side = match sel {
        Some(c) => {
            let cid = c.id;
            div().class("side")
                .child(details().open(true).child(summary().child("City"))
                    .child(div().class("row").child(span().child("Name"))
                        .child(TextField().id(id("name")).value(c.name.clone())
                            .on_change(act(weak, move |this, e: &ChangeEvent| {
                                if let Some(c) = this.st.cities.iter_mut().find(|c| c.id == cid) {
                                    c.name.clone_from(&e.value);
                                }
                            }))))
                    .child(div().class("row").child(span().child("UTC offset"))
                        .child(NumberField().id(id("off")).min(-12.0).max(14.0).step(0.25).value(format!("{}", c.offset))
                            .on_change(act(weak, move |this, e: &ChangeEvent| {
                                if let Some(c) = this.st.cities.iter_mut().find(|c| c.id == cid) {
                                    c.offset = e.number as f32;
                                }
                            }))))
                    .child(div().class("row").child(Button().id(id("remove")).child("Remove City")
                        .on_click(act(weak, move |this, _: &ClickEvent| {
                            this.st.cities.retain(|c| c.id != cid);
                            this.st.city_sel = this.st.cities.first().map(|c| c.id);
                        })))))
                .child(details().open(true).child(summary().child("Display"))
                    .child(div().class("row").child(Checkbox().id(id("h24")).checked(st.set.h24)
                        .on_change(act(weak, |this, e: &ChangeEvent| this.st.set.h24 = e.checked))).child("24-hour time"))
                    .child(div().class("row").child(Checkbox().id(id("secs")).checked(st.set.seconds)
                        .on_change(act(weak, |this, e: &ChangeEvent| this.st.set.seconds = e.checked))).child("Show seconds")))
                .child(div().class("hint").child("Offsets are fixed (standard time, no daylight saving). Drag a number to scrub it, double-click to type."))
        }
        None => div().class("side").child(div().class("hint").child("Select a city.")),
    };
    let main = if st.cities.is_empty() { div().class("main").child(div().class("empty").child("No cities: add one from the header.")) }
               else { div().class("main").child(grid) };
    Editor::new(main).header(header).sidebar(Region::new(side).size(250).overlap(false).hotkey("n"))
}

fn fmt_hours(h: f32) -> String {
    if h.fract().abs() < 1e-3 { format!("{h:.0}") } else { format!("{h}") }
}

fn alarms(st: &State, weak: Weak, t: f32, n: u32) -> Editor {
    let id = |s: &str| format!("al{n}-{s}");
    let mut list = div();
    let next = st.next_alarm(t);
    list = list.child(div().class("next").child(match next {
        Some((d, a)) => format!("Next: {} in {}h {}m", a.label, (d / 3600.0) as u32, (d / 60.0) as u32 % 60),
        None => String::from("No alarm set"),
    }));
    for a in &st.alarms {
        let aid = a.id;
        let (time, ampm) = time_of_day(f64::from(a.hour * 3600 + a.minute * 60), &Settings { seconds: false, ..st.set.clone() });
        let days = if a.days.iter().all(|d| *d) { String::from("Every day") }
                   else if a.days[..5].iter().all(|d| *d) && !a.days[5] && !a.days[6] { String::from("Weekdays") }
                   else if !a.days.iter().any(|d| *d) { String::from("Never") }
                   else { DAYS.iter().zip(a.days).filter(|(_, on)| *on).map(|(d, _)| *d).collect::<Vec<_>>().join(" ") };
        let class = match (st.alarm_sel == Some(aid), a.on) {
            (true, true) => "alarm sel",
            (true, false) => "alarm sel off",
            (false, true) => "alarm",
            (false, false) => "alarm off",
        };
        list = list.child(div().id(id(&format!("row{aid}"))).class(class)
            .on_click(act(weak, move |this, _: &ClickEvent| this.st.alarm_sel = Some(aid)))
            .child(div().class("when").child(time).child(small().child(ampm)))
            .child(div().class("what").child(div().child(a.label.clone())).child(div().child(days)))
            .child(Checkbox().id(id(&format!("on{aid}"))).checked(a.on).on_change(act(weak, move |this, e: &ChangeEvent| {
                if let Some(a) = this.st.alarms.iter_mut().find(|a| a.id == aid) {
                    a.on = e.checked;
                }
            }))));
    }
    let header = div().class("hdr").child(span().class("title").child("Alarms"))
        .child(Button().id(id("add")).child("Add Alarm").on_click(act(weak, move |this, _: &ClickEvent| {
            let (sec, _) = this.st.local(now(), this.st.set.home);
            let m = ((sec / 60.0) as u32 + 1) % 1440;
            let id = this.st.id();
            this.st.alarms.push(Alarm { id, label: format!("Alarm {}", this.st.alarms.len() + 1), hour: m / 60, minute: m % 60,
                                        days: [true; 7], on: true, rang: -1, snooze_until: None });
            this.st.alarm_sel = Some(id);
        })));
    let sel = st.alarms.iter().find(|a| Some(a.id) == st.alarm_sel).cloned();
    let side = match sel {
        Some(a) => {
            let aid = a.id;
            let edit = move |f: fn(&mut Alarm, &ChangeEvent)| act(weak, move |this, e: &ChangeEvent| {
                if let Some(a) = this.st.alarms.iter_mut().find(|a| a.id == aid) {
                    f(a, e);
                    a.rang = -1;
                }
            });
            let mut days = div().class("days");
            for (k, d) in DAYS.iter().enumerate() {
                days = days.child(label().child(Checkbox().id(id(&format!("day{k}"))).checked(a.days[k])
                    .on_change(act(weak, move |this, e: &ChangeEvent| {
                        if let Some(a) = this.st.alarms.iter_mut().find(|a| a.id == aid) {
                            a.days[k] = e.checked;
                        }
                    }))).child(*d));
            }
            div().class("side")
                .child(details().open(true).child(summary().child("Alarm"))
                    .child(div().class("row").child(span().child("Label"))
                        .child(TextField().id(id("label")).value(a.label.clone()).on_change(edit(|a, e| a.label.clone_from(&e.value)))))
                    .child(div().class("row").child(span().child("Hour"))
                        .child(NumberField().id(id("hour")).min(0.0).max(23.0).step(1.0).value(a.hour.to_string())
                            .on_change(edit(|a, e| a.hour = e.number.round().clamp(0.0, 23.0) as u32))))
                    .child(div().class("row").child(span().child("Minute"))
                        .child(NumberField().id(id("minute")).min(0.0).max(59.0).step(1.0).value(a.minute.to_string())
                            .on_change(edit(|a, e| a.minute = e.number.round().clamp(0.0, 59.0) as u32)))))
                .child(details().open(true).child(summary().child("Repeat")).child(days)
                    .child(div().class("row")
                        .child(Button().id(id("weekdays")).child("Weekdays").on_click(act(weak, move |this, _: &ClickEvent| {
                            if let Some(a) = this.st.alarms.iter_mut().find(|a| a.id == aid) {
                                a.days = [true, true, true, true, true, false, false];
                            }
                        })))
                        .child(Button().id(id("everyday")).child("Every Day").on_click(act(weak, move |this, _: &ClickEvent| {
                            if let Some(a) = this.st.alarms.iter_mut().find(|a| a.id == aid) {
                                a.days = [true; 7];
                            }
                        })))))
                .child(details().child(summary().child("Delete"))
                    .child(div().class("row").child(Button().id(id("delete")).child("Delete Alarm").on_click(act(weak, move |this, _: &ClickEvent| {
                        this.st.alarms.retain(|a| a.id != aid);
                        this.st.alarm_sel = this.st.alarms.first().map(|a| a.id);
                    })))))
        }
        None => div().class("side").child(div().class("hint").child("Select an alarm.")),
    };
    let main = div().class("main").child(list);
    Editor::new(main).header(header).sidebar(Region::new(side).size(250).overlap(false).hotkey("n"))
}

fn timers(st: &State, weak: Weak, t: f32, n: u32) -> Editor {
    let id = |s: &str| format!("tm{n}-{s}");
    let accent = st.accent();
    let mut form = div().class("new")
        .child(span().child("Duration (s)"))
        .child(NumberField().id(id("secs")).min(1.0).max(86_400.0).step(1.0).value(format!("{}", st.form_secs))
            .on_change(act(weak, |this, e: &ChangeEvent| this.st.form_secs = e.number.max(1.0) as f32)))
        .child(span().class("read").child(clock_left(st.form_secs)))
        .child(TextField().id(id("label")).placeholder("Label").value(st.form_label.clone())
            .on_change(act(weak, |this, e: &ChangeEvent| this.st.form_label.clone_from(&e.value))))
        .child(Button().id(id("add")).child("Start Timer").on_click(act(weak, |this, _: &ClickEvent| this.add_timer())));
    for (k, (name, secs)) in PRESETS.into_iter().enumerate() {
        form = form.child(Button().id(id(&format!("p{k}"))).child(name).on_click(act(weak, move |this, _: &ClickEvent| this.st.form_secs = secs)));
    }
    let mut grid = div().class("timers");
    for x in &st.timers {
        let tid = x.id;
        let left = x.left(t);
        let frac = 1.0 - left / x.total;
        let running = x.since.is_some();
        let class = match (st.timer_sel == Some(tid), x.done) {
            (true, true) => "timer sel done",
            (true, false) => "timer sel",
            (false, true) => "timer done",
            (false, false) => "timer",
        };
        grid = grid.child(div().id(id(&format!("t{tid}"))).class(class)
            .on_click(act(weak, move |this, _: &ClickEvent| this.st.timer_sel = Some(tid)))
            .child(div().class("dial").child(ring_dial(frac, if x.done { rgb(0xe06060) } else { accent }, 140.0))
                .child(div().class("left").child(if x.done { String::from("Done") } else { clock_left(left) }))
                .child(div().class("lbl").child(x.label.clone())))
            .child(ProgressBar().id(id(&format!("bar{tid}"))).max(1.0).value(format!("{:.3}", frac.clamp(0.0, 1.0))))
            .child(div().class("btns")
                .child(Button().id(id(&format!("go{tid}"))).child(if running { "Pause" } else if x.done { "Again" } else { "Start" })
                    .on_click(act(weak, move |this, _: &ClickEvent| this.toggle_timer(tid))))
                .child(Button().id(id(&format!("reset{tid}"))).child("Reset").on_click(act(weak, move |this, _: &ClickEvent| this.reset_timer(tid))))
                .child(Button().id(id(&format!("x{tid}"))).child("Remove").on_click(act(weak, move |this, _: &ClickEvent| {
                    this.st.timers.retain(|x| x.id != tid);
                })))));
    }
    let running = st.timers.iter().filter(|x| x.since.is_some()).count();
    let header = div().class("hdr").child(span().class("title").child("Timers"))
        .child(span().child(format!("{running} running, {} in all", st.timers.len())))
        .child(div().class("sep"))
        .child(Button().id(id("pauseall")).child("Pause All").on_click(act(weak, |this, _: &ClickEvent| {
            let t = now();
            for x in &mut this.st.timers {
                if let Some(s) = x.since.take() {
                    x.before += t - s;
                }
            }
        })))
        .child(Button().id(id("clear")).child("Clear Done").on_click(act(weak, |this, _: &ClickEvent| this.st.timers.retain(|x| !x.done))));
    let main = div().class("main").child(form).child(if st.timers.is_empty() {
        div().class("empty").child("No timers. Pick a duration (drag the field, or double-click it and type 2*60+30) and press Start Timer.")
    } else {
        grid
    });
    Editor::new(main).header(header)
}

fn stopwatch(st: &State, weak: Weak, t: f32, n: u32) -> Editor {
    let id = |s: &str| format!("sw{n}-{s}");
    let sw = &st.sw;
    let total = sw.run(t);
    let accent = st.accent();
    let laps: Vec<f32> = sw.laps.iter().scan(0.0, |prev, &end| {
        let lap = end - *prev;
        *prev = end;
        Some(lap)
    }).collect();
    let marks = sw.laps.clone();
    let dial = canvas(move |b, p| {
        let mut pen = Pen::new(b, p, 100.0);
        let c = (50.0, 50.0);
        pen.disc(c, 48.0, rgb(0x262626), rgb(0x1a1a1a), 1.0);
        for k in 0..60 {
            pen.ray(c, k as f32 * 6.0, if k % 5 == 0 { 40.0 } else { 43.0 }, 46.0, rgb(if k % 5 == 0 { 0xbdbdbd } else { 0x6a6a6a }), 0.9);
        }
        for m in &marks {
            pen.ray(c, m % 60.0 * 6.0, 30.0, 38.0, rgb(0xe0a040), 1.4);
        }
        pen.arc(c, 34.0, (0.0, total % 60.0 * 6.0), Rgba { a: 0.35, ..accent }, 3.0);
        pen.ray(c, total % 60.0 * 6.0, -8.0, 44.0, accent, 1.4);
        pen.disc(c, 2.8, accent, Rgba::TRANSPARENT, 0.0);
    }).class("dial");
    let (best, worst) = if laps.len() >= 2 {
        let b = laps.iter().enumerate().min_by(|x, y| x.1.total_cmp(y.1)).map(|x| x.0);
        let w = laps.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1)).map(|x| x.0);
        (b, w)
    } else {
        (None, None)
    };
    let mut table = table().class("laps").child(thead().child(tr().child(th().child("Lap")).child(th().child("Time")).child(th().child("Total"))));
    let mut body = tbody();
    if sw.since.is_some() || sw.before > 0.0 {
        let cur = total - sw.laps.last().copied().unwrap_or(0.0);
        body = body.child(tr().child(td().child(format!("{}", laps.len() + 1))).child(td().child(clock_up(cur))).child(td().child(clock_up(total))));
    }
    for (k, lap) in laps.iter().enumerate().rev() {
        let class = if Some(k) == best { "best" } else if Some(k) == worst { "worst" } else { "" };
        let class = if k + 1 == laps.len() { format!("{class} fresh") } else { class.to_string() };
        body = body.child(tr().id(id(&format!("lap{k}"))).class(class).child(td().child(format!("{}", k + 1)))
            .child(td().child(clock_up(*lap))).child(td().child(clock_up(sw.laps[k]))));
    }
    table = table.child(body);
    let running = sw.since.is_some();
    let csv: String = std::iter::once(String::from("lap,time,total")).chain(laps.iter().enumerate().map(|(k, l)| {
        format!("{},{:.2},{:.2}", k + 1, l, sw.laps[k])
    })).collect::<Vec<_>>().join("\n");
    let main = div().class("main").child(div().class("sw").child(dial)
        .child(div().child(div().class("big").child(clock_up(total)))
            .child(div().class("ctl")
                .child(Button().id(id("go")).child(if running { "Stop" } else { "Start" }).on_click(act(weak, |this, _: &ClickEvent| this.sw_toggle())))
                .child(Button().id(id("lap")).child("Lap").disabled(!running).on_click(act(weak, |this, _: &ClickEvent| this.sw_lap())))
                .child(Button().id(id("reset")).child("Reset").on_click(act(weak, |this, _: &ClickEvent| this.sw_reset()))))
            .child(if laps.is_empty() { div().class("empty").child("Laps appear here (L).") } else { div().child(table) })));
    let side = div().class("side")
        .child(details().open(true).child(summary().child("Laps as CSV"))
            .child(TextArea().id(id("csv")).value(csv).css("width", "100%").css("height", "160px")))
        .child(div().class("hint").child("Space starts and stops, L laps, R resets."));
    Editor::new(main).header(div().class("hdr").child(span().class("title").child("Stopwatch"))
        .child(span().child(format!("{} laps", laps.len()))))
        .sidebar(Region::new(side).size(220).overlap(false).hidden(true).hotkey("n"))
}

fn pomodoro(st: &State, weak: Weak, t: f32, n: u32, area: AreaRef) -> Editor {
    let id = |s: &str| format!("po{n}-{s}");
    let p = &st.pomo;
    let len = p.length();
    let left = (len - p.run(t)).max(0.0);
    let color = match p.phase {
        Phase::Work => st.accent(),
        Phase::Short => rgb(0x4fae6a),
        Phase::Long => rgb(0x9a6ad0),
    };
    let mut dots = div().class("dots");
    for k in 0..p.cycles {
        dots = dots.child(div().class(if k < p.done_cycles % p.cycles || (p.done_cycles > 0 && p.done_cycles.is_multiple_of(p.cycles) && p.phase == Phase::Long) { "dot on" } else { "dot" }));
    }
    let running = p.since.is_some();
    let focused = st.focus == Some(area);
    let slider = |key: &str, lo: f64, hi: f64, v: f32, f: fn(&mut Pomodoro, f32)| {
        NumberSlider().id(id(key)).min(lo).max(hi).step(1.0).value(format!("{v}")).css("width", "200px")
            .on_change(act(weak, move |this, e: &ChangeEvent| f(&mut this.st.pomo, e.number as f32)))
    };
    let main = div().class(if focused { "main pomo focus" } else { "main pomo" })
        .child(div().class("dial").child(ring_dial(1.0 - left / len, color, 170.0))
            .child(div().class("phase").child(p.phase.name()))
            .child(div().class("left").child(clock_left(left))))
        .child(dots)
        .child(div().class("ctl")
            .child(Button().id(id("go")).child(if running { "Pause" } else { "Start" }).on_click(act(weak, |this, _: &ClickEvent| this.pomo_toggle())))
            .child(Button().id(id("skip")).child("Skip").on_click(act(weak, |this, _: &ClickEvent| this.pomo_next(false))))
            .child(Button().id(id("reset")).child("Reset").on_click(act(weak, |this, _: &ClickEvent| {
                let p = &mut this.st.pomo;
                (p.phase, p.before, p.since, p.done_cycles) = (Phase::Work, 0.0, None, 0);
            })))
            .child(Button().id(id("focus")).child(if focused { "Leave Focus" } else { "Focus" }).on_click(act(weak, move |this, _: &ClickEvent| {
                if this.st.focus == Some(area) {
                    this.leave_focus();
                } else {
                    // Blender's Ctrl+Alt+Space: this area alone, without its header and regions
                    area.focus();
                    this.st.focus = Some(area);
                }
            }))))
        .child(div().class("stats")
            .child(div().child(format!("Focused today: {} of {} min", (p.focused / 60.0).round(), p.goal)))
            .child(ProgressBar().id(id("goal")).max(1.0).value(format!("{:.3}", (p.focused / 60.0 / p.goal).clamp(0.0, 1.0)))));
    let side = div().class("side")
        .child(details().open(true).child(summary().child("Lengths (min)"))
            .child(div().class("row").child(span().child("Focus")).child(slider("work", 5.0, 90.0, p.work, |p, v| p.work = v)))
            .child(div().class("row").child(span().child("Short")).child(slider("short", 1.0, 30.0, p.short, |p, v| p.short = v)))
            .child(div().class("row").child(span().child("Long")).child(slider("long", 5.0, 60.0, p.long, |p, v| p.long = v))))
        .child(details().open(true).child(summary().child("Cycle"))
            .child(div().class("row").child(span().child("Long after")).child(slider("cycles", 2.0, 8.0, p.cycles as f32, |p, v| p.cycles = v.max(1.0) as u32)))
            .child(div().class("row").child(Checkbox().id(id("auto")).checked(p.auto)
                .on_change(act(weak, |this, e: &ChangeEvent| this.st.pomo.auto = e.checked))).child("Start the next phase by itself"))
            .child(div().class("row").child(span().child("Daily goal"))
                .child(NumberField().id(id("goalmin")).min(10.0).max(600.0).step(5.0).value(format!("{}", p.goal))
                    .on_change(act(weak, |this, e: &ChangeEvent| this.st.pomo.goal = e.number.max(1.0) as f32)))));
    Editor::new(main).header(div().class("hdr").child(span().class("title").child("Pomodoro"))
        .child(span().child(format!("{} · cycle {} of {}", p.phase.name(), p.done_cycles % p.cycles + 1, p.cycles))))
        .sidebar(Region::new(side).size(236).overlap(false).hotkey("n"))
}

fn settings(st: &State, weak: Weak, n: u32) -> Editor {
    let id = |s: &str| format!("se{n}-{s}");
    let s = &st.set;
    let mut sound = Dropdown().id(id("sound")).on_change(act(weak, |this, e: &ChangeEvent| this.st.set.sound = e.number as usize));
    for (k, name) in SOUNDS.iter().enumerate() {
        sound = sound.child(option().selected(k == s.sound).child(*name));
    }
    let main = div().class("main").child(div().class("set")
        .child(fieldset().child(legend().child("Sound"))
            .child(div().class("row").child(span().child("Chime")).child(sound)
                .child(Button().id(id("test")).child("Test").on_click(act(weak, |this, _: &ClickEvent| this.chime()))))
            .child(div().class("row").child(span().child("Volume"))
                .child(NumberSlider().id(id("vol")).min(0.0).max(100.0).step(1.0).value(format!("{}", s.volume)).css("width", "220px")
                    .on_change(act(weak, |this, e: &ChangeEvent| {
                        this.st.set.volume = e.number;
                        if let Some(snd) = &this.sound {
                            snd.set_volume(e.number / 100.0);
                        }
                    })))))
        .child(fieldset().child(legend().child("Time"))
            .child(div().class("row").child(Checkbox().id(id("h24")).checked(s.h24)
                .on_change(act(weak, |this, e: &ChangeEvent| this.st.set.h24 = e.checked))).child("24-hour time"))
            .child(div().class("row").child(Checkbox().id(id("secs")).checked(s.seconds)
                .on_change(act(weak, |this, e: &ChangeEvent| this.st.set.seconds = e.checked))).child("Show seconds"))
            .child(div().class("row").child(span().child("Home zone (h)"))
                .child(NumberField().id(id("home")).min(-12.0).max(14.0).step(0.25).value(format!("{}", s.home))
                    .on_change(act(weak, |this, e: &ChangeEvent| this.st.set.home = e.number as f32)))))
        .child(fieldset().child(legend().child("Look"))
            .child(div().class("row").child(span().child("Accent")).child(ColorSwatch().id(id("accent")).value(s.accent.clone())
                .on_change(act(weak, |this, e: &ChangeEvent| this.st.set.accent.clone_from(&e.value)))))
            .child(div().class("swatch-preview"))
            .child(div().class("row").child(span().child("Presets"))
                .child(Button().id(id("blue")).child("Blender Blue").on_click(act(weak, |this, _: &ClickEvent| this.st.set.accent = "#4772b3".into())))
                .child(Button().id(id("orange")).child("Orange").on_click(act(weak, |this, _: &ClickEvent| this.st.set.accent = "#e87d0d".into())))
                .child(Button().id(id("green")).child("Green").on_click(act(weak, |this, _: &ClickEvent| this.st.set.accent = "#4fae6a".into()))))));
    Editor::new(main).header(div().class("hdr").child(span().class("title").child("Settings")))
}

impl Render for TimerApp {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        focus_root(w, &mut self.frames);
        self.theme_accent();
        let t = now();
        self.tick(t);
        self.desktop(w, t);
        // the clocks' seconds move, and alarms are checked as time passes (BUI has no timer to
        // wake a view once a second: a view that changes with time asks for frames)
        w.request_animation_frame();
        let weak = cx.weak_entity();
        let st = Rc::new(self.st.clone());
        // ids an editor gives its elements carry its place among the areas showing it
        let counter = |st: Rc<State>| (st, std::cell::Cell::new(0u32));
        let (s1, n1) = counter(st.clone());
        let (s2, n2) = counter(st.clone());
        let (s3, n3) = counter(st.clone());
        let (s4, n4) = counter(st.clone());
        let (s5, n5) = counter(st.clone());
        let (s6, n6) = counter(st);
        let next = |c: &std::cell::Cell<u32>| {
            let n = c.get();
            c.set(n + 1);
            n
        };
        let editors = Editors::new()
            .editor("Timers", "TIME", move |_: &AreaRef| timers(&s1, weak, t, next(&n1)))
            .editor("Stopwatch", "SORTTIME", move |_: &AreaRef| stopwatch(&s2, weak, t, next(&n2)))
            .editor("Pomodoro", "PREVIEW_RANGE", move |a: &AreaRef| pomodoro(&s3, weak, t, next(&n3), *a))
            .editor("World Clock", "WORLD", move |_: &AreaRef| world_clock(&s4, weak, t, next(&n4)))
            .editor("Alarms", "OUTLINER_OB_SPEAKER", move |_: &AreaRef| alarms(&s5, weak, t, next(&n5)))
            .editor("Settings", "PREFERENCES", move |_: &AreaRef| settings(&s6, weak, next(&n6)));
        let ws = self.ws;
        let screen = Screen().id(format!("screen{ws}")).class("screen").editors(editors).layout(workspace_layout(ws))
            .saved(self.saved[ws].as_deref())
            .on_change(act(weak, move |this, e: &ScreenEvent| this.saved[ws] = Some(e.layout().to_owned())));

        // the top bar: workspaces as Blender has them, and a ringing alarm
        let mut top = div().class("top").child(span().class("brand").child("BUI Timer"));
        for (k, name) in WORKSPACES.iter().enumerate() {
            top = top.child(div().id(format!("ws{k}")).class(if k == ws { "tab on" } else { "tab" }).child(*name)
                .on_click(act(weak, move |this, _: &ClickEvent| this.workspace(k))));
        }
        top = top.child(div().class("spacer"));
        if let Some((aid, _)) = self.st.ringing
            && let Some(a) = self.st.alarms.iter().find(|a| a.id == aid)
        {
            let (time, ampm) = time_of_day(f64::from(a.hour * 3600 + a.minute * 60), &Settings { seconds: false, ..self.st.set.clone() });
            top = top.child(div().id("ring").class("ring").child(b().child(format!("{time} {ampm}"))).child(a.label.clone())
                .child(Button().id("snooze").child("Snooze 9 min").on_click(act(weak, move |this, _: &ClickEvent| {
                    let wall = this.st.wall(now());
                    if let Some(a) = this.st.alarms.iter_mut().find(|a| a.id == aid) {
                        a.snooze_until = Some(wall + 540.0);
                    }
                    this.st.ringing = None;
                })))
                .child(Button().id("dismiss").child("Dismiss").on_click(act(weak, |this, _: &ClickEvent| this.st.ringing = None))));
        }
        let (sec, _) = self.st.local(t, self.st.set.home);
        let (time, ampm) = time_of_day(sec, &self.st.set);
        top = top.child(span().css("font-variant-numeric", "tabular-nums").css("margin-left", "10px").child(format!("{time} {ampm}")));

        // the status bar: what the keys do, and what is running
        let running = self.st.timers.iter().filter(|x| x.since.is_some()).count();
        let next = self.st.next_alarm(t).map_or_else(|| String::from("none"), |(d, a)| format!("{} in {}h {}m", a.label, (d / 3600.0) as u32, (d / 60.0) as u32 % 60));
        let status = div().class("status")
            .child(span().child(kbd().child("Space")).child(" stopwatch  ").child(kbd().child("L")).child(" lap  ")
                .child(kbd().child("P")).child(" pomodoro  ").child(kbd().child("T")).child(" timer  ")
                .child(kbd().child("N")).child(" sidebar  ").child(kbd().child("Ctrl+Space")).child(" maximize"))
            .child(div().class("spacer"))
            .child(span().child("Timers ").child(b().child(running.to_string())))
            .child(span().child("Next alarm ").child(b().child(next)));

        app().id("root").css("--accent", self.st.set.accent.clone())
            // keys typed into a text field reach the page too: no shortcuts while one is typed in
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                // a key we act on is taken: Blender's own handling (Ctrl+Z over widget edits, a
                // hovered field's Backspace) doesn't run as well
                if !w.text_field_focused() && this.key(e) {
                    cx.prevent_default();
                    cx.notify();
                }
            }))
            // a press anywhere ends typing; a text field's own press (after this) starts it again
            .child(style().child(CSS))
            .child(top)
            .child(screen)
            .child(status)
    }
}

/// Every feature: timers from presets and typed durations, finishing; the stopwatch with laps;
/// the pomodoro and its focus; world clock and alarms (one rings, is snoozed, rings again,
/// is dismissed); settings recolouring the app; workspaces.
fn storyboard(s: &mut Story) {
    s.play("00-open", &[0, 300]);
    s.cx.simulate_mouse_move(s.win, 5.0, 5.0);
    // timers: a preset, a typed duration (the field takes math), a label
    s.click("tm0-p0");
    s.click("tm0-add");
    s.click("tm0-p2");
    s.click("tm0-label");
    s.key("End");
    for _ in 0..3 {
        s.key("BackSpace");
    }
    s.type_text("Pasta");
    s.key("Return");
    s.click("tm0-add");
    s.play("01-two-timers", &[0, 400, 2000]);
    s.tick(59_000);
    s.play("02-first-done", &[0, 350]);
    // stopwatch: start, laps, stop
    s.key("space");
    s.tick(4_200);
    s.key("l");
    s.tick(3_100);
    s.key("l");
    s.tick(5_600);
    s.click("sw0-lap");
    s.play("03-laps", &[0, 120, 900]);
    s.key("space");
    // pomodoro: start, then focus its area
    s.click("po0-go");
    s.tick(60_000);
    s.play("04-pomodoro", &[0]);
    s.click("po0-focus");
    s.play("05-focus", &[0, 300]);
    s.key("Escape");
    s.play("06-unfocus", &[0, 300]);
    // clocks: the world clock and alarms
    s.click("ws1");
    s.play("07-clocks", &[0, 300]);
    s.click("wc0-city2");
    s.play("08-city", &[0, 200]);
    // an alarm a minute from now: it rings, is snoozed, rings again, is dismissed
    s.click("al0-add");
    s.play("09-new-alarm", &[0, 200]);
    // rename it in the sidebar (an "n" typed there: does the area's N hotkey take it too?)
    s.click("al0-label");
    s.key("End");
    s.type_text(" run");
    s.key("Return");
    s.play("09b-renamed", &[0, 200]);
    s.tick(61_000);
    s.play("10-ringing", &[0, 450]);
    s.click("snooze");
    s.play("11-snoozed", &[0]);
    s.tick(541_000);
    s.play("12-again", &[0]);
    s.click("dismiss");
    s.play("13-dismissed", &[0]);
    // settings: an accent preset recolours everything
    s.click("ws3");
    s.play("14-settings", &[0, 300]);
    s.click("se0-orange");
    s.play("15-orange", &[0, 200]);
    s.click("ws0");
    s.play("16-orange-timers", &[0, 200]);
}

fn main() {
    let (w, h) = size_arg((1280.0, 800.0));
    let board = arg("--storyboard").map(|dir| -> Script { story(dir, storyboard) });
    let headless = board.is_some();
    run_with(w, h, "BUI Timer", move || TimerApp::new(headless), board);
}

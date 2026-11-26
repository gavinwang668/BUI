//! BUI Calendar: a scheduling app made of Blender's UI. The window is a Blender screen: a Calendar
//! editor (week, day, month or agenda; a header with the view, navigation, search and time zone; a
//! toolbar region with a mini month and the calendars; an event editor in its sidebar) beside a
//! Day editor showing the selected day. Split, join or swap the areas as in Blender.
//!
//! Week and day views: drag on empty time to create an event, drag an event to move it (to another
//! day too), drag its bottom edge to resize it, click it to edit it in the sidebar. Overlapping
//! events share the column side by side. The month view: drag an event's chip onto another day,
//! double-click a day to add an event. The editor is Blender widgets: a title text field, a start
//! and a duration you drag or type math into (`1.5*60`), a calendar dropdown, an all-day and a
//! repeat-weekly checkbox with a count, notes in a text area. Calendars show and hide with their
//! checkboxes and recolor with Blender's color picker. Times follow the time-zone dropdown. A red
//! line marks now, moving by itself (a CSS animation: nothing renders while you wait).
//!
//! Keys: T today, ← / → previous / next, D W M A the views, Delete the selected event, Ctrl+D
//! duplicate, Ctrl+Z / Ctrl+Y undo / redo, Esc deselect.
//!
//! `cargo run --release --bin bui_calendar`, `-- --storyboard DIR` drives it headlessly.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use blenderui::prelude::*;
use blenderui::screen::{AreaRef, AreaSplit, Editor, Editors, Region, Screen};
use bui_demos::story::story;
use bui_demos::{arg, focus_root, now, run_with, size_arg, Script};
use bui_demos::color::hex;

/// Pixels per hour in the time grid.
const HOUR_H: f32 = 44.0;
const SNAP: i64 = 15;
const DAY_MIN: i64 = 1440;
const GUTTER: f32 = 52.0;
const UNDO: usize = 200;

// ------------------------------------------------------------------------------------------------
// Dates: days since 1970-01-01 (Howard Hinnant's civil algorithms)

const fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

const fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 0 Monday … 6 Sunday.
const fn weekday(z: i64) -> i64 {
    (z + 3).rem_euclid(7)
}

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September",
                            "October", "November", "December"];
const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

const fn month_start(z: i64) -> i64 {
    let (y, m, _) = civil(z);
    days_from_civil(y, m, 1)
}

fn add_months(z: i64, n: i64) -> i64 {
    let (y, m, d) = civil(z);
    let k = y * 12 + (m - 1) + n;
    let (ny, nm) = (k.div_euclid(12), k.rem_euclid(12) + 1);
    let last = days_from_civil(ny + i64::from(nm == 12), if nm == 12 { 1 } else { nm + 1 }, 1) - days_from_civil(ny, nm, 1);
    days_from_civil(ny, nm, d.min(last))
}

fn clock(min: i64) -> String {
    let m = min.rem_euclid(DAY_MIN);
    format!("{:02}:{:02}", m / 60, m % 60)
}

fn date_label(z: i64) -> String {
    let (_, m, d) = civil(z);
    format!("{} {} {d}", DAYS[weekday(z) as usize], &MONTHS[(m - 1) as usize][..3])
}

/// Time zones: a name and the minutes from UTC.
const ZONES: [(&str, i64); 7] = [("UTC", 0), ("Los Angeles (UTC−8)", -480), ("New York (UTC−5)", -300), ("London (UTC+0)", 0),
                                 ("Berlin (UTC+1)", 60), ("Delhi (UTC+5:30)", 330), ("Tokyo (UTC+9)", 540)];

// ------------------------------------------------------------------------------------------------
// The data

#[derive(Clone, PartialEq, Debug)]
struct Event {
    id: u32,
    cal: usize,
    title: String,
    notes: String,
    /// Minutes since the epoch, UTC.
    start: i64,
    dur: i64,
    all_day: bool,
    /// Weekly occurrences in all (1: once).
    repeat: u32,
}

#[derive(Clone, PartialEq, Debug)]
struct Cal {
    name: &'static str,
    color: u32,
    shown: bool,
}

#[derive(Clone)]
struct Data {
    events: Vec<Event>,
    cals: Vec<Cal>,
    next_id: u32,
}

/// One occurrence of an event, in the view's time zone: its local start and which repeat it is.
#[derive(Clone, Copy)]
struct Occ {
    idx: usize,
    start: i64,
    dur: i64,
    k: u32,
}

impl Data {
    fn find(&self, id: u32) -> Option<usize> {
        self.events.iter().position(|e| e.id == id)
    }

    /// The occurrences shown between local days `from` and `to` (exclusive), in zone `tz`.
    fn occurrences(&self, from: i64, to: i64, tz: i64, filter: &str) -> Vec<Occ> {
        let lo = from * DAY_MIN;
        let hi = to * DAY_MIN;
        let q = filter.trim().to_lowercase();
        let mut out = Vec::new();
        for (idx, e) in self.events.iter().enumerate() {
            if !self.cals.get(e.cal).is_some_and(|c| c.shown) {
                continue;
            }
            if !q.is_empty() && !e.title.to_lowercase().contains(&q) && !e.notes.to_lowercase().contains(&q) {
                continue;
            }
            for k in 0..e.repeat.max(1) {
                let start = e.start + tz + i64::from(k) * 7 * DAY_MIN;
                if start < hi && start + e.dur.max(1) > lo {
                    out.push(Occ { idx, start, dur: e.dur, k });
                }
            }
        }
        out.sort_by_key(|o| (o.start, -o.dur));
        out
    }
}

/// Side-by-side columns for overlapping events: (column, columns in its cluster) per occurrence.
fn columns(occ: &[Occ]) -> Vec<(usize, usize)> {
    let mut out = vec![(0, 1); occ.len()];
    let mut cluster: Vec<usize> = Vec::new();
    let mut ends: Vec<i64> = Vec::new();
    let mut cluster_end = i64::MIN;
    let close = |cluster: &mut Vec<usize>, ends: &mut Vec<i64>, out: &mut Vec<(usize, usize)>| {
        let n = ends.len().max(1);
        for &i in cluster.iter() {
            out[i].1 = n;
        }
        cluster.clear();
        ends.clear();
    };
    for (i, o) in occ.iter().enumerate() {
        if o.start >= cluster_end && !cluster.is_empty() {
            close(&mut cluster, &mut ends, &mut out);
        }
        let end = o.start + o.dur.max(SNAP);
        let col = ends.iter().position(|&e| e <= o.start).unwrap_or(ends.len());
        if col == ends.len() {
            ends.push(end);
        } else {
            ends[col] = end;
        }
        out[i].0 = col;
        cluster.push(i);
        cluster_end = cluster_end.max(end);
    }
    close(&mut cluster, &mut ends, &mut out);
    out
}

fn seed(today: i64) -> Data {
    let cals = vec![
        Cal { name: "Work", color: 0x4772b3, shown: true },
        Cal { name: "Personal", color: 0x5aa85a, shown: true },
        Cal { name: "Renders", color: 0xe0913a, shown: true },
        Cal { name: "Holidays", color: 0x9b6bd1, shown: true },
    ];
    let monday = today - weekday(today);
    let mut events = Vec::new();
    let mut id = 1;
    let mut add = |cal: usize, title: &str, day: i64, h: f64, dur: i64, repeat: u32, notes: &str| {
        events.push(Event { id, cal, title: title.into(), notes: notes.into(), start: day * DAY_MIN + (h * 60.0) as i64,
                            dur, all_day: false, repeat });
        id += 1;
    };
    for d in 0..5 {
        add(0, "Stand-up", monday + d, 9.5, 15, 3, if d == 0 { "Yesterday, today, blockers." } else { "" });
    }
    add(0, "Layout review: shot 12", monday + 1, 11.0, 60, 1, "Bring the animatic.");
    add(0, "Lighting pass", monday + 1, 11.5, 90, 1, "Overlaps the review: shows side by side.");
    add(1, "Lunch with Ada", monday + 2, 12.5, 60, 1, "The ramen place.");
    add(2, "Farm: final frames", monday + 2, 14.0, 240, 1, "Shots 10 to 14, 4K, 240 frames each.");
    add(0, "Client call", monday + 3, 16.0, 45, 1, "Little Moth Studio, the teaser.");
    add(1, "Climbing", monday + 1, 18.5, 90, 4, "Bring chalk.");
    add(1, "Climbing", monday + 3, 18.5, 90, 4, "");
    add(0, "Sprint planning", monday + 4, 10.0, 120, 1, "");
    add(2, "Farm: lookdev batch", monday + 4, 13.0, 60, 1, "");
    add(1, "Market", monday + 5, 10.0, 90, 1, "");
    add(0, "Retro", monday + 11, 15.0, 60, 1, "");
    add(2, "Farm: trailer", monday + 9, 9.0, 300, 1, "");
    let mut data = Data { events, cals, next_id: id };
    let all_day = |data: &mut Data, cal: usize, title: &str, day: i64| {
        let id = data.next_id;
        data.next_id += 1;
        data.events.push(Event { id, cal, title: title.into(), notes: String::new(), start: day * DAY_MIN, dur: DAY_MIN,
                                 all_day: true, repeat: 1 });
    };
    all_day(&mut data, 3, "Studio offsite", monday + 4);
    all_day(&mut data, 3, "Long weekend", monday + 12);
    data
}

// ------------------------------------------------------------------------------------------------
// The app

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Day,
    Week,
    Month,
    Agenda,
}

const VIEWS: [(&str, View, &str); 4] = [("Day", View::Day, "D"), ("Week", View::Week, "W"), ("Month", View::Month, "M"),
                                        ("Agenda", View::Agenda, "A")];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag {
    /// On empty time: from and to (local minutes).
    Create { from: i64, to: i64 },
    /// An event: where it was grabbed (minutes into it) and its local start now.
    Move { id: u32, grab: i64, at: i64, moved: bool },
    Resize { id: u32, end: i64 },
    /// A month chip: the event and the day it is over.
    Chip { id: u32, day: i64 },
}

#[derive(Clone)]
struct Ui {
    view: View,
    /// The selected day (local).
    day: i64,
    today: i64,
    sel: Option<u32>,
    drag: Option<Drag>,
    search: String,
    tz: usize,
    status: String,
    /// Bumped by each new event: keys its pop-in.
    born: u32,
}

struct CalApp {
    data: Arc<Data>,
    ui: Ui,
    undo: Vec<Arc<Data>>,
    redo: Vec<Arc<Data>>,
    frames: u32,
    /// The wall clock's day minute when the app began (the now line runs on from it).
    start_minute: i64,
}

struct Snap {
    data: Arc<Data>,
    ui: Ui,
    weak: WeakEntity<CalApp>,
    now_min: i64,
}

impl CalApp {
    fn new() -> Self {
        let wall = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs().cast_signed() / 60);
        let today = wall.div_euclid(DAY_MIN);
        Self {
            data: Arc::new(seed(today)),
            ui: Ui { view: View::Week, day: today, today, sel: None, drag: None, search: String::new(), tz: 0,
                     status: "Drag on empty time to add an event".into(), born: 0 },
            undo: Vec::new(),
            redo: Vec::new(),
            frames: 0,
            start_minute: wall,
        }
    }

    const fn tz(&self) -> i64 {
        ZONES[self.ui.tz].1
    }

    fn change(&mut self, what: &str, f: impl FnOnce(&mut Data)) {
        self.undo.push(self.data.clone());
        if self.undo.len() > UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        f(Arc::make_mut(&mut self.data));
        self.ui.status = what.to_string();
    }

    fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(&mut self.data, prev));
        self.ui.status = "Undo".into();
        self.fix_sel();
        true
    }

    fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(&mut self.data, next));
        self.ui.status = "Redo".into();
        self.fix_sel();
        true
    }

    fn fix_sel(&mut self) {
        if self.ui.sel.is_some_and(|id| self.data.find(id).is_none()) {
            self.ui.sel = None;
        }
    }

    fn edit(&mut self, what: &str, f: impl FnOnce(&mut Event)) {
        let Some(i) = self.ui.sel.and_then(|id| self.data.find(id)) else { return };
        self.change(what, |d| f(&mut d.events[i]));
    }

    /// A new event at local minute `start` for `dur`, selected.
    fn create(&mut self, start: i64, dur: i64, all_day: bool) {
        let id = self.data.next_id;
        let tz = self.tz();
        let cal = self.ui.sel.and_then(|s| self.data.find(s)).map_or(0, |i| self.data.events[i].cal);
        self.change("Event added", |d| {
            d.next_id += 1;
            d.events.push(Event { id, cal, title: "New event".into(), notes: String::new(), start: start - tz, dur: dur.max(SNAP),
                                  all_day, repeat: 1 });
        });
        self.ui.sel = Some(id);
        self.ui.born = id;
        self.ui.day = start.div_euclid(DAY_MIN);
    }

    fn delete(&mut self) -> bool {
        let Some(id) = self.ui.sel else { return false };
        let Some(i) = self.data.find(id) else { return false };
        let title = self.data.events[i].title.clone();
        self.change(&format!("Deleted “{title}”"), |d| {
            d.events.remove(i);
        });
        self.ui.sel = None;
        true
    }

    fn duplicate(&mut self) -> bool {
        let Some(i) = self.ui.sel.and_then(|id| self.data.find(id)) else { return false };
        let id = self.data.next_id;
        self.change("Duplicated", |d| {
            let mut e = d.events[i].clone();
            e.id = id;
            e.start += 60;
            d.next_id += 1;
            d.events.push(e);
        });
        self.ui.sel = Some(id);
        self.ui.born = id;
        true
    }

    /// The days the main view's period moves by.
    const fn period(&self) -> i64 {
        match self.ui.view {
            View::Day => 1,
            View::Week | View::Agenda => 7,
            View::Month => 0,
        }
    }

    fn step(&mut self, dir: i64) {
        self.ui.day = if self.ui.view == View::Month { add_months(self.ui.day, dir) } else { self.ui.day + dir * self.period() };
    }

    fn key(&mut self, e: &KeyDownEvent) -> bool {
        let k = &e.keystroke;
        let ctrl = k.modifiers.control;
        match (k.key.as_str(), ctrl) {
            ("z" | "Z", true) => return self.undo(),
            ("y" | "Y", true) => return self.redo(),
            ("d" | "D", true) => return self.duplicate(),
            ("t" | "T", false) => {
                self.ui.day = self.ui.today;
                self.ui.status = "Today".into();
            }
            ("Left", _) => self.step(-1),
            ("Right", _) => self.step(1),
            ("Delete" | "BackSpace", _) => return self.delete(),
            ("Escape", _) => {
                self.ui.sel = None;
                self.ui.drag = None;
            }
            (key, false) => match VIEWS.iter().find(|v| v.2.eq_ignore_ascii_case(key)) {
                Some(v) => self.ui.view = v.1,
                None => return false,
            },
            _ => return false,
        }
        true
    }

    /// A drag let go.
    fn release(&mut self) -> bool {
        let Some(d) = self.ui.drag.take() else { return false };
        let tz = self.tz();
        match d {
            Drag::Create { from, to } => {
                let (a, b) = (from.min(to), from.max(to));
                self.create(a, (b - a).max(30), false);
            }
            Drag::Move { id, at, moved, .. } => {
                if moved && let Some(i) = self.data.find(id) {
                    let start = at - tz;
                    if self.data.events[i].start != start {
                        self.change(&format!("Moved to {} {}", date_label(at.div_euclid(DAY_MIN)), clock(at)), |d| d.events[i].start = start);
                    }
                }
            }
            Drag::Resize { id, end } => {
                if let Some(i) = self.data.find(id) {
                    let local_start = self.data.events[i].start + tz;
                    let dur = (end - local_start).max(SNAP);
                    if dur != self.data.events[i].dur {
                        self.change(&format!("Now {dur} min"), |d| d.events[i].dur = dur);
                    }
                }
            }
            Drag::Chip { id, day } => {
                if let Some(i) = self.data.find(id) {
                    let local = self.data.events[i].start + tz;
                    let start = day * DAY_MIN + local.rem_euclid(DAY_MIN) - tz;
                    if start != self.data.events[i].start {
                        self.change(&format!("Moved to {}", date_label(day)), |d| d.events[i].start = start);
                    }
                }
            }
        }
        true
    }
}

/// The local minute at window y in the time grid of day `day` whose top is `top`.
fn minute_at(day: i64, top: f32, y: f32) -> i64 {
    let m = ((y - top) / HOUR_H * 60.0) as i64;
    day * DAY_MIN + (m.div_euclid(SNAP) * SNAP).clamp(0, DAY_MIN - SNAP)
}


const CSS: &str = r"
.scr { width: 100vw; height: 100vh; }
bui-region { background: #303030; }
bui-region[region=sidebar] { background: #2b2b2b; border-left: 1px solid #1f1f1f; overflow-y: auto; }
bui-region[region=toolbar] { background: #2b2b2b; border-right: 1px solid #1f1f1f; overflow-y: auto; }
bui-region[region=footer] { background: #2e2e2e; border-top: 1px solid #1f1f1f; }
.hdr { display: flex; align-items: center; gap: 6px; height: 100%; padding: 0 8px; white-space: nowrap; }
.hdr .title { font-size: 15px; font-weight: 700; color: #ececec; min-width: 190px; margin: 0 6px; font-variant-numeric: tabular-nums; }
.hdr .sep { width: 1px; height: 16px; background: #4a4a4a; margin: 0 4px; }
.hdr .lbl { color: #9a9a9a; font-size: 11px; }
.foot { display: flex; align-items: center; height: 100%; padding: 0 10px; font-size: 12px; color: #a8a8a8; gap: 14px; }
.foot .st { color: #8cb4ff; }
.view { display: flex; flex-direction: column; height: 100%; background: #262626; user-select: none; }
.dayhead { display: flex; flex-shrink: 0; border-bottom: 1px solid #1d1d1d; background: #2e2e2e; }
.dayhead .gut { width: 52px; flex-shrink: 0; font-size: 10px; color: #8a8a8a; text-align: center; padding-top: 6px; }
.dh { flex: 1; padding: 5px 8px; font-size: 12px; color: #b6b6b6; cursor: pointer; border-left: 1px solid #222;
      transition-property: background-color; transition-duration: 120ms; }
.dh:hover { background: #363636; }
.dh .n { font-size: 18px; font-weight: 700; color: #e6e6e6; font-variant-numeric: tabular-nums; }
.dh.today .n { color: #ff6a5e; }
.dh.sel { background: #34405a; }
.allday { display: flex; flex-shrink: 0; min-height: 26px; border-bottom: 1px solid #1d1d1d; background: #2a2a2a; }
.allday .gut { width: 52px; flex-shrink: 0; font-size: 10px; color: #8a8a8a; text-align: center; line-height: 26px; }
.ad { flex: 1; padding: 2px 3px; border-left: 1px solid #222; display: flex; flex-direction: column; gap: 2px; }
.body { flex: 1; overflow-y: scroll; position: relative; }
.tgrid { position: relative; display: flex; height: 1056px; }
.hours { width: 52px; flex-shrink: 0; position: relative; }
.hl { position: absolute; right: 6px; font-size: 10px; color: #8a8a8a; font-variant-numeric: tabular-nums; }
.line { position: absolute; left: 52px; right: 0px; height: 1px; background: #333333; }
.line.half { background: #2c2c2c; }
.col { flex: 1; position: relative; border-left: 1px solid #222; }
.col.today { background: rgba(255,106,94,0.035); }
.ev { position: absolute; border-radius: 5px; padding: 3px 6px; font-size: 11px; color: #ffffff; overflow: hidden;
      box-sizing: border-box; cursor: grab; box-shadow: 0 1px 2px rgba(0,0,0,0.35);
      transition-property: opacity, box-shadow; transition-duration: 120ms; }
.ev:hover { box-shadow: 0 2px 8px rgba(0,0,0,0.45); }
.ev .t { font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ev .w { opacity: 0.85; font-variant-numeric: tabular-nums; white-space: nowrap; }
.ev.sel { box-shadow: 0 0 0 2px #ffffff, 0 4px 14px rgba(0,0,0,0.5); }
.ev.ghost { opacity: 0.55; }
.ev.born { animation: pop 260ms cubic-bezier(0.2, 0, 0, 1); }
.grip { position: absolute; left: 0px; right: 0px; bottom: 0px; height: 7px; cursor: ns-resize; }
.newbox { position: absolute; left: 2px; right: 2px; border-radius: 5px; background: rgba(140,180,255,0.35);
          border: 1px dashed #9cc0ff; font-size: 11px; color: #ffffff; padding: 3px 6px; box-sizing: border-box; pointer-events: none; }
.now { position: absolute; left: 0px; right: 0px; height: 2px; background: #ff5a4e; pointer-events: none; }
.now::before { content: ''; position: absolute; left: -5px; top: -4px; width: 10px; height: 10px; border-radius: 5px; background: #ff5a4e; }
.chip { border-radius: 4px; padding: 1px 6px; font-size: 11px; color: #ffffff; white-space: nowrap; overflow: hidden;
        text-overflow: ellipsis; cursor: grab; transition-property: opacity; transition-duration: 120ms; }
.chip.sel { box-shadow: 0 0 0 2px #ffffff; }
.chip.ghost { opacity: 0.45; }
.month { flex: 1; display: grid; grid-template-columns: repeat(7, 1fr); grid-template-rows: 28px repeat(6, 1fr); }
.mh { font-size: 11px; color: #8a8a8a; padding: 6px 8px; border-bottom: 1px solid #1d1d1d; }
.md { border-left: 1px solid #222; border-bottom: 1px solid #222; padding: 4px; display: flex; flex-direction: column;
      gap: 2px; overflow: hidden; transition-property: background-color; transition-duration: 100ms; }
.md:hover { background: #2c2c2c; }
.md.out { color: #5e5e5e; }
.md.sel { background: #34405a; }
.md.drop { background: #3b4a63; }
.md .n { font-size: 12px; color: #c8c8c8; font-variant-numeric: tabular-nums; }
.md.out .n { color: #6a6a6a; }
.md.today .n { color: #ffffff; background: #ff5a4e; border-radius: 9px; padding: 0 6px; align-self: flex-start; }
.more { font-size: 10px; color: #9a9a9a; padding-left: 4px; }
.agenda { flex: 1; overflow-y: scroll; padding: 10px 14px; }
.ag-day { font-size: 12px; font-weight: 700; color: #8cb4ff; margin: 12px 0 4px; }
.ag { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border-radius: 4px; font-size: 12px; cursor: pointer;
      transition-property: background-color; transition-duration: 100ms; }
.ag:hover { background: #333333; }
.ag.sel { background: #34405a; }
.ag .dot { width: 9px; height: 9px; border-radius: 5px; flex-shrink: 0; }
.ag .tm { width: 96px; color: #a8a8a8; font-variant-numeric: tabular-nums; }
.ag .nt { color: #8a8a8a; margin-left: auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 40%; }
.empty { color: #8a8a8a; padding: 24px; text-align: center; }
.tools { padding: 8px; font-size: 12px; }
.mini { display: grid; grid-template-columns: repeat(7, 1fr); gap: 1px; margin: 6px 0 12px; }
.mini .h { font-size: 9px; color: #7a7a7a; text-align: center; }
.mini .d { font-size: 11px; text-align: center; line-height: 22px; border-radius: 11px; cursor: pointer; color: #c8c8c8;
           font-variant-numeric: tabular-nums; transition-property: background-color; transition-duration: 100ms; }
.mini .d:hover { background: #3d3d3d; }
.mini .d.out { color: #5a5a5a; }
.mini .d.busy { font-weight: 700; color: #ffffff; }
.mini .d.sel { background: #4772b3; color: #ffffff; }
.mini .d.today { box-shadow: inset 0 0 0 1px #ff5a4e; }
.mhead { display: flex; align-items: center; justify-content: space-between; font-weight: 700; color: #dedede; }
.mhead .arrow { width: 26px; text-align: center; cursor: pointer; color: #aaaaaa; border-radius: 4px; line-height: 22px; }
.mhead .arrow:hover { background: #3d3d3d; }
.calrow { display: flex; align-items: center; gap: 6px; margin: 5px 0; }
.calrow .nm { flex: 1; }
.side { padding: 8px; font-size: 12px; }
.side details { margin-bottom: 6px; }
.row { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin: 5px 0; }
.row .k { color: #9a9a9a; flex-shrink: 0; }
.hint { color: #8a8a8a; text-wrap: pretty; margin: 10px 2px; line-height: 1.45; }
.colorbar { height: 4px; border-radius: 2px; margin: 2px 0 8px; }
@keyframes pop { from { opacity: 0; transform: scale(0.96); } to { opacity: 1; transform: scale(1); } }
@keyframes fall { from { transform: translateY(0px); } to { transform: translateY(var(--fall)); } }
";

impl Snap {
    const fn tz(&self) -> i64 {
        ZONES[self.ui.tz].1
    }

    fn color_of(&self, e: &Event) -> u32 {
        self.data.cals.get(e.cal).map_or(0x666666, |c| c.color)
    }

    /// The days the main view shows in its time grid.
    fn week_days(&self) -> Vec<i64> {
        if self.ui.view == View::Day {
            vec![self.ui.day]
        } else {
            let mon = self.ui.day - weekday(self.ui.day);
            (mon..mon + 7).collect()
        }
    }

    fn title(&self) -> String {
        let (y, m, d) = civil(self.ui.day);
        match self.ui.view {
            View::Day => format!("{} {d} {} {y}", DAYS[weekday(self.ui.day) as usize], MONTHS[(m - 1) as usize]),
            View::Month => format!("{} {y}", MONTHS[(m - 1) as usize]),
            View::Week | View::Agenda => {
                let mon = self.ui.day - weekday(self.ui.day);
                let (_, m1, d1) = civil(mon);
                let (y2, m2, d2) = civil(mon + 6);
                if m1 == m2 {
                    format!("{} {d1}–{d2}, {y2}", &MONTHS[(m1 - 1) as usize][..3])
                } else {
                    format!("{} {d1} – {} {d2}, {y2}", &MONTHS[(m1 - 1) as usize][..3], &MONTHS[(m2 - 1) as usize][..3])
                }
            }
        }
    }

    /// A time grid of `days` (the week and day views), `tag` keeping its element ids its own.
    fn time_grid(&self, days: &[i64], tag: &'static str) -> Div {
        let w = self.weak;
        let tz = self.tz();
        let (lo, hi) = (days[0], days[days.len() - 1] + 1);
        let occ = self.data.occurrences(lo, hi, tz, &self.ui.search);
        // the day heads
        let mut head = div().class("dayhead").child(div().class("gut").child(ZONES[self.ui.tz].0.split(' ').next().unwrap_or("")));
        let mut allday = div().class("allday").child(div().class("gut").child("all day"));
        for &d in days {
            let (_, _, dn) = civil(d);
            let mut cls = String::from("dh");
            if d == self.ui.today {
                cls.push_str(" today");
            }
            if d == self.ui.day && days.len() > 1 {
                cls.push_str(" sel");
            }
            head = head.child(div().id(format!("{tag}dh{d}")).class(cls).child(DAYS[weekday(d) as usize])
                .child(div().class("n").child(dn.to_string()))
                .on_click(w.listener(move |this: &mut CalApp, ev: &ClickEvent, _, cx| {
                    this.ui.day = d;
                    if ev.click_count() > 1 {
                        this.ui.view = View::Day;
                    }
                    cx.notify();
                })));
            let mut cell = div().class("ad");
            for o in occ.iter().filter(|o| self.data.events[o.idx].all_day && o.start.div_euclid(DAY_MIN) == d) {
                cell = cell.child(self.chip(o, tag));
            }
            allday = allday.child(cell);
        }
        // the grid: hours, lines, the columns with their events
        let mut hours = div().class("hours");
        let mut grid = div().id(format!("{tag}grid")).class("tgrid");
        for h in 0..24 {
            let y = h as f32 * HOUR_H;
            if h > 0 {
                hours = hours.child(div().class("hl").css("top", format!("{}px", y - 7.0)).child(format!("{h:02}:00")));
            }
            grid = grid.child(div().class("line").css("top", format!("{y}px")))
                .child(div().class("line half").css("top", format!("{}px", y + HOUR_H / 2.0)));
        }
        grid = grid.child(hours);
        let now_local = self.now_min + tz;
        for (ci, &d) in days.iter().enumerate() {
            let col_id = format!("{tag}col{d}");
            let mut col = div().id(col_id.clone()).class(if d == self.ui.today { "col today" } else { "col" });
            let day_occ: Vec<Occ> = occ.iter().copied()
                .filter(|o| !self.data.events[o.idx].all_day && o.start.div_euclid(DAY_MIN) == d).collect();
            let cols = columns(&day_occ);
            for (o, (c, n)) in day_occ.iter().zip(cols) {
                col = col.child(self.block(*o, (c, n), tag, d));
            }
            // the drag being made
            if let Some(Drag::Create { from, to }) = self.ui.drag
                && from.div_euclid(DAY_MIN) == d
            {
                let (a, b) = (from.min(to), from.max(to).max(from.min(to) + SNAP));
                let top = (a - d * DAY_MIN) as f32 / 60.0 * HOUR_H;
                let h = (b - a) as f32 / 60.0 * HOUR_H;
                col = col.child(div().class("newbox").css("top", format!("{top}px")).css("height", format!("{h}px"))
                    .child(format!("{} – {}", clock(a), clock(b))));
            }
            // a moving event's ghost on the day it is over
            if let Some(Drag::Move { id, at, moved: true, .. }) = self.ui.drag
                && at.div_euclid(DAY_MIN) == d
                && let Some(i) = self.data.find(id)
            {
                let e = &self.data.events[i];
                let top = (at - d * DAY_MIN) as f32 / 60.0 * HOUR_H;
                col = col.child(div().class("ev ghost").css("left", "2px").css("right", "2px")
                    .css("top", format!("{top}px")).css("height", format!("{}px", (e.dur as f32 / 60.0 * HOUR_H).max(18.0)))
                    .css("background", hex(self.color_of(e))).child(div().class("t").child(e.title.clone()))
                    .child(div().class("w").child(format!("{} – {}", clock(at), clock(at + e.dur)))));
            }
            if d == now_local.div_euclid(DAY_MIN) {
                // the now line falls on by itself until midnight: one CSS animation, no frames
                let m = now_local.rem_euclid(DAY_MIN);
                let top = m as f32 / 60.0 * HOUR_H;
                let left = (DAY_MIN - m) as f32;
                col = col.child(div().id(format!("{tag}now{m}")).class("now").css("top", format!("{top}px"))
                    .css("--fall", format!("{}px", left / 60.0 * HOUR_H))
                    .css("animation", format!("fall {}s linear forwards", left * 60.0)));
            }
            let id_for_down = col_id.clone();
            let id_for_move = col_id;
            col = col
                .on_pointer_down(w.listener(move |this: &mut CalApp, e: &PointerEvent, win, cx| {
                    let Some(top) = win.bounds_of(&id_for_down).map(|b| b.origin.y) else { return };
                    let m = minute_at(d, top, e.position.y.0);
                    this.ui.day = d;
                    this.ui.sel = None;
                    this.ui.drag = Some(Drag::Create { from: m, to: m + 60 });
                    win.set_pointer_capture(cx);
                    cx.notify();
                }))
                .on_pointer_move(w.listener(move |this: &mut CalApp, e: &PointerEvent, win, cx| {
                    if let Some(Drag::Create { from, .. }) = this.ui.drag
                        && let Some(top) = win.bounds_of(&id_for_move).map(|b| b.origin.y)
                    {
                        let m = minute_at(d, top, e.position.y.0) + SNAP;
                        this.ui.drag = Some(Drag::Create { from, to: m.max(from + SNAP) });
                        cx.notify();
                    }
                }))
                .on_pointer_up(w.listener(move |this: &mut CalApp, _: &PointerEvent, win, cx| {
                    win.release_pointer_capture(cx);
                    if this.release() {
                        cx.notify();
                    }
                }));
            let _ = ci;
            grid = grid.child(col);
        }
        div().class("view")
            .child(head)
            .child(allday)
            .child(div().id(format!("{tag}body")).class("body").child(grid))
    }

    /// An event in a time grid: `(column, columns)` side by side with its overlaps.
    fn block(&self, o: Occ, (c, n): (usize, usize), tag: &'static str, d: i64) -> Div {
        let w = self.weak;
        let e = &self.data.events[o.idx];
        let id = e.id;
        let top = (o.start - d * DAY_MIN) as f32 / 60.0 * HOUR_H;
        let h = (o.dur as f32 / 60.0 * HOUR_H).max(18.0);
        let mut cls = String::from("ev");
        if self.ui.sel == Some(id) {
            cls.push_str(" sel");
        }
        if self.ui.born == id {
            cls.push_str(" born");
        }
        let moving = matches!(self.ui.drag, Some(Drag::Move { id: m, moved: true, .. }) if m == id);
        if moving {
            cls.push_str(" ghost");
        }
        let end = match self.ui.drag {
            Some(Drag::Resize { id: r, end }) if r == id => end,
            _ => o.start + o.dur,
        };
        let h = if end == o.start + o.dur { h } else { ((end - o.start) as f32 / 60.0 * HOUR_H).max(18.0) };
        let width = 100.0 / n as f32;
        let block_id = format!("{tag}ev{id}_{}", o.k);
        let grid_id = format!("{tag}grid");
        let grid_id2 = grid_id.clone();
        let days = self.week_days();
        let tz = self.tz();
        let start_utc = e.start;
        let k = i64::from(o.k);
        let mut b = div().id(block_id).class(cls)
            .css("left", format!("calc({}% + 2px)", width * c as f32)).css("width", format!("calc({width}% - 4px)"))
            .css("top", format!("{top}px")).css("height", format!("{h}px")).css("background", hex(self.color_of(e)))
            .child(div().class("t").child(e.title.clone()));
        if h > 30.0 {
            b = b.child(div().class("w").child(format!("{} – {}", clock(o.start), clock(end))));
        }
        b.child(div().id(format!("{tag}grip{id}_{}", o.k)).class("grip")
                .on_pointer_down(w.listener(move |this: &mut CalApp, _: &PointerEvent, win, cx| {
                    this.ui.sel = Some(id);
                    this.ui.day = d;
                    this.ui.drag = Some(Drag::Resize { id, end: start_utc + tz + k * 7 * DAY_MIN + o.dur });
                    win.set_pointer_capture(cx);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .on_pointer_move(w.listener(move |this: &mut CalApp, e: &PointerEvent, win, cx| {
                    if let Some(Drag::Resize { id: r, .. }) = this.ui.drag
                        && r == id
                        && let Some(g) = win.bounds_of(&grid_id2)
                    {
                        let m = minute_at(d, g.origin.y, e.position.y.0) + SNAP;
                        this.ui.drag = Some(Drag::Resize { id, end: m.max(o.start + SNAP) });
                        cx.notify();
                    }
                }))
                .on_pointer_up(w.listener(|this: &mut CalApp, _: &PointerEvent, win, cx| {
                    win.release_pointer_capture(cx);
                    if this.release() {
                        cx.notify();
                    }
                })))
            .on_pointer_down(w.listener(move |this: &mut CalApp, e: &PointerEvent, win, cx| {
                let Some(g) = win.bounds_of(&grid_id) else { return };
                let at = minute_at(d, g.origin.y, e.position.y.0);
                this.ui.sel = Some(id);
                this.ui.day = d;
                this.ui.drag = Some(Drag::Move { id, grab: at - o.start, at: o.start, moved: false });
                win.set_pointer_capture(cx);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_pointer_move(w.listener(move |this: &mut CalApp, e: &PointerEvent, win, cx| {
                let Some(Drag::Move { id: m, grab, at, .. }) = this.ui.drag else { return };
                if m != id {
                    return;
                }
                let Some(g) = win.bounds_of(&format!("{tag}grid")) else { return };
                // which day column the pointer is over, and the time there
                let col_w = (g.size.width - GUTTER) / days.len() as f32;
                let ci = (((e.position.x.0 - g.origin.x - GUTTER) / col_w).floor() as i64).clamp(0, i64::try_from(days.len()).unwrap_or(1) - 1);
                let day = days[ci as usize];
                let t = minute_at(day, g.origin.y, e.position.y.0) - grab;
                let t = day * DAY_MIN + (t - day * DAY_MIN).clamp(0, DAY_MIN - SNAP);
                if t != at {
                    this.ui.drag = Some(Drag::Move { id, grab, at: t, moved: true });
                    cx.notify();
                }
            }))
            .on_pointer_up(w.listener(|this: &mut CalApp, _: &PointerEvent, win, cx| {
                win.release_pointer_capture(cx);
                if this.release() {
                    cx.notify();
                }
            }))
    }

    /// An all-day or month chip.
    fn chip(&self, o: &Occ, tag: &'static str) -> Div {
        let w = self.weak;
        let e = &self.data.events[o.idx];
        let id = e.id;
        let mut cls = String::from("chip");
        if self.ui.sel == Some(id) {
            cls.push_str(" sel");
        }
        if matches!(self.ui.drag, Some(Drag::Chip { id: m, .. }) if m == id) {
            cls.push_str(" ghost");
        }
        let label = if e.all_day { e.title.clone() } else { format!("{} {}", clock(o.start), e.title) };
        let day = o.start.div_euclid(DAY_MIN);
        div().id(format!("{tag}chip{id}_{}", o.k)).class(cls).css("background", hex(self.color_of(e))).child(label)
            .on_pointer_down(w.listener(move |this: &mut CalApp, _: &PointerEvent, _, cx| {
                this.ui.sel = Some(id);
                this.ui.day = day;
                this.ui.drag = Some(Drag::Chip { id, day });
                cx.stop_propagation();
                cx.notify();
            }))
    }

    fn month(&self) -> Div {
        let w = self.weak;
        let tz = self.tz();
        let first = month_start(self.ui.day);
        let grid0 = first - weekday(first);
        let occ = self.data.occurrences(grid0, grid0 + 42, tz, &self.ui.search);
        let (_, m, _) = civil(self.ui.day);
        let mut g = div().class("month");
        for d in DAYS {
            g = g.child(div().class("mh").child(d));
        }
        let drop_day = match self.ui.drag {
            Some(Drag::Chip { day, .. }) => Some(day),
            _ => None,
        };
        for z in grid0..grid0 + 42 {
            let (_, zm, zd) = civil(z);
            let mut cls = String::from("md");
            if zm != m {
                cls.push_str(" out");
            }
            if z == self.ui.today {
                cls.push_str(" today");
            }
            if drop_day == Some(z) {
                cls.push_str(" drop");
            } else if z == self.ui.day {
                cls.push_str(" sel");
            }
            let mut cell = div().id(format!("md{z}")).class(cls).child(div().class("n").child(zd.to_string()));
            let mine: Vec<&Occ> = occ.iter().filter(|o| o.start.div_euclid(DAY_MIN) == z).collect();
            for o in mine.iter().take(3) {
                cell = cell.child(self.chip(o, "m"));
            }
            if mine.len() > 3 {
                cell = cell.child(div().class("more").child(format!("+{} more", mine.len() - 3)));
            }
            g = g.child(cell
                .on_pointer_enter(w.listener(move |this: &mut CalApp, _: &PointerEvent, _, cx| {
                    if let Some(Drag::Chip { id, day }) = this.ui.drag
                        && day != z
                    {
                        this.ui.drag = Some(Drag::Chip { id, day: z });
                        cx.notify();
                    }
                }))
                .on_click(w.listener(move |this: &mut CalApp, e: &ClickEvent, _, cx| {
                    this.ui.day = z;
                    if e.click_count() > 1 {
                        this.create(z * DAY_MIN + 9 * 60, 60, false);
                    }
                    cx.notify();
                })));
        }
        div().class("view").child(g)
    }

    fn agenda(&self) -> Div {
        let w = self.weak;
        let tz = self.tz();
        let from = self.ui.day - weekday(self.ui.day);
        let occ = self.data.occurrences(from, from + 21, tz, &self.ui.search);
        let mut list = div().id("agenda").class("agenda");
        let mut last = i64::MIN;
        for o in &occ {
            let e = &self.data.events[o.idx];
            let day = o.start.div_euclid(DAY_MIN);
            if day != last {
                list = list.child(div().class("ag-day").child(if day == self.ui.today { format!("Today · {}", date_label(day)) } else { date_label(day) }));
                last = day;
            }
            let id = e.id;
            let when = if e.all_day { "all day".to_string() } else { format!("{} – {}", clock(o.start), clock(o.start + o.dur)) };
            list = list.child(div().id(format!("ag{id}_{}", o.k)).class(if self.ui.sel == Some(id) { "ag sel" } else { "ag" })
                .child(div().class("dot").css("background", hex(self.color_of(e))))
                .child(span().class("tm").child(when))
                .child(span().child(e.title.clone()))
                .child(span().class("nt").child(e.notes.clone()))
                .on_click(w.listener(move |this: &mut CalApp, _: &ClickEvent, _, cx| {
                    this.ui.sel = Some(id);
                    this.ui.day = day;
                    cx.notify();
                })));
        }
        if occ.is_empty() {
            list = list.child(div().class("empty").child(if self.ui.search.is_empty() { "Nothing in these three weeks" } else { "Nothing matches the search" }));
        }
        div().class("view").child(list)
    }

    fn header(&self) -> Div {
        let w = self.weak;
        let mut d = div().class("hdr")
            .child(Button().id("today").child("Today").on_click(w.listener(|this: &mut CalApp, _: &ClickEvent, _, cx| {
                this.ui.day = this.ui.today;
                cx.notify();
            })))
            .child(Button().id("prev").child("‹").css("width", "28px").on_click(w.listener(|this: &mut CalApp, _: &ClickEvent, _, cx| {
                this.step(-1);
                cx.notify();
            })))
            .child(Button().id("next").child("›").css("width", "28px").on_click(w.listener(|this: &mut CalApp, _: &ClickEvent, _, cx| {
                this.step(1);
                cx.notify();
            })))
            .child(span().class("title").child(self.title()))
            .child(div().class("sep"));
        for (label, v, _) in VIEWS {
            d = d.child(Radio().id(format!("v{label}")).attr("name", "view").checked(self.ui.view == v)
                .on_change(w.listener(move |this: &mut CalApp, e: &ChangeEvent, _, cx| {
                    if e.checked {
                        this.ui.view = v;
                        cx.notify();
                    }
                })))
                .child(span().class("lbl").child(label));
        }
        let zones = ZONES.iter().fold(Dropdown().id("tz").css("width", "150px").value(ZONES[self.ui.tz].0), |d, z| d.child(option().child(z.0)));
        d.child(div().class("sep"))
            .child(TextField().id("search").attr("live", "").value(self.ui.search.clone()).placeholder("Search events")
                .css("width", "170px")
                .on_input(w.listener(|this: &mut CalApp, e: &ChangeEvent, _, cx| {
                    this.ui.search.clone_from(&e.value);
                    this.ui.status = if e.value.is_empty() { "Showing everything".into() } else { format!("Search: {}", e.value) };
                    cx.notify();
                })))
            .child(span().class("lbl").child("Zone"))
            .child(zones.on_change(w.listener(|this: &mut CalApp, e: &ChangeEvent, _, cx| {
                this.ui.tz = (e.number as usize).min(ZONES.len() - 1);
                this.ui.status = format!("Times in {}", ZONES[this.ui.tz].0);
                cx.notify();
            })))
    }

    fn tools(&self) -> Div {
        let w = self.weak;
        let first = month_start(self.ui.day);
        let grid0 = first - weekday(first);
        let (y, m, _) = civil(self.ui.day);
        let busy = self.data.occurrences(grid0, grid0 + 42, self.tz(), "");
        let mut mini = div().class("mini");
        for d in ["M", "T", "W", "T", "F", "S", "S"] {
            mini = mini.child(div().class("h").child(d));
        }
        for z in grid0..grid0 + 42 {
            let (_, zm, zd) = civil(z);
            let mut cls = String::from("d");
            if zm != m {
                cls.push_str(" out");
            }
            if busy.iter().any(|o| o.start.div_euclid(DAY_MIN) == z) {
                cls.push_str(" busy");
            }
            if z == self.ui.day {
                cls.push_str(" sel");
            }
            if z == self.ui.today {
                cls.push_str(" today");
            }
            mini = mini.child(div().id(format!("mini{z}")).class(cls).child(zd.to_string())
                .on_click(w.listener(move |this: &mut CalApp, _: &ClickEvent, _, cx| {
                    this.ui.day = z;
                    cx.notify();
                })));
        }
        let mut t = div().class("tools")
            .child(div().class("mhead")
                .child(div().id("miniprev").class("arrow").child("‹").on_click(w.listener(|this: &mut CalApp, _: &ClickEvent, _, cx| {
                    this.ui.day = add_months(this.ui.day, -1);
                    cx.notify();
                })))
                .child(span().child(format!("{} {y}", &MONTHS[(m - 1) as usize][..3])))
                .child(div().id("mininext").class("arrow").child("›").on_click(w.listener(|this: &mut CalApp, _: &ClickEvent, _, cx| {
                    this.ui.day = add_months(this.ui.day, 1);
                    cx.notify();
                }))))
            .child(mini)
            .child(details().open(true).child(summary().child("Calendars")));
        for (i, c) in self.data.cals.iter().enumerate() {
            let n = self.data.events.iter().filter(|e| e.cal == i).count();
            t = t.child(div().class("calrow")
                .child(Checkbox().id(format!("cal{i}")).checked(c.shown)
                    .on_change(w.listener(move |this: &mut CalApp, e: &ChangeEvent, _, cx| {
                        let on = e.checked;
                        let name = this.data.cals[i].name;
                        this.change(if on { "Shown" } else { "Hidden" }, |d| d.cals[i].shown = on);
                        this.ui.status = format!("{name} {}", if on { "shown" } else { "hidden" });
                        cx.notify();
                    })))
                .child(span().class("nm").child(format!("{} ({n})", c.name)))
                .child(ColorSwatch().id(format!("calcol{i}")).value(hex(c.color)).css("width", "34px")
                    .on_change(w.listener(move |this: &mut CalApp, e: &ChangeEvent, _, cx| {
                        if let Ok(col) = u32::from_str_radix(e.value.trim_start_matches('#'), 16) {
                            this.change("Recolored", |d| d.cals[i].color = col);
                            cx.notify();
                        }
                    }))));
        }
        t.child(p().class("hint").child("Drag on empty time to add an event. Drag an event to move it, its bottom edge to resize it."))
    }

    /// The selected event's editor (the sidebar).
    fn editor(&self) -> Div {
        let w = self.weak;
        let Some(e) = self.ui.sel.and_then(|id| self.data.find(id)).map(|i| &self.data.events[i]) else {
            return div().class("side").child(details().open(true).child(summary().child("Event"))
                .child(p().class("hint").child("No event selected. Click one, or drag on empty time in the week to make one.")));
        };
        let tz = self.tz();
        let local = e.start + tz;
        let row = |k: &str| div().class("row").child(span().class("k").child(k.to_string()));
        let cal_dd = self.data.cals.iter().fold(Dropdown().id("ecal").css("width", "140px").value(self.data.cals[e.cal].name),
                                                 |d, c| d.child(option().child(c.name)));
        let start_h = (local.rem_euclid(DAY_MIN)) as f64 / 60.0;
        let id = e.id;
        div().class("side")
            .child(div().class("colorbar").css("background", hex(self.color_of(e))))
            .child(details().open(true).child(summary().child("Event"))
                .child(row("Title").child(TextField().id("etitle").value(e.title.clone()).css("width", "170px")
                    .on_change(w.listener(|this: &mut CalApp, ev: &ChangeEvent, win, cx| {
                        win.focus_id("root");
                        let t = ev.value.trim().to_string();
                        if !t.is_empty() {
                            this.edit("Renamed", |e| e.title = t);
                        }
                        cx.notify();
                    }))))
                .child(row("Calendar").child(cal_dd.on_change(w.listener(|this: &mut CalApp, ev: &ChangeEvent, _, cx| {
                    let c = (ev.number as usize).min(this.data.cals.len() - 1);
                    this.edit("Calendar changed", |e| e.cal = c);
                    cx.notify();
                }))))
                .child(row("Day").child(span().child(date_label(local.div_euclid(DAY_MIN)))))
                .child(row("Starts (h)").child(NumberField().id("estart").min(0.0).max(23.75).step(0.25).precision(2)
                    .value(format!("{start_h:.2}")).css("width", "120px")
                    .on_change(w.listener(move |this: &mut CalApp, ev: &ChangeEvent, _, cx| {
                        let m = ((ev.number.clamp(0.0, 23.75) * 60.0) as i64).div_euclid(SNAP) * SNAP;
                        let tz = this.tz();
                        this.edit(&format!("Starts {}", clock(m)), |e| {
                            let day = (e.start + tz).div_euclid(DAY_MIN);
                            e.start = day * DAY_MIN + m - tz;
                        });
                        cx.notify();
                    }))))
                .child(row("Minutes").child(NumberField().id("edur").min(15.0).max(1440.0).step(15.0).value(e.dur.to_string())
                    .css("width", "120px")
                    .on_change(w.listener(|this: &mut CalApp, ev: &ChangeEvent, _, cx| {
                        let d = (ev.number.round() as i64).clamp(SNAP, DAY_MIN);
                        this.edit(&format!("Lasts {d} min"), |e| e.dur = d);
                        cx.notify();
                    }))))
                .child(p().class("hint").child("Drag the numbers, or double-click and type math: 1.5*60"))
                .child(row("All day").child(Checkbox().id("eallday").checked(e.all_day)
                    .on_change(w.listener(move |this: &mut CalApp, ev: &ChangeEvent, _, cx| {
                        let on = ev.checked;
                        let tz = this.tz();
                        this.edit(if on { "All day" } else { "Timed" }, |e| {
                            e.all_day = on;
                            if on {
                                e.start = (e.start + tz).div_euclid(DAY_MIN) * DAY_MIN - tz;
                                e.dur = DAY_MIN;
                            } else {
                                e.start += 9 * 60;
                                e.dur = 60;
                            }
                        });
                        cx.notify();
                    }))))
                .child(row("Weekly").child(Checkbox().id("erepeat").checked(e.repeat > 1)
                    .on_change(w.listener(|this: &mut CalApp, ev: &ChangeEvent, _, cx| {
                        let n = if ev.checked { 6 } else { 1 };
                        this.edit(if ev.checked { "Repeats weekly" } else { "Once" }, |e| e.repeat = n);
                        cx.notify();
                    }))))
                .child(row("Times").child(NumberField().id("ecount").min(1.0).max(52.0).step(1.0).value(e.repeat.to_string())
                    .disabled(e.repeat <= 1).css("width", "120px")
                    .on_change(w.listener(|this: &mut CalApp, ev: &ChangeEvent, _, cx| {
                        let n = (ev.number.round() as u32).clamp(1, 52);
                        this.edit(&format!("{n} weeks"), |e| e.repeat = n);
                        cx.notify();
                    })))))
            .child(details().open(true).child(summary().child("Notes"))
                .child(TextArea().id("enotes").value(e.notes.clone()).attr("rows", "5").css("width", "100%")
                    .on_change(w.listener(|this: &mut CalApp, ev: &ChangeEvent, _, cx| {
                        let n = ev.value.clone();
                        this.edit("Notes saved", |e| e.notes = n);
                        cx.notify();
                    }))))
            .child(div().class("row")
                .child(Button().id("edup").child("Duplicate").on_click(w.listener(|this: &mut CalApp, _: &ClickEvent, _, cx| {
                    if this.duplicate() {
                        cx.notify();
                    }
                })))
                .child(Button().id("edel").child("Delete").on_click(w.listener(move |this: &mut CalApp, _: &ClickEvent, _, cx| {
                    this.ui.sel = Some(id);
                    if this.delete() {
                        cx.notify();
                    }
                }))))
    }

    fn footer(&self) -> Div {
        let tz = self.tz();
        let days = self.week_days();
        let occ = self.data.occurrences(days[0], days[days.len() - 1] + 1, tz, &self.ui.search);
        let mins: i64 = occ.iter().filter(|o| !self.data.events[o.idx].all_day).map(|o| o.dur).sum();
        let mut f = div().class("foot")
            .child(span().class("st").child(self.ui.status.clone()))
            .child(span().child(format!("{} events · {}h {:02}m booked", occ.len(), mins / 60, mins % 60)));
        let mut s = String::new();
        let _ = write!(s, "Now {}", clock(self.now_min + tz));
        f = f.child(span().css("margin-left", "auto").child(s));
        f
    }

    fn main_editor(&self) -> Editor {
        let body = match self.ui.view {
            View::Week | View::Day => self.time_grid(&self.week_days(), "w"),
            View::Month => self.month(),
            View::Agenda => self.agenda(),
        };
        Editor::new(body)
            .header(self.header())
            .toolbar(Region::new(self.tools()).size(200).overlap(false))
            .sidebar(Region::new(self.editor()).size(290).overlap(false))
            .footer(Region::new(self.footer()).size(26).overlap(false))
    }

    fn day_editor(&self) -> Editor {
        let w = self.weak;
        Editor::new(self.time_grid(&[self.ui.day], "d"))
            .header(div().class("hdr")
                .child(Button().id("dprev").child("‹").css("width", "28px").on_click(w.listener(|this: &mut CalApp, _: &ClickEvent, _, cx| {
                    this.ui.day -= 1;
                    cx.notify();
                })))
                .child(span().class("title").child(date_label(self.ui.day)))
                .child(Button().id("dnext").child("›").css("width", "28px").on_click(w.listener(|this: &mut CalApp, _: &ClickEvent, _, cx| {
                    this.ui.day += 1;
                    cx.notify();
                }))))
    }
}

impl Render for CalApp {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        focus_root(w, &mut self.frames);
        let now_min = self.start_minute + (now() / 60.0) as i64;
        let snap = Arc::new(Snap { data: self.data.clone(), ui: self.ui.clone(), weak: cx.weak_entity(), now_min });
        let (s1, s2, s3) = (snap.clone(), snap.clone(), snap);
        let editors = Editors::new()
            .editor("Calendar", "TIME", move |_: &AreaRef| s1.main_editor())
            .editor("Day", "SEQUENCE", move |_: &AreaRef| s2.day_editor())
            .editor("Agenda", "TEXT", move |_: &AreaRef| Editor::new(s3.agenda()));
        app().id("root").attr("tabindex", "0")
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                // a key the calendar takes is not Blender's as well (Ctrl+Z would undo a field edit too)
                if !w.text_field_focused() && this.key(e) {
                    cx.prevent_default();
                    cx.notify();
                }
            }))
            .on_pointer_up(cx.listener(|this, _: &PointerEvent, _, cx| if this.release() { cx.notify() }))
            .child(style().child(CSS))
            .child(Screen().editors(editors).class("scr").layout(AreaSplit::v(0.74, "Calendar", "Day")))
    }
}

/// Every view, a drag to create, a move, a resize, the editor, the month drag, search, zones, undo.
fn storyboard(dir: String) -> Script {
    story(dir, |s| {
        s.play("00-week", &[0, 300]);
        s.cx.simulate_mouse_move(s.win, 5.0, 5.0);
        // scroll the week body to the working day
        let body = s.bounds("wbody");
        let (bx, by) = (body.origin.x + 300.0, body.origin.y + 200.0);
        s.cx.simulate_scroll(s.win, bx, by, -10);
        s.play("01-scrolled", &[0]);
        let app_days = {
            let wall = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs().cast_signed() / 60);
            let today = wall.div_euclid(DAY_MIN);
            today - weekday(today)
        };
        // drag on empty Thursday time to make an event
        let col = s.bounds(&format!("wcol{}", app_days + 3));
        let x = col.origin.x + col.size.width / 2.0;
        let y0 = col.origin.y + 13.0 * HOUR_H + 4.0;
        s.cx.simulate_drag(s.win, (x, y0), (x, y0 + 1.5 * HOUR_H));
        s.play("02-created", &[0, 300]);
        // rename it in the sidebar
        s.click("etitle");
        s.tick(100);
        s.cx.simulate_key_with(s.win, "a", "a", true, false);
        s.type_text("Shot 14 review");
        s.key("Return");
        s.hover("wbody");
        s.play("03-renamed", &[0]);
        // move the climbing session on Tuesday to Wednesday evening
        let ev = s.bounds(&format!("wev{}_0", 11));
        let (ex, ey) = (ev.origin.x + ev.size.width / 2.0, ev.origin.y + 10.0);
        s.cx.simulate_drag(s.win, (ex, ey), (ex + col.size.width, ey + HOUR_H));
        s.play("04-moved", &[0, 200]);
        // resize the lunch by its bottom edge
        let g = s.bounds(&format!("wgrip{}_0", 8));
        let (gx, gy) = (g.origin.x + g.size.width / 2.0, g.origin.y + g.size.height / 2.0);
        s.cx.simulate_drag(s.win, (gx, gy), (gx, gy + HOUR_H));
        s.play("05-resized", &[0]);
        // the month: drag the offsite chip to the next Monday
        s.key("m");
        s.play("06-month", &[0, 200]);
        let chip = format!("mchip{}_0", 18);
        if let Some(c) = s.cx.element_bounds(s.win, &chip) {
            let to = s.bounds(&format!("md{}", app_days + 7));
            s.cx.simulate_drag(s.win, (c.origin.x + 10.0, c.origin.y + 6.0),
                               (to.origin.x + to.size.width / 2.0, to.origin.y + to.size.height / 2.0));
        }
        s.play("07-chip-moved", &[0, 200]);
        // the agenda, searched
        s.key("a");
        s.click("search");
        s.tick(100);
        s.type_text("farm");
        s.key("Return");
        s.hover("agenda");
        s.play("08-agenda-search", &[0]);
        s.click("search");
        s.tick(100);
        s.cx.simulate_key_with(s.win, "a", "a", true, false);
        s.key("BackSpace");
        s.key("Return");
        s.hover("agenda");
        // a calendar hidden, Tokyo time, the day view
        s.key("w");
        s.click("cal1");
        s.play("09-personal-hidden", &[0, 200]);
        s.click("cal1");
        s.key("d");
        s.play("10-day", &[0]);
        s.key("w");
        s.key("Left");
        s.play("11-last-week", &[0]);
        s.key("t");
        s.cx.simulate_key_with(s.win, "z", "z", true, false);
        s.play("12-undo", &[0, 200]);
    })
}

fn main() {
    let (w, h) = size_arg((1500.0, 920.0));
    run_with(w, h, "BUI Calendar", CalApp::new, arg("--storyboard").map(storyboard));
}

//! BUI Chat: a community chat app built from Blender's own interface. The window is a Blender
//! screen: the server rail is the Channels editor's toolbar region, your user panel its footer,
//! the composer the chat's footer, threads open in the chat's sidebar, and the members list is an
//! editor of its own. Split the chat area (its header's Split, or drag an area corner) and each
//! area keeps its own channel, so two channels sit side by side; Maximize gives one the window.
//!
//! What it does: four servers with unread pips and mention badges; text and voice channels in
//! categories (Blender panels you fold); messages grouped by author, with reactions you toggle,
//! replies that quote, and threads; a composer that sends on Enter (a Blender text field confirms
//! on Enter), after which someone starts typing and answers; mail-like background activity that
//! lights other channels; a voice channel with a live speaking meter drawn with BUI's `Painter`, a
//! volume slider per person, mute and deafen; and settings that act at once: your status (a
//! Blender menu), name color (Blender's color picker), accent (radio buttons), compact mode and
//! avatars (checkboxes), and a UI scale slider that resizes the whole app.
//!
//! Keys (with the pointer over a chat): Alt+Up / Alt+Down channels, Esc cancels a reply or closes a
//! thread, Ctrl+Z takes back your last message.
//!
//! `cargo run --release --bin bui_chat`, `-- --storyboard DIR` drives it headlessly.

use std::rc::Rc;
use std::sync::OnceLock;

use blenderui::prelude::*;
use blenderui::screen::{AreaRef, AreaSplit, Axis, Editor, Editors, Region, Screen};
use bui_demos::story::story;
use bui_demos::{arg, focus_root, now, run_with, size_arg, Script};
use bui_demos::area_id;

// ==========================================================================
// The community

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Online,
    Idle,
    Busy,
    Offline,
}

impl Status {
    const ALL: [Self; 4] = [Self::Online, Self::Idle, Self::Busy, Self::Offline];

    const fn name(self) -> &'static str {
        match self {
            Self::Online => "Online",
            Self::Idle => "Idle",
            Self::Busy => "Do not disturb",
            Self::Offline => "Offline",
        }
    }

    const fn color(self) -> &'static str {
        match self {
            Self::Online => "#43b581",
            Self::Idle => "#faa61a",
            Self::Busy => "#f04747",
            Self::Offline => "#747f8d",
        }
    }
}

struct Member {
    name: String,
    color: String,
    status: Status,
    role: &'static str,
    /// Voice: their volume (0..2) and whether you muted them.
    volume: f64,
}

#[derive(Clone)]
struct Line {
    id: u32,
    author: usize,
    text: String,
    /// Minutes after midnight today (09:41 is now at the start).
    at: u32,
    reactions: Vec<(&'static str, u32, bool)>,
    reply_to: Option<u32>,
    thread: Vec<Self>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Voice,
}

struct Channel {
    name: &'static str,
    kind: Kind,
    category: &'static str,
    topic: &'static str,
    lines: Vec<Line>,
    unread: usize,
    mentions: usize,
    /// Voice: who is in it.
    voice: Vec<usize>,
}

struct Server {
    name: &'static str,
    glyph: &'static str,
    color: &'static str,
    channels: Vec<Channel>,
}

/// The members (0 is you).
const MEMBERS: [(&str, &str, Status, &str); 12] = [
    ("you", "#e3913c", Status::Online, "Lighting"),
    ("Ana", "#5aa0e0", Status::Online, "Lighting"),
    ("Jonas", "#43b581", Status::Online, "Pipeline"),
    ("Kenji", "#b38fe0", Status::Idle, "Rigging"),
    ("Priya", "#e06b8f", Status::Online, "Production"),
    ("Tomas", "#e0c35a", Status::Busy, "FX"),
    ("Yuki", "#5ae0c8", Status::Online, "Texturing"),
    ("Omar", "#e07a5a", Status::Idle, "Color"),
    ("Lena", "#8fb3e0", Status::Offline, "Guest"),
    ("Sam", "#a0e05a", Status::Online, "Animation"),
    ("Bot", "#9aa4b0", Status::Online, "Bot"),
    ("Mo", "#e05a9e", Status::Offline, "Editorial"),
];

const EMOJI: [&str; 6] = ["👍", "🔥", "😂", "🎉", "👀", "✔"];

type SeedLine = (usize, &'static str, u32, &'static [(&'static str, u32)]);

fn lines(seed: &[SeedLine], first: u32) -> Vec<Line> {
    seed.iter().enumerate().map(|(i, &(author, text, at, reacts))| Line {
        id: first + i as u32,
        author,
        text: text.into(),
        at,
        reactions: reacts.iter().map(|&(e, n)| (e, n, false)).collect(),
        reply_to: None,
        thread: Vec::new(),
    }).collect()
}

fn text(name: &'static str, category: &'static str, topic: &'static str, seed: &[SeedLine], first: u32) -> Channel {
    Channel { name, kind: Kind::Text, category, topic, lines: lines(seed, first), unread: 0, mentions: 0, voice: Vec::new() }
}

fn voice(name: &'static str, who: &[usize]) -> Channel {
    Channel { name, kind: Kind::Voice, category: "Voice", topic: "", lines: Vec::new(), unread: 0, mentions: 0, voice: who.to_vec() }
}

fn community() -> Vec<Server> {
    let mut general = text("general", "Studio", "Everything Lumen. Be kind, post renders.", &[
        (2, "morning all ☕ farm is back online, queue is empty", 520, &[("👍", 4)]),
        (1, "finally! I have 40 frames of shot 052 waiting", 523, &[]),
        (1, "rendering now", 523, &[("🔥", 2)]),
        (4, "reminder: dailies at 16:00 in screening room B", 540, &[("👀", 3)]),
        (9, "is anyone else seeing the viewport stutter with the new driver?", 561, &[]),
        (2, "yes, roll back to the previous one for now. I'll pin it in #pipeline", 563, &[("✔", 5)]),
        (6, "new tileable concrete set is in the library, 8K", 570, &[("🎉", 6), ("🔥", 3)]),
        (3, "the rig review moved to 11:00 btw", 575, &[]),
        (1, "@you can you check the rim light on 052 when you're in?", 579, &[]),
    ], 1);
    general.lines[5].thread = lines(&[
        (9, "rolled back, smooth again. thanks!", 566, &[("👍", 1)]),
        (5, "same here on the FX boxes", 568, &[]),
        (2, "I'll update the install script", 569, &[]),
    ], 900);
    general.lines[4].thread = vec![];
    general.mentions = 1;
    general.unread = 3;
    let renders = text("renders", "Studio", "Post your frames. Critique welcome, be specific.", &[
        (6, "lookdev turntable for the lantern, v3", 480, &[("🔥", 5)]),
        (7, "lovely. the glass reads a touch green under AgX", 486, &[]),
        (6, "good eye, fixed the IOR tint", 501, &[("👍", 2)]),
        (5, "first pass of the waterfall sim, 240 frames", 550, &[("🎉", 4), ("👀", 2)]),
    ], 20);
    let pipeline = text("pipeline", "Studio", "Tools, builds, the farm. Bot posts here.", &[
        (10, "✔ nightly #1842 passed in 14m 08s", 506, &[]),
        (2, "pinned: driver 551.23 is the good one, avoid 560.x", 564, &[("✔", 7)]),
        (10, "farm: 24/24 nodes online, queue 0", 571, &[]),
    ], 40);
    let random = text("random", "Off topic", "Cats, coffee, cameras.", &[
        (9, "my cat walked over the keyboard and keyed a whole animation", 512, &[("😂", 9)]),
        (1, "honestly better timing than my last one", 513, &[("😂", 6)]),
        (11, "lunch: thai downstairs?", 555, &[("👍", 3)]),
    ], 60);
    let jobs = text("jobs-board", "Off topic", "Who is hiring, who is available.", &[
        (4, "we're looking for a freelance compositor for 3 weeks in December", 400, &[("👀", 4)]),
    ], 80);
    let studio = Server {
        name: "Lumen Studio",
        glyph: "L",
        color: "#e3913c",
        channels: vec![general, renders, pipeline, random, jobs, voice("Lighting sync", &[1, 2, 9]), voice("Quiet room", &[])],
    };
    let mut nodes = text("node-wrangling", "Learn", "Geometry and shader nodes: questions and tricks.", &[
        (3, "how do I scatter only on upward faces?", 300, &[]),
        (6, "compare the face normal's Z with 0.7 and use that as the selection", 304, &[("✔", 3)]),
        (3, "perfect, thank you", 306, &[("👍", 1)]),
    ], 100);
    nodes.unread = 2;
    let blender_club = Server {
        name: "Nodes Club",
        glyph: "N",
        color: "#5aa0e0",
        channels: vec![nodes, text("showcase", "Learn", "Share what you made.", &[
            (9, "procedural brick wall, all nodes, no textures", 330, &[("🔥", 12)]),
        ], 120), voice("Study hall", &[3, 6])],
    };
    let mut lounge = text("lounge", "Hangout", "The couch.", &[
        (8, "anyone up for the film festival on the 14th?", 200, &[("🎉", 2)]),
        (11, "I'm in!", 210, &[]),
    ], 140);
    lounge.unread = 5;
    lounge.mentions = 2;
    let friends = Server { name: "Weekend Crew", glyph: "W", color: "#43b581", channels: vec![lounge, voice("Couch", &[8])] };
    let bikes = Server {
        name: "Spokes",
        glyph: "S",
        color: "#e06b8f",
        channels: vec![text("saturday-ride", "Rides", "Routes, times, coffee stops.", &[
            (9, "62 km loop, meet at the bakery 8:00", 150, &[("👍", 6)]),
        ], 160)],
    };
    vec![studio, blender_club, friends, bikes]
}

/// What others answer when you post (one after another).
const ANSWERS: [(usize, &str); 6] = [
    (1, "oh nice, on it"),
    (2, "makes sense to me 👍"),
    (9, "can you drop the file in the share?"),
    (6, "looks great already"),
    (4, "noted, I'll add it to the dailies list"),
    (3, "ha, same thought"),
];

/// Chatter that arrives in other channels: (server, channel, author, text).
const CHATTER: [(usize, usize, usize, &str); 5] = [
    (0, 1, 5, "waterfall v2 with more spray, 240 frames"),
    (1, 0, 6, "tip: use a Merge by Distance after the instance realize"),
    (2, 0, 11, "@you festival tickets are on sale!"),
    (0, 2, 10, "✔ nightly #1843 passed"),
    (3, 0, 9, "rain tomorrow, ride moved to Sunday"),
];

// ==========================================================================
// State

const ACCENTS: [(&str, &str); 4] = [("Blender blue", "#4772b3"), ("Orange", "#e3913c"), ("Green", "#43b581"), ("Purple", "#9a6ee0")];

struct Settings {
    compact: bool,
    avatars: bool,
    show_ids: bool,
    accent: usize,
    scale: f64,
}

#[allow(clippy::struct_excessive_bools)]
struct Chat {
    servers: Vec<Server>,
    members: Vec<Member>,
    /// The server and channel of each chat area (by the area), and of the last one used.
    areas: Vec<(AreaRef, (usize, usize))>,
    active: (usize, usize),
    /// The chat area keys and clicks act on last.
    focus_area: Option<AreaRef>,
    replying: Option<u32>,
    thread: Option<u32>,
    /// The composer: sends so far (keys a fresh field after each, as a value pushed again would
    /// not clear one the user typed in).
    sent: u32,
    my_last: Option<(usize, usize, u32)>,
    typing: Option<(usize, f32, (usize, usize))>,
    answered: usize,
    chatter: usize,
    next_chatter: f32,
    next_id: u32,
    settings: Settings,
    in_voice: Option<(usize, usize)>,
    muted: bool,
    deafened: bool,
    show_settings: bool,
    /// The members list filter (a live Blender text field: each key filters).
    member_filter: String,
    picker: Option<u32>,
    toast: Option<(String, f32)>,
    frames: u32,
}

impl Chat {
    fn new() -> Self {
        let mut chat = Self {
            servers: community(),
            members: MEMBERS.iter().map(|&(n, c, s, r)| Member { name: n.into(), color: c.into(), status: s, role: r, volume: 1.0 }).collect(),
            areas: Vec::new(),
            active: (0, 0),
            focus_area: None,
            replying: None,
            thread: None,
            sent: 0,
            my_last: None,
            typing: None,
            answered: 0,
            chatter: 0,
            next_chatter: 12.0,
            next_id: 1000,
            settings: Settings { compact: false, avatars: true, show_ids: false, accent: 0, scale: 1.0 },
            in_voice: Some((0, 5)),
            muted: false,
            deafened: false,
            show_settings: false,
            member_filter: String::new(),
            picker: None,
            toast: None,
            frames: 0,
        };
        chat.open((0, 0));
        chat
    }

    fn channel(&self, (s, c): (usize, usize)) -> &Channel {
        &self.servers[s].channels[c]
    }

    fn channel_mut(&mut self, (s, c): (usize, usize)) -> &mut Channel {
        &mut self.servers[s].channels[c]
    }

    /// The channel area `a` shows (an area not seen before shows the active one).
    fn of(&self, a: Option<AreaRef>) -> (usize, usize) {
        a.and_then(|a| self.areas.iter().find(|(x, _)| *x == a).map(|(_, c)| *c)).unwrap_or(self.active)
    }

    /// Opens `at` in the chat area used last (and makes it the active channel).
    fn open(&mut self, at: (usize, usize)) {
        self.active = at;
        if let Some(a) = self.focus_area {
            match self.areas.iter_mut().find(|(x, _)| *x == a) {
                Some(e) => e.1 = at,
                None => self.areas.push((a, at)),
            }
        }
        let ch = self.channel_mut(at);
        ch.unread = 0;
        ch.mentions = 0;
        if ch.kind == Kind::Voice {
            self.in_voice = Some(at);
        }
        self.replying = None;
        self.thread = None;
        self.show_settings = false;
    }

    /// Alt+Up / Alt+Down: the previous or next channel of the server.
    fn step(&mut self, down: bool) {
        let (s, c) = self.active;
        let n = self.servers[s].channels.len();
        let next = if down { (c + 1) % n } else { (c + n - 1) % n };
        self.open((s, next));
    }

    fn line_mut(&mut self, at: (usize, usize), id: u32) -> Option<&mut Line> {
        let ch = self.channel_mut(at);
        for l in &mut ch.lines {
            if l.id == id {
                return Some(l);
            }
            if let Some(t) = l.thread.iter_mut().find(|t| t.id == id) {
                return Some(t);
            }
        }
        None
    }

    fn react(&mut self, at: (usize, usize), id: u32, emoji: &'static str) {
        if let Some(l) = self.line_mut(at, id) {
            match l.reactions.iter_mut().position(|r| r.0 == emoji) {
                Some(k) => {
                    let r = &mut l.reactions[k];
                    r.2 = !r.2;
                    if r.2 { r.1 += 1 } else { r.1 -= 1 }
                    if r.1 == 0 {
                        l.reactions.remove(k);
                    }
                }
                None => l.reactions.push((emoji, 1, true)),
            }
        }
        self.picker = None;
    }

    /// Posts `text` as you in `at` (in the open thread when there is one), then someone answers.
    fn post(&mut self, at: (usize, usize), text: &str) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let line = Line { id, author: 0, text, at: clock(), reactions: Vec::new(), reply_to: self.replying.take(), thread: Vec::new() };
        let thread = self.thread;
        let ch = self.channel_mut(at);
        match thread.and_then(|t| ch.lines.iter_mut().find(|l| l.id == t)) {
            Some(parent) => parent.thread.push(line),
            None => ch.lines.push(line),
        }
        self.my_last = Some((at.0, at.1, id));
        self.sent += 1;
        if self.channel(at).kind == Kind::Text {
            let who = ANSWERS[self.answered % ANSWERS.len()].0;
            self.typing = Some((who, now() + 1.6, at));
        }
    }

    /// Ctrl+Z: your last message taken back.
    fn unsend(&mut self) {
        let Some((s, c, id)) = self.my_last.take() else { return };
        let ch = self.channel_mut((s, c));
        ch.lines.retain(|l| l.id != id);
        for l in &mut ch.lines {
            l.thread.retain(|t| t.id != id);
        }
        self.toast = Some(("Message deleted".into(), now()));
    }

    /// The clock moves on: answers arrive, other channels light up, toasts go.
    fn tick(&mut self) -> bool {
        let t = now();
        let mut busy = false;
        if let Some((who, at, ch)) = self.typing {
            busy = true;
            if t >= at {
                self.typing = None;
                let text = ANSWERS[self.answered % ANSWERS.len()].1.to_string();
                self.answered += 1;
                let id = self.next_id;
                self.next_id += 1;
                let thread = self.thread;
                let c = self.channel_mut(ch);
                let line = Line { id, author: who, text, at: clock(), reactions: Vec::new(), reply_to: None, thread: Vec::new() };
                match thread.and_then(|tid| c.lines.iter_mut().find(|l| l.id == tid)) {
                    Some(parent) => parent.thread.push(line),
                    None => c.lines.push(line),
                }
            }
        }
        if t >= self.next_chatter {
            let (s, c, author, text) = CHATTER[self.chatter % CHATTER.len()];
            self.chatter += 1;
            self.next_chatter = t + 12.0;
            let id = self.next_id;
            self.next_id += 1;
            let shown = self.areas.iter().any(|(_, x)| *x == (s, c)) || self.active == (s, c);
            let ch = self.channel_mut((s, c));
            ch.lines.push(Line { id, author, text: text.into(), at: clock(), reactions: Vec::new(), reply_to: None, thread: Vec::new() });
            if !shown {
                ch.unread += 1;
                if text.contains("@you") {
                    ch.mentions += 1;
                }
            }
        }
        if self.toast.as_ref().is_some_and(|(_, at)| t - at > 4.0) {
            self.toast = None;
        }
        busy | self.toast.is_some() | self.in_voice.is_some()
    }

    fn key(&mut self, e: &KeyDownEvent, w: &Window) -> bool {
        let k = &e.keystroke;
        // keys typed into the composer reach the page too (as in a browser): no shortcuts then
        if w.text_field_focused() {
            return false;
        }
        match (k.key.as_str(), k.modifiers.alt, k.modifiers.control) {
            ("Up", true, _) => self.step(false),
            ("Down", true, _) => self.step(true),
            ("z", false, true) => self.unsend(),
            ("Escape", _, _) => {
                if self.replying.take().is_none() && self.thread.take().is_none() {
                    self.picker = None;
                }
            }
            _ => return false,
        }
        true
    }
}

/// Minutes after midnight now: the demo's day starts at 09:41.
fn clock() -> u32 {
    9 * 60 + 41 + (now() / 60.0) as u32
}

fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60 % 24, m % 60)
}

// ==========================================================================
// What the areas draw

struct Snap {
    servers: Vec<(String, &'static str, &'static str, usize, usize)>,
    channels: Vec<ChanRow>,
    server: usize,
    active: (usize, usize),
    /// Every chat area's channel, and each channel's lines (by `(server, channel)`).
    views: Vec<((usize, usize), ChannelView)>,
    areas: Vec<(AreaRef, (usize, usize))>,
    members: Vec<MemberRow>,
    me: (String, String, Status),
    replying: Option<(u32, String)>,
    thread: Option<u32>,
    sent: u32,
    typing: Option<(String, (usize, usize))>,
    settings: (bool, bool, bool, usize, f64),
    in_voice: Option<(usize, usize)>,
    muted: bool,
    deafened: bool,
    show_settings: bool,
    picker: Option<u32>,
    member_filter: String,
}

/// A channel in the list: its index, name, kind, category, unread, mentions and who is in it.
type ChanRow = (usize, &'static str, Kind, &'static str, usize, usize, Vec<usize>);

/// A member as drawn: name, color, status, role, voice volume.
type MemberRow = (String, String, Status, &'static str, f64);

struct ChannelView {
    name: &'static str,
    kind: Kind,
    topic: &'static str,
    lines: Vec<Line>,
    voice: Vec<usize>,
}

impl Chat {
    fn snap(&self) -> Snap {
        let (server, _) = self.active;
        let mut shown: Vec<(usize, usize)> = self.areas.iter().map(|(_, c)| *c).collect();
        shown.push(self.active);
        shown.sort_unstable();
        shown.dedup();
        let views = shown.into_iter().map(|at| {
            let ch = self.channel(at);
            (at, ChannelView { name: ch.name, kind: ch.kind, topic: ch.topic, lines: ch.lines.clone(), voice: ch.voice.clone() })
        }).collect();
        let me = &self.members[0];
        Snap {
            servers: self.servers.iter().map(|s| (s.name.to_string(), s.glyph, s.color,
                                                   s.channels.iter().map(|c| c.unread).sum(), s.channels.iter().map(|c| c.mentions).sum()))
                .collect(),
            channels: self.servers[server].channels.iter().enumerate()
                .map(|(i, c)| (i, c.name, c.kind, c.category, c.unread, c.mentions, c.voice.clone())).collect(),
            server,
            active: self.active,
            views,
            areas: self.areas.clone(),
            members: self.members.iter().map(|m| (m.name.clone(), m.color.clone(), m.status, m.role, m.volume)).collect(),
            me: (me.name.clone(), me.color.clone(), me.status),
            replying: self.replying.and_then(|id| {
                self.channel(self.active).lines.iter().find(|l| l.id == id)
                    .map(|l| (id, self.members[l.author].name.clone()))
            }),
            thread: self.thread,
            sent: self.sent,
            typing: self.typing.map(|(who, _, at)| (self.members[who].name.clone(), at)),
            settings: (self.settings.compact, self.settings.avatars, self.settings.show_ids, self.settings.accent, self.settings.scale),
            in_voice: self.in_voice,
            muted: self.muted,
            deafened: self.deafened,
            show_settings: self.show_settings,
            picker: self.picker,
            member_filter: self.member_filter.clone(),
        }
    }
}

// ==========================================================================
// Look

const CSS: &str = r"
body { color: #dcddde; -webkit-font-smoothing: antialiased; user-select: none; }
.shell { display: flex; flex-direction: column; height: 100vh; }
.screen { flex: 1; min-height: 0; }
.main { height: 100%; background: #303030; overflow-y: auto; }
.main.col { display: flex; flex-direction: column; overflow: hidden; }
.scroll { flex: 1; min-height: 0; overflow-y: auto; }
.hdr { display: flex; align-items: center; gap: 6px; height: 100%; padding: 0 8px; background: #3d3d3d; }
.hdr .hash { color: #8e9297; font-size: 1.15em; }
.hdr .name { font-weight: 700; color: #f0f0f0; }
.hdr .topic { color: #a0a0a0; font-size: 0.92em; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; flex: 1;
              padding-left: 8px; margin-left: 4px; border-left: 1px solid #555; }
.grow { flex: 1; }

/* the server rail (the channels' toolbar) */
.rail { height: 100%; background: #1f1f1f; display: flex; flex-direction: column; align-items: center; gap: 8px; padding-top: 8px; }
.srv { position: relative; width: 44px; height: 44px; border-radius: 22px; display: flex; align-items: center; justify-content: center;
       font-weight: 800; font-size: 1.15em; color: #fff; background: #36393f; cursor: pointer;
       transition-property: border-radius, background-color, transform; transition-duration: 180ms;
       transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.srv:hover { border-radius: 14px; }
.srv:active { transform: scale(0.96); }
.srv.on { border-radius: 14px; }
.pip { position: absolute; left: -9px; top: 18px; width: 4px; height: 8px; border-radius: 0 4px 4px 0; background: #fff; }
.srv.on .pip { top: 6px; height: 32px; }
.badge { position: absolute; right: -4px; bottom: -4px; min-width: 16px; height: 16px; padding: 0 4px; border-radius: 8px;
         background: #f04747; color: #fff; font-size: 11px; font-weight: 700; display: flex; align-items: center; justify-content: center;
         box-shadow: 0 0 0 3px #1f1f1f; font-variant-numeric: tabular-nums; animation: pop 260ms cubic-bezier(0.2, 0, 0, 1); }

/* channels */
.srvname { padding: 12px 12px 8px; font-weight: 700; color: #f0f0f0; border-bottom: 1px solid #262626; }
.cat summary { font-size: 0.8em; text-transform: uppercase; letter-spacing: 0.5px; color: #8e9297; }
.chan { display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 8px; margin: 1px 6px; border-radius: 4px; color: #8e9297;
        cursor: pointer; transition-property: background-color, color; transition-duration: 120ms; }
.chan:hover { background: #3a3a3a; color: #dcddde; }
.chan.on { background: #45484d; color: #fff; }
.chan.unread { color: #fff; font-weight: 600; }
.chan .ic { width: 16px; text-align: center; }
.chan .men { margin-left: auto; min-width: 16px; height: 16px; padding: 0 4px; border-radius: 8px; background: #f04747; color: #fff;
             font-size: 11px; font-weight: 700; display: flex; align-items: center; justify-content: center; }
.voiceppl { padding: 0 0 4px 34px; }
.voiceppl div { display: flex; align-items: center; gap: 6px; height: 22px; color: #a0a0a0; font-size: 0.92em; }

/* your panel (the channels' footer) */
.me { display: flex; align-items: center; gap: 8px; height: 52px; flex-shrink: 0; padding: 0 8px; background: #262626; }
.me .who { flex: 1; min-width: 0; }
.me .who b { display: block; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.me .who small { color: #a0a0a0; }

/* avatars and status dots */
.av { position: relative; width: 36px; height: 36px; border-radius: 18px; flex-shrink: 0; display: flex; align-items: center;
      justify-content: center; font-weight: 700; color: #fff; }
.av.s { width: 28px; height: 28px; border-radius: 14px; font-size: 0.85em; }
.dot { position: absolute; right: -2px; bottom: -2px; width: 10px; height: 10px; border-radius: 5px; box-shadow: 0 0 0 3px #262626; }

/* messages */
.lines { padding: 12px 0 8px; }
.day { display: flex; align-items: center; gap: 8px; margin: 6px 16px 10px; color: #8e9297; font-size: 0.8em; font-weight: 600; }
.day::before, .day::after { content: ''; flex: 1; height: 1px; background: #3f4147; }
.grp { display: flex; gap: 12px; padding: 4px 16px 2px; margin-top: 10px; }
.grp.compact { margin-top: 2px; }
.ln { position: relative; padding: 2px 16px 2px 64px; transition-property: background-color; transition-duration: 100ms; }
.compact .ln, .ln.compact { padding-left: 16px; }
.ln:hover { background: #2a2a2a; }
.ln .acts { position: absolute; right: 16px; top: -12px; display: flex; gap: 2px; padding: 2px; border-radius: 4px; background: #3a3a3a;
            box-shadow: 0 2px 8px rgba(0,0,0,0.35); opacity: 0; transition-property: opacity; transition-duration: 120ms; }
.ln:hover .acts { opacity: 1; }
.act { padding: 0 6px; border-radius: 3px; cursor: pointer; color: #b9bbbe; }
.act:hover { background: #4a4a4a; color: #fff; }
.author { font-weight: 600; margin-right: 6px; }
.ts { color: #72767d; font-size: 0.8em; font-variant-numeric: tabular-nums; }
.txt { color: #dcddde; line-height: 1.45; text-wrap: pretty; }
.txt .ment { background: rgba(250,166,26,0.18); color: #faa61a; padding: 0 2px; border-radius: 3px; }
.mine .txt { }
.quote { display: flex; gap: 6px; align-items: center; color: #b9bbbe; font-size: 0.85em; margin-bottom: 2px; }
.quote::before { content: ''; width: 20px; height: 8px; border-left: 2px solid #4f545c; border-top: 2px solid #4f545c; border-radius: 6px 0 0 0; margin-top: 6px; }
.reacts { display: flex; gap: 4px; margin: 4px 0 2px; flex-wrap: wrap; }
.re { display: flex; align-items: center; gap: 4px; padding: 0 6px; height: 22px; border-radius: 6px; background: #2f3136; cursor: pointer;
      box-shadow: inset 0 0 0 1px #3f4147; font-size: 0.85em; font-variant-numeric: tabular-nums;
      transition-property: background-color, box-shadow, transform; transition-duration: 120ms; }
.re:active { transform: scale(0.96); }
.re.mine { background: rgba(var(--acc-rgb), 0.25); box-shadow: inset 0 0 0 1px var(--acc); }
.picker { display: flex; gap: 2px; padding: 4px; border-radius: 6px; background: #3a3a3a; width: fit-content; margin: 4px 0;
          animation: pop 200ms cubic-bezier(0.2, 0, 0, 1); }
.picker .act { font-size: 1.1em; }
.threadlink { color: var(--acc); font-size: 0.85em; font-weight: 600; cursor: pointer; margin-top: 2px; }
.typing { height: 22px; padding: 0 16px; color: #b9bbbe; font-size: 0.85em; display: flex; align-items: center; gap: 6px; }
.typing i { width: 6px; height: 6px; border-radius: 3px; background: #b9bbbe; animation: bob 900ms ease-in-out infinite; }
.typing i:nth-child(2) { animation-delay: 150ms; }
.typing i:nth-child(3) { animation-delay: 300ms; }
.idtag { color: #72767d; font-size: 0.75em; margin-left: 6px; font-variant-numeric: tabular-nums; }

/* composer (the chat's footer) */
.composer { display: flex; flex-direction: column; justify-content: center; gap: 4px; min-height: 52px; flex-shrink: 0; padding: 8px 12px;
            background: #383838; }
.replybar { display: flex; align-items: center; gap: 8px; color: #b9bbbe; font-size: 0.85em; }
.replybar b { color: #fff; }

/* thread (sidebar) */
.thread { height: 100%; background: #2b2b2b; display: flex; flex-direction: column; }
.thread .th { padding: 10px 12px; font-weight: 700; border-bottom: 1px solid #222; display: flex; align-items: center; }

/* members */
.grouphd { padding: 14px 12px 4px; font-size: 0.8em; text-transform: uppercase; letter-spacing: 0.5px; color: #8e9297; }
.mem { display: flex; align-items: center; gap: 10px; padding: 4px 10px; margin: 0 6px; border-radius: 4px; }
.mem:hover { background: #3a3a3a; }
.mem.off { opacity: 0.4; }
.mem small { display: block; color: #8e9297; }

/* voice */
.voice { padding: 18px; display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 12px; }
.vcard { position: relative; height: 150px; border-radius: 10px; background: #232323; display: flex; flex-direction: column;
         align-items: center; justify-content: center; gap: 8px; box-shadow: 0 0 0 2px transparent;
         transition-property: box-shadow; transition-duration: 120ms; }
.vcard.speaking { box-shadow: 0 0 0 2px #43b581; }
.vcard .av { width: 56px; height: 56px; border-radius: 28px; font-size: 1.3em; }
.vcard .meter { width: 120px; height: 18px; }
.vctl { display: flex; align-items: center; gap: 8px; height: 44px; flex-shrink: 0; padding: 0 12px; background: #262626; }
.vctl .live { color: #43b581; font-weight: 600; }

/* settings */
.settings { padding: 18px 24px; max-width: 640px; }
.settings h2 { margin: 0 0 12px; font-size: 1.25em; text-wrap: balance; }
.srow { display: flex; align-items: center; gap: 10px; padding: 5px 0; }
.srow label { width: 150px; color: #b9bbbe; }
.toast { position: absolute; left: 50%; bottom: 74px; transform: translateX(-50%); padding: 6px 14px; border-radius: 6px;
         background: #111; color: #fff; box-shadow: 0 8px 24px rgba(0,0,0,0.5); animation: rise 220ms cubic-bezier(0.2, 0, 0, 1); }
@keyframes pop { from { opacity: 0; transform: scale(0.6); } to { opacity: 1; transform: scale(1); } }
@keyframes rise { from { opacity: 0; transform: translate(-50%, 8px); } to { opacity: 1; transform: translate(-50%, 0); } }
@keyframes bob { 0%, 100% { transform: translateY(0); opacity: 0.5; } 50% { transform: translateY(-3px); opacity: 1; } }
";

fn css() -> &'static str {
    static S: OnceLock<&'static str> = OnceLock::new();
    S.get_or_init(|| CSS)
}

type W = WeakEntity<Chat>;

fn avatar(name: &str, color: &str, status: Option<Status>, small: bool) -> Div {
    let mut d = div().class(if small { "av s" } else { "av" }).css("background", color)
        .child(name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default());
    if let Some(s) = status {
        d = d.child(div().class("dot").css("background", s.color()));
    }
    d
}

/// A message's text, an `@you` mention lit.
fn message_text(t: &str) -> Div {
    let mut d = div().class("txt");
    let mut rest = t;
    while let Some(at) = rest.find("@you") {
        d = d.child(span().child(rest[..at].to_string())).child(span().class("ment").child("@you"));
        rest = &rest[at + 4..];
    }
    d.child(span().child(rest.to_string()))
}

// ==========================================================================
// Areas

fn channels_area(s: &Snap, weak: W, k: u32) -> Editor {
    // the server rail: the editor's toolbar
    let mut rail = div().class("rail");
    for (i, (name, glyph, color, unread, mentions)) in s.servers.iter().enumerate() {
        let on = i == s.server;
        let mut srv = div().id(area_id(&format!("srv{i}"), k)).class(if on { "srv on" } else { "srv" })
            .css("background", if on { (*color).to_string() } else { "#36393f".to_string() })
            .attr("aria-label", name.clone())
            .child(*glyph)
            .on_click(weak.listener(move |this: &mut Chat, _: &ClickEvent, _, cx| {
                let first = this.servers[i].channels.iter().position(|c| c.kind == Kind::Text).unwrap_or(0);
                this.open((i, first));
                cx.notify();
            }));
        if on || *unread > 0 {
            srv = srv.child(div().class("pip"));
        }
        if *mentions > 0 {
            srv = srv.child(div().id(area_id(&format!("badge{i}-{mentions}"), k)).class("badge").child(mentions.to_string()));
        }
        rail = rail.child(srv);
    }
    let mut list = div().class("scroll").child(div().class("srvname").child(s.servers[s.server].0.clone()));
    let mut cats: Vec<&str> = Vec::new();
    for c in &s.channels {
        if !cats.contains(&c.3) {
            cats.push(c.3);
        }
    }
    for cat in cats {
        let mut panel = details().id(area_id(&format!("cat-{}-{cat}", s.server), k)).open(true).class("cat").child(summary().child(cat));
        for (i, name, kind, _, unread, mentions, voice) in s.channels.iter().filter(|c| c.3 == cat) {
            let at = (s.server, *i);
            let on = at == s.active;
            let cls = match (on, *unread > 0) {
                (true, _) => "chan on",
                (false, true) => "chan unread",
                _ => "chan",
            };
            let mut row = div().id(area_id(&format!("ch{}-{i}", s.server), k)).class(cls)
                .child(div().class("ic").child(if *kind == Kind::Voice { "🔊" } else { "#" }))
                .child(*name)
                .on_click(weak.listener(move |this: &mut Chat, _: &ClickEvent, _, cx| {
                    this.open(at);
                    cx.notify();
                }));
            if *mentions > 0 {
                row = row.child(div().class("men").child(mentions.to_string()));
            }
            panel = panel.child(row);
            if *kind == Kind::Voice && !voice.is_empty() {
                let mut ppl = div().class("voiceppl");
                for &m in voice {
                    let (n, c, ..) = &s.members[m];
                    ppl = ppl.child(div().child(avatar(n, c, None, true).css("width", "18px").css("height", "18px").css("font-size", "10px"))
                        .child(n.clone()));
                }
                if s.in_voice == Some(at) {
                    ppl = ppl.child(div().child(avatar(&s.me.0, &s.me.1, None, true).css("width", "18px").css("height", "18px")
                        .css("font-size", "10px")).child(s.me.0.clone()));
                }
                panel = panel.child(ppl);
            }
        }
        list = list.child(panel);
    }
    // you: the editor's footer
    let me = div().class("me")
        .child(avatar(&s.me.0, &s.me.1, Some(s.me.2), true))
        .child(div().class("who").child(b().css("color", s.me.1.clone()).child(s.me.0.clone())).child(small().child(s.me.2.name())))
        .child(Button().id(area_id("gear", k)).child("Settings").on_click(weak.listener(|this: &mut Chat, _: &ClickEvent, _, cx| {
            this.show_settings = !this.show_settings;
            cx.notify();
        })));
    // your panel sits under the list in the main view: a footer region is a header's height
    Editor::new(div().class("main col").child(list).child(me))
        .toolbar(Region::new(rail).size(64).overlap(false))
}

fn chat_area(s: &Snap, weak: W, k: u32, area: AreaRef) -> Editor {
    let at = s.areas.iter().find(|(a, _)| *a == area).map_or(s.active, |(_, c)| *c);
    // a click anywhere in an area makes it the one keys and the channel list act on
    let take = move |this: &mut Chat| {
        this.focus_area = Some(area);
        if !this.areas.iter().any(|(a, _)| *a == area) {
            this.areas.push((area, at));
        }
        this.active = this.of(Some(area));
    };
    let Some((_, view)) = s.views.iter().find(|(c, _)| *c == at) else { return Editor::new("") };
    if s.show_settings && k == 0 {
        return settings_area(s, weak, k);
    }
    let header = div().class("hdr")
        .child(div().class("hash").child(if view.kind == Kind::Voice { "🔊" } else { "#" }))
        .child(div().class("name").child(view.name))
        .child(div().class("topic").child(view.topic))
        .child(Button().id(area_id("split", k)).child("Split").on_click(weak.listener(move |this: &mut Chat, _: &ClickEvent, _, cx| {
            // both halves keep this channel; the channel list goes on acting on this one
            if let Some(new) = area.split(Axis::V, 0.5) {
                for a in [area, new] {
                    match this.areas.iter_mut().find(|(x, _)| *x == a) {
                        Some(e) => e.1 = at,
                        None => this.areas.push((a, at)),
                    }
                }
                this.focus_area = Some(area);
            }
            cx.notify();
        })))
        .child(Button().id(area_id("max", k)).child("Maximize").on_click(move |_, _, _| area.maximize()));
    if view.kind == Kind::Voice {
        return voice_area(s, view, at, weak, k, header);
    }
    let (compact, avatars, show_ids, ..) = s.settings;
    let mut lines = div().class("lines").child(div().class("day").child("Today"));
    let mut last_author = usize::MAX;
    let mut last_at = 0;
    for l in &view.lines {
        let (name, color, status, ..) = &s.members[l.author];
        let (name, color) = if l.author == 0 { (&s.me.0, &s.me.1) } else { (name, color) };
        let new_group = l.author != last_author || l.at > last_at + 5 || l.reply_to.is_some();
        last_author = l.author;
        last_at = l.at;
        let id = l.id;
        let mut body = div();
        if let Some(r) = l.reply_to.and_then(|r| view.lines.iter().find(|x| x.id == r)) {
            let (rn, rc, ..) = &s.members[r.author];
            body = body.child(div().class("quote").child(span().css("color", rc.clone()).child(format!("@{rn}")))
                .child(r.text.chars().take(60).collect::<String>()));
        }
        if new_group || compact {
            let mut head = div().child(span().class("author").css("color", color.clone()).child(name.clone()))
                .child(span().class("ts").child(hhmm(l.at)));
            if show_ids {
                head = head.child(span().class("idtag").child(format!("#{id}")));
            }
            body = body.child(head);
        }
        body = body.child(message_text(&l.text));
        if !l.reactions.is_empty() {
            let mut re = div().class("reacts");
            for (j, &(e, n, mine)) in l.reactions.iter().enumerate() {
                re = re.child(div().id(area_id(&format!("re{id}-{j}"), k)).class(if mine { "re mine" } else { "re" })
                    .child(e).child(n.to_string())
                    .on_click(weak.listener(move |this: &mut Chat, _: &ClickEvent, _, cx| {
                        take(this);
                        this.react(at, id, e);
                        cx.notify();
                    })));
            }
            body = body.child(re);
        }
        if s.picker == Some(id) {
            let mut p = div().class("picker");
            for (j, e) in EMOJI.iter().enumerate() {
                p = p.child(div().id(area_id(&format!("pick{id}-{j}"), k)).class("act").child(*e)
                    .on_click(weak.listener(move |this: &mut Chat, _: &ClickEvent, _, cx| {
                        take(this);
                        this.react(at, id, e);
                        cx.notify();
                    })));
            }
            body = body.child(p);
        }
        if !l.thread.is_empty() {
            let n = l.thread.len();
            body = body.child(div().id(area_id(&format!("th{id}"), k)).class("threadlink")
                .child(format!("{n} {} ›", if n == 1 { "reply" } else { "replies" }))
                .on_click(weak.listener(move |this: &mut Chat, _: &ClickEvent, _, cx| {
                    take(this);
                    this.thread = Some(id);
                    cx.notify();
                })));
        }
        let acts = div().class("acts")
            .child(div().id(area_id(&format!("react{id}"), k)).class("act").child("☺+")
                .on_click(weak.listener(move |this: &mut Chat, _: &ClickEvent, _, cx| {
                    take(this);
                    this.picker = if this.picker == Some(id) { None } else { Some(id) };
                    cx.notify();
                })))
            .child(div().id(area_id(&format!("rep{id}"), k)).class("act").child("Reply")
                .on_click(weak.listener(move |this: &mut Chat, _: &ClickEvent, _, cx| {
                    take(this);
                    this.replying = Some(id);
                    cx.notify();
                })))
            .child(div().id(area_id(&format!("thr{id}"), k)).class("act").child("Thread")
                .on_click(weak.listener(move |this: &mut Chat, _: &ClickEvent, _, cx| {
                    take(this);
                    this.thread = Some(id);
                    cx.notify();
                })));
        if new_group && !compact {
            let av = if avatars { avatar(name, color, (l.author != 0).then_some(*status), false) } else { div().css("width", "36px") };
            lines = lines.child(div().class("grp").child(av).child(div().css("flex", "1").css("min-width", "0").css("position", "relative")
                .child(div().id(area_id(&format!("l{id}"), k)).class("ln").css("padding-left", "0px").child(body).child(acts))));
        } else {
            lines = lines.child(div().id(area_id(&format!("l{id}"), k)).class(if compact { "ln compact" } else { "ln" }).child(body).child(acts));
        }
    }
    let typing = s.typing.as_ref().filter(|(_, c)| *c == at);
    let mut main = div().id(area_id("lines", k)).class("main col")
        .on_mouse_down(MouseButton::Left, weak.listener(move |this: &mut Chat, _: &MouseDownEvent, _, _| take(this)))
        .child(div().id(area_id(&format!("scroll{}-{}", at.0, at.1), k)).class("scroll").attr("data-stick", "always").child(lines));
    main = main.child(match typing {
        Some((who, _)) => div().class("typing").child(i()).child(i()).child(i()).child(b().child(who.clone())).child(" is typing…"),
        None => div().class("typing"),
    });
    // the composer: the editor's footer (a Blender text field: Enter confirms, which sends)
    let mut composer = div().class("composer");
    if let Some((_, who)) = s.replying.as_ref().filter(|_| s.active == at) {
        composer = composer.child(div().class("replybar").child("Replying to").child(b().child(who.clone()))
            .child(Button().id(area_id("cancel-reply", k)).child("✕").on_click(weak.listener(|this: &mut Chat, _: &ClickEvent, _, cx| {
                this.replying = None;
                cx.notify();
            }))));
    }
    composer = composer.child(TextField().id(area_id(&format!("compose{}", s.sent), k))
        .attr("placeholder", format!("Message #{}  (Enter sends)", view.name)).css("width", "100%")
        .on_change(weak.listener(move |this: &mut Chat, e: &ChangeEvent, _, cx| {
            take(this);
            this.post(at, &e.value);
            cx.notify();
        })));
    let mut ed = Editor::new(main.child(composer)).header(Region::new(header).size(28));
    if let Some(t) = s.thread.and_then(|t| view.lines.iter().find(|l| l.id == t)) {
        let (n, c, ..) = &s.members[t.author];
        let mut th = div().class("thread")
            .child(div().class("th").child("Thread").child(div().class("grow"))
                .child(Button().id(area_id("close-thread", k)).child("Close").on_click(weak.listener(|this: &mut Chat, _: &ClickEvent, _, cx| {
                    this.thread = None;
                    cx.notify();
                }))));
        let mut body = div().css("flex", "1").css("overflow-y", "auto").child(div().class("grp")
            .child(avatar(n, c, None, true)).child(div().child(span().class("author").css("color", c.clone()).child(n.clone()))
                .child(message_text(&t.text))));
        for r in &t.thread {
            let (rn, rc, ..) = &s.members[r.author];
            let (rn, rc) = if r.author == 0 { (&s.me.0, &s.me.1) } else { (rn, rc) };
            body = body.child(div().class("grp compact").child(avatar(rn, rc, None, true))
                .child(div().child(span().class("author").css("color", rc.clone()).child(rn.clone()))
                    .child(span().class("ts").child(hhmm(r.at))).child(message_text(&r.text))));
        }
        th = th.child(body).child(div().css("padding", "8px").child(div().class("ts").child("Replies go in the thread while it is open.")));
        ed = ed.sidebar(Region::new(th).size(300).overlap(false));
    }
    ed
}

fn voice_area(s: &Snap, view: &ChannelView, at: (usize, usize), weak: W, k: u32, header: Div) -> Editor {
    let joined = s.in_voice == Some(at);
    let t = now();
    let mut grid = div().class("voice");
    let mut people: Vec<usize> = view.voice.clone();
    if joined {
        people.insert(0, 0);
    }
    if people.is_empty() {
        grid = grid.child(div().css("color", "#8e9297").child("No one is here yet. Join to start talking."));
    }
    for (slot, &m) in people.iter().enumerate() {
        let (name, color, _, role, volume) = &s.members[m];
        let (name, color) = if m == 0 { (&s.me.0, &s.me.1) } else { (name, color) };
        // a voice level: speech-like bursts, each person their own rhythm; muted or deafened: none
        let silent = (m == 0 && s.muted) || s.deafened;
        let phase = m as f32 * 1.7;
        let level = if silent { 0.0 } else {
            ((t * 1.3 + phase).sin() * 0.5 + 0.5).powi(3) * (0.6 + 0.4 * (t * 9.0 + phase * 3.0).sin().abs())
        } * if m == 0 { 1.0 } else { *volume as f32 };
        let speaking = level > 0.25;
        let meter = canvas(move |b, p| {
            // twelve bars, the level spread across them with a little shimmer
            let n = 12;
            let w = b.size.width / n as f32;
            for i in 0..n {
                let x = b.origin.x + i as f32 * w;
                let f = 1.0 - (i as f32 - n as f32 / 2.0).abs() / (n as f32 / 2.0);
                let h = (level * f * 1.4 + 0.08).min(1.0) * b.size.height;
                let col = if i as f32 / n as f32 > 0.8 && level > 0.8 { rgb(0xf04747) } else { rgb(0x43b581) };
                p.quad(Bounds { origin: point(x + 1.0, b.origin.y + (b.size.height - h) / 2.0), size: size(w - 2.0, h) },
                       1.5, col, Rgba::TRANSPARENT, 0.0);
            }
        }).class("meter");
        let mut card = div().id(area_id(&format!("v{m}"), k)).class(if speaking { "vcard speaking" } else { "vcard" })
            .child(avatar(name, color, None, false)).child(b().child(name.clone())).child(meter);
        if m != 0 {
            card = card.child(div().css("display", "flex").css("align-items", "center").css("gap", "6px")
                .child(small().css("color", "#8e9297").child(*role))
                .child(NumberSlider().id(area_id(&format!("vol{m}"), k)).min(0.0).max(2.0).step(0.05).value(format!("{volume}")).w(px(100.0))
                    .on_change(weak.listener(move |this: &mut Chat, e: &ChangeEvent, _, cx| {
                        this.members[m].volume = e.number.clamp(0.0, 2.0);
                        cx.notify();
                    }))));
        }
        let _ = slot;
        grid = grid.child(card);
    }
    let ctl = div().class("vctl")
        .child(if joined { span().class("live").child("● Voice connected") } else { span().child("Not connected") })
        .child(div().class("grow"))
        .child(Button().id(area_id("mute", k)).child(if s.muted { "Unmute" } else { "Mute" }).on_click(weak.listener(|this: &mut Chat, _: &ClickEvent, _, cx| {
            this.muted = !this.muted;
            cx.notify();
        })))
        .child(Button().id(area_id("deafen", k)).child(if s.deafened { "Undeafen" } else { "Deafen" }).on_click(weak.listener(|this: &mut Chat, _: &ClickEvent, _, cx| {
            this.deafened = !this.deafened;
            cx.notify();
        })))
        .child(Button().id(area_id("join", k)).child(if joined { "Leave" } else { "Join" }).on_click(weak.listener(move |this: &mut Chat, _: &ClickEvent, _, cx| {
            this.in_voice = if this.in_voice == Some(at) { None } else { Some(at) };
            cx.notify();
        })));
    Editor::new(div().class("main col").child(div().class("scroll").child(grid)).child(ctl)).header(Region::new(header).size(28))
}

fn settings_area(s: &Snap, weak: W, k: u32) -> Editor {
    let (compact, avatars, show_ids, accent, scale) = s.settings;
    let check = |id: &str, label: &str, on: bool, f: fn(&mut Settings, bool)| {
        div().class("srow").child(Checkbox().id(area_id(id, k)).checked(on)
            .on_change(weak.listener(move |this: &mut Chat, e: &ChangeEvent, _, cx| {
                f(&mut this.settings, e.checked);
                cx.notify();
            }))).child(label.to_string())
    };
    let mut accents = div().class("srow").child(label().child("Accent"));
    for (i, (name, _)) in ACCENTS.iter().enumerate() {
        accents = accents.child(Radio().id(area_id(&format!("acc{i}"), k)).attr("name", area_id("accent", k)).checked(accent == i)
            .on_change(weak.listener(move |this: &mut Chat, e: &ChangeEvent, _, cx| {
                if e.checked {
                    this.settings.accent = i;
                    cx.notify();
                }
            }))).child(*name);
    }
    let body = div().class("main").child(div().class("settings")
        .child(h2().child("Settings"))
        .child(details().id(area_id("set-profile", k)).open(true).child(summary().child("Profile"))
            .child(div().class("srow").child(label().child("Display name"))
                .child(TextField().id(area_id("name", k)).value(s.me.0.clone()).w(px(220.0))
                    .on_change(weak.listener(|this: &mut Chat, e: &ChangeEvent, _, cx| {
                        let v = e.value.trim();
                        if !v.is_empty() {
                            this.members[0].name = v.to_string();
                        }
                        cx.notify();
                    }))))
            .child(div().class("srow").child(label().child("Name color"))
                .child(ColorSwatch().id(area_id("namecolor", k)).value(s.me.1.clone()).w(px(80.0))
                    .on_change(weak.listener(|this: &mut Chat, e: &ChangeEvent, _, cx| {
                        this.members[0].color.clone_from(&e.value);
                        cx.notify();
                    }))))
            .child(div().class("srow").child(label().child("Status"))
                .child(Dropdown().id(area_id("status", k)).value(s.me.2.name())
                    .children(Status::ALL.map(|st| option().child(st.name())))
                    .on_change(weak.listener(|this: &mut Chat, e: &ChangeEvent, _, cx| {
                        if let Some(&st) = Status::ALL.get(e.number as usize) {
                            this.members[0].status = st;
                        }
                        cx.notify();
                    })))))
        .child(details().id(area_id("set-look", k)).open(true).child(summary().child("Appearance"))
            .child(accents)
            .child(div().class("srow").child(label().child("UI scale"))
                .child(NumberSlider().id(area_id("scale", k)).min(0.8).max(1.4).step(0.05).value(format!("{scale}")).w(px(220.0))
                    .on_change(weak.listener(|this: &mut Chat, e: &ChangeEvent, _, cx| {
                        this.settings.scale = e.number.clamp(0.8, 1.4);
                        cx.notify();
                    }))))
            .child(check("compact", "Compact messages", compact, |st, v| st.compact = v))
            .child(check("avatars", "Show avatars", avatars, |st, v| st.avatars = v))
            .child(check("ids", "Developer mode (message ids)", show_ids, |st, v| st.show_ids = v)))
        .child(div().class("srow").child(Button().id(area_id("done", k)).child("Done").on_click(weak.listener(|this: &mut Chat, _: &ClickEvent, _, cx| {
            this.show_settings = false;
            cx.notify();
        })))));
    Editor::new(body).header(Region::new(div().class("hdr").child(div().class("name").child("User settings"))).size(28))
}

fn members_area(s: &Snap, weak: W, k: u32) -> Editor {
    let q = s.member_filter.to_lowercase();
    let mut list = div().class("main").child(div().css("padding", "8px 8px 0")
        .child(TextField().id(area_id("member-filter", k)).attr("live", "").attr("placeholder", "Find a member")
            .value(s.member_filter.clone()).css("width", "100%")
            .on_input(weak.listener(|this: &mut Chat, e: &ChangeEvent, _, cx| {
                this.member_filter.clone_from(&e.value);
                cx.notify();
            }))));
    let groups: [(&str, &[Status]); 2] = [("Online", &[Status::Online, Status::Idle, Status::Busy]), ("Offline", &[Status::Offline])];
    for (title, which) in groups {
        let ms: Vec<(usize, &MemberRow)> = s.members.iter().enumerate()
            .filter(|(i, m)| which.contains(&m.2)
                && (q.is_empty() || (if *i == 0 { &s.me.0 } else { &m.0 }).to_lowercase().contains(&q)
                    || m.3.to_lowercase().contains(&q)))
            .collect();
        list = list.child(div().class("grouphd").child(format!("{title} — {}", ms.len())));
        for (i, (name, color, status, role, _)) in ms {
            let (name, color, status) = if i == 0 { (&s.me.0, &s.me.1, s.me.2) } else { (name, color, *status) };
            list = list.child(div().class(if status == Status::Offline { "mem off" } else { "mem" })
                .child(avatar(name, color, Some(status), true))
                .child(div().child(b().css("color", color.clone()).child(name.clone())).child(small().child(*role))));
        }
    }
    Editor::new(list).header(Region::new(div().class("hdr").child(div().class("name").child("Members"))).size(28))
}

// ==========================================================================
// The window

impl Render for Chat {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        focus_root(w, &mut self.frames);
        if self.tick() {
            w.request_animation_frame();
        }
        let weak = cx.weak_entity();
        let snap = Rc::new(self.snap());
        let editor = |make: fn(&Snap, W, u32, AreaRef) -> Editor| {
            let snap = snap.clone();
            let n = std::cell::Cell::new(0u32);
            move |area: &AreaRef| {
                let k = n.get();
                n.set(k + 1);
                make(&snap, weak, k, *area)
            }
        };
        let editors = Editors::new()
            .editor("Channels", "OUTLINER", editor(|s, w, k, _| channels_area(s, w, k)))
            .editor("Chat", "TEXT", editor(chat_area))
            .editor("Members", "USER", editor(|s, w, k, _| members_area(s, w, k)))
            .editor("Settings", "PREFERENCES", editor(|s, w, k, _| settings_area(s, w, k)));
        let screen = Screen().id("chat-screen").editors(editors).class("screen")
            .layout(AreaSplit::v(0.22, "Channels", AreaSplit::v(0.78, "Chat", "Members")));
        let (accent_name, accent) = ACCENTS[self.settings.accent];
        let _ = accent_name;
        let rgb_of = |h: &str| {
            let v = u32::from_str_radix(h.trim_start_matches('#'), 16).unwrap_or(0x4772b3);
            format!("{}, {}, {}", v >> 16 & 255, v >> 8 & 255, v & 255)
        };
        let mut root = app().id("root").attr("tabindex", "0")
            .css("font-size", format!("{}px", 14.0 * self.settings.scale))
            .css("--acc", accent).css("--acc-rgb", rgb_of(accent))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                if this.key(e, w) {
                    // taken: Blender's own keys (a widget's undo, a hovered widget's shortcut) don't run too
                    cx.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(head().child(style().child(css())))
            .child(div().class("shell").child(screen));
        if let Some((t, _)) = &self.toast {
            root = root.child(div().class("toast").child(t.clone()));
        }
        root
    }
}

/// Every feature in turn: channels, a reaction, the picker, a reply that is answered, a thread, a
/// second channel beside the first, servers, voice with a volume, settings that change the app.
fn storyboard(dir: String) -> Script {
    story(dir, |s| {
        s.play("00-open", &[0, 300]);
        s.hover("lines");
        s.click("re1-0");
        s.play("01-react", &[0, 200]);
        s.hover("l9");
        s.click("react9");
        s.play("02-picker", &[0, 200]);
        s.click("pick9-3");
        s.play("03-picked", &[0, 200]);
        s.hover("l9");
        s.click("rep9");
        s.play("04-replying", &[0, 200]);
        s.click("compose0");
        s.type_text("on it, the rim is too hot on 052");
        s.key("Return");
        s.play("05-sent", &[0, 200, 1000, 2000]);
        s.click("th6");
        s.play("06-thread", &[0, 300]);
        s.click("compose1");
        s.type_text("rolled back here too");
        s.key("Return");
        s.play("07-thread-reply", &[0, 2000]);
        s.click("close-thread");
        s.click("split");
        s.play("08-split", &[0, 300]);
        s.click("ch0-1");
        s.play("09-second-channel", &[0, 300]);
        s.click("srv2");
        s.play("10-server", &[0, 300]);
        s.click("srv0");
        s.click("ch0-5");
        s.play("11-voice", &[0, 400, 900]);
        let b = s.bounds("vol1");
        s.cx.simulate_drag(s.win, (b.origin.x + 40.0, b.origin.y + 8.0), (b.origin.x + 90.0, b.origin.y + 8.0));
        s.click("mute");
        s.play("12-voice-volume-mute", &[0, 400]);
        s.click("gear");
        s.play("13-settings", &[0, 300]);
        s.click("acc1");
        s.click("compact");
        let b = s.bounds("scale");
        s.cx.simulate_drag(s.win, (b.origin.x + 60.0, b.origin.y + 8.0), (b.origin.x + 150.0, b.origin.y + 8.0));
        s.play("14-settings-changed", &[0, 300]);
        s.click("done");
        s.click("ch0-0");
        s.play("15-after-settings", &[0, 300]);
        s.tick(12_000);
        s.play("16-chatter", &[0, 300]);
        s.click("member-filter");
        s.type_text("li");
        s.play("17-member-filter", &[0, 200]);
        s.key("Return");
        s.cx.simulate_key_with(s.win, "z", "z", true, false);
        s.play("18-unsend", &[0, 200]);
    })
}

fn main() {
    let (w, h) = size_arg((1440.0, 860.0));
    run_with(w, h, "BUI Chat", Chat::new, arg("--storyboard").map(storyboard));
}

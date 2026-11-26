//! BUI Mail: an email client built from Blender's own interface. The window is a Blender screen:
//! folders, the message list and the reader are areas you can resize, split, join and maximize
//! (drag an area's corner, or the buttons in each header), each with Blender's header, toolbar and
//! sidebar regions. Every control is a real Blender widget: the account and sort menus are
//! Blender dropdowns, the checkboxes, number fields (drag to scrub, double-click to type `7*2`),
//! sliders, color swatches (Blender's color picker), text fields and the text editor.
//!
//! What it does: two accounts with their own mailboxes; folders with unread counts, and labels
//! whose colors you pick; live search; sort; Shift / Ctrl click multi-select (with a checkbox per
//! row); star and flag; conversations as Blender panels in the reader; reply, reply-all and forward
//! into a compose view (To / Cc / Subject fields, a body editor, a signature menu, attachments
//! that upload with a progress bar); a send that waits five seconds so you can undo it; drag a
//! message onto a folder or a label; rules (auto-archive older than N days, mark read on open,
//! snippets, density); every action undoable; and mail that keeps arriving.
//!
//! Keys (over the list or reader): J / K next / previous, Enter or O open, E archive, Del / # delete,
//! S star, U toggle read, R reply, A reply all, F forward, C compose, / search, Ctrl+A select all,
//! Esc clear, Ctrl+Z undo.
//!
//! `cargo run --release --bin bui_mail`, `-- --storyboard DIR` drives it headlessly.

use std::rc::Rc;
use std::sync::OnceLock;

use blenderui::prelude::*;
use blenderui::screen::{AreaRef, AreaSplit, Axis, Editor, Editors, Region, Screen};
use bui_demos::story::story;
use bui_demos::{arg, focus_root, now, run_with, size_arg, Script};
use bui_demos::area_id;

// ==========================================================================
// The mailbox

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Folder {
    Inbox,
    Sent,
    Drafts,
    Archive,
    Spam,
    Trash,
}

impl Folder {
    const ALL: [Self; 6] = [Self::Inbox, Self::Sent, Self::Drafts, Self::Archive, Self::Spam, Self::Trash];

    const fn name(self) -> &'static str {
        match self {
            Self::Inbox => "Inbox",
            Self::Sent => "Sent",
            Self::Drafts => "Drafts",
            Self::Archive => "Archive",
            Self::Spam => "Spam",
            Self::Trash => "Trash",
        }
    }

    const fn glyph(self) -> &'static str {
        match self {
            Self::Inbox => "▤",
            Self::Sent => "➤",
            Self::Drafts => "✎",
            Self::Archive => "▣",
            Self::Spam => "⚠",
            Self::Trash => "✕",
        }
    }
}

/// What the list shows: a folder, a smart view, or a label.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Folder(Folder),
    Starred,
    Unread,
    Label(usize),
}

#[derive(Clone)]
struct Msg {
    id: u32,
    account: usize,
    folder: Folder,
    thread: u32,
    from: &'static str,
    addr: &'static str,
    to: String,
    subject: String,
    body: String,
    /// Minutes before "now" (the demo's clock starts at Tuesday 09:41).
    age: u32,
    unread: bool,
    starred: bool,
    flagged: bool,
    /// Bit `k`: label `k`.
    labels: u8,
    /// 0 low, 1 normal, 2 high.
    importance: u8,
    attachments: Vec<(String, u32)>,
}

struct Label {
    name: &'static str,
    color: String,
}

const ACCOUNTS: [(&str, &str); 2] = [("Mira Okafor", "mira@lumen.studio"), ("Mira (personal)", "mira.ok@postbox.net")];

/// (account, folder, thread, from, address, subject, body, minutes ago, unread, starred, labels, attachment)
type Seed = (usize, Folder, u32, &'static str, &'static str, &'static str, &'static str, u32, bool, bool, u8, Option<(&'static str, u32)>);

const SEEDS: &[Seed] = &[
    (0, Folder::Inbox, 1, "Jonas Weber", "jonas@lumen.studio", "Render farm is back online", "All 24 nodes report in again. The queue drained overnight; the shot 040 renders finished at 06:12. If you see a frame with fireflies, flag it and I'll re-queue it with more samples.", 14, true, false, 0b0001, None),
    (0, Folder::Inbox, 2, "Ana Lima", "ana@lumen.studio", "Lighting notes for shot 052", "Rim light is too hot on the left shoulder. Try 0.6 of the current strength and warm it a little. Otherwise the mood is right. Attached the paintover.", 38, true, true, 0b0001, Some(("paintover_052.png", 2480))),
    (0, Folder::Inbox, 3, "Build Bot", "bot@ci.lumen.studio", "✔ nightly #1842 passed", "All 312 tests passed in 14m 08s. Artifacts: addon-1.9.4.zip, docs.tar.gz.", 55, false, false, 0b0100, None),
    (0, Folder::Inbox, 4, "Priya Shah", "priya@orbitfilms.co", "Contract draft v3", "Hi Mira, here is v3 with the changes from Friday. Section 4 now caps revisions at three rounds. Let me know if legal has comments by Thursday.", 92, true, false, 0b0001, Some(("contract_v3.pdf", 412))),
    (0, Folder::Inbox, 4, "Mira Okafor", "mira@lumen.studio", "Re: Contract draft v3", "Thanks Priya. Forwarding to legal now; one question on the payment schedule in 6.2 — is net 30 from delivery or from approval?", 80, false, false, 0b0001, None),
    (0, Folder::Inbox, 4, "Priya Shah", "priya@orbitfilms.co", "Re: Contract draft v3", "From approval. I'll make that explicit in v4.", 61, true, false, 0b0001, None),
    (0, Folder::Inbox, 5, "Tomas Reyes", "tomas@lumen.studio", "Geometry nodes setup for the forest", "I rebuilt the scatter with a density mask from the path. Viewport is at 40 fps now with instances. Can you check it on your machine?", 130, false, false, 0b0001, None),
    (0, Folder::Inbox, 6, "Asset Shelf", "noreply@assetshelf.io", "Your weekly asset digest", "12 new materials, 4 HDRIs and a pack of foliage were added to libraries you follow.", 240, false, false, 0, None),
    (0, Folder::Inbox, 7, "Ana Lima", "ana@lumen.studio", "Lunch?", "Thai place downstairs at 12:30?", 300, false, false, 0, None),
    (0, Folder::Inbox, 8, "Kenji Mori", "kenji@hollowpine.jp", "Rigging review tomorrow", "Agenda: the face rig controls, the shoulder twist and the IK/FK switch. 30 minutes should do.", 420, true, false, 0b0001, None),
    (0, Folder::Inbox, 9, "Finance", "finance@lumen.studio", "Expense report approved", "Your October expenses (4 items, 1,284.00) were approved and will be paid on Friday.", 600, false, false, 0b0010, None),
    (0, Folder::Inbox, 10, "Build Bot", "bot@ci.lumen.studio", "✘ nightly #1841 failed", "2 tests failed: test_uv_unwrap_islands, test_bake_normals. See the log for details.", 1500, false, true, 0b0100, Some(("log_1841.txt", 88))),
    (0, Folder::Inbox, 10, "Jonas Weber", "jonas@lumen.studio", "Re: ✘ nightly #1841 failed", "The bake test is flaky on the old GPU node. I pinned it to the new pool.", 1440, false, false, 0b0100, None),
    (0, Folder::Inbox, 11, "Lena Fischer", "lena@pixelpost.de", "Interview for the studio feature", "We'd love to feature Lumen in our December issue. Do you have 45 minutes next week for a call?", 1700, true, false, 0, None),
    (0, Folder::Inbox, 12, "Travel Desk", "trips@lumen.studio", "Your trip to the festival is booked", "Flight LS 214 on the 14th, 08:20. Hotel near the venue for three nights. Your badge is waiting at registration.", 2900, false, true, 0b1000, Some(("itinerary.pdf", 156))),
    (0, Folder::Inbox, 13, "Omar Haddad", "omar@lumen.studio", "Color pipeline: ACES or AgX?", "For the new show I'd move us to AgX for viewing. It handles the saturated neons far better. Thoughts before Monday?", 3100, false, false, 0b0001, None),
    (0, Folder::Inbox, 13, "Ana Lima", "ana@lumen.studio", "Re: Color pipeline: ACES or AgX?", "+1 for AgX. Let's test it on shot 052 first.", 3000, false, false, 0b0001, None),
    (0, Folder::Inbox, 14, "Calendar", "calendar@lumen.studio", "Reminder: dailies at 16:00", "Screening room B. Bring the latest renders of sequence 4.", 3500, false, false, 0, None),
    (0, Folder::Inbox, 15, "Yuki Tanaka", "yuki@lumen.studio", "Texture bake times", "Baking 8K textures takes 6 min per asset on my machine. Is there a farm profile for bakes?", 4400, false, false, 0b0001, None),
    (0, Folder::Inbox, 16, "Store", "receipts@gridmarket.com", "Your receipt for Studio Lights Pack", "Thanks for your purchase. Total 39.00. Download your files from your library.", 5800, false, false, 0b0010, None),
    (0, Folder::Inbox, 17, "Dev Weekly", "news@devweekly.io", "Issue 211: Shaders without tears", "This week: writing a toon shader, a look at node-based compositing, and a dozen add-ons worth trying.", 7200, false, false, 0, None),
    (0, Folder::Inbox, 18, "Sam Carter", "sam@lumen.studio", "Hand-off for shot 061", "Animation is final on 061. Cache is on the share under /shots/061/cache. Ping me if the cloth sim needs another pass.", 8600, false, false, 0b0001, None),
    (0, Folder::Archive, 19, "Priya Shah", "priya@orbitfilms.co", "Kick-off notes", "Thanks for a great kick-off. Notes attached: milestones, contacts, delivery specs.", 21000, false, false, 0b0001, Some(("kickoff_notes.pdf", 220))),
    (0, Folder::Archive, 20, "Jonas Weber", "jonas@lumen.studio", "New workstation specs", "Ordered: 2 x 24 GB GPUs, 128 GB RAM, the quiet case.", 26000, false, false, 0, None),
    (0, Folder::Archive, 21, "Finance", "finance@lumen.studio", "Q3 budget closed", "Q3 is closed. Thanks everyone for getting receipts in on time.", 40000, false, false, 0b0010, None),
    (0, Folder::Sent, 22, "Mira Okafor", "mira@lumen.studio", "Shot list for sequence 4", "Hi all, attached is the shot list for sequence 4 with owners and due dates.", 4000, false, false, 0b0001, Some(("seq4_shots.csv", 12))),
    (0, Folder::Sent, 23, "Mira Okafor", "mira@lumen.studio", "Re: Lunch?", "Yes! See you there.", 290, false, false, 0, None),
    (0, Folder::Drafts, 24, "Mira Okafor", "mira@lumen.studio", "Studio feature answers", "1. How did Lumen start? We were three friends with one render node and a lot of coffee…", 900, false, false, 0, None),
    (0, Folder::Spam, 25, "Prize Center", "win@lucky-prizes.biz", "You have WON a free GPU!!!", "Click here to claim your prize before midnight.", 700, true, false, 0, None),
    (0, Folder::Spam, 26, "Crypto Daily", "alpha@coin-moon.xyz", "This one coin will 1000x", "Don't miss out.", 2000, true, false, 0, None),
    (0, Folder::Trash, 27, "Asset Shelf", "noreply@assetshelf.io", "Last week's asset digest", "8 new materials were added.", 10500, false, false, 0, None),
    (1, Folder::Inbox, 30, "Dad", "papa.okafor@postbox.net", "Sunday dinner", "Are you coming Sunday? Your aunt is making jollof. Bring the camera, the garden looks great.", 45, true, false, 0b0000, Some(("garden.jpg", 3100))),
    (1, Folder::Inbox, 31, "Bike Club", "rides@spokesclub.org", "Saturday ride: 62 km loop", "Meet at the bakery at 8:00. Route and elevation attached. Coffee stop at km 34.", 210, false, false, 0, Some(("route.gpx", 44))),
    (1, Folder::Inbox, 32, "Lena", "lena.k@postbox.net", "Photos from the hike", "Uploaded the photos from the ridge. The one with the fog is my favourite.", 620, true, true, 0, None),
    (1, Folder::Inbox, 32, "Mira (personal)", "mira.ok@postbox.net", "Re: Photos from the hike", "They're gorgeous. Can I print the fog one?", 600, false, false, 0, None),
    (1, Folder::Inbox, 33, "Library", "notices@citylibrary.org", "Your books are due Friday", "3 items are due on Friday. Renew online or at the desk.", 1300, false, false, 0, None),
    (1, Folder::Inbox, 34, "Airline", "booking@skyhopper.travel", "Check-in is open", "Check in now for your flight on the 14th. Seat 14A is reserved.", 2600, false, false, 0b1000, None),
    (1, Folder::Inbox, 35, "Bank", "alerts@harborbank.com", "Your statement is ready", "Your November statement is available in online banking.", 5100, false, false, 0b0010, None),
    (1, Folder::Inbox, 36, "Plant Shop", "hello@leafandpot.com", "Your monstera has shipped", "Tracking: LP-42-7781. Expected Thursday.", 6300, false, false, 0b0010, None),
    (1, Folder::Inbox, 37, "Mum", "mama.okafor@postbox.net", "Recipe you asked for", "Here it is, with my notes in the margin. Don't skip the smoked paprika.", 9000, false, true, 0, Some(("recipe.jpg", 1900))),
    (1, Folder::Archive, 38, "Bike Club", "rides@spokesclub.org", "Membership renewed", "Thanks for renewing! Your new jersey is on its way.", 30000, false, false, 0, None),
    (1, Folder::Sent, 39, "Mira (personal)", "mira.ok@postbox.net", "Re: Sunday dinner", "I'll be there by 6, with the camera!", 30, false, false, 0, None),
    (1, Folder::Spam, 40, "Pharmacy", "deals@cheap-meds.biz", "Limited offer", "Act now.", 3300, true, false, 0, None),
];

/// What arrives later, one every half minute (account 0).
const ARRIVALS: &[(&str, &str, &str, &str)] = &[
    ("Ana Lima", "ana@lumen.studio", "Paintover v2", "Updated the paintover with the warmer rim. Much better, I think."),
    ("Build Bot", "bot@ci.lumen.studio", "✔ nightly #1843 passed", "All 312 tests passed in 13m 51s."),
    ("Kenji Mori", "kenji@hollowpine.jp", "Moved the review to 11:00", "Sorry for the shuffle, the client call moved."),
    ("Jonas Weber", "jonas@lumen.studio", "Disk space on the share", "We're at 91%. I'll archive season 1 caches to cold storage tonight."),
];

const SIGNATURES: [(&str, &str); 3] = [
    ("None", ""),
    ("Studio", "\n\n— Mira Okafor\nLighting Lead, Lumen Studio"),
    ("Short", "\n\nMira"),
];

const SORTS: [&str; 5] = ["Newest first", "Oldest first", "Sender", "Subject", "Unread first"];

// ==========================================================================
// State

/// A reply, forward or new message being written.
#[derive(Clone)]
struct Draft {
    kind: &'static str,
    to: String,
    cc: String,
    subject: String,
    body: String,
    signature: usize,
    /// (file name, size KB, upload progress 0..1, when it started)
    files: Vec<(String, u32, f32)>,
    /// Bumped to push a new body into the editor (values go to a widget only when they change).
    rev: u32,
}

/// An action that can be undone: each message it touched and how that message was.
struct Undo {
    what: String,
    before: Vec<Msg>,
}

/// A message on its way: sent at `at` unless undone.
struct Outgoing {
    msg: Msg,
    at: f32,
}

#[allow(clippy::struct_excessive_bools)]
struct Rules {
    archive_days: f64,
    auto_archive: bool,
    read_on_open: bool,
    snippets: bool,
    threads: bool,
    density: f64,
}

struct Mail {
    msgs: Vec<Msg>,
    next_id: u32,
    labels: Vec<Label>,
    account: usize,
    view: View,
    query: String,
    sort: usize,
    /// Selected message ids, and the one the reader shows (the cursor).
    sel: Vec<u32>,
    cursor: Option<u32>,
    /// The last row clicked without Shift (a range starts there).
    anchor: Option<u32>,
    draft: Option<Draft>,
    undo: Vec<Undo>,
    toast: Option<(String, f32, bool)>,
    outgoing: Option<Outgoing>,
    /// The draft a waiting send came from (Undo brings it back).
    undo_draft: Option<Draft>,
    rules: Rules,
    /// A message being dragged (its id), and where the pointer is.
    drag: Option<(u32, f32, f32, bool)>,
    hover_drop: Option<View>,
    /// The folders' sidebar (label colors, rules) and the reader's (message properties) are shown.
    sides: (bool, bool),
    arrived: usize,
    next_arrival: f32,
    headless: bool,
    frames: u32,
}

/// What the areas draw this render (their factories get a copy of this).
struct Snap {
    rows: Vec<Row>,
    counts: Vec<(View, usize, usize)>,
    labels: Vec<(String, String)>,
    reader: Vec<Msg>,
    draft: Option<Draft>,
    view: View,
    sel: Vec<u32>,
    cursor: Option<u32>,
    rules_snippets: bool,
    rules: (f64, bool, bool, bool, bool, f64),
    hover_drop: Option<View>,
    dragging: bool,
    sides: (bool, bool),
}

#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
struct Row {
    id: u32,
    from: &'static str,
    subject: String,
    snippet: String,
    when: String,
    unread: bool,
    starred: bool,
    flagged: bool,
    labels: u8,
    count: usize,
    attach: bool,
}

fn seed() -> Vec<Msg> {
    SEEDS.iter().enumerate().map(|(i, s)| {
        let (account, folder, thread, from, addr, subject, body, age, unread, starred, labels, attach) = *s;
        Msg {
            id: i as u32 + 1,
            account,
            folder,
            thread,
            from,
            addr,
            to: ACCOUNTS[account].1.to_string(),
            subject: subject.to_string(),
            body: body.to_string(),
            age,
            unread,
            starred,
            flagged: false,
            labels,
            importance: 1,
            attachments: attach.map(|(n, k)| vec![(n.to_string(), k)]).unwrap_or_default(),
        }
    }).collect()
}

/// "09:41", "Yesterday", "Mon", "Nov 12" for a message `age` minutes old (now: Tue Nov 26, 09:41).
fn when(age: u32) -> String {
    const DAYS: [&str; 7] = ["Mon", "Sun", "Sat", "Fri", "Thu", "Wed", "Tue"];
    let now = 9 * 60 + 41;
    if age <= now {
        let m = now - age;
        return format!("{:02}:{:02}", m / 60, m % 60);
    }
    let days = (age - now) / 1440 + 1;
    match days {
        1 => "Yesterday".to_string(),
        2..=6 => DAYS[(days as usize - 1) % 7].to_string(),
        d => format!("Nov {}", 26u32.saturating_sub(d).max(1)),
    }
}

impl Mail {
    fn new() -> Self {
        Self {
            next_id: SEEDS.len() as u32 + 1,
            msgs: seed(),
            labels: vec![
                Label { name: "Work", color: "#4772b3".into() },
                Label { name: "Receipts", color: "#e0a030".into() },
                Label { name: "Builds", color: "#5aa05a".into() },
                Label { name: "Travel", color: "#b35a9e".into() },
            ],
            account: 0,
            view: View::Folder(Folder::Inbox),
            query: String::new(),
            sort: 0,
            sel: vec![1],
            cursor: Some(1),
            anchor: Some(1),
            draft: None,
            undo: Vec::new(),
            toast: None,
            outgoing: None,
            undo_draft: None,
            rules: Rules { archive_days: 30.0, auto_archive: false, read_on_open: true, snippets: true, threads: true, density: 1.0 },
            drag: None,
            hover_drop: None,
            sides: (false, true),
            arrived: 0,
            next_arrival: 30.0,
            headless: arg("--storyboard").is_some(),
            frames: 0,
        }
    }

    fn in_view(&self, m: &Msg, view: View) -> bool {
        if m.account != self.account {
            return false;
        }
        let live = !matches!(m.folder, Folder::Trash | Folder::Spam);
        match view {
            View::Folder(f) => m.folder == f,
            View::Starred => m.starred && live,
            View::Unread => m.unread && live,
            View::Label(k) => m.labels & (1 << k) != 0 && live,
        }
    }

    /// The list: messages in the view that match the search, one row a conversation (or a message
    /// when conversations are off), sorted.
    fn rows(&self) -> Vec<Row> {
        let q = self.query.to_lowercase();
        let hit = |m: &Msg| {
            q.is_empty() || m.subject.to_lowercase().contains(&q) || m.from.to_lowercase().contains(&q)
                || m.body.to_lowercase().contains(&q)
        };
        let mut picked: Vec<&Msg> = self.msgs.iter().filter(|m| self.in_view(m, self.view) && hit(m)).collect();
        // newest first inside a conversation: the row is its newest message
        picked.sort_by_key(|m| m.age);
        let mut rows: Vec<Row> = Vec::new();
        let mut seen: Vec<u32> = Vec::new();
        for m in picked {
            if self.rules.threads && seen.contains(&m.thread) {
                if let Some(r) = rows.iter_mut().find(|r| self.msg(r.id).is_some_and(|x| x.thread == m.thread)) {
                    r.count += 1;
                    r.unread |= m.unread;
                    r.attach |= !m.attachments.is_empty();
                }
                continue;
            }
            seen.push(m.thread);
            rows.push(Row {
                id: m.id,
                from: m.from,
                subject: m.subject.clone(),
                snippet: m.body.chars().take(90).collect(),
                when: when(m.age),
                unread: m.unread,
                starred: m.starred,
                flagged: m.flagged,
                labels: m.labels,
                count: 1,
                attach: !m.attachments.is_empty(),
            });
        }
        let age = |r: &Row| self.msg(r.id).map_or(0, |m| m.age);
        match self.sort {
            1 => rows.sort_by_key(|r| std::cmp::Reverse(age(r))),
            2 => rows.sort_by(|a, b| a.from.cmp(b.from)),
            3 => rows.sort_by_cached_key(|r| r.subject.to_lowercase()),
            4 => rows.sort_by_key(|r| (!r.unread, age(r))),
            _ => {}
        }
        rows
    }

    fn msg(&self, id: u32) -> Option<&Msg> {
        self.msgs.iter().find(|m| m.id == id)
    }

    fn msg_mut(&mut self, id: u32) -> Option<&mut Msg> {
        self.msgs.iter_mut().find(|m| m.id == id)
    }

    /// The ids an action on the selection touches: every message of each selected conversation.
    fn targets(&self) -> Vec<u32> {
        let ids: Vec<u32> = if self.sel.is_empty() { self.cursor.into_iter().collect() } else { self.sel.clone() };
        let threads: Vec<u32> = ids.iter().filter_map(|&i| self.msg(i)).map(|m| m.thread).collect();
        self.msgs.iter()
            .filter(|m| m.account == self.account && (ids.contains(&m.id) || (self.rules.threads && threads.contains(&m.thread))))
            .filter(|m| self.in_view(m, self.view) || ids.contains(&m.id))
            .map(|m| m.id)
            .collect()
    }

    /// Changes the messages `ids` with `f`, keeping how they were for Undo.
    fn act(&mut self, what: impl Into<String>, ids: &[u32], f: impl Fn(&mut Msg)) {
        if ids.is_empty() {
            return;
        }
        let what = what.into();
        let before: Vec<Msg> = self.msgs.iter().filter(|m| ids.contains(&m.id)).cloned().collect();
        for m in self.msgs.iter_mut().filter(|m| ids.contains(&m.id)) {
            f(m);
        }
        self.toast = Some((what.clone(), now(), true));
        self.undo.push(Undo { what, before });
        // what left the view leaves the selection; the cursor moves to the next row
        let rows = self.rows();
        self.sel.retain(|id| rows.iter().any(|r| r.id == *id));
        if self.cursor.is_some_and(|c| !rows.iter().any(|r| r.id == c)) {
            self.cursor = rows.first().map(|r| r.id);
            self.sel = self.cursor.into_iter().collect();
        }
    }

    fn undo(&mut self) {
        let Some(u) = self.undo.pop() else {
            self.toast = Some(("Nothing to undo".into(), now(), false));
            return;
        };
        for b in u.before {
            if let Some(m) = self.msg_mut(b.id) {
                *m = b;
            }
        }
        self.toast = Some((format!("Undone: {}", u.what), now(), false));
    }

    fn move_to(&mut self, ids: &[u32], to: Folder) {
        let n = ids.len();
        self.act(format!("{n} moved to {}", to.name()), ids, |m| m.folder = to);
    }

    fn open(&mut self, id: u32) {
        self.cursor = Some(id);
        if self.rules.read_on_open {
            let thread = self.msg(id).map(|m| m.thread);
            for m in self.msgs.iter_mut().filter(|m| Some(m.thread) == thread) {
                m.unread = false;
            }
        }
    }

    /// J / K: the next or previous row, opened and selected alone.
    fn step(&mut self, down: bool) {
        let rows = self.rows();
        let last = rows.len().saturating_sub(1);
        let at = self.cursor.and_then(|c| rows.iter().position(|r| r.id == c))
            .map_or(0, |i| if down { (i + 1).min(last) } else { i.saturating_sub(1) });
        if let Some(r) = rows.get(at) {
            let id = r.id;
            self.sel = vec![id];
            self.anchor = Some(id);
            self.open(id);
        }
    }

    /// A click on a row: alone, Ctrl adds or removes, Shift selects the range from the anchor.
    fn click_row(&mut self, id: u32, shift: bool, ctrl: bool) {
        if shift && let Some(a) = self.anchor {
            let rows = self.rows();
            let (i, j) = (rows.iter().position(|r| r.id == a), rows.iter().position(|r| r.id == id));
            if let (Some(i), Some(j)) = (i, j) {
                self.sel = rows[i.min(j)..=i.max(j)].iter().map(|r| r.id).collect();
            }
        } else if ctrl {
            if let Some(k) = self.sel.iter().position(|&s| s == id) {
                self.sel.remove(k);
            } else {
                self.sel.push(id);
            }
            self.anchor = Some(id);
        } else {
            self.sel = vec![id];
            self.anchor = Some(id);
        }
        self.open(id);
    }

    fn compose(&mut self, kind: &'static str) {
        let base = self.cursor.and_then(|c| self.msg(c)).cloned();
        let me = ACCOUNTS[self.account].1;
        let quote = |m: &Msg| format!("\n\nOn {}, {} wrote:\n> {}", when(m.age), m.from, m.body);
        let draft = match (kind, base) {
            ("Reply", Some(m)) => Draft { kind, to: m.addr.into(), cc: String::new(), subject: re(&m.subject, "Re: "), body: quote(&m),
                                          signature: 1, files: Vec::new(), rev: 0 },
            ("Reply all", Some(m)) => Draft { kind, to: m.addr.into(), cc: m.to.replace(me, "").trim_matches(',').to_string(),
                                              subject: re(&m.subject, "Re: "), body: quote(&m), signature: 1, files: Vec::new(), rev: 0 },
            ("Forward", Some(m)) => Draft { kind, to: String::new(), cc: String::new(), subject: re(&m.subject, "Fwd: "),
                                            body: format!("\n\n---------- Forwarded message ----------\nFrom: {} <{}>\nSubject: {}\n\n{}", m.from, m.addr, m.subject, m.body),
                                            signature: 1,
                                            files: m.attachments.iter().map(|(n, k)| (n.clone(), *k, 1.0)).collect(), rev: 0 },
            _ => Draft { kind: "New message", to: String::new(), cc: String::new(), subject: String::new(), body: String::new(),
                         signature: 1, files: Vec::new(), rev: 0 },
        };
        let mut d = draft;
        d.body.insert_str(0, SIGNATURES[d.signature].1);
        if d.kind != "New message" {
            // the signature goes above the quote, after where you write
            let sig = SIGNATURES[d.signature].1;
            d.body = format!("{sig}{}", &d.body[sig.len()..]);
        }
        self.draft = Some(d);
    }

    fn send(&mut self) {
        let Some(d) = self.draft.take() else { return };
        if d.to.trim().is_empty() {
            self.toast = Some(("Add a recipient first".into(), now(), false));
            self.draft = Some(d);
            return;
        }
        let (name, addr) = ACCOUNTS[self.account];
        let thread = self.cursor.and_then(|c| self.msg(c)).filter(|_| d.kind != "New message").map_or(1000 + self.next_id, |m| m.thread);
        let msg = Msg {
            id: self.next_id,
            account: self.account,
            folder: Folder::Sent,
            thread,
            from: name,
            addr,
            to: d.to.clone(),
            subject: if d.subject.is_empty() { "(no subject)".into() } else { d.subject.clone() },
            body: d.body.clone(),
            age: 0,
            unread: false,
            starred: false,
            flagged: false,
            labels: 0,
            importance: 1,
            attachments: d.files.iter().map(|(n, k, _)| (n.clone(), *k)).collect(),
        };
        self.next_id += 1;
        self.outgoing = Some(Outgoing { msg, at: now() + 5.0 });
        // kept to bring back on Undo
        self.draft = None;
        self.undo_draft = Some(d);
        self.toast = None;
    }

    /// The messages around: the clock moves on (sending, uploads, toasts, arrivals).
    fn tick(&mut self) -> bool {
        let t = now();
        let mut busy = false;
        if let Some(o) = &self.outgoing {
            busy = true;
            if t >= o.at {
                let msg = self.outgoing.take().map(|o| o.msg);
                if let Some(m) = msg {
                    self.toast = Some((format!("Sent to {}", m.to), t, false));
                    self.msgs.push(m);
                }
                self.undo_draft = None;
            }
        }
        if let Some(d) = &mut self.draft {
            for f in &mut d.files {
                if f.2 < 1.0 {
                    // 600 KB a second
                    f.2 = (f.2 + 600.0 / (f.1.max(1) as f32) / 60.0).min(1.0);
                    busy = true;
                }
            }
        }
        if self.toast.as_ref().is_some_and(|(_, at, _)| t - at > 6.0) {
            self.toast = None;
        }
        busy |= self.toast.is_some();
        if t >= self.next_arrival && self.arrived < ARRIVALS.len() {
            let (from, addr, subject, body) = ARRIVALS[self.arrived];
            self.arrived += 1;
            self.next_arrival = t + 30.0;
            let id = self.next_id;
            self.next_id += 1;
            self.msgs.push(Msg {
                id, account: 0, folder: Folder::Inbox, thread: 2000 + id, from, addr, to: ACCOUNTS[0].1.into(),
                subject: subject.into(), body: body.into(), age: 0, unread: true, starred: false, flagged: false,
                labels: 0b0001, importance: 1, attachments: Vec::new(),
            });
            self.toast = Some((format!("New mail from {from}"), t, false));
            if !self.headless {
                let _ = blenderui::shell::notify(&format!("New mail from {from}"), subject, None);
            }
        }
        busy
    }

    fn snap(&self) -> Snap {
        let mut counts = Vec::new();
        for f in Folder::ALL {
            let v = View::Folder(f);
            let all = self.msgs.iter().filter(|m| self.in_view(m, v)).count();
            let unread = self.msgs.iter().filter(|m| self.in_view(m, v) && m.unread).count();
            counts.push((v, unread, all));
        }
        for v in [View::Starred, View::Unread] {
            counts.push((v, self.msgs.iter().filter(|m| self.in_view(m, v)).count(), 0));
        }
        for k in 0..self.labels.len() {
            let v = View::Label(k);
            counts.push((v, self.msgs.iter().filter(|m| self.in_view(m, v) && m.unread).count(),
                         self.msgs.iter().filter(|m| self.in_view(m, v)).count()));
        }
        let reader = self.cursor.and_then(|c| self.msg(c)).map_or_else(Vec::new, |m| {
            let mut t: Vec<Msg> = if self.rules.threads {
                self.msgs.iter().filter(|x| x.thread == m.thread && x.account == m.account).cloned().collect()
            } else {
                vec![m.clone()]
            };
            t.sort_by_key(|x| std::cmp::Reverse(x.age));
            t
        });
        Snap {
            rows: self.rows(),
            counts,
            labels: self.labels.iter().map(|l| (l.name.to_string(), l.color.clone())).collect(),
            reader,
            draft: self.draft.clone(),
            view: self.view,
            sel: self.sel.clone(),
            cursor: self.cursor,
            rules_snippets: self.rules.snippets,
            rules: (self.rules.archive_days, self.rules.auto_archive, self.rules.read_on_open, self.rules.snippets,
                    self.rules.threads, self.rules.density),
            hover_drop: self.hover_drop,
            dragging: self.drag.is_some_and(|d| d.3),
            sides: self.sides,
        }
    }

    fn key(&mut self, e: &KeyDownEvent, w: &Window) -> bool {
        let k = &e.keystroke;
        let ctrl = k.modifiers.control;
        // keys typed into a text field reach the page too (as in a browser): no shortcuts then
        if w.text_field_focused() {
            return false;
        }
        match (k.key.as_str(), ctrl) {
            ("j" | "Down", false) => self.step(true),
            ("k" | "Up", false) => self.step(false),
            ("Return" | "o", false) => {
                if let Some(c) = self.cursor {
                    self.open(c);
                }
            }
            ("e", false) => {
                let ids = self.targets();
                self.move_to(&ids, Folder::Archive);
            }
            ("Delete" | "numbersign", false) => {
                let ids = self.targets();
                self.move_to(&ids, Folder::Trash);
            }
            ("s", false) => self.star(),
            ("u", false) => self.toggle_read(),
            ("r", false) => self.compose("Reply"),
            ("a", false) => self.compose("Reply all"),
            ("f", false) => self.compose("Forward"),
            ("c", false) => self.compose("New message"),
            ("a", true) => self.sel = self.rows().iter().map(|r| r.id).collect(),
            ("z", true) => self.undo_or_unsend(),
            ("Escape", _) => {
                self.sel.clear();
                self.query.clear();
            }
            _ => return false,
        }
        true
    }

    fn star(&mut self) {
        let ids = self.targets();
        let on = !ids.iter().any(|&i| self.msg(i).is_some_and(|m| m.starred));
        self.act(if on { "Starred" } else { "Unstarred" }, &ids, |m| m.starred = on);
    }

    fn toggle_read(&mut self) {
        let ids = self.targets();
        let unread = !ids.iter().any(|&i| self.msg(i).is_some_and(|m| m.unread));
        self.act(if unread { "Marked unread" } else { "Marked read" }, &ids, |m| m.unread = unread);
    }

    /// Ctrl+Z: a message still waiting to go comes back to the editor; else the last action.
    fn undo_or_unsend(&mut self) {
        if self.outgoing.take().is_some() {
            self.draft = self.undo_draft.take();
            self.toast = Some(("Sending undone".into(), now(), false));
        } else {
            self.undo();
        }
    }

    /// Rules: archive what in the inbox is older than the days given.
    fn apply_rules(&mut self) {
        let mins = (self.rules.archive_days.max(0.0) * 1440.0) as u32;
        let ids: Vec<u32> = self.msgs.iter()
            .filter(|m| m.account == self.account && m.folder == Folder::Inbox && m.age > mins && !m.starred)
            .map(|m| m.id).collect();
        if ids.is_empty() {
            self.toast = Some((format!("Nothing older than {} days", self.rules.archive_days), now(), false));
        } else {
            let n = ids.len();
            self.act(format!("Rule archived {n}"), &ids, |m| m.folder = Folder::Archive);
        }
    }

    /// A dragged message let go over `to`: moved to a folder, or a label added.
    fn drop_on(&mut self, to: View) {
        let Some((id, ..)) = self.drag.take() else { return };
        self.hover_drop = None;
        let ids = if self.sel.contains(&id) { self.targets() } else { vec![id] };
        match to {
            View::Folder(f) => self.move_to(&ids, f),
            View::Label(k) => {
                let name = self.labels[k].name;
                self.act(format!("Labelled {name}"), &ids, |m| m.labels |= 1 << k);
            }
            View::Starred => self.act("Starred", &ids, |m| m.starred = true),
            View::Unread => self.act("Marked unread", &ids, |m| m.unread = true),
        }
    }
}

fn re(subject: &str, prefix: &str) -> String {
    if subject.starts_with(prefix) { subject.to_string() } else { format!("{prefix}{subject}") }
}

// ==========================================================================
// Look

const CSS: &str = r"
body { font-size: 13px; color: #dcdcdc; -webkit-font-smoothing: antialiased; user-select: none; }
.shell { display: flex; flex-direction: column; height: 100vh; }
.topbar { height: 34px; flex-shrink: 0; display: flex; align-items: center; gap: 8px; padding: 0 8px; background: #232323;
          border-bottom: 1px solid #161616; }
.brand { font-weight: 700; color: #e8e8e8; letter-spacing: 0.2px; margin-right: 6px; }
.brand b { color: #e3913c; }
.grow { flex: 1; }
.screen { flex: 1; min-height: 0; }
.status { height: 26px; flex-shrink: 0; display: flex; align-items: center; gap: 10px; padding: 0 10px; background: #232323;
          border-top: 1px solid #161616; color: #a8a8a8; font-size: 12px; font-variant-numeric: tabular-nums; }
.status kbd { font-size: 10.5px; }
.toast { display: flex; align-items: center; gap: 8px; color: #e8e8e8; animation: rise 220ms cubic-bezier(0.2, 0, 0, 1); }
.toast .dot { width: 8px; height: 8px; border-radius: 4px; background: #5aa05a; }
.toast.sending .dot { background: #e3913c; }

/* region contents, in Blender's region colors */
.hdr { display: flex; align-items: center; gap: 6px; height: 100%; padding: 0 6px; background: #3d3d3d; color: #e0e0e0; }
.hdr .title { font-weight: 600; margin-right: 4px; }
.tools { display: flex; flex-direction: column; gap: 4px; padding: 6px 4px; height: 100%; background: #2a2a2a; }
.side { height: 100%; padding: 6px 8px; background: #383838; overflow-y: auto; }
.side h4 { margin: 4px 0 6px; font-size: 12px; color: #bdbdbd; font-weight: 600; }
.kv { display: flex; justify-content: space-between; gap: 8px; padding: 2px 0; color: #bfbfbf; }
.kv span:last-child { color: #e6e6e6; text-align: right; }
.main { height: 100%; background: #303030; overflow-y: auto; }

/* folders */
.folder { display: flex; align-items: center; gap: 8px; height: 26px; padding: 0 10px; margin: 1px 4px; border-radius: 4px;
          cursor: pointer; color: #cfcfcf; transition-property: background-color, color; transition-duration: 120ms; }
.folder:hover { background: #3c3c3c; }
.folder.on { background: #4772b3; color: #fff; }
.folder.drop { background: #5a7fbf; box-shadow: inset 0 0 0 1px #9fc0ff; }
.folder .g { width: 14px; text-align: center; color: #9a9a9a; }
.folder.on .g { color: #fff; }
.folder .n { margin-left: auto; font-size: 11px; color: #9a9a9a; font-variant-numeric: tabular-nums; }
.folder .n.unread { color: #fff; background: #5a5a5a; border-radius: 8px; padding: 0 6px; }
.folder.on .n { color: #fff; }
.sect { padding: 10px 14px 4px; font-size: 11px; color: #8a8a8a; text-transform: uppercase; letter-spacing: 0.6px; }
.chip { width: 10px; height: 10px; border-radius: 3px; flex-shrink: 0; }
.labelrow { display: flex; align-items: center; gap: 6px; padding: 3px 0; }
.labelrow .nm { flex: 1; }

/* the list */
.row { display: flex; align-items: flex-start; gap: 8px; padding: 6px 8px 6px 6px; border-bottom: 1px solid #2a2a2a;
       cursor: pointer; transition-property: background-color; transition-duration: 100ms; }
.row:hover { background: #363636; }
.row.sel { background: #334d80; }
.row.cur { background: #4772b3; }
.row.dense { padding-top: 2px; padding-bottom: 2px; }
.row .mid { flex: 1; min-width: 0; }
.row .top { display: flex; gap: 6px; align-items: baseline; }
.row .who { flex: 1; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; color: #cfcfcf; }
.row.unread .who, .row.unread .subj { font-weight: 700; color: #f2f2f2; }
.row .time { font-size: 11.5px; color: #9a9a9a; font-variant-numeric: tabular-nums; flex-shrink: 0; }
.row.cur .time, .row.cur .snip { color: #dbe6fb; }
.row .subj { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: #d6d6d6; }
.row .snip { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: #8f8f8f; font-size: 12px; }
.row .count { font-size: 11px; color: #a0a0a0; padding: 0 5px; border-radius: 7px; background: #444; font-variant-numeric: tabular-nums; }
.row .dot { width: 7px; height: 7px; border-radius: 4px; background: #5c9cff; margin-top: 6px; flex-shrink: 0; opacity: 0; }
.row.unread .dot { opacity: 1; }
.star { width: 18px; text-align: center; color: #6a6a6a; cursor: pointer; transition-property: color, transform; transition-duration: 140ms; }
.star.on { color: #f0b429; }
.star:active { transform: scale(0.96); }
.lchips { display: flex; gap: 3px; margin-top: 3px; }
.lchip { font-size: 10px; padding: 0 5px; border-radius: 3px; color: #111; }
.empty { padding: 40px 20px; text-align: center; color: #888; text-wrap: balance; }

/* the reader */
.read { padding: 14px 18px 30px; }
.read h1 { margin: 0 0 10px; font-size: 19px; font-weight: 600; color: #f0f0f0; text-wrap: balance; }
.msg { margin-bottom: 8px; }
.msg summary { display: flex; gap: 8px; align-items: center; }
.msg .av { width: 26px; height: 26px; border-radius: 13px; display: flex; align-items: center; justify-content: center;
           font-weight: 700; color: #fff; font-size: 12px; flex-shrink: 0; }
.msg .meta { flex: 1; }
.msg .meta small { color: #9a9a9a; }
.msg .body { padding: 8px 4px 4px 34px; color: #d8d8d8; line-height: 1.55; white-space: pre-wrap; text-wrap: pretty; }
.att { display: inline-flex; align-items: center; gap: 6px; margin: 8px 6px 0 34px; padding: 6px 10px; border-radius: 4px;
       background: #3a3a3a; box-shadow: inset 0 0 0 1px #4a4a4a; color: #d0d0d0; }
.att small { color: #9a9a9a; font-variant-numeric: tabular-nums; }

/* compose */
.compose { padding: 12px 16px; display: flex; flex-direction: column; gap: 6px; }
.compose .line { display: flex; align-items: center; gap: 8px; }
.compose .line label { width: 54px; color: #a8a8a8; text-align: right; }
.compose .file { display: flex; align-items: center; gap: 8px; padding: 2px 0 2px 62px; }
.compose .file span { width: 160px; color: #cfcfcf; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.compose .file small { width: 64px; color: #9a9a9a; font-variant-numeric: tabular-nums; text-align: right; }

.ghost { position: absolute; padding: 4px 10px; border-radius: 4px; background: #4772b3; color: #fff; font-size: 12px;
         box-shadow: 0 6px 16px rgba(0,0,0,0.45); pointer-events: none; }
@keyframes rise { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: translateY(0); } }
";

fn css() -> &'static str {
    static S: OnceLock<String> = OnceLock::new();
    S.get_or_init(|| CSS.to_string())
}

/// An avatar's color from a name.
fn tint(name: &str) -> &'static str {
    const C: [&str; 6] = ["#4772b3", "#b35a5a", "#5aa05a", "#b3893f", "#7a5ab3", "#3f9cb3"];
    C[name.bytes().map(usize::from).sum::<usize>() % C.len()]
}

type W = WeakEntity<Mail>;

// ==========================================================================
// Areas

fn folders_area(s: &Snap, weak: W, k: u32) -> Editor {
    let mut list = div().class("main").child(div().class("sect").child("Mailboxes"));
    let item = |v: View, glyph: &str, name: String, unread: usize, all: usize| {
        let on = s.view == v;
        let drop = s.dragging && s.hover_drop == Some(v);
        let id = match v {
            View::Folder(f) => format!("f-{}", f.name().to_lowercase()),
            View::Starred => "f-starred".into(),
            View::Unread => "f-unread".into(),
            View::Label(i) => format!("l-{i}"),
        };
        let count = if unread > 0 { div().class("n unread").child(unread.to_string()) }
                    else { div().class("n").child(if all > 0 { all.to_string() } else { String::new() }) };
        div().id(area_id(&id, k)).class(match (on, drop) { (_, true) => "folder drop", (true, _) => "folder on", _ => "folder" })
            .child(div().class("g").child(glyph.to_string())).child(name).child(count)
            .on_click(weak.listener(move |this: &mut Mail, _: &ClickEvent, _, cx| {
                if this.drag.is_some_and(|d| d.3) {
                    this.drop_on(v);
                } else {
                    this.drag = None;
                    this.view = v;
                    let rows = this.rows();
                    this.cursor = rows.first().map(|r| r.id);
                    this.sel = this.cursor.into_iter().collect();
                }
                cx.notify();
            }))
            .on_mouse_up(MouseButton::Left, weak.listener(move |this: &mut Mail, _: &MouseUpEvent, _, cx| {
                if this.drag.is_some_and(|d| d.3) {
                    this.drop_on(v);
                    cx.notify();
                }
            }))
            .on_hover(weak.listener(move |this: &mut Mail, over: &bool, _, cx| {
                if this.drag.is_some() {
                    this.hover_drop = if *over { Some(v) } else { None };
                    cx.notify();
                }
            }))
    };
    for (v, unread, all) in &s.counts {
        match *v {
            View::Folder(f) => list = list.child(item(*v, f.glyph(), f.name().into(), *unread, if f == Folder::Inbox { 0 } else { *all })),
            View::Starred => list = list.child(div().class("sect").child("Views")).child(item(*v, "★", "Starred".into(), 0, *unread)),
            View::Unread => list = list.child(item(*v, "●", "Unread".into(), 0, *unread)),
            View::Label(i) => {
                if i == 0 {
                    list = list.child(div().class("sect").child("Labels"));
                }
                let (name, color) = &s.labels[i];
                list = list.child(item(*v, "", name.clone(), *unread, *all)
                    .child(div().class("chip").css("background", color.clone()).css("order", "-1")));
            }
        }
    }
    // the sidebar: label colors (Blender's color picker) and the rules
    let mut side = div().class("side").child(h4().child("Label colors"));
    for (i, (name, color)) in s.labels.iter().enumerate() {
        side = side.child(div().class("labelrow")
            .child(div().class("nm").child(name.clone()))
            .child(ColorSwatch().id(area_id(&format!("lc-{i}"), k)).value(color.clone()).w(px(56.0))
                .on_change(weak.listener(move |this: &mut Mail, e: &ChangeEvent, _, cx| {
                    this.labels[i].color.clone_from(&e.value);
                    cx.notify();
                }))));
    }
    let (days, auto, read_open, snippets, threads, density) = s.rules;
    let check = |id: &str, label: &str, on: bool, f: fn(&mut Rules, bool)| {
        div().class("labelrow").child(Checkbox().id(area_id(id, k)).checked(on)
            .on_change(weak.listener(move |this: &mut Mail, e: &ChangeEvent, _, cx| {
                f(&mut this.rules, e.checked);
                cx.notify();
            }))).child(label.to_string())
    };
    side = side.child(details().id(area_id("rules", k)).open(true).child(summary().child("Rules"))
        .child(div().class("labelrow").child("Archive after")
            .child(NumberField().id(area_id("days", k)).min(1.0).max(365.0).step(1.0).value(format!("{days}")).w(px(70.0))
                .on_change(weak.listener(|this: &mut Mail, e: &ChangeEvent, _, cx| {
                    this.rules.archive_days = e.number.round().clamp(1.0, 365.0);
                    cx.notify();
                })))
            .child("days"))
        .child(check("auto", "Archive automatically", auto, |r, v| r.auto_archive = v))
        .child(Button().id(area_id("apply-rules", k)).child("Apply now")
            .on_click(weak.listener(|this: &mut Mail, _: &ClickEvent, _, cx| {
                this.apply_rules();
                cx.notify();
            }))))
        .child(details().id(area_id("display", k)).open(true).child(summary().child("Display"))
            .child(check("read-open", "Mark read when opened", read_open, |r, v| r.read_on_open = v))
            .child(check("snippets", "Show snippets", snippets, |r, v| r.snippets = v))
            .child(check("threads", "Conversations", threads, |r, v| r.threads = v))
            .child(div().class("labelrow").child("Density")
                .child(NumberSlider().id(area_id("density", k)).min(0.0).max(2.0).step(1.0).value(format!("{density}")).w(px(110.0))
                    .on_change(weak.listener(|this: &mut Mail, e: &ChangeEvent, _, cx| {
                        this.rules.density = e.number.round().clamp(0.0, 2.0);
                        cx.notify();
                    })))));
    let header = div().class("hdr").child(div().class("title").child("Folders"))
        .child(div().class("grow"))
        .child(Button().id(area_id("folders-side", k)).child(if s.sides.0 { "Rules ◂" } else { "Rules ▸" })
            .on_click(weak.listener(|this: &mut Mail, _: &ClickEvent, _, cx| {
                this.sides.0 = !this.sides.0;
                cx.notify();
            })));
    Editor::new(list)
        .header(Region::new(header).size(26))
        .sidebar(Region::new(side).size(230).overlap(false).hidden(!s.sides.0).hotkey("n"))
}

fn list_area(s: &Snap, weak: W, k: u32, area: AreaRef) -> Editor {
    let dense = s.rules.5 < 0.5;
    let mut list = div().id(area_id("list", k)).class("main");
    if s.rows.is_empty() {
        list = list.child(div().class("empty").child("Nothing here. Drag a message onto a folder on the left to move it."));
    }
    for r in &s.rows {
        let id = r.id;
        let mut cls = String::from("row");
        if r.unread {
            cls.push_str(" unread");
        }
        if s.sel.contains(&id) {
            cls.push_str(" sel");
        }
        if s.cursor == Some(id) {
            cls.push_str(" cur");
        }
        if dense {
            cls.push_str(" dense");
        }
        let mut top = div().class("top").child(div().class("who").child(r.from));
        if r.count > 1 {
            top = top.child(div().class("count").child(r.count.to_string()));
        }
        if r.attach {
            top = top.child(span().child("⎘"));
        }
        if r.flagged {
            top = top.child(span().css("color", "#e05a5a").child("⚑"));
        }
        top = top.child(div().class("time").child(r.when.clone()));
        let mut mid = div().class("mid").child(top).child(div().class("subj").child(r.subject.clone()));
        if s.rules_snippets && !dense {
            mid = mid.child(div().class("snip").child(r.snippet.clone()));
        }
        if r.labels != 0 {
            let mut chips = div().class("lchips");
            for (i, (name, color)) in s.labels.iter().enumerate() {
                if r.labels & (1 << i) != 0 {
                    chips = chips.child(div().class("lchip").css("background", color.clone()).child(name.clone()));
                }
            }
            mid = mid.child(chips);
        }
        list = list.child(div().id(area_id(&format!("m{id}"), k)).class(cls)
            .child(Checkbox().id(area_id(&format!("c{id}"), k)).checked(s.sel.contains(&id))
                .on_change(weak.listener(move |this: &mut Mail, e: &ChangeEvent, _, cx| {
                    if e.checked {
                        if !this.sel.contains(&id) {
                            this.sel.push(id);
                        }
                    } else {
                        this.sel.retain(|&x| x != id);
                    }
                    cx.notify();
                })))
            .child(div().id(area_id(&format!("s{id}"), k)).class(if r.starred { "star on" } else { "star" }).child(if r.starred { "★" } else { "☆" })
                .on_click(weak.listener(move |this: &mut Mail, _: &ClickEvent, _, cx| {
                    let on = !this.msg(id).is_some_and(|m| m.starred);
                    this.act(if on { "Starred" } else { "Unstarred" }, &[id], |m| m.starred = on);
                    cx.stop_propagation();
                    cx.notify();
                })))
            .child(div().class("dot"))
            .child(mid)
            .on_mouse_down(MouseButton::Left, weak.listener(move |this: &mut Mail, e: &MouseDownEvent, _, _| {
                this.drag = Some((id, e.position.x.0, e.position.y.0, false));
            }))
            .on_click(weak.listener(move |this: &mut Mail, e: &ClickEvent, w, cx| {
                if this.drag.is_some_and(|d| d.3) {
                    return;
                }
                this.drag = None;
                this.click_row(id, e.down.modifiers.shift, e.down.modifiers.control);
                w.focus_id("root");
                cx.notify();
            })));
    }
    let header = div().class("hdr")
        .child(div().class("title").child(match s.view {
            View::Folder(f) => f.name().to_string(),
            View::Starred => "Starred".into(),
            View::Unread => "Unread".into(),
            View::Label(i) => s.labels[i].0.clone(),
        }))
        .child(small().child(format!("{} conversations", s.rows.len())))
        .child(div().class("grow"))
        .child(Button().id(area_id("split", k)).child("Split").on_click(move |_, _, _| {
            let _ = area.split(Axis::H, 0.5);
        }))
        .child(Button().id(area_id("max", k)).child("Maximize").on_click(move |_, _, _| area.maximize()));
    let sel = !s.sel.is_empty() || s.cursor.is_some();
    let tool = |id: &str, label: &'static str, f: fn(&mut Mail)| {
        Button().id(area_id(id, k)).child(label).disabled(!sel)
            .on_click(weak.listener(move |this: &mut Mail, _: &ClickEvent, _, cx| {
                f(this);
                cx.notify();
            }))
    };
    let tools = div().class("tools")
        .child(tool("t-archive", "Archive", |m| { let ids = m.targets(); m.move_to(&ids, Folder::Archive); }))
        .child(tool("t-delete", "Delete", |m| { let ids = m.targets(); m.move_to(&ids, Folder::Trash); }))
        .child(tool("t-spam", "Spam", |m| { let ids = m.targets(); m.move_to(&ids, Folder::Spam); }))
        .child(tool("t-star", "Star", Mail::star))
        .child(tool("t-read", "Read", Mail::toggle_read))
        .child(tool("t-flag", "Flag", |m| {
            let ids = m.targets();
            let on = !ids.iter().any(|&i| m.msg(i).is_some_and(|x| x.flagged));
            m.act(if on { "Flagged" } else { "Unflagged" }, &ids, |x| x.flagged = on);
        }));
    Editor::new(list)
        .header(Region::new(header).size(26))
        .toolbar(Region::new(tools).size(76).overlap(false))
}

fn reader_area(s: &Snap, weak: W, k: u32, area: AreaRef) -> Editor {
    if let Some(d) = &s.draft {
        return compose_area(d, weak, k);
    }
    let mut read = div().class("main read");
    let Some(head) = s.reader.last() else {
        return Editor::new(read.child(div().class("empty").child("Pick a message. J and K walk the list.")));
    };
    read = read.child(h1().child(head.subject.clone()));
    let n = s.reader.len();
    for (i, m) in s.reader.iter().enumerate() {
        let initial: String = m.from.chars().take(1).collect();
        let mut panel = details().id(area_id(&format!("p{}", m.id), k)).open(i + 1 == n || m.unread).class("msg")
            .child(summary()
                .child(div().class("av").css("background", tint(m.from)).child(initial))
                .child(div().class("meta").child(b().child(m.from)).child(small().child(format!("  <{}>", m.addr))))
                .child(small().child(when(m.age))))
            .child(div().class("body").child(m.body.clone()));
        for (name, kb) in &m.attachments {
            panel = panel.child(div().class("att").child("⎘").child(name.clone()).child(small().child(format!("{kb} KB"))));
        }
        read = read.child(panel);
    }
    let action = |id: &str, label: &'static str, kind: &'static str| {
        Button().id(area_id(id, k)).child(label).on_click(weak.listener(move |this: &mut Mail, _: &ClickEvent, _, cx| {
            this.compose(kind);
            cx.notify();
        }))
    };
    let header = div().class("hdr")
        .child(action("reply", "Reply", "Reply"))
        .child(action("reply-all", "Reply all", "Reply all"))
        .child(action("forward", "Forward", "Forward"))
        .child(div().class("grow"))
        .child(Button().id(area_id("reader-side", k)).child("Properties")
            .on_click(weak.listener(|this: &mut Mail, _: &ClickEvent, _, cx| {
                this.sides.1 = !this.sides.1;
                cx.notify();
            })))
        .child(Button().id(area_id("rmax", k)).child("Maximize").on_click(move |_, _, _| area.maximize()));
    // the sidebar: this message's properties
    let m = head;
    let id = m.id;
    let mut side = div().class("side")
        .child(h4().child("Message"))
        .child(div().class("kv").child(span().child("From")).child(span().child(m.from)))
        .child(div().class("kv").child(span().child("Date")).child(span().child(when(m.age))))
        .child(div().class("kv").child(span().child("Size")).child(span().child(format!("{} KB", 3 + m.body.len() / 40 + m.attachments.iter().map(|a| a.1 as usize).sum::<usize>()))))
        .child(div().class("kv").child(span().child("In conversation")).child(span().child(n.to_string())))
        .child(h4().child("Labels"));
    for (i, (name, color)) in s.labels.iter().enumerate() {
        let on = m.labels & (1 << i) != 0;
        side = side.child(div().class("labelrow")
            .child(Checkbox().id(area_id(&format!("ml{i}"), k)).checked(on)
                .on_change(weak.listener(move |this: &mut Mail, e: &ChangeEvent, _, cx| {
                    let on = e.checked;
                    let name = this.labels[i].name;
                    this.act(format!("{} {name}", if on { "Labelled" } else { "Unlabelled" }), &[id], |m| {
                        if on { m.labels |= 1 << i } else { m.labels &= !(1 << i) }
                    });
                    cx.notify();
                })))
            .child(div().class("chip").css("background", color.clone()))
            .child(name.clone()));
    }
    side = side.child(h4().child("Importance"));
    for (v, label) in [(0u8, "Low"), (1, "Normal"), (2, "High")] {
        side = side.child(div().class("labelrow").child(Radio().id(area_id(&format!("imp{v}"), k)).attr("name", area_id("importance", k))
            .checked(m.importance == v)
            .on_change(weak.listener(move |this: &mut Mail, e: &ChangeEvent, _, cx| {
                if e.checked && let Some(m) = this.msg_mut(id) {
                    m.importance = v;
                    cx.notify();
                }
            }))).child(label));
    }
    side = side.child(h4().child("Move to"))
        .child(Dropdown().id(area_id("moveto", k)).value(m.folder.name())
            .children(Folder::ALL.map(|f| option().child(f.name())))
            .on_change(weak.listener(move |this: &mut Mail, e: &ChangeEvent, _, cx| {
                if let Some(&f) = Folder::ALL.get(e.number as usize) {
                    this.move_to(&[id], f);
                    cx.notify();
                }
            })));
    Editor::new(read)
        .header(Region::new(header).size(26))
        .sidebar(Region::new(side).size(210).overlap(false).hidden(!s.sides.1).hotkey("n"))
}

fn compose_area(d: &Draft, weak: W, k: u32) -> Editor {
    let field = |id: &str, label: &str, value: &str, f: fn(&mut Draft, String)| {
        div().class("line").child(label_el(label))
            .child(TextField().id(area_id(id, k)).value(value.to_string()).flex_1()
                .on_input(weak.listener(move |this: &mut Mail, e: &ChangeEvent, _, cx| {
                    if let Some(d) = &mut this.draft {
                        f(d, e.value.clone());
                    }
                    cx.notify();
                })))
    };
    let mut c = div().class("main compose")
        .child(field("to", "To", &d.to, |d, v| d.to = v))
        .child(field("cc", "Cc", &d.cc, |d, v| d.cc = v))
        .child(field("subject", "Subject", &d.subject, |d, v| d.subject = v))
        .child(TextArea().id(area_id(&format!("body{}", d.rev), k)).value(d.body.clone()).h(px(260.0))
            .on_input(weak.listener(|this: &mut Mail, e: &ChangeEvent, _, cx| {
                if let Some(d) = &mut this.draft {
                    d.body.clone_from(&e.value);
                }
                cx.notify();
            })));
    for (i, (name, kb, p)) in d.files.iter().enumerate() {
        c = c.child(div().class("file").child(span().child(format!("⎘ {name}")))
            .child(ProgressBar().id(area_id(&format!("up{i}"), k)).max(1.0).value(format!("{p}")).w(px(160.0)))
            .child(small().child(if *p >= 1.0 { format!("{kb} KB") } else { format!("{:.0}%", p * 100.0) })));
    }
    let header = div().class("hdr").child(div().class("title").child(d.kind))
        .child(div().class("grow"))
        .child(small().child("Signature"))
        .child(Dropdown().id(area_id("sig", k)).value(SIGNATURES[d.signature].0)
            .children(SIGNATURES.map(|(n, _)| option().child(n)))
            .on_change(weak.listener(|this: &mut Mail, e: &ChangeEvent, _, cx| {
                if let Some(d) = &mut this.draft {
                    let old = SIGNATURES[d.signature].1;
                    let new = e.number as usize;
                    d.body = if !old.is_empty() && let Some(at) = d.body.find(old) {
                        format!("{}{}{}", &d.body[..at], SIGNATURES[new].1, &d.body[at + old.len()..])
                    } else {
                        format!("{}{}", SIGNATURES[new].1, d.body)
                    };
                    d.signature = new;
                    d.rev += 1;
                }
                cx.notify();
            })))
        .child(Button().id(area_id("attach", k)).child("Attach").on_click(weak.listener(|this: &mut Mail, _: &ClickEvent, _, cx| {
            if let Some(d) = &mut this.draft {
                const FILES: [(&str, u32); 4] = [("frame_0420.exr", 9400), ("notes.txt", 6), ("lookdev.blend", 21000), ("ref_board.jpg", 2300)];
                let (n, kb) = FILES[d.files.len() % FILES.len()];
                d.files.push((n.to_string(), kb, 0.0));
            }
            cx.notify();
        })))
        .child(Button().id(area_id("discard", k)).child("Discard").on_click(weak.listener(|this: &mut Mail, _: &ClickEvent, _, cx| {
            this.draft = None;
            this.toast = Some(("Draft discarded".into(), now(), false));
            cx.notify();
        })))
        .child(Button().id(area_id("send", k)).child("Send").on_click(weak.listener(|this: &mut Mail, _: &ClickEvent, _, cx| {
            this.send();
            cx.notify();
        })));
    Editor::new(c).header(Region::new(header).size(26))
}

fn label_el(text: &str) -> Div {
    label().child(text.to_string())
}

// ==========================================================================
// The window

impl Render for Mail {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        focus_root(w, &mut self.frames);
        if self.tick() || self.drag.is_some() {
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
            .editor("Folders", "OUTLINER", editor(|s, w, k, _| folders_area(s, w, k)))
            .editor("Messages", "SPREADSHEET", editor(list_area))
            .editor("Reader", "TEXT", editor(reader_area));
        let screen = Screen().id("mail-screen").editors(editors).class("screen")
            .layout(AreaSplit::v(0.2, "Folders", AreaSplit::v(0.44, "Messages", "Reader")));

        // the top bar: account, compose, search, sort
        let top = div().class("topbar")
            .child(div().class("brand").child("BUI ").child(b().child("Mail")))
            .child(Dropdown().id("account").value(ACCOUNTS[self.account].0)
                .children(ACCOUNTS.map(|(n, _)| option().child(n)))
                .on_change(cx.listener(|this, e: &ChangeEvent, _, cx| {
                    this.account = (e.number as usize).min(ACCOUNTS.len() - 1);
                    this.view = View::Folder(Folder::Inbox);
                    let rows = this.rows();
                    this.cursor = rows.first().map(|r| r.id);
                    this.sel = this.cursor.into_iter().collect();
                    this.draft = None;
                    cx.notify();
                })))
            .child(Button().id("compose").child("Compose").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.compose("New message");
                cx.notify();
            })))
            .child(div().class("grow"))
            .child(TextField().id("search").attr("placeholder", "Search mail").attr("live", "").value(self.query.clone()).w(px(280.0))
                .on_input(cx.listener(|this, e: &ChangeEvent, _, cx| {
                    this.query.clone_from(&e.value);
                    let rows = this.rows();
                    if this.cursor.is_some_and(|c| !rows.iter().any(|r| r.id == c)) {
                        this.cursor = rows.first().map(|r| r.id);
                    }
                    cx.notify();
                })))
            .child(Dropdown().id("sort").value(SORTS[self.sort])
                .children(SORTS.map(|s| option().child(s)))
                .on_change(cx.listener(|this, e: &ChangeEvent, _, cx| {
                    this.sort = (e.number as usize).min(SORTS.len() - 1);
                    cx.notify();
                })))
            .child(Button().id("undo").child("Undo").disabled(self.undo.is_empty() && self.outgoing.is_none())
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.undo_or_unsend();
                    cx.notify();
                })));

        // the status bar: what just happened (with Undo), the keys, the counts
        let mut status = div().class("status");
        if let Some(o) = &self.outgoing {
            let left = (o.at - now()).max(0.0);
            status = status.child(div().id("sending").class("toast sending").child(div().class("dot"))
                .child(format!("Sending “{}” in {:.0} s", o.msg.subject, left.ceil()))
                .child(ProgressBar().id("send-progress").max(5.0).value(format!("{}", 5.0 - left)).w(px(90.0)))
                .child(Button().id("unsend").child("Undo").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.undo_or_unsend();
                    cx.notify();
                }))));
        } else if let Some((text, _, can_undo)) = &self.toast {
            let mut t = div().id(format!("toast-{}", self.undo.len())).class("toast").child(div().class("dot")).child(text.clone());
            if *can_undo {
                t = t.child(Button().id("toast-undo").child("Undo").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.undo();
                    cx.notify();
                })));
            }
            status = status.child(t);
        }
        let unread = self.msgs.iter().filter(|m| m.account == self.account && m.folder == Folder::Inbox && m.unread).count();
        status = status.child(div().class("grow"))
            .child(span().child(kbd().child("J")).child(" ").child(kbd().child("K")).child(" move  ")
                .child(kbd().child("E")).child(" archive  ").child(kbd().child("R")).child(" reply  ")
                .child(kbd().child("Ctrl")).child(kbd().child("Z")).child(" undo  ").child(kbd().child("N")).child(" sidebar"))
            .child(span().child(format!("{unread} unread · {} selected", self.sel.len())));

        let mut root = app().id("root").attr("tabindex", "0")
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                if this.key(e, w) {
                    // taken: Blender's own keys (a widget's undo, a hovered widget's shortcut) don't run too
                    cx.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if let Some((id, x0, y0, moved)) = this.drag {
                    let (x, y) = (e.position.x.0, e.position.y.0);
                    let far = moved || (x - x0).abs() + (y - y0).abs() > 6.0;
                    this.drag = Some((id, if far { x } else { x0 }, if far { y } else { y0 }, far));
                    if far {
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| {
                if this.drag.is_some_and(|d| d.3) {
                    // let go anywhere but a folder: the drag is over
                    let to = this.hover_drop;
                    match to {
                        Some(v) => this.drop_on(v),
                        None => this.drag = None,
                    }
                    cx.notify();
                } else {
                    this.drag = None;
                }
            }))
            .child(head().child(style().child(css())))
            .child(div().class("shell").child(top).child(screen).child(status));
        if let Some((id, x, y, true)) = self.drag {
            let n = if self.sel.contains(&id) { self.sel.len() } else { 1 };
            let subject = self.msg(id).map(|m| m.subject.clone()).unwrap_or_default();
            root = root.child(div().class("ghost").css("left", format!("{}px", x + 12.0)).css("top", format!("{}px", y + 10.0))
                .child(if n > 1 { format!("{n} messages") } else { subject }));
        }
        root
    }
}

/// Every feature in turn: open, J / K, multi-select, star, archive, undo, drag to a folder, search,
/// sort, reply with a signature and an attachment, send and undo, label colors, rules, accounts.
fn storyboard(dir: String) -> Script {
    story(dir, |s| {
        s.play("00-open", &[0, 300]);
        s.hover("list");
        s.key("j");
        s.key("j");
        s.play("01-jk", &[0, 200]);
        let b = s.bounds("m6");
        s.cx.simulate_click_with(s.win, b.origin.x + 200.0, b.origin.y + 10.0, false, true);
        s.play("02-shift-select", &[0, 200]);
        s.key("s");
        s.play("03-star", &[0, 200]);
        s.key("e");
        s.play("04-archive", &[0, 200]);
        s.cx.simulate_key_with(s.win, "z", "z", true, false);
        s.play("05-undo", &[0, 200]);
        // drag a message onto Trash
        let (from, to) = (s.centre("m7"), s.centre("f-trash"));
        s.cx.simulate_drag(s.win, (from.0 + 60.0, from.1), (to.0, to.1));
        s.play("06-drag-to-trash", &[0, 200]);
        s.click("search");
        s.type_text("cont");
        s.play("07-typing", &[0, 200]);
        s.type_text("ract");
        s.play("07-search", &[0, 200]);
        s.key("Return");
        s.key("Escape");
        s.play("08-cleared", &[0, 200]);
        s.click("m2");
        s.hover("list");
        s.key("r");
        s.play("09-reply", &[0, 200]);
        s.click("attach");
        s.play("10-attach", &[0, 300, 1500, 4000]);
        s.click("send");
        s.play("11-sending", &[0, 1500]);
        s.click("unsend");
        s.play("12-unsent", &[0, 200]);
        s.click("send");
        s.play("13-sent", &[0, 5600]);
        s.click("f-sent");
        s.play("14-sent-folder", &[0, 200]);
        s.click("f-inbox");
        s.click("folders-side");
        s.click("reader-side");
        s.play("15-sidebars", &[0, 300]);
        s.click("threads");
        s.play("16-no-threads", &[0, 200]);
        s.click("max");
        s.play("17-maximized", &[0, 300]);
        s.tick(30_000);
        s.play("18-new-mail", &[0, 200]);
    })
}

fn main() {
    let (w, h) = size_arg((1440.0, 860.0));
    run_with(w, h, "BUI Mail", Mail::new, arg("--storyboard").map(storyboard));
}

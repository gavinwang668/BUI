//! Tessera: a block notes wiki and task database made of Blender's UI. One Blender screen holds
//! three editors: Pages (search, favourites and the page tree), and two Page editors side by side,
//! each showing its own page (Ctrl+click a page in the tree to open it in the side one). A Page
//! editor's regions are a header (breadcrumb, favourite, properties, undo / redo), a properties
//! sidebar (icon, cover colour, tag colours, stats) and a footer.
//!
//! A page is a list of blocks: text, three headings, to-dos (Blender checkboxes), toggles, bullet
//! and numbered lists, quotes, callouts, code, dividers and image placeholders. Click a block and
//! type; Enter starts the next one, Backspace on an empty one turns it back into text and then
//! removes it. Start a line with `# `, `## `, `- `, `1. `, `[] `, `> `, `+ ` (toggle), `! `
//! (callout), ```` ``` ```` or `---` for Markdown shortcuts, or type `/` for the block menu (filter
//! by typing, arrows and Enter). Tab / Shift+Tab indent, Alt+Up / Down move a block, its grip drags
//! it, and a double-click opens its text in a Blender text field for editing mid-line.
//!
//! A database page is a task table of Blender widgets (a text field, a status dropdown, an
//! estimate number field and a tag dropdown whose colours are `ColorSwatch`es in the sidebar), sorted
//! by its column headers and filtered by status, or a board whose cards drag between columns.
//! The tree's rows drag too: onto a row's top or bottom edge to reorder, onto its middle to nest.
//! Ctrl+Z / Ctrl+Y (or Ctrl+Shift+Z) undo and redo every change.
//!
//! What Blender's UI cannot do here: a text field holds plain text, so blocks are text Tessera
//! draws itself, typed into at their end (a double-click opens a field for edits mid-line); its
//! menus are dropdowns, so the slash menu is drawn in the page.
//!
//! `cargo run --release --bin bui_notes`; `-- --storyboard DIR` drives it headlessly.

use std::sync::Arc;

use blenderui::prelude::*;
use blenderui::screen::{AreaRef, AreaSplit, Editor, Editors, Region, Screen};
use bui_demos::story::{story, Story};
use bui_demos::text::{edit, Edit};
use bui_demos::{arg, focus_root, run_with, size_arg, Script};
use bui_demos::color::{hex, parse_hex};

// ==========================================================================
// The workspace

/// What a block is. A to-do knows whether it is done, a toggle whether it is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Text,
    H1,
    H2,
    H3,
    Todo { done: bool },
    Toggle { open: bool },
    Bullet,
    Number,
    Quote,
    Callout,
    Code,
    Divider,
    Image,
}

/// The slash menu: each kind, its name and what it is for.
const KINDS: [(Kind, &str, &str); 13] = [
    (Kind::Text, "Text", "Just start writing"),
    (Kind::H1, "Heading 1", "Big section heading"),
    (Kind::H2, "Heading 2", "Medium section heading"),
    (Kind::H3, "Heading 3", "Small section heading"),
    (Kind::Todo { done: false }, "To-do list", "Track tasks with a checkbox"),
    (Kind::Toggle { open: true }, "Toggle list", "Hide what is indented under it"),
    (Kind::Bullet, "Bulleted list", "A simple bulleted list"),
    (Kind::Number, "Numbered list", "A list that counts"),
    (Kind::Quote, "Quote", "Set a quotation apart"),
    (Kind::Callout, "Callout", "Make writing stand out"),
    (Kind::Code, "Code", "A snippet in monospace"),
    (Kind::Divider, "Divider", "Separate sections"),
    (Kind::Image, "Image", "A placeholder with a caption"),
];

impl Kind {
    const fn class(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::H1 => "h1",
            Self::H2 => "h2",
            Self::H3 => "h3",
            Self::Todo { done: false } => "todo",
            Self::Todo { done: true } => "todo done",
            Self::Toggle { .. } => "toggle",
            Self::Bullet => "bullet",
            Self::Number => "number",
            Self::Quote => "quote",
            Self::Callout => "callout",
            Self::Code => "code",
            Self::Divider => "divider",
            Self::Image => "image",
        }
    }

    /// What Enter at its end starts.
    const fn next(self) -> Self {
        match self {
            Self::Todo { .. } => Self::Todo { done: false },
            Self::Bullet | Self::Number => self,
            _ => Self::Text,
        }
    }

    /// A list kind: Enter on an empty one ends the list.
    const fn listed(self) -> bool {
        matches!(self, Self::Todo { .. } | Self::Bullet | Self::Number | Self::Toggle { .. } | Self::Quote | Self::Callout)
    }

    const fn texty(self) -> bool {
        !matches!(self, Self::Divider)
    }

    const fn placeholder(self) -> &'static str {
        match self {
            Self::H1 => "Heading 1",
            Self::H2 => "Heading 2",
            Self::H3 => "Heading 3",
            Self::Todo { .. } => "To-do",
            Self::Toggle { .. } => "Toggle",
            Self::Bullet | Self::Number => "List",
            Self::Quote => "Quote",
            Self::Callout => "Callout",
            Self::Code => "Code",
            Self::Image => "Add a caption",
            _ => "Type \u{2018}/\u{2019} for commands",
        }
    }
}

/// A Markdown shortcut typed at the start of a text block.
fn shortcut(text: &str) -> Option<Kind> {
    Some(match text {
        "# " => Kind::H1,
        "## " => Kind::H2,
        "### " => Kind::H3,
        "- " | "* " => Kind::Bullet,
        "1. " => Kind::Number,
        "[] " | "[ ] " => Kind::Todo { done: false },
        "> " => Kind::Quote,
        "+ " => Kind::Toggle { open: true },
        "! " => Kind::Callout,
        "``` " => Kind::Code,
        "--- " => Kind::Divider,
        _ => return None,
    })
}

#[derive(Clone, PartialEq, Debug)]
struct Block {
    kind: Kind,
    text: String,
    indent: u8,
}

fn blk(kind: Kind, text: &str) -> Block {
    Block { kind, text: text.to_owned(), indent: 0 }
}

/// Blocks hidden under a closed toggle (more indented, right after it).
fn hidden(blocks: &[Block]) -> Vec<bool> {
    let mut under: Option<u8> = None;
    blocks.iter().map(|b| {
        if let Some(d) = under {
            if b.indent > d {
                return true;
            }
            under = None;
        }
        if b.kind == (Kind::Toggle { open: false }) {
            under = Some(b.indent);
        }
        false
    }).collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Todo,
    Doing,
    Done,
}

const STATUSES: [Status; 3] = [Status::Todo, Status::Doing, Status::Done];

impl Status {
    const fn name(self) -> &'static str {
        match self {
            Self::Todo => "To do",
            Self::Doing => "Doing",
            Self::Done => "Done",
        }
    }

    const fn class(self) -> &'static str {
        match self {
            Self::Todo => "s-todo",
            Self::Doing => "s-doing",
            Self::Done => "s-done",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
struct Task {
    name: String,
    status: Status,
    estimate: f32,
    tag: usize,
}

#[derive(Clone, PartialEq, Debug)]
struct Tag {
    name: &'static str,
    color: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Table,
    Board,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sort {
    Manual,
    Name,
    Status,
    Estimate,
}

#[derive(Clone, PartialEq, Debug)]
struct Db {
    tasks: Vec<Task>,
    tags: Vec<Tag>,
    view: View,
    sort: Sort,
    filter: Option<Status>,
}

impl Db {
    /// The tasks shown, in order.
    fn order(&self) -> Vec<usize> {
        let mut o: Vec<usize> = (0..self.tasks.len()).filter(|&t| self.filter.is_none_or(|f| self.tasks[t].status == f)).collect();
        let t = &self.tasks;
        match self.sort {
            Sort::Manual => {}
            Sort::Name => o.sort_by(|&a, &b| t[a].name.to_lowercase().cmp(&t[b].name.to_lowercase())),
            Sort::Status => o.sort_by_key(|&a| t[a].status as u8),
            Sort::Estimate => o.sort_by(|&a, &b| t[b].estimate.total_cmp(&t[a].estimate)),
        }
        o
    }
}

#[derive(Clone, PartialEq, Debug)]
enum Body {
    Blocks(Vec<Block>),
    Db(Db),
}

#[derive(Clone, PartialEq, Debug)]
struct Page {
    title: String,
    icon: usize,
    cover: u32,
    parent: Option<usize>,
    children: Vec<usize>,
    fav: bool,
    body: Body,
}

/// Page icons: a glyph on the page's cover colour.
const ICONS: [(&str, &str); 8] = [("\u{25c6}", "Diamond"), ("\u{25cf}", "Dot"), ("\u{25b2}", "Peak"), ("\u{25a0}", "Square"),
                                  ("\u{2605}", "Star"), ("\u{2731}", "Spark"), ("\u{2630}", "Lines"), ("\u{270e}", "Pencil")];

/// Where a dragged page lands, by the row it is over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Drop {
    Before(usize),
    Into(usize),
    After(usize),
}

#[derive(Clone, PartialEq, Debug)]
struct Space {
    pages: Vec<Page>,
    roots: Vec<usize>,
}

impl Space {
    fn blocks(&self, p: usize) -> &[Block] {
        match &self.pages[p].body {
            Body::Blocks(b) => b,
            Body::Db(_) => &[],
        }
    }

    fn blocks_mut(&mut self, p: usize) -> Option<&mut Vec<Block>> {
        match &mut self.pages[p].body {
            Body::Blocks(b) => Some(b),
            Body::Db(_) => None,
        }
    }

    fn db_mut(&mut self, p: usize) -> Option<&mut Db> {
        match &mut self.pages[p].body {
            Body::Db(d) => Some(d),
            Body::Blocks(_) => None,
        }
    }

    /// `p` is `of` or under it.
    fn within(&self, mut p: usize, of: usize) -> bool {
        loop {
            if p == of {
                return true;
            }
            match self.pages[p].parent {
                Some(q) => p = q,
                None => return false,
            }
        }
    }

    fn siblings_mut(&mut self, parent: Option<usize>) -> &mut Vec<usize> {
        match parent {
            Some(q) => &mut self.pages[q].children,
            None => &mut self.roots,
        }
    }

    /// Moves `p` to where `to` says (never into itself).
    fn move_page(&mut self, p: usize, to: Drop) {
        let (Drop::Before(t) | Drop::Into(t) | Drop::After(t)) = to;
        if self.within(t, p) {
            return;
        }
        let old = self.pages[p].parent;
        self.siblings_mut(old).retain(|&c| c != p);
        let (parent, at) = match to {
            Drop::Into(t) => (Some(t), self.pages[t].children.len()),
            Drop::Before(t) | Drop::After(t) => {
                let parent = self.pages[t].parent;
                let k = self.siblings_mut(parent).iter().position(|&c| c == t).unwrap_or(0);
                (parent, k + usize::from(matches!(to, Drop::After(_))))
            }
        };
        self.siblings_mut(parent).insert(at, p);
        self.pages[p].parent = parent;
    }

    fn add_page(&mut self, parent: Option<usize>) -> usize {
        let id = self.pages.len();
        self.pages.push(Page { title: String::new(), icon: id % ICONS.len(), cover: 0x5a_7d_b8, parent, children: Vec::new(),
                               fav: false, body: Body::Blocks(vec![blk(Kind::Text, "")]) });
        self.siblings_mut(parent).push(id);
        id
    }

    /// The pages from the root to `p`.
    fn trail(&self, p: usize) -> Vec<usize> {
        let mut t = vec![p];
        while let Some(q) = self.pages[*t.last().unwrap_or(&p)].parent {
            t.push(q);
        }
        t.reverse();
        t
    }
}

/// `needle` is in `hay`, ignoring ASCII case (no allocation).
fn has(hay: &str, needle: &str) -> bool {
    needle.is_empty() || hay.as_bytes().windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

fn words(blocks: &[Block]) -> usize {
    blocks.iter().map(|b| b.text.split_whitespace().count()).sum()
}

fn sample() -> Space {
    use Kind::{Bullet, Callout, Code, Divider, Image, Number, Quote, Text, Todo, Toggle, H1, H2, H3};
    let ind = |mut b: Block| {
        b.indent = 1;
        b
    };
    let welcome = vec![
        blk(H1, "Welcome to the co-op wiki"),
        blk(Text, "Everything the allotment co-op knows lives here: plans, notes and the task board. Each control is a Blender widget or a block the page draws."),
        blk(Callout, "Type / on an empty line for the block menu, or start a line with # or - or [] or >."),
        blk(H2, "This week"),
        blk(Todo { done: true }, "Turn the compost bays"),
        blk(Todo { done: false }, "Order glazing clips for the greenhouse"),
        blk(Todo { done: false }, "Label the bean seed packets"),
        blk(Toggle { open: true }, "Opening hours"),
        ind(blk(Bullet, "Weekdays: 7 \u{2013} 10 and 16 \u{2013} 20")),
        ind(blk(Bullet, "Weekends: dawn to dusk")),
        blk(H2, "House rules"),
        blk(Number, "Close the gate behind you"),
        blk(Number, "Water before ten in summer"),
        blk(Number, "Return tools clean and dry"),
        blk(Quote, "A garden is a friend you can visit any time."),
        blk(Divider, ""),
        blk(Code, "rota = members[week % members.len()]"),
        blk(Image, "Plot map, spring layout"),
    ];
    let seeds = vec![
        blk(H2, "Shelves"),
        blk(Bullet, "Beans and peas: top shelf, by sowing month"),
        blk(Bullet, "Brassicas: the tin with the blue lid"),
        blk(Bullet, "Saved tomato seed: paper envelopes, dated"),
        blk(H3, "Borrowing"),
        blk(Text, "Take a packet, write it in the book, bring back twice as much at the end of the season."),
    ];
    let week = vec![
        blk(Text, "Twelve members at the Saturday work party. The greenhouse frame is stripped and the glazing is on order."),
        blk(Todo { done: false }, "Ask the school about the spare water butts"),
        blk(Todo { done: true }, "Pay the skip deposit"),
        blk(Quote, "Next meeting: first Sunday of the month, in the shed."),
    ];
    let recipes = vec![
        blk(H2, "Glut chutney"),
        blk(Number, "Chop two kilos of whatever is ripest"),
        blk(Number, "Simmer with vinegar, sugar and spice for two hours"),
        blk(Number, "Jar while hot, label, share at the gate"),
    ];
    let tags = vec![Tag { name: "Build", color: 0xe5_48_4d }, Tag { name: "Plants", color: 0x46_a7_58 },
                    Tag { name: "Admin", color: 0x3e_63_dd }, Tag { name: "Water", color: 0x0b_9e_d6 }];
    let task = |name: &str, status, estimate, tag| Task { name: name.to_owned(), status, estimate, tag };
    let db = Db {
        tasks: vec![
            task("Strip broken glazing", Status::Done, 6.0, 0),
            task("Treat the frame", Status::Doing, 4.0, 0),
            task("Order clips and putty", Status::Todo, 1.0, 2),
            task("Fit new panes", Status::Todo, 8.0, 0),
            task("Plumb the drip line", Status::Todo, 3.0, 3),
            task("Move seedlings back in", Status::Todo, 2.0, 1),
            task("Book the skip", Status::Done, 0.5, 2),
        ],
        tags,
        view: View::Table,
        sort: Sort::Manual,
        filter: None,
    };
    let page = |title: &str, icon, cover, parent, children: Vec<usize>, fav, body| Page {
        title: title.to_owned(), icon, cover, parent, children, fav, body,
    };
    Space {
        pages: vec![
            page("Welcome", 0, 0x4772b3, None, vec![], true, Body::Blocks(welcome)),
            page("Projects", 3, 0x8a5cc2, None, vec![2, 3], false, Body::Blocks(vec![blk(Text, "The co-op's projects, one page each.")])),
            page("Greenhouse rebuild", 2, 0x46a758, Some(1), vec![], true, Body::Db(db)),
            page("Seed library", 1, 0xc28a2e, Some(1), vec![], false, Body::Blocks(seeds)),
            page("Meeting notes", 6, 0x6b7280, None, vec![5], false, Body::Blocks(vec![blk(Text, "One page a week.")])),
            page("Week 47", 7, 0x6b7280, Some(4), vec![], false, Body::Blocks(week)),
            page("Recipes", 5, 0xd0643b, None, vec![], false, Body::Blocks(recipes)),
        ],
        roots: vec![0, 1, 4, 6],
    }
}

// ==========================================================================
// The app

/// What the keys type into: a page's title or one of its blocks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum At {
    Title,
    Block(usize),
}

/// A selection in a page editor (`slot` 0 the main one, 1 the side one).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Sel {
    slot: usize,
    at: At,
}

/// A drag in flight: `live` once the pointer has moved far enough to be one.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag {
    Page { page: usize, y: f32, live: bool, over: Option<Drop> },
    Block { slot: usize, from: usize, to: usize },
    Card { slot: usize, task: usize, x: f32, live: bool, over: Option<Status> },
}

#[derive(Clone)]
struct State {
    space: Arc<Space>,
    open: [usize; 2],
    sel: Option<Sel>,
    /// The highlighted slash-menu item.
    menu: usize,
    query: String,
    props: bool,
    /// A block open in a Blender text field.
    field: Option<(usize, usize)>,
    collapsed: Vec<usize>,
    drag: Option<Drag>,
    status: String,
}

struct Notes {
    s: State,
    undo: Vec<Arc<Space>>,
    redo: Vec<Arc<Space>>,
    /// Typing that one undo step covers.
    typing: bool,
    frames: u32,
    /// Where the pointer is during a drag.
    pointer: (f32, f32),
}

impl Notes {
    fn new() -> Self {
        Self {
            s: State { space: Arc::new(sample()), open: [0, 5], sel: None, menu: 0, query: String::new(), props: true, field: None,
                       collapsed: Vec::new(), drag: None, status: String::new() },
            undo: Vec::new(),
            redo: Vec::new(),
            typing: false,
            frames: 0,
            pointer: (0.0, 0.0),
        }
    }

    fn space(&self) -> &Space {
        &self.s.space
    }

    /// The workspace to change, one undo step before it (typing coalesces into one).
    fn edit(&mut self, typing: bool) -> &mut Space {
        if !(typing && self.typing) {
            self.undo.push(self.s.space.clone());
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.typing = typing;
        self.redo.clear();
        Arc::make_mut(&mut self.s.space)
    }

    fn undo(&mut self, back: bool) {
        let (from, to) = if back { (&mut self.undo, &mut self.redo) } else { (&mut self.redo, &mut self.undo) };
        if let Some(s) = from.pop() {
            to.push(std::mem::replace(&mut self.s.space, s));
            self.typing = false;
            self.s.field = None;
            // a selection past the end of a page that got shorter
            if let Some(Sel { slot, at: At::Block(i) }) = self.s.sel {
                let n = self.space().blocks(self.s.open[slot]).len();
                if i >= n {
                    self.s.sel = n.checked_sub(1).map(|i| Sel { slot, at: At::Block(i) });
                }
            }
            self.s.status = if back { "Undone".into() } else { "Redone".into() };
        }
    }

    const fn open(&mut self, slot: usize, page: usize) {
        self.s.open[slot] = page;
        self.s.sel = None;
        self.s.field = None;
        self.typing = false;
    }

    const fn select(&mut self, slot: usize, at: At) {
        self.s.sel = Some(Sel { slot, at });
        self.s.menu = 0;
        self.typing = false;
    }

    /// The selected block's page and index.
    const fn current(&self) -> Option<(usize, usize, usize)> {
        match self.s.sel {
            Some(Sel { slot, at: At::Block(i) }) => Some((slot, self.s.open[slot], i)),
            _ => None,
        }
    }

    fn block(&self) -> Option<&Block> {
        self.current().and_then(|(_, p, i)| self.space().blocks(p).get(i))
    }

    fn block_mut(&mut self, typing: bool) -> Option<&mut Block> {
        let (_, p, i) = self.current()?;
        self.edit(typing).blocks_mut(p)?.get_mut(i)
    }

    fn insert(&mut self, slot: usize, at: usize, b: Block) {
        let p = self.s.open[slot];
        if let Some(bs) = self.edit(false).blocks_mut(p) {
            bs.insert(at.min(bs.len()), b);
            self.select(slot, At::Block(at));
        }
    }

    /// The slash menu's items for the selected block, when it is open.
    fn menu(&self) -> Option<Vec<usize>> {
        let q = self.block()?.text.strip_prefix('/')?;
        Some((0..KINDS.len()).filter(|&k| has(KINDS[k].1, q)).collect())
    }

    fn turn_into(&mut self, kind: Kind) {
        if let Some(b) = self.block_mut(false) {
            b.kind = kind;
            b.text.clear();
        }
        self.s.status = format!("Turned into {}", KINDS.iter().find(|k| std::mem::discriminant(&k.0) == std::mem::discriminant(&kind)).map_or("", |k| k.1));
    }

    fn step(&mut self, down: bool) {
        let Some(Sel { slot, at }) = self.s.sel else { return };
        let blocks = self.space().blocks(self.s.open[slot]);
        let hide = hidden(blocks);
        let to = match (at, down) {
            (At::Title, true) => (0..blocks.len()).find(|&i| !hide[i]).map(At::Block),
            (At::Title, false) => None,
            (At::Block(i), true) => (i + 1..blocks.len()).find(|&j| !hide[j]).map(At::Block),
            (At::Block(i), false) => Some((0..i).rev().find(|&j| !hide[j]).map_or(At::Title, At::Block)),
        };
        if let Some(at) = to {
            self.select(slot, at);
        }
    }

    fn move_block(&mut self, slot: usize, from: usize, to: usize) {
        let p = self.s.open[slot];
        let n = self.space().blocks(p).len();
        if from >= n || to > n || to == from || to == from + 1 {
            return;
        }
        if let Some(bs) = self.edit(false).blocks_mut(p) {
            let b = bs.remove(from);
            let at = if to > from { to - 1 } else { to };
            bs.insert(at, b);
            self.select(slot, At::Block(at));
            self.s.status = "Moved block".into();
        }
    }

    fn enter(&mut self, ctrl: bool) {
        let Some((slot, _, i)) = self.current() else { return };
        let Some(b) = self.block().cloned() else { return };
        if let Some(items) = self.menu() {
            if let Some(&k) = items.get(self.s.menu.min(items.len().saturating_sub(1))) {
                self.turn_into(KINDS[k].0);
            }
            return;
        }
        match b.kind {
            Kind::Todo { done } if ctrl => {
                if let Some(t) = self.block_mut(false) {
                    t.kind = Kind::Todo { done: !done };
                }
            }
            k if b.text.is_empty() && k.listed() => {
                if let Some(t) = self.block_mut(false) {
                    t.kind = Kind::Text;
                }
            }
            Kind::Toggle { open: true } => self.insert(slot, i + 1, Block { kind: Kind::Text, text: String::new(), indent: b.indent + 1 }),
            k => self.insert(slot, i + 1, Block { kind: k.next(), text: String::new(), indent: b.indent }),
        }
    }

    fn backspace(&mut self) -> bool {
        let Some((slot, p, i)) = self.current() else { return false };
        let Some(b) = self.block() else { return false };
        if !b.text.is_empty() {
            return false;
        }
        let (kind, indent) = (b.kind, b.indent);
        if kind != Kind::Text && kind != Kind::Divider {
            if let Some(t) = self.block_mut(false) {
                t.kind = Kind::Text;
            }
        } else if indent > 0 {
            if let Some(t) = self.block_mut(false) {
                t.indent -= 1;
            }
        } else if let Some(bs) = self.edit(false).blocks_mut(p) {
            bs.remove(i);
            let prev = (0..i).rev().find(|&j| !hidden(self.space().blocks(p))[j]);
            self.select(slot, prev.map_or(At::Title, At::Block));
        }
        true
    }

    fn indent(&mut self, out: bool) {
        let Some((_, p, i)) = self.current() else { return };
        let most = if i == 0 { 0 } else { self.space().blocks(p)[i - 1].indent + 1 };
        if let Some(b) = self.block_mut(false) {
            b.indent = if out { b.indent.saturating_sub(1) } else { (b.indent + 1).min(most).min(4) };
        }
    }

    fn type_key(&mut self, k: &Keystroke) -> bool {
        let Some(Sel { slot, at }) = self.s.sel else { return false };
        let p = self.s.open[slot];
        let typable = k.key == "BackSpace" || k.key_char.as_ref().is_some_and(|c| !k.modifiers.control && !c.chars().any(char::is_control));
        if !typable {
            return false;
        }
        match at {
            At::Title => {
                let _ = edit(&mut self.edit(true).pages[p].title, k, 120);
            }
            At::Block(_) => {
                if self.block().is_some_and(|b| !b.kind.texty()) {
                    return false;
                }
                let Some(b) = self.block_mut(true) else { return false };
                if edit(&mut b.text, k, 4000) == Edit::Typed && b.kind == Kind::Text && let Some(kind) = shortcut(&b.text) {
                    b.kind = kind;
                    b.text.clear();
                    self.typing = false;
                    if kind == Kind::Divider && let Some((slot, _, i)) = self.current() {
                        self.insert(slot, i + 1, blk(Kind::Text, ""));
                    }
                }
                self.s.menu = 0;
            }
        }
        true
    }

    fn key(&mut self, e: &KeyDownEvent) -> bool {
        let k = &e.keystroke;
        let (ctrl, shift, alt) = (k.modifiers.control, k.modifiers.shift, k.modifiers.alt);
        let key = k.key.as_str();
        if ctrl && key.eq_ignore_ascii_case("z") {
            self.undo(!shift);
            return true;
        }
        if ctrl && key.eq_ignore_ascii_case("y") {
            self.undo(false);
            return true;
        }
        let Some(Sel { slot, at }) = self.s.sel else { return false };
        if let (At::Title, "Return" | "KP_Enter" | "Down") = (at, key) {
            if self.space().blocks(self.s.open[slot]).is_empty() {
                self.insert(slot, 0, blk(Kind::Text, ""));
            } else {
                self.step(true);
            }
            return true;
        }
        let menu = self.menu().map(|m| m.len());
        match key {
            "Up" | "Down" if menu.is_some_and(|n| n > 0) => {
                let n = menu.unwrap_or(1);
                self.s.menu = if key == "Down" { (self.s.menu + 1) % n } else { (self.s.menu + n - 1) % n };
            }
            "Up" | "Down" if alt => {
                if let Some((slot, _, i)) = self.current() {
                    if key == "Up" && i > 0 {
                        self.move_block(slot, i, i - 1);
                    } else if key == "Down" {
                        self.move_block(slot, i, i + 2);
                    }
                }
            }
            "Up" | "Down" => self.step(key == "Down"),
            "Return" | "KP_Enter" => self.enter(ctrl),
            "Tab" => self.indent(shift),
            "Escape" => {
                if menu.is_some() {
                    if let Some(b) = self.block_mut(false) {
                        b.text.clear();
                    }
                } else {
                    self.s.sel = None;
                }
            }
            "BackSpace" if self.backspace() => {}
            "Delete" if self.block().is_some_and(|b| !b.kind.texty()) => {
                let _ = self.backspace();
            }
            _ => return self.type_key(k),
        }
        if key != "Return" {
            self.typing = false;
        }
        true
    }

    /// What a drag is over, from the boxes of the last frame (called as the page draws).
    fn drag_to(&mut self, w: &Window, (x, y): (f32, f32)) {
        let Some(d) = self.s.drag else { return };
        self.s.drag = Some(match d {
            Drag::Page { page, y: y0, live, .. } => {
                let live = live || (y - y0).abs() > 4.0;
                let over = live.then(|| self.page_drop(w, page, y)).flatten();
                Drag::Page { page, y: y0, live, over }
            }
            Drag::Block { slot, from, .. } => {
                let n = self.space().blocks(self.s.open[slot]).len();
                let to = (0..n).filter(|&i| w.bounds_of(&format!("b{slot}_{i}")).is_some_and(|b| b.origin.y + b.size.height / 2.0 < y))
                    .map(|i| i + 1).max().unwrap_or(0);
                Drag::Block { slot, from, to }
            }
            Drag::Card { slot, task, x: x0, live, .. } => {
                let live = live || (x - x0).abs() > 4.0;
                let over = STATUSES.iter().enumerate().find(|(k, _)| w.bounds_of(&format!("col{slot}_{k}"))
                    .is_some_and(|b| x >= b.origin.x && x < b.origin.x + b.size.width)).map(|(_, s)| *s);
                Drag::Card { slot, task, x: x0, live, over: over.filter(|_| live) }
            }
        });
    }

    fn page_drop(&self, w: &Window, dragged: usize, y: f32) -> Option<Drop> {
        (0..self.space().pages.len()).filter(|&p| !self.space().within(p, dragged)).find_map(|p| {
            let b = w.bounds_of(&format!("t{p}"))?;
            let f = (y - b.origin.y) / b.size.height;
            (0.0..1.0).contains(&f).then_some(if f < 0.3 { Drop::Before(p) } else if f > 0.7 { Drop::After(p) } else { Drop::Into(p) })
        })
    }

    fn drop(&mut self, ctrl: bool) {
        match self.s.drag.take() {
            Some(Drag::Page { page, live: false, .. }) => {
                self.open(usize::from(ctrl), page);
                self.s.status = if ctrl { "Opened beside".into() } else { String::new() };
            }
            Some(Drag::Page { page, over: Some(to), .. }) => {
                self.edit(false).move_page(page, to);
                if let Drop::Into(t) = to {
                    self.s.collapsed.retain(|&c| c != t);
                }
                self.s.status = format!("Moved \u{201c}{}\u{201d}", self.space().pages[page].title);
            }
            Some(Drag::Block { slot, from, to }) => self.move_block(slot, from, to),
            Some(Drag::Card { slot, task, over: Some(status), .. }) => {
                let p = self.s.open[slot];
                if let Some(db) = self.edit(false).db_mut(p) {
                    db.tasks[task].status = status;
                }
                self.s.status = format!("Moved to {}", status.name());
            }
            _ => {}
        }
    }

    fn new_page(&mut self, parent: Option<usize>) {
        let p = self.edit(false).add_page(parent);
        if let Some(q) = parent {
            self.s.collapsed.retain(|&c| c != q);
        }
        self.open(0, p);
        self.select(0, At::Title);
        self.s.status = "New page: type its title".into();
    }

    fn with_db(&mut self, slot: usize, f: impl FnOnce(&mut Db)) {
        let p = self.s.open[slot];
        if let Some(db) = self.edit(false).db_mut(p) {
            f(db);
        }
    }
}

// ==========================================================================
// Drawing

const CSS: &str = r#"
html, body { height: 100%; }
body { -webkit-font-smoothing: antialiased; user-select: none; }
.screen { width: 100%; height: 100%; }
bui-region { background: #303030; }
bui-region[region="header"] { background: #2a2a2a; border-bottom: 1px solid #1c1c1c; }
bui-region[region="footer"] { background: #262626; border-top: 1px solid #1c1c1c; }
bui-region[region="sidebar"] { background: #2b2b2b; border-left: 1px solid #1c1c1c; }

.bar { display: flex; align-items: center; gap: 4px; height: 100%; padding: 0 8px; font-size: 12px; color: #b4b4b4; }
.bar .spacer { flex: 1; }
.crumb { padding: 2px 6px; border-radius: 4px; cursor: pointer; white-space: nowrap; transition-property: background-color, color;
         transition-duration: 120ms; }
.crumb:hover { background: #3a3a3a; color: #fff; }
.sep { color: #6a6a6a; }
.tbtn { height: 20px; padding: 0 8px; display: flex; align-items: center; justify-content: center; border-radius: 4px;
        background: #545454; color: #e6e6e6; font-size: 12px; cursor: pointer; box-shadow: 0 1px 1px rgba(0,0,0,0.35);
        transition-property: background-color, transform; transition-duration: 100ms; white-space: nowrap; }
.tbtn:hover { background: #656565; }
.tbtn:active { transform: scale(0.96); }
.tbtn.on { background: #4772b3; color: #fff; }
.foot { display: flex; align-items: center; gap: 14px; height: 100%; padding: 0 10px; font-size: 11px; color: #a0a0a0;
        font-variant-numeric: tabular-nums; }
.foot .grow { flex: 1; }

.pane { height: 100%; overflow-y: auto; background: #262626; color: #e2e2e2; }
.cover { height: 64px; opacity: 0.85; }
.sheet { max-width: 720px; margin: 0 auto; padding: 0 48px 120px; }
.icon { width: 1.6em; height: 1.6em; border-radius: 0.3em; display: inline-flex; align-items: center; justify-content: center;
        color: #fff; flex: 0 0 auto; box-shadow: inset 0 0 0 1px rgba(255,255,255,0.12); }
.big-icon { font-size: 34px; margin-top: -28px; box-shadow: 0 4px 12px rgba(0,0,0,0.45), inset 0 0 0 1px rgba(255,255,255,0.12); }
.title { font-size: 30px; font-weight: 700; letter-spacing: -0.02em; padding: 10px 0 4px; cursor: text; text-wrap: balance;
         border-radius: 4px; }
.meta { font-size: 11px; color: #8a8a8a; margin-bottom: 14px; font-variant-numeric: tabular-nums; }
.empty { color: #6c6c6c; }
.caret { display: inline-block; width: 2px; height: 1.1em; vertical-align: text-bottom; margin-left: 1px; background: #5b8ff0; }

.blk { display: flex; align-items: flex-start; gap: 4px; border-radius: 4px; padding: 2px 4px 2px 0; position: relative;
       cursor: text; transition-property: background-color; transition-duration: 120ms; font-size: 15px; line-height: 1.5; }
.blk:hover { background: #2c2c2c; }
.blk.sel { background: #2e3442; }
.blk.ghost { opacity: 0.4; }
.blk.drop { box-shadow: inset 0 2px 0 #5b8ff0; }
.tail.drop { box-shadow: inset 0 2px 0 #5b8ff0; }
.grip { width: 18px; flex: 0 0 18px; text-align: center; color: transparent; cursor: grab; border-radius: 3px; font-size: 12px;
        line-height: 22px; transition-property: color, background-color; transition-duration: 120ms; }
.blk:hover .grip, .blk.sel .grip { color: #7a7a7a; }
.grip:hover { background: #3a3a3a; color: #d0d0d0; }
.body { flex: 1; min-width: 0; display: flex; align-items: flex-start; gap: 6px; }
.txt { flex: 1; white-space: pre-wrap; text-wrap: pretty; min-height: 1.5em; }
.mark { flex: 0 0 auto; min-width: 1.2em; color: #9a9a9a; font-variant-numeric: tabular-nums; text-align: right; }
.h1 .txt { font-size: 24px; font-weight: 700; padding-top: 14px; text-wrap: balance; }
.h2 .txt { font-size: 19px; font-weight: 650; padding-top: 10px; text-wrap: balance; }
.h3 .txt { font-size: 16px; font-weight: 600; padding-top: 6px; }
.done .txt { color: #7c7c7c; text-decoration: line-through; }
.quote .txt { border-left: 3px solid #d0d0d0; padding-left: 12px; font-style: italic; color: #cfcfcf; }
.callout .body { background: #343434; border-radius: 8px; padding: 10px 12px; box-shadow: 0 1px 2px rgba(0,0,0,0.3); }
.callout .icon { background: #c28a2e; font-weight: 700; font-size: 12px; }
.code .txt { font-family: Consolas; font-size: 13px; background: #1d1d1d; border-radius: 6px; padding: 10px 12px; color: #9fd3a8; }
.rule { flex: 1; height: 1px; background: #4a4a4a; margin: 11px 0; }
.img { flex: 1; display: flex; flex-direction: column; gap: 6px; }
.img .frame { height: 150px; border-radius: 8px; background: linear-gradient(135deg, #3a4a3a, #2f3a4c);
              box-shadow: inset 0 0 0 1px rgba(255,255,255,0.1); display: flex; align-items: center; justify-content: center;
              color: #9aa7b8; font-size: 12px; }
.img .txt { font-size: 12px; color: #9a9a9a; }
.arrow { width: 1.2em; flex: 0 0 auto; text-align: center; color: #b0b0b0; cursor: pointer; border-radius: 3px;
         transition-property: transform, background-color; transition-duration: 160ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.arrow:hover { background: #3a3a3a; }
.arrow.open { transform: rotate(90deg); }
.tail { height: 40px; cursor: text; border-radius: 4px; color: transparent; font-size: 12px; padding: 10px 22px;
        transition-property: color; transition-duration: 120ms; }
.tail:hover { color: #6c6c6c; }

.menu { margin: 2px 0 6px 22px; width: 300px; background: #1f1f1f; border-radius: 8px; padding: 4px;
        box-shadow: 0 8px 24px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.08); font-size: 12px; }
.menu .cap { color: #8a8a8a; padding: 4px 8px; font-size: 11px; }
.menu .item { display: flex; flex-direction: column; padding: 5px 8px; border-radius: 4px; cursor: pointer;
              transition-property: background-color; transition-duration: 100ms; }
.menu .item:hover { background: #2c2c2c; }
.menu .item.on { background: #4772b3; }
.menu .hint { color: #9a9a9a; font-size: 11px; }
.menu .item.on .hint { color: #dce6f7; }

.dbbar { display: flex; align-items: center; gap: 4px; margin-bottom: 10px; font-size: 12px; }
.dbbar .spacer { flex: 1; }
.dbbar .gap { width: 10px; }
table.db { width: 100%; border-collapse: collapse; font-size: 12px; table-layout: fixed; }
.db th { text-align: left; color: #9a9a9a; font-weight: 500; padding: 6px 4px; border-bottom: 1px solid #3c3c3c; cursor: pointer;
         white-space: nowrap; transition-property: color; transition-duration: 120ms; }
.db th:hover { color: #fff; }
.db th.on { color: #8fb3f0; }
.db td { padding: 3px 4px; border-bottom: 1px solid #333; }
.db .est { width: 64px; }
.db .st { width: 84px; }
.db .tg { width: 104px; }
.chip { display: inline-flex; align-items: center; gap: 5px; padding: 1px 7px; border-radius: 9px; font-size: 11px; white-space: nowrap; }
.dot { width: 8px; height: 8px; border-radius: 4px; flex: 0 0 8px; }
.tagcell { display: flex; align-items: center; gap: 6px; }
.s-todo { background: #3c3c3c; color: #cfcfcf; }
.s-doing { background: #24406e; color: #cfe0ff; }
.s-done { background: #23502c; color: #c9f0d1; }
.sum { color: #8a8a8a; font-size: 11px; padding: 8px 4px; font-variant-numeric: tabular-nums; }
.board { display: flex; gap: 10px; align-items: flex-start; }
.col { flex: 1; min-width: 0; background: #2b2b2b; border-radius: 10px; padding: 6px; display: flex; flex-direction: column; gap: 6px;
       transition-property: background-color, box-shadow; transition-duration: 120ms; }
.col.over { background: #2e3442; box-shadow: inset 0 0 0 1px #5b8ff0; }
.col .head { display: flex; align-items: center; gap: 6px; font-size: 12px; padding: 2px 4px; color: #cfcfcf;
             font-variant-numeric: tabular-nums; }
.col .head .n { color: #8a8a8a; margin-left: auto; white-space: nowrap; }
.card { background: #383838; border-radius: 4px; padding: 8px; display: flex; flex-direction: column; gap: 6px; cursor: grab;
        box-shadow: 0 1px 2px rgba(0,0,0,0.4), inset 0 0 0 1px rgba(255,255,255,0.05); font-size: 12px;
        transition-property: background-color, transform; transition-duration: 120ms; }
.card:hover { background: #404040; }
.card.ghost { opacity: 0.45; transform: scale(0.96); }
.card .row { display: flex; align-items: center; gap: 6px; color: #9a9a9a; font-variant-numeric: tabular-nums; }

.side { padding: 10px; display: flex; flex-direction: column; gap: 8px; font-size: 12px; }
.side h4 { margin: 6px 0 0; font-size: 12px; color: #dcdcdc; }
.side .line { display: flex; gap: 6px; align-items: center; }
.side .line .k { width: 64px; color: #9a9a9a; }
.side .stat { color: #a8a8a8; font-variant-numeric: tabular-nums; line-height: 1.6; }
.side .keys { color: #8f8f8f; line-height: 1.7; }

.tree { padding: 6px; display: flex; flex-direction: column; gap: 1px; height: 100%; overflow-y: auto; font-size: 12px; }
.tree .cap { color: #8a8a8a; font-size: 11px; padding: 10px 8px 4px; }
.row { display: flex; align-items: center; gap: 4px; height: 24px; padding-right: 4px; border-radius: 4px; color: #d2d2d2;
       cursor: pointer; transition-property: background-color; transition-duration: 120ms; white-space: nowrap; }
.row:hover { background: #3a3a3a; }
.row.on { background: #4772b3; color: #fff; }
.row.beside { box-shadow: inset 2px 0 0 #c28a2e; }
.row.ghost { opacity: 0.4; }
.row.before { box-shadow: inset 0 2px 0 #5b8ff0; }
.row.after { box-shadow: inset 0 -2px 0 #5b8ff0; }
.row.into { background: #2e3442; box-shadow: inset 0 0 0 1px #5b8ff0; }
.row .icon { font-size: 11px; }
.row .name { flex: 1; overflow: hidden; text-overflow: ellipsis; }
.row .x { width: 16px; text-align: center; color: #8a8a8a; border-radius: 3px; transition-property: transform; transition-duration: 160ms;
          transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.row .x.open { transform: rotate(90deg); }
.row .star { width: 18px; text-align: center; color: transparent; border-radius: 3px; transition-property: color; transition-duration: 120ms; }
.row:hover .star { color: #7a7a7a; }
.row .star.on { color: #f2c14e; }
.result { padding: 6px 8px; border-radius: 4px; cursor: pointer; display: flex; flex-direction: column; gap: 2px;
          transition-property: background-color; transition-duration: 120ms; }
.result:hover { background: #3a3a3a; }
.result .snip { color: #8f8f8f; font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.searchbar { display: flex; align-items: center; height: 100%; padding: 0 6px; }
.new { margin-top: 10px; }
"#;

/// A page's icon tile.
fn icon(p: &Page) -> Div {
    div().class("icon").css("background", hex(p.cover)).child(ICONS[p.icon % ICONS.len()].0)
}

fn title_of(p: &Page) -> &str {
    if p.title.is_empty() { "Untitled" } else { &p.title }
}

/// Text with the caret at its end when it is what keys type into, else its placeholder.
fn typed(text: &str, caret: bool, placeholder: &'static str) -> Div {
    let d = div().class("txt");
    match (text.is_empty(), caret) {
        (true, true) => d.child(span().class("caret")).child(span().class("empty").child(placeholder)),
        (true, false) => d,
        (false, true) => d.child(text.to_owned()).child(span().class("caret")),
        (false, false) => d.child(text.to_owned()),
    }
}

/// A toggle drawn like a Blender button: blue when on (a Blender `Button` shows no state).
fn tog(id: String, label: &'static str, on: bool, weak: WeakEntity<Notes>, f: impl Fn(&mut Notes) + 'static) -> Div {
    div().id(id).class(if on { "tbtn on" } else { "tbtn" }).child(label)
        .on_click(weak.listener(move |this: &mut Notes, _: &ClickEvent, w, cx| {
            f(this);
            w.focus_id("root");
            cx.notify();
        }))
}

// ---- the Pages editor

fn tree(st: &State, weak: WeakEntity<Notes>) -> Div {
    let sp = &st.space;
    let mut d = div().class("tree");
    if !st.query.is_empty() {
        d = d.child(div().class("cap").child("Results"));
        let mut n = 0;
        for (p, page) in sp.pages.iter().enumerate() {
            let hit = sp.blocks(p).iter().position(|b| has(&b.text, &st.query));
            if !has(&page.title, &st.query) && hit.is_none() {
                continue;
            }
            n += 1;
            let snip = hit.map_or_else(|| "Title".to_owned(), |i| sp.blocks(p)[i].text.clone());
            d = d.child(div().id(format!("r{p}")).class("result")
                .on_click(weak.listener(move |this: &mut Notes, _: &ClickEvent, w, cx| {
                    this.open(0, p);
                    if let Some(i) = hit {
                        this.select(0, At::Block(i));
                    }
                    w.focus_id("root");
                    cx.notify();
                }))
                .child(div().class("line").css("display", "flex").css("gap", "6px").child(icon(page)).child(title_of(page).to_owned()))
                .child(div().class("snip").child(snip)));
        }
        if n == 0 {
            d = d.child(div().class("cap").child("Nothing matches"));
        }
        return d;
    }
    d = d.child(div().class("cap").child("Favourites"));
    for (p, page) in sp.pages.iter().enumerate().filter(|(_, p)| p.fav) {
        d = d.child(div().id(format!("f{p}")).class(if st.open[0] == p { "row on" } else { "row" })
            .on_click(weak.listener(move |this: &mut Notes, e: &ClickEvent, w, cx| {
                this.open(usize::from(e.down.modifiers.control), p);
                w.focus_id("root");
                cx.notify();
            }))
            .child(div().class("x")).child(icon(page)).child(div().class("name").child(title_of(page).to_owned())));
    }
    d = d.child(div().class("cap").child("Pages"));
    let mut stack: Vec<(usize, usize)> = sp.roots.iter().rev().map(|&p| (p, 0)).collect();
    while let Some((p, depth)) = stack.pop() {
        let open = !st.collapsed.contains(&p);
        if open {
            stack.extend(sp.pages[p].children.iter().rev().map(|&c| (c, depth + 1)));
        }
        d = d.child(tree_row(st, p, depth, open, weak));
    }
    d.child(div().class("new").child(Button().id("new-page").child("+ New page")
        .on_click(weak.listener(|this: &mut Notes, _: &ClickEvent, w, cx| {
            this.new_page(None);
            w.focus_id("root");
            cx.notify();
        }))))
}

fn tree_row(st: &State, p: usize, depth: usize, open: bool, weak: WeakEntity<Notes>) -> Div {
    let page = &st.space.pages[p];
    let mut class = String::from("row");
    if st.open[0] == p {
        class.push_str(" on");
    } else if st.open[1] == p {
        class.push_str(" beside");
    }
    if let Some(Drag::Page { page: q, live: true, over, .. }) = st.drag {
        if q == p {
            class.push_str(" ghost");
        }
        match over {
            Some(Drop::Before(t)) if t == p => class.push_str(" before"),
            Some(Drop::After(t)) if t == p => class.push_str(" after"),
            Some(Drop::Into(t)) if t == p => class.push_str(" into"),
            _ => {}
        }
    }
    let arrow = if page.children.is_empty() { div().class("x") } else {
        div().id(format!("x{p}")).class(if open { "x open" } else { "x" }).child("\u{25b8}")
            .on_pointer_down(weak.listener(move |this: &mut Notes, _: &PointerEvent, _, cx| {
                if let Some(k) = this.s.collapsed.iter().position(|&c| c == p) {
                    this.s.collapsed.remove(k);
                } else {
                    this.s.collapsed.push(p);
                }
                cx.stop_propagation();
                cx.notify();
            }))
    };
    div().id(format!("t{p}")).class(class).css("padding-left", format!("{}px", 2 + depth * 14))
        .on_pointer_down(weak.listener(move |this: &mut Notes, e: &PointerEvent, w, cx| {
            this.s.drag = Some(Drag::Page { page: p, y: e.position.y.0, live: false, over: None });
            w.set_pointer_capture(cx);
            cx.notify();
        }))
        .child(arrow)
        .child(icon(page))
        .child(div().class("name").child(title_of(page).to_owned()))
        .child(div().id(format!("s{p}")).class(if page.fav { "star on" } else { "star" }).child("\u{2605}")
            .on_pointer_down(weak.listener(move |this: &mut Notes, _: &PointerEvent, _, cx| {
                let fav = &mut this.edit(false).pages[p].fav;
                *fav = !*fav;
                cx.stop_propagation();
                cx.notify();
            })))
}

// ---- a Page editor

fn page_header(st: &State, slot: usize, weak: WeakEntity<Notes>) -> Div {
    let sp = &st.space;
    let p = st.open[slot];
    let mut d = div().class("bar");
    for (k, q) in sp.trail(p).into_iter().enumerate() {
        if k > 0 {
            d = d.child(div().class("sep").child("/"));
        }
        d = d.child(div().id(format!("crumb{slot}_{q}")).class("crumb").child(title_of(&sp.pages[q]).to_owned())
            .on_click(weak.listener(move |this: &mut Notes, _: &ClickEvent, _, cx| {
                this.open(slot, q);
                cx.notify();
            })));
    }
    let act = |id: String, label: &'static str, f: fn(&mut Notes, usize)| Button().id(id).child(label)
        .on_click(weak.listener(move |this: &mut Notes, _: &ClickEvent, w, cx| {
            f(this, slot);
            w.focus_id("root");
            cx.notify();
        }));
    d = d.child(div().class("spacer"))
        .child(tog(format!("fav{slot}"), "\u{2605} Favourite", sp.pages[p].fav, weak, move |t| {
            let p = t.s.open[slot];
            let fav = &mut t.edit(false).pages[p].fav;
            *fav = !*fav;
        }))
        .child(act(format!("swap{slot}"), "\u{21c4} Swap", |t, _| t.s.open.swap(0, 1)));
    // the side pane keeps to what it needs: the main one holds the rest
    if slot == 1 {
        return d;
    }
    d.child(act(format!("sub{slot}"), "+ Subpage", |t, slot| {
            let p = t.s.open[slot];
            t.new_page(Some(p));
        }))
        .child(tog(format!("props{slot}"), "Properties", st.props, weak, |t| t.s.props = !t.s.props))
        .child(act(format!("undo{slot}"), "Undo", |t, _| t.undo(true)))
        .child(act(format!("redo{slot}"), "Redo", |t, _| t.undo(false)))
}

fn page_footer(st: &State, slot: usize) -> Div {
    let sp = &st.space;
    let p = st.open[slot];
    let blocks = sp.blocks(p);
    let info = match &sp.pages[p].body {
        Body::Blocks(_) => format!("{} blocks \u{00b7} {} words", blocks.len(), words(blocks)),
        Body::Db(db) => format!("{} tasks \u{00b7} {} done", db.tasks.len(), db.tasks.iter().filter(|t| t.status == Status::Done).count()),
    };
    let status = if st.sel.is_some_and(|s| s.slot == slot) || slot == 0 { st.status.clone() } else { String::new() };
    div().class("foot").child(info).child(div().class("grow").child(status))
        .child(if slot == 0 { "Ctrl+click a page to open it beside" } else { "Side page" })
}

fn block_view(st: &State, slot: usize, i: usize, b: &Block, number: usize, weak: WeakEntity<Notes>) -> Div {
    let selected = st.sel == Some(Sel { slot, at: At::Block(i) });
    let mut class = format!("blk {}", b.kind.class());
    if selected {
        class.push_str(" sel");
    }
    if let Some(Drag::Block { slot: s, from, to }) = st.drag {
        if s == slot && from == i {
            class.push_str(" ghost");
        }
        if s == slot && to == i && to != from && to != from + 1 {
            class.push_str(" drop");
        }
    }
    let grip = div().id(format!("g{slot}_{i}")).class("grip").child("\u{2237}")
        .on_pointer_down(weak.listener(move |this: &mut Notes, _: &PointerEvent, w, cx| {
            this.select(slot, At::Block(i));
            this.s.drag = Some(Drag::Block { slot, from: i, to: i });
            w.set_pointer_capture(cx);
            w.focus_id("root");
            cx.stop_propagation();
            cx.notify();
        }));
    let text = if st.field == Some((slot, i)) {
        // live: the page follows each key; Enter or Escape closes it (see the key listener)
        div().class("txt").child(TextField().id(format!("fe{slot}_{i}")).attr("live", "").w(relative(1.0)).value(b.text.clone())
            .on_input(weak.listener(move |this: &mut Notes, e: &ChangeEvent, _, cx| {
                let p = this.s.open[slot];
                if let Some(t) = this.edit(true).blocks_mut(p).and_then(|bs| bs.get_mut(i)) {
                    t.text.clone_from(&e.value);
                }
                cx.notify();
            })))
    } else {
        typed(&b.text, selected, b.kind.placeholder())
    };
    let body = div().class("body");
    let body = match b.kind {
        Kind::Todo { done } => body.child(Checkbox().id(format!("cb{slot}_{i}")).checked(done)
            .on_change(weak.listener(move |this: &mut Notes, e: &ChangeEvent, w, cx| {
                let p = this.s.open[slot];
                if let Some(t) = this.edit(false).blocks_mut(p).and_then(|bs| bs.get_mut(i)) {
                    t.kind = Kind::Todo { done: e.checked };
                }
                w.focus_id("root");
                cx.notify();
            }))).child(text),
        Kind::Toggle { open } => body.child(div().id(format!("tg{slot}_{i}")).class(if open { "arrow open" } else { "arrow" }).child("\u{25b8}")
            .on_mouse_down(MouseButton::Left, weak.listener(move |this: &mut Notes, _: &MouseDownEvent, _, cx| {
                let p = this.s.open[slot];
                if let Some(t) = this.edit(false).blocks_mut(p).and_then(|bs| bs.get_mut(i)) {
                    t.kind = Kind::Toggle { open: !open };
                }
                cx.stop_propagation();
                cx.notify();
            }))).child(text),
        Kind::Bullet => body.child(div().class("mark").child("\u{2022}")).child(text),
        Kind::Number => body.child(div().class("mark").child(format!("{number}."))).child(text),
        Kind::Callout => body.child(div().class("icon").child("!")).child(text),
        Kind::Divider => body.child(div().class("rule")),
        Kind::Image => body.child(div().class("img").child(div().class("frame").child("Image placeholder \u{00b7} 16 : 9")).child(text)),
        _ => body.child(text),
    };
    div().id(format!("b{slot}_{i}")).class(class).css("padding-left", format!("{}px", u32::from(b.indent) * 24))
        .on_mouse_down(MouseButton::Left, weak.listener(move |this: &mut Notes, e: &MouseDownEvent, w, cx| {
            this.select(slot, At::Block(i));
            if e.click_count == 2 && this.block().is_some_and(|b| b.kind.texty()) {
                this.s.field = Some((slot, i));
                this.s.status = "Click the field to edit the text anywhere; Enter keeps it".into();
            } else if this.s.field != Some((slot, i)) {
                this.s.field = None;
                w.focus_id("root");
            }
            cx.notify();
        }))
        .child(grip)
        .child(body)
}

fn slash_menu(st: &State, items: &[usize], weak: WeakEntity<Notes>) -> Div {
    let mut d = div().class("menu").child(div().class("cap").child(if items.is_empty() { "No results" } else { "Basic blocks" }));
    for (n, &k) in items.iter().enumerate() {
        let (kind, name, hint) = KINDS[k];
        d = d.child(div().id(format!("m{k}")).class(if n == st.menu.min(items.len() - 1) { "item on" } else { "item" })
            .on_mouse_down(MouseButton::Left, weak.listener(move |this: &mut Notes, _: &MouseDownEvent, w, cx| {
                this.turn_into(kind);
                w.focus_id("root");
                cx.stop_propagation();
                cx.notify();
            }))
            .child(name).child(div().class("hint").child(hint)));
    }
    d
}

fn blocks_view(st: &State, slot: usize, blocks: &[Block], weak: WeakEntity<Notes>) -> Vec<Div> {
    let hide = hidden(blocks);
    let mut out = Vec::with_capacity(blocks.len() + 2);
    let mut number = 0;
    for (i, b) in blocks.iter().enumerate() {
        number = if b.kind == Kind::Number { number + 1 } else if b.indent == 0 { 0 } else { number };
        if hide[i] {
            continue;
        }
        out.push(block_view(st, slot, i, b, number, weak));
        if st.sel == Some(Sel { slot, at: At::Block(i) }) && let Some(q) = b.text.strip_prefix('/') {
            let items: Vec<usize> = (0..KINDS.len()).filter(|&k| has(KINDS[k].1, q)).collect();
            out.push(slash_menu(st, &items, weak));
        }
    }
    let n = blocks.len();
    let drop = matches!(st.drag, Some(Drag::Block { slot: s, from, to }) if s == slot && to == n && from + 1 != n);
    out.push(div().id(format!("tail{slot}")).class(if drop { "tail drop" } else { "tail" }).child("Click to add a block")
        .on_mouse_down(MouseButton::Left, weak.listener(move |this: &mut Notes, _: &MouseDownEvent, w, cx| {
            let p = this.s.open[slot];
            let n = this.space().blocks(p).len();
            if this.space().blocks(p).last().is_some_and(|b| b.text.is_empty() && b.kind == Kind::Text) {
                this.select(slot, At::Block(n - 1));
            } else {
                this.insert(slot, n, blk(Kind::Text, ""));
            }
            w.focus_id("root");
            cx.notify();
        })));
    out
}

fn db_view(st: &State, slot: usize, db: &Db, weak: WeakEntity<Notes>) -> Div {
    let view = |v: View, label: &'static str| tog(format!("view{slot}_{}", v as u8), label, db.view == v, weak, move |t| t.with_db(slot, |d| d.view = v));
    let mut bar = div().class("dbbar").child(view(View::Table, "Table")).child(view(View::Board, "Board")).child(div().class("gap"));
    let filter = |k: usize, label: &'static str, f: Option<Status>| tog(format!("f{slot}_{k}"), label, db.filter == f, weak, move |t| t.with_db(slot, |d| d.filter = f));
    bar = bar.child(filter(0, "All", None));
    for (k, s) in STATUSES.iter().enumerate() {
        bar = bar.child(filter(k + 1, s.name(), Some(*s)));
    }
    bar = bar.child(div().class("spacer")).child(Button().id(format!("new{slot}")).child("+ Task")
        .on_click(weak.listener(move |this: &mut Notes, _: &ClickEvent, w, cx| {
            this.with_db(slot, |d| d.tasks.push(Task { name: "New task".into(), status: d.filter.unwrap_or(Status::Todo), estimate: 1.0, tag: 0 }));
            this.s.status = "Added a task".into();
            w.focus_id("root");
            cx.notify();
        })));
    let order = db.order();
    let hours: f32 = order.iter().map(|&t| db.tasks[t].estimate).sum();
    let body = match db.view {
        View::Table => db_table(st, slot, db, &order, weak),
        View::Board => db_board(st, slot, db, weak),
    };
    div().child(bar).child(body).child(div().class("sum").child(format!("{} of {} tasks \u{00b7} {hours} h estimated", order.len(), db.tasks.len())))
}

fn db_table(_st: &State, slot: usize, db: &Db, order: &[usize], weak: WeakEntity<Notes>) -> Div {
    let head = |s: Sort, label: &'static str, class: &'static str| {
        let on = db.sort == s;
        th().id(format!("sort{slot}_{}", s as u8)).class(if on { format!("{class} on") } else { class.to_owned() })
            .child(if on { format!("{label} \u{2193}") } else { label.to_owned() })
            .on_click(weak.listener(move |this: &mut Notes, _: &ClickEvent, _, cx| {
                this.with_db(slot, |d| d.sort = if d.sort == s { Sort::Manual } else { s });
                cx.notify();
            }))
    };
    let mut t = table().class("db").child(tr().child(head(Sort::Name, "Name", "nm")).child(head(Sort::Status, "Status", "st"))
        .child(head(Sort::Estimate, "Hours", "est")).child(th().class("tg").child("Tag")));
    for &k in order {
        let task = &db.tasks[k];
        let tag = &db.tags[task.tag.min(db.tags.len() - 1)];
        t = t.child(tr()
            .child(td().child(TextField().id(format!("name{slot}_{k}")).w(relative(1.0)).value(task.name.clone())
                .on_change(weak.listener(move |this: &mut Notes, e: &ChangeEvent, _, cx| {
                    this.with_db(slot, |d| d.tasks[k].name.clone_from(&e.value));
                    cx.notify();
                }))))
            .child(td().class("st").child(Dropdown().id(format!("st{slot}_{k}")).value(task.status.name())
                .children(STATUSES.iter().map(|s| option().child(s.name())))
                .on_change(weak.listener(move |this: &mut Notes, e: &ChangeEvent, _, cx| {
                    let s = STATUSES[(e.number as usize).min(2)];
                    this.with_db(slot, |d| d.tasks[k].status = s);
                    cx.notify();
                }))))
            .child(td().class("est").child(NumberField().id(format!("est{slot}_{k}")).min(0.0).max(99.0).step(0.5).precision(1).w(relative(1.0))
                .value(format!("{}", task.estimate))
                .on_change(weak.listener(move |this: &mut Notes, e: &ChangeEvent, _, cx| {
                    let v = (e.number as f32 * 2.0).round() / 2.0;
                    this.with_db(slot, |d| d.tasks[k].estimate = v.clamp(0.0, 99.0));
                    cx.notify();
                }))))
            .child(td().class("tg").child(div().class("tagcell").child(div().class("dot").css("background", hex(tag.color)))
                .child(Dropdown().id(format!("tag{slot}_{k}")).value(tag.name).children(db.tags.iter().map(|g| option().child(g.name)))
                    .on_change(weak.listener(move |this: &mut Notes, e: &ChangeEvent, _, cx| {
                        let g = e.number as usize;
                        this.with_db(slot, |d| d.tasks[k].tag = g.min(d.tags.len() - 1));
                        cx.notify();
                    }))))));
    }
    t
}

fn db_board(st: &State, slot: usize, db: &Db, weak: WeakEntity<Notes>) -> Div {
    let (dragged, over) = match st.drag {
        Some(Drag::Card { slot: s, task, live: true, over, .. }) if s == slot => (Some(task), over),
        _ => (None, None),
    };
    let order = db.order();
    let mut board = div().class("board");
    for (k, status) in STATUSES.iter().enumerate() {
        let cards: Vec<usize> = order.iter().copied().filter(|&t| db.tasks[t].status == *status).collect();
        let hours: f32 = cards.iter().map(|&t| db.tasks[t].estimate).sum();
        let mut col = div().id(format!("col{slot}_{k}")).class(if over == Some(*status) { "col over" } else { "col" })
            .child(div().class("head").child(span().class(format!("chip {}", status.class())).child(status.name()))
                .child(div().class("n").child(format!("{} \u{00b7} {hours} h", cards.len()))));
        for t in cards {
            let task = &db.tasks[t];
            let tag = &db.tags[task.tag.min(db.tags.len() - 1)];
            col = col.child(div().id(format!("card{slot}_{t}")).class(if dragged == Some(t) { "card ghost" } else { "card" })
                .on_pointer_down(weak.listener(move |this: &mut Notes, e: &PointerEvent, w, cx| {
                    this.s.drag = Some(Drag::Card { slot, task: t, x: e.position.x.0, live: false, over: None });
                    w.set_pointer_capture(cx);
                    cx.notify();
                }))
                .child(task.name.clone())
                .child(div().class("row").child(div().class("dot").css("background", hex(tag.color))).child(tag.name)
                    .child(div().css("margin-left", "auto").child(format!("{} h", task.estimate)))));
        }
        board = board.child(col);
    }
    board
}

fn page_view(st: &State, slot: usize, weak: WeakEntity<Notes>) -> Div {
    let p = st.open[slot];
    let page = &st.space.pages[p];
    let title_sel = st.sel == Some(Sel { slot, at: At::Title });
    let mut sheet = div().class("sheet")
        .child(icon(page).class("big-icon"))
        .child(div().id(format!("title{slot}")).class("title")
            .on_mouse_down(MouseButton::Left, weak.listener(move |this: &mut Notes, _: &MouseDownEvent, w, cx| {
                this.select(slot, At::Title);
                this.s.field = None;
                w.focus_id("root");
                cx.notify();
            }))
            .child(typed(&page.title, title_sel, "Untitled")))
        .child(div().class("meta").child(match &page.body {
            Body::Blocks(b) => format!("{} subpages \u{00b7} {} words", page.children.len(), words(b)),
            Body::Db(_) => "Database \u{00b7} drag cards between columns on the board".to_owned(),
        }));
    sheet = match &page.body {
        Body::Blocks(b) => sheet.children(blocks_view(st, slot, b, weak)),
        Body::Db(db) => sheet.child(db_view(st, slot, db, weak)),
    };
    div().id(format!("pane{slot}")).class("pane").child(div().class("cover").css("background", hex(page.cover))).child(sheet)
}

fn props(st: &State, slot: usize, weak: WeakEntity<Notes>) -> Div {
    let p = st.open[slot];
    let page = &st.space.pages[p];
    let mut d = div().class("side").child(h4().child("Page"))
        .child(div().class("line").child(div().class("k").child("Icon"))
            .child(Dropdown().id(format!("icon{slot}")).w(relative(1.0)).value(format!("{} {}", ICONS[page.icon].0, ICONS[page.icon].1))
                .children(ICONS.iter().map(|(g, n)| option().child(format!("{g} {n}"))))
                .on_change(weak.listener(move |this: &mut Notes, e: &ChangeEvent, _, cx| {
                    let p = this.s.open[slot];
                    this.edit(false).pages[p].icon = (e.number as usize).min(ICONS.len() - 1);
                    cx.notify();
                }))))
        .child(div().class("line").child(div().class("k").child("Cover"))
            .child(ColorSwatch().id(format!("cover{slot}")).w(relative(1.0)).value(hex(page.cover))
                .on_change(weak.listener(move |this: &mut Notes, e: &ChangeEvent, _, cx| {
                    if let Some(c) = parse_hex(&e.value) {
                        let p = this.s.open[slot];
                        this.edit(false).pages[p].cover = c;
                    }
                    cx.notify();
                }))))
        .child(div().class("line").child(Checkbox().id(format!("pfav{slot}")).checked(page.fav)
            .on_change(weak.listener(move |this: &mut Notes, e: &ChangeEvent, _, cx| {
                let p = this.s.open[slot];
                this.edit(false).pages[p].fav = e.checked;
                cx.notify();
            }))).child("Favourite"));
    match &page.body {
        Body::Blocks(b) => {
            let todos = b.iter().filter(|b| matches!(b.kind, Kind::Todo { .. })).count();
            let done = b.iter().filter(|b| b.kind == (Kind::Todo { done: true })).count();
            d = d.child(h4().child("Stats")).child(div().class("stat")
                .child(div().child(format!("{} blocks, {} words", b.len(), words(b))))
                .child(div().child(format!("{done} of {todos} to-dos done")))
                .child(div().child(format!("{} subpages", page.children.len()))));
            if todos > 0 {
                d = d.child(ProgressBar().id(format!("progress{slot}")).value(format!("{}", done as f32 / todos as f32)));
            }
        }
        Body::Db(db) => {
            d = d.child(h4().child("Tag colours"));
            for (k, tag) in db.tags.iter().enumerate() {
                d = d.child(div().class("line").child(div().class("k").child(tag.name))
                    .child(ColorSwatch().id(format!("tc{slot}_{k}")).w(relative(1.0)).value(hex(tag.color))
                        .on_change(weak.listener(move |this: &mut Notes, e: &ChangeEvent, _, cx| {
                            if let Some(c) = parse_hex(&e.value) {
                                this.with_db(slot, |d| d.tags[k].color = c);
                            }
                            cx.notify();
                        }))));
            }
        }
    }
    d.child(h4().child("Keys")).child(div().class("keys")
        .child(div().child("/  block menu \u{00b7} Esc closes it"))
        .child(div().child("# ## ###  headings \u{00b7} - 1.  lists"))
        .child(div().child("[]  to-do \u{00b7} >  quote \u{00b7} +  toggle"))
        .child(div().child("Tab / Shift+Tab  indent"))
        .child(div().child("Alt+\u{2191} \u{2193}  move \u{00b7} drag \u{2237} to reorder"))
        .child(div().child("Double-click  edit in a field"))
        .child(div().child("Ctrl+Z / Ctrl+Y  undo, redo")))
}

fn page_editor(st: &State, slot: usize, weak: WeakEntity<Notes>) -> Editor {
    Editor::new(page_view(st, slot, weak))
        .header(Region::new(page_header(st, slot, weak)))
        .sidebar(Region::new(props(st, slot, weak)).size(250).hidden(!st.props || slot == 1).overlap(false))
        .footer(Region::new(page_footer(st, slot)).overlap(false))
}

impl Render for Notes {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        focus_root(w, &mut self.frames);
        if self.s.drag.is_some() {
            self.drag_to(w, self.pointer);
        }
        let weak = cx.weak_entity();
        let st = Arc::new(self.s.clone());
        let (a, b, c) = (st.clone(), st.clone(), st);
        let editors = Editors::new()
            .editor("Pages", "OUTLINER", move |_: &AreaRef| {
                Editor::new(tree(&a, weak)).header(Region::new(div().class("searchbar")
                    .child(TextField().id("search").attr("live", "").placeholder("Search pages").w(relative(1.0)).value(a.query.clone())
                        .on_input(weak.listener(|this: &mut Self, e: &ChangeEvent, _, cx| {
                            this.s.query.clone_from(&e.value);
                            cx.notify();
                        })))))
            })
            .editor("Page", "TEXT", move |_: &AreaRef| page_editor(&b, 0, weak))
            .editor("Side page", "TEXT", move |_: &AreaRef| page_editor(&c, 1, weak));
        app().id("root").attr("tabindex", "0")
            .child(style().child(CSS))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                // keys typed into a Blender field reach the page too: they are the field's, and the
                // Enter or Escape that ends a block's field closes it
                if w.text_field_focused() {
                    if this.s.field.is_some() && matches!(e.keystroke.key.as_str(), "Return" | "KP_Enter" | "Escape") {
                        this.s.field = None;
                        this.typing = false;
                        this.s.status = "Text updated".into();
                        cx.notify();
                    }
                    return;
                }
                if this.key(e) {
                    // taken: Blender's own handling (a hovered widget's shortcuts, its undo) stays out
                    cx.prevent_default();
                    cx.notify();
                }
            }))
            .on_pointer_down(cx.listener(|this, e: &PointerEvent, _, _| this.pointer = (e.position.x.0, e.position.y.0)))
            .on_pointer_move(cx.listener(|this, e: &PointerEvent, _, cx| {
                // where it lands is found as the page draws (a listener knows only its own path's boxes)
                if this.s.drag.is_some() {
                    this.pointer = (e.position.x.0, e.position.y.0);
                    cx.notify();
                }
            }))
            .on_pointer_up(cx.listener(|this, e: &PointerEvent, w, cx| {
                if this.s.drag.is_some() {
                    this.drop(e.modifiers.control);
                    w.release_pointer_capture(cx);
                    w.focus_id("root");
                    cx.notify();
                }
            }))
            .child(Screen().id("screen").class("screen").editors(editors)
                .layout(AreaSplit::v(0.17, "Pages", AreaSplit::v(0.6, "Page", "Side page"))))
    }
}

// ==========================================================================
// Storyboard

/// Presses `from`, moves (a frame on the way, where the drop target is found) to `to`, saves the
/// frame mid-drag as `name` and lets go.
fn drag(s: &mut Story, from: &str, to: (f32, f32), name: &str) {
    let a = s.centre(from);
    s.cx.simulate_press(s.win, a.0, a.1);
    s.cx.simulate_mouse_move(s.win, a.0 + (to.0 - a.0) * 0.5, a.1 + (to.1 - a.1) * 0.5);
    s.tick(16);
    s.cx.simulate_mouse_move(s.win, to.0, to.1);
    s.shot(name);
    s.cx.simulate_release(s.win, to.0, to.1);
}

fn storyboard(dir: String) -> Script {
    story(dir, |s| {
        s.play("00-open", &[0, 200, 600]);
        // type into a block, Enter starts the next
        s.click("b0_1");
        s.type_text(" Welcome aboard.");
        s.key("Return");
        s.type_text("/to");
        s.play("01-slash", &[0, 200]);
        s.key("Return");
        s.type_text("Fix the shed door");
        s.key("Return");
        s.type_text("Sharpen the hoes");
        s.play("02-todos", &[0, 200]);
        // an empty to-do ends the list; markdown shortcuts
        s.key("Return");
        s.key("Return");
        s.type_text("## Tools");
        s.key("Return");
        s.type_text("- Spades: four");
        s.key("Return");
        s.type_text("Forks: three");
        s.key("Tab");
        s.play("03-markdown", &[0, 200]);
        // tick a to-do, close the toggle
        s.click("cb0_10");
        s.click("tg0_12");
        s.play("04-check-toggle", &[0, 300]);
        // drag a block by its grip to the top
        let top = s.bounds("b0_0");
        s.hover("b0_3");
        drag(s, "g0_3", (top.origin.x + 40.0, top.origin.y + 2.0), "05a-dragging");
        s.play("05-dragged", &[0, 200]);
        s.ctrl("z", false);
        s.play("06-undo", &[0, 200]);
        // the database: table, sort, filter, board drag
        s.click("t2");
        s.play("07-db", &[0, 300]);
        s.click("sort0_3");
        s.play("08-sorted", &[0, 200]);
        s.click("f0_1");
        s.play("09-filter", &[0, 200]);
        s.click("f0_0");
        s.click("view0_1");
        s.play("10-board", &[0, 200]);
        let done = s.centre("col0_2");
        drag(s, "card0_3", done, "11-card-drag");
        s.play("12-card-dropped", &[0, 200]);
        // search
        s.click("search");
        s.type_text("glazing");
        s.key("Return");
        s.play("13-search", &[0, 200]);
        s.click("r0");
        s.play("14-result", &[0, 200]);
        s.click("search");
        for _ in 0..7 {
            s.key("BackSpace");
        }
        s.key("Return");
        s.play("15-cleared", &[0, 200]);
        // Ctrl+click opens beside; the tree drags: Recipes into Meeting notes
        let (x, y) = s.centre("t3");
        s.cx.simulate_click_with(s.win, x, y, true, false);
        s.play("16-beside", &[0, 200]);
        let into = s.centre("t4");
        drag(s, "t6", into, "17-tree-drag");
        s.play("18-tree-dropped", &[0, 200]);
        // a new page, titled by typing
        s.click("new-page");
        s.type_text("Harvest log");
        s.key("Return");
        s.type_text("First beans picked on the 3rd.");
        s.play("19-new-page", &[0, 300]);
        // double-click opens the text in a Blender field
        let (x, y) = s.centre("b0_0");
        s.cx.simulate_click(s.win, x, y);
        s.cx.simulate_click(s.win, x, y);
        s.play("20-field", &[0, 200, 400]);
        s.click("fe0_0");
        s.cx.simulate_key(s.win, "", "Home");
        s.type_text("Today: ");
        s.key("Return");
        s.play("21-field-edited", &[0, 300]);
    })
}

fn main() {
    let (w, h) = size_arg((1440.0, 900.0));
    run_with(w, h, "Tessera", Notes::new, arg("--storyboard").map(storyboard));
}

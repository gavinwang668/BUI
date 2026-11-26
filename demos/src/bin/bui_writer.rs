//! Folio: a word processor made of Blender's UI. One Blender screen holds two editors: the
//! Navigator (the document's headings, click to jump) and the Document, whose regions are a
//! ribbon header (Home / Insert / Layout / Review tabs of Blender buttons, dropdowns, number
//! fields, colour swatches and checkboxes), a ruler in the tool header (painted; drag its margin
//! and indent markers), a sidebar (find & replace, comments) and a status footer (pages, words,
//! a zoom slider). Split, join or maximize the areas as in Blender (Ctrl+Space).
//!
//! The page is real editing: the text is Folio's own model (paragraphs of formatted runs, and
//! tables whose cells are Blender text fields), drawn as styled spans on white A4 pages that
//! paginate as they fill. Click or drag to place the caret and select (double-click a word,
//! triple-click a paragraph), type, and format what is selected: Ctrl+B / I / U, Ctrl+1 / 2 / 0
//! for headings, Ctrl+E / L / R / J to align, Tab / Shift+Tab to nest a list, Ctrl+Enter for a
//! page break, Ctrl+C / X / V, Ctrl+Z / Ctrl+Y (or Ctrl+Shift+Z), Ctrl+F to find, Ctrl+A.
//! Export writes Markdown to the system's temp folder.
//!
//! What Blender's UI cannot do here (and how Folio gets around it): a Blender text field holds
//! plain text, so the document is spans Folio lays out itself; there is no text-measuring call in
//! the public API, so a click finds its character by where it falls in the word's box; Up / Down
//! move by paragraph (lines need a layout query).
//!
//! `cargo run --release --bin bui_writer`; `-- --storyboard DIR` drives it headlessly.

use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

use blenderui::prelude::*;
use blenderui::screen::{AreaRef, AreaSplit, Editor, Editors, Region, Screen};
use bui_demos::story::story;
use bui_demos::{arg, focus_root, run_with, size_arg, Script};
use bui_demos::color::parse_hex;

// ==========================================================================
// The document

const FONTS: [&str; 4] = ["Inter", "Georgia", "Consolas", "Times New Roman"];
/// Words the spelling check knows to be wrong, and what they should be.
const TYPOS: [(&str, &str); 6] = [("teh", "the"), ("recieve", "receive"), ("seperate", "separate"), ("occured", "occurred"),
                                  ("definately", "definitely"), ("wich", "which")];
const CM: f32 = 37.795; // px a centimetre at 100 %

/// How a stretch of text looks: `size` 0 is its paragraph style's, `color` None the style's.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent toggles, as a word processor's are")]
struct Fmt {
    bold: bool,
    italic: bool,
    under: bool,
    strike: bool,
    font: u8,
    size: u8,
    color: Option<u32>,
    mark: Option<u32>,
}

#[derive(Clone, Debug)]
struct Run {
    text: String,
    fmt: Fmt,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Style {
    Normal,
    Title,
    H1,
    H2,
    Quote,
    Code,
}

const STYLES: [(Style, &str); 6] = [(Style::Normal, "Normal"), (Style::Title, "Title"), (Style::H1, "Heading 1"),
                                    (Style::H2, "Heading 2"), (Style::Quote, "Quote"), (Style::Code, "Code")];

impl Style {
    /// Its text size, points.
    const fn pt(self) -> f32 {
        match self {
            Self::Normal | Self::Quote => 11.0,
            Self::Title => 26.0,
            Self::H1 => 18.0,
            Self::H2 => 14.0,
            Self::Code => 10.0,
        }
    }

    const fn class(self) -> &'static str {
        match self {
            Self::Normal => "para",
            Self::Title => "para title",
            Self::H1 => "para h1",
            Self::H2 => "para h2",
            Self::Quote => "para quote",
            Self::Code => "para code",
        }
    }

    const fn heading(self) -> bool {
        matches!(self, Self::Title | Self::H1 | Self::H2)
    }

    fn name(self) -> &'static str {
        STYLES.iter().find(|(s, _)| *s == self).map_or("Normal", |(_, n)| n)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Align {
    Left,
    Center,
    Right,
    Justify,
}

impl Align {
    const fn css(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Justify => "justify",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum List {
    None,
    Bullet,
    Number,
}

#[derive(Clone, Debug)]
struct Para {
    runs: Vec<Run>,
    style: Style,
    align: Align,
    list: List,
    /// List nesting.
    level: u8,
    /// Left indent and first-line indent, centimetres (the ruler's markers).
    indent: f32,
    first: f32,
    /// Starts a new page.
    page_break: bool,
}

/// The byte where char `n` of `s` starts.
fn byte_of(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map_or(s.len(), |(i, _)| i)
}

impl Para {
    fn new(style: Style, text: &str) -> Self {
        Self { runs: vec![Run { text: text.to_owned(), fmt: Fmt::default() }], style, align: Align::Left, list: List::None,
               level: 0, indent: 0.0, first: 0.0, page_break: false }
    }

    fn len(&self) -> usize {
        self.runs.iter().map(|r| r.text.chars().count()).sum()
    }

    fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    /// The runs split so that one starts at char `o`: its index.
    fn cut(&mut self, o: usize) -> usize {
        let mut at = 0;
        for i in 0..self.runs.len() {
            if o == at {
                return i;
            }
            let n = self.runs[i].text.chars().count();
            if o < at + n {
                let at_byte = byte_of(&self.runs[i].text, o - at);
                let tail = self.runs[i].text.split_off(at_byte);
                let fmt = self.runs[i].fmt;
                self.runs.insert(i + 1, Run { text: tail, fmt });
                return i + 1;
            }
            at += n;
        }
        self.runs.len()
    }

    /// Empty runs gone and neighbours of one format joined (one empty run stays, keeping its format).
    fn tidy(&mut self) {
        let keep = self.runs.first().map(|r| r.fmt).unwrap_or_default();
        self.runs.retain(|r| !r.text.is_empty());
        let mut i = 1;
        while i < self.runs.len() {
            if self.runs[i].fmt == self.runs[i - 1].fmt {
                let r = self.runs.remove(i);
                self.runs[i - 1].text.push_str(&r.text);
            } else {
                i += 1;
            }
        }
        if self.runs.is_empty() {
            self.runs.push(Run { text: String::new(), fmt: keep });
        }
    }

    /// The format typing at `o` continues: the char before it, or the first run's.
    fn fmt_at(&self, o: usize) -> Fmt {
        let mut at = 0;
        for r in &self.runs {
            let n = r.text.chars().count();
            if o > at && o <= at + n {
                return r.fmt;
            }
            at += n;
        }
        self.runs.first().map(|r| r.fmt).unwrap_or_default()
    }

    fn insert(&mut self, o: usize, s: &str, fmt: Fmt) {
        let i = self.cut(o);
        self.runs.insert(i, Run { text: s.to_owned(), fmt });
        self.tidy();
    }

    fn delete(&mut self, a: usize, z: usize) {
        let i = self.cut(a);
        let j = self.cut(z);
        self.runs.drain(i..j);
        self.tidy();
    }

    /// The runs from char `o` on, taken out.
    fn split_off(&mut self, o: usize) -> Vec<Run> {
        let i = self.cut(o);
        let tail = self.runs.split_off(i);
        self.tidy();
        tail
    }

    fn format(&mut self, a: usize, z: usize, f: &dyn Fn(&mut Fmt)) {
        let i = self.cut(a);
        let j = self.cut(z);
        for r in &mut self.runs[i..j] {
            f(&mut r.fmt);
        }
        self.tidy();
    }

    fn slice(&self, a: usize, z: usize) -> String {
        self.text().chars().skip(a).take(z.saturating_sub(a)).collect()
    }
}

#[derive(Clone, Debug)]
struct Table {
    cols: usize,
    cells: Vec<String>,
}

#[derive(Clone, Debug)]
enum Block {
    Para(Para),
    Table(Table),
}

#[derive(Clone, Debug)]
struct Comment {
    block: usize,
    author: &'static str,
    text: String,
}

#[derive(Clone, Debug)]
struct Doc {
    blocks: Vec<Block>,
    /// Left and right margins, centimetres (top and bottom stay 2.54).
    margins: [f32; 2],
    landscape: bool,
    /// Line spacing (1 = single).
    spacing: f32,
    tint: u32,
    comments: Vec<Comment>,
}

/// A place in the text: a paragraph block and a char offset in it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
struct Pos {
    b: usize,
    o: usize,
}

impl Doc {
    fn para(&self, b: usize) -> Option<&Para> {
        match self.blocks.get(b)? {
            Block::Para(p) => Some(p),
            Block::Table(_) => None,
        }
    }

    fn para_mut(&mut self, b: usize) -> Option<&mut Para> {
        match self.blocks.get_mut(b)? {
            Block::Para(p) => Some(p),
            Block::Table(_) => None,
        }
    }

    fn len_of(&self, b: usize) -> usize {
        self.para(b).map_or(0, Para::len)
    }

    /// The paragraph before or after block `b` (tables are stepped over).
    fn step(&self, b: usize, forward: bool) -> Option<usize> {
        if forward {
            (b + 1..self.blocks.len()).find(|&k| self.para(k).is_some())
        } else {
            (0..b).rev().find(|&k| self.para(k).is_some())
        }
    }

    fn last_pos(&self) -> Pos {
        let b = (0..self.blocks.len()).rev().find(|&k| self.para(k).is_some()).unwrap_or(0);
        Pos { b, o: self.len_of(b) }
    }

    /// Blocks inserted (`delta` > 0) or removed at `at`: comments follow theirs (or go with it).
    fn shift_comments(&mut self, at: usize, delta: isize) {
        self.comments.retain(|c| delta >= 0 || c.block < at || c.block >= at + delta.unsigned_abs());
        for c in &mut self.comments {
            if c.block >= at {
                c.block = c.block.saturating_add_signed(delta);
            }
        }
    }

    fn insert_block(&mut self, at: usize, block: Block) {
        self.blocks.insert(at, block);
        self.shift_comments(at, 1);
    }

    /// The text from `a` to `z` deleted (both in paragraphs, `a` first): where they meet.
    fn delete(&mut self, a: Pos, z: Pos) -> Pos {
        if a.b == z.b {
            if let Some(p) = self.para_mut(a.b) {
                p.delete(a.o, z.o);
            }
            return a;
        }
        let tail = self.para_mut(z.b).map(|p| p.split_off(z.o)).unwrap_or_default();
        if let Some(p) = self.para_mut(a.b) {
            p.split_off(a.o);
            p.runs.extend(tail);
            p.tidy();
        }
        self.blocks.drain(a.b + 1..=z.b);
        self.shift_comments(a.b + 1, -(z.b - a.b).cast_signed());
        a
    }

    /// A new paragraph from `p` on (the next one is Normal after a heading, as editors do).
    fn split(&mut self, p: Pos) -> Pos {
        let Some(para) = self.para_mut(p.b) else { return p };
        let runs = para.split_off(p.o);
        let mut next = para.clone();
        next.runs = runs;
        next.tidy();
        next.page_break = false;
        if next.style.heading() || next.style == Style::Code && next.len() == 0 {
            next.style = Style::Normal;
        }
        self.insert_block(p.b + 1, Block::Para(next));
        Pos { b: p.b + 1, o: 0 }
    }

    /// `s` (lines become paragraphs) typed at `p` in `fmt`: where it ends.
    fn insert_text(&mut self, mut p: Pos, s: &str, fmt: Fmt) -> Pos {
        for (k, line) in s.split('\n').enumerate() {
            if k > 0 {
                p = self.split(p);
            }
            if !line.is_empty()
                && let Some(para) = self.para_mut(p.b)
            {
                para.insert(p.o, line, fmt);
                p.o += line.chars().count();
            }
        }
        p
    }

    fn text_between(&self, a: Pos, z: Pos) -> String {
        let mut out = String::new();
        for b in a.b..=z.b {
            if let Some(p) = self.para(b) {
                let (s, e) = (if b == a.b { a.o } else { 0 }, if b == z.b { z.o } else { p.len() });
                if b > a.b {
                    out.push('\n');
                }
                out.push_str(&p.slice(s, e));
            }
        }
        out
    }

    fn words(&self) -> (usize, usize) {
        let count = |(w, c): (usize, usize), t: &str| (w + t.split_whitespace().count(), c + t.chars().count());
        self.blocks.iter().fold((0, 0), |acc, b| match b {
            Block::Para(p) => count(acc, &p.text()),
            Block::Table(t) => t.cells.iter().fold(acc, |a, cell| count(a, cell)),
        })
    }

    /// Every occurrence of `q`: (block, from, to) in chars.
    fn find(&self, q: &str, case: bool) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        if q.is_empty() {
            return out;
        }
        let fold = |s: &str| if case { s.to_owned() } else { s.to_lowercase() };
        let q = fold(q);
        let n = q.chars().count();
        for (b, block) in self.blocks.iter().enumerate() {
            let Block::Para(p) = block else { continue };
            let t = fold(&p.text());
            let mut from = 0;
            while let Some(i) = t[from..].find(&q) {
                let at = t[..from + i].chars().count();
                out.push((b, at, at + n));
                from += i + q.len().max(1);
            }
        }
        out
    }

    /// The document as Markdown.
    fn markdown(&self) -> String {
        let mut out = String::new();
        let mut number = 0;
        for block in &self.blocks {
            match block {
                Block::Table(t) => {
                    for (r, row) in t.cells.chunks(t.cols).enumerate() {
                        let _ = writeln!(out, "| {} |", row.join(" | "));
                        if r == 0 {
                            let _ = writeln!(out, "|{}|", vec!["---"; t.cols].join("|"));
                        }
                    }
                    out.push('\n');
                }
                Block::Para(p) => {
                    let mut line = String::new();
                    for r in &p.runs {
                        let m = match (r.fmt.bold, r.fmt.italic) {
                            (true, true) => "***",
                            (true, false) => "**",
                            (false, true) => "*",
                            (false, false) => "",
                        };
                        let _ = write!(line, "{m}{}{m}", r.text);
                    }
                    number = if p.list == List::Number { number + 1 } else { 0 };
                    let pad = "  ".repeat(p.level as usize);
                    let _ = match (p.style, p.list) {
                        (Style::Title | Style::H1, _) => writeln!(out, "# {line}\n"),
                        (Style::H2, _) => writeln!(out, "## {line}\n"),
                        (Style::Quote, _) => writeln!(out, "> {line}\n"),
                        (Style::Code, _) => writeln!(out, "```\n{line}\n```\n"),
                        (_, List::Bullet) => writeln!(out, "{pad}- {line}"),
                        (_, List::Number) => writeln!(out, "{pad}{number}. {line}"),
                        _ => writeln!(out, "{line}\n"),
                    };
                }
            }
        }
        out
    }
}

/// The document Folio opens with.
fn sample() -> Doc {
    let p = |style, text: &str| Block::Para(Para::new(style, text));
    let li = |list, text: &str| {
        let mut para = Para::new(Style::Normal, text);
        para.list = list;
        Block::Para(para)
    };
    let mut intro = Para::new(Style::Normal, "");
    intro.runs = vec![
        Run { text: "This report follows the ".into(), fmt: Fmt::default() },
        Run { text: "Saltmarsh Light".into(), fmt: Fmt { bold: true, ..Fmt::default() } },
        Run { text: " restoration from survey to first lighting. It is written in Folio, a word processor whose \
                     every control is one of Blender's widgets, wich makes it a fair test of what that toolkit can carry."
                  .into(), fmt: Fmt::default() },
    ];
    let mut quote = Para::new(Style::Quote, "A lighthouse is a promise kept in the dark, one sweep at a time.");
    quote.align = Align::Center;
    let mut blocks = vec![
        p(Style::Title, "The Saltmarsh Light"),
        p(Style::H2, "Restoration report, spring season"),
        Block::Para(intro),
        p(Style::H1, "1. Survey"),
        p(Style::Normal, "The tower stands on a shingle spit two kilometres from the village. Teh survey team spent three \
                          days on the lantern room alone: the glazing bars had rusted through on the seaward side, and \
                          salt had worked into every joint of the gallery rail."),
        li(List::Bullet, "Lantern glazing: 14 of 24 panes cracked or clouded"),
        li(List::Bullet, "Gallery rail: replace the seaward third"),
        li(List::Bullet, "Lens: second-order Fresnel, intact, needs cleaning"),
        p(Style::Normal, "We will recieve the replacement glass in two batches, and keep the lens under cover until the \
                          roof is sound."),
        p(Style::H1, "2. Works"),
        p(Style::Normal, "Works ran in four stages. Each stage had its own crew and its own weather window, kept seperate \
                          so a delay at the top of the tower never held up the masons at its foot."),
        li(List::Number, "Scaffold and weather-proof the lantern"),
        li(List::Number, "Strip and treat the gallery steel"),
        li(List::Number, "Re-glaze and re-seal"),
        li(List::Number, "Clean, level and re-time the lens"),
        Block::Table(Table { cols: 3, cells: ["Stage", "Weeks", "Crew", "Scaffold", "2", "4", "Steel", "5", "6", "Glazing", "3",
                                              "3", "Lens", "2", "2"].map(String::from).to_vec() }),
        p(Style::Normal, "The longest stage was the steel. Most of the rail was sound once the paint was off, but a crack \
                          had occured in the bracket under the north-east stanchion, and the whole bracket was remade."),
        Block::Para(quote),
        p(Style::H1, "3. First light"),
        p(Style::Normal, "The light was lit again at dusk on the last day of April. From the harbour wall it reads as it \
                          did a century ago: two white flashes every ten seconds, visible for eighteen nautical miles."),
        p(Style::H2, "Next steps"),
        p(Style::Normal, "Over the summer the keeper's cottage will be restored as a visitor centre. The lens will \
                          definately need a second clean after the first winter storms."),
        p(Style::Code, "flash: 2 x 0.3 s every 10 s, range 18 nmi"),
    ];
    for k in 0..5 {
        blocks.push(p(Style::Normal, &format!(
            "Appendix note {}: logbook entries for the season are kept with the trust. Each one records the weather, \
             the crew on site and what was done, so the next restoration starts from a written record rather than \
             memory.", k + 1)));
    }
    Doc { blocks, margins: [2.54, 2.54], landscape: false, spacing: 1.15, tint: 0xffffff,
          comments: vec![Comment { block: 4, author: "Ines", text: "Should we name the survey team here?".into() }] }
}

// ==========================================================================
// The app's state

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Insert,
    Layout,
    Review,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    None,
    Find,
    Comments,
}

/// A ruler marker being dragged.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    LeftMargin,
    RightMargin,
    First,
    Indent,
}

/// Everything the editors draw from (cheap to clone: the document is shared).
#[derive(Clone)]
struct State {
    doc: Arc<Doc>,
    caret: Pos,
    anchor: Pos,
    /// The format the next typed text takes (a toggle with nothing selected).
    pending: Option<Fmt>,
    tab: Tab,
    side: Side,
    zoom: f32,
    query: String,
    with: String,
    case: bool,
    current: usize,
    spelling: bool,
    ruler: bool,
    draft: String,
    /// Table size to insert.
    rows: usize,
    cols: usize,
    /// Each block's page, and the page count.
    pages: Vec<u16>,
    /// Where the page sits in the ruler (px from its left) at this zoom.
    ruler_x: f32,
    status: String,
}

struct Snap {
    doc: Arc<Doc>,
    caret: Pos,
    anchor: Pos,
}

#[allow(clippy::struct_excessive_bools, reason = "independent gestures in flight")]
struct Writer {
    s: State,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    /// Typing that one undo step covers.
    typing: bool,
    /// The pointer is selecting.
    dragging: bool,
    ruler_drag: Option<Mark>,
    /// The caret moved by keys: keep it in view.
    follow: bool,
    /// Frames rendered (keys go to the page once one exists), and passes spent paginating.
    frames: u32,
    paginating: u8,
}

impl Writer {
    fn new() -> Self {
        let doc = Arc::new(sample());
        Self {
            s: State { doc, caret: Pos { b: 2, o: 0 }, anchor: Pos { b: 2, o: 0 }, pending: None, tab: Tab::Home, side: Side::None,
                       zoom: 1.0, query: String::new(), with: String::new(), case: false, current: 0, spelling: false,
                       ruler: true, draft: String::new(), rows: 3, cols: 3, pages: Vec::new(), ruler_x: 0.0,
                       status: "Ready".into() },
            undo: Vec::new(),
            redo: Vec::new(),
            typing: false,
            dragging: false,
            ruler_drag: None,
            follow: false,
            frames: 0,
            paginating: 0,
        }
    }

    fn doc(&self) -> &Doc {
        &self.s.doc
    }

    /// The selection, first to last.
    fn sel(&self) -> (Pos, Pos) {
        (self.s.caret.min(self.s.anchor), self.s.caret.max(self.s.anchor))
    }

    // -- history -------------------------------------------------------------------------

    fn snapshot(&mut self) {
        self.undo.push(Snap { doc: self.s.doc.clone(), caret: self.s.caret, anchor: self.s.anchor });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// An edit: one undo step, then the document to change (shared copies are copied first).
    fn edit(&mut self) -> &mut Doc {
        self.snapshot();
        self.typing = false;
        Arc::make_mut(&mut self.s.doc)
    }

    fn undo(&mut self, back: bool) {
        let (from, to) = if back { (&mut self.undo, &mut self.redo) } else { (&mut self.redo, &mut self.undo) };
        if let Some(s) = from.pop() {
            to.push(Snap { doc: self.s.doc.clone(), caret: self.s.caret, anchor: self.s.anchor });
            self.s.doc = s.doc;
            self.s.caret = s.caret;
            self.s.anchor = s.anchor;
            self.s.status = if back { "Undone" } else { "Redone" }.into();
        }
        self.typing = false;
    }

    // -- the caret -----------------------------------------------------------------------

    const fn place(&mut self, p: Pos, extend: bool) {
        self.s.caret = p;
        if !extend {
            self.s.anchor = p;
        }
        self.s.pending = None;
        self.typing = false;
    }

    /// A press on the text at `p`: a click places, a double-click takes the word, a triple the paragraph.
    fn press(&mut self, p: Pos, clicks: usize, shift: bool) {
        match clicks {
            2 => {
                let text: Vec<char> = self.doc().para(p.b).map(Para::text).unwrap_or_default().chars().collect();
                let word = |c: char| c.is_alphanumeric() || c == '\'';
                let mut a = p.o.min(text.len());
                while a > 0 && word(text[a - 1]) {
                    a -= 1;
                }
                let mut z = p.o.min(text.len());
                while z < text.len() && word(text[z]) {
                    z += 1;
                }
                self.place(Pos { b: p.b, o: a }, false);
                self.s.caret.o = z;
            }
            3 => {
                self.place(Pos { b: p.b, o: 0 }, false);
                self.s.caret.o = self.doc().len_of(p.b);
            }
            _ => {
                self.place(p, shift);
                self.dragging = true;
            }
        }
    }

    /// The caret one step on: a char (a word with Ctrl), across paragraphs.
    fn moved(&self, p: Pos, forward: bool, word: bool) -> Pos {
        let doc = self.doc();
        let len = doc.len_of(p.b);
        if forward && p.o >= len {
            return doc.step(p.b, true).map_or(p, |b| Pos { b, o: 0 });
        }
        if !forward && p.o == 0 {
            return doc.step(p.b, false).map_or(p, |b| Pos { b, o: doc.len_of(b) });
        }
        if !word {
            return Pos { b: p.b, o: if forward { p.o + 1 } else { p.o - 1 } };
        }
        let text: Vec<char> = doc.para(p.b).map(Para::text).unwrap_or_default().chars().collect();
        let mut o = p.o;
        if forward {
            while o < text.len() && !text[o].is_alphanumeric() {
                o += 1;
            }
            while o < text.len() && text[o].is_alphanumeric() {
                o += 1;
            }
        } else {
            while o > 0 && !text[o - 1].is_alphanumeric() {
                o -= 1;
            }
            while o > 0 && text[o - 1].is_alphanumeric() {
                o -= 1;
            }
        }
        Pos { b: p.b, o }
    }

    // -- editing -------------------------------------------------------------------------

    /// The selection deleted (nothing selected: nothing): where the caret is.
    fn cut_selection(&mut self, doc_changed: bool) -> Pos {
        let (a, z) = self.sel();
        if a == z {
            return a;
        }
        if !doc_changed {
            self.snapshot();
        }
        let p = Arc::make_mut(&mut self.s.doc).delete(a, z);
        self.place(p, false);
        p
    }

    fn type_text(&mut self, s: &str) {
        let (a, z) = self.sel();
        if !(self.typing && a == z) {
            self.snapshot();
        }
        let p = self.cut_selection(true);
        let fmt = self.s.pending.unwrap_or_else(|| self.doc().para(p.b).map(|para| para.fmt_at(p.o)).unwrap_or_default());
        let end = Arc::make_mut(&mut self.s.doc).insert_text(p, s, fmt);
        let pending = self.s.pending;
        self.place(end, false);
        self.s.pending = pending;
        // a word is one undo step
        self.typing = s != " ";
        self.follow = true;
    }

    fn backspace(&mut self, forward: bool, word: bool) {
        let (a, z) = self.sel();
        if a != z {
            self.cut_selection(false);
            return;
        }
        let other = self.moved(a, forward, word);
        if other == a {
            return;
        }
        let (from, to) = (a.min(other), a.max(other));
        let doc = self.edit();
        let p = doc.delete(from, to);
        self.place(p, false);
        self.follow = true;
    }

    fn enter(&mut self, page_break: bool) {
        self.cut_selection(false);
        let at = self.s.caret;
        let doc = self.edit();
        let p = doc.split(at);
        if page_break && let Some(para) = doc.para_mut(p.b) {
            para.page_break = true;
        }
        self.place(p, false);
        self.follow = true;
        if page_break {
            self.s.status = "Page break".into();
        }
    }

    /// Each selected paragraph changed by `f`.
    fn paras(&mut self, f: impl Fn(&mut Para)) {
        let (a, z) = self.sel();
        let doc = self.edit();
        for b in a.b..=z.b {
            if let Some(p) = doc.para_mut(b) {
                f(p);
            }
        }
    }

    /// The selected text's format changed by `f`; with nothing selected, what is typed next.
    fn format(&mut self, f: impl Fn(&mut Fmt)) {
        let (a, z) = self.sel();
        if a == z {
            let mut fmt = self.s.pending.unwrap_or_else(|| self.doc().para(a.b).map(|p| p.fmt_at(a.o)).unwrap_or_default());
            f(&mut fmt);
            self.s.pending = Some(fmt);
            return;
        }
        let doc = self.edit();
        for b in a.b..=z.b {
            if let Some(p) = doc.para_mut(b) {
                let (s, e) = (if b == a.b { a.o } else { 0 }, if b == z.b { z.o } else { p.len() });
                p.format(s, e, &f);
            }
        }
    }

    /// The format at the caret (what the ribbon shows).
    fn current(&self) -> (Fmt, Option<&Para>) {
        let (a, _) = self.sel();
        let para = self.doc().para(a.b);
        (self.s.pending.unwrap_or_else(|| para.map(|p| p.fmt_at(if self.s.caret == self.s.anchor { a.o } else { a.o + 1 })).unwrap_or_default()), para)
    }

    fn toggle(&mut self, which: fn(&mut Fmt) -> &mut bool) {
        let mut probe = self.current().0;
        let on = !*which(&mut probe);
        self.format(move |f| *which(f) = on);
    }

    fn copy(&self, w: &mut Window) {
        let (a, z) = self.sel();
        if a != z {
            w.write_clipboard(&self.doc().text_between(a, z));
        }
    }

    fn paste(&mut self, w: &Window) {
        if let Some(text) = w.read_clipboard() {
            self.typing = false;
            self.type_text(&text.replace("\r\n", "\n"));
            self.typing = false;
        }
    }

    fn insert_table(&mut self) {
        let (rows, cols) = (self.s.rows.max(1), self.s.cols.max(1));
        let at = self.s.caret.b + 1;
        let doc = self.edit();
        let mut cells = vec![String::new(); rows * cols];
        for (c, cell) in cells.iter_mut().take(cols).enumerate() {
            *cell = format!("Column {}", c + 1);
        }
        doc.insert_block(at, Block::Table(Table { cols, cells }));
        doc.insert_block(at + 1, Block::Para(Para::new(Style::Normal, "")));
        self.place(Pos { b: at + 1, o: 0 }, false);
        self.s.status = format!("Inserted a {rows} x {cols} table");
    }

    /// The find matches, and the current one selected and shown.
    fn find_step(&mut self, by: isize) {
        let hits = self.doc().find(&self.s.query, self.s.case);
        if hits.is_empty() {
            self.s.status = format!("No match for \u{201c}{}\u{201d}", self.s.query);
            return;
        }
        let n = hits.len().cast_signed();
        self.s.current = (self.s.current.cast_signed() + by).rem_euclid(n).cast_unsigned();
        let (b, s, e) = hits[self.s.current];
        self.place(Pos { b, o: s }, false);
        self.s.caret.o = e;
        self.follow = true;
        self.s.status = format!("Match {} of {}", self.s.current + 1, hits.len());
    }

    fn replace(&mut self, all: bool) {
        let hits = self.doc().find(&self.s.query, self.s.case);
        if hits.is_empty() {
            return;
        }
        let with = self.s.with.clone();
        let picked: Vec<(usize, usize, usize)> = if all { hits } else { vec![hits[self.s.current.min(hits.len() - 1)]] };
        let count = picked.len();
        let doc = self.edit();
        // last first: earlier offsets stay right
        for &(b, s, e) in picked.iter().rev() {
            if let Some(p) = doc.para_mut(b) {
                let fmt = p.fmt_at(s + 1);
                p.delete(s, e);
                p.insert(s, &with, fmt);
            }
        }
        self.place(self.s.caret.min(self.doc().last_pos()), false);
        self.s.status = format!("Replaced {count}");
    }

    fn fix_spelling(&mut self) {
        let mut fixed = 0;
        let doc = self.edit();
        for block in &mut doc.blocks {
            let Block::Para(p) = block else { continue };
            for r in &mut p.runs {
                for (bad, good) in TYPOS {
                    for (from, to) in [(bad.to_owned(), good.to_owned()), (capital(bad), capital(good))] {
                        let n = r.text.matches(from.as_str()).count();
                        if n > 0 {
                            fixed += n;
                            r.text = r.text.replace(&from, &to);
                        }
                    }
                }
            }
        }
        let last = self.doc().last_pos();
        self.place(self.s.caret.min(last), false);
        self.s.status = format!("Fixed {fixed} spellings");
    }

    fn export(&mut self) {
        let path = std::env::temp_dir().join("folio-export.md");
        self.s.status = match std::fs::write(&path, self.doc().markdown()) {
            Ok(()) => format!("Exported to {}", path.display()),
            Err(e) => format!("Export failed: {e}"),
        };
    }

    const fn jump(&mut self, b: usize) {
        self.place(Pos { b, o: 0 }, false);
        self.follow = true;
    }

    /// A ruler marker dragged to `x` (px from the ruler's left).
    fn ruler_to(&mut self, mark: Mark, x: f32) {
        let cm_px = CM * self.s.zoom;
        let width = if self.doc().landscape { 29.7 } else { 21.0 };
        let cm = ((x - self.s.ruler_x) / cm_px).clamp(0.0, width);
        let snap = |v: f32| (v * 4.0).round() / 4.0;
        let [ml, mr] = self.doc().margins;
        // one undo step for the whole drag (the press took it)
        let doc = Arc::make_mut(&mut self.s.doc);
        match mark {
            Mark::LeftMargin => doc.margins[0] = snap(cm).clamp(0.5, width - mr - 3.0),
            Mark::RightMargin => doc.margins[1] = snap(width - cm).clamp(0.5, width - ml - 3.0),
            Mark::Indent | Mark::First => {
                let b = self.s.caret.b;
                if let Some(p) = doc.para_mut(b) {
                    let v = snap(cm - ml).clamp(-ml, width - ml - mr - 2.0);
                    if mark == Mark::Indent {
                        p.first += p.indent - v;
                        p.indent = v.max(0.0);
                    } else {
                        p.first = v - p.indent;
                    }
                }
            }
        }
    }

    fn key(&mut self, e: &KeyDownEvent, w: &mut Window) -> bool {
        let k = &e.keystroke;
        let (ctrl, shift) = (k.modifiers.control, k.modifiers.shift);
        let key = k.key.as_str();
        let letter = |c: &str| key.eq_ignore_ascii_case(c);
        match key {
            "Left" | "Right" => {
                let (a, z) = self.sel();
                let p = if a != z && !shift { if key == "Left" { a } else { z } } else { self.moved(self.s.caret, key == "Right", ctrl) };
                self.place(p, shift);
            }
            "Up" | "Down" => {
                let p = self.s.caret;
                let to = self.doc().step(p.b, key == "Down").map_or(p, |b| Pos { b, o: p.o.min(self.doc().len_of(b)) });
                self.place(to, shift);
            }
            "Home" | "End" => {
                let p = match (key, ctrl) {
                    ("Home", true) => Pos { b: self.doc().step(usize::MAX, true).unwrap_or(0).min(self.doc().step(0, true).map_or(0, |_| 0)), o: 0 },
                    ("End", true) => self.doc().last_pos(),
                    ("Home", false) => Pos { b: self.s.caret.b, o: 0 },
                    _ => Pos { b: self.s.caret.b, o: self.doc().len_of(self.s.caret.b) },
                };
                self.place(p, shift);
            }
            "BackSpace" => self.backspace(false, ctrl),
            "Delete" => self.backspace(true, ctrl),
            "Return" | "KP_Enter" => self.enter(ctrl),
            "Tab" => self.paras(move |p| {
                if p.list == List::None {
                    p.indent = if shift { (p.indent - 1.25).max(0.0) } else { p.indent + 1.25 };
                } else {
                    p.level = if shift { p.level.saturating_sub(1) } else { (p.level + 1).min(4) };
                }
            }),
            "Escape" => {
                if self.s.side == Side::None {
                    let c = self.s.caret;
                    self.place(c, false);
                } else {
                    self.s.side = Side::None;
                }
            }
            _ if ctrl && letter("a") => {
                self.place(Pos::default(), false);
                self.s.caret = self.doc().last_pos();
            }
            _ if ctrl && letter("z") => self.undo(!shift),
            _ if ctrl && letter("y") => self.undo(false),
            _ if ctrl && letter("b") => self.toggle(|f| &mut f.bold),
            _ if ctrl && letter("i") => self.toggle(|f| &mut f.italic),
            _ if ctrl && letter("u") => self.toggle(|f| &mut f.under),
            _ if ctrl && letter("c") => self.copy(w),
            _ if ctrl && letter("x") => {
                self.copy(w);
                self.cut_selection(false);
            }
            _ if ctrl && letter("v") => self.paste(w),
            _ if ctrl && letter("e") => self.paras(|p| p.align = Align::Center),
            _ if ctrl && letter("l") => self.paras(|p| p.align = Align::Left),
            _ if ctrl && letter("r") => self.paras(|p| p.align = Align::Right),
            _ if ctrl && letter("j") => self.paras(|p| p.align = Align::Justify),
            _ if ctrl && letter("f") => self.s.side = Side::Find,
            _ if ctrl && letter("s") => self.export(),
            _ if ctrl && key == "1" => self.paras(|p| p.style = Style::H1),
            _ if ctrl && key == "2" => self.paras(|p| p.style = Style::H2),
            _ if ctrl && key == "0" => self.paras(|p| p.style = Style::Normal),
            _ => match &k.key_char {
                Some(c) if !ctrl && !c.chars().any(char::is_control) => self.type_text(c),
                _ => return false,
            },
        }
        if !matches!(key, "Escape") {
            self.follow = true;
        }
        true
    }

    /// Each block's page from the heights it had last frame: a block that would overflow starts the next.
    fn paginate(&mut self, w: &Window) -> bool {
        let zoom_px = 16.0 * self.s.zoom;
        let page_h = if self.doc().landscape { 21.0 } else { 29.7 } * CM / 16.0;
        let avail = (page_h - 2.0 * 2.54 * CM / 16.0) * zoom_px;
        let mut pages = Vec::with_capacity(self.doc().blocks.len());
        let (mut page, mut used) = (0u16, 0.0f32);
        let mut id = String::new();
        for (b, block) in self.doc().blocks.iter().enumerate() {
            id.clear();
            let _ = write!(id, "blk{b}");
            let h = w.bounds_of(&id).map_or(1.6 * zoom_px, |r| r.size.height);
            let forced = matches!(block, Block::Para(p) if p.page_break);
            if (forced || used + h > avail) && used > 0.0 {
                page += 1;
                used = 0.0;
            }
            used += h;
            pages.push(page);
        }
        let changed = pages != self.s.pages;
        self.s.pages = pages;
        changed
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
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
bui-region[region="tool_header"] { background: #262626; }

.main { display: flex; flex-direction: column; height: 100%; }
.tabs { display: flex; gap: 2px; align-items: center; height: 100%; padding: 0 8px; }
.tab { height: 22px; padding: 0 14px; display: flex; align-items: center; border-radius: 4px 4px 0 0; color: #b4b4b4;
       font-size: 12px; cursor: pointer; transition-property: background-color, color; transition-duration: 120ms; }
.tab:hover { background: #3a3a3a; color: #e6e6e6; }
.tab.on { background: #303030; color: #ffffff; box-shadow: inset 0 2px 0 #4772b3; }
.tbtn { height: 20px; padding: 0 8px; display: flex; align-items: center; justify-content: center; min-width: 14px;
        border-radius: 4px; background: #545454; color: #e6e6e6; font-size: 12px; cursor: pointer;
        box-shadow: 0 1px 1px rgba(0,0,0,0.35); transition-property: background-color, transform; transition-duration: 100ms; }
.tbtn:hover { background: #656565; }
.tbtn:active { transform: scale(0.96); }
.tbtn.on { background: #4772b3; color: #ffffff; }
.tabs .spacer { flex: 1; }
.tabs .doc { color: #9a9a9a; font-size: 11px; padding: 0 10px; }
.row { display: flex; gap: 6px; align-items: center; flex-wrap: nowrap; height: 100%; padding: 0 6px; }
.group { display: flex; gap: 2px; align-items: center; padding: 0 6px; border-right: 1px solid #3c3c3c; }
.group:last-child { border-right: 0; }
.group .cap { font-size: 10px; color: #8c8c8c; margin-right: 4px; }
.tight { min-width: 26px; }
.row select { width: 120px; }
.row .narrow { width: 64px; }

.pages { flex: 1; min-height: 0; overflow-y: auto; background: #1f1f1f; padding: 1.5em 0 3em; }
.page { position: relative; margin: 0 auto 1.5em; background: #ffffff; color: #1b1b1b;
        box-shadow: 0 0.4em 1.6em rgba(0,0,0,0.5), 0 0 0 1px rgba(0,0,0,0.3); font-family: Inter; }
.sheet { box-sizing: border-box; }
.folio { position: absolute; left: 0; right: 0; bottom: 1.4em; text-align: center; font-size: 0.6em; color: #9a9a9a;
         font-variant-numeric: tabular-nums; }
.para { padding-bottom: 0.45em; position: relative; cursor: text; }
.para .row-in { display: flex; }
.para .marker { flex: 0 0 auto; width: 1.4em; color: #4a4a4a; font-variant-numeric: tabular-nums; }
.para .txt { flex: 1; white-space: pre-wrap; min-height: 1em; }
.title { color: #1d2b4f; font-weight: 800; letter-spacing: -0.03em; text-wrap: balance; }
.h1 { color: #20356a; font-weight: 700; padding-top: 0.5em; text-wrap: balance; }
.h2 { color: #3b5797; font-weight: 600; text-wrap: balance; }
.quote .txt { color: #50555f; font-style: italic; border-left: 0.2em solid #c9d3ea; padding-left: 0.8em; }
.code .txt { font-family: Consolas; background: #f1f2f4; padding: 0.4em 0.6em; border-radius: 0.3em; }
.commented::after { content: ""; position: absolute; right: -1.2em; top: 0.2em; width: 0.35em; height: 1em;
                    border-radius: 0.2em; background: #f2c14e; }
.w { }
.b { font-weight: 700; }
.i { font-style: italic; }
.u { text-decoration: underline; }
.s { text-decoration: line-through; }
.u.s { text-decoration: underline line-through; }
.typo { text-decoration: underline wavy #e5484d; }
.sel { background: #b8cdf2; }
.hit { background: #ffe58a; }
.hit.cur { background: #ffa94d; }
.caret { display: inline-block; width: 2px; height: 1.15em; vertical-align: text-bottom; margin: 0 -1px;
         background: #2f6df6; }
.tbl { padding-bottom: 0.6em; }
.tbl table { border-collapse: collapse; width: 100%; }
.tbl td { border: 1px solid #c8ccd4; padding: 3px; }
.tbl tr:first-child td { background: #eef1f7; }
.tbl .add { margin-top: 4px; }

.ruler { height: 22px; flex: 0 0 22px; position: relative; border-bottom: 1px solid #1c1c1c; }
.status { display: flex; align-items: center; gap: 14px; padding: 0 10px; height: 100%; font-size: 11px; color: #a8a8a8;
          font-variant-numeric: tabular-nums; }
.status .grow { flex: 1; overflow: hidden; white-space: nowrap; }
.status .zoom { width: 160px; }

.side { padding: 10px; display: flex; flex-direction: column; gap: 8px; font-size: 12px; }
.side h4 { margin: 0 0 2px; font-size: 12px; color: #dcdcdc; }
.side .line { display: flex; gap: 6px; align-items: center; }
.side .count { color: #9a9a9a; font-variant-numeric: tabular-nums; }
.side .note { background: #353535; border-radius: 6px; padding: 8px; display: flex; flex-direction: column; gap: 4px;
              box-shadow: 0 1px 2px rgba(0,0,0,0.4); cursor: pointer; }
.side .note:hover { background: #3c3c3c; }
.side .who { color: #f2c14e; font-weight: 600; font-size: 11px; }
.side .quote-of { color: #8c8c8c; font-style: italic; font-size: 11px; }

.nav { padding: 8px 6px; display: flex; flex-direction: column; gap: 1px; height: 100%; overflow-y: auto; font-size: 12px; }
.nav .head { padding: 4px 8px; border-radius: 4px; color: #d2d2d2; cursor: pointer; white-space: nowrap; overflow: hidden;
             text-overflow: ellipsis; transition-property: background-color; transition-duration: 120ms; }
.nav .head:hover { background: #3a3a3a; }
.nav .head.on { background: #4772b3; color: #fff; }
.nav .l1 { padding-left: 20px; }
.nav .l2 { padding-left: 32px; color: #a9a9a9; }
.nav .stats { margin-top: 12px; padding: 8px; color: #8c8c8c; border-top: 1px solid #3a3a3a; font-variant-numeric: tabular-nums;
              line-height: 1.6; }
"#;

/// A block's element id.
fn block_id(b: usize) -> String {
    format!("blk{b}")
}

/// Where a press at window `x` falls in the word `id` (chars `start` .. `start + n`).
fn hit(w: &Window, id: &str, start: usize, n: usize, x: f32) -> usize {
    w.bounds_of(id).map_or(start, |r| {
        let f = ((x - r.origin.x) / r.size.width.max(1.0)).clamp(0.0, 1.0);
        start + (f * n as f32).round() as usize
    })
}

/// The style of one stretch of text, in the page's ems.
fn text_style(d: Div, fmt: Fmt, style: Style) -> Div {
    let mut d = d;
    let cls = [(fmt.bold, "b"), (fmt.italic, "i"), (fmt.under, "u"), (fmt.strike, "s")];
    for (on, c) in cls {
        if on {
            d = d.class(c);
        }
    }
    if fmt.font > 0 {
        d = d.css("font-family", FONTS[fmt.font as usize % FONTS.len()]);
    }
    if fmt.size > 0 && (f32::from(fmt.size) - style.pt()).abs() > f32::EPSILON {
        d = d.css("font-size", format!("{}em", f32::from(fmt.size) / style.pt()));
    }
    if let Some(c) = fmt.color {
        d = d.css("color", format!("#{c:06x}"));
    }
    if let Some(m) = fmt.mark {
        d = d.css("background", format!("#{m:06x}"));
    }
    d
}

/// What the page draws a paragraph with.
struct Marks<'a> {
    st: &'a State,
    hits: &'a [(usize, usize, usize)],
    weak: WeakEntity<Writer>,
}

fn para_view(m: &Marks, b: usize, p: &Para, number: usize) -> Div {
    let st = m.st;
    let weak = m.weak;
    let (sa, sz) = (st.caret.min(st.anchor), st.caret.max(st.anchor));
    let len = p.len();
    let sel = (sa != sz && sa.b <= b && b <= sz.b).then_some((if sa.b == b { sa.o } else { 0 }, if sz.b == b { sz.o } else { len }));
    let caret = (st.caret.b == b).then_some(st.caret.o);
    let hits: Vec<(usize, usize, bool)> = m.hits.iter().enumerate().filter(|(_, h)| h.0 == b)
        .map(|(k, h)| (h.1, h.2, k == st.current)).collect();
    // every place a word's look changes: the selection's ends, the caret, the matches' ends
    let mut cuts: Vec<usize> = Vec::new();
    if let Some((s, e)) = sel {
        cuts.extend([s, e]);
    }
    cuts.extend(caret);
    for h in &hits {
        cuts.extend([h.0, h.1]);
    }
    cuts.sort_unstable();
    cuts.dedup();

    let mut words: Vec<Div> = Vec::new();
    let mut at = 0usize;
    for run in &p.runs {
        // words with the spaces after them: one box each (a click finds its char in it)
        let mut start_byte = 0;
        let mut start_char = at;
        let mut prev_space = false;
        let chars: Vec<(usize, char)> = run.text.char_indices().collect();
        for (k, &(i, c)) in chars.iter().enumerate() {
            let space = c.is_whitespace();
            if prev_space && !space {
                words.push(word(m, b, p, run, &run.text[start_byte..i], start_char, at + k - start_char + start_char - start_char, &cuts, sel, caret, &hits));
                start_byte = i;
                start_char = at + k;
            }
            prev_space = space;
        }
        if start_byte < run.text.len() {
            let n = run.text[start_byte..].chars().count();
            words.push(word(m, b, p, run, &run.text[start_byte..], start_char, n, &cuts, sel, caret, &hits));
        }
        at += chars.len();
    }
    if caret == Some(len) {
        words.push(span().id("caret").class("caret"));
    }
    let marker = match p.list {
        List::None => None,
        List::Bullet => Some(["\u{2022}", "\u{25e6}", "\u{25aa}"][p.level as usize % 3].to_owned()),
        List::Number => Some(format!("{number}.")),
    };
    let mut row = div().class("row-in");
    if let Some(mk) = marker {
        row = row.child(div().class("marker").child(mk));
    }
    let txt = div().class("txt").css("text-align", p.align.css())
        .css("text-indent", format!("{}em", p.first * CM / 16.0 / (p.style.pt() * 4.0 / 3.0 / 16.0)))
        .children(words);
    let indent = (p.indent + f32::from(p.level) * 0.9) * CM / 16.0 / (p.style.pt() * 4.0 / 3.0 / 16.0);
    let mut d = div().id(block_id(b)).class(p.style.class())
        .css("font-size", format!("{}em", p.style.pt() * 4.0 / 3.0 / 16.0))
        .css("padding-left", format!("{indent}em"))
        .child(row.child(txt))
        .on_mouse_down(MouseButton::Left, weak.listener(move |this: &mut Writer, e: &MouseDownEvent, w, cx| {
            let end = Pos { b, o: this.doc().len_of(b) };
            this.press(end, e.click_count, e.modifiers.shift);
            w.focus_id("root");
            cx.notify();
        }));
    if st.doc.comments.iter().any(|c| c.block == b) {
        d = d.class("commented");
    }
    d
}

/// One word (with its spaces): its stretches cut where the selection, the caret or a match change.
#[allow(clippy::too_many_arguments)]
fn word(m: &Marks, b: usize, p: &Para, run: &Run, text: &str, start: usize, n: usize, cuts: &[usize],
        sel: Option<(usize, usize)>, caret: Option<usize>, hits: &[(usize, usize, bool)]) -> Div {
    let weak = m.weak;
    let id = format!("w{b}_{start}");
    let trimmed = text.trim_end().trim_start_matches(|c: char| !c.is_alphanumeric());
    let bare: String = trimmed.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    let typo = m.st.spelling && TYPOS.iter().any(|(bad, _)| *bad == bare);
    let mut d = span().id(id.clone()).class("w");
    if typo {
        d = d.class("typo");
    }
    // the stretches between cuts
    let end = start + n;
    let mut from = start;
    let inner: Vec<usize> = cuts.iter().copied().filter(|&c| c > start && c < end).chain(std::iter::once(end)).collect();
    for to in inner {
        if caret == Some(from) {
            d = d.child(span().id("caret").class("caret"));
        }
        let piece: String = text.chars().skip(from - start).take(to - from).collect();
        let selected = sel.is_some_and(|(a, z)| from >= a && to <= z);
        let hit_class = hits.iter().find(|h| from >= h.0 && to <= h.1).map(|h| if h.2 { "hit cur" } else { "hit" });
        let mut s = text_style(span(), run.fmt, p.style);
        if selected {
            s = s.class("sel");
        }
        if let Some(c) = hit_class {
            s = s.class(c);
        }
        d = d.child(s.child(piece));
        from = to;
    }
    let (down_id, move_id) = (id.clone(), id);
    d.on_mouse_down(MouseButton::Left, weak.listener(move |this: &mut Writer, e: &MouseDownEvent, w, cx| {
        let o = hit(w, &down_id, start, n, e.position.x.0);
        this.press(Pos { b, o }, e.click_count, e.modifiers.shift);
        w.focus_id("root");
        cx.stop_propagation();
        cx.notify();
    }))
    .on_mouse_move(weak.listener(move |this: &mut Writer, e: &MouseMoveEvent, w, cx| {
        if this.dragging && e.pressed_button == Some(MouseButton::Left) {
            let o = hit(w, &move_id, start, n, e.position.x.0);
            if this.s.caret != (Pos { b, o }) {
                this.s.caret = Pos { b, o };
                cx.notify();
            }
        }
    }))
}

fn table_view(st: &State, b: usize, t: &Table, weak: WeakEntity<Writer>) -> Div {
    let mut tbl = table();
    for (r, row) in t.cells.chunks(t.cols).enumerate() {
        let mut tr_el = tr();
        for (c, cell) in row.iter().enumerate() {
            let k = r * t.cols + c;
            tr_el = tr_el.child(td().child(TextField().id(format!("c{b}_{k}")).value(cell.clone()).w(relative(1.0))
                .on_change(weak.listener(move |this: &mut Writer, e: &ChangeEvent, w, cx| {
                    let same = this.doc().blocks.get(b).is_some_and(|bl| matches!(bl, Block::Table(t) if t.cells.get(k) == Some(&e.value)));
                    if !same {
                        let doc = this.edit();
                        if let Some(Block::Table(t)) = doc.blocks.get_mut(b)
                            && let Some(cell) = t.cells.get_mut(k)
                        {
                            cell.clone_from(&e.value);
                        }
                        this.s.status = "Cell edited".into();
                    }
                    w.focus_id("root");
                    cx.notify();
                }))));
        }
        tbl = tbl.child(tr_el);
    }
    let _ = st;
    div().id(block_id(b)).class("tbl").child(tbl)
        .child(div().class("add").child(Button().id(format!("addrow{b}")).child("+ Row")
            .on_click(weak.listener(move |this: &mut Writer, _: &ClickEvent, w, cx| {
                let doc = this.edit();
                if let Some(Block::Table(t)) = doc.blocks.get_mut(b) {
                    let cols = t.cols;
                    t.cells.extend(std::iter::repeat_n(String::new(), cols));
                }
                w.focus_id("root");
                cx.notify();
            }))))
}

fn pages_view(st: &State, weak: WeakEntity<Writer>) -> Div {
    let doc = &st.doc;
    let hits = if st.side == Side::Find { doc.find(&st.query, st.case) } else { Vec::new() };
    let marks = Marks { st, hits: &hits, weak };
    let (pw, ph) = if doc.landscape { (29.7, 21.0) } else { (21.0, 29.7) };
    let em = |cm: f32| format!("{}em", cm * CM / 16.0);
    let count = st.pages.last().map_or(1, |&p| p as usize + 1).max(1);
    let mut sheets: Vec<Div> = (0..count).map(|k| {
        div().id(format!("page{k}")).class("page").css("width", em(pw)).css("min-height", em(ph))
            .css("background", format!("#{:06x}", doc.tint))
    }).collect();
    let mut contents: Vec<Vec<Div>> = (0..count).map(|_| Vec::new()).collect();
    let mut number = 0;
    for (b, block) in doc.blocks.iter().enumerate() {
        let page = st.pages.get(b).map_or(count - 1, |&p| (p as usize).min(count - 1));
        let el = match block {
            Block::Para(p) => {
                number = if p.list == List::Number { number + 1 } else { 0 };
                para_view(&marks, b, p, number)
            }
            Block::Table(t) => {
                number = 0;
                table_view(st, b, t, weak)
            }
        };
        contents[page].push(el);
    }
    let line = format!("{}", doc.spacing * 1.25);
    let mut col = div().id("pages").class("pages").css("font-size", format!("{}px", 16.0 * st.zoom));
    for (k, (sheet, kids)) in sheets.drain(..).zip(contents).enumerate() {
        col = col.child(sheet
            .child(div().class("sheet").css("padding", format!("{} {} {} {}", em(2.54), em(doc.margins[1]), em(2.54), em(doc.margins[0])))
                .css("line-height", line.clone()).children(kids))
            .child(div().class("folio").child(format!("{} / {}", k + 1, count))));
    }
    col
}

/// The ribbon's tabs (the header).
fn tabs(st: &State, weak: WeakEntity<Writer>) -> Div {
    let tab = |id: &'static str, label: &'static str, t: Tab| {
        div().id(id).class(if st.tab == t { "tab on" } else { "tab" }).child(label)
            .on_click(weak.listener(move |this: &mut Writer, _: &ClickEvent, w, cx| {
                this.s.tab = t;
                w.focus_id("root");
                cx.notify();
            }))
    };
    let (words, _) = st.doc.words();
    div().class("tabs").child(tab("tab-home", "Home", Tab::Home)).child(tab("tab-insert", "Insert", Tab::Insert))
        .child(tab("tab-layout", "Layout", Tab::Layout)).child(tab("tab-review", "Review", Tab::Review))
        .child(div().class("spacer")).child(div().class("doc").child(format!("saltmarsh-report.folio \u{00b7} {words} words")))
}

/// The active tab's controls (the tool header: Blender's tool settings bar).
fn ribbon(st: &State, weak: WeakEntity<Writer>, fmt: Fmt, para: Option<(Style, Align, List)>) -> Div {
    let act = |id: &'static str, label: &'static str, f: fn(&mut Writer)| {
        Button().id(id).class("tight").child(label).on_click(weak.listener(move |this: &mut Writer, _: &ClickEvent, w, cx| {
            f(this);
            w.focus_id("root");
            cx.notify();
        }))
    };
    // a Blender button shows no state of its own: a toggle is a box drawn like one, blue when on
    let tog = |id: &'static str, label: &'static str, on: bool, f: fn(&mut Writer)| {
        div().id(id).class(if on { "tbtn on" } else { "tbtn" }).child(label)
            .on_click(weak.listener(move |this: &mut Writer, _: &ClickEvent, w, cx| {
                f(this);
                w.focus_id("root");
                cx.notify();
            }))
    };
    let (style, align, list) = para.unwrap_or((Style::Normal, Align::Left, List::None));
    match st.tab {
        Tab::Home => {
            let styles = Dropdown().id("style").value(style.name())
                .children(STYLES.iter().map(|(_, n)| option().child(*n)))
                .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, w, cx| {
                    let s = STYLES[(e.number as usize).min(STYLES.len() - 1)].0;
                    this.paras(move |p| p.style = s);
                    w.focus_id("root");
                    cx.notify();
                }));
            let fonts = Dropdown().id("font").value(FONTS[fmt.font as usize % FONTS.len()])
                .children(FONTS.iter().map(|f| option().child(*f)))
                .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, w, cx| {
                    let k = e.number as u8;
                    this.format(move |f| f.font = k);
                    w.focus_id("root");
                    cx.notify();
                }));
            let size_now = if fmt.size > 0 { f32::from(fmt.size) } else { style.pt() };
            let size = NumberField().id("size").class("narrow").min(6.0).max(96.0).step(1.0).value(format!("{size_now}"))
                .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, w, cx| {
                    let pt = e.number.round().clamp(6.0, 96.0) as u8;
                    this.format(move |f| f.size = pt);
                    w.focus_id("root");
                    cx.notify();
                }));
            let swatch = |id: &'static str, now: u32, mark: bool| ColorSwatch().id(id).value(format!("#{now:06x}"))
                .on_change(weak.listener(move |this: &mut Writer, e: &ChangeEvent, w, cx| {
                    if let Some(c) = parse_hex(&e.value) {
                        this.format(move |f| if mark { f.mark = Some(c) } else { f.color = Some(c) });
                    }
                    w.focus_id("root");
                    cx.notify();
                }));
            div().class("row")
                .child(div().class("group").child(div().class("cap").child("Style")).child(styles))
                .child(div().class("group").child(fonts).child(size)
                    .child(tog("bold", "B", fmt.bold, |t| t.toggle(|f| &mut f.bold)))
                    .child(tog("italic", "I", fmt.italic, |t| t.toggle(|f| &mut f.italic)))
                    .child(tog("under", "U", fmt.under, |t| t.toggle(|f| &mut f.under)))
                    .child(tog("strike", "S", fmt.strike, |t| t.toggle(|f| &mut f.strike))))
                .child(div().class("group").child(div().class("cap").child("Ink")).child(swatch("ink", fmt.color.unwrap_or(0x1b1b1b), false))
                    .child(div().class("cap").child("Marker")).child(swatch("marker", fmt.mark.unwrap_or(0xffe58a), true))
                    .child(act("nomark", "Clear", |t| t.format(|f| { f.mark = None; f.color = None; }))))
                .child(div().class("group")
                    .child(tog("al-l", "Left", align == Align::Left, |t| t.paras(|p| p.align = Align::Left)))
                    .child(tog("al-c", "Center", align == Align::Center, |t| t.paras(|p| p.align = Align::Center)))
                    .child(tog("al-r", "Right", align == Align::Right, |t| t.paras(|p| p.align = Align::Right)))
                    .child(tog("al-j", "Justify", align == Align::Justify, |t| t.paras(|p| p.align = Align::Justify))))
                .child(div().class("group")
                    .child(tog("bul", "\u{2022} List", list == List::Bullet,
                               |t| t.paras(|p| p.list = if p.list == List::Bullet { List::None } else { List::Bullet })))
                    .child(tog("num", "1. List", list == List::Number,
                               |t| t.paras(|p| p.list = if p.list == List::Number { List::None } else { List::Number })))
                    .child(act("outdent", "\u{2190}", |t| t.paras(|p| {
                        if p.list == List::None { p.indent = (p.indent - 1.25).max(0.0) } else { p.level = p.level.saturating_sub(1) }
                    })))
                    .child(act("indent", "\u{2192}", |t| t.paras(|p| {
                        if p.list == List::None { p.indent += 1.25 } else { p.level = (p.level + 1).min(4) }
                    }))))
        }
        Tab::Insert => div().class("row")
            .child(div().class("group").child(div().class("cap").child("Table"))
                .child(NumberField().id("rows").class("narrow").min(1.0).max(30.0).step(1.0).value(format!("{}", st.rows))
                    .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, _, cx| {
                        this.s.rows = e.number.round().clamp(1.0, 30.0) as usize;
                        cx.notify();
                    })))
                .child(div().class("cap").child("\u{00d7}"))
                .child(NumberField().id("cols").class("narrow").min(1.0).max(8.0).step(1.0).value(format!("{}", st.cols))
                    .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, _, cx| {
                        this.s.cols = e.number.round().clamp(1.0, 8.0) as usize;
                        cx.notify();
                    })))
                .child(act("ins-table", "Insert table", Writer::insert_table)))
            .child(div().class("group")
                .child(act("ins-break", "Page break", |t| t.enter(true)))
                .child(act("ins-date", "Date", |t| t.type_text("26 November 2026")))
                .child(act("ins-rule", "Divider", |t| t.type_text("\n\u{2014} \u{2014} \u{2014}\n"))))
            .child(div().class("group")
                .child(act("ins-comment", "Comment", |t| t.s.side = Side::Comments))),
        Tab::Layout => {
            let margins = Dropdown().id("margins")
                .value(match st.doc.margins[0] { m if m < 2.0 => "Narrow", m if m > 3.5 => "Wide", _ => "Normal" })
                .child(option().child("Narrow")).child(option().child("Normal")).child(option().child("Wide"))
                .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, w, cx| {
                    let m = [1.27, 2.54, 5.08][(e.number as usize).min(2)];
                    this.edit().margins = [m, m];
                    w.focus_id("root");
                    cx.notify();
                }));
            let orient = Dropdown().id("orient").value(if st.doc.landscape { "Landscape" } else { "Portrait" })
                .child(option().child("Portrait")).child(option().child("Landscape"))
                .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, w, cx| {
                    let land = e.number >= 1.0;
                    if land != this.doc().landscape {
                        this.edit().landscape = land;
                    }
                    w.focus_id("root");
                    cx.notify();
                }));
            div().class("row")
                .child(div().class("group").child(div().class("cap").child("Margins")).child(margins))
                .child(div().class("group").child(div().class("cap").child("Orientation")).child(orient))
                .child(div().class("group").child(div().class("cap").child("Line spacing"))
                    .child(NumberSlider().id("spacing").min(1.0).max(2.5).step(0.05).value(format!("{:.2}", st.doc.spacing))
                        .css("width", "150px")
                        .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, _, cx| {
                            let v = e.number as f32;
                            if (v - this.doc().spacing).abs() > 1e-3 {
                                this.edit().spacing = v;
                                cx.notify();
                            }
                        }))))
                .child(div().class("group").child(div().class("cap").child("Paper"))
                    .child(ColorSwatch().id("paper").value(format!("#{:06x}", st.doc.tint))
                        .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, _, cx| {
                            if let Some(c) = parse_hex(&e.value) {
                                this.edit().tint = c;
                                cx.notify();
                            }
                        }))))
                .child(div().class("group").child(tog("ruler-on", "Ruler", st.ruler, |t| t.s.ruler = !t.s.ruler)))
        }
        Tab::Review => div().class("row")
            .child(div().class("group").child(tog("spell", "Check spelling", st.spelling, |t| t.s.spelling = !t.s.spelling))
                .child(act("fix-all", "Fix all", Writer::fix_spelling)))
            .child(div().class("group")
                .child(tog("open-find", "Find & replace", st.side == Side::Find, |t| t.s.side = Side::Find))
                .child(tog("open-notes", "Comments", st.side == Side::Comments, |t| t.s.side = Side::Comments)))
            .child(div().class("group").child(act("export", "Export .md", Writer::export))),
    }
}

fn find_panel(st: &State, weak: WeakEntity<Writer>) -> Div {
    let hits = st.doc.find(&st.query, st.case);
    let counter = if st.query.is_empty() { "Type a word, press Enter".to_owned() }
                  else if hits.is_empty() { "No matches".to_owned() }
                  else { format!("{} of {}", st.current.min(hits.len() - 1) + 1, hits.len()) };
    let step = |id: &'static str, label: &'static str, by: isize| Button().id(id).child(label)
        .on_click(weak.listener(move |this: &mut Writer, _: &ClickEvent, w, cx| {
            this.find_step(by);
            w.focus_id("root");
            cx.notify();
        }));
    let rep = |id: &'static str, label: &'static str, all: bool| Button().id(id).child(label)
        .on_click(weak.listener(move |this: &mut Writer, _: &ClickEvent, w, cx| {
            this.replace(all);
            w.focus_id("root");
            cx.notify();
        }));
    div().class("side")
        .child(h4().child("Find"))
        .child(TextField().id("find").attr("live", "").placeholder("Find in document").w(relative(1.0)).value(st.query.clone())
            .on_input(weak.listener(|this: &mut Writer, e: &ChangeEvent, _, cx| {
                this.s.query.clone_from(&e.value);
                this.s.current = 0;
                this.find_step(0);
                cx.notify();
            })))
        .child(div().class("line").child(Checkbox().id("case").checked(st.case)
            .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, _, cx| {
                this.s.case = e.checked;
                cx.notify();
            }))).child("Match case").child(div().class("count").css("margin-left", "auto").child(counter)))
        .child(div().class("line").child(step("prev", "Previous", -1)).child(step("next", "Next", 1)))
        .child(h4().child("Replace with"))
        .child(TextField().id("with").placeholder("Replacement").w(relative(1.0)).value(st.with.clone())
            .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, _, cx| {
                this.s.with.clone_from(&e.value);
                cx.notify();
            })))
        .child(div().class("line").child(rep("replace", "Replace", false)).child(rep("replace-all", "Replace all", true)))
}

fn comments_panel(st: &State, weak: WeakEntity<Writer>) -> Div {
    let doc = &st.doc;
    let mut d = div().class("side").child(h4().child("New comment on this paragraph"))
        .child(TextField().id("draft").attr("live", "").placeholder("Write a comment")
            .w(relative(1.0)).value(st.draft.clone())
            .on_input(weak.listener(|this: &mut Writer, e: &ChangeEvent, _, cx| {
                this.s.draft.clone_from(&e.value);
                cx.notify();
            })))
        .child(Button().id("post").child("Post comment").on_click(weak.listener(|this: &mut Writer, _: &ClickEvent, w, cx| {
            let text = std::mem::take(&mut this.s.draft);
            if !text.trim().is_empty() {
                let block = this.s.caret.b;
                this.edit().comments.push(Comment { block, author: "You", text });
                this.s.status = "Comment posted".into();
            }
            w.focus_id("root");
            cx.notify();
        })))
        .child(h4().child(format!("Comments \u{00b7} {}", doc.comments.len())));
    for (k, c) in doc.comments.iter().enumerate() {
        let excerpt: String = doc.para(c.block).map(Para::text).unwrap_or_default().chars().take(60).collect();
        let block = c.block;
        d = d.child(div().id(format!("note{k}")).class("note")
            .on_click(weak.listener(move |this: &mut Writer, _: &ClickEvent, _, cx| {
                this.jump(block);
                cx.notify();
            }))
            .child(div().class("who").child(c.author))
            .child(div().class("quote-of").child(format!("\u{201c}{excerpt}\u{2026}\u{201d}")))
            .child(div().child(c.text.clone()))
            .child(div().child(Button().id(format!("resolve{k}")).child("Resolve")
                .on_click(weak.listener(move |this: &mut Writer, _: &ClickEvent, w, cx| {
                    this.edit().comments.remove(k);
                    this.s.status = "Comment resolved".into();
                    w.focus_id("root");
                    cx.stop_propagation();
                    cx.notify();
                })))));
    }
    d
}

fn status_bar(st: &State, weak: WeakEntity<Writer>) -> Div {
    let (words, chars) = st.doc.words();
    let pages = st.pages.last().map_or(1, |&p| p + 1);
    let page_now = st.pages.get(st.caret.b).map_or(1, |&p| p + 1);
    let typos = st.doc.blocks.iter().filter_map(|b| match b {
        Block::Para(p) => Some(p.text()),
        Block::Table(_) => None,
    }).map(|t| t.split(|c: char| !c.is_alphanumeric()).filter(|w| TYPOS.iter().any(|(bad, _)| w.eq_ignore_ascii_case(bad))).count())
        .sum::<usize>();
    div().class("status")
        .child(div().child(format!("Page {page_now} of {pages}")))
        .child(div().child(format!("{words} words")))
        .child(div().child(format!("{chars} characters")))
        .child(div().child(if typos == 0 { "No spelling issues".to_owned() } else { format!("{typos} spelling issues") }))
        .child(div().class("grow").child(st.status.clone()))
        .child(div().child("Zoom"))
        .child(NumberSlider().id("zoom").class("zoom").min(50.0).max(200.0).step(5.0).value(format!("{}", (st.zoom * 100.0).round()))
            .on_change(weak.listener(|this: &mut Writer, e: &ChangeEvent, _, cx| {
                this.s.zoom = (e.number as f32 / 100.0).clamp(0.5, 2.0);
                cx.notify();
            })))
}

/// The ruler: centimetres over the page, its margins shaded, and the caret paragraph's indents.
fn ruler(st: &State, weak: WeakEntity<Writer>) -> Div {
    let doc = st.doc.clone();
    let (zoom, x0) = (st.zoom, st.ruler_x);
    let para = doc.para(st.caret.b).map(|p| (p.indent, p.first)).unwrap_or_default();
    let width = if doc.landscape { 29.7 } else { 21.0 };
    let paint = canvas(move |b, p| {
        let cm = CM * zoom;
        let (ox, oy, h) = (b.origin.x + x0, b.origin.y, b.size.height);
        p.fill(b, rgb(0x262626));
        p.fill(Bounds { origin: point(ox, oy + 3.0), size: size(width * cm, h - 6.0) }, rgb(0x3d3d3d));
        let [ml, mr] = doc.margins;
        p.fill(Bounds { origin: point(ox + ml * cm, oy + 3.0), size: size((width - ml - mr) * cm, h - 6.0) }, rgb(0xe8e8e8));
        for k in 0..=(width * 4.0) as usize {
            let x = ox + k as f32 * cm / 4.0;
            let tall = if k % 4 == 0 { 7.0 } else if k % 2 == 0 { 4.0 } else { 2.5 };
            p.fill(Bounds { origin: point(x, oy + h - 4.0 - tall), size: size(1.0, tall) }, rgb(0x777777));
            if k % 4 == 0 && k > 0 {
                p.text(x - 3.0, oy + 3.0, &format!("{}", k / 4), 9.0, rgb(0x5a5a5a));
            }
        }
        // the indents: first line (top), the rest (bottom), the right margin
        let tri = |p: &mut Painter, x: f32, up: bool, c: u32| {
            let (y0, y1) = if up { (oy + h - 3.0, oy + h - 10.0) } else { (oy + 3.0, oy + 10.0) };
            p.polygon(&[(x - 5.0, y0), (x + 5.0, y0), (x, y1)], rgb(c));
        };
        tri(p, ox + (ml + para.0 + para.1) * cm, false, 0x4772b3);
        tri(p, ox + (ml + para.0) * cm, true, 0x4772b3);
        tri(p, ox + (width - mr) * cm, true, 0x9a9a9a);
    });
    div().id("ruler").class("ruler").child(paint.css("position", "absolute").css("left", "0").css("top", "0")
        .css("width", "100%").css("height", "100%"))
        .on_pointer_down(weak.listener(|this: &mut Writer, e: &PointerEvent, w, cx| {
            let Some(r) = w.bounds_of("ruler") else { return };
            let x = e.position.x.0 - r.origin.x;
            let cm = CM * this.s.zoom;
            let [ml, mr] = this.doc().margins;
            let width = if this.doc().landscape { 29.7 } else { 21.0 };
            let (ind, first) = this.doc().para(this.s.caret.b).map(|p| (p.indent, p.first)).unwrap_or_default();
            let top = e.position.y.0 - r.origin.y < r.size.height / 2.0;
            let marks = [(Mark::First, ml + ind + first, top), (Mark::Indent, ml + ind, !top),
                         (Mark::RightMargin, width - mr, !top), (Mark::LeftMargin, ml, true)];
            let near = marks.iter().filter(|m| m.2).map(|m| (m.0, ((this.s.ruler_x + m.1 * cm) - x).abs()))
                .filter(|m| m.1 < 9.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|m| m.0)
                .or_else(|| marks.iter().map(|m| (m.0, ((this.s.ruler_x + m.1 * cm) - x).abs())).filter(|m| m.1 < 6.0).map(|m| m.0).next());
            if let Some(mark) = near {
                this.snapshot();
                this.ruler_drag = Some(mark);
                w.set_pointer_capture(cx);
                cx.notify();
            }
        }))
        .on_pointer_move(weak.listener(|this: &mut Writer, e: &PointerEvent, w, cx| {
            if let Some(mark) = this.ruler_drag
                && let Some(r) = w.bounds_of("ruler")
            {
                this.ruler_to(mark, e.position.x.0 - r.origin.x);
                cx.notify();
            }
        }))
        .on_pointer_up(weak.listener(|this: &mut Writer, _: &PointerEvent, w, cx| {
            if this.ruler_drag.take().is_some() {
                w.release_pointer_capture(cx);
                cx.notify();
            }
        }))
}

fn navigator(st: &State, weak: WeakEntity<Writer>) -> Div {
    let mut d = div().class("nav");
    let here = (0..=st.caret.b).rev().find(|&b| st.doc.para(b).is_some_and(|p| p.style.heading()));
    for (b, block) in st.doc.blocks.iter().enumerate() {
        let Block::Para(p) = block else { continue };
        if !p.style.heading() {
            continue;
        }
        let lvl = match p.style {
            Style::Title => "head",
            Style::H1 => "head l1",
            _ => "head l2",
        };
        let mut item = div().id(format!("nav{b}")).class(lvl).child(p.text())
            .on_click(weak.listener(move |this: &mut Writer, _: &ClickEvent, w, cx| {
                this.jump(b);
                w.focus_id("root");
                cx.notify();
            }));
        if here == Some(b) {
            item = item.class("on");
        }
        d = d.child(item);
    }
    let (words, chars) = st.doc.words();
    let read = (words as f32 / 230.0).ceil();
    d.child(div().class("stats").child(format!("{words} words")).child(br()).child(format!("{chars} characters")).child(br())
        .child(format!("{} comments", st.doc.comments.len())).child(br()).child(format!("about {read} min to read")))
}

fn css() -> &'static str {
    static C: OnceLock<String> = OnceLock::new();
    C.get_or_init(|| CSS.to_owned())
}

impl Render for Writer {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        focus_root(w, &mut self.frames);
        // the pages from last frame's heights: a change lays out once more
        if self.paginate(w) && self.paginating < 3 {
            self.paginating += 1;
            w.request_animation_frame();
        } else {
            self.paginating = 0;
        }
        if let (Some(page), Some(r)) = (w.bounds_of("page0"), w.bounds_of("ruler")) {
            self.s.ruler_x = page.origin.x - r.origin.x;
        }
        if std::mem::take(&mut self.follow) {
            w.scroll_into_view("caret", 0.35, None);
        }
        let weak = cx.weak_entity();
        let (fmt, para) = self.current();
        let para = para.map(|p| (p.style, p.align, p.list));
        let st = Arc::new(self.s.clone());
        let (a, b, c) = (st.clone(), st.clone(), st);
        let editors = Editors::new()
            .editor("Document", "TEXT", move |_: &AreaRef| {
                let side = match a.side {
                    Side::Find => find_panel(&a, weak),
                    Side::Comments => comments_panel(&a, weak),
                    Side::None => div(),
                };
                let mut main = div().class("main");
                if a.ruler {
                    main = main.child(ruler(&a, weak));
                }
                Editor::new(main.child(pages_view(&a, weak)))
                    .header(Region::new(tabs(&a, weak)))
                    .tool_header(Region::new(ribbon(&a, weak, fmt, para)).hidden(false).overlap(false))
                    .sidebar(Region::new(side).size(270).hidden(a.side == Side::None).overlap(false))
                    .footer(Region::new(status_bar(&b, weak)).overlap(false))
            })
            .editor("Navigator", "OUTLINER", move |_: &AreaRef| Editor::new(navigator(&c, weak)));
        app().id("root").attr("tabindex", "0")
            .child(style().child(css()))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                // keys typed into a Blender field reach the page too: they are the field's
                if w.text_field_focused() {
                    return;
                }
                if this.key(e, w) {
                    // taken: Blender's own handling (a hovered widget's shortcuts, its undo) stays out
                    cx.prevent_default();
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| this.dragging = false))
            .child(Screen().id("screen").class("screen").editors(editors).layout(AreaSplit::v(0.17, "Navigator", "Document")))
    }
}

// ==========================================================================
// Storyboard

fn storyboard(dir: String) -> Script {
    story(dir, |s| {
        s.play("00-open", &[0, 200, 600]);
        // place the caret in the intro and type
        let w = s.bounds("w2_0");
        s.cx.simulate_click(s.win, w.origin.x + 1.0, w.origin.y + w.size.height / 2.0);
        s.type_text("Field edition. ");
        s.play("01-typed", &[0, 300]);
        // double-click a word, make it bold and marked
        let (x, y) = s.centre("w2_20");
        s.cx.simulate_click(s.win, x, y);
        s.cx.simulate_click(s.win, x, y);
        s.ctrl("b", false);
        s.play("02-bold", &[0, 200]);
        // select across words with shift, italic and underline
        s.cx.simulate_key_with(s.win, "", "Right", true, true);
        s.cx.simulate_key_with(s.win, "", "Right", true, true);
        s.ctrl("i", false);
        s.ctrl("u", false);
        s.play("03-italic", &[0, 200]);
        // headings by key, centre
        s.click("w4_0");
        s.ctrl("2", false);
        s.ctrl("e", false);
        s.play("04-heading", &[0, 300]);
        s.ctrl("z", false);
        s.ctrl("z", false);
        s.play("05-undo", &[0, 200]);
        // a drag selection
        let (ax, ay) = s.centre("w10_0");
        let (bx, by) = s.centre("w10_45");
        s.cx.simulate_drag(s.win, (ax, ay), (bx, by));
        s.play("06-drag-select", &[0, 200]);
        // Insert tab: a table
        s.click("tab-insert");
        s.click("ins-table");
        s.play("07-table", &[0, 300]);
        // Layout: narrow margins, landscape is a dropdown; the ruler's indent
        s.click("tab-layout");
        s.play("08-layout", &[0, 200]);
        // Review: spelling and find & replace
        s.click("tab-review");
        s.click("spell");
        s.play("09-spelling", &[0, 300]);
        s.click("open-find");
        s.play("09b-find-open", &[0, 100, 400]);
        s.click("find");
        s.type_text("light");
        s.key("Return");
        s.play("10-find", &[0, 300]);
        s.click("next");
        s.click("next");
        s.play("11-next", &[0, 300]);
        s.click("with");
        s.type_text("lamp");
        s.key("Return");
        s.click("replace-all");
        s.play("12-replaced", &[0, 300]);
        s.cx.simulate_click(s.win, 700.0, 330.0);
        s.key("Escape");
        // comments: the sidebar slides over to them (frames drive its animation)
        s.click("open-notes");
        s.play("12b-notes-open", &[0, 100, 400]);
        s.click("draft");
        s.type_text("Great section.");
        s.click("post");
        s.play("13-comments", &[0, 300]);
        s.click("fix-all");
        s.play("14-fixed", &[0, 300]);
        // zoom out with the footer slider
        let z = s.bounds("zoom");
        s.cx.simulate_drag(s.win, (z.origin.x + z.size.width * 0.5, z.origin.y + 8.0), (z.origin.x + z.size.width * 0.15, z.origin.y + 8.0));
        s.play("15-zoom", &[0, 300, 800]);
        // the navigator jumps to its last heading
        if let Some(id) = (0..60).rev().map(|b| format!("nav{b}")).find(|id| s.cx.element_bounds(s.win, id).is_some()) {
            s.click(&id);
        }
        s.play("16-jump", &[0, 300]);
        s.click("export");
        s.play("17-export", &[0, 300]);
    })
}

fn main() {
    let (w, h) = size_arg((1440.0, 900.0));
    run_with(w, h, "Folio", Writer::new, arg("--storyboard").map(storyboard));
}

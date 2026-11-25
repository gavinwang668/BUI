//! From an element tree to positioned boxes: cascade, box tree, taffy layout.
//!
//! * Block containers with inline content become wrapping flex lines of word
//!   boxes (so text, links and inline-blocks wrap like a browser's line
//!   boxes); inline elements own their words for painting and hit testing.
//! * Flex and grid containers get text runs as measured, wrapping leaves.
//! * Tables map onto flex rows and columns, lists paint their markers.
//!
//! Everything lives in flat arenas owned by the window and reused every
//! frame: boxes and element data in [`Frame`], computed styles in
//! [`StyleStore`] (shared between identical elements), the taffy tree, text
//! widths and images. Boxes refer to each other by index; nothing is
//! reference-counted.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

mod inline;

use taffy::prelude::*;
use taffy::style::{Direction, Overflow};

use crate::css::{Decl, Prop, Rule, Stylesheet, Subject};
use crate::element::{recycle_children, recycle_decls, AnyElement, Div, Listeners, PaintFn, ParentElement, ReplacedSource, SharedString};
use crate::geometry::{rect, Bounds, Units};
use crate::html::{tag_kind, ua_styled_stylesheet, ua_stylesheet, TagKind};
use crate::paint::{Image, ImageCache, ImageFile};
use crate::prof::{span, Phase};
use crate::style::ContentPart;
use crate::style::{apply, key_hasher, Computed, DisplayKind, KeyMap, KeySet, TextAlign, VAlign, WhiteSpace};
use crate::text::{self, TextCache};
use crate::tree::StyleIx;
use crate::widgets::{WidgetAsk, WidgetSpec};

// ==========================================================================
// Keys

/// An element's identity across frames: a hash of its `.id()`, or of its
/// parent's key, tag and position.
pub(crate) type Key = u64;

fn hash_parts(parts: &[&[u8]]) -> u64 {
    let mut h = key_hasher();
    for p in parts {
        h.write(p);
        h.write(&[0xff]);
    }
    h.finish()
}

/// The key of the element with `.id(id)`.
pub(crate) fn id_key(id: &str) -> Key {
    hash_parts(&[b"#", id.as_bytes()])
}

fn child_key(parent: Key, tag: &str, n: usize) -> Key {
    hash_parts(&[&parent.to_le_bytes(), tag.as_bytes(), &(n as u64).to_le_bytes()])
}

/// Boxes from the root down to one: an event's path (no allocation for a tree this deep).
pub(crate) type Path = smallvec::SmallVec<[usize; 16]>;

/// The key of the `<html>` element.
pub(crate) const ROOT_KEY: Key = 0x68746d6c; // "html"

/// Interaction state the cascade reads (`:hover`, `:active`, `:focus`, `details[open]`).
#[derive(Default)]
pub(crate) struct UiState {
    pub hovered: KeySet<Key>,
    pub active: KeySet<Key>,
    pub focused: Option<Key>,
    /// The focused element and its ancestors (`:focus-within`).
    pub focus_path: KeySet<Key>,
    /// The `.id()` of the focused element.
    pub focused_id: Option<String>,
    pub details_open: KeyMap<Key, bool>,
    /// Popovers and modal dialogs shown (`:popover-open`, `:modal`), by `Window::show_popover`.
    pub popover_open: KeySet<Key>,
    pub scroll: KeyMap<Key, (f32, f32)>,
    pub checked: KeyMap<Key, bool>,
}

// ==========================================================================
// Styles

/// A computed style in a [`StyleStore`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct StyleId(u32);

/// The style everything inherits from at the root.
pub(crate) const ROOT_STYLE: StyleId = StyleId(0);

/// What a computed style depends on (style sharing).
struct StyleKey {
    parent: StyleId,
    viewport: (u32, u32),
    author: u64,
    rules: Vec<usize>,
    decls: Vec<Decl>,
    fixup: u8,
}

/// Computed styles, shared by every element with the same inputs and kept
/// across frames (browsers call this style sharing). Elements refer to them
/// by [`StyleId`].
pub(crate) struct StyleStore {
    styles: Vec<Computed>,
    cache: KeyMap<u64, Vec<(StyleKey, StyleId)>>,
    /// Custom property maps the styles point into (`Computed::vars`).
    vars: crate::vars::Vars,
    /// The author sheet whose `@property` initial values seed the root
    /// style's variables.
    seeded: u64,
    /// Counts the times the store was emptied: a [`StyleId`] means the same style within one epoch.
    epoch: u32,
}

impl Default for StyleStore {
    fn default() -> Self {
        StyleStore { styles: vec![Computed::default()], cache: Default::default(), vars: Default::default(), seeded: 0, epoch: 0 }
    }
}

impl std::ops::Index<StyleId> for StyleStore {
    type Output = Computed;
    fn index(&self, id: StyleId) -> &Computed {
        &self.styles[id.0 as usize]
    }
}

impl StyleStore {
    /// A style one element owns (an animated one's private copy, never in
    /// the cache), to change in place.
    pub(crate) fn private_mut(&mut self, id: StyleId) -> &mut Computed {
        &mut self.styles[id.0 as usize]
    }

    /// Before building a frame: drop everything when the store has grown
    /// large (ids from the previous frame are not used after this).
    fn begin_frame(&mut self) {
        if self.styles.len() > 20_000 {
            self.styles.truncate(1);
            self.cache.clear();
            self.vars.clear();
            self.seeded = 0;
            self.epoch += 1;
        }
    }

    /// `@property --x { initial-value: .. }`: registered properties hold
    /// their initial value until something sets them (`var(--x)` finds it
    /// without a fallback). Seeded into the root style once per sheet.
    fn seed_properties(&mut self, sheet: &Stylesheet, hash: u64) {
        if self.seeded == hash {
            return;
        }
        self.seeded = hash;
        let mut map: Vec<(Prop, Box<[crate::css::Tok]>)> = sheet.properties.iter()
            .filter(|p| p.name.starts_with("--"))
            .filter_map(|p| Some((Prop::custom(&p.name), crate::css::tokenize(p.initial.as_deref()?).into_boxed_slice())))
            .collect();
        map.sort_by_key(|e| e.0);
        map.dedup_by_key(|e| e.0);
        // styles computed against the old root variables are stale
        self.styles.truncate(1);
        self.cache.clear();
        self.vars.clear();
        self.epoch += 1;
        self.styles[0].vars = if map.is_empty() { 0 } else { self.vars.add(map.into_boxed_slice()) };
    }

    fn add(&mut self, c: Computed) -> StyleId {
        self.styles.push(c);
        StyleId((self.styles.len() - 1) as u32)
    }
}

/// The author stylesheet (all `<style>` elements), reparsed only when its text changes.
/// Tailwind classes seen so far, compiled to CSS (like Tailwind's JIT):
/// each distinct `class` attribute is scanned once. A class the page's own
/// CSS styles (`.hidden { ... }`) is the page's, not also a utility.
#[derive(Default)]
pub(crate) struct TwJit {
    on: bool,
    seen: KeyMap<u64, ()>,
    known: KeyMap<String, ()>,
    /// (Tailwind's order, first seen, CSS, class)
    rules: Vec<((u32, u32), usize, String, String)>,
    css: String,
    dirty: bool,
    /// The hash of the page CSS `css` was made for.
    author: u64,
}

/// Whether class names compile as Tailwind utilities (on by default).
pub(crate) static TAILWIND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

impl TwJit {
    fn scan(&mut self, classes: &str) {
        let h = hash_parts(&[classes.as_bytes()]);
        if self.seen.insert(h, ()).is_some() {
            return;
        }
        for c in classes.split_whitespace() {
            if self.known.contains_key(c) {
                continue;
            }
            self.known.insert(c.to_string(), ());
            if let Some(r) = crate::tailwind::compile(c) {
                let n = self.rules.len();
                self.rules.push((r.sort, n, r.css, c.to_string()));
                self.dirty = true;
            }
        }
    }

    /// The utilities, in Tailwind's order, but for the classes `author` (the page's CSS) styles.
    fn css(&mut self, author: &str) -> &str {
        let h = hash_parts(&[author.as_bytes()]);
        if self.dirty || h != self.author {
            self.rules.sort_by_key(|r| (r.0, r.1));
            self.css.clear();
            for r in self.rules.iter().filter(|r| !styles_class(author, &r.3)) {
                self.css.push_str(&r.2);
                self.css.push('\n');
            }
            (self.dirty, self.author) = (false, h);
        }
        &self.css
    }
}

/// Whether CSS `css` has a selector with class `name` (`.name`, not followed by more of a name).
fn styles_class(css: &str, name: &str) -> bool {
    css.match_indices(name).any(|(at, _)| {
        let after = css[at + name.len()..].chars().next();
        css[..at].ends_with('.') && !after.is_some_and(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '\\'))
    })
}

#[derive(Default)]
pub(crate) struct AuthorSheet {
    hash: u64,
    pub(crate) sheet: Stylesheet,
}

// ==========================================================================
// Boxes

/// What a box holds.
pub(crate) enum Content {
    Element,
    /// A word (or pre-formatted line) inside a line container: bytes
    /// `start..end` of the text of box `src`.
    Word { src: u32, start: u32, end: u32, lang: u8 },
    /// A whole text run in a flex/grid container (wraps when measured).
    Text(SharedString),
    LineBreak,
    /// Boxed: a widget's spec is large and few boxes are widgets.
    Widget(Box<WidgetSpec>),
    Replaced(Replaced),
    /// An anonymous line container (inline content between blocks).
    Anon,
    /// The start (`true`) or end edge of its owner, an inline box with margin, border or padding
    /// there: room in a line, where the box's fragment paints its side.
    Edge(bool),
}

pub(crate) enum Replaced {
    Image(Arc<Image>),
    /// `<img src>`: decoded on the kit's workers when shown; animations
    /// play, or hold the frame of the `frame` attribute.
    File(ImageFile, Option<u32>),
    /// `<video>`: its player (yours, or the window's for `src`).
    Video(std::rc::Rc<crate::video::Video>),
    Painted(PaintFn),
    Placeholder(&'static str),
}

/// The element-level data kept from a `Div` after building (moved, not copied).
pub(crate) struct ElData {
    pub tag: &'static str,
    pub id: Option<SharedString>,
    pub attrs: Vec<(SharedString, SharedString)>,
    pub listeners: Option<Box<Listeners>>,
    /// Which of the author sheet's `:has()` hold here (bit = index).
    pub has: u64,
    /// Something answers to this element being pressed or focused: listeners, `.active()` / `.focus()`
    /// refinements or Motion props. Without any (and no `:active` / `:focus` rule in the sheet) a press
    /// changes nothing on screen by itself.
    pub reactive: bool,
    /// `:empty`: no element or text children (rules match before the children are built).
    pub empty: bool,
    /// `dir=auto` (or a `<bdi>` without `dir`): its text's direction, see [`auto_direction`].
    pub auto_dir: Option<bool>,
    /// Answers to the author sheet's `Query::LaterMatches` queries: (query, following siblings
    /// that match its `S`).
    pub later: smallvec::SmallVec<[(u16, u32); 1]>,
}

const NONE: u32 = u32::MAX;

pub(crate) struct BoxNode {
    pub parent: Option<usize>,
    /// Element and text children, linked (words / breaks / line boxes point
    /// at their parent but aren't linked into its list).
    first: u32,
    last: u32,
    next: u32,
    prev: u32,
    pub content: Content,
    /// Index into [`Frame::els`].
    el: u32,
    /// The element this box belongs to (itself, or the inline element of a word).
    pub owner: usize,
    pub style: StyleId,
    pub key: Key,
    /// What selectors can see of this element (its tag, id, attributes,
    /// position and state, chained through its parent and previous sibling):
    /// equal fingerprints match the same rules.
    fp: u64,
    pub taffy: Option<NodeId>,
    pub pos: (usize, usize),
    /// Position among siblings of the same tag, and their count.
    pub tpos: (u32, u32),
    pub marker: Option<String>,
    /// Inside a `styled` subtree: matched against the Blender UA sheet.
    pub styled: bool,
    // filled after layout
    pub bounds: Bounds<f32>,
    pub clip: Option<Bounds<f32>>,
    pub opacity: f32,
    pub visible: bool,
    pub scroll_range: (f32, f32),
    pub scroll_pos: (f32, f32),
    /// This element is one layout leaf measuring its lone text child (that box).
    pub fold: Option<usize>,
    /// This text run is laid out by its parent (`fold`), not by its own node.
    pub folded: bool,
    /// Where layout placed this box in document order (text selection orders
    /// text by it; words are built after the elements around them, so the
    /// arena index is not that order).
    pub seq: u32,
}

/// A laid-out frame. The arenas are reused by the next frame.
#[derive(Default)]
pub(crate) struct Frame {
    pub nodes: Vec<BoxNode>,
    els: Vec<ElData>,
    /// The kinds of event something in this frame listens for (bits of `EventKind::bit`).
    pub listen: u64,
    /// The boxes with a listener that hears the whole window, in document order (usually none).
    pub wide: Vec<u32>,
    pub title: Option<String>,
    by_key: std::cell::OnceCell<KeyMap<Key, usize>>,
    /// Where each box's subtree is, for the pointer's hit test (built on the first one; dropped when
    /// boxes move: [`moved`](Frame::moved)).
    hit_index: std::cell::OnceCell<HitIndex>,
}

/// For every box: the end of its subtree in the arena (boxes come parents first, a subtree is the run
/// after its root), and the rectangle holding the boxes in it. A hit test skips a whole subtree its point
/// is not in. `None` when the arena is not laid out that way (the hit test scans every box then).
struct HitIndex(Option<(Vec<u32>, Vec<Bounds<f32>>)>);

impl HitIndex {
    fn build(nodes: &[BoxNode]) -> Self {
        let n = nodes.len();
        let mut end: Vec<u32> = (1..=n as u32).collect();
        let mut descendants = vec![0u32; n];
        let mut area: Vec<Bounds<f32>> = nodes.iter().map(|b| b.bounds).collect();
        for j in (0..n).rev() {
            let Some(p) = nodes[j].parent else { continue };
            if p >= j {
                return Self(None);
            }
            end[p] = end[p].max(end[j]);
            descendants[p] += 1 + descendants[j];
            let (a, b) = (area[p], area[j]);
            let (x0, y0) = (a.origin.x.min(b.origin.x), a.origin.y.min(b.origin.y));
            let (x1, y1) = ((a.origin.x + a.size.width).max(b.origin.x + b.size.width),
                            (a.origin.y + a.size.height).max(b.origin.y + b.size.height));
            area[p] = rect(x0, y0, x1 - x0, y1 - y0);
        }
        // a subtree is the run after its root: nothing else in it
        if (0..n).any(|i| end[i] - i as u32 - 1 != descendants[i]) {
            return Self(None);
        }
        Self(Some((end, area)))
    }
}

/// Measure contexts of taffy leaves. Compared across frames: an unchanged
/// context keeps the leaf's cached layout.
#[derive(PartialEq)]
pub(crate) enum Measure {
    /// Text of the box registered under `key` this frame (already
    /// whitespace-processed and transformed); `hash` identifies the text.
    Text { key: Key, hash: u64, size: f32, flags: i32, line: f32, nowrap: bool, brk: u8 },
    /// One word (nothing to wrap): measured when built. Compared by size, not text, so a new
    /// word of the same width keeps the cached layout of the leaf and of its ancestors.
    Word { width: f32, line: f32 },
    /// A textarea with `field-sizing: content`: its height is asked of the kit at the width it gets;
    /// `hash` identifies the text.
    Area { key: Key, hash: u64 },
    /// A replaced element (image, video, canvas) of this natural size: a size given in one axis
    /// gives the other through its ratio.
    Replaced { width: f32, height: f32 },
}

/// The computed style a taffy style was made from, and how: boxes with the same shape have the
/// same taffy style, so they share one.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Shape {
    epoch: u32,
    style: StyleId,
    form: Form,
}

/// How the taffy style of a box follows from its computed style.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Form {
    Plain,
    /// A box with its lone text: one block leaf.
    Folded,
    /// A block container of line boxes.
    Lines,
}

/// A taffy style as a box makes it.
enum Made {
    Shared(StyleIx),
    Own(Style),
}

/// The styles a layout tree holds before it first drops those no node uses.
const STYLES_MAX: usize = 4096;

/// The persistent taffy tree's nodes by box key (with the frame they were
/// last used in). Reconciling instead of rebuilding lets taffy's cache skip
/// every subtree whose style, content and children didn't change.
#[derive(Default)]
struct Recon {
    map: KeyMap<Key, (NodeId, u32)>,
    stamp: u32,
    /// Nodes for duplicate keys this frame: removed next frame.
    extra: Vec<NodeId>,
    /// The root and viewport (bits) of the last computed layout: with nothing changed since,
    /// the layouts in the tree are still current and taffy is not run.
    laid: Option<(NodeId, (u32, u32))>,
}

/// Everything layout keeps across frames, owned by the window.
pub(crate) struct LayoutCaches {
    pub styles: StyleStore,
    pub text: TextCache,
    pub images: ImageCache,
    pub author: AuthorSheet,
    /// The author sheet has a `:hover`, `:active` or `:focus` rule.
    pub state_rules: bool,
    pub(crate) taffy: crate::tree::LayoutTree<Measure>,
    /// Box index of each taffy node (by slot), rebuilt in place each frame.
    box_of: Vec<u32>,
    /// Scratch for `finish`.
    acc: Vec<Option<Bounds<f32>>>,
    /// Matched rules by element fingerprint (+ viewport): selector matching
    /// runs once per distinct element, not every frame.
    matched: MatchCache,
    recon: Recon,
    /// Measured text leaves: key -> box index, this frame.
    text_box: KeyMap<Key, usize>,
    /// The table entry of each shape's taffy style.
    shapes: KeyMap<Shape, StyleIx>,
    /// The style count that compacts the table next: twice what the last
    /// compaction kept (at least `STYLES_MAX`), so live styles never compact every frame.
    styles_limit: usize,
    /// Content widths of blocks with `overflow-wrap: anywhere` from the last
    /// layout: their long words are split to fit before layout.
    wrap_widths: KeyMap<Key, f32>,
    tw: TwJit,
    /// `@container`: query containers' content sizes from the last layout.
    containers: Containers,
    /// Anchor positioning: anchors and containing blocks from the last layout.
    anchors: crate::anchor::Anchors,
    /// CSS animations and transitions by element key.
    pub(crate) anim: crate::anim::Animations,
    /// The viewport of the last layout.
    viewport: (f32, f32),
}

impl LayoutCaches {
    /// After a layout: record query containers' sizes. True when the
    /// window must render again (an `@container` query may match differently).
    pub(crate) fn after_layout(&mut self, frame: &Frame) -> bool {
        let queried = self.author.sheet.has_container_queries();
        let mut again = self.containers.update(frame, &self.styles, queried);
        again |= self.anchors.update(frame, &self.styles, self.viewport);
        // overflow-wrap: blocks whose width changed lay out their words again
        for n in &frame.nodes {
            let c = &self.styles[n.style];
            if c.wrap_anywhere && n.taffy.is_some() && !c.display.is_inline_level()
                && matches!(n.content, Content::Element) {
                let w = crate::paint::content_box(c, n.bounds).size.width;
                if self.wrap_widths.get(&n.key).is_none_or(|old| (old - w).abs() > 0.5) {
                    self.wrap_widths.insert(n.key, w);
                    again = true;
                }
            }
        }
        if self.wrap_widths.len() > 4096 {
            self.wrap_widths.clear();
        }
        again
    }
}

/// Query containers by element key: content-box size, `container-type`
/// and names, from the previous layout (queries see last frame's sizes;
/// a change re-renders once more).
#[derive(Default)]
pub(crate) struct Containers {
    map: KeyMap<Key, (f32, f32, u8, Option<Box<str>>)>,
    /// Hash of `map`, part of the selector-match memo.
    generation: u64,
    /// Consecutive re-renders caused by container size changes.
    chain: u8,
}

impl Containers {
    /// After a layout: record the containers; true when a re-render is needed.
    fn update(&mut self, frame: &Frame, styles: &StyleStore, queried: bool) -> bool {
        let mut map: KeyMap<Key, (f32, f32, u8, Option<Box<str>>)> = KeyMap::default();
        for n in &frame.nodes {
            let c = &styles[n.style];
            if c.container_type != 0 && n.taffy.is_some() {
                let b = crate::paint::content_box(c, n.bounds);
                map.insert(n.key, (b.size.width, b.size.height, c.container_type, c.container_name.clone()));
            }
        }
        let mut h = key_hasher();
        let mut keys: Vec<&Key> = map.keys().collect();
        keys.sort_unstable();
        for k in keys {
            let (w, hh, t, _) = &map[k];
            (k, w.to_bits(), hh.to_bits(), t).hash(&mut h);
        }
        let generation = h.finish();
        let changed = generation != self.generation;
        self.map = map;
        self.generation = generation;
        // a query may now match differently: render again (bounded, in case
        // the result feeds back into the container's own size)
        if changed && queried && self.chain < 3 {
            self.chain += 1;
            return true;
        }
        self.chain = 0;
        false
    }
}

/// Rule indices (`Rule::order`) in the UA and author sheets.
type RuleIds = smallvec::SmallVec<[u32; 8]>;

#[derive(Default)]
struct MatchCache {
    map: KeyMap<u64, (RuleIds, RuleIds)>,
    author: u64,
}

impl Default for LayoutCaches {
    fn default() -> Self {
        LayoutCaches {
            styles: StyleStore::default(),
            text: TextCache::default(),
            images: ImageCache::default(),
            author: AuthorSheet::default(),
            state_rules: false,
            taffy: Default::default(),
            box_of: Vec::new(),
            acc: Vec::new(),
            matched: MatchCache::default(),
            recon: Recon::default(),
            text_box: KeyMap::default(),
            shapes: KeyMap::default(),
            styles_limit: STYLES_MAX,
            wrap_widths: KeyMap::default(),
            tw: TwJit::default(),
            containers: Containers::default(),
            anchors: Default::default(),
            anim: Default::default(),
            viewport: (0.0, 0.0),
        }
    }
}

struct Builder<'a> {
    nodes: Vec<BoxNode>,
    els: Vec<ElData>,
    taffy: &'a mut crate::tree::LayoutTree<Measure>,
    styles: &'a mut StyleStore,
    text: &'a mut TextCache,
    images: &'a mut ImageCache,
    matched: &'a mut MatchCache,
    recon: &'a mut Recon,
    text_box: &'a mut KeyMap<Key, usize>,
    shapes: &'a mut KeyMap<Shape, StyleIx>,
    /// Set when a node changed while building the current subtree.
    changed: bool,
    /// `tpos` for the element about to be built.
    next_tpos: (u32, u32),
    author: &'a Stylesheet,
    /// Blender's `styled` UA sheet as it was when this pass began (the pass holds it, see
    /// [`ua_styled_stylesheet`]).
    ua_styled: &'a Stylesheet,
    author_hash: u64,
    containers: &'a Containers,
    anchors: &'a crate::anchor::Anchors,
    /// Media query results of the UA and author sheets this frame.
    ua_media: Vec<bool>,
    ua_styled_media: Vec<bool>,
    au_media: Vec<bool>,
    state: &'a UiState,
    viewport: (f32, f32),
    title: Option<String>,
    widgets: &'a mut dyn FnMut(Key, WidgetAsk) -> (f32, f32),
    /// CSS counters in scope: name, value, and the box whose subtree the
    /// instance is scoped to (dropped when that box is finished).
    counters: Vec<(Box<str>, i32, usize)>,
    /// Column widths of each table (by box), computed before its cells.
    table_cols: KeyMap<usize, TableCols>,
    wrap_widths: &'a KeyMap<Key, f32>,
    anim: &'a mut crate::anim::Animations,
    /// The box being built made a container of inline content (its node lays out in lines).
    inlined: bool,
    /// Run-in boxes taken in by the block after them (block -> run-in, `adopt_run_ins`).
    run_ins: KeyMap<usize, usize>,
}

/// A table's columns: widths in px, and whether the table sizes to them.
struct TableCols {
    widths: Vec<f32>,
    /// the table's width is auto: it shrinks to its columns
    fit: bool,
    fixed: bool,
}

/// Child box indices / layout nodes being assembled: on the stack for the usual few.
type Kids = smallvec::SmallVec<[usize; 8]>;
type NodeList = smallvec::SmallVec<[NodeId; 8]>;

/// The slot index of a layout node (dense: the tree reuses freed slots).
/// The declarations for both passes of `compute`, which apply the normal ones and then the
/// `!important` ones, each in list order. The normal ones go UA, author (rules in cascade order),
/// then the element's own; the important ones reverse the origins and the layers (CSS Cascade 5):
/// author unlayered, author layers from the last to the first, the element's own, then UA.
fn cascade_order<'d>(ua: &[&'d Rule], au: &[&'d Rule], own: &[&'d Decl]) -> smallvec::SmallVec<[&'d Decl; 24]> {
    let rules = || ua.iter().chain(au).flat_map(|r| r.decls.iter());
    let mut out: smallvec::SmallVec<[&'d Decl; 24]> = rules().chain(own.iter().copied()).filter(|d| !d.important).collect();
    // `revert-layer` and `revert` roll a property back to what the layers below (the user agent
    // origin) give it (CSS Cascade 5 7.3): to the latest such declaration in cascade order; with
    // none, the keyword stays (it acts as `unset`)
    let keyword = |d: &Decl| match &*d.value {
        [crate::css::Tok::Ident(k)] if k.as_ref() == "revert-layer" => Some(true),
        [crate::css::Tok::Ident(k)] if k.as_ref() == "revert" => Some(false),
        _ => None,
    };
    if out.iter().any(|d| keyword(d).is_some()) {
        // each normal declaration's rank: the user agent's lowest, then the author's by layer
        // (unlayered and inline last)
        let rank: smallvec::SmallVec<[u32; 24]> = ua.iter().flat_map(|r| r.decls.iter().filter(|d| !d.important).map(|_| 0))
            .chain(au.iter().flat_map(|r| r.decls.iter().filter(|d| !d.important).map(move |_| 1 + u32::from(r.layer))))
            .chain(own.iter().filter(|d| !d.important).map(|_| 2 + u32::from(u16::MAX)))
            .collect();
        const ALL: Prop = Prop::named("all");
        let mut rolled: smallvec::SmallVec<[&'d Decl; 24]> = smallvec::SmallVec::with_capacity(out.len());
        // each declaration as it resolved (a rolled-back one: what it rolled back to)
        let mut resolved: smallvec::SmallVec<[&'d Decl; 24]> = smallvec::SmallVec::with_capacity(out.len());
        for k in 0..out.len() {
            let Some(layer) = keyword(out[k]) else {
                rolled.push(out[k]);
                resolved.push(out[k]);
                continue;
            };
            let below = |j: usize| if layer { rank[j] < rank[k] } else { rank[j] == 0 };
            if out[k].prop == ALL {
                // every property: reset (as `unset`), then what the layers below say again
                rolled.push(out[k]);
                rolled.extend((0..k).filter(|&j| below(j) && keyword(resolved[j]).is_none()).map(|j| resolved[j]));
                resolved.push(out[k]);
            } else {
                let j = (0..k).rev().find(|&j| out[j].prop == out[k].prop && below(j));
                let to = j.map_or(out[k], |j| resolved[j]);
                rolled.push(to);
                resolved.push(to);
            }
        }
        out = rolled;
    }
    let important = |d: &&Decl| d.important;
    if !rules().chain(own.iter().copied()).any(|d| d.important) {
        return out;
    }
    let mut layered: smallvec::SmallVec<[&'d Rule; 8]> = smallvec::SmallVec::new();
    for r in au {
        if r.layer == u16::MAX {
            out.extend(r.decls.iter().filter(important));
        } else {
            layered.push(r);
        }
    }
    layered.sort_by_key(|r| std::cmp::Reverse(r.layer)); // stable: specificity and order stay
    out.extend(layered.iter().flat_map(|r| r.decls.iter()).filter(important));
    out.extend(own.iter().copied().filter(important));
    out.extend(ua.iter().flat_map(|r| r.decls.iter()).filter(important));
    out
}

fn slot(id: NodeId) -> usize {
    u64::from(id) as usize
}

/// Children of box `i` in the arena `nodes`.
pub(crate) fn kids(nodes: &[BoxNode], i: usize) -> impl Iterator<Item = usize> + '_ {
    let mut k = nodes[i].first;
    std::iter::from_fn(move || {
        (k != NONE).then(|| {
            let cur = k as usize;
            k = nodes[cur].next;
            cur
        })
    })
}

fn el_attr<'a>(nodes: &[BoxNode], els: &'a [ElData], i: usize, name: &str) -> Option<&'a str> {
    let e = nodes[i].el;
    if e == NONE {
        return None;
    }
    els[e as usize].attrs.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.as_ref())
}

// -- selector subject -------------------------------------------------------------------

#[derive(Clone)]
struct NodeRef<'b> {
    nodes: &'b [BoxNode],
    els: &'b [ElData],
    state: &'b UiState,
    containers: &'b Containers,
    i: usize,
}

impl NodeRef<'_> {
    /// A `<textarea>` or an `<input>` of a type one types text into (where `readonly` and
    /// `placeholder` apply).
    fn text_field(&self) -> bool {
        match self.tag() {
            "textarea" => true,
            "input" => matches!(self.own_attr("type").map(str::to_ascii_lowercase).as_deref(),
                None | Some("" | "text" | "search" | "url" | "tel" | "email" | "password" | "number"
                    | "date" | "month" | "week" | "time" | "datetime-local")),
            _ => false,
        }
    }

    /// A control `required` applies to: `<select>`, `<textarea>`, an `<input>` other than a button,
    /// a hidden one, a range or a colour.
    fn requirable(&self) -> bool {
        match self.tag() {
            "select" | "textarea" => true,
            "input" => !matches!(self.own_attr("type").map(str::to_ascii_lowercase).as_deref(),
                Some("hidden" | "range" | "color" | "submit" | "image" | "reset" | "button")),
            _ => false,
        }
    }

    fn el(&self) -> Option<&ElData> {
        let e = self.nodes[self.i].el;
        (e != NONE).then(|| &self.els[e as usize])
    }
    fn own_attr(&self, name: &str) -> Option<&str> {
        el_attr(self.nodes, self.els, self.i, name)
    }
    /// A `::before` / `::after` box (invisible to sibling selectors).
    fn is_generated(&self, k: usize) -> bool {
        let e = self.nodes[k].el;
        e != NONE && self.els[e as usize].tag.starts_with("::")
    }
}

impl Subject for NodeRef<'_> {
    fn container(&self, name: Option<&str>) -> Option<(f32, f32)> {
        let mut cur = self.nodes[self.i].parent;
        while let Some(p) = cur {
            if let Some((w, h, ty, names)) = self.containers.map.get(&self.nodes[p].key) {
                let named = name.is_none_or(|n| names.as_deref().is_some_and(|s| s.split(' ').any(|x| x == n)));
                if named {
                    // an inline-size container has no queryable height
                    return Some((*w, if *ty == 2 { *h } else { f32::NAN }));
                }
            }
            cur = self.nodes[p].parent;
        }
        None
    }
    fn tag(&self) -> &str {
        self.el().map(|e| e.tag).unwrap_or("")
    }
    fn id(&self) -> Option<&str> {
        self.el()?.id.as_deref()
    }
    fn has_class(&self, class: &str) -> bool {
        self.own_attr("class").is_some_and(|c| c.split_whitespace().any(|w| w == class))
    }
    fn attr(&self, name: &str) -> Option<&str> {
        let n = &self.nodes[self.i];
        if name == "open" && self.tag() == "details" {
            return self.state.details_open.get(&n.key).copied()
                .unwrap_or(self.own_attr("open").is_some()).then_some("");
        }
        self.own_attr(name)
    }
    fn state(&self, pseudo: &str) -> bool {
        let key = self.nodes[self.i].key;
        match pseudo {
            "hover" => self.state.hovered.contains(&key),
            "active" => self.state.active.contains(&key),
            "focus" | "focus-visible" => self.state.focused == Some(key),
            "focus-within" => self.state.focus_path.contains(&key),
            "checked" => self.state.checked.get(&key).copied().unwrap_or(self.own_attr("checked").is_some()),
            "disabled" => self.own_attr("disabled").is_some(),
            "enabled" => self.own_attr("disabled").is_none(),
            "link" | "any-link" => self.tag() == "a" && self.own_attr("href").is_some(),
            // the form pseudo-classes are of the controls they apply to (HTML 4.16.3)
            "required" => self.requirable() && self.own_attr("required").is_some(),
            "optional" => self.requirable() && self.own_attr("required").is_none(),
            "read-write" => self.text_field() && self.own_attr("readonly").is_none() && self.own_attr("disabled").is_none(),
            "read-only" => !(self.text_field() && self.own_attr("readonly").is_none() && self.own_attr("disabled").is_none()),
            "open" => self.attr("open").is_some(),
            "popover-open" => self.own_attr("popover").is_some() && self.state.popover_open.contains(&key),
            // an open dialog shown modally: in the top layer as popovers are
            "modal" => self.tag() == "dialog" && self.own_attr("open").is_some() && self.state.popover_open.contains(&key),
            "placeholder-shown" => {
                self.text_field() && self.own_attr("placeholder").is_some() && self.own_attr("value").is_none_or(str::is_empty)
            }
            _ => false,
        }
    }
    fn parent(&self) -> Option<Self> {
        self.nodes[self.i].parent.map(|p| NodeRef { i: p, ..self.clone() })
    }
    fn prev_sibling(&self) -> Option<Self> {
        let mut k = self.nodes[self.i].prev;
        while k != NONE {
            if self.nodes[k as usize].el != NONE && !self.is_generated(k as usize) {
                return Some(NodeRef { i: k as usize, ..self.clone() });
            }
            k = self.nodes[k as usize].prev;
        }
        None
    }
    fn position(&self) -> (usize, usize) {
        self.nodes[self.i].pos
    }
    fn type_position(&self) -> (usize, usize) {
        let t = self.nodes[self.i].tpos;
        (t.0 as usize, t.1 as usize)
    }
    fn is_empty(&self) -> bool {
        self.el().is_none_or(|e| e.empty)
    }
    fn auto_dir(&self) -> Option<bool> {
        self.el()?.auto_dir
    }
    fn has(&self, id: u16) -> bool {
        id < 64 && self.el().is_some_and(|e| e.has & (1 << id) != 0)
    }
    fn later_matches(&self, id: u16) -> Option<usize> {
        self.el()?.later.iter().find(|q| q.0 == id).map(|q| q.1 as usize)
    }
    fn same(&self, o: &Self) -> bool {
        self.i == o.i
    }
    fn next_sibling(&self) -> Option<Self> {
        let mut k = self.nodes[self.i].next;
        while k != NONE {
            if self.nodes[k as usize].el != NONE && !self.is_generated(k as usize) {
                return Some(NodeRef { i: k as usize, ..self.clone() });
            }
            k = self.nodes[k as usize].next;
        }
        None
    }
    fn first_child(&self) -> Option<Self> {
        let k = self.nodes[self.i].first;
        if k == NONE {
            return None;
        }
        let c = NodeRef { i: k as usize, ..self.clone() };
        if self.nodes[k as usize].el != NONE && !self.is_generated(k as usize) { Some(c) } else { c.next_sibling() }
    }
}

/// An element of a not-yet-built subtree (for `:has()`): its tag, id,
/// attributes and tree links, flattened from the declarative children.
struct Flat<'d> {
    tag: &'static str,
    id: Option<&'d str>,
    attrs: &'d [(SharedString, SharedString)],
    parent: u32,
    first: u32,
    next: u32,
    prev: u32,
    pos: (usize, usize),
    tpos: (usize, usize),
    key: Key,
}

/// (same-tag siblings before `k`, same-tag siblings in all) for `:has()`
/// subtrees (small: linear scans are fine there).
fn type_counts(els: &[&Div], k: usize) -> (usize, usize) {
    let tag = els[k].tag;
    let before = els[..k].iter().filter(|e| e.tag == tag).count();
    (before, before + els[k..].iter().filter(|e| e.tag == tag).count())
}

/// The element children of `div` (and theirs) under flat entry `me`, whose key is `key`.
fn flatten_kids<'d>(div: &'d Div, me: u32, key: Key, out: &mut Vec<Flat<'d>>) {
    let els: smallvec::SmallVec<[&'d Div; 16]> = div.children.iter().filter_map(|c| match c {
        AnyElement::Element(d) => Some(d),
        AnyElement::Text(_) => None,
    }).collect();
    let mut prev = NONE;
    for (k, d) in els.iter().enumerate() {
        let ck = match &d.id {
            Some(id) => id_key(id),
            None => child_key(key, d.tag, k + 1),
        };
        let (before, total) = type_counts(&els, k);
        let idx = out.len() as u32;
        out.push(Flat { tag: d.tag, id: d.id.as_deref(), attrs: &d.attrs, parent: me, first: NONE, next: NONE,
                        prev, pos: (k + 1, els.len()), tpos: (before + 1, total), key: ck });
        if prev == NONE {
            out[me as usize].first = idx;
        } else {
            out[prev as usize].next = idx;
        }
        prev = idx;
        flatten_kids(d, idx, ck, out);
    }
}

/// The element siblings after the element at flat entry `me` (`pos` among its siblings): the
/// first `take` of `later`, each with its subtree, chained after `me` under `parent`. Keys are
/// the ones `children` gives them; `built` counts the element and the siblings before it by tag,
/// for their type positions. Returns the first sibling's entry (`NONE` without any).
#[allow(clippy::too_many_arguments)]
fn flatten_later<'d>(later: &'d [AnyElement], take: usize, me: u32, pos: (usize, usize), parent: u32,
                     parent_key: Key, built: &[(&str, usize)], out: &mut Vec<Flat<'d>>) -> u32 {
    let els: smallvec::SmallVec<[&'d Div; 16]> = later.iter().filter_map(|c| match c {
        AnyElement::Element(d) => Some(d),
        AnyElement::Text(_) => None,
    }).collect();
    let count = |tag: &str| built.iter().find(|b| b.0.eq_ignore_ascii_case(tag)).map_or(0, |b| b.1);
    let (first, mut prev) = (out.len() as u32, me);
    for (k, d) in els.iter().enumerate().take(take) {
        let ck = match &d.id {
            Some(id) => id_key(id),
            None => child_key(parent_key, d.tag, pos.0 + k + 1),
        };
        let (before, total) = type_counts(&els, k);
        let idx = out.len() as u32;
        let tpos = (count(d.tag) + before + 1, count(d.tag) + total);
        out.push(Flat { tag: d.tag, id: d.id.as_deref(), attrs: &d.attrs, parent, first: NONE, next: NONE,
                        prev, pos: (pos.0 + k + 1, pos.1), tpos, key: ck });
        out[prev as usize].next = idx;
        prev = idx;
        flatten_kids(d, idx, ck, out);
    }
    if out.len() as u32 == first { NONE } else { first }
}

/// An inline `<svg>` as SVG markup (its elements, attributes and text) for the kit's SVG reader.
fn svg_markup(div: &Div) -> String {
    fn put(d: &Div, out: &mut String) {
        let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
        out.push('<');
        out.push_str(d.tag);
        if d.tag == "svg" {
            out.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
        }
        if let Some(id) = &d.id {
            out.push_str(&format!(" id=\"{}\"", esc(id)));
        }
        for (k, v) in &d.attrs {
            out.push_str(&format!(" {}=\"{}\"", k, esc(v)));
        }
        out.push('>');
        for c in &d.children {
            match c {
                AnyElement::Element(e) => put(e, out),
                AnyElement::Text(t) => out.push_str(&esc(t)),
            }
        }
        out.push_str("</");
        out.push_str(d.tag);
        out.push('>');
    }
    let mut out = String::new();
    put(div, &mut out);
    out
}

/// HTML's auto directionality of `div` when it has `dir=auto` (or is a `<bdi>` without `dir`):
/// the direction of the first strong character of its text (an `<input>`'s value), skipping
/// elements with their own `dir` and `bdi`, `script`, `style` and `textarea` subtrees.
fn auto_direction(div: &Div) -> Option<bool> {
    fn walk(kids: &[AnyElement]) -> Option<bool> {
        kids.iter().find_map(|c| match c {
            AnyElement::Text(t) => text::first_strong(t),
            AnyElement::Element(d) if matches!(d.tag, "bdi" | "script" | "style" | "textarea")
                || d.attrs.iter().any(|a| a.0.as_ref() == "dir") => None,
            AnyElement::Element(d) => walk(&d.children),
        })
    }
    let attr = |name: &str| div.attrs.iter().find(|a| a.0.as_ref() == name).map(|a| a.1.as_ref());
    let auto = match attr("dir") {
        Some(d) => d.eq_ignore_ascii_case("auto"),
        None => div.tag == "bdi",
    };
    if !auto {
        return None;
    }
    if div.tag == "input" {
        return attr("value").and_then(text::first_strong);
    }
    walk(&div.children)
}

#[derive(Clone)]
struct DivSubject<'f, 'd> {
    flat: &'f [Flat<'d>],
    state: &'f UiState,
    i: usize,
}

impl DivSubject<'_, '_> {
    fn at(&self, k: u32) -> Option<Self> {
        (k != NONE).then(|| DivSubject { i: k as usize, ..self.clone() })
    }
}

impl Subject for DivSubject<'_, '_> {
    fn tag(&self) -> &str {
        self.flat[self.i].tag
    }
    fn id(&self) -> Option<&str> {
        self.flat[self.i].id
    }
    fn has_class(&self, class: &str) -> bool {
        self.attr("class").is_some_and(|c| c.split_whitespace().any(|w| w == class))
    }
    fn attr(&self, name: &str) -> Option<&str> {
        self.flat[self.i].attrs.iter().find(|(k, _)| k.as_ref() == name).map(|(_, v)| v.as_ref())
    }
    fn state(&self, pseudo: &str) -> bool {
        let key = self.flat[self.i].key;
        match pseudo {
            "hover" => self.state.hovered.contains(&key),
            "active" => self.state.active.contains(&key),
            "focus" | "focus-visible" => self.state.focused == Some(key),
            "focus-within" => self.state.focus_path.contains(&key),
            "checked" => self.state.checked.get(&key).copied().unwrap_or(self.attr("checked").is_some()),
            "disabled" => self.attr("disabled").is_some(),
            "enabled" => self.attr("disabled").is_none(),
            "required" => self.attr("required").is_some(),
            "link" | "any-link" => self.tag() == "a" && self.attr("href").is_some(),
            _ => false,
        }
    }
    fn parent(&self) -> Option<Self> {
        self.at(self.flat[self.i].parent)
    }
    fn prev_sibling(&self) -> Option<Self> {
        self.at(self.flat[self.i].prev)
    }
    fn next_sibling(&self) -> Option<Self> {
        self.at(self.flat[self.i].next)
    }
    fn first_child(&self) -> Option<Self> {
        self.at(self.flat[self.i].first)
    }
    fn position(&self) -> (usize, usize) {
        self.flat[self.i].pos
    }
    fn type_position(&self) -> (usize, usize) {
        self.flat[self.i].tpos
    }
    fn is_empty(&self) -> bool {
        self.flat[self.i].first == NONE
    }
    fn same(&self, o: &Self) -> bool {
        self.i == o.i
    }
}

// -- building -------------------------------------------------------------------------------

fn collect_styles(el: &AnyElement, css: &mut String, title: &mut Option<String>, jit: &mut TwJit) {
    let AnyElement::Element(d) = el else { return };
    if jit.on
        && let Some((_, classes)) = d.attrs.iter().find(|(k, _)| k.as_ref() == "class")
    {
        jit.scan(classes);
    }
    let text = || d.children.iter().filter_map(|c| match c {
        AnyElement::Text(t) => Some(t.as_ref()),
        _ => None,
    }).collect::<String>();
    match d.tag {
        "style" => css.push_str(&text()),
        "title" if title.is_none() => *title = Some(text()),
        _ => {}
    }
    for c in &d.children {
        collect_styles(c, css, title, jit);
    }
}

impl<'a> Builder<'a> {
    /// Element `i` and its built siblings before it, counted by tag (type positions of the
    /// siblings after it).
    fn built_tags(&self, i: usize) -> smallvec::SmallVec<[(&str, usize); 8]> {
        let mut out: smallvec::SmallVec<[(&str, usize); 8]> = smallvec::SmallVec::new();
        let mut cur = Some(self.subject(i));
        while let Some(s) = cur {
            let tag = self.els[self.nodes[s.i].el as usize].tag;
            match out.iter_mut().find(|t| t.0.eq_ignore_ascii_case(tag)) {
                Some(t) => t.1 += 1,
                None => out.push((tag, 1)),
            }
            cur = s.prev_sibling();
        }
        out
    }

    /// Answers to the author sheet's `Query::LaterMatches` queries (`:nth-last-child(An+B of S)`)
    /// for element `i` at `pos`: how many of its following siblings `later` match `S`. They are
    /// matched on the declarative elements, under `i`'s built ancestors (`S` may name them); only
    /// queries whose `S` the element matches itself are answered.
    fn later_matches<'s>(&'s self, i: usize, pos: (usize, usize), later: &'s [AnyElement])
        -> smallvec::SmallVec<[(u16, u32); 1]> {
        let mut out = smallvec::SmallVec::new();
        let me = self.subject(i);
        let mut queries = self.author.has.iter().enumerate().filter(|(_, q)| {
            q.kind == crate::css::Query::LaterMatches && q.rels.iter().any(|r| r.sel.matches(&me))
        }).peekable();
        if queries.peek().is_none() {
            return out;
        }
        // the built ancestors from the root down, then `i` itself
        let mut chain: smallvec::SmallVec<[usize; 16]> = smallvec::SmallVec::new();
        let mut n = Some(i);
        while let Some(k) = n {
            if self.nodes[k].el != NONE {
                chain.push(k);
            }
            n = self.nodes[k].parent;
        }
        let mut flat: Vec<Flat<'s>> = Vec::with_capacity(chain.len() + later.len());
        for &k in chain.iter().rev() {
            let (node, el) = (&self.nodes[k], &self.els[self.nodes[k].el as usize]);
            let idx = flat.len() as u32;
            let parent = idx.checked_sub(1).unwrap_or(NONE);
            flat.push(Flat { tag: el.tag, id: el.id.as_deref(), attrs: &el.attrs, parent, first: NONE, next: NONE,
                             prev: NONE, pos: node.pos, tpos: self.subject(k).type_position(), key: node.key });
            if parent != NONE {
                flat[parent as usize].first = idx;
            }
        }
        let at = flat.len() as u32 - 1;
        let parent = flat[at as usize].parent;
        let parent_key = self.nodes[i].parent.map_or(self.nodes[i].key, |p| self.nodes[p].key);
        let built = self.built_tags(i);
        let first = flatten_later(later, usize::MAX, at, pos, parent, parent_key, &built, &mut flat);
        for (id, q) in queries {
            let mut count = 0u32;
            let mut k = first;
            while k != NONE {
                let s = DivSubject { flat: &flat, state: self.state, i: k as usize };
                count += u32::from(q.rels.iter().any(|r| r.sel.matches(&s)));
                k = flat[k as usize].next;
            }
            out.push((id as u16, count));
        }
        out
    }

    fn subject(&self, i: usize) -> NodeRef<'_> {
        NodeRef { nodes: &self.nodes, els: &self.els, state: self.state, containers: self.containers, i }
    }

    fn st(&self, i: usize) -> &Computed {
        &self.styles[self.nodes[i].style]
    }

    /// `<video src>`'s player: autoplay with sound unless `autoplay="false"`
    /// (React's `autoPlay={false}`); `loop`, `muted`, `poster`, `audioeffects` (the HTML attributes).
    fn video_src(&mut self, i: usize) -> Option<std::rc::Rc<crate::video::Video>> {
        let attrs = crate::video::VideoAttrs {
            autoplay: self.attr(i, "autoplay").or_else(|| self.attr(i, "autoPlay")) != Some("false"), // React: autoPlay
            looped: self.attr(i, "loop").is_some_and(|v| v != "false"),
            muted: self.attr(i, "muted").is_some_and(|v| v != "false"),
            audio_effects: self.attr(i, "audioeffects").is_some_and(|v| v != "false"),
            poster: self.attr(i, "poster").map(str::to_owned),
        };
        let src = self.attr(i, "src")?.to_owned();
        self.images.videos.get(&src, attrs)
    }

    fn attr(&self, i: usize, name: &str) -> Option<&str> {
        el_attr(&self.nodes, &self.els, i, name)
    }

    /// The computed style of element `i`, shared with every element (this
    /// frame or earlier) that had the same parent style, matched rules and
    /// declarations. `fixup` adjusts `display` for widgets and flex/grid
    /// items before the result is shared.
    fn cascade(&mut self, i: usize, div: &Div, parent: StyleId, fixup: u8) -> StyleId {
        let _s = span(Phase::Cascade);
        // the root's <body>: its `overflow` is the viewport's while the root's is visible (CSS
        // Overflow 3 3.3), so it neither clips nor starts a formatting context itself. Only a
        // clipping one: the window doesn't scroll as a viewport does, so a scrolling body (an
        // app's) keeps scrolling
        let fixup = if div.tag == "body" && self.nodes[i].parent == Some(0) { fixup | 4 } else { fixup };
        let (ua, au) = self.matched_rules(i);
        let decls = self.declared(i, div, &ua, &au);
        let viewport = (self.viewport.0.to_bits(), self.viewport.1.to_bits());
        let rule_ids = || ua.iter().map(|r| *r as *const Rule as usize).chain(au.iter().map(|r| r.order));
        // probe without building a key: hash the inputs, compare in place
        let hash = {
            let mut h = key_hasher();
            parent.hash(&mut h);
            viewport.hash(&mut h);
            self.author_hash.hash(&mut h);
            fixup.hash(&mut h);
            for r in rule_ids() {
                r.hash(&mut h);
            }
            for d in &decls {
                d.probe_hash(&mut h);
            }
            h.finish()
        };
        let same = |k: &StyleKey| {
            k.parent == parent
                && k.viewport == viewport
                && k.author == self.author_hash
                && k.fixup == fixup
                && k.rules.len() == ua.len() + au.len()
                && k.rules.iter().copied().eq(rule_ids())
                && k.decls.len() == decls.len()
                && k.decls.iter().zip(&decls).all(|(a, d)| a == *d)
        };
        if let Some(&(_, id)) = self.styles.cache.get(&hash).and_then(|b| b.iter().find(|(k, _)| same(k))) {
            return id;
        }
        let key = StyleKey {
            parent,
            viewport,
            author: self.author_hash,
            rules: rule_ids().collect(),
            decls: decls.iter().map(|d| (*d).clone()).collect(),
            fixup,
        };
        let mut c = self.compute(parent, &decls);
        match fixup & 3 {
            1 if c.display == DisplayKind::Inline => c.display = DisplayKind::InlineBlock,
            2 if c.display == DisplayKind::Inline => c.display = DisplayKind::Block,
            _ => {}
        }
        let visible = taffy::Point { x: Overflow::Visible, y: Overflow::Visible };
        let clips = |o: Overflow| matches!(o, Overflow::Hidden | Overflow::Clip);
        if fixup & 4 != 0 && self.styles[parent].layout.overflow == visible
            && clips(c.layout.overflow.x) && clips(c.layout.overflow.y) {
            c.layout.overflow = visible;
        }
        // a float, or an absolutely positioned box, is blockified (CSS 2 §9.7)
        if c.layout.float != taffy::style::Float::None || c.layout.position == Position::Absolute {
            c.inline_static = c.layout.float == taffy::style::Float::None
                && matches!(c.display, DisplayKind::Inline | DisplayKind::InlineBlock | DisplayKind::InlineFlex | DisplayKind::InlineGrid);
            c.display = match c.display {
                DisplayKind::Inline | DisplayKind::InlineBlock => DisplayKind::Block,
                DisplayKind::InlineFlex => DisplayKind::Flex,
                DisplayKind::InlineGrid => DisplayKind::Grid,
                d => d,
            };
        }
        let id = self.styles.add(c);
        self.styles.cache.entry(hash).or_default().push((key, id));
        id
    }

    /// CSS animations and transitions of element `i` (`anim.rs`): a private
    /// copy of its shared style `style` with the animated values on top.
    /// Never cached, so the shared styles stay what the cascade made them.
    fn animate(&mut self, i: usize, div: &Div, parent: StyleId, style: StyleId) -> StyleId {
        let key = self.nodes[i].key;
        if !self.anim.wants(key, &self.styles[style]) && div.motion().is_none() && !crate::motion::pending() {
            return style;
        }
        // `!important` holds against CSS animations only (not Motion's values,
        // nor transitions): looked up only for elements that have some
        let important: smallvec::SmallVec<[Prop; 8]> = if self.styles[style].anim.is_none() {
            smallvec::SmallVec::new()
        } else {
            let (ua, au) = self.matched_rules(i);
            self.declared(i, div, &ua, &au).iter().filter(|d| d.important).map(|d| d.prop).collect()
        };
        let base = &self.styles[style];
        let env = crate::anim::Env {
            parent: &self.styles[parent],
            viewport: self.viewport,
            vars: self.styles.vars.get(base.vars),
            sheet: self.author,
            media: &self.au_media,
            important: &important,
            el: self.els.last().expect("the element being laid out"),
            node: i,
            motion: div.motion(),
            gestures: crate::motion::Gestures {
                hover: self.state.hovered.contains(&key),
                tap: self.state.active.contains(&key),
                focus: self.state.focused == Some(key),
            },
        };
        match self.anim.apply(key, base, &env) {
            Some(c) => self.styles.add(c),
            None => style,
        }
    }

    /// The UA and author rules matching element `i`: from the memo when an
    /// element with the same fingerprint was matched before.
    fn matched_rules(&mut self, i: usize) -> (crate::css::Matches<'a>, crate::css::Matches<'a>) {
        let (ua_sheet, ua_media) = self.ua_for(i);
        let author: &'a Stylesheet = self.author;
        let memo = {
            let mut h = key_hasher();
            self.nodes[i].fp.hash(&mut h);
            self.nodes[i].styled.hash(&mut h);
            self.containers.generation.hash(&mut h);
            self.author_hash.hash(&mut h); // media results follow the theme's mode
            self.viewport.0.to_bits().hash(&mut h);
            self.viewport.1.to_bits().hash(&mut h);
            h.finish()
        };
        if let Some((u, a)) = self.matched.map.get(&memo) {
            return (u.iter().map(|&k| &ua_sheet.rules[k as usize]).collect(),
                    a.iter().map(|&k| &author.rules[k as usize]).collect());
        }
        let subj = self.subject(i);
        let ua = ua_sheet.matches(&subj, ua_media, None);
        let au = author.matches(&subj, &self.au_media, None);
        self.matched.map.insert(memo, (ua.iter().map(|r| r.order as u32).collect(),
                                       au.iter().map(|r| r.order as u32).collect()));
        (ua, au)
    }

    /// The UA sheet for box `i` (plain, or Blender's inside `styled`) and
    /// its media query results.
    fn ua_for(&self, i: usize) -> (&'a Stylesheet, &Vec<bool>) {
        if self.nodes[i].styled {
            (self.ua_styled, &self.ua_styled_media)
        } else {
            (ua_stylesheet(), &self.ua_media)
        }
    }

    /// The fingerprint of element `i` (see [`BoxNode::fp`]).
    fn fingerprint(&self, i: usize) -> u64 {
        let n = &self.nodes[i];
        let mut h = key_hasher();
        n.parent.map(|p| self.nodes[p].fp).unwrap_or(0).hash(&mut h);
        // the previous element sibling (text in between doesn't count)
        let mut k = n.prev;
        while k != NONE && self.nodes[k as usize].el == NONE {
            k = self.nodes[k as usize].prev;
        }
        (if k == NONE { 0 } else { self.nodes[k as usize].fp }).hash(&mut h);
        n.pos.hash(&mut h);
        n.tpos.hash(&mut h);
        if let Some(e) = (n.el != NONE).then(|| &self.els[n.el as usize]) {
            e.has.hash(&mut h);
            e.empty.hash(&mut h);
            e.later.hash(&mut h);
            e.tag.hash(&mut h);
            e.id.as_deref().hash(&mut h);
            for (a, v) in &e.attrs {
                a.hash(&mut h);
                v.hash(&mut h);
            }
        }
        let st = self.state;
        let key = n.key;
        (st.hovered.contains(&key), st.active.contains(&key), st.focused == Some(key),
         st.focus_path.contains(&key), st.checked.get(&key).copied(), st.details_open.get(&key).copied(),
         st.popover_open.contains(&key))
            .hash(&mut h);
        h.finish()
    }

    /// Every declaration that applies to element `i`, in cascade order.
    fn declared<'d>(&self, i: usize, div: &'d Div, ua: &[&'d Rule], au: &[&'d Rule])
        -> smallvec::SmallVec<[&'d Decl; 24]> {
        let key = self.nodes[i].key;
        let mut own: smallvec::SmallVec<[&'d Decl; 16]> = div.decls.iter().collect();
        if let Some(st) = &div.states {
            if self.state.hovered.contains(&key) {
                own.extend(st.hover.iter());
            }
            if self.state.active.contains(&key) {
                own.extend(st.active.iter());
            }
            if self.state.focused == Some(key) {
                own.extend(st.focus.iter());
            }
        }
        cascade_order(ua, au, &own)
    }

    fn compute(&mut self, parent: StyleId, decls: &[&Decl]) -> Computed {
        // custom properties first: other declarations may reference them
        let pvars = self.styles[parent].vars;
        let vars = if decls.iter().any(|d| d.prop.is_custom()) {
            let mut map: Vec<(Prop, Box<[crate::css::Tok]>)> = self.styles.vars.get(pvars).to_vec();
            for important in [false, true] {
                for d in decls.iter().filter(|d| d.important == important && d.prop.is_custom()) {
                    let v: Box<[crate::css::Tok]> = if crate::vars::uses_vars(&d.value) {
                        match crate::vars::substitute(&d.value, &map) {
                            Some(v) => v.into_boxed_slice(),
                            None => Box::new([]), // guaranteed-invalid: falls back
                        }
                    } else {
                        d.value.to_vec().into_boxed_slice()
                    };
                    match map.binary_search_by_key(&d.prop, |e| e.0) {
                        Ok(i) => map[i].1 = v,
                        Err(i) => map.insert(i, (d.prop, v)),
                    }
                }
            }
            self.styles.vars.add(map.into_boxed_slice())
        } else {
            pvars
        };
        let root = parent == ROOT_STYLE;
        let parent = &self.styles[parent];
        let mut c = Computed::inherit(parent);
        c.vars = vars;
        let map = self.styles.vars.get(vars);
        let subst = |d: &Decl| -> Option<Decl> {
            crate::vars::substitute(&d.value, map).map(|v| Decl {
                prop: d.prop,
                value: crate::css::Value::Owned(v.into_boxed_slice()),
                important: d.important,
            })
        };
        const FONT_SIZE: Prop = Prop::named("font-size");
        const FONT: Prop = Prop::named("font");
        const FONT_FAMILY: Prop = Prop::named("font-family");
        const DIRECTION: Prop = Prop::named("direction");
        let font_first = |d: &&Decl| d.prop == FONT_SIZE || d.prop == FONT || d.prop == FONT_FAMILY || d.prop == DIRECTION;
        let run = |c: &mut Computed, u: Units, d: &Decl| {
            if crate::vars::uses_vars(&d.value) {
                if let Some(d) = subst(d) {
                    apply(c, parent, u, &d);
                }
            } else {
                apply(c, parent, u, d);
            }
        };
        for important in [false, true] {
            // the font first: em and ch lengths in the other properties depend on it (and
            // `direction`: the inline-start and -end properties are its sides)
            for d in decls.iter().filter(|d| d.important == important && font_first(d)) {
                run(&mut c, parent.units(self.viewport), d);
            }
        }
        if root {
            c.root_font_size = c.font_size; // `rem` everywhere (the root's own font-size used the initial)
        }
        // then the line height (`lh` lengths in the others), with the font's units
        const LINE_HEIGHT: Prop = Prop::named("line-height");
        for important in [false, true] {
            for d in decls.iter().filter(|d| d.important == important && d.prop == LINE_HEIGHT) {
                let fu = c.units(self.viewport);
                run(&mut c, fu, d);
            }
        }
        let u = c.units(self.viewport);
        for important in [false, true] {
            for d in decls.iter().filter(|d| d.important == important && !font_first(d) && d.prop != LINE_HEIGHT) {
                run(&mut c, u, d);
            }
        }
        if c.background_current.is_some() {
            crate::style::resolve_current(&mut c);
        }
        if c.stretch_fn.iter().any(Option::is_some) {
            crate::style::resolve_stretch(&mut c, &u);
        }
        c
    }

    /// A new box. `link`: append it to its parent's child list (elements and
    /// text); words, breaks and line boxes only point at their parent.
    fn push(&mut self, parent: Option<usize>, content: Content, owner: Option<usize>, style: StyleId,
            key: Key, link: bool) -> usize {
        let i = self.nodes.len();
        let prev = match parent {
            Some(p) if link => self.nodes[p].last,
            _ => NONE,
        };
        self.nodes.push(BoxNode {
            parent,
            first: NONE,
            last: NONE,
            next: NONE,
            prev,
            content,
            el: NONE,
            owner: owner.unwrap_or(i),
            style,
            key,
            fp: 0,
            taffy: None,
            pos: (1, 1),
            tpos: (1, 1),
            marker: None,
            styled: false,
            bounds: rect(0.0, 0.0, 0.0, 0.0),
            clip: None,
            opacity: 1.0,
            visible: true,
            scroll_range: (0.0, 0.0),
            scroll_pos: (0.0, 0.0),
            fold: None,
            folded: false,
            seq: 0,
        });
        if let (Some(p), true) = (parent, link) {
            if prev == NONE {
                self.nodes[p].first = i as u32;
            } else {
                self.nodes[prev as usize].next = i as u32;
            }
            self.nodes[p].last = i as u32;
        }
        i
    }

    /// Remove the last box (an element that turned out not to render).
    fn pop(&mut self) {
        let n = self.nodes.pop().expect("a box to pop");
        if let Some(p) = n.parent {
            let i = self.nodes.len() as u32;
            if self.nodes[p].last == i {
                self.nodes[p].last = n.prev;
                if n.prev == NONE {
                    self.nodes[p].first = NONE;
                } else {
                    self.nodes[n.prev as usize].next = NONE;
                }
            }
        }
        if n.el != NONE && n.el as usize == self.els.len() - 1 {
            self.els.pop();
        }
    }

    /// Build an element; returns its index (None when not rendered).
    /// `later`: the siblings after it (for `:has()` with `+` / `~`).
    fn element(&mut self, mut div: Div, parent: Option<usize>, key: Key, pos: (usize, usize), later: &[AnyElement])
        -> Option<usize> {
        let parent_style = parent.map(|p| self.nodes[p].style).unwrap_or(ROOT_STYLE);
        let i = self.push(parent, Content::Element, None, parent_style, key, true);
        self.nodes[i].pos = pos;
        self.nodes[i].tpos = std::mem::replace(&mut self.next_tpos, (1, 1));
        self.nodes[i].el = self.els.len() as u32;
        // (the user-agent sheet's own pointer rule is `a:hover`)
        let reactive = div.listeners.is_some() || div.states.is_some() || div.motion().is_some() || div.tag == "a";
        let auto_dir = auto_direction(&div);
        self.els.push(ElData {
            tag: div.tag,
            id: div.id.take(),
            attrs: std::mem::take(&mut div.attrs),
            listeners: std::mem::take(&mut div.listeners),
            has: 0,
            reactive,
            empty: div.children.iter().all(|c| matches!(c, AnyElement::Text(t) if t.is_empty())),
            auto_dir,
            later: smallvec::SmallVec::new(),
        });
        if !self.author.has.is_empty() {
            // :has() looks at the element's subtree, which isn't built yet:
            // evaluate it on the declarative children instead
            let mut bits = 0u64;
            let mut flat = Vec::new();
            let is_has = |e: &&crate::css::HasEntry| e.kind == crate::css::Query::Has;
            for (id, entry) in self.author.has.iter().enumerate().take(64) {
                if !is_has(&entry) || !entry.host_matches(&self.subject(i)) {
                    continue;
                }
                if flat.is_empty() {
                    // the following siblings as far as any `:has()` reaches with `+` / `~`
                    let reach = self.author.has.iter().take(64).filter(is_has)
                        .map(crate::css::HasEntry::sibling_reach).max().unwrap_or(0);
                    let parent_key = parent.map_or(key, |p| self.nodes[p].key);
                    // the anchor (index 0) and its subtree, then the siblings it reaches
                    flat.push(Flat { tag: div.tag, id: None, attrs: &[], parent: NONE, first: NONE, next: NONE,
                                     prev: NONE, pos, tpos: self.subject(i).type_position(), key });
                    flatten_kids(&div, 0, key, &mut flat);
                    if reach > 0 {
                        let built = self.built_tags(i);
                        flatten_later(later, reach, 0, pos, NONE, parent_key, &built, &mut flat);
                    }
                }
                let anchor = DivSubject { flat: &flat, state: self.state, i: 0 };
                if crate::css::has_match(&entry.rels, &anchor) {
                    bits |= 1 << id;
                }
            }
            let counts = self.later_matches(i, pos, later);
            if let Some(e) = self.els.last_mut() {
                e.has = bits;
                e.later = counts;
            }
        }
        self.nodes[i].fp = self.fingerprint(i);
        // `styled` (any value but "false") turns Blender's theme on for the subtree
        self.nodes[i].styled = match self.els.last().and_then(|e| e.attrs.iter().find(|(n, _)| n == "styled")) {
            Some((_, v)) => v.as_ref() != "false",
            None => parent.is_some_and(|p| self.nodes[p].styled),
        };
        let kind = tag_kind(div.tag);
        // widgets and replaced elements are atomic inlines; flex/grid items are blockified
        let fixup = if kind == TagKind::Widget || kind == TagKind::Replaced {
            1
        } else if parent.is_some_and(|p| matches!(self.st(p).display,
                                                  DisplayKind::Flex | DisplayKind::InlineFlex
                                                  | DisplayKind::Grid | DisplayKind::InlineGrid)) {
            2
        } else {
            0
        };
        let style = self.cascade(i, &div, parent_style, fixup);
        if self.styles[style].display == DisplayKind::None || kind == TagKind::Metadata {
            self.pop();
            recycle_decls(std::mem::take(&mut div.decls));
            return None;
        }
        let style = self.animate(i, &div, parent_style, style);
        let style = match self.anchors.apply(self.nodes[i].key, &self.styles[style]) {
            Some(c) => self.styles.add(c),
            None => style,
        };
        self.nodes[i].style = style;
        self.counter_ops(style, parent.unwrap_or(usize::MAX));
        // attributes moved into ElData: widgets read them back through the arena
        div.attrs = Vec::new();
        match kind {
            TagKind::Widget => {
                let spec = WidgetSpec::from_element(&div, self.els.last().map(|e| &e.attrs[..]).unwrap_or(&[]),
                                                    key, self.state);
                if let Some(mut spec) = spec {
                    // browser-look controls in plain HTML; Blender widgets inside
                    // `styled` and for the Blender components (`bui-blender`)
                    spec.html = !self.nodes[i].styled && self.attr(i, "bui-blender").is_none();
                    spec.bare = spec.html && self.styles[style].border_drawn.iter().all(|d| !d);
                    let own = &self.styles[style];
                    let u = own.units(self.viewport);
                    spec.radius = own.radius.iter().filter_map(|r| r.to_px(&u)).reduce(f32::max).filter(|r| *r > 0.0);
                    if spec.html && matches!(spec.kind, crate::widgets::WKind::TextArea) {
                        let own = &self.styles[style];
                        let argb = |c: crate::Rgba| {
                            let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
                            (c.a > 0.0).then(|| b(c.a) << 24 | b(c.r) << 16 | b(c.g) << 8 | b(c.b))
                        };
                        spec.text_color = argb(own.color);
                        spec.field_bg = argb(own.background);
                        spec.grow = own.field_sizing_content;
                        if spec.grow {
                            let px = |l: LengthPercentage| match l.into_raw() { r if r.tag() == taffy::CompactLength::LENGTH_TAG => r.value(), _ => 0.0 };
                            let p = own.layout.padding;
                            spec.pad = [px(p.top), px(p.right), px(p.bottom), px(p.left)];
                            let m = own.layout.max_size.height.into_raw();
                            spec.max_h = (m.tag() == taffy::CompactLength::LENGTH_TAG).then(|| m.value());
                        }
                    }
                    if spec.html && matches!(spec.kind, crate::widgets::WKind::Text { .. }) {
                        // a form control does not inherit its text color: the UA sheet gives it
                        // `fieldtext`, so the computed color is the field's own (`text-white` on a
                        // dark field), and its CSS background replaces the browser's white
                        let (own, up) = (&self.styles[style], &self.styles[parent_style]);
                        let argb = |c: crate::Rgba| {
                            let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
                            (c.a > 0.0).then(|| b(c.a) << 24 | b(c.r) << 16 | b(c.g) << 8 | b(c.b))
                        };
                        spec.text_color = argb(own.color);
                        spec.field_bg = argb(own.background);
                        spec.font_px = (own.font_size != up.font_size).then_some(own.font_size);
                    }
                    self.nodes[i].content = Content::Widget(Box::new(spec));
                } else {
                    self.children(i, div.children);
                }
            }
            TagKind::Replaced => {
                let r = if let Some(s) = div.rare.as_mut().and_then(|r| r.source.take()) {
                    match s {
                        ReplacedSource::Painted(f) => Replaced::Painted(f),
                        ReplacedSource::Image(img) => Replaced::Image(img),
                        ReplacedSource::Video(v) => Replaced::Video(v),
                    }
                } else if div.tag == "video" && let Some(v) = self.video_src(i) {
                    Replaced::Video(v)
                } else if div.tag == "svg" && let Some(img) = self.images.svg(&svg_markup(&div)) {
                    Replaced::Image(img)
                } else if let Some(file) = self.attr(i, "src").map(str::to_owned).and_then(|s| self.images.load(&s)) {
                    Replaced::File(file, self.attr(i, "frame").and_then(|v| v.trim().parse().ok()))
                } else {
                    Replaced::Placeholder(div.tag)
                };
                self.nodes[i].content = Content::Replaced(r);
            }
            // <wbr>: a word break opportunity, U+200B in the text
            TagKind::Break if div.tag == "wbr" => {
                self.nodes[i].content = Content::Text("\u{200b}".into());
            }
            TagKind::Break => {
                self.nodes[i].content = Content::LineBreak;
            }
            TagKind::Box | TagKind::Metadata => {
                let mut children = div.children;
                if div.tag == "details" {
                    let open = self.subject(i).attr("open").is_some();
                    if !open {
                        children.retain(|c| matches!(c, AnyElement::Element(d) if d.tag == "summary"));
                    }
                }
                if div.tag == "q" {
                    let (open, close) = self.st(i).quotes.clone()
                        .unwrap_or_else(|| ("\u{201c}".into(), "\u{201d}".into()));
                    children.insert(0, AnyElement::Text(open.into()));
                    children.push(AnyElement::Text(close.into()));
                }
                let pseudo = self.author.uses_pseudo() || self.ua_for(i).0.uses_pseudo();
                if pseudo {
                    self.generated(i, crate::css::PseudoElement::Before);
                }
                self.children(i, children);
                if pseudo {
                    self.generated(i, crate::css::PseudoElement::After);
                }
            }
        }
        // counters this element's children created go out of scope
        while self.counters.last().is_some_and(|c| c.2 == i) {
            self.counters.pop();
        }
        recycle_decls(std::mem::take(&mut div.decls));
        Some(i)
    }

    /// Apply `counter-reset`, then `-increment`, then `-set` of style `sid`; new counters are scoped to `scope` (the parent: an element's
    /// counter is seen by its following siblings).
    fn counter_ops(&mut self, sid: StyleId, scope: usize) {
        let Some(ops) = self.styles[sid].counters.clone() else { return };
        for (name, v) in &ops.reset {
            // a sibling's reset replaces the instance a previous sibling made
            let same_scope = self.counters.iter_mut().rev().take_while(|c| c.2 == scope).find(|c| c.0 == *name);
            match same_scope {
                Some(c) => c.1 = *v,
                None => self.counters.push((name.clone(), *v, scope)),
            }
        }
        let find = |b: &mut Self, name: &str| -> usize {
            match b.counters.iter().rposition(|c| &*c.0 == name) {
                Some(k) => k,
                None => {
                    // using a counter that doesn't exist instantiates it here
                    b.counters.push((name.into(), 0, scope));
                    b.counters.len() - 1
                }
            }
        };
        for (name, v) in &ops.increment {
            let k = find(self, name);
            self.counters[k].1 = self.counters[k].1.wrapping_add(*v);
        }
        for (name, v) in &ops.set {
            let k = find(self, name);
            self.counters[k].1 = *v;
        }
    }

    /// The `::before` / `::after` box of element `i`, when a rule gives it content.
    fn generated(&mut self, i: usize, pe: crate::css::PseudoElement) {
        let (ua_sheet, ua_media) = self.ua_for(i);
        let author: &'a Stylesheet = self.author;
        let (ua, au) = {
            let subj = self.subject(i);
            (ua_sheet.matches(&subj, ua_media, Some(pe)), author.matches(&subj, &self.au_media, Some(pe)))
        };
        if ua.is_empty() && au.is_empty() {
            return;
        }
        let parent = self.nodes[i].style;
        let tag = if pe == crate::css::PseudoElement::Before { "::before" } else { "::after" };
        // shared like element styles: same parent + same rules = same style
        let hash = {
            let mut h = key_hasher();
            (parent, tag, self.author_hash).hash(&mut h);
            for r in ua.iter().chain(au.iter()) {
                (*r as *const Rule as usize).hash(&mut h);
            }
            h.finish()
        };
        let rule_ids: Vec<usize> = ua.iter().chain(au.iter()).map(|r| *r as *const Rule as usize).collect();
        let fixup = if pe == crate::css::PseudoElement::Before { 3 } else { 4 };
        let viewport = (self.viewport.0.to_bits(), self.viewport.1.to_bits());
        let found = self.styles.cache.get(&hash).and_then(|b| b.iter().find(|(k, _)| {
            k.parent == parent && k.fixup == fixup && k.viewport == viewport && k.author == self.author_hash
                && k.rules == rule_ids
        })).map(|(_, id)| *id);
        let sid = match found {
            Some(id) => id,
            None => {
                let decls = cascade_order(&ua, &au, &[]);
                let mut c = self.compute(parent, &decls);
                if matches!(self.st(i).display, DisplayKind::Flex | DisplayKind::InlineFlex
                            | DisplayKind::Grid | DisplayKind::InlineGrid) && c.display == DisplayKind::Inline {
                    c.display = DisplayKind::Block; // flex / grid items are blockified
                }
                let id = self.styles.add(c);
                let key = StyleKey { parent, viewport, author: self.author_hash, rules: rule_ids, decls: Vec::new(), fixup };
                self.styles.cache.entry(hash).or_default().push((key, id));
                id
            }
        };
        let st = &self.styles[sid];
        if st.content.is_none() || st.display == DisplayKind::None {
            return;
        }
        self.counter_ops(sid, i);
        let st = &self.styles[sid];
        let Some(parts) = st.content.as_ref() else { return };
        let quotes = st.quotes.clone().unwrap_or_else(|| ("\u{201c}".into(), "\u{201d}".into()));
        let mut text = String::new();
        for p in parts.iter() {
            match p {
                ContentPart::Text(t) => text.push_str(t),
                ContentPart::Attr(n) => text.push_str(self.attr(i, n).unwrap_or("")),
                ContentPart::OpenQuote => text.push_str(&quotes.0),
                ContentPart::CloseQuote => text.push_str(&quotes.1),
                ContentPart::Counter(name, ls) => {
                    let v = self.counters.iter().rev().find(|c| c.0 == *name).map_or(0, |c| c.1);
                    text.push_str(&ls.format(v));
                }
                ContentPart::Counters(name, sep, ls) => {
                    let all: Vec<String> = self.counters.iter().filter(|c| c.0 == *name)
                        .map(|c| ls.format(c.1)).collect();
                    text.push_str(&if all.is_empty() { ls.format(0) } else { all.join(sep) });
                }
            }
        }
        let key = child_key(self.nodes[i].key, tag, 0);
        let g = self.push(Some(i), Content::Element, None, sid, key, true);
        self.nodes[g].el = self.els.len() as u32;
        self.els.push(ElData { tag, id: None, attrs: Vec::new(), listeners: None, has: 0, reactive: false,
                               empty: text.is_empty(), auto_dir: None, later: smallvec::SmallVec::new() });
        if !text.is_empty() {
            self.push(Some(g), Content::Text(text.into()), Some(g), sid, child_key(key, "#text", 1), true);
        }
    }

    /// Children of element `i`: elements and text, keyed by position.
    fn children(&mut self, i: usize, mut children: Vec<AnyElement>) {
        // list markers: painted outside the box, or inline text when `inside`
        if self.st(i).display == DisplayKind::ListItem {
            let ls = self.st(i).list_style;
            let tag = self.subject(i).tag().to_owned();
            let marker = if tag == "summary" {
                let details = self.nodes[i].parent.unwrap_or(i);
                Some(if self.subject(details).attr("open").is_some() { "\u{25be}" } else { "\u{25b8}" }.into())
            } else {
                let idx = self.nodes[i].pos.0 + self.nodes[i].parent
                    .and_then(|p| self.attr(p, "start"))
                    .and_then(|s| s.parse::<usize>().ok()).map(|s| s - 1).unwrap_or(0);
                ls.marker(idx).or_else(|| self.st(i).list_image.as_ref().map(|_| String::new()))
            };
            match marker {
                Some(m) if self.st(i).marker_inside => {
                    children.insert(0, AnyElement::Text(format!("{m}\u{a0}").into()));
                }
                m => self.nodes[i].marker = m,
            }
        }
        let count = children.iter().filter(|c| matches!(c, AnyElement::Element(_))).count();
        // positions among siblings of the same tag (`:nth-of-type`, `:last-of-type`):
        // totals per tag (few distinct tags), then running counters, O(n)
        let mut tag_counts: smallvec::SmallVec<[(&'static str, u32, u32); 8]> = smallvec::SmallVec::new();
        for c in &children {
            if let AnyElement::Element(d) = c {
                match tag_counts.iter_mut().find(|t| std::ptr::eq(t.0, d.tag) || t.0 == d.tag) {
                    Some(t) => t.1 += 1,
                    None => tag_counts.push((d.tag, 1, 0)),
                }
            }
        }
        let (mut k, mut n) = (0, 0);
        let parent_key = self.nodes[i].key;
        // taken one at a time: the siblings after each child stay in place for `:has(+ / ~)`
        for at in 0..children.len() {
            let child = std::mem::replace(&mut children[at], AnyElement::Text(SharedString::Borrowed("")));
            n += 1;
            match child {
                AnyElement::Element(d) => {
                    k += 1;
                    let key = match &d.id {
                        Some(id) => id_key(id),
                        None => child_key(parent_key, d.tag, k),
                    };
                    if let Some(t) = tag_counts.iter_mut().find(|t| std::ptr::eq(t.0, d.tag) || t.0 == d.tag) {
                        t.2 += 1;
                        self.next_tpos = (t.2, t.1);
                    }
                    self.element(d, Some(i), key, (k, count), &children[at + 1..]);
                }
                // an empty text run renders nothing (`None` as a child)
                AnyElement::Text(t) if t.is_empty() => {}
                AnyElement::Text(t) => {
                    let style = self.nodes[i].style;
                    self.push(Some(i), Content::Text(t), Some(i), style,
                              child_key(parent_key, "#text", n), true);
                }
            }
        }
        recycle_children(children);
    }

    // -- taffy ------------------------------------------------------------------------------

    fn is_block_level(&self, i: usize) -> bool {
        match &self.nodes[i].content {
            // an image or a control is inline unless its display says otherwise (`display: block`)
            Content::Element | Content::Replaced(_) | Content::Widget(_) => {
                let s = self.st(i);
                !s.display.is_inline_level() && !s.inline_static
            }
            _ => false,
        }
    }

    fn base_style(&self, i: usize) -> Style {
        let c = self.st(i);
        let mut s = c.layout.clone();
        s.display = match c.display {
            DisplayKind::Flex | DisplayKind::InlineFlex => Display::Flex,
            DisplayKind::Grid | DisplayKind::InlineGrid => Display::Grid,
            DisplayKind::Table | DisplayKind::TableRowGroup => {
                s.flex_direction = FlexDirection::Column;
                let sp = if c.border_collapse { 0.0 } else { c.border_spacing.1 };
                s.gap.height = LengthPercentage::length(sp);
                if c.display == DisplayKind::Table && !c.border_collapse {
                    let pad = |v: LengthPercentage, add: f32| match v.into_raw() {
                        r if r.tag() == taffy::CompactLength::LENGTH_TAG => LengthPercentage::length(r.value() + add),
                        _ => v,
                    };
                    let (h, v) = c.border_spacing;
                    s.padding = Rect { left: pad(s.padding.left, h), right: pad(s.padding.right, h),
                                       top: pad(s.padding.top, v), bottom: pad(s.padding.bottom, v) };
                }
                Display::Flex
            }
            DisplayKind::TableRow => {
                s.flex_direction = FlexDirection::Row;
                s.gap.width = LengthPercentage::length(if c.border_collapse { 0.0 } else { c.border_spacing.0 });
                Display::Flex
            }
            _ if c.layout.display == Display::FlowRoot => Display::FlowRoot,
            _ => Display::Block,
        };
        if c.phys_align != 0 {
            self.physical_alignment(i, &mut s);
        }
        // a collapsed flex item (CSS Flexbox 4.4): no main size, margins or flexing; its cross size
        // stays, a strut for its line
        if c.collapsed && let Some(p) = self.nodes[i].parent {
            let ps = self.st(p);
            if matches!(ps.display, DisplayKind::Flex | DisplayKind::InlineFlex) {
                let zero = LengthPercentageAuto::length(0.0);
                let (size, min, max, m0, m1) = if matches!(ps.layout.flex_direction, FlexDirection::Row | FlexDirection::RowReverse) {
                    (&mut s.size.width, &mut s.min_size.width, &mut s.max_size.width, &mut s.margin.left, &mut s.margin.right)
                } else {
                    (&mut s.size.height, &mut s.min_size.height, &mut s.max_size.height, &mut s.margin.top, &mut s.margin.bottom)
                };
                (*size, *min, *max, *m0, *m1) = (Dimension::length(0.0), zero, zero, zero, zero);
                (s.flex_grow, s.flex_shrink, s.flex_basis) = (0.0, 0.0, Dimension::length(0.0));
                s.overflow = taffy::Point { x: Overflow::Hidden, y: Overflow::Hidden };
                let pad = LengthPercentage::length(0.0);
                if matches!(ps.layout.flex_direction, FlexDirection::Row | FlexDirection::RowReverse) {
                    (s.padding.left, s.padding.right, s.border.left, s.border.right) = (pad, pad, pad, pad);
                } else {
                    (s.padding.top, s.padding.bottom, s.border.top, s.border.bottom) = (pad, pad, pad, pad);
                }
            }
        }
        let [start, end] = self.trimmed(i);
        if start {
            s.margin.top = LengthPercentageAuto::length(0.0);
        }
        if end {
            s.margin.bottom = LengthPercentageAuto::length(0.0);
        }
        if c.display == DisplayKind::Table {
            if let Some(tc) = self.table_cols.get(&i).filter(|t| t.fit && !t.widths.is_empty()) {
                // auto width: the table shrinks to its columns (up to its container)
                let sp = if c.border_collapse { 0.0 } else { c.border_spacing.0 };
                let w: f32 = tc.widths.iter().sum::<f32>() + sp * (tc.widths.len() + 1) as f32;
                let pad = |v: LengthPercentage| match v.into_raw() {
                    r if r.tag() == taffy::CompactLength::LENGTH_TAG => r.value(),
                    _ => 0.0,
                };
                let extra = pad(s.padding.left) + pad(s.padding.right) + pad(s.border.left) + pad(s.border.right);
                s.size.width = Dimension::length((w + extra).ceil());
                if s.max_size.width == LengthPercentageAuto::auto() {
                    s.max_size.width = LengthPercentageAuto::percent(1.0);
                }
            }
        }
        if c.display == DisplayKind::TableCell && s.size.width == Dimension::auto() {
            let span = self.attr(i, "colspan").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).max(1);
            match self.cell_columns(i, span) {
                // every cell of a column gets the column's width: rows line up
                Some((w, fit, fixed)) if w > 0.0 || fixed => {
                    s.flex_basis = Dimension::length(w);
                    s.flex_grow = if fit && !fixed { 0.0 } else { w.max(1.0) };
                    s.flex_shrink = w.max(1.0);
                    if fixed {
                        s.min_size.width = LengthPercentageAuto::length(0.0); // fixed tables don't widen for content
                    }
                }
                _ => {
                    s.flex_grow = span as f32;
                    s.flex_shrink = 1.0;
                    s.flex_basis = Dimension::percent(0.0);
                }
            }
        }
        s
    }

    /// The rows of table `t` (in row groups or directly inside).
    fn table_rows(&self, t: usize) -> Vec<usize> {
        let mut rows = Vec::new();
        for k in kids(&self.nodes, t) {
            if !matches!(self.nodes[k].content, Content::Element) {
                continue;
            }
            match self.st(k).display {
                DisplayKind::TableRow => rows.push(k),
                DisplayKind::TableRowGroup => rows.extend(kids(&self.nodes, k)
                    .filter(|&r| matches!(self.nodes[r].content, Content::Element)
                            && self.st(r).display == DisplayKind::TableRow)),
                _ => {}
            }
        }
        rows
    }

    fn row_cells(&self, r: usize) -> Vec<usize> {
        kids(&self.nodes, r).filter(|&c| matches!(self.nodes[c].content, Content::Element | Content::Widget(_))
                                     && self.st(c).display == DisplayKind::TableCell).collect()
    }

    /// The single-line width of everything in box `k` (its text, in its fonts).
    fn max_content(&mut self, k: usize) -> f32 {
        let mut w = 0.0f32;
        let list: Vec<usize> = kids(&self.nodes, k).collect();
        for c in list {
            match &self.nodes[c].content {
                Content::Text(t) => {
                    let st = &self.styles[self.nodes[c].style];
                    let (size, flags) = (st.font_size, text::flags(st));
                    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
                    w += self.text.width(&t, size, flags) + st.letter_spacing * t.chars().count() as f32;
                }
                Content::Element => {
                    let inner = self.max_content(c);
                    let st = self.st(c);
                    let u = st.units(self.viewport);
                    let px = |v: LengthPercentage| match v.into_raw() {
                        r if r.tag() == taffy::CompactLength::LENGTH_TAG => r.value(),
                        _ => 0.0,
                    };
                    let edges = px(st.layout.padding.left) + px(st.layout.padding.right)
                        + px(st.layout.border.left) + px(st.layout.border.right);
                    let explicit = match st.layout.size.width.into_raw() {
                        r if r.tag() == taffy::CompactLength::LENGTH_TAG => Some(r.value()),
                        _ => None,
                    };
                    let _ = u;
                    let own = explicit.unwrap_or(inner + edges);
                    // blocks stack; inline content continues the line
                    if st.display.is_inline_level() || st.display == DisplayKind::Inline {
                        w += own;
                    } else {
                        w = w.max(own);
                    }
                }
                _ => {}
            }
        }
        w
    }

    /// Column widths of table `t`: the widest single-line content of each
    /// column (auto layout), or the first row's cell widths (fixed).
    fn table_columns(&mut self, t: usize) {
        let fixed = self.st(t).table_fixed;
        let fit = self.st(t).layout.size.width == Dimension::auto();
        let mut widths: Vec<f32> = Vec::new();
        let rows = self.table_rows(t);
        for (ri, r) in rows.iter().enumerate() {
            if fixed && ri > 0 {
                break;
            }
            let mut col = 0usize;
            for c in self.row_cells(*r) {
                let span = self.attr(c, "colspan").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).max(1);
                let st = self.st(c);
                let px = |v: LengthPercentage| match v.into_raw() {
                    r if r.tag() == taffy::CompactLength::LENGTH_TAG => r.value(),
                    _ => 0.0,
                };
                let edges = px(st.layout.padding.left) + px(st.layout.padding.right)
                    + px(st.layout.border.left) + px(st.layout.border.right);
                let explicit = match st.layout.size.width.into_raw() {
                    r if r.tag() == taffy::CompactLength::LENGTH_TAG => Some(r.value()),
                    _ => None,
                };
                let w = if fixed { explicit.unwrap_or(0.0) } else { explicit.unwrap_or(0.0).max(self.max_content(c) + edges) };
                if widths.len() < col + span {
                    widths.resize(col + span, 0.0);
                }
                if span == 1 {
                    widths[col] = widths[col].max(w.ceil());
                }
                col += span;
            }
        }
        if fixed {
            // unspecified columns share what the specified ones leave
            let given: f32 = widths.iter().sum();
            let free = widths.iter().filter(|w| **w == 0.0).count();
            if free > 0 {
                let total = match self.st(t).layout.size.width.into_raw() {
                    r if r.tag() == taffy::CompactLength::LENGTH_TAG => r.value(),
                    _ => self.viewport.0,
                };
                let each = ((total - given) / free as f32).max(0.0);
                widths.iter_mut().filter(|w| **w == 0.0).for_each(|w| *w = each);
            }
        }
        self.table_cols.insert(t, TableCols { widths, fit, fixed });
    }

    /// The width (px) of the columns cell `c` spans, and its table's mode.
    fn cell_columns(&self, c: usize, span: usize) -> Option<(f32, bool, bool)> {
        let row = self.nodes[c].parent?;
        let mut t = self.nodes[row].parent?;
        if self.st(t).display == DisplayKind::TableRowGroup {
            t = self.nodes[t].parent?;
        }
        let tc = self.table_cols.get(&t)?;
        let mut col = 0usize;
        for k in self.row_cells(row) {
            if k == c {
                break;
            }
            col += self.attr(k, "colspan").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).max(1);
        }
        let end = (col + span).min(tc.widths.len());
        if col >= end {
            return None;
        }
        let sp = self.st(t).border_spacing.0;
        let w = tc.widths[col..end].iter().sum::<f32>() + sp * (end - col - 1) as f32;
        Some((w, tc.fit, tc.fixed))
    }

    /// Make line-container style: a wrapping flex row aligned per text-align.
    /// How container style `c`'s lines go, with `items` (`tree/inline.rs`); `first`: they begin the
    /// block (`text-indent`).
    fn inline_of(&self, c: &Computed, items: Vec<crate::tree::Item>, first: bool) -> crate::tree::Inline {
        use crate::tree::{ALIGN_CENTER, ALIGN_JUSTIFY, ALIGN_LEFT, ALIGN_RIGHT};
        let rtl = c.layout.direction == taffy::style::Direction::Rtl;
        let code = |a: TextAlign| match a.physical(rtl) {
            TextAlign::Right => ALIGN_RIGHT,
            TextAlign::Center => ALIGN_CENTER,
            TextAlign::Justify => ALIGN_JUSTIFY,
            _ => ALIGN_LEFT,
        };
        // `text-align-last: auto`: as `text-align`, but `justify` aligns the last line to the start
        let last = match (c.text_align_last, c.text_align) {
            (Some(a), _) => code(a),
            (None, TextAlign::Justify) => code(TextAlign::Start),
            (None, a) => code(a),
        };
        let line = c.line_px();
        let above = text::baseline(0.0, line, c.font_size, c.font_id);
        crate::tree::Inline {
            items,
            align: code(c.text_align),
            last,
            // a percentage is of the line's room, known when the lines are laid out
            indent: if first {
                let u = c.units(self.viewport);
                let px = c.text_indent.resolve(&u, 0.0);
                (px, (c.text_indent.resolve(&u, 100.0) - px) / 100.0)
            } else {
                (0.0, 0.0)
            },
            strut: [above, line - above],
            middle: c.font_size * 0.25,
            balance: c.text_wrap & crate::linebreak::WRAP_BALANCE != 0,
        }
    }

    /// The taffy node for `key` with this style, measure context and
    /// children: last frame's node, updated only where something differs
    /// (which marks it dirty), or a new one.
    fn node(&mut self, key: Key, style: Style, ctx: Option<Measure>, children: &[NodeId]) -> NodeId {
        self.node_from(key, None, |_| style, ctx, children)
    }

    /// The taffy node of `key`, written only where it differs from last frame's. With a `shape`,
    /// `style` is called once per shape, and the nodes of a shape share its result.
    fn node_from(&mut self, key: Key, shape: Option<(StyleId, Form)>, style: impl FnOnce(&Self) -> Style,
                 ctx: Option<Measure>, children: &[NodeId]) -> NodeId {
        let stamp = self.recon.stamp;
        let reuse = match self.recon.map.get_mut(&key) {
            Some((id, st)) if *st != stamp => {
                *st = stamp;
                Some(*id)
            }
            _ => None,
        };
        let made = match shape {
            Some((style_id, form)) => Made::Shared(self.shared_style(Shape { epoch: self.styles.epoch, style: style_id, form }, style)),
            None => Made::Own(style(self)),
        };
        let Some(id) = reuse else {
            let style = match made {
                Made::Shared(ix) => ix,
                Made::Own(s) => self.taffy.intern(s),
            };
            let id = match ctx {
                Some(m) => self.taffy.new_leaf_with_context(style, m),
                None => self.taffy.new_with_children(style, children),
            };
            self.changed = true;
            match self.recon.map.entry(key) {
                std::collections::hash_map::Entry::Occupied(_) => self.recon.extra.push(id), // duplicate key
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert((id, stamp));
                }
            }
            return id;
        };
        match made {
            Made::Shared(new) if new != self.taffy.style_ix(id) => {
                if self.taffy.has_style(id, new) {
                    self.taffy.replace_style(id, new);
                } else {
                    self.taffy.set_style(id, new);
                    self.changed = true;
                }
            }
            Made::Shared(_) => {}
            Made::Own(new) => {
                if *self.taffy.style(id) != new {
                    let ix = self.taffy.intern(new);
                    self.taffy.set_style(id, ix);
                    self.changed = true;
                }
            }
        }
        let t = &mut *self.taffy;
        if t.children(id) != children {
            t.set_children(id, children);
            self.changed = true;
        }
        if t.context(id) != ctx.as_ref() {
            t.set_context(id, ctx);
            self.changed = true;
        }
        id
    }

    /// The table entry of `shape`'s taffy style: the shared one, made when there is none.
    fn shared_style(&mut self, shape: Shape, style: impl FnOnce(&Self) -> Style) -> StyleIx {
        if let Some(&ix) = self.shapes.get(&shape) {
            return ix;
        }
        let ix = self.taffy.intern(style(self));
        self.shapes.insert(shape, ix);
        ix
    }

    /// The shape of box `i`'s taffy style when it follows from its computed style alone.
    fn shape(&self, i: usize, form: Form) -> Option<(StyleId, Form)> {
        let table = matches!(self.st(i).display, DisplayKind::Table | DisplayKind::TableRowGroup
                             | DisplayKind::TableRow | DisplayKind::TableCell);
        (i != 0 && !table).then_some((self.nodes[i].style, form))
    }

    /// The taffy style of box `i`: its computed layout, and the root filling the viewport.
    /// `left` / `right` in `justify-*` (`Computed::phys_align`): parsed as start / end, which swap
    /// when the direction they are relative to (the container's) is right to left. In a flex
    /// column's vertical main axis, `justify-content` takes them as start.
    fn physical_alignment(&self, i: usize, s: &mut Style) {
        use crate::style::{PHYS_JUSTIFY_CONTENT, PHYS_JUSTIFY_ITEMS, PHYS_JUSTIFY_SELF};
        use taffy::style::{AlignContentKeyword as Ac, AlignItemsKeyword as Ai};
        let bits = self.st(i).phys_align;
        let rtl = s.direction == taffy::style::Direction::Rtl;
        let items = |a: AlignItems| AlignItems { keyword: match a.keyword { Ai::Start => Ai::End, Ai::End => Ai::Start, k => k }, ..a };
        if bits & PHYS_JUSTIFY_ITEMS != 0 && rtl {
            s.justify_items = items(s.justify_items);
        }
        let parent_rtl = self.nodes[i].parent.is_some_and(|p| self.st(p).layout.direction == taffy::style::Direction::Rtl);
        if bits & PHYS_JUSTIFY_SELF != 0 && parent_rtl {
            s.justify_self = s.justify_self.map(items);
        }
        if bits & PHYS_JUSTIFY_CONTENT != 0 {
            let column = s.display == Display::Flex && matches!(s.flex_direction, FlexDirection::Column | FlexDirection::ColumnReverse);
            let a = s.justify_content;
            s.justify_content = AlignContent { keyword: match a.keyword {
                _ if column => Ac::Start,
                Ac::Start if rtl => Ac::End,
                Ac::End if rtl => Ac::Start,
                k => k,
            }, ..a };
        }
    }

    /// Which of box `i`'s block margins `margin-trim` drops (css-box-4 3): it is its block
    /// container's first (last) in-flow child, or its margin collapses through ancestors that are
    /// (block boxes with no border or padding on that side) up to one that trims that edge.
    fn trimmed(&self, i: usize) -> [bool; 2] {
        use crate::style::{TRIM_BLOCK_END, TRIM_BLOCK_START};
        // only under a trimming container (a few ancestors up at most)
        let mut up = self.nodes[i].parent;
        let mut any = false;
        for _ in 0..16 {
            let Some(p) = up else { break };
            if self.st(p).margin_trim & (TRIM_BLOCK_START | TRIM_BLOCK_END) != 0 {
                any = true;
                break;
            }
            up = self.nodes[p].parent;
        }
        if !any {
            return [false; 2];
        }
        let in_flow = |k: usize| match &self.nodes[k].content {
            Content::Text(t) => !t.trim().is_empty(),
            Content::Element | Content::Replaced(_) | Content::Widget(_) => {
                let s = self.st(k);
                s.display != DisplayKind::None && !s.layout.position.is_out_of_flow() && s.layout.float == taffy::style::Float::None
            }
            _ => false,
        };
        let mut out = [false; 2];
        for (side, bit) in [(0, TRIM_BLOCK_START), (1, TRIM_BLOCK_END)] {
            let mut cur = i;
            while let Some(p) = self.nodes[cur].parent {
                // the edge child, or one with only self-collapsing boxes (margins collapse
                // through them) between it and the edge
                let list: Kids = kids(&self.nodes, p).filter(|&k| in_flow(k)).collect();
                let Some(at) = list.iter().position(|&k| k == cur) else { break };
                let between = if side == 0 { &list[..at] } else { &list[at + 1..] };
                if !between.iter().all(|&k| self.self_collapsing(k)) {
                    break;
                }
                let ps = self.st(p);
                if ps.margin_trim & bit != 0 {
                    out[side] = true;
                    break;
                }
                // its margin adjoins the parent's only through a plain block edge
                let (pad, border) = if side == 0 { (ps.layout.padding.top, ps.layout.border.top) } else { (ps.layout.padding.bottom, ps.layout.border.bottom) };
                let zero = LengthPercentage::length(0.0);
                let block = matches!(ps.display, DisplayKind::Block | DisplayKind::ListItem | DisplayKind::Inline)
                    && ps.layout.display != Display::FlowRoot && !overflow_clips(ps);
                if !block || pad != zero || border != zero {
                    break;
                }
                cur = p;
            }
        }
        out
    }

    /// Whether block box `k` collapses through (CSS 2 8.3.1): no height, border, padding or content.
    fn self_collapsing(&self, k: usize) -> bool {
        let s = self.st(k);
        let zero = LengthPercentage::length(0.0);
        let l = &s.layout;
        matches!(self.nodes[k].content, Content::Element)
            && matches!(s.display, DisplayKind::Block | DisplayKind::ListItem)
            && (l.size.height == Dimension::length(0.0) || (l.size.height == Dimension::auto() && kids(&self.nodes, k).next().is_none()))
            && matches!(l.min_size.height, m if m == LengthPercentageAuto::auto() || m == LengthPercentageAuto::length(0.0))
            && [l.padding.top, l.padding.bottom, l.border.top, l.border.bottom].iter().all(|v| *v == zero)
    }

    fn box_style(&self, i: usize) -> Style {
        let mut base = self.base_style(i);
        if i == 0 {
            base.min_size = Size { width: LengthPercentageAuto::length(self.viewport.0),
                                   height: LengthPercentageAuto::length(self.viewport.1) };
        }
        base
    }

    fn leaf(&mut self, i: usize, style: Style) -> NodeId {
        let id = self.node(self.nodes[i].key, style, None, &[]);
        self.nodes[i].taffy = Some(id);
        id
    }

    /// Text leaf `id` in style `sid`: its baselines, where its font's sit in its line boxes.
    fn text_baseline(&mut self, id: NodeId, sid: StyleId) {
        let c = &self.styles[sid];
        let line = c.line_px();
        let above = text::baseline(0.0, line, c.font_size, c.font_id);
        self.changed |= self.taffy.set_baseline(id, Some(crate::tree::Baseline::Text([above, line - above])));
    }

    /// Replace the text of box `k` by its whitespace-processed, transformed form.
    fn transform_text(&mut self, k: usize, c: StyleId) {
        let (tt, ws, tab) = ((self.styles[c].text_transform, self.styles[c].text_transform_extra), self.styles[c].white_space, self.styles[c].tab_size);
        let (rtl, ovr) = (self.styles[c].layout.direction == taffy::style::Direction::Rtl, self.styles[c].bidi_override);
        if let Content::Text(t) = &self.nodes[k].content {
            // the stored text stays as it is unless a transform or the bidi order changes it
            let changed = match crate::linebreak::transform(t, tt, ws, Some(tab)) {
                std::borrow::Cow::Owned(s) => Some(s),
                std::borrow::Cow::Borrowed(_) => None,
            };
            let changed = text::bidi_visual(changed.as_deref().unwrap_or(t), rtl, ovr).or(changed);
            if let Some(t) = changed {
                self.nodes[k].content = Content::Text(t.into());
            }
        }
    }

    /// The measure context for the text of box `node`, registered under `key`.
    fn text_measure(&mut self, key: Key, node: usize, c: StyleId, nowrap: bool) -> Measure {
        let c = &self.styles[c];
        let (size, flags, line, brk) = (c.font_size, text::flags(c), c.line_px(), text::break_mode(c));
        let t = match &self.nodes[node].content {
            Content::Text(t) => t.as_ref(),
            _ => "",
        };
        if brk == 0 && !t.contains([' ', '\n']) {
            return Measure::Word { width: self.text.width(t, size, flags).ceil(), line };
        }
        let hash = hash_parts(&[t.as_bytes()]);
        self.text_box.insert(key, node);
        Measure::Text { key, hash, size, flags, line, nowrap, brk }
    }

    /// The taffy node for box `i` (and its subtree). Marks it dirty when
    /// anything below changed (taffy's own propagation follows parent links,
    /// which reconciling can leave stale).
    fn layout_node(&mut self, i: usize) -> NodeId {
        let outer = std::mem::replace(&mut self.changed, false);
        let made = std::mem::replace(&mut self.inlined, false);
        let id = self.layout_node_inner(i);
        // a box that no longer holds lines lays out as its display says
        if !std::mem::replace(&mut self.inlined, made) {
            self.changed |= self.taffy.set_inline(id, None);
        }
        let c = &self.styles[self.nodes[i].style];
        let (kw, contain) = (c.size_kw, c.contain);
        self.changed |= self.taffy.set_sizing(id, kw);
        self.changed |= self.taffy.set_contain(id, contain);
        if self.changed {
            self.taffy.mark_dirty(id);
        }
        self.changed |= outer;
        id
    }

    fn layout_node_inner(&mut self, i: usize) -> NodeId {
        if self.st(i).display == DisplayKind::Table {
            self.table_columns(i);
        }
        let sid = self.nodes[i].style;
        match &self.nodes[i].content {
            Content::Widget(spec) => {
                let (w, h) = (self.widgets)(self.nodes[i].key, WidgetAsk::Ensure(spec));
                let mut s = self.box_style(i);
                s.item_is_compressible_replaced = s.size.width == Dimension::auto();
                if s.size.width == Dimension::auto() {
                    s.size.width = Dimension::length(w);
                }
                if spec.grow && s.size.height == Dimension::auto() {
                    // as tall as its wrapped text, at least its rows
                    let hash = hash_parts(&[spec.text.as_deref().unwrap_or_default().as_bytes()]);
                    let key = self.nodes[i].key;
                    s.padding = Rect::zero(); // the widget insets its text by the padding itself
                    if s.min_size.height == LengthPercentageAuto::auto() {
                        s.min_size.height = LengthPercentageAuto::length(h);
                    }
                    let id = self.node(key, s, Some(Measure::Area { key, hash }), &[]);
                    self.nodes[i].taffy = Some(id);
                    return id;
                }
                if s.size.height == Dimension::auto() {
                    s.size.height = Dimension::length(h);
                    // a control's automatic minimum height is its own: a flex column with a
                    // definite height must scroll or overflow, never squash it to nothing (unless
                    // the author capped the height: the cap wins)
                    if s.min_size.height == LengthPercentageAuto::auto()
                        && s.max_size.height == LengthPercentageAuto::auto()
                    {
                        s.min_size.height = LengthPercentageAuto::length(h);
                    }
                }
                return self.leaf(i, s);
            }
            Content::Replaced(r) => {
                let (iw, ih) = match r {
                    Replaced::Image(img) => (img.width as f32, img.height as f32),
                    Replaced::File(file, _) => (file.width as f32, file.height as f32),
                    Replaced::Video(v) => v.size().map_or((300.0, 150.0), |(w, h)| (w as f32, h as f32)),
                    _ => (300.0, 150.0),
                };
                let attr = |n: &str| self.attr(i, n).and_then(|v| v.trim_end_matches("px").parse::<f32>().ok());
                let mut s = self.box_style(i);
                // (an auto width only: taffy resolves the others, sizing keywords too, as for any box)
                s.item_is_replaced = s.size.width == Dimension::auto();
                let (aw, ah) = (attr("width"), attr("height"));
                // a canvas's `width` / `height` are its bitmap's: its natural size
                let svg = self.els.get(self.nodes[i].el as usize).is_some_and(|e| e.tag == "svg");
                let canvas = svg || matches!(r, Replaced::Painted(_) | Replaced::Placeholder("canvas"));
                let (iw, ih) = if canvas { (aw.unwrap_or(300.0), ah.unwrap_or(150.0)) } else { (iw, ih) };
                // an inline <svg> without sizes takes its `viewBox`'s ratio (its picture scales)
                let view_box = self.attr(i, "viewBox").and_then(|v| {
                    let n: Vec<f32> = v.split([' ', ',']).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect();
                    (n.len() == 4 && n[2] > 0.0 && n[3] > 0.0).then(|| (n[2], n[3]))
                });
                let (iw, ih) = match view_box {
                    Some(vb) if aw.is_none() && ah.is_none() => vb,
                    _ => (iw, ih),
                };
                // an image's are sizes under any CSS one
                if !canvas && s.size.width == Dimension::auto() && let Some(w) = aw {
                    s.size.width = Dimension::length(w);
                }
                if !canvas && s.size.height == Dimension::auto() && let Some(h) = ah {
                    s.size.height = Dimension::length(h);
                }
                // an auto size follows the other through a ratio, as in CSS: the
                // `aspect-ratio`, unless `auto` comes with it; the picture's own; the
                // attributes' (`aspect-ratio: auto w / h` in the UA sheet). A `width: 70%`
                // image is as tall as 70% of the width makes it
                let attrs = aw.zip(ah).filter(|&(_, h)| h > 0.0).map(|(w, h)| w / h);
                let natural = (ih > 0.0).then(|| iw / ih);
                // a natural ratio is the content box's, which the measure keeps; taffy would make it
                // the border box's, so it gives it only when they are the same box
                let zero = |l: LengthPercentage| l == LengthPercentage::length(0.0);
                let bare = [s.padding, s.border].iter().all(|r| zero(r.left) && zero(r.right) && zero(r.top) && zero(r.bottom));
                s.aspect_ratio = if self.styles[sid].aspect_auto || s.aspect_ratio.is_none() {
                    natural.or(s.aspect_ratio).or(attrs).filter(|_| bare)
                } else {
                    s.aspect_ratio
                };
                // measured: its natural size, or through its ratio (CSS 2 §10.3.2, §10.6.2), which
                // is also its automatic minimum size as a flex or grid item
                let key = self.nodes[i].key;
                let id = self.node(key, s, Some(Measure::Replaced { width: iw, height: ih }), &[]);
                self.nodes[i].taffy = Some(id);
                return id;
            }
            Content::LineBreak => {
                // in lines, a forced break (their layout makes the empty line two of them leave)
                let s = Style { size: Size { width: Dimension::percent(1.0), height: Dimension::length(0.0) },
                                flex_shrink: 0.0, ..Default::default() };
                return self.leaf(i, s);
            }
            Content::Text(_) => {
                // a text run as a flex/grid item: measured, wrapping. The box
                // keeps the transformed text, so paint draws what was measured.
                self.transform_text(i, sid);
                let nowrap = matches!(self.styles[sid].white_space, WhiteSpace::NoWrap | WhiteSpace::Pre);
                let key = self.nodes[i].key;
                let m = self.text_measure(key, i, sid, nowrap);
                let id = self.node(key, Style::default(), Some(m), &[]);
                self.text_baseline(id, sid);
                self.nodes[i].taffy = Some(id);
                return id;
            }
            _ => {}
        }
        if let Some(k) = self.foldable_text(i).filter(|_| !self.run_ins.contains_key(&i)) {
            // one leaf for the box and its text: the text is measured as the
            // box's content and placed inside it after layout
            self.transform_text(k, sid);
            let nowrap = self.styles[sid].white_space == WhiteSpace::NoWrap;
            let key = self.nodes[i].key;
            let m = self.text_measure(key, k, sid, nowrap);
            let shape = self.shape(i, Form::Folded);
            let id = self.node_from(key, shape, |b| Style { display: Display::Block, ..b.box_style(i) }, Some(m), &[]);
            // its text is at the top of the box unless the box aligns it lower as a flex item
            let c = &self.styles[sid];
            let top = |a: AlignItems| matches!(a, AlignItems::NORMAL | AlignItems::STRETCH | AlignItems::START
                                                  | AlignItems::FLEX_START | AlignItems::BASELINE);
            let lower = matches!(c.display, DisplayKind::Flex | DisplayKind::InlineFlex) && match c.layout.flex_direction {
                FlexDirection::Row | FlexDirection::RowReverse => !top(c.layout.align_items),
                _ => !matches!(c.layout.justify_content, JustifyContent::NORMAL | JustifyContent::START | JustifyContent::FLEX_START),
            };
            if lower {
                self.changed |= self.taffy.set_baseline(id, None);
            } else {
                self.text_baseline(id, sid);
            }
            self.nodes[i].taffy = Some(id);
            self.nodes[i].fold = Some(k);
            self.nodes[k].folded = true;
            return id;
        }
        let c = self.styles[sid].clone_display();
        let flex_like = matches!(c.display, DisplayKind::Flex | DisplayKind::InlineFlex
                                 | DisplayKind::Grid | DisplayKind::InlineGrid
                                 | DisplayKind::Table | DisplayKind::TableRowGroup
                                 | DisplayKind::TableRow);
        let mut list: Kids = kids(&self.nodes, i).collect();
        if let Some(&r) = self.run_ins.get(&i) {
            list.insert(0, r);
        }
        if list.iter().any(|&k| self.styles[self.nodes[k].style].run_in) {
            self.adopt_run_ins(&mut list);
        }
        // a `display: contents` element has no box: its children are this box's (CSS Display 2.5)
        if list.iter().any(|&k| self.contents(k)) {
            let mut flat = Kids::new();
            self.flatten_contents(&list, &mut flat);
            list = flat;
        }
        let ordered = |b: &Builder, k: usize| {
            let st = b.st(k);
            if st.display == DisplayKind::TableCaption && st.caption_bottom { i32::MAX } else { st.order }
        };
        if list.iter().any(|&k| ordered(self, k) != 0) {
            list.sort_by_key(|&k| ordered(self, k));
        }
        let mut out: NodeList = NodeList::new();
        if flex_like {
            for k in list {
                // white space alone isn't an item, whatever `white-space` says (Flexbox §4, Grid §6)
                if let Content::Text(t) = &self.nodes[k].content {
                    if t.trim().is_empty() {
                        continue;
                    }
                    // text that needs inline layout (break opportunities, spacing ...) is an
                    // anonymous block of lines, as browsers make it
                    if !simple_text(t, self.st(k)) {
                        self.flush_run(i, &mut smallvec::smallvec![k], &mut out);
                        continue;
                    }
                }
                self.child_nodes(k, &mut out);
            }
            let shape = self.shape(i, Form::Plain);
            let id = self.node_from(self.nodes[i].key, shape, |b| b.box_style(i), None, &out);
            self.changed |= self.taffy.set_subgrid(id, self.styles[sid].subgrid);
            self.changed |= self.taffy.set_lanes(id, self.styles[sid].lanes());
            self.nodes[i].taffy = Some(id);
            return id;
        }
        // block container: runs of inline content become line boxes
        if list.iter().all(|&k| !self.is_block_level(k)) {
            let items = self.inline_items(&list, i, &mut out);
            let shape = self.shape(i, Form::Lines);
            let id = self.node_from(self.nodes[i].key, shape, |b| b.box_style(i), None, &out);
            let inline = self.inline_of(&self.styles[sid], items, true);
            self.changed |= self.taffy.set_inline(id, Some(inline));
            self.inlined = true;
            self.nodes[i].taffy = Some(id);
            return id;
        }
        let mut run: Kids = Kids::new();
        for k in list {
            if self.is_block_level(k) {
                self.flush_run(i, &mut run, &mut out);
                self.child_nodes(k, &mut out);
            } else {
                run.push(k);
            }
        }
        self.flush_run(i, &mut run, &mut out);
        let shape = self.shape(i, Form::Plain);
        let id = self.node_from(self.nodes[i].key, shape, |b| b.box_style(i), None, &out);
        self.nodes[i].taffy = Some(id);
        id
    }

    /// The lone text child of element `i`, when laying the pair out as one
    /// measured leaf gives the same result as two nodes (or a line of words).
    fn foldable_text(&self, i: usize) -> Option<usize> {
        let n = &self.nodes[i];
        if !matches!(n.content, Content::Element) || n.first == NONE || n.first != n.last {
            return None;
        }
        let k = n.first as usize;
        let c = self.st(i);
        match &self.nodes[k].content {
            Content::Text(t) if !t.trim().is_empty() && (simple_text(t, c) || ellipsized_line(t, c)) => {}
            _ => return None,
        }
        let s = &c.layout;
        let display_ok = match c.display {
            DisplayKind::Block | DisplayKind::InlineBlock | DisplayKind::ListItem
            | DisplayKind::TableCell | DisplayKind::TableCaption => true,
            DisplayKind::Flex | DisplayKind::InlineFlex => {
                s.flex_wrap == FlexWrap::NoWrap
                    && matches!(s.flex_direction, FlexDirection::Row | FlexDirection::Column)
            }
            _ => false,
        };
        (display_ok
            && c.letter_spacing == 0.0
            && c.word_spacing == 0.0
            && c.text_indent == crate::geometry::Length::Px(0.0)
            && c.vertical_align == VAlign::Baseline
            && !c.small_caps
            && matches!(c.white_space, WhiteSpace::Normal | WhiteSpace::NoWrap)
            // a scroll container moves its content: only clipping ones fold
            && s.overflow.x != Overflow::Scroll
            && s.overflow.y != Overflow::Scroll)
            .then_some(k)
    }

    /// `display: run-in` (CSS Display 2.4): a run-in followed by a block box (white space between
    /// aside) becomes an inline box at the start of that block, taken out of `list`; any other stays
    /// a block box.
    fn adopt_run_ins(&mut self, list: &mut Kids) {
        let mut k = 0;
        while k < list.len() {
            let r = list[k];
            let next = list[k + 1..].iter().copied()
                .find(|&n| !matches!(&self.nodes[n].content, Content::Text(t) if t.trim().is_empty()
                    && matches!(self.st(n).white_space, WhiteSpace::Normal | WhiteSpace::NoWrap)));
            let st = &self.styles[self.nodes[r].style];
            let target = next.filter(|&n| {
                let s = self.st(n);
                st.run_in && matches!(self.nodes[n].content, Content::Element) && !s.run_in
                    && matches!(s.display, DisplayKind::Block | DisplayKind::ListItem)
                    && s.layout.display != Display::FlowRoot && !s.layout.position.is_out_of_flow()
                    && s.layout.float == taffy::style::Float::None
            });
            // (one holding a block-level box, a run-in's too, stays a block)
            match target {
                Some(n) if matches!(self.nodes[r].content, Content::Element) && !self.holds_block(r) => {
                    let inline = Computed { display: DisplayKind::Inline, run_in: false, ..st.clone() };
                    self.nodes[r].style = self.styles.add(inline);
                    self.run_ins.insert(n, r);
                    list.remove(k);
                }
                _ => k += 1,
            }
        }
    }

    /// Whether box `k` holds a block-level box (through its inline ones).
    fn holds_block(&self, k: usize) -> bool {
        // (a table cell, row or caption among inline content is wrapped in an inline table)
        let table_part = |c: usize| matches!(self.st(c).display, DisplayKind::TableCell | DisplayKind::TableRow
            | DisplayKind::TableRowGroup | DisplayKind::TableCaption);
        kids(&self.nodes, k).any(|c| (self.is_block_level(c) && !table_part(c))
            || (matches!(self.nodes[c].content, Content::Element) && (self.st(c).run_in || self.holds_block(c))))
    }

    /// Whether box `k` is an element with `display: contents`.
    fn contents(&self, k: usize) -> bool {
        matches!(self.nodes[k].content, Content::Element) && self.st(k).display == DisplayKind::Contents
    }

    /// `list` with its `display: contents` elements replaced by their children, in `out`.
    fn flatten_contents(&self, list: &[usize], out: &mut Kids) {
        for &k in list {
            if self.contents(k) {
                let inner: Kids = kids(&self.nodes, k).collect();
                self.flatten_contents(&inner, out);
            } else {
                out.push(k);
            }
        }
    }

    fn child_nodes(&mut self, k: usize, out: &mut NodeList) {
        if self.st(k).display == DisplayKind::Contents {
            let list: Kids = kids(&self.nodes, k).collect();
            for c in list {
                self.child_nodes(c, out);
            }
            return;
        }
        out.push(self.layout_node(k));
    }

    /// An anonymous line box for inline content between blocks.
    fn flush_run(&mut self, parent: usize, run: &mut Kids, out: &mut NodeList) {
        let only_space = run.iter().all(|&k| matches!(&self.nodes[k].content,
                                                      Content::Text(t) if t.trim().is_empty()));
        if run.is_empty() || only_space {
            run.clear();
            return;
        }
        let style = self.nodes[parent].style;
        let key = child_key(self.nodes[parent].key, "#anon", out.len());
        let anon = self.push(Some(parent), Content::Anon, Some(parent), style, key, false);
        let mut line = NodeList::new();
        let run: Kids = std::mem::take(run);
        let items = self.inline_items(&run, parent, &mut line);
        let id = self.node(key, Style { display: Display::Block, ..Default::default() }, None, &line);
        // `text-indent` is the block's first line's: here when nothing block-level came before
        let inline = self.inline_of(&self.styles[style], items, out.is_empty());
        self.changed |= self.taffy.set_inline(id, Some(inline));
        if self.changed {
            self.taffy.mark_dirty(id);
        }
        self.nodes[anon].taffy = Some(id);
        out.push(id);
    }

}

impl Computed {
    /// The few fields `layout_node` reads after building children (avoids
    /// holding a borrow of the style store across recursion).
    fn clone_display(&self) -> DisplayOnly {
        DisplayOnly { display: self.display }
    }
}

struct DisplayOnly {
    display: DisplayKind,
}

/// Build, cascade and lay out `root` in a `viewport` (logical pixels),
/// reusing the arenas of `recycle` (the previous frame) and `caches`.
pub(crate) fn layout(root: AnyElement, viewport: (f32, f32), state: &UiState, caches: &mut LayoutCaches,
                     recycle: Frame, widgets: &mut dyn FnMut(Key, WidgetAsk) -> (f32, f32)) -> Frame {
    let mut css = String::new();
    let mut title = None;
    caches.tw.on = TAILWIND.load(std::sync::atomic::Ordering::Relaxed);
    collect_styles(&root, &mut css, &mut title, &mut caches.tw);
    if caches.tw.on && !caches.tw.rules.is_empty() {
        let utilities = caches.tw.css(&css);
        if !utilities.is_empty() {
            let mut all = String::with_capacity(css.len() + utilities.len() + 1);
            all.push_str(&css);
            all.push('\n');
            all.push_str(utilities);
            css = all;
        }
    }
    // the pointer and the focus restyle only where the sheet says so
    caches.state_rules = [":hover", ":active", ":focus"].iter().any(|p| css.contains(p));
    let css_hash = hash_parts(&[css.as_bytes()]);
    if css_hash != caches.author.hash {
        caches.author = AuthorSheet { hash: css_hash, sheet: Stylesheet::parse(&css) };
        load_font_faces(&caches.author.sheet);
    }
    caches.styles.begin_frame();
    caches.text.begin_frame();
    caches.styles.seed_properties(&caches.author.sheet, caches.author.hash);
    // the taffy tree persists: drop last frame's duplicates, start a new stamp
    for id in caches.recon.extra.drain(..) {
        caches.taffy.remove(id);
    }
    caches.recon.stamp = caches.recon.stamp.wrapping_add(1);
    caches.text_box.clear();
    if caches.taffy.style_count() >= caches.styles_limit {
        caches.taffy.compact_styles();
        caches.shapes.clear();
        caches.styles_limit = (caches.taffy.style_count() * 2).max(STYLES_MAX);
    }
    let Frame { mut nodes, mut els, mut wide, .. } = recycle;
    nodes.clear();
    els.clear();
    wide.clear();
    if caches.matched.author != caches.author.hash || caches.matched.map.len() > 50_000 {
        caches.matched.map.clear();
        caches.matched.author = caches.author.hash;
    }
    caches.images.videos.begin_pass();
    let ua_styled = ua_styled_stylesheet();
    caches.viewport = viewport;
    let LayoutCaches { styles, text, images, author, taffy, box_of, acc, matched, recon, text_box, shapes, containers,
                       anchors, wrap_widths, anim, .. } = caches;
    let mut b = Builder {
        nodes,
        els,
        taffy,
        styles,
        text,
        images,
        matched,
        recon,
        text_box,
        shapes,
        containers,
        anchors,
        wrap_widths,
        changed: false,
        next_tpos: (1, 1),
        author: &author.sheet,
        ua_styled: &ua_styled,
        // the theme's mode is part of what the sheet means (`light-dark()`, system colors,
        // `prefers-color-scheme`): styles computed in the other mode are not reused
        author_hash: author.hash ^ u64::from(crate::color4::dark()),
        ua_media: ua_stylesheet().media_results(&crate::css::MediaEnv::viewport(viewport)),
        ua_styled_media: ua_styled.media_results(&crate::css::MediaEnv::viewport(viewport)),
        au_media: author.sheet.media_results(&crate::css::MediaEnv::viewport(viewport)),
        state,
        viewport,
        title,
        widgets,
        counters: Vec::new(),
        table_cols: KeyMap::default(),
        anim,
        inlined: false,
        run_ins: KeyMap::default(),
    };
    // the document: <html><body> unless the app built them itself
    let root_div = match root {
        AnyElement::Element(d) if d.tag == "html" => d,
        AnyElement::Element(d) if d.tag == "body" => crate::html::html().child(d),
        other => {
            let mut body = crate::html::body();
            body.children.push(other);
            crate::html::html().child(body)
        }
    };
    let s = span(Phase::Build);
    let root = match b.element(root_div, None, ROOT_KEY, (1, 1), &[]) {
        Some(r) => r,
        None => b.push(None, Content::Element, None, ROOT_STYLE, ROOT_KEY, false),
    };
    drop(s);
    let s = span(Phase::Boxes);
    let root_id = b.layout_node(root);
    drop(s);
    let s = span(Phase::Taffy);
    // nodes not used this frame
    let stamp = b.recon.stamp;
    let taffy_ref = &mut *b.taffy;
    b.recon.map.retain(|_, (id, st)| {
        if *st == stamp {
            return true;
        }
        taffy_ref.remove(*id);
        false
    });
    let avail = Size { width: AvailableSpace::Definite(viewport.0),
                       height: AvailableSpace::Definite(viewport.1) };
    let laid = Some((root_id, (viewport.0.to_bits(), viewport.1.to_bits())));
    let stale = std::mem::replace(&mut b.recon.laid, laid) != laid || b.changed;
    let Builder { nodes, els, taffy, styles, text: tc, title, text_box, widgets, .. } = b;
    if stale {
        taffy.compute(root_id, avail, |known, available, ctx| {
                match ctx {
                    Measure::Area { key, .. } => {
                        let w = known.width.unwrap_or(match available.width {
                            AvailableSpace::Definite(w) => w,
                            _ => 0.0,
                        });
                        Size { width: known.width.unwrap_or(w), height: known.height.unwrap_or_else(|| widgets(*key, WidgetAsk::Height(w)).1) }
                    }
                    Measure::Word { width, line } => Size { width: known.width.unwrap_or(*width),
                                                            height: known.height.unwrap_or(*line) },
                    // `known` is the border box; the content box, when known, is the definite space
                    Measure::Replaced { width, height } => {
                        let content = |k: Option<f32>, a: AvailableSpace| k.and(match a {
                            AvailableSpace::Definite(v) => Some(v),
                            _ => None,
                        });
                        match (content(known.width, available.width), content(known.height, available.height)) {
                            (Some(w), Some(h)) => Size { width: w, height: h },
                            (Some(w), None) => Size { width: w, height: if *width > 0.0 { w * height / width } else { *height } },
                            (None, Some(h)) => Size { width: if *height > 0.0 { h * width / height } else { *width }, height: h },
                            (None, None) => Size { width: *width, height: *height },
                        }
                    }
                    Measure::Text { key, size, flags, line, nowrap, brk, .. } => {
                        let text = match text_box.get(key).map(|&k| &nodes[k].content) {
                            Some(Content::Text(t)) => t.as_ref(),
                            _ => "",
                        };
                        let max_w = known.width.unwrap_or(match available.width {
                            AvailableSpace::Definite(w) => w,
                            AvailableSpace::MinContent => 0.0,
                            AvailableSpace::MaxContent => f32::INFINITY,
                        });
                        let wrapped = tc.wrap(text, *size, *flags, if *nowrap { f32::INFINITY } else { max_w }, *brk);
                        Size { width: known.width.unwrap_or(wrapped.width.ceil()),
                               height: known.height.unwrap_or(*line * wrapped.lines.len().max(1) as f32) }
                    }
                }
            });
    }
    drop(s);
    let _s = span(Phase::Place);
    let (listen, any_wide) = els.iter().filter_map(|e| e.listeners.as_deref())
        .fold((0, false), |(m, w), l| (m | l.kinds, w || l.entries.iter().any(|en| en.wide)));
    if any_wide {
        wide.extend((0..nodes.len() as u32).filter(|&i| {
            let e = nodes[i as usize].el;
            e != NONE && els[e as usize].listeners.as_deref().is_some_and(|l| l.entries.iter().any(|en| en.wide))
        }));
    }
    let mut frame = Frame { nodes, els, listen, wide, title, by_key: Default::default(), hit_index: Default::default() };
    box_of.clear();
    for (i, n) in frame.nodes.iter().enumerate() {
        if let Some(t) = n.taffy {
            let s = slot(t);
            if s >= box_of.len() {
                box_of.resize(s + 1, NONE);
            }
            box_of[s] = i as u32;
        }
    }
    let mut walk = Walk { t: taffy, box_of, state, vp: viewport, styles, text: tc, seq: 0,
                          port: rect(0.0, 0.0, viewport.0, viewport.1), within: rect(0.0, 0.0, viewport.0, viewport.1) };
    walk.place(&mut frame.nodes, root_id, 0.0, 0.0, None, (0.0, 0.0));
    finish(&mut frame.nodes, styles, acc);
    frame
}

/// The edges of a box's `clip` (css2 11.1.2): only an absolutely positioned box has one.
/// Text the one-leaf text paths (a box with its lone text, a text run as a flex or grid item) lay
/// out as inline layout would: printable ASCII words between spaces (no hyphens, slashes, tabs or
/// other break opportunities), in white space that collapses, aligned but not justified, with no
/// line-breaking or spacing options.
fn simple_text(t: &str, c: &Computed) -> bool {
    t.bytes().all(|b| b == b' ' || (b.is_ascii_graphic() && !matches!(b, b'-' | b'/' | b'?' | b'!' | b'%' | b'$' | b'}' | b')' | b']')))
        && c.text_align != TextAlign::Justify
        && c.text_wrap == 0
        && !c.break_all
        && !c.wrap_anywhere
}

/// A line cut with an ellipsis (`white-space: nowrap`, `text-overflow: ellipsis`) of any left-to-right
/// text: it never breaks, so its break opportunities and characters (hyphens, brackets, quotes,
/// Indic scripts ...) lay out alike either way, and only the one-leaf path ellipsizes (inline
/// layout clips it mid-glyph). Right-to-left text stays inline, where it is reordered.
fn ellipsized_line(t: &str, c: &Computed) -> bool {
    c.white_space == WhiteSpace::NoWrap
        && c.ellipsizes()
        && c.text_align != TextAlign::Justify
        && c.layout.direction != Direction::Rtl
        && text::bidi_visual(t, false, false).is_none()
}

fn clip_edges(st: &Computed) -> Option<[Option<crate::geometry::Length>; 4]> {
    st.clip_rect.filter(|_| matches!(st.layout.position, Position::Absolute | Position::Fixed))
}

/// Whether a box's `overflow` clips what is under it to its padding box.
fn overflow_clips(st: &Computed) -> bool {
    let ov = st.layout.overflow;
    ov.x != Overflow::Visible || ov.y != Overflow::Visible
}

/// Whether a redraw that moves box `j` (with everything under it) leaves
/// clips that are not the old ones moved: `j` clips (its `overflow`, or its
/// `clip`) inside a clip from above, which stays where it is, so the clips
/// it makes are new intersections. Then the frame is placed again
/// ([`replace`]); without a clip above, the clips move with the box.
pub(crate) fn move_reclips(nodes: &[BoxNode], styles: &StyleStore, j: usize) -> bool {
    let st = &styles[nodes[j].style];
    nodes[j].clip.is_some() && (overflow_clips(st) || clip_edges(st).is_some())
}

/// Place `frame` again where only the scroll offsets (`state.scroll`) changed
/// since it was laid out: the taffy tree and the boxes are the ones of that
/// layout, so only positions, clips and the scroll positions are redone.
/// False when the frame has no root to place from.
pub(crate) fn replace(frame: &mut Frame, viewport: (f32, f32), state: &UiState, caches: &mut LayoutCaches) -> bool {
    let Some(root) = frame.nodes.first().and_then(|n| n.taffy) else { return false };
    let LayoutCaches { styles, text, taffy, box_of, acc, .. } = caches;
    let mut walk = Walk { t: taffy, box_of, state, vp: viewport, styles, text, seq: 0,
                          port: rect(0.0, 0.0, viewport.0, viewport.1), within: rect(0.0, 0.0, viewport.0, viewport.1) };
    walk.place(&mut frame.nodes, root, 0.0, 0.0, None, (0.0, 0.0));
    finish(&mut frame.nodes, styles, acc);
    frame.moved();
    true
}

/// Positions follow the taffy tree (words live in line containers, which may
/// be anonymous); translate and scroll offsets shift whole subtrees.
struct Walk<'a> {
    t: &'a crate::tree::LayoutTree<Measure>,
    box_of: &'a [u32],
    state: &'a UiState,
    vp: (f32, f32),
    styles: &'a StyleStore,
    text: &'a mut TextCache,
    /// Boxes placed so far (`BoxNode::seq`).
    seq: u32,
    /// The nearest scrollport (the viewport, or a scroll container's padding box) and the content
    /// box of the box being placed into: what a sticky box sticks to and stays within.
    port: Bounds<f32>,
    within: Bounds<f32>,
}

impl Walk<'_> {
    /// `cb`: the origin of the containing block of absolutely positioned
    /// descendants (taffy places those relative to it, not to their parent).
    fn place(&mut self, nodes: &mut [BoxNode], id: NodeId, ox: f32, oy: f32, clip: Option<Bounds<f32>>,
             cb: (f32, f32)) {
        let l = *self.t.layout(id);
        let (mut x, mut y) = (ox + l.location.x, oy + l.location.y);
        let (w, h) = (l.size.width, l.size.height);
        if self.t.style(id).position == Position::Sticky {
            let (dx, dy) = self.sticky(id, rect(x, y, w, h), &l);
            x += dx;
            y += dy;
        }
        let mut child_clip = clip;
        let (mut sx, mut sy) = (0.0, 0.0);
        let mut port = None;
        if let Some(i) = self.box_of.get(slot(id)).filter(|&&b| b != NONE).map(|&b| b as usize) {
            let st = &self.styles[nodes[i].style];
            let u = st.units(self.vp);
            // a word carries its owner's style: the owner's translate already moved the line it is in,
            // unless the owner is an inline element (no box of its own to carry it)
            let own_translate = !matches!(nodes[i].content, Content::Word { .. }) || nodes[nodes[i].owner].taffy.is_none();
            if own_translate {
                x += st.translate.0.resolve(&u, w);
                y += st.translate.1.resolve(&u, h);
            }
            if matches!(nodes[i].content, Content::Word { .. }) {
                y += match st.vertical_align {
                    VAlign::Sub => st.font_size * 0.25,
                    VAlign::Super => -st.font_size * 0.35,
                    VAlign::Baseline | VAlign::Top | VAlign::Middle | VAlign::Bottom => 0.0,
                };
            }
            nodes[i].bounds = rect(x, y, w, h);
            nodes[i].clip = clip;
            nodes[i].seq = self.seq;
            self.seq += 1;
            if let Some(cr) = clip_edges(st) {
                // edges from the border box's top-left corner; auto = that edge
                let e = |k: usize, auto: f32, basis: f32| cr[k].map_or(auto, |l| l.resolve(&u, basis));
                let (top, right, bottom, left) = (e(0, 0.0, h), e(1, w, w), e(2, h, h), e(3, 0.0, w));
                let r = rect(x + left, y + top, (right - left).max(0.0), (bottom - top).max(0.0));
                let r = match clip {
                    Some(c) => c.intersect(&r),
                    None => r,
                };
                nodes[i].clip = Some(r);
                child_clip = Some(r);
            }
            if overflow_clips(st) {
                let ov = st.layout.overflow;
                let b = l.border;
                let pad = rect(x + b.left, y + b.top, (w - b.left - b.right).max(0.0),
                               (h - b.top - b.bottom).max(0.0));
                child_clip = Some(match clip {
                    Some(c) => c.intersect(&pad),
                    None => pad,
                });
                // `hidden` is a scroll container too, scrolled by programs only: its range moves it,
                // the user's scrolling (scrollbars, wheel: `scroll_range`) is `scroll`'s
                let scrolls = |o: Overflow| matches!(o, Overflow::Scroll | Overflow::Hidden);
                if scrolls(ov.x) || scrolls(ov.y) {
                    port = Some(pad);
                    let axis = |o: Overflow, content: f32, view: f32, user: bool| {
                        if o == Overflow::Scroll || (!user && o == Overflow::Hidden) { (content - view).max(0.0) } else { 0.0 }
                    };
                    let range = |user| (axis(ov.x, l.scrollable_overflow_rect.right, pad.size.width, user),
                                        axis(ov.y, l.scrollable_overflow_rect.bottom, pad.size.height, user));
                    nodes[i].scroll_range = range(true);
                    let (ax, ay) = self.state.scroll.get(&nodes[i].key).copied().unwrap_or((0.0, 0.0));
                    let all = range(false);
                    sx = ax.clamp(0.0, all.0);
                    sy = ay.clamp(0.0, all.1);
                    nodes[i].scroll_pos = (sx, sy);
                }
            }
            // the root scrolls as the viewport does, by programs (`window.scrollTo`) only
            if i == 0 && !overflow_clips(st) {
                if let Some(&(ax, ay)) = self.state.scroll.get(&nodes[i].key) {
                    let o = l.scrollable_overflow_rect;
                    sx = ax.clamp(0.0, (o.right.max(w) - self.vp.0).max(0.0));
                    sy = ay.clamp(0.0, (o.bottom.max(h) - self.vp.1).max(0.0));
                    nodes[i].scroll_pos = (sx, sy);
                }
            }
            if let Some(k) = nodes[i].fold {
                place_folded_text(nodes, i, k, rect(x, y, w, h), &l, self.styles, self.text);
                // the text is clipped by its container's `overflow`
                nodes[k].clip = child_clip;
                nodes[k].seq = self.seq;
                self.seq += 1;
            }
        }
        let inner = (x - sx, y - sy);
        let cb = if self.t.style(id).position == Position::Static { cb } else { inner };
        let (b, p) = (l.border, l.padding);
        let content = rect(inner.0 + b.left + p.left, inner.1 + b.top + p.top,
                           (w - b.left - b.right - p.left - p.right).max(0.0), (h - b.top - b.bottom - p.top - p.bottom).max(0.0));
        let saved = (self.port, std::mem::replace(&mut self.within, content));
        if let Some(port) = port {
            self.port = port;
        }
        for &k in self.t.children(id) {
            let (kx, ky) = match self.t.style(k).position {
                Position::Absolute => cb,
                Position::Fixed => (0.0, 0.0),
                _ => inner,
            };
            self.place(nodes, k, kx, ky, child_clip, cb);
        }
        (self.port, self.within) = saved;
    }

    /// How far sticky box `id` (at `at`, laid out as `l`) moves (css-position-3 3.4): into the
    /// scrollport inset by its `top` / `right` / `bottom` / `left`, but its margin box no further
    /// than its containing block's content box; `top` and `left` win when both sides can't hold.
    fn sticky(&self, id: NodeId, at: Bounds<f32>, l: &taffy::Layout) -> (f32, f32) {
        let s = self.t.style(id);
        let (port, cb, m) = (self.port, self.within, l.margin);
        let inset = |v: LengthPercentageAuto, basis: f32| taffy::MaybeResolve::maybe_resolve(v, Some(basis), crate::tree::resolve_calc);
        let axis = |start: Option<f32>, end: Option<f32>, (pos, size): (f32, f32), (lo, hi): (f32, f32), (clo, chi): (f32, f32), (ms, me): (f32, f32)| {
            let mut d = 0.0f32;
            if let Some(e) = end {
                d = d.min(hi - e - (pos + size));
            }
            if let Some(s) = start {
                d = d.max(lo + s - pos);
            }
            if d > 0.0 {
                d.min((chi - me - size - pos).max(0.0))
            } else {
                d.max((clo + ms - pos).min(0.0))
            }
        };
        let (pw, ph) = (port.size.width, port.size.height);
        let dx = axis(inset(s.inset.left, pw), inset(s.inset.right, pw), (at.origin.x, at.size.width),
                      (port.origin.x, port.origin.x + pw), (cb.origin.x, cb.origin.x + cb.size.width), (m.left, m.right));
        let dy = axis(inset(s.inset.top, ph), inset(s.inset.bottom, ph), (at.origin.y, at.size.height),
                      (port.origin.y, port.origin.y + ph), (cb.origin.y, cb.origin.y + cb.size.height), (m.top, m.bottom));
        (dx, dy)
    }
}

/// Put a folded text run where its own flex item / line box would have gone:
/// inside the content box, aligned like a lone flex item (or by `text-align`
/// in a block, which paint applies across the full width).
fn place_folded_text(nodes: &mut [BoxNode], i: usize, k: usize, b: Bounds<f32>, l: &taffy::Layout,
                     styles: &StyleStore, tc: &mut TextCache) {
    let c = &styles[nodes[i].style];
    let cb = rect(b.origin.x + l.border.left + l.padding.left, b.origin.y + l.border.top + l.padding.top,
                  (b.size.width - l.border.left - l.border.right - l.padding.left - l.padding.right).max(0.0),
                  (b.size.height - l.border.top - l.border.bottom - l.padding.top - l.padding.bottom).max(0.0));
    let Content::Text(t) = &nodes[k].content else { return };
    let flags = text::flags(c);
    let line = c.line_px();
    let brk = text::break_mode(c);
    let (tw, th) = if !t.contains([' ', '\n']) && brk == 0 {
        (tc.width(t, c.font_size, flags).ceil(), line)
    } else {
        let max = if c.white_space == WhiteSpace::NoWrap { f32::INFINITY } else { cb.size.width + 0.5 };
        let wrapped = tc.wrap(t, c.font_size, flags, max, brk);
        (wrapped.width.ceil(), line * wrapped.lines.len().max(1) as f32)
    };
    // nowrap text overflows its container, unless that container ellipsizes
    // it: then the text is cut to the content box (paint adds the `…`)
    let tw = if c.white_space == WhiteSpace::NoWrap && !c.ellipsizes() { tw } else { tw.min(cb.size.width) };
    let s = &c.layout;
    // offset of a box of `size` in `room` for a start / center / end choice
    let at = |room: f32, size: f32, pos: u8| match pos {
        1 => (room - size) / 2.0,
        2 => room - size,
        _ => 0.0,
    };
    use taffy::style::{AlignContentKeyword as Ac, AlignItemsKeyword as Ai};
    let justify = match s.justify_content.keyword() {
        Ac::Center | Ac::SpaceAround | Ac::SpaceEvenly => 1,
        Ac::End | Ac::FlexEnd => 2,
        _ => 0,
    };
    let align = match s.align_items.keyword() {
        Ai::Center => 1,
        Ai::End | Ai::FlexEnd => 2,
        _ => 0,
    };
    let stretch = matches!(s.align_items.keyword(), Ai::Normal | Ai::Stretch);
    let tb = match c.display {
        DisplayKind::Flex | DisplayKind::InlineFlex if s.flex_direction == FlexDirection::Row => {
            rect(cb.origin.x + at(cb.size.width, tw, justify), cb.origin.y + at(cb.size.height, th, align), tw, th)
        }
        DisplayKind::Flex | DisplayKind::InlineFlex => {
            let w = if stretch { cb.size.width } else { tw };
            rect(cb.origin.x + at(cb.size.width, w, align), cb.origin.y + at(cb.size.height, th, justify), w, th)
        }
        _ => rect(cb.origin.x, cb.origin.y, cb.size.width, th), // block: text-align at paint
    };
    nodes[k].bounds = tb;
}

/// Opacity and visibility as boxes inherit them (parents precede children in
/// the arena): after layout, and on redraws that change them.
pub(crate) fn inherit(nodes: &mut [BoxNode], styles: &StyleStore) {
    inherit_at(nodes, styles, 0..nodes.len());
}

/// Whether a box of this content has its own style (text, words and line
/// boxes carry their element's: its opacity, effects and animations).
pub(crate) const fn owns_style(content: &Content) -> bool {
    matches!(content, Content::Element | Content::Widget(_) | Content::Replaced(_))
}

/// [`inherit`] for the boxes `at` (ascending: parents first).
pub(crate) fn inherit_at(nodes: &mut [BoxNode], styles: &StyleStore, at: impl Iterator<Item = usize>) {
    for i in at {
        let (po, pv) = nodes[i].parent.map_or((1.0, true), |p| (nodes[p].opacity, nodes[p].visible));
        let st = &styles[nodes[i].style];
        let own = owns_style(&nodes[i].content);
        nodes[i].opacity = if own { po * st.opacity } else { po };
        nodes[i].visible = pv && st.visible;
    }
}

/// Inherited opacity / visibility (parents precede children in the arena),
/// and bounds of boxes without a layout node (inline elements): the union of
/// their pieces (children follow parents, so a reverse pass sees them first).
fn finish(nodes: &mut [BoxNode], styles: &StyleStore, acc: &mut Vec<Option<Bounds<f32>>>) {
    inherit(nodes, styles);
    for i in 0..nodes.len() {
        if nodes[i].taffy.is_none() && !nodes[i].folded {
            nodes[i].clip = nodes[i].parent.and_then(|p| nodes[p].clip);
        }
    }
    acc.clear();
    acc.resize(nodes.len(), None);
    for i in (0..nodes.len()).rev() {
        if nodes[i].taffy.is_none() {
            if let Some(a) = acc[i] {
                nodes[i].bounds = a;
            }
        }
        let Some(p) = nodes[i].parent else { continue };
        if nodes[p].taffy.is_some() {
            continue;
        }
        let cb = nodes[i].bounds;
        if cb.size.width <= 0.0 && cb.size.height <= 0.0 {
            continue;
        }
        acc[p] = Some(match acc[p] {
            None => cb,
            Some(a) => {
                let x0 = a.origin.x.min(cb.origin.x);
                let y0 = a.origin.y.min(cb.origin.y);
                let x1 = (a.origin.x + a.size.width).max(cb.origin.x + cb.size.width);
                let y1 = (a.origin.y + a.size.height).max(cb.origin.y + cb.size.height);
                rect(x0, y0, x1 - x0, y1 - y0)
            }
        });
    }
}

impl Frame {
    /// Element boxes by key, built on first use (events and tests need it,
    /// rendering doesn't).
    pub(crate) fn by_key(&self) -> &KeyMap<Key, usize> {
        self.by_key.get_or_init(|| {
            self.nodes.iter().enumerate().filter(|(_, n)| n.el != NONE).map(|(i, n)| (n.key, i)).collect()
        })
    }

    /// The element data of box `i`.
    pub(crate) fn el(&self, i: usize) -> Option<&ElData> {
        let e = self.nodes[i].el;
        (e != NONE).then(|| &self.els[e as usize])
    }

    pub(crate) fn attr(&self, i: usize, name: &str) -> Option<&str> {
        el_attr(&self.nodes, &self.els, i, name)
    }

    /// Element children of box `i`.
    pub(crate) fn children_of(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        kids(&self.nodes, i).filter(|&k| matches!(self.nodes[k].content, Content::Element | Content::Widget(_)
                                                  | Content::Replaced(_)))
    }

    /// Box `i` has no content (only collapsible whitespace): `empty-cells`.
    pub(crate) fn is_empty(&self, i: usize) -> bool {
        kids(&self.nodes, i).all(|k| match &self.nodes[k].content {
            Content::Text(t) => t.trim().is_empty(),
            Content::Anon => kids(&self.nodes, k).next().is_none(),
            _ => false,
        })
    }

    /// The text a box draws: a text run's text, or a word's slice of it.
    pub(crate) fn text(&self, i: usize) -> &str {
        match &self.nodes[i].content {
            Content::Text(t) => t,
            Content::Word { src, start, end, .. } => match &self.nodes[*src as usize].content {
                Content::Text(t) => &t[*start as usize..*end as usize],
                _ => "",
            },
            _ => "",
        }
    }

    /// The boxes moved (a redraw, a scroll): the hit test's index of where they are is stale.
    pub(crate) fn moved(&mut self) {
        self.hit_index.take();
    }

    /// Element boxes from the root down to the topmost box under (x, y).
    pub(crate) fn hit_path(&self, x: f32, y: f32, styles: &StyleStore) -> Path {
        self.path_to(self.hit_box(x, y, styles))
    }

    /// The topmost box under (x, y): a word, a text run, an element ...
    pub(crate) fn hit_box(&self, x: f32, y: f32, styles: &StyleStore) -> Option<usize> {
        let mut best: Option<(i32, usize)> = None;
        let nodes = &self.nodes;
        let index = self.hit_index.get_or_init(|| HitIndex::build(nodes)).0.as_ref();
        let mut next = 0;
        while next < nodes.len() {
            let (i, n) = (next, &nodes[next]);
            next += 1;
            // a box that is hidden, clipped away or has the point outside everything under it: its whole
            // subtree is out (a clip only narrows what is under a box; visibility is inherited)
            if let Some((end, area)) = index
                && (!n.visible || n.clip.is_some_and(|c| !c.contains(x, y)) || !area[i].contains(x, y)) {
                    next = end[i] as usize;
                    continue;
                }
            if !n.visible || n.taffy.is_none() || !styles[n.style].pointer_events {
                continue;
            }
            if n.clip.is_some_and(|c| !c.contains(x, y)) || !n.bounds.contains(x, y) {
                continue;
            }
            // later boxes paint over earlier ones; z-index lifts a subtree
            let z = self.z_of(i, styles);
            if best.is_none_or(|(bz, _)| z >= bz) {
                best = Some((z, i));
            }
        }
        best.map(|(_, b)| b)
    }

    /// The element boxes from the root down to the element that box `hit` belongs to.
    pub(crate) fn path_to(&self, hit: Option<usize>) -> Path {
        let mut path = Path::new();
        let mut cur = hit.map(|b| self.nodes[b].owner);
        while let Some(i) = cur {
            if self.nodes[i].el != NONE {
                path.push(i);
            }
            cur = self.nodes[i].parent;
        }
        path.reverse();
        path
    }

    /// The effective stacking level of a box (its nearest z-index).
    /// Its boxes in order-modified document order (CSS Flexbox 5.4.1, Grid 4.2: flex and grid
    /// items paint in `order`), `None` when no item has an `order` (it is tree order then).
    pub(crate) fn order_modified(&self, styles: &StyleStore) -> Option<Vec<usize>> {
        let ordered = |i: usize| {
            let c = &styles[self.nodes[i].style];
            // (an absolutely positioned child isn't an item)
            let o = if c.layout.position == taffy::style::Position::Absolute { 0 } else { c.order };
            (o != 0 && self.nodes[i].parent.is_some_and(|p| matches!(styles[self.nodes[p].style].display,
                DisplayKind::Flex | DisplayKind::InlineFlex | DisplayKind::Grid | DisplayKind::InlineGrid))).then_some(o)
        };
        if !(0..self.nodes.len()).any(|i| ordered(i).is_some()) {
            return None;
        }
        // each box's path from the root, a step being (its `order`, its tree position)
        let path = |mut i: usize| {
            let mut k = Vec::new();
            loop {
                k.push((ordered(i).unwrap_or(0), i));
                match self.nodes[i].parent {
                    Some(p) => i = p,
                    None => break,
                }
            }
            k.reverse();
            k
        };
        let mut v: Vec<usize> = (0..self.nodes.len()).collect();
        v.sort_by_cached_key(|&i| path(i));
        Some(v)
    }

    pub(crate) fn z_of(&self, mut i: usize, styles: &StyleStore) -> i32 {
        // the root's box is the canvas's background: under every stacking layer, negative ones too
        // (CSS 2 Appendix E)
        if i == 0 {
            return i32::MIN;
        }
        loop {
            if let Some(z) = styles[self.nodes[i].style].z_index {
                return z;
            }
            match self.nodes[i].parent {
                Some(p) => i = p,
                None => return 0,
            }
        }
    }
}

/// `@font-face`: registers each face's first `url()` that loads (paths are
/// relative to the working directory), once per process. Styles computed
/// before a face loaded are recomputed because the sheet changed.
fn load_font_faces(sheet: &Stylesheet) {
    static LOADED: std::sync::Mutex<Vec<(String, bool, bool)>> = std::sync::Mutex::new(Vec::new());
    for face in &sheet.font_faces {
        let key = (face.family.to_ascii_lowercase(), face.bold, face.italic);
        if LOADED.lock().unwrap_or_else(|e| e.into_inner()).contains(&key) {
            continue;
        }
        let loaded = face.src.iter().any(|path| {
            std::fs::read(path).is_ok_and(|bytes| crate::add_font(&face.family, face.bold, face.italic, &bytes))
        });
        if loaded {
            LOADED.lock().unwrap_or_else(|e| e.into_inner()).push(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BoxNode;

    /// A box is made for every element and text run: a widget's spec lives behind a pointer.
    #[test]
    fn box_node_stays_small() {
        assert!(std::mem::size_of::<BoxNode>() <= 256, "{}", std::mem::size_of::<BoxNode>());
    }
}

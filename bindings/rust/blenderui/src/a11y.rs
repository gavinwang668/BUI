//! Screen readers: the laid-out page as the accessibility subtree of the window's canvas.
//!
//! The kit owns each window's accessibility tree (its widgets describe themselves); this module
//! adds what the HTML engine draws, as browsers expose it (HTML-AAM, WAI-ARIA, accname): roles
//! from tags and `role`, names from `aria-labelledby`, `aria-label`, `<label>`, `alt`, `title`
//! or the content, states from the elements and `aria-*`. Form controls are kit widgets: their
//! nodes are the kit's, named here. Nothing is built until a screen reader reads the window
//! (the kit's `BK_EV_A11Y_TREE`); then the subtree is built again with every frame.
//!
//! Requests on the page's nodes come back as [`Request`]s, which the window performs as the
//! input they stand for (a click runs `on_click`, focus moves the focus ...).

use std::ffi::c_char;

use blenderui_sys as sys;
pub use sys::a11y::{Action, Role, flags};
use sys::BKA11yNode as Node;

use crate::geometry::Bounds;
use crate::layout::{Content, Frame, Key, UiState, kids};
use crate::style::KeySet;

/// A node of a window's accessibility tree, as [`App::accessibility_tree`] reads it.
///
/// [`App::accessibility_tree`]: crate::App::accessibility_tree
#[derive(Clone, Debug, PartialEq)]
pub struct AccessNode {
    /// Stable while the element (or widget) lives; what [`App::accessibility_action`] takes.
    ///
    /// [`App::accessibility_action`]: crate::App::accessibility_action
    pub id: u64,
    /// Index of the parent in the tree; `None` for the root (the window).
    pub parent: Option<usize>,
    pub role: Role,
    pub name: String,
    pub description: String,
    pub value: String,
    /// `flags::*` (focusable, checked, expanded ...).
    pub flags: u32,
    /// Heading level, tree depth (0: none).
    pub level: u16,
    /// The numeric value and range (`flags::NUMERIC`).
    pub numeric: Option<(f64, f64, f64)>,
    /// Window pixels.
    pub bounds: [f32; 4],
}

impl AccessNode {
    /// Whether all of `f` (`flags::*`) are set.
    #[must_use]
    pub const fn has(&self, f: u32) -> bool {
        self.flags & f == f
    }
}

/// The tree as the kit gives it (`BK_a11y_tree`): its root first.
///
/// # Safety
/// `nodes` points at `n` valid nodes, whose strings live for the call.
pub(crate) unsafe fn snapshot(nodes: *const Node, n: i32) -> Vec<AccessNode> {
    if nodes.is_null() || n <= 0 {
        return Vec::new();
    }
    // SAFETY: the caller's contract.
    let nodes = unsafe { std::slice::from_raw_parts(nodes, n as usize) };
    // SAFETY: (each string) the caller's contract.
    let s = |p| unsafe { sys::a11y::node::text(p) }.unwrap_or("").to_owned();
    nodes.iter().map(|n| AccessNode {
        id: n.id,
        parent: usize::try_from(n.parent).ok(),
        role: Role::from_raw(n.role).unwrap_or(Role::Generic),
        name: s(n.name),
        description: s(n.description),
        value: s(n.value),
        flags: n.flags,
        level: n.level,
        numeric: (n.flags & flags::NUMERIC != 0).then_some((n.num, n.min, n.max)),
        bounds: [n.x, n.y, n.w, n.h],
    }).collect()
}

/// A window's accessibility tree, as [`App::accessibility_tree`] reads it.
///
/// [`App::accessibility_tree`]: crate::App::accessibility_tree
#[derive(Clone, Debug, Default)]
pub struct AccessTree {
    /// The window first, then each node after its parent, siblings in reading order.
    pub nodes: Vec<AccessNode>,
    /// The id of the node with the keyboard focus (the window's when none has it).
    pub focus: u64,
}

impl AccessTree {
    /// The first node with `role` and `name`.
    #[must_use]
    pub fn find(&self, role: Role, name: &str) -> Option<&AccessNode> {
        self.nodes.iter().find(|n| n.role == role && n.name == name)
    }

    /// The node the keyboard focus is on.
    #[must_use]
    pub fn focused(&self) -> Option<&AccessNode> {
        self.nodes.iter().find(|n| n.id == self.focus)
    }

    /// The children of node `i`.
    pub fn children(&self, i: usize) -> impl Iterator<Item = &AccessNode> {
        self.nodes.iter().filter(move |n| n.parent == Some(i))
    }
}

/// A screen reader's request for an element of the page.
pub(crate) struct Request {
    pub key: Key,
    pub action: Action,
    /// `SetValue`'s text, or its number.
    pub text: Option<String>,
    pub num: f64,
}

impl Request {
    /// The request a `BK_EV_A11Y_ACTION` carries (`r` valid for the call), if it is one.
    ///
    /// # Safety
    /// `r` points at the kit's request, valid for this call.
    pub(crate) unsafe fn from_kit(r: *const sys::BKA11yRequest) -> Option<Self> {
        // SAFETY: the caller's contract.
        let r = unsafe { r.as_ref()? };
        // SAFETY: the kit's request text is NUL-terminated or null.
        let text = unsafe { sys::a11y::node::text(r.text) }.map(str::to_owned);
        Some(Self { key: r.id, action: Action::from_raw(r.action)?, text, num: r.num })
    }
}

/// The window's half of the tree: the page's nodes, rebuilt each frame while a reader reads.
#[derive(Default)]
pub(crate) struct Accessibility {
    nodes: Vec<Node>,
    /// The nodes' strings, NUL-terminated; `strs` holds each node's (name, description, value)
    /// offsets (`NONE`: none) until the pointers are set, once nothing more is added.
    text: Vec<u8>,
    strs: Vec<[u32; 3]>,
    /// The bounds of each text run, from its words.
    runs: Vec<Option<Bounds<f32>>>,
    ids: KeySet<u64>,
    /// `id` attributes -> element boxes, for `aria-labelledby`, `aria-describedby` and `<label for>`.
    by_id: Vec<(u64, usize)>,
    /// `<label for>` and the label elements around form controls (box indices).
    labels: Vec<usize>,
    scratch: String,
}

const NONE: u32 = u32::MAX;

/// What the page builder reads.
pub(crate) struct Page<'a> {
    pub frame: &'a Frame,
    pub state: &'a UiState,
    /// A box's kit node, when it is a widget or a screen.
    pub kit: &'a dyn Fn(Key) -> Option<*mut sys::BKNode>,
    pub scale: f32,
}

impl Accessibility {
    /// The page as the canvas's subtree (set in the kit).
    pub(crate) fn update(&mut self, page: &Page, canvas: *mut sys::BKNode) {
        self.build(page);
        // SAFETY: our nodes (validated by the kit), their strings in `self.text`, which outlives the call.
        unsafe { sys::BK_a11y_set_subtree(canvas, self.nodes.as_ptr(), count(&self.nodes)) };
    }

    fn build(&mut self, page: &Page) {
        let f = page.frame;
        self.nodes.clear();
        self.text.clear();
        self.strs.clear();
        self.ids.clear();
        self.index(f);
        if !f.nodes.is_empty() {
            self.walk(page, 0, -1);
        }
        // the strings are complete: point at them
        for (n, s) in self.nodes.iter_mut().zip(&self.strs) {
            let at = |o: u32| if o == NONE { std::ptr::null() } else { self.text[o as usize..].as_ptr().cast::<c_char>() };
            (n.name, n.description, n.value) = (at(s[0]), at(s[1]), at(s[2]));
        }
    }

    /// What naming needs from the whole page: the words' runs, the ids and the labels.
    fn index(&mut self, f: &Frame) {
        self.runs.clear();
        self.runs.resize(f.nodes.len(), None);
        self.by_id.clear();
        self.labels.clear();
        for (i, n) in f.nodes.iter().enumerate() {
            if let Content::Word { src, .. } = n.content {
                let r = &mut self.runs[src as usize];
                *r = Some(r.map_or(n.bounds, |a| union(&a, &n.bounds)));
            }
            let Some(e) = f.el(i) else { continue };
            if let Some(id) = f.attr(i, "id") {
                self.by_id.push((crate::layout::id_key(id), i));
            }
            if e.tag == "label" {
                self.labels.push(i);
            }
        }
        self.by_id.sort_unstable_by_key(|&(k, _)| k);
    }

    /// The element whose `id` attribute is `id`.
    fn find_id(&self, id: &str) -> Option<usize> {
        let k = crate::layout::id_key(id);
        self.by_id.binary_search_by_key(&k, |&(k, _)| k).ok().map(|at| self.by_id[at].1)
    }

    /// Add box `i` and what is under it, under node `parent` (-1: the canvas).
    fn walk(&mut self, page: &Page, i: usize, parent: i32) {
        let f = page.frame;
        let n = &f.nodes[i];
        match &n.content {
            Content::Word { .. } | Content::LineBreak => {}
            Content::Anon => self.walk_kids(page, i, parent),
            Content::Text(run) => {
                if n.visible && !run.trim().is_empty() {
                    let bounds = self.runs[i].unwrap_or(n.bounds);
                    let at = self.push(page, n.key, i, parent, Role::Label, bounds);
                    self.scratch.clear();
                    push_words(&mut self.scratch, run);
                    let name = std::mem::take(&mut self.scratch);
                    self.set_str(at, 0, &name);
                    self.scratch = name;
                }
            }
            Content::Widget(_) | Content::Element | Content::Replaced(_) => self.element(page, i, parent),
        }
    }

    fn walk_kids(&mut self, page: &Page, i: usize, parent: i32) {
        for k in kids(&page.frame.nodes, i) {
            self.walk(page, k, parent);
        }
    }

    fn element(&mut self, page: &Page, i: usize, parent: i32) {
        let f = page.frame;
        let n = &f.nodes[i];
        let Some(e) = f.el(i) else { return self.walk_kids(page, i, parent) };
        let attr = |name: &str| f.attr(i, name);
        if attr("aria-hidden") == Some("true") || e.tag == "head" {
            return; // neither it nor anything in it
        }
        let widget = (page.kit)(n.key);
        let role = role_of(f, i, e.tag, attr("role"), widget.is_some());
        let hidden = !n.visible;
        let named = attr("aria-label").is_some() || attr("aria-labelledby").is_some() || attr("title").is_some();
        let mut state = self.states(page, i, role);
        if widget.is_some() {
            // the kit's widget: focus is the page's (the kit draws its own when it edits text)
            state |= flags::FOCUSABLE;
            if page.state.focused == Some(n.key) {
                state |= flags::FOCUSED;
            }
        }
        // a plain container (not shown, or nothing a reader needs): its children are its parent's
        if hidden || (widget.is_none() && role == Role::Generic && !named && state == 0) {
            return self.walk_kids(page, i, parent);
        }
        let at = self.push(page, n.key, i, parent, role, n.bounds);
        let node = &mut self.nodes[at as usize];
        node.flags = state;
        node.widget = widget.unwrap_or(std::ptr::null_mut()).cast();
        if let Some(l) = heading_level(e.tag).or_else(|| attr("aria-level").and_then(|v| v.parse().ok())) {
            node.level = l;
        }
        self.numeric(f, i, at);
        self.name(page, i, at, role);
        // a form control's content is the widget's (its label, its options); a screen's is the page's
        if !matches!(n.content, Content::Widget(_)) {
            self.walk_kids(page, i, at);
        }
    }

    /// A node for box `i` (its key as its id, made unique) under `parent`.
    fn push(&mut self, page: &Page, key: Key, i: usize, parent: i32, role: Role, b: Bounds<f32>) -> i32 {
        let mut id = key.max(1);
        while !self.ids.insert(id) {
            id = id.rotate_left(7) ^ (i as u64 + 1); // a run's words share nothing: unique all the same
        }
        let s = page.scale;
        self.nodes.push(Node {
            id,
            parent,
            role: role as u16,
            x: b.origin.x * s,
            y: b.origin.y * s,
            w: b.size.width * s,
            h: b.size.height * s,
            ..Node::default()
        });
        self.strs.push([NONE; 3]);
        count(&self.nodes) - 1
    }

    /// Set string `which` (0 name, 1 description, 2 value) of node `at`.
    fn set_str(&mut self, at: i32, which: usize, s: &str) {
        let s = s.trim();
        if s.is_empty() {
            return;
        }
        self.strs[at as usize][which] = self.text.len() as u32;
        self.text.extend(s.bytes().filter(|&b| b != 0));
        self.text.push(0);
    }

    /// The `BK_A11Y_*` states of element `i` (zero for a plain container).
    fn states(&self, page: &Page, i: usize, role: Role) -> u32 {
        let f = page.frame;
        let attr = |name: &str| f.attr(i, name);
        let truthy = |name: &str| attr(name).is_some_and(|v| v != "false");
        let mut s = 0;
        let interactive = matches!(role, Role::Button | Role::Link | Role::CheckBox | Role::Radio | Role::Switch
            | Role::Tab | Role::MenuItem | Role::MenuItemCheckBox | Role::MenuItemRadio | Role::Summary
            | Role::ListBoxOption | Role::TreeItem | Role::Slider | Role::SpinButton | Role::TextInput
            | Role::ComboBox | Role::SearchInput | Role::MultilineTextInput);
        let clicks = f.el(i).and_then(|e| e.listeners.as_deref()).is_some_and(|l| !l.click.is_empty());
        if interactive || clicks || attr("tabindex").is_some_and(|t| t.trim() != "-1") {
            s |= flags::FOCUSABLE;
        }
        if interactive || clicks {
            s |= flags::CLICKABLE;
        }
        if s & flags::FOCUSABLE != 0 && page.state.focused == Some(f.nodes[i].key) {
            s |= flags::FOCUSED;
        }
        if truthy("aria-disabled") || attr("disabled").is_some() {
            s |= flags::DISABLED;
        }
        let checked = attr("aria-checked").or_else(|| attr("aria-pressed"));
        if checked.is_some() || matches!(role, Role::CheckBox | Role::Radio | Role::Switch | Role::MenuItemCheckBox
                                         | Role::MenuItemRadio) {
            s |= flags::CHECKABLE;
            match checked {
                Some("true") => s |= flags::CHECKED,
                Some("mixed") => s |= flags::MIXED,
                _ => {}
            }
        }
        let details_open = |d: usize| {
            page.state.details_open.get(&f.nodes[d].key).copied().unwrap_or_else(|| f.attr(d, "open").is_some())
        };
        match attr("aria-expanded") {
            Some(v) => s |= flags::EXPANDABLE | if v == "true" { flags::EXPANDED } else { 0 },
            None if role == Role::Summary => {
                s |= flags::EXPANDABLE;
                if f.nodes[i].parent.is_some_and(details_open) {
                    s |= flags::EXPANDED;
                }
            }
            None => {}
        }
        if let Some(v) = attr("aria-selected") {
            s |= flags::SELECTABLE | if v == "true" { flags::SELECTED } else { 0 };
        }
        for (name, bit) in [("aria-required", flags::REQUIRED), ("aria-readonly", flags::READONLY),
                            ("aria-invalid", flags::INVALID), ("aria-modal", flags::MODAL),
                            ("aria-multiselectable", flags::MULTISELECTABLE), ("aria-haspopup", flags::POPUP)] {
            if truthy(name) {
                s |= bit;
            }
        }
        if attr("required").is_some() {
            s |= flags::REQUIRED;
        }
        s |= match (attr("aria-live"), role) {
            (Some("assertive"), _) | (None, Role::Alert) => flags::LIVE_ASSERTIVE,
            (Some("polite"), _) | (None, Role::Status | Role::Log) => flags::LIVE_POLITE,
            _ => 0,
        };
        if f.nodes[i].scroll_range.0 > 0.0 || f.nodes[i].scroll_range.1 > 0.0 {
            s |= flags::CLIPS;
        }
        s
    }

    /// `aria-valuenow`, `-valuemin`, `-valuemax` and `-valuetext`.
    fn numeric(&mut self, f: &Frame, i: usize, at: i32) {
        let num = |name: &str| f.attr(i, name).and_then(|v| v.trim().parse::<f64>().ok());
        if let Some(v) = num("aria-valuenow") {
            let n = &mut self.nodes[at as usize];
            n.flags |= flags::NUMERIC;
            (n.num, n.min, n.max) = (v, num("aria-valuemin").unwrap_or(f64::NAN), num("aria-valuemax").unwrap_or(f64::NAN));
        }
        if let Some(t) = f.attr(i, "aria-valuetext") {
            self.set_str(at, 2, t);
        }
    }

    /// The accessible name and description (accname): `aria-labelledby`, `aria-label`, the host
    /// language's (`<label>`, `alt`, `<legend>`, `<caption>`, `<figcaption>`, `<title>`), the
    /// content for roles named by it, then `title`. The description: `aria-describedby`, else a
    /// `title` the name did not use.
    fn name(&mut self, page: &Page, i: usize, at: i32, role: Role) {
        let f = page.frame;
        let attr = |name: &str| f.attr(i, name).filter(|v| !v.trim().is_empty());
        let mut name = std::mem::take(&mut self.scratch);
        name.clear();
        let mut title_used = false;
        if let Some(ids) = attr("aria-labelledby") {
            self.refs_text(f, ids, &mut name);
        }
        if name.trim().is_empty() && let Some(l) = attr("aria-label") {
            name.push_str(l);
        }
        if name.trim().is_empty() {
            self.native_name(page, i, role, &mut name);
        }
        if name.trim().is_empty() && from_content(role) {
            content_text(f, i, None, &mut name);
        }
        if name.trim().is_empty() && let Some(t) = attr("title") {
            name.push_str(t);
            title_used = true;
        }
        self.set_str(at, 0, &name);
        name.clear();
        if let Some(ids) = attr("aria-describedby") {
            self.refs_text(f, ids, &mut name);
        } else if let (Some(t), false) = (attr("title"), title_used) {
            name.push_str(t);
        }
        self.set_str(at, 1, &name);
        self.scratch = name;
    }

    /// The host language's name of element `i`.
    fn native_name(&self, page: &Page, i: usize, role: Role, out: &mut String) {
        let f = page.frame;
        let Some(e) = f.el(i) else { return };
        match e.tag {
            "img" | "area" => out.push_str(f.attr(i, "alt").unwrap_or("")),
            "input" | "select" | "textarea" | "meter" | "progress" => {
                // <label for=id>, else the <label> around it
                let id = f.attr(i, "id");
                let label = self.labels.iter().copied().find(|&l| id.is_some() && f.attr(l, "for") == id)
                    .or_else(|| ancestors(f, i).find(|&a| f.el(a).is_some_and(|e| e.tag == "label")));
                if let Some(l) = label {
                    content_text(f, l, Some(i), out);
                } else if matches!(f.attr(i, "type"), Some("button" | "submit" | "reset")) {
                    out.push_str(f.attr(i, "value").unwrap_or(""));
                }
            }
            "fieldset" | "figure" | "table" | "html" => {
                let caption = match e.tag {
                    "fieldset" => "legend",
                    "figure" => "figcaption",
                    "table" => "caption",
                    _ => {
                        out.push_str(f.title.as_deref().unwrap_or(""));
                        return;
                    }
                };
                if let Some(c) = kids(&f.nodes, i).find(|&k| f.el(k).is_some_and(|e| e.tag == caption)) {
                    content_text(f, c, None, out);
                }
            }
            _ if role == Role::Generic => {}
            _ => {}
        }
    }

    /// The text of the elements `ids` names (space-separated).
    fn refs_text(&self, f: &Frame, ids: &str, out: &mut String) {
        for id in ids.split_whitespace() {
            if let Some(el) = self.find_id(id) {
                if !out.is_empty() {
                    out.push(' ');
                }
                content_text(f, el, None, out);
            }
        }
    }

}

/// The text under box `i` (images by their `alt`), leaving out `skip` (a control inside its own
/// label) and what is hidden.
fn content_text(f: &Frame, i: usize, skip: Option<usize>, out: &mut String) {
    for k in kids(&f.nodes, i) {
        if Some(k) == skip || !f.nodes[k].visible {
            continue;
        }
        match &f.nodes[k].content {
            Content::Text(t) => push_words(out, t),
            Content::Anon => content_text(f, k, skip, out),
            Content::Element | Content::Replaced(_) | Content::Widget(_) => {
                if f.attr(k, "aria-hidden") == Some("true") {
                    continue;
                }
                if let Some(alt) = f.attr(k, "aria-label").or_else(|| f.attr(k, "alt")) {
                    push_words(out, alt);
                } else {
                    content_text(f, k, skip, out);
                }
            }
            _ => {}
        }
    }
}

/// A node count (or index) as the kit's `int`.
fn count(nodes: &[Node]) -> i32 {
    i32::try_from(nodes.len()).unwrap_or(i32::MAX)
}

/// `t`'s words onto `out`, one space between (and before, when `out` has text).
fn push_words(out: &mut String, t: &str) {
    for w in t.split_whitespace() {
        if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push_str(w);
    }
}

fn union(a: &Bounds<f32>, b: &Bounds<f32>) -> Bounds<f32> {
    let x0 = a.origin.x.min(b.origin.x);
    let y0 = a.origin.y.min(b.origin.y);
    let x1 = (a.origin.x + a.size.width).max(b.origin.x + b.size.width);
    let y1 = (a.origin.y + a.size.height).max(b.origin.y + b.size.height);
    crate::geometry::Bounds { origin: crate::geometry::Point { x: x0, y: y0 },
                              size: crate::geometry::Size { width: x1 - x0, height: y1 - y0 } }
}

fn ancestors(f: &Frame, i: usize) -> impl Iterator<Item = usize> + '_ {
    std::iter::successors(f.nodes[i].parent, |&p| f.nodes[p].parent)
}

const fn heading_level(tag: &str) -> Option<u16> {
    match tag.as_bytes() {
        [b'h', d @ b'1'..=b'6'] => Some((*d - b'0') as u16),
        _ => None,
    }
}

/// Roles whose name is their content (accname "name from content").
const fn from_content(role: Role) -> bool {
    matches!(role, Role::Button | Role::Link | Role::Heading | Role::CheckBox | Role::Radio | Role::Switch
        | Role::Tab | Role::MenuItem | Role::MenuItemCheckBox | Role::MenuItemRadio | Role::ListBoxOption
        | Role::TreeItem | Role::Cell | Role::ColumnHeader | Role::RowHeader | Role::Summary | Role::Tooltip
        | Role::Row | Role::Caption | Role::Legend | Role::Term | Role::Label)
}

/// The role of element `i` (`tag`, its `role` attribute; `widget`: a kit widget shows it, whose
/// role is the kit's).
fn role_of(f: &Frame, i: usize, tag: &str, aria: Option<&str>, widget: bool) -> Role {
    if let Some(r) = aria.and_then(|r| r.split_whitespace().find_map(aria_role)) {
        return r;
    }
    if widget {
        return Role::Generic; // the kit's own
    }
    let in_section = || ancestors(f, i).any(|a| f.el(a).is_some_and(|e| matches!(e.tag, "article" | "aside" | "main"
                                                                                  | "nav" | "section")));
    match tag {
        "html" => Role::Document,
        "a" if f.attr(i, "href").is_some() => Role::Link,
        "button" => Role::Button,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Role::Heading,
        "p" => Role::Paragraph,
        "ul" | "ol" | "menu" => Role::List,
        "li" => Role::ListItem,
        "dl" => Role::DescriptionList,
        "dt" => Role::Term,
        "dd" => Role::Definition,
        "table" => Role::Table,
        "tr" => Role::Row,
        "td" => Role::Cell,
        "th" if f.attr(i, "scope") == Some("row") => Role::RowHeader,
        "th" => Role::ColumnHeader,
        "caption" | "figcaption" => Role::Caption,
        "img" if f.attr(i, "alt") == Some("") => Role::Generic,
        "img" | "svg" | "picture" => Role::Image,
        "nav" => Role::Navigation,
        "main" => Role::Main,
        "header" if !in_section() => Role::Banner,
        "footer" if !in_section() => Role::ContentInfo,
        "aside" => Role::Complementary,
        "section" if f.attr(i, "aria-label").is_some() || f.attr(i, "aria-labelledby").is_some() => Role::Region,
        "article" => Role::Article,
        "form" => Role::Form,
        "search" => Role::Search,
        "figure" => Role::Figure,
        "details" => Role::Details,
        "summary" => Role::Summary,
        "dialog" => Role::Dialog,
        "blockquote" => Role::Blockquote,
        "code" | "pre" => Role::Code,
        "time" => Role::Time,
        "fieldset" | "optgroup" => Role::Group,
        "legend" => Role::Legend,
        "output" => Role::Status,
        "canvas" => Role::Canvas,
        "video" => Role::Video,
        "option" => Role::ListBoxOption,
        _ => Role::Generic,
    }
}

/// An ARIA role's kit role (unknown ones: None, so the next token or the tag decides).
fn aria_role(r: &str) -> Option<Role> {
    Some(match r {
        "button" => Role::Button,
        "checkbox" => Role::CheckBox,
        "radio" => Role::Radio,
        "switch" => Role::Switch,
        "link" => Role::Link,
        "heading" => Role::Heading,
        "img" | "image" => Role::Image,
        "list" => Role::List,
        "listitem" => Role::ListItem,
        "listbox" => Role::ListBox,
        "option" => Role::ListBoxOption,
        "menu" => Role::Menu,
        "menubar" => Role::MenuBar,
        "menuitem" => Role::MenuItem,
        "menuitemcheckbox" => Role::MenuItemCheckBox,
        "menuitemradio" => Role::MenuItemRadio,
        "tab" => Role::Tab,
        "tablist" => Role::TabList,
        "tabpanel" => Role::TabPanel,
        "tree" => Role::Tree,
        "treeitem" => Role::TreeItem,
        "table" | "grid" | "treegrid" => Role::Table,
        "row" => Role::Row,
        "cell" | "gridcell" => Role::Cell,
        "columnheader" => Role::ColumnHeader,
        "rowheader" => Role::RowHeader,
        "dialog" => Role::Dialog,
        "alertdialog" => Role::AlertDialog,
        "alert" => Role::Alert,
        "status" => Role::Status,
        "log" => Role::Log,
        "navigation" => Role::Navigation,
        "main" => Role::Main,
        "banner" => Role::Banner,
        "contentinfo" => Role::ContentInfo,
        "complementary" => Role::Complementary,
        "region" => Role::Region,
        "form" => Role::Form,
        "search" => Role::Search,
        "article" => Role::Article,
        "figure" => Role::Figure,
        "group" => Role::Group,
        "radiogroup" => Role::RadioGroup,
        "toolbar" => Role::Toolbar,
        "tooltip" => Role::Tooltip,
        "progressbar" => Role::Progress,
        "meter" => Role::Meter,
        "slider" => Role::Slider,
        "spinbutton" => Role::SpinButton,
        "textbox" => Role::TextInput,
        "searchbox" => Role::SearchInput,
        "combobox" => Role::ComboBox,
        "scrollbar" => Role::ScrollBar,
        "document" | "application" => Role::Document,
        "paragraph" => Role::Paragraph,
        "term" => Role::Term,
        "definition" => Role::Definition,
        "presentation" | "none" | "generic" => Role::Generic,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aria_roles_and_levels() {
        assert_eq!(aria_role("menuitemcheckbox"), Some(Role::MenuItemCheckBox));
        assert_eq!(aria_role("grid"), Some(Role::Table));
        assert_eq!(aria_role("presentation"), Some(Role::Generic));
        assert_eq!(aria_role("bogus"), None);
        assert_eq!(heading_level("h3"), Some(3));
        assert_eq!(heading_level("h7"), None);
        assert_eq!(heading_level("hr"), None);
        assert!(from_content(Role::Button) && !from_content(Role::TextInput) && !from_content(Role::List));
        let mut s = String::from("Save");
        push_words(&mut s, "  the \n file ");
        assert_eq!(s, "Save the file");
    }
}

//! A block's inline content as line items (`tree/inline.rs` lays them out), built the way browsers
//! build theirs (Blink's inline items builder):
//!
//! 1. the content is collected into one string: each text's white space processed by its style
//!    (`linebreak::transform`), spaces collapsing across elements (a space after a space, or at the
//!    start of a line, goes), each atomic inline a U+FFFC, each forced break a newline;
//! 2. the string's break opportunities are found once (`linebreak::opportunities`), so a break
//!    between `aa<span>bb</span>` follows the text, not the element boundary;
//! 3. each text is cut at them into pieces: a word box for each, with its advance, the spaces after
//!    it (which hang at the end of a line), and the break after it.

use super::*;
use crate::linebreak::{self, Opportunity, SOFT_HYPHEN};
use crate::tree::{Item, BREAK_ALLOWED, BREAK_EMERGENCY, BREAK_NEVER, ITEM_ATOMIC, ITEM_BREAK, ITEM_OUT, ITEM_TEXT,
                  VALIGN_BASELINE, VALIGN_BOTTOM, VALIGN_MIDDLE, VALIGN_TOP};

/// An inline box's start or end edge (its margin, border and padding there): a run of no text.
const EDGE: u8 = 0xe0;

/// A run of a block's inline content: bytes `cat..cat + len` of its string.
struct Run {
    kind: u8,
    /// The text box (a text run, or a newline of one), or the atomic inline / `<br>` box.
    node: usize,
    /// The inline element whose style the text has.
    owner: usize,
    cat: usize,
    len: usize,
    /// Where the run starts in its text box's processed text.
    start: usize,
    /// An atomic inline's or a `<br>`'s node.
    id: Option<NodeId>,
}

/// What collecting builds: the string, its runs, and whether it ends in a collapsible space (or a
/// line start, where spaces collapse away too).
struct Collected {
    text: String,
    runs: Vec<Run>,
    space: bool,
    /// White space from here on is discarded until content comes (`white-space-trim`).
    discard: bool,
}

impl Collected {
    /// Discards the white space at the end of the text collected since `since` (`white-space-trim`):
    /// the last text run gets shorter, past boxes' empty edge runs.
    fn trim_end(&mut self, since: usize) {
        while self.text.len() > since && self.text.ends_with([' ', '\t']) {
            let Some(r) = self.runs.iter_mut().rev().find(|r| r.len > 0) else { break };
            if r.kind != ITEM_TEXT {
                break;
            }
            r.len -= 1;
            self.text.pop();
        }
        self.runs.retain(|r| r.len > 0 || r.kind != ITEM_TEXT);
        self.space = self.text.is_empty() || self.text.ends_with([' ', '\n']);
    }
}

impl Builder<'_> {
    /// Line items for inline content `list` of `owner` (`tree/inline.rs`): `line` gets their nodes.
    pub(super) fn inline_items(&mut self, list: &[usize], owner: usize, line: &mut NodeList) -> Vec<Item> {
        // `white-space-trim: discard-inner` on the container: its content's edges
        let inner = self.st(owner).ws_trim & crate::style::TRIM_WS_INNER != 0;
        let mut c = Collected { text: String::new(), runs: Vec::new(), space: true, discard: inner };
        self.collect(list, owner, &mut c);
        if inner {
            c.trim_end(0);
        }
        let Collected { text, runs, .. } = c;
        let ops = {
            let owner = |at: usize| runs[runs.partition_point(|r| r.cat + r.len <= at).min(runs.len().saturating_sub(1))].owner;
            let style = |before: usize, at: usize| {
                let (a, b) = (owner(before), owner(at));
                let s = self.st(a);
                // the boundary's white space is the nearest common ancestor's of the text on each side
                let ws = if a == b { s.white_space } else { self.st(self.common_ancestor(a, b)).white_space };
                (ws, s.text_wrap, s.break_all, s.wrap_anywhere)
            };
            linebreak::opportunities(&text, style)
        };
        // each opportunity's `wrap-inside: avoid` depth: the avoiding inline boxes around the text on
        // both sides of it (none of them, the common case: none computed)
        let avoids = |b: &Self, mut k: usize| {
            let mut d = 0u8;
            while k != owner {
                d += u8::from(b.st(k).wrap_avoid);
                match b.nodes[k].parent {
                    Some(p) => k = p,
                    None => break,
                }
            }
            d
        };
        let depths: Vec<u8> = if runs.iter().any(|r| avoids(self, r.owner) > 0) {
            let run_owner = |at: usize| runs[runs.partition_point(|r| r.cat + r.len <= at).min(runs.len().saturating_sub(1))].owner;
            ops.iter().map(|&(at, _)| avoids(self, self.common_ancestor(run_owner(at.saturating_sub(1)), run_owner(at)))).collect()
        } else {
            Vec::new()
        };
        let depth_at = |at: usize| ops.binary_search_by_key(&at, |o| o.0).ok().and_then(|k| depths.get(k)).copied().unwrap_or(0);
        let mut items = Vec::with_capacity(runs.len());
        let mut counts: KeyMap<usize, usize> = KeyMap::default();
        for r in &runs {
            let after = |at: usize| ops.binary_search_by_key(&at, |o| o.0).ok().map(|k| ops[k].1);
            let brk = |o: Option<Opportunity>| match o {
                Some(Opportunity::Allowed | Opportunity::Forced) => BREAK_ALLOWED,
                Some(Opportunity::LastResort) => BREAK_EMERGENCY,
                Some(Opportunity::Glued) | None => BREAK_NEVER,
            };
            match r.kind {
                ITEM_TEXT => self.pieces(r, (&text, &ops, &depths), &mut counts, line, &mut items),
                EDGE => {
                    // an edge sticks to the box's content: no break between them
                    let start = r.start == 1;
                    let (s, e) = self.edges(r.node);
                    let sid = self.nodes[r.node].style;
                    let c = &self.styles[sid];
                    let line_h = c.line_px();
                    let above = text::baseline(0.0, line_h, c.font_size, c.font_id);
                    let width = if start { s } else { e };
                    let (brk, avoid) = if start {
                        (BREAK_NEVER, 0)
                    } else {
                        items.last_mut().map_or((BREAK_NEVER, 0), |l| (std::mem::replace(&mut l.brk, BREAK_NEVER), l.avoid))
                    };
                    let key = child_key(self.nodes[r.node].key, if start { "#edge-start" } else { "#edge-end" }, 0);
                    let node = self.push(Some(r.node), Content::Edge(start), Some(r.node), sid, key, false);
                    let s = Style { size: Size { width: Dimension::length(width), height: Dimension::length(line_h) }, ..Default::default() };
                    line.push(self.leaf(node, s));
                    items.push(Item { kind: ITEM_TEXT, brk, valign: VALIGN_BASELINE, last_baseline: false, width, above,
                                      below: line_h - above, hang: 0.0, expand: 0, hyphen: 0.0, trim: false, hangs: false, avoid, tab: 0.0, tab_min: 0.0 });
                }
                _ => {
                    let id = match r.id {
                        Some(id) => id,
                        // a preserved newline: a break box of its text
                        None => {
                            let n = counts.entry(r.node).or_default();
                            *n += 1;
                            let key = child_key(self.nodes[r.node].key, "#br", *n);
                            let br = self.push(Some(r.owner), Content::LineBreak, Some(r.owner), self.nodes[r.owner].style, key, false);
                            self.leaf(br, Style::default())
                        }
                    };
                    line.push(id);
                    let c = self.st(r.node);
                    let valign = match c.vertical_align {
                        VAlign::Top => VALIGN_TOP,
                        VAlign::Middle => VALIGN_MIDDLE,
                        VAlign::Bottom => VALIGN_BOTTOM,
                        _ => VALIGN_BASELINE,
                    };
                    let brk = if r.kind == ITEM_OUT { BREAK_NEVER } else { brk(after(r.cat + r.len)) };
                    items.push(Item { kind: r.kind, brk, valign,
                                      last_baseline: c.display == DisplayKind::InlineBlock, width: 0.0, above: 0.0,
                                      below: 0.0, hang: 0.0, expand: 0, hyphen: 0.0, trim: false, hangs: false,
                                      avoid: depth_at(r.cat + r.len), tab: 0.0, tab_min: 0.0 });
                }
            }
        }
        items
    }

    /// Inline box `k`'s margin, border and padding at its start and end (px; a percentage of nothing).
    fn edges(&self, k: usize) -> (f32, f32) {
        let s = &self.st(k).layout;
        if self.st(k).display != DisplayKind::Inline {
            return (0.0, 0.0);
        }
        let px = |v: LengthPercentage| taffy::ResolveOrZero::resolve_or_zero(v, Some(0.0), crate::tree::resolve_calc);
        let m = |v: LengthPercentageAuto| taffy::ResolveOrZero::resolve_or_zero(v, Some(0.0), crate::tree::resolve_calc);
        (m(s.margin.left) + px(s.border.left) + px(s.padding.left), m(s.margin.right) + px(s.border.right) + px(s.padding.right))
    }

    /// The nearest box that is `a` or an ancestor of it and is `b` or one of `b`'s.
    fn common_ancestor(&self, a: usize, b: usize) -> usize {
        let mut x = Some(a);
        while let Some(k) = x {
            let mut y = Some(b);
            while let Some(j) = y {
                if j == k {
                    return k;
                }
                y = self.nodes[j].parent;
            }
            x = self.nodes[k].parent;
        }
        a
    }

    /// Collects inline content `list` of `owner` into `c` (step 1 of the module docs); atomic inlines
    /// are built on the way.
    fn collect(&mut self, list: &[usize], owner: usize, c: &mut Collected) {
        for &k in list {
            match &self.nodes[k].content {
                Content::Text(_) => self.collect_text(k, owner, c),
                Content::Element if matches!(self.st(k).display, DisplayKind::Inline | DisplayKind::Contents) => {
                    use crate::style::{TRIM_WS_AFTER, TRIM_WS_BEFORE, TRIM_WS_INNER};
                    let trim = self.st(k).ws_trim;
                    if trim & TRIM_WS_BEFORE != 0 {
                        c.trim_end(0);
                    }
                    let since = c.text.len();
                    c.discard |= trim & TRIM_WS_INNER != 0;
                    let inner: Kids = kids(&self.nodes, k).collect();
                    // an inline box's margin, border and padding at its start and end take room
                    let (start, end) = self.edges(k);
                    if start > 0.0 {
                        c.runs.push(Run { kind: EDGE, node: k, owner: k, cat: c.text.len(), len: 0, start: 1, id: None });
                    }
                    self.collect(&inner, k, c);
                    if trim & TRIM_WS_INNER != 0 {
                        c.trim_end(since);
                    }
                    if end > 0.0 {
                        c.runs.push(Run { kind: EDGE, node: k, owner: k, cat: c.text.len(), len: 0, start: 0, id: None });
                    }
                    c.discard |= trim & TRIM_WS_AFTER != 0;
                }
                _ => {
                    let mut out = NodeList::new();
                    self.child_nodes(k, &mut out);
                    let st = self.st(k);
                    let clips = st.display == DisplayKind::InlineBlock && overflow_clips(st);
                    let brk = matches!(self.nodes[k].content, Content::LineBreak);
                    // an absolutely positioned box takes no room: it is no character of the text
                    if matches!(st.layout.position, Position::Absolute) {
                        for id in out {
                            c.runs.push(Run { kind: ITEM_OUT, node: k, owner, cat: c.text.len(), len: 0, start: 0, id: Some(id) });
                        }
                        continue;
                    }
                    // `white-space-trim` around an inline block container (not a flex or replaced box)
                    let trim = if st.display == DisplayKind::InlineBlock && matches!(self.nodes[k].content, Content::Element) { st.ws_trim } else { 0 };
                    if trim & crate::style::TRIM_WS_BEFORE != 0 {
                        c.trim_end(0);
                    }
                    for id in out {
                        // an inline-block that clips its overflow sits on its bottom margin edge
                        if clips {
                            self.changed |= self.taffy.set_baseline(id, Some(crate::tree::Baseline::MarginEdge));
                        }
                        let ch = if brk { '\n' } else { linebreak::OBJECT };
                        c.runs.push(Run { kind: if brk { ITEM_BREAK } else { ITEM_ATOMIC }, node: k, owner, cat: c.text.len(),
                                          len: ch.len_utf8(), start: 0, id: Some(id) });
                        c.text.push(ch);
                        c.space = brk;
                        c.discard = false;
                    }
                    c.discard |= trim & crate::style::TRIM_WS_AFTER != 0;
                }
            }
        }
    }

    /// Text box `k` (in `owner`'s style) into `c`: processed, its spaces collapsing with those before.
    fn collect_text(&mut self, k: usize, owner: usize, c: &mut Collected) {
        let st = self.st(owner);
        let (ws, tt) = (st.white_space, (st.text_transform, st.text_transform_extra));
        let (rtl, ovr) = (st.layout.direction == taffy::style::Direction::Rtl, st.bidi_override);
        let Content::Text(raw) = std::mem::replace(&mut self.nodes[k].content, Content::Anon) else { return };
        let mut t: String = linebreak::transform(&raw, tt, ws, None).into_owned();
        // bidirectional text: laid out in visual order
        if let Some(v) = text::bidi_visual(&t, rtl, ovr) {
            t = v;
        }
        let collapse = linebreak::collapses(ws);
        // `word-space-transform`: U+200B (a <wbr>'s too) becomes a space that collapses with none
        let word_space = match self.st(k).word_space {
            1 => Some(' '),
            2 => Some('\u{3000}'),
            _ => None,
        };
        let mut kept = String::with_capacity(t.len());
        // the run being added: where it starts in `kept`
        let mut from = 0;
        let push_run = |c: &mut Collected, kept: &str, from: usize| {
            if kept.len() > from {
                c.runs.push(Run { kind: ITEM_TEXT, node: k, owner, cat: c.text.len(), len: kept.len() - from, start: from, id: None });
                c.text.push_str(&kept[from..]);
            }
        };
        for ch in t.chars() {
            if ch == '\n' && linebreak::keeps_breaks(ws) {
                push_run(c, &kept, from);
                // the newline is the text's, a forced break of its own
                c.runs.push(Run { kind: ITEM_BREAK, node: k, owner, cat: c.text.len(), len: 1, start: kept.len(), id: None });
                c.text.push('\n');
                kept.push('\n');
                from = kept.len();
                c.space = true;
                continue;
            }
            if let Some(sp) = word_space.filter(|_| ch == '\u{200b}') {
                c.space = false;
                c.discard = false;
                kept.push(sp);
                continue;
            }
            // discarded white space (`white-space-trim`), until the content
            if c.discard && matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0c') {
                continue;
            }
            c.discard = false;
            if collapse && linebreak::collapsible(ch) {
                if c.space {
                    continue;
                }
                c.space = true;
                kept.push(' ');
                continue;
            }
            c.space = false;
            kept.push(ch);
        }
        push_run(c, &kept, from);
        self.nodes[k].content = Content::Text(kept.into());
    }

    /// Text run `r` cut into word boxes at the break opportunities `ops` (step 3 of the module docs).
    fn pieces(&mut self, r: &Run, (all, ops, depths): (&str, &[(usize, Opportunity)], &[u8]), counts: &mut KeyMap<usize, usize>,
              line: &mut NodeList, items: &mut Vec<Item>) {
        let sid = self.nodes[r.owner].style;
        let c = &self.styles[sid];
        let (size, letter, word_sp, ws) = (c.font_size, c.letter_spacing, c.word_spacing, c.white_space);
        let (tab_size, font_id) = (c.tab_size, c.font_id);
        let line_h = c.line_px();
        let above = text::baseline(0.0, line_h, size, c.font_id);
        // Han in a Japanese or Korean paragraph keeps its faces, drawn word by word
        let hint = text::han_hint(all);
        let flags = text::flags(c) | hint;
        let break_word = c.text_wrap & linebreak::WRAP_BREAK_WORD != 0;
        let autospace = c.text_wrap & linebreak::WRAP_NO_AUTOSPACE == 0;
        let Content::Text(t) = &self.nodes[r.node].content else { return };
        let t = t.clone();
        let src_key = self.nodes[r.node].key;
        // the last-frame width of the block (`overflow-wrap: break-word` cuts only words wider)
        let max = {
            let mut blk = r.owner;
            while self.st(blk).display.is_inline_level() || self.st(blk).display == DisplayKind::Inline {
                match self.nodes[blk].parent {
                    Some(p) => blk = p,
                    None => break,
                }
            }
            self.wrap_widths.get(&self.nodes[blk].key).copied().unwrap_or(self.viewport.0).max(1.0)
        };
        let end = r.cat + r.len;
        let first = ops.partition_point(|o| o.0 <= r.cat);
        let inside: Vec<(usize, Opportunity)> = ops[first..].iter().take_while(|o| o.0 <= end).copied().collect();
        let at_end = inside.last().filter(|o| o.0 == end).map(|o| o.1);
        let mut cuts: Vec<(usize, Opportunity)> = inside.into_iter().filter(|o| o.0 < end).collect();
        cuts.push((end, at_end.unwrap_or(Opportunity::LastResort)));
        let glued_end = at_end.is_none();
        let mut a = r.cat;
        let width = |tc: &mut TextCache, s: &str| {
            if s.contains(SOFT_HYPHEN) {
                let bare: String = s.chars().filter(|&c| c != SOFT_HYPHEN).collect();
                tc.width(&bare, size, flags) + letter * bare.chars().count() as f32
            } else {
                tc.width(s, size, flags) + letter * s.chars().count() as f32
            }
        };
        // a preserved tab is a piece of its own (its advance depends on where it falls)
        if !linebreak::collapses(ws) && t[r.start..r.start + r.len].contains('\t') {
            let mut tabbed: Vec<(usize, Opportunity)> = Vec::with_capacity(cuts.len());
            let mut from = r.cat;
            for &(b, op) in &cuts {
                for (o, _) in t[r.start + from - r.cat..r.start + b - r.cat].match_indices('\t') {
                    let at = from + o;
                    if at > from && tabbed.last().is_none_or(|l| l.0 != at) {
                        tabbed.push((at, Opportunity::Glued));
                    }
                    if at + 1 < b {
                        tabbed.push((at + 1, if ws == WhiteSpace::BreakSpaces { Opportunity::Allowed } else { Opportunity::Glued }));
                    }
                }
                if tabbed.last().is_none_or(|l| l.0 != b) {
                    tabbed.push((b, op));
                }
                from = b;
            }
            cuts = tabbed;
        }
        // a tab stop is `tab-size` spaces, measured as a run of them (with their spacing)
        let spaces_n = " ".repeat(usize::from(tab_size));
        let tab_stop = self.text.width(&spaces_n, size, flags) + (word_sp + letter) * f32::from(tab_size);
        let ch = crate::text::ch(font_id, size);
        let mut k = 0;
        while k < cuts.len() {
            let (b, op) = cuts[k];
            k += 1;
            // last resorts cut only `anywhere` text, or a `break-word` word wider than its block
            if op == Opportunity::LastResort && k != cuts.len() {
                let next = cuts[k..].iter().find(|c| c.1 != Opportunity::LastResort).map_or(end, |c| c.0);
                let word = &t[r.start + a - r.cat..r.start + next - r.cat];
                if break_word && width(&mut *self.text, word) <= max {
                    continue;
                }
            }
            let piece = &t[r.start + a - r.cat..r.start + b - r.cat];
            let tab = piece == "\t";
            // the spaces after its text hang (`break-spaces` ones don't: lines break after them; nor
            // `pre` ones, on lines that don't wrap)
            let body = if matches!(ws, WhiteSpace::BreakSpaces | WhiteSpace::Pre) || tab { piece } else { piece.trim_end_matches(linebreak::hangs) };
            let spaces = &piece[body.len()..];
            let w = if tab { 0.0 } else { width(&mut *self.text, body) };
            let n_sp = spaces.chars().count();
            // (the spaces measured with the word before them, as the text draws them: a space alone
            // can measure a pixel off its advance after a word)
            let spaces_w = if body.is_empty() || body.contains(SOFT_HYPHEN) {
                self.text.width(spaces, size, flags)
            } else {
                (self.text.width(piece, size, flags) - self.text.width(body, size, flags)).max(0.0)
            };
            let mut hang = if n_sp == 0 { 0.0 } else { spaces_w + (word_sp + letter) * n_sp as f32 };
            // `text-autospace`: an ideograph against a Latin letter or digit, with nothing between
            if autospace && n_sp == 0 && let (Some(x), Some(y)) = (body.chars().next_back(), all[b..].chars().next())
                && linebreak::autospace(x, y) {
                hang += size / 8.0;
            }
            let hyphen = if body.ends_with(SOFT_HYPHEN) { self.text.width("\u{2010}", size, flags) } else { 0.0 };
            let at = r.start + a - r.cat;
            let n = counts.entry(r.node).or_default();
            *n += 1;
            let lang = match hint {
                blenderui_sys::BUI_FONT_HAN_JA => 1,
                blenderui_sys::BUI_FONT_HAN_KO => 2,
                _ => 0,
            };
            let content = Content::Word { src: r.node as u32, start: at as u32, end: (at + body.len()) as u32, lang };
            let node = self.push(Some(r.owner), content, Some(r.owner), sid, child_key(src_key, "#w", *n), false);
            let s = Style { size: Size { width: Dimension::length(w), height: Dimension::length(line_h) }, ..Default::default() };
            let id = self.leaf(node, s);
            self.text_baseline(id, sid);
            line.push(id);
            let brk = match op {
                _ if b == end && glued_end => BREAK_NEVER,
                Opportunity::LastResort => BREAK_EMERGENCY,
                Opportunity::Glued => BREAK_NEVER,
                _ => BREAK_ALLOWED,
            };
            items.push(Item { kind: ITEM_TEXT, brk, valign: VALIGN_BASELINE, last_baseline: false, width: w, above,
                              below: line_h - above, hang, expand: spaces.chars().filter(|&c| c == ' ').count() as u16,
                              // only collapsible spaces go at a line's end; others (U+3000 ...) hang
                              hyphen, trim: linebreak::collapses(ws) && spaces.chars().all(|c| c == ' '),
                              hangs: linebreak::collapses(ws),
                              avoid: ops.binary_search_by_key(&b, |o| o.0).ok().and_then(|k| depths.get(k)).copied().unwrap_or(0),
                              tab: if tab { tab_stop } else { 0.0 }, tab_min: if tab { ch / 2.0 } else { 0.0 } });
            if op == Opportunity::Forced && b < end {
                let n = counts.entry(r.node).or_default();
                *n += 1;
                let key = child_key(src_key, "#br", *n);
                let br = self.push(Some(r.owner), Content::LineBreak, Some(r.owner), sid, key, false);
                line.push(self.leaf(br, Style::default()));
                items.push(Item { kind: ITEM_BREAK, brk: BREAK_ALLOWED, valign: VALIGN_BASELINE, last_baseline: false,
                                  width: 0.0, above: 0.0, below: 0.0, hang: 0.0, expand: 0, hyphen: 0.0, trim: false, hangs: false, avoid: 0, tab: 0.0, tab_min: 0.0 });
            }
            a = b;
        }
    }
}

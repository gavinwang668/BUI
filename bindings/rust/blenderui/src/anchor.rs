//! CSS Anchor Positioning 1: `anchor-name`, `position-anchor`, `anchor()` in the insets,
//! `anchor-size()` in the sizes, `position-area`, `anchor-center` and `position-try-fallbacks`
//! (`flip-block`, `flip-inline` and `<position-area>` options).
//!
//! An anchored box is placed from where its anchor was in the last layout, like `@container`
//! queries read the last layout's sizes: its `anchor()` values become lengths before layout, and
//! when an anchor (or a containing block) moved, the window lays out once more ([`Anchors::update`]).
//! The first frame has nothing to read: `anchor()` takes its fallback, or `auto`.
//!
//! `position-visibility` hides an anchored box whose default anchor is out of sight
//! (`anchors-visible`, the initial value), missing (`anchors-valid`), or that overflows its
//! containing block (`no-overflow`), as of the last layout.
//!
//! `position-anchor: auto` (and the initial `normal` with a `position-area`) anchors a `::before` or
//! `::after` box to its originating element, its implicit anchor.
//!
//! Not here: `@position-try` rules, `flip-start`, `position-try-order`, and a popover's invoker as
//! its implicit anchor.

use crate::css::Tok;
use crate::geometry::{rect, Bounds};
use crate::layout::{Frame, Key};
use crate::style::{key_hasher, Computed, KeyMap};
use std::hash::{Hash, Hasher};
use taffy::prelude::*;

/// What a box says about anchors (`Computed::anchor`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnchorStyle {
    /// `anchor-name`: the names this box anchors.
    pub names: Box<[Box<str>]>,
    /// `position-anchor`: the anchor of `anchor()`s and `position-area` that name none.
    pub default: Option<Box<str>>,
    /// `anchor()` in `top`, `right`, `bottom`, `left`.
    pub insets: [Option<Edge>; 4],
    /// `anchor-size()` in `width`, `height`, `min-width`, `min-height`, `max-width`, `max-height`.
    pub sizes: [Option<Extent>; 6],
    /// `position-area`.
    pub area: Option<Area>,
    /// `justify-self` / `align-self: anchor-center` (bit 0: horizontal, bit 1: vertical).
    pub center: u8,
    /// `position-try-fallbacks`, in order.
    pub fallbacks: Box<[Try]>,
    /// `position-visibility` (`VIS_*`); `None`: the initial `anchors-visible`.
    pub visibility: Option<u8>,
    /// `position-anchor` without a name: `auto` (true), `none` (false); `None`: the initial `normal`.
    pub implicit: Option<bool>,
}

/// `position-visibility: anchors-valid`.
pub(crate) const VIS_ANCHORS_VALID: u8 = 1;
/// `anchors-visible`.
pub(crate) const VIS_ANCHORS_VISIBLE: u8 = 2;
/// `no-overflow`.
pub(crate) const VIS_NO_OVERFLOW: u8 = 4;

/// `position-visibility: always | [anchors-valid || anchors-visible || no-overflow]`.
pub(crate) fn parse_visibility(v: &[Tok]) -> Option<u8> {
    let mut bits = 0;
    for t in v {
        let bit = match t.ident()? {
            "always" if v.len() == 1 => return Some(0),
            // (and the draft's singular names, which browsers shipped)
            "anchors-valid" | "anchor-valid" => VIS_ANCHORS_VALID,
            "anchors-visible" | "anchor-visible" => VIS_ANCHORS_VISIBLE,
            "no-overflow" => VIS_NO_OVERFLOW,
            _ => return None,
        };
        if bits & bit != 0 {
            return None;
        }
        bits |= bit;
    }
    (bits != 0).then_some(bits)
}

/// `anchor([name] side [, fallback])`, plus a constant from `calc(anchor(..) ± length)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    name: Option<Box<str>>,
    side: Side,
    fallback: Option<f32>,
    offset: f32,
}

/// The `<anchor-side>` of `anchor()`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Side {
    Top,
    Right,
    Bottom,
    Left,
    /// `start` / `self-start` (horizontal-tb, ltr: top or left).
    Start,
    End,
    Center,
    /// The side of the inset property's own edge.
    Inside,
    Outside,
    /// `<percentage>` of the way from the start edge, as a fraction.
    At(f32),
}

/// `anchor-size([name] [dimension] [, fallback])` (+ a `calc` constant).
#[derive(Clone, Debug, PartialEq)]
pub struct Extent {
    name: Option<Box<str>>,
    /// The anchor's width (`width`, `inline`, `self-inline`) or height; `None`: the property's own axis.
    width: Option<bool>,
    fallback: Option<f32>,
    offset: f32,
}

/// `position-area`: the rows and columns of the 3x3 grid the anchor cuts its containing block
/// into (0: before the anchor, 1: the anchor's span, 2: after), first and last of each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    rows: (u8, u8),
    cols: (u8, u8),
}

/// One `position-try-fallbacks` option.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Try {
    /// `flip-block` (bit 0) and/or `flip-inline` (bit 1).
    Flip(u8),
    Area(Area),
}

// -- parsing ----------------------------------------------------------------------------------

fn dashed(t: &Tok) -> Option<&str> {
    t.ident().filter(|s| s.starts_with("--"))
}

/// A length in a fallback or `calc` offset: px only (the containing block isn't known here).
fn px(t: &Tok, font: f32) -> Option<f32> {
    match t {
        Tok::Num(n) if *n == 0.0 => Some(0.0),
        Tok::Dim(n, u) if u.eq_ignore_ascii_case("px") => Some(*n),
        Tok::Dim(n, u) if u.eq_ignore_ascii_case("em") => Some(n * font),
        _ => None,
    }
}

/// `calc(f(..) ± length)` or `f(..)`: the function's tokens and the constant.
fn with_offset<'v>(v: &'v [Tok], f: &str, font: f32) -> Option<(&'v crate::css::FuncTok, f32)> {
    match v {
        [Tok::Func(fx)] if fx.name == f => Some((fx, 0.0)),
        [Tok::Func(c)] if c.name == "calc" => match c.args.first()?.as_slice() {
            [Tok::Func(fx), op, l] if fx.name == f => {
                // the tokenizer reads a lone `-` as a name
                let minus = match op {
                    Tok::Delim(o) => *o == '-',
                    t if t.ident() == Some("-") => true,
                    _ => return None,
                };
                if matches!(op, Tok::Delim(o) if *o != '+' && *o != '-') {
                    return None;
                }
                let d = px(l, font)?;
                Some((fx, if minus { -d } else { d }))
            }
            [Tok::Func(fx)] if fx.name == f => Some((fx, 0.0)),
            _ => None,
        },
        _ => None,
    }
}

impl Edge {
    /// `anchor()` (or `calc(anchor() ± length)`) as an inset value.
    pub(crate) fn parse(v: &[Tok], font: f32) -> Option<Edge> {
        let (fx, offset) = with_offset(v, "anchor", font)?;
        let first = fx.args.first()?;
        let (name, rest) = match first.as_slice() {
            [n, rest @ ..] if dashed(n).is_some() => (dashed(n).map(Box::from), rest),
            all => (None, all),
        };
        let side = match rest {
            [Tok::Pct(p)] => Side::At(p / 100.0),
            [t] => match t.ident()? {
                "top" => Side::Top,
                "right" => Side::Right,
                "bottom" => Side::Bottom,
                "left" => Side::Left,
                "start" | "self-start" => Side::Start,
                "end" | "self-end" => Side::End,
                "center" => Side::Center,
                "inside" => Side::Inside,
                "outside" => Side::Outside,
                _ => return None,
            },
            _ => return None,
        };
        let fallback = match fx.args.get(1).map(Vec::as_slice) {
            Some([t]) => Some(px(t, font)?),
            Some(_) => return None,
            None => None,
        };
        Some(Edge { name, side, fallback, offset })
    }
}

impl Extent {
    /// `anchor-size()` (or `calc(anchor-size() ± length)`) as a size value.
    pub(crate) fn parse(v: &[Tok], font: f32) -> Option<Extent> {
        let (fx, offset) = with_offset(v, "anchor-size", font)?;
        let mut name = None;
        let mut width = None;
        for t in fx.args.first().map(Vec::as_slice).unwrap_or_default() {
            match t.ident()? {
                n if n.starts_with("--") => name = Some(Box::from(n)),
                "width" | "inline" | "self-inline" => width = Some(true),
                "height" | "block" | "self-block" => width = Some(false),
                _ => return None,
            }
        }
        let fallback = match fx.args.get(1).map(Vec::as_slice) {
            Some([t]) => Some(px(t, font)?),
            Some(_) => return None,
            None => None,
        };
        Some(Extent { name, width, fallback, offset })
    }
}

impl Area {
    /// `position-area`: one or two keywords (`top`, `center`, `span-left`, `bottom span-all` ...);
    /// `None` for `none`.
    pub(crate) fn parse(v: &[Tok]) -> Option<Option<Area>> {
        let words: Vec<&str> = v.iter().map(Tok::ident).collect::<Option<_>>()?;
        // (axis: 0 either, 1 rows, 2 columns; first and last region)
        let region = |w: &str| -> Option<(u8, (u8, u8))> {
            Some(match w {
                "top" | "block-start" | "y-start" => (1, (0, 0)),
                "bottom" | "block-end" | "y-end" => (1, (2, 2)),
                "span-top" | "span-block-start" | "span-y-start" => (1, (0, 1)),
                "span-bottom" | "span-block-end" | "span-y-end" => (1, (1, 2)),
                "left" | "inline-start" | "x-start" => (2, (0, 0)),
                "right" | "inline-end" | "x-end" => (2, (2, 2)),
                "span-left" | "span-inline-start" | "span-x-start" => (2, (0, 1)),
                "span-right" | "span-inline-end" | "span-x-end" => (2, (1, 2)),
                "start" | "self-start" => (0, (0, 0)),
                "end" | "self-end" => (0, (2, 2)),
                "span-start" | "span-self-start" => (0, (0, 1)),
                "span-end" | "span-self-end" => (0, (1, 2)),
                "center" => (0, (1, 1)),
                "span-all" => (0, (0, 2)),
                _ => return None,
            })
        };
        let all = (0, 2);
        let (rows, cols) = match words.as_slice() {
            ["none"] => return Some(None),
            // one keyword: a row or column one spans the other axis; any other is both
            [a] => match region(a)? {
                (1, r) => (r, all),
                (2, c) => (all, c),
                (_, x) => (x, x),
            },
            [a, b] => match (region(a)?, region(b)?) {
                // a vertical keyword after an axis-free one (`center bottom`): that is the row
                ((2, c), (_, r)) | ((0, r), (2, c)) | ((0, c), (1, r)) => (r, c),
                ((1 | 0, r), (_, c)) => (r, c),
                _ => return None,
            },
            _ => return None,
        };
        Some(Some(Area { rows, cols }))
    }

    /// The area mirrored in the block (`rows`) or inline axis.
    fn flipped(self, rows: bool) -> Area {
        let m = |(a, b): (u8, u8)| (2 - b, 2 - a);
        if rows { Area { rows: m(self.rows), cols: self.cols } } else { Area { rows: self.rows, cols: m(self.cols) } }
    }
}

/// `position-try-fallbacks: none | [flip-block || flip-inline | <position-area>]#`.
pub(crate) fn parse_fallbacks(v: &[Tok]) -> Option<Box<[Try]>> {
    if v.len() == 1 && v[0].ident() == Some("none") {
        return Some(Box::default());
    }
    v.split(|t| matches!(t, Tok::Comma)).map(|opt| {
        let flips = opt.iter().map(|t| match t.ident() {
            Some("flip-block") => Some(1u8),
            Some("flip-inline") => Some(2),
            _ => None,
        }).collect::<Option<Vec<u8>>>();
        match flips {
            Some(f) if !f.is_empty() => Some(Try::Flip(f.iter().fold(0, |a, b| a | b))),
            _ => Area::parse(opt).flatten().map(Try::Area),
        }
    }).collect()
}

// -- resolving --------------------------------------------------------------------------------

impl AnchorStyle {
    /// Whether this box is placed by an anchor (not only an anchor itself).
    /// `position-visibility`'s conditions, the initial `anchors-visible` included.
    fn visibility(&self) -> u8 {
        self.visibility.unwrap_or(VIS_ANCHORS_VISIBLE)
    }

    /// Whether the box takes its implicit anchor: `auto`, or `normal` with a `position-area`.
    fn wants_implicit(&self) -> bool {
        self.default.is_none() && self.implicit.unwrap_or(self.area.is_some())
    }

    fn places(&self) -> bool {
        self.insets.iter().any(Option::is_some) || self.sizes.iter().any(Option::is_some)
            || self.area.is_some() || self.center != 0
    }
}

/// Anchors and containing blocks from the last layout (positions in the window), and the
/// fallback each anchored box uses.
#[derive(Default)]
pub(crate) struct Anchors {
    named: KeyMap<Box<str>, Bounds<f32>>,
    /// Each anchored box's containing block: the padding box, moved by its scroll.
    cb: KeyMap<Key, Bounds<f32>>,
    /// The `position-try-fallbacks` option in use (0: the box's own style).
    tried: KeyMap<Key, u8>,
    /// Anchored boxes `position-visibility` hides.
    hidden: crate::style::KeySet<Key>,
    /// Generated boxes anchored to their originating element: the private name it is under.
    implicit: KeyMap<Key, Box<str>>,
    generation: u64,
    /// Consecutive layouts caused by anchors moving.
    chain: u8,
}

/// The rect a box of `size` gets from insets `[top, right, bottom, left]` in containing block `b`
/// (where an axis has no inset it keeps `at`).
fn placed(ins: [Option<f32>; 4], size: (f32, f32), at: (f32, f32), b: Bounds<f32>) -> Bounds<f32> {
    let axis = |start: Option<f32>, end: Option<f32>, len: f32, base: f32, room: f32, at: f32| match (start, end) {
        (Some(s), _) => base + s,
        (None, Some(e)) => base + room - e - len,
        _ => at,
    };
    rect(axis(ins[3], ins[1], size.0, b.origin.x, b.size.width, at.0),
         axis(ins[0], ins[2], size.1, b.origin.y, b.size.height, at.1), size.0, size.1)
}

/// Box `i`'s bounds through its own and its ancestors' transforms (their bounding box): where an
/// anchor is seen (CSS Anchor Positioning 1, 6.4).
fn transformed(frame: &Frame, styles: &crate::layout::StyleStore, i: usize, vp: (f32, f32)) -> Bounds<f32> {
    let b = frame.nodes[i].bounds;
    let mut corners = [(b.origin.x, b.origin.y), (b.origin.x + b.size.width, b.origin.y),
                       (b.origin.x, b.origin.y + b.size.height), (b.origin.x + b.size.width, b.origin.y + b.size.height)];
    let mut moved = false;
    let mut at = Some(i);
    while let Some(k) = at {
        let n = &frame.nodes[k];
        let c = &styles[n.style];
        if let Some(e) = c.effects.as_deref().filter(|e| e.has_transform()) {
            let m = e.matrix(n.bounds, &c.units(vp));
            for p in &mut corners {
                *p = crate::effects::apply(&m, p.0, p.1);
            }
            moved = true;
        }
        at = n.parent;
    }
    if !moved {
        return b;
    }
    let (x0, x1) = corners.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, z), p| (a.min(p.0), z.max(p.0)));
    let (y0, y1) = corners.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, z), p| (a.min(p.1), z.max(p.1)));
    rect(x0, y0, x1 - x0, y1 - y0)
}

/// Whether `r` is inside `b` (with half a pixel of slack).
fn inside(r: Bounds<f32>, b: Bounds<f32>) -> bool {
    r.origin.x >= b.origin.x - 0.5 && r.origin.y >= b.origin.y - 0.5
        && r.origin.x + r.size.width <= b.origin.x + b.size.width + 0.5
        && r.origin.y + r.size.height <= b.origin.y + b.size.height + 0.5
}

/// The inset (top, right, bottom, left) and size values of one option: what replaces `auto`.
struct Resolved {
    insets: [Option<f32>; 4],
    sizes: [Option<f32>; 6],
    /// Self-alignment the area or `anchor-center` gives (justify, align), when the box has none.
    align: (Option<AlignSelf>, Option<AlignSelf>),
}

impl Anchors {
    fn anchor(&self, name: Option<&str>, own: &AnchorStyle) -> Option<Bounds<f32>> {
        self.named.get(name.or(own.default.as_deref())?).copied()
    }

    /// Option `option` of `a` (0: as styled; k: fallback k) for the box with containing block `b`.
    fn resolve(&self, a: &AnchorStyle, option: u8, b: Option<Bounds<f32>>) -> Resolved {
        let (flip, area) = match a.fallbacks.get(usize::from(option).wrapping_sub(1)) {
            Some(Try::Flip(f)) => (*f, a.area.map(|x| {
                let x = if f & 1 != 0 { x.flipped(true) } else { x };
                if f & 2 != 0 { x.flipped(false) } else { x }
            })),
            Some(Try::Area(x)) => (0, Some(*x)),
            None => (0, a.area),
        };
        let mut out = Resolved { insets: [None; 4], sizes: [None; 6], align: (None, None) };
        // a flip swaps the inset properties: `top: anchor(bottom)` becomes `bottom: anchor(top)`
        let from = |side: usize| match side {
            0 | 2 if flip & 1 != 0 => 2 - side,
            1 | 3 if flip & 2 != 0 => 4 - side,
            _ => side,
        };
        for (side, slot) in out.insets.iter_mut().enumerate() {
            let Some(e) = &a.insets[from(side)] else { continue };
            *slot = e.fallback.map(|f| f + e.offset);
            let (Some(b), Some(r)) = (b, self.anchor(e.name.as_deref(), a)) else { continue };
            let vertical = side % 2 == 0;
            let (start, len, b0, blen) = if vertical {
                (r.origin.y, r.size.height, b.origin.y, b.size.height)
            } else {
                (r.origin.x, r.size.width, b.origin.x, b.size.width)
            };
            let own_start = side == 0 || side == 3;
            let mut s = e.side;
            if from(side) != side {
                s = match s {
                    Side::Top => Side::Bottom,
                    Side::Bottom => Side::Top,
                    Side::Left => Side::Right,
                    Side::Right => Side::Left,
                    Side::Start => Side::End,
                    Side::End => Side::Start,
                    Side::At(p) => Side::At(1.0 - p),
                    other => other,
                };
            }
            let frac = match s {
                Side::Top if vertical => 0.0,
                Side::Bottom if vertical => 1.0,
                Side::Left if !vertical => 0.0,
                Side::Right if !vertical => 1.0,
                Side::Top | Side::Bottom | Side::Left | Side::Right => continue, // the other axis: invalid
                Side::Start => 0.0,
                Side::End => 1.0,
                Side::Center => 0.5,
                Side::Inside => if own_start { 0.0 } else { 1.0 },
                Side::Outside => if own_start { 1.0 } else { 0.0 },
                Side::At(p) => p,
            };
            let at = start + len * frac;
            *slot = Some(if own_start { at - b0 } else { b0 + blen - at } + e.offset);
        }
        for (k, slot) in out.sizes.iter_mut().enumerate() {
            let Some(x) = &a.sizes[k] else { continue };
            *slot = x.fallback.map(|f| f + x.offset);
            if let Some(r) = self.anchor(x.name.as_deref(), a) {
                let width = x.width.unwrap_or(k % 2 == 0);
                *slot = Some(if width { r.size.width } else { r.size.height } + x.offset);
            }
        }
        let (Some(b), Some(r)) = (b, self.anchor(None, a)) else { return out };
        // an area spanning the whole axis centres on the anchor there, as `anchor-center` does
        let mut centre = a.center;
        if let Some(area) = area {
            // the 3x3 grid's lines in each axis: the containing block's edges and the anchor's
            let lines = |b0: f32, blen: f32, a0: f32, alen: f32| {
                let c = |v: f32| v.clamp(b0, b0 + blen);
                [b0, c(a0), c(a0 + alen), b0 + blen]
            };
            let ys = lines(b.origin.y, b.size.height, r.origin.y, r.size.height);
            let xs = lines(b.origin.x, b.size.width, r.origin.x, r.size.width);
            let (r0, r1) = (usize::from(area.rows.0), usize::from(area.rows.1) + 1);
            let (c0, c1) = (usize::from(area.cols.0), usize::from(area.cols.1) + 1);
            out.insets = [Some(ys[r0] - ys[0]), Some(xs[3] - xs[c1]), Some(ys[3] - ys[r1]), Some(xs[c0] - xs[0])];
            // the default alignment: toward the anchor, centred on it when the area spans it
            let toward = |(a, b): (u8, u8)| Some(match (a, b) {
                (0, 0) | (0, 1) => AlignSelf::END,
                (2, 2) | (1, 2) => AlignSelf::START,
                _ => AlignSelf::CENTER,
            });
            out.align = (toward(area.cols), toward(area.rows));
            if area.cols == (0, 2) {
                centre |= 1;
                (out.insets[1], out.insets[3]) = (None, None);
            }
            if area.rows == (0, 2) {
                centre |= 2;
                (out.insets[0], out.insets[2]) = (None, None);
            }
        }
        // anchor-center: the inset-modified containing block centred on the anchor
        for (bit, vertical) in [(1u8, false), (2u8, true)] {
            if centre & bit == 0 {
                continue;
            }
            let (i0, i1) = if vertical { (0, 2) } else { (3, 1) };
            if out.insets[i0].is_some() || out.insets[i1].is_some() {
                continue;
            }
            let (b0, blen, mid) = if vertical {
                (b.origin.y, b.size.height, r.origin.y + r.size.height / 2.0)
            } else {
                (b.origin.x, b.size.width, r.origin.x + r.size.width / 2.0)
            };
            let half = (mid - b0).min(b0 + blen - mid).max(0.0);
            out.insets[i0] = Some(mid - half - b0);
            out.insets[i1] = Some(b0 + blen - mid - half);
        }
        out
    }

    /// Style `c` of the box `key` with its anchor references resolved: `None` when it has none.
    pub(crate) fn apply(&self, key: Key, c: &Computed) -> Option<Computed> {
        let a = c.anchor.as_deref()?;
        let named;
        let a = match self.implicit.get(&key) {
            Some(name) => {
                named = AnchorStyle { default: Some(name.clone()), ..a.clone() };
                &named
            }
            None => a,
        };
        let hide = self.hidden.contains(&key);
        if !a.places() {
            return hide.then(|| Computed { visible: false, ..c.clone() });
        }
        let option = self.tried.get(&key).copied().unwrap_or(0);
        let res = self.resolve(a, option, self.cb.get(&key).copied());
        let mut out = c.clone();
        if hide {
            out.visible = false;
        }
        let l = &mut out.layout;
        let set = |v: Option<f32>, slot: &mut LengthPercentageAuto| {
            if let Some(v) = v {
                *slot = LengthPercentageAuto::length(v);
            }
        };
        set(res.insets[0], &mut l.inset.top);
        set(res.insets[1], &mut l.inset.right);
        set(res.insets[2], &mut l.inset.bottom);
        set(res.insets[3], &mut l.inset.left);
        if let Some(w) = res.sizes[0] {
            l.size.width = Dimension::length(w);
        }
        if let Some(h) = res.sizes[1] {
            l.size.height = Dimension::length(h);
        }
        set(res.sizes[2], &mut l.min_size.width);
        set(res.sizes[3], &mut l.min_size.height);
        set(res.sizes[4], &mut l.max_size.width);
        set(res.sizes[5], &mut l.max_size.height);
        // the area's alignment is the default: the box's own wins
        if l.justify_self.is_none() {
            l.justify_self = res.align.0;
        }
        if l.align_self.is_none() {
            l.align_self = res.align.1;
        }
        if a.center & 1 != 0 {
            l.justify_self = Some(AlignSelf::CENTER);
        }
        if a.center & 2 != 0 {
            l.align_self = Some(AlignSelf::CENTER);
        }
        Some(out)
    }

    /// After a layout: record the anchors, the anchored boxes' containing blocks and the fallback
    /// each one fits with. True when the window must lay out again (something moved).
    pub(crate) fn update(&mut self, frame: &Frame, styles: &crate::layout::StyleStore, vp: (f32, f32)) -> bool {
        let mut named: KeyMap<Box<str>, Bounds<f32>> = KeyMap::default();
        // each name's box
        let mut seen: KeyMap<Box<str>, usize> = KeyMap::default();
        let mut implicit: KeyMap<Key, Box<str>> = KeyMap::default();
        let mut anchored = Vec::new();
        for (i, n) in frame.nodes.iter().enumerate() {
            let Some(a) = styles[n.style].anchor.as_deref() else { continue };
            if n.taffy.is_none() {
                continue;
            }
            for name in &a.names {
                named.insert(name.clone(), transformed(frame, styles, i, vp));
                seen.insert(name.clone(), i);
            }
            // a generated box's implicit anchor: its originating element, under a name of its own
            if a.wants_implicit() && frame.el(i).is_some_and(|e| e.tag.starts_with("::"))
                && let Some(p) = n.parent
            {
                let name: Box<str> = format!("\u{1}{i}").into();
                named.insert(name.clone(), frame.nodes[p].bounds);
                seen.insert(name.clone(), p);
                implicit.insert(n.key, name);
            }
            let watches = a.default.is_some() && a.visibility() & (VIS_ANCHORS_VALID | VIS_ANCHORS_VISIBLE) != 0
                || a.visibility() & VIS_NO_OVERFLOW != 0;
            if a.places() || watches {
                anchored.push(i);
            }
        }
        self.implicit = implicit;
        if anchored.is_empty() && self.cb.is_empty() && self.hidden.is_empty() {
            self.named = named;
            self.chain = 0;
            return false;
        }
        let mut cb: KeyMap<Key, Bounds<f32>> = KeyMap::default();
        // each anchored box's containing block (the clips between it and an anchor intervene)
        let mut cb_node: KeyMap<Key, Option<usize>> = KeyMap::default();
        for &i in &anchored {
            let n = &frame.nodes[i];
            let c = &styles[n.style];
            let block = if c.layout.position == Position::Fixed {
                rect(0.0, 0.0, vp.0, vp.1)
            } else {
                let mut p = n.parent;
                while let Some(k) = p {
                    if styles[frame.nodes[k].style].layout.position != Position::Static {
                        break;
                    }
                    p = frame.nodes[k].parent;
                }
                cb_node.insert(n.key, p);
                match p {
                    Some(k) => {
                        let (pn, pc) = (&frame.nodes[k], &styles[frame.nodes[k].style]);
                        let bw = crate::paint::border_widths(pc, pn.bounds.size.width);
                        rect(pn.bounds.origin.x + bw[3] - pn.scroll_pos.0, pn.bounds.origin.y + bw[0] - pn.scroll_pos.1,
                             (pn.bounds.size.width - bw[1] - bw[3]).max(0.0), (pn.bounds.size.height - bw[0] - bw[2]).max(0.0))
                    }
                    None => rect(0.0, 0.0, vp.0, vp.1),
                }
            };
            cb.insert(n.key, block);
        }
        self.named = named;
        self.cb = cb;
        // the first option that fits (the box's own style first); none fits: its own
        let mut tried: KeyMap<Key, u8> = KeyMap::default();
        for &i in &anchored {
            let n = &frame.nodes[i];
            let Some(a) = styles[n.style].anchor.as_deref() else { continue };
            if a.fallbacks.is_empty() {
                continue;
            }
            let b = self.cb.get(&n.key).copied();
            let Some(block) = b else { continue };
            let size = (n.bounds.size.width, n.bounds.size.height);
            let at = (n.bounds.origin.x, n.bounds.origin.y);
            let fits = |o: u8| inside(placed(self.resolve(a, o, b).insets, size, at, block), block);
            let pick = (0..=a.fallbacks.len() as u8).find(|&o| fits(o)).unwrap_or(0);
            if pick != 0 {
                tried.insert(n.key, pick);
            }
        }
        self.tried = tried;
        // `position-visibility`, from where everything was laid out
        let mut hidden = crate::style::KeySet::default();
        for &i in &anchored {
            let n = &frame.nodes[i];
            let Some(a) = styles[n.style].anchor.as_deref() else { continue };
            let v = a.visibility();
            let anchor = a.default.as_ref().or(self.implicit.get(&n.key)).map(|d| seen.get(d).copied());
            // out of sight: hidden, or clipped away by the overflow of a box between it and the
            // anchored box's containing block
            let stop = cb_node.get(&n.key).copied().flatten();
            let out = |k: usize| {
                let an = &frame.nodes[k];
                if !an.visible {
                    return true;
                }
                let mut r = an.bounds;
                let mut up = an.parent;
                while let Some(u) = up.filter(|&u| Some(u) != stop) {
                    let (un, uc) = (&frame.nodes[u], &styles[frame.nodes[u].style]);
                    let ov = uc.layout.overflow;
                    if ov.x != taffy::style::Overflow::Visible || ov.y != taffy::style::Overflow::Visible {
                        let bw = crate::paint::border_widths(uc, un.bounds.size.width);
                        let pad = rect(un.bounds.origin.x + bw[3], un.bounds.origin.y + bw[0],
                                       (un.bounds.size.width - bw[1] - bw[3]).max(0.0), (un.bounds.size.height - bw[0] - bw[2]).max(0.0));
                        let (x0, y0) = (r.origin.x.max(pad.origin.x), r.origin.y.max(pad.origin.y));
                        let (x1, y1) = ((r.origin.x + r.size.width).min(pad.origin.x + pad.size.width),
                                        (r.origin.y + r.size.height).min(pad.origin.y + pad.size.height));
                        // (an empty anchor at the edge is still in sight)
                        let empty = r.size.width <= 0.0 || r.size.height <= 0.0;
                        if x1 < x0 || y1 < y0 || (!empty && (x1 <= x0 || y1 <= y0)) {
                            return true;
                        }
                        r = rect(x0, y0, x1 - x0, y1 - y0);
                    }
                    up = un.parent;
                }
                false
            };
            let hide = (v & VIS_ANCHORS_VALID != 0 && anchor == Some(None))
                || (v & VIS_ANCHORS_VISIBLE != 0 && anchor.flatten().is_some_and(out))
                || (v & VIS_NO_OVERFLOW != 0 && self.cb.get(&n.key).is_some_and(|b| !inside(n.bounds, *b)));
            if hide {
                hidden.insert(n.key);
            }
        }
        let mut keys: Vec<_> = hidden.iter().copied().collect();
        keys.sort_unstable();
        self.hidden = hidden;
        let mut h = key_hasher();
        keys.hash(&mut h);
        let mut put = |b: &Bounds<f32>| {
            (b.origin.x.to_bits(), b.origin.y.to_bits(), b.size.width.to_bits(), b.size.height.to_bits()).hash(&mut h);
        };
        let mut names: Vec<_> = self.named.iter().collect();
        names.sort_unstable_by(|a, b| a.0.cmp(b.0));
        names.iter().for_each(|(_, b)| put(b));
        let mut cbs: Vec<_> = self.cb.iter().collect();
        cbs.sort_unstable_by_key(|(k, _)| **k);
        cbs.iter().for_each(|(_, b)| put(b));
        let mut picks: Vec<_> = self.tried.iter().collect();
        picks.sort_unstable();
        picks.hash(&mut h);
        let generation = h.finish();
        let changed = generation != self.generation;
        self.generation = generation;
        if changed && !anchored.is_empty() && self.chain < 3 {
            self.chain += 1;
            return true;
        }
        self.chain = 0;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{Area, Edge, Side};
    use crate::css::Tok;

    fn toks(css: &str) -> Vec<Tok> {
        crate::css::tokenize(css)
    }

    #[test]
    fn anchor_function_parses() {
        let e = Edge::parse(&toks("anchor(--tip bottom, 4px)"), 16.0).unwrap();
        assert_eq!((e.name.as_deref(), e.side, e.fallback, e.offset), (Some("--tip"), Side::Bottom, Some(4.0), 0.0));
        let e = Edge::parse(&toks("calc(anchor(50%) - 2px)"), 16.0).unwrap();
        assert_eq!((e.name, e.side, e.offset), (None, Side::At(0.5), -2.0));
        assert!(Edge::parse(&toks("anchor(--a sideways)"), 16.0).is_none());
    }

    #[test]
    fn position_area_parses_like_the_spec_grid() {
        let a = |s: &str| Area::parse(&toks(s)).unwrap().unwrap();
        assert_eq!(a("bottom"), Area { rows: (2, 2), cols: (0, 2) }, "one row keyword: span-all columns");
        assert_eq!(a("top span-right"), Area { rows: (0, 0), cols: (1, 2) });
        assert_eq!(a("left center"), Area { rows: (1, 1), cols: (0, 0) });
        assert_eq!(a("top").flipped(true), a("bottom"));
    }
}

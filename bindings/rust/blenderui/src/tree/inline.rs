//! Inline layout: a block container's lines (CSS Inline 3, CSS Text 3 §4 and §7).
//!
//! The builder flattens a block's inline content into items, each a child of the container: a piece
//! of text (a word, or what lies between two line-break opportunities), an atomic inline (an
//! inline-block, an image, a control), or a forced line break. Each has its advance, the white space
//! after it (which hangs at the end of a line: it doesn't count towards whether the line fits nor
//! towards its alignment), whether a line may break after it, and how much justification may
//! stretch the space after it. Laying the container out, as browsers do (Blink's line breaker, the
//! CSS Text "Phase II" white-space rules):
//!
//! 1. lines are filled greedily: an item that doesn't fit ends the line at the last opportunity
//!    before it (an item wider than the line stays, overflowing, when there is none);
//! 2. a line box is as tall as its strut (the container's own font at its `line-height`) and its
//!    items, which sit on its baseline (`vertical-align: baseline`), or at its top, middle or bottom;
//! 3. each line is aligned in the container (`text-align`, `text-align-last` for the last line and
//!    one ended by a forced break; `justify` spreads the free space over the line's spaces), the
//!    first one indented by `text-indent`.
//!
//! The container's min-content width is its widest unbreakable run, its max-content width its
//! widest line between forced breaks; its baselines are its first and last lines'.

use super::{ix, resolve_calc, View};
use taffy::prelude::*;
use taffy::geometry::Point;
use taffy::compute::BlockContext;
use taffy::tree::{AxisStaticPosition, Baselines, Layout, LayoutInput, LayoutOutput, OofCandidate, OofCandidates,
                  OofPositioningArea, RequestedAxis, RunMode, SizingMode};
use taffy::{LayoutPartialTree, ResolveOrZero};

/// A piece of text.
pub(crate) const ITEM_TEXT: u8 = 0;
/// An atomic inline: a box laid out on its own.
pub(crate) const ITEM_ATOMIC: u8 = 1;
/// A forced line break (`<br>`, a preserved newline).
pub(crate) const ITEM_BREAK: u8 = 2;
/// An absolutely positioned box: no room in the line, which gives it its static position.
pub(crate) const ITEM_OUT: u8 = 3;

/// No line may break after the item.
pub(crate) const BREAK_NEVER: u8 = 0;
/// A line may break after it.
pub(crate) const BREAK_ALLOWED: u8 = 1;
/// One may, only when nothing else on the line fits (`overflow-wrap: anywhere | break-word`).
pub(crate) const BREAK_EMERGENCY: u8 = 2;

/// `text-align` and `text-align-last`, resolved against the direction.
pub(crate) const ALIGN_LEFT: u8 = 0;
pub(crate) const ALIGN_RIGHT: u8 = 1;
pub(crate) const ALIGN_CENTER: u8 = 2;
pub(crate) const ALIGN_JUSTIFY: u8 = 3;

/// `vertical-align` of an atomic inline.
pub(crate) const VALIGN_BASELINE: u8 = 0;
pub(crate) const VALIGN_TOP: u8 = 1;
pub(crate) const VALIGN_MIDDLE: u8 = 2;
pub(crate) const VALIGN_BOTTOM: u8 = 3;

/// One inline item (a child of the container, in the same order).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Item {
    /// `ITEM_*`.
    pub kind: u8,
    /// `BREAK_*`: whether a line may end after it.
    pub brk: u8,
    /// `VALIGN_*` (atomic inlines).
    pub valign: u8,
    /// Its baseline is its last line's (an inline-block's), not its first.
    pub last_baseline: bool,
    /// A piece of text's advance, and its line box's height above and below its baseline.
    pub width: f32,
    pub above: f32,
    pub below: f32,
    /// The white space after it, which hangs at the end of a line.
    pub hang: f32,
    /// How many justification opportunities the space after it has (a line's free space is shared
    /// out over the opportunities of all its items but the last).
    pub expand: u16,
    /// What a line ending after it adds: a soft hyphen's hyphen (it shows only then).
    pub hyphen: f32,
    /// Its hanging space is removed at the end of a line (collapsible white space), not just hung:
    /// its box (and an inline box's background) stops before it there.
    pub trim: bool,
    /// Its hanging space is collapsible white space (`white-space` collapses), which hangs
    /// unconditionally: it never counts at a line's end, a forced break's line included (preserved
    /// spaces before a forced break do: they hang only where the line would overflow).
    pub hangs: bool,
    /// How many `wrap-inside: avoid` inline boxes its break would be inside: a line breaks at the
    /// last of its opportunities that is inside the fewest.
    pub avoid: u8,
    /// A preserved tab: its advance takes the pen to the next multiple of this from the start of
    /// the line (the content box's edge), or the one after when that is nearer than `tab_min`
    /// (CSS Text 3 section 4.2); its `width` is laid out.
    pub tab: f32,
    pub tab_min: f32,
}

impl Item {
    /// Its advance at `x` on a line (a tab's depends on where it is), given its laid-out `width`.
    fn advance(&self, width: f32, x: f32) -> f32 {
        if self.tab <= 0.0 {
            return width;
        }
        let w = self.tab - x.rem_euclid(self.tab);
        if w < self.tab_min { w + self.tab } else { w }
    }
}

/// A container's inline content and how its lines go.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Inline {
    pub items: Vec<Item>,
    /// `ALIGN_*`: `text-align`, and `text-align-last` (for the last line, and lines a forced break
    /// ends).
    pub align: u8,
    pub last: u8,
    /// `text-indent` of the first line: px, and a fraction of the room (a percentage).
    pub indent: (f32, f32),
    /// The strut: the container's line box height above and below its baseline.
    pub strut: [f32; 2],
    /// Half the container font's x-height (`vertical-align: middle`).
    pub middle: f32,
    /// `text-wrap: balance`: its lines are made as even as their count allows.
    pub balance: bool,
}

/// A line: items `start..end`, its width (without the last item's hanging space), and whether a
/// forced break ends it.
#[derive(Clone, Copy)]
struct LineSpan {
    start: usize,
    end: usize,
    width: f32,
    forced: bool,
}

/// How wide the lines may be.
#[derive(Clone, Copy)]
enum Room {
    Width(f32),
    MinContent,
    MaxContent,
}

impl Room {
    const fn width(self) -> f32 {
        match self {
            Room::Width(w) => w,
            Room::MinContent => 0.0,
            Room::MaxContent => f32::INFINITY,
        }
    }

    const fn space(self) -> AvailableSpace {
        match self {
            Room::Width(w) => AvailableSpace::Definite(w),
            Room::MinContent => AvailableSpace::MinContent,
            Room::MaxContent => AvailableSpace::MaxContent,
        }
    }
}

/// An item's outer box as laid out: its size, margins and baseline (from its margin box's top).
#[derive(Clone, Copy, Default)]
struct Placed {
    size: Size<f32>,
    margin: Rect<f32>,
    baseline: f32,
}

impl Placed {
    fn outer(&self) -> Size<f32> {
        Size { width: self.size.width + self.margin.left + self.margin.right,
               height: self.size.height + self.margin.top + self.margin.bottom }
    }
}

/// What laying the lines out gives: the content size, the room's width it was laid out in, the
/// first and last baselines, and (when placed) the absolutely positioned boxes to lay out, its own
/// and its atomic inlines' (CSS 2 section 10.3.7: at their static positions, from the line).
struct Laid {
    size: Size<f32>,
    width: f32,
    baselines: (Option<f32>, Option<f32>),
    out: OofCandidates,
}

/// A layout of the lines: in `room`; with `at` (the content box's offset), putting the items there
/// too; `height`: the content box's height, when definite (percentages of atomic inlines).
#[derive(Clone, Copy)]
struct Pass {
    room: Room,
    at: Option<Point<f32>>,
    height: Option<f32>,
}

/// Fills lines of `room` px (`indent` on the first) with items of outer widths `widths`: greedy,
/// each ending at the last break opportunity before an item that doesn't fit (an emergency one only
/// when a line has no other). A tab's width is where it ends up on its line (it is set in `widths`).
fn break_lines(items: &[Item], widths: &mut [f32], room: f32, indent: f32) -> Vec<LineSpan> {
    let mut lines = Vec::new();
    let mut start = 0;
    while start < items.len() {
        let line = next_line(items, widths, start, room, if start == 0 { indent } else { 0.0 });
        start = line.end;
        lines.push(line);
    }
    lines
}

/// The line that starts at item `start` in `room` px (`indent` first): see [`break_lines`].
fn next_line(items: &[Item], widths: &mut [f32], start: usize, room: f32, indent: f32) -> LineSpan {
    // the line's width so far, with the hanging space of its last item; and where it may break
    let (mut x, mut normal, mut emergency) = (indent, None::<(usize, u8)>, None::<usize>);
    let mut end = (items.len(), false);
    for i in start..items.len() {
        let it = &items[i];
        if it.kind == ITEM_BREAK {
            end = (i + 1, true);
            break;
        }
        widths[i] = it.advance(widths[i], x);
        if i > start && x + widths[i] > room + 0.01 && let Some(b) = normal.map(|n| n.0).or(emergency) {
            end = (b + 1, false);
            break;
        }
        x += widths[i] + it.hang;
        match it.brk {
            // the last break inside the fewest `wrap-inside: avoid` boxes
            BREAK_ALLOWED if normal.is_none_or(|n| it.avoid <= n.1) => normal = Some((i, it.avoid)),
            BREAK_EMERGENCY => emergency = Some(i),
            _ => {}
        }
    }
    let last = end.0 - 1;
    // (the line's last item, or the last before its forced break, ends it)
    let end_item = if end.1 && last > start && items[last - 1].hangs { last - 1 } else { last };
    let mut x = indent;
    for k in start..end.0 {
        widths[k] = items[k].advance(widths[k], x);
        x += widths[k] + if k == last || k == end_item { items[k].hyphen } else { items[k].hang };
    }
    LineSpan { start, end: end.0, width: x, forced: end.1 }
}

impl<C, M> View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    /// A block container of inline content (see the module docs); `floats`: the float context of the
    /// block formatting context it is in, whose floats its lines go round.
    pub(super) fn inline_layout(&mut self, id: NodeId, inputs: LayoutInput, mut floats: Option<&mut BlockContext<'_>>) -> LayoutOutput {
        let style = self.t.container_style(id).clone();
        let parent = inputs.parent_size;
        // the content box: lines go there, and the baselines are measured from the border box's top
        let (pad, border) = (style.padding.resolve_or_zero(parent.width, resolve_calc),
                             style.border.resolve_or_zero(parent.width, resolve_calc));
        let at = Point { x: pad.left + border.left, y: pad.top + border.top };
        if let Some(f) = floats.as_deref_mut() {
            f.apply_content_box_inset([pad.left + border.left, pad.right + border.right]);
        }
        let floats = floats.as_deref().filter(|f| f.has_floats());
        // percentages of the content height: of the known height, or of the box's own definite one
        // (a float or abspos box measured for its width has none known yet)
        let insets = pad.top + pad.bottom + border.top + border.bottom;
        let own = taffy::MaybeResolve::maybe_resolve(style.size.height, parent.height, resolve_calc)
            .map(|h| if style.box_sizing == BoxSizing::ContentBox { h + insets } else { h });
        let height = inputs.known_dimensions.height.or(own).map(|h| (h - insets).max(0.0));
        let perform = inputs.run_mode == RunMode::PerformLayout;
        // (the measure gets the content box's width as definite space when it is known: a final
        // layout places the items as it measures them)
        let mut laid = None;
        let mut out = taffy::compute_leaf_layout(inputs, &style, resolve_calc, |_, avail| {
            let room = match avail.width {
                AvailableSpace::Definite(w) => Room::Width(w),
                AvailableSpace::MinContent => Room::MinContent,
                AvailableSpace::MaxContent => Room::MaxContent,
            };
            let place = (perform && matches!(room, Room::Width(_))).then_some(at);
            let l = self.lines(id, Pass { room, at: place, height }, floats);
            let size = l.size;
            if place.is_some() {
                laid = Some(l);
            }
            size
        });
        let width = (out.size.width - pad.left - pad.right - border.left - border.right).max(0.0);
        let mut laid = match laid {
            Some(l) if (l.width - width).abs() < 0.01 => l,
            _ => self.lines(id, Pass { room: Room::Width(width), at: perform.then_some(at), height }, floats),
        };
        out.baselines = Baselines { first: laid.baselines.0.map(|b| b + at.y), last: laid.baselines.1.map(|b| b + at.y) };
        if perform {
            out.oof_candidates = laid.out.take();
            // its positioned descendants' containing block, if it is one: its padding box
            out.oof_positioning_area = Some(OofPositioningArea {
                size: Size { width: out.size.width - border.left - border.right, height: out.size.height - border.top - border.bottom },
                offset: Point { x: border.left, y: border.top },
            });
        }
        out
    }

    /// Lays container `id`'s items out in lines (see [`Pass`]), round `floats`.
    fn lines(&mut self, id: NodeId, pass: Pass, floats: Option<&BlockContext<'_>>) -> Laid {
        // the items and children, held here while children lay out
        let Some(inline) = self.t.nodes[ix(id)].inline.take() else {
            return Laid { size: Size::ZERO, width: 0.0, baselines: (None, None), out: OofCandidates::NONE };
        };
        let kids = std::mem::take(&mut self.t.nodes[ix(id)].children);
        let laid = self.lay_lines(&inline, &kids, pass, floats);
        let n = &mut self.t.nodes[ix(id)];
        n.inline = Some(inline);
        n.children = kids;
        laid
    }

    fn lay_lines(&mut self, inline: &Inline, kids: &[NodeId], pass: Pass, floats: Option<&BlockContext<'_>>) -> Laid {
        let Pass { room, at, height: basis_h } = pass;
        let items = &inline.items;
        let basis = match room {
            Room::Width(w) => Some(w),
            _ => None,
        };
        // atomic inlines: laid out at the room's width (min- or max-content when measuring)
        let mut placed = vec![Placed::default(); items.len()];
        for (k, it) in items.iter().enumerate().filter(|(_, it)| it.kind == ITEM_ATOMIC) {
            let Some(&kid) = kids.get(k) else { continue };
            let s = self.t.item_style(kid);
            let m = s.margin.resolve_or_zero(basis, resolve_calc);
            // a sizing keyword width (`width: min-content` ...) is the parent's to resolve
            let keyword = match s.size.width.into_raw().tag() {
                taffy::CompactLength::MIN_CONTENT_TAG => Some(AvailableSpace::MinContent),
                taffy::CompactLength::MAX_CONTENT_TAG => Some(AvailableSpace::MaxContent),
                taffy::CompactLength::FIT_CONTENT_KEYWORD_TAG => Some(room.space()),
                taffy::CompactLength::STRETCH_TAG => basis.map(|w| AvailableSpace::Definite((w - m.left - m.right).max(0.0))),
                _ => None,
            };
            let width = keyword.map(|space| self.compute_child_layout(kid, LayoutInput {
                known_dimensions: Size::NONE,
                known_dimensions_are_definite: Size { width: true, height: true },
                parent_size: Size { width: basis, height: basis_h },
                available_space: Size { width: space, height: AvailableSpace::MaxContent },
                run_mode: RunMode::ComputeSize,
                sizing_mode: SizingMode::InherentSize,
                axis: RequestedAxis::Horizontal,
                vertical_margins_are_collapsible: Line::FALSE,
            }).size.width);
            let o = self.compute_child_layout(kid, LayoutInput {
                known_dimensions: Size { width, height: None },
                known_dimensions_are_definite: Size { width: true, height: true },
                parent_size: Size { width: basis, height: basis_h },
                available_space: Size { width: room.space(), height: AvailableSpace::MaxContent },
                run_mode: RunMode::ComputeSize,
                sizing_mode: SizingMode::InherentSize,
                axis: RequestedAxis::Both,
                vertical_margins_are_collapsible: Line::FALSE,
            });
            // its min and max sizes are the parent's to apply (resolved sizing keywords among them)
            let clamped = {
                let s = self.t.item_style(kid);
                let (p, b) = (s.padding.resolve_or_zero(basis, resolve_calc), s.border.resolve_or_zero(basis, resolve_calc));
                let content = s.box_sizing == BoxSizing::ContentBox;
                let lim = |v: LengthPercentageAuto, basis: Option<f32>, inset: f32| {
                    taffy::MaybeResolve::maybe_resolve(v, basis, resolve_calc).map(|x| if content { x + inset } else { x })
                };
                let clamp = |v: f32, min: Option<f32>, max: Option<f32>| max.map_or(v, |m| v.min(m)).max(min.unwrap_or(0.0));
                let (iw, ih) = (p.left + p.right + b.left + b.right, p.top + p.bottom + b.top + b.bottom);
                Size {
                    width: clamp(o.size.width, lim(s.min_size.width, basis, iw), lim(s.max_size.width, basis, iw)),
                    height: clamp(o.size.height, lim(s.min_size.height, basis_h, ih), lim(s.max_size.height, basis_h, ih)),
                }
            };
            let o = if (clamped.width - o.size.width).abs() > 0.01 || (clamped.height - o.size.height).abs() > 0.01 {
                let grew_w = (clamped.width - o.size.width).abs() > 0.01;
                self.compute_child_layout(kid, LayoutInput {
                    known_dimensions: Size { width: Some(clamped.width),
                                             height: (!grew_w || (clamped.height - o.size.height).abs() > 0.01).then_some(clamped.height) },
                    known_dimensions_are_definite: Size { width: true, height: true },
                    parent_size: Size { width: basis, height: basis_h },
                    available_space: Size { width: room.space(), height: AvailableSpace::MaxContent },
                    run_mode: RunMode::ComputeSize,
                    sizing_mode: SizingMode::InherentSize,
                    axis: RequestedAxis::Both,
                    vertical_margins_are_collapsible: Line::FALSE,
                })
            } else {
                o
            };
            // sizing passes don't give baselines (taffy's flex, grid and fixed-size block return
            // early): its layout at its size does, the one `place_atomic` reuses from the cache.
            // Only text gives one here (an inline-block of empty boxes, tables among them, sits
            // on its bottom margin edge as in browsers; a flex container would synthesize one)
            let o = if o.baselines.first.is_none() && self.t.has_text(kid) {
                self.compute_child_layout(kid, LayoutInput {
                    known_dimensions: o.size.map(Some),
                    known_dimensions_are_definite: Size { width: true, height: true },
                    parent_size: Size { width: basis, height: basis_h },
                    available_space: o.size.map(AvailableSpace::Definite),
                    run_mode: RunMode::PerformLayout,
                    sizing_mode: SizingMode::InherentSize,
                    axis: RequestedAxis::Both,
                    vertical_margins_are_collapsible: Line::FALSE,
                })
            } else {
                o
            };
            let base = if it.last_baseline { o.baselines.last.or(o.baselines.first) } else { o.baselines.first };
            placed[k] = Placed { size: o.size, margin: m, baseline: m.top + base.unwrap_or(o.size.height + m.bottom) };
        }
        let mut out = OofCandidates::NONE;
        // a percentage indent is of a definite room (0 in intrinsic sizing)
        let indent = inline.indent.0 + match room {
            Room::Width(w) => inline.indent.1 * w,
            _ => 0.0,
        };
        let mut widths: Vec<f32> = items.iter().zip(&placed).map(|(it, p)| match it.kind {
            ITEM_TEXT => it.width,
            ITEM_ATOMIC => p.outer().width,
            _ => 0.0,
        }).collect();
        // beside floats, each line is broken in the room the floats leave at its top (Blink's
        // exclusion space): so one at a time, as the lines above it set where it is
        let around = floats.filter(|_| matches!(room, Room::Width(_)));
        let mut lines = if around.is_some() { Vec::new() } else { break_lines(items, &mut widths, room.width(), indent) };
        // `text-wrap: balance` (as Blink does): a paragraph of 2 to 6 lines, without forced breaks,
        // takes the narrowest width that keeps its line count; the lines still align in the room
        if around.is_none() && inline.balance && (2..=6).contains(&lines.len()) && lines.iter().all(|l| !l.forced) {
            let n = lines.len();
            let (mut lo, mut hi) = (0.0f32, room.width());
            while hi - lo > 0.5 {
                let mid = (lo + hi) / 2.0;
                if break_lines(items, &mut widths, mid, indent).len() <= n { hi = mid } else { lo = mid }
            }
            lines = break_lines(items, &mut widths, hi, indent);
        }
        let mut size = Size::ZERO;
        let mut baselines = (None, None);
        let (mut n, mut start, mut wrapped) = (0usize, 0usize, false);
        loop {
            // the line, and where it may go: its offset and width beside the floats at its top
            let (line, shift, width) = match around {
                Some(f) => {
                    if start >= items.len() {
                        break;
                    }
                    let y = at.map_or(0.0, |a| a.y) + size.height;
                    let slot = f.find_content_slot(y, taffy::style::Clear::None, None);
                    let shift = (slot.x - at.map_or(0.0, |a| a.x)).max(0.0);
                    let w = slot.width.min(room.width() - shift).max(0.0);
                    (next_line(items, &mut widths, start, w, if start == 0 { indent } else { 0.0 }), shift, w)
                }
                None => match lines.get(n) {
                    Some(&l) => (l, 0.0, room.width()),
                    None => break,
                },
            };
            start = line.end;
            // a line its room ended (not a forced break, not the last): the lines fill the room
            wrapped |= !line.forced && line.end < items.len();
            let line = &line;
            let first = n == 0;
            n += 1;
            // the line box: the strut and its items on the baseline; top / bottom ones after
            let (mut above, mut below) = (inline.strut[0], inline.strut[1]);
            let mut tall = 0.0f32;
            for k in line.start..line.end {
                let it = &items[k];
                match it.kind {
                    ITEM_TEXT => {
                        above = above.max(it.above);
                        below = below.max(it.below);
                    }
                    ITEM_ATOMIC => {
                        let h = placed[k].outer().height;
                        match it.valign {
                            VALIGN_TOP | VALIGN_BOTTOM => tall = tall.max(h),
                            VALIGN_MIDDLE => {
                                above = above.max(h / 2.0 + inline.middle);
                                below = below.max(h / 2.0 - inline.middle);
                            }
                            _ => {
                                above = above.max(placed[k].baseline);
                                below = below.max(h - placed[k].baseline);
                            }
                        }
                    }
                    _ => {}
                }
            }
            let height = (above + below).max(tall);
            let top = size.height;
            if baselines.0.is_none() {
                baselines.0 = Some(top + above);
            }
            baselines.1 = Some(top + above);
            size.height += height;
            // a max-content line is as wide as its preserved spaces at the end too (they hang only
            // past a line's edge)
            let tail = &items[line.end - 1];
            let kept = if matches!(room, Room::MaxContent) && !tail.hangs { tail.hang } else { 0.0 };
            size.width = size.width.max(line.width + kept);
            let Some(at) = at else { continue };
            // where it starts, and what each space between its items gets on top (justification)
            let free = match room {
                Room::Width(_) => width - line.width,
                _ => 0.0,
            };
            let last = line.end == items.len() || line.forced;
            let align = if last { inline.last } else { inline.align };
            let opportunities: u32 = (line.start..line.end - 1).map(|k| u32::from(items[k].expand)).sum();
            let (mut x, extra) = match align {
                _ if free <= 0.0 => (0.0, 0.0),
                ALIGN_RIGHT => (free, 0.0),
                ALIGN_CENTER => (free / 2.0, 0.0),
                ALIGN_JUSTIFY if opportunities > 0 => (0.0, free / opportunities as f32),
                _ => (0.0, 0.0),
            };
            if first {
                x += indent;
            }
            x += shift;
            for k in line.start..line.end {
                let it = &items[k];
                let Some(&kid) = kids.get(k) else { continue };
                match it.kind {
                    ITEM_OUT => {
                        // its static position: where it is in the line
                        let (sx, sy) = (at.x + x, at.y + top);
                        let start = |v: f32| AxisStaticPosition::from_alignment(AlignSelf::START, Line { start: v, end: v }, false);
                        out.push(OofCandidate { node: kid, order: k as u32, position: self.t.style(kid).position,
                                                static_position: Point { x: start(sx), y: start(sy) } });
                    }
                    ITEM_ATOMIC => {
                        let p = placed[k];
                        let outer = p.outer().height;
                        let y = match it.valign {
                            VALIGN_TOP => 0.0,
                            VALIGN_BOTTOM => height - outer,
                            VALIGN_MIDDLE => above - outer / 2.0 - inline.middle,
                            _ => above - p.baseline,
                        };
                        let mut oof = self.place_atomic(kid, p, Size { width: basis, height: basis_h },
                                                        Point { x: at.x + x + p.margin.left, y: at.y + top + y + p.margin.top });
                        out.append(&mut oof);
                    }
                    _ => {
                        // the box holds the spaces after the text (an inline box's background covers
                        // them), but those a line's end removes; a soft hyphen ending the line shows
                        // its hyphen
                        let end = k + 1 == line.end;
                        let extra = if end { it.hyphen + if it.trim { 0.0 } else { it.hang } } else { it.hang };
                        let mut l = Layout::with_order(k as u32);
                        l.location = Point { x: at.x + x, y: at.y + top + above - it.above };
                        l.size = Size { width: widths[k] + extra, height: it.above + it.below };
                        self.set_unrounded_layout(kid, &l);
                    }
                }
                x += widths[k] + it.hang + extra * f32::from(it.expand);
            }
        }
        // shrink-to-fit is min(max-content, max(min-content, room)): lines that wrapped take the room
        if wrapped && let Room::Width(w) = room {
            size.width = size.width.max(w);
        }
        Laid { size, width: room.width(), baselines, out }
    }

    /// Lays atomic inline `kid` out at its size and puts its border box at `location`; returns the
    /// absolutely positioned boxes in it whose containing block is further out.
    fn place_atomic(&mut self, kid: NodeId, p: Placed, parent: Size<Option<f32>>, location: Point<f32>) -> OofCandidates {
        let basis = parent.width;
        let mut o = self.compute_child_layout(kid, LayoutInput {
            known_dimensions: p.size.map(Some),
            known_dimensions_are_definite: Size { width: true, height: true },
            parent_size: parent,
            available_space: p.size.map(AvailableSpace::Definite),
            run_mode: RunMode::PerformLayout,
            sizing_mode: SizingMode::InherentSize,
            axis: RequestedAxis::Both,
            vertical_margins_are_collapsible: Line::FALSE,
        });
        let s = self.t.style(kid);
        // `position: relative` moves it from where the line put it (left and top win)
        let mut location = location;
        if s.position == Position::Relative {
            let inset = |v: LengthPercentageAuto, basis: Option<f32>| taffy::MaybeResolve::maybe_resolve(v, basis, resolve_calc);
            let i = s.inset;
            location.x += inset(i.left, basis).or_else(|| inset(i.right, basis).map(|r| -r)).unwrap_or(0.0);
            location.y += inset(i.top, parent.height).or_else(|| inset(i.bottom, parent.height).map(|b| -b)).unwrap_or(0.0);
        }
        let mut l = Layout::with_order(0);
        l.location = location;
        l.size = o.size;
        l.scrollable_overflow_rect = o.scrollable_overflow_rect;
        l.margin = p.margin;
        l.padding = s.padding.resolve_or_zero(basis, resolve_calc);
        l.border = s.border.resolve_or_zero(basis, resolve_calc);
        self.set_unrounded_layout(kid, &l);
        let mut oof = o.oof_candidates.take();
        oof.translate(location);
        oof
    }
}

#[cfg(test)]
mod tests {
    use super::{break_lines, Item, BREAK_ALLOWED, BREAK_NEVER, ITEM_BREAK, ITEM_TEXT};

    fn word(width: f32, hang: f32, brk: u8) -> Item {
        Item { kind: ITEM_TEXT, brk, valign: 0, last_baseline: false, width, above: 0.0, below: 0.0, hang, expand: 1, hyphen: 0.0, trim: true, hangs: true, avoid: 0, tab: 0.0, tab_min: 0.0 }
    }

    /// Lines end at the last opportunity before what doesn't fit; trailing spaces hang (they don't
    /// make the line overflow); a forced break ends a line.
    #[test]
    fn lines_fill_greedily_and_spaces_hang() {
        let items = [word(20.0, 10.0, BREAK_ALLOWED), word(20.0, 10.0, BREAK_ALLOWED), word(20.0, 10.0, BREAK_ALLOWED)];
        let mut widths: Vec<f32> = items.iter().map(|i| i.width).collect();
        let lines = break_lines(&items, &mut widths, 50.0, 0.0);
        let spans: Vec<(usize, usize, f32)> = lines.iter().map(|l| (l.start, l.end, l.width)).collect();
        assert_eq!(spans, vec![(0, 2, 50.0), (2, 3, 20.0)], "20 + 10 + 20 fits 50 exactly, its space hanging");
        let glued = [word(30.0, 0.0, BREAK_NEVER), word(30.0, 0.0, BREAK_ALLOWED), word(10.0, 0.0, BREAK_ALLOWED)];
        let mut widths: Vec<f32> = glued.iter().map(|i| i.width).collect();
        let lines = break_lines(&glued, &mut widths, 40.0, 0.0);
        assert_eq!((lines[0].end, lines.len()), (2, 2), "no break inside the unbreakable pair: it overflows");
        let mut br = word(0.0, 0.0, BREAK_ALLOWED);
        br.kind = ITEM_BREAK;
        let items = [word(10.0, 0.0, BREAK_ALLOWED), br, br, word(10.0, 0.0, BREAK_ALLOWED)];
        let lines = break_lines(&items, &mut [10.0, 0.0, 0.0, 10.0], 100.0, 0.0);
        assert_eq!(lines.len(), 3, "the second break makes an empty line");
    }
}

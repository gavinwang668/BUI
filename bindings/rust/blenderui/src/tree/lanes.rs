//! Grid lanes (`display: grid-lanes`, CSS Grid 3, "masonry"): a grid in one axis (the lanes), with
//! items stacked in the other, each auto-placed item in the lane where it ends up highest.
//!
//! taffy has no grid lanes, so the container lays out as a grid twice:
//!
//! 1. each item pinned to its own track in the stacking axis, which sizes the lanes and measures each
//!    item's extent in the stacking axis: an auto-placed item counts towards every lane it could go
//!    in (§ "Grid Axis Track Sizing"), by stand-ins of it ("echoes") in the other lanes;
//! 2. the items placed in lanes (§ "grid-lanes layout algorithm": the lane whose running position is
//!    lowest, within `fit-tolerance` of it, the first in lane order), then laid out again in those
//!    lanes, all in one stacking track as long as the tallest lane, and moved down their lane.
//!
//! A subgrid item shares the lanes it spans (`subgrid/tracks.rs`).
//!
//! An auto repetition of intrinsic tracks (`repeat(auto-fill, auto)`) is counted from a hypothetical
//! layout first (§ "Intrinsic Auto-Repeat").

use super::subgrid::tracks::{sized_tracks, track_count, Echoes, Tracks};
use super::subgrid::{grow, in_flow, lines};
use super::{ix, LayoutTree, View};
use taffy::compute::compute_grid_layout;
use taffy::prelude::*;
use taffy::tree::{DetailedLayoutInfo, LayoutInput, LayoutOutput, RunMode};
use taffy::style::{GridTemplateComponent, GridTemplateRepetition, RepetitionCount, TrackSizingFunction};
use taffy::{CompactLength, ResolveOrZero};

/// `grid-lanes-direction: row`: rows are the lanes, items stack along them.
pub(crate) const LANES_ROW: u8 = 1;
/// `grid-lanes-direction: column` (`normal` is column unless only rows are templated).
pub(crate) const LANES_COLUMN: u8 = 2;
/// `fill-reverse`: items stack from the end of their lanes.
pub(crate) const FILL_REVERSE: u8 = 4;
/// `track-reverse`: ties go to the last lane.
pub(crate) const TRACK_REVERSE: u8 = 8;
/// `fit-tolerance: infinite`: every lane ties, items fill the lanes in order.
pub(crate) const TOLERANCE_INFINITE: u8 = 16;
/// `grid-lanes-pack: dense`: items fill gaps they fit in higher up their lanes.
pub(crate) const DENSE: u8 = 32;

/// `justify-content: flow-start | flow-end` (see [`Lanes::flow`]).
pub(crate) const FLOW_JUSTIFY_CONTENT: u8 = 1;
/// `align-content: flow-*`.
pub(crate) const FLOW_ALIGN_CONTENT: u8 = 2;
/// `justify-items: flow-*`.
pub(crate) const FLOW_JUSTIFY_ITEMS: u8 = 4;
/// `align-items: flow-*`.
pub(crate) const FLOW_ALIGN_ITEMS: u8 = 8;

/// A grid-lanes container's layout settings (`grid-lanes-direction`, `fit-tolerance`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lanes {
    pub flags: u8,
    /// The alignments given as `flow-start` / `flow-end` (`FLOW_*`): their start is the end of an
    /// axis the lanes reverse (`track-reverse` the lane axis, `fill-reverse` the stacking axis).
    pub flow: u8,
    /// `fit-tolerance` (`normal` is 1em, which the style resolves).
    pub tolerance: LengthPercentage,
}

impl Lanes {
    /// Whether rows are the lanes for container style `s`.
    fn rows(self, s: &Style) -> bool {
        self.flags & LANES_ROW != 0
            || (self.flags & LANES_COLUMN == 0 && s.grid_template_columns.is_empty() && !s.grid_template_rows.is_empty())
    }
}

/// `start` for `end` and back.
fn flip<T: PartialEq + Copy>(v: &mut T, start: T, end: T) {
    if *v == start {
        *v = end;
    } else if *v == end {
        *v = start;
    }
}

/// Swaps a size to (lane axis, stacking axis).
const fn split<T: Copy>(rows: bool, s: Size<T>) -> (T, T) {
    if rows { (s.height, s.width) } else { (s.width, s.height) }
}

/// A length-percentage of `basis` px.
fn resolve(l: LengthPercentage, basis: f32) -> f32 {
    let raw = l.into_raw();
    match raw.tag() {
        CompactLength::LENGTH_TAG => raw.value(),
        CompactLength::PERCENT_TAG => raw.value() * basis,
        _ if raw.is_plain_calc() => super::resolve_calc(raw.calc_value(), basis),
        _ => 0.0,
    }
}

/// What the measuring pass learns: the lanes, and each item's lane and extents.
struct Measure {
    /// How many lanes there are.
    n: u16,
    /// Each lane's size.
    lanes: Vec<f32>,
    /// From the first lane's start to the last's end.
    reach: f32,
    /// The container's content-box size in the stacking axis, when known.
    content: Option<f32>,
    /// Each item's first lane (0-based) when definite, and its span.
    want: Vec<(Option<u16>, u16)>,
    /// Each item's outer size in the stacking axis: `ext[at[i] + t]` starting in lane `t` for an
    /// auto-placed item (measured in each lane it can start in), `ext[at[i]]` for a definite one.
    ext: Vec<f32>,
    at: Vec<usize>,
}

impl Measure {
    /// Item `i`'s extent starting in lane `t`.
    fn extent(&self, i: usize, t: u16) -> f32 {
        let own = if self.want[i].0.is_some() { 0 } else { usize::from(t) };
        let end = self.at.get(i + 1).copied().unwrap_or(self.ext.len());
        self.ext.get(self.at[i] + own).filter(|_| self.at[i] + own < end).or_else(|| self.ext.get(self.at[i])).copied()
            .unwrap_or(0.0)
    }
}

/// Whether placement `p` names a line (else the item is auto-placed in that axis).
fn definite<S: taffy::CheapCloneStr>(p: &Line<GridPlacement<S>>) -> bool {
    let named = |g: &GridPlacement<S>| matches!(g, GridPlacement::Line(_) | GridPlacement::NamedLine(..));
    named(&p.start) || named(&p.end)
}

/// Whether placement `p` is auto, or numbered lines inside a grid of `n` tracks (so it makes no
/// implicit tracks).
fn within<S: taffy::CheapCloneStr>(p: &Line<GridPlacement<S>>, n: u16) -> bool {
    let n = i32::from(n);
    // a line as 1-based from the start; 0 or a name is unknown
    let line = |g: &GridPlacement<S>| match g {
        GridPlacement::Line(l) => match i32::from(l.as_i16()) {
            0 => Some(None),
            v if v > 0 => Some(Some(v)),
            v => Some(Some(n + 2 + v)),
        },
        GridPlacement::NamedLine(..) | GridPlacement::NamedSpan(..) => Some(None),
        GridPlacement::Auto | GridPlacement::Span(_) => None,
    };
    let span = |g: &GridPlacement<S>| match g {
        GridPlacement::Span(s) => i32::from(*s),
        _ => 1,
    };
    let ok = |a: i32, b: i32| a >= 1 && b <= n + 1;
    match (line(&p.start), line(&p.end)) {
        (Some(None), _) | (_, Some(None)) => false,
        (Some(Some(a)), Some(Some(b))) => ok(a.min(b), a.max(b)),
        (Some(Some(a)), None) => ok(a, a + span(&p.end)),
        (None, Some(Some(b))) => ok(b - span(&p.start), b),
        (None, None) => true,
    }
}

impl<C> LayoutTree<C> {
    /// The placement item `k` is written with in the rows (`rows`) or columns.
    pub(in crate::tree) fn placement(&self, k: NodeId, rows: bool) -> &Line<GridPlacement<String>> {
        let s = self.style(k);
        if rows { &s.grid_row } else { &s.grid_column }
    }

    /// The container style a lanes pass lays `g` out with: no tracks of its own in the stacking axis
    /// but `stack` (a track that long), each implicit one sized by its item; `tracks` stands in for its
    /// lane tracks. When the stacking range sits at the flow start (`normal` content alignment), a
    /// flexible track after it (before, with `fill-reverse`) takes the rest of the container, which
    /// the last items of the lanes align in (§ "Stacking Axis Self-Alignment"). Returns the
    /// stacking track's line, and whether that rest track is there.
    fn lanes_frame(&mut self, g: NodeId, lanes: Lanes, rows: bool, stack: Option<f32>, tracks: Option<&Tracks>) -> (u16, bool) {
        let mut s = self.style(g).clone();
        // flow-relative alignment: which of the inline (justify) and block (align) axes run backwards
        let (lane_rev, stack_rev) = (lanes.flags & TRACK_REVERSE != 0, lanes.flags & FILL_REVERSE != 0);
        let (x_rev, y_rev) = if rows { (stack_rev, lane_rev) } else { (lane_rev, stack_rev) };
        let on = |bit: u8, rev: bool| lanes.flow & bit != 0 && rev;
        if on(FLOW_JUSTIFY_CONTENT, x_rev) {
            flip(&mut s.justify_content, AlignContent::START, AlignContent::END);
        }
        if on(FLOW_ALIGN_CONTENT, y_rev) {
            flip(&mut s.align_content, AlignContent::START, AlignContent::END);
        }
        if on(FLOW_JUSTIFY_ITEMS, x_rev) {
            flip(&mut s.justify_items, AlignItems::START, AlignItems::END);
        }
        if on(FLOW_ALIGN_ITEMS, y_rev) {
            flip(&mut s.align_items, AlignItems::START, AlignItems::END);
        }
        let content = if rows { s.justify_content } else { s.align_content };
        let flow_start = if stack_rev { [AlignContent::END, AlignContent::FLEX_END] } else { [AlignContent::START, AlignContent::FLEX_START] };
        let rest = stack.is_some()
            && [AlignContent::NORMAL, AlignContent::STRETCH, flow_start[0], flow_start[1]].contains(&content);
        let mut stacked: Vec<GridTemplateComponent<_>> = stack.map(|l| GridTemplateComponent::Single(length(l))).into_iter().collect();
        let mut line = 1;
        if stack.is_none() {
            // measuring: each track as long as its item, not stretched over a definite container
            *(if rows { &mut s.justify_content } else { &mut s.align_content }) = AlignContent::START;
        }
        if rest {
            let fill = GridTemplateComponent::Single(minmax(length(0.0), fr(1.0)));
            if stack_rev {
                stacked.insert(0, fill);
                line = 2;
                // and when the items overflow, they do so at the start
                *(if rows { &mut s.justify_content } else { &mut s.align_content }) = AlignContent::END;
            } else {
                stacked.push(fill);
            }
            *(if rows { &mut s.gap.width } else { &mut s.gap.height }) = LengthPercentage::length(0.0);
        }
        let (stack_tracks, stack_names, stack_auto, lane_tracks, lane_names) = if rows {
            (&mut s.grid_template_columns, &mut s.grid_template_column_names, &mut s.grid_auto_columns,
             &mut s.grid_template_rows, &mut s.grid_template_row_names)
        } else {
            (&mut s.grid_template_rows, &mut s.grid_template_row_names, &mut s.grid_auto_rows,
             &mut s.grid_template_columns, &mut s.grid_template_column_names)
        };
        *stack_tracks = stacked;
        stack_names.clear();
        stack_auto.clear();
        if let Some(t) = tracks {
            // line names go with the components they sit between: a list of other length drops them
            if t.len() != lane_tracks.len() {
                lane_names.clear();
            }
            lane_tracks.clone_from(t);
        }
        s.grid_template_areas = None;
        self.framed.retain(|(n, _)| *n != g);
        self.framed.push((g, s));
        (line, rest)
    }

    /// Whether item `k` aligns in the stacking axis (in the gap after it, § "Stacking Axis
    /// Self-Alignment"): its alignment there isn't `normal` (or a baseline, which falls back to start)
    /// and its margins there are lengths (the alignment container is made with them).
    fn lanes_aligns(&self, k: NodeId, g: NodeId, rows: bool) -> bool {
        let s = self.style(k);
        let c = self.style(g);
        let align = (if rows { s.justify_self } else { s.align_self }).unwrap_or(if rows { c.justify_items } else { c.align_items });
        let (start, end) = if rows { (s.margin.left, s.margin.right) } else { (s.margin.top, s.margin.bottom) };
        let length = |m: LengthPercentageAuto| m.into_raw().tag() == CompactLength::LENGTH_TAG;
        align != AlignItems::NORMAL && align != AlignItems::BASELINE && length(start) && length(end)
    }

    /// Pins `k` (an item, or an echo of item `of`) to stacking lines `stack` and, when given, lane
    /// lines `lane`. `margins` (added at the start and end of the stacking axis) make its alignment
    /// container; without, it sits at the start of its track (as long as its content, not the track).
    pub(in crate::tree) fn lanes_pin(&mut self, k: NodeId, of: NodeId, rows: bool, stack: (u16, u16), lane: Option<(u16, u16)>,
                 margins: Option<(f32, f32)>) {
        let mut s = self.style(of).clone();
        let (stacking, across) = if rows { (&mut s.grid_column, &mut s.grid_row) } else { (&mut s.grid_row, &mut s.grid_column) };
        *stacking = lines(stack);
        if let Some(l) = lane {
            *across = lines(l);
        }
        if let Some((a, b)) = margins {
            let m = &mut s.margin;
            let (start, end) = if rows { (&mut m.left, &mut m.right) } else { (&mut m.top, &mut m.bottom) };
            grow(start, a);
            grow(end, b);
        } else if rows {
            s.justify_self = Some(JustifySelf::START);
        } else {
            s.align_self = Some(AlignSelf::START);
        }
        self.pinned.insert(k, s);
    }

    /// Resolves item `k`'s pinned percentage sizes in the stacking axis against `content`, the
    /// container's content box there (its containing block in that axis, § "Containing Block"; its
    /// grid area would be the track it is laid out on).
    fn lanes_percent(&mut self, k: NodeId, rows: bool, content: Option<f32>) {
        let (Some(c), Some(s)) = (content, self.pinned.get_mut(&k)) else { return };
        let of = |raw: CompactLength| (raw.tag() == CompactLength::PERCENT_TAG).then(|| raw.value() * c);
        let (size, min, max) = if rows {
            (&mut s.size.width, &mut s.min_size.width, &mut s.max_size.width)
        } else {
            (&mut s.size.height, &mut s.min_size.height, &mut s.max_size.height)
        };
        if let Some(v) = of(size.into_raw()) {
            *size = Dimension::length(v);
        }
        if let Some(v) = of(min.into_raw()) {
            *min = LengthPercentageAuto::length(v);
        }
        if let Some(v) = of(max.into_raw()) {
            *max = LengthPercentageAuto::length(v);
        }
    }

    /// Container `g`'s content-box size in the stacking axis, when `known` gives its border box.
    fn stack_content(&self, g: NodeId, rows: bool, known: Size<Option<f32>>) -> Option<f32> {
        let s = self.style(g);
        let basis = known.width;
        let edge = |p: LengthPercentage, b: LengthPercentage| {
            p.resolve_or_zero(basis, super::resolve_calc) + b.resolve_or_zero(basis, super::resolve_calc)
        };
        let (outer, inset) = if rows {
            (known.width, edge(s.padding.left, s.border.left) + edge(s.padding.right, s.border.right))
        } else {
            (known.height, edge(s.padding.top, s.border.top) + edge(s.padding.bottom, s.border.bottom))
        };
        outer.map(|o| (o - inset).max(0.0))
    }

    /// Drops what a lanes layout of `g` pinned.
    fn lanes_unpin(&mut self, g: NodeId, kids: &[NodeId]) {
        for k in kids {
            self.pinned.remove(k);
        }
        // the subgrids' frames too (each lays out with the lanes it was last measured or placed in)
        let framed = self.framed.len();
        self.framed.retain(|(n, _)| *n != g && !kids.contains(n));
        if self.framed.len() + 1 < framed {
            for &k in kids {
                self.nodes[ix(k)].cache.clear();
            }
        }
    }
}

/// Places items in lanes (§ "Grid Lanes Layout Algorithm"): `want` is each item's first lane when
/// definite and its span; `extent` its outer size in the stacking axis starting in a lane; `sizes` the
/// lanes'. An auto-placed item goes to the lane line where it starts highest (after everything in the
/// lanes it spans): those within `tolerance` of the highest tie, and the first from the auto-placement
/// cursor wins (just after the previous auto-placed item's lanes; from the last lane down with
/// `track_reverse`). With `dense`, an item then takes a gap above that which it fits in, in lanes as
/// wide as the ones it would take: of those within `tolerance` of the highest, the first (the last
/// with `track_reverse`); that leaves the cursor and the lanes' ends as they were. Returns each item's first lane and stacking offset, and the length of
/// the longest lane.
fn place(want: &[(Option<u16>, u16)], extent: impl Fn(usize, u16) -> f32, sizes: &[f32], gap: f32, tolerance: f32,
         track_reverse: bool, dense: bool) -> (Vec<(u16, f32)>, f32) {
    let n = (sizes.len() as u16).max(1);
    // each lane's items, as [start, end + gap) by start
    let mut taken: Vec<Vec<(f32, f32)>> = vec![Vec::new(); usize::from(n)];
    let mut out = Vec::with_capacity(want.len());
    let mut cursor = if track_reverse { n - 1 } else { 0 };
    for (i, &(at, span)) in want.iter().enumerate() {
        let span = span.clamp(1, n);
        let last = n - span;
        let lanes = |t: u16| &taken[usize::from(t)..usize::from(t + span)];
        // the running position of the lanes an item starting in lane `t` spans
        let end = |t: u16| lanes(t).iter().filter_map(|l| l.last()).map(|l| l.1).fold(0.0, f32::max);
        let mut t = at.map_or_else(|| {
            let low = (0..=last).map(end).fold(f32::INFINITY, f32::min);
            // past the last line it can start at, the search starts over from the first
            // (reversed, the cursor is the lane it ends in: a span starts that many lanes before)
            let from = if track_reverse {
                if cursor + 1 >= span { (cursor + 1 - span).min(last) } else { last }
            } else if cursor > last { 0 } else { cursor };
            (0..=last).map(|k| if track_reverse { (from + last + 1 - k) % (last + 1) } else { (from + k) % (last + 1) })
                .find(|&t| end(t) <= low + tolerance).unwrap_or(0)
        }, |a| a.min(last));
        let mut top = end(t);
        let mut filled = false;
        if dense {
            let wide = |t: u16| sizes.get(usize::from(t)..usize::from(t + span)).map_or(0.0, |s| s.iter().sum::<f32>());
            let width = wide(t);
            let lines = if at.is_some() { t..=t } else { 0..=last };
            // the highest gap above `top` at each line
            let gaps: Vec<(u16, f32)> = lines.filter(|&c| (wide(c) - width).abs() < 0.01).filter_map(|c| {
                let e = extent(i, c);
                let free = |p: f32| lanes(c).iter().flatten().all(|&(a, b)| p + e + gap <= a || p >= b);
                let end = end(c);
                std::iter::once(0.0).chain(lanes(c).iter().flatten().map(|l| l.1))
                    .filter(|&p| p < end && p < top && free(p)).reduce(f32::min).map(|p| (c, p))
            }).collect();
            // within `tolerance` of the highest, the first (the last with `track_reverse`)
            let low = gaps.iter().map(|g| g.1).fold(f32::INFINITY, f32::min);
            let tied = |g: &&(u16, f32)| g.1 <= low + tolerance;
            let pick = if track_reverse { gaps.iter().rev().find(tied) } else { gaps.iter().find(tied) };
            if let Some(&(c, p)) = pick {
                (t, top, filled) = (c, p, true);
            }
        }
        let e = extent(i, t);
        for l in &mut taken[usize::from(t)..usize::from(t + span)] {
            let k = l.partition_point(|&(a, _)| a <= top);
            l.insert(k, (top, top + e + gap));
        }
        out.push((t, top));
        // a definite or backfilled item leaves the cursor where it was
        if at.is_none() && !filled {
            cursor = if track_reverse { (t + n - 1) % n } else { (t + span) % n };
        }
    }
    let end = taken.iter().filter_map(|l| l.last()).map(|l| l.1).fold(0.0, f32::max);
    (out, if want.is_empty() { 0.0 } else { (end - gap).max(0.0) })
}

impl<C, M> View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    /// A grid-lanes container's layout (see the module docs).
    pub(super) fn lanes_layout(&mut self, g: NodeId, inputs: LayoutInput, lanes: Lanes) -> LayoutOutput {
        let rows = lanes.rows(self.t.style(g));
        let kids: Vec<NodeId> = self.t.nodes[ix(g)].children.iter().copied().filter(|&k| in_flow(self.t.style(k))).collect();
        let tracks = self.lanes_repeat(g, inputs, lanes, rows, &kids);
        let measured = self.lanes_measure(g, inputs, lanes, &kids, tracks.as_ref(), false);
        let out = match measured.and_then(|m| self.lanes_plan(g, rows, lanes, &kids, &m, tracks.as_ref())) {
            None => {
                // nothing to stack: a grid of the lanes, items as written
                self.t.lanes_unpin(g, &kids);
                self.t.lanes_frame(g, lanes, rows, None, tracks.as_ref());
                compute_grid_layout(self, g, inputs)
            }
            Some(shifts) => {
                let out = compute_grid_layout(self, g, inputs);
                if inputs.run_mode == RunMode::PerformLayout {
                    for (&k, &by) in kids.iter().zip(&shifts) {
                        let l = &mut self.t.nodes[ix(k)].unrounded;
                        if rows { l.location.x += by } else { l.location.y += by }
                    }
                }
                out
            }
        };
        self.t.lanes_unpin(g, &kids);
        if inputs.run_mode != RunMode::PerformLayout {
            // the items were laid out for this measuring pass: the final layout (if cached) no longer matches
            self.t.nodes[ix(g)].cache.clear();
        }
        out
    }

    /// Lays `g` out with each item on a stacking track of its own, which sizes the lanes from the
    /// items and measures each item's extent: an auto-placed item in each lane it can start in (by
    /// echoes of it, which count towards those lanes' sizes, § "Grid Axis Track Sizing"), in its
    /// first lane, or in the one it names. `all_auto`: every item is auto-placed (an intrinsic auto
    /// repetition's hypothetical layout). `None` when nothing stacks.
    fn lanes_measure(&mut self, g: NodeId, inputs: LayoutInput, lanes: Lanes, kids: &[NodeId], tracks: Option<&Tracks>,
                     all_auto: bool) -> Option<Measure> {
        if kids.is_empty() {
            return None;
        }
        let rows = lanes.rows(self.t.style(g));
        let content = self.t.stack_content(g, rows, inputs.known_dimensions);
        self.t.lanes_frame(g, lanes, rows, None, tracks);
        let auto: Vec<bool> = kids.iter().map(|&k| all_auto || !definite(self.t.placement(k, rows))).collect();
        let span = |t: &LayoutTree<C>, k: NodeId| super::subgrid::axis(t.placement(k, rows), u16::MAX).1;
        let pin = |t: &mut LayoutTree<C>, n: u16| for (i, &k) in kids.iter().enumerate() {
            let lane = auto[i].then(|| (1, 1 + span(t, k).min(n)));
            t.lanes_pin(k, k, rows, (i as u16 + 1, i as u16 + 2), lane, None);
            t.lanes_percent(k, rows, content);
        };
        // the lanes: the lane template's tracks, unless items make more (or an auto repetition is
        // left to taffy): then a layout counts them
        let s = self.t.container_style(g);
        let written = if rows { &s.grid_template_rows } else { &s.grid_template_columns };
        let fits = |t: &LayoutTree<C>, n: u16| kids.iter().zip(&auto).all(|(&k, &a)| a || within(t.placement(k, rows), n));
        let counted = track_count(written).filter(|&n| n > 0 && fits(self.t, n));
        let perform = LayoutInput { run_mode: RunMode::PerformLayout, ..inputs };
        let n = counted.unwrap_or_else(|| {
            pin(self.t, u16::MAX);
            compute_grid_layout(self, g, perform);
            match &self.t.nodes[ix(g)].detail {
                DetailedLayoutInfo::Grid(info) => (if rows { &info.rows } else { &info.columns }).positions.len() as u16,
                _ => 1,
            }
        }).max(1);
        pin(self.t, n);
        let sub: Vec<bool> = kids.iter().map(|&k| self.t.subgrid_shares(k, rows)).collect();
        let basis = split(rows, inputs.known_dimensions).0.unwrap_or(0.0);
        let lane_gap = resolve(split(rows, self.t.style(g).gap).0, basis);
        // an echo for each other lane an auto-placed item can start in, on stacking tracks after the
        // items' (a subgrid's items instead, in every lane they can be in)
        let mut track = kids.len() as u16;
        let mut echoes = Vec::new();
        let mut first = vec![0u16; kids.len()];
        for (i, &k) in kids.iter().enumerate().filter(|(i, _)| sub[*i]) {
            self.t.subgrid_hide(k, rows);
            let (at, s) = super::subgrid::axis(self.t.placement(k, rows), n);
            let starts = if auto[i] { 0..=n - s } else { at.unwrap_or(0)..=at.unwrap_or(0) };
            let mut out = Echoes { gap: lane_gap, basis, across: None, outer: (0.0, 0.0), track, made: echoes };
            self.t.subgrid_echoes(k, rows, s, starts, auto[i], &mut out);
            (track, echoes) = (out.track, out.made);
        }
        for (i, &k) in kids.iter().enumerate().filter(|(i, _)| auto[*i] && !sub[*i]) {
            let s = span(self.t, k).min(n);
            first[i] = track;
            for t in 1..=n - s {
                let e = self.t.alloc(super::StyleIx(0), None, &[]);
                track += 1;
                self.t.lanes_pin(e, k, rows, (track, track + 1), Some((t + 1, t + 1 + s)), None);
                self.t.echoes.insert(e, k);
                echoes.push(e);
            }
        }
        if counted.is_some() || !echoes.is_empty() {
            if !echoes.is_empty() {
                let mut all = self.t.nodes[ix(g)].children.clone();
                all.extend_from_slice(&echoes);
                self.t.lifted.push((g, all));
            }
            compute_grid_layout(self, g, perform);
            if !echoes.is_empty() {
                self.t.lifted.pop();
                self.t.drop_echoes(echoes);
            }
        }
        let DetailedLayoutInfo::Grid(info) = &self.t.nodes[ix(g)].detail else { return None };
        let (lane, stack) = if rows { (&info.rows, &info.columns) } else { (&info.columns, &info.rows) };
        if lane.negative_implicit_tracks != 0 || stack.positions.len() < usize::from(track) || info.items.len() < kids.len() {
            return None;
        }
        let size = |l: &Line<f32>| l.end - l.start;
        let mut m = Measure {
            n: lane.positions.len() as u16,
            lanes: lane.positions.iter().map(size).collect(),
            reach: lane.positions.last().map_or(0.0, |l| l.end) - lane.positions.first().map_or(0.0, |l| l.start),
            content,
            want: Vec::with_capacity(kids.len()),
            ext: Vec::with_capacity(usize::from(track)),
            at: Vec::with_capacity(kids.len()),
        };
        // subgrids: measured in each lane they can start in once the lanes are known
        let mut fits = Vec::new();
        for (i, (&k, p)) in kids.iter().zip(&info.items).enumerate() {
            m.at.push(m.ext.len());
            m.ext.push(size(&stack.positions[i]));
            if sub[i] {
                let (a, b) = if rows { (p.row_start, p.row_end) } else { (p.column_start, p.column_end) };
                let s = if auto[i] { super::subgrid::axis(self.t.placement(k, rows), m.n).1 } else { b.saturating_sub(a).max(1) };
                m.want.push((if auto[i] { None } else { Some(a.saturating_sub(1)) }, s));
                let starts = if auto[i] { 0..=m.n.saturating_sub(s) } else { a.saturating_sub(1)..=a.saturating_sub(1) };
                for (j, t) in starts.enumerate() {
                    if j > 0 {
                        m.ext.push(0.0);
                    }
                    fits.push((k, m.ext.len() - 1, t, s));
                }
            } else if auto[i] {
                let s = super::subgrid::axis(self.t.placement(k, rows), m.n).1;
                m.want.push((None, s));
                let from = usize::from(first[i]);
                m.ext.extend(stack.positions[from..from + usize::from(n.saturating_sub(s.min(n)))].iter().map(size));
            } else {
                let (a, b) = if rows { (p.row_start, p.row_end) } else { (p.column_start, p.column_end) };
                m.want.push((Some(a.saturating_sub(1)), b.saturating_sub(a).max(1)));
            }
        }
        let gap = resolve(split(rows, self.t.style(g).gap).0, m.reach);
        for (k, slot, t, s) in fits {
            m.ext[slot] = self.subgrid_extent(k, rows, &m.lanes, (t, s), gap, m.reach);
        }
        Some(m)
    }

    /// The lane tracks for an auto repetition of intrinsic sizes (`repeat(auto-fill, auto)`, § "Intrinsic
    /// Auto-Repeat"), which taffy can't size: in a hypothetical layout with every item auto-placed and
    /// the repetition written out `2 + (largest span - 2) / its tracks` times (once for spans of 1),
    /// each repeated track takes the size of the largest of its copies, and as many repetitions of
    /// those sizes as fit are the count. `None` when there is none.
    fn lanes_repeat(&mut self, g: NodeId, inputs: LayoutInput, lanes: Lanes, rows: bool, kids: &[NodeId]) -> Option<Tracks> {
        let s = self.t.style(g);
        let written = if rows { &s.grid_template_rows } else { &s.grid_template_columns };
        let r = written.iter().position(GridTemplateComponent::is_auto_repetition)?;
        let GridTemplateComponent::Repeat(rep) = &written[r] else { return None };
        let fixed = |c: &GridTemplateComponent<String>| match c {
            GridTemplateComponent::Single(t) => t.has_fixed_component(),
            GridTemplateComponent::Repeat(q) => q.tracks.iter().all(TrackSizingFunction::has_fixed_component),
        };
        let k = rep.tracks.len() as u16;
        // the tracks around it must be fixed (else the list is invalid, as taffy has it)
        let around = written.iter().enumerate().all(|(i, c)| i == r || (fixed(c) && !c.is_auto_repetition()));
        if k == 0 || fixed(&written[r]) || !around {
            return None;
        }
        let (written, rep) = (written.clone(), rep.clone());
        let others = track_count(&written[..r])?.saturating_add(track_count(&written[r + 1..])?);
        let largest = kids.iter().map(|&c| super::subgrid::axis(self.t.placement(c, rows), u16::MAX).1).max().unwrap_or(1);
        let reps = if largest >= 2 { 2 + (largest - 2) / k } else { 1 };
        let with = |count: RepetitionCount, tracks: Vec<TrackSizingFunction>| {
            let mut t = written.clone();
            t[r] = GridTemplateComponent::Repeat(GridTemplateRepetition { count, tracks, line_names: rep.line_names.clone() });
            t
        };
        // the tracks' intrinsic sizes, not grown into free space: a lane axis of no length
        let mut none = inputs;
        if rows {
            none.known_dimensions.height = Some(0.0);
        } else {
            none.known_dimensions.width = Some(0.0);
        }
        let m = self.lanes_measure(g, none, lanes, kids, Some(&with(RepetitionCount::Count(reps), rep.tracks.clone())), true);
        // without items, every track is as small as it gets: the lane sizes the hypothetical layout gave
        let sized = m.map_or_else(Vec::new, |m| m.lanes);
        let at = |i: u16| sized.get(usize::from(i)).copied().unwrap_or(0.0);
        let before = track_count(&written[..r])?;
        let sizes: Vec<TrackSizingFunction> = (0..k)
            .map(|j| length((0..reps).map(|q| at(before + q * k + j)).fold(0.0, f32::max)))
            .collect();
        // how many repetitions fit: the same grid with those lengths, and no items, counts them
        let mut fixed_tracks: Tracks = Vec::with_capacity(usize::from(others) + 1);
        fixed_tracks.extend((0..before).map(|i| GridTemplateComponent::Single(length(at(i)))));
        fixed_tracks.push(GridTemplateComponent::Repeat(GridTemplateRepetition {
            count: RepetitionCount::AutoFill, tracks: sizes, line_names: Vec::new() }));
        let after = before + reps * k;
        fixed_tracks.extend((after..after + others - before).map(|i| GridTemplateComponent::Single(length(at(i)))));
        self.t.lanes_unpin(g, kids);
        self.t.lanes_frame(g, lanes, rows, None, Some(&fixed_tracks));
        self.t.lifted.push((g, Vec::new()));
        compute_grid_layout(self, g, LayoutInput { run_mode: RunMode::PerformLayout, ..inputs });
        self.t.lifted.pop();
        let n = match &self.t.nodes[ix(g)].detail {
            DetailedLayoutInfo::Grid(info) => (if rows { &info.rows } else { &info.columns }).positions.len() as u16,
            _ => others + k,
        };
        let count = (n.saturating_sub(others) / k).max(1);
        Some(with(RepetitionCount::Count(count), rep.tracks))
    }

    /// From the measuring pass: each item's lane and its alignment container in the stacking axis,
    /// pinned for the second pass, which lays them out on a stacking track as long as the longest lane
    /// (the lanes keep the sizes every item they could hold gave them, not just the items placed in
    /// them). Returns how far each item moves down the track after it; `None` when there is nothing
    /// to stack.
    fn lanes_plan(&mut self, g: NodeId, rows: bool, lanes: Lanes, kids: &[NodeId], m: &Measure, tracks: Option<&Tracks>)
        -> Option<Vec<f32>> {
        let n = m.n.max(1);
        let s = self.t.style(g);
        let (_, gap) = split(rows, s.gap);
        let gap = resolve(gap, split(rows, self.t.nodes[ix(g)].unrounded.size).1);
        let sized = sized_tracks(tracks.unwrap_or(if rows { &s.grid_template_rows } else { &s.grid_template_columns }), &m.lanes);
        let tolerance = if lanes.flags & TOLERANCE_INFINITE != 0 { f32::INFINITY } else { resolve(lanes.tolerance, m.reach) };
        let (spots, longest) = place(&m.want, |i, t| m.extent(i, t), &m.lanes, gap, tolerance.max(0.0),
                                     lanes.flags & TRACK_REVERSE != 0, lanes.flags & DENSE != 0);
        let span = |i: usize| m.want[i].1.clamp(1, n);
        // each lane's items, by where they start
        let mut held: Vec<Vec<f32>> = vec![Vec::new(); usize::from(n)];
        for (i, &(t, at)) in spots.iter().enumerate() {
            for l in &mut held[usize::from(t)..usize::from(t + span(i))] {
                l.push(at);
            }
        }
        // where the next item in an item's lanes starts (none: it is the last in them)
        let next = |i: usize| {
            let (t, at) = spots[i];
            held[usize::from(t)..usize::from(t + span(i))].iter().flatten().copied().filter(|&p| p > at).reduce(f32::min)
        };
        let (track, rest) = self.t.lanes_frame(g, lanes, rows, Some(longest), Some(&sized));
        let reverse = lanes.flags & FILL_REVERSE != 0;
        let lane_gap = resolve(split(rows, self.t.style(g).gap).0, m.reach);
        let mut shifts = Vec::with_capacity(kids.len());
        for (i, (&k, &(t, at))) in kids.iter().zip(&spots).enumerate() {
            let lane = Some((t + 1, t + 1 + span(i)));
            if self.t.subgrid_shares(k, rows) {
                // pinned first, then fit: the fit stretches the pinned style over the lanes
                self.t.lanes_pin(k, k, rows, (track, track + 1), lane, None);
                self.t.lanes_percent(k, rows, m.content);
                self.t.subgrid_fit(k, rows, &m.lanes, (t, span(i)), lane_gap, m.reach);
                shifts.push(if reverse { longest - at - m.extent(i, t) } else { at });
                continue;
            }
            if !self.t.lanes_aligns(k, g, rows) {
                self.t.lanes_pin(k, k, rows, (track, track + 1), lane, None);
                self.t.lanes_percent(k, rows, m.content);
                // `fill-reverse` stacks up from the end of the track
                shifts.push(if reverse { longest - at - m.extent(i, t) } else { at });
                continue;
            }
            // its alignment container: from where it is to the next item's gutter, or the end of the
            // track (and of the rest of the container, when there is a track for that)
            let (stack, far) = match next(i) {
                Some(p) => ((track, track + 1), longest - (p - gap)),
                None if rest => ((1, 3), 0.0),
                None => ((track, track + 1), 0.0),
            };
            let margins = if reverse { (far, at) } else { (at, far) };
            self.t.lanes_pin(k, k, rows, stack, lane, Some(margins));
            self.t.lanes_percent(k, rows, m.content);
            shifts.push(0.0);
        }
        Some(shifts)
    }
}

#[cfg(test)]
mod tests {
    use super::place;

    /// Each item goes to the lane that is shortest so far; ties go to the first lane.
    #[test]
    fn items_fill_the_shortest_lane() {
        let (spots, longest) = place(&[(None, 1); 4], |i, _| [50.0, 20.0, 30.0, 10.0][i], &[1.0; 3], 0.0, 0.0, false, false);
        assert_eq!(spots, vec![(0, 0.0), (1, 0.0), (2, 0.0), (1, 20.0)]);
        assert_eq!(longest, 50.0);
    }

    /// Lanes within the tolerance of the shortest tie, so the first of them wins; a definite lane is kept.
    #[test]
    fn tolerance_and_definite_lanes() {
        let (spots, _) = place(&[(None, 1), (None, 1), (None, 1)], |i, _| [10.0, 12.0, 5.0][i], &[1.0; 2], 0.0, 5.0, false, false);
        assert_eq!(spots[2], (0, 10.0));
        let (spots, _) = place(&[(Some(1), 1), (None, 2)], |i, _| [10.0, 5.0][i], &[1.0; 2], 4.0, 0.0, false, false);
        assert_eq!(spots, vec![(1, 0.0), (0, 14.0)]);
    }

    /// Dense packing fills the highest gap an item fits in (column-dense-packing-005).
    #[test]
    fn dense_fills_gaps() {
        let want = [(None, 1), (None, 1), (None, 3), (Some(1), 2), (None, 3), (None, 1)];
        let (spots, longest) = place(&want, |_, _| 20.0, &[1.0; 3], 0.0, 0.0, false, true);
        assert_eq!(spots[5], (2, 0.0));
        assert_eq!(longest, 80.0);
    }
}

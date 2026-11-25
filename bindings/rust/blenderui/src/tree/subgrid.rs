//! `subgrid` (CSS Grid 2 §9): a grid item that is a grid itself and takes its tracks in an axis from
//! its parent grid, so the items of sibling subgrids (cards' headers, bodies and footers) line up.
//!
//! taffy has no subgrids. A subgrid in both axes has its items laid out as the parent's own, which is
//! what the spec's track sizing amounts to (§9.4: a subgrid's items take part in the parent's track
//! sizing, with the subgrid's margin, border and padding added to the items at its edges):
//!
//! 1. the parent lays out as usual, to learn where each item goes (and so the area of each subgrid);
//! 2. it lays out again with every item pinned to that area and each subgrid replaced by its items,
//!    placed inside the subgrid's area (definite lines first, then auto-placement within it: a
//!    subgrid has no implicit tracks, so items that don't fit are clamped into it, §9.2);
//! 3. each subgrid gets its area back as its own box, and its items move to be relative to it.
//!
//! A subgrid in one axis keeps its own tracks in the other, so it stays an item: its items size the
//! parent's tracks it spans, then it lays out with theirs (`tracks.rs`), between steps 1 and 2.
//! Line names in the subgrid (`subgrid [a] [b]`) aren't used, nor the gap of a subgrid in both axes.

use super::{ix, LayoutTree, View};
use taffy::compute::compute_grid_layout;
use taffy::prelude::*;
use taffy::tree::{DetailedLayoutInfo, LayoutInput, LayoutOutput, RunMode};
use taffy::geometry::AbsoluteAxis;
use taffy::{CompactLength, DetailedGridItemsInfo, ResolveOrZero};

pub(super) mod tracks;

use tracks::Echoes;

/// `grid-template-columns: subgrid`.
pub(crate) const SUBGRID_COLUMNS: u8 = 1;
/// `grid-template-rows: subgrid`.
pub(crate) const SUBGRID_ROWS: u8 = 2;

/// Where a track span starts and ends.
type Span = (f32, f32);

/// A subgrid in the second pass: its parent-grid lines and the items laid out in it.
struct Lift {
    node: NodeId,
    rows: (u16, u16),
    columns: (u16, u16),
    items: Vec<NodeId>,
}

/// A subgrid in one axis (`rows`, or the columns), and the parent's tracks it spans there.
struct Shared {
    node: NodeId,
    rows: bool,
    lines: (u16, u16),
    /// The lines it spans in the other axis.
    other: (u16, u16),
}

/// What the first pass found: the second pass's items, and the subgrids.
struct Plan {
    items: Vec<NodeId>,
    lifts: Vec<Lift>,
    shared: Vec<Shared>,
}

impl<C> LayoutTree<C> {
    /// The children layout sees: a lifting grid's own and lifted items while it lifts.
    #[inline(always)]
    pub(super) fn laid_out(&self, id: NodeId) -> &[NodeId] {
        if !self.lifted.is_empty() && let Some((_, items)) = self.lifted.iter().rev().find(|(g, _)| *g == id) {
            return items;
        }
        &self.nodes[ix(id)].children
    }

    /// Whether grid item `k` is a subgrid with items.
    fn subgrid_of(&self, k: NodeId) -> bool {
        let n = &self.nodes[ix(k)];
        let s = self.style(k);
        n.subgrid != 0 && s.display == Display::Grid && s.position != Position::Absolute
            && n.children.iter().any(|&c| in_flow(self.style(c)))
    }

    /// The second pass's items and pinned styles for grid `g`, from where the first pass put things;
    /// `None` when there is no subgrid (or a placement it can't pin: lines before the first).
    fn plan(&mut self, g: NodeId) -> Option<Plan> {
        let DetailedLayoutInfo::Grid(info) = &self.nodes[ix(g)].detail else { return None };
        if info.rows.negative_implicit_tracks != 0 || info.columns.negative_implicit_tracks != 0 {
            return None;
        }
        let places: Vec<DetailedGridItemsInfo> = info.items.clone();
        let mut items = Vec::with_capacity(self.nodes[ix(g)].children.len());
        let mut lifts = Vec::new();
        let mut shared = Vec::new();
        let mut placed = places.iter();
        for k in 0..self.nodes[ix(g)].children.len() {
            let child = self.nodes[ix(g)].children[k];
            if !in_flow(self.style(child)) {
                items.push(child);
                continue;
            }
            let p = placed.next()?;
            let (rows, columns) = ((p.row_start, p.row_end), (p.column_start, p.column_end));
            let axes = self.nodes[ix(child)].subgrid;
            if self.subgrid_of(child) && axes != super::SUBGRID_COLUMNS | super::SUBGRID_ROWS {
                // in one axis: an item, with its own tracks in the other
                let mut s = self.style(child).clone();
                s.grid_row = lines(rows);
                s.grid_column = lines(columns);
                self.pinned.insert(child, s);
                items.push(child);
                let by_rows = axes & super::SUBGRID_ROWS != 0;
                let (lines, other) = if by_rows { (rows, columns) } else { (columns, rows) };
                shared.push(Shared { node: child, rows: by_rows, lines, other });
            } else if self.subgrid_of(child) {
                let lift = self.lift(child, rows, columns);
                items.extend_from_slice(&lift.items);
                lifts.push(lift);
            } else {
                let mut s = self.style(child).clone();
                s.grid_row = lines(rows);
                s.grid_column = lines(columns);
                self.pinned.insert(child, s);
                items.push(child);
            }
        }
        (!lifts.is_empty() || !shared.is_empty()).then_some(Plan { items, lifts, shared })
    }

    /// Places subgrid `node`'s in-flow items in its area (parent lines `rows` x `columns`), pinning
    /// each one's style there with the subgrid's margin, border and padding added at the edges.
    fn lift(&mut self, node: NodeId, rows: (u16, u16), columns: (u16, u16)) -> Lift {
        let axes = self.nodes[ix(node)].subgrid;
        let s = self.style(node);
        let by_rows = s.grid_auto_flow.primary_axis() == AbsoluteAxis::Horizontal;
        let dense = s.grid_auto_flow.is_dense();
        let shares = |axis: u8, (a, b): (u16, u16)| if axes & axis != 0 { b - a } else { 1 };
        let (nr, nc) = (shares(super::SUBGRID_ROWS, rows), shares(super::SUBGRID_COLUMNS, columns));
        let kids: Vec<NodeId> = self.nodes[ix(node)].children.iter().copied().filter(|&c| in_flow(self.style(c))).collect();
        let wants: Vec<[(Option<u16>, u16); 2]> = kids.iter().map(|&c| {
            let st = self.style(c);
            [axis(&st.grid_row, nr), axis(&st.grid_column, nc)]
        }).collect();
        let spots = place(&wants, nr, nc, by_rows, dense);
        let l = self.nodes[ix(node)].unrounded;
        let edge = |m: f32, b: f32, p: f32| m + b + p;
        let (top, bottom) = (edge(l.margin.top, l.border.top, l.padding.top), edge(l.margin.bottom, l.border.bottom, l.padding.bottom));
        let (left, right) = (edge(l.margin.left, l.border.left, l.padding.left), edge(l.margin.right, l.border.right, l.padding.right));
        for (&c, &[(r, rs), (k, ks)]) in kids.iter().zip(&spots) {
            let mut st = self.style(c).clone();
            let (r0, r1) = if axes & super::SUBGRID_ROWS != 0 { (rows.0 + r, rows.0 + r + rs) } else { rows };
            let (c0, c1) = if axes & super::SUBGRID_COLUMNS != 0 { (columns.0 + k, columns.0 + k + ks) } else { columns };
            st.grid_row = lines((r0, r1));
            st.grid_column = lines((c0, c1));
            if r == 0 { grow(&mut st.margin.top, top) }
            if r + rs == nr { grow(&mut st.margin.bottom, bottom) }
            if k == 0 { grow(&mut st.margin.left, left) }
            if k + ks == nc { grow(&mut st.margin.right, right) }
            self.pinned.insert(c, st);
        }
        Lift { node, rows, columns, items: kids }
    }

    /// Drops the pinned styles of grid `g`'s items and its subgrids' items, and the frames of `g` and
    /// its subgrids in one axis (each laid out with the tracks it was fit to).
    fn unpin(&mut self, g: NodeId, shared: &[Shared]) {
        let Self { nodes, pinned, .. } = self;
        for &c in &nodes[ix(g)].children {
            pinned.remove(&c);
            if nodes[ix(c)].subgrid != 0 {
                for k in &nodes[ix(c)].children {
                    pinned.remove(k);
                }
            }
        }
        if !shared.is_empty() {
            self.framed.retain(|(n, _)| *n != g && !shared.iter().any(|s| s.node == *n));
            for s in shared {
                self.nodes[ix(s.node)].cache.clear();
            }
        }
    }

    /// Gives each subgrid its area in grid `g` (just laid out) and moves its items to be relative to it.
    fn settle(&mut self, g: NodeId, lifts: &[Lift]) {
        let DetailedLayoutInfo::Grid(info) = &self.nodes[ix(g)].detail else { return };
        let span = |pos: &[Line<f32>], (a, b): (u16, u16)| {
            let (first, last) = (pos.get(usize::from(a) - 1), pos.get(usize::from(b) - 2));
            first.zip(last).map(|(f, l)| (f.start, l.end))
        };
        let areas: Vec<Option<(Span, Span)>> = lifts.iter()
            .map(|lift| span(&info.columns.positions, lift.columns).zip(span(&info.rows.positions, lift.rows)))
            .collect();
        for (lift, area) in lifts.iter().zip(areas) {
            let Some(((x0, x1), (y0, y1))) = area else { continue };
            let n = &mut self.nodes[ix(lift.node)];
            let (m, l) = (n.unrounded.margin, &mut n.unrounded);
            // a shared axis is its area; the other is where it was laid out as an item
            if n.subgrid & super::SUBGRID_COLUMNS != 0 {
                l.location.x = x0 + m.left;
                l.size.width = (x1 - x0 - m.left - m.right).max(0.0);
            }
            if n.subgrid & super::SUBGRID_ROWS != 0 {
                l.location.y = y0 + m.top;
                l.size.height = (y1 - y0 - m.top - m.bottom).max(0.0);
            }
            let at = l.location;
            for &k in &lift.items {
                let loc = &mut self.nodes[ix(k)].unrounded.location;
                loc.x -= at.x;
                loc.y -= at.y;
            }
        }
    }
}

impl<C, M> View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    /// A grid's layout, with its subgrids' items as its own (see the module docs).
    pub(super) fn grid_layout(&mut self, g: NodeId, inputs: LayoutInput) -> LayoutOutput {
        if let Some(lanes) = self.t.nodes[ix(g)].lanes {
            return self.lanes_layout(g, inputs, lanes);
        }
        let t = &self.t;
        if !t.nodes[ix(g)].children.iter().any(|&k| t.nodes[ix(k)].subgrid != 0) {
            return compute_grid_layout(self, g, inputs);
        }
        let first = compute_grid_layout(self, g, LayoutInput { run_mode: RunMode::PerformLayout, ..inputs });
        let (out, shared) = match self.t.plan(g) {
            None => (first, Vec::new()),
            Some(plan) => {
                self.t.lifted.push((g, plan.items));
                if !plan.shared.is_empty() {
                    self.share(g, inputs, &plan.shared);
                }
                let out = compute_grid_layout(self, g, inputs);
                self.t.lifted.pop();
                if inputs.run_mode == RunMode::PerformLayout {
                    self.t.settle(g, &plan.lifts);
                }
                (out, plan.shared)
            }
        };
        self.t.unpin(g, &shared);
        if inputs.run_mode != RunMode::PerformLayout {
            // the first pass laid the items out for a size that may not be the final one: this
            // grid's final layout (if cached) no longer matches its items'
            self.t.nodes[ix(g)].cache.clear();
        }
        out
    }
}

impl<C, M> View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    /// Sizes the tracks grid `g`'s subgrids in one axis share from their items (a pass with echoes of
    /// them in tracks of their own after the grid's), then frames `g` with those tracks at those
    /// sizes, and each subgrid with its part of them, for the pass that follows.
    fn share(&mut self, g: NodeId, inputs: LayoutInput, shared: &[Shared]) {
        let DetailedLayoutInfo::Grid(info) = &self.t.nodes[ix(g)].detail else { return };
        let counts = (info.rows.positions.len() as u16, info.columns.positions.len() as u16);
        let basis = inputs.known_dimensions.width.unwrap_or(0.0);
        let s = self.t.style(g);
        let gaps = (s.gap.height.resolve_or_zero(Some(basis), super::resolve_calc), s.gap.width.resolve_or_zero(Some(basis), super::resolve_calc));
        // echoes of subgrids in rows go on columns after the grid's, and the other way round
        let mut across = [Echoes { gap: gaps.1, basis, across: None, outer: (0.0, 0.0), track: counts.0, made: Vec::new() },
                          Echoes { gap: gaps.0, basis, across: None, outer: (0.0, 0.0), track: counts.1, made: Vec::new() }];
        // each subgrid's content size in the other axis: its area there from the first pass, less its edges
        let areas: Vec<Option<f32>> = shared.iter().map(|sg| {
            let pos = if sg.rows { &info.columns.positions } else { &info.rows.positions };
            let (a, b) = sg.other;
            pos.get(usize::from(a) - 1).zip(pos.get(usize::from(b) - 2)).map(|(f, l)| l.end - f.start)
        }).collect();
        for (sg, area) in shared.iter().zip(areas) {
            self.t.subgrid_hide(sg.node, sg.rows);
            let (x, y) = self.t.subgrid_edges(sg.node, !sg.rows, basis);
            across[usize::from(sg.rows)].across = area.map(|w| (w - x - y).max(0.0));
            let (a, b) = sg.lines;
            self.t.subgrid_echoes(sg.node, sg.rows, b - a, a - 1..=a - 1, false, &mut across[usize::from(sg.rows)]);
        }
        let [by_columns, by_rows] = across;
        let mut echoes = by_columns.made;
        echoes.extend(by_rows.made);
        if let Some((_, items)) = self.t.lifted.last_mut() {
            items.extend_from_slice(&echoes);
        }
        compute_grid_layout(self, g, LayoutInput { run_mode: RunMode::PerformLayout, ..inputs });
        if let Some((_, items)) = self.t.lifted.last_mut() {
            items.truncate(items.len() - echoes.len());
        }
        self.t.drop_echoes(echoes);
        let DetailedLayoutInfo::Grid(info) = &self.t.nodes[ix(g)].detail else { return };
        let size = |l: &Line<f32>| l.end - l.start;
        let rows: Vec<f32> = info.rows.positions.iter().take(usize::from(counts.0)).map(size).collect();
        let columns: Vec<f32> = info.columns.positions.iter().take(usize::from(counts.1)).map(size).collect();
        let mut frame = self.t.container_style(g).clone();
        // line names go with the components they sit between: a list of other length drops them
        let fit = |t: &mut tracks::Tracks, names: &mut Vec<Vec<String>>, sizes: &[f32]| {
            let sized = tracks::sized_tracks(t, sizes);
            if sized.len() != t.len() {
                names.clear();
            }
            *t = sized;
        };
        if shared.iter().any(|s| s.rows) {
            fit(&mut frame.grid_template_rows, &mut frame.grid_template_row_names, &rows);
        }
        if shared.iter().any(|s| !s.rows) {
            fit(&mut frame.grid_template_columns, &mut frame.grid_template_column_names, &columns);
        }
        self.t.framed.retain(|(n, _)| *n != g);
        self.t.framed.push((g, frame));
        for sg in shared {
            let (a, b) = sg.lines;
            let (sizes, gap) = if sg.rows { (&rows, gaps.0) } else { (&columns, gaps.1) };
            self.t.subgrid_fit(sg.node, sg.rows, sizes, (a - 1, b - a), gap, basis);
        }
    }
}

pub(super) fn in_flow(s: &Style) -> bool {
    s.display != Display::None && s.position != Position::Absolute
}

/// Grid lines `a` to `b` (1-based, `a < b`).
pub(super) fn lines<S: taffy::CheapCloneStr>((a, b): (u16, u16)) -> Line<GridPlacement<S>> {
    Line { start: line(a as i16), end: line(b as i16) }
}

/// Adds `by` to a length margin (`auto` and percentages keep theirs).
pub(super) fn grow(m: &mut LengthPercentageAuto, by: f32) {
    let raw = m.into_raw();
    if by != 0.0 && raw.tag() == CompactLength::LENGTH_TAG {
        *m = LengthPercentageAuto::length(raw.value() + by);
    }
}

/// An item's placement in one axis of a subgrid of `n` tracks: its first track if definite (0-based,
/// clamped into the subgrid) and its span.
pub(super) fn axis<S: taffy::CheapCloneStr>(p: &Line<GridPlacement<S>>, n: u16) -> (Option<u16>, u16) {
    let n = i32::from(n.max(1));
    let at = |g: &GridPlacement<S>| match g {
        GridPlacement::Line(l) => {
            let v = i32::from(l.as_i16());
            Some(if v > 0 { v - 1 } else { n + 1 + v })
        }
        _ => None,
    };
    let span_of = |g: &GridPlacement<S>| match g {
        GridPlacement::Span(s) => Some(i32::from(*s)),
        _ => None,
    };
    let (start, span) = match (at(&p.start), at(&p.end)) {
        (Some(a), Some(b)) if a != b => (Some(a.min(b)), (a - b).abs()),
        (Some(a), _) => (Some(a), span_of(&p.end).unwrap_or(1)),
        (None, Some(b)) => {
            let s = span_of(&p.start).unwrap_or(1);
            (Some(b - s), s)
        }
        (None, None) => (None, span_of(&p.start).or_else(|| span_of(&p.end)).unwrap_or(1)),
    };
    let span = span.clamp(1, n);
    (start.map(|s| s.clamp(0, n - span) as u16), span as u16)
}

/// Grid item placement (CSS Grid §8.5) inside a subgrid of `nr` x `nc` tracks: items with both lines
/// first, then the rest in order along the auto-flow axis. An item that fits nowhere is clamped to
/// the subgrid's end (a subgrid has no implicit tracks). `wants`: (row, column) per item; returns
/// ((row, row span), (column, column span)).
fn place(wants: &[[(Option<u16>, u16); 2]], nr: u16, nc: u16, by_rows: bool, dense: bool) -> Vec<[(u16, u16); 2]> {
    let (nr, nc) = (nr.max(1), nc.max(1));
    let mut used = vec![false; usize::from(nr) * usize::from(nc)];
    let fits = |used: &[bool], r: u16, c: u16, rs: u16, cs: u16| {
        r + rs <= nr && c + cs <= nc
            && (r..r + rs).all(|y| (c..c + cs).all(|x| !used[usize::from(y) * usize::from(nc) + usize::from(x)]))
    };
    let take = |used: &mut [bool], r: u16, c: u16, rs: u16, cs: u16| {
        for y in r..(r + rs).min(nr) {
            for x in c..(c + cs).min(nc) {
                used[usize::from(y) * usize::from(nc) + usize::from(x)] = true;
            }
        }
    };
    let mut out = vec![[(0, 1), (0, 1)]; wants.len()];
    for (o, &[(r, rs), (c, cs)]) in out.iter_mut().zip(wants) {
        if let (Some(r), Some(c)) = (r, c) {
            take(&mut used, r, c, rs, cs);
            *o = [(r, rs), (c, cs)];
        }
    }
    // flow in (major, minor) terms: rows then columns, or columns then rows
    let (nmaj, nmin) = if by_rows { (nr, nc) } else { (nc, nr) };
    let mut cursor = (0u16, 0u16);
    for (o, &[(r, rs), (c, cs)]) in out.iter_mut().zip(wants) {
        if r.is_some() && c.is_some() {
            continue;
        }
        let ((maj, majs), (min, mins)) = if by_rows { ((r, rs), (c, cs)) } else { ((c, cs), (r, rs)) };
        if dense {
            cursor = (0, 0);
        }
        let at = |a: u16, b: u16| if by_rows { (a, b) } else { (b, a) };
        let free = |used: &[bool], a: u16, b: u16| {
            let ((y, x), (ys, xs)) = (at(a, b), at(majs, mins));
            fits(used, y, x, ys, xs)
        };
        let spot = match (maj, min) {
            // a definite major line: the first free minor position in it
            (Some(a), _) => (0..nmin).find(|&b| free(&used, a, b)).map(|b| (a, b)).or(Some((a, 0))),
            // a definite minor line: the first major line from the cursor where it fits
            (None, Some(b)) => {
                let from = if b < cursor.1 { cursor.0 + 1 } else { cursor.0 };
                (from..nmaj).find(|&a| free(&used, a, b)).map(|a| (a, b))
            }
            (None, None) => (cursor.0..nmaj)
                .flat_map(|a| (if a == cursor.0 { cursor.1 } else { 0 }..nmin).map(move |b| (a, b)))
                .find(|&(a, b)| free(&used, a, b)),
        };
        let (a, b) = spot.unwrap_or((nmaj.saturating_sub(majs), min.unwrap_or(0)));
        cursor = (a, b + mins);
        let ((y, x), (ys, xs)) = (at(a, b), at(majs, mins));
        take(&mut used, y, x, ys, xs);
        *o = [(y, ys), (x, xs)];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::place;

    /// Auto items fill a subgrid row by row; a definite one keeps its cell and the others flow round it.
    #[test]
    fn auto_placement_flows_round_definite_items() {
        let auto = [(None, 1), (None, 1)];
        let got = place(&[auto, [(Some(0), 1), (Some(0), 1)], auto, auto], 2, 2, true, false);
        assert_eq!(got, vec![[(0, 1), (1, 1)], [(0, 1), (0, 1)], [(1, 1), (0, 1)], [(1, 1), (1, 1)]]);
    }

    /// No implicit tracks: an item past the end is clamped into the subgrid.
    #[test]
    fn overflow_is_clamped() {
        let got = place(&[[(None, 1), (None, 1)]; 3], 2, 1, true, false);
        assert_eq!(got[2], [(1, 1), (0, 1)]);
    }
}

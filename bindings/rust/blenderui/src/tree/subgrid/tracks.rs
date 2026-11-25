//! Track lists, and subgrids that share one axis with their parent: a grid's or a grid-lanes
//! container's (CSS Grid 2 § "Subgrids", Grid 3 § "Subgrids").
//!
//! The subgrid's items size the parent's tracks it spans (§ "Subgrid Sizing Algorithm": with the
//! subgrid's margin, border and padding added at its edges), by echoes of them on tracks of their own
//! in the other axis; the subgrid itself adds nothing there. Then it is stretched over those tracks
//! and lays out with its tracks in that axis theirs, less its edges at the first and last (and half
//! the difference of its own gap and the parent's at the others: its gutters are centered on the
//! parent's). In the other axis its tracks are its own.

use super::super::{ix, resolve_calc, LayoutTree, View};
use super::{axis, grow, in_flow, place};
use taffy::geometry::AbsoluteAxis;
use taffy::prelude::*;
use taffy::style::{GridTemplateComponent, RepetitionCount, TrackSizingFunction};
use taffy::tree::{LayoutInput, RequestedAxis, RunMode, SizingMode};
use taffy::{LayoutPartialTree, ResolveOrZero};

/// Echoes being made for a pass: the parent's gap in the shared axis and the basis for percentages,
/// the subgrid's content size in the other axis when known (its items are as long there, when its
/// tracks there aren't lengths), the edges of outer subgrids, the last track in the other axis (each echo goes on the one after),
/// and the echoes.
pub(in crate::tree) struct Echoes {
    pub gap: f32,
    pub basis: f32,
    pub across: Option<f32>,
    /// Margin, border and padding of the subgrids around the one echoed at its start and end, which
    /// its items there get too.
    pub outer: (f32, f32),
    pub track: u16,
    pub made: Vec<NodeId>,
}

/// A track list (`grid-template-columns` / `-rows`).
pub(in crate::tree) type Tracks = Vec<GridTemplateComponent<String>>;

/// How many tracks `template` has (`None` with an auto repetition).
pub(in crate::tree) fn track_count(template: &[GridTemplateComponent<String>]) -> Option<u16> {
    template.iter().try_fold(0u16, |n, c| match c {
        GridTemplateComponent::Single(_) => Some(n.saturating_add(1)),
        GridTemplateComponent::Repeat(r) => match r.count {
            RepetitionCount::Count(k) => Some(n.saturating_add(k.saturating_mul(r.tracks.len() as u16))),
            RepetitionCount::AutoFill | RepetitionCount::AutoFit => None,
        },
    })
}

/// Whether a track's size doesn't depend on its items (a length or percentage).
fn fixed(t: &TrackSizingFunction) -> bool {
    !t.min.is_intrinsic() && !t.max.is_intrinsic() && !t.max.is_fr()
}

/// Track list `written` with its tracks of other sizes (intrinsic, flexible) at the `sizes` they came
/// out at, for a pass that keeps them. Fixed tracks keep theirs (a percentage of a size known only
/// then), and so does an auto repetition of them (whose empty tracks `auto-fit` collapses); tracks
/// past the list (implicit) are added.
pub(in crate::tree) fn sized_tracks(written: &[GridTemplateComponent<String>], sizes: &[f32]) -> Tracks {
    let others = track_count(&written.iter().filter(|c| !c.is_auto_repetition()).cloned().collect::<Tracks>()).unwrap_or(0);
    let repeated = usize::from(u16::try_from(sizes.len()).unwrap_or(u16::MAX).saturating_sub(others));
    let mut out = Tracks::with_capacity(written.len());
    let mut at = 0;
    let one = |t: TrackSizingFunction, at: usize| GridTemplateComponent::Single(if fixed(&t) {
        t
    } else {
        length(sizes.get(at).copied().unwrap_or(0.0))
    });
    for c in written {
        match c {
            GridTemplateComponent::Single(t) => {
                out.push(one(*t, at));
                at += 1;
            }
            GridTemplateComponent::Repeat(_) if c.is_auto_repetition() => {
                out.push(c.clone());
                at += repeated;
            }
            GridTemplateComponent::Repeat(r) => {
                let RepetitionCount::Count(k) = r.count else { continue };
                for _ in 0..k {
                    for t in &r.tracks {
                        out.push(one(*t, at));
                        at += 1;
                    }
                }
            }
        }
    }
    out.extend(sizes.iter().skip(at).map(|&l| GridTemplateComponent::Single(length(l))));
    out
}

impl<C> LayoutTree<C> {
    /// Whether item `k` is a subgrid sharing the parent's rows (`rows`) or columns.
    pub(in crate::tree) fn subgrid_shares(&self, k: NodeId, rows: bool) -> bool {
        let axis = if rows { super::SUBGRID_ROWS } else { super::SUBGRID_COLUMNS };
        self.nodes[ix(k)].subgrid & axis != 0 && self.style(k).display == Display::Grid
    }

    /// Subgrid `sg`'s margin, border and padding at the start and end of the shared axis (`basis` for
    /// percentages): how far its first and last tracks are inside the parent's.
    pub(in crate::tree) fn subgrid_edges(&self, sg: NodeId, rows: bool, basis: f32) -> (f32, f32) {
        let s = self.style(sg);
        let b = Some(basis);
        let edge = |m: LengthPercentageAuto, w: LengthPercentage, p: LengthPercentage| {
            m.resolve_or_zero(b, resolve_calc) + w.resolve_or_zero(b, resolve_calc) + p.resolve_or_zero(b, resolve_calc)
        };
        let (m, w, p) = (s.margin, s.border, s.padding);
        if rows {
            (edge(m.top, w.top, p.top), edge(m.bottom, w.bottom, p.bottom))
        } else {
            (edge(m.left, w.left, p.left), edge(m.right, w.right, p.right))
        }
    }

    /// Subgrid `sg`'s gutter in the shared axis (`gap`, the parent's, when its own is 0, as for
    /// `normal`) and what each of its tracks gives up for it at an inner edge: half the difference.
    fn subgrid_gap(&self, sg: NodeId, rows: bool, gap: f32, basis: f32) -> (f32, f32) {
        let s = self.style(sg);
        let own = (if rows { s.gap.height } else { s.gap.width }).resolve_or_zero(Some(basis), resolve_calc);
        if own == 0.0 { (gap, 0.0) } else { (own, (own - gap) / 2.0) }
    }

    /// Subgrid item `sg`'s pinned style while its items size the shared tracks: nothing there.
    pub(in crate::tree) fn subgrid_hide(&mut self, sg: NodeId, rows: bool) {
        if let Some(s) = self.pinned.get_mut(&sg) {
            let (size, min) = if rows { (&mut s.size.height, &mut s.min_size.height) } else { (&mut s.size.width, &mut s.min_size.width) };
            *size = Dimension::length(0.0);
            *min = LengthPercentageAuto::length(0.0);
        }
        self.nodes[ix(sg)].cache.clear();
    }

    /// Where subgrid `sg`'s in-flow items go in its `span` shared tracks: grid item placement in its
    /// own grid (§8.5). Its tracks in the other axis: its own (at least one) when items fill those
    /// first, else as many as it has items (its auto-placement makes them as it goes).
    fn subgrid_spots(&self, sg: NodeId, rows: bool, span: u16, items: &[NodeId]) -> Vec<(u16, u16)> {
        let s = self.style(sg);
        let by_rows = s.grid_auto_flow.primary_axis() == AbsoluteAxis::Horizontal;
        let own = track_count(if rows { &s.grid_template_columns } else { &s.grid_template_rows }).unwrap_or(0);
        let other = if rows == by_rows { own } else { own.max(items.len() as u16) }.max(1);
        let (nr, nc) = if rows { (span, other) } else { (other, span) };
        let wants: Vec<[(Option<u16>, u16); 2]> = items.iter().map(|&c| {
            let st = self.style(c);
            [axis(&st.grid_row, nr), axis(&st.grid_column, nc)]
        }).collect();
        place(&wants, nr, nc, by_rows, s.grid_auto_flow.is_dense()).into_iter()
            .map(|[r, c]| if rows { r } else { c }).collect()
    }

    /// Echoes of subgrid `sg`'s items into `out`, in the shared tracks it puts them in with its `span`
    /// tracks starting at each of `starts` (0-based); with `every`, in every track they can be in (an
    /// auto-placed subgrid in grid lanes, § "Grid Axis Track Sizing"). A subgrid among them in the same axis has its items echoed instead. When the subgrid's own
    /// tracks in the other axis are all lengths, an item is as long there as the ones it is in.
    pub(in crate::tree) fn subgrid_echoes(&mut self, sg: NodeId, rows: bool, span: u16, starts: std::ops::RangeInclusive<u16>,
                                          every: bool, out: &mut Echoes) {
        let (start, end) = self.subgrid_edges(sg, rows, out.basis);
        let (start, end) = (start + out.outer.0, end + out.outer.1);
        let (_, half) = self.subgrid_gap(sg, rows, out.gap, out.basis);
        let items: Vec<NodeId> = self.nodes[ix(sg)].children.iter().copied().filter(|&c| in_flow(self.style(c))).collect();
        let spots = if every { Vec::new() } else { self.subgrid_spots(sg, rows, span, &items) };
        let s = self.style(sg);
        let own: Option<Vec<f32>> = (if rows { &s.grid_template_columns } else { &s.grid_template_rows }).iter()
            .map(|c| match c {
                GridTemplateComponent::Single(t) => t.max.definite_value(Some(out.basis), resolve_calc),
                GridTemplateComponent::Repeat(_) => None,
            })
            .collect::<Option<Vec<f32>>>().filter(|t| !t.is_empty());
        let own_gap = (if rows { s.gap.width } else { s.gap.height }).resolve_or_zero(Some(out.basis), resolve_calc);
        for (i, &c) in items.iter().enumerate() {
            let (cs, within) = if every {
                let (at, cs) = axis(self.placement(c, rows), span);
                (cs, at.map_or(0..=span - cs, |a| a..=a))
            } else {
                let (q, cs) = spots[i];
                (cs, q..=q)
            };
            let edges = |q: u16| (if q == 0 { start } else { half }, if q + cs == span { end } else { half });
            if self.subgrid_shares(c, rows) && self.subgrid_of(c) {
                let (across, outer) = (out.across.take(), out.outer);
                for p in starts.clone() {
                    for q in within.clone() {
                        out.outer = edges(q);
                        self.subgrid_echoes(c, rows, cs, p + q..=p + q, false, out);
                    }
                }
                (out.across, out.outer) = (across, outer);
                continue;
            }
            let along = own.as_ref().map(|own| {
                let (a, n) = axis(self.placement(c, !rows), own.len() as u16);
                let a = usize::from(a.unwrap_or(0));
                own[a..(a + usize::from(n)).min(own.len())].iter().sum::<f32>() + own_gap * f32::from(n.saturating_sub(1))
            }).or(out.across);
            for p in starts.clone() {
                for q in within.clone() {
                    let e = self.alloc(super::super::StyleIx(0), None, &[]);
                    out.track += 1;
                    self.lanes_pin(e, c, rows, (out.track, out.track + 1), Some((p + q + 1, p + q + cs + 1)), None);
                    if let Some(s) = self.pinned.get_mut(&e) {
                        let size = if rows { &mut s.size.width } else { &mut s.size.height };
                        if let Some(l) = along.filter(|_| *size == Dimension::auto()) {
                            *size = Dimension::length(l);
                            s.box_sizing = BoxSizing::BorderBox;
                        }
                        let m = &mut s.margin;
                        let (a, b) = if rows { (&mut m.top, &mut m.bottom) } else { (&mut m.left, &mut m.right) };
                        let (x, y) = edges(q);
                        grow(a, x);
                        grow(b, y);
                    }
                    self.echoes.insert(e, c);
                    out.made.push(e);
                }
            }
        }
    }

    /// Drops echoes made for a pass.
    pub(in crate::tree) fn drop_echoes(&mut self, echoes: Vec<NodeId>) {
        for e in echoes {
            self.echoes.remove(&e);
            self.pinned.remove(&e);
            self.remove(e);
        }
    }

    /// Lays subgrid `sg` out in the parent's shared tracks `t..t + span` of sizes `sizes` (`gap`
    /// between): its tracks there are theirs, less its edges and gutters, and it is stretched over
    /// them (as an item, when pinned). Returns its border-box size in that axis.
    pub(in crate::tree) fn subgrid_fit(&mut self, sg: NodeId, rows: bool, sizes: &[f32], (t, span): (u16, u16), gap: f32,
                                       basis: f32) -> f32 {
        let (start, end) = self.subgrid_edges(sg, rows, basis);
        let (own_gap, half) = self.subgrid_gap(sg, rows, gap, basis);
        let own = sizes.get(usize::from(t)..usize::from(t + span)).unwrap_or(&[]);
        let last = own.len().saturating_sub(1);
        let tracks: Tracks = own.iter().enumerate().map(|(i, &l)| {
            let inset = if i == 0 { start } else { half } + if i == last { end } else { half };
            GridTemplateComponent::Single(length((l - inset).max(0.0)))
        }).collect();
        let s = self.style(sg);
        let b = Some(basis);
        let margins = if rows {
            s.margin.top.resolve_or_zero(b, resolve_calc) + s.margin.bottom.resolve_or_zero(b, resolve_calc)
        } else {
            s.margin.left.resolve_or_zero(b, resolve_calc) + s.margin.right.resolve_or_zero(b, resolve_calc)
        };
        let width = own.iter().sum::<f32>() + gap * f32::from(span.saturating_sub(1)) - margins;
        let stretch = |s: &mut Style| {
            let (size, min, max) = if rows {
                (&mut s.size.height, &mut s.min_size.height, &mut s.max_size.height)
            } else {
                (&mut s.size.width, &mut s.min_size.width, &mut s.max_size.width)
            };
            (*size, *min, *max) = (Dimension::auto(), LengthPercentageAuto::auto(), LengthPercentageAuto::auto());
            if rows { s.align_self = Some(AlignSelf::STRETCH) } else { s.justify_self = Some(JustifySelf::STRETCH) }
        };
        let mut framed = s.clone();
        stretch(&mut framed);
        let (template, names) = if rows {
            (&mut framed.grid_template_rows, &mut framed.grid_template_row_names)
        } else {
            (&mut framed.grid_template_columns, &mut framed.grid_template_column_names)
        };
        *template = tracks;
        // its lines are named as the parent's are, its own names (`subgrid [a] [b]`) added
        let own = std::mem::take(names);
        let parent = (0..self.nodes.len()).find(|&i| self.nodes[i].children.contains(&sg)).map(|i| NodeId::from(i as u64));
        let inherited: Vec<Vec<String>> = parent.and_then(|p| {
            let ps = self.container_style(p);
            let all = if rows { &ps.grid_template_row_names } else { &ps.grid_template_column_names };
            all.get(usize::from(t)..=usize::from(t + span)).map(<[_]>::to_vec)
        }).unwrap_or_default();
        let lines = usize::from(span) + 1;
        if !inherited.is_empty() || !own.is_empty() {
            *names = (0..lines).map(|k| {
                let mut n = inherited.get(k).cloned().unwrap_or_default();
                n.extend(own.get(k).cloned().unwrap_or_default());
                n
            }).collect();
        }
        *(if rows { &mut framed.gap.height } else { &mut framed.gap.width }) = LengthPercentage::length(own_gap);
        self.framed.retain(|(n, _)| *n != sg);
        self.framed.push((sg, framed));
        if let Some(p) = self.pinned.get_mut(&sg) {
            stretch(p);
        }
        self.nodes[ix(sg)].cache.clear();
        width.max(0.0)
    }
}

impl<C, M> View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    /// Subgrid `sg`'s outer size in the other axis in shared tracks `t..t + span` (see `subgrid_fit`).
    pub(in crate::tree) fn subgrid_extent(&mut self, sg: NodeId, rows: bool, sizes: &[f32], at: (u16, u16), gap: f32,
                                          basis: f32) -> f32 {
        let w = self.t.subgrid_fit(sg, rows, sizes, at, gap, basis);
        let (known, avail) = if rows {
            (Size { width: None, height: Some(w) }, Size { width: AvailableSpace::MaxContent, height: AvailableSpace::Definite(w) })
        } else {
            (Size { width: Some(w), height: None }, Size { width: AvailableSpace::Definite(w), height: AvailableSpace::MaxContent })
        };
        let out = self.compute_child_layout(sg, LayoutInput {
            known_dimensions: known,
            known_dimensions_are_definite: Size { width: true, height: true },
            parent_size: Size::NONE,
            available_space: avail,
            run_mode: RunMode::ComputeSize,
            sizing_mode: SizingMode::InherentSize,
            axis: RequestedAxis::Both,
            vertical_margins_are_collapsible: Line::FALSE,
        });
        self.t.nodes[ix(sg)].cache.clear();
        let s = self.t.style(sg);
        let b = Some(basis);
        if rows {
            out.size.width + s.margin.left.resolve_or_zero(b, resolve_calc) + s.margin.right.resolve_or_zero(b, resolve_calc)
        } else {
            out.size.height + s.margin.top.resolve_or_zero(b, resolve_calc) + s.margin.bottom.resolve_or_zero(b, resolve_calc)
        }
    }
}

//! Sizing keywords on min and max sizes (`min-width: min-content`, `max-height: fit-content`,
//! `min-width: stretch`, CSS Sizing 3 §3.2). taffy takes them on `width` / `height` only, so a
//! container resolves its items' before laying them out: each becomes the length the item measures
//! at (its min-content or max-content size, the fit-content size between them, or the stretch size,
//! the container's content box less the item's margins), in the style the container reads for it.
//! A block size keyword is the item's content height at the container's width.
//!
//! Size containment (`contain: size | inline-size`, CSS Containment 2 §3.2): a box sizes as if it
//! had no content but its `contain-intrinsic-size`, so an automatic size there is that plus its
//! padding and border (within its min and max), given to its layout as known. Each is measured
//! with the item's own sizes auto (the keywords are about its content).

use super::{ix, resolve_calc, LayoutTree, View};
use taffy::prelude::*;
use taffy::tree::{LayoutInput, RequestedAxis, RunMode, SizingMode};
use taffy::{LayoutPartialTree, ResolveOrZero};

/// `contain: inline-size` (and `size`): the width is contained.
pub(crate) const CONTAIN_WIDTH: u8 = 1;
/// `contain: size`: the height too.
pub(crate) const CONTAIN_HEIGHT: u8 = 2;

impl<C> LayoutTree<C> {
    /// Size-contained box `id`'s automatic sizes, as known to its layout (see the module docs).
    pub(super) fn contained(&self, id: NodeId, inputs: &mut LayoutInput) {
        let (bits, content) = self.nodes[ix(id)].contain;
        let s = self.style(id);
        let parent = inputs.parent_size;
        let (p, w) = (s.padding.resolve_or_zero(parent.width, resolve_calc), s.border.resolve_or_zero(parent.width, resolve_calc));
        let inset = Size { width: p.left + p.right + w.left + w.right, height: p.top + p.bottom + w.top + w.bottom };
        // min and max sizes as border-box sizes
        let limit = |l: LengthPercentageAuto, basis: Option<f32>, inset: f32| {
            let v = taffy::MaybeResolve::maybe_resolve(l, basis, resolve_calc);
            if s.box_sizing == BoxSizing::ContentBox { v.map(|v| v + inset) } else { v }
        };
        let axis = |known: &mut Option<f32>, size: Dimension, min: LengthPercentageAuto, max: LengthPercentageAuto,
                    basis: Option<f32>, content: f32, inset: f32| {
            if known.is_none() && size == Dimension::auto() {
                let v = content + inset;
                let v = limit(max, basis, inset).map_or(v, |m| v.min(m));
                *known = Some(limit(min, basis, inset).map_or(v, |m| v.max(m)));
            }
        };
        let k = &mut inputs.known_dimensions;
        if bits & CONTAIN_WIDTH != 0 {
            axis(&mut k.width, s.size.width, s.min_size.width, s.max_size.width, parent.width, content[0], inset.width);
        }
        if bits & CONTAIN_HEIGHT != 0 {
            axis(&mut k.height, s.size.height, s.min_size.height, s.max_size.height, parent.height, content[1], inset.height);
        }
    }
}

/// `min-content` (in `Computed::size_kw`, 4 bits per min / max size).
pub(crate) const SIZING_MIN_CONTENT: u16 = 1;
/// `max-content`.
pub(crate) const SIZING_MAX_CONTENT: u16 = 2;
/// `fit-content`.
pub(crate) const SIZING_FIT_CONTENT: u16 = 3;
/// `stretch`.
pub(crate) const SIZING_STRETCH: u16 = 4;

impl<C, M> View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    /// Resolves the sizing keywords on the min and max sizes of container `id`'s items, for the
    /// layout with `inputs` about to run; returns the items (whose resolved styles the caller drops
    /// after it).
    pub(super) fn resolve_sizing(&mut self, id: NodeId, inputs: LayoutInput) -> Vec<NodeId> {
        // (not a subgrid's: measured alone, it hasn't its parent's tracks)
        let items: Vec<NodeId> = self.t.laid_out(id).iter().copied()
            .filter(|&k| self.t.nodes[ix(k)].sizing != 0 && self.t.nodes[ix(k)].subgrid == 0).collect();
        if items.is_empty() {
            return items;
        }
        // the container's content box, where known
        let s = self.t.container_style(id);
        let basis = inputs.parent_size.width;
        let (pad, border) = (s.padding.resolve_or_zero(basis, resolve_calc), s.border.resolve_or_zero(basis, resolve_calc));
        let definite = |known: Option<f32>, avail: AvailableSpace| known.or(match avail {
            AvailableSpace::Definite(v) => Some(v),
            _ => None,
        });
        let content = Size {
            width: definite(inputs.known_dimensions.width, inputs.available_space.width)
                .map(|w| (w - pad.left - pad.right - border.left - border.right).max(0.0)),
            height: inputs.known_dimensions.height.map(|h| (h - pad.top - pad.bottom - border.top - border.bottom).max(0.0)),
        };
        for &k in &items {
            let code = self.t.nodes[ix(k)].sizing;
            let mut style = self.t.style(k).clone();
            let at = |width: AvailableSpace| LayoutInput {
                known_dimensions: Size::NONE,
                known_dimensions_are_definite: Size { width: true, height: true },
                parent_size: content,
                available_space: Size { width, height: AvailableSpace::MaxContent },
                run_mode: RunMode::ComputeSize,
                sizing_mode: SizingMode::InherentSize,
                axis: RequestedAxis::Both,
                vertical_margins_are_collapsible: Line::FALSE,
            };
            let b = Some(content.width.unwrap_or(0.0));
            let m = style.margin;
            let margins = Size {
                width: m.left.resolve_or_zero(b, resolve_calc) + m.right.resolve_or_zero(b, resolve_calc),
                height: m.top.resolve_or_zero(b, resolve_calc) + m.bottom.resolve_or_zero(b, resolve_calc),
            };
            // what `content-box` sizing leaves out of a border-box size
            let (p, w) = (style.padding.resolve_or_zero(b, resolve_calc), style.border.resolve_or_zero(b, resolve_calc));
            let inset = if style.box_sizing == BoxSizing::ContentBox {
                Size { width: p.left + p.right + w.left + w.right, height: p.top + p.bottom + w.top + w.bottom }
            } else {
                Size::ZERO
            };
            // its content's size in an axis: measured with its own size there auto (and no min or
            // max), its others as they are (an aspect ratio still transfers them)
            let own = style.clone();
            let mut measured = [None; 6];
            let mut measure = |horizontal: bool, width: AvailableSpace| {
                let slot = usize::from(!horizontal) * 3 + match width {
                    AvailableSpace::MinContent => 0,
                    AvailableSpace::MaxContent => 1,
                    AvailableSpace::Definite(_) => 2,
                };
                *measured[slot].get_or_insert_with(|| {
                    let mut bare = own.clone();
                    if horizontal {
                        (bare.size.width, bare.min_size.width, bare.max_size.width) = (Dimension::auto(), LengthPercentageAuto::auto(), LengthPercentageAuto::auto());
                    } else {
                        (bare.size.height, bare.min_size.height, bare.max_size.height) = (Dimension::auto(), LengthPercentageAuto::auto(), LengthPercentageAuto::auto());
                    }
                    self.t.framed.retain(|(n, _)| *n != k);
                    self.t.framed.push((k, bare));
                    self.t.nodes[ix(k)].cache.clear();
                    self.compute_child_layout(k, at(width)).size
                })
            };
            for slot in 0..4u16 {
                let kw = (code >> (slot * 4)) & 0xf;
                if kw == 0 {
                    continue;
                }
                let horizontal = slot % 2 == 0;
                let stretch = if horizontal { content.width.map(|c| c - margins.width) } else { content.height.map(|c| c - margins.height) };
                let size = if horizontal {
                    match kw {
                        SIZING_MIN_CONTENT => Some(measure(true, AvailableSpace::MinContent).width),
                        SIZING_MAX_CONTENT => Some(measure(true, AvailableSpace::MaxContent).width),
                        SIZING_FIT_CONTENT => {
                            let (min, max) = (measure(true, AvailableSpace::MinContent).width, measure(true, AvailableSpace::MaxContent).width);
                            // `fit-content(L)`: L (kept as the value) is the limit, else the stretch size
                            let given = if slot == 0 { own.min_size.width } else { own.max_size.width };
                            let limit = taffy::MaybeResolve::maybe_resolve(given, content.width, resolve_calc).map(|l| l + inset.width).or(stretch);
                            Some(limit.map_or(max, |s| s.max(min).min(max)))
                        }
                        _ => stretch,
                    }
                } else if kw == SIZING_STRETCH {
                    stretch
                } else {
                    // a block size's min-, max- and fit-content are its content height, at the
                    // width it would get
                    let width = content.width.map_or(AvailableSpace::MaxContent, |c| AvailableSpace::Definite(c - margins.width));
                    Some(measure(false, width).height)
                };
                let Some(size) = size else { continue };
                let v = LengthPercentageAuto::length((size - if horizontal { inset.width } else { inset.height }).max(0.0));
                match slot {
                    0 => style.min_size.width = v,
                    1 => style.min_size.height = v,
                    2 => style.max_size.width = v,
                    _ => style.max_size.height = v,
                }
            }
            self.t.framed.retain(|(n, _)| *n != k);
            self.t.nodes[ix(k)].cache.clear();
            self.t.sized.insert(k, style);
        }
        items
    }
}

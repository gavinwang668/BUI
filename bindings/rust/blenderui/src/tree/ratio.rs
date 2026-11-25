//! `aspect-ratio` where taffy gets it wrong:
//!
//! - the automatic minimum size of a box with a ratio (CSS Sizing 4 §5.2): a block that is neither
//!   replaced nor a scroll container is at least as tall as its content in the axis the ratio sizes,
//!   so `aspect-ratio: 2/1` on a block with tall content grows it instead of overflowing. taffy sizes
//!   it from the ratio alone, so it is laid out again without the ratio at the width the ratio gave
//!   it; when its content is taller, it lays out at that height (both heights respect `max-height`).
//!   Only a ratio that sizes the height is handled (a definite width);
//! - a leaf's ratio with `box-sizing: content-box`: taffy's leaf layout last makes the height at
//!   least the *border-box* width over the ratio (the ratio is of the content box), even over the
//!   height its parent gave it.

use super::{ix, resolve_calc, Baseline, View};
use taffy::ResolveOrZero;
use taffy::compute::{compute_block_layout, BlockContext};
use taffy::prelude::*;
use taffy::style::Overflow;
use taffy::tree::{Baselines, LayoutInput, LayoutOutput, RunMode, SizingMode};

impl<C, M> View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    /// A leaf's layout (measured by its context, if any), with its baselines when it is text.
    pub(super) fn leaf_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        let mut out = self.ratio_leaf(id, inputs);
        if let Some(Baseline::Text([above, below])) = self.t.nodes[ix(id)].baseline {
            let s = self.t.style(id);
            let basis = inputs.parent_size.width;
            let (pad, border) = (s.padding.resolve_or_zero(basis, resolve_calc), s.border.resolve_or_zero(basis, resolve_calc));
            out.baselines = Baselines {
                first: Some(pad.top + border.top + above),
                last: Some(out.size.height - pad.bottom - border.bottom - below),
            };
        }
        out
    }

    /// A leaf's layout, with its ratio fixed (see the module docs).
    fn ratio_leaf(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        let out = self.leaf_pass(id, inputs);
        // a height given (a parent that applied the ratio itself) stands: laid out again without
        // the ratio, which taffy would make the border box's again
        let s = self.t.style(id);
        let Some(height) = inputs.known_dimensions.height.filter(|&h| s.aspect_ratio.is_some()
            && s.box_sizing == BoxSizing::ContentBox && out.size.height > h + 0.01) else { return out };
        let mut free = s.clone();
        free.aspect_ratio = None;
        self.t.framed.push((id, free));
        let out = self.leaf_pass(id, LayoutInput { known_dimensions: Size { width: Some(out.size.width), height: Some(height) }, ..inputs });
        self.t.framed.pop();
        out
    }

    fn leaf_pass(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        let n = &self.t.nodes[ix(id)];
        let measure = &mut self.measure;
        match &n.ctx {
            Some(c) => taffy::compute_leaf_layout(inputs, self.t.container_style(id), resolve_calc, |known, avail| measure(known, avail, c)),
            None => taffy::compute_leaf_layout(inputs, self.t.container_style(id), resolve_calc, |_, _| Size::ZERO),
        }
    }

    /// A block container's layout, at least as tall as its content when it has an aspect ratio.
    pub(super) fn block_layout(&mut self, id: NodeId, inputs: LayoutInput, mut block: Option<&mut BlockContext<'_>>)
        -> LayoutOutput {
        let s = self.t.style(id);
        // (a height given is the ratio's too: a block parent applies the ratio itself)
        let floored = s.aspect_ratio.is_some() && s.size.height.is_auto() && s.min_size.height.is_auto()
            && s.overflow.y == Overflow::Visible && inputs.sizing_mode == SizingMode::InherentSize;
        let out = compute_block_layout(self, id, inputs, block.as_deref_mut());
        if !floored {
            return out;
        }
        // the content's height at the width the ratio gave
        let mut free = self.t.style(id).clone();
        free.aspect_ratio = None;
        self.t.framed.push((id, free));
        let width = Size { width: Some(out.size.width), height: None };
        let content = compute_block_layout(self, id, LayoutInput {
            known_dimensions: width, run_mode: RunMode::ComputeSize, ..inputs
        }, block.as_deref_mut());
        self.t.framed.pop();
        if content.size.height <= out.size.height + 0.01 {
            return out;
        }
        compute_block_layout(self, id, LayoutInput {
            known_dimensions: Size { width: Some(out.size.width), height: Some(content.size.height) }, ..inputs
        }, block)
    }
}

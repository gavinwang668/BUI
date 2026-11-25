//! The layout tree: taffy's algorithms (block with floats, flexbox, grid,
//! out-of-flow positioning) over our own node storage.
//!
//! taffy's `TaffyTree` resolves every `calc()` to 0; this tree resolves
//! them (`crate::calc`), so `width: calc(100% - 2rem)` and friends work. It
//! is also leaner: nodes live in a `Vec` with a free list (no slot maps), and
//! a node's cache is only cleared when its own inputs change: the
//! reconciler in `layout.rs` marks changed subtrees itself.

use taffy::compute::{
    compute_cached_layout, compute_flexbox_layout,
    compute_hidden_layout, compute_oof_layout, compute_root_layout, BlockContext,
};
use taffy::prelude::*;
use taffy::ResolveOrZero;
use taffy::tree::{Cache, DetailedLayoutInfo, Layout, LayoutInput, LayoutOutput, RunMode};
use taffy::{
    CacheTree, DetailedGridInfo, LayoutBlockContainer, LayoutContainingBlock, LayoutFlexboxContainer,
    LayoutGridContainer, LayoutPartialTree, TraversePartialTree, TraverseTree,
};

mod inline;
mod lanes;
mod ratio;
mod sizing;
mod subgrid;
pub(crate) use inline::{Inline, Item, ALIGN_CENTER, ALIGN_JUSTIFY, ALIGN_LEFT, ALIGN_RIGHT, BREAK_ALLOWED,
                        BREAK_EMERGENCY, BREAK_NEVER, ITEM_ATOMIC, ITEM_BREAK, ITEM_OUT, ITEM_TEXT, VALIGN_BASELINE,
                        VALIGN_BOTTOM, VALIGN_MIDDLE, VALIGN_TOP};
pub(crate) use lanes::{Lanes, DENSE, FILL_REVERSE, FLOW_ALIGN_CONTENT, FLOW_ALIGN_ITEMS, FLOW_JUSTIFY_CONTENT,
                       FLOW_JUSTIFY_ITEMS, LANES_COLUMN, LANES_ROW, TOLERANCE_INFINITE, TRACK_REVERSE};
pub(crate) use sizing::{CONTAIN_HEIGHT, CONTAIN_WIDTH, SIZING_FIT_CONTENT, SIZING_MAX_CONTENT, SIZING_MIN_CONTENT,
                        SIZING_STRETCH};
pub(crate) use subgrid::{SUBGRID_COLUMNS, SUBGRID_ROWS};

struct Node<C> {
    style: StyleIx,
    ctx: Option<C>,
    children: Vec<NodeId>,
    hoisted: Vec<NodeId>,
    cache: Cache,
    unrounded: Layout,
    detail: DetailedLayoutInfo,
    live: bool,
    /// The axes in which this grid item is a subgrid (`SUBGRID_COLUMNS`, `SUBGRID_ROWS`).
    subgrid: u8,
    /// A grid that is a grid-lanes container, with its settings.
    lanes: Option<Lanes>,
    /// Where this box's baselines are when not where its layout puts them (`set_baseline`).
    baseline: Option<Baseline>,
    /// Sizing keywords on its min and max sizes (`sizing.rs`).
    sizing: u16,
    /// Size containment and the content size it has then (`Computed::contain`, `sizing.rs`).
    contain: (u8, [f32; 2]),
    /// A block container of inline content: its children are its inline items (`inline.rs`).
    inline: Option<Box<Inline>>,
}

/// A box's baselines, set by the builder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Baseline {
    /// A text leaf's: its first line's below the content box's top, its last line's above the
    /// content box's bottom.
    Text([f32; 2]),
    /// The bottom margin edge (an inline-block scroll container's, CSS 2 §10.8.1).
    MarginEdge,
}

/// A style in a tree's table: nodes with the same layout style point at one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct StyleIx(u32);

/// Nodes with an optional measure context `C` per leaf.
pub(crate) struct LayoutTree<C> {
    nodes: Vec<Node<C>>,
    free: Vec<usize>,
    /// The styles of the nodes; entry 0 is the default, which a freed node points at.
    styles: Vec<Style>,
    /// Grids laying out their subgrids' items as their own, with the items they lay out then.
    lifted: Vec<(NodeId, Vec<NodeId>)>,
    /// The grid-item styles in force while they do: each item pinned to its grid area.
    pinned: std::collections::HashMap<NodeId, Style>,
    /// Containers laying out with a style other than their own (`container_style`).
    framed: Vec<(NodeId, Style)>,
    /// Stand-ins a grid-lanes container measures its items with in more lanes than one: each
    /// sizes as the item it echoes (`lanes.rs`).
    echoes: std::collections::HashMap<NodeId, NodeId>,
    /// How many nodes have sizing keywords on their min or max sizes (none: nothing to resolve).
    keyworded: usize,
    /// Item styles with those keywords resolved, while their container lays them out.
    sized: std::collections::HashMap<NodeId, Style>,
}

impl<C> Default for LayoutTree<C> {
    fn default() -> Self {
        LayoutTree { nodes: Vec::new(), free: Vec::new(), styles: vec![Style::default()], lifted: Vec::new(),
                     pinned: std::collections::HashMap::new(), framed: Vec::new(),
                     echoes: std::collections::HashMap::new(), keyworded: 0, sized: std::collections::HashMap::new() }
    }
}

#[inline]
fn ix(id: NodeId) -> usize {
    u64::from(id) as usize
}

impl<C> LayoutTree<C> {
    fn alloc(&mut self, style: StyleIx, ctx: Option<C>, children: &[NodeId]) -> NodeId {
        let node = Node {
            style,
            ctx,
            children: children.to_vec(),
            hoisted: Vec::new(),
            cache: Cache::new(),
            unrounded: Layout::new(),
            detail: DetailedLayoutInfo::None,
            live: true,
            subgrid: 0,
            lanes: None,
            baseline: None,
            sizing: 0,
            contain: (0, [0.0; 2]),
            inline: None,
        };
        match self.free.pop() {
            Some(i) => {
                self.nodes[i] = node;
                NodeId::from(i)
            }
            None => {
                self.nodes.push(node);
                NodeId::from(self.nodes.len() - 1)
            }
        }
    }

    pub fn new_leaf_with_context(&mut self, style: StyleIx, ctx: C) -> NodeId {
        self.alloc(style, Some(ctx), &[])
    }

    pub fn new_with_children(&mut self, style: StyleIx, children: &[NodeId]) -> NodeId {
        self.alloc(style, None, children)
    }

    /// Frees a node (its children are not touched: the reconciler removes
    /// every unused node itself).
    pub fn remove(&mut self, id: NodeId) {
        let i = ix(id);
        if let Some(n) = self.nodes.get_mut(i) {
            if n.live {
                if n.sizing != 0 {
                    self.keyworded -= 1;
                }
                n.sizing = 0;
                n.inline = None;
                n.live = false;
                n.style = StyleIx(0);
                n.ctx = None;
                n.children = Vec::new();
                n.hoisted = Vec::new();
                n.detail = DetailedLayoutInfo::None;
                self.free.push(i);
            }
        }
    }

    /// The style node `id` lays out its children with: a layout pass may stand in another
    /// (`framed`: a grid-lanes container as a grid, a block without its aspect ratio).
    fn container_style(&self, id: NodeId) -> &Style {
        if !self.framed.is_empty() && let Some((_, s)) = self.framed.iter().rev().find(|(g, _)| *g == id) {
            return s;
        }
        self.style(id)
    }

    pub fn style(&self, id: NodeId) -> &Style {
        &self.styles[self.nodes[ix(id)].style.0 as usize]
    }

    pub fn style_ix(&self, id: NodeId) -> StyleIx {
        self.nodes[ix(id)].style
    }

    /// Whether the node's style equals the table entry `style`.
    pub fn has_style(&self, id: NodeId, style: StyleIx) -> bool {
        *self.style(id) == self.styles[style.0 as usize]
    }

    /// Adds a style to the table.
    pub fn intern(&mut self, style: Style) -> StyleIx {
        self.styles.push(style);
        StyleIx((self.styles.len() - 1) as u32)
    }

    pub fn style_count(&self) -> usize {
        self.styles.len()
    }

    pub fn set_style(&mut self, id: NodeId, style: StyleIx) {
        let n = &mut self.nodes[ix(id)];
        n.style = style;
        n.cache.clear();
    }

    /// Points a node at another copy of the style it has: its cache stays.
    pub fn replace_style(&mut self, id: NodeId, style: StyleIx) {
        self.nodes[ix(id)].style = style;
    }

    /// Drops the styles no node points at. Every `StyleIx` held outside the tree is stale after it.
    pub fn compact_styles(&mut self) {
        let mut kept = vec![Style::default()];
        let mut moved = vec![u32::MAX; self.styles.len()];
        moved[0] = 0;
        for n in self.nodes.iter_mut().filter(|n| n.live) {
            let slot = &mut moved[n.style.0 as usize];
            if *slot == u32::MAX {
                *slot = kept.len() as u32;
                kept.push(std::mem::take(&mut self.styles[n.style.0 as usize]));
            }
            n.style = StyleIx(*slot);
        }
        self.styles = kept;
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[ix(id)].children
    }

    pub fn set_children(&mut self, id: NodeId, children: &[NodeId]) {
        let n = &mut self.nodes[ix(id)];
        n.children.clear();
        n.children.extend_from_slice(children);
        n.cache.clear();
    }

    pub fn context(&self, id: NodeId) -> Option<&C> {
        self.nodes[ix(id)].ctx.as_ref()
    }

    pub fn set_context(&mut self, id: NodeId, ctx: Option<C>) {
        let n = &mut self.nodes[ix(id)];
        n.ctx = ctx;
        n.cache.clear();
    }

    /// Makes the node a subgrid in `axes` (a grid item that is a grid itself: elsewhere it is ignored).
    /// Returns whether that changed.
    pub fn set_subgrid(&mut self, id: NodeId, axes: u8) -> bool {
        let n = &mut self.nodes[ix(id)];
        let changed = n.subgrid != axes;
        if changed {
            n.subgrid = axes;
            n.cache.clear();
        }
        changed
    }

    /// Makes the node a grid-lanes container (a grid: elsewhere it is ignored), or not. Returns whether
    /// that changed.
    pub fn set_lanes(&mut self, id: NodeId, lanes: Option<Lanes>) -> bool {
        let n = &mut self.nodes[ix(id)];
        let changed = n.lanes != lanes;
        if changed {
            n.lanes = lanes;
            n.cache.clear();
        }
        changed
    }

    /// Gives a box its baselines (`Node::baseline`), which flex, grid and line boxes align by.
    /// Returns whether that changed.
    pub fn set_baseline(&mut self, id: NodeId, baseline: Option<Baseline>) -> bool {
        let n = &mut self.nodes[ix(id)];
        let changed = n.baseline != baseline;
        if changed {
            n.baseline = baseline;
            n.cache.clear();
        }
        changed
    }

    /// Gives a box the sizing keywords on its min and max sizes (`Computed::size_kw`). Returns
    /// whether that changed.
    pub fn set_sizing(&mut self, id: NodeId, sizing: u16) -> bool {
        let n = &mut self.nodes[ix(id)];
        if n.sizing == sizing {
            return false;
        }
        match (n.sizing, sizing) {
            (0, _) => self.keyworded += 1,
            (_, 0) => self.keyworded -= 1,
            _ => {}
        }
        n.sizing = sizing;
        n.cache.clear();
        true
    }

    /// Makes a block container lay its children out in lines (`inline.rs`), or not. Returns whether
    /// that changed.
    pub fn set_inline(&mut self, id: NodeId, inline: Option<Inline>) -> bool {
        let n = &mut self.nodes[ix(id)];
        if n.inline.as_deref() == inline.as_ref() {
            return false;
        }
        n.inline = inline.map(Box::new);
        n.cache.clear();
        true
    }

    /// Gives a box size containment (`Computed::contain`). Returns whether that changed.
    pub fn set_contain(&mut self, id: NodeId, contain: (u8, [f32; 2])) -> bool {
        let n = &mut self.nodes[ix(id)];
        let changed = n.contain != contain;
        if changed {
            n.contain = contain;
            n.cache.clear();
        }
        changed
    }

    /// The style a container lays item `id` out with: with its sizing keywords resolved, or pinned
    /// to its grid area, while either is so.
    #[inline(always)]
    /// Whether `id`'s subtree holds text (lines or text leaves): what gives a box its baselines.
    pub(super) fn has_text(&self, id: NodeId) -> bool {
        let n = &self.nodes[ix(id)];
        // (lines of collapsed white space have none)
        let lines = n.inline.as_ref().is_some_and(|l| l.items.iter().any(|it| it.kind == inline::ITEM_TEXT && it.width > 0.0));
        lines || matches!(n.baseline, Some(Baseline::Text(_))) || n.children.iter().any(|&c| self.has_text(c))
    }

    pub(super) fn item_style(&self, id: NodeId) -> &Style {
        if !self.sized.is_empty() && let Some(s) = self.sized.get(&id) {
            return s;
        }
        if !self.pinned.is_empty() && let Some(s) = self.pinned.get(&id) {
            return s;
        }
        self.style(id)
    }

    pub fn mark_dirty(&mut self, id: NodeId) {
        self.nodes[ix(id)].cache.clear();
    }

    /// The layout of a node, in fractional pixels as browsers lay out (paint snaps box edges to
    /// device pixels: `Item::snap` in `paint.rs`).
    pub fn layout(&self, id: NodeId) -> &Layout {
        &self.nodes[ix(id)].unrounded
    }

    /// Lay out the tree under `root`; `measure` sizes leaves that carry a context.
    pub fn compute<M>(&mut self, root: NodeId, available: Size<AvailableSpace>, measure: M)
    where
        M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
    {
        let mut view = View { t: self, measure };
        compute_root_layout(&mut view, root, available);
    }
}

/// The tree plus the measure function for one layout pass.
struct View<'t, C, M> {
    t: &'t mut LayoutTree<C>,
    measure: M,
}

impl<C, M> View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    fn child_layout(&mut self, id: NodeId, inputs: LayoutInput, block: Option<&mut BlockContext<'_>>)
        -> LayoutOutput {
        if inputs.run_mode == RunMode::PerformHiddenLayout {
            self.t.nodes[ix(id)].unrounded = Layout::with_order(0);
            return compute_hidden_layout(self, id);
        }
        if !self.t.echoes.is_empty() && let Some(&item) = self.t.echoes.get(&id) {
            return self.child_layout(item, LayoutInput { run_mode: RunMode::ComputeSize, ..inputs }, None);
        }
        compute_cached_layout(self, id, inputs, |tree, id, mut inputs| {
            if tree.t.nodes[ix(id)].contain.0 != 0 {
                tree.t.contained(id, &mut inputs);
            }
            let n = &tree.t.nodes[ix(id)];
            let sized = if tree.t.keyworded > 0 && !n.children.is_empty() { tree.resolve_sizing(id, inputs) } else { Vec::new() };
            let n = &tree.t.nodes[ix(id)];
            let mut out = match (tree.t.style(id).display, !n.children.is_empty()) {
                (Display::None, _) => compute_hidden_layout(tree, id),
                (_, true) if n.inline.is_some() => tree.inline_layout(id, inputs, block),
                (Display::Block, true) => tree.block_layout(id, inputs, block),
                (Display::FlowRoot, true) => tree.block_layout(id, inputs, None),
                (Display::Flex, true) => compute_flexbox_layout(tree, id, inputs),
                // an empty grid still has its explicit tracks
                (Display::Grid, true) => tree.grid_layout(id, inputs),
                (Display::Grid, false) if n.ctx.is_none() => tree.grid_layout(id, inputs),
                (_, false) => tree.leaf_layout(id, inputs),
            };
            if inputs.run_mode == RunMode::PerformLayout {
                compute_oof_layout(tree, id, &mut out);
            }
            for k in sized {
                tree.t.sized.remove(&k);
            }
            if tree.t.nodes[ix(id)].baseline == Some(Baseline::MarginEdge) {
                let s = tree.t.style(id);
                let margin = s.margin.bottom.resolve_or_zero(inputs.parent_size.width, resolve_calc);
                let edge = Some(out.size.height + margin);
                out.baselines = taffy::tree::Baselines { first: edge, last: edge };
            }
            out
        })
    }
}

/// taffy's calc callback: the pointer is a `&'static crate::calc::Resolved`.
pub(crate) fn resolve_calc(ptr: *const (), basis: f32) -> f32 {
    // SAFETY: every calc pointer in a style comes from `crate::calc::intern`,
    // which leaks its values: they live for the rest of the process.
    unsafe { crate::calc::Resolved::from_ptr(ptr) }.eval(basis)
}

impl<C, M> TraversePartialTree for View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    type ChildIter<'a> = std::iter::Copied<std::slice::Iter<'a, NodeId>> where Self: 'a;

    #[inline(always)]
    fn child_ids(&self, id: NodeId) -> Self::ChildIter<'_> {
        self.t.laid_out(id).iter().copied()
    }

    #[inline(always)]
    fn child_count(&self, id: NodeId) -> usize {
        self.t.laid_out(id).len()
    }

    #[inline(always)]
    fn get_child_id(&self, id: NodeId, k: usize) -> NodeId {
        self.t.laid_out(id)[k]
    }
}

impl<C, M> TraverseTree for View<'_, C, M> where M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32> {}

impl<C, M> LayoutPartialTree for View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    type CoreContainerStyle<'a> = &'a Style where Self: 'a;
    type CustomIdent = String;

    #[inline(always)]
    fn get_core_container_style(&self, id: NodeId) -> Self::CoreContainerStyle<'_> {
        self.t.container_style(id)
    }

    #[inline(always)]
    fn set_unrounded_layout(&mut self, id: NodeId, layout: &Layout) {
        self.t.nodes[ix(id)].unrounded = *layout;
    }

    #[inline(always)]
    fn resolve_calc_value(&self, ptr: *const (), basis: f32) -> f32 {
        resolve_calc(ptr, basis)
    }

    #[inline(always)]
    fn compute_child_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        self.child_layout(id, inputs, None)
    }
}

impl<C, M> LayoutContainingBlock for View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    type OofItemStyle<'a> = &'a Style where Self: 'a;

    #[inline(always)]
    fn get_oof_item_style(&self, id: NodeId) -> Self::OofItemStyle<'_> {
        self.t.item_style(id)
    }

    #[inline(always)]
    fn clear_hoisted_children(&mut self, id: NodeId) {
        self.t.nodes[ix(id)].hoisted.clear();
    }

    #[inline(always)]
    fn add_hoisted_children(&mut self, id: NodeId, hoisted: &[NodeId]) {
        self.t.nodes[ix(id)].hoisted.extend_from_slice(hoisted);
    }

    #[inline(always)]
    fn get_detailed_layout_info(&self, id: NodeId) -> &DetailedLayoutInfo {
        &self.t.nodes[ix(id)].detail
    }
}

impl<C, M> CacheTree for View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    #[inline(always)]
    fn cache_get(&mut self, id: NodeId, input: &LayoutInput) -> Option<LayoutOutput> {
        self.t.nodes[ix(id)].cache.get(input)
    }

    #[inline(always)]
    fn cache_store(&mut self, id: NodeId, input: &LayoutInput, out: LayoutOutput) {
        self.t.nodes[ix(id)].cache.store(input, out)
    }

    #[inline(always)]
    fn cache_clear(&mut self, id: NodeId) {
        self.t.nodes[ix(id)].cache.clear();
    }
}

impl<C, M> LayoutBlockContainer for View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    type BlockContainerStyle<'a> = &'a Style where Self: 'a;
    type BlockItemStyle<'a> = &'a Style where Self: 'a;

    #[inline(always)]
    fn get_block_container_style(&self, id: NodeId) -> Self::BlockContainerStyle<'_> {
        self.t.container_style(id)
    }

    #[inline(always)]
    fn get_block_child_style(&self, id: NodeId) -> Self::BlockItemStyle<'_> {
        self.t.item_style(id)
    }

    #[inline(always)]
    fn compute_block_child_layout(&mut self, id: NodeId, inputs: LayoutInput,
                                  block: Option<&mut BlockContext<'_>>) -> LayoutOutput {
        self.child_layout(id, inputs, block)
    }
}

impl<C, M> LayoutFlexboxContainer for View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    type FlexboxContainerStyle<'a> = &'a Style where Self: 'a;
    type FlexboxItemStyle<'a> = &'a Style where Self: 'a;

    #[inline(always)]
    fn get_flexbox_container_style(&self, id: NodeId) -> Self::FlexboxContainerStyle<'_> {
        self.t.style(id)
    }

    #[inline(always)]
    fn get_flexbox_child_style(&self, id: NodeId) -> Self::FlexboxItemStyle<'_> {
        self.t.item_style(id)
    }
}

impl<C, M> LayoutGridContainer for View<'_, C, M>
where
    M: FnMut(Size<Option<f32>>, Size<AvailableSpace>, &C) -> Size<f32>,
{
    type GridContainerStyle<'a> = &'a Style where Self: 'a;
    type GridItemStyle<'a> = &'a Style where Self: 'a;

    #[inline(always)]
    fn get_grid_container_style(&self, id: NodeId) -> Self::GridContainerStyle<'_> {
        self.t.container_style(id)
    }

    #[inline(always)]
    fn get_grid_child_style(&self, id: NodeId) -> Self::GridItemStyle<'_> {
        self.t.item_style(id)
    }

    #[inline(always)]
    fn set_detailed_grid_info(&mut self, id: NodeId, info: DetailedGridInfo) {
        self.t.nodes[ix(id)].detail = DetailedLayoutInfo::Grid(Box::new(info));
    }
}

#[cfg(test)]
mod tests {
    use super::{LayoutTree, Node};
    use taffy::prelude::Style;

    /// A node is made for every box that lays out: its style is shared, not stored.
    #[test]
    fn node_stays_small() {
        assert!(std::mem::size_of::<Node<[u8; 32]>>() <= 704, "{}", std::mem::size_of::<Node<[u8; 32]>>());
    }

    /// Compaction keeps what live nodes use and drops the rest.
    #[test]
    fn compaction_keeps_the_styles_nodes_use() {
        let mut tree: LayoutTree<()> = LayoutTree::default();
        let (a, b) = (tree.intern(Style { flex_grow: 2.0, ..Default::default() }),
                      tree.intern(Style { flex_grow: 3.0, ..Default::default() }));
        let (gone, kept) = (tree.new_with_children(a, &[]), tree.new_with_children(b, &[]));
        tree.remove(gone);
        tree.compact_styles();
        assert_eq!(tree.style_count(), 2);
        assert_eq!(tree.style(kept).flex_grow, 3.0);
    }
}

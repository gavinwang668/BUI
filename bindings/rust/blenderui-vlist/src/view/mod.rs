//! [`ListView`]: the scroll state and everything a frame needs, around one
//! [`HeightIndex`].
//!
//! **Position is an anchor**, not a pixel offset: `(item, offset inside the
//! item)`. Items are painted relative to it in small local `f32`
//! coordinates, so there is no precision problem at 10M items, and a height
//! that changes above the viewport (an image arrived, a block was
//! measured, history was prepended) cannot move what the user sees: the
//! anchor item is where it was. The alternative position, [`Pos::Bottom`],
//! is "the end of the list at the bottom edge" (chat autoscroll,
//! open-at-newest): it has no anchor at all, and is recomputed each frame
//! from the end, so opening a 10M-message chat at the newest message
//! touches only the last screenful.
//!
//! Blender's text view (`textview_draw`) does the second thing only: it
//! starts from the newest line and walks backwards, wrapping each line.
//!
//! * [`frame`]: one frame (resolve the anchor, place items, measure what is
//!   unmeasured within the budget, clamp at the end, prefetch).
//! * [`edit`]: what the owner tells the view when the items change.

mod edit;
mod frame;
mod relative;

use std::time::Duration;

use crate::fx::{px_to_units, units_to_px};
use crate::index::{HeightIndex, IndexError};
use crate::measure::{Measurer, TextLayout};
use crate::source::{ItemSource, Page};

pub use frame::{Budget, Placed};

/// The measurer's estimator for a list `width` wide, with `opts.estimate_size` applied.
pub(crate) fn estimator_for<M: Measurer>(m: &M, opts: &Options, width: f32) -> crate::Estimator {
    let est = m.estimator(width.max(1.0));
    opts.estimate_size.map_or(est, |px| est.with_item_size(f64::from(px)))
}

/// Where the viewport is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pos {
    /// The top of the viewport is `off` fixed-point units below the top of `item`.
    Anchor { item: usize, off: u64 },
    /// The end of the list is at the bottom of the viewport; stays there as items arrive.
    Bottom,
}

/// Where a jumped-to item ends up.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

/// Keyboard navigation (an adapter maps its keys to these).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Nav {
    LineUp,
    LineDown,
    PageUp,
    PageDown,
    Home,
    End,
}

/// Behaviour switches.
#[derive(Copy, Clone, Debug)]
pub struct Options {
    /// Follow new items while at the bottom; stop when the user scrolls up;
    /// resume when they reach the bottom again.
    pub stick_to_bottom: bool,
    /// Open at the newest item.
    pub start_at_end: bool,
    /// Wrapped texts the layout cache keeps.
    pub cache_capacity: usize,
    /// Measure budget while the user is flinging (what is not measured is
    /// drawn as a skeleton at its estimated height).
    pub fling_budget: Budget,
    /// Most items whose heights are held one by one (older ones go back to
    /// estimates, far from the viewport first).
    pub max_measured: usize,
    /// How many items beyond the viewport the source is asked to load
    /// (TanStack Virtual's `overscan`); at least a screenful is always asked for.
    pub overscan: usize,
    /// The height assumed, in px, for an item of one line before it is
    /// measured (TanStack Virtual's `estimateSize`); `None`: the style's own.
    pub estimate_size: Option<f32>,
    /// A drag or a fling past either end stretches the content like a rubber band (UIScrollView's
    /// curve) and springs back when the finger lets go. Off by default, as in Blender.
    pub overscroll: bool,
    /// A wheel notch eases in over a few frames, as on macOS, instead of scrolling at once as
    /// Blender's wheel does. Off by default.
    pub smooth_wheel: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            stick_to_bottom: true,
            start_at_end: false,
            cache_capacity: 4096,
            fling_budget: Budget::Time(Duration::from_micros(500)),
            max_measured: 1 << 20,
            overscan: 8,
            estimate_size: None,
            overscroll: false,
            smooth_wheel: false,
        }
    }
}

/// UIScrollView's rubber band: the shown stretch is `range * (1 - 1 / (x * RUBBER / range + 1))`.
const RUBBER: f64 = 0.55;
/// How long a stretch takes to relax (the time constant, s).
const RELAX: f64 = 0.09;
/// The most a stretch is asked for, in viewports (what a fling past an end can pile up).
const OVER_MAX: f64 = 2.0;
/// A fling that reaches an end stretches it by what that frame's step could not move, times this
/// (Android's EdgeEffect.onAbsorb: the stretch follows the speed it arrived at), at most half a viewport.
const FLING_ABSORB: f64 = 3.0;
const FLING_ABSORB_MAX: f64 = 0.5;

/// See the module docs.
pub struct ListView {
    pub(crate) index: HeightIndex,
    pub(crate) text: TextLayout,
    pub(crate) page: Page,
    pub(crate) page_version: u64,
    pub(crate) ahead: Page,
    pub(crate) pos: Pos,
    pub(crate) goal: Option<(usize, Align)>,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) opts: Options,
    pub(crate) placed: Vec<Placed>,
    pub(crate) updates: Vec<(usize, crate::fx::Fx)>,
    pub(crate) samples: Vec<(u64, f64)>,
    /// Smooth-scroll distance still to go, px.
    pub(crate) pending: f64,
    /// Scrolling since the last layout, to be walked exactly there (see `relative.rs`).
    pub(crate) rel: Option<relative::Relative>,
    /// Scrolling asked for past an end that is still stretched, px (+: past the end, -: past the top).
    pub(crate) over: f64,
    /// A finger is on the content: the stretch stays until it lifts.
    pub(crate) held: bool,
    /// A fling reached an end and gave its momentum to the stretch: the rest of it is dropped.
    pub(crate) absorbed: bool,
    /// Recent scroll speed, px per frame (decays): above a few screens per frame the user is flinging.
    pub(crate) fling: f64,
    pub(crate) last_dir: i8,
    pub(crate) frames: u32,
    pub(crate) requested: Option<std::ops::Range<usize>>,
    pub(crate) more_to_measure: bool,
}

impl ListView {
    /// A view over `src` in a viewport `width` x `height`.
    pub fn new<S: ItemSource, M: Measurer>(src: &S, m: &M, width: f32, height: f32, opts: Options) -> Result<Self, IndexError> {
        let index = HeightIndex::estimated(src.len(), estimator_for(m, &opts, width), |r| src.bytes(r))?;
        Ok(Self {
            index,
            text: TextLayout::new(opts.cache_capacity),
            page: Page::new(),
            page_version: u64::MAX,
            ahead: Page::new(),
            pos: if opts.start_at_end && opts.stick_to_bottom { Pos::Bottom } else { Pos::Anchor { item: 0, off: 0 } },
            goal: None,
            width,
            height,
            opts,
            placed: Vec::new(),
            updates: Vec::new(),
            samples: Vec::new(),
            pending: 0.0,
            rel: None,
            over: 0.0,
            held: false,
            absorbed: false,
            fling: 0.0,
            last_dir: 1,
            frames: 0,
            requested: None,
            more_to_measure: false,
        })
        .map(|mut v| {
            if opts.start_at_end && !opts.stick_to_bottom {
                v.scroll_to_end();
            }
            v
        })
    }

    /// A view over equal-height items (`n` rows of `h` px): no estimates, no measuring.
    pub fn uniform<M: Measurer>(n: usize, h: f64, m: &M, width: f32, height: f32, opts: Options) -> Result<Self, IndexError> {
        let index = HeightIndex::uniform(n, crate::fx::Fx::from_px(h), m.estimator(width.max(1.0)))?;
        let mut v = Self::new(&crate::source::VecSource::new(), m, width, height, opts)?;
        v.index = index;
        if opts.start_at_end {
            v.scroll_to_end();
        }
        Ok(v)
    }

    #[must_use]
    pub const fn options(&self) -> &Options {
        &self.opts
    }

    #[must_use]
    pub const fn index(&self) -> &HeightIndex {
        &self.index
    }

    #[must_use]
    pub const fn pos(&self) -> Pos {
        self.pos
    }

    #[must_use]
    pub const fn viewport(&self) -> (f32, f32) {
        (self.width, self.height)
    }

    /// The items placed by the last [`ListView::layout`].
    #[must_use]
    pub fn placed(&self) -> &[Placed] {
        &self.placed
    }

    /// The id of the anchor item, which stays put when everything above changes.
    #[must_use]
    pub fn anchor_id<S: ItemSource>(&self, src: &S) -> Option<u64> {
        match self.pos {
            Pos::Anchor { item, .. } => src.id_at(item),
            Pos::Bottom => src.id_at(self.index.len().checked_sub(1)?),
        }
    }

    pub(crate) fn view_units(&self) -> u64 {
        px_to_units(f64::from(self.height))
    }

    /// Whether the viewport is pinned to the end.
    #[must_use]
    pub const fn at_bottom(&self) -> bool {
        matches!(self.pos, Pos::Bottom)
    }

    /// The scroll position and the content height in px (estimates for what
    /// is not measured: the scrollbar's inputs).
    #[must_use]
    pub fn scroll_extent(&self) -> (f64, f64) {
        let total = self.index.total();
        let top = match self.pos {
            Pos::Anchor { item, off } => self.index.offset_of(item) + off,
            Pos::Bottom => total.saturating_sub(self.view_units()),
        };
        (units_to_px(top), units_to_px(total))
    }

    // -- scrolling ------------------------------------------------------------------------------------------

    /// Scroll to absolute offset `abs` (fixed-point units from the top).
    pub(crate) fn set_scroll_units(&mut self, abs: u64) {
        let max = self.index.total().saturating_sub(self.view_units());
        if abs >= max && self.opts.stick_to_bottom {
            self.pos = Pos::Bottom;
        } else if let Some(l) = self.index.locate(abs.min(max)) {
            self.pos = Pos::Anchor { item: l.item, off: l.within };
        }
    }

    /// Scroll by `dy` px (positive: towards the end). NaN is ignored.
    pub fn scroll_by(&mut self, dy: f64) {
        self.scroll_edge(dy);
    }

    /// Scrolls by `dy` px and returns how much of it the ends refused (positive past the end).
    fn scroll_edge(&mut self, dy: f64) -> f64 {
        if !dy.is_finite() || dy == 0.0 {
            return 0.0;
        }
        self.goal = None;
        self.last_dir = if dy > 0.0 { 1 } else { -1 };
        self.fling = self.fling.mul_add(0.85, dy.abs());
        let (top, total) = self.scroll_extent();
        let max_top = (total - f64::from(self.height)).max(0.0);
        let wanted = top + dy;
        let target = wanted.clamp(0.0, total);
        self.note_move(target - top);
        // stay exactly at the end when the end was reached (no estimate wobble)
        self.set_scroll_units(px_to_units(target));
        wanted - wanted.clamp(0.0, max_top)
    }

    /// A finger moves the content by `dy` px (positive: towards the end): 1:1, and past an end it
    /// stretches when [`Options::overscroll`] is on. The stretch stays until [`ListView::release`].
    pub fn drag(&mut self, dy: f64) {
        self.held = true;
        self.absorbed = false;
        let refused = self.gesture(dy);
        self.stretch_by(refused, 1.0);
    }

    /// The content coasts on after the finger by `dy` px. At an end its momentum is spent once, on a
    /// stretch that follows the speed it arrived at, and the rest of the fling is dropped (it does not
    /// hold the band out while it runs down): the band springs back at once.
    pub fn fling(&mut self, dy: f64) {
        self.held = false;
        if self.absorbed {
            return;
        }
        let refused = self.gesture(dy);
        if refused != 0.0 && self.opts.overscroll {
            self.absorbed = true;
            let most = f64::from(self.height) * FLING_ABSORB_MAX;
            self.over = (self.over + refused * FLING_ABSORB).clamp(-most, most);
        }
    }

    /// The finger lifted: a stretch springs back ([`ListView::tick`]).
    pub fn release(&mut self) {
        self.held = false;
    }

    /// Moves the content by `dy`: first back out of a stretch, as far as the finger went on screen
    /// (the content under it follows 1:1 even though the band is not linear), then through the
    /// list. Returns what the ends refused (positive past the end).
    fn gesture(&mut self, dy: f64) -> f64 {
        let mut dy = dy;
        if self.over * dy < 0.0 {
            let shown = self.shown(self.over.abs());
            let back = dy.abs().min(shown);
            self.over = self.raw_of(shown - back).copysign(self.over);
            dy -= back.copysign(dy);
        }
        self.scroll_edge(dy)
    }

    /// A finger's pull that the ends refused, `weight` of it added to the stretch.
    fn stretch_by(&mut self, refused: f64, weight: f64) {
        if self.opts.overscroll && refused != 0.0 {
            let max = f64::from(self.height) * OVER_MAX;
            self.over = (self.over + refused * weight).clamp(-max, max);
        }
    }

    /// What a pull of `raw` px shows, px: UIScrollView's curve, stiffer the further it goes.
    fn shown(&self, raw: f64) -> f64 {
        let range = f64::from(self.height).max(1.0);
        range * (1.0 - 1.0 / (raw * RUBBER / range + 1.0))
    }

    /// The pull that shows `shown` px ([`ListView::shown`] the other way).
    fn raw_of(&self, shown: f64) -> f64 {
        let range = f64::from(self.height).max(1.0);
        range / RUBBER * (1.0 / (1.0 - (shown / range).min(0.999)) - 1.0)
    }

    /// How far the content is shifted by a stretch, px: positive moves it down (stretched past the
    /// top), negative up (past the end).
    #[must_use]
    pub fn stretch(&self) -> f64 {
        -self.shown(self.over.abs()).copysign(self.over)
    }

    /// Scroll to `offset` px from the top of the content (TanStack
    /// Virtual's `scrollToOffset`): estimates for what is not measured, so
    /// an offset far down lands on the item the scrollbar would show. NaN is ignored.
    pub fn scroll_to_offset(&mut self, offset: f64) {
        if offset.is_nan() {
            return;
        }
        self.goal = None;
        self.rel = None;
        self.pending = 0.0;
        self.set_scroll_units(px_to_units(offset.clamp(0.0, self.scroll_extent().1)));
    }

    /// The items in the viewport after the last layout, first to last+1
    /// (TanStack Virtual's `range` without the overscan).
    #[must_use]
    pub fn visible_range(&self) -> Option<std::ops::Range<usize>> {
        let (first, last) = (self.placed.first()?, self.placed.last()?);
        Some(first.item..last.item + 1)
    }

    /// Scroll to a fraction `0..=1` of the content (the scrollbar thumb being dragged).
    pub fn scroll_to_fraction(&mut self, f: f64) {
        if f.is_nan() {
            return;
        }
        self.goal = None;
        self.rel = None;
        let max = self.index.total().saturating_sub(self.view_units());
        self.set_scroll_units((max as f64 * f.clamp(0.0, 1.0)) as u64);
    }

    /// The end of the list, at the bottom of the viewport.
    pub fn scroll_to_end(&mut self) {
        self.goal = None;
        self.rel = None;
        self.set_scroll_units(u64::MAX / 2);
        if !self.opts.stick_to_bottom {
            self.pos = self.bottom_anchor();
        }
    }

    /// Bring `item` to `align` in the viewport. The target is re-aimed each
    /// frame until it is measured, so estimates between here and there
    /// cannot leave it off.
    pub fn jump_to(&mut self, item: usize, align: Align) {
        if self.index.is_empty() {
            return;
        }
        let item = item.min(self.index.len() - 1);
        self.pending = 0.0;
        self.rel = None;
        self.goal = Some((item, align));
        self.pos = self.goal_pos(item, align);
    }

    /// [`ListView::jump_to`] by stable id; false when the id is unknown.
    pub fn jump_to_id<S: ItemSource>(&mut self, src: &S, id: u64, align: Align) -> bool {
        src.index_of(id).map(|i| self.jump_to(i, align)).is_some()
    }

    pub(crate) fn goal_pos(&self, item: usize, align: Align) -> Pos {
        let top = self.index.offset_of(item);
        let h = self.index.height_of(item).map_or(0, |(h, _)| u64::from(h.units()));
        let view = self.view_units();
        let abs = match align {
            Align::Start => top,
            Align::Center => (top + h / 2).saturating_sub(view / 2),
            Align::End => (top + h).saturating_sub(view),
        };
        let max = self.index.total().saturating_sub(view);
        if abs >= max && self.opts.stick_to_bottom && align != Align::Start {
            return Pos::Bottom;
        }
        self.index.locate(abs.min(max)).map_or(Pos::Anchor { item: 0, off: 0 }, |l| Pos::Anchor { item: l.item, off: l.within })
    }

    /// The anchor that puts the end of the list at the bottom edge.
    pub(crate) fn bottom_anchor(&self) -> Pos {
        let view = self.view_units();
        let n = self.index.len();
        if n == 0 {
            return Pos::Anchor { item: 0, off: 0 };
        }
        let mut acc = 0u64;
        let mut found = Pos::Anchor { item: 0, off: 0 };
        self.index.walk_backward(n - 1, |i, h, _| {
            acc += u64::from(h.units());
            if acc >= view {
                found = Pos::Anchor { item: i, off: acc - view };
                return false;
            }
            true
        });
        found
    }

    /// Keyboard navigation.
    pub fn navigate(&mut self, nav: Nav, line_px: f64) {
        let page = (f64::from(self.height) - line_px).max(line_px);
        match nav {
            Nav::LineUp => self.scroll_by(-line_px),
            Nav::LineDown => self.scroll_by(line_px),
            Nav::PageUp => self.scroll_by(-page),
            Nav::PageDown => self.scroll_by(page),
            Nav::Home => self.jump_to(0, Align::Start),
            Nav::End => self.scroll_to_end(),
        }
    }

    /// A wheel or trackpad delta. Precise deltas (a trackpad) apply at once;
    /// notched wheel deltas ease in over a few frames ([`ListView::tick`]).
    pub fn wheel(&mut self, dy: f64, precise: bool) {
        self.absorbed = false;
        if precise {
            self.scroll_by(dy);
        } else if dy.is_finite() {
            self.pending += dy;
        }
    }

    /// Advance the smooth scroll by `dt` seconds. Returns whether another
    /// frame is needed (false when idle: no frames while nothing moves).
    pub fn tick(&mut self, dt: f64) -> bool {
        let mut moving = false;
        if self.over != 0.0 && !self.held {
            // the band relaxes: critically damped, so it settles without swinging back
            self.over *= (-dt.max(0.0) / RELAX).exp();
            if self.over.abs() < 0.25 {
                self.over = 0.0;
            }
            moving = true;
        }
        if self.pending == 0.0 {
            return moving || self.more_to_measure;
        }
        let step = if self.pending.abs() < 0.5 { self.pending } else { self.pending * (1.0 - (-dt.max(0.0) / 0.075).exp()) };
        self.pending -= step;
        self.scroll_by(step);
        if self.pending.abs() < 0.01 {
            self.pending = 0.0;
        }
        true
    }

    /// Resize the viewport. A new width wraps text differently, so every
    /// measured height becomes an estimate again ([`HeightIndex::set_estimator`])
    /// and the anchor keeps its fraction of its item: what the user looks at
    /// stays at the top.
    pub fn resize<M: Measurer>(&mut self, m: &M, width: f32, height: f32) {
        self.height = height;
        if (width - self.width).abs() > f32::EPSILON {
            self.width = width;
            self.restyle(m);
        }
    }

    /// The measurer's style changed (font, size, metrics): forget every
    /// wrapped text and measured height; the anchor keeps its fraction of its item.
    pub fn restyle<M: Measurer>(&mut self, m: &M) {
        let before = match self.pos {
            Pos::Anchor { item, .. } => self.index.height_of(item).map(|(h, _)| u64::from(h.units())),
            Pos::Bottom => None,
        };
        let est = estimator_for(m, &self.opts, self.width);
        self.text.clear();
        self.index.set_estimator(est.with_text_width(est.text_width));
        self.page_version = u64::MAX;
        if let (Pos::Anchor { item, off }, Some(old)) = (&mut self.pos, before)
            && let Some((h, _)) = self.index.height_of(*item)
        {
            *off = (u128::from(*off) * u128::from(h.units()) / u128::from(old.max(1))) as u64;
        }
    }

    /// The placed item under viewport `y`.
    #[must_use]
    pub fn item_at(&self, y: f32) -> Option<usize> {
        self.placed.iter().find(|p| y >= p.y && y < p.y + p.h).map(|p| p.item)
    }

    /// Whether the user is flinging (loads wait, measuring is budgeted).
    #[must_use]
    pub fn flinging(&self) -> bool {
        self.fling > f64::from(self.height) * 2.0
    }
}

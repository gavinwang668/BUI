//! A scroll is a distance, and it moves the content exactly that far.
//!
//! [`ListView::scroll_by`] has no measurer, so it can only turn a distance into a position through the
//! index, where the items it has not measured yet (the ones above, when scrolling up) have estimated
//! heights; moving "100 px" through them moved the content by whatever the estimates were off by. So the
//! scroll is also remembered as the anchor it started from and the distance moved, and the next
//! [`ListView::layout`] walks that distance item by item from the anchor, measuring each item it passes,
//! and puts the anchor where the content really is.

use super::frame::Clock;
use super::{ListView, Pos};
use crate::fx::px_to_units;
use crate::measure::{Measurer, TextWidth};
use crate::source::{ItemSource, FLAG_PLACEHOLDER};

/// Scrolling since the last layout: where it started and how far it went (px, positive: towards the end).
#[derive(Copy, Clone, Debug)]
pub(crate) struct Relative {
    item: usize,
    off: u64,
    dy: f64,
}

impl ListView {
    /// Remember that the anchor moves by `dy` more, starting where it is now unless a move is pending.
    pub(crate) fn note_move(&mut self, dy: f64) {
        if self.rel.is_none() {
            let from = match self.pos {
                Pos::Anchor { item, off } => Some((item, off)),
                Pos::Bottom => match self.bottom_anchor() {
                    Pos::Anchor { item, off } => Some((item, off)),
                    Pos::Bottom => None,
                },
            };
            self.rel = from.map(|(item, off)| Relative { item, off, dy: 0.0 });
        }
        if let Some(r) = &mut self.rel {
            r.dy += dy;
        }
    }

    /// The height of `item` in units: measured now when it has not been (its text is wrapped once, and
    /// the layout of this frame finds it cached), else what the index holds.
    fn height_now<S: ItemSource, M: Measurer>(&mut self, src: &S, m: &M, tw: TextWidth<'_>, clock: &mut Clock, item: usize) -> u64 {
        let Some((h, exact)) = self.index.height_of(item) else { return 0 };
        if exact || clock.exhausted() {
            return u64::from(h.units());
        }
        self.ensure_page(src, &(item..item + 1));
        let Some(rec) = self.page.record(item).filter(|r| r.flags & FLAG_PLACEHOLDER == 0) else {
            return u64::from(h.units());
        };
        let measured = self.text.height(&rec, m, self.width, tw);
        self.updates.clear();
        self.samples.clear();
        self.updates.push((item, measured));
        self.samples.push((rec.text.len() as u64, measured.px()));
        clock.done += 1;
        self.commit_measured();
        u64::from(measured.units())
    }

    /// Put the anchor where the scroll since the last layout really took it.
    pub(super) fn apply_relative<S: ItemSource, M: Measurer>(&mut self, src: &S, m: &M, tw: TextWidth<'_>, clock: &mut Clock) {
        let Some(rel) = self.rel.take() else { return };
        // pinned to the end, or a goal being chased: the position was not made by this scroll
        if rel.dy == 0.0 || self.goal.is_some() || self.pos == Pos::Bottom || self.index.is_empty() {
            return;
        }
        let last = self.index.len() - 1;
        let (mut item, mut off) = (rel.item.min(last), rel.off);
        let mut left = px_to_units(rel.dy.abs());
        if rel.dy < 0.0 {
            // up: through what is above the anchor item's top edge
            while left > 0 {
                if left <= off {
                    off -= left;
                    break;
                }
                left -= off;
                if item == 0 {
                    off = 0;
                    break;
                }
                item -= 1;
                off = self.height_now(src, m, tw, clock, item);
            }
        } else {
            // down: through what is below the anchor's position
            while left > 0 {
                let h = self.height_now(src, m, tw, clock, item);
                let room = h.saturating_sub(off);
                if left <= room {
                    off += left;
                    break;
                }
                left -= room;
                if item == last {
                    off = h;
                    break;
                }
                item += 1;
                off = 0;
            }
        }
        self.pos = Pos::Anchor { item, off };
    }
}

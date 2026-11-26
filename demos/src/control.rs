//! A value the pointer drags: a knob, a fader, a scrubber. Press anywhere on the control and
//! drag (Blender's number-field drag: relative, so the value never jumps under the pointer),
//! or turn the wheel over it. The control holds the pointer while dragging (pointer capture),
//! so the drag carries on outside it. Plus the arcs knobs are drawn with.

use blenderui::prelude::*;

/// Which way the pointer moves the value up.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Up raises it (a knob).
    Up,
    /// Right raises it (a fader).
    Right,
}

/// A 0..1 value and the drag on it, if one is under way.
#[derive(Clone, Copy)]
pub struct Dial {
    pub value: f32,
    /// The pointer (along the axis) and the value when the drag began.
    grab: Option<(f32, f32)>,
}

impl Dial {
    #[must_use]
    pub const fn new(value: f32) -> Self {
        Self { value, grab: None }
    }

    #[must_use]
    pub const fn dragging(&self) -> bool {
        self.grab.is_some()
    }

    /// Sets the value (clamped); answers whether it changed.
    pub fn set(&mut self, v: f32) -> bool {
        let v = v.clamp(0.0, 1.0);
        let changed = (v - self.value).abs() > f32::EPSILON;
        self.value = v;
        changed
    }
}

const fn along(axis: Axis, p: Point<Pixels>) -> f32 {
    match axis {
        Axis::Up => -p.y.0,
        Axis::Right => p.x.0,
    }
}

/// Makes `d` drag the [`Dial`] `dial` picks out of the view: `travel` pixels along `axis` cover
/// the whole range, a wheel line moves it `1/40`. `changed` runs after every change (the view
/// re-renders after it).
pub fn draggable<V: 'static>(d: Div, cx: &mut Context<V>, axis: Axis, travel: f32, dial: fn(&mut V) -> &mut Dial,
                             changed: fn(&mut V)) -> Div {
    d.on_pointer_down(cx.listener(move |this, e: &PointerEvent, w, cx| {
        let k = dial(this);
        k.grab = Some((along(axis, e.position), k.value));
        w.set_pointer_capture(cx);
        cx.stop_propagation();
        cx.notify();
    }))
    .on_pointer_move(cx.listener(move |this, e: &PointerEvent, _, cx| {
        let k = dial(this);
        if let Some((at, from)) = k.grab
            && k.set(from + (along(axis, e.position) - at) / travel)
        {
            changed(this);
            cx.notify();
        }
    }))
    .on_pointer_up(cx.listener(move |this, _: &PointerEvent, w, cx| {
        dial(this).grab = None;
        w.release_pointer_capture(cx);
        cx.notify();
    }))
    .on_scroll_wheel(cx.listener(move |this, e: &ScrollWheelEvent, _, cx| {
        let k = dial(this);
        if k.set(k.value + e.delta / 40.0) {
            changed(this);
            cx.notify();
        }
        cx.stop_propagation();
    }))
}

/// Points on a circle about (`cx`, `cy`) from `from` to `to` degrees, clockwise from 12 o'clock
/// (how a dial reads), about one per 3 degrees.
#[must_use]
pub fn arc_points(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
    let n = (((to - from).abs() / 3.0).ceil() as usize).max(1);
    (0..=n)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / n as f32).to_radians();
            (cx + r * a.sin(), cy - r * a.cos())
        })
        .collect()
}

/// A stroked arc (see [`arc_points`]) with a soft glow: `glow` wider, fainter strokes under it.
pub fn glow_arc(p: &mut Painter, (cx, cy, r): (f32, f32, f32), (from, to): (f32, f32), color: Rgba, width: f32,
                glow: u32) {
    if (to - from).abs() < 0.5 {
        return;
    }
    let pts = arc_points(cx, cy, r, from, to);
    for g in (1..=glow).rev() {
        let k = g as f32;
        p.line(&pts, Rgba { a: color.a * 0.12 / k, ..color }, width + k * 4.0);
    }
    p.line(&pts, color, width);
}

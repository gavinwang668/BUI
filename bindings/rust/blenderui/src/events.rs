//! Event types passed to listeners (named as in gpui).

use crate::geometry::{Pixels, Point};
use blenderui_sys as sys;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Modifier keys held during an event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

impl Modifiers {
    pub(crate) fn from_kit(mods: i32) -> Modifiers {
        Modifiers { shift: mods & 1 != 0, control: mods & 2 != 0, alt: mods & 4 != 0 }
    }
}

#[derive(Clone, Debug)]
pub struct MouseDownEvent {
    pub button: MouseButton,
    pub position: Point<Pixels>,
    pub modifiers: Modifiers,
    pub click_count: usize,
}

#[derive(Clone, Debug)]
pub struct MouseUpEvent {
    pub button: MouseButton,
    pub position: Point<Pixels>,
    pub modifiers: Modifiers,
}

#[derive(Clone, Debug)]
pub struct MouseMoveEvent {
    pub position: Point<Pixels>,
    pub pressed_button: Option<MouseButton>,
    pub modifiers: Modifiers,
}

/// A click: the press and the release.
#[derive(Clone, Debug)]
pub struct ClickEvent {
    pub down: MouseDownEvent,
    pub up: MouseUpEvent,
}

impl ClickEvent {
    pub fn click_count(&self) -> usize {
        self.down.click_count
    }
}

/// How a scroll came: a wheel notch or touchpad, a finger dragging the content, the content coasting
/// on after the finger, or the finger lifting (`delta` 0, nothing to scroll).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollPhase {
    #[default]
    Wheel,
    Drag,
    Fling,
    End,
}

impl ScrollPhase {
    /// From the kit's `BK_SCROLL_*` (a wheel event's third number); anything else is a wheel.
    #[must_use]
    pub fn from_kit(n: i32) -> Self {
        match n {
            sys::BK_SCROLL_DRAG => Self::Drag,
            sys::BK_SCROLL_FLING => Self::Fling,
            sys::BK_SCROLL_END => Self::End,
            _ => Self::Wheel,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScrollWheelEvent {
    pub position: Point<Pixels>,
    /// Lines: positive scrolls the content up (wheel away from the user).
    pub delta: f32,
    /// The exact distance in logical pixels (same sign as `delta`) when the host knows it, as for
    /// a touch drag; `None` for a wheel notch. A view that scrolls itself follows it 1:1.
    pub pixels: Option<f32>,
    pub phase: ScrollPhase,
    pub modifiers: Modifiers,
}

/// Content a finger (or two) drags: the scroll a touch screen makes, Blender's MOUSEPAN
/// (`on_pan`). It comes before the scroll; a listener that calls `cx.stop_propagation()` takes
/// it, and nothing scrolls (a map that pans itself).
#[derive(Clone, Debug)]
pub struct PanEvent {
    /// Where the drag began (the fingers' centre for two), relative to the window.
    pub position: Point<Pixels>,
    /// Logical pixels the content follows the fingers since the last one: x right, y down. Each
    /// axis can come as an event of its own; the last one (`ScrollPhase::End`) is zero.
    pub delta: Point<Pixels>,
    /// `Drag` while a finger is down, `Fling` as the content coasts on after it, `End` when it
    /// lifted.
    pub phase: ScrollPhase,
    pub modifiers: Modifiers,
}

/// A pinch: two fingers spreading or closing about their centre, or a trackpad's (Blender's
/// MOUSEZOOM; `on_pinch`).
#[derive(Clone, Debug)]
pub struct PinchEvent {
    /// The centre: between the fingers, or the pointer for a trackpad.
    pub position: Point<Pixels>,
    /// The scale change since the last one: 0.1 is 10% larger, -0.1 10% smaller (a zoom is
    /// multiplied by `1.0 + delta`).
    pub delta: f32,
    pub modifiers: Modifiers,
    /// From a touch screen (`false`: a trackpad).
    pub touch: bool,
}

/// A turn of two fingers about their centre, or a trackpad's (Blender's MOUSEROTATE;
/// `on_rotate`).
#[derive(Clone, Debug)]
pub struct RotateEvent {
    /// The centre: between the fingers, or the pointer for a trackpad.
    pub position: Point<Pixels>,
    /// Degrees since the last one, positive counter-clockwise.
    pub degrees: f32,
    pub modifiers: Modifiers,
    /// From a touch screen (`false`: a trackpad).
    pub touch: bool,
}

/// One step of a touch screen contact ([`App::simulate_touch`](crate::App::simulate_touch)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Down,
    Move,
    Up,
    /// The platform took the contact back: nothing it did is kept.
    Cancel,
}

impl TouchPhase {
    pub(crate) const fn to_kit(self) -> i32 {
        match self {
            Self::Down => sys::BK_TOUCH_DOWN,
            Self::Move => sys::BK_TOUCH_MOVE,
            Self::Up => sys::BK_TOUCH_UP,
            Self::Cancel => sys::BK_TOUCH_CANCEL,
        }
    }
}

/// A key: `key` is the keysym (`"a"`, `"Return"`, `"Left"` ...).
#[derive(Clone, Debug)]
pub struct Keystroke {
    pub key: String,
    pub key_char: Option<String>,
    pub modifiers: Modifiers,
}

#[derive(Clone, Debug)]
pub struct KeyDownEvent {
    pub keystroke: Keystroke,
}

#[derive(Clone, Debug)]
pub struct KeyUpEvent {
    pub keystroke: Keystroke,
}

/// A form control's value (`input`, `select`, `textarea` ...).
#[derive(Clone, Debug, Default)]
pub struct ChangeEvent {
    /// The value as text (what `.value` holds in the DOM).
    pub value: String,
    /// Checkboxes and radio buttons.
    pub checked: bool,
    /// Numbers, ranges and selects (the selected index).
    pub number: f64,
}

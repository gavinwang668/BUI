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

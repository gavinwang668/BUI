//! A one-line text draft typed into by key events: what a demo's own text field does with a key.

use blenderui::prelude::*;

/// What a key did to a draft.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    /// A character went in or came out.
    Typed,
    /// Enter: the draft is done.
    Submit,
    /// Escape: stop typing.
    Leave,
    /// Not a key for the field.
    Ignored,
}

/// Feeds `k` to `draft` (at most `max` bytes long).
pub fn edit(draft: &mut String, k: &Keystroke, max: usize) -> Edit {
    match k.key.as_str() {
        "Escape" => Edit::Leave,
        "Return" | "KP_Enter" => Edit::Submit,
        "BackSpace" => {
            draft.pop();
            Edit::Typed
        }
        _ => match &k.key_char {
            Some(c) if !k.modifiers.control && !c.chars().any(char::is_control) && draft.len() + c.len() <= max => {
                draft.push_str(c);
                Edit::Typed
            }
            _ => Edit::Ignored,
        },
    }
}

//! A storyboard: a headless run that drives a demo like a person would (clicks, keys, typing,
//! drags) and saves frames along the virtual clock, so a change can be checked frame by frame.

use std::time::Duration;

use blenderui::prelude::*;

use crate::{advance, save_png, Script};

/// The app, its window and the folder frames are saved to.
pub struct Story<'a> {
    pub cx: &'a mut App,
    pub win: WindowId,
    dir: &'a str,
}

/// A [`Script`] that saves its frames into `dir` (made if missing): `--storyboard DIR`.
///
/// # Panics
/// When `dir` can't be made.
pub fn story(dir: String, f: impl FnOnce(&mut Story) + 'static) -> Script {
    Box::new(move |cx, win| {
        std::fs::create_dir_all(&dir).expect("create storyboard dir");
        f(&mut Story { cx, win, dir: &dir });
    })
}

impl Story<'_> {
    /// Saves the current frame as `dir/name.png`.
    pub fn shot(&mut self, name: &str) {
        let (w, h, px) = self.cx.draw(self.win);
        save_png(&format!("{}/{name}.png", self.dir), w, h, &px);
    }

    /// Moves the clock (BUI's and [`crate::now`]) on by `ms`.
    pub fn tick(&mut self, ms: u64) {
        advance(self.cx, Duration::from_millis(ms));
    }

    /// Saves `name-XXXXms` at each of `steps` (ms from now, ascending), the clock running between.
    pub fn play(&mut self, name: &str, steps: &[u64]) {
        let mut at = 0;
        for &ms in steps {
            self.tick(ms - at);
            at = ms;
            self.shot(&format!("{name}-{ms:04}ms"));
        }
    }

    /// The bounds of `#id`.
    ///
    /// # Panics
    /// When nothing has that id.
    pub fn bounds(&mut self, id: &str) -> Bounds<f32> {
        self.cx.element_bounds(self.win, id).unwrap_or_else(|| panic!("no #{id}"))
    }

    /// The centre of `#id`.
    pub fn centre(&mut self, id: &str) -> (f32, f32) {
        let b = self.bounds(id);
        (b.origin.x + b.size.width / 2.0, b.origin.y + b.size.height / 2.0)
    }

    /// Lays the window out as its next frame would: Blender widgets a render just made take clicks
    /// once a frame placed them, and in a window a frame always comes before the next click.
    fn settle(&mut self) {
        self.cx.layout(self.win);
    }

    /// Clicks the centre of `#id`.
    pub fn click(&mut self, id: &str) {
        self.settle();
        let (x, y) = self.centre(id);
        self.cx.simulate_click(self.win, x, y);
    }

    /// Moves the pointer to the centre of `#id` (a hover).
    pub fn hover(&mut self, id: &str) {
        self.settle();
        let (x, y) = self.centre(id);
        self.cx.simulate_mouse_move(self.win, x, y);
    }

    /// Presses and releases a named key (`Return`, `Down`, `Escape`).
    pub fn key(&mut self, key: &str) {
        self.cx.simulate_key(self.win, "", key);
    }

    /// Presses Ctrl (and Shift) with the named key.
    pub fn ctrl(&mut self, key: &str, shift: bool) {
        self.cx.simulate_key_with(self.win, "", key, true, shift);
    }

    /// Types `text` one character at a time.
    pub fn type_text(&mut self, text: &str) {
        let mut buf = [0u8; 4];
        for c in text.chars() {
            let s: &str = c.encode_utf8(&mut buf);
            self.cx.simulate_key(self.win, s, if c == ' ' { "space" } else { s });
        }
    }
}

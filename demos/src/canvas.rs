//! A design canvas that fits any window crisply. A demo lays out at a design size in CSS pixels;
//! [`rem_css`] rewrites those pixels as `rem`, and [`Fit`] sets the root font size to the scale, so
//! BUI lays everything out at the window's real size. (Scaling a finished layout with a CSS
//! `transform` instead resamples it: text and turned layers come out soft at large sizes.)

use blenderui::prelude::*;

/// How a design canvas sits in a window: `scale` fits it whole, and the canvas grows (`w`, `h` in
/// design pixels) to fill the rest; (`ox`, `oy`) centres the design inside it.
#[derive(Clone, Copy)]
pub struct Fit {
    pub scale: f32,
    pub w: f32,
    pub h: f32,
    pub ox: f32,
    pub oy: f32,
}

impl Fit {
    #[must_use]
    pub fn new(window: &Window, design: (f32, f32)) -> Self {
        let vp = window.viewport_size();
        let scale = (vp.width.0 / design.0).min(vp.height.0 / design.1).max(0.1);
        let (w, h) = (vp.width.0 / scale, vp.height.0 / scale);
        Self { scale, w, h, ox: (w - design.0) / 2.0, oy: (h - design.1) / 2.0 }
    }

    /// A window point (logical px) in design pixels.
    #[must_use]
    pub fn design(&self, p: Point<Pixels>) -> (f32, f32) {
        (p.x.0 / self.scale - self.ox, p.y.0 / self.scale - self.oy)
    }

    /// The root font size that makes `1rem` one design pixel x 16 at this scale.
    #[must_use]
    pub fn root_font(&self) -> String {
        format!("{}px", 16.0 * self.scale)
    }
}

/// A length in design pixels, as `rem`.
#[must_use]
pub fn rem(design_px: f32) -> String {
    format!("{}rem", design_px / 16.0)
}

/// Places `d` absolutely at (`x`, `y`) design pixels (its `.abs` class is `position: absolute`).
#[must_use]
pub fn at(d: Div, x: f32, y: f32) -> Div {
    d.class("abs").css("left", rem(x)).css("top", rem(y))
}

/// `css` with every `<number>px` written as `rem` (16 design pixels each).
#[must_use]
pub fn rem_css(css: &str) -> String {
    let mut out = String::with_capacity(css.len() + css.len() / 8);
    let b = css.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // a number: digits with at most one '.', not part of a name (`h2`, `#3fd8ff`)
        let starts = (b[i].is_ascii_digit() || (b[i] == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)))
            && !(i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'#'));
        if starts {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
                j += 1;
            }
            if css[j..].starts_with("px") {
                let v: f32 = css[i..j].parse().unwrap_or(0.0);
                out.push_str(&rem(v));
                i = j + 2;
                continue;
            }
            out.push_str(&css[i..j]);
            i = j;
            continue;
        }
        let ch = css[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

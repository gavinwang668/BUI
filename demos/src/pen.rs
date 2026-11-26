//! A `Painter` that draws in a design box's own units: a demo draws its dials and icons once, in
//! the numbers of its design (a 24-unit icon, a 300-unit face), and the pen scales them onto
//! whatever bounds the canvas got. Angles read like a dial: degrees clockwise from 12 o'clock.

use blenderui::prelude::*;

use crate::control::{arc_points, glow_arc};
use crate::ease::lerp;

/// sRGB `a` to `b` by `k`, opaque (a translucent stroke darkens where its segments overlap, so a
/// dimmed colour is mixed against its ground instead).
#[must_use]
pub fn mix(a: Rgba, b: Rgba, k: f32) -> Rgba {
    Rgba { r: lerp(a.r, b.r, k), g: lerp(a.g, b.g, k), b: lerp(a.b, b.b, k), a: 1.0 }
}

/// A painter scaled from a `units`-wide design box onto the canvas bounds.
pub struct Pen<'a> {
    pub p: &'a mut Painter,
    ox: f32,
    oy: f32,
    /// Window pixels per design unit.
    pub k: f32,
}

impl<'a> Pen<'a> {
    pub fn new(b: Bounds<f32>, p: &'a mut Painter, units: f32) -> Self {
        Self { p, ox: b.origin.x, oy: b.origin.y, k: b.size.width / units }
    }

    /// A design point in window pixels.
    #[must_use]
    pub fn pt(&self, x: f32, y: f32) -> (f32, f32) {
        (self.ox + x * self.k, self.oy + y * self.k)
    }

    /// A polyline through design points (no heap: the points are an array).
    pub fn line<const N: usize>(&mut self, pts: [(f32, f32); N], c: Rgba, w: f32) {
        self.p.line(&pts.map(|(x, y)| self.pt(x, y)), c, w * self.k);
    }

    /// From `centre` towards `deg`, radii `r0` to `r1`.
    pub fn ray(&mut self, (cx, cy): (f32, f32), deg: f32, r0: f32, r1: f32, c: Rgba, w: f32) {
        let (s, co) = deg.to_radians().sin_cos();
        self.line([(cx + s * r0, cy - co * r0), (cx + s * r1, cy - co * r1)], c, w);
    }

    /// A disc of radius `r` round `centre`, with an optional `ring` `w` wide.
    pub fn disc(&mut self, (cx, cy): (f32, f32), r: f32, fill: Rgba, ring: Rgba, w: f32) {
        let (x, y) = self.pt(cx - r, cy - r);
        let d = 2.0 * r * self.k;
        self.p.oval(Bounds { origin: point(x, y), size: size(d, d) }, fill, ring, w * self.k);
    }

    /// A rounded rectangle at design (`x`, `y`), `w` x `h`.
    pub fn quad(&mut self, (x, y): (f32, f32), (w, h): (f32, f32), radius: f32, fill: Rgba) {
        let (px, py) = self.pt(x, y);
        self.p.quad(Bounds { origin: point(px, py), size: size(w * self.k, h * self.k) }, radius * self.k, fill,
                    Rgba::TRANSPARENT, 0.0);
    }

    /// A stroked arc round `centre` from `from` to `to` degrees.
    pub fn arc(&mut self, (cx, cy): (f32, f32), r: f32, (from, to): (f32, f32), c: Rgba, w: f32) {
        let (x, y) = self.pt(cx, cy);
        self.p.line(&arc_points(x, y, r * self.k, from, to), c, w * self.k);
    }

    /// [`Pen::arc`] with [`glow_arc`]'s glow under it.
    pub fn glow_arc(&mut self, (cx, cy): (f32, f32), r: f32, span: (f32, f32), c: Rgba, w: f32, glow: u32) {
        let (x, y) = self.pt(cx, cy);
        glow_arc(self.p, (x, y, r * self.k), span, c, w * self.k, glow);
    }
}

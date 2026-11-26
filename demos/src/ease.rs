//! The interpolation every demo's hand-driven motion is made of: a mix, a window of a timeline
//! as 0..1, and the curves (CSS / GSAP names in the docs). Shared so no demo grows its own.

/// `a` to `b` by `t`.
#[must_use]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `u` through `a..b` as 0..1 (clamped).
#[must_use]
pub fn progress(u: f32, a: f32, b: f32) -> f32 {
    ((u - a) / (b - a)).clamp(0.0, 1.0)
}

/// Cubic ease-in (GSAP `power2.in`).
#[must_use]
pub fn ease_in(t: f32) -> f32 {
    t * t * t
}

/// Cubic ease-out (GSAP `power2.out`).
#[must_use]
pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Exponential ease-out (GSAP `expo.out`): fast, then a long settle.
#[must_use]
pub fn expo_out(t: f32) -> f32 {
    if t >= 1.0 { 1.0 } else { 1.0 - 2f32.powf(-10.0 * t) }
}

/// Moves `from` towards `to` by the fraction an exponential follow of rate `k` (1/s) covers in
/// `dt` seconds: frame-rate independent smoothing for values that chase a target.
#[must_use]
pub fn approach(from: f32, to: f32, k: f32, dt: f32) -> f32 {
    to + (from - to) * (-k * dt).exp()
}

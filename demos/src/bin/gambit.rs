//! Gambit: a noir chess collage, rendered by GPU shaders on BUI. A marble queen hangs over a
//! chessboard in perspective, a pawn rocks on its rim, poker chips tumble, chains sway, and it all
//! sits on drifting halftone smoke. There is no UI: a click anywhere is a move, which lifts the
//! queen and slams her down: a flash, a shock ring through the dots, the pawn knocked flat (it
//! gets back up), the chips spun up.
//!
//! Two passes: the raymarched scene at `SCENE_RES` into a render target, then a full-resolution
//! composite (halftone, sparkles, grade) that samples it. Everything that changes per frame (each
//! piece's position, every rotation's sine and cosine, the move's curves) is computed once here on
//! the CPU and handed to the shaders as floats, named by `slots!`.
//!
//! `cargo run --release --bin gambit`
//! `-- --frames DIR` renders frames off-screen on the GPU to DIR and times the passes (no window);

use std::fmt::Write as _;
use std::rc::Rc;

use blenderui::prelude::*;
use blenderui::{GpuTarget, RenderTarget, TargetFormat};
use bui_demos::{arg, gpu_stills, now, run, Fx};

/// Declares the shader floats once: the Rust indices (`Slot::QY as usize`) and the GLSL names
/// (`#define QY U[4]`) come from the same list.
macro_rules! slots {
    ($($name:ident),* $(,)?) => {
        #[derive(Clone, Copy)]
        #[allow(clippy::upper_case_acronyms)]
        enum Slot { $($name),* }
        const SLOT_NAMES: &[&str] = &[$(stringify!($name)),*];
    };
}

slots!(
    T, SHOCK, FLASH, // time, seconds since the queen landed (< 0: never), the landing's flash
    QX, QY, QZ, QC, QS, QYC, QYS, // queen: position, tilt (cos, sin), turn
    HC, HS, HPC, HPS, HYC, HYS, // hanging piece: sway, its tilt and turn
    FY, FC, FS, // the far piece: height, roll
    BC, BS, // the board pawn's lean
    C0Y, C1Y, C2Y, C0C, C0S, C1C, C1S, C2C, C2S, C0YC, C0YS, C1YC, C1YS, C2YC, C2YS, CBC, CBS, // chips
    GC, GS, GBEND, // the foreground chain: swing and bend
    KY, KC, KS, // crown
    LX, LY, LZ, // light direction
    CX, CY, CZ, // camera position
    MARCH, REFLECT, SHADOWS, BOUNCES, TAPS, // loop bounds (see `Pose::at`)
);
const N: usize = SLOT_NAMES.len();

/// The scene pass's resolution: the raymarch runs at this size, the composite at the window's.
const SCENE_RES: (u32, u32) = (800, 450);

// A move is `age` seconds old (< 0: none yet). The queen lifts for 0.22 s (ease-out), then drops
// onto a damped spring and lands at IMPACT.
const IMPACT: f32 = 0.34;

fn slam(age: f32) -> f32 {
    if age < 0.0 {
        return 0.0;
    }
    if age < 0.22 {
        let u = age / 0.22;
        return 0.45 * (1.0 - (1.0 - u) * (1.0 - u));
    }
    let a = age - 0.22;
    0.45 * (-a * 3.2).exp() * (a * 13.0).cos() - 0.35 * (-a * 2.4).exp() * (a * 13.0).min(std::f32::consts::PI).sin()
}

fn settle(age: f32) -> f32 {
    if age < 0.0 { 0.0 } else { 1.0 - (-age * 1.6).exp() }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let u = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// The board pawn, knocked over at impact (a gravity ease-in, a small bounce), up again after 4 s.
fn topple(age: f32) -> f32 {
    if age < IMPACT {
        return 0.0;
    }
    let a = age - IMPACT;
    let down = (a / 0.42).clamp(0.0, 1.0).powi(2);
    let b = (a - 0.42).max(0.0);
    let bounce = 0.12 * (-b * 7.0).exp() * (b * 16.0).sin().abs();
    (down - bounce) * (1.0 - smoothstep(4.0, 5.6, a))
}

/// Every shader float for time `t` and a move `age` seconds old.
struct Pose([f32; N]);

impl Pose {
    const fn set(&mut self, s: Slot, v: f32) {
        self.0[s as usize] = v;
    }

    fn turn(&mut self, c: Slot, s: Slot, angle: f32) {
        let (sin, cos) = angle.sin_cos();
        self.set(c, cos);
        self.set(s, sin);
    }

    fn at(t: f32, age: f32) -> Self {
        let mut u = Self([0.0; N]);
        let since = if age >= IMPACT { age - IMPACT } else { -1.0 };
        u.set(Slot::T, t);
        u.set(Slot::SHOCK, since);
        u.set(Slot::FLASH, if since >= 0.0 { (-since * 7.0).exp() } else { 0.0 });
        // the queen breathes and turns; a move lifts and slams her
        let slm = slam(age);
        u.set(Slot::QX, -0.5);
        u.set(Slot::QY, 1.55 + 0.1 * (t * 0.55).sin() + slm);
        u.set(Slot::QZ, -1.55);
        u.turn(Slot::QC, Slot::QS, 0.17 + 0.03 * (t * 0.4).sin() - 0.08 * slm);
        u.turn(Slot::QYC, Slot::QYS, t * 0.12);
        // the hanging piece swings on its chain
        let sw = 0.12 * (t * 0.7).sin();
        u.turn(Slot::HC, Slot::HS, sw);
        u.turn(Slot::HPC, Slot::HPS, 2.6 + sw * 0.5);
        u.turn(Slot::HYC, Slot::HYS, -t * 0.2);
        u.set(Slot::FY, 3.85 + 0.15 * (t * 0.3 + 1.0).sin());
        u.turn(Slot::FC, Slot::FS, 1.9 + t * 0.05);
        // the board pawn rocks on its rim until a move knocks it flat
        let lean = 0.55 + 0.08 * (t * 1.3).sin();
        u.turn(Slot::BC, Slot::BS, lean + (1.3 - lean) * topple(age));
        // chips bob and turn; a move spins them up
        let spin = t * 0.9 + 9.0 * settle(age);
        for (i, (y, (c, s), (yc, ys))) in [
            (Slot::C0Y, (Slot::C0C, Slot::C0S), (Slot::C0YC, Slot::C0YS)),
            (Slot::C1Y, (Slot::C1C, Slot::C1S), (Slot::C1YC, Slot::C1YS)),
            (Slot::C2Y, (Slot::C2C, Slot::C2S), (Slot::C2YC, Slot::C2YS)),
        ].into_iter().enumerate() {
            let f = i as f32;
            u.set(y, 1.55 - 0.47 * f + 0.06 * (t * 0.8 + f * 2.0).sin());
            u.turn(c, s, 0.5 + 0.4 * f + 0.15 * (t * 0.5 + f).sin());
            u.turn(yc, ys, spin * (1.0 + 0.3 * f));
        }
        u.turn(Slot::CBC, Slot::CBS, t * 0.1);
        u.turn(Slot::GC, Slot::GS, 2.38 + 0.05 * (t * 0.6).sin());
        u.set(Slot::GBEND, 0.1 * (t * 0.5).sin());
        u.set(Slot::KY, 2.25 + 0.06 * (t * 0.7).sin());
        u.turn(Slot::KC, Slot::KS, t * 0.3);
        // the key light drifts; the camera dollies in on first light, then breathes
        let l = [-0.55 + 0.35 * (t * 0.17).sin(), 0.85, -0.3f32];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        u.set(Slot::LX, l[0] / n);
        u.set(Slot::LY, l[1] / n);
        u.set(Slot::LZ, l[2] / n);
        let intro = (1.0 - smoothstep(0.0, 4.0, t)).powi(2);
        u.set(Slot::CX, 0.3 + 0.3 * (t * 0.09).sin());
        u.set(Slot::CY, 2.3 + 0.08 * (t * 0.21).sin() + 0.4 * intro);
        u.set(Slot::CZ, -7.0 + 0.3 * (t * 0.06).sin() - 2.2 * intro);
        // Loop bounds come in as floats, not literals. Direct3D compiles effects at its top
        // optimization level: it inlines every call and unrolls every loop it can count, so each
        // `map` call site and each counted step became another copy of the whole scene (33 s to
        // the first frame). Bounds it can't see stay loops; with one march, one normal tap and
        // one shadow loop there are four copies of `map` in all.
        u.set(Slot::MARCH, 72.0);
        u.set(Slot::REFLECT, 28.0);
        u.set(Slot::SHADOWS, 14.0);
        u.set(Slot::BOUNCES, 2.0);
        u.set(Slot::TAPS, 4.0);
        u
    }
}

/// Shared by both passes.
const NOISE: &str = r"
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
    vec2 i = floor(p); vec2 f = fract(p); f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}
float fbm(vec2 p) {
    float s = 0.0; float a = 0.5;
    for (int i = 0; i < 5; i++) { s += a * vnoise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; }
    return s;
}
";

const SCENE: &str = r"
float smin(float a, float b, float k) { float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0); return mix(b, a, h) - k * h * (1.0 - h); }
// rotations by a (cos, sin) from the CPU; rotx/rotz with a literal angle fold to constants
vec3 rz(vec3 p, float c, float s) { return vec3(c * p.x - s * p.y, s * p.x + c * p.y, p.z); }
vec3 ry(vec3 p, float c, float s) { return vec3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z); }
vec3 rotx(vec3 p, float a) { float c = cos(a); float s = sin(a); return vec3(p.x, c * p.y - s * p.z, s * p.y + c * p.z); }
vec3 rotz(vec3 p, float a) { return rz(p, cos(a), sin(a)); }
vec3 roty(vec3 p, float a) { return ry(p, cos(a), sin(a)); }
float box2(vec2 p, vec2 b, float r) { vec2 d = abs(p) - b + r; return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - r; }
float box3(vec3 p, vec3 b, float r) { vec3 d = abs(p) - b + r; return length(max(d, vec3(0.0))) + min(max(d.x, max(d.y, d.z)), 0.0) - r; }
float trap(vec2 p, float r1, float r2, float he) {
    vec2 k1 = vec2(r2, he); vec2 k2 = vec2(r2 - r1, 2.0 * he);
    p.x = abs(p.x);
    vec2 ca = vec2(p.x - min(p.x, (p.y < 0.0) ? r1 : r2), abs(p.y) - he);
    vec2 cb = p - k1 + k2 * clamp(dot(k1 - p, k2) / dot(k2, k2), 0.0, 1.0);
    float s = (cb.x < 0.0 && ca.y < 0.0) ? -1.0 : 1.0;
    return s * sqrt(min(dot(ca, ca), dot(cb, cb)));
}
vec3 sector(vec3 p, float n) {
    float sec = 6.2831853 / n; float a = atan2(p.z, p.x);
    float an = mod(a + sec * 0.5, sec) - sec * 0.5; float r = length(p.xz);
    return vec3(cos(an) * r, p.y, sin(an) * r);
}

float pawn(vec3 p) {
    vec2 q = vec2(length(p.xz), p.y);
    float d = box2(q - vec2(0.0, 0.08), vec2(0.42, 0.08), 0.04);
    d = smin(d, length(q - vec2(0.37, 0.2)) - 0.07, 0.05);
    d = smin(d, trap(q - vec2(0.0, 0.55), 0.3, 0.12, 0.35), 0.08);
    d = smin(d, box2(q - vec2(0.0, 0.92), vec2(0.25, 0.035), 0.03), 0.05);
    d = smin(d, length(q - vec2(0.0, 1.17)) - 0.24, 0.05);
    return d;
}
float queen(vec3 p) {
    vec2 q = vec2(length(p.xz), p.y);
    float d = box2(q - vec2(0.0, 0.09), vec2(0.52, 0.09), 0.05);
    d = smin(d, length(q - vec2(0.44, 0.24)) - 0.08, 0.05);
    d = smin(d, box2(q - vec2(0.0, 0.36), vec2(0.4, 0.04), 0.03), 0.05);
    d = smin(d, trap(q - vec2(0.0, 0.95), 0.34, 0.13, 0.6), 0.12);
    d = smin(d, box2(q - vec2(0.0, 1.58), vec2(0.31, 0.035), 0.03), 0.06);
    d = smin(d, box2(q - vec2(0.0, 1.68), vec2(0.24, 0.03), 0.03), 0.04);
    d = smin(d, trap(q - vec2(0.0, 1.92), 0.15, 0.31, 0.2), 0.06);
    // the coronet's points (an angle per sample: only near the top)
    if (p.y > 1.8) d = smin(d, length(sector(p, 8.0) - vec3(0.29, 2.14, 0.0)) - 0.065, 0.04);
    return min(d, length(q - vec2(0.0, 2.3)) - 0.1);
}
float chip(vec3 p) {
    vec2 d = abs(vec2(length(p.xz), p.y)) - vec2(0.45, 0.04);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2(0.0))) - 0.012;
}
float chipInk(vec3 p) {
    float a = atan2(p.z, p.x); float r = length(p.xz);
    float edge = step(0.5, fract(a * 6.0 / 6.2831853)) * step(0.33, r);
    float ring = step(abs(r - 0.27), 0.025);
    return max(edge, ring);
}
float clink(vec3 p) {
    vec3 q = vec3(p.x, max(abs(p.y) - 0.13, 0.0), p.z);
    return length(vec2(length(q.xy) - 0.17, q.z)) - 0.045;
}
// links along +y from 0 to len, every other one turned a quarter; whole links only, never cut
float chain(vec3 p, float len) {
    float ca = floor(p.y + 0.5); float cb = floor(p.y) + 0.5;
    float da = ca >= 0.0 && ca <= len ? clink(vec3(p.x, p.y - ca, p.z)) : 1e3;
    float db = cb >= 0.0 && cb <= len ? clink(vec3(p.z, p.y - cb, p.x)) : 1e3;
    return max(min(da, db), max(-p.y - 0.4, p.y - len - 0.4));
}
// a court card's face: an inner frame, a large diamond pip and two corner pips
float cardInk(vec2 c) {
    vec2 a = abs(c);
    float frame = step(abs(max(a.x - 0.25, a.y - 0.39)), 0.008);
    float pip = step(a.x / 0.11 + a.y / 0.16, 1.0);
    vec2 k = vec2(a.x - 0.25, a.y - 0.39);
    float corner = step(abs(k.x) / 0.03 + abs(k.y) / 0.045, 1.0) * step(0.0, c.x * c.y);
    return max(max(frame, pip), corner);
}
float crown(vec3 p) {
    vec2 q = vec2(length(p.xz), p.y);
    float d = box2(q - vec2(0.32, 0.08), vec2(0.03, 0.08), 0.02);
    vec3 c = sector(p, 6.0);
    vec3 pa = c - vec3(0.32, 0.14, 0.0); vec3 ba = vec3(0.05, 0.3, 0.0);
    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    d = min(d, length(pa - ba * h) - mix(0.05, 0.012, h));
    return min(d, length(c - vec3(0.375, 0.47, 0.0)) - 0.04);
}

vec2 opU(vec2 a, vec2 b) { return a.x < b.x ? a : b; }
// the distance to a segment from the origin along `d`, `len` long: a capsule's core
float seg(vec3 q, vec3 d, float len) { return length(q - d * clamp(dot(q, d), 0.0, len)); }

vec2 chipAt(vec3 p, vec3 c, float zc, float zs, float tilt, float yc, float ys) {
    vec3 q = p - c;
    float b = length(q) - 0.3;
    if (b > 0.2) return vec2(b, 3.0);
    q = ry(rotx(rz(q, zc, zs), tilt), yc, ys) / 0.55;
    return vec2(chip(q) * 0.55, 3.0 + 0.5 * chipInk(q));
}

// Each object is first a cheap bound around its pivot; it is turned and measured only when near.
vec2 map(vec3 p) {
    vec2 res = vec2(1e5, 0.0);
    // the queen (material 1: white marble)
    vec3 q = p - vec3(QX, QY, QZ);
    float b = length(q - vec3(0.15, 1.1, 0.0)) - 1.6;
    res = opU(res, vec2(b > 0.2 ? b : queen(ry(rotx(rz(q, QC, QS), -0.1), QYC, QYS) / 0.95) * 0.95, 1.0));
    // the hanging piece and its chain (4: metal)
    vec3 h = rz(p - vec3(2.0, 4.9, 1.0), HC, HS);
    vec3 k = -rotz(h, 0.62);
    b = seg(k, vec3(0.0, 1.0, 0.0), 2.1) - 0.35;
    res = opU(res, vec2(b > 0.2 ? b : chain(k / 0.8, 2.6) * 0.8, 4.0));
    q = h - vec3(1.25, -2.05, 0.0);
    b = length(q) - 1.75;
    res = opU(res, vec2(b > 0.2 ? b : pawn(ry(rz(q, HPC, HPS), HYC, HYS) / 1.1) * 1.1, 1.0));
    // the far piece (2: dark marble)
    q = p - vec3(0.95, FY, 3.2);
    b = length(q) - 1.6;
    res = opU(res, vec2(b > 0.2 ? b : pawn(rotx(rz(q, FC, FS), 0.4) / 0.85) * 0.85, 2.0));
    // the pawn on the board, leaning on its rim
    q = p - vec3(1.1, 0.0, 1.0);
    b = length(q - vec3(0.0, 0.45, 0.0)) - 1.2;
    res = opU(res, vec2(b > 0.2 ? b : pawn((rz(q - vec3(0.33, 0.0, 0.0), BC, BS) + vec3(0.33, 0.0, 0.0)) / 0.75) * 0.75, 1.0));
    // poker chips (3, ink in the fraction)
    res = opU(res, chipAt(p, vec3(2.35, C0Y, -1.5), C0C, C0S, 1.25, C0YC, C0YS));
    res = opU(res, chipAt(p, vec3(2.63, C1Y, -1.35), C1C, C1S, 1.0, C1YC, C1YS));
    res = opU(res, chipAt(p, vec3(2.51, C2Y, -1.2), C2C, C2S, 0.75, C2YC, C2YS));
    q = p - vec3(1.9, 0.025, -0.4);
    b = length(q) - 0.3;
    if (b > 0.2) res = opU(res, vec2(b, 3.0));
    else { q = ry(q, CBC, CBS) / 0.55; res = opU(res, vec2(chip(q) * 0.55, 3.0 + 0.5 * chipInk(q))); }
    // the foreground chain, bottom left, bending as it sways
    q = p - vec3(-2.3, 2.3, -3.0);
    b = seg(q, vec3(0.69, -0.723, 0.0), 2.7) - 0.4;
    if (b > 0.2) res = opU(res, vec2(b, 4.0));
    else { q = rz(q, GC, GS); q = rotz(q, GBEND * q.y); res = opU(res, vec2(chain(q / 0.6, 4.4) * 0.48, 4.0)); }
    // crown (4) and card (5, ink in the fraction)
    q = p - vec3(-1.95, KY, -1.5);
    b = length(q) - 1.0;
    if (b > 0.2) return opU(res, vec2(b, 5.0));
    res = opU(res, vec2(crown(ry(rotz(rotx(q, 0.35), -0.3), KC, KS)), 4.0));
    vec3 cd = rotz(roty(q - vec3(-0.25, -0.35, 0.25), -0.35), 0.35);
    return opU(res, vec2(box3(cd, vec3(0.32, 0.46, 0.008), 0.03), 5.0 + 0.5 * cardInk(cd.xy)));
}

// the tetrahedral gradient, as a loop: one `map` in the compiled shader, not four
vec3 normalAt(vec3 p) {
    vec3 n = vec3(0.0);
    for (int k = 0; k < int(TAPS); k++) {
        float f = float(k);
        vec3 e = 2.0 * vec3(mod(floor((f + 3.0) * 0.5), 2.0), mod(floor(f * 0.5), 2.0), mod(f, 2.0)) - 1.0;
        n += e * map(p + e * 0.0015).x;
    }
    return normalize(n);
}

float march(vec3 ro, vec3 rd, float tmax, int steps, out float id) {
    float t = 0.05; id = -1.0;
    for (int i = 0; i < steps; i++) {
        vec2 h = map(ro + rd * t);
        if (h.x < 0.002 * t) { id = h.y; return t; }
        t += h.x * 0.9;
        if (t > tmax) break;
    }
    return -1.0;
}

float shadow(vec3 ro, vec3 rd) {
    float r = 1.0; float t = 0.06;
    for (int i = 0; i < int(SHADOWS); i++) {
        float h = map(ro + rd * t).x;
        r = min(r, 9.0 * h / t);
        t += clamp(h, 0.08, 0.5);
        if (r < 0.01 || t > 6.0) break;
    }
    return clamp(r, 0.0, 1.0);
}

float shade(vec3 p, vec3 n, vec3 rd, float id) {
    vec3 L = vec3(LX, LY, LZ);
    float dif = clamp(dot(n, L), 0.0, 1.0);
    float bac = clamp(dot(n, normalize(vec3(0.7, 0.2, 0.8))), 0.0, 1.0);
    float fre = pow(1.0 - clamp(dot(n, -rd), 0.0, 1.0), 3.0);
    float spe = pow(clamp(dot(n, normalize(L - rd)), 0.0, 1.0), 60.0);
    float alb = 0.8; float gloss = 0.8;
    if (id < 1.5) { vec2 m = p.xy * 3.0 + p.z * 2.0; alb = 0.95 - 0.25 * smoothstep(0.55, 0.8, 0.55 * vnoise(m) + 0.3 * vnoise(m * 2.1) + 0.15 * vnoise(m * 4.3)); }
    else if (id < 2.5) { alb = 0.34; gloss = 0.6; }
    else if (id < 3.75) { alb = id > 3.25 ? 0.88 : 0.16; gloss = 0.5; }
    else if (id < 4.5) { alb = 0.55; gloss = 1.8; }
    else { alb = id > 5.25 ? 0.12 : 0.95; gloss = 0.3; }
    float ao = clamp(map(p + n * 0.12).x / 0.12, 0.3, 1.0);
    return alb * (0.06 + 0.85 * dif + 0.2 * bac) * ao + gloss * spe + 0.3 * fre * ao;
}

// Pass 1, at SCENE_RES: r = the lit surface, g = its coverage (the rest is the halftone, drawn in
// pass 2 at full resolution), b = 1 where the ray meets the floor plane.
vec4 effect(vec2 p) {
    vec2 uv = (p - 0.5 * box_size) / box_size.y; uv.y = -uv.y;
    vec3 ro = vec3(CX, CY, CZ);
    vec3 ww = normalize(vec3(0.4, 1.7, 0.0) - ro);
    vec3 uu = normalize(cross(vec3(0.0, 1.0, 0.0), ww));
    vec3 vv = cross(ww, uu);
    vec3 rd = normalize(uv.x * uu + uv.y * vv + 1.75 * ww);
    // One march for the camera ray and, off a white square, its reflection: one call site.
    vec4 board = vec4(0.0, 0.0, 0.0, 1.0);
    for (int bounce = 0; bounce < int(BOUNCES); bounce++) {
        // nothing lies past the board or beyond z = 5: rays stop there
        float tb = rd.y < 0.0 ? -ro.y / rd.y : -1.0;
        float id;
        float far = bounce == 0 ? (tb > 0.0 ? min(tb, 14.0) : 14.0) : 8.0;
        float t = march(ro, rd, far, bounce == 0 ? int(MARCH) : int(REFLECT), id);
        if (t > 0.0) {
            vec3 hit = ro + rd * t;
            float c = shade(hit, normalAt(hit), rd, id);
            // far pieces let the halftone through: a double exposure
            if (bounce == 0) return vec4(c, 1.0 - clamp((t - 9.0) * 0.15, 0.0, 0.4), 0.0, 1.0);
            board.r += 0.18 * c;
            break;
        }
        if (bounce > 0 || tb < 0.0) break;
        vec3 pos = ro + rd * tb;
        vec2 g = (pos.xz - vec2(0.45, 0.6)) / 0.62;
        float inside = step(abs(g.x), 6.0) * step(abs(g.y), 6.0);
        float chk = mod(floor(g.x) + floor(g.y), 2.0);
        vec2 fg = abs(fract(g) - 0.5);
        float seam = smoothstep(0.5, 0.485, max(fg.x, fg.y));
        float sq = mix(0.05, 0.8, chk) * mix(0.55, 1.0, seam);
        vec3 L = vec3(LX, LY, LZ);
        vec3 rr = reflect(rd, vec3(0.0, 1.0, 0.0));
        float sh = inside > 0.5 ? shadow(pos + vec3(0.0, 0.002, 0.0), L) : 1.0;
        float fade = exp(-0.11 * max(tb - 4.0, 0.0)) * smoothstep(-0.45, 0.25, -rd.y * 2.0 + 0.1);
        board = vec4(sq * (0.25 + 0.75 * sh) + 0.35 * pow(clamp(dot(rr, L), 0.0, 1.0), 40.0),
                     inside * fade * (1.0 - 0.25 * smoothstep(8.0, 14.0, tb)), 1.0, 1.0);
        // a glossy board: the pieces reflect in the white squares (the black ones hide it)
        if (chk < 0.5 || inside < 0.5) break;
        ro = pos + vec3(0.0, 0.01, 0.0);
        rd = rr;
    }
    return board;
}
";

const COMPOSITE: &str = r"
float halftone(vec2 p, float t, float shock, vec2 shockAt) {
    float cell = max(7.0, box_size.y / 56.0);
    vec2 c = (floor(p / cell) + 0.5) * cell;
    vec2 u = c / box_size.y;
    float smoke = fbm(u * 2.6 + vec2(t * 0.025, -t * 0.018));
    float light = 0.3 + 1.0 * exp(-5.0 * dot(u - vec2(0.85, 0.02), u - vec2(0.85, 0.02)))
                + 0.7 * exp(-7.0 * dot(u - vec2(1.6, 0.3), u - vec2(1.6, 0.3)));
    float holes = 1.0 - 0.95 * exp(-30.0 * dot(u - vec2(0.24, 0.1), u - vec2(0.24, 0.1)))
                      - 0.9 * exp(-22.0 * dot(u - vec2(1.13, 0.34), u - vec2(1.13, 0.34)));
    float L = clamp(smoke * light * holes * 1.25, 0.0, 1.0);
    // the shock ring of a landing rolls out through the dots
    if (shock >= 0.0) {
        float dr = length(u - shockAt) - shock * 0.9;
        L += 0.6 * exp(-dr * dr * 400.0) * exp(-shock * 1.2);
    }
    // the dots fade in on first light, scattered
    L *= smoothstep(0.0, 1.0, t * 0.7 - hash(c) * 0.8);
    float rad = 0.5 * cell * sqrt(clamp(L, 0.0, 1.0));
    return smoothstep(rad + 0.8, rad - 0.8, length(p - c)) * (0.3 + 0.55 * L);
}

float sparkle(vec2 p, float t) {
    float cell = box_size.y / 9.0;
    vec2 id = floor(p / cell);
    float h = hash(id + 3.1);
    if (h < 0.72) return 0.0;
    vec2 c = (id + vec2(hash(id + 7.0), hash(id + 1.3))) * cell;
    vec2 d = abs(p - c) / (box_size.y / 720.0);
    float tw = pow(0.5 + 0.5 * sin(t * (1.5 + 2.0 * h) + h * 40.0), 6.0);
    float star = exp(-length(d) * 0.35) + 0.6 * exp(-d.x * 0.9) * exp(-d.y * 0.04) + 0.6 * exp(-d.y * 0.9) * exp(-d.x * 0.04);
    return star * tw * 0.8;
}

// Pass 2, at full resolution: the halftone smoke, the scene over it, sparkles and the grade.
vec4 effect(vec2 p) {
    float t = T;
    vec2 uv = (p - 0.5 * box_size) / box_size.y; uv.y = -uv.y;
    vec2 n = input_size(0);
    float sc = max(box_size.x / n.x, box_size.y / n.y);
    vec4 s = input_raw(0, (p - 0.5 * box_size) / sc + 0.5 * n);
    float bg = 0.025 + 0.05 * (1.0 - length(uv)) + halftone(p, t, SHOCK, vec2(0.6, 0.62));
    bg *= 1.0 - 0.4 * s.b;
    float col = mix(bg, s.r, s.g);
    col += sparkle(p, t) * (0.55 + 0.45 * s.g);
    // grade: a soft shoulder, vignette, grain, first light, the flash of a landing queen
    col *= 1.0 - 0.55 * pow(length(uv * vec2(0.85, 1.1)), 2.2);
    col = col / (1.0 + 0.35 * col) * 1.3;
    col += (hash(p + fract(t * 13.0) * 91.7) - 0.5) * 0.06;
    col *= smoothstep(0.0, 2.2, t);
    col += 0.16 * FLASH;
    return vec4(vec3(clamp(col, 0.0, 1.0)), 1.0);
}
";

/// The two passes: the raymarched scene, and the composite that samples it.
fn shaders() -> (Fx, Fx) {
    let defines = SLOT_NAMES.iter().enumerate().fold(String::new(), |mut s, (i, n)| {
        let _ = writeln!(s, "#define {n} U[{i}]");
        s
    });
    (Fx::new("gambit_scene", &format!("{defines}{NOISE}{SCENE}"), N, [12, 12, 12]),
     Fx::sampling("gambit", &format!("{defines}{NOISE}{COMPOSITE}"), 1, N, [12, 12, 12]))
}

fn target() -> RenderTarget {
    RenderTarget::new(SCENE_RES.0, SCENE_RES.1, TargetFormat::Rgba16F).expect("scene render target")
}

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; position: relative; background: #050505; overflow: hidden; cursor: pointer; }
.scene { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
";

struct Gambit {
    scene: Fx,
    composite: Fx,
    target: Rc<RenderTarget>,
    /// When the last move was made (`now()`), if any.
    moved: Option<f32>,
    /// Whether the shaders' compile logs have been reported (once, after the first GPU frames).
    reported: bool,
}

impl Render for Gambit {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        if !self.reported && now() > 1.0 {
            self.reported = true;
            eprintln!("scene shader: {:?}, composite shader: {:?}", self.scene.error(), self.composite.error());
        }
        let moved = self.moved;
        let (pass, target) = (self.scene.effect().clone(), self.target.clone());
        let scene = self.composite.canvas_after(vec![target.texture()],
            move |b, p, f| p.effect_to(b, &target, &pass, &[], f),
            move |_| {
                let t = now();
                Pose::at(t, moved.map_or(-1.0, |m| t - m)).0
            }).class("scene");
        // no UI: a click anywhere is a move
        html()
            .child(head().child(style().child(CSS)))
            .child(body().child(scene).on_click(cx.listener(|this, _, _, cx| {
                this.moved = Some(now());
                cx.notify();
            })))
    }
}

/// Renders GPU frames off-screen through a move, then times each pass.
fn frames(dir: &str) {
    let (scene, fx) = shaders();
    let rt = target();
    let frame = |gpu: &mut GpuTarget, f: &[f32]| gpu.draw_to(&rt, scene.effect(), &[], f) && gpu.draw(fx.effect(), &[rt.texture()], f);
    let stills = [("intro", 1.2, -1.0), ("t09", 9.0, -1.0), ("lift", 9.2, 0.2), ("impact", 9.4, 0.4), ("fallen", 10.3, 1.3), ("after", 15.0, 6.0)]
        .map(|(name, t, age)| (name, Pose::at(t, age).0));
    let start = std::time::Instant::now();
    let Some(mut gpu) = gpu_stills(dir, 1280, 720, stills, frame) else {
        eprintln!("scene shader: {:?}, composite shader: {:?}", scene.error(), fx.error());
        return;
    };
    // the first frame compiles both shaders: this is the wait before the first picture
    eprintln!("compile + 6 stills: {:.1} s", start.elapsed().as_secs_f64());
    // the cost of each pass once compiled (pixels() waits for the GPU)
    let time = |gpu: &mut GpuTarget, what: &str, f: &dyn Fn(&mut GpuTarget, &[f32]) -> bool| {
        let start = std::time::Instant::now();
        for i in 0..30 {
            f(gpu, &Pose::at(5.0 + i as f32 / 60.0, -1.0).0);
            let _ = gpu.pixels();
        }
        eprintln!("{what}: {:.1} ms", start.elapsed().as_secs_f64() * 1000.0 / 30.0);
    };
    time(&mut gpu, "scene pass", &|g, f| g.draw_to(&rt, scene.effect(), &[], f));
    time(&mut gpu, "composite pass", &|g, f| g.draw(fx.effect(), &[rt.texture()], f));
    time(&mut gpu, "1280x720 frame", &frame);
}

fn main() {
    if let Some(dir) = arg("--frames") {
        frames(&dir);
        return;
    }
    run(1280.0, 720.0, "BUI · Gambit", || {
        let (scene, composite) = shaders();
        Gambit { scene, composite, target: Rc::new(target()), moved: None, reported: false }
    });
}

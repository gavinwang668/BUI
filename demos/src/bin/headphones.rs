//! Headphones: a split light / dark controller for a pair of Bluetooth headphones on BUI. The
//! whole control surface is built once and laid out twice, a light copy clipped to the left of a
//! seam and a dark copy clipped to the right of it, so anything that straddles the seam (the mode
//! pill, the faders, the filter box) is half one look and half the other, and dragging the grip
//! on the seam slides the line through every control.
//!
//! Controls: LEFT and RIGHT volume knobs (drag up / down or scroll; the light side's knob carries
//! a handle on its arc, the dark side's a glowing arc), LINK (the knobs move together, keeping
//! their offset), ANC / AMBIENT, SPATIAL, GAIN and MIX faders over dotted scales, SYNC (copy one
//! ear to the other), a LPASS / HPASS pair on one track, presets on ‹ ›, power, and a live stereo
//! meter in the header tab. Readouts use tabular figures.
//!
//! Motion: on first light the seam sweeps in from the left edge and the knobs wind up from zero;
//! a preset or SYNC glides every value there in 700 ms (ease-out, and a drag takes over mid-way);
//! the preset name slides in from the side you stepped to; presses sink to `scale(0.96)`;
//! power fades the surface and winds the knobs back up when it returns.
//!
//! Keys: ← / → presets, ↑ / ↓ both ears, L link, A mode, S spatial, P power.
//!
//! `cargo run --release --bin headphones`
//! `-- --storyboard DIR` drags, scrolls and clicks through it headlessly and saves frames along
//! BUI's virtual clock.

use std::sync::OnceLock;

use blenderui::prelude::*;
use bui_demos::canvas::{at, rem, rem_css, Fit};
use bui_demos::control::{draggable, Axis, Dial};
use bui_demos::ease::{ease_out, expo_out, lerp, progress};
use bui_demos::pen::{mix, Pen};
use bui_demos::{arg, focus_root, now, run_with, size_arg, story::story, Script};

const DESIGN: (f32, f32) = (1600.0, 900.0);
/// The device's inside width (design px, 764 tall); everything below is placed in it.
const W: f32 = 1484.0;
const KNOBS: [(f32, f32); 2] = [(340.0, 400.0), (1144.0, 400.0)];
/// Knob sweep: from (degrees clockwise from 12) and through.
const FROM: f32 = -135.0;
const SWEEP: f32 = 270.0;
const ARC_R: f32 = 186.0;
const DOT_R: f32 = 152.0;
/// The faders' track: x from, x to; rows' y.
const TRACK: (f32, f32) = (612.0, 872.0);
const GAIN_Y: f32 = 372.0;
const MIX_Y: f32 = 476.0;
const PASS_Y: f32 = 738.0;
/// Pointer travel (design px) over a whole knob.
const KNOB_TRAVEL: f32 = 320.0;
const GLIDE: f32 = 0.7;

/// The dials, by index.
const LEFT: usize = 0;
const RIGHT: usize = 1;
const GAIN: usize = 2;
const MIX: usize = 3;
const LPASS: usize = 4;
const HPASS: usize = 5;
const SEAM: usize = 6;
const DIALS: usize = 7;

struct Preset {
    name: &'static str,
    /// left, right, gain, mix, lpass, hpass
    v: [f32; 6],
}

const PRESETS: [Preset; 4] = [
    Preset { name: "STUDIO", v: [0.62, 0.62, 0.5, 0.42, 0.08, 0.92] },
    Preset { name: "BASS BOOST", v: [0.78, 0.74, 0.72, 0.66, 0.02, 0.58] },
    Preset { name: "PODCAST", v: [0.48, 0.52, 0.4, 0.22, 0.28, 0.8] },
    Preset { name: "NIGHT", v: [0.3, 0.3, 0.32, 0.5, 0.12, 0.66] },
];

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; overflow: hidden; user-select: none; background: #ffffff;
       font-family: 'Segoe UI', Inter, system-ui, sans-serif; -webkit-font-smoothing: antialiased; }
.abs { position: absolute; }
.device { width: 1500px; height: 780px; border: 8px solid #0d0d0d; border-radius: 90px; box-sizing: border-box;
          box-shadow: 0 40px 70px rgba(0,0,0,0.18), 0 8px 18px rgba(0,0,0,0.12);
          overflow: hidden; animation: rise 700ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.half { top: 0; height: 764px; overflow: hidden; }
.half.l { left: 0; background: #f6f6f6; }
.half.r { background: #464646; }
.layer { top: 0; width: 1484px; height: 764px;
         --ink: #1c1c1c; --mute: #9c9c9c; --b1: #ffffff; --b2: #e6e6e6; --bink: #222; --pip: transparent;
         --edge: #2a2a2a; --led: #1c1c1c; color: var(--ink); }
.layer.dark { --ink: #f4f4f4; --mute: #b8b8b8; --b1: #5c5c5c; --b2: #2f2f2f; --bink: #f0f0f0; --pip: #ffffff;
              --edge: #111; --led: #ffffff; }
.paint { left: 0; top: 0; width: 1484px; height: 764px; }
.ctl { transition-property: opacity; transition-duration: 400ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.off .ctl { opacity: 0.35; }

.tab { left: 542px; top: -8px; width: 400px; height: 78px; border: 3px solid #0d0d0d; border-top: 0;
       border-radius: 0 0 26px 26px; box-sizing: border-box; }
.tab .name { position: absolute; left: 20px; top: 14px; font-size: 12px; letter-spacing: 2px; font-weight: 600; }
.tab .batt { position: absolute; right: 20px; top: 14px; font-size: 12px; color: var(--mute); font-variant-numeric: tabular-nums; }
.box { left: 542px; top: 652px; width: 400px; height: 124px; border: 3px solid #0d0d0d; border-bottom: 0;
       border-radius: 26px 26px 0 0; box-sizing: border-box; }

.circ { width: 48px; height: 48px; border-radius: 24px; border: 2px solid var(--edge); box-sizing: border-box; cursor: pointer;
        transition-property: transform, background-color; transition-duration: 160ms; }
.circ:hover { background: rgba(128,128,128,0.12); }
.circ:active, .btn:active { transform: scale(0.96); }
.glyph { position: absolute; left: 10px; top: 10px; width: 24px; height: 24px; }

.btn { height: 46px; border: 2px solid var(--edge); border-radius: 14px; box-sizing: border-box; cursor: pointer;
       background: linear-gradient(180deg, var(--b1) 0%, var(--b2) 100%); color: var(--bink);
       font-size: 14px; letter-spacing: 1.5px; font-weight: 500; display: flex; align-items: center; justify-content: center; gap: 8px;
       box-shadow: 0 6px 12px rgba(0,0,0,0.16), inset 0 1px 0 rgba(255,255,255,0.35);
       transition-property: transform, box-shadow; transition-duration: 160ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.btn.on { box-shadow: inset 0 3px 8px rgba(0,0,0,0.35), 0 1px 0 rgba(255,255,255,0.2); }
.led { width: 7px; height: 7px; border-radius: 4px; background: rgba(128,128,128,0.35);
       transition-property: background-color, box-shadow; transition-duration: 200ms; }
.on .led { background: var(--led); box-shadow: 0 0 8px var(--led); }
.pill { left: 580px; top: 96px; width: 324px; height: 46px; border: 2px solid var(--edge); border-radius: 23px; box-sizing: border-box;
        background: linear-gradient(180deg, var(--b1) 0%, var(--b2) 100%); box-shadow: 0 6px 12px rgba(0,0,0,0.16); }
.seg { position: absolute; top: 4px; width: 154px; height: 34px; border-radius: 17px; font-size: 14px; letter-spacing: 1.5px;
       color: var(--mute); display: flex; align-items: center; justify-content: center; cursor: pointer;
       transition-property: color; transition-duration: 200ms; }
.seg.on { color: var(--bink); }
.thumb { top: 4px; width: 154px; height: 34px; border-radius: 17px; background: rgba(128,128,128,0.22);
         box-shadow: inset 0 2px 6px rgba(0,0,0,0.25);
         transition-property: transform; transition-duration: 320ms; transition-timing-function: cubic-bezier(0.34, 1.4, 0.64, 1); }

.khit { width: 400px; height: 400px; border-radius: 200px; cursor: grab; }
.kb { left: 105px; top: 105px; width: 190px; height: 190px; border-radius: 95px;
      background: linear-gradient(160deg, #3c3c3c 0%, #121212 100%);
      box-shadow: 0 22px 36px rgba(0,0,0,0.38), 0 4px 8px rgba(0,0,0,0.3), inset 0 1px 1px rgba(255,255,255,0.18);
      transition-property: transform; transition-duration: 160ms; }
.grab .kb { transform: scale(0.98); }
.kf { left: 20px; top: 20px; width: 150px; height: 150px; border-radius: 75px;
      background: linear-gradient(165deg, #6e6e6e 0%, #3a3a3a 55%, #2a2a2a 100%);
      box-shadow: inset 0 2px 2px rgba(255,255,255,0.28), inset 0 -6px 12px rgba(0,0,0,0.35), 0 3px 8px rgba(0,0,0,0.5); }
.rot { left: 0; top: 0; width: 150px; height: 150px; }
.pip { left: 69px; top: 14px; width: 12px; height: 12px; border-radius: 6px; background: var(--pip);
       box-shadow: 0 0 10px var(--pip); }
.klabel { width: 300px; text-align: center; font-size: 26px; letter-spacing: 2px; font-weight: 400; }
.kval { width: 300px; text-align: center; font-size: 15px; color: var(--mute); font-variant-numeric: tabular-nums; }

.flabel { left: 612px; width: 260px; text-align: center; font-size: 13px; letter-spacing: 3px; }
.flabel span { color: var(--mute); letter-spacing: 0.5px; margin-left: 8px; font-variant-numeric: tabular-nums; }
.fhit { left: 596px; width: 292px; height: 44px; cursor: ew-resize; }
.handle { top: 9px; width: 34px; height: 26px; border-radius: 8px;
          background: linear-gradient(180deg, #6a6a6a 0%, #2c2c2c 100%); border: 1px solid #0d0d0d; box-sizing: border-box;
          box-shadow: 0 4px 8px rgba(0,0,0,0.35), inset 0 1px 0 rgba(255,255,255,0.25); }
.gl { position: absolute; left: 10px; top: 11px; width: 12px; height: 2px; border-radius: 1px; background: rgba(255,255,255,0.7); }
.phit { top: 716px; width: 48px; height: 44px; cursor: ew-resize; }
.phit .handle { left: 7px; }
.hz { top: 728px; font-size: 12px; color: var(--mute); }
.plabel { top: 668px; width: 140px; text-align: center; font-size: 13px; letter-spacing: 2px; }
.plabel span { display: block; font-size: 12px; letter-spacing: 0.5px; color: var(--mute); margin-top: 2px; font-variant-numeric: tabular-nums; }

.preset { left: 1060px; top: 34px; width: 264px; text-align: right; }
.preset small { display: block; font-size: 11px; letter-spacing: 2px; color: var(--mute); font-variant-numeric: tabular-nums; }
.preset b { display: block; font-size: 19px; font-weight: 600; letter-spacing: 2px; }
.fromr { animation: fromr 380ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.froml { animation: froml 380ms cubic-bezier(0.2, 0, 0, 1) backwards; }

.grip { left: 1250px; top: 0; width: 40px; height: 40px; border-radius: 20px; background: #0d0d0d; cursor: ew-resize;
        box-shadow: 0 6px 14px rgba(0,0,0,0.3); }
.grip .glyph { left: 8px; top: 8px; }
.cap { top: 832px; width: 320px; text-align: center; font-size: 12px; letter-spacing: 1px; color: #9a9a9a; }

@keyframes rise { from { opacity: 0; transform: translateY(20px); } to { opacity: 1; transform: translateY(0); } }
@keyframes fromr { from { opacity: 0; transform: translateX(18px); } to { opacity: 1; transform: translateX(0); } }
@keyframes froml { from { opacity: 0; transform: translateX(-18px); } to { opacity: 1; transform: translateX(0); } }
";

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

/// Values gliding to a target: where each started, where it goes, when.
struct Glide {
    from: [f32; DIALS],
    to: [f32; DIALS],
    at: f32,
    secs: f32,
}

/// What the headphones do with the room.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Listen {
    Anc,
    Ambient,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

struct Phones {
    d: [Dial; DIALS],
    glide: Option<Glide>,
    preset: usize,
    /// Preset steps so far: keys the name's slide, which comes from the side stepped to.
    picks: u32,
    from: Side,
    link: bool,
    /// RIGHT - LEFT when LINK went on.
    offset: f32,
    listen: Listen,
    spatial: bool,
    power: bool,
    frames: u32,
}

/// The accessor `draggable` takes for dial `i`.
fn dial(i: usize) -> fn(&mut Phones) -> &mut Dial {
    match i {
        LEFT => |p| &mut p.d[LEFT],
        RIGHT => |p| &mut p.d[RIGHT],
        GAIN => |p| &mut p.d[GAIN],
        MIX => |p| &mut p.d[MIX],
        LPASS => |p| &mut p.d[LPASS],
        HPASS => |p| &mut p.d[HPASS],
        _ => |p| &mut p.d[SEAM],
    }
}

/// What a drag on dial `i` does besides moving it: a linked ear follows.
fn moved(i: usize) -> fn(&mut Phones) {
    match i {
        LEFT => |p| p.follow(LEFT),
        RIGHT => |p| p.follow(RIGHT),
        _ => |p| p.glide = None,
    }
}

/// 20 Hz .. 20 kHz on a log scale.
fn hz(v: f32) -> String {
    let f = 20.0 * 1000f32.powf(v);
    if f >= 1000.0 { format!("{:.1} kHz", f / 1000.0) } else { format!("{f:.0} Hz") }
}

/// A volume as dB below full (−∞ at the bottom).
fn db(v: f32) -> String {
    if v <= 0.001 { "−∞ dB".to_string() } else { format!("{:.1} dB", -48.0 * (1.0 - v)) }
}

impl Phones {
    fn new() -> Self {
        let mut d = [Dial::new(0.0); DIALS];
        // first light: the knobs wind up and the seam sweeps in, gliding from zero
        d[LPASS].value = PRESETS[0].v[LPASS];
        d[HPASS].value = PRESETS[0].v[HPASS];
        let mut p = Self { d, glide: None, preset: 0, picks: 0, from: Side::Right, link: false, offset: 0.0, listen: Listen::Anc,
                           spatial: true, power: true, frames: 0 };
        p.glide_to(&PRESETS[0].v, 1.4);
        if let Some(g) = &mut p.glide {
            g.to[SEAM] = 0.5;
        }
        p
    }

    fn values(&self) -> [f32; DIALS] {
        self.d.map(|d| d.value)
    }

    /// Glide the six controls to `v` over `secs` (the seam stays where it was put).
    fn glide_to(&mut self, v: &[f32; 6], secs: f32) {
        let from = self.values();
        let mut to = from;
        to[..6].copy_from_slice(v);
        self.glide = Some(Glide { from, to, at: now(), secs });
    }

    fn step_preset(&mut self, to: Side) {
        let n = PRESETS.len();
        self.preset = (self.preset + if to == Side::Left { n - 1 } else { 1 }) % n;
        self.picks += 1;
        self.from = to;
        self.glide_to(&PRESETS[self.preset].v, GLIDE);
    }

    /// The ear `i` moved: with LINK on, the other keeps its offset.
    fn follow(&mut self, i: usize) {
        self.glide = None;
        if self.link {
            let v = self.d[i].value;
            if i == LEFT { self.d[RIGHT].set(v + self.offset) } else { self.d[LEFT].set(v - self.offset) };
        }
    }

    fn toggle_link(&mut self) {
        self.link = !self.link;
        self.offset = self.d[RIGHT].value - self.d[LEFT].value;
    }

    /// SYNC: the other ear glides to ear `i`.
    fn sync(&mut self, i: usize) {
        let mut v = [0.0; 6];
        v.copy_from_slice(&self.values()[..6]);
        v[1 - i] = v[i];
        self.offset = 0.0;
        self.glide_to(&v, GLIDE);
    }

    fn toggle_power(&mut self) {
        self.power = !self.power;
        if self.power {
            // winds back up from zero
            let mut v = [0.0; 6];
            v.copy_from_slice(&self.values()[..6]);
            self.d[LEFT].value = 0.0;
            self.d[RIGHT].value = 0.0;
            self.glide_to(&v, 1.0);
        }
    }

    fn nudge(&mut self, by: f32) {
        self.glide = None;
        for i in [LEFT, RIGHT] {
            let v = self.d[i].value + by;
            self.d[i].set(v);
        }
    }

    fn key(&mut self, key: &str) -> bool {
        match key {
            "Left" => self.step_preset(Side::Left),
            "Right" => self.step_preset(Side::Right),
            "Up" => self.nudge(0.04),
            "Down" => self.nudge(-0.04),
            "l" | "L" => self.toggle_link(),
            "a" | "A" => self.listen = if self.listen == Listen::Anc { Listen::Ambient } else { Listen::Anc },
            "s" | "S" => self.spatial = !self.spatial,
            "p" | "P" => self.toggle_power(),
            _ => return false,
        }
        true
    }

    fn tick(&mut self, t: f32) {
        if let Some(g) = &self.glide {
            let k = if g.secs > 1.0 { expo_out(progress(t, g.at, g.at + g.secs)) } else { ease_out(progress(t, g.at, g.at + g.secs)) };
            for i in 0..DIALS {
                if !self.d[i].dragging() {
                    self.d[i].value = lerp(g.from[i], g.to[i], k);
                }
            }
            if t >= g.at + g.secs {
                self.glide = None;
            }
        }
    }

    /// The stereo level now (0..1 per ear): a beat and a shimmer, scaled by the volumes.
    fn level(&self, t: f32) -> [f32; 2] {
        if !self.power {
            return [0.0; 2];
        }
        let beat = (1.0 - (t * 2.1).fract()).powi(3);
        let body = 0.45 + 0.25 * (t * 7.3).sin() * (t * 1.7).cos() + 0.3 * beat;
        let gain = 0.6 + 0.8 * self.d[GAIN].value;
        [
            (body * gain * self.d[LEFT].value * (0.92 + 0.08 * (t * 13.0).sin())).clamp(0.0, 1.0),
            (body * gain * self.d[RIGHT].value * (0.92 + 0.08 * (t * 11.0).cos())).clamp(0.0, 1.0),
        ]
    }
}

/// Everything painted: knob arcs and dot rings, fader scales and tracks, the meter. `dark`
/// picks the look; drawn in the layer's own box (`b`, the device inside).
fn paint(b: Bounds<f32>, p: &mut Painter, dark: bool, v: [f32; DIALS], dim: f32, level: [f32; 2]) {
    let mut pen = Pen::new(b, p, W);
    let none = Rgba::TRANSPARENT;
    let ground = if dark { rgb(0x464646) } else { rgb(0xf6f6f6) };
    let a = |c: Rgba, k: f32| mix(ground, c, k);
    let (ink, faint, dot) = if dark { (rgb(0xffffff), rgb(0x141414), rgb(0xe6e6e6)) } else { (rgb(0x141414), rgb(0xcfcfcf), rgb(0x1c1c1c)) };
    let ink = a(ink, dim);

    for (i, &c) in KNOBS.iter().enumerate() {
        let to = FROM + SWEEP * v[i];
        // the dot ring: a dot every 9 degrees round the sweep
        for d in 0..=30 {
            let (s, co) = (FROM + d as f32 * 9.0).to_radians().sin_cos();
            let lit = (d as f32) / 30.0 <= v[i];
            pen.disc((c.0 + DOT_R * s, c.1 - DOT_R * co), if d % 5 == 0 { 2.4 } else { 1.7 },
                     a(dot, if lit { dim } else { 0.35 * dim }), none, 0.0);
        }
        if dark {
            pen.arc(c, ARC_R, (to, FROM + SWEEP), faint, 5.0);
            pen.glow_arc(c, ARC_R, (FROM, to), ink, 3.5, 4);
        } else {
            pen.arc(c, ARC_R, (to, FROM + SWEEP), faint, 2.0);
            pen.arc(c, ARC_R, (FROM, to), ink, 3.5);
            // the handle riding the arc
            let (s, co) = to.to_radians().sin_cos();
            let h = (c.0 + ARC_R * s, c.1 - ARC_R * co);
            pen.disc((h.0, h.1 + 3.0), 19.0, Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.18 }, none, 0.0);
            pen.disc(h, 18.0, rgb(0x111111), none, 0.0);
            pen.disc(h, 14.0, rgb(0x4a4a4a), none, 0.0);
            pen.disc((h.0 - 2.0, h.1 - 3.0), 6.0, Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.12 }, none, 0.0);
        }
    }

    let fx = |u: f32| TRACK.0 + (TRACK.1 - TRACK.0) * u;
    for (y, lo, hi) in [(GAIN_Y, 0.0, v[GAIN]), (MIX_Y, 0.0, v[MIX]), (PASS_Y, v[LPASS].min(v[HPASS]), v[LPASS].max(v[HPASS]))] {
        // the dotted scale over the fader
        for i in 0..=13 {
            pen.disc((fx(i as f32 / 13.0), y - 24.0), if i % 13 == 0 || i == 6 { 2.2 } else { 1.6 }, a(dot, 0.8 * dim), none, 0.0);
        }
        pen.line([(TRACK.0, y), (TRACK.1, y)], faint, if dark { 4.0 } else { 2.0 });
        let seg = [(fx(lo), y), (fx(hi), y)];
        if dark {
            for g in (1..=3).rev() {
                pen.line(seg, a(ink, 0.1 / g as f32), 3.0 + g as f32 * 4.0);
            }
        }
        pen.line(seg, ink, 3.5);
    }

    // the meter in the header tab: 30 cells an ear
    for (e, &l) in level.iter().enumerate() {
        let y = 40.0 + e as f32 * 12.0;
        for c in 0..30 {
            let lit = (c as f32 + 0.5) / 30.0 < l;
            let col = if lit { if c >= 26 { a(rgb(0xff5a36), dim.max(0.6)) } else { ink } } else { a(dot, 0.18) };
            pen.quad((562.0 + c as f32 * 12.0, y), (9.0, 6.0), 1.5, col);
        }
    }
}

/// A 24-unit glyph: power, chevrons, the seam grip's arrows.
#[derive(Clone, Copy)]
enum Glyph {
    Power,
    Prev,
    Next,
    Grip,
}

fn glyph(g: Glyph, color: Rgba) -> Div {
    canvas(move |b, p| {
        let mut pen = Pen::new(b, p, 24.0);
        let w = 1.8;
        match g {
            Glyph::Power => {
                pen.arc((12.0, 13.0), 8.0, (35.0, 325.0), color, w);
                pen.line([(12.0, 3.0), (12.0, 11.0)], color, w);
            }
            Glyph::Prev => pen.line([(14.5, 6.0), (8.5, 12.0), (14.5, 18.0)], color, w),
            Glyph::Next => pen.line([(9.5, 6.0), (15.5, 12.0), (9.5, 18.0)], color, w),
            Glyph::Grip => {
                pen.line([(8.0, 7.0), (3.0, 12.0), (8.0, 17.0)], color, w);
                pen.line([(16.0, 7.0), (21.0, 12.0), (16.0, 17.0)], color, w);
            }
        }
    })
    .class("glyph")
}

impl Phones {
    /// One copy of the control surface in `dark` or light; `side` keeps its ids apart.
    fn layer(&self, dark: bool, scale: f32, t: f32, cx: &mut Context<Self>) -> Div {
        let side = if dark { "r" } else { "l" };
        let v = self.values();
        let (dim, level) = (if self.power { 1.0 } else { 0.35 }, self.level(t));
        let ink = if dark { rgb(0xf4f4f4) } else { rgb(0x1c1c1c) };
        let mut l = div().class(if dark { "abs layer dark" } else { "abs layer" })
            .child(canvas(move |b, p| paint(b, p, dark, v, dim, level)).class("abs paint"));

        // header tab, power, presets
        l = l.child(div().class("abs tab ctl")
                .child(div().class("name").child(if self.listen == Listen::Ambient { "HALO ONE · AMBIENT" } else { "HALO ONE · ANC" }))
                .child(div().class("batt").child(if self.power { "L 82%  R 79%" } else { "standby" })))
            .child(at(div().id(format!("power-{side}")), 40.0, 40.0).class("circ").child(glyph(Glyph::Power, ink))
                .on_click(cx.listener(|this, _, _, cx| { this.toggle_power(); cx.notify(); })))
            .child(at(div().id(format!("preset{}-{side}", self.picks)), 1060.0, 34.0).class(match (self.picks, self.from) {
                    (0, _) => "preset ctl",
                    (_, Side::Left) => "preset ctl froml",
                    (_, Side::Right) => "preset ctl fromr",
                })
                .child(small().child(format!("PRESET {} / {}", self.preset + 1, PRESETS.len())))
                .child(b().child(PRESETS[self.preset].name)))
            .child(at(div().id(format!("prev-{side}")), 1340.0, 40.0).class("circ").child(glyph(Glyph::Prev, ink))
                .on_click(cx.listener(|this, _, _, cx| { this.step_preset(Side::Left); cx.notify(); })))
            .child(at(div().id(format!("next-{side}")), 1404.0, 40.0).class("circ").child(glyph(Glyph::Next, ink))
                .on_click(cx.listener(|this, _, _, cx| { this.step_preset(Side::Right); cx.notify(); })));

        // the top row: LINK, the mode pill, SPATIAL
        let btn = |id: &str, on: bool, label: &'static str, x: f32, w: f32| {
            at(div().id(format!("{id}-{side}")), x, 96.0).css("width", rem(w)).class(if on { "btn ctl on" } else { "btn ctl" })
                .child(div().class("led")).child(label)
        };
        l = l.child(btn("link", self.link, "LINK", 452.0, 108.0)
                .on_click(cx.listener(|this, _, _, cx| { this.toggle_link(); cx.notify(); })))
            .child(btn("spatial", self.spatial, "SPATIAL", 924.0, 128.0)
                .on_click(cx.listener(|this, _, _, cx| { this.spatial = !this.spatial; cx.notify(); })));
        let mut pill = div().class("abs pill ctl")
            .child(div().class("abs thumb").css("left", rem(4.0))
                .css("transform", format!("translateX({})", rem(if self.listen == Listen::Ambient { 162.0 } else { 0.0 }))));
        for (k, (label, mode)) in [("ANC", Listen::Anc), ("AMBIENT", Listen::Ambient)].into_iter().enumerate() {
            let on = self.listen == mode;
            pill = pill.child(div().id(format!("mode{k}-{side}")).class(if on { "seg on" } else { "seg" })
                .css("left", rem(4.0 + k as f32 * 162.0)).child(label)
                .on_click(cx.listener(move |this, _, _, cx| { this.listen = mode; cx.notify(); })));
        }
        l = l.child(pill);

        // the knobs
        for (i, &(kx, ky)) in KNOBS.iter().enumerate() {
            let grab = self.d[i].dragging();
            let knob = at(div().id(format!("knob{i}-{side}")), kx - 200.0, ky - 200.0)
                .class(if grab { "khit ctl grab" } else { "khit ctl" })
                .child(div().class("abs kb").child(div().class("abs kf")
                    .child(div().class("abs rot").css("transform", format!("rotate({}deg)", FROM + SWEEP * v[i]))
                        .child(div().class("abs pip")))));
            l = l.child(draggable(knob, cx, Axis::Up, KNOB_TRAVEL * scale, dial(i), moved(i)))
                .child(at(div(), kx - 150.0, 590.0).class("klabel ctl").child(if i == LEFT { "LEFT" } else { "RIGHT" }))
                .child(at(div(), kx - 150.0, 628.0).class("kval ctl")
                    .child(format!("{:.0}%  ·  {}", v[i] * 100.0, db(v[i]))))
                .child(btn(if i == LEFT { "sync0" } else { "sync1" }, false, "SYNC", kx - 55.0, 110.0).css("top", rem(682.0))
                    .on_click(cx.listener(move |this, _, _, cx| { this.sync(i); cx.notify(); })));
        }

        // GAIN and MIX
        let travel = (TRACK.1 - TRACK.0) * scale;
        let fx = |u: f32| TRACK.0 + (TRACK.1 - TRACK.0) * u;
        for (i, y, label, read) in [(GAIN, GAIN_Y, "GAIN", format!("{:+.1} dB", -12.0 + 24.0 * v[GAIN])),
                                    (MIX, MIX_Y, "MIX", format!("{:.0}%", v[MIX] * 100.0))] {
            l = l.child(at(div(), TRACK.0, y - 62.0).class("flabel ctl").child(label).child(span().child(read)));
            let fader = at(div().id(format!("fader{i}-{side}")), 596.0, y - 22.0).class("fhit ctl")
                .child(div().class("abs handle").css("left", rem(fx(v[i]) - 596.0 - 17.0)).child(div().class("gl")));
            l = l.child(draggable(fader, cx, Axis::Right, travel, dial(i), moved(i)));
        }

        // the filter box: LPASS and HPASS on one track
        l = l.child(div().class("abs box ctl"))
            .child(at(div(), 572.0, 668.0).class("plabel ctl").child("LPASS").child(span().child(hz(v[LPASS]))))
            .child(at(div(), 772.0, 668.0).class("plabel ctl").child("HPASS").child(span().child(hz(v[HPASS]))))
            .child(at(div(), 556.0, 728.0).class("hz ctl").child("0 Hz"))
            .child(at(div(), 882.0, 728.0).class("hz ctl").child("20 kHz"));
        for i in [LPASS, HPASS] {
            let h = at(div().id(format!("pass{i}-{side}")), fx(v[i]) - 24.0, 716.0).class("phit ctl")
                .child(div().class("abs handle").child(div().class("gl")));
            l = l.child(draggable(h, cx, Axis::Right, travel, dial(i), moved(i)));
        }
        l
    }
}

impl Render for Phones {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        focus_root(w, &mut self.frames);
        let t = now();
        self.tick(t);
        let fit = Fit::new(w, DESIGN);
        let seam = W * self.d[SEAM].value;
        // each half is a look on its own ground; the device's rounded `overflow` cuts their corners
        let device = at(div(), 50.0, 52.0).class(if self.power { "device" } else { "device off" })
            .child(at(div(), 0.0, 0.0).class("half l").css("width", rem(seam))
                .child(self.layer(false, fit.scale, t, cx).css("left", "0")))
            .child(at(div(), seam, 0.0).class("half r").css("width", rem(W - seam))
                .child(self.layer(true, fit.scale, t, cx).css("left", rem(-seam))));
        let grip = at(div().id("grip"), 50.0 + 8.0 + seam - 20.0, 52.0 + 780.0 - 28.0).class("grip")
            .child(glyph(Glyph::Grip, rgb(0xffffff)));
        let grip = draggable(grip, cx, Axis::Right, W * fit.scale, dial(SEAM), moved(SEAM));
        let stage = at(div(), fit.ox, fit.oy).css("width", rem(DESIGN.0)).css("height", rem(DESIGN.1))
            .child(device)
            .child(grip)
            .child(at(div(), 58.0 + seam - 160.0, 846.0).class("cap").child("drag the seam · ← → presets · L link"));
        html().css("font-size", fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root").attr("tabindex", "0")
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| if this.key(&e.keystroke.key) { cx.notify() }))
                .child(stage))
    }
}

/// Drags, scrolls and clicks through the controller headlessly.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        // the glides run on the wall clock: wait them out for real, then step CSS along
        s.play("00-enter", &[0, 300, 700, 1600]);
        let (x, y) = s.centre("knob0-l");
        s.cx.simulate_drag(s.win, (x, y), (x, y - 90.0));
        s.play("01-left-up", &[0]);
        s.click("link-l");
        let (x, y) = s.centre("knob1-r");
        s.cx.simulate_drag(s.win, (x, y), (x, y + 120.0));
        s.play("02-linked-down", &[0]);
        s.cx.simulate_scroll(s.win, x, y, 4);
        s.play("03-wheel", &[0]);
        let (x, y) = s.centre("fader2-l");
        s.cx.simulate_drag(s.win, (x, y), (x + 70.0, y));
        s.play("04-gain", &[0]);
        s.click("mode1-r");
        s.play("05-ambient", &[0, 120, 400]);
        s.click("next-r");
        s.play("06-preset", &[0, 150, 350, 800]);
        s.click("sync1-r");
        s.play("07-sync", &[0, 800]);
        let (x, y) = s.centre("grip");
        s.cx.simulate_drag(s.win, (x, y), (x - 300.0, y));
        s.play("08-seam", &[0]);
        let (x, y) = s.centre("pass5-r");
        s.cx.simulate_drag(s.win, (x, y), (x - 80.0, y));
        s.play("09-hpass", &[0]);
        s.click("power-l");
        s.play("10-off", &[0, 450]);
        s.click("power-l");
        s.play("11-on", &[0, 300, 1100]);
        s.key("Left");
        s.play("12-prev", &[0, 200, 800]);
    })
}

fn main() {
    let (w, h) = size_arg(DESIGN);
    run_with(w, h, "BUI · Headphones", Phones::new, arg("--storyboard").map(storyboard));
}

//! An After Effects-style motion editor on BUI: menu bar, project and effects panels, a composition
//! (a GPU shader background layer under `Painter`-drawn layers), effect controls whose sliders set
//! keyframes at the playhead, and a timeline with clips, keyframe diamonds and a scrubbable playhead.
//! `cargo run --release --bin editor`

use std::sync::Arc;

use bui_demos::{now, run, Fx};
use blenderui::prelude::*;

const DUR: f32 = 5.0;
const FPS: f32 = 24.0;
const COMP: (f32, f32) = (640.0, 360.0);
const ROW_H: f32 = 24.0;

/// A layer's transform: position in 0..1 of the comp, uniform scale, rotation in degrees.
#[derive(Clone, Copy)]
struct Xf {
    x: f32,
    y: f32,
    s: f32,
    r: f32,
}

#[derive(Clone, Copy)]
enum Shape {
    Rect(f32, f32),
    Circle(f32),
    Text(&'static str),
}

#[derive(Clone)]
struct Layer {
    name: &'static str,
    label: u32,
    fill: u32,
    shape: Shape,
    span: (f32, f32),
    keys: Vec<(f32, Xf)>,
}

impl Layer {
    /// The transform at `t`: smoothstep between the keyframes around it, held before and after.
    fn at(&self, t: f32) -> Xf {
        let k = &self.keys;
        let (first, last) = (k[0], k[k.len() - 1]);
        if t <= first.0 { return first.1; }
        if t >= last.0 { return last.1; }
        let i = k.windows(2).position(|w| t < w[1].0).unwrap_or(0);
        let ((a, xa), (b, xb)) = (k[i], k[i + 1]);
        let u = (t - a) / (b - a);
        let u = u * u * (3.0 - 2.0 * u);
        let l = |p: f32, q: f32| p + (q - p) * u;
        Xf { x: l(xa.x, xb.x), y: l(xa.y, xb.y), s: l(xa.s, xb.s), r: l(xa.r, xb.r) }
    }

    /// Sets a keyframe at `t` (the stopwatch is on for every property, as after a first key).
    fn key(&mut self, t: f32, f: impl FnOnce(&mut Xf)) {
        let mut v = self.at(t);
        f(&mut v);
        if let Some(i) = self.keys.iter().position(|k| (k.0 - t).abs() < 0.5 / FPS) {
            self.keys[i].1 = v;
        } else {
            let i = self.keys.partition_point(|k| k.0 < t);
            self.keys.insert(i, (t, v));
        }
    }
}

const fn xf(x: f32, y: f32, s: f32, r: f32) -> Xf {
    Xf { x, y, s, r }
}

fn layers() -> Vec<Layer> {
    vec![
        Layer { name: "nav icon", label: 0x9b8bd9, fill: 0xeeeeff, shape: Shape::Rect(54.0, 54.0), span: (0.0, 5.0),
                keys: vec![(0.0, xf(0.2, 0.2, 1.0, 0.0)), (1.4, xf(0.2, 0.2, 1.4, 90.0)), (3.2, xf(0.2, 0.2, 1.0, 180.0))] },
        Layer { name: "btn bg", label: 0x5aa0d8, fill: 0x2f6fb0, shape: Shape::Circle(64.0), span: (0.0, 5.0),
                keys: vec![(0.0, xf(0.2, 0.2, 1.0, 0.0)), (2.0, xf(0.5, 0.5, 1.6, 0.0)), (4.0, xf(0.8, 0.78, 1.0, 0.0))] },
        Layer { name: "unleash shrimpfish", label: 0x4caf50, fill: 0xffffff, shape: Shape::Text("UNLEASH · CROSS-MEDIA"), span: (0.5, 5.0),
                keys: vec![(0.5, xf(-0.3, 0.42, 1.0, 0.0)), (1.6, xf(0.5, 0.42, 1.0, 0.0)), (4.5, xf(1.3, 0.42, 1.0, 0.0))] },
        Layer { name: "divider", label: 0xe87d0d, fill: 0xe87d0d, shape: Shape::Rect(360.0, 6.0), span: (0.0, 5.0),
                keys: vec![(0.0, xf(0.5, 0.6, 0.0, 0.0)), (1.0, xf(0.5, 0.6, 1.0, 0.0)), (4.0, xf(0.5, 0.6, 1.0, 0.0)), (5.0, xf(0.5, 0.6, 0.0, 0.0))] },
        Layer { name: "selection box", label: 0x4a6fe0, fill: 0x4a6fe0, shape: Shape::Rect(180.0, 100.0), span: (1.8, 5.0),
                keys: vec![(1.8, xf(0.3, 0.75, 0.2, 0.0)), (3.0, xf(0.7, 0.3, 1.0, 15.0))] },
    ]
}

// Layer 6, "nav bg": a real GPU shader layer. U[0] = comp time (it scrubs with the playhead).
const NAV_BG: &str = "
vec4 effect(vec2 p) {
    vec2 uv = p / box_size;
    float w = 0.5 + 0.5 * sin(uv.x * 9.0 + U[0] * 2.0 + sin(uv.y * 5.0 + U[0]));
    vec3 a = vec3(0.16, 0.36, 0.55); vec3 b = vec3(0.55, 0.82, 0.95);
    vec3 c = mix(a, b, uv.y * 0.8 + w * 0.2);
    return vec4(c * (0.85 + 0.15 * w), 1.0);
}";

fn paint_comp(b: Bounds<f32>, p: &mut Painter, layers: &[Layer], t: f32, sel: usize) {
    let (ox, oy, sx) = (b.origin.x, b.origin.y, b.size.width / COMP.0);
    for (i, l) in layers.iter().enumerate().rev() {
        if t < l.span.0 || t > l.span.1 { continue; }
        let v = l.at(t);
        let (cx, cy) = (ox + v.x * b.size.width, oy + v.y * b.size.height);
        let (sin, cos) = v.r.to_radians().sin_cos();
        let quad = |w: f32, h: f32| -> [(f32, f32); 4] {
            let (hw, hh) = (w * v.s * sx / 2.0, h * v.s * sx / 2.0);
            [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].map(|(x, y)| (cx + x * cos - y * sin, cy + x * sin + y * cos))
        };
        let (w, h) = match l.shape {
            Shape::Rect(w, h) => {
                p.polygon(&quad(w, h), rgb(l.fill));
                (w, h)
            }
            Shape::Circle(d) => {
                let r = d * v.s * sx / 2.0;
                p.oval(Bounds { origin: point(cx - r, cy - r), size: size(r * 2.0, r * 2.0) }, rgb(l.fill), rgb(0xffffff), 1.0);
                (d, d)
            }
            Shape::Text(s) => {
                let fs = 22.0 * v.s * sx;
                p.text(cx - s.chars().count() as f32 * fs * 0.29, cy - fs * 0.6, s, fs, rgb(l.fill));
                (s.chars().count() as f32 * 13.0, 30.0)
            }
        };
        if i == sel {
            let mut q = quad(w + 10.0, h + 10.0).to_vec();
            q.push(q[0]);
            p.line(&q, rgb(0xe8b30d), 1.5);
            for &(x, y) in &q[..4] {
                p.fill(Bounds { origin: point(x - 3.0, y - 3.0), size: size(6.0, 6.0) }, rgb(0xe8b30d));
            }
        }
    }
}

fn paint_timeline(b: Bounds<f32>, p: &mut Painter, layers: &[Layer], t: f32, sel: usize) {
    let (ox, oy, w) = (b.origin.x, b.origin.y, b.size.width);
    let x_of = |s: f32| ox + s / DUR * w;
    p.fill(b, rgb(0x2a2a2a));
    p.fill(Bounds { origin: point(ox, oy), size: size(w, ROW_H) }, rgb(0x262626));
    for f in 0..=(DUR * 2.0) as i32 {
        let s = f as f32 * 0.5;
        let x = x_of(s);
        p.line(&[(x, oy + if f % 2 == 0 { 12.0 } else { 18.0 }), (x, oy + ROW_H)], rgb(0x777777), 1.0);
        if f % 2 == 0 { p.text(x + 3.0, oy + 2.0, &format!("{:02}:00f", f / 2), 10.0, rgb(0x999999)); }
    }
    for (i, l) in layers.iter().enumerate() {
        let y = oy + ROW_H * (i as f32 + 1.0);
        if i == sel { p.fill(Bounds { origin: point(ox, y), size: size(w, ROW_H) }, rgb(0x3a3f48)); }
        p.line(&[(ox, y + ROW_H), (ox + w, y + ROW_H)], rgb(0x1e1e1e), 1.0);
        p.quad(Bounds { origin: point(x_of(l.span.0), y + 3.0), size: size(x_of(l.span.1) - x_of(l.span.0), ROW_H - 6.0) },
               3.0, rgba((l.label << 8) | 0xbb), rgb(0x000000), 0.0);
        for k in &l.keys {
            let (x, c) = (x_of(k.0), y + ROW_H / 2.0);
            p.polygon(&[(x, c - 5.0), (x + 5.0, c), (x, c + 5.0), (x - 5.0, c)], rgb(0xe8b30d));
        }
    }
    // the shader layer's track
    let y = oy + ROW_H * (layers.len() as f32 + 1.0);
    p.quad(Bounds { origin: point(ox, y + 3.0), size: size(w, ROW_H - 6.0) }, 3.0, rgba(0xb6a7ffbb), rgb(0x000000), 0.0);
    let x = x_of(t);
    p.line(&[(x, oy), (x, oy + b.size.height)], rgb(0xe5534b), 1.0);
    p.polygon(&[(x - 6.0, oy), (x + 6.0, oy), (x, oy + 10.0)], rgb(0xe8b30d));
}

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; display: flex; flex-direction: column; height: 100%; background: #232323; color: #c8c8c8; font-size: 12px; }
.menu { display: flex; gap: 16px; height: 22px; align-items: center; padding: 0 10px; background: #2b2b2b; border-bottom: 1px solid #111; }
.menu b { color: #9b8bff; }
.top { flex: 1; display: flex; gap: 2px; padding: 2px; }
.pane { background: #2f2f2f; border: 1px solid #151515; overflow: auto; }
.pane h4 { margin: 0; font-weight: 500; padding: 4px 8px; background: #262626; color: #aaa; }
.item { padding: 2px 12px; }
.item:hover { background: #3a3a3a; }
.left { width: 210px; } .right { width: 250px; }
.view { flex: 1; display: flex; flex-direction: column; }
.stage { flex: 1; display: flex; align-items: center; justify-content: center; background: #161616; }
.comp { position: relative; width: 640px; height: 360px; }
.cv { position: absolute; left: 0; top: 0; width: 640px; height: 360px; }
.ctl { display: flex; gap: 8px; align-items: center; padding: 4px 8px; background: #262626; }
.tc { color: #e8b30d; font-family: monospace; font-size: 14px; width: 92px; }
.scrub { flex: 1; }
.fx { padding: 6px 12px; }
.fx .r { display: flex; justify-content: space-between; margin-top: 8px; color: #999; }
.fx .r span { color: #e8b30d; }
.tl { height: 196px; display: flex; margin: 0 2px 2px; border-top: 1px solid #111; }
.names { width: 260px; background: #2f2f2f; border-right: 1px solid #111; }
.lrow { height: 24px; display: flex; align-items: center; gap: 8px; padding: 0 8px; border-bottom: 1px solid #222; }
.lrow.sel { background: #41464f; }
.lrow small { margin-left: auto; color: #e8b30d; }
.sw { width: 10px; height: 10px; border-radius: 2px; }
.track { flex: 1; }
";

struct Editor {
    nav_bg: Fx,
    layers: Arc<Vec<Layer>>,
    sel: usize,
    t: f32,
    /// Wall clock and comp time when play started.
    playing: Option<(f32, f32)>,
}

impl Editor {
    fn set_t(&mut self, t: f32) {
        self.t = (t / (1.0 / FPS)).round() * (1.0 / FPS);
        self.t = self.t.clamp(0.0, DUR);
    }

    fn jump_key(&mut self, dir: f32) {
        let keys = &self.layers[self.sel].keys;
        let t = self.t;
        let next = if dir > 0.0 {
            keys.iter().map(|k| k.0).find(|&k| k > t + 1e-3)
        } else {
            keys.iter().rev().map(|k| k.0).find(|&k| k < t - 1e-3)
        };
        if let Some(k) = next { self.t = k; }
    }
}

impl Render for Editor {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some((wall, from)) = self.playing {
            self.t = (from + now() - wall) % DUR;
            w.request_animation_frame();
        }
        let (t, sel) = (self.t, self.sel);
        let frame = (t * FPS).round() as i32;
        let tc = format!("0:00:{:02}:{:02}", frame / FPS as i32, frame % FPS as i32);
        let cur = self.layers[sel].at(t);

        let (lc, lt) = (self.layers.clone(), self.layers.clone());
        let comp = div().class("comp")
            .child(self.nav_bg.canvas(move |_| vec![t]).class("cv"))
            .child(canvas(move |b, p| paint_comp(b, p, &lc, t, sel)).class("cv"));

        let mut names = div().class("names").child(div().class("lrow").child("#   Layer name"));
        for (i, l) in self.layers.iter().enumerate() {
            names = names.child(div().class(if i == sel { "lrow sel" } else { "lrow" })
                .child(div().class("sw").bg(rgb(l.label)))
                .child(format!("{}   {}", i + 1, l.name))
                .child(small().child(format!("{} ◆", l.keys.len())))
                .on_click(cx.listener(move |this, _, _, cx| { this.sel = i; cx.notify(); })));
        }
        names = names.child(div().class("lrow").child(div().class("sw").bg(rgb(0xb6a7ff)))
            .child(format!("{}   nav bg (GPU shader)", self.layers.len() + 1)));

        // effect controls: each slider keys its property at the playhead
        let prop = |name: &'static str, val: f32, lo: f32, hi: f32, set: fn(&mut Xf, f32)| {
            div().child(div().class("r").child(name).child(span().child(format!("{val:.2}"))))
                .child(input().input_type("range").min(f64::from(lo)).max(f64::from(hi)).step(f64::from((hi - lo) / 200.0))
                    .value(format!("{val}"))
                    .on_input(cx.listener(move |this: &mut Self, e: &ChangeEvent, _, cx| {
                        let (t, s) = (this.t, this.sel);
                        Arc::make_mut(&mut this.layers)[s].key(t, |x| set(x, e.number as f32));
                        cx.notify();
                    })))
        };
        let fx = div().class("fx").child(b().child(self.layers[sel].name))
            .child(prop("Position X", cur.x, -0.3, 1.3, |x, v| x.x = v))
            .child(prop("Position Y", cur.y, 0.0, 1.0, |x, v| x.y = v))
            .child(prop("Scale", cur.s, 0.0, 3.0, |x, v| x.s = v))
            .child(prop("Rotation", cur.r, -360.0, 360.0, |x, v| x.r = v));

        let item = |s: &'static str| div().class("item").child(s);
        html().styled()
            .child(head().child(style().child(CSS)))
            .child(body()
                .child(div().class("menu").child(b().child("Ae"))
                    .children(["File", "Edit", "Composition", "Layer", "Effect", "Animation", "View", "Window", "Help"].map(span_of)))
                .child(div().class("top")
                    .child(div().class("pane left").child(h4().child("Project"))
                        .child(item("▣ Main")).child(item("▣ nav-out")).child(item("▤ iphone image.psd")).child(item("▸ new-ui Layers"))
                        .child(h4().child("Effects & Presets"))
                        .children(["▸ 3D Channel", "▸ Blur & Sharpen", "▸ Color Correction", "▸ Distort", "▸ Generate", "▸ Keying", "▸ Noise & Grain", "▸ Stylize"].map(item)))
                    .child(div().class("pane view")
                        .child(div().class("stage").child(comp))
                        .child(div().class("ctl")
                            .child(button().child(if self.playing.is_some() { "❚❚" } else { "▶" })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.playing = if this.playing.is_some() { None } else { Some((now(), this.t)) };
                                    cx.notify();
                                })))
                            .child(button().child("◆◀").on_click(cx.listener(|this, _, _, cx| { this.jump_key(-1.0); cx.notify(); })))
                            .child(button().child("▶◆").on_click(cx.listener(|this, _, _, cx| { this.jump_key(1.0); cx.notify(); })))
                            .child(div().class("tc").child(tc))
                            .child(input().class("scrub").input_type("range").min(0.0).max(f64::from(DUR)).step(f64::from(1.0 / FPS))
                                .value(format!("{t}"))
                                .on_input(cx.listener(|this, e: &ChangeEvent, _, cx| {
                                    this.playing = None;
                                    this.set_t(e.number as f32);
                                    cx.notify();
                                })))
                            .child(span().child("50%  ·  Full  ·  Active Camera"))))
                    .child(div().class("pane right").child(h4().child("Effect Controls")).child(fx)))
                .child(div().class("tl")
                    .child(names)
                    .child(canvas(move |b, p| paint_timeline(b, p, &lt, t, sel)).class("track"))))
    }
}

fn span_of(s: &'static str) -> Div {
    span().child(s)
}

fn main() {
    run(1240.0, 680.0, "BUI · motion editor", || Editor {
        nav_bg: Fx::new("nav_bg", NAV_BG, 1, [80, 150, 200]),
        layers: Arc::new(layers()),
        sel: 1,
        t: 0.0,
        playing: None,
    });
}

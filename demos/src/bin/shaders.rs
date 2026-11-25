//! Three GPU shader components on BUI's `ShaderEffect`: a plasma button (hover), an aurora card
//! (follows the cursor) and a ripple field (click). `cargo run --release --bin shaders`

use bui_demos::{now, run, uv_in, Fx};
use blenderui::prelude::*;

// U[0] = time, U[1] = hover 0..1
const PLASMA: &str = "
vec4 effect(vec2 p) {
    vec2 q = p / box_size * 6.0;
    float t = U[0] * (1.0 + U[1] * 2.0);
    float v = sin(q.x + t) + sin(q.y * 1.3 - t) + sin((q.x + q.y) * 0.7 + t * 0.5)
            + sin(length(q - 3.0) * 1.5 - t * 1.2);
    vec3 c = 0.5 + 0.5 * cos(vec3(v) + vec3(0.0, 2.1, 4.2) + U[1] * 1.5);
    return vec4(c * (0.7 + 0.3 * U[1]), 1.0);
}";

// U[0] = time, U[1..2] = cursor uv
const AURORA: &str = "
float h(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5); }
float n(vec2 p) {
    vec2 i = floor(p); vec2 f = fract(p); f = f * f * (3.0 - 2.0 * f);
    return mix(mix(h(i), h(i + vec2(1.0, 0.0)), f.x), mix(h(i + vec2(0.0, 1.0)), h(i + vec2(1.0, 1.0)), f.x), f.y);
}
float fbm(vec2 p) { return 0.5 * n(p) + 0.25 * n(p * 2.0) + 0.125 * n(p * 4.0) + 0.0625 * n(p * 8.0); }
vec4 effect(vec2 p) {
    vec2 uv = p / box_size; uv.y = 1.0 - uv.y;
    vec2 m = vec2(U[1], U[2]);
    vec2 q = uv * vec2(box_size.x / box_size.y, 1.0) * 3.0;
    float a = fbm(q + U[0] * 0.1);
    float r = fbm(q + a * 2.0 + vec2(U[0] * 0.15, 0.0) + m * 2.0);
    vec3 c = mix(vec3(0.02, 0.05, 0.15), vec3(0.1, 0.9, 0.6), smoothstep(0.3, 0.8, r));
    c = mix(c, vec3(0.6, 0.2, 0.9), smoothstep(0.5, 1.0, a) * 0.7);
    c += 0.25 * smoothstep(0.5, 0.0, distance(uv, m));
    return vec4(c, 1.0);
}";

// U[0] = time, U[1] = click age (<0: none), U[2..3] = click uv
const RIPPLE: &str = "
vec4 effect(vec2 p) {
    vec2 uv = p / box_size; uv.y = 1.0 - uv.y;
    float asp = box_size.x / box_size.y;
    vec2 c = vec2(U[2], U[3]);
    float d = length((uv - vec2(0.5)) * vec2(asp, 1.0));
    float w = sin(d * 40.0 - U[0] * 4.0) * exp(-d * 4.0);
    float dk = length((uv - c) * vec2(asp, 1.0));
    float k = U[1] >= 0.0 ? sin(dk * 40.0 - U[1] * 30.0) * exp(-dk * 3.0 - U[1] * 2.0) : 0.0;
    float g = 0.5 + 0.5 * (w + k);
    vec3 col = mix(vec3(0.02, 0.1, 0.2), vec3(0.2, 0.8, 1.0), g * g);
    col += pow(max(0.0, 1.0 - d * 2.0), 3.0) * 0.3;
    return vec4(col, 1.0);
}";

const CSS: &str = r"
body { margin: 0; padding: 18px; background: #232323; }
.row { display: flex; gap: 18px; }
.card { position: relative; width: 300px; height: 250px; }
.cv { position: absolute; left: 0; top: 0; width: 300px; height: 250px; border-radius: 14px; }
.cap { position: absolute; left: 0; top: 96px; width: 300px; text-align: center; color: #fff; }
.cap h2 { margin: 0; font-size: 22px; }
.cap p { margin: 4px 0 0; font-size: 12px; opacity: 0.8; }
";

struct Shaders {
    plasma: Fx,
    aurora: Fx,
    ripple: Fx,
    hover: f32,
    hover_target: f32,
    mouse: (f32, f32),
    click: Option<(f32, f32, f32)>, // time, window x, y
    reported: bool,
}

impl Render for Shaders {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        self.hover += (self.hover_target - self.hover) * 0.1;
        if !self.reported && now() > 0.5 {
            self.reported = true;
            for (n, f) in [("plasma", &self.plasma), ("aurora", &self.aurora), ("ripple", &self.ripple)] {
                eprintln!("{n}: {:?}", f.error());
            }
        }
        let hover = self.hover;
        let (mx, my) = self.mouse;
        let click = self.click;
        let plasma = self.plasma.canvas(move |_| vec![now(), hover]).class("cv");
        let aurora = self.aurora.canvas(move |b| {
            let [u, v] = uv_in(b, mx, my);
            vec![now(), u, v]
        }).class("cv");
        let ripple = self.ripple.canvas(move |b| match click {
            Some((t0, x, y)) => {
                let [u, v] = uv_in(b, x, y);
                vec![now(), now() - t0, u, 1.0 - v]
            }
            None => vec![now(), -1.0, 0.5, 0.5],
        }).class("cv");
        let cap = |t: &'static str, s: &'static str| div().class("cap").child(h2().child(t)).child(p().child(s));
        html().styled()
            .child(head().child(style().child(CSS)))
            .child(body().child(div().class("row")
                .child(div().class("card")
                    .on_hover(cx.listener(|this, over: &bool, _, _| this.hover_target = f32::from(u8::from(*over))))
                    .child(plasma).child(cap("Plasma Button", "hover to speed up")))
                .child(div().class("card")
                    .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, _| this.mouse = (e.position.x.0, e.position.y.0)))
                    .child(aurora).child(cap("Aurora Card", "follows your cursor")))
                .child(div().class("card")
                    .on_any_mouse_down(cx.listener(|this, e: &MouseDownEvent, _, _| {
                        this.click = Some((now(), e.position.x.0, e.position.y.0));
                    }))
                    .child(ripple).child(cap("Ripple Field", "click to drop a wave")))))
    }
}

fn main() {
    run(990.0, 290.0, "BUI · shader components", || Shaders {
        plasma: Fx::new("plasma", PLASMA, 2, [120, 60, 200]),
        aurora: Fx::new("aurora", AURORA, 3, [20, 120, 100]),
        ripple: Fx::new("ripple", RIPPLE, 4, [20, 90, 140]),
        hover: 0.0,
        hover_target: 0.0,
        mouse: (0.0, 0.0),
        click: None,
        reported: false,
    });
}

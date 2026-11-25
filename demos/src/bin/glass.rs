//! Glass UI: a GPU shader paints moving light orbs, and the panels over it are real BUI CSS glass
//! (`backdrop-filter: blur() saturate()`, translucent fill, rim light) holding Blender widgets.
//! `cargo run --release --bin glass`

use bui_demos::{now, run, Fx};
use blenderui::prelude::*;

// U[0] = time. Orbs on a faint grid: plenty of edges and colour for the glass to blur.
const ORBS: &str = "
vec4 effect(vec2 p) {
    vec2 uv = p / box_size; float a = box_size.x / box_size.y;
    vec2 q = uv * vec2(a, 1.0);
    vec3 c = vec3(0.02, 0.03, 0.08);
    float t = U[0];
    c += vec3(1.0, 0.3, 0.5) * 0.06 / (0.02 + distance(q, vec2(a * (0.5 + 0.35 * sin(t * 0.3)), 0.5 + 0.3 * cos(t * 0.25))));
    c += vec3(0.2, 0.6, 1.0) * 0.06 / (0.02 + distance(q, vec2(a * (0.5 + 0.35 * sin(t * 0.3 + 1.7)), 0.5 + 0.3 * cos(t * 0.25 + 2.3))));
    c += vec3(0.4, 1.0, 0.5) * 0.06 / (0.02 + distance(q, vec2(a * (0.5 + 0.35 * sin(t * 0.3 + 3.4)), 0.5 + 0.3 * cos(t * 0.25 + 4.6))));
    c += vec3(1.0, 0.8, 0.2) * 0.06 / (0.02 + distance(q, vec2(a * (0.5 + 0.35 * sin(t * 0.3 + 5.1)), 0.5 + 0.3 * cos(t * 0.25 + 6.9))));
    float g = step(0.97, fract(uv.x * 30.0)) + step(0.97, fract(uv.y * 30.0 * (1.0 / a)));
    return vec4(c + 0.03 * g, 1.0);
}";

const CSS: &str = r"
body { margin: 0; width: 900px; height: 560px; position: relative; font-size: 13px; color: #fff; }
.bg { position: absolute; left: 0; top: 0; width: 900px; height: 560px; }
.glass { position: absolute; border-radius: 22px; padding: 18px 22px;
  background: rgba(255,255,255,0.12); border: 1px solid rgba(255,255,255,0.35);
  backdrop-filter: blur(22px) saturate(1.6);
  box-shadow: 0 12px 40px rgba(0,0,0,0.35), inset 0 1px 0 rgba(255,255,255,0.4); }
.glass h3 { margin: 0 0 10px; font-size: 15px; }
.row { display: flex; justify-content: space-between; margin: 6px 0; opacity: 0.9; }
#display { left: 50px; top: 50px; width: 300px; }
#clock { left: 560px; top: 70px; width: 270px; height: 190px; text-align: center; }
#clock .big { font-size: 56px; font-weight: 300; margin-top: 16px; }
#music { left: 140px; top: 300px; width: 360px; }
#dock { left: 560px; top: 330px; width: 270px; height: 56px; display: flex; gap: 18px; align-items: center; justify-content: center; border-radius: 28px; }
.ico { width: 34px; height: 34px; border-radius: 17px; background: rgba(255,255,255,0.18); text-align: center; line-height: 34px; }
.ico:hover { background: rgba(255,255,255,0.38); }
.bar { height: 6px; border-radius: 3px; background: rgba(255,255,255,0.25); margin-top: 10px; }
.bar div { width: 40%; height: 6px; border-radius: 3px; background: #fff; }
";

struct Glass {
    orbs: Fx,
    brightness: f32,
    wifi: bool,
    playing: bool,
}

impl Render for Glass {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        let bg = self.orbs.canvas(|_| vec![now()]).class("bg");
        let play = if self.playing { "❚❚" } else { "▶" };
        html().styled()
            .child(head().child(style().child(CSS)))
            .child(body()
                .child(bg)
                .child(div().id("display").class("glass").child(h3().child("Display"))
                    .child(div().class("row").child(span().child("Brightness")).child(span().child(format!("{:.0}%", self.brightness))))
                    .child(input().input_type("range").min(0.0).max(100.0).value(format!("{}", self.brightness))
                        .on_input(cx.listener(|this, e: &ChangeEvent, _, cx| { this.brightness = e.number as f32; cx.notify(); })))
                    .child(div().class("row").child(label().child(input().input_type("checkbox").checked(self.wifi)
                        .on_change(cx.listener(|this, e: &ChangeEvent, _, cx| { this.wifi = e.checked; cx.notify(); })))
                        .child(" Wi-Fi"))))
                .child(div().id("clock").class("glass").child(div().class("big").child("12:00"))
                    .child(p().child("backdrop-filter on a live GPU shader")))
                .child(div().id("music").class("glass").child(h3().child("Now playing"))
                    .child(div().class("row").child(span().child("Orbital Drift")).child(span().child("2:41 / 4:10")))
                    .child(div().class("bar").child(div()))
                    .child(div().class("row").child(button().child(play).on_click(cx.listener(|this, _, _, cx| { this.playing = !this.playing; cx.notify(); })))
                        .child(span().child("◀◀  ▶▶"))))
                .child(div().id("dock").class("glass")
                    .child(div().class("ico").child("☀")).child(div().class("ico").child("♫"))
                    .child(div().class("ico").child("✉")).child(div().class("ico").child("⚙"))))
    }
}

fn main() {
    run(900.0, 560.0, "BUI · glass UI", || Glass {
        orbs: Fx::new("orbs", ORBS, 1, [20, 30, 70]),
        brightness: 70.0,
        wifi: true,
        playing: true,
    });
}

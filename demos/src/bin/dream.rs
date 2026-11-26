//! Dreamlog: a Dreamcore dream journal on BUI. One GPU shader is the world: a pastel field under
//! a rainbow with a checkerboard floor running to the horizon, endless tiled pool rooms, and the
//! same field at night under a moon, all through a worn VHS tape (line jitter, a rolling tracking
//! band, colour fringing, grain). Eyes open in the clouds and follow the pointer; the clouds part
//! around it. Over it, soft pastel windows drift: the journal (type a dream, pick its mood, keep
//! it) and the sleep companion (a dream-depth dial that makes everything stranger, a lullaby,
//! and a wake-up button that never quite works). The door on the floor opens into the next place.
//!
//! Motion ("slow, slightly wrong"): windows bob on long sine loops, the caption's letters wave,
//! the door swings on its hinge and the scene crossfades through a white bloom (1.4 s); a kept
//! dream rises into the list with a glow; waking glitches the tape and the screen says no.
//!
//! Pointer: move (the eyes follow), click the sky (they blink), drag or wheel the dial, click the
//! door. Keys: type into the journal once its field is clicked (Enter keeps, Esc leaves);
//! otherwise ↑ / ↓ depth, D the door, W wake, Space blink.
//!
//! `cargo run --release --bin dream`
//! `-- --storyboard DIR` drives it headlessly (software renderer: the world is its flat colour);
//! `-- --frames DIR` renders the world on the GPU (each place, deep and shallow, a crossing, a wake).

use std::sync::OnceLock;

use blenderui::motion::MotionValue;
use blenderui::prelude::*;
use blenderui::vars;
use bui_demos::canvas::{at, rem, rem_css, Fit};
use bui_demos::control::{arc_points, draggable, glow_arc, Axis, Dial};
use bui_demos::ease::approach;
use bui_demos::text::{edit, Edit};
use bui_demos::{arg, focus_root, gpu_stills, now, run_with, story::story, uv_in, Fx, Script};

/// U[0] time, U[1..2] pointer uv (y up), U[3] place, U[4] next place, U[5] crossing 0..1,
/// U[6] depth 0..1, U[7] seconds since a wake (< 0: none), U[8] seconds since a click in the sky
/// (< 0: none), U[9..10] its uv.
const WORLD: &str = r"
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p) {
    vec2 i = floor(p); vec2 f = fract(p); f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}
float fbm(vec2 p) {
    float v = 0.0; float a = 0.5;
    for (int i = 0; i < 5; i++) { v += a * noise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; }
    return v;
}

// an almond eye at c, w wide, looking towards m; open 0..1. rgb, and its coverage in a
vec4 eye(vec2 q, vec2 c, float w, vec2 m, float open) {
    vec2 d = (q - c) / w;
    float lid = open * 0.44 * (1.0 - d.x * d.x);
    float within = step(abs(d.x), 1.0);
    float inside = smoothstep(0.03, -0.03, abs(d.y) - lid) * within;
    vec2 to = (m - c) / w;
    vec2 look = to / (1.0 + length(to)) * vec2(0.55, 0.3);
    vec2 ir = d - look;
    vec3 col = vec3(1.0, 0.97, 0.99);
    col = mix(col, vec3(0.62, 0.74, 1.0), smoothstep(0.31, 0.27, length(ir)));
    col = mix(col, vec3(0.42, 0.5, 0.95), smoothstep(0.2, 0.31, length(ir)) * smoothstep(0.31, 0.29, length(ir)));
    col = mix(col, vec3(0.07, 0.05, 0.14), smoothstep(0.13, 0.1, length(ir)));
    col = mix(col, vec3(1.0), smoothstep(0.06, 0.03, length(ir - vec2(-0.09, 0.09))));
    col = mix(col, vec3(0.85, 0.75, 0.9), smoothstep(0.0, -0.2, abs(d.y) - lid) * 0.0 + smoothstep(lid - 0.14, lid, abs(d.y)) * 0.35);
    float edge = smoothstep(0.06, 0.0, abs(abs(d.y) - lid)) * within * step(0.02, open);
    col = mix(col, vec3(0.36, 0.24, 0.48), edge * 0.85);
    return vec4(col, max(inside, edge * 0.85) * step(0.01, open + edge));
}

vec3 field(vec2 uv, vec2 q, float asp, vec2 m, float t, float depth, float night) {
    float hz = 0.38;
    vec3 top = mix(vec3(0.70, 0.77, 1.0), vec3(0.09, 0.09, 0.27), night);
    vec3 mid = mix(vec3(1.0, 0.82, 0.93), vec3(0.50, 0.33, 0.62), night);
    vec3 low = mix(vec3(1.0, 0.96, 0.91), vec3(0.96, 0.66, 0.76), night);
    float s = clamp((uv.y - hz) / (1.0 - hz), 0.0, 1.0);
    vec3 sky = mix(low, mid, smoothstep(0.0, 0.35, s));
    sky = mix(sky, top, smoothstep(0.3, 1.0, s));
    // stars and a moon at night
    vec2 sg = q * 70.0;
    float star = step(0.985, hash(floor(sg))) * smoothstep(0.32, 0.0, length(fract(sg) - 0.5))
               * (0.55 + 0.45 * sin(t * 2.0 + hash(floor(sg) + 3.0) * 30.0));
    sky += night * star * smoothstep(0.1, 0.5, s);
    float md = length(q - vec2(asp * 0.2, 0.8));
    sky += night * vec3(1.0, 0.8, 0.92) * 0.28 * exp(-md * 7.0);
    sky = mix(sky, vec3(1.0, 0.97, 0.9), night * smoothstep(0.072, 0.066, md));
    // the rainbow
    float rd = length(q - vec2(asp * 0.64, hz - 0.14));
    float band = (rd - 0.44) / 0.11;
    vec3 rb = mix(0.5 + 0.5 * cos(6.2831 * (band * 0.85 + vec3(0.0, 0.33, 0.67))), vec3(1.0), 0.42);
    float rw = smoothstep(0.0, 0.12, band) * smoothstep(1.0, 0.88, band) * smoothstep(hz, hz + 0.12, uv.y);
    sky = mix(sky, rb, rw * 0.5 * (1.0 - night * 0.65));
    // clouds: warped further the deeper you are, parting around the pointer
    vec2 away = q - m;
    float pd = length(away);
    vec2 cq = q * vec2(1.5, 3.2) + vec2(t * 0.02, 0.0) + away / (pd + 1e-3) * 0.3 * exp(-pd * 7.0);
    float warp = fbm(cq * 0.7 + t * 0.03) * (0.5 + depth * 1.8);
    float cl = fbm(cq + warp);
    float cloud = smoothstep(0.5, 0.78, cl) * smoothstep(hz, hz + 0.22, uv.y);
    vec3 cc = mix(vec3(1.0, 0.98, 1.0), vec3(0.78, 0.68, 0.96), night);
    cc = mix(cc, mix(vec3(1.0, 0.8, 0.9), vec3(0.48, 0.38, 0.7), night), smoothstep(0.75, 0.52, cl) * 0.6);
    sky = mix(sky, cc, cloud * 0.93);
    vec3 col = sky;
    // the checkerboard floor, running to the horizon (it creeps towards you when deep)
    if (uv.y < hz) {
        float dy = hz - uv.y;
        float z = 0.12 / dy;
        vec2 f = vec2((uv.x - 0.5) * asp * z, z + t * 0.12 * depth);
        float chk = mod(floor(f.x * 9.0) + floor(f.y * 9.0), 2.0);
        vec3 fl = mix(mix(vec3(1.0, 0.84, 0.93), vec3(0.42, 0.32, 0.6), night), mix(vec3(0.99, 0.98, 1.0), vec3(0.2, 0.17, 0.36), night), chk);
        fl *= 0.9 + 0.1 * smoothstep(0.0, 0.3, dy);
        col = mix(fl, low, exp(-dy * 10.0));
        // the pointer's glow, reflected
        col += vec3(1.0, 0.85, 0.95) * 0.1 * exp(-length(vec2(q.x - m.x, (uv.y - (2.0 * hz - m.y)) * 2.0)) * 8.0) * step(hz, m.y);
    }
    col += vec3(1.0, 0.86, 0.96) * 0.16 * exp(-pd * pd * 28.0);
    return col;
}

vec3 pool(vec2 uv, vec2 q, float asp, vec2 m, float t, float depth) {
    float hz = 0.42;
    vec3 col;
    vec2 dc = q - vec2(asp * 0.5, hz);
    if (uv.y >= hz) {
        vec2 g = vec2(q.x, uv.y) * 26.0;
        vec2 f = abs(fract(g) - 0.5);
        vec3 tile = mix(vec3(0.80, 0.95, 0.98), vec3(0.93, 1.0, 1.0), hash(floor(g)) * 0.35);
        col = mix(tile, vec3(0.62, 0.82, 0.86), smoothstep(0.44, 0.5, max(f.x, f.y)));
        col *= 0.88 + 0.16 * smoothstep(0.45, 1.0, uv.y);
        // a doorway in the far wall, lit from somewhere else
        float inside = max(step(abs(dc.x), 0.075) * step(dc.y, 0.2), step(length(dc - vec2(0.0, 0.2)), 0.075));
        vec3 beyond = mix(vec3(0.16, 0.36, 0.42), vec3(0.75, 0.98, 1.0), smoothstep(0.0, 0.28, dc.y) * 0.4 + 0.25 * sin(t * 0.5) * 0.2);
        col = mix(col, beyond, inside);
        // light caustics dancing on the wall
        vec2 zz = q * 9.0 + t * 0.25;
        float k = 0.0;
        for (int i = 0; i < 3; i++) { zz = vec2(zz.x + sin(zz.y + t * 0.6), zz.y + cos(zz.x - t * 0.5)); k += 1.0 / (1.0 + 25.0 * abs(sin(zz.x) * sin(zz.y))); }
        col += vec3(0.85, 1.0, 1.0) * pow(k / 3.0, 2.0) * 0.25 * (1.0 - inside) * smoothstep(0.75, 0.42, uv.y);
    } else {
        float dy = hz - uv.y;
        float z = 0.1 / dy;
        vec2 w = vec2((uv.x - 0.5) * asp * z, z);
        vec2 zz = w * 3.0 + t * 0.2;
        float k = 0.0;
        for (int i = 0; i < 3; i++) { zz = vec2(zz.x + sin(zz.y + t * 0.7), zz.y + cos(zz.x - t * 0.6)); k += 1.0 / (1.0 + 20.0 * abs(sin(zz.x) * sin(zz.y))); }
        vec3 water = mix(vec3(0.33, 0.83, 0.9), vec3(0.6, 0.95, 1.0), smoothstep(0.0, 0.3, dy));
        water += vec3(0.9, 1.0, 1.0) * pow(k / 3.0, 2.0) * 0.55;
        vec2 g = fract(w * 2.0 + 0.03 * sin(t + w.y * 4.0));
        water -= 0.06 * smoothstep(0.44, 0.5, max(abs(g.x - 0.5), abs(g.y - 0.5)));
        // the doorway, reflected and wobbling
        float rx = dc.x + 0.006 * sin(uv.y * 120.0 + t * 2.0);
        float refl = step(abs(rx), 0.075) * step(dy, 0.2);
        water = mix(water, vec3(0.2, 0.45, 0.5), refl * 0.5);
        col = mix(water, vec3(0.82, 0.96, 0.98), exp(-dy * 14.0));
    }
    col += vec3(0.8, 1.0, 1.0) * 0.14 * exp(-length(q - m) * length(q - m) * 28.0);
    return col;
}

vec3 eyes(vec3 c, float id, vec2 q, float asp, vec2 m, float t, float depth, float blink) {
    bool rooms = id > 0.5 && id < 1.5;
    for (int i = 0; i < 5; i++) {
        float fi = float(i);
        vec2 pos = vec2(asp * (0.1 + 0.2 * fi), 0.6 + 0.24 * fract(fi * 0.618 + 0.3));
        pos += vec2(sin(t * 0.3 + fi * 2.0), cos(t * 0.27 + fi)) * 0.012;
        float w = 0.034 + 0.026 * fract(fi * 0.37 + 0.5);
        float vis = smoothstep(0.08 + fi * 0.16, 0.22 + fi * 0.16, depth);
        if (rooms) {
            if (i > 0) break;
            pos = vec2(asp * 0.5, 0.53);
            w = 0.045;
            vis = smoothstep(0.3, 0.55, depth);
        }
        float ph = fract(t / (4.0 + fi * 1.3) + fi * 0.21);
        float open = smoothstep(0.0, 0.05, abs(ph - 0.97)) * vis;
        if (blink >= 0.0 && blink < 0.45) open *= smoothstep(0.0, 0.16, abs(blink - 0.16));
        vec4 e = eye(q, pos, w, m, open);
        c = mix(c, e.rgb, e.a * vis);
    }
    return c;
}

vec3 place(float id, vec2 uv, float asp, vec2 m, float t, float depth, float blink) {
    vec2 q = vec2(uv.x * asp, uv.y);
    vec3 c;
    if (id < 0.5) c = field(uv, q, asp, m, t, depth, 0.0);
    else if (id < 1.5) c = pool(uv, q, asp, m, t, depth);
    else c = field(uv, q, asp, m, t, depth, 1.0);
    return eyes(c, id, q, asp, m, t, depth, blink);
}

vec3 world(vec2 uv, float asp, vec2 m, float t, float depth, float blink) {
    float k = U[5];
    if (k <= 0.0) return place(U[3], uv, asp, m, t, depth, blink);
    if (k >= 1.0) return place(U[4], uv, asp, m, t, depth, blink);
    vec3 a = place(U[3], uv, asp, m, t, depth, blink);
    vec3 b = place(U[4], uv, asp, m, t, depth, blink);
    return mix(mix(a, b, smoothstep(0.35, 0.65, k)), vec3(1.0, 0.97, 1.0), sin(k * 3.14159) * 0.9);
}

vec4 effect(vec2 p) {
    vec2 uv = p / box_size; uv.y = 1.0 - uv.y;
    float asp = box_size.x / box_size.y;
    float t = U[0];
    vec2 m = vec2(U[1] * asp, U[2]);
    float depth = U[6];
    float wake = U[7] >= 0.0 ? exp(-U[7] * 2.5) : 0.0;
    // the tape: lines jitter, a tracking band rolls up, waking tears it
    float row = floor(p.y / 3.0);
    float jit = (hash(vec2(row, floor(t * 12.0))) - 0.5) * 0.003 * (0.3 + depth);
    float roll = smoothstep(0.03, 0.0, abs(fract(uv.y * 0.6 - t * 0.07) - 0.5));
    jit += roll * 0.008 * (0.4 + depth);
    jit += wake * (hash(vec2(row, floor(t * 30.0))) - 0.5) * 0.09;
    vec2 u = uv + vec2(jit, 0.0);
    float ca = 0.0012 + 0.004 * depth + wake * 0.02;
    float blink = U[8];
    vec3 c = vec3(world(u + vec2(ca, 0.0), asp, m, t, depth, blink).r,
                  world(u, asp, m, t, depth, blink).g,
                  world(u - vec2(ca, 0.0), asp, m, t, depth, blink).b);
    // soft bloom and a pastel lift
    c += max(c - 0.78, 0.0) * 0.7;
    c = mix(c, vec3(dot(c, vec3(0.333))), 0.08) * 0.93 + 0.07;
    c += roll * 0.05;
    // a ring where the sky was touched
    if (blink >= 0.0) {
        float d = length((uv - vec2(U[9], U[10])) * vec2(asp, 1.0));
        c += vec3(1.0, 0.9, 1.0) * smoothstep(0.012, 0.0, abs(d - blink * 0.5)) * exp(-blink * 2.2);
    }
    c = mix(c, vec3(1.0), wake * 0.85);
    c *= 0.965 + 0.035 * sin(p.y * 3.14159);
    c *= 1.0 - 0.32 * pow(length(uv - 0.5) * 1.25, 2.0);
    c += (hash(p + fract(t) * 91.0) - 0.5) * (0.05 + 0.05 * depth);
    return vec4(clamp(c, 0.0, 1.0), 1.0);
}
";

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; position: relative; overflow: hidden; background: #ffd6ec; user-select: none; color: #5b4676;
       font-family: Inter, system-ui, sans-serif; -webkit-font-smoothing: antialiased; font-size: 14px; }
.abs { position: absolute; }
.world { position: absolute; left: 0px; top: 0px; width: 100%; height: 100%; }
.kid { font-family: 'Comic Sans MS', Inter, sans-serif; }

/* -- the windows: soft pastel glass, drifting -- */
.drift { position: absolute; animation: bob 7s ease-in-out infinite alternate; }
.win { border-radius: 20px; background: rgba(255,249,253,0.8); backdrop-filter: blur(12px) saturate(1.4); overflow: hidden;
       box-shadow: 0 0 0 1.5px #fff, 0 0 0 4px rgba(196,166,236,0.5), 0 14px 30px rgba(120,80,170,0.22);
       animation: winin 760ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.bar { height: 36px; display: flex; align-items: center; gap: 8px; padding: 0 8px 0 14px;
       background: linear-gradient(90deg, #ffc9e7, #dccbff 55%, #c3e5ff); color: #6d4b93; font-weight: 700; font-size: 14px; }
.bar .lamp { width: 10px; height: 10px; border-radius: 5px; background: #fff; box-shadow: 0 0 6px #fff; animation: lamp 2.4s ease-in-out infinite; }
.x { margin-left: auto; width: 28px; height: 28px; border-radius: 14px; background: rgba(255,255,255,0.7); display: flex;
     align-items: center; justify-content: center; font-size: 16px; color: #9a7bbf; cursor: pointer;
     transition-property: background-color, transform; transition-duration: 140ms; }
.x:hover { background: #fff; }
.x:active { transform: scale(0.96); }
.content { max-height: 560px; overflow: hidden; transition-property: max-height, opacity; transition-duration: 420ms, 260ms;
           transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.folded .content { max-height: 0px; opacity: 0; }
.pad { padding: 14px 16px 16px; }
.lbl { font-size: 12px; font-weight: 700; color: #a58cc4; letter-spacing: 0.4px; margin-bottom: 8px; }

.field { min-height: 44px; border-radius: 14px; background: #fff; padding: 12px 14px; box-sizing: border-box; color: #b3a3c9; cursor: text;
         box-shadow: inset 0 0 0 1.5px #eedff9; transition-property: box-shadow; transition-duration: 160ms; }
.field:hover { box-shadow: inset 0 0 0 1.5px #dcc6f5; }
.field.on { color: #5b4676; box-shadow: inset 0 0 0 2px #c59cf2, 0 0 0 4px rgba(197,156,242,0.25); }
.caret { display: inline-block; width: 2px; height: 16px; border-radius: 1px; background: #c59cf2; margin-left: 1px; vertical-align: -3px;
         animation: blink 1.06s steps(1) infinite; }
.moods { display: flex; gap: 6px; margin: 12px 0; }
.mood { flex: 1; height: 54px; border-radius: 14px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 5px;
        font-size: 11px; font-weight: 700; color: #9c88b8; cursor: pointer; background: rgba(255,255,255,0.55);
        transition-property: background-color, box-shadow, color, transform; transition-duration: 180ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.mood:hover { background: #fff; }
.mood:active { transform: scale(0.96); }
.mood.on { background: #fff; color: #5b4676; }
.orb { width: 18px; height: 18px; border-radius: 9px; }
.mood.on .orb { animation: pop 420ms cubic-bezier(0.3, 1.6, 0.5, 1); }
.keep { height: 44px; border-radius: 22px; display: flex; align-items: center; justify-content: center; font-weight: 800; font-size: 15px;
        color: #fff; cursor: pointer; background: linear-gradient(90deg, #ff9fd2, #b79bff); box-shadow: 0 4px 12px rgba(183,155,255,0.45);
        transition-property: transform, opacity; transition-duration: 140ms; }
.keep:active { transform: scale(0.96); }
.keep.off { opacity: 0.45; }
.nudge { animation: nudge 360ms cubic-bezier(0.2, 0, 0, 1); }
.entries { margin-top: 14px; }
.entry { display: flex; gap: 10px; align-items: center; padding: 9px 10px; border-radius: 14px;
         transition-property: background-color; transition-duration: 140ms;
         animation: rise 520ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.entry:hover { background: rgba(255,255,255,0.7); }
.entry .orb { width: 14px; height: 14px; flex: none; }
.entry .t { flex: 1; font-size: 14px; color: #5b4676; line-height: 1.3; }
.entry .when { font-size: 11.5px; color: #b3a3c9; white-space: nowrap; font-variant-numeric: tabular-nums; }
.entry.new { animation: rise 520ms cubic-bezier(0.2, 0, 0, 1) backwards, glow 1600ms ease-out; }

/* -- the sleep companion -- */
.dialwrap { display: flex; flex-direction: column; align-items: center; }
.dial { position: relative; width: 176px; height: 176px; cursor: ns-resize; }
.dial canvas { position: absolute; left: 0px; top: 0px; width: 176px; height: 176px; }
.knob { position: absolute; left: 34px; top: 34px; width: 108px; height: 108px; border-radius: 54px;
        background: radial-gradient(#ffffff, #fbe9ff 55%, #e7d4fb);
        box-shadow: 0 8px 18px rgba(150,110,200,0.3), inset 0 -4px 8px rgba(180,140,230,0.25), inset 0 3px 6px #fff; }
.tick { position: absolute; left: 51px; top: 8px; width: 6px; height: 16px; border-radius: 3px; background: #ff9fd2; transform-origin: 3px 46px; }
.depth { margin-top: 4px; font-size: 13px; color: #a58cc4; font-weight: 700; }
.depth b { font-size: 26px; color: #6d4b93; font-variant-numeric: tabular-nums; letter-spacing: -0.5px; }
.row2 { display: flex; gap: 8px; margin-top: 14px; }
.btn { flex: 1; height: 44px; border-radius: 16px; display: flex; align-items: center; justify-content: center; gap: 8px; font-weight: 800;
       font-size: 13.5px; cursor: pointer; background: #fff; color: #6d4b93; box-shadow: inset 0 0 0 1.5px #eedff9;
       transition-property: background-color, color, transform, box-shadow; transition-duration: 160ms; }
.btn:hover { box-shadow: inset 0 0 0 1.5px #dcc6f5; }
.btn:active { transform: scale(0.96); }
.btn.on { background: #cbb2ff; color: #fff; box-shadow: none; }
.eq { display: flex; align-items: flex-end; gap: 2px; height: 14px; }
.eq i { width: 3px; height: 4px; border-radius: 1.5px; background: currentColor; display: block; }
.btn.on .eq i { animation: eq 900ms ease-in-out infinite alternate; }
.tries { margin-top: 10px; text-align: center; font-size: 11.5px; color: #b3a3c9; font-variant-numeric: tabular-nums; }

/* -- the door -- */
.door { position: absolute; width: 112px; height: 196px; cursor: pointer; }
.frame { position: absolute; left: 0px; top: 0px; width: 112px; height: 196px; background: #fff4fb;
         border-top-left-radius: 56px; border-top-right-radius: 56px; border-bottom-left-radius: 6px; border-bottom-right-radius: 6px;
         box-shadow: 0 0 0 3px #f5d2ea, 0 14px 22px rgba(140,90,150,0.25); }
.light { position: absolute; left: 10px; top: 10px; width: 92px; height: 186px;
         border-top-left-radius: 46px; border-top-right-radius: 46px; border-bottom-left-radius: 2px; border-bottom-right-radius: 2px;
         background: linear-gradient(180deg, #ffffff, #fff6c9 60%, #ffe39b);
         transition-property: box-shadow; transition-duration: 600ms; }
.door.open .light { box-shadow: 0 0 18px 4px rgba(255,244,190,0.95); }
.leaf { position: absolute; left: 10px; top: 10px; width: 92px; height: 186px;
        border-top-left-radius: 46px; border-top-right-radius: 46px; border-bottom-left-radius: 2px; border-bottom-right-radius: 2px;
        background: linear-gradient(180deg, #ffc7e6, #f3a9d3); transform-origin: 0px 91px;
        box-shadow: inset 0 0 0 4px rgba(255,255,255,0.35);
        transition-property: transform, background-color; transition-duration: 620ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.door:hover .leaf { transform: scaleX(0.94); }
.door.open .leaf { transform: scaleX(0.12); }
.handle { position: absolute; left: 72px; top: 96px; width: 10px; height: 10px; border-radius: 5px; background: #fff; box-shadow: 0 1px 2px rgba(120,60,120,0.4); }
.sign { position: absolute; left: -30px; top: -44px; width: 172px; text-align: center; font-size: 15px; font-weight: 700; color: #fff;
        text-shadow: 0 1px 2px rgba(120,70,150,0.55); animation: sway 3.2s ease-in-out infinite alternate; }

/* -- words -- */
.wave { left: 0px; right: 0px; text-align: center; font-size: 36px; font-weight: 700; color: #fff;
        text-shadow: 0 2px 0 rgba(197,156,242,0.8), 0 0 14px rgba(255,180,230,0.9); }
.wave span { display: inline-block; animation: wave 2.6s ease-in-out infinite; }
.status { font-size: 12.5px; color: #fff; font-weight: 700; text-shadow: 0 1px 2px rgba(120,70,150,0.6); font-variant-numeric: tabular-nums;
          display: flex; gap: 10px; align-items: center; }
.status i { width: 8px; height: 8px; border-radius: 4px; background: #ff6fae; display: block; animation: lamp 1.2s steps(1) infinite; }
.place { animation: swap 600ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.nope { left: 0px; right: 0px; text-align: center; font-size: 46px; font-weight: 700; color: #fff; pointer-events: none;
        text-shadow: 3px 0 0 rgba(255,80,160,0.7), -3px 0 0 rgba(80,200,255,0.7), 0 0 18px rgba(255,255,255,0.9);
        animation: nope 2600ms cubic-bezier(0.2, 0, 0, 1) forwards; }

@keyframes bob { from { transform: translate(0px, 0px); } to { transform: translate(5px, -9px); } }
@keyframes winin { from { opacity: 0; transform: translateY(18px) scale(0.97); filter: blur(4px); }
                   to { opacity: 1; transform: translateY(0px) scale(1); filter: blur(0px); } }
@keyframes rise { from { opacity: 0; transform: translateY(-8px); } to { opacity: 1; transform: translateY(0px); } }
@keyframes glow { 0% { background-color: rgba(255,240,180,0.9); } 100% { background-color: rgba(255,255,255,0); } }
@keyframes pop { 0% { transform: scale(1); } 40% { transform: scale(1.35); } 100% { transform: scale(1); } }
@keyframes nudge { 0% { transform: translateX(0px); } 25% { transform: translateX(-5px); } 60% { transform: translateX(4px); } 100% { transform: translateX(0px); } }
@keyframes blink { 0% { opacity: 1; } 50% { opacity: 0; } }
@keyframes lamp { 0% { opacity: 1; } 50% { opacity: 0.35; } 100% { opacity: 1; } }
@keyframes eq { from { height: 3px; } to { height: 14px; } }
@keyframes sway { from { transform: rotate(-3deg); } to { transform: rotate(3deg); } }
@keyframes wave { 0% { transform: translateY(0px); } 50% { transform: translateY(-8px); } 100% { transform: translateY(0px); } }
@keyframes swap { from { opacity: 0; filter: blur(4px); } to { opacity: 1; filter: blur(0px); } }
@keyframes nope { 0% { opacity: 0; transform: scale(1.06); letter-spacing: 8px; } 10% { opacity: 1; transform: translateX(-6px); }
                  14% { transform: translateX(6px); } 18% { transform: translateX(0px) scale(1); letter-spacing: 0px; }
                  75% { opacity: 1; } 100% { opacity: 0; transform: translateY(-10px); } }
";

const DESIGN: (f32, f32) = (1280.0, 720.0);
const PLACES: [&str; 3] = ["the field", "the pool rooms", "the night field"];
const SIGNS: [&str; 4] = ["do not open", "again?", "it's warmer in there", "you've been here before"];
const NOPE: [&str; 5] = ["you can't wake up yet", "not yet.", "the hallway is longer now", "who are you talking to?", "stay a little longer"];
const WAVE: &str = "are you still dreaming?";
/// A mood: its name and its colour.
const MOODS: [(&str, &str); 5] = [
    ("floaty", "#9fd0ff"),
    ("sweet", "#ffa9d8"),
    ("uneasy", "#a6e8c4"),
    ("lucid", "#ffe27a"),
    ("lost", "#c9b2ff"),
];

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

struct Entry {
    text: String,
    mood: usize,
    when: &'static str,
    /// Kept just now: it rises in with a glow.
    new: bool,
}

struct Dream {
    world: Fx,
    /// Where the pointer is (window px) and where the eyes look (eased towards it).
    pointer: (f32, f32),
    look: (f32, f32),
    last: f32,
    place: usize,
    next: usize,
    crossing: MotionValue,
    opens: u32,
    door_at: Option<f32>,
    depth: Dial,
    lullaby: bool,
    wakes: u32,
    woke: Option<f32>,
    /// A click in the sky: when and where (window px).
    touch: Option<(f32, f32, f32)>,
    entries: Vec<Entry>,
    draft: String,
    typing: bool,
    mood: usize,
    /// Keep pressed with nothing typed: the button shakes its head (keys the shake).
    nudges: u32,
    folded: [bool; 2],
    frames: u32,
}

impl Dream {
    fn new() -> Self {
        let entries = vec![
            Entry { text: "the school hallway went on forever and the lockers were humming".into(), mood: 2, when: "3 nights ago", new: false },
            Entry { text: "grandma's kitchen, but underwater, and nobody minded".into(), mood: 0, when: "last week", new: false },
            Entry { text: "i could fly, but only very slowly".into(), mood: 3, when: "a while ago", new: false },
        ];
        Self {
            world: world(), pointer: (640.0, 300.0), look: (640.0, 300.0), last: now(), place: 0, next: 0,
            crossing: MotionValue::new(0.0f32), opens: 0, door_at: None, depth: Dial::new(0.42), lullaby: true, wakes: 0,
            woke: None, touch: None, entries, draft: String::new(), typing: false, mood: 1, nudges: 0, folded: [false; 2],
            frames: 0,
        }
    }

    /// Through the door: it swings open, and the world crosses into the next place behind a bloom.
    fn open_door(&mut self) {
        if self.crossing.is_animating() {
            return;
        }
        self.next = (self.place + 1) % PLACES.len();
        self.opens += 1;
        self.door_at = Some(now());
        self.crossing.set(0.0f32);
        drop(self.crossing.animate(1.0f32, vars! { duration: 1.4, delay: 0.45, ease: "sine.inOut" }));
    }

    fn wake(&mut self) {
        self.wakes += 1;
        self.woke = Some(now());
    }

    fn keep(&mut self) {
        let text = self.draft.trim();
        if text.is_empty() {
            self.nudges += 1;
            return;
        }
        for e in &mut self.entries {
            e.new = false;
        }
        self.entries.insert(0, Entry { text: text.to_string(), mood: self.mood, when: "just now", new: true });
        self.entries.truncate(5);
        self.draft.clear();
        self.typing = false;
    }

    fn key(&mut self, e: &KeyDownEvent) -> bool {
        let k = &e.keystroke;
        if self.typing {
            match edit(&mut self.draft, k, 90) {
                Edit::Leave => self.typing = false,
                Edit::Submit => self.keep(),
                Edit::Typed => {}
                Edit::Ignored => return false,
            }
            return true;
        }
        match k.key.as_str() {
            "Up" => self.depth.set(self.depth.value + 0.1),
            "Down" => self.depth.set(self.depth.value - 0.1),
            "d" | "D" => {
                self.open_door();
                true
            }
            "w" | "W" => {
                self.wake();
                true
            }
            "space" => {
                self.touch = Some((now(), self.pointer.0, self.pointer.1));
                true
            }
            _ => false,
        }
    }
}

impl Dream {
    /// Window `n`: a title bar (its button folds it) over `body`, at (`x`, `y`) `width` wide (design px).
    fn window(&self, n: usize, title: &str, (x, y, width): (f32, f32, f32), body: Div, cx: &mut Context<Self>) -> Div {
        let win = div().class("win").css("width", rem(width)).css("animation-delay", format!("{}ms", 250 + n * 180))
            .child(div().class("bar kid").child(div().class("lamp")).child(title.to_string())
                .child(div().id(format!("x{n}")).class("x").child(if self.folded[n] { "+" } else { "–" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.folded[n] = !this.folded[n];
                        cx.stop_propagation();
                        cx.notify();
                    }))))
            .child(div().class("content").child(body));
        // the drift is on a wrapper: the window's own animation is its entrance
        at(div().class("drift").class(if self.folded[n] { "folded" } else { "" }), x, y)
            .css("animation-duration", format!("{}ms", 6400 + n * 1700)).css("animation-delay", format!("-{}ms", n * 2300))
            .child(win)
    }

    fn journal(&self, cx: &mut Context<Self>) -> Div {
        let mut field = div().id("field").class(if self.typing { "field on" } else { "field" });
        field = if self.draft.is_empty() && !self.typing {
            field.child("what did you dream about?")
        } else {
            let mut f = field.child(span().child(self.draft.clone()));
            if self.typing {
                f = f.child(span().class("caret"));
            }
            f
        };
        let mut moods = div().class("moods");
        for (k, (name, color)) in MOODS.iter().enumerate() {
            let on = k == self.mood;
            moods = moods.child(div().id(format!("mood{k}")).class(if on { "mood on" } else { "mood" })
                .css("box-shadow", if on { format!("inset 0 0 0 2px {color}") } else { "none".into() })
                .child(div().id(format!("orb{k}-{on}")).class("orb").css("background", *color)
                    .css("box-shadow", format!("0 0 8px {color}")))
                .child(*name)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.mood = k;
                    cx.notify();
                })));
        }
        let mut entries = div().class("entries").child(div().class("lbl").child("YOUR DREAMS"));
        for e in &self.entries {
            let color = MOODS[e.mood].1;
            entries = entries.child(div().id(format!("e-{}", e.text.len() * 31 + e.mood)).class(if e.new { "entry new" } else { "entry" })
                .child(div().class("orb").css("background", color).css("box-shadow", format!("0 0 6px {color}")))
                .child(div().class("t").child(e.text.clone()))
                .child(div().class("when").child(e.when)));
        }
        let off = self.draft.trim().is_empty();
        let body = div().class("pad")
            .child(div().class("lbl").child("LAST NIGHT"))
            .child(field.on_click(cx.listener(|this, _, _, cx| {
                this.typing = true;
                cx.stop_propagation();
                cx.notify();
            })))
            .child(moods)
            .child(div().id(format!("keep{}", self.nudges)).class(if off { "keep off" } else { "keep" })
                .class(if self.nudges > 0 && off { "nudge" } else { "" }).child("keep this dream")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.keep();
                    cx.stop_propagation();
                    cx.notify();
                })))
            .child(entries);
        self.window(0, "dreamlog.exe", (44.0, 40.0, 390.0), body, cx)
    }

    fn companion(&self, cx: &mut Context<Self>) -> Div {
        let v = self.depth.value;
        let ring = canvas(move |b, p| {
            let s = b.size.width / 176.0;
            let (cx, cy) = (b.origin.x + b.size.width / 2.0, b.origin.y + b.size.height / 2.0);
            let r = 80.0 * s;
            p.line(&arc_points(cx, cy, r, -135.0, 135.0), rgba(0xffffffcc), 7.0 * s);
            let end = -135.0 + 270.0 * v;
            glow_arc(p, (cx, cy, r), (-135.0, end), rgb(0xff8fcf), 7.0 * s, 3);
            // the dots along the track
            for k in 0..=10 {
                let a = (-135.0 + 27.0 * k as f32).to_radians();
                let (x, y) = (cx + 64.0 * s * a.sin(), cy - 64.0 * s * a.cos());
                let on = k as f32 / 10.0 <= v + 0.001;
                p.oval(Bounds { origin: point(x - 2.0 * s, y - 2.0 * s), size: size(4.0 * s, 4.0 * s) },
                       if on { rgb(0xc59cf2) } else { rgba(0xc59cf255) }, rgba(0x00000000), 0.0);
            }
        });
        let dial = div().id("depth").class("dial").child(ring)
            .child(div().class("knob").child(div().class("tick").css("transform", format!("rotate({}deg)", -135.0 + 270.0 * v))));
        let dial = draggable(dial, cx, Axis::Up, 220.0, |this: &mut Self| &mut this.depth, |_| {});
        let mut eq = div().class("eq");
        for k in 0..4 {
            eq = eq.child(i().css("animation-delay", format!("-{}ms", k * 230)));
        }
        let body = div().class("pad")
            .child(div().class("dialwrap").child(dial)
                .child(div().class("depth").child(b().child(format!("{:.0}%", v * 100.0))).child("  dream depth")))
            .child(div().class("row2")
                .child(div().id("lull").class(if self.lullaby { "btn on" } else { "btn" }).child(eq).child("lullaby")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.lullaby = !this.lullaby;
                        cx.notify();
                    })))
                .child(div().id("wake").class("btn").child("wake up")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.wake();
                        cx.notify();
                    }))))
            .child(div().class("tries").child(if self.wakes == 0 {
                "you have been asleep for 9 hours".to_string()
            } else {
                format!("attempts to wake up: {}", self.wakes)
            }));
        self.window(1, "sleep.exe", (920.0, 56.0, 300.0), body, cx)
    }
}

impl Render for Dream {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        focus_root(w, &mut self.frames);
        let t = now();
        let dt = (t - self.last).clamp(0.0, 0.1);
        self.last = t;
        // the eyes take their time finding you
        self.look = (approach(self.look.0, self.pointer.0, 3.5, dt), approach(self.look.1, self.pointer.1, 3.5, dt));
        // the crossing has finished: the next place is this place
        let k = self.crossing.get();
        if k >= 1.0 && !self.crossing.is_animating() {
            self.place = self.next;
            self.crossing.set(0.0f32);
        }
        let open = self.door_at.is_some_and(|d| t - d < 2.2);
        let fit = Fit::new(w, DESIGN);

        let (look, place, next, depth) = (self.look, self.place, self.next, self.depth.value);
        let (woke, touch, crossing) = (self.woke, self.touch, self.crossing);
        let world = self.world.canvas(move |b| {
            let t = now();
            let m = uv_in(b, look.0, look.1);
            let (ta, tu) = touch.map_or((-1.0, [0.0, 0.0]), |(at, x, y)| (t - at, uv_in(b, x, y)));
            [t, m[0], m[1], place as f32, next as f32, crossing.get(), depth, woke.map_or(-1.0, |w| t - w), ta, tu[0], tu[1]]
        }).id("bg").class("world")
            .on_click(cx.listener(|this, e: &ClickEvent, _, cx| {
                this.touch = Some((now(), e.up.position.x.0, e.up.position.y.0));
                this.typing = false;
                cx.notify();
            }));

        let shown = if k > 0.5 { next } else { place };
        let mut wave = at(div().class("wave kid"), 0.0, 616.0).css("width", rem(DESIGN.0));
        for (n, ch) in WAVE.chars().enumerate() {
            wave = wave.child(span().css("animation-delay", format!("{}ms", n * 90)).child(if ch == ' ' { "\u{a0}".to_string() } else { ch.to_string() }));
        }
        let door = at(div().id("door").class(if open { "door open" } else { "door" }), 700.0, 332.0)
            .child(div().class("frame")).child(div().class("light"))
            .child(div().class("leaf").child(div().class("handle")))
            .child(div().id(format!("sign{}", self.opens)).class("sign kid place").child(SIGNS[(self.opens as usize).min(SIGNS.len() - 1)]))
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_door();
                cx.stop_propagation();
                cx.notify();
            }));
        let mut stage = at(div(), fit.ox, fit.oy).css("width", rem(DESIGN.0)).css("height", rem(DESIGN.1))
            .child(door)
            .child(wave)
            .child(at(div().class("status"), 48.0, 676.0)
                .child(i())
                .child(span().id(format!("place{shown}")).class("place").child(format!("now: {}", PLACES[shown])))
                .child(span().child("·  3:33 am")))
            .child(self.journal(cx))
            .child(self.companion(cx));
        if let Some(at_) = self.woke
            && t - at_ < 2.6
        {
            stage = stage.child(at(div().id(format!("nope{}", self.wakes)).class("nope kid"), 0.0, 196.0).css("width", rem(DESIGN.0))
                .child(NOPE[(self.wakes as usize - 1) % NOPE.len()]));
        }
        html().css("font-size", fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root").attr("tabindex", "0")
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                    if this.key(e) {
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, _| this.pointer = (e.position.x.0, e.position.y.0)))
                .child(world)
                .child(div().class("abs").css("left", "0").css("top", "0").css("width", rem(fit.w)).css("height", rem(fit.h))
                    .css("pointer-events", "none")
                    .child(stage.css("pointer-events", "auto"))))
    }
}

/// Everything, headlessly: the entrance, a typed dream kept (and an empty one refused), moods,
/// the dial dragged, the lullaby, a wake, the door twice, a window folded, a touch in the sky.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        s.play("00-enter", &[0, 200, 500, 900, 1500]);
        let (kx, ky) = s.centre("keep0");
        s.cx.simulate_click(s.win, kx, ky);
        s.play("01-refused", &[0, 150, 400]);
        s.click("field");
        for c in "a house with no doors but i got in".chars() {
            let key = if c == ' ' { "space".to_string() } else { c.to_string() };
            s.cx.simulate_key(s.win, &c.to_string(), &key);
        }
        s.click("mood4");
        s.play("02-typed", &[0, 200]);
        s.key("Return");
        s.play("03-kept", &[0, 150, 400, 900]);
        let (x, y) = s.centre("depth");
        s.cx.simulate_drag(s.win, (x, y), (x, y - 90.0));
        s.play("04-deeper", &[0, 200]);
        s.click("lull");
        s.play("05-lullaby-off", &[0, 200]);
        s.click("wake");
        s.play("06-wake", &[0, 120, 300, 900, 2700]);
        s.click("door");
        s.play("07-door", &[0, 200, 600, 1100, 1900, 2600]);
        s.click("door");
        s.play("08-door-again", &[0, 1100, 2600]);
        s.click("x0");
        s.play("09-fold", &[0, 200, 500]);
        s.cx.simulate_click(s.win, 640.0, 160.0);
        s.play("10-touch", &[0, 300]);
    })
}

fn world() -> Fx {
    Fx::new("dream", WORLD, 11, [255, 214, 236])
}

fn main() {
    if let Some(dir) = arg("--frames") {
        let fx = world();
        let still = |place: f32, next: f32, k: f32, depth: f32, wake: f32| {
            [7.0f32, 0.32, 0.7, place, next, k, depth, wake, -1.0, 0.0, 0.0]
        };
        let stills = [
            ("field", still(0.0, 0.0, 0.0, 0.42, -1.0)),
            ("field-deep", still(0.0, 0.0, 0.0, 0.95, -1.0)),
            ("pool", still(1.0, 1.0, 0.0, 0.6, -1.0)),
            ("night", still(2.0, 2.0, 0.0, 0.8, -1.0)),
            ("crossing", still(0.0, 1.0, 0.45, 0.5, -1.0)),
            ("wake", still(0.0, 0.0, 0.0, 0.5, 0.15)),
        ];
        if gpu_stills(&dir, 1280, 720, stills, |g, f| g.draw(fx.effect(), &[], f)).is_none() {
            eprintln!("dream shader: {:?}", fx.error());
        }
        return;
    }
    run_with(DESIGN.0, DESIGN.1, "BUI · Dreamlog", Dream::new, arg("--storyboard").map(storyboard));
}

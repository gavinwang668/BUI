//! Breeze: a Frutiger Aero music player on BUI. A GPU shader paints the world behind the app:
//! a deep blue sky, drifting clouds, the sun with its rays and lens ghosts, droplets on the glass
//! and a meadow of grass blades swaying in the wind. Over it, glossy glass (CSS `backdrop-filter:
//! blur() saturate()`, rim light, a gloss band) holds a streaming app: a sidebar of playlists, an
//! album with shader cover art, a track list and a player bar.
//!
//! It plays: time runs, the now-playing row's equalizer bounces, the track ends into the next one.
//! Click a track to play it, the round buttons or Space to play / pause (the icons cross-fade:
//! scale .25 -> 1, blur 4px -> 0), drag or click the scrubber to seek and the fader for volume
//! (← / → seek, ↑ / ↓ volume, N / P skip), the ✓ to like.
//!
//! Motion: the panels rise in on first light (60 ms apart), the rows after them (40 ms apart);
//! hover and selection are 120-160 ms colour transitions, presses scale to 0.96.
//!
//! `cargo run --release --bin aero`
//! `-- --storyboard DIR` drives it headlessly and saves frames; `-- --frames DIR` renders the sky
//! and the cover art off-screen on the GPU.

use std::sync::OnceLock;

use blenderui::prelude::*;
use bui_demos::canvas::{rem, rem_css, Fit};
use bui_demos::control::{draggable, Axis, Dial};
use bui_demos::{arg, focus_root, gpu_stills, now, run_with, story::story, Fx, Script};

/// U[0] = time, U[1] = the music's energy (0 paused).
const SKY: &str = r"
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
// one layer of blades: n per screen height, tips no higher than `top`, swaying with the wind
float blades(vec2 q, float y, float n, float top, float seed, float t) {
    float h = clamp(1.0 - y, 0.0, 1.0);
    float wind = 0.5 + 0.5 * sin(t * 0.7 + q.x * 1.3 + seed);
    float x = q.x * n + (0.6 * sin(t * 1.6 + q.x * 2.7 + seed) * wind + 0.4) * h * n * 0.09;
    float id = floor(x); float f = fract(x) - 0.5;
    float r = hash(vec2(id, seed));
    float tip = top + 0.16 * r;
    float lean = (r - 0.5) * 1.6 * (y - tip) / max(1.0 - tip, 0.01);
    float w = 0.46 * smoothstep(tip, 1.05, y);
    return step(tip, y) * smoothstep(w, w * 0.55, abs(f - lean * 0.5));
}
vec4 effect(vec2 p) {
    float t = U[0]; float beat = U[1];
    vec2 uv = p / box_size; float a = box_size.x / box_size.y;
    vec2 q = vec2(uv.x * a, uv.y);
    // sky: deep blue up top, pale at the horizon
    vec3 c = mix(vec3(0.07, 0.36, 0.86), vec3(0.55, 0.83, 1.0), smoothstep(0.0, 0.78, uv.y));
    // clouds, thinning towards the horizon
    float cl = fbm(q * vec2(2.4, 5.0) + vec2(t * 0.018, 0.0));
    float cl2 = fbm(q * vec2(5.0, 9.0) + vec2(t * 0.03, 3.0));
    float cloud = smoothstep(0.52, 0.78, cl * 0.8 + cl2 * 0.3) * (1.0 - smoothstep(0.2, 0.68, uv.y));
    c = mix(c, vec3(0.97, 0.99, 1.0), cloud * 0.9);
    // the sun, its rays and the lens ghosts running through the middle
    vec2 sp = vec2(0.9 * a, 0.12);
    vec2 dv = q - sp; float d = length(dv);
    float ang = atan2(dv.y, dv.x);
    c += vec3(1.0, 0.97, 0.86) * (0.018 / (d + 0.012)) * (1.0 + 0.25 * beat);
    c += vec3(1.0, 0.95, 0.8) * pow(0.5 + 0.5 * sin(ang * 14.0 + t * 0.15), 10.0) * exp(-d * 2.6) * 0.35;
    vec2 axis = vec2(0.5 * a, 0.5) - sp;
    for (int k = 1; k < 4; k++) {
        float fk = float(k);
        vec2 g = sp + axis * (0.55 * fk);
        float gr = 0.03 + 0.025 * fk;
        float ring = smoothstep(gr, gr * 0.7, length(q - g)) * 0.06;
        c += vec3(0.6, 0.9 - 0.1 * fk, 1.0) * ring;
    }
    // a meadow: the hill, then three layers of blades, far to near
    float hill = 0.74 + 0.04 * sin(q.x * 2.1 + 0.4) + 0.025 * sin(q.x * 5.7 + 1.0);
    float field = smoothstep(hill - 0.004, hill + 0.004, uv.y);
    vec3 grass = mix(vec3(0.55, 0.86, 0.25), vec3(0.12, 0.45, 0.05), smoothstep(hill, 1.0, uv.y));
    c = mix(c, grass, field);
    float b1 = blades(q, uv.y, 70.0, hill - 0.05, 1.0, t);
    c = mix(c, vec3(0.42, 0.78, 0.18) * (0.75 + 0.35 * uv.y), b1 * 0.9);
    float b2 = blades(q, uv.y, 38.0, hill - 0.02, 2.0, t * 1.1);
    c = mix(c, vec3(0.3, 0.68, 0.1) * (0.8 + 0.3 * (1.0 - uv.y)), b2);
    float b3 = blades(q, uv.y, 17.0, 0.86, 3.0, t * 1.2);
    c = mix(c, vec3(0.2, 0.55, 0.08) + 0.12 * vec3(0.6, 0.8, 0.2) * smoothstep(0.86, 0.95, uv.y), b3);
    // droplets on the glass, upper right: dark rim, bright cap, a glint
    vec2 gq = q * 11.0;
    vec2 id = floor(gq); vec2 f = fract(gq) - 0.5;
    float hd = hash(id + 7.0);
    vec2 jit = (vec2(hash(id + 1.3), hash(id + 2.9)) - 0.5) * 0.5;
    float rad = 0.12 + 0.22 * hash(id + 4.4);
    float side = smoothstep(0.52, 0.8, uv.x) * (1.0 - smoothstep(0.55, 0.75, uv.y));
    if (hd > 0.7 && side > 0.0) {
        float dd = length(f - jit);
        float inside = smoothstep(rad, rad * 0.85, dd) * side;
        float rim = smoothstep(rad * 0.6, rad, dd) * inside;
        float glint = smoothstep(rad * 0.32, 0.0, length(f - jit + vec2(rad * 0.35, rad * 0.38)));
        c = mix(c, c * 1.12 + 0.06, inside);
        c -= rim * 0.1;
        c += glint * inside * 0.9;
    }
    // bubbles drifting up from the meadow
    vec2 bq = q * vec2(9.0, 6.0) + vec2(0.0, t * 0.12);
    vec2 bid = floor(bq); vec2 bf = fract(bq) - 0.5;
    float hb = hash(bid + 11.0);
    if (hb > 0.88) {
        float br = 0.12 + 0.12 * hash(bid + 5.0);
        float bd = length(bf - (vec2(hash(bid), hash(bid + 3.0)) - 0.5) * 0.4);
        float ring = smoothstep(br, br * 0.8, bd) - smoothstep(br * 0.8, br * 0.55, bd);
        c += vec3(0.85, 0.95, 1.0) * ring * 0.35 * smoothstep(0.95, 0.4, uv.y);
    }
    c *= 1.0 - 0.18 * length(uv - vec2(0.5, 0.45));
    c += (hash(p + fract(t) * 13.0) - 0.5) * 0.015;
    return vec4(clamp(c, 0.0, 1.0), 1.0);
}
";

/// U[0] = time, U[1] = hue turn, U[2] = playing (0 / 1). The album art: a glass orb over a
/// sunrise sea, its insides swirling while the album plays. Rounded corners are its own alpha.
const COVER: &str = r"
vec3 hue(float h) { return clamp(abs(fract(h + vec3(0.0, 0.66, 0.33)) * 6.0 - 3.0) - 1.0, 0.0, 1.0); }
vec4 effect(vec2 p) {
    float t = U[0]; vec2 uv = p / box_size;
    vec3 c = mix(vec3(1.0, 0.78, 0.55), vec3(0.2, 0.55, 0.95), smoothstep(0.1, 0.62, uv.y));
    c = mix(c, vec3(0.05, 0.3, 0.6) + 0.12 * sin(uv.x * 40.0 + t * 2.0 * U[2]) * smoothstep(0.62, 1.0, uv.y),
            smoothstep(0.6, 0.64, uv.y));
    vec2 o = uv - vec2(0.5, 0.48);
    float d = length(o);
    float r = 0.3;
    if (d < r) {
        float a = atan2(o.y, o.x) + t * 0.6 * U[2];
        float sw = 0.5 + 0.5 * sin(a * 3.0 + d * 22.0 - t * 1.5 * U[2]);
        vec3 inner = mix(hue(U[1] + d * 1.6 + 0.1 * sw), vec3(1.0), 0.25) * (0.75 + 0.35 * sw);
        float edge = smoothstep(r * 0.65, r, d);
        inner = mix(inner, vec3(0.75, 0.92, 1.0), edge * 0.6);
        inner += smoothstep(0.1, 0.0, length(o - vec2(-0.1, -0.12))) * 0.9;
        c = mix(c, inner, smoothstep(r, r - 0.006, d));
    }
    c += vec3(1.0, 0.9, 0.7) * 0.12 * smoothstep(0.5, 0.0, length(uv - vec2(0.15, 0.1)));
    // rounded corners (7% of the side)
    vec2 hb = box_size * 0.5; float rr = box_size.x * 0.07;
    vec2 qd = abs(p - hb) - hb + rr;
    float sd = length(max(qd, 0.0)) + min(max(qd.x, qd.y), 0.0) - rr;
    float alpha = clamp(0.5 - sd, 0.0, 1.0);
    return vec4(c * alpha, alpha);
}
";

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; position: relative; overflow: hidden; background: #3d8ee6; user-select: none;
       font-family: 'Segoe UI', Inter, system-ui, sans-serif; color: #fff; -webkit-font-smoothing: antialiased; }
.sky { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
.app { position: absolute; width: 1200px; height: 776px; }
.txt { text-shadow: 0 1px 2px rgba(0,40,100,0.35); }

/* glass: a translucent fill, frosted backdrop, a rim and a gloss band along the top */
.glass { position: absolute; border-radius: 22px;
         background: linear-gradient(180deg, rgba(255,255,255,0.26) 0%, rgba(255,255,255,0.08) 100%);
         backdrop-filter: blur(20px) saturate(1.5);
         box-shadow: 0 14px 40px rgba(8,50,120,0.28), 0 0 0 1px rgba(255,255,255,0.42), inset 0 1px 0 rgba(255,255,255,0.75); }
.gloss { position: absolute; left: 0; top: 0; width: 100%; height: 46%; border-radius: 22px 22px 0 0;
         background: linear-gradient(180deg, rgba(255,255,255,0.28) 0%, rgba(255,255,255,0.0) 100%); }
.rise { animation: rise 520ms cubic-bezier(0.2, 0, 0, 1) backwards; }
@keyframes rise { from { opacity: 0; transform: translateY(14px); } to { opacity: 1; transform: translateY(0); } }

/* round glossy buttons */
.btn { position: relative; display: flex; align-items: center; justify-content: center; cursor: pointer;
       transition-property: transform, background-color; transition-duration: 140ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.btn:active { transform: scale(0.96); }
.orb { border-radius: 50%; background: linear-gradient(180deg, rgba(255,255,255,0.55) 0%, rgba(255,255,255,0.18) 55%, rgba(255,255,255,0.3) 100%);
       box-shadow: 0 4px 12px rgba(10,60,140,0.25), 0 0 0 1px rgba(255,255,255,0.6), inset 0 1px 0 #fff; }
.orb:hover { background-color: rgba(255,255,255,0.22); }
.go { border-radius: 50%; background: linear-gradient(180deg, #b8ff8a 0%, #4fd12f 48%, #2a9e18 52%, #4ec431 100%);
      box-shadow: 0 6px 18px rgba(30,120,10,0.45), 0 0 0 1px rgba(255,255,255,0.7), inset 0 2px 1px rgba(255,255,255,0.8); }
.go:hover { background-color: #6fe04a; }
.pearl { border-radius: 50%; background: linear-gradient(180deg, #ffffff 0%, #e9f4ff 50%, #cfe3f7 52%, #f4f9ff 100%);
         box-shadow: 0 4px 14px rgba(10,60,140,0.3), 0 0 0 1px rgba(255,255,255,0.9); }
.shine { position: absolute; left: 14%; top: 5%; width: 72%; height: 42%; border-radius: 50%;
         background: linear-gradient(180deg, rgba(255,255,255,0.85) 0%, rgba(255,255,255,0.0) 100%); }
.ico { width: 20px; height: 20px; }
.swap { position: relative; width: 26px; height: 26px; }
.swap .ico { position: absolute; left: 0; top: 0; width: 26px; height: 26px;
             transition-property: opacity, transform, filter; transition-duration: 220ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.swap .off { opacity: 0; transform: scale(0.25); filter: blur(4px); }
.swap .on { opacity: 1; transform: scale(1); filter: blur(0px); }

/* top bar */
.logo { left: 18px; top: 14px; display: flex; align-items: center; gap: 10px; font-size: 22px; font-weight: 700; letter-spacing: -0.5px; }
.logo .mark { width: 34px; height: 34px; border-radius: 50%; position: relative;
              background: linear-gradient(180deg, #a6ff7a 0%, #3cc522 55%, #1f8f12 100%);
              box-shadow: 0 3px 10px rgba(20,110,10,0.45), 0 0 0 1px rgba(255,255,255,0.7); }
.search { left: 280px; top: 12px; width: 420px; height: 40px; border-radius: 20px; display: flex; align-items: center; gap: 10px;
          padding: 0 16px; box-sizing: border-box; font-size: 14px; color: rgba(255,255,255,0.85); }
.pill { height: 40px; border-radius: 20px; display: flex; align-items: center; gap: 8px; padding: 0 16px; box-sizing: border-box;
        font-size: 14px; font-weight: 600; }
.avatar { width: 30px; height: 30px; border-radius: 50%; background: linear-gradient(135deg, #ffd27a, #ff7aa8 55%, #7a8cff);
          box-shadow: 0 0 0 1px rgba(255,255,255,0.8); }

/* sidebar */
.nav { margin: 10px; padding: 6px; border-radius: 16px; background: rgba(255,255,255,0.14); box-shadow: 0 0 0 1px rgba(255,255,255,0.3); }
.navi { display: flex; align-items: center; gap: 14px; height: 40px; padding: 0 12px; border-radius: 10px; font-size: 15px; font-weight: 600; cursor: pointer;
        transition-property: background-color; transition-duration: 140ms; }
.navi:hover { background: rgba(255,255,255,0.16); }
.navi.sel { background: rgba(255,255,255,0.38); box-shadow: inset 0 1px 0 rgba(255,255,255,0.8); }
.plhead { display: flex; align-items: center; justify-content: space-between; padding: 8px 18px 6px; font-size: 13px; font-weight: 700; }
.plhead .tools { display: flex; gap: 6px; }
.pl { display: flex; align-items: center; gap: 12px; height: 56px; margin: 0 10px; padding: 0 8px; border-radius: 12px; cursor: pointer;
      transition-property: background-color; transition-duration: 140ms; }
.pl:hover { background: rgba(255,255,255,0.16); }
.pl.sel { background: rgba(255,255,255,0.32); }
.pl .art { position: relative; width: 44px; height: 44px; border-radius: 8px; flex-shrink: 0;
           box-shadow: 0 2px 6px rgba(0,40,100,0.25), 0 0 0 1px rgba(0,0,0,0.1); display: flex; align-items: center; justify-content: center; }
.pl .art .shine { left: 0; top: 0; width: 100%; height: 50%; border-radius: 8px 8px 0 0; background: linear-gradient(180deg, rgba(255,255,255,0.55), rgba(255,255,255,0)); }
.pl .name { font-size: 14px; font-weight: 600; }
.pl .meta { font-size: 11.5px; opacity: 0.8; margin-top: 1px; }

/* album header */
.cover { left: 24px; top: 24px; width: 176px; height: 176px; border-radius: 12px;
         box-shadow: 0 12px 28px rgba(0,40,110,0.4), 0 0 0 1px rgba(0,0,0,0.1); }
.kind { left: 228px; top: 46px; font-size: 13px; font-weight: 600; }
.title { left: 224px; top: 62px; font-size: 64px; font-weight: 800; letter-spacing: -2px; line-height: 1.1;
         text-shadow: 0 2px 12px rgba(0,40,100,0.35); }
.by { left: 228px; top: 146px; display: flex; align-items: center; gap: 8px; font-size: 14px; font-weight: 700; }
.by .avatar { width: 24px; height: 24px; }
.meta2 { left: 228px; top: 178px; font-size: 13px; opacity: 0.9; }
.actions { left: 24px; top: 222px; display: flex; align-items: center; gap: 16px; }

/* tracks */
.thead { left: 16px; top: 296px; width: 872px; height: 30px; display: flex; align-items: center; font-size: 13px; font-weight: 600; opacity: 0.85;
         border-bottom: 1px solid rgba(255,255,255,0.3); }
.list { left: 16px; top: 332px; width: 872px; height: 240px; overflow-y: auto; }
.row { display: flex; align-items: center; height: 46px; border-radius: 10px; cursor: pointer; font-size: 14px;
       transition-property: background-color, box-shadow; transition-duration: 120ms; }
.row:hover { background: rgba(255,255,255,0.18); }
.row.on { background: rgba(255,255,255,0.3); box-shadow: inset 0 1px 0 rgba(255,255,255,0.7), 0 0 0 1px rgba(255,255,255,0.35); }
.c-n { width: 48px; text-align: center; font-variant-numeric: tabular-nums; opacity: 0.9; display: flex; justify-content: center; }
.c-t { flex: 1; }
.c-t .a { font-size: 12px; opacity: 0.8; margin-top: 1px; }
.c-p { width: 200px; font-variant-numeric: tabular-nums; opacity: 0.9; }
.c-l { width: 44px; display: flex; justify-content: center; }
.c-d { width: 60px; font-variant-numeric: tabular-nums; text-align: right; }
.c-m { width: 44px; display: flex; justify-content: center; opacity: 0;
       transition-property: opacity; transition-duration: 120ms; }
.row:hover .c-m { opacity: 1; }
.eq { display: flex; align-items: flex-end; gap: 2px; height: 16px; }
.eq div { width: 3px; border-radius: 1.5px; background: #b9ff8f; }
.like { width: 22px; height: 22px; border-radius: 50%; display: flex; align-items: center; justify-content: center; cursor: pointer;
        border: 1.5px solid rgba(255,255,255,0.75); box-sizing: border-box; transition-property: transform; transition-duration: 120ms; }
.like:active { transform: scale(0.96); }
.like.yes { background: linear-gradient(180deg, #8af06a, #2fae1c); border-color: rgba(255,255,255,0.9); box-shadow: 0 1px 4px rgba(20,100,10,0.4); }
.pop { animation: pop 300ms cubic-bezier(0.2, 0, 0, 1); }
@keyframes pop { 0% { transform: scale(0.6); } 60% { transform: scale(1.15); } 100% { transform: scale(1); } }

/* player bar */
.now { left: 18px; top: 20px; display: flex; align-items: center; gap: 14px; }
.now .mini { width: 64px; height: 64px; border-radius: 10px; box-shadow: 0 6px 14px rgba(0,40,110,0.35), 0 0 0 1px rgba(0,0,0,0.1); }
.now .t { font-size: 15px; font-weight: 700; }
.now .a { font-size: 12.5px; opacity: 0.85; margin-top: 2px; }
.ctrl { left: 384px; top: 12px; width: 400px; display: flex; align-items: center; justify-content: center; gap: 22px; }
.tog { opacity: 0.75; transition-property: opacity; transition-duration: 140ms; }
.tog.lit { opacity: 1; }
.time { top: 70px; width: 44px; font-size: 12px; font-variant-numeric: tabular-nums; }
.bar { height: 24px; display: flex; align-items: center; cursor: pointer; }
.track { position: relative; width: 100%; height: 6px; border-radius: 3px; background: rgba(255,255,255,0.32);
         box-shadow: inset 0 1px 2px rgba(0,40,100,0.3); }
.fill { position: absolute; left: 0; top: 0; height: 6px; border-radius: 3px; background: linear-gradient(180deg, #c6ff9e, #45c42a); }
.knob { position: absolute; top: -5px; width: 16px; height: 16px; margin-left: -8px; border-radius: 50%;
        background: linear-gradient(180deg, #fff, #dbeaf7); box-shadow: 0 1px 4px rgba(0,40,100,0.4);
        transform: scale(0.75); transition-property: transform; transition-duration: 140ms; }
.bar:hover .knob { transform: scale(1); }
.grab .knob { transform: scale(1.15); }
.right { left: 920px; top: 32px; display: flex; align-items: center; gap: 14px; }
/* last, so it wins over any class that positions relatively */
.abs { position: absolute; }
";

/// Title, plays, length (seconds).
const TRACKS: [(&str, &str, u32); 8] = [
    ("Cloud Harbor", "21,734,620", 236), ("Dewfall", "12,664,330", 257), ("Sunlit Static", "8,725,384", 196),
    ("Paper Planes", "11,264,330", 214), ("Greenhouse Heart", "9,345,124", 216), ("Bubble Tram", "6,102,775", 178),
    ("Aquarium Sky", "4,918,203", 245), ("Fin", "3,377,941", 192),
];
const ARTIST: &str = "Mira Solenne";

/// Name, meta, cover gradient.
const PLAYLISTS: [(&str, &str, &str); 7] = [
    ("Liked Songs", "Playlist · 1,338 songs", "linear-gradient(135deg, #5b4dff, #b9a9ff)"),
    ("Morning Dew", "Playlist · Breeze", "linear-gradient(135deg, #ff9a8b, #ffd36e)"),
    ("Chill Lagoon", "Playlist · oceanic", "linear-gradient(135deg, #00c6ff, #0062ff)"),
    ("Run Club", "Playlist · oceanic", "linear-gradient(135deg, #3f9e1c, #b2ec6a)"),
    ("2000s Gloss", "Playlist · Breeze", "linear-gradient(135deg, #ff6ec4, #7873f5)"),
    ("Focus Garden", "Playlist · Breeze", "linear-gradient(135deg, #11998e, #38ef7d)"),
    ("Summer Drive", "Playlist · oceanic", "linear-gradient(135deg, #f7971e, #ffd200)"),
];

const DESIGN: (f32, f32) = (1200.0, 776.0);
/// The player bar's corner, and the scrubber and the volume fader in it (x, y, width): a press
/// anywhere on one jumps there, so where they are is known up front.
const PLAYER: (f32, f32) = (16.0, 660.0);
const SCRUB: (f32, f32, f32) = (444.0, 66.0, 280.0);
const VOLUME: (f32, f32, f32) = (1036.0, 30.0, 110.0);

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

/// Icons, drawn with the painter on a 24-unit grid (one stroke weight: 1.8 units).
#[derive(Clone, Copy)]
enum Ico {
    Home,
    Search,
    Library,
    Plus,
    Play,
    Pause,
    Prev,
    Next,
    Shuffle,
    Repeat,
    Heart,
    Check,
    Download,
    More,
    Back,
    Forward,
    Bell,
    Volume,
    Device,
    Sparkle,
    Chevron,
    Clock,
    Queue,
}

fn ico(k: Ico, color: u32, size: f32) -> Div {
    canvas(move |b, p| paint_ico(k, b, p, rgb(color))).class("ico").css("width", rem(size)).css("height", rem(size))
}

fn paint_ico(k: Ico, b: Bounds<f32>, p: &mut Painter, c: Rgba) {
    let s = b.size.width.min(b.size.height) / 24.0;
    let (ox, oy) = (b.origin.x + (b.size.width - 24.0 * s) / 2.0, b.origin.y + (b.size.height - 24.0 * s) / 2.0);
    let at = |(x, y): (f32, f32)| (ox + x * s, oy + y * s);
    let w = 1.8 * s;
    let bx = |x: f32, y: f32, ww: f32, hh: f32| Bounds { origin: point(ox + x * s, oy + y * s), size: size(ww * s, hh * s) };
    macro_rules! line {
        ($($pt:expr),+) => { p.line(&[$(at($pt)),+], c, w) };
    }
    macro_rules! fill {
        ($($pt:expr),+) => { p.polygon(&[$(at($pt)),+], c) };
    }
    match k {
        Ico::Home => {
            line!((3.5, 11.0), (12.0, 3.8), (20.5, 11.0));
            line!((6.0, 9.5), (6.0, 20.0), (18.0, 20.0), (18.0, 9.5));
        }
        Ico::Search => {
            p.oval(bx(4.0, 4.0, 13.0, 13.0), Rgba::TRANSPARENT, c, w);
            line!((15.0, 15.0), (20.5, 20.5));
        }
        Ico::Library => {
            line!((5.0, 4.0), (5.0, 20.0));
            line!((10.0, 4.0), (10.0, 20.0));
            line!((14.0, 5.0), (19.0, 19.5));
        }
        Ico::Plus => {
            line!((12.0, 5.0), (12.0, 19.0));
            line!((5.0, 12.0), (19.0, 12.0));
        }
        // the triangle sits a unit right of the box centre: its visual centre is its centroid
        Ico::Play => fill!((8.0, 4.5), (20.0, 12.0), (8.0, 19.5)),
        Ico::Pause => {
            p.quad(bx(6.0, 5.0, 4.2, 14.0), 1.2 * s, c, Rgba::TRANSPARENT, 0.0);
            p.quad(bx(13.8, 5.0, 4.2, 14.0), 1.2 * s, c, Rgba::TRANSPARENT, 0.0);
        }
        Ico::Prev => {
            fill!((19.0, 5.5), (8.5, 12.0), (19.0, 18.5));
            p.quad(bx(5.0, 5.5, 2.6, 13.0), 1.0 * s, c, Rgba::TRANSPARENT, 0.0);
        }
        Ico::Next => {
            fill!((5.0, 5.5), (15.5, 12.0), (5.0, 18.5));
            p.quad(bx(16.4, 5.5, 2.6, 13.0), 1.0 * s, c, Rgba::TRANSPARENT, 0.0);
        }
        Ico::Shuffle => {
            line!((3.5, 7.0), (8.0, 7.0), (15.0, 17.0), (19.0, 17.0));
            line!((3.5, 17.0), (8.0, 17.0), (15.0, 7.0), (19.0, 7.0));
            fill!((18.0, 4.0), (21.5, 7.0), (18.0, 10.0));
            fill!((18.0, 14.0), (21.5, 17.0), (18.0, 20.0));
        }
        Ico::Repeat => {
            line!((4.5, 13.0), (4.5, 8.0), (17.0, 8.0));
            line!((19.5, 11.0), (19.5, 16.0), (7.0, 16.0));
            fill!((16.0, 5.0), (19.5, 8.0), (16.0, 11.0));
            fill!((8.0, 13.0), (4.5, 16.0), (8.0, 19.0));
        }
        Ico::Heart => {
            let pts: [(f32, f32); 28] = std::array::from_fn(|i| {
                let t = i as f32 / 28.0 * std::f32::consts::TAU;
                let x = 16.0 * t.sin().powi(3);
                let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
                at((12.0 + x * 0.5, 11.5 - y * 0.5))
            });
            p.polygon(&pts, c);
        }
        Ico::Check => line!((6.0, 12.5), (10.2, 16.5), (18.0, 7.5)),
        Ico::Download => {
            line!((12.0, 4.5), (12.0, 16.0));
            line!((7.0, 11.5), (12.0, 16.5), (17.0, 11.5));
            line!((5.0, 20.0), (19.0, 20.0));
        }
        Ico::More => {
            for x in [5.0, 12.0, 19.0] {
                p.oval(bx(x - 1.7, 10.3, 3.4, 3.4), c, Rgba::TRANSPARENT, 0.0);
            }
        }
        Ico::Back => line!((14.5, 5.5), (8.0, 12.0), (14.5, 18.5)),
        Ico::Forward | Ico::Chevron => line!((9.5, 5.5), (16.0, 12.0), (9.5, 18.5)),
        Ico::Bell => {
            line!((5.0, 17.0), (6.5, 15.0), (6.5, 10.5), (8.0, 6.5), (12.0, 4.5), (16.0, 6.5), (17.5, 10.5), (17.5, 15.0), (19.0, 17.0), (5.0, 17.0));
            line!((10.0, 19.5), (14.0, 19.5));
        }
        Ico::Volume => {
            fill!((3.5, 9.0), (7.5, 9.0), (12.5, 4.8), (12.5, 19.2), (7.5, 15.0), (3.5, 15.0));
            line!((15.5, 9.0), (16.8, 12.0), (15.5, 15.0));
            line!((18.0, 6.0), (20.5, 12.0), (18.0, 18.0));
        }
        Ico::Device => {
            p.quad(bx(3.5, 5.0, 17.0, 11.5), 2.0 * s, Rgba::TRANSPARENT, c, w);
            line!((8.0, 19.5), (16.0, 19.5));
        }
        Ico::Sparkle => fill!((12.0, 3.0), (13.8, 10.2), (21.0, 12.0), (13.8, 13.8), (12.0, 21.0), (10.2, 13.8), (3.0, 12.0), (10.2, 10.2)),
        Ico::Clock => {
            p.oval(bx(3.5, 3.5, 17.0, 17.0), Rgba::TRANSPARENT, c, w);
            line!((12.0, 7.0), (12.0, 12.0), (15.5, 14.0));
        }
        Ico::Queue => {
            line!((4.0, 6.0), (20.0, 6.0));
            line!((4.0, 12.0), (14.0, 12.0));
            line!((4.0, 18.0), (14.0, 18.0));
            fill!((16.5, 10.0), (21.0, 14.5), (16.5, 19.0));
        }
    }
}

/// A glossy round button: `class` picks the finish (`orb`, `go`, `pearl`).
fn round(id: &'static str, class: &str, d: f32, child: Div) -> Div {
    div().id(id).class(format!("btn {class}")).css("width", rem(d)).css("height", rem(d))
        .child(div().class("shine")).child(child)
}

/// Play and pause stacked; the one showing scales and un-blurs in as the other goes.
fn play_pause(playing: bool, color: u32, size: f32) -> Div {
    let (a, b) = if playing { ("ico off", "ico on") } else { ("ico on", "ico off") };
    div().class("swap").css("width", rem(size)).css("height", rem(size))
        .child(ico(Ico::Play, color, size).class(a).css("position", "absolute"))
        .child(ico(Ico::Pause, color, size).class(b).css("position", "absolute"))
}

fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

struct Aero {
    sky: Fx,
    cover: Fx,
    playing: bool,
    track: usize,
    /// Seconds into the track.
    pos: f32,
    /// `now()` at the last frame.
    last: f32,
    seek: Dial,
    volume: Dial,
    liked: [bool; TRACKS.len()],
    /// Likes so far: keys the pop, so it plays again.
    likes: u32,
    nav: usize,
    playlist: usize,
    shuffle: bool,
    repeat: bool,
    fit: Fit,
    frames: u32,
}

impl Aero {
    const fn len(&self) -> f32 {
        TRACKS[self.track].2 as f32
    }

    const fn play(&mut self, i: usize) {
        if i == self.track {
            self.playing = !self.playing;
        } else {
            self.track = i;
            self.pos = 0.0;
            self.playing = true;
        }
    }

    fn skip(&mut self, forward: bool) {
        let n = TRACKS.len();
        // back within the first 3 s goes to the previous track, else to the start of this one
        if !forward && self.pos > 3.0 {
            self.pos = 0.0;
            return;
        }
        let step = if self.shuffle { 3 } else { 1 };
        self.track = if forward { (self.track + step) % n } else { (self.track + n - step) % n };
        self.pos = 0.0;
    }

    /// The time since the last frame: moves the playhead, the track ending into the next.
    fn tick(&mut self) {
        let t = now();
        let dt = (t - self.last).clamp(0.0, 0.25);
        self.last = t;
        if self.playing && !self.seek.dragging() {
            self.pos += dt;
            if self.pos >= self.len() {
                if self.repeat {
                    self.pos = 0.0;
                } else {
                    self.skip(true);
                }
            }
        }
        if !self.seek.dragging() {
            self.seek.value = self.pos / self.len();
        }
    }

    fn key(&mut self, key: &str) -> bool {
        match key {
            "space" => self.playing = !self.playing,
            "Right" => self.pos = (self.pos + 5.0).min(self.len() - 0.1),
            "Left" => self.pos = (self.pos - 5.0).max(0.0),
            "Up" => return self.volume.set(self.volume.value + 0.05),
            "Down" => return self.volume.set(self.volume.value - 0.05),
            "n" | "N" => self.skip(true),
            "p" | "P" => self.skip(false),
            _ => return false,
        }
        true
    }

    /// A window point in player-bar coordinates.
    fn in_player(&self, p: Point<Pixels>) -> (f32, f32) {
        let (x, y) = self.fit.design(p);
        (x - PLAYER.0, y - PLAYER.1)
    }

    fn top_bar() -> Div {
        div().class("rise")
            .child(div().class("abs logo txt").child(div().class("mark").child(div().class("shine"))).child("breeze"))
            .child(round("back", "abs orb", 40.0, ico(Ico::Back, 0xffffff, 20.0)).css("left", rem(184.0)).css("top", rem(12.0)))
            .child(round("fwd", "abs orb", 40.0, ico(Ico::Forward, 0xffffff, 20.0)).css("left", rem(230.0)).css("top", rem(12.0)))
            .child(div().class("abs glass search").child(div().class("gloss")).child(ico(Ico::Search, 0xffffff, 18.0))
                .child(span().child("What do you want to play?")))
            .child(div().id("upgrade").class("abs glass pill btn").css("left", rem(760.0)).css("top", rem(12.0))
                .child(div().class("gloss")).child(ico(Ico::Sparkle, 0xfff3a8, 16.0)).child("Upgrade"))
            .child(round("bell", "abs orb", 40.0, ico(Ico::Bell, 0xffffff, 18.0)).css("left", rem(898.0)).css("top", rem(12.0)))
            .child(div().class("abs glass pill").css("left", rem(952.0)).css("top", rem(12.0)).css("width", rem(232.0))
                .child(div().class("gloss")).child(div().class("avatar")).child(div().css("flex", "1").child("oceanic"))
                .child(ico(Ico::Chevron, 0xffffff, 14.0).css("transform", "rotate(90deg)")))
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let mut nav = div().class("nav");
        for (i, (name, icon)) in [("Home", Ico::Home), ("Search", Ico::Search), ("Your Library", Ico::Library)].into_iter().enumerate() {
            nav = nav.child(div().id(format!("nav{i}")).class(if i == self.nav { "navi sel txt" } else { "navi txt" })
                .child(ico(icon, 0xffffff, 20.0)).child(name)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.nav = i;
                    cx.notify();
                })));
        }
        let mut side = div().class("glass rise").css("left", rem(16.0)).css("top", rem(64.0)).css("width", rem(252.0))
            .css("height", rem(584.0)).css("animation-delay", "60ms")
            .child(div().class("gloss")).child(nav)
            .child(div().class("plhead txt").child("Playlists")
                .child(div().class("tools").child(ico(Ico::Plus, 0xffffff, 16.0)).child(ico(Ico::Chevron, 0xffffff, 16.0))));
        for (i, (name, meta, art)) in PLAYLISTS.iter().enumerate() {
            let glyph = if i == 0 { ico(Ico::Heart, 0xffffff, 20.0) } else { div() };
            side = side.child(div().id(format!("pl{i}")).class(if i == self.playlist { "pl sel" } else { "pl" })
                .child(div().class("art").css("background", *art).child(div().class("shine")).child(glyph))
                .child(div().class("txt").child(div().class("name").child(*name)).child(div().class("meta").child(*meta)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.playlist = i;
                    cx.notify();
                })));
        }
        side
    }

    fn like(&self, i: usize, cx: &mut Context<Self>) -> Div {
        let yes = self.liked[i];
        let mut d = div().id(format!("like{i}-{}", if yes { self.likes } else { 0 }))
            .class(if yes { "like yes pop" } else { "like" })
            .child(ico(if yes { Ico::Check } else { Ico::Plus }, 0xffffff, 14.0));
        d = d.on_click(cx.listener(move |this, _, _, cx| {
            this.liked[i] = !this.liked[i];
            this.likes += 1;
            cx.stop_propagation();
            cx.notify();
        }));
        d
    }

    fn main_panel(&self, cx: &mut Context<Self>, t: f32) -> Div {
        let playing = self.playing;
        let cover = self.cover.canvas(move |_| [t, 0.55, if playing { 1.0 } else { 0.0 }]).class("abs cover");
        let total: u32 = TRACKS.iter().map(|t| t.2).sum();
        let big = round("play", "go", 60.0, play_pause(self.playing, 0xffffff, 26.0))
            .on_click(cx.listener(|this, _, _, cx| {
                this.playing = !this.playing;
                cx.notify();
            }));
        let mut list = div().class("abs list");
        for (i, (title, plays, secs)) in TRACKS.iter().enumerate() {
            let on = i == self.track;
            let n = if on {
                // the equalizer: four bars, each on its own beat while it plays
                let mut eq = div().class("eq");
                for k in 0..4 {
                    let f = k as f32;
                    let h = if self.playing { 0.25 + 0.75 * (t * (5.0 + f * 1.7) + f * 1.3).sin().abs() } else { 0.3 };
                    eq = eq.child(div().css("height", rem(16.0 * h)));
                }
                eq
            } else {
                div().child(format!("{}", i + 1))
            };
            let row = div().id(format!("row{i}")).class(if on { "row on txt" } else { "row txt" })
                .child(div().class("c-n").child(n))
                .child(div().class("c-t").child(div().css("font-weight", "600").child(*title)).child(div().class("a").child(ARTIST)))
                .child(div().class("c-p").child(*plays))
                .child(div().class("c-l").child(self.like(i, cx)))
                .child(div().class("c-d").child(clock(*secs as f32)))
                .child(div().class("c-m").child(ico(Ico::More, 0xffffff, 18.0)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.play(i);
                    cx.notify();
                }));
            list = list.child(div().class("rise").css("animation-delay", format!("{}ms", 260 + i * 40)).child(row));
        }
        div().class("glass rise").css("left", rem(280.0)).css("top", rem(64.0)).css("width", rem(904.0)).css("height", rem(584.0))
            .css("animation-delay", "120ms")
            .child(div().class("gloss")).child(cover)
            .child(div().class("abs kind txt").child("Album"))
            .child(div().class("abs title").child("Glasswing"))
            .child(div().class("abs by txt").child(div().class("avatar")).child(ARTIST))
            .child(div().class("abs meta2 txt").child(format!("2007 · {} songs, {} min {} sec", TRACKS.len(), total / 60, total % 60)))
            .child(div().class("abs actions").child(big)
                .child(round("add", "orb", 40.0, ico(Ico::Plus, 0xffffff, 20.0)))
                .child(round("dl", "orb", 40.0, ico(Ico::Download, 0xffffff, 18.0)))
                .child(round("more", "orb", 40.0, ico(Ico::More, 0xffffff, 18.0))))
            .child(div().class("abs thead txt").child(div().class("c-n").child("#")).child(div().class("c-t").child("Title"))
                .child(div().class("c-p").child("Plays")).child(div().class("c-l")).child(div().class("c-d").child(ico(Ico::Clock, 0xffffff, 16.0).css("margin-left", "auto")))
                .child(div().class("c-m")))
            .child(list)
    }

    fn player(&self, cx: &mut Context<Self>, t: f32) -> Div {
        let playing = self.playing;
        let mini = self.cover.canvas(move |_| [t, 0.55, if playing { 1.0 } else { 0.0 }]).class("mini");
        let scale = self.fit.scale;
        let pp = round("pp", "pearl", 48.0, play_pause(self.playing, 0x1d4f8a, 22.0))
            .on_click(cx.listener(|this, _, _, cx| {
                this.playing = !this.playing;
                cx.notify();
            }));
        let tog = |id: &'static str, on: bool, k: Ico| div().id(id).class(if on { "btn tog lit" } else { "btn tog" })
            .css("width", rem(40.0)).css("height", rem(40.0))
            .child(ico(k, if on { 0xc6ff9e } else { 0xffffff }, 20.0));
        let ctrl = div().class("abs ctrl")
            .child(tog("shuffle", self.shuffle, Ico::Shuffle).on_click(cx.listener(|this, _, _, cx| {
                this.shuffle = !this.shuffle;
                cx.notify();
            })))
            .child(div().id("prev").class("btn").css("width", rem(40.0)).css("height", rem(40.0)).child(ico(Ico::Prev, 0xffffff, 22.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.skip(false);
                    cx.notify();
                })))
            .child(pp)
            .child(div().id("next").class("btn").css("width", rem(40.0)).css("height", rem(40.0)).child(ico(Ico::Next, 0xffffff, 22.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.skip(true);
                    cx.notify();
                })))
            .child(tog("repeat", self.repeat, Ico::Repeat).on_click(cx.listener(|this, _, _, cx| {
                this.repeat = !this.repeat;
                cx.notify();
            })));
        // the scrubber: press anywhere to jump there, then drag (the dial takes it from the jump)
        let pct = format!("{}%", self.seek.value * 100.0);
        let scrub = draggable(div().id("scrub").class(if self.seek.dragging() { "abs bar grab" } else { "abs bar" }), cx, Axis::Right,
                              SCRUB.2 * scale, |a: &mut Self| &mut a.seek, |a| a.pos = a.seek.value * a.len())
            .css("left", rem(SCRUB.0)).css("top", rem(SCRUB.1)).css("width", rem(SCRUB.2))
            .child(div().class("track").child(div().class("fill").css("width", pct.clone())).child(div().class("knob").css("left", pct)));
        let vpct = format!("{}%", self.volume.value * 100.0);
        let vol = draggable(div().id("vol").class(if self.volume.dragging() { "abs bar grab" } else { "abs bar" }), cx, Axis::Right,
                            VOLUME.2 * scale, |a: &mut Self| &mut a.volume, |_| {})
            .css("left", rem(VOLUME.0)).css("top", rem(VOLUME.1)).css("width", rem(VOLUME.2))
            .child(div().class("track").child(div().class("fill").css("width", vpct.clone())).child(div().class("knob").css("left", vpct)));
        let (title, _, _) = TRACKS[self.track];
        div().class("glass rise").css("left", rem(PLAYER.0)).css("top", rem(PLAYER.1)).css("width", rem(1168.0)).css("height", rem(104.0))
            .css("animation-delay", "180ms")
            .on_mouse_down_capture(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, _| {
                let (x, y) = this.in_player(e.position);
                let hit = |(bx, by, bw): (f32, f32, f32)| (bx - 8.0..bx + bw + 8.0).contains(&x) && (by..by + 24.0).contains(&y);
                if hit(SCRUB) {
                    this.seek.set((x - SCRUB.0) / SCRUB.2);
                    this.pos = this.seek.value * this.len();
                } else if hit(VOLUME) {
                    this.volume.set((x - VOLUME.0) / VOLUME.2);
                }
            }))
            .child(div().class("gloss"))
            .child(div().class("abs now").child(mini)
                .child(div().class("txt").child(div().class("t").child(title)).child(div().class("a").child(ARTIST)))
                .child(self.like(self.track, cx)).child(ico(Ico::Plus, 0xffffff, 18.0)))
            .child(ctrl)
            .child(div().class("abs time txt").css("left", rem(SCRUB.0 - 54.0)).css("text-align", "right").child(clock(self.pos)))
            .child(scrub)
            .child(div().class("abs time txt").css("left", rem(SCRUB.0 + SCRUB.2 + 10.0)).child(clock(self.len())))
            .child(div().class("abs right").child(ico(Ico::Queue, 0xffffff, 20.0)).child(ico(Ico::Device, 0xffffff, 20.0))
                .child(ico(Ico::Volume, 0xffffff, 20.0)))
            .child(vol)
    }
}

impl Render for Aero {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // the sky always moves, so every frame renders
        w.request_animation_frame();
        focus_root(w, &mut self.frames);
        self.fit = Fit::new(w, DESIGN);
        self.tick();
        let t = now();
        let energy = if self.playing { 0.5 + 0.5 * (t * 7.3).sin().abs() * self.volume.value } else { 0.0 };
        let sky = self.sky.canvas(move |_| [t, energy]).class("sky");
        let app = div().class("app").css("left", rem(self.fit.ox)).css("top", rem(self.fit.oy))
            .child(Self::top_bar())
            .child(self.sidebar(cx))
            .child(self.main_panel(cx, t))
            .child(self.player(cx, t));
        html().css("font-size", self.fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root").attr("tabindex", "0")
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| if this.key(&e.keystroke.key) { cx.notify() }))
                .child(sky).child(app))
    }
}

/// Every feature in turn: the entrance, a hover, a click to play, the icon morph, pause, a
/// scrub, the volume, a like, skipping, shuffle and the keys.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        s.play("00-enter", &[0, 120, 300, 600, 1000]);
        s.hover("row2");
        s.play("01-hover", &[0, 200]);
        s.click("row2");
        s.play("02-click-plays", &[0, 60, 120, 300, 1500]);
        s.click("pp");
        s.play("03-pause", &[0, 60, 120, 300]);
        s.click("play");
        s.play("04-play", &[0, 300]);
        let b = s.cx.element_bounds(s.win, "scrub").expect("scrub");
        let y = b.origin.y + b.size.height / 2.0;
        s.cx.simulate_drag(s.win, (b.origin.x + b.size.width * 0.2, y), (b.origin.x + b.size.width * 0.7, y));
        s.play("05-scrub", &[0, 300]);
        let b = s.cx.element_bounds(s.win, "vol").expect("vol");
        let y = b.origin.y + b.size.height / 2.0;
        s.cx.simulate_click(s.win, b.origin.x + b.size.width * 0.3, y);
        s.play("06-volume", &[0]);
        s.click("like3-0");
        s.play("07-like", &[0, 100, 300]);
        s.click("shuffle");
        s.click("next");
        s.play("08-next", &[0, 300]);
        s.click("pl3");
        s.click("nav1");
        s.cx.simulate_key(s.win, " ", "space");
        s.play("09-keys", &[0, 300]);
    })
}

fn sky() -> Fx {
    Fx::new("aero-sky", SKY, 2, [74, 150, 230])
}

fn cover() -> Fx {
    Fx::new("aero-cover", COVER, 3, [90, 150, 220])
}

fn main() {
    if let Some(dir) = arg("--frames") {
        let (s, c) = (sky(), cover());
        if gpu_stills(&dir, 1200, 776, [("sky", [6.0f32, 0.0]), ("sky-later", [14.0, 0.8])], |g, f| g.draw(s.effect(), &[], f)).is_none() {
            eprintln!("sky shader: {:?}", s.error());
        }
        if gpu_stills(&dir, 352, 352, [("cover", [3.0f32, 0.55, 1.0])], |g, f| g.draw(c.effect(), &[], f)).is_none() {
            eprintln!("cover shader: {:?}", c.error());
        }
        return;
    }
    let make = || Aero {
        sky: sky(),
        cover: cover(),
        playing: false,
        track: 0,
        pos: 42.0,
        last: now(),
        seek: Dial::new(0.0),
        volume: Dial::new(0.72),
        liked: [true, false, false, false, true, false, false, false],
        likes: 0,
        nav: 0,
        playlist: 2,
        shuffle: false,
        repeat: false,
        fit: Fit { scale: 1.0, w: DESIGN.0, h: DESIGN.1, ox: 0.0, oy: 0.0 },
        frames: 0,
    };
    run_with(DESIGN.0, DESIGN.1, "Breeze", make, arg("--storyboard").map(storyboard));
}

//! Orbit Messenger: a Y2K-futurism chat client on BUI. A GPU shader is the world behind it: liquid
//! chrome metaballs with an iridescent thin-film sheen over a silver sky and a perspective grid;
//! one blob follows the pointer, a buzz makes them wobble. Over it, translucent aqua plastic
//! (CSS glass, a glossy top highlight), chrome orb avatars, iridescent bubble buttons and pill tabs.
//!
//! Chat: pick a buddy, type and press Enter (or the send orb): the bubble pops in, a starburst
//! fires, the buddy types (three bouncing dots) and answers. BUZZ! shakes the window and the
//! chrome. Files: click a disc to download it (a fill, then a tick). Status: drag the mood fader
//! (or turn the wheel on it) to retint the avatar and the whole chrome world; pick a status.
//!
//! Motion ("bubbly"): bubbles pop 260 ms with an overshoot, starbursts fly 520 ms, the tab pill
//! slides 280 ms, rows stagger 40 ms on first light; hover lifts 2 px, press scales to 0.96;
//! sparkles twinkle and the chrome keeps flowing underneath.
//!
//! `cargo run --release --bin y2k`
//! `-- --storyboard DIR` drives it headlessly (typing, send, buzz, tabs, a drag) and saves frames
//! along BUI's virtual clock; `-- --frames DIR` renders the chrome world on the GPU.

use std::borrow::Cow;
use std::sync::OnceLock;

use blenderui::prelude::*;
use bui_demos::canvas::{rem, rem_css, Fit};
use bui_demos::control::{draggable, Axis, Dial};
use bui_demos::{arg, gpu_stills, now, run_with, story::story, uv_in, Fx, Script};

/// U[0] = time, U[1] = mood 0..1, U[2..3] = pointer uv (top-down; < 0: none), U[4] = seconds
/// since the last buzz (< 0: none), U[5] = seconds since the last send (< 0: none).
const CHROME: &str = r"
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

float field(vec2 q, float t, float a, vec2 m) {
    float f = 0.0;
    for (int i = 0; i < 6; i++) {
        float k = float(i);
        vec2 c = vec2(a * (0.5 + 0.36 * sin(t * 0.21 * (1.0 + k * 0.13) + k * 1.7)),
                      0.5 + 0.3 * cos(t * 0.17 * (1.0 + k * 0.11) + k * 2.9));
        float r = 0.1 + 0.04 * sin(k * 3.1 + t * 0.3);
        vec2 d = q - c;
        f += r * r / (dot(d, d) + 0.0015);
    }
    if (m.x >= 0.0) { vec2 d = q - m; f += 0.011 / (dot(d, d) + 0.0015); }
    return f;
}

vec3 irid(float k) { return 0.5 + 0.5 * cos(6.2831853 * (k + vec3(0.0, 0.33, 0.67))); }

// what the chrome reflects: sky above, a dark floor below, a bright horizon and two studio lights
vec3 env(vec3 r) {
    float up = -r.y;
    vec3 sky = mix(vec3(0.72, 0.82, 1.0), vec3(1.0, 0.97, 1.0), smoothstep(0.0, 0.9, up));
    vec3 ground = mix(vec3(0.08, 0.1, 0.25), vec3(0.42, 0.48, 0.66), smoothstep(-0.9, 0.0, up));
    vec3 c = up > 0.0 ? sky : ground;
    c += smoothstep(0.05, 0.0, abs(up - 0.02)) * 0.7;
    c += pow(max(0.0, dot(r, normalize(vec3(-0.5, -0.6, 0.6)))), 30.0) * 1.6;
    c += pow(max(0.0, dot(r, normalize(vec3(0.7, -0.2, 0.7)))), 60.0) * 0.8;
    return c;
}

vec4 effect(vec2 p) {
    float t = U[0]; float mood = U[1]; float buzz = U[4]; float sent = U[5];
    vec2 uv = p / box_size;
    float a = box_size.x / box_size.y;
    vec2 q = vec2(uv.x * a, uv.y);
    // a buzz shakes the world
    if (buzz >= 0.0) q.x += sin(q.y * 34.0 + t * 70.0) * 0.012 * exp(-buzz * 4.0);
    vec2 m = U[2] >= 0.0 ? vec2(U[2] * a, U[3]) : vec2(-1.0);

    // the backdrop: a silver-lilac sky, rings, a perspective grid running to the horizon
    vec3 tint = irid(mood * 0.8 + 0.55);
    vec3 c = mix(vec3(0.82, 0.9, 1.0), vec3(0.93, 0.86, 1.0), uv.y);
    c = mix(c, c * (0.75 + 0.35 * tint), 0.35);
    float ring = abs(fract(length(q - vec2(a * 0.83, 0.2)) * 9.0 - t * 0.15) - 0.5);
    c += 0.05 * smoothstep(0.06, 0.0, ring - 0.44);
    if (uv.y > 0.64) {
        float z = 1.0 / (uv.y - 0.6);
        float gx = abs(fract((uv.x - 0.5) * z * 1.6) - 0.5);
        float gz = abs(fract(z * 0.8 - t * 0.35) - 0.5);
        float grid = smoothstep(0.04 * z * 0.3, 0.0, min(gx, gz) - 0.01);
        c = mix(c, vec3(0.35, 0.55, 1.0), grid * 0.45 * smoothstep(0.64, 0.8, uv.y));
    }

    // the chrome: metaballs, shaded by their slope (1 - 1/f keeps the slope bounded)
    float f = field(q, t, a, m);
    if (f > 0.9) {
        float e = 0.002;
        float s = 1.0 - 1.0 / f;
        float sx = 1.0 - 1.0 / field(q + vec2(e, 0.0), t, a, m);
        float sy = 1.0 - 1.0 / field(q + vec2(0.0, e), t, a, m);
        vec2 g = vec2(sx - s, sy - s) / e;
        vec3 n = normalize(vec3(-g * 0.09, 1.0));
        // the eye looks a little down onto the chrome, so its flat tops catch the sky
        vec3 r = reflect(normalize(vec3(0.0, 0.45, -1.0)), n);
        vec3 ch = env(r);
        float fres = pow(1.0 - n.z, 2.0);
        vec3 film = irid(n.x * 0.6 - n.y * 0.4 + mood + t * 0.03);
        ch = mix(ch, ch * film * 1.5, 0.35 + 0.4 * fres);
        if (sent >= 0.0) ch += vec3(0.6, 0.8, 1.0) * exp(-sent * 5.0) * 0.5;
        c = mix(c, ch, smoothstep(0.96, 1.04, f));
    }
    c += (hash(p + fract(t) * 37.0) - 0.5) * 0.02;
    return vec4(clamp(c, 0.0, 1.0), 1.0);
}
";

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; position: relative; overflow: hidden; background: #cfe0ff; user-select: none; color: #11214f;
       font-family: 'Trebuchet MS', Verdana, sans-serif; -webkit-font-smoothing: antialiased; }
.world { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
.abs { position: absolute; }
.stage { position: absolute; width: 1280px; height: 800px; }

/* -- sparkles -- */
.star { width: 26px; height: 26px; background: #fff;
        clip-path: polygon(50% 0, 60% 40%, 100% 50%, 60% 60%, 50% 100%, 40% 60%, 0 50%, 40% 40%);
        animation: twinkle 2400ms ease-in-out infinite; }

/* -- the window: aqua plastic -- */
.app { left: 100px; top: 64px; width: 1080px; height: 672px; border-radius: 36px;
       background: linear-gradient(180deg, rgba(232,246,255,0.62) 0%, rgba(160,210,255,0.42) 55%, rgba(130,180,255,0.5) 100%);
       backdrop-filter: blur(20px) saturate(1.7); border: 1px solid rgba(255,255,255,0.8);
       box-shadow: 0 40px 90px rgba(30,50,140,0.35), 0 2px 0 rgba(255,255,255,0.9) inset, 0 -8px 30px rgba(120,180,255,0.35) inset;
       animation: appin 600ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.gloss { left: 14px; top: 8px; right: 14px; height: 54px; border-radius: 30px;
         background: linear-gradient(180deg, rgba(255,255,255,0.85), rgba(255,255,255,0.0)); }
.shake { animation: shake 520ms cubic-bezier(0.36, 0.07, 0.19, 0.97); }
.shake2 { animation: shake2 520ms cubic-bezier(0.36, 0.07, 0.19, 0.97); }

/* -- title bar -- */
.top { left: 28px; top: 16px; right: 28px; height: 56px; display: flex; align-items: center; }
.lights { display: flex; gap: 10px; width: 260px; }
.orb { position: relative; border-radius: 50%; flex-shrink: 0; }
.orb .shine { position: absolute; left: 20%; top: 8%; width: 60%; height: 38%; border-radius: 50%;
              background: linear-gradient(180deg, rgba(255,255,255,0.95), rgba(255,255,255,0.05)); }
.light { width: 20px; height: 20px; box-shadow: 0 2px 4px rgba(0,0,40,0.35), inset 0 -2px 4px rgba(0,0,0,0.25); }
.logo { flex: 1; text-align: center; font-family: 'Arial Black', 'Trebuchet MS', sans-serif; font-style: italic; font-size: 30px;
        letter-spacing: -1px; color: #f4f8ff;
        text-shadow: 0 1px 0 #fff, 0 2px 0 #9fb2d8, 0 3px 0 #6c7fa8, 0 4px 0 #3e4f7a, 0 8px 14px rgba(20,40,120,0.45); }
.logo span { color: #7fdcff; }
.tabs { position: relative; width: 300px; height: 40px; border-radius: 20px; display: flex;
        background: linear-gradient(180deg, rgba(20,60,140,0.28), rgba(255,255,255,0.35));
        box-shadow: inset 0 2px 6px rgba(10,30,90,0.35), 0 1px 0 rgba(255,255,255,0.9); }
.pill { position: absolute; top: 4px; left: 4px; width: 96px; height: 32px; border-radius: 16px;
        background: linear-gradient(180deg, #ffffff 0%, #bfe8ff 45%, #5cc2ff 55%, #9fe4ff 100%);
        box-shadow: 0 2px 6px rgba(0,40,140,0.4), inset 0 1px 0 #fff;
        transition-property: left; transition-duration: 280ms; transition-timing-function: cubic-bezier(0.34, 1.4, 0.64, 1); }
.tab { position: relative; width: 100px; height: 40px; display: flex; align-items: center; justify-content: center; cursor: pointer;
       font-size: 13px; font-weight: 700; letter-spacing: 1.5px; color: #1d3a7a; transition-property: color; transition-duration: 160ms; }
.tab.on { color: #062063; }

/* -- the buddy list -- */
.side { left: 24px; top: 88px; width: 262px; bottom: 24px; border-radius: 26px; padding: 14px 10px;
        background: linear-gradient(180deg, rgba(255,255,255,0.55), rgba(220,238,255,0.3));
        box-shadow: inset 0 1px 0 #fff, 0 8px 24px rgba(30,60,150,0.15); box-sizing: border-box; }
.side h3 { margin: 0 8px 10px; font-size: 12px; letter-spacing: 2px; color: #3c5a9c; text-wrap: balance; }
.buddy { position: relative; display: flex; align-items: center; gap: 12px; height: 60px; padding: 0 10px; border-radius: 18px; cursor: pointer;
         transition-property: background-color, transform, box-shadow; transition-duration: 160ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1);
         animation: rowin 360ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.buddy:hover { background: rgba(255,255,255,0.45); transform: translateY(-2px); }
.buddy:active { transform: scale(0.96); }
.buddy.on { background: linear-gradient(180deg, rgba(255,255,255,0.95), rgba(170,225,255,0.85));
            box-shadow: 0 6px 16px rgba(30,90,200,0.25), inset 0 1px 0 #fff; }
.av { width: 40px; height: 40px; box-shadow: 0 3px 8px rgba(10,30,90,0.35), inset 0 -3px 6px rgba(0,0,40,0.3); }
.dot { position: absolute; right: -2px; bottom: -2px; width: 12px; height: 12px; border-radius: 6px; border: 2px solid #fff; }
.who { flex: 1; min-width: 0; }
.who b { display: block; font-size: 14px; color: #0d1f4f; }
.who i { display: block; font-size: 11px; font-style: normal; color: #4f68a3; white-space: nowrap; overflow: hidden; }
.badge { min-width: 22px; height: 22px; border-radius: 11px; color: #fff; font-size: 11px; font-weight: 700; display: flex; align-items: center;
         justify-content: center; font-variant-numeric: tabular-nums;
         background: linear-gradient(135deg, #ff7ad9, #8a7bff 50%, #4fe3ff);
         box-shadow: 0 2px 6px rgba(120,40,200,0.4), inset 0 1px 0 rgba(255,255,255,0.7);
         animation: pop 300ms cubic-bezier(0.34, 1.56, 0.64, 1) backwards; }

/* -- the main pane -- */
.main { left: 302px; top: 88px; right: 24px; bottom: 24px; border-radius: 26px;
        background: linear-gradient(180deg, rgba(255,255,255,0.5), rgba(235,245,255,0.35));
        box-shadow: inset 0 1px 0 #fff, 0 8px 24px rgba(30,60,150,0.15); overflow: hidden; }
.head { position: absolute; left: 0; right: 0; top: 0; height: 66px; display: flex; align-items: center; gap: 14px; padding: 0 22px;
        border-bottom: 1px solid rgba(255,255,255,0.7); }
.head .av { width: 42px; height: 42px; }
.head b { font-size: 17px; }
.head i { display: block; font-size: 12px; font-style: normal; color: #4f68a3; }
.typing { display: inline-flex; gap: 4px; margin-left: 6px; }
.typing span { width: 6px; height: 6px; border-radius: 3px; background: #4f8bff; animation: bob 900ms ease-in-out infinite; }
.log { position: absolute; left: 0; right: 0; top: 66px; bottom: 82px; padding: 16px 22px; display: flex; flex-direction: column;
       justify-content: flex-end; gap: 10px; overflow: hidden; }
.msg { max-width: 62%; padding: 10px 16px; border-radius: 20px; font-size: 15px; line-height: 1.35; text-wrap: pretty;
       animation: pop 280ms cubic-bezier(0.34, 1.56, 0.64, 1) backwards; }
.msg.them { align-self: flex-start; border-bottom-left-radius: 6px; color: #0e2458;
            background: linear-gradient(180deg, #ffffff, #d6efff); box-shadow: 0 3px 10px rgba(30,80,180,0.18), inset 0 1px 0 #fff; }
.msg.me { align-self: flex-end; border-bottom-right-radius: 6px; color: #fff; text-shadow: 0 1px 1px rgba(20,20,90,0.35);
          background: linear-gradient(135deg, #7fdcff 0%, #8a8bff 45%, #ff8be0 100%);
          box-shadow: 0 4px 14px rgba(120,60,220,0.3), inset 0 1px 0 rgba(255,255,255,0.7); }
.msg.buzz { align-self: center; font-weight: 700; letter-spacing: 3px; color: #ff4fb8; background: none; box-shadow: none; font-size: 13px; }

/* -- the compose bar -- */
.compose { position: absolute; left: 18px; right: 18px; bottom: 16px; height: 52px; display: flex; gap: 10px; align-items: center; }
.field { flex: 1; height: 46px; border-radius: 23px; padding: 6px 18px; box-sizing: border-box; border: 1px solid rgba(255,255,255,0.9);
         background: linear-gradient(180deg, rgba(220,235,255,0.9), #ffffff); box-shadow: inset 0 2px 5px rgba(20,50,140,0.25); }
.raw { width: 100%; height: 100%; border: none; background: transparent; font-size: 15px; color: #0e2458; outline: none;
       font-family: 'Trebuchet MS', Verdana, sans-serif; }
.bubble { position: relative; height: 46px; border-radius: 23px; padding: 0 20px; display: flex; align-items: center; justify-content: center;
          font-size: 13px; font-weight: 700; letter-spacing: 1.5px; color: #fff; cursor: pointer; text-shadow: 0 1px 1px rgba(0,0,60,0.4);
          background: linear-gradient(180deg, #c9f1ff 0%, #4fc0ff 48%, #1b7fe8 52%, #6fd2ff 100%);
          box-shadow: 0 4px 10px rgba(0,60,180,0.35), inset 0 1px 0 rgba(255,255,255,0.9);
          transition-property: transform, box-shadow; transition-duration: 140ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.bubble:hover { transform: translateY(-2px); box-shadow: 0 8px 16px rgba(0,60,180,0.35), inset 0 1px 0 rgba(255,255,255,0.9); }
.bubble:active { transform: scale(0.96); }
.bubble.pink { background: linear-gradient(180deg, #ffd6f3 0%, #ff7ad9 48%, #e83fb8 52%, #ff9ee6 100%); box-shadow: 0 4px 10px rgba(200,40,150,0.35), inset 0 1px 0 #fff; }
.send { width: 52px; height: 52px; padding: 0; border-radius: 26px; font-size: 18px;
        background: linear-gradient(135deg, #a6f0ff 0%, #8a8bff 45%, #ff8be0 100%); }
.burst { position: absolute; left: 50%; top: 50%; width: 0; height: 0; pointer-events: none; }
.ray { position: absolute; left: 0; top: 0; width: 0; height: 0; }
.ray .star { position: absolute; left: -8px; top: -8px; width: 16px; height: 16px; animation: fly 560ms cubic-bezier(0.2, 0, 0, 1) forwards; }

/* -- files -- */
.files { position: absolute; left: 22px; right: 22px; top: 84px; display: flex; flex-wrap: wrap; gap: 18px; }
.file { width: 220px; height: 150px; border-radius: 22px; padding: 14px; box-sizing: border-box; cursor: pointer; position: relative;
        background: linear-gradient(180deg, rgba(255,255,255,0.8), rgba(210,232,255,0.6)); box-shadow: 0 6px 18px rgba(30,70,170,0.18), inset 0 1px 0 #fff;
        transition-property: transform, box-shadow; transition-duration: 160ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1);
        animation: rowin 360ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.file:hover { transform: translateY(-3px); box-shadow: 0 12px 24px rgba(30,70,170,0.25), inset 0 1px 0 #fff; }
.file:active { transform: scale(0.96); }
.disc { width: 64px; height: 64px; box-shadow: 0 4px 10px rgba(20,40,120,0.3);
        background: linear-gradient(135deg, #e9f3ff 0%, #ffc4f0 25%, #c3fff4 45%, #d9d2ff 65%, #fff6c4 85%, #e9f3ff 100%); }
.disc .hole { position: absolute; left: 24px; top: 24px; width: 16px; height: 16px; border-radius: 8px; background: rgba(200,220,255,0.9);
              box-shadow: inset 0 1px 3px rgba(0,0,60,0.4); }
.disc.spin { animation: spin 900ms linear infinite; }
.file b { position: absolute; left: 92px; top: 20px; right: 12px; font-size: 13px; color: #0d1f4f; word-break: break-all; }
.file small { position: absolute; left: 92px; top: 58px; font-size: 11px; color: #4f68a3; font-variant-numeric: tabular-nums; }
.prog { position: absolute; left: 14px; right: 14px; bottom: 16px; height: 12px; border-radius: 6px; background: rgba(20,60,150,0.15);
        box-shadow: inset 0 1px 3px rgba(10,30,90,0.3); overflow: hidden; }
.prog i { display: block; height: 100%; border-radius: 6px; background: linear-gradient(90deg, #7fdcff, #8a8bff, #ff8be0);
          transform-origin: left center; animation: fill 1600ms cubic-bezier(0.4, 0, 0.2, 1) backwards; }
.prog.done i { animation: none; }
.saved { position: absolute; right: 14px; bottom: 32px; font-size: 11px; font-weight: 700; color: #1b9a6a; letter-spacing: 1px;
         animation: pop 260ms cubic-bezier(0.34, 1.56, 0.64, 1) backwards; }

/* -- status -- */
.mine { position: absolute; left: 40px; top: 90px; right: 40px; display: flex; gap: 46px; align-items: center; }
.big { width: 200px; height: 200px; box-shadow: 0 20px 40px rgba(20,40,140,0.35), inset 0 -10px 24px rgba(0,0,60,0.3); }
.halo { position: absolute; left: -14px; top: -14px; width: 228px; height: 228px; border-radius: 50%; border: 3px solid rgba(255,255,255,0.8);
        animation: breathe 2600ms ease-in-out infinite; }
.panel { flex: 1; }
.panel h2 { margin: 0 0 4px; font-size: 26px; text-wrap: balance; }
.panel p { margin: 0 0 22px; color: #4f68a3; font-size: 14px; }
.lbl { font-size: 11px; letter-spacing: 2px; font-weight: 700; color: #3c5a9c; margin-bottom: 8px; display: flex; justify-content: space-between; }
.lbl span { font-variant-numeric: tabular-nums; }
.fader { position: relative; height: 44px; cursor: ew-resize; margin-bottom: 26px; }
.track { position: absolute; left: 0; right: 0; top: 16px; height: 12px; border-radius: 6px;
         background: linear-gradient(90deg, #ff7ad9, #ffd36e, #7dffb2, #4fe3ff, #8a7bff, #ff7ad9);
         box-shadow: inset 0 2px 4px rgba(0,0,60,0.35), 0 1px 0 #fff; }
.thumb { position: absolute; top: 4px; width: 36px; height: 36px; margin-left: -18px;
         background: linear-gradient(180deg, #ffffff 0%, #d9e4f5 45%, #9aaccb 55%, #eef4ff 100%);
         box-shadow: 0 4px 10px rgba(0,20,80,0.4), inset 0 1px 0 #fff; transition-property: transform; transition-duration: 120ms; }
.thumb.held { transform: scale(1.12); }
.states { display: flex; gap: 10px; }
.state { opacity: 0.72; transition-property: opacity, transform, box-shadow; }
.state.on { opacity: 1; }

/* -- motion -- */
@keyframes appin { from { opacity: 0; transform: translateY(24px) scale(0.98); } to { opacity: 1; transform: translateY(0) scale(1); } }
@keyframes rowin { from { opacity: 0; transform: translateX(-16px); } to { opacity: 1; transform: translateX(0); } }
@keyframes pop { from { opacity: 0; transform: scale(0.6); } to { opacity: 1; transform: scale(1); } }
@keyframes twinkle { 0%, 100% { opacity: 0.25; transform: scale(0.6) rotate(0deg); } 50% { opacity: 1; transform: scale(1) rotate(45deg); } }
@keyframes fly { 0% { opacity: 1; transform: translateX(0) scale(0.4) rotate(0deg); } 100% { opacity: 0; transform: translateX(78px) scale(1.1) rotate(90deg); } }
@keyframes shake { 0%, 100% { transform: translateX(0); } 15% { transform: translateX(-14px) rotate(-1deg); } 30% { transform: translateX(12px) rotate(1deg); }
                   45% { transform: translateX(-9px); } 60% { transform: translateX(7px); } 75% { transform: translateX(-4px); } }
@keyframes shake2 { 0%, 100% { transform: translateX(0); } 15% { transform: translateX(-14px) rotate(-1deg); } 30% { transform: translateX(12px) rotate(1deg); }
                    45% { transform: translateX(-9px); } 60% { transform: translateX(7px); } 75% { transform: translateX(-4px); } }
@keyframes bob { 0%, 100% { transform: translateY(0); opacity: 0.5; } 50% { transform: translateY(-5px); opacity: 1; } }
@keyframes fill { from { transform: scaleX(0); } to { transform: scaleX(1); } }
@keyframes breathe { 0%, 100% { transform: scale(1); opacity: 0.5; } 50% { transform: scale(1.04); opacity: 1; } }
@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
";

const DESIGN: (f32, f32) = (1280.0, 800.0);

/// One buddy: screen name, away line, avatar colours (light, deep), online, and what they say.
struct Buddy {
    name: &'static str,
    line: &'static str,
    colors: (&'static str, &'static str),
    online: bool,
    replies: [&'static str; 4],
}

const BUDDIES: [Buddy; 6] = [
    Buddy { name: "xX_starla_Xx", line: "♫ dancing in the matrix", colors: ("#ffd1f0", "#ff4fb8"), online: true,
            replies: ["omg yes!!! ✦", "brb changing my away msg", "that is SO 3000", "lol ur a genius"] },
    Buddy { name: "neo.kid", line: "brb saving the world", colors: ("#c9f1ff", "#1b7fe8"), online: true,
            replies: ["copy that.", "the server room is glowing again", "send the file when u can", "ok ok one sec"] },
    Buddy { name: "cyb3rangel", line: "~*~ away ~*~", colors: ("#e7dcff", "#7b5cff"), online: true,
            replies: ["*floats in*", "wings recharging…", "u up?", "i saw a comet today"] },
    Buddy { name: "dj_mercury", line: "now playing: chrome heart", colors: ("#d4fff3", "#16b392"), online: true,
            replies: ["drop the bass", "new mix at midnight", "turn it UP", "that loop slaps"] },
    Buddy { name: "lil.pixel", line: "offline since 11:42 pm", colors: ("#f2ffc9", "#8ccf1e"), online: false,
            replies: ["(auto) i'm away, leave a msg!", "(auto) back soon", "(auto) zzz", "(auto) offline"] },
    Buddy { name: "orbit.bot", line: "your friendly orbit bot", colors: ("#fff3c9", "#ff9f1a"), online: true,
            replies: ["beep! message received.", "orbit stable at 99.9%", "did you know? chrome is a mood.", "boop."] },
];

const FILES: [(&str, &str); 6] = [
    ("summer_mix_2000.mp3", "4.2 MB"), ("rave_pics.zip", "18.6 MB"), ("chrome_heart.wav", "31.0 MB"),
    ("skin_aqua_v2.skn", "860 KB"), ("away_msgs.txt", "2 KB"), ("comet.avi", "64.3 MB"),
];

/// Seconds a download takes (the CSS fill runs this long too).
const DOWNLOAD: f32 = 1.6;
/// Seconds a buddy types before answering.
const TYPING: f32 = 1.3;
const STATES: [(&str, &str); 3] = [("ONLINE", "#16c07a"), ("AWAY", "#ffb21a"), ("INVISIBLE", "#9aa6c2")];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Chat,
    Files,
    Status,
}

const TABS: [(Tab, &str); 3] = [(Tab::Chat, "CHAT"), (Tab::Files, "FILES"), (Tab::Status, "STATUS")];

struct Msg {
    id: u32,
    from_me: bool,
    buzz: bool,
    text: Cow<'static, str>,
}

struct Orbit {
    world: Fx,
    tab: Tab,
    sel: usize,
    /// Each buddy's conversation, and how many of its lines are unread.
    chats: [Vec<Msg>; BUDDIES.len()],
    unread: [u8; BUDDIES.len()],
    /// Each buddy's next canned reply.
    said: [usize; BUDDIES.len()],
    /// A buddy typing an answer, since when.
    typing: Option<(usize, f32)>,
    draft: String,
    msg_n: u32,
    /// Bumped per send / buzz: keys the starburst and the shake so they replay.
    bursts: u32,
    buzzes: u32,
    sent_at: f32,
    buzzed_at: f32,
    /// When each file's download began.
    downloads: [Option<f32>; FILES.len()],
    mood: Dial,
    status: usize,
    pointer: Option<(f32, f32)>,
    frames: u32,
}

impl Orbit {
    fn push(&mut self, buddy: usize, from_me: bool, buzz: bool, text: Cow<'static, str>) {
        self.msg_n += 1;
        let log = &mut self.chats[buddy];
        log.push(Msg { id: self.msg_n, from_me, buzz, text });
        // only the last few fit; older lines scroll away
        if log.len() > 7 {
            log.remove(0);
        }
        if buddy != self.sel || self.tab != Tab::Chat {
            self.unread[buddy] = self.unread[buddy].saturating_add(1);
        }
    }

    fn send(&mut self) -> bool {
        let text = self.draft.trim();
        if text.is_empty() {
            return false;
        }
        let text = Cow::Owned(text.to_string());
        self.draft.clear();
        self.push(self.sel, true, false, text);
        self.bursts += 1;
        self.sent_at = now();
        self.typing = Some((self.sel, now()));
        true
    }

    fn buzz(&mut self) {
        self.push(self.sel, true, true, Cow::Borrowed("✦ BUZZ! ✦"));
        self.buzzes += 1;
        self.bursts += 1;
        self.buzzed_at = now();
        self.typing = Some((self.sel, now()));
    }

    const fn pick(&mut self, i: usize) {
        self.sel = i;
        self.unread[i] = 0;
        self.tab = Tab::Chat;
    }

    /// A buddy who has typed long enough answers.
    fn tick(&mut self) {
        if let Some((b, since)) = self.typing
            && now() - since > TYPING
        {
            self.typing = None;
            let k = self.said[b];
            self.said[b] = (k + 1) % BUDDIES[b].replies.len();
            self.push(b, false, false, Cow::Borrowed(BUDDIES[b].replies[k]));
        }
    }
}

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

/// A glossy orb (`class` includes `orb`): a radial gradient from `light` to `deep`, with a shine on top.
fn orb(light: &str, deep: &str, class: &'static str) -> Div {
    div().class(class).css("background", format!("radial-gradient(circle at 35% 30%, {light} 0%, {deep} 75%, {deep} 100%)"))
        .child(div().class("shine"))
}

/// A starburst: `n` stars flying out on rays.
fn burst(id: u32, n: usize) -> Div {
    let mut d = div().id(format!("burst{id}")).class("burst");
    for k in 0..n {
        d = d.child(div().class("ray").css("transform", format!("rotate({}deg)", k * 360 / n + 10))
            .child(div().class("star").css("animation-delay", format!("{}ms", (k % 3) * 30))));
    }
    d
}

/// The colour the mood fader gives (the iridescent film's hue): `t` 0..1.
fn mood_color(t: f32) -> String {
    let c = |o: f32| (0.5 + 0.5 * (std::f32::consts::TAU * (t * 0.8 + 0.55 + o)).cos()) * 255.0;
    format!("rgb({:.0}, {:.0}, {:.0})", c(0.0), c(0.33), c(0.67))
}

impl Orbit {
    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let online = BUDDIES.iter().filter(|b| b.online).count();
        let mut side = div().class("abs side").child(h3().child(format!("BUDDIES  {online}/{}", BUDDIES.len())));
        for (k, bd) in BUDDIES.iter().enumerate() {
            let on = k == self.sel;
            let mut row = div().id(format!("buddy{k}")).class(if on { "buddy on" } else { "buddy" })
                .css("animation-delay", format!("{}ms", 260 + k * 40))
                .child(orb(bd.colors.0, bd.colors.1, "orb av")
                    .child(div().class("dot").css("background", if bd.online { "#16c07a" } else { "#9aa6c2" })))
                .child(div().class("who").child(b().child(bd.name)).child(i().child(bd.line)));
            if self.unread[k] > 0 {
                row = row.child(div().id(format!("badge{k}-{}", self.unread[k])).class("badge").child(format!("{}", self.unread[k])));
            }
            side = side.child(row.on_click(cx.listener(move |this, _, _, cx| {
                this.pick(k);
                cx.notify();
            })));
        }
        side
    }

    fn chat(&self, cx: &mut Context<Self>) -> Div {
        let bd = &BUDDIES[self.sel];
        let typing = self.typing.is_some_and(|(t, _)| t == self.sel);
        let mut status = i().child(if typing { "is typing" } else { bd.line });
        if typing {
            status = status.child(span().class("typing").child(span()).child(span().css("animation-delay", "150ms"))
                .child(span().css("animation-delay", "300ms")));
        }
        let head = div().class("head").child(orb(bd.colors.0, bd.colors.1, "orb av")).child(div().child(b().child(bd.name)).child(status));
        let mut log = div().class("log");
        for m in &self.chats[self.sel] {
            let class = match (m.buzz, m.from_me) {
                (true, _) => "msg buzz",
                (false, true) => "msg me",
                (false, false) => "msg them",
            };
            log = log.child(div().id(format!("msg{}", m.id)).class(class).child(m.text.to_string()));
        }
        // keyed by the sends: a fresh, empty field after each one
        let field = input().id(format!("field{}", self.bursts)).class("raw").placeholder(format!("Message {}…", bd.name)).value(self.draft.clone())
            .on_input(cx.listener(|this, e: &ChangeEvent, _, _| this.draft.clone_from(&e.value)))
            // the field commits on Enter (a Blender text field): that is the send
            .on_change(cx.listener(|this, e: &ChangeEvent, _, cx| {
                this.draft.clone_from(&e.value);
                if this.send() {
                    cx.notify();
                }
            }));
        let mut send = div().id("send").class("bubble send").child("➤")
            .on_click(cx.listener(|this, _, _, cx| if this.send() { cx.notify() }));
        if self.sent_at > self.buzzed_at {
            send = send.child(burst(self.bursts, 10));
        }
        let mut buzz = div().id("buzz").class("bubble pink").child("BUZZ!").on_click(cx.listener(|this, _, _, cx| {
            this.buzz();
            cx.notify();
        }));
        if self.buzzed_at > self.sent_at {
            buzz = buzz.child(burst(self.bursts, 10));
        }
        let compose = div().class("compose")
            .child(div().class("field").child(field))
            .child(buzz)
            .child(send);
        div().child(head).child(log).child(compose)
    }

    fn files(&self, cx: &mut Context<Self>) -> Div {
        let t = now();
        let mut grid = div().class("files");
        for (k, (name, size)) in FILES.iter().enumerate() {
            let state = self.downloads[k].map(|s| t - s);
            let mut card = div().id(format!("file{k}")).class("file").css("animation-delay", format!("{}ms", k * 40))
                .child(div().class(if state.is_some_and(|s| s < DOWNLOAD) { "orb disc spin" } else { "orb disc" })
                    .child(div().class("shine")).child(div().class("hole")))
                .child(b().child(*name)).child(small().child(*size));
            card = match state {
                None => card.child(div().class("prog").child(i().css("transform", "scaleX(0)").css("animation", "none"))),
                Some(s) if s < DOWNLOAD => card.child(div().id(format!("prog{k}")).class("prog").child(i())),
                Some(_) => card.child(div().class("prog done").child(i())).child(div().class("saved").child("✓ SAVED")),
            };
            grid = grid.child(card.on_click(cx.listener(move |this, _, _, cx| {
                if this.downloads[k].is_none() {
                    this.downloads[k] = Some(now());
                    cx.notify();
                }
            })));
        }
        div().child(div().class("head").child(b().child("Shared files")).child(i().child("click a disc to download"))).child(grid)
    }

    fn status(&self, cx: &mut Context<Self>) -> Div {
        let mood = self.mood.value;
        let tint = mood_color(mood);
        let avatar = div().css("position", "relative")
            .child(div().class("halo").css("border-color", tint.clone()))
            .child(orb("#ffffff", &tint, "orb big"));
        let fader = draggable(div().id("mood").class("fader"), cx, Axis::Right, 520.0, |v: &mut Self| &mut v.mood, |_| {})
            .child(div().class("track"))
            .child(orb("#ffffff", "#aebdd8", if self.mood.dragging() { "orb thumb held" } else { "orb thumb" }).css("left", format!("{:.1}%", mood * 100.0)));
        let mut states = div().class("states");
        for (k, (name, color)) in STATES.iter().enumerate() {
            states = states.child(div().id(format!("state{k}")).class(if k == self.status { "bubble state on" } else { "bubble state" })
                .css("background", format!("linear-gradient(180deg, #ffffff 0%, {color} 50%, {color} 100%)"))
                .child(*name)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.status = k;
                    cx.notify();
                })));
        }
        div().child(div().class("head").child(b().child("My status")).child(i().child("how the world sees you")))
            .child(div().class("mine").child(avatar).child(div().class("panel")
                .child(h2().child("orbit_me_2000"))
                .child(p().child("~*~ living in the future, one buzz at a time ~*~"))
                .child(div().class("lbl").child("MOOD · DRAG OR SCROLL").child(span().child(format!("{:.0}%", mood * 100.0))))
                .child(fader)
                .child(div().class("lbl").child("STATUS"))
                .child(states)))
    }
}

impl Render for Orbit {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        self.frames += 1;
        self.tick();
        let fit = Fit::new(w, DESIGN);
        let (mood, pointer, buzzed, sent) = (self.mood.value, self.pointer, self.buzzed_at, self.sent_at);
        let world = self.world.canvas(move |b| {
            let t = now();
            let [u, v] = pointer.map_or([-1.0, -1.0], |(x, y)| {
                let [u, v] = uv_in(b, x, y);
                [u, 1.0 - v]
            });
            let age = |at: f32| if at > 0.0 { t - at } else { -1.0 };
            [t, mood, u, v, age(buzzed), age(sent)]
        }).class("world");

        let mut stage = div().class("stage").css("left", rem(fit.ox)).css("top", rem(fit.oy));
        for (x, y, s, d) in [(70.0, 90.0, 1.0, 0), (1200.0, 60.0, 0.7, 600), (1215.0, 690.0, 1.2, 1200), (40.0, 600.0, 0.8, 1800),
                             (640.0, 30.0, 0.6, 900), (1236.0, 380.0, 0.5, 300)] {
            stage = stage.child(div().class("abs star").css("left", rem(x)).css("top", rem(y)).css("width", rem(26.0 * s))
                .css("height", rem(26.0 * s)).css("animation-delay", format!("{d}ms")));
        }

        let mut tabs = div().class("tabs").child(div().class("pill").css("left", rem(4.0 + 100.0 * TABS.iter().position(|t| t.0 == self.tab).unwrap_or(0) as f32)));
        for (tab, name) in TABS {
            tabs = tabs.child(div().id(name).class(if tab == self.tab { "tab on" } else { "tab" }).child(name)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.tab = tab;
                    if tab == Tab::Chat {
                        this.unread[this.sel] = 0;
                    }
                    cx.notify();
                })));
        }
        let top = div().class("abs top")
            .child(div().class("lights").child(orb("#ffd6d6", "#ff4a5a", "orb light")).child(orb("#fff1c9", "#ffb21a", "orb light"))
                .child(orb("#d4ffd9", "#16c07a", "orb light")))
            .child(div().class("logo").child("ORBIT").child(span().child("·")).child("MSG"))
            .child(tabs);
        let pane = match self.tab {
            Tab::Chat => self.chat(cx),
            Tab::Files => self.files(cx),
            Tab::Status => self.status(cx),
        };
        // two names for one shake, alternated per buzz, so each buzz restarts it on the same element
        let app = div().id("app").class(match self.buzzes {
            0 => "abs app",
            n if n % 2 == 1 => "abs app shake",
            _ => "abs app shake2",
        })
            .child(div().class("abs gloss"))
            .child(top)
            .child(self.sidebar(cx))
            .child(div().class("abs main").child(pane));
        stage = stage.child(app);

        html().css("font-size", fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root")
                .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, _| this.pointer = Some((e.position.x.0, e.position.y.0))))
                .on_mouse_leave(cx.listener(|this, _: &MouseMoveEvent, _, _| this.pointer = None))
                .child(world)
                .child(div().class("abs").css("left", "0").css("top", "0").css("width", rem(fit.w)).css("height", rem(fit.h)).child(stage)))
    }
}

/// Drives the messenger headlessly: first light, typing and Enter, the reply, a buzz, another
/// buddy, the files tab and a download, the status tab and a mood drag.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        s.play("00-enter", &[0, 200, 450, 900]);
        s.click("field0");
        s.type_text("hey!! wanna see my new skin?");
        s.play("01-typed", &[0]);
        s.key("Return");
        s.play("02-sent", &[0, 80, 200, 500]);
        s.tick(((TYPING + 0.2) * 1000.0) as u64);
        s.play("03-reply", &[0, 300]);
        s.click("buzz");
        s.play("04-buzz", &[0, 100, 250, 600]);
        s.hover("buddy1");
        s.click("buddy1");
        s.play("05-buddy", &[0, 200]);
        s.tick(((TYPING + 0.2) * 1000.0) as u64);
        s.play("06-unread", &[0, 300]);
        s.click("FILES");
        s.play("07-files", &[0, 150, 400]);
        s.click("file0");
        s.play("08-download", &[0, 600]);
        s.tick(((DOWNLOAD + 0.2) * 1000.0) as u64);
        s.play("09-saved", &[0, 1700]);
        s.click("STATUS");
        s.play("10-status", &[0, 300]);
        let (x, y) = s.centre("mood");
        s.cx.simulate_drag(s.win, (x - 100.0, y), (x + 120.0, y));
        s.play("11-mood", &[0, 200]);
    })
}

fn world() -> Fx {
    Fx::new("orbit-chrome", CHROME, 6, [200, 216, 250])
}

fn main() {
    if let Some(dir) = arg("--frames") {
        let fx = world();
        let stills = [("chrome", [6.0f32, 0.2, -1.0, -1.0, -1.0, -1.0]), ("chrome-pointer", [9.0, 0.7, 0.4, 0.45, -1.0, 0.05]),
                      ("chrome-buzz", [12.0, 0.45, -1.0, -1.0, 0.1, -1.0])];
        if gpu_stills(&dir, 1280, 800, stills, |g, f| g.draw(fx.effect(), &[], f)).is_none() {
            eprintln!("chrome shader: {:?}", fx.error());
        }
        return;
    }
    let make = || {
        let mut o = Orbit {
            world: world(),
            tab: Tab::Chat,
            sel: 0,
            chats: Default::default(),
            unread: [0, 2, 0, 1, 0, 0],
            said: [0; BUDDIES.len()],
            typing: None,
            draft: String::new(),
            msg_n: 0,
            bursts: 0,
            buzzes: 0,
            sent_at: 0.0,
            buzzed_at: 0.0,
            downloads: [None; FILES.len()],
            mood: Dial::new(0.3),
            status: 0,
            pointer: None,
            frames: 0,
        };
        for (b, me, text) in [(0, false, "heyyy ✦ r u online??"), (0, true, "always. the chrome never sleeps"),
                              (0, false, "ok but have u seen the new aqua skin"), (1, false, "the server room is glowing again"),
                              (1, false, "u there?"), (3, false, "new mix at midnight, be there")] {
            o.push(b, me, false, Cow::Borrowed(text));
        }
        o.unread = [0, 2, 0, 1, 0, 0];
        o
    };
    run_with(DESIGN.0, DESIGN.1, "BUI · Orbit Messenger", make, arg("--storyboard").map(storyboard));
}

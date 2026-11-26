//! HUD Shell: a desktop taskbar dressed as a heads-up display, on BUI. Black glass, signal yellow
//! and cyan, condensed caps, serial micro-labels and bracket frames. A GPU shader is the screen
//! (dot grid, scanlines, grain, a tear on every launch); another draws the live minimap in the
//! clock box (city blocks, range rings, a radar sweep, your cone of view).
//!
//! The taskbar's slots launch, focus and minimise apps (a yellow underline marks the running
//! ones); right-click a slot (or X for the focused app) and its options come up as a dialogue
//! choice list. The launcher (S, or the bracket at the left) opens a program list. Launches and
//! finished tasks slide in "completed" notifications; the status line types what happened. The
//! objectives on the right tick off when clicked; the bio-monitor bars follow CPU and battery.
//!
//! Keys: S launcher, 1-7 launch / focus, Tab cycle, X options, W signal, N notify, ↑ ↓ Enter in
//! lists, Escape closes whatever is on top.
//!
//! Motion ("glitch"): panels open with a 3-step flicker and a 180 ms scale-in, text types in
//! (stepped clip), lists stagger 40 ms per row, windows open as a CRT line (scaleY) in 220 ms;
//! exits are 140 ms fades with a 10 px drift; the shader tears for a quarter second per launch.
//!
//! `cargo run --release --bin hud`
//! `-- --storyboard DIR` drives it headlessly (keys, clicks, right-click, hover) and saves
//! frames along BUI's virtual clock; `-- --frames DIR` renders both shaders on the GPU.

use std::fmt::Write as _;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use blenderui::prelude::*;
use bui_demos::canvas::{rem, rem_css, Fit};
use bui_demos::{arg, focus_root, gpu_stills, now, run_with, story::story, Fx, Script};

/// The screen. U[0] = time, U[1] = seconds since the last launch (< 0: none).
const SCREEN: &str = r"
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

vec4 effect(vec2 p) {
    float t = U[0]; float g = U[1];
    vec2 uv = p / box_size;
    // a launch tears the screen: slices jump sideways, easing out over a quarter second
    vec3 y = vec3(0.95, 0.9, 0.0);
    vec3 c = vec3(0.0);
    if (g >= 0.0 && g < 0.25) {
        float k = 1.0 - g / 0.25;
        float r = hash(vec2(floor(uv.y * 40.0), floor(t * 30.0)));
        p.x += step(0.7, r) * (r - 0.7) * 120.0 * k * k;
        // torn slices light up, yellow and cyan
        c += mix(y, vec3(0.25, 0.9, 1.0), fract(r * 7.0)) * 0.35 * step(0.86, r) * k;
    }
    // a warm haze in the middle, falling off to the corners
    c += y * 0.05 * (1.0 - smoothstep(0.1, 0.8, length((uv - 0.5) * vec2(1.6, 1.0))));
    // dot grid
    vec2 d = mod(p, 32.0) - 16.0;
    c += y * 0.2 * smoothstep(1.6, 0.6, length(d));
    // a slow bright band rolling down, and the scanlines
    float roll = fract(t * 0.07);
    c += vec3(0.2, 0.9, 1.0) * 0.05 * smoothstep(0.06, 0.0, abs(1.0 - uv.y - roll));
    c *= 0.8 + 0.2 * sin(p.y * 3.14159);
    if (g >= 0.0 && g < 0.06) c += vec3(0.25, 0.9, 1.0) * 0.12;
    c += (hash(p + fract(t) * 91.0) - 0.5) * 0.035;
    return vec4(max(c, 0.0), 1.0);
}
";

/// The minimap. U[0] = time, U[1] = 1 when the link is secure.
const MINIMAP: &str = r"
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

vec4 effect(vec2 p) {
    float t = U[0]; float secure = U[1];
    vec2 uv = p / box_size;
    vec2 q = (uv - vec2(0.55, 0.45)) * vec2(box_size.x / box_size.y, 1.0);
    vec3 y = vec3(0.95, 0.9, 0.0);
    vec3 cy = vec3(0.25, 0.75, 1.0);
    vec3 c = vec3(0.0);
    // city blocks, turned 35 degrees, drifting as you walk
    float ca = 0.82; float sa = 0.572;
    vec2 w = q + vec2(0.0, t * 0.012);
    vec2 m = vec2(ca * w.x - sa * w.y, sa * w.x + ca * w.y) * 4.2;
    vec2 id = floor(m); vec2 f = fract(m) - 0.5;
    float h = hash(id);
    vec2 sz = vec2(0.30 + 0.12 * h, 0.27 + 0.16 * hash(id + 3.1));
    float dd = max(abs(f.x) - sz.x, abs(f.y) - sz.y);
    float on = step(0.22, h);
    c += y * on * (0.07 * step(dd, 0.0) + 0.5 * smoothstep(0.018, 0.0, abs(dd)));
    // parking stripes in some blocks
    c += y * 0.25 * on * step(0.75, h) * step(dd, -0.05) * step(0.8, fract(f.x * 9.0));
    // range rings and the sweep
    float r = length(q);
    c += y * 0.12 * smoothstep(0.006, 0.0, abs(fract(r * 3.2) - 0.5) - 0.49);
    float ang = atan2(q.x, q.y) / 6.2831853 + 0.5;
    float sweep = fract(ang - t * 0.3);
    c += mix(y, cy, secure) * pow(1.0 - sweep, 8.0) * 0.32;
    // blips: a few blocks light as the sweep passes
    vec2 bid = floor(q * 6.0);
    float bl = step(0.93, hash(bid + 17.0)) * smoothstep(0.12, 0.0, length(fract(q * 6.0) - 0.5));
    float since = fract(atan2(bid.x + 0.5, bid.y + 0.5) / 6.2831853 + 0.5 - t * 0.3);
    c += vec3(1.0, 0.35, 0.3) * bl * pow(1.0 - since, 3.0) * 1.4;
    // you: a cone of view and a triangle
    vec2 o = q - vec2(-0.02, 0.06);
    float view = atan2(o.x, o.y);
    float cone = smoothstep(0.5, 0.0, abs(view + 0.9)) * smoothstep(0.32, 0.0, length(o));
    c += cy * cone * 0.45;
    vec2 tq = o * 22.0;
    float tri = max(abs(tq.x) * 0.866 + tq.y * 0.5, -tq.y) - 0.5;
    c = mix(c, cy, smoothstep(0.08, 0.0, abs(tri)) * 0.9);
    // the noise you are making: rings off a point to the west
    vec2 s = q - vec2(-0.33, 0.12);
    c += y * smoothstep(0.03, 0.0, length(s));
    float ring = fract(length(s) * 7.0 - t * 1.2);
    c += y * 0.4 * smoothstep(0.1, 0.0, abs(ring - 0.5) - 0.4) * smoothstep(0.2, 0.0, length(s)) * step(0.0, s.x);
    c *= 0.85 + 0.15 * sin(p.y * 3.14159);
    return vec4(c, 1.0);
}
";

const CSS: &str = r"
html { height: 100%; }
body { margin: 0; height: 100%; position: relative; overflow: hidden; background: #000; color: #f3e600; user-select: none;
       font-family: 'Bahnschrift Condensed', Bahnschrift, 'Arial Narrow', sans-serif; -webkit-font-smoothing: antialiased; }
.screen { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
.abs { position: absolute; }
.stage { position: absolute; width: 1600px; height: 900px; }

/* -- small parts: micro-labels, serial stacks, the grid glyph -- */
.micro { font-family: Consolas, monospace; font-size: 10px; letter-spacing: 1px; color: #f3e600; white-space: nowrap; }
.tag { display: flex; align-items: flex-start; gap: 8px; }
.serial { font-family: Consolas, monospace; font-size: 5px; line-height: 6px; color: #b9ae00; }
.grid { width: 24px; height: 17px; border: 1.5px solid #f3e600; border-radius: 2px; display: flex; flex-wrap: wrap;
        gap: 2px; padding: 2px; box-sizing: border-box; }
.grid i { width: 4px; height: 4px; background: #f3e600; }

/* -- top left: bio monitor -- */
.bio { left: 40px; top: 30px; width: 380px; }
.meter { display: flex; align-items: center; gap: 14px; margin-top: 4px; }
.track { width: 262px; height: 14px; border: 1.5px solid #f3e600; padding: 2px; box-sizing: border-box; }
.fill { height: 100%; background: #f3e600; transform-origin: left center;
        transition-property: width; transition-duration: 600ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1);
        animation: grow 900ms cubic-bezier(0.2, 0, 0, 1) 250ms backwards; }
.num { font-size: 28px; font-weight: 600; font-variant-numeric: tabular-nums; }
.num small { font-size: 12px; vertical-align: top; margin-left: 4px; }
.cpu { display: flex; gap: 3px; margin-top: 8px; }
.cpu i { width: 8px; height: 7px; background: rgba(243,230,0,0.18); transition-property: background-color; transition-duration: 120ms; }
.cpu i.lit { background: #f3e600; }
.cpu i.hot { background: #ff5a4a; }
.brand { left: 40px; top: 120px; }
.word { font-size: 30px; font-weight: 800; letter-spacing: 1px; font-style: italic; margin-top: 6px; }

/* -- top centre: the focused window's banner -- */
.banner { left: 560px; top: 118px; display: flex; gap: 14px; animation: drop 360ms cubic-bezier(0.2, 0, 0, 1) 120ms backwards; }
.alert { width: 42px; text-align: center; }
.alert .box { width: 38px; height: 38px; border: 2px solid #5ef6ff; color: #5ef6ff; font-size: 26px; font-weight: 700;
              display: flex; align-items: center; justify-content: center; box-sizing: border-box; }
.alert .micro { color: #5ef6ff; font-size: 8px; letter-spacing: 0; margin-top: 3px; }
.strip { width: 470px; }
.strip .bar { height: 44px; border: 2px solid #5ef6ff; border-left-width: 12px; background: rgba(94,246,255,0.14);
              color: #5ef6ff; font-size: 23px; font-weight: 600; letter-spacing: 0.5px; display: flex; align-items: center;
              padding-left: 18px; box-sizing: border-box; }
.strip .bits { display: flex; justify-content: space-between; margin-top: 3px; }
.strip .bits .micro { color: #5ef6ff; font-size: 7px; letter-spacing: 0; opacity: 0.8; }
.chips { display: flex; gap: 2px; }
.chips i { width: 36px; height: 14px; border: 1px solid #5ef6ff; }

/* -- top right: clock and minimap -- */
.clock { left: 1336px; top: 26px; width: 230px; }
.clock .head { display: flex; justify-content: flex-end; gap: 10px; align-items: flex-start; }
.mapbox { position: relative; margin-top: 4px; width: 222px; height: 252px; border: 2px solid #f3e600; box-sizing: border-box; overflow: hidden; }
.map { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
.time { position: absolute; left: 8px; top: 4px; font-size: 22px; font-weight: 600; color: #f3e600;
        font-variant-numeric: tabular-nums; }
.time span { font-size: 11px; margin-left: 4px; }
.foot { position: absolute; left: 0; right: 0; bottom: 0; height: 30px; border-top: 2px solid #f3e600; background: #000;
        font-size: 21px; font-weight: 600; padding-left: 8px; display: flex; align-items: center;
        transition-property: color; transition-duration: 200ms; }
.foot.ok { color: #5ef6ff; }
.signal { position: absolute; left: -40px; top: 52px; width: 30px; height: 26px; display: flex; align-items: flex-end; gap: 3px; }
.signal i { width: 4px; background: #f3e600; transition-property: opacity; transition-duration: 200ms; }

/* -- right: objectives and shortcuts -- */
.goals { left: 1176px; top: 340px; width: 390px; }
.goals .head { display: flex; justify-content: flex-end; gap: 10px; margin-bottom: 8px; }
.goal { font-size: 22px; font-weight: 600; letter-spacing: 0.4px; padding: 3px 0; cursor: pointer; text-align: right;
        transition-property: opacity, color; transition-duration: 160ms; }
.goal:hover { color: #fffbb0; }
.goal.done { opacity: 0.38; text-decoration: line-through; }
.hint { display: flex; justify-content: flex-end; gap: 10px; margin-top: 8px; color: #5ef6ff; font-size: 20px; text-align: right; }
.dia { width: 14px; height: 14px; background: #5ef6ff; transform: rotate(45deg); margin-top: 6px; flex-shrink: 0; }
.keys { left: 1276px; top: 560px; width: 290px; }
.keys .head { display: flex; justify-content: flex-end; gap: 10px; margin-bottom: 6px; }
.key { height: 40px; display: flex; justify-content: flex-end; align-items: center; gap: 12px; color: #3f8cff; font-size: 20px;
       font-weight: 600; letter-spacing: 0.5px; cursor: pointer; transition-property: color, transform; transition-duration: 120ms; }
.key:hover { color: #5ef6ff; }
.key:active { transform: scale(0.96); }
.cap { width: 26px; height: 26px; border: 2px solid #f3e600; border-radius: 15px; color: #f3e600; font-size: 14px;
       display: flex; align-items: center; justify-content: center; }

/* -- left: notifications -- */
.toasts { left: 40px; top: 226px; width: 440px; }
.toast { display: flex; gap: 14px; margin-bottom: 18px;
         animation: toastin 420ms cubic-bezier(0.2, 0, 0, 1) backwards, toastout 220ms cubic-bezier(0.4, 0, 1, 1) 4600ms forwards; }
.chip { width: 56px; height: 72px; background: #f3e600; position: relative; flex-shrink: 0;
        clip-path: polygon(0 0, 100% 0, 100% 100%, 22% 100%, 0 86%); }
.chip .disc { position: absolute; left: 13px; top: 18px; width: 30px; height: 30px; border: 3px solid #000; border-radius: 50%; box-sizing: border-box; }
.chip .disc i { position: absolute; left: 8px; top: 8px; width: 8px; height: 8px; border-radius: 4px; background: #000; }
.toast .hd { display: flex; align-items: center; gap: 10px; font-size: 20px; font-weight: 700; }
.toast .hd .dia { background: #f3e600; margin-top: 0; width: 12px; height: 12px; }
.toast .t1 { font-size: 20px; font-weight: 600; letter-spacing: 0.5px; margin-top: 4px; }
.toast .t2 { font-size: 13px; letter-spacing: 0.5px; margin-top: 6px; color: #e9e4b8; }

/* -- centre: crosshair and the focused window -- */
.cross { left: 760px; top: 420px; width: 80px; height: 60px; transition-property: opacity; transition-duration: 200ms; }
.cross i { position: absolute; background: #f3e600; }
.win { left: 470px; top: 210px; width: 660px; z-index: 5; background: rgba(0,0,0,0.82); border: 1.5px solid #f3e600;
       box-shadow: 0 0 0 1px rgba(243,230,0,0.15), 0 0 40px rgba(243,230,0,0.12);
       animation: crt 240ms cubic-bezier(0.2, 0, 0, 1) backwards; transform-origin: center center; }
.win.out { animation: crtout 160ms cubic-bezier(0.4, 0, 1, 1) forwards; pointer-events: none; }
.win .title { height: 40px; background: #f3e600; color: #000; display: flex; align-items: center; justify-content: space-between;
              padding-left: 14px; font-size: 22px; font-weight: 700; letter-spacing: 1px; }
.x { width: 44px; height: 40px; display: flex; align-items: center; justify-content: center; font-size: 20px; cursor: pointer;
     transition-property: background-color, color; transition-duration: 100ms; }
.x:hover { background: #000; color: #f3e600; }
.win .body { padding: 14px 18px 18px; }
.line { display: flex; justify-content: space-between; padding: 7px 0; border-bottom: 1px solid rgba(243,230,0,0.18); font-size: 18px;
        letter-spacing: 0.5px; animation: rowin 260ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.line b { font-weight: 600; color: #5ef6ff; font-variant-numeric: tabular-nums; }
.load { margin-top: 14px; height: 8px; border: 1px solid #f3e600; padding: 1px; }
.load i { display: block; height: 100%; background: #5ef6ff; transform-origin: left center; animation: grow 1200ms cubic-bezier(0.2, 0, 0, 1) 200ms backwards; }
.corner { position: absolute; width: 14px; height: 14px; border-color: #5ef6ff; border-style: solid; border-width: 0; }

/* -- bottom centre: the choice list -- */
.ask { left: 390px; top: 560px; display: flex; gap: 14px; z-index: 6; animation: pop 200ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.ask.out { animation: fadeout 140ms cubic-bezier(0.4, 0, 1, 1) forwards; pointer-events: none; }
.who { display: flex; gap: 12px; align-items: flex-start; }
.who .name { height: 34px; border: 2px solid #f3e600; padding: 0 16px; font-size: 22px; font-weight: 700; letter-spacing: 1px;
             display: flex; align-items: center; background: #000; }
.who .cap { width: 30px; height: 30px; border-radius: 17px; }
.opts { width: 560px; }
.opt { height: 42px; display: flex; align-items: center; gap: 14px; padding-left: 12px; font-size: 21px; font-weight: 600; letter-spacing: 0.4px;
       border-left: 5px solid #3f8cff; color: #3f8cff; cursor: pointer; margin-bottom: 4px; box-sizing: border-box; background: rgba(0,0,0,0.6);
       transition-property: background-color, color, border-color; transition-duration: 90ms;
       animation: rowin 220ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.opt.warn { border-left-color: #f3e600; color: #f3e600; }
.opt.on { border: 2px solid #5ef6ff; border-left-width: 5px; background: rgba(94,246,255,0.16); color: #5ef6ff; }
.opt .n { margin-left: auto; margin-right: 14px; font-variant-numeric: tabular-nums; opacity: 0.8; }

/* -- the status line -- */
.say { left: 0; top: 770px; width: 1600px; text-align: center; font-size: 24px; letter-spacing: 0.4px; color: #f2eed2; }
.say b { color: #3f8cff; font-weight: 600; }
.type { display: inline-block; white-space: nowrap; animation: type 700ms steps(24) backwards; }

/* -- the taskbar -- */
.taskbar { left: 40px; top: 790px; width: 1520px; height: 96px; }
.taskbar > .micro { position: absolute; left: 0; top: 0; }
.dock { position: absolute; left: 0; top: 20px; display: flex; gap: 12px; align-items: flex-start; }
.slot { position: relative; width: 66px; height: 66px; border: 1.5px solid rgba(243,230,0,0.5); box-sizing: border-box; cursor: pointer;
        display: flex; align-items: center; justify-content: center; font-size: 30px; color: rgba(243,230,0,0.75);
        transition-property: transform, border-color, background-color, color; transition-duration: 140ms;
        transition-timing-function: cubic-bezier(0.2, 0, 0, 1);
        animation: slotin 320ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.slot:hover { border-color: #f3e600; color: #f3e600; transform: translateY(-3px); }
.slot:active { transform: scale(0.96); }
.slot.focus { border-color: #f3e600; background: rgba(243,230,0,0.16); color: #f3e600; }
.slot .no { position: absolute; left: -1.5px; top: -1.5px; width: 13px; height: 18px; background: #f3e600; color: #000;
            font-size: 9px; font-weight: 700; display: flex; align-items: center; justify-content: center; }
.slot .lbl { position: absolute; left: 0; bottom: -17px; font-family: Consolas, monospace; font-size: 8px; letter-spacing: 0.5px; color: #b9ae00; }
.slot .run { position: absolute; left: 10px; right: 10px; bottom: -6px; height: 3px; background: #f3e600; transform-origin: center;
             animation: grow 260ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.start { width: 66px; height: 66px; position: relative; cursor: pointer; display: flex; align-items: center; justify-content: center;
         transition-property: transform, background-color; transition-duration: 140ms; margin-right: 12px; }
.start:active { transform: scale(0.96); }
.start.on { background: #f3e600; }
.start.on .grid { border-color: #000; }
.start.on .grid i { background: #000; }
.start .grid { width: 30px; height: 30px; gap: 3px; padding: 3px; }
.start .grid i { width: 5px; height: 5px; }
.brk { position: absolute; width: 12px; height: 12px; border-color: #f3e600; border-style: solid; border-width: 0; }
.tl { left: 0; top: 0; border-top-width: 2px; border-left-width: 2px; }
.tr { right: 0; top: 0; border-top-width: 2px; border-right-width: 2px; }
.bl { left: 0; bottom: 0; border-bottom-width: 2px; border-left-width: 2px; }
.br { right: 0; bottom: 0; border-bottom-width: 2px; border-right-width: 2px; }
.tray { position: absolute; right: 0; top: 18px; display: flex; gap: 26px; align-items: center; }
.tray .num { font-size: 22px; }
.code { display: flex; gap: 2px; height: 34px; align-items: stretch; }
.code i { background: #f3e600; opacity: 0.85; }
.edge { left: 0px; top: 640px; width: 30px; display: flex; flex-direction: column; gap: 2px; }
.edge i { height: 2px; background: #b9ae00; }

/* -- the launcher -- */
.launch { left: 40px; top: 396px; width: 420px; z-index: 7; background: rgba(0,0,0,0.9); border: 1.5px solid #f3e600;
          animation: pop 220ms cubic-bezier(0.2, 0, 0, 1) backwards; transform-origin: left bottom; }
.launch.out { animation: fadeout 140ms cubic-bezier(0.4, 0, 1, 1) forwards; pointer-events: none; }
.launch .title { height: 36px; background: #f3e600; color: #000; display: flex; align-items: center; padding-left: 12px;
                 font-size: 19px; font-weight: 700; letter-spacing: 1px; }
.launch .prompt { padding: 10px 14px 6px; font-family: Consolas, monospace; font-size: 13px; color: #5ef6ff; }
.caret { display: inline-block; width: 8px; height: 14px; background: #5ef6ff; margin-left: 2px; animation: blink 1s steps(2) infinite; }
.prog { height: 40px; display: flex; align-items: center; gap: 14px; padding: 0 14px; font-size: 19px; font-weight: 600; cursor: pointer;
        letter-spacing: 0.5px; transition-property: background-color, color; transition-duration: 80ms;
        animation: rowin 220ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.prog .g { width: 26px; text-align: center; font-size: 20px; }
.prog .st { margin-left: auto; font-family: Consolas, monospace; font-size: 10px; color: #b9ae00; }
.prog.on { background: #5ef6ff; color: #000; }
.prog.on .st { color: #000; }
.launch .foot2 { padding: 8px 14px 10px; display: flex; justify-content: space-between; }

/* -- motion -- */
@keyframes grow { from { transform: scaleX(0); } to { transform: scaleX(1); } }
@keyframes drop { from { opacity: 0; transform: translateY(-14px); } to { opacity: 1; transform: translateY(0); } }
@keyframes toastin { 0% { opacity: 0; transform: translateX(-40px); } 30% { opacity: 1; } 45% { opacity: 0.3; } 60% { opacity: 1; }
                     100% { opacity: 1; transform: translateX(0); } }
@keyframes toastout { to { opacity: 0; transform: translateX(-10px); } }
@keyframes crt { 0% { opacity: 0; transform: scaleY(0.02) scaleX(0.6); } 45% { opacity: 1; transform: scaleY(0.02) scaleX(1); }
                 100% { opacity: 1; transform: scaleY(1) scaleX(1); } }
@keyframes crtout { to { opacity: 0; transform: scaleY(0.02); } }
@keyframes pop { 0% { opacity: 0; transform: scale(0.96); } 35% { opacity: 1; } 50% { opacity: 0.35; } 65% { opacity: 1; }
                 100% { opacity: 1; transform: scale(1); } }
@keyframes fadeout { to { opacity: 0; transform: translateY(10px); } }
@keyframes rowin { from { opacity: 0; transform: translateX(-14px); } to { opacity: 1; transform: translateX(0); } }
@keyframes slotin { from { opacity: 0; transform: translateY(18px); } to { opacity: 1; transform: translateY(0); } }
@keyframes type { from { clip-path: inset(0 100% 0 0); } to { clip-path: inset(0 0 0 0); } }
@keyframes blink { 50% { opacity: 0; } }
";

const DESIGN: (f32, f32) = (1600.0, 900.0);

/// One program on the dock: its name, glyph, what its window shows, and what the status line
/// says when it starts.
struct Program {
    name: &'static str,
    glyph: &'static str,
    rows: [(&'static str, &'static str); 4],
    says: &'static str,
}

const PROGRAMS: [Program; 7] = [
    Program { name: "TERMINAL", glyph: "▶", says: "Shell is up. Root's yours.",
              rows: [("SESSION", "tty/04"), ("UPTIME", "41D 06H"), ("LOAD", "0.42 0.37 0.30"), ("LAST LOGIN", "09:58 UTC")] },
    Program { name: "NETLINK", glyph: "◎", says: "Link holds. Nobody's listening.",
              rows: [("NODE", "551.98.20"), ("LATENCY", "12 MS"), ("UPLINK", "940 MB/S"), ("PEERS", "28 TRUSTED")] },
    Program { name: "MAIL", glyph: "✉", says: "Three new. One of them matters.",
              rows: [("INBOX", "3 UNREAD"), ("FLAGGED", "1"), ("DRAFTS", "2"), ("SYNC", "2 MIN AGO")] },
    Program { name: "FILES", glyph: "▤", says: "Archive's mounted. Read only.",
              rows: [("VOLUME", "SHARD-07"), ("USED", "612 / 1024 GB"), ("ENCRYPTION", "AES-256"), ("SNAPSHOT", "TODAY 04:00")] },
    Program { name: "AUDIO", glyph: "♫", says: "Playing. Keep it low.",
              rows: [("TRACK", "SIGNAL DECAY"), ("ARTIST", "VOID CHOIR"), ("POSITION", "02:41 / 04:10"), ("OUTPUT", "IMPLANT L+R")] },
    Program { name: "MAPS", glyph: "⌖", says: "Route plotted. Avoid the north side.",
              rows: [("DISTRICT", "NORTH HARBOR"), ("DESTINATION", "PIER 9"), ("ETA", "06 MIN"), ("THREAT", "LOW")] },
    Program { name: "OPTICS", glyph: "◉", says: "Optics online. I see what you see.",
              rows: [("ZOOM", "4.0X"), ("MODE", "THERMAL"), ("RECORDING", "OFF"), ("FIRMWARE", "V3.18")] },
];

/// The objectives on the right, and the step each one hints at.
const GOALS: [(&str, &str); 3] = [
    ("SHIP THE 2.14 BUILD", "Run the release pipeline"),
    ("CLEAR THE INBOX", "Answer the one that matters"),
    ("BACK UP SHARD-07", "Snapshot before midnight"),
];

/// The choice list's options, as they read for a running / a stopped program.
const CHOICES: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Open,
    NewWindow,
    Restart,
    Quit,
}

const CHOICE_ORDER: [Choice; CHOICES] = [Choice::Open, Choice::NewWindow, Choice::Restart, Choice::Quit];

impl Choice {
    const fn label(self, running: bool) -> &'static str {
        match (self, running) {
            (Self::Open, false) => "Launch",
            (Self::Open, true) => "Bring to front",
            (Self::NewWindow, _) => "New window",
            (Self::Restart, _) => "Restart with a clean cache",
            (Self::Quit, _) => "Force quit",
        }
    }
}

/// A notification sliding in on the left.
struct Toast {
    id: u32,
    head: &'static str,
    title: &'static str,
    sub: String,
    born: f32,
}

/// Seconds a notification stays (its CSS exit plays at 4.6 s).
const TOAST_LIFE: f32 = 5.0;

/// A panel that opens and plays its exit when it closes: what is open, and what is on its way out.
#[derive(Default)]
struct Panel<T: Copy> {
    open: Option<T>,
    closing: Option<T>,
    /// Bumped on each open: keys the panel, so its entrance plays again.
    n: u32,
}

impl<T: Copy + PartialEq> Panel<T> {
    fn show(&mut self, v: T) {
        if self.open != Some(v) {
            self.closing = self.open.take();
            self.open = Some(v);
            self.n += 1;
        }
    }

    fn hide(&mut self) -> bool {
        self.open.take().is_some_and(|v| {
            self.closing = Some(v);
            true
        })
    }
}

struct Hud {
    screen: Fx,
    minimap: Fx,
    /// Windows per program (0: not running).
    windows: [u8; PROGRAMS.len()],
    /// The focused program's window.
    win: Panel<usize>,
    /// The launcher and its highlighted row.
    launcher: Panel<()>,
    launch_sel: usize,
    /// The choice list (for a program) and its highlighted option.
    ask: Panel<usize>,
    ask_sel: usize,
    toasts: Vec<Toast>,
    toast_n: u32,
    goals: [bool; GOALS.len()],
    /// Who speaks on the status line, what, and a key that replays its typing.
    say: (&'static str, &'static str, u32),
    secure: bool,
    /// When the last launch tore the screen.
    torn: f32,
    frames: u32,
}

impl Hud {
    const fn running(&self, i: usize) -> bool {
        self.windows[i] > 0
    }

    const fn speak(&mut self, who: &'static str, what: &'static str) {
        self.say = (who, what, self.say.2 + 1);
    }

    fn notify(&mut self, head: &'static str, title: &'static str, sub: String) {
        self.toast_n += 1;
        self.toasts.push(Toast { id: self.toast_n, head, title, sub, born: now() });
        // two at most: the oldest makes room
        if self.toasts.len() > 2 {
            self.toasts.remove(0);
        }
    }

    /// Starts program `i` (or brings it up when it runs).
    fn launch(&mut self, i: usize) {
        if !self.running(i) {
            self.windows[i] = 1;
            self.torn = now();
            self.notify("PROGRAM LOADED", PROGRAMS[i].name, format!("NODE 551.98.20 · SLOT {:02}", i + 1));
            self.speak("ECHO", PROGRAMS[i].says);
        }
        self.win.show(i);
    }

    /// A dock slot's click: launch, focus, or minimise the focused one.
    fn press_slot(&mut self, i: usize) {
        if self.win.open == Some(i) {
            self.win.hide();
        } else {
            self.launch(i);
        }
    }

    fn quit(&mut self, i: usize) {
        if self.running(i) {
            self.windows[i] = 0;
            if self.win.open == Some(i) {
                self.win.hide();
            }
            self.speak("ECHO", "Gone. Like it never ran.");
        }
    }

    /// Tab: the next running program after the focused one.
    fn cycle(&mut self) -> bool {
        let n = PROGRAMS.len();
        let from = self.win.open.unwrap_or(n - 1);
        let next = (1..=n).map(|k| (from + k) % n).find(|&i| self.running(i));
        next.is_some_and(|i| {
            self.win.show(i);
            true
        })
    }

    fn choose(&mut self) {
        let Some(i) = self.ask.open else { return };
        match CHOICE_ORDER[self.ask_sel] {
            Choice::Open => self.launch(i),
            Choice::NewWindow => {
                self.launch(i);
                self.windows[i] = self.windows[i].saturating_add(1);
                self.notify("WINDOW OPENED", PROGRAMS[i].name, format!("{} WINDOWS RUNNING", self.windows[i]));
            }
            Choice::Restart => {
                self.windows[i] = 0;
                self.launch(i);
                self.speak("ECHO", "Cache flushed. Fresh as the first boot.");
            }
            Choice::Quit => self.quit(i),
        }
        self.ask.hide();
    }

    fn open_ask(&mut self, i: usize) {
        self.ask_sel = 0;
        self.ask.show(i);
        self.launcher.hide();
    }

    fn toggle_launcher(&mut self) {
        if !self.launcher.hide() {
            self.launch_sel = self.win.open.unwrap_or(0);
            self.launcher.show(());
            self.ask.hide();
        }
    }

    fn finish_goal(&mut self, k: usize) {
        self.goals[k] = !self.goals[k];
        if self.goals[k] {
            self.torn = now();
            let left = self.goals.iter().filter(|d| !**d).count();
            self.notify("TASK COMPLETED", GOALS[k].0, format!("{left} OBJECTIVES LEFT · NORTH HARBOR"));
            self.speak("ECHO", if left == 0 { "That's all of them. Go home." } else { "Done. Next one's on the right." });
        }
    }

    const fn toggle_signal(&mut self) {
        self.secure = !self.secure;
        self.speak("NETLINK", if self.secure { "Tunnel up. You're dark now." } else { "Tunnel down. They can see you." });
    }

    /// Every key; answers whether it did anything. Lists on top take the arrows first.
    fn key(&mut self, key: &str) -> bool {
        if self.ask.open.is_some() {
            match key {
                "Up" => self.ask_sel = (self.ask_sel + CHOICES - 1) % CHOICES,
                "Down" => self.ask_sel = (self.ask_sel + 1) % CHOICES,
                "Return" | "KP_Enter" | "space" => self.choose(),
                "Escape" | "x" | "X" => {
                    self.ask.hide();
                }
                _ => return false,
            }
            return true;
        }
        if self.launcher.open.is_some() {
            let n = PROGRAMS.len();
            match key {
                "Up" => self.launch_sel = (self.launch_sel + n - 1) % n,
                "Down" => self.launch_sel = (self.launch_sel + 1) % n,
                "Return" | "KP_Enter" | "space" => {
                    self.launcher.hide();
                    self.launch(self.launch_sel);
                }
                "Escape" | "s" | "S" => {
                    self.launcher.hide();
                }
                _ => return false,
            }
            return true;
        }
        match key {
            "s" | "S" => self.toggle_launcher(),
            "Tab" => return self.cycle(),
            "x" | "X" => match self.win.open {
                Some(i) => self.open_ask(i),
                None => return false,
            },
            "w" | "W" => self.toggle_signal(),
            "n" | "N" => {
                let k = self.goals.iter().position(|d| !d).unwrap_or(0);
                self.finish_goal(k);
            }
            "Escape" => return self.win.hide(),
            k if k.len() == 1 && (b'1'..=b'7').contains(&k.as_bytes()[0]) => self.launch((k.as_bytes()[0] - b'1') as usize),
            _ => return false,
        }
        true
    }
}

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

/// The micro-label every block wears: a serial stack and a connection id.
fn tag(label: &str) -> Div {
    div().class("tag")
        .child(div().class("serial").child(div().child("404181")).child(div().child("415185")).child(div().child("0001364")))
        .child(div().class("micro").child(label.to_string()))
}

/// The little window-grid glyph next to the labels.
fn grid(cells: usize) -> Div {
    let mut g = div().class("grid");
    for _ in 0..cells {
        g = g.child(i());
    }
    g
}

/// Corner brackets inside a box: four L-shapes, `size` design px, in `color`.
fn brackets(mut d: Div, size: f32, color: &'static str) -> Div {
    for corner in ["brk tl", "brk tr", "brk bl", "brk br"] {
        d = d.child(div().class(corner).css("width", rem(size)).css("height", rem(size)).css("border-color", color));
    }
    d
}

/// A barcode: bars of the widths a fixed sequence gives.
fn barcode(n: usize, class: &'static str) -> Div {
    let mut d = div().class(class);
    for k in 0..n {
        let w = [1.0, 2.0, 1.0, 3.0, 1.0, 1.0, 2.0, 4.0, 1.0, 2.0][(k * 7 + 3) % 10];
        d = d.child(i().css(if class == "code" { "width" } else { "height" }, rem(w)));
    }
    d
}

/// The time of day (UTC: the demo carries no time-zone database), as "HH:MM".
fn clock() -> (u64, u64) {
    let s = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    ((s / 3600) % 24, (s / 60) % 60)
}

impl Hud {
    fn bio(t: f32) -> Div {
        // CPU wanders; the battery drains a hair per minute
        let cpu = (0.34 + 0.18 * (t * 0.9).sin() + 0.1 * (t * 2.3).sin() + 0.06 * (t * 5.1).sin()).clamp(0.05, 0.98);
        let lit = (cpu * 24.0).round() as usize;
        let battery = (61.0 - t / 60.0).max(5.0);
        let mut bars = div().class("cpu");
        for k in 0..24 {
            bars = bars.child(i().class(if k >= lit { "" } else if k >= 19 { "hot" } else { "lit" }));
        }
        div().class("abs bio")
            .child(tag("BIO MONITOR 129 936"))
            .child(div().class("meter")
                .child(div().class("track").child(div().class("fill").css("width", format!("{battery}%"))))
                .child(div().class("num").child(format!("{:.0}", battery.round())).child(small().child("100"))))
            .child(div().class("meter")
                .child(bars)
                .child(div().class("micro").css("font-variant-numeric", "tabular-nums").child(format!("CPU {:>3.0}%", cpu * 100.0))))
    }

    fn banner(&self) -> Div {
        let (title, n) = match self.win.open {
            Some(i) => (PROGRAMS[i].name, self.win.n),
            None => ("DESKTOP // NO WINDOW FOCUSED", 0),
        };
        let text = if self.win.open.is_some() { format!("{title} // FOCUSED") } else { title.to_string() };
        div().class("abs banner")
            .child(div().class("alert").child(div().class("box").child("!"))
                .child(div().class("micro").child("PROTOCOL")).child(div().class("micro").child("7754_S25A")))
            .child(div().class("strip")
                .child(div().class("bar").child(span().id(format!("banner{n}")).class("type").child(text)))
                .child(div().class("bits")
                    .child(div().child(div().class("micro").child("PAGE NAME: SHELL-2.18.20"))
                        .child(div().class("micro").child("PAGE TYPE: HUD OVERLAY · LOAD ADDRESS 00000000")))
                    .child(div().class("chips").child(i()).child(i()))))
    }

    fn clock_box(&self, t: f32) -> Div {
        let (h, m) = clock();
        let colon = if ((t * 2.0) as u32).is_multiple_of(2) { ":" } else { " " };
        let secure = self.secure;
        let map = self.minimap.canvas(move |_| [now(), if secure { 1.0 } else { 0.0 }]).class("map");
        let mut signal = div().class("signal");
        for (k, hgt) in [8.0, 13.0, 18.0, 24.0].into_iter().enumerate() {
            signal = signal.child(i().css("height", rem(hgt)).css("opacity", if secure || k < 2 { "1" } else { "0.25" }));
        }
        div().class("abs clock")
            .child(div().class("head").child(tag("CONNECTION 551.98.20")).child(grid(6)))
            .child(div().class("mapbox")
                .child(map)
                .child(div().class("time").child(format!("{h:02}{colon}{m:02}")).child(span().child("UTC")))
                .child(div().class(if secure { "foot ok" } else { "foot" }).child(if secure { "SECURE" } else { "LIMITED" })))
            .child(signal)
    }

    fn goals(&self, cx: &mut Context<Self>) -> Div {
        let mut d = div().class("abs goals").child(div().class("head").child(tag("CONNECTION 985.20.54")).child(grid(6)));
        for (k, (goal, _)) in GOALS.iter().enumerate() {
            d = d.child(div().id(format!("goal{k}")).class(if self.goals[k] { "goal done" } else { "goal" }).child(*goal)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.finish_goal(k);
                    cx.notify();
                })));
        }
        let next = self.goals.iter().position(|d| !d).map_or("All clear. Nothing left tonight.", |k| GOALS[k].1);
        d.child(div().class("hint").child(div().class("dia")).child(div().child(next)))
    }

    fn keys(cx: &mut Context<Self>) -> Div {
        let mut d = div().class("abs keys").child(div().class("head").child(tag("CONNECTION 411.10.28")).child(grid(6)));
        for (name, cap, key) in [("LAUNCHER", "S", "s"), ("CYCLE APPS", "⇥", "Tab"), ("OPTIONS", "X", "x"), ("SIGNAL", "W", "w")] {
            d = d.child(div().id(format!("key-{key}")).class("key").child(name).child(div().class("cap").child(cap))
                .on_click(cx.listener(move |this, _, _, cx| if this.key(key) { cx.notify() })));
        }
        d
    }

    fn toasts(&self) -> Div {
        let mut d = div().class("abs toasts");
        for t in &self.toasts {
            d = d.child(div().id(format!("toast{}", t.id)).class("toast")
                .child(div().class("chip").child(div().class("disc").child(i())))
                .child(div()
                    .child(div().class("hd").child(div().class("dia")).child(t.head))
                    .child(div().class("t1").child(t.title))
                    .child(div().class("t2").child(t.sub.clone()))));
        }
        d
    }

    fn window(&self, k: usize, leaving: bool, cx: &mut Context<Self>) -> Div {
        let p = &PROGRAMS[k];
        let mut body = div().class("body");
        for (r, (name, value)) in p.rows.iter().enumerate() {
            body = body.child(div().class("line").css("animation-delay", format!("{}ms", 160 + r * 40))
                .child(span().child(*name)).child(b().child(*value)));
        }
        body = body.child(div().class("load").child(i()));
        let title = div().class("title")
            .child(span().child(format!("{}  ·  {} WINDOW{}", p.name, self.windows[k].max(1), if self.windows[k] > 1 { "S" } else { "" })))
            .child(div().id("win-x").class("x").child("✕").on_click(cx.listener(|this, _, _, cx| if this.win.hide() { cx.notify() })));
        let w = div().id(format!("win{k}-{}", self.win.n)).class(if leaving { "abs win out" } else { "abs win" })
            .child(title).child(body);
        brackets(w, 16.0, "#5ef6ff")
    }

    fn ask(&self, i: usize, leaving: bool, cx: &mut Context<Self>) -> Div {
        let running = self.running(i);
        let mut opts = div().class("opts");
        for (k, c) in CHOICE_ORDER.iter().enumerate() {
            let class = match (k == self.ask_sel, *c == Choice::Quit) {
                (true, _) => "opt on",
                (false, true) => "opt warn",
                (false, false) => "opt",
            };
            opts = opts.child(div().id(format!("opt{k}")).class(class).css("animation-delay", format!("{}ms", 60 + k * 40))
                .child(span().child(c.label(running)))
                .child(span().class("n").child(if k == 0 && running { format!("{} OPEN", self.windows[i]) } else { String::new() }))
                .on_hover(cx.listener(move |this, over: &bool, _, cx| if *over && this.ask_sel != k {
                    this.ask_sel = k;
                    cx.notify();
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.ask_sel = k;
                    this.choose();
                    cx.notify();
                })));
        }
        div().id(format!("ask{}", self.ask.n)).class(if leaving { "abs ask out" } else { "abs ask" })
            .child(div().class("who").child(div().class("name").child(PROGRAMS[i].name)).child(div().class("cap").child("X")))
            .child(opts)
    }

    fn launcher_panel(&self, leaving: bool, cx: &mut Context<Self>) -> Div {
        let mut d = div().id(format!("launch{}", self.launcher.n)).class(if leaving { "abs launch out" } else { "abs launch" })
            .child(div().class("title").child(format!("LAUNCHER // {:02} PROGRAMS", PROGRAMS.len())))
            .child(div().class("prompt").child(format!("> run {}", PROGRAMS[self.launch_sel].name.to_lowercase())).child(span().class("caret")));
        for (k, p) in PROGRAMS.iter().enumerate() {
            let st = if self.running(k) { "RUNNING" } else { "IDLE" };
            d = d.child(div().id(format!("prog{k}")).class(if k == self.launch_sel { "prog on" } else { "prog" })
                .css("animation-delay", format!("{}ms", 80 + k * 40))
                .child(span().class("g").child(p.glyph)).child(span().child(p.name)).child(span().class("st").child(st))
                .on_hover(cx.listener(move |this, over: &bool, _, cx| if *over && this.launch_sel != k {
                    this.launch_sel = k;
                    cx.notify();
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.launcher.hide();
                    this.launch(k);
                    cx.notify();
                })));
        }
        d.child(div().class("foot2").child(div().class("micro").child("↑ ↓ SELECT · ENTER RUN · ESC CLOSE"))
            .child(div().class("micro").child("SHARD-07")))
    }

    fn taskbar(&self, cx: &mut Context<Self>) -> Div {
        let start = brackets(div().id("start").class(if self.launcher.open.is_some() { "start on" } else { "start" }), 12.0, "#f3e600")
            .child(grid(9))
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_launcher();
                cx.notify();
            }));
        let mut dock = div().class("dock").child(start);
        for (k, p) in PROGRAMS.iter().enumerate() {
            let focus = self.win.open == Some(k);
            let mut s = div().id(format!("slot{k}")).class(if focus { "slot focus" } else { "slot" })
                .css("animation-delay", format!("{}ms", 300 + k * 40))
                .child(div().class("no").child(format!("{}", k + 1)))
                .child(p.glyph)
                .child(div().class("lbl").child(p.name));
            if self.running(k) {
                s = s.child(div().class("run"));
            }
            dock = dock.child(s
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.press_slot(k);
                    cx.notify();
                }))
                .on_context_menu(cx.listener(move |this, _: &ContextMenuEvent, _, cx| {
                    this.open_ask(k);
                    cx.notify();
                })));
        }
        let open = self.windows.iter().filter(|w| **w > 0).count();
        div().class("abs taskbar")
            .child(div().class("micro").child("CONNECTION 541.84.45 · DOCK"))
            .child(dock)
            .child(div().class("tray")
                .child(div().child(div().class("micro").child("RUNNING")).child(div().class("num").child(format!("{open:02} / {:02}", PROGRAMS.len()))))
                .child(barcode(28, "code"))
                .child(div().child(div().class("micro").child("57491 00745")).child(div().class("word").css("font-size", rem(22.0)).css("margin-top", "0").child("VANTA//OS"))))
    }
}

impl Render for Hud {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        focus_root(w, &mut self.frames);
        let t = now();
        self.toasts.retain(|n| t - n.born < TOAST_LIFE);
        let fit = Fit::new(w, DESIGN);
        let torn = self.torn;
        let screen = self.screen.canvas(move |_| {
            let t = now();
            [t, if torn > 0.0 { t - torn } else { -1.0 }]
        }).class("screen");

        let mut cross = div().class("abs cross").css("opacity", if self.win.open.is_some() { "0" } else { "1" });
        for (l, tp, wd, ht) in [(0.0, 29.0, 30.0, 2.0), (50.0, 29.0, 30.0, 2.0), (39.0, 0.0, 2.0, 14.0), (39.0, 46.0, 2.0, 14.0), (36.0, 27.0, 8.0, 6.0)] {
            cross = cross.child(i().css("left", rem(l)).css("top", rem(tp)).css("width", rem(wd)).css("height", rem(ht)));
        }

        let mut stage = div().class("stage").css("left", rem(fit.ox)).css("top", rem(fit.oy))
            .child(Self::bio(t))
            .child(div().class("abs brand").child(tag("SYSTEM MNGMNT PANEL")).child(div().class("word").child("VANTA//OS")))
            .child(self.banner())
            .child(self.clock_box(t))
            .child(self.goals(cx))
            .child(Self::keys(cx))
            .child(self.toasts())
            .child(cross)
            .child(barcode(34, "abs edge"));
        if let Some(i) = self.win.closing {
            stage = stage.child(self.window(i, true, cx));
        }
        if let Some(i) = self.win.open {
            stage = stage.child(self.window(i, false, cx));
        }
        if let Some(i) = self.ask.closing {
            stage = stage.child(self.ask(i, true, cx));
        }
        if let Some(i) = self.ask.open {
            stage = stage.child(self.ask(i, false, cx));
        }
        let (who, what, n) = self.say;
        let mut said = String::with_capacity(what.len() + 2);
        let _ = write!(said, " {what}");
        stage = stage
            .child(div().class("abs say").child(span().id(format!("say{n}")).class("type").child(b().child(format!("{who}:"))).child(said)))
            .child(self.taskbar(cx));
        if self.launcher.closing.is_some() {
            stage = stage.child(self.launcher_panel(true, cx));
        }
        if self.launcher.open.is_some() {
            stage = stage.child(self.launcher_panel(false, cx));
        }

        html().css("font-size", fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root").attr("tabindex", "0")
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| if this.key(&e.keystroke.key) {
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(screen)
                .child(div().class("abs").css("left", "0").css("top", "0").css("width", rem(fit.w)).css("height", rem(fit.h)).child(stage)))
    }
}

/// Drives the shell headlessly: first light, the launcher by key, a launch from it, slot clicks,
/// a right-click choice list, a goal, Tab, Escape.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        s.play("00-enter", &[0, 200, 500, 1200]);
        s.cx.simulate_mouse_move(s.win, 5.0, 5.0);
        s.cx.simulate_key(s.win, "s", "s");
        s.play("01-launcher", &[0, 80, 300]);
        s.key("Down");
        s.key("Down");
        s.key("Return");
        s.play("02-launch-mail", &[0, 120, 260, 700]);
        s.hover("slot0");
        s.play("03-hover-slot", &[0, 200]);
        s.click("slot0");
        s.play("04-click-terminal", &[0, 120, 700]);
        let (x, y) = s.centre("slot4");
        s.cx.simulate_button(s.win, x, y, MouseButton::Right);
        s.play("05-right-click", &[0, 100, 400]);
        s.key("Down");
        s.play("06-down", &[0, 150]);
        s.key("Return");
        s.play("07-new-window", &[0, 200, 800]);
        s.click("goal0");
        s.play("08-goal", &[0, 200, 800]);
        s.cx.simulate_key(s.win, "w", "w");
        s.key("Tab");
        s.play("09-tab-signal", &[0, 300, 900]);
        s.key("Escape");
        s.play("10-escape", &[0, 100, 400, 5000]);
    })
}

fn screen_fx() -> Fx {
    Fx::new("hud-screen", SCREEN, 2, [6, 6, 0])
}

fn minimap_fx() -> Fx {
    Fx::new("hud-minimap", MINIMAP, 2, [28, 26, 0])
}

fn main() {
    if let Some(dir) = arg("--frames") {
        let fx = screen_fx();
        if gpu_stills(&dir, 1600, 900, [("screen", [3.0f32, -1.0]), ("screen-torn", [3.1, 0.08])], |g, f| g.draw(fx.effect(), &[], f)).is_none() {
            eprintln!("screen shader: {:?}", fx.error());
        }
        let fx = minimap_fx();
        if gpu_stills(&dir, 444, 504, [("minimap", [2.0f32, 0.0]), ("minimap-secure", [3.3, 1.0])], |g, f| g.draw(fx.effect(), &[], f)).is_none() {
            eprintln!("minimap shader: {:?}", fx.error());
        }
        return;
    }
    let make = || Hud {
        screen: screen_fx(),
        minimap: minimap_fx(),
        windows: [0; PROGRAMS.len()],
        win: Panel::default(),
        launcher: Panel::default(),
        launch_sel: 0,
        ask: Panel::default(),
        ask_sel: 0,
        toasts: Vec::new(),
        toast_n: 0,
        goals: [false; GOALS.len()],
        say: ("ECHO", "I see what you see. No chatter.", 0),
        secure: false,
        torn: 0.0,
        frames: 0,
    };
    run_with(DESIGN.0, DESIGN.1, "BUI · HUD Shell", make, arg("--storyboard").map(storyboard));
}

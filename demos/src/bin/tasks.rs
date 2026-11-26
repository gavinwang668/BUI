//! Dayline: a dark desktop planner with a white task sheet, on BUI. A sidebar of places (its
//! selection pill glides between them), a search bar that filters as you type, the sheet grouped
//! Inbox / Overdue / Tomorrow (each folds away under its chevron) and a detail pane for the
//! selected task: streak, a monthly progress ring, the week, chips and subtasks typed in place.
//!
//! Motion ("calm, exact"): checking a task fills its circle, draws the tick in two strokes,
//! strikes the title through and then lets the row fold out (300 ms, 6 px up, never a full-height
//! slide); the group count rolls to its new number and a toast offers Undo. The detail pane slides
//! 40 px in and its contents stagger 45 ms apart; finishing the daily task pops the streak chip,
//! fills today's dot and sweeps the ring a step on. Everything interactive is a CSS transition
//! (interruptible); keyframes only for the one-shot sequences. Nothing redraws while nothing moves.
//!
//! Keys: ↑ / ↓ select, Space checks, Enter opens, Esc stops typing, then closes the pane.
//!
//! `cargo run --release --bin tasks`
//! `-- --storyboard DIR` drives every feature headlessly and saves frames along BUI's virtual clock.

use std::sync::OnceLock;

use blenderui::motion::MotionValue;
use blenderui::prelude::*;
use blenderui::vars;
use bui_demos::canvas::{rem, rem_css, Fit};
use bui_demos::control::arc_points;
use bui_demos::text::{edit, Edit};
use bui_demos::{arg, focus_root, now, run_with, story::story, Script};

const DESIGN: (f32, f32) = (1120.0, 680.0);
/// Seconds from the check to the row being gone (its fold ends here).
const LEAVE: f32 = 1.0;
/// Seconds the Undo toast stays.
const TOAST: f32 = 4.0;
const SIDEBAR: f32 = 212.0;
const PANE: f32 = 340.0;
const GAP: f32 = 12.0;

const CSS: &str = r"
html { height: 100%; background: #0c0c0d; }
body { margin: 0; height: 100%; overflow: hidden; background: #0c0c0d; color: #e9e9eb; user-select: none;
       font-family: Inter, system-ui, sans-serif; -webkit-font-smoothing: antialiased; font-size: 14px; }
.abs { position: absolute; }

/* -- the window chrome -- */
.top { left: 0px; top: 0px; right: 0px; height: 60px; display: flex; align-items: center; gap: 10px; padding-left: 18px; }
.lights { display: flex; gap: 8px; margin-right: 12px; }
.lights i { width: 12px; height: 12px; border-radius: 6px; display: block; }
.tb { width: 40px; height: 40px; border-radius: 12px; background: #1b1b1d; display: flex; align-items: center;
      justify-content: center; color: #b4b4b8; cursor: pointer;
      transition-property: background-color, color, transform; transition-duration: 140ms;
      transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.tb:hover { background: #262629; color: #fff; }
.tb:active { transform: scale(0.96); }
.tb.off { color: #4a4a4e; }
.tb.off:hover { background: #1b1b1d; color: #4a4a4e; }
.glyph { font-size: 17px; line-height: 1; }
.chev { width: 8px; height: 8px; border-left: 2px solid currentColor; border-bottom: 2px solid currentColor; }
.search { margin-left: 70px; width: 360px; height: 40px; border-radius: 20px; background: #1b1b1d; display: flex;
          align-items: center; gap: 10px; padding: 0 8px 0 16px; box-sizing: border-box; color: #7d7d82; cursor: text;
          box-shadow: 0 0 0 1px rgba(255,255,255,0.04);
          transition-property: box-shadow, background-color; transition-duration: 160ms; }
.search:hover { background: #202023; }
.search.on { background: #222225; box-shadow: 0 0 0 1.5px #5b5bd6, 0 0 0 5px rgba(91,91,214,0.22); }
.search .q { flex: 1; color: #e9e9eb; white-space: nowrap; overflow: hidden; }
.search .ph { flex: 1; }
.lens { width: 10px; height: 10px; border-radius: 6px; border: 2px solid currentColor; position: relative; flex: none; }
.lens i { position: absolute; width: 5px; height: 2px; background: currentColor; left: 9px; top: 11px; transform: rotate(45deg); }
.kbd { height: 24px; padding: 0 8px; border-radius: 8px; background: #2a2a2d; color: #9a9a9f; font-size: 12px;
       display: flex; align-items: center; font-weight: 600; }
.caret { display: inline-block; width: 1.5px; height: 16px; background: #8b8bf5; margin-left: 1px;
         vertical-align: -3px; animation: blink 1.06s steps(1) infinite; }
.note .caret { background: #5b5bd6; }

/* -- the sidebar -- */
.side { left: 0px; top: 60px; width: 212px; bottom: 0px; padding: 6px 12px; box-sizing: border-box;
        transition-property: transform, opacity; transition-duration: 320ms, 220ms;
        transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.side.shut { transform: translateX(-220px); opacity: 0; pointer-events: none; }
.nav { position: relative; }
.pill { position: absolute; left: 0px; top: 0px; right: 0px; height: 40px; border-radius: 12px; background: #232326;
        box-shadow: inset 0 1px 0 rgba(255,255,255,0.05);
        transition-property: transform; transition-duration: 300ms; transition-timing-function: cubic-bezier(0.3, 1.25, 0.5, 1); }
.item { position: relative; height: 40px; display: flex; align-items: center; gap: 12px; padding: 0 12px;
        color: #a3a3a8; font-size: 14.5px; border-radius: 12px; cursor: pointer;
        transition-property: color; transition-duration: 160ms;
        animation: rise 420ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.item:hover { color: #e9e9eb; }
.item.on { color: #fff; font-weight: 600; }
.sec { margin: 18px 12px 6px; font-size: 12px; color: #6c6c71; font-weight: 600; display: flex; align-items: center; gap: 6px;
       animation: rise 420ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.sec .chev { width: 5px; height: 5px; transform: rotate(-45deg); margin-top: -3px; border-width: 1.5px; }
.ico { width: 16px; height: 16px; flex: none; position: relative; box-sizing: border-box; }
.i-sun { border: 2px solid currentColor; border-radius: 8px; width: 12px; height: 12px; margin: 2px; box-shadow: 0 0 0 2px #0c0c0d, 0 0 0 3.5px currentColor; }
.i-up { border: 2px solid currentColor; border-radius: 4px; border-top-width: 4px; }
.i-cal { border: 2px solid currentColor; border-radius: 4px; }
.i-cal i { position: absolute; left: 2px; top: 2px; width: 4px; height: 4px; border-radius: 1px; background: currentColor; }
.i-note { background: linear-gradient(180deg, currentColor 0px, currentColor 2px, transparent 2px, transparent 6px,
          currentColor 6px, currentColor 8px, transparent 8px, transparent 12px, currentColor 12px, currentColor 14px, transparent 14px);
          width: 14px; margin: 2px 1px 0; }
.i-task { background: #e5484d; border-radius: 4px; }
.i-task i { position: absolute; left: 4px; top: 3px; width: 4px; height: 7px; border-right: 2px solid #fff; border-bottom: 2px solid #fff; transform: rotate(40deg); }
.i-ev { border: 2px solid currentColor; border-radius: 4px; height: 13px; margin-top: 2px; }
.i-ev i { position: absolute; right: 2px; bottom: 2px; width: 4px; height: 3px; border-radius: 1px; background: currentColor; }
.dot { width: 16px; height: 16px; border-radius: 5px; flex: none; }

/* -- the sheet -- */
.sheet { top: 64px; bottom: -24px; background: #fafaf8; border-radius: 24px; color: #1d1d1f; overflow: hidden;
         box-shadow: 0 0 0 1px rgba(0,0,0,0.4), 0 14px 30px rgba(0,0,0,0.4);
         transition-property: left, width; transition-duration: 320ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1);
         animation: sheetin 520ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.head { display: flex; align-items: center; padding: 26px 26px 10px 28px; gap: 8px; }
.head h1 { margin: 0; font-size: 36px; font-weight: 800; letter-spacing: -1.2px; flex: 1; text-wrap: balance; }
.h1 { animation: swap 320ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.chip { height: 32px; padding: 0 12px; border-radius: 16px; background: #efefec; color: #48484c; font-size: 13px; font-weight: 600;
        display: flex; align-items: center; gap: 6px; cursor: pointer;
        transition-property: background-color, transform; transition-duration: 140ms; }
.chip:hover { background: #e6e6e2; }
.chip:active { transform: scale(0.96); }
.chip .chev { width: 5px; height: 5px; transform: rotate(-45deg); margin-top: -3px; border-width: 1.5px; }
.seg { position: relative; display: flex; height: 32px; border-radius: 16px; background: #efefec; padding: 3px; box-sizing: border-box; }
.thumb { position: absolute; left: 3px; top: 3px; width: 34px; height: 26px; border-radius: 13px; background: #1d1d1f;
         transition-property: transform; transition-duration: 260ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.seg .opt { position: relative; width: 34px; height: 26px; display: flex; align-items: center; justify-content: center; cursor: pointer; color: #8e8e93;
            transition-property: color; transition-duration: 200ms; }
.seg .opt.on { color: #fff; }
.bars { width: 12px; height: 10px; background: linear-gradient(180deg, currentColor 0px, currentColor 2px, transparent 2px,
        transparent 4px, currentColor 4px, currentColor 6px, transparent 6px, transparent 8px, currentColor 8px, currentColor 10px); }
.cols { width: 12px; height: 10px; border: 1.5px solid currentColor; border-radius: 2px; box-sizing: border-box;
        background: linear-gradient(90deg, transparent 0px, transparent 3.5px, currentColor 3.5px, currentColor 5px, transparent 5px); }

.list { padding: 0 18px 0 28px; }
.group { display: flex; border-top: 1px solid #ececea; padding: 16px 0 10px;
         animation: rise 480ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.group.first { border-top-width: 0px; }
.ghead { width: 150px; flex: none; cursor: pointer; padding-top: 6px; }
.gname { display: flex; align-items: center; gap: 8px; font-size: 18px; font-weight: 700; letter-spacing: -0.3px; }
.gname .chev { width: 6px; height: 6px; border-width: 1.8px; color: #8e8e93; transform: rotate(-45deg); margin-top: -4px;
               transition-property: transform, margin-top; transition-duration: 240ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.shut .gname .chev { transform: rotate(-135deg); margin-top: 0px; }
.count { margin: 3px 0 0 16px; font-size: 12.5px; color: #8e8e93; font-variant-numeric: tabular-nums; height: 16px; overflow: hidden; }
.count span { display: inline-block; animation: roll 360ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.rows { flex: 1; overflow: hidden; max-height: 900px;
        transition-property: max-height, opacity; transition-duration: 340ms, 220ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.shut .rows { max-height: 0px; opacity: 0; }
.board { display: flex; gap: 12px; padding-right: 26px; }
.board .group { flex: 1; min-width: 0px; flex-direction: column; border-top-width: 0px; background: #f2f2ef; border-radius: 20px; padding: 12px 6px 6px; }
.board .ghead { width: auto; padding: 4px 10px 8px; }
.board .row:hover { background: #ebebe7; }
.board .row.sel { background: #fafaf8; }
.empty { color: #a1a1a6; font-size: 13.5px; padding: 10px 14px 12px; }

.row { display: flex; gap: 12px; padding: 10px 14px; border-radius: 16px; border: 1.5px solid transparent; max-height: 130px;
       cursor: pointer; overflow: hidden;
       transition-property: border-color, background-color; transition-duration: 160ms;
       animation: rise 420ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.row:hover { background: #f3f3f0; }
.row.sel { border-color: #2b2b2e; background: #fafaf8; }
.row.done { animation: leave 300ms cubic-bezier(0.2, 0, 0, 1) 650ms forwards; }
.body { flex: 1; min-width: 0px; }
.title { position: relative; display: inline-block; font-size: 15.5px; font-weight: 500; line-height: 22px;
         transition-property: color; transition-duration: 240ms; }
.done .title { color: #a1a1a6; }
.strike { position: absolute; left: -2px; right: -2px; top: 11px; height: 1.5px; background: #a1a1a6; transform: scaleX(0); transform-origin: 0px 0px; }
.done .strike { animation: draw 240ms cubic-bezier(0.2, 0, 0, 1) 140ms forwards; }
.meta { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 8px; margin-top: 5px; font-size: 12.5px; color: #8e8e93;
        font-variant-numeric: tabular-nums; }
.meta span { white-space: nowrap; }
.meta .late { color: #e5484d; font-weight: 600; }
.tag { height: 22px; padding: 0 9px; border-radius: 11px; font-size: 12px; font-weight: 600; display: flex; align-items: center; gap: 5px; }
.t-health { background: #e2f4e5; color: #2b8a3e; }
.t-work { background: #e5eefc; color: #3466d1; }
.t-ideas { background: #efe7fc; color: #7a4bd4; }
.t-home { background: #fdf0de; color: #b46d12; }
.t-plain { background: #f0f0ed; color: #5d5d62; }
.star { width: 9px; height: 9px; background: #f5b200; transform: rotate(45deg); border-radius: 2px; }
.flame { width: 8px; height: 8px; border-radius: 50% 0 50% 50%; transform: rotate(-45deg); background: currentColor; margin: 0 1px 0 2px; }

/* the check: 22 px drawn, 40 px to hit */
.hit { width: 40px; height: 40px; margin: -9px -9px -9px -9px; display: flex; align-items: center; justify-content: center; flex: none; cursor: pointer; }
.hit:active .check { transform: scale(0.96); }
.check { width: 22px; height: 22px; border-radius: 11px; border: 1.6px solid #c4c4c8; box-sizing: border-box; position: relative;
         transition-property: border-color, background-color, transform; transition-duration: 180ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.hit:hover .check { border-color: #8e8e93; }
.check.orange { border-color: #f5a524; border-width: 2px; }
.check.red { border-color: #e5484d; border-width: 2px; background: linear-gradient(90deg, #e5484d 0px, #e5484d 9px, transparent 9px); }
.check.on { background: #1d1d1f; border-color: #1d1d1f; animation: fill 280ms cubic-bezier(0.2, 0, 0, 1); }
.tick { position: absolute; left: 0px; top: 0px; width: 19px; height: 19px; }
.ta { position: absolute; left: 4.5px; top: 9px; width: 5px; height: 2px; transform: rotate(45deg); transform-origin: 0px 1px; }
.tb2 { position: absolute; left: 7.6px; top: 12.4px; width: 9px; height: 2px; transform: rotate(-52deg); transform-origin: 0px 1px; }
.ta i, .tb2 i { display: block; width: 100%; height: 2px; border-radius: 1px; background: #fff; transform: scaleX(0); transform-origin: 0px 1px; }
.ta i { animation: draw 120ms cubic-bezier(0.4, 0, 1, 1) 60ms forwards; }
.tb2 i { animation: draw 170ms cubic-bezier(0.2, 0, 0, 1) 175ms forwards; }

/* -- the detail pane -- */
.pane { top: 64px; bottom: -24px; width: 340px; background: #fafaf8; border-radius: 24px; color: #1d1d1f; overflow: hidden;
        box-shadow: 0 0 0 1px rgba(0,0,0,0.4), 0 14px 30px rgba(0,0,0,0.4);
        transition-property: transform, opacity; transition-duration: 340ms, 220ms; transition-timing-function: cubic-bezier(0.2, 0, 0, 1); }
.pane.shut { transform: translateX(40px); opacity: 0; pointer-events: none; }
.pin { padding: 18px 18px 0 22px; }
.ptop { display: flex; align-items: center; gap: 4px; height: 40px; }
.ptop .sp { flex: 1; }
.rb { width: 32px; height: 32px; border-radius: 16px; display: flex; align-items: center; justify-content: center; color: #8e8e93;
      cursor: pointer; font-size: 15px; transition-property: background-color, color, transform; transition-duration: 140ms; }
.rb:hover { background: #efefec; color: #1d1d1f; }
.rb:active { transform: scale(0.96); }
.st { animation: rise 380ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.ptitle { font-size: 26px; font-weight: 800; letter-spacing: -0.7px; margin: 10px 0 12px; text-wrap: balance; line-height: 1.15;
          transition-property: color; transition-duration: 240ms; }
.ptitle.done { color: #a1a1a6; }
.chips { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 8px; }
.pchip { height: 28px; padding: 0 11px; border-radius: 14px; font-size: 12.5px; font-weight: 700; display: flex; align-items: center; gap: 6px;
         font-variant-numeric: tabular-nums; }
.streak { background: #ff4b36; color: #fff; box-shadow: 0 3px 8px rgba(255,75,54,0.3); }
.pk { display: flex; align-items: center; gap: 6px; }
.pk.pop { animation: pop 460ms cubic-bezier(0.3, 1.6, 0.5, 1); }
.daily { background: #efefec; color: #48484c; }
.when { background: #fde9e9; color: #d93036; }
.plus { width: 28px; height: 28px; border-radius: 14px; border: 1.5px dashed #c4c4c8; box-sizing: border-box; display: flex;
        align-items: center; justify-content: center; color: #8e8e93; font-size: 16px; }
.mini { width: 14px; height: 14px; }
.plusone { position: relative; }
.plusone b { position: absolute; left: 30px; top: -2px; color: #ff4b36; font-size: 14px; font-weight: 800;
             animation: float 900ms cubic-bezier(0.2, 0, 0, 1) forwards; }

.card { margin-top: 14px; border-radius: 18px; background: #f2f2ef; padding: 14px; display: flex; gap: 14px; align-items: center; }
.ring { position: relative; width: 64px; height: 64px; flex: none; }
.ring .num { position: absolute; left: 0px; top: 0px; width: 64px; height: 64px; display: flex; align-items: center; justify-content: center;
             font-size: 15px; font-weight: 800; font-variant-numeric: tabular-nums; letter-spacing: -0.3px; }
.ring canvas { position: absolute; left: 0px; top: 0px; width: 64px; height: 64px; }
.week { flex: 1; }
.week .lbl { font-size: 12px; color: #8e8e93; font-weight: 600; margin-bottom: 8px; }
.days { display: flex; gap: 3px; }
.day { width: 24px; text-align: center; font-size: 10.5px; color: #a1a1a6; font-weight: 600; }
.day i { display: block; width: 20px; height: 20px; margin: 0 2px 4px; border-radius: 10px; background: #e3e3df; }
.day i.on { background: #ff4b36; }
.day i.now { animation: pop 460ms cubic-bezier(0.3, 1.6, 0.5, 1); box-shadow: 0 0 0 3px rgba(255,75,54,0.25); }

.subs { margin-top: 16px; }
.sub { display: flex; align-items: center; gap: 10px; height: 34px; font-size: 14px; cursor: pointer;
       animation: rise 340ms cubic-bezier(0.2, 0, 0, 1) backwards; }
.sub .box { width: 16px; height: 16px; border-radius: 5px; border: 1.5px solid #c4c4c8; box-sizing: border-box; position: relative;
            transition-property: background-color, border-color; transition-duration: 160ms; }
.sub.on .box { background: #1d1d1f; border-color: #1d1d1f; }
.sub.on .box i { position: absolute; left: 4px; top: 1.5px; width: 4px; height: 7px; border-right: 1.8px solid #fff; border-bottom: 1.8px solid #fff; transform: rotate(40deg); }
.sub.on .txt { color: #a1a1a6; }
.note { margin-top: 6px; min-height: 40px; border-radius: 12px; padding: 10px 12px; box-sizing: border-box; color: #a1a1a6; font-size: 14px;
        cursor: text; transition-property: background-color, box-shadow; transition-duration: 160ms; }
.note:hover { background: #f2f2ef; }
.note.on { background: #fff; color: #1d1d1f; box-shadow: 0 0 0 1.5px #5b5bd6, 0 0 0 5px rgba(91,91,214,0.16); }

/* -- toast -- */
.toasts { left: 0px; right: 0px; bottom: 26px; display: flex; justify-content: center; pointer-events: none; }
.toast { pointer-events: auto; height: 44px; border-radius: 22px; background: #1d1d1f; color: #e9e9eb; display: flex; align-items: center;
         gap: 14px; padding: 0 6px 0 18px; font-size: 13.5px; box-shadow: 0 0 0 1px rgba(255,255,255,0.1), 0 10px 22px rgba(0,0,0,0.38);
         animation: toast 4000ms cubic-bezier(0.2, 0, 0, 1) forwards; }
.toast .ok { width: 18px; height: 18px; border-radius: 9px; background: #30a46c; position: relative; }
.toast .ok i { position: absolute; left: 6px; top: 3px; width: 4px; height: 8px; border-right: 2px solid #fff; border-bottom: 2px solid #fff; transform: rotate(40deg); }
.undo { height: 32px; padding: 0 14px; border-radius: 16px; background: #2e2e31; color: #fff; font-weight: 600; display: flex; align-items: center;
        cursor: pointer; transition-property: background-color, transform; transition-duration: 140ms; }
.undo:hover { background: #3a3a3e; }
.undo:active { transform: scale(0.96); }

@keyframes rise { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0px); } }
@keyframes sheetin { from { opacity: 0; transform: translateY(18px); } to { opacity: 1; transform: translateY(0px); } }
@keyframes swap { from { opacity: 0; transform: translateY(6px); filter: blur(4px); } to { opacity: 1; transform: translateY(0px); filter: blur(0px); } }
@keyframes roll { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0px); } }
@keyframes leave { 0% { opacity: 1; transform: translateY(0px); max-height: 130px; }
                   100% { opacity: 0; transform: translateY(-6px); max-height: 0px; padding-top: 0px; padding-bottom: 0px; } }
@keyframes draw { from { transform: scaleX(0); } to { transform: scaleX(1); } }
@keyframes fill { 0% { transform: scale(1); } 45% { transform: scale(0.86); } 100% { transform: scale(1); } }
@keyframes pop { 0% { transform: scale(1); } 40% { transform: scale(1.18); } 100% { transform: scale(1); } }
@keyframes float { 0% { opacity: 0; transform: translateY(6px); } 25% { opacity: 1; } 100% { opacity: 0; transform: translateY(-18px); } }
@keyframes blink { 0% { opacity: 1; } 50% { opacity: 0; } }
@keyframes toast { 0% { opacity: 0; transform: translateY(14px) scale(0.98); } 8% { opacity: 1; transform: translateY(0px) scale(1); }
                   90% { opacity: 1; transform: translateY(0px); } 100% { opacity: 0; transform: translateY(6px); } }
";

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tag {
    Health,
    Work,
    Ideas,
    Home,
}

impl Tag {
    const fn class(self) -> &'static str {
        match self {
            Self::Health => "tag t-health",
            Self::Work => "tag t-work",
            Self::Ideas => "tag t-ideas",
            Self::Home => "tag t-home",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Health => "Health",
            Self::Work => "Work",
            Self::Ideas => "Ideas",
            Self::Home => "Home",
        }
    }
}

/// The ring a task's circle is drawn with: its priority.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ring {
    Plain,
    Orange,
    Red,
}

struct Task {
    id: u32,
    title: &'static str,
    group: usize,
    /// Date, and a time (drawn red: late).
    when: Option<(&'static str, &'static str)>,
    tag: Option<Tag>,
    /// Shown as a plain chip with a star (a linked note).
    link: Option<&'static str>,
    ring: Ring,
    daily: bool,
    /// When it was checked (`now()`); gone once the fold has played.
    done: Option<f32>,
    gone: bool,
    /// Bumped by Undo: keys the row so its entrance plays again.
    born: u32,
    subs: Vec<(String, bool)>,
}

const fn task(id: u32, title: &'static str, group: usize, when: Option<(&'static str, &'static str)>, tag: Option<Tag>,
              ring: Ring) -> Task {
    Task { id, title, group, when, tag, link: None, ring, daily: false, done: None, gone: false, born: 0, subs: Vec::new() }
}

const GROUPS: [&str; 3] = ["Inbox", "Overdue", "Tomorrow"];
/// Sidebar places: name, icon class, and which groups the sheet shows for it.
const NAV: [(&str, &str, &[usize]); 6] = [
    ("Today", "i-sun", &[0, 1]),
    ("Upcoming", "i-up", &[2]),
    ("Calendar", "i-cal", &[]),
    ("Notes", "i-note", &[]),
    ("Tasks", "i-task", &[0, 1, 2]),
    ("Events", "i-ev", &[]),
];
const FAVORITES: [(&str, &str); 1] = [("Welcome to Dayline", "#f5b200")];
const RECENT: [(&str, &str); 4] = [
    ("Everything Dayline can do", "#e5484d"),
    ("Weekend in Lisbon", "#3e9bff"),
    ("Product launch plan", "#30a46c"),
    ("App ideas", "#f5a524"),
];
const WEEK: [&str; 7] = ["M", "T", "W", "T", "F", "S", "S"];

/// Which text field takes the keys.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    Search,
    Note,
}

struct Dayline {
    tasks: Vec<Task>,
    next_id: u32,
    nav: usize,
    /// The sheet's title re-keys on every place change, so it cross-fades.
    navs: u32,
    shut: [bool; 3],
    board: bool,
    sidebar: bool,
    sel: Option<u32>,
    open: bool,
    field: Field,
    query: String,
    note: String,
    streak: u32,
    /// Daily progress (of 100) and the ring that sweeps to it.
    reps: u32,
    ring: MotionValue,
    /// The last task checked and when (the toast and its Undo).
    toast: Option<(u32, f32)>,
    toasts: u32,
    frames: u32,
}

impl Dayline {
    fn new() -> Self {
        let mut daily = task(3, "Pushups ×100", 1, Some(("Thu, Oct 29", "7:30")), Some(Tag::Health), Ring::Plain);
        daily.daily = true;
        let mut welcome = task(2, "Check this off to see it complete", 0, None, Some(Tag::Ideas), Ring::Plain);
        welcome.link = Some("Welcome to Dayline");
        let mut anna = task(4, "Reply to Anna about the kickoff", 1, Some(("Thu, Oct 29", "16:00")), Some(Tag::Work), Ring::Orange);
        anna.subs = vec![("Confirm the room".into(), true), ("Share the agenda".into(), false)];
        let tasks = vec![
            task(1, "Read last year's brief", 0, None, None, Ring::Plain),
            welcome,
            daily,
            anna,
            task(5, "Review Mark's pull request", 1, Some(("Thu, Oct 29", "17:00")), Some(Tag::Work), Ring::Red),
            task(6, "Book flights to Lisbon", 2, Some(("Fri, Oct 30", "9:00")), Some(Tag::Home), Ring::Plain),
            task(7, "Draft the launch announcement", 2, Some(("Fri, Oct 30", "")), Some(Tag::Ideas), Ring::Plain),
            task(8, "Water the plants", 2, None, None, Ring::Plain),
        ];
        Self {
            tasks, next_id: 9, nav: 4, navs: 0, shut: [false; 3], board: false, sidebar: true, sel: Some(3), open: true,
            field: Field::None, query: String::new(), note: String::new(), streak: 5, reps: 12,
            ring: MotionValue::new(0.12f32), toast: None, toasts: 0, frames: 0,
        }
    }

    fn get(&self, id: u32) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    fn get_mut(&mut self, id: u32) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    fn matches(&self, t: &Task) -> bool {
        self.query.is_empty() || t.title.to_lowercase().contains(&self.query.to_lowercase())
    }

    /// The rows on the sheet, top to bottom (what ↑ / ↓ walk).
    fn visible(&self) -> impl Iterator<Item = &Task> {
        NAV[self.nav].2.iter().filter(|&&g| !self.shut[g])
            .flat_map(move |&g| self.tasks.iter().filter(move |t| t.group == g && !t.gone && self.matches(t)))
    }

    fn count(&self, g: usize) -> usize {
        self.tasks.iter().filter(|t| t.group == g && t.done.is_none() && self.matches(t)).count()
    }

    /// Checks (or unchecks) a task. Checking the daily one moves the streak, the reps and the ring.
    fn toggle(&mut self, id: u32) {
        let t0 = now();
        let Some(t) = self.get_mut(id) else { return };
        let daily = t.daily;
        let checking = t.done.is_none();
        t.done = if checking { Some(t0) } else { None };
        if checking {
            self.toast = Some((id, t0));
            self.toasts += 1;
        } else {
            self.toast = None;
        }
        if daily {
            if checking {
                self.streak += 1;
                self.reps += 1;
            } else {
                self.streak -= 1;
                self.reps -= 1;
            }
            drop(self.ring.animate(self.reps as f32 / 100.0, vars! { duration: 0.7, ease: "power3.out" }));
        }
    }

    fn undo(&mut self) {
        if let Some((id, _)) = self.toast.take()
            && let Some(t) = self.get_mut(id)
        {
            let daily = t.daily;
            t.done = None;
            t.gone = false;
            t.born += 1;
            if daily {
                self.streak -= 1;
                self.reps -= 1;
                drop(self.ring.animate(self.reps as f32 / 100.0, vars! { duration: 0.7, ease: "power3.out" }));
            }
        }
    }

    /// ↑ / ↓: the row above or below the selection, wrapping.
    fn step(&mut self, down: bool) {
        let ids: Vec<u32> = self.visible().map(|t| t.id).collect();
        let n = ids.len();
        if n == 0 {
            return;
        }
        let at = self.sel.and_then(|s| ids.iter().position(|&i| i == s));
        let next = match (at, down) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(a), true) => (a + 1) % n,
            (Some(a), false) => (a + n - 1) % n,
        };
        self.sel = Some(ids[next]);
    }

    fn compose(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.insert(0, task(id, "New task", 0, None, None, Ring::Plain));
        self.shut[0] = false;
        self.sel = Some(id);
        self.open = true;
        if !NAV[self.nav].2.contains(&0) {
            self.nav = 4;
            self.navs += 1;
        }
    }

    /// Keys: into the focused field, else the list.
    fn key(&mut self, e: &KeyDownEvent) -> bool {
        let k = &e.keystroke;
        if self.field != Field::None {
            let text = if self.field == Field::Search { &mut self.query } else { &mut self.note };
            match edit(text, k, 200) {
                Edit::Leave => self.field = Field::None,
                Edit::Submit => {
                    if self.field == Field::Note && !self.note.trim().is_empty() {
                        let s = std::mem::take(&mut self.note);
                        if let Some(id) = self.sel
                            && let Some(t) = self.get_mut(id)
                        {
                            t.subs.push((s, false));
                        }
                    } else if self.field == Field::Search {
                        self.field = Field::None;
                    }
                }
                Edit::Typed => {}
                Edit::Ignored => return false,
            }
            return true;
        }
        match k.key.as_str() {
            "Up" => self.step(false),
            "Down" => self.step(true),
            "space" => {
                if let Some(id) = self.sel {
                    self.toggle(id);
                }
            }
            "Return" | "KP_Enter" => self.open = self.sel.is_some(),
            "Escape" => self.open = false,
            "k" | "K" if k.modifiers.control => self.field = Field::Search,
            _ => return false,
        }
        true
    }

    /// Rows whose fold has played are gone; answers whether anything still moves on its own.
    fn settle(&mut self) -> bool {
        let t = now();
        let mut moving = self.ring.is_animating();
        for task in &mut self.tasks {
            if let Some(d) = task.done
                && !task.gone
            {
                if t - d > LEAVE {
                    task.gone = true;
                } else {
                    moving = true;
                }
            }
        }
        if let Some((_, at)) = self.toast {
            if t - at > TOAST {
                self.toast = None;
            } else {
                moving = true;
            }
        }
        moving
    }
}

/// The round check: 22 px, in a 40 px hit area.
fn check(id: String, ring: Ring, done: bool, born: u32) -> Div {
    let class = match (done, ring) {
        (true, _) => "check on",
        (false, Ring::Orange) => "check orange",
        (false, Ring::Red) => "check red",
        (false, Ring::Plain) => "check",
    };
    let mut c = div().class(class);
    if done {
        // keyed so the tick draws again on every check
        c = c.child(div().id(format!("{id}-tick{born}")).class("tick")
            .child(div().class("ta").child(i())).child(div().class("tb2").child(i())));
    }
    div().id(id).class("hit").child(c)
}

/// A ring of `size` px: the track and `value` (0..1) of it, from 12 o'clock.
fn ring(value: MotionValue, size: f32, width: f32, track: u32, color: u32) -> Div {
    canvas(move |b, p| {
        let s = b.size.width / size;
        let (cx, cy, r) = (b.origin.x + b.size.width / 2.0, b.origin.y + b.size.height / 2.0, (size - width) / 2.0 * s);
        p.line(&arc_points(cx, cy, r, 0.0, 360.0), rgb(track), width * s);
        let v = value.get().clamp(0.0, 1.0);
        if v > 0.002 {
            p.line(&arc_points(cx, cy, r, 0.0, 360.0 * v), rgb(color), width * s);
        }
    })
}

impl Dayline {
    fn top(&self, cx: &mut Context<Self>) -> Div {
        let search = if self.field == Field::Search { "search on" } else { "search" };
        let mut s = div().id("search").class(search).child(div().class("lens").child(i()));
        s = if self.query.is_empty() && self.field != Field::Search {
            s.child(div().class("ph").child("Search, or create anything…"))
        } else {
            let mut q = div().class("q").child(self.query.clone());
            if self.field == Field::Search {
                q = q.child(span().class("caret"));
            }
            s.child(q)
        };
        s = s.child(div().class("kbd").child("Ctrl K"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.field = Field::Search;
                cx.stop_propagation();
                cx.notify();
            }));
        div().class("abs top")
            .child(div().class("lights").child(i().css("background", "#ff5f57")).child(i().css("background", "#febc2e"))
                .child(i().css("background", "#28c840")))
            .child(div().id("side-toggle").class("tb").child(div().class("ico i-cal").css("width", rem(16.0)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.sidebar = !this.sidebar;
                    cx.notify();
                })))
            .child(div().id("compose").class("tb").child(span().class("glyph").child("+"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.compose();
                    cx.notify();
                })))
            .child(div().class("tb").child(div().class("chev").css("transform", "rotate(45deg)").css("margin-left", rem(3.0))))
            .child(div().class("tb off").child(div().class("chev").css("transform", "rotate(-135deg)").css("margin-right", rem(3.0))))
            .child(s)
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let mut nav = div().class("nav")
            .child(div().class("pill").css("transform", format!("translateY({})", rem(self.nav as f32 * 40.0))));
        for (k, (name, ico, _)) in NAV.iter().enumerate() {
            let class = if k == self.nav { "item on" } else { "item" };
            let mut icon = div().class("ico").class(*ico);
            if matches!(*ico, "i-cal" | "i-task" | "i-ev") {
                icon = icon.child(i());
            }
            nav = nav.child(div().id(format!("nav{k}")).class(class).css("animation-delay", format!("{}ms", 80 + k * 35))
                .child(icon).child(*name)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.nav != k {
                        this.nav = k;
                        this.navs += 1;
                        cx.notify();
                    }
                })));
        }
        let mut side = div().class(if self.sidebar { "abs side" } else { "abs side shut" }).child(nav);
        let mut at = 300;
        for (title, items) in [("Favorites", &FAVORITES[..]), ("Recent", &RECENT[..])] {
            side = side.child(div().class("sec").css("animation-delay", format!("{at}ms")).child(title).child(div().class("chev")));
            for (name, color) in items {
                at += 35;
                side = side.child(div().class("item").css("animation-delay", format!("{at}ms"))
                    .child(div().class("dot").css("background", *color)).child(*name));
            }
            at += 35;
        }
        side
    }

    fn row(&self, t: &Task, k: usize, cx: &mut Context<Self>) -> Div {
        let id = t.id;
        let mut class = String::from("row");
        if self.sel == Some(id) {
            class.push_str(" sel");
        }
        if t.done.is_some() {
            class.push_str(" done");
        }
        let mut meta = div().class("meta");
        let mut any = false;
        if let Some(link) = t.link {
            meta = meta.child(div().class("tag t-plain").child(div().class("star")).child(link));
            any = true;
        }
        if let Some((date, time)) = t.when {
            meta = meta.child(span().child(date));
            if !time.is_empty() {
                meta = meta.child(span().class("late").child(time));
            }
            any = true;
        }
        if let Some(tag) = t.tag {
            meta = meta.child(div().class(tag.class()).child(tag.name()));
            any = true;
        }
        let mut body = div().class("body").child(div().class("title").child(t.title).child(div().class("strike")));
        if any {
            body = body.child(meta);
        }
        div().id(format!("row{id}-{}", t.born)).class(class).css("animation-delay", format!("{}ms", 260 + k * 40))
            .child(check(format!("chk{id}"), t.ring, t.done.is_some(), t.born)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle(id);
                    this.sel = Some(id);
                    cx.stop_propagation();
                    cx.notify();
                })))
            .child(body)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.sel = Some(id);
                this.open = true;
                this.field = Field::None;
                cx.notify();
            }))
    }

    fn sheet(&self, fit: &Fit, cx: &mut Context<Self>) -> Div {
        let left = if self.sidebar { SIDEBAR } else { GAP };
        let width = fit.w - left - GAP - if self.open { PANE + GAP } else { 0.0 };
        let (name, _, groups) = NAV[self.nav];
        let thumb = if self.board { 34.0 } else { 0.0 };
        let head = div().class("head")
            .child(h1().id(format!("h1-{}", self.navs)).class("h1").child(name))
            .child(div().class("chip").child("Due date").child(div().class("chev")))
            .child(div().class("chip").css("padding", "0").css("width", rem(32.0)).css("justify-content", "center")
                .child(div().class("bars").css("width", rem(12.0)).css("height", rem(8.0))))
            .child(div().class("seg")
                .child(div().class("thumb").css("transform", format!("translateX({})", rem(thumb))))
                .child(div().id("list-view").class(if self.board { "opt" } else { "opt on" }).child(div().class("bars"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.board = false;
                        cx.notify();
                    })))
                .child(div().id("board-view").class(if self.board { "opt on" } else { "opt" }).child(div().class("cols"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.board = true;
                        cx.notify();
                    }))));
        let mut list = div().id("list").class(if self.board { "list board" } else { "list" });
        if groups.is_empty() {
            list = list.child(div().id(format!("empty{}", self.navs)).class("empty st")
                .child(format!("Nothing in {name} yet. Press + to add something.")));
        }
        let mut k = 0;
        for (n, &g) in groups.iter().enumerate() {
            let count = self.count(g);
            let mut rows = div().id(format!("rows{g}")).class("rows");
            let mut any = false;
            for t in self.tasks.iter().filter(|t| t.group == g && !t.gone && self.matches(t)) {
                rows = rows.child(self.row(t, k, cx));
                k += 1;
                any = true;
            }
            if !any {
                rows = rows.child(div().class("empty").child(if self.query.is_empty() { "All clear." } else { "No matches." }));
            }
            let mut class = String::from("group");
            if n == 0 {
                class.push_str(" first");
            }
            if self.shut[g] {
                class.push_str(" shut");
            }
            list = list.child(div().id(format!("g{g}-{}", self.navs)).class(class).css("animation-delay", format!("{}ms", 180 + n * 70))
                .child(div().id(format!("grp{g}")).class("ghead")
                    .child(div().class("gname").child(div().class("chev")).child(GROUPS[g]))
                    // keyed by the number: a new count rolls in
                    .child(div().class("count").child(span().id(format!("c{g}-{count}"))
                        .child(format!("{count} task{}", if count == 1 { "" } else { "s" }))))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.shut[g] = !this.shut[g];
                        cx.notify();
                    })))
                .child(rows));
        }
        div().class("abs sheet").css("left", rem(left)).css("width", rem(width)).child(head).child(list)
    }

    fn pane(&self, fit: &Fit, cx: &mut Context<Self>) -> Div {
        let pane = div().class(if self.open { "abs pane" } else { "abs pane shut" }).css("left", rem(fit.w - GAP - PANE));
        let Some(t) = self.sel.and_then(|id| self.get(id)) else { return pane };
        let id = t.id;
        let done = t.done.is_some();
        // every child keyed by the task: a new selection staggers in again
        let mut k = 0;
        let mut st = |d: Div| {
            k += 1;
            d.id(format!("p{id}-{}-{k}", t.born)).class("st").css("animation-delay", format!("{}ms", 60 + k * 45))
        };
        let top = div().class("ptop")
            .child(check(format!("pchk{id}"), t.ring, done, t.born + 1000)
                .css("margin", "0")
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle(id);
                    cx.notify();
                })))
            .child(div().class("sp"))
            .child(div().class("rb").child("↗"))
            .child(div().class("rb").child("•••").css("font-size", rem(9.0)).css("letter-spacing", rem(1.0)))
            .child(div().id("close").class("rb").child("×").css("font-size", rem(20.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.open = false;
                    this.field = Field::None;
                    cx.notify();
                })));
        let mut inner = div().class("pin").child(st(top))
            .child(st(div().class(if done { "ptitle done" } else { "ptitle" }).child(t.title)));
        let mut chips = div().class("chips");
        if t.daily {
            let mut streak = div().class("pchip streak")
                .child(div().id(format!("streak{}", self.streak)).class(if self.streak > 5 { "pk pop" } else { "pk" })
                    .child(div().class("flame")).child(format!("{} in a row", self.streak)));
            if self.streak > 5 {
                streak = streak.child(div().class("plusone").child(b().id(format!("plus{}", self.streak)).child("+1")));
            }
            chips = chips.child(streak)
                .child(div().class("pchip daily").child(ring(self.ring, 14.0, 2.5, 0xd5d5d0, 0xff4b36).class("mini"))
                    .child(format!("Daily · {}/100", self.reps)));
        }
        if let Some((date, time)) = t.when {
            let label = if time.is_empty() { date.to_string() } else { format!("{date}, {time}") };
            chips = chips.child(div().class("pchip when").child(label));
        }
        if let Some(tag) = t.tag {
            chips = chips.child(div().class(tag.class()).css("height", rem(28.0)).css("border-radius", rem(14.0)).child(tag.name()));
        }
        chips = chips.child(div().class("plus").child("+"));
        inner = inner.child(st(chips));
        if t.daily {
            let mut days = div().class("days");
            let filled = (self.streak as usize).min(7);
            for (d, name) in WEEK.iter().enumerate() {
                let on = d < filled;
                let now = on && d + 1 == filled && self.streak > 5;
                let class = match (on, now) {
                    (true, true) => "on now",
                    (true, false) => "on",
                    _ => "",
                };
                days = days.child(div().class("day").child(i().id(format!("wk{d}-{}", self.streak)).class(class)).child(*name));
            }
            inner = inner.child(st(div().class("card")
                .child(div().class("ring").child(ring(self.ring, 64.0, 7.0, 0xe1e1dc, 0xff4b36))
                    .child(div().class("num").child(format!("{}%", self.reps))))
                .child(div().class("week").child(div().class("lbl").child("This week")).child(days))));
        }
        let mut subs = div().class("subs");
        for (n, (text, on)) in t.subs.iter().enumerate() {
            subs = subs.child(div().id(format!("sub{id}-{n}")).class(if *on { "sub on" } else { "sub" })
                .child(div().class("box").child(i())).child(span().class("txt").child(text.clone()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(t) = this.get_mut(id)
                        && let Some(s) = t.subs.get_mut(n)
                    {
                        s.1 = !s.1;
                        cx.notify();
                    }
                })));
        }
        let editing = self.field == Field::Note;
        let mut note = div().id("note").class(if editing { "note on" } else { "note" });
        note = if self.note.is_empty() && !editing {
            note.child("Write a subtask or a note…")
        } else {
            note.child(span().child(self.note.clone())).child(span().class("caret"))
        };
        subs = subs.child(note.on_click(cx.listener(|this, _, _, cx| {
            this.field = Field::Note;
            cx.stop_propagation();
            cx.notify();
        })));
        inner = inner.child(st(subs));
        pane.child(inner)
    }
}

impl Render for Dayline {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        focus_root(w, &mut self.frames);
        // only while something moves on its own (a fold, the toast, the ring); CSS runs the rest
        if self.settle() || self.frames < 3 {
            w.request_animation_frame();
        }
        let fit = Fit::new(w, (DESIGN.0, DESIGN.1));
        let mut ui = div().class("abs").css("left", "0").css("top", "0").css("width", rem(fit.w)).css("height", rem(fit.h))
            .child(self.top(cx))
            .child(self.sidebar(cx))
            .child(self.sheet(&fit, cx))
            .child(self.pane(&fit, cx));
        if let Some((id, _)) = self.toast
            && let Some(t) = self.get(id)
        {
            ui = ui.child(div().class("abs toasts").child(div().id(format!("toast{}", self.toasts)).class("toast")
                .child(div().class("ok").child(i())).child(format!("Completed “{}”", t.title))
                .child(div().id("undo").class("undo").child("Undo").on_click(cx.listener(|this, _, _, cx| {
                    this.undo();
                    cx.stop_propagation();
                    cx.notify();
                })))));
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
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.field != Field::None {
                        this.field = Field::None;
                        cx.notify();
                    }
                }))
                .child(ui))
    }
}

/// Every feature, headlessly: the entrance, checks (with the fold and the toast), the daily task's
/// streak, keys, a new selection, typed subtasks, folding a group, places, search, Undo, the sidebar.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        s.play("00-enter", &[0, 120, 300, 600, 1100]);
        s.cx.simulate_mouse_move(s.win, 600.0, 300.0);
        s.click("chk2");
        s.play("01-check", &[0, 100, 220, 400, 800, 1200]);
        // the daily one: streak, ring, week
        s.key("Down");
        s.key("Up");
        s.click("row3-0");
        s.cx.simulate_key(s.win, " ", "space");
        s.play("02-daily", &[0, 150, 400, 800, 1300]);
        s.click("undo");
        s.play("03-undo", &[0, 200, 500]);
        s.click("row4-0");
        s.play("04-select", &[0, 60, 150, 300, 600]);
        s.click("note");
        s.type_text("Book the venue");
        s.key("Return");
        s.play("05-subtask", &[0, 120, 400]);
        s.type_text("Send the deck");
        s.play("05-typing", &[0, 600]);
        s.key("Return");
        s.key("Escape");
        s.play("05-added", &[0, 400]);
        s.click("grp0");
        s.play("06-fold", &[0, 120, 400]);
        s.click("grp0");
        s.play("06-unfold", &[0, 120, 400]);
        s.click("nav0");
        s.play("07-today", &[0, 100, 200, 400]);
        s.click("nav2");
        s.play("07-calendar", &[0, 400]);
        s.click("nav4");
        s.click("search");
        s.type_text("re");
        s.play("08-search", &[0, 400]);
        s.key("BackSpace");
        s.key("BackSpace");
        s.key("Escape");
        s.key("Escape");
        s.play("09-close", &[0, 120, 400]);
        s.click("side-toggle");
        s.play("10-sidebar", &[0, 150, 450]);
        s.click("board-view");
        s.click("compose");
        s.play("11-compose", &[0, 150, 500]);
    })
}

fn main() {
    run_with(DESIGN.0, DESIGN.1, "BUI · Dayline", Dayline::new, arg("--storyboard").map(storyboard));
}

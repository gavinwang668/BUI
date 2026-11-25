//! A GitHub Actions workflow as a Blender node graph, painted with BUI's `Painter` (nodes, bezier
//! links, sockets, progress) and run on a timeline: jobs light up, links flow, the log scrolls.
//! `cargo run --release --bin pipeline`

use bui_demos::{now, run};
use blenderui::prelude::*;

/// Seconds of wall time per scheduled unit.
const PACE: f32 = 0.45;
const NODE_W: f32 = 176.0;
const NODE_H: f32 = 74.0;

#[derive(Clone, Copy)]
struct Job {
    name: &'static str,
    color: u32,
    x: f32,
    y: f32,
    /// Units this job runs for.
    secs: f32,
    /// Indices of the jobs it needs (`needs:` in the workflow).
    needs: &'static [usize],
}

const JOBS: [Job; 8] = [
    Job { name: "actions/checkout", color: 0x6b4c9a, x: 30.0, y: 190.0, secs: 1.0, needs: &[] },
    Job { name: "Lint · clippy", color: 0x4772b3, x: 280.0, y: 30.0, secs: 2.0, needs: &[0] },
    Job { name: "Build · ubuntu", color: 0x2e8b57, x: 280.0, y: 140.0, secs: 4.0, needs: &[0] },
    Job { name: "Build · windows", color: 0x2e8b57, x: 280.0, y: 250.0, secs: 5.0, needs: &[0] },
    Job { name: "Build · macos", color: 0x2e8b57, x: 280.0, y: 360.0, secs: 4.0, needs: &[0] },
    Job { name: "Test · matrix", color: 0xc26a1b, x: 540.0, y: 190.0, secs: 3.0, needs: &[1, 2, 3, 4] },
    Job { name: "Package · wheels", color: 0xb03a5b, x: 770.0, y: 190.0, secs: 2.0, needs: &[5] },
    Job { name: "Publish · release", color: 0xb03a5b, x: 1000.0, y: 190.0, secs: 1.0, needs: &[6] },
];
const FLAKY: usize = 5;

#[derive(Clone, Copy, PartialEq)]
enum State {
    Queued,
    Running(f32),
    Done,
    Failed,
    Skipped,
}

/// When each job starts and ends (units), from its `needs`.
fn schedule() -> [(f32, f32); JOBS.len()] {
    let mut s = [(0.0, 0.0); JOBS.len()];
    for (i, j) in JOBS.iter().enumerate() {
        let start = j.needs.iter().map(|&n| s[n].1).fold(0.0, f32::max);
        s[i] = (start, start + j.secs);
    }
    s
}

/// Every job's state `t` units into a run; jobs after a failure are skipped.
fn states(t: f32, fail: bool) -> [State; JOBS.len()] {
    let s = schedule();
    let mut out = [State::Queued; JOBS.len()];
    for i in 0..JOBS.len() {
        let (a, b) = s[i];
        out[i] = if JOBS[i].needs.iter().any(|&n| matches!(out[n], State::Failed | State::Skipped)) {
            State::Skipped
        } else if t >= b {
            if fail && i == FLAKY { State::Failed } else { State::Done }
        } else if t >= a {
            State::Running((t - a) / (b - a))
        } else {
            State::Queued
        };
    }
    out
}

const fn tint(s: State) -> u32 {
    match s {
        State::Running(_) => 0xe8b30d,
        State::Done => 0x4caf50,
        State::Failed => 0xe5534b,
        State::Queued | State::Skipped => 0x8a8a8a,
    }
}

/// A Blender node link: a horizontal-tangent cubic bezier from an output to an input socket.
fn link(p: &mut Painter, (x1, y1): (f32, f32), (x2, y2): (f32, f32), color: u32, width: f32) {
    let d = ((x2 - x1) * 0.5).abs().max(40.0);
    let pts: [(f32, f32); 25] = std::array::from_fn(|i| {
        let t = i as f32 / 24.0;
        let u = 1.0 - t;
        let (a, b, c, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        (a * x1 + b * (x1 + d) + c * (x2 - d) + e * x2, a * y1 + b * y1 + c * y2 + e * y2)
    });
    p.line(&pts, rgb(color), width);
}

fn paint_graph(b: Bounds<f32>, p: &mut Painter, st: &[State; JOBS.len()], t: f32) {
    let (ox, oy) = (b.origin.x, b.origin.y);
    p.fill(b, rgb(0x232323));
    // dot grid, like the node editor's background
    let mut y = 0.0;
    while y < b.size.height {
        let mut x = 0.0;
        while x < b.size.width {
            p.fill(Bounds { origin: point(ox + x, oy + y), size: size(1.5, 1.5) }, rgb(0x333333));
            x += 20.0;
        }
        y += 20.0;
    }
    let out_sock = |j: &Job| (ox + j.x + NODE_W, oy + j.y + 34.0);
    let in_sock = |j: &Job, k: usize| (ox + j.x, oy + j.y + 34.0 + k as f32 * 9.0);
    for (i, j) in JOBS.iter().enumerate() {
        for (k, &n) in j.needs.iter().enumerate() {
            let c = tint(st[i]);
            link(p, out_sock(&JOBS[n]), in_sock(j, k), c, if matches!(st[i], State::Running(_)) { 3.0 } else { 2.2 });
            if let State::Running(_) = st[i] {
                // a pulse travelling down the wire
                let f = (t * 1.6).fract();
                let ((x1, y1), (x2, y2)) = (out_sock(&JOBS[n]), in_sock(j, k));
                let (x, y) = (x1 + (x2 - x1) * f, y1 + (y2 - y1) * (f * f * (3.0 - 2.0 * f)));
                p.oval(Bounds { origin: point(x - 4.0, y - 4.0), size: size(8.0, 8.0) }, rgb(0xffe27a), rgb(0x000000), 0.0);
            }
        }
    }
    for (i, j) in JOBS.iter().enumerate() {
        let (x, y) = (ox + j.x, oy + j.y);
        let s = st[i];
        let ring = match s { State::Running(_) | State::Failed => tint(s), _ => 0x111111 };
        p.quad(Bounds { origin: point(x - 2.0, y - 2.0), size: size(NODE_W + 4.0, NODE_H + 4.0) }, 8.0, rgba(0x00000055), rgb(ring), 2.0);
        p.quad(Bounds { origin: point(x, y), size: size(NODE_W, NODE_H) }, 6.0, rgb(0x353535), rgb(0x111111), 1.0);
        p.quad(Bounds { origin: point(x, y), size: size(NODE_W, 22.0) }, 6.0, rgb(j.color), rgb(j.color), 0.0);
        p.oval(Bounds { origin: point(x + 7.0, y + 7.0), size: size(8.0, 8.0) }, rgb(tint(s)), rgb(0x000000), 1.0);
        p.text(x + 21.0, y + 4.0, j.name, 12.0, rgb(0xffffff));
        let label = match s {
            State::Queued => "queued".to_string(),
            State::Running(f) => format!("running {:.0}%", f * 100.0),
            State::Done => format!("success · {:.0}s", j.secs * 10.0),
            State::Failed => "failed (exit 1)".to_string(),
            State::Skipped => "skipped".to_string(),
        };
        p.text(x + 10.0, y + 30.0, &label, 11.0, rgb(0xbbbbbb));
        let frac = match s { State::Running(f) => f, State::Done | State::Failed => 1.0, _ => 0.0 };
        p.quad(Bounds { origin: point(x + 10.0, y + 54.0), size: size(NODE_W - 20.0, 5.0) }, 2.5, rgb(0x222222), rgb(0x222222), 0.0);
        if frac > 0.0 {
            p.quad(Bounds { origin: point(x + 10.0, y + 54.0), size: size((NODE_W - 20.0) * frac, 5.0) }, 2.5, rgb(tint(s)), rgb(tint(s)), 0.0);
        }
        if i != 0 {
            for k in 0..j.needs.len() {
                let (sx, sy) = in_sock(j, k);
                p.oval(Bounds { origin: point(sx - 5.0, sy - 5.0), size: size(10.0, 10.0) }, rgb(0xc7c729), rgb(0x000000), 1.0);
            }
        }
        if i != JOBS.len() - 1 {
            let (sx, sy) = out_sock(j);
            p.oval(Bounds { origin: point(sx - 5.0, sy - 5.0), size: size(10.0, 10.0) }, rgb(0xc7c729), rgb(0x000000), 1.0);
        }
    }
}

const CSS: &str = r"
body { margin: 0; display: flex; flex-direction: column; height: 100%; background: #1d1d1d; color: #d9d9d9; font-size: 12px; }
.bar { display: flex; align-items: center; gap: 10px; height: 30px; padding: 0 10px; background: #252525; border-bottom: 1px solid #111; }
.bar .sp { flex: 1; }
html { height: 100%; }
.graph { flex: 1 1 0; min-height: 0; }
.log { height: 150px; flex-shrink: 0; background: #161616; border-top: 1px solid #000; padding: 6px 10px; overflow: auto; font-family: monospace; font-size: 11px; color: #9a9a9a; }
.ok { color: #4caf50; } .run { color: #e8b30d; } .fail { color: #e5534b; }
";

struct Pipeline {
    started: Option<f32>,
    fail: bool,
}

impl Pipeline {
    fn elapsed(&self) -> f32 {
        self.started.map_or(-1.0, |s| (now() - s) / PACE)
    }
}

impl Render for Pipeline {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.elapsed();
        let st = if t < 0.0 { [State::Queued; JOBS.len()] } else { states(t, self.fail) };
        let finished = st.iter().all(|s| matches!(s, State::Done | State::Failed | State::Skipped));
        if self.started.is_some() && !finished {
            w.request_animation_frame();
        }
        // the log: one line per event that has happened so far
        let sch = schedule();
        let mut log = div().class("log");
        if t < 0.0 {
            log = log.child(div().child("Press ▶ Run workflow."));
        }
        for (i, j) in JOBS.iter().enumerate() {
            if st[i] == State::Skipped { continue; }
            if t >= sch[i].0 {
                log = log.child(div().class("run").child(format!("[{:>5.1}s] ▶ {}", sch[i].0 * 10.0, j.name)));
            }
            match st[i] {
                State::Done => log = log.child(div().class("ok").child(format!("[{:>5.1}s] ✔ {}", sch[i].1 * 10.0, j.name))),
                State::Failed => log = log.child(div().class("fail").child(format!("[{:>5.1}s] ✖ {} — 1 test failed on macos-14", sch[i].1 * 10.0, j.name))),
                _ => {}
            }
        }
        if finished && t >= 0.0 {
            let (c, m) = if self.fail { ("fail", "Workflow failed: downstream jobs skipped") } else { ("ok", "Workflow succeeded") };
            log = log.child(div().class(c).child(m));
        }
        html().styled()
            .child(head().child(style().child(CSS)))
            .child(body()
                .child(div().class("bar")
                    .child(b().child(".github/workflows/ci.yml"))
                    .child(div().class("sp"))
                    .child(label().child(input().input_type("checkbox").checked(self.fail)
                        .on_change(cx.listener(|this, e: &ChangeEvent, _, cx| { this.fail = e.checked; cx.notify(); })))
                        .child(" Make the test job fail"))
                    .child(button().child("▶ Run workflow").on_click(cx.listener(|this, _, _, cx| {
                        this.started = Some(now());
                        cx.notify();
                    }))))
                .child(canvas(move |b, p| paint_graph(b, p, &st, now())).class("graph"))
                .child(log))
    }
}

fn main() {
    run(1210.0, 620.0, "BUI · CI pipeline", || Pipeline { started: None, fail: false });
}

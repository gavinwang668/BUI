//! The web-platform-tests reftests of the CSS directories `tools/wpt_fetch.py` checks out, run
//! against the engine: each test and its references render off-screen at 800 x 600 and their
//! pixels are compared (within the test's `<meta name="fuzzy">`). Slow, so ignored by default:
//!
//! ```text
//! py -3 tools/wpt_fetch.py
//! cd bindings/rust && cargo test -p blenderui --test wpt -- --ignored --nocapture wpt_run
//! ```
//!
//! `WPT_FILTER=css-grid/` runs the tests whose path has that text, `WPT_JOBS=n` sets the worker
//! processes, `WPT_DIR` the checkout (default `.wpt/` at the repository root). A test listed in
//! `expectations.txt` that no longer passes fails the run; `WPT_UPDATE=1` (on a full run) rewrites
//! that list and `results.md` instead.
//!
//! Tests run in worker processes (this binary's `wpt_worker`, given a list of tests): the kit is
//! one per process, and a test that crashes or hangs the engine costs its worker only, which is
//! started again on the tests after it.

mod doc;
mod script;

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::io::{BufRead, Seek, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use blenderui::prelude::{App, Application, Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowId, WindowOptions};
use blenderui::prelude::{point, px, size};
use doc::Doc;

/// The viewport WPT's reftests are written for.
const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;

/// A worker that reports no result for this long is hung on its test.
const HANG: Duration = Duration::from_secs(60);

/// Tests a worker process runs before the next one takes over: a process that ran a thousand
/// grew to 2 GB and died on tests that pass in a fresh one.
const BATCH: usize = 150;

/// Where the test's own files are: the checked-in summary and expectations.
fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/wpt")
}

fn checkout() -> PathBuf {
    std::env::var_os("WPT_DIR").map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.wpt"), PathBuf::from)
}

// -- the worker: renders and compares -----------------------------------------------------------

/// A page, and how many times it rendered (what its scripts scrolled and focused applies in the
/// first two: focus finds its ancestors in the frame before).
struct Page(Doc, u8);

impl Render for Page {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if self.1 < 2 {
            self.1 += 1;
            for (id, x, y) in &self.0.scrolls {
                window.set_scroll_offset(id, *x, *y);
            }
            if let Some(id) = &self.0.focus {
                window.focus_id(id);
            }
            for id in &self.0.popovers {
                window.show_popover(id, true);
            }
        }
        self.0.element()
    }
}

/// The pixels of `doc` once they settle (container queries and anchors take a frame or two more),
/// and what `after` reads of its window then.
fn render<T>(cx: &mut App, doc: Doc, after: impl FnOnce(&mut App, WindowId) -> T) -> Option<(Vec<u32>, T)> {
    let opts = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.0), px(0.0)), size: size(px(WIDTH), px(HEIGHT)) })),
        ..Default::default()
    };
    let win = cx.open_window(opts, |_, cx| cx.new(|_| Page(doc, 0))).ok()?;
    let mut pixels = cx.draw(win.id).2;
    for _ in 0..3 {
        let next = cx.draw(win.id).2;
        if next == pixels {
            break;
        }
        pixels = next;
    }
    let read = after(cx, win.id);
    cx.close_window(win.id);
    Some((pixels, read))
}

/// The pixels of page `doc`: rendered as it loaded, or, when it has scripts, after they ran on
/// it (reading the layout it loaded with).
fn pixels(cx: &mut App, wpt: &Path, mut doc: Doc) -> Vec<u32> {
    if doc.scripted {
        let ids = doc.ids();
        let layout = render(cx, doc.snapshot(), |cx, win| {
            ids.iter().filter_map(|(serial, id)| {
                cx.element_bounds(win, id).map(|b| (*serial, [b.origin.x, b.origin.y, b.size.width, b.size.height]))
            }).collect::<Vec<_>>()
        });
        doc.run_scripts(wpt, &layout.map(|l| l.1).unwrap_or_default());
    }
    render(cx, doc, |_, _| ()).map(|r| r.0).unwrap_or_default()
}

/// The largest channel difference and how many pixels differ.
fn diff(a: &[u32], b: &[u32]) -> (u32, u32) {
    if a.len() != b.len() {
        return (255, u32::MAX);
    }
    a.iter().zip(b).filter(|(x, y)| x != y).fold((0, 0), |(max, n), (x, y)| {
        let d = (0..3).map(|k| ((x >> (8 * k)) & 0xff).abs_diff((y >> (8 * k)) & 0xff)).max().unwrap_or(0);
        (max.max(d), n + 1)
    })
}

/// `WPT_DUMP=dir`: a failing test's pixels and its reference's, as PNGs, to look at.
fn dump(path: &Path, pixels: &[u32]) {
    let rgb: Vec<u8> = pixels.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
    let (w, h) = (WIDTH as u32, (pixels.len() / WIDTH as usize) as u32);
    let Ok(file) = std::fs::File::create(path) else { return };
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    let _ = enc.write_header().and_then(|mut wr| wr.write_image_data(&rgb));
}

/// References rendered lately: tests of a directory share a few.
struct Refs(Vec<(PathBuf, Vec<u32>)>);

impl Refs {
    const KEPT: usize = 16;

    fn pixels(&mut self, cx: &mut App, wpt: &Path, path: &Path, fuzzy: &mut Option<doc::Fuzzy>) -> Result<&[u32], String> {
        if let Some(i) = self.0.iter().position(|(p, _)| p == path) {
            return Ok(&self.0[i].1);
        }
        let doc = Doc::load(wpt, path).map_err(|_| "skip\tmissing reference".to_owned())?;
        if let Some(why) = doc.skip {
            return Err(format!("skip\t{why}"));
        }
        *fuzzy = fuzzy.or(doc.fuzzy);
        let px = pixels(cx, wpt, doc);
        if self.0.len() == Self::KEPT {
            self.0.remove(0);
        }
        self.0.push((path.to_path_buf(), px));
        Ok(&self.0[self.0.len() - 1].1)
    }
}

/// One test's result line: `status\tdetail`.
fn run_test(cx: &mut App, refs: &mut Refs, wpt: &Path, test: &str) -> String {
    let Ok(mut doc) = Doc::load(wpt, &wpt.join(test)) else { return "skip\tunreadable".into() };
    if let Some(why) = doc.skip {
        return format!("skip\t{why}");
    }
    if doc.refs.is_empty() {
        return "skip\tno reference".into();
    }
    let references = std::mem::take(&mut doc.refs);
    let fuzzy = doc.fuzzy;
    let test_px = pixels(cx, wpt, doc);
    // the test's `match` references are alternatives (one matching is enough), and every
    // `mismatch` one must differ (WPT's reftest semantics)
    let mut want_match = None;
    for r in references {
        let mut ref_fuzzy = fuzzy;
        let ref_px = match refs.pixels(cx, wpt, &r.path, &mut ref_fuzzy) {
            Ok(px) => px,
            Err(line) => return line,
        };
        let (max, count) = diff(&test_px, ref_px);
        let same = fuzzy.or(ref_fuzzy).map_or(count == 0, |f| f.allows(max, count));
        if same == r.matches {
            if r.matches {
                want_match = Some(None);
            }
            continue;
        }
        if let Some(dir) = std::env::var_os("WPT_DUMP") {
            let name = test.replace('/', "_");
            dump(&Path::new(&dir).join(format!("{name}.png")), &test_px);
            dump(&Path::new(&dir).join(format!("{name}.ref.png")), ref_px);
        }
        if !r.matches {
            return format!("fail\tmatches mismatch: max {max} pixels {count}");
        }
        want_match.get_or_insert(Some(format!("fail\tdiffers: max {max} pixels {count}")));
    }
    want_match.flatten().unwrap_or_else(|| "pass\t".into())
}

/// A worker: runs the tests listed in `WPT_WORKER` (one per line), appending `test\tstatus\tdetail`
/// lines to `WPT_WORKER_OUT` as each finishes. Does nothing outside a run.
#[test]
#[ignore = "a worker of wpt_run"]
fn wpt_worker() {
    let (Some(list), Some(out)) = (std::env::var_os("WPT_WORKER"), std::env::var_os("WPT_WORKER_OUT")) else { return };
    let wpt = checkout();
    let tests = std::fs::read_to_string(list).expect("worker list");
    let mut out = std::fs::OpenOptions::new().create(true).append(true).open(out).expect("worker output");
    if let Ok(ahem) = std::fs::read(wpt.join("fonts/Ahem.ttf")) {
        blenderui::add_font("Ahem", false, false, &ahem);
    }
    Application::headless().run(|cx| {
        let mut refs = Refs(Vec::new());
        for test in tests.lines().filter(|l| !l.is_empty()).take(BATCH) {
            // an engine panic is the test's crash, with its message; the engine's state is then
            // unknown, so the worker stops and the run starts another on the tests after it
            let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_test(cx, &mut refs, &wpt, test)));
            let line = run.unwrap_or_else(|e| {
                let msg = e.downcast_ref::<String>().map(String::as_str).or_else(|| e.downcast_ref::<&str>().copied());
                format!("crash\tpanic: {}", msg.unwrap_or("?").replace(['\n', '\t'], " "))
            });
            writeln!(out, "{test}\t{line}").and_then(|()| out.flush()).expect("worker output");
            if line.starts_with("crash") {
                return;
            }
        }
    });
}

// -- the run: workers, results, expectations ----------------------------------------------------

/// Whether a WPT file is a reftest: it links a reference (the parser confirms it).
fn is_reftest(path: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if !matches!(ext.as_str(), "html" | "htm" | "xht" | "xhtml") || name.contains("-ref.") || name.contains("-notref.") {
        return false;
    }
    std::fs::read(path).is_ok_and(|b| {
        let s = String::from_utf8_lossy(&b).to_ascii_lowercase();
        ["rel=\"match\"", "rel='match'", "rel=match", "rel=\"mismatch\"", "rel='mismatch'", "rel=mismatch"]
            .iter()
            .any(|p| s.contains(p))
    })
}

/// The reftests under the checkout's `css/` (paths relative to the checkout, `/`-separated, sorted).
fn discover(wpt: &Path, filter: &str) -> Vec<String> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name();
            if p.is_dir() {
                if !matches!(name.to_str(), Some("support" | "reference" | "resources" | "crashtests" | "tools" | "META.yml")) {
                    walk(&p, out);
                }
            } else {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&wpt.join("css"), &mut files);
    let candidates: Vec<String> = files.iter()
        .filter_map(|p| Some(p.strip_prefix(wpt).ok()?.to_string_lossy().replace('\\', "/")))
        .filter(|t| t.contains(filter))
        .collect();
    // reading ~15,000 files is bound by the file system's latency: read them in parallel
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let mut tests: Vec<String> = std::thread::scope(|s| {
        #[allow(clippy::needless_collect)] // every thread starts before the first is joined
        let chunks: Vec<_> = candidates.chunks(candidates.len().div_ceil(threads).max(1))
            .map(|chunk| s.spawn(move || chunk.iter().filter(|t| is_reftest(&wpt.join(t))).cloned().collect::<Vec<_>>()))
            .collect();
        chunks.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    tests.sort_unstable();
    tests
}

/// A worker process and the tests it has left.
struct Worker {
    tests: Vec<String>,
    /// Results read so far.
    done: usize,
    out: PathBuf,
    read: u64,
    child: Option<Child>,
    progress: Instant,
    /// `done` when the current process started.
    started_at: usize,
}

impl Worker {
    fn spawn(&mut self, list: &Path) {
        std::fs::write(list, self.tests[self.done..].join("\n")).expect("write worker list");
        std::fs::write(&self.out, "").expect("reset worker output");
        self.read = 0;
        self.started_at = self.done;
        self.progress = Instant::now();
        let exe = std::env::current_exe().expect("test binary");
        self.child = Some(Command::new(exe)
            .args(["wpt_worker", "--exact", "--ignored", "--nocapture", "--test-threads=1"])
            .env("WPT_WORKER", list)
            .env("WPT_WORKER_OUT", &self.out)
            .env_remove("BUI_UI_SCALE")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start a worker"));
    }

    /// New result lines since the last call.
    fn poll(&mut self, results: &mut BTreeMap<String, (String, String)>) {
        let Ok(mut f) = std::fs::File::open(&self.out) else { return };
        if f.seek(std::io::SeekFrom::Start(self.read)).is_err() {
            return;
        }
        let mut reader = std::io::BufReader::new(f);
        let mut line = String::new();
        while reader.read_line(&mut line).is_ok_and(|n| n > 0) && line.ends_with('\n') {
            self.read += line.len() as u64;
            let mut parts = line.trim_end_matches('\n').splitn(3, '\t');
            if let (Some(t), Some(s), d) = (parts.next(), parts.next(), parts.next()) {
                results.insert(t.to_owned(), (s.to_owned(), d.unwrap_or("").to_owned()));
                self.done += 1;
                self.progress = Instant::now();
            }
            line.clear();
        }
    }
}

#[derive(Default)]
struct Tally {
    pass: usize,
    fail: usize,
    skip: usize,
    crash: usize,
}

impl Tally {
    fn add(&mut self, status: &str) {
        match status {
            "pass" => self.pass += 1,
            "skip" => self.skip += 1,
            "crash" | "timeout" => self.crash += 1,
            _ => self.fail += 1,
        }
    }

    /// Pass rate of the tests that ran.
    fn rate(&self) -> f64 {
        let ran = self.pass + self.fail + self.crash;
        if ran == 0 { 0.0 } else { 100.0 * self.pass as f64 / ran as f64 }
    }

    fn row(&self, name: &str) -> String {
        format!("| {name} | {} | {} | {} | {} | {:.1}% |", self.pass, self.fail, self.crash, self.skip, self.rate())
    }
}

#[test]
#[ignore = "slow: renders thousands of WPT reftests (py -3 tools/wpt_fetch.py first)"]
fn wpt_run() {
    let wpt = checkout();
    assert!(wpt.join("css").is_dir(), "no WPT checkout at {}: run `py -3 tools/wpt_fetch.py`", wpt.display());
    let filter = std::env::var("WPT_FILTER").unwrap_or_default();
    let update = std::env::var_os("WPT_UPDATE").is_some_and(|v| v == "1");
    let tests = discover(&wpt, &filter);
    let jobs = std::env::var("WPT_JOBS").ok().and_then(|j| j.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(2, |n| n.get().div_ceil(2)))
        .clamp(1, tests.len().max(1));
    println!("wpt: {} reftests, {jobs} workers", tests.len());
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("wpt");
    std::fs::create_dir_all(&tmp).expect("temp dir");

    // contiguous slices: a directory's tests share references, which a worker keeps
    let per = tests.len().div_ceil(jobs);
    let mut workers: Vec<Worker> = tests.chunks(per.max(1)).enumerate().map(|(k, chunk)| {
        let mut w = Worker { tests: chunk.to_vec(), done: 0, out: tmp.join(format!("worker{k}.out")), read: 0, child: None, progress: Instant::now(), started_at: 0 };
        w.spawn(&tmp.join(format!("worker{k}.list")));
        w
    }).collect();
    let mut results = BTreeMap::new();
    let start = Instant::now();
    let mut last_report = Instant::now();
    while workers.iter().any(|w| w.child.is_some()) {
        std::thread::sleep(Duration::from_millis(200));
        for (k, w) in workers.iter_mut().enumerate() {
            let Some(exit) = w.child.as_mut().map(|c| c.try_wait().ok().flatten()) else { continue };
            w.poll(&mut results);
            let hung = exit.is_none() && w.progress.elapsed() > HANG;
            if exit.is_none() && !hung {
                continue;
            }
            if let Some(mut child) = w.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            w.poll(&mut results);
            // a worker ends cleanly after its batch or a reported panic; one that died or hung
            // (or ended having run nothing) takes the test it was on with it
            let clean = exit.is_some_and(|e| e.success()) && w.done > w.started_at;
            if w.done < w.tests.len() && !clean {
                let status = if hung { "timeout" } else { "crash" };
                results.insert(w.tests[w.done].clone(), (status.to_owned(), String::new()));
                w.done += 1;
            }
            if w.done < w.tests.len() {
                w.spawn(&tmp.join(format!("worker{k}.list")));
            }
        }
        if last_report.elapsed() > Duration::from_secs(30) {
            last_report = Instant::now();
            println!("wpt: {}/{} in {:.0}s", results.len(), tests.len(), start.elapsed().as_secs_f32());
        }
    }

    // per directory, and why tests were skipped
    let mut dirs: BTreeMap<&str, Tally> = BTreeMap::new();
    let mut total = Tally::default();
    let mut skips: BTreeMap<&str, usize> = BTreeMap::new();
    // crashes by panic message (without the byte offsets that vary), or "died" without one
    let mut crashes: BTreeMap<String, usize> = BTreeMap::new();
    let mut all = String::new();
    for (test, (status, detail)) in &results {
        let dir = test.splitn(3, '/').take(2).last().unwrap_or(test);
        dirs.entry(dir).or_default().add(status);
        total.add(status);
        if status == "skip" {
            *skips.entry(detail).or_default() += 1;
        }
        if matches!(status.as_str(), "crash" | "timeout") {
            let why = if detail.is_empty() {
                format!("{status} (no panic)")
            } else {
                // the message without the offsets and code points that vary
                let msg = detail.split(';').next().unwrap_or(detail);
                msg.chars().fold(String::new(), |mut out, c| {
                    if !c.is_ascii_digit() {
                        out.push(c);
                    } else if !out.ends_with('N') {
                        out.push('N');
                    }
                    out
                })
            };
            *crashes.entry(why).or_default() += 1;
        }
        let _ = writeln!(all, "{test}\t{status}\t{detail}");
    }
    std::fs::write(tmp.join("results.tsv"), &all).expect("write results.tsv");
    let mut md = String::from("| Directory | Pass | Fail | Crash | Skip | Pass rate |\n|---|---:|---:|---:|---:|---:|\n");
    for (dir, t) in &dirs {
        md.push_str(&t.row(dir));
        md.push('\n');
    }
    md.push_str(&total.row("**total**"));
    md.push_str("\n\n| Skipped because | Tests |\n|---|---:|\n");
    for (why, n) in &skips {
        let _ = writeln!(md, "| {why} | {n} |");
    }
    md.push_str("\n| Crashed with | Tests |\n|---|---:|\n");
    for (why, n) in &crashes {
        let _ = writeln!(md, "| {} | {n} |", why.replace('|', "\\|"));
    }
    println!("\n{md}\nwpt: {:.0}s; every result in {}", start.elapsed().as_secs_f32(), tmp.join("results.tsv").display());

    // regressions: listed as passing, and run, but not passing now
    let expected_path = here().join("expectations.txt");
    let expected = std::fs::read_to_string(&expected_path).unwrap_or_default();
    let ran: HashSet<&str> = results.keys().map(String::as_str).collect();
    let regressed: Vec<&str> = expected.lines().filter(|t| !t.starts_with('#') && ran.contains(t))
        .filter(|t| results.get(*t).is_none_or(|(s, _)| s != "pass"))
        .collect();
    let expected: HashSet<&str> = expected.lines().collect();
    let newly = results.iter().filter(|(t, (s, _))| s == "pass" && !expected.contains(t.as_str())).count();
    println!("wpt: {} newly passing, {} regressed", newly, regressed.len());

    if update && filter.is_empty() {
        let commit = std::fs::read_to_string(wpt.join(".git/HEAD")).unwrap_or_default();
        let mut list = format!("# WPT reftests that pass (WPT {}); see main.rs\n", commit.trim());
        for (t, _) in results.iter().filter(|(_, (s, _))| s == "pass") {
            list.push_str(t);
            list.push('\n');
        }
        std::fs::write(&expected_path, list).expect("write expectations.txt");
        let summary = format!(
            "# WPT reftests\n\nThe CSS reftests of web-platform-tests {} (`tools/wpt_fetch.py`) rendered by the \
             engine at 800 x 600 and compared with their references (`tests/wpt/main.rs`). Crashes and hangs \
             count as failures; the pass rate is of the tests that ran.\n\n{md}",
            commit.trim()
        );
        std::fs::write(here().join("results.md"), summary).expect("write results.md");
        println!("wpt: wrote expectations.txt and results.md");
    } else {
        assert!(regressed.is_empty(), "WPT reftests that passed no longer do:\n{}", regressed.join("\n"));
    }
}

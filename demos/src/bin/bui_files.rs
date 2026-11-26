//! BUI Files: a file explorer built from Blender's own UI, on your real file system, which it
//! only reads. Everything that changes files (rename, new folder, delete, cut / copy / paste, drag
//! to move, duplicate, undo) works only inside its sandbox, a folder of sample files it makes in
//! the temp folder ("Sandbox" in Places); anywhere else those are refused, and the guard checks
//! every path before it touches the disk.
//!
//! Blender's screen holds three editors you can rearrange like Blender's (split, join, swap,
//! maximize, change an area's editor): Places (the sandbox, your folders, drives, bookmarks and a
//! folder tree), Files (a list or a thumbnail grid, a preview and properties sidebar on N) and
//! Operations (copies with progress bars, and the undo history).
//!
//! Mouse: click selects, Ctrl+click toggles, Shift+click a range, drag on empty space draws a
//! selection box, drag files onto a folder (in the list or Places) to move them, double-click
//! opens. Keys: arrows move (Shift extends), Enter opens, Backspace goes up, Alt+Left / Alt+Right
//! history, Ctrl+A all, typing jumps to a name, F2 renames, Delete trashes, Ctrl+C / X / V,
//! Ctrl+D duplicates, Ctrl+Shift+N a new folder, Ctrl+Z / Ctrl+Shift+Z undo / redo, Ctrl+L the
//! path, Ctrl+B bookmarks the folder, Ctrl+H hidden files, Ctrl+1 / Ctrl+2 list or grid. The
//! filter is Blender's own text field, live (it filters as you type). "Blender File Browser…" opens Blender's own File Browser in its window, to jump.
//!
//! `cargo run --release --bin bui_files`; `-- --storyboard DIR` drives it headlessly (in a fresh
//! sandbox).

use std::collections::{BTreeSet, HashMap};
use std::ffi::{c_char, c_int, CStr, CString};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use blenderui::prelude::*;
use blenderui::screen::{AreaRef, AreaSplit, Editor, Editors, Region, Screen};
use bui_demos::story::{story, Story};
use bui_demos::text::{edit, Edit};
use bui_demos::{arg, focus_root, now, run_with, size_arg, Script};
use bui_demos::act;

const CSS: &str = r"
html, body { height: 100%; }
body { display: flex; flex-direction: column; font-size: 13px; color: #e6e6e6; -webkit-font-smoothing: antialiased; user-select: none; }
.top { height: 30px; flex-shrink: 0; display: flex; align-items: center; gap: 10px; padding: 0 10px; background: #232323;
       border-bottom: 1px solid #161616; }
.brand { font-weight: 700; color: #f0f0f0; }
.sandbox-chip { padding: 2px 8px; border-radius: 9px; background: #3d4a2d; color: #cfe8a8; font-size: 11px; }
.ro-chip { padding: 2px 8px; border-radius: 9px; background: #3a3a3a; color: #b8b8b8; font-size: 11px; }
.spacer { flex: 1; }
.screen { flex: 1; min-height: 0; }
.status { height: 22px; flex-shrink: 0; display: flex; align-items: center; gap: 14px; padding: 0 10px; background: #232323;
          border-top: 1px solid #161616; color: #9a9a9a; font-size: 11px; }
.status b { color: #d6d6d6; font-weight: 600; font-variant-numeric: tabular-nums; }
.msg { color: #e8c070; }
.msg.ok { color: #9ad27a; }

/* regions */
.hdr { height: 100%; display: flex; align-items: center; gap: 4px; padding: 0 6px; background: #2b2b2b; overflow: hidden; }
.hdr .sep { width: 1px; height: 16px; background: #444; margin: 0 3px; flex-shrink: 0; }
.hdr label { display: flex; align-items: center; gap: 4px; margin: 0 4px; white-space: nowrap; }
.side { height: 100%; overflow-y: auto; background: #2b2b2b; padding: 4px; box-sizing: border-box; }
.side details { margin-bottom: 4px; }
.prop { display: flex; gap: 8px; margin: 3px 6px; font-variant-numeric: tabular-nums; }
.prop span:first-child { width: 70px; flex-shrink: 0; color: #9a9a9a; }
.prop span + span { min-width: 0; overflow-wrap: anywhere; }
.preview { margin: 6px; border-radius: 4px; background: #1f1f1f; display: flex; align-items: center; justify-content: center;
           min-height: 120px; overflow: hidden; }
.preview img { max-width: 100%; }
.preview pre { margin: 0; padding: 8px; width: 100%; box-sizing: border-box; font-size: 11px; color: #cfcfcf; white-space: pre-wrap; max-height: 260px; overflow: hidden; }
.footer { height: 100%; display: flex; align-items: center; gap: 12px; padding: 0 8px; background: #2b2b2b; color: #a8a8a8; font-size: 11px;
          font-variant-numeric: tabular-nums; }

/* fields of our own (keys go where we say: see `Field`) */
.field { display: flex; align-items: center; height: 20px; padding: 0 6px; border-radius: 4px; background: #1d1d1d; color: #e6e6e6;
         box-shadow: inset 0 0 0 1px #3d3d3d; cursor: text; min-width: 0; overflow: hidden; white-space: nowrap; }
.field.on { box-shadow: inset 0 0 0 1px var(--accent); background: #161616; }
.caret { width: 1px; height: 14px; background: #fff; margin-left: 1px; animation: blink 1s steps(1) infinite; }
/* the path takes the free room, so the controls after it never move (and the deepest folder shows) */
.where { flex: 1; min-width: 0; display: flex; overflow: hidden; }
.crumbs { flex: 1; display: flex; align-items: center; justify-content: flex-end; min-width: 0; overflow: hidden; }
.crumb { padding: 2px 5px; border-radius: 3px; color: #cfcfcf; white-space: nowrap; cursor: pointer;
         transition-property: background-color; transition-duration: 100ms; }
.crumb:hover { background: #3d3d3d; color: #fff; }
.crumb.drop { background: var(--accent); color: #fff; }
.slash { color: #666; }

/* places */
.places { height: 100%; overflow-y: auto; background: #303030; padding: 6px 4px; box-sizing: border-box; }
.place { display: flex; align-items: center; gap: 6px; padding: 3px 8px; border-radius: 4px; cursor: pointer; white-space: nowrap;
         transition-property: background-color; transition-duration: 100ms; }
.place:hover { background: #3d3d3d; }
.place.on { background: #4a4a4a; }
.place.drop { background: var(--accent); }
.place .n { overflow: hidden; text-overflow: ellipsis; }
.tree-tw { width: 12px; flex-shrink: 0; color: #8a8a8a; text-align: center; }

/* the file list */
.list { height: 100%; overflow-y: auto; background: #303030; position: relative; box-sizing: border-box; }
table.rows { width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }
.rows th { position: sticky; top: 0; z-index: 1; background: #2b2b2b; text-align: left; font-weight: 400; color: #a8a8a8; padding: 4px 8px;
           cursor: pointer; white-space: nowrap; }
.rows th:hover { color: #fff; }
.rows th.num, .rows td.num { text-align: right; }
.rows td { padding: 3px 8px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 320px; }
.rows tr.it { transition-property: background-color; transition-duration: 80ms; }
.rows tr.it:hover { background: #3a3a3a; }
.rows tr.it:nth-child(even) { background: #333; }
.rows tr.it.sel, .rows tr.it.sel:hover { background: #33496b; }
.rows tr.it.cur td:first-child { border-left: 2px solid var(--accent); padding-left: 6px; }
.rows tr.it.drop { background: var(--accent); }
.rows tr.it.cut { opacity: 0.5; }
.name { display: flex; align-items: center; gap: 6px; }
.dim { color: #8a8a8a; }
.grid { display: flex; flex-wrap: wrap; gap: 6px; padding: 8px; align-content: flex-start; }
.cell { display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 6px; border-radius: 6px; box-sizing: border-box;
        transition-property: background-color; transition-duration: 80ms; }
.cell:hover { background: #3a3a3a; }
.cell.sel, .cell.sel:hover { background: #33496b; }
.cell.cur { box-shadow: inset 0 0 0 1px var(--accent); }
.cell.drop { background: var(--accent); }
.cell.cut { opacity: 0.5; }
.cell .thumb { display: flex; align-items: center; justify-content: center; overflow: hidden; border-radius: 3px; }
.cell .thumb img { outline: 1px solid rgba(255,255,255,0.1); }
.cell .cap { width: 100%; text-align: center; font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.empty { color: #8a8a8a; text-align: center; margin-top: 40px; }
.more { color: #8a8a8a; padding: 8px; text-align: center; }
.band { position: absolute; border: 1px solid var(--accent); background: rgba(71,114,179,0.18); pointer-events: none; }
.ghost { position: absolute; z-index: 10; padding: 3px 8px; border-radius: 4px; background: #222; box-shadow: 0 4px 14px rgba(0,0,0,0.5);
         pointer-events: none; font-size: 11px; white-space: nowrap; }

/* icons: drawn, a folder or a page with its kind's colour */
.ic { position: relative; flex-shrink: 0; }
.ic.folder { border-radius: 2px 3px 3px 3px; background: #d6a64e; }
.ic.folder::before { content: ''; position: absolute; left: 0; top: -2px; width: 45%; height: 3px; border-radius: 2px 2px 0 0; background: #d6a64e; }
.ic.file { border-radius: 2px; background: #cfcfcf; }
.ic.file::after { content: ''; position: absolute; right: 0; top: 0; width: 30%; height: 30%; background: #8f8f8f; border-radius: 0 0 0 2px; }
.ic.k-image { background: #6fb3e0; } .ic.k-text { background: #dedede; } .ic.k-blend { background: #e87d0d; }
.ic.k-audio { background: #b07ee0; } .ic.k-archive { background: #b0a060; } .ic.k-data { background: #8ad08a; }

/* operations */
.ops { height: 100%; overflow-y: auto; background: #303030; padding: 8px; box-sizing: border-box; }
.job { margin-bottom: 8px; }
.job .what { display: flex; justify-content: space-between; font-variant-numeric: tabular-nums; margin-bottom: 3px; }
.job progress { width: 100%; }
.hist { display: flex; gap: 6px; padding: 2px 4px; font-variant-numeric: tabular-nums; }
.hist.undone { opacity: 0.45; text-decoration: line-through; }
.hist .k { color: #9a9a9a; width: 64px; flex-shrink: 0; }
.log { color: #a8a8a8; font-size: 11px; margin: 2px 4px; }
.log.bad { color: #e8a070; }
.log.fresh { animation: logIn 240ms cubic-bezier(0.2, 0, 0, 1); }
h4 { margin: 10px 4px 4px; color: #b8b8b8; font-weight: 600; font-size: 11px; text-transform: uppercase; letter-spacing: 0.6px; }

@keyframes blink { 50% { opacity: 0; } }
@keyframes logIn { from { opacity: 0; transform: translateY(-4px); } to { opacity: 1; transform: translateY(0); } }
";

/// The most entries a folder is read to, and drawn (a narrower filter shows the rest).
const READ_MAX: usize = 20_000;
const DRAW_MAX: usize = 600;
/// Bytes a copy moves per frame: a big file's progress bar has something to show.
const COPY_STEP: usize = 192 * 1024;

// ---------------------------------------------------------------------------------------------
// The sandbox: the one place this app writes

fn sandbox_path() -> PathBuf {
    std::env::temp_dir().join("bui_files_demo")
}

/// A sample PNG: a procedural pattern `kind` (bricks, checker, noise, gradient, render).
fn sample_png(path: &Path, kind: u32, seed: u32) -> std::io::Result<()> {
    let (w, h) = (192u32, 128u32);
    let mut px = Vec::with_capacity((w * h * 3) as usize);
    let hash = |x: u32, y: u32| ((x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263) ^ seed.wrapping_mul(2_654_435_761)) >> 13) & 255;
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = match kind {
                0 => {
                    let row = y / 16;
                    let off = if row % 2 == 0 { 0 } else { 16 };
                    let mortar = y % 16 < 2 || (x + off) % 32 < 2;
                    if mortar { (200, 196, 186) } else { (150 + hash(x / 32, row) % 40, 70, 52) }
                }
                1 => if (x / 16 + y / 16) % 2 == 0 { (230, 230, 230) } else { (40, 40, 40) },
                2 => {
                    let v = hash(x / 3, y / 3);
                    (v, v, v)
                }
                3 => (x * 255 / w, y * 255 / h, 200),
                _ => {
                    // a "render": a lit sphere on a floor
                    let (cx, cy, r) = (96.0 + seed as f32 * 6.0, 58.0, 34.0);
                    let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < r {
                        let l = (1.0 - (dx + dy * 1.4) / (r * 2.6)).clamp(0.2, 1.0);
                        ((230.0 * l) as u32, (120.0 * l) as u32, (40.0 * l) as u32)
                    } else if y > 86 {
                        (60, 64, 72)
                    } else {
                        (28 + y / 4, 32 + y / 4, 44 + y / 3)
                    }
                }
            };
            px.extend_from_slice(&[r as u8, g as u8, b as u8]);
        }
    }
    let file = fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().map_err(std::io::Error::other)?.write_image_data(&px).map_err(std::io::Error::other)
}

/// Makes the sandbox and its sample files (`fresh`: from nothing, for a storyboard).
fn seed_sandbox(fresh: bool) -> std::io::Result<PathBuf> {
    let root = sandbox_path();
    if fresh && root.exists() && root == sandbox_path() {
        fs::remove_dir_all(&root)?;
    }
    if root.exists() {
        return Ok(root);
    }
    for d in ["Projects/Spaceship/renders", "Projects/Lighthouse", "Textures", "Documents", "Audio", ".trash"] {
        fs::create_dir_all(root.join(d))?;
    }
    let text = |p: &str, s: &str| fs::write(root.join(p), s);
    text("Documents/readme.txt", "This is the BUI Files sandbox.\n\nEverything here may be renamed, moved, copied or deleted:\nthe app writes nowhere else.\n")?;
    text("Documents/todo.md", "# To do\n\n- [x] model the hull\n- [ ] UV the wings\n- [ ] bake the AO\n- [ ] render the turntable\n")?;
    text("Documents/budget.csv", "item,cost\nrender farm,120\nHDRIs,35\ncoffee,48\n")?;
    text("Projects/Spaceship/notes.md", "Spaceship\n=========\n\nScale: 1 unit = 1 m. Cycles, 256 samples, denoised.\n")?;
    text("Projects/Lighthouse/scene.txt", "camera 35mm, f/2.8\nsun 4.5 W/m², 32° elevation\n")?;
    text(".hidden_settings", "theme = dark\n")?;
    fs::write(root.join("Projects/Spaceship/spaceship.blend"), b"BLENDER-v400 (a placeholder, not a real .blend)")?;
    fs::write(root.join("Projects/Lighthouse/lighthouse.blend"), b"BLENDER-v400 (a placeholder)")?;
    fs::write(root.join("Audio/hum.wav"), b"RIFF\0\0\0\0WAVE")?;
    fs::write(root.join("Textures/textures.zip"), vec![0u8; 4096])?;
    // a big one, so a copy has progress to show
    fs::write(root.join("Projects/Spaceship/renders/cache.bin"), vec![7u8; 6 * 1024 * 1024])?;
    for (name, kind) in [("bricks.png", 0), ("checker.png", 1), ("noise.png", 2), ("gradient.png", 3)] {
        sample_png(&root.join("Textures").join(name), kind, 1)?;
    }
    for k in 1..=4u32 {
        sample_png(&root.join(format!("Projects/Spaceship/renders/frame_{k:04}.png")), 4, k)?;
    }
    Ok(root)
}

// ---------------------------------------------------------------------------------------------
// The model

#[derive(Clone)]
struct Entry {
    name: String,
    path: PathBuf,
    dir: bool,
    size: u64,
    modified: Option<SystemTime>,
    hidden: bool,
}

impl Entry {
    fn ext(&self) -> String {
        if self.dir { String::new() } else { Path::new(&self.name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default() }
    }

    fn kind(&self) -> &'static str {
        if self.dir {
            return "folder";
        }
        match self.ext().as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tga" | "tif" | "tiff" | "hdr" | "exr" | "svg" => "image",
            "txt" | "md" | "rs" | "py" | "toml" | "json" | "cfg" | "ini" | "log" | "c" | "h" | "js" | "ts" | "html" | "css" => "text",
            "csv" | "tsv" => "data",
            "blend" | "blend1" => "blend",
            "wav" | "mp3" | "flac" | "ogg" | "m4a" => "audio",
            "zip" | "7z" | "tar" | "gz" | "rar" => "archive",
            _ => "file",
        }
    }

    fn is_image(&self) -> bool {
        self.kind() == "image" && self.ext() != "exr"
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Name,
    Size,
    Kind,
    Modified,
}

const SORTS: [(Sort, &str); 4] = [(Sort::Name, "Name"), (Sort::Size, "Size"), (Sort::Kind, "Type"), (Sort::Modified, "Modified")];

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    List,
    Grid,
}

/// One change on disk, with what undoes it.
#[derive(Clone)]
enum Op {
    Rename { from: PathBuf, to: PathBuf },
    Mkdir(PathBuf),
    /// Moved to the sandbox's trash (undo puts it back).
    Trash { from: PathBuf, to: PathBuf },
    Move { from: PathBuf, to: PathBuf },
    /// A copy made (undo deletes it, to the trash).
    Made(PathBuf),
}

/// What the user did: one undo step of one or more ops.
#[derive(Clone)]
struct Step {
    what: String,
    ops: Vec<Op>,
    undone: bool,
}

/// A copy under way: files still to go, a frame's worth at a time.
struct Job {
    label: String,
    files: Vec<(PathBuf, PathBuf, u64)>,
    total: u64,
    done: u64,
    open: Option<(fs::File, fs::File)>,
    made: Vec<PathBuf>,
    /// Where a frame's bytes pass through (made once for the job).
    buf: Box<[u8]>,
}

#[derive(Clone, PartialEq, Eq)]
enum Field {
    None,
    Path,
    Rename(PathBuf),
}

#[derive(Clone, Copy)]
struct Band {
    from: (f32, f32),
    to: (f32, f32),
    /// The selection before the box, which Ctrl adds to.
    add: bool,
}

#[derive(Clone)]
struct Drag {
    paths: Vec<PathBuf>,
    from: (f32, f32),
    at: (f32, f32),
    live: bool,
}

#[derive(Clone)]
struct Line {
    text: String,
    bad: bool,
    at: f32,
}

/// Something the pointer can land on, by its element id: an item of area `item`'s list, or
/// a folder (in the list or in Places) to drop on.
struct Target {
    id: String,
    path: PathBuf,
    item: Option<u32>,
    folder: bool,
}

/// A [`Target`] where the last frame placed it.
struct Hit {
    at: Bounds<f32>,
    path: PathBuf,
    item: Option<u32>,
    folder: bool,
}

struct Files {
    root: PathBuf,
    /// The sandbox as the file system resolves it: what every write is checked against.
    canon: PathBuf,
    dir: PathBuf,
    entries: Vec<Entry>,
    error: Option<String>,
    back: Vec<PathBuf>,
    fwd: Vec<PathBuf>,
    sel: BTreeSet<PathBuf>,
    cursor: Option<PathBuf>,
    anchor: Option<PathBuf>,
    view: View,
    /// How many times the view changed: the radios' ids (a clicked control keeps its own state
    /// over `.checked()`, as an HTML input does, so a changed view makes them anew).
    views: u32,
    sort: Sort,
    desc: bool,
    hidden: bool,
    thumb: f32,
    filter: String,
    field: Field,
    draft: String,
    bookmarks: Vec<PathBuf>,
    open_tree: BTreeSet<PathBuf>,
    tree_kids: HashMap<PathBuf, Vec<PathBuf>>,
    clip: Option<(Vec<PathBuf>, bool)>,
    steps: Vec<Step>,
    job: Option<Job>,
    log: Vec<Line>,
    typed: (String, f32),
    band: Option<Band>,
    drag: Option<Drag>,
    /// The folder under a drag (a drop target), by its element id.
    drop: Option<PathBuf>,
    /// A text file's first bytes, for the preview.
    peek: Option<(PathBuf, String)>,
    /// What the pointer can land on, by element id: this frame's items and folders.
    targets: Vec<Target>,
    /// The same, placed: where the last frame put them (a listener can't ask, so render reads it).
    hits: Vec<Hit>,
    /// Columns of the grid at the last frame (arrow keys move by a row).
    cols: usize,
    headless: bool,
    frames: u32,
}

impl Files {
    fn new(headless: bool) -> Self {
        let root = seed_sandbox(headless).unwrap_or_else(|_| sandbox_path());
        let canon = root.canonicalize().unwrap_or_else(|_| root.clone());
        let mut f = Self {
            root: root.clone(),
            canon,
            dir: root.clone(),
            entries: Vec::new(),
            error: None,
            back: Vec::new(),
            fwd: Vec::new(),
            sel: BTreeSet::new(),
            cursor: None,
            anchor: None,
            view: View::List,
            views: 0,
            sort: Sort::Name,
            desc: false,
            hidden: false,
            thumb: 96.0,
            filter: String::new(),
            field: Field::None,
            draft: String::new(),
            bookmarks: vec![root.join("Textures")],
            open_tree: BTreeSet::new(),
            tree_kids: HashMap::new(),
            clip: None,
            steps: Vec::new(),
            job: None,
            log: Vec::new(),
            typed: (String::new(), -10.0),
            band: None,
            drag: None,
            drop: None,
            peek: None,
            targets: Vec::new(),
            hits: Vec::new(),
            cols: 6,
            headless,
            frames: 0,
        };
        f.open_tree.insert(root);
        f.read();
        f
    }

    // -- reading --------------------------------------------------------------------------------

    /// Reads the folder once (on arriving or after a change), never while drawing.
    fn read(&mut self) {
        self.entries.clear();
        self.error = None;
        match fs::read_dir(&self.dir) {
            Ok(rd) => {
                for e in rd.flatten().take(READ_MAX) {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let meta = e.metadata().ok();
                    let dir = meta.as_ref().is_some_and(fs::Metadata::is_dir);
                    let hidden = name.starts_with('.') || is_hidden_attr(meta.as_ref());
                    self.entries.push(Entry { path: e.path(), dir, size: meta.as_ref().map_or(0, fs::Metadata::len),
                                              modified: meta.as_ref().and_then(|m| m.modified().ok()), hidden, name });
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.sel.retain(|p| p.exists());
        self.tree_kids.remove(&self.dir);
    }

    /// The entries shown: hidden ones only when asked, the filter, sorted (folders first).
    fn visible(&self) -> Vec<&Entry> {
        let f = self.filter.to_lowercase();
        let mut v: Vec<&Entry> = self.entries.iter()
            .filter(|e| (self.hidden || !e.hidden) && (f.is_empty() || e.name.to_lowercase().contains(&f)))
            .collect();
        v.sort_by(|a, b| {
            let o = match self.sort {
                Sort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                Sort::Size => a.size.cmp(&b.size),
                Sort::Kind => a.kind().cmp(b.kind()).then_with(|| a.ext().cmp(&b.ext())),
                Sort::Modified => a.modified.cmp(&b.modified),
            };
            b.dir.cmp(&a.dir).then(if self.desc { o.reverse() } else { o })
        });
        v
    }

    fn tree_children(&mut self, dir: &Path) -> Vec<PathBuf> {
        if let Some(k) = self.tree_kids.get(dir) {
            return k.clone();
        }
        let mut kids: Vec<PathBuf> = fs::read_dir(dir).map(|rd| rd.flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter(|e| self.hidden || !e.file_name().to_string_lossy().starts_with('.'))
            .take(200).map(|e| e.path()).collect()).unwrap_or_default();
        kids.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        self.tree_kids.insert(dir.to_path_buf(), kids.clone());
        kids
    }

    // -- going places ---------------------------------------------------------------------------

    fn go(&mut self, dir: PathBuf) {
        if dir == self.dir || !dir.is_dir() {
            if !dir.is_dir() {
                self.say(format!("Not a folder: {}", dir.display()), true);
            }
            return;
        }
        let from = std::mem::replace(&mut self.dir, dir);
        self.back.push(from);
        self.fwd.clear();
        self.arrive();
    }

    fn arrive(&mut self) {
        self.typed.0.clear();
        self.sel.clear();
        self.cursor = None;
        self.anchor = None;
        self.filter.clear();
        self.read();
        // the tree shows the way here
        let mut p = self.dir.clone();
        while let Some(parent) = p.parent().map(Path::to_path_buf) {
            self.open_tree.insert(parent.clone());
            p = parent;
        }
    }

    fn history(&mut self, forward: bool) {
        let (from, to) = if forward { (&mut self.fwd, &mut self.back) } else { (&mut self.back, &mut self.fwd) };
        if let Some(d) = from.pop() {
            to.push(std::mem::replace(&mut self.dir, d));
            self.arrive();
        }
    }

    fn up(&mut self) {
        if let Some(p) = self.dir.parent().map(Path::to_path_buf) {
            let here = self.dir.clone();
            self.go(p);
            self.sel.insert(here.clone());
            self.cursor = Some(here);
        }
    }

    fn open(&mut self, path: &Path) {
        if path.is_dir() {
            self.go(path.to_path_buf());
        } else {
            self.say(format!("{}: shown in the preview (opening files is up to you)", name_of(path)), false);
        }
    }

    // -- the guard ------------------------------------------------------------------------------

    /// Whether `p` is inside the sandbox (what it would be, for a path that doesn't exist yet).
    fn writable(&self, p: &Path) -> bool {
        let real = p.canonicalize().ok().or_else(|| p.parent().and_then(|d| d.canonicalize().ok()).map(|d| d.join(p.file_name().unwrap_or_default())));
        real.is_some_and(|r| r.starts_with(&self.canon) && r != self.canon)
    }

    /// Whether `p` is the sandbox itself (things go into it; it is never changed itself).
    fn is_root(&self, p: &Path) -> bool {
        p.canonicalize().is_ok_and(|c| c == self.canon)
    }

    fn here_writable(&self) -> bool {
        self.dir.canonicalize().is_ok_and(|d| d.starts_with(&self.canon))
    }

    fn refuse(&mut self) {
        self.say(String::from("Read-only here: changes are made only in the Sandbox."), true);
    }

    fn say(&mut self, text: String, bad: bool) {
        self.log.push(Line { text, bad, at: now() });
        if self.log.len() > 60 {
            self.log.remove(0);
        }
    }

    // -- changes (sandbox only) -----------------------------------------------------------------

    /// Does `op` (forward) on the disk, both its paths checked first.
    fn apply(&self, op: &Op) -> std::io::Result<()> {
        let guard = |p: &Path| if self.writable(p) { Ok(()) } else { Err(std::io::Error::other("outside the sandbox")) };
        match op {
            Op::Rename { from, to } | Op::Trash { from, to } | Op::Move { from, to } => {
                guard(from)?;
                guard(to)?;
                if to.exists() {
                    return Err(std::io::Error::other(format!("{} exists", name_of(to))));
                }
                fs::rename(from, to)
            }
            Op::Mkdir(p) => {
                guard(p)?;
                fs::create_dir(p)
            }
            Op::Made(_) => Ok(()),
        }
    }

    /// The op that undoes `op`.
    fn inverse(&self, op: &Op) -> Op {
        match op {
            Op::Rename { from, to } => Op::Rename { from: to.clone(), to: from.clone() },
            Op::Move { from, to } => Op::Move { from: to.clone(), to: from.clone() },
            Op::Trash { from, to } => Op::Trash { from: to.clone(), to: from.clone() },
            Op::Mkdir(p) | Op::Made(p) => Op::Trash { from: p.clone(), to: self.trash_slot(p) },
        }
    }

    fn trash_slot(&self, p: &Path) -> PathBuf {
        let trash = self.root.join(".trash");
        let _ = fs::create_dir_all(&trash);
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        trash.join(format!("{stamp}_{}", name_of(p)))
    }

    /// Runs `ops` as one undo step named `what`; stops at the first refusal.
    fn change(&mut self, what: String, ops: Vec<Op>) {
        let mut done = Vec::new();
        for op in ops {
            match self.apply(&op) {
                Ok(()) => done.push(op),
                Err(e) => {
                    self.say(format!("{what}: {e}"), true);
                    break;
                }
            }
        }
        if !done.is_empty() {
            self.say(format!("{what} ({} item{})", done.len(), if done.len() == 1 { "" } else { "s" }), false);
            self.steps.retain(|s| !s.undone);
            self.steps.push(Step { what, ops: done, undone: false });
        }
        self.read();
    }

    fn undo(&mut self, redo: bool) {
        let Some(k) = (if redo { self.steps.iter().position(|s| s.undone) } else { self.steps.iter().rposition(|s| !s.undone) }) else {
            self.say(String::from(if redo { "Nothing to redo" } else { "Nothing to undo" }), true);
            return;
        };
        let step = self.steps[k].clone();
        let ops: Vec<Op> = if redo { step.ops.clone() } else { step.ops.iter().rev().map(|o| self.inverse(o)).collect() };
        // a step redone after its copies were undone makes them again: copies are redone by copying
        let mut ok = true;
        for op in &ops {
            if let Err(e) = self.apply(op) {
                self.say(format!("{}: {e}", step.what), true);
                ok = false;
                break;
            }
        }
        if ok {
            if !redo {
                // what the undo made (trash slots) is what a redo must take back
                let fixed: Vec<Op> = step.ops.iter().zip(ops.iter().rev()).map(|(o, inv)| match (o, inv) {
                    (Op::Mkdir(_) | Op::Made(_), Op::Trash { from, to }) => Op::Trash { from: to.clone(), to: from.clone() },
                    _ => o.clone(),
                }).collect();
                self.steps[k].ops = fixed;
            }
            self.steps[k].undone = !redo;
            self.say(format!("{} {}", if redo { "Redo" } else { "Undo" }, step.what), false);
        }
        self.read();
    }

    fn targets_or_refuse(&mut self) -> Option<Vec<PathBuf>> {
        if self.sel.is_empty() {
            self.say(String::from("Nothing selected"), true);
            return None;
        }
        if !self.sel.iter().all(|p| self.writable(p)) {
            self.refuse();
            return None;
        }
        Some(self.sel.iter().cloned().collect())
    }

    fn trash(&mut self) {
        let Some(paths) = self.targets_or_refuse() else { return };
        let ops = paths.iter().map(|p| Op::Trash { from: p.clone(), to: self.trash_slot(p) }).collect();
        self.change(String::from("Delete"), ops);
    }

    fn new_folder(&mut self) {
        if !self.here_writable() {
            return self.refuse();
        }
        let mut k = 1;
        let mut p = self.dir.join("New Folder");
        while p.exists() {
            k += 1;
            p = self.dir.join(format!("New Folder {k}"));
        }
        self.change(String::from("New Folder"), vec![Op::Mkdir(p.clone())]);
        self.sel = BTreeSet::from([p.clone()]);
        self.cursor = Some(p.clone());
        self.begin_rename(p);
    }

    fn begin_rename(&mut self, p: PathBuf) {
        if !self.writable(&p) {
            return self.refuse();
        }
        self.draft = name_of(&p);
        self.field = Field::Rename(p);
    }

    fn finish_rename(&mut self, p: &Path) {
        let name = self.draft.trim().to_owned();
        if name.is_empty() || name.contains(['/', '\\']) || name == name_of(p) {
            return;
        }
        let to = p.with_file_name(&name);
        self.change(String::from("Rename"), vec![Op::Rename { from: p.to_path_buf(), to: to.clone() }]);
        self.sel = BTreeSet::from([to.clone()]);
        self.cursor = Some(to);
    }

    fn move_into(&mut self, paths: &[PathBuf], dir: &Path) {
        if !self.writable(dir) && !self.is_root(dir) {
            return self.refuse();
        }
        let ops: Vec<Op> = paths.iter().filter(|p| p.parent() != Some(dir) && !dir.starts_with(p))
            .map(|p| Op::Move { from: p.clone(), to: dir.join(name_of(p)) }).collect();
        if ops.is_empty() {
            return;
        }
        if !paths.iter().all(|p| self.writable(p)) {
            return self.refuse();
        }
        self.change(format!("Move to {}", name_of(dir)), ops);
    }

    /// Copies `paths` into `dir`: a job a frame's worth at a time (names made unique).
    fn copy_into(&mut self, paths: &[PathBuf], dir: &Path, label: &str) {
        if !(self.writable(dir) || self.is_root(dir)) {
            return self.refuse();
        }
        let mut files = Vec::new();
        let mut made = Vec::new();
        for p in paths {
            let mut to = dir.join(name_of(p));
            let mut k = 1;
            while to.exists() {
                k += 1;
                let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                to = dir.join(format!("{stem} {k}{ext}"));
            }
            made.push(to.clone());
            collect_copy(p, &to, &mut files);
        }
        // folders are made at once; files fill in over frames
        for (_, to, size) in &files {
            if *size == u64::MAX {
                let _ = fs::create_dir_all(to);
            }
        }
        files.retain(|f| f.2 != u64::MAX);
        let total = files.iter().map(|f| f.2).sum();
        self.job = Some(Job { label: label.to_owned(), files, total, done: 0, open: None, made, buf: vec![0; 64 * 1024].into_boxed_slice() });
    }

    /// The copy moves on by one frame's worth.
    fn step_job(&mut self) {
        let Some(mut job) = self.job.take() else { return };
        let mut budget = COPY_STEP;
        while budget > 0 {
            if job.open.is_none() {
                let Some((from, to, _)) = job.files.first() else { break };
                if !self.writable(to) {
                    job.files.clear();
                    break;
                }
                if let (Ok(a), Ok(b)) = (fs::File::open(from), fs::File::create(to)) {
                    job.open = Some((a, b));
                } else {
                    job.files.remove(0);
                    continue;
                }
            }
            let Some((src, dst)) = &mut job.open else { break };
            let want = budget.min(job.buf.len());
            match src.read(&mut job.buf[..want]) {
                Ok(0) | Err(_) => {
                    job.open = None;
                    job.files.remove(0);
                }
                Ok(n) => {
                    let _ = dst.write_all(&job.buf[..n]);
                    job.done += n as u64;
                    budget = budget.saturating_sub(n);
                }
            }
        }
        if !job.files.is_empty() {
            self.job = Some(job);
            return;
        }
        let ops = job.made.iter().map(|p| Op::Made(p.clone())).collect();
        self.steps.retain(|s| !s.undone);
        self.steps.push(Step { what: job.label.clone(), ops, undone: false });
        self.say(format!("{} ({} item{}, {})", job.label, job.made.len(), if job.made.len() == 1 { "" } else { "s" }, size_text(job.total)), false);
        self.read();
    }

    fn paste(&mut self) {
        let Some((paths, cut)) = self.clip.clone() else {
            return self.say(String::from("The clipboard is empty"), true);
        };
        let dir = self.dir.clone();
        if cut {
            self.move_into(&paths, &dir);
            self.clip = None;
        } else {
            self.copy_into(&paths, &dir, "Paste");
        }
    }

    fn duplicate(&mut self) {
        let Some(paths) = self.targets_or_refuse() else { return };
        let dir = self.dir.clone();
        self.copy_into(&paths, &dir, "Duplicate");
    }

    // -- selection ------------------------------------------------------------------------------

    fn click(&mut self, path: &Path, ctrl: bool, shift: bool) {
        let vis: Vec<PathBuf> = self.visible().iter().map(|e| e.path.clone()).collect();
        if shift && let Some(a) = self.anchor.clone() {
            let (i, j) = (vis.iter().position(|p| *p == a), vis.iter().position(|p| p == path));
            if let (Some(i), Some(j)) = (i, j) {
                if !ctrl {
                    self.sel.clear();
                }
                self.sel.extend(vis[i.min(j)..=i.max(j)].iter().cloned());
            }
        } else if ctrl {
            if !self.sel.remove(path) {
                self.sel.insert(path.to_path_buf());
            }
            self.anchor = Some(path.to_path_buf());
        } else {
            self.sel = BTreeSet::from([path.to_path_buf()]);
            self.anchor = Some(path.to_path_buf());
        }
        self.cursor = Some(path.to_path_buf());
        self.peek_text();
    }

    fn set_view(&mut self, view: View) {
        if self.view != view {
            self.view = view;
            self.views += 1;
        }
    }

    /// Arrow keys: by one, or by a row of the grid.
    fn step(&mut self, by: isize, extend: bool) {
        let p = {
            let vis = self.visible();
            let Some(last) = vis.len().checked_sub(1) else { return };
            let at = self.cursor.as_ref().and_then(|c| vis.iter().position(|e| e.path == *c));
            vis[at.map_or(0, |i| i.saturating_add_signed(by).min(last))].path.clone()
        };
        if extend {
            self.click(&p, false, true);
            self.cursor = Some(p);
        } else {
            self.click(&p, false, false);
        }
    }

    /// Typing jumps to the first name that starts with what was typed (within a second).
    fn type_ahead(&mut self, ch: &str) {
        let t = now();
        if t - self.typed.1 > 1.0 {
            self.typed.0.clear();
        }
        self.typed = (format!("{}{}", self.typed.0, ch.to_lowercase()), t);
        let hit = self.visible().iter().find(|e| e.name.to_lowercase().starts_with(&self.typed.0)).map(|e| e.path.clone());
        if let Some(p) = hit {
            self.click(&p, false, false);
        }
    }

    fn peek_text(&mut self) {
        let Some(c) = self.cursor.clone() else { return };
        if self.peek.as_ref().is_some_and(|(p, _)| *p == c) {
            return;
        }
        let entry = self.entries.iter().find(|e| e.path == c);
        if entry.is_some_and(|e| matches!(e.kind(), "text" | "data")) {
            let mut buf = vec![0u8; 4096];
            let n = fs::File::open(&c).and_then(|mut f| f.read(&mut buf)).unwrap_or(0);
            self.peek = Some((c, String::from_utf8_lossy(&buf[..n]).into_owned()));
        } else {
            self.peek = None;
        }
    }

    // -- keys -----------------------------------------------------------------------------------

    fn key(&mut self, e: &KeyDownEvent) -> bool {
        let k = &e.keystroke;
        let (ctrl, shift, alt) = (k.modifiers.control, k.modifiers.shift, k.modifiers.alt);
        // our own fields take the keys while they are being typed in
        if self.field != Field::None {
            return match edit(&mut self.draft, k, 260) {
                Edit::Typed => true,
                Edit::Submit => {
                    match std::mem::replace(&mut self.field, Field::None) {
                        Field::Path => {
                            let p = PathBuf::from(self.draft.trim());
                            self.go(p);
                        }
                        Field::Rename(p) => self.finish_rename(&p),
                        Field::None => {}
                    }
                    true
                }
                Edit::Leave => {
                    self.field = Field::None;
                    true
                }
                Edit::Ignored => false,
            };
        }
        let key = k.key.as_str();
        let grid = if self.view == View::Grid { self.cols.max(1).cast_signed() } else { 1 };
        match (key, ctrl, shift, alt) {
            ("Left", _, _, true) => self.history(false),
            ("Right", _, _, true) => self.history(true),
            ("Up", false, s, _) => self.step(-grid, s),
            ("Down", false, s, _) => self.step(grid, s),
            ("Left", false, s, _) if self.view == View::Grid => self.step(-1, s),
            ("Right", false, s, _) if self.view == View::Grid => self.step(1, s),
            ("Return" | "KP_Enter", ..) => {
                if let Some(c) = self.cursor.clone() {
                    self.open(&c);
                }
            }
            ("BackSpace", false, ..) => self.up(),
            ("F2", ..) => {
                if let Some(c) = self.cursor.clone() {
                    self.begin_rename(c);
                }
            }
            ("Delete", ..) => self.trash(),
            ("Escape", ..) => {
                self.sel.clear();
                self.clip = None;
            }
            ("a" | "A", true, ..) => self.sel = self.visible().iter().map(|e| e.path.clone()).collect(),
            ("c" | "C", true, ..) => self.clip = (!self.sel.is_empty()).then(|| (self.sel.iter().cloned().collect(), false)),
            ("x" | "X", true, ..) => {
                if self.targets_or_refuse().is_some() {
                    self.clip = Some((self.sel.iter().cloned().collect(), true));
                }
            }
            ("v" | "V", true, ..) => self.paste(),
            ("d" | "D", true, ..) => self.duplicate(),
            ("n" | "N", true, true, _) => self.new_folder(),
            ("z" | "Z", true, s, _) => self.undo(s),
            ("y" | "Y", true, ..) => self.undo(true),
            ("l" | "L", true, ..) => {
                self.draft = self.dir.display().to_string();
                self.field = Field::Path;
            }
            ("b" | "B", true, ..) => self.bookmark(),
            ("h" | "H", true, ..) => {
                self.hidden = !self.hidden;
                self.tree_kids.clear();
            }
            ("1", true, ..) => self.set_view(View::List),
            ("2", true, ..) => self.set_view(View::Grid),
            _ => match &k.key_char {
                Some(c) if !ctrl && !alt && c.chars().all(|ch| ch.is_alphanumeric() || ch == '.' || ch == '_') => self.type_ahead(c),
                _ => return false,
            },
        }
        true
    }

    fn bookmark(&mut self) {
        if self.bookmarks.contains(&self.dir) {
            self.bookmarks.retain(|b| *b != self.dir);
        } else {
            self.bookmarks.push(self.dir.clone());
        }
    }

    // -- the pointer: boxes and drags ----------------------------------------------------------

    /// The items of area `n` a selection box covers (as the last frame placed them).
    fn band_select(&mut self, n: u32) {
        let Some(b) = self.band else { return };
        let (x0, x1) = (b.from.0.min(b.to.0), b.from.0.max(b.to.0));
        let (y0, y1) = (b.from.1.min(b.to.1), b.from.1.max(b.to.1));
        if !b.add {
            self.sel.clear();
        }
        let r = |h: &Hit| (h.at.origin.x, h.at.origin.y, h.at.origin.x + h.at.size.width, h.at.origin.y + h.at.size.height);
        self.sel.extend(self.hits.iter().filter(|h| h.item == Some(n)).filter(|h| {
            let (l, t, rt, bt) = r(h);
            l < x1 && rt > x0 && t < y1 && bt > y0
        }).map(|h| h.path.clone()));
    }

    /// The folder under the pointer, if any.
    fn target_at(&self, (x, y): (f32, f32)) -> Option<PathBuf> {
        self.hits.iter().filter(|h| h.folder).find(|h| {
            let a = h.at;
            x >= a.origin.x && x < a.origin.x + a.size.width && y >= a.origin.y && y < a.origin.y + a.size.height
        }).map(|h| h.path.clone())
    }
}

/// Every file under `from` (a file, or a folder's files) as (from, to, size); folders as
/// (from, to, `u64::MAX`) so they are made first.
fn collect_copy(from: &Path, to: &Path, out: &mut Vec<(PathBuf, PathBuf, u64)>) {
    if from.is_dir() {
        out.push((from.to_path_buf(), to.to_path_buf(), u64::MAX));
        if let Ok(rd) = fs::read_dir(from) {
            for e in rd.flatten().take(5000) {
                collect_copy(&e.path(), &to.join(e.file_name()), out);
            }
        }
    } else {
        out.push((from.to_path_buf(), to.to_path_buf(), fs::metadata(from).map_or(0, |m| m.len())));
    }
}

#[cfg(windows)]
fn is_hidden_attr(meta: Option<&fs::Metadata>) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.is_some_and(|m| m.file_attributes() & 0x2 != 0)
}

#[cfg(not(windows))]
const fn is_hidden_attr(_: Option<&fs::Metadata>) -> bool {
    false
}

fn name_of(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

fn size_text(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{b} B") } else { format!("{v:.1} {}", UNITS[u]) }
}

/// `YYYY-MM-DD HH:MM` (UTC) of a file time: the civil date from the day count.
fn date_text(t: Option<SystemTime>) -> String {
    let Some(s) = t.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs().cast_signed()) else { return String::from("—") };
    let (days, rem) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y}-{m:02}-{d:02} {:02}:{:02}", rem / 3600, rem / 60 % 60)
}

/// The places on this machine: the sandbox first, then the user's folders and the drives.
fn places(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = vec![(String::from("Sandbox"), root.to_path_buf())];
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    if let Some(h) = home {
        out.push((String::from("Home"), h.clone()));
        for d in ["Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos"] {
            if h.join(d).is_dir() {
                out.push((d.to_owned(), h.join(d)));
            }
        }
    }
    out.push((String::from("Temp"), std::env::temp_dir()));
    if cfg!(windows) {
        for l in b'C'..=b'Z' {
            let p = PathBuf::from(format!("{}:\\", l as char));
            if p.is_dir() {
                out.push((format!("Drive {}:", l as char), p));
            }
        }
    } else {
        out.push((String::from("Root"), PathBuf::from("/")));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Blender's File Browser, the kit's (not in blenderui-sys: declared here)

unsafe extern "C" {
    fn BK_file_dialog(title: *const c_char, mode: c_int, dir: *const c_char, file: *const c_char,
                      filters: *const *const c_char, nfilters: c_int, multiple: c_int, r_paths: *mut *mut *mut c_char) -> c_int;
    fn BK_file_dialog_free(paths: *mut *mut c_char);
}

/// Blender's File Browser in its own window, picking a folder (`None`: cancelled, or no window host).
fn blender_pick_folder(start: &Path) -> Option<PathBuf> {
    let title = CString::new("Jump to Folder").ok()?;
    let dir = CString::new(start.display().to_string()).ok()?;
    let empty = CString::new("").ok()?;
    let mut paths: *mut *mut c_char = std::ptr::null_mut();
    // SAFETY: NUL-terminated strings alive for the call, no filters; the kit fills `paths`
    // (NULL-terminated) when it returns a count > 0, which we free with its own call.
    let n = unsafe { BK_file_dialog(title.as_ptr(), 2, dir.as_ptr(), empty.as_ptr(), std::ptr::null(), 0, 0, &raw mut paths) };
    if n <= 0 || paths.is_null() {
        return None;
    }
    // SAFETY: at least one entry, a NUL-terminated string the kit owns until freed below.
    let first = unsafe { CStr::from_ptr(*paths) }.to_string_lossy().into_owned();
    // SAFETY: the array the call gave us, freed once.
    unsafe { BK_file_dialog_free(paths) };
    Some(PathBuf::from(first))
}

// ---------------------------------------------------------------------------------------------
// Drawing

type Weak = WeakEntity<Files>;

fn icon(e: &Entry, size: f32) -> Div {
    let (w, h) = if e.dir { (size, size * 0.74) } else { (size * 0.78, size) };
    let class = if e.dir { String::from("ic folder") } else { format!("ic file k-{}", e.kind()) };
    div().class(class).css("width", format!("{w}px")).css("height", format!("{h}px"))
}

/// A field of ours (the path, a name being changed): the draft and a caret.
fn field(id: String, text: &str, width: &str) -> Div {
    div().id(id).class("field on").css("width", width).child(span().child(text.to_owned())).child(div().class("caret"))
}

/// What the editors need of the app, copied for their factories.
struct Snap {
    dir: PathBuf,
    root: PathBuf,
    items: Vec<Entry>,
    shown: usize,
    error: Option<String>,
    sel: BTreeSet<PathBuf>,
    cursor: Option<PathBuf>,
    view: View,
    views: u32,
    /// The column sorted by, and whether backwards.
    order: (Sort, bool),
    hidden: bool,
    thumb: f32,
    filter: String,
    field: Field,
    draft: String,
    cut: BTreeSet<PathBuf>,
    /// How far back and forward the history goes.
    back: usize,
    fwd: usize,
    writable: bool,
    drop: Option<PathBuf>,
    places: Vec<(String, PathBuf)>,
    bookmarks: Vec<PathBuf>,
    tree: Vec<(PathBuf, usize, bool, bool)>,
    peek: Option<(PathBuf, String)>,
    job: Option<(String, u64, u64, usize)>,
    steps: Vec<Step>,
    log: Vec<Line>,
    headless: bool,
}

/// Element ids for one area's editor: `{area}{n}-{name}`, so two copies of an editor (a split)
/// never share one.
fn ids(area: &'static str, n: u32) -> impl Fn(&str) -> String {
    move |x| format!("{area}{n}-{x}")
}

fn files_editor(s: &Snap, weak: Weak, n: u32) -> Editor {
    let id = ids("fl", n);
    // the header: history, the path (breadcrumbs, or our own field), the view, sort, filter
    let mut crumbs = div().class("crumbs");
    let mut acc = PathBuf::new();
    for (k, comp) in s.dir.components().enumerate() {
        acc.push(comp);
        let label = comp.as_os_str().to_string_lossy().trim_end_matches(['\\', '/']).to_owned();
        if label.is_empty() {
            continue;
        }
        if k > 0 {
            crumbs = crumbs.child(span().class("slash").child("›"));
        }
        let to = acc.clone();
        crumbs = crumbs.child(div().id(id(&format!("cr{k}"))).class(if s.drop.as_ref() == Some(&to) { "crumb drop" } else { "crumb" })
            .child(label).on_click(act(weak, move |this, _: &ClickEvent| this.go(to.clone()))));
    }
    let path = if s.field == Field::Path {
        field(id("path"), &s.draft, "100%")
    } else {
        crumbs.id(id("crumbs"))
    };
    let mut sort = Dropdown().id(id("sort")).on_change(act(weak, |this, e: &ChangeEvent| {
        this.sort = SORTS.get(e.number as usize).map_or(Sort::Name, |s| s.0);
    }));
    for (srt, name) in SORTS {
        sort = sort.child(option().selected(srt == s.order.0).child(name));
    }
    let vk = s.views;
    let header = div().class("hdr")
        .child(Button().id(id("back")).child("◀").disabled(s.back == 0).on_click(act(weak, |this, _: &ClickEvent| this.history(false))))
        .child(Button().id(id("fwd")).child("▶").disabled(s.fwd == 0).on_click(act(weak, |this, _: &ClickEvent| this.history(true))))
        .child(Button().id(id("up")).child("Up").on_click(act(weak, |this, _: &ClickEvent| this.up())))
        .child(div().class("sep"))
        .child(div().class("where").child(path))
        .child(Button().id(id("edit")).child("Path…").on_click(act(weak, |this, _: &ClickEvent| {
            this.draft = this.dir.display().to_string();
            this.field = Field::Path;
        })))
        .child(label().child(Radio().id(id(&format!("vlist{vk}"))).attr("name", format!("view{n}{vk}")).checked(s.view == View::List)
            .on_change(act(weak, |this, e: &ChangeEvent| if e.checked { this.set_view(View::List) }))).child("List"))
        .child(label().child(Radio().id(id(&format!("vgrid{vk}"))).attr("name", format!("view{n}{vk}")).checked(s.view == View::Grid)
            .on_change(act(weak, |this, e: &ChangeEvent| if e.checked { this.set_view(View::Grid) }))).child("Grid"))
        .child(div().class("sep"))
        .child(sort)
        .child(label().child(Checkbox().id(id("desc")).checked(s.order.1).on_change(act(weak, |this, e: &ChangeEvent| this.desc = e.checked))).child("Desc"))
        .child(label().child(Checkbox().id(id("hidden")).checked(s.hidden).on_change(act(weak, |this, e: &ChangeEvent| {
            this.hidden = e.checked;
            this.tree_kids.clear();
        }))).child("Hidden"))
        .child(div().class("sep"))
        // Blender's text field, live: it filters as you type (like Blender's own filter fields)
        .child(TextField().id(id("filter")).attr("live", "").placeholder("Filter").value(s.filter.clone()).css("width", "130px")
            .on_input(act(weak, |this, e: &ChangeEvent| this.filter.clone_from(&e.value))));

    // the list or the grid
    let mut list = div().id(id("list")).class("list")
        .on_pointer_down(weak.listener(move |this: &mut Files, e: &PointerEvent, w, cx| {
            // a press on empty space starts a box (the items stop theirs from getting here)
            let p = (e.position.x.0, e.position.y.0);
            this.band = Some(Band { from: p, to: p, add: e.modifiers.control || e.modifiers.shift });
            if !e.modifiers.control && !e.modifiers.shift {
                this.sel.clear();
            }
            this.field = Field::None;
            w.set_pointer_capture(cx);
            cx.notify();
        }))
        .on_pointer_move(weak.listener(move |this: &mut Files, e: &PointerEvent, w, cx| {
            let p = (e.position.x.0, e.position.y.0);
            if let Some(b) = &mut this.band {
                b.to = p;
                this.band_select(n);
                cx.notify();
            } else if let Some(d) = &mut this.drag {
                d.at = p;
                if !d.live && (p.0 - d.from.0).abs() + (p.1 - d.from.1).abs() > 6.0 {
                    // a drag now: the list holds the pointer, wherever it goes (Places too)
                    d.live = true;
                    w.set_pointer_capture(cx);
                }
                let at = this.target_at(p);
                this.drop = at.filter(|t| !this.drag.as_ref().is_some_and(|d| d.paths.contains(t)));
                cx.notify();
            }
        }))
        .on_pointer_up(weak.listener(move |this: &mut Files, _: &PointerEvent, w, cx| {
            this.band = None;
            if let Some(d) = this.drag.take() {
                if d.live && let Some(t) = this.drop.take() {
                    this.move_into(&d.paths, &t);
                }
                this.drop = None;
            }
            w.release_pointer_capture(cx);
            cx.notify();
        }));
    let item_events = |d: Div, e: &Entry| {
        let (p1, p2) = (e.path.clone(), e.path.clone());
        d.on_pointer_down(weak.listener(move |this: &mut Files, ev: &PointerEvent, w, cx| {
            let m = ev.modifiers;
            if !this.sel.contains(&p1) || m.control || m.shift {
                this.click(&p1, m.control, m.shift);
            } else {
                this.cursor = Some(p1.clone());
                this.peek_text();
            }
            this.field = Field::None;
            // a drag of the selection (moves into a folder it is dropped on)
            let at = (ev.position.x.0, ev.position.y.0);
            this.drag = Some(Drag { paths: this.sel.iter().cloned().collect(), from: at, at, live: false });
            let _ = w;
            cx.stop_propagation();
            cx.notify();
        }))
        .on_double_click(act(weak, move |this, _: &ClickEvent| this.open(&p2)))
    };
    let hidden_more = s.shown.saturating_sub(s.items.len());
    if s.view == View::List {
        let head = |k: Sort, label: &'static str, class: &'static str| {
            let arrow = if s.order.0 == k { if s.order.1 { " ▾" } else { " ▴" } } else { "" };
            th().id(id(&format!("h{label}"))).class(class).child(format!("{label}{arrow}"))
                // a header is no empty space: a press here starts no box (which would take the click)
                .on_pointer_down(weak.listener(|_: &mut Files, _: &PointerEvent, _, cx| cx.stop_propagation()))
                .on_click(act(weak, move |this, _: &ClickEvent| {
                    if this.sort == k { this.desc = !this.desc } else { (this.sort, this.desc) = (k, false) }
                }))
        };
        let mut body = tbody();
        for (k, e) in s.items.iter().enumerate() {
            let mut class = String::from("it");
            if s.sel.contains(&e.path) { class.push_str(" sel"); }
            if s.cursor.as_ref() == Some(&e.path) { class.push_str(" cur"); }
            if s.drop.as_ref() == Some(&e.path) { class.push_str(" drop"); }
            if s.cut.contains(&e.path) { class.push_str(" cut"); }
            let name_cell = if let Field::Rename(p) = &s.field && *p == e.path {
                div().class("name").child(icon(e, 14.0)).child(field(id("rename"), &s.draft, "220px"))
            } else {
                div().class("name").child(icon(e, 14.0)).child(span().class(if e.hidden { "dim" } else { "" }).child(e.name.clone()))
            };
            body = body.child(item_events(tr().id(id(&format!("it{k}"))).class(class)
                .child(td().child(name_cell))
                .child(td().class("num").child(if e.dir { String::from("—") } else { size_text(e.size) }))
                .child(td().class("dim").child(if e.dir { String::from("Folder") } else if e.ext().is_empty() { String::from("File") } else { format!("{} file", e.ext().to_uppercase()) }))
                .child(td().class("dim").child(date_text(e.modified))), e));
        }
        list = list.child(table().class("rows")
            .child(thead().child(tr().child(head(Sort::Name, "Name", "")).child(head(Sort::Size, "Size", "num"))
                .child(head(Sort::Kind, "Type", "")).child(head(Sort::Modified, "Modified", ""))))
            .child(body));
    } else {
        let mut grid = div().class("grid");
        let t = s.thumb;
        for (k, e) in s.items.iter().enumerate() {
            let mut class = String::from("cell");
            if s.sel.contains(&e.path) { class.push_str(" sel"); }
            if s.cursor.as_ref() == Some(&e.path) { class.push_str(" cur"); }
            if s.drop.as_ref() == Some(&e.path) { class.push_str(" drop"); }
            if s.cut.contains(&e.path) { class.push_str(" cut"); }
            let thumb = div().class("thumb").css("width", format!("{t}px")).css("height", format!("{}px", t * 0.75));
            let thumb = if e.is_image() {
                thumb.child(img().src(e.path.display().to_string()).css("max-width", format!("{t}px")).css("max-height", format!("{}px", t * 0.75)))
            } else {
                thumb.child(icon(e, t * 0.5))
            };
            let cap = if let Field::Rename(p) = &s.field && *p == e.path {
                field(id("rename"), &s.draft, &format!("{t}px"))
            } else {
                div().class("cap").child(e.name.clone())
            };
            grid = grid.child(item_events(div().id(id(&format!("it{k}"))).class(class).css("width", format!("{}px", t + 16.0))
                .child(thumb).child(cap), e));
        }
        list = list.child(grid);
    }
    // under the column headers, so they stay put
    if let Some(err) = &s.error {
        list = list.child(div().class("empty").child(format!("Can't read this folder: {err}")));
    } else if s.items.is_empty() {
        list = list.child(div().class("empty").child(if s.filter.is_empty() { "Empty folder" } else { "Nothing matches the filter" }));
    }
    if hidden_more > 0 {
        list = list.child(div().class("more").child(format!("{hidden_more} more: narrow them down with the filter")));
    }
    // the sidebar: preview and properties of the item under the cursor
    let cur = s.cursor.as_ref().and_then(|c| s.items.iter().find(|e| e.path == *c));
    let side = match cur {
        Some(e) => {
            let preview = if e.is_image() {
                div().class("preview").child(img().src(e.path.display().to_string()).css("max-width", "100%").css("max-height", "220px"))
            } else if let Some((_, txt)) = s.peek.as_ref().filter(|(p, _)| *p == e.path) {
                div().class("preview").child(pre().child(txt.clone()))
            } else {
                div().class("preview").child(icon(e, 64.0))
            };
            let prop = |k: &str, v: String| div().class("prop").child(span().child(k.to_owned())).child(span().child(v));
            div().class("side")
                .child(details().open(true).child(summary().child("Preview")).child(preview))
                .child(details().open(true).child(summary().child("Properties"))
                    .child(prop("Name", e.name.clone()))
                    .child(prop("Kind", if e.dir { String::from("Folder") } else { e.kind().to_owned() }))
                    .child(prop("Size", if e.dir { String::from("—") } else { format!("{} ({} bytes)", size_text(e.size), e.size) }))
                    .child(prop("Modified", date_text(e.modified)))
                    .child(prop("Location", e.path.parent().map(|p| p.display().to_string()).unwrap_or_default()))
                    .child(prop("Writable", String::from(if e.path.starts_with(&s.root) { "Yes (sandbox)" } else { "No (read-only here)" }))))
        }
        None => div().class("side").child(div().class("preview").child(span().class("dim").child("Select a file to preview it"))),
    };
    let total: u64 = s.items.iter().filter(|e| s.sel.contains(&e.path)).map(|e| e.size).sum();
    let footer = div().class("footer")
        .child(span().child(format!("{} item{}", s.shown, if s.shown == 1 { "" } else { "s" })))
        .child(span().child(format!("{} selected ({})", s.sel.len(), size_text(total))))
        .child(div().class("spacer"))
        .child(span().child("Thumbnails"))
        .child(NumberSlider().id(id("thumb")).min(48.0).max(192.0).step(8.0).value(format!("{}", s.thumb)).css("width", "150px")
            .on_change(act(weak, |this, e: &ChangeEvent| this.thumb = e.number as f32)))
        .child(Button().id(id("newdir")).child("New Folder").disabled(!s.writable).on_click(act(weak, |this, _: &ClickEvent| this.new_folder())))
        .child(Button().id(id("browse")).child("Blender File Browser…").disabled(s.headless).on_click(act(weak, |this, _: &ClickEvent| {
            if let Some(p) = blender_pick_folder(&this.dir) {
                this.go(p);
            }
        })));
    Editor::new(list).header(header)
        .sidebar(Region::new(side).size(260).overlap(false).hotkey("n"))
        .footer(Region::new(footer).overlap(false))
}

fn places_editor(s: &Snap, weak: Weak, n: u32) -> Editor {
    let id = ids("pl", n);
    let mut col = div().class("places").child(h4().child("Places"));
    let row = |rid: String, label: String, path: &PathBuf, depth: usize, twisty: Option<(bool, PathBuf)>, folder: bool| {
        let on = s.dir == *path;
        let drop = s.drop.as_ref() == Some(path);
        let to = path.clone();
        let mut d = div().id(rid).class(if drop { "place drop" } else if on { "place on" } else { "place" })
            .css("padding-left", format!("{}px", 8 + depth * 12));
        if let Some((open, p)) = twisty {
            d = d.child(div().class("tree-tw").child(if open { "▾" } else { "▸" }).on_click(act(weak, move |this, _: &ClickEvent| {
                if !this.open_tree.remove(&p) {
                    this.open_tree.insert(p.clone());
                }
            })));
        }
        if folder {
            d = d.child(div().class("ic folder").css("width", "13px").css("height", "10px"));
        }
        d.child(span().class("n").child(label)).on_click(act(weak, move |this, _: &ClickEvent| this.go(to.clone())))
    };
    for (k, (name, p)) in s.places.iter().enumerate() {
        col = col.child(row(id(&format!("place{k}")), name.clone(), p, 0, None, true));
    }
    col = col.child(h4().child("Bookmarks"));
    if s.bookmarks.is_empty() {
        col = col.child(div().class("log").child("Ctrl+B bookmarks the folder"));
    }
    for (k, b) in s.bookmarks.iter().enumerate() {
        col = col.child(row(id(&format!("bm{k}")), name_of(b), b, 0, None, true));
    }
    col = col.child(h4().child("Sandbox Tree"));
    for (k, (p, depth, open, has_kids)) in s.tree.iter().enumerate() {
        col = col.child(row(id(&format!("tree{k}")), name_of(p), p, *depth, has_kids.then(|| (*open, p.clone())), true));
    }
    Editor::new(col).header(div().class("hdr").child(b().child("Places")))
}

fn ops_editor(s: &Snap, weak: Weak, n: u32) -> Editor {
    let id = ids("op", n);
    let mut col = div().class("ops");
    if let Some((label, done, total, left)) = &s.job {
        col = col.child(div().class("job")
            .child(div().class("what").child(span().child(format!("{label}: {left} file{} left", if *left == 1 { "" } else { "s" })))
                .child(span().child(format!("{} of {}", size_text(*done), size_text(*total)))))
            .child(ProgressBar().id(id("job")).max(1.0).value(format!("{:.3}", if *total == 0 { 1.0 } else { *done as f64 / *total as f64 }))));
    }
    col = col.child(h4().child("History"));
    if s.steps.is_empty() {
        col = col.child(div().class("log").child("Nothing done yet. Changes work in the Sandbox, and Ctrl+Z undoes them."));
    }
    for (k, st) in s.steps.iter().enumerate().rev() {
        col = col.child(div().id(id(&format!("h{k}"))).class(if st.undone { "hist undone" } else { "hist" })
            .child(span().class("k").child(format!("#{}", k + 1))).child(span().child(format!("{} · {} item{}", st.what, st.ops.len(), if st.ops.len() == 1 { "" } else { "s" }))));
    }
    col = col.child(h4().child("Log"));
    let t = now();
    for l in s.log.iter().rev().take(30) {
        let mut class = String::from(if l.bad { "log bad" } else { "log" });
        if t - l.at < 0.3 {
            class.push_str(" fresh");
        }
        col = col.child(div().class(class).child(l.text.clone()));
    }
    let header = div().class("hdr").child(b().child("Operations"))
        .child(div().class("spacer"))
        .child(Button().id(id("undo")).child("Undo").disabled(!s.steps.iter().any(|x| !x.undone)).on_click(act(weak, |this, _: &ClickEvent| this.undo(false))))
        .child(Button().id(id("redo")).child("Redo").disabled(!s.steps.iter().any(|x| x.undone)).on_click(act(weak, |this, _: &ClickEvent| this.undo(true))));
    Editor::new(col).header(header)
}

impl Files {
    fn snap(&mut self) -> Snap {
        self.peek_text();
        let vis: Vec<Entry> = self.visible().into_iter().cloned().collect();
        let shown = vis.len();
        let items: Vec<Entry> = vis.into_iter().take(DRAW_MAX).collect();
        // the sandbox's tree, as far as it is open
        let mut tree = Vec::new();
        let mut stack = vec![(self.root.clone(), 0usize)];
        while let Some((p, depth)) = stack.pop() {
            let kids = self.tree_children(&p);
            let open = self.open_tree.contains(&p);
            tree.push((p.clone(), depth, open, !kids.is_empty()));
            if open && tree.len() < 400 {
                for k in kids.into_iter().rev() {
                    stack.push((k, depth + 1));
                }
            }
        }
        Snap {
            dir: self.dir.clone(),
            root: self.root.clone(),
            items,
            shown,
            error: self.error.clone(),
            sel: self.sel.clone(),
            cursor: self.cursor.clone(),
            view: self.view,
            views: self.views,
            order: (self.sort, self.desc),
            hidden: self.hidden,
            thumb: self.thumb,
            filter: self.filter.clone(),
            field: self.field.clone(),
            draft: self.draft.clone(),
            cut: self.clip.as_ref().filter(|c| c.1).map(|c| c.0.iter().cloned().collect()).unwrap_or_default(),
            back: self.back.len(),
            fwd: self.fwd.len(),
            writable: self.here_writable(),
            drop: self.drop.clone(),
            places: places(&self.root),
            bookmarks: self.bookmarks.clone(),
            tree,
            peek: self.peek.clone(),
            job: self.job.as_ref().map(|j| (j.label.clone(), j.done, j.total, j.files.len())),
            steps: self.steps.clone(),
            log: self.log.clone(),
            headless: self.headless,
        }
    }
}

impl Render for Files {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        focus_root(w, &mut self.frames);
        self.step_job();
        // frames only while something moves: a copy, a fresh log line, a caret (CSS blinks it)
        if self.job.is_some() || self.log.last().is_some_and(|l| now() - l.at < 0.3) {
            w.request_animation_frame();
        }
        // the grid's columns, from the list's width at the last frame
        if let Some(b) = w.bounds_of("fl0-list") {
            self.cols = ((b.size.width - 16.0) / (self.thumb + 22.0)).floor().max(1.0) as usize;
        }
        let s = Rc::new(self.snap());
        // where the last frame's targets went (a listener can't ask: it only sees its own path)
        self.hits.clear();
        for t in self.targets.drain(..) {
            if let Some(at) = w.bounds_of(&t.id) {
                self.hits.push(Hit { at, path: t.path, item: t.item, folder: t.folder });
            }
        }
        // this frame's, while a box or a drag is on: the items (box select, drops on folders)
        // and Places' folders, by the ids the editors give them
        let mut add = |id: String, path: &Path, item: Option<u32>, folder: bool| {
            self.targets.push(Target { id, path: path.to_path_buf(), item, folder });
        };
        for n in (0..4).filter(|_| self.band.is_some() || self.drag.is_some()) {
            for (k, e) in s.items.iter().enumerate() {
                add(format!("fl{n}-it{k}"), &e.path, Some(n), e.dir);
            }
            for (k, p) in s.places.iter().enumerate() {
                add(format!("pl{n}-place{k}"), &p.1, None, true);
            }
            for (k, p) in s.bookmarks.iter().enumerate() {
                add(format!("pl{n}-bm{k}"), p, None, true);
            }
            for (k, p) in s.tree.iter().enumerate() {
                add(format!("pl{n}-tree{k}"), &p.0, None, true);
            }
        }
        let weak = cx.weak_entity();
        let counter = || std::cell::Cell::new(0u32);
        let next = |c: &std::cell::Cell<u32>| {
            let n = c.get();
            c.set(n + 1);
            n
        };
        let (s1, s2, s3) = (s.clone(), s.clone(), s.clone());
        let (n1, n2, n3) = (counter(), counter(), counter());
        let editors = Editors::new()
            .editor("Files", "FILEBROWSER", move |_: &AreaRef| files_editor(&s1, weak, next(&n1)))
            .editor("Places", "BOOKMARKS", move |_: &AreaRef| places_editor(&s2, weak, next(&n2)))
            .editor("Operations", "INFO", move |_: &AreaRef| ops_editor(&s3, weak, next(&n3)));

        let writable = self.here_writable();
        let last = self.log.last().cloned();
        // the selection box and the dragged files, over everything (in window coordinates)
        let overlay = {
            let mut o = div().css("position", "absolute").css("left", "0").css("top", "0").css("width", "0").css("height", "0");
            if let Some(b) = self.band {
                let (x0, y0) = (b.from.0.min(b.to.0), b.from.1.min(b.to.1));
                o = o.child(div().class("band").css("left", format!("{x0}px")).css("top", format!("{y0}px"))
                    .css("width", format!("{}px", (b.to.0 - b.from.0).abs())).css("height", format!("{}px", (b.to.1 - b.from.1).abs())));
            }
            if let Some(d) = self.drag.as_ref().filter(|d| d.live) {
                o = o.child(div().class("ghost").css("left", format!("{}px", d.at.0 + 14.0)).css("top", format!("{}px", d.at.1 + 10.0))
                    .child(format!("{} item{}{}", d.paths.len(), if d.paths.len() == 1 { "" } else { "s" },
                                   self.drop.as_ref().map_or(String::new(), |t| format!(" → {}", name_of(t))))));
            }
            o
        };
        let top = div().class("top").child(span().class("brand").child("BUI Files"))
            .child(span().class(if writable { "sandbox-chip" } else { "ro-chip" }).child(if writable { "Sandbox: changes allowed" } else { "Read-only here" }))
            .child(div().class("spacer"))
            .child(span().css("color", "#9a9a9a").child(self.dir.display().to_string()));
        let status = div().class("status")
            .child(span().child(kbd().child("F2")).child(" rename  ").child(kbd().child("Del")).child(" delete  ")
                .child(kbd().child("Ctrl+C")).child(" copy  ").child(kbd().child("Ctrl+V")).child(" paste  ").child(kbd().child("Ctrl+Z")).child(" undo  ")
                .child(kbd().child("Ctrl+L")).child(" path  ").child(kbd().child("N")).child(" sidebar"))
            .child(div().class("spacer"))
            .child(match last {
                Some(l) => span().class(if l.bad { "msg" } else { "msg ok" }).child(l.text),
                None => span(),
            });
        let root = app().id("root").css("--accent", "#4772b3").css("position", "relative")
            // keys typed into a Blender text field (the filter, a number typed into the slider) reach
            // the page too: no shortcuts then
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                // a key we act on is taken: Blender's own handling (Ctrl+Z over widget edits, a
                // hovered field's Backspace) doesn't run as well
                if !w.text_field_focused() && this.key(e) {
                    cx.prevent_default();
                    cx.notify();
                }
            }))
            .child(style().child(CSS))
            .child(top)
            .child(Screen().id("screen").class("screen").editors(editors)
                .layout(AreaSplit::v(0.2, "Places", AreaSplit::h(0.74, "Files", "Operations"))))
            .child(status);
        root.child(overlay)
    }
}

/// Every feature, in the sandbox only: opening folders, the grid and its slider, selecting by
/// click, Ctrl, Shift, the box and Ctrl+A, the preview, renaming, a new folder, copy and paste
/// (with progress), a drag into a folder, delete, undo and redo, the filter, the path field,
/// sorting, hidden files, bookmarks, and a refusal outside the sandbox.
fn storyboard(s: &mut Story) {
    let dbl = |s: &mut Story, id: &str| {
        let (x, y) = s.centre(id);
        s.cx.simulate_click(s.win, x, y);
        s.cx.simulate_click(s.win, x, y);
    };
    s.play("00-open", &[0, 300]);
    s.cx.simulate_mouse_move(s.win, 600.0, 300.0);
    // into Textures (sorted: Audio, Documents, Projects, Textures, then files), grid view
    dbl(s, "fl0-it3");
    s.play("01-textures", &[0, 200]);
    s.click("fl0-vgrid0");
    s.play("02-grid", &[0, 200]);
    s.click("fl0-it0");
    s.play("03-preview-image", &[0, 200]);
    // the thumbnail slider (Blender's: dragged)
    let b = s.bounds("fl0-thumb");
    s.cx.simulate_drag(s.win, (b.origin.x + 20.0, b.origin.y + b.size.height / 2.0), (b.origin.x + b.size.width - 10.0, b.origin.y + b.size.height / 2.0));
    s.play("04-big-thumbs", &[0, 200]);
    // the pointer stays on the slider: Backspace is the app's (taken), so the slider keeps its size
    // back up, into Documents, list view, a text preview
    s.key("BackSpace");
    s.click("fl0-vlist1");
    s.play("04b-list", &[0]);
    dbl(s, "fl0-it1");
    s.play("05a-documents", &[0]);
    s.click("fl0-it1");
    s.play("05-text-preview", &[0, 200]);
    // rename todo.md (F2) to tasks.md
    s.key("Down");
    s.key("F2");
    for _ in 0..7 {
        s.key("BackSpace");
    }
    s.type_text("tasks.md");
    s.play("06-renaming", &[0]);
    s.key("Return");
    s.play("07-renamed", &[0, 300]);
    // a new folder, named Exports
    s.cx.simulate_key_with(s.win, "", "n", true, true);
    for _ in 0..12 {
        s.key("BackSpace");
    }
    s.type_text("Exports");
    s.key("Return");
    s.play("08-new-folder", &[0, 300]);
    // copy readme.txt, paste it into Exports
    s.tick(1100);
    s.type_text("readme");
    s.ctrl("c", false);
    s.tick(1100);
    s.type_text("exports");
    s.key("Return");
    s.ctrl("v", false);
    s.play("09-pasted", &[0, 100, 400]);
    s.key("BackSpace");
    // a big copy with progress: up to the sandbox, Projects/Spaceship/renders, cache.bin
    s.key("BackSpace");
    s.tick(1100);
    s.type_text("projects");
    s.key("Return");
    s.tick(1100);
    s.type_text("spaceship");
    s.key("Return");
    s.tick(1100);
    s.type_text("renders");
    s.key("Return");
    s.tick(1100);
    s.type_text("cache");
    s.ctrl("d", false);
    s.play("10-copying", &[0, 16, 48, 120, 400]);
    // select frames 1..3 with Shift, then drag them onto the folder… there is none here: make one
    s.click("fl0-it1");
    let (x, y) = s.centre("fl0-it3");
    s.cx.simulate_click_with(s.win, x, y, false, true);
    s.play("11-shift", &[0]);
    // a selection box: from the empty space under the rows, up over the last three
    let z = s.bounds("fl0-it4");
    s.cx.simulate_drag(s.win, (z.origin.x + 300.0, z.origin.y + z.size.height + 40.0),
                       (z.origin.x + 120.0, z.origin.y - z.size.height * 1.5));
    s.play("12-box", &[0, 200]);
    // Ctrl+A, then delete everything selected, then undo it
    s.ctrl("a", false);
    s.key("Delete");
    s.play("13-deleted", &[0, 300]);
    // Ctrl+Z is the app's (taken): the slider dragged earlier keeps its size
    s.ctrl("z", false);
    s.play("14-undone", &[0, 300]);
    // up to Spaceship, find notes.md by typing, and drag it into renders
    s.key("BackSpace");
    s.tick(1100);
    s.type_text("notes");
    let from = s.centre("fl0-it1");
    let to = s.centre("fl0-it0");
    s.cx.simulate_drag(s.win, from, to);
    s.play("15-dragged", &[0, 300]);
    // the filter (Blender's live text field) and the hidden files
    s.click("fl0-filter");
    s.type_text("ren");
    s.play("16-filter", &[0, 200]);
    s.key("Escape");
    s.click("pl0-place0");
    s.ctrl("h", false);
    s.play("17-hidden", &[0, 200]);
    // sort by size by its header, then go read-only (Temp) and try to delete there: refused
    s.click("fl0-hSize");
    s.click("fl0-hSize");
    s.play("18-sorted", &[0, 200]);
    s.ctrl("b", false);
    s.ctrl("l", false);
    for _ in 0..200 {
        s.key("BackSpace");
    }
    s.type_text(&std::env::temp_dir().display().to_string());
    s.key("Return");
    s.key("Down");
    s.key("Delete");
    s.play("19-refused", &[0, 300]);
    s.click("pl0-place0");
    s.play("20-back-home", &[0, 300]);
}

fn main() {
    let (w, h) = size_arg((1360.0, 820.0));
    let board = arg("--storyboard").map(|dir| -> Script { story(dir, storyboard) });
    let headless = board.is_some();
    run_with(w, h, "BUI Files", move || Files::new(headless), board);
}

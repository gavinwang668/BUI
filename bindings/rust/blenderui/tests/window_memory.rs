//! No leaks: windows opened, drawn and closed, again and again, give back what they held (the
//! kit's tree and context, the raster, the root view, the Rust frame and caches), counted by a Rust
//! allocator and Blender's MEM counters (one test: the counters are global).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

use blenderui::prelude::*;
use blenderui::sys;

/// Bytes the Rust side holds (alloc - dealloc).
struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        // SAFETY: same layout contract as the caller's.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
        // SAFETY: `p` came from `alloc` with `l`.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        LIVE.fetch_add(n as isize - l.size() as isize, Ordering::Relaxed);
        // SAFETY: as `dealloc` + `alloc`.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// A page with text, a border, a shadow and a list: a bit of everything a frame holds.
struct Page(Vec<String>);

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .class("flex flex-col gap-2 p-4 rounded-lg border shadow-md")
            .bg(rgb(0x202020))
            .text_color(rgb(0xeeeeee))
            .children(self.0.iter().map(|s| div().class("px-2 py-1 rounded").child(s.clone())))
    }
}

/// Open a window on a `Page`, draw it twice (a pointer move between), close it.
fn round(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(640.), px(400.)) })),
        ..Default::default()
    };
    let rows = (0..20).map(|i| format!("row {i}")).collect();
    let win = cx.open_window(options, move |_, cx| cx.new(|_| Page(rows))).unwrap();
    cx.draw(win.id);
    cx.simulate_mouse_move(win.id, 100., 100.);
    cx.draw(win.id);
    cx.close_window(win.id);
}

/// What is held now, once the windows closed so far are freed (the next flush frees them).
fn held(cx: &mut App) -> (isize, u32) {
    cx.run_pending();
    // SAFETY: a counter read.
    (LIVE.load(Ordering::Relaxed), unsafe { sys::MEM_get_memory_blocks_in_use() })
}

#[test]
fn closed_windows_return_their_memory() {
    Application::headless().run(|cx| {
        // warm up: fonts, glyphs, style tables, thread-locals
        for _ in 0..3 {
            round(cx);
        }
        let (rust0, c0) = held(cx);
        const ROUNDS: isize = 200;
        for _ in 0..ROUNDS {
            round(cx);
        }
        let (rust1, c1) = held(cx);
        assert_eq!(c1, c0, "C (MEM) blocks returned");
        // a closed window keeps its id (its slot in the app's lists): a few words each, not its frame
        assert!(rust1 - rust0 < 64 * ROUNDS, "Rust bytes returned: {} more after {ROUNDS} windows", rust1 - rust0);
    });
}

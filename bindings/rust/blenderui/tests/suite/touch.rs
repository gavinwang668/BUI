//! Touch screens and trackpads: `on_pinch`, `on_rotate` and `on_pan` listeners, a tap's click and
//! a finger's scroll, driven through `simulate_touch` / `simulate_magnify` / `simulate_rotate`
//! (the kit's touch model, as a window host feeds it).

use std::sync::Mutex;

use blenderui::prelude::*;

fn open<V: Render>(cx: &mut App, w: f32, h: f32, v: V) -> WindowHandle<V> {
    let opts = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.0), px(0.0)), size: size(px(w), px(h)) })),
        ..Default::default()
    };
    cx.open_window(opts, |_, cx| cx.new(|_| v)).unwrap()
}

fn rows() -> Div {
    let mut rows = div().flex().flex_col();
    for i in 0..40 {
        rows = rows.child(div().h(px(20.0)).child(format!("Row {i}")));
    }
    rows
}

static PINCHES: Mutex<Vec<(f32, f32, f32, bool)>> = Mutex::new(Vec::new());
static TURNS: Mutex<Vec<(f32, bool)>> = Mutex::new(Vec::new());

struct Map;

impl Render for Map {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("map")
            .w(px(300.0))
            .h(px(200.0))
            .on_pinch(|e, _, _| PINCHES.lock().unwrap().push((e.delta, e.position.x.0, e.position.y.0, e.touch)))
            .on_rotate(|e, _, _| TURNS.lock().unwrap().push((e.degrees, e.touch)))
    }
}

#[test]
fn two_fingers_pinch_and_turn_an_element_and_so_does_a_trackpad() {
    Application::headless().run(|cx| {
        let win = open(cx, 300.0, 200.0, Map);
        cx.draw(win.id);
        // spread from 100 px apart to 200 about (150, 100)
        cx.simulate_touch(win.id, 1, TouchPhase::Down, 100.0, 100.0);
        cx.simulate_touch(win.id, 2, TouchPhase::Down, 200.0, 100.0);
        for i in 1..=5 {
            let d = 10.0 * i as f32;
            cx.simulate_touch(win.id, 1, TouchPhase::Move, 100.0 - d, 100.0);
            cx.simulate_touch(win.id, 2, TouchPhase::Move, 200.0 + d, 100.0);
        }
        cx.simulate_touch(win.id, 1, TouchPhase::Up, 50.0, 100.0);
        cx.simulate_touch(win.id, 2, TouchPhase::Up, 250.0, 100.0);
        let pinches = std::mem::take(&mut *PINCHES.lock().unwrap());
        assert_eq!(pinches.len(), 5, "{pinches:?}");
        let scale: f32 = pinches.iter().map(|p| 1.0 + p.0).product();
        assert!((scale - 2.0).abs() < 1e-3, "scale {scale}");
        assert!(pinches.iter().all(|p| (p.1 - 150.0).abs() < 1.0 && (p.2 - 100.0).abs() < 1.0 && p.3));
        assert!(TURNS.lock().unwrap().is_empty(), "a straight spread is no turn");

        // a trackpad's pinch and turn: not from a touch screen
        cx.simulate_magnify(win.id, 120.0, 80.0, 0.25);
        cx.simulate_rotate(win.id, 120.0, 80.0, 30.0);
        let pinches = std::mem::take(&mut *PINCHES.lock().unwrap());
        assert_eq!(pinches.len(), 1);
        assert!((pinches[0].0 - 0.25).abs() < 1e-6 && !pinches[0].3);
        assert_eq!(*TURNS.lock().unwrap(), vec![(30.0, false)]);
    });
}

static CLICKS: Mutex<u32> = Mutex::new(0);
static PANS: Mutex<Vec<(f32, f32, ScrollPhase)>> = Mutex::new(Vec::new());
static TAKE_PANS: Mutex<bool> = Mutex::new(false);

struct Page;

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        app()
            .child(div().id("top").h(px(20.0)).on_click(|_, _, _| *CLICKS.lock().unwrap() += 1).child("top"))
            .child(rows())
            .on_pan(|e, _, cx| {
                PANS.lock().unwrap().push((e.delta.x.0, e.delta.y.0, e.phase));
                if *TAKE_PANS.lock().unwrap() {
                    cx.stop_propagation();
                }
            })
    }
}

#[test]
fn a_tap_clicks_and_a_finger_scrolls_unless_a_pan_listener_takes_it() {
    Application::headless().run(|cx| {
        let win = open(cx, 300.0, 200.0, Page);
        cx.draw(win.id);
        cx.draw(win.id);
        // a tap on the first row: a click
        cx.simulate_touch(win.id, 1, TouchPhase::Down, 20.0, 10.0);
        cx.simulate_touch(win.id, 1, TouchPhase::Up, 21.0, 10.0);
        assert_eq!(*CLICKS.lock().unwrap(), 1);

        // a finger dragged up by 58 px (8 of them the touch slop) scrolls the page by 50, with
        // the pointer left elsewhere: the scroll is where the finger is
        cx.simulate_mouse_move(win.id, -10.0, -10.0);
        let at = cx.element_bounds(win.id, "top").unwrap().origin.y;
        cx.simulate_touch(win.id, 2, TouchPhase::Down, 150.0, 150.0);
        for i in 1..=4 {
            cx.simulate_touch(win.id, 2, TouchPhase::Move, 150.0, 150.0 - 14.5 * i as f32);
        }
        cx.draw(win.id);
        let moved = at - cx.element_bounds(win.id, "top").unwrap().origin.y;
        assert!((moved - 50.0).abs() < 0.5, "scrolled {moved}");
        let pans = std::mem::take(&mut *PANS.lock().unwrap());
        assert!(pans.iter().all(|p| p.2 == ScrollPhase::Drag && p.0 == 0.0), "{pans:?}");
        assert!((pans.iter().map(|p| p.1).sum::<f32>() + 50.0).abs() < 0.5, "{pans:?}");
        // rested before lifting: the end, no fling, and no click
        std::thread::sleep(std::time::Duration::from_millis(60));
        cx.simulate_touch(win.id, 2, TouchPhase::Up, 150.0, 92.0);
        assert_eq!(PANS.lock().unwrap().last().map(|p| p.2), Some(ScrollPhase::End));
        assert_eq!(*CLICKS.lock().unwrap(), 1);

        // a pan listener that stops the propagation takes the drag: nothing scrolls
        *TAKE_PANS.lock().unwrap() = true;
        let before = cx.element_bounds(win.id, "top").unwrap().origin.y;
        cx.simulate_touch(win.id, 3, TouchPhase::Down, 150.0, 150.0);
        cx.simulate_touch(win.id, 3, TouchPhase::Move, 150.0, 100.0);
        cx.simulate_touch(win.id, 3, TouchPhase::Cancel, 150.0, 100.0);
        cx.draw(win.id);
        assert_eq!(cx.element_bounds(win.id, "top").unwrap().origin.y, before);
        assert!(!PANS.lock().unwrap().is_empty());
    });
}

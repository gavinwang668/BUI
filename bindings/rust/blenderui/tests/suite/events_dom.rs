//! HTML's events: the capture pass before the bubbling one, `stopPropagation`, `mouseenter` against
//! `mouseover`, listeners of the whole window, pointer capture, the pointer events, focus and scroll.

use std::sync::Mutex;

use blenderui::prelude::*;

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn log(s: &str) {
    LOG.lock().unwrap().push(s.to_owned());
}

fn take() -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap())
}

/// A tree of three boxes inside each other, a box beside them, and a scroller.
struct Page;

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("body").size_full()
            .child(
                div().id("outer").absolute().left(px(0.0)).top(px(0.0)).w(px(200.0)).h(px(200.0))
                    .on_click_capture(|_, _, _| log("outer capture"))
                    .on_click(|_, _, _| log("outer bubble"))
                    .on_mouse_enter(|_, _, _| log("outer enter"))
                    .on_mouse_leave(|_, _, _| log("outer leave"))
                    .on_mouse_over(|_, _, _| log("outer over"))
                    .on_mouse_out(|_, _, _| log("outer out"))
                    .child(
                        div().id("inner").absolute().left(px(50.0)).top(px(50.0)).w(px(100.0)).h(px(100.0))
                            .on_click_capture(|_, _, _| log("inner capture"))
                            .on_click(|_, _, _| log("inner bubble"))
                            .on_double_click(|_, _, _| log("inner dblclick"))
                            .on_aux_click(|_, _, _| log("inner auxclick"))
                            .on_context_menu(|e, _, _| log(&format!("inner contextmenu {}", e.position.x.0 as i32)))
                            .on_pointer_down(|_, _, _| log("inner pointerdown"))
                            .on_press_drag(|e, _, _| log(&format!("inner pressdrag {:?}", e.direction)))
                            .on_any_mouse_down(|_, _, _| log("inner mousedown"))
                            .on_mouse_enter(|_, _, _| log("inner enter"))
                            .on_mouse_leave(|_, _, _| log("inner leave"))
                            .on_mouse_over(|_, _, _| log("inner over"))
                            .on_mouse_out(|_, _, _| log("inner out")),
                    ),
            )
            .child(
                div().id("side").absolute().left(px(300.0)).top(px(0.0)).w(px(100.0)).h(px(100.0))
                    .on_mouse_enter(|_, _, _| log("side enter")),
            )
    }
}

fn open(cx: &mut App) -> WindowId {
    blenderui::motion::lag_smoothing(0.0, 0.0);
    let opts = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(500.0), px(300.0)),
        })),
        ..Default::default()
    };
    let w = cx.open_window(opts, |_, cx| cx.new(|_| Page)).unwrap().id;
    cx.draw(w);
    w
}

#[test]
fn passes_and_enter_leave() {
    Application::headless().run(|cx| {
        let w = open(cx);
        // capture runs from the root down, then bubble back up
        cx.simulate_click(w, 100.0, 100.0);
        let l = take();
        let at = |s: &str| l.iter().position(|x| x == s).unwrap_or(usize::MAX);
        assert!(at("outer capture") < at("inner capture") && at("inner capture") < at("inner bubble")
                && at("inner bubble") < at("outer bubble"), "click: {l:?}");
        // entering the inner box: over bubbles (and the pointer came from outside everything), enter is
        // for each element on its own, outermost first
        assert!(l.contains(&"outer enter".to_owned()) && l.contains(&"inner enter".to_owned()), "enter: {l:?}");
        assert!(at("outer enter") < at("inner enter"), "outermost first: {l:?}");

        // from the inner box to the outer one: the inner box is left, the outer one is not
        take();
        cx.simulate_mouse_move(w, 20.0, 20.0);
        let l = take();
        assert!(l.contains(&"inner leave".to_owned()) && !l.contains(&"outer leave".to_owned()), "leave: {l:?}");
        assert!(l.contains(&"inner out".to_owned()), "out bubbles from the left box: {l:?}");
        assert!(!l.contains(&"outer enter".to_owned()), "no second enter for the parent: {l:?}");

        // out of everything
        take();
        cx.simulate_mouse_move(w, 450.0, 250.0);
        let l = take();
        assert!(l.contains(&"outer leave".to_owned()), "left it: {l:?}");
    });
}

#[test]
fn pointer_events_come_before_mouse_events() {
    Application::headless().run(|cx| {
        let w = open(cx);
        take();
        cx.simulate_press(w, 100.0, 100.0);
        let l = take();
        let at = |s: &str| l.iter().position(|x| x == s).unwrap_or(usize::MAX);
        assert!(at("inner pointerdown") < at("inner mousedown"), "{l:?}");
        cx.simulate_release(w, 100.0, 100.0);
    });
}

static WIDE: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A drag: it takes the pointer on press, and hears the moves and the release wherever they are.
struct Drag;

impl Render for Drag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("page").size_full()
            .child(
                div().id("handle").absolute().left(px(10.0)).top(px(10.0)).w(px(50.0)).h(px(50.0))
                    .on_pointer_down(|_, window, app| {
                        window.set_pointer_capture(app);
                        WIDE.lock().unwrap().push("down".into());
                    })
                    .on_pointer_move(|e, _, _| WIDE.lock().unwrap().push(format!("move {}", e.position.x.0 as i32)))
                    .on_pointer_up(|_, _, _| WIDE.lock().unwrap().push("up".into()))
                    .on_got_pointer_capture(|_, _, _| WIDE.lock().unwrap().push("got".into()))
                    .on_lost_pointer_capture(|_, _, _| WIDE.lock().unwrap().push("lost".into())),
            )
            .child(
                // hears everything in the window, in both passes
                div().id("watcher")
                    .on_mouse_event(|e: &MouseMoveEvent, phase, _, _| {
                        WIDE.lock().unwrap().push(format!("wide {:?} {}", phase, e.position.x.0 as i32));
                    }),
            )
    }
}

#[test]
fn pointer_capture_and_window_wide_listeners() {
    Application::headless().run(|cx| {
        blenderui::motion::lag_smoothing(0.0, 0.0);
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(500.0), px(300.0)),
            })),
            ..Default::default()
        };
        let w = cx.open_window(opts, |_, cx| cx.new(|_| Drag)).unwrap().id;
        cx.draw(w);
        cx.simulate_mouse_move(w, 30.0, 30.0);
        WIDE.lock().unwrap().clear();
        cx.simulate_press(w, 30.0, 30.0);
        // far outside the handle: the handle still hears the pointer
        cx.simulate_mouse_move(w, 400.0, 200.0);
        cx.simulate_release(w, 400.0, 200.0);
        let l = std::mem::take(&mut *WIDE.lock().unwrap());
        assert!(l.contains(&"got".to_owned()), "took the pointer: {l:?}");
        assert!(l.contains(&"move 400".to_owned()), "heard a move far outside: {l:?}");
        assert!(l.contains(&"up".to_owned()), "and the release: {l:?}");
        assert!(l.contains(&"lost".to_owned()), "and let go with the button: {l:?}");
        // the window-wide listener: capture pass before bubble pass, for a move anywhere
        let cap = l.iter().position(|x| x == "wide Capture 400");
        let bub = l.iter().position(|x| x == "wide Bubble 400");
        assert!(cap.is_some() && bub.is_some() && cap < bub, "both passes: {l:?}");

        // without capture, the handle hears nothing far away
        WIDE.lock().unwrap().clear();
        cx.simulate_mouse_move(w, 450.0, 250.0);
        let l = std::mem::take(&mut *WIDE.lock().unwrap());
        assert!(!l.iter().any(|x| x.starts_with("move")), "no capture, no move: {l:?}");
    });
}

struct Stop;

impl Render for Stop {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("a").size_full()
            .on_click(|_, _, _| log("a bubble"))
            .child(
                div().id("b").absolute().left(px(0.0)).top(px(0.0)).w(px(100.0)).h(px(100.0))
                    .on_click_capture(|_, _, app| {
                        log("b capture");
                        app.stop_propagation();
                    })
                    .on_click(|_, _, _| log("b bubble"))
                    .child(div().id("c").size_full().on_click(|_, _, _| log("c bubble"))),
            )
    }
}

#[test]
fn stop_propagation_in_the_capture_pass() {
    Application::headless().run(|cx| {
        blenderui::motion::lag_smoothing(0.0, 0.0);
        let w = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Stop)).unwrap().id;
        cx.draw(w);
        take();
        cx.simulate_click(w, 50.0, 50.0);
        assert_eq!(take(), ["b capture"], "stopped on the way down: nothing below or after it hears the click");
    });
}

struct Focus;

impl Render for Focus {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("page").size_full()
            .child(div().id("one").absolute().left(px(0.0)).top(px(0.0)).w(px(100.0)).h(px(100.0))
                .on_focus(|_, _, _| log("one focus")).on_blur(|_, _, _| log("one blur"))
                .on_focus_in(|_, _, _| log("one focusin")).on_focus_out(|_, _, _| log("one focusout")))
            .child(div().id("two").absolute().left(px(200.0)).top(px(0.0)).w(px(100.0)).h(px(100.0))
                .on_focus(|_, _, _| log("two focus")).on_blur(|_, _, _| log("two blur")))
            .child(
                div().id("scroller").absolute().left(px(0.0)).top(px(150.0)).w(px(100.0)).h(px(100.0))
                    .overflow_y_scroll()
                    .on_scroll(|e, _, _| log(&format!("scroll {}", e.offset.y.0 as i32)))
                    .child(div().h(px(500.0)).w(px(50.0))),
            )
    }
}

#[test]
fn focus_and_scroll() {
    Application::headless().run(|cx| {
        blenderui::motion::lag_smoothing(0.0, 0.0);
        let w = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Focus)).unwrap().id;
        cx.draw(w);
        take();
        cx.simulate_click(w, 50.0, 50.0);
        assert_eq!(take(), ["one focus", "one focusin"], "focused");
        cx.simulate_click(w, 250.0, 50.0);
        assert_eq!(take(), ["one blur", "one focusout", "two focus"], "focus moves: out of one, into two");
        cx.simulate_scroll(w, 50.0, 200.0, -1);
        let l = take();
        assert!(l.iter().any(|x| x.starts_with("scroll ") && x != "scroll 0"), "the scroller scrolled: {l:?}");
    });
}

#[test]
fn double_click_and_the_other_buttons() {
    Application::headless().run(|cx| {
        let w = open(cx);
        take();
        cx.simulate_click(w, 100.0, 100.0);
        cx.simulate_click(w, 100.0, 100.0);
        let l = take();
        assert_eq!(l.iter().filter(|x| *x == "inner bubble").count(), 2, "two clicks: {l:?}");
        assert_eq!(l.iter().filter(|x| *x == "inner dblclick").count(), 1, "one double click: {l:?}");

        take();
        cx.simulate_button(w, 100.0, 100.0, MouseButton::Right);
        let l = take();
        assert!(l.contains(&"inner auxclick".to_owned()) && l.contains(&"inner contextmenu 100".to_owned()), "{l:?}");
        assert!(!l.contains(&"inner bubble".to_owned()), "a right click isn't a click: {l:?}");
    });
}

struct Rsx;

impl Render for Rsx {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        rsx! {
            <div id="a" style="width: 100px; height: 100px" onClickCapture={|| log("rsx capture")} onClick={|| log("rsx bubble")}
                 onPointerDown={|| log("rsx pointerdown")}>
                <div id="b" style="width: 50px; height: 50px" onMouseEnter={|| log("rsx enter")} />
            </div>
        }
    }
}

#[test]
fn rsx_spellings() {
    Application::headless().run(|cx| {
        blenderui::motion::lag_smoothing(0.0, 0.0);
        let w = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Rsx)).unwrap().id;
        cx.draw(w);
        take();
        cx.simulate_click(w, 20.0, 20.0);
        let l = take();
        assert!(l.contains(&"rsx pointerdown".to_owned()) && l.contains(&"rsx enter".to_owned()), "{l:?}");
        let at = |s: &str| l.iter().position(|x| x == s).unwrap_or(usize::MAX);
        assert!(at("rsx capture") < at("rsx bubble"), "{l:?}");
    });
}

#[test]
fn press_drag_comes_once_past_the_threshold() {
    Application::headless().run(|cx| {
        let w = open(cx);
        cx.simulate_mouse_move(w, 100.0, 100.0);
        take();
        cx.simulate_press(w, 100.0, 100.0);
        cx.simulate_mouse_move(w, 102.0, 100.0); // 2 px: not a drag (Blender's threshold is 3)
        assert!(!take().iter().any(|x| x.starts_with("inner pressdrag")));
        cx.simulate_mouse_move(w, 110.0, 100.0);
        cx.simulate_mouse_move(w, 130.0, 100.0);
        let l = take();
        assert_eq!(l.iter().filter(|x| x.starts_with("inner pressdrag")).count(), 1, "once per press: {l:?}");
        assert!(l.contains(&"inner pressdrag East".to_owned()), "to the right: {l:?}");
        cx.simulate_release(w, 130.0, 100.0);
    });
}

/// A frame renders again for a hover only when something is styled by it. The built-in sheets style nothing
/// by interaction but links (`a:hover`), which the frame knows by the tag: another rule there needs the
/// frame to learn about it (`layout::div_states`).
#[test]
fn the_built_in_sheets_only_style_links_by_hover() {
    for css in [blenderui::html::UA_CSS, blenderui::html::UA_STYLED_CSS] {
        assert_eq!(css.matches(":hover").count(), css.matches("a:hover").count(), "a hover rule that isn't a link's");
        assert_eq!(css.matches(":active").count(), 0, ":active in a built-in sheet");
        assert_eq!(css.matches(":focus").count(), 0, ":focus in a built-in sheet");
    }
}

struct Hover;

impl Render for Hover {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // the first renders count: they are what a hover would make again
        RENDERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        div().size_full()
            .child(div().id("plain").absolute().left(px(0.0)).top(px(0.0)).w(px(100.0)).h(px(100.0)))
            .child(div().id("styled").absolute().left(px(200.0)).top(px(0.0)).w(px(100.0)).h(px(100.0))
                .hover(|s| s.bg(rgb(0xff0000))))
    }
}

static RENDERS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[test]
fn hover_renders_again_only_when_styled_by_it() {
    use std::sync::atomic::Ordering::Relaxed;
    Application::headless().run(|cx| {
        let w = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Hover)).unwrap().id;
        cx.draw(w);
        // with a hover style somewhere, the pointer entering an element renders again
        let before = RENDERS.load(Relaxed);
        cx.simulate_mouse_move(w, 250.0, 50.0);
        assert!(RENDERS.load(Relaxed) > before, "styled by hover: rendered again");
    });
}

struct Plain;

static PLAIN_RENDERS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

impl Render for Plain {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        PLAIN_RENDERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // a window-wide listener on the root: the elements the pointer goes over have none of their own
        div().size_full()
            .on_mouse_event(|e: &MouseMoveEvent, phase, _, _| {
                if phase.bubble() && e.position.x.0 < 100.0 {
                    log("over a");
                }
            })
            .child(div().id("a").absolute().left(px(0.0)).top(px(0.0)).w(px(100.0)).h(px(100.0)))
            .child(div().id("b").absolute().left(px(200.0)).top(px(0.0)).w(px(100.0)).h(px(100.0)))
    }
}

#[test]
fn nothing_styled_by_hover_means_no_render() {
    use std::sync::atomic::Ordering::Relaxed;
    Application::headless().run(|cx| {
        let w = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Plain)).unwrap().id;
        cx.draw(w);
        cx.simulate_mouse_move(w, 250.0, 50.0); // into the window (the root, which listens, is entered)
        take();
        let before = PLAIN_RENDERS.load(Relaxed);
        for x in [50.0, 250.0, 50.0, 250.0] {
            cx.simulate_mouse_move(w, x, 50.0);
        }
        assert_eq!(PLAIN_RENDERS.load(Relaxed), before, "moving over unstyled elements doesn't render the view");
        assert!(take().contains(&"over a".to_owned()), "and the listeners still hear the moves");
    });
}

struct Moves {
    moved: bool,
}

impl Render for Moves {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let left = if self.moved { 0.0 } else { 300.0 };
        div().size_full()
            .child(div().id("mover").absolute().left(px(left)).top(px(0.0)).w(px(100.0)).h(px(100.0))
                .on_mouse_enter(|_, _, _| log("mover enter"))
                .on_mouse_leave(|_, _, _| log("mover leave")))
    }
}

/// The layout moves an element under a pointer that stays still (a page opening under the mouse): the
/// element is entered, as in a browser, which does it on the next frame.
#[test]
fn an_element_that_arrives_under_the_pointer_is_entered() {
    Application::headless().run(|cx| {
        let h = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Moves { moved: false })).unwrap();
        let w = h.id;
        cx.draw(w);
        cx.simulate_mouse_move(w, 50.0, 50.0);
        take();
        h.update(cx, |v, _, cx| {
            v.moved = true;
            cx.notify();
        });
        cx.draw(w);
        assert_eq!(take(), ["mover enter"], "arrived under the pointer");
        h.update(cx, |v, _, cx| {
            v.moved = false;
            cx.notify();
        });
        cx.draw(w);
        assert_eq!(take(), ["mover leave"], "and left when it went");
    });
}

struct Gone {
    show: bool,
    offset: bool,
}

impl Render for Gone {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut page = div().id("page").w(px(800.0)).h(px(600.0))
            .on_window_event(EventKind::PointerCancel, |_: &PointerEvent, _, _, _| log("cancel"))
            .on_window_event(EventKind::LostPointerCapture, |_: &PointerEvent, _, _, _| log("lost"))
            .on_key_press(|e, _, _| log(&format!("keypress {}", e.keystroke.key_char.clone().unwrap_or_default())))
            .on_key_down(|_, _, _| log("keydown"))
            .child(
                div().id("scroller").absolute().left(px(300.0)).top(px(0.0)).w(px(100.0)).h(px(100.0))
                    .overflow_y_scroll()
                    .on_scroll(|e, _, _| log(&format!("scroll {}", e.offset.y.0 as i32)))
                    .child(div().h(px(500.0)).w(px(50.0))),
            );
        if self.show {
            page = page.child(
                div().id("handle").absolute().left(px(0.0)).top(px(0.0)).w(px(100.0)).h(px(100.0))
                    .on_pointer_down(|_, window, app| window.set_pointer_capture(app))
                    .on_focus(|_, _, _| log("focus")),
            );
        }
        let _ = self.offset;
        page
    }
}

#[test]
fn removed_elements_let_go_and_other_scrolls_and_keypress() {
    Application::headless().run(|cx| {
        let h = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Gone { show: true, offset: false })).unwrap();
        let w = h.id;
        cx.draw(w);
        take();

        // a scroll from code (not the wheel) is a scroll event too
        h.update(cx, |_, window, cx| {
            window.set_scroll_offset("scroller", 0.0, 40.0);
            cx.notify();
        });
        cx.draw(w);
        assert!(take().contains(&"scroll 40".to_owned()), "set_scroll_offset");

        // the element that holds the pointer and the focus goes away: both are let go
        cx.simulate_press(w, 50.0, 50.0);
        take();
        h.update(cx, |v, _, cx| {
            v.show = false;
            cx.notify();
        });
        cx.draw(w);
        let l = take();
        assert!(l.contains(&"cancel".to_owned()) && l.contains(&"lost".to_owned()), "pointer cancelled: {l:?}");
        assert_eq!(h.update(cx, |_, window, _| window.focused_id().is_none()), Some(true), "no focus on what is gone");
        cx.simulate_release(w, 50.0, 50.0);

        // keypress follows keydown for a key that types, and not for a shortcut
        cx.simulate_click(w, 200.0, 200.0); // focus goes to the page's area
        take();
        cx.simulate_key(w, "a", "a");
        assert_eq!(take(), ["keydown", "keypress a"]);
        cx.simulate_key_with(w, "a", "a", true, false);
        assert_eq!(take(), ["keydown"], "ctrl+a isn't typing");
    });
}

static OVER: Mutex<Vec<(i32, i32)>> = Mutex::new(Vec::new());

/// A drag that crosses one of Blender's widgets.
struct DragOverWidget;

impl Render for DragOverWidget {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("page").size_full()
            .child(
                div().id("grip").absolute().left(px(10.0)).top(px(10.0)).w(px(50.0)).h(px(50.0))
                    .on_pointer_down(|_, window, app| window.set_pointer_capture(app))
                    .on_pointer_move(|e, _, _| OVER.lock().unwrap().push((e.position.x.0 as i32, e.position.y.0 as i32))),
            )
            .child(div().absolute().left(px(200.0)).top(px(100.0)).child(Checkbox().id("cb")))
    }
}

/// The pointer coming back onto the page from a widget is an enter with no position: it moves
/// nothing (it used to be a move to (0, 0) mid-drag).
#[test]
fn a_drag_across_a_widget_never_jumps_to_the_corner() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(500.0), px(300.0)),
            })),
            ..Default::default()
        };
        let w = cx.open_window(opts, |_, cx| cx.new(|_| DragOverWidget)).unwrap().id;
        cx.draw(w);
        let cb = cx.element_bounds(w, "cb").expect("the checkbox");
        let (cx_, cy_) = (cb.origin.x + cb.size.width / 2.0, cb.origin.y + cb.size.height / 2.0);
        OVER.lock().unwrap().clear();
        cx.simulate_press(w, 30.0, 30.0);
        for (x, y) in [(100.0, 60.0), (cx_, cy_), (cx_ + 2.0, cy_), (300.0, 200.0), (320.0, 210.0)] {
            cx.simulate_mouse_move(w, x, y);
        }
        cx.simulate_release(w, 320.0, 210.0);
        let moves = OVER.lock().unwrap().clone();
        assert!(!moves.is_empty(), "the drag is heard");
        assert!(!moves.contains(&(0, 0)), "no move to the corner: {moves:?}");
    });
}

static RESETS: Mutex<Vec<f64>> = Mutex::new(Vec::new());

/// A page that acts on Backspace itself, over one of Blender's sliders.
struct TakesKeys {
    take: bool,
}

impl Render for TakesKeys {
    fn render(&mut self, w: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // keys go to the focused element: the page's, as an app focuses its root
        if w.focused_id().is_none() {
            w.focus_id("root");
        }
        let take = self.take;
        div().id("root").size_full().attr("tabindex", "0")
            .on_key_down(move |e: &KeyDownEvent, _, cx| {
                if take && e.keystroke.key == "BackSpace" {
                    cx.prevent_default();
                }
            })
            .child(NumberSlider().id("s").min(0.0).max(10.0).value("7").w(px(200.0))
                .on_change(|e: &ChangeEvent, _, _| RESETS.lock().unwrap().push(e.number)))
    }
}

/// A key a page listener takes (`prevent_default`) is not Blender's too (Backspace over a slider
/// resets it): in a headless run as in a window, the listener hears the key while the kit delivers it.
#[test]
fn a_key_the_page_takes_is_not_the_widgets() {
    for take in [false, true] {
        Application::headless().run(move |cx| {
            let opts = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(400.0), px(200.0)),
                })),
                ..Default::default()
            };
            let w = cx.open_window(opts, |_, cx| cx.new(|_| TakesKeys { take })).unwrap().id;
            cx.draw(w);
            let b = cx.element_bounds(w, "s").expect("the slider");
            let y = b.origin.y + b.size.height / 2.0;
            // away from its default first (Backspace resets to it)
            cx.simulate_drag(w, (b.origin.x + 150.0, y), (b.origin.x + 30.0, y));
            cx.draw(w);
            RESETS.lock().unwrap().clear();
            cx.simulate_mouse_move(w, b.origin.x + 30.0, y);
            cx.simulate_key(w, "", "BackSpace");
            cx.draw(w);
            let changed = !RESETS.lock().unwrap().is_empty();
            assert_eq!(changed, !take, "take {take}: the slider reset {changed}");
        });
    }
}

/// Two radios kept in the app's state (`view`), and a box `:checked` styles after the first.
struct Views {
    grid: bool,
}

impl Render for Views {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
            .child(style().child("#mark { width: 10px; height: 10px } #list:checked ~ #mark { width: 50px }"))
            .child(Radio().id("list").attr("name", "view").checked(!self.grid)
                .on_change(cx.listener(|this, e: &ChangeEvent, _, cx| if e.checked { this.grid = false; cx.notify(); })))
            .child(Radio().id("grid").attr("name", "view").checked(self.grid)
                .on_change(cx.listener(|this, e: &ChangeEvent, _, cx| if e.checked { this.grid = true; cx.notify(); })))
            .child(div().id("mark"))
            .child(div().id("code").w(px(40.0)).h(px(40.0))
                .on_click(cx.listener(|this, _, _, cx| { this.grid = false; cx.notify(); })))
    }
}

/// A control kept in the app's state follows it after the user clicked it: the app's next
/// `checked` wins over the click (it used to keep the user's state for good).
#[test]
fn a_controlled_radio_follows_the_app_after_a_click() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(400.0), px(200.0)),
            })),
            ..Default::default()
        };
        let w = cx.open_window(opts, |_, cx| cx.new(|_| Views { grid: false })).unwrap().id;
        cx.draw(w);
        let mark = |cx: &mut App| cx.element_bounds(w, "mark").expect("mark").size.width;
        assert_eq!(mark(cx), 50.0, "List starts checked");
        let g = cx.element_bounds(w, "grid").expect("grid");
        cx.simulate_click(w, g.origin.x + g.size.width / 2.0, g.origin.y + g.size.height / 2.0);
        cx.draw(w);
        assert_eq!(mark(cx), 10.0, "the user picked Grid");
        let c = cx.element_bounds(w, "code").expect("code");
        cx.simulate_click(w, c.origin.x + 5.0, c.origin.y + 5.0);
        cx.draw(w);
        assert_eq!(mark(cx), 50.0, "the app set List again: it shows");
    });
}

/// A composer whose field is a new one after each send (a new id: the value is cleared).
struct Composer {
    sent: Vec<String>,
}

impl Render for Composer {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if w.focused_id().is_none() {
            w.focus_id("root");
        }
        div().id("root").size_full().attr("tabindex", "0")
            .child(TextField().id(format!("field{}", self.sent.len())).w(px(200.0))
                .on_change(cx.listener(|this, e: &ChangeEvent, _, cx| {
                    if !e.value.is_empty() {
                        this.sent.push(e.value.clone());
                        cx.notify();
                    }
                })))
    }
}

/// The Enter that sends takes the field away (a render replaces it) while the kit is still handling
/// that key: the render waits until the kit is done, as a window's next frame does.
#[test]
fn a_field_its_own_enter_replaces_survives() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(400.0), px(200.0)),
            })),
            ..Default::default()
        };
        let h = cx.open_window(opts, |_, cx| cx.new(|_| Composer { sent: Vec::new() })).unwrap();
        let w = h.id;
        for (k, msg) in ["hi", "again"].into_iter().enumerate() {
            cx.draw(w);
            let b = cx.element_bounds(w, &format!("field{k}")).expect("the field");
            cx.simulate_click(w, b.origin.x + 20.0, b.origin.y + b.size.height / 2.0);
            for c in msg.chars() {
                let s = c.to_string();
                cx.simulate_key(w, &s, &s);
            }
            cx.simulate_key(w, "", "Return");
        }
        cx.draw(w);
        let sent = h.update(cx, |v, _, _| v.sent.clone()).unwrap_or_default();
        assert_eq!(sent, ["hi", "again"]);
    });
}

struct Adder {
    added: usize,
    keys: String,
}

impl Render for Adder {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if w.focused_id().is_none() {
            w.focus_id("root");
        }
        div().id("root").size_full().attr("tabindex", "0")
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, _| this.keys.push_str(&e.keystroke.key)))
            .child(Button().id(format!("add{}", self.added)).child("Add").w(px(120.0))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.added += 1;
                    cx.notify();
                })))
    }
}

/// The button clicked is rebuilt away (a new id): the keys typed next, before the pointer moves,
/// still reach the page under the pointer, as Blender sends a key to the region under the cursor.
#[test]
fn keys_after_a_click_that_rebuilds_the_button_reach_the_page() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(400.0), px(200.0)),
            })),
            ..Default::default()
        };
        let h = cx.open_window(opts, |_, cx| cx.new(|_| Adder { added: 0, keys: String::new() })).unwrap();
        let w = h.id;
        cx.draw(w);
        let b = cx.element_bounds(w, "add0").expect("the button");
        cx.simulate_click(w, b.origin.x + b.size.width / 2.0, b.origin.y + b.size.height / 2.0);
        for c in ["o", "k"] {
            cx.simulate_key(w, c, c);
        }
        cx.draw(w);
        let (added, keys) = h.update(cx, |v, _, _| (v.added, v.keys.clone())).unwrap_or_default();
        assert_eq!((added, keys.as_str()), (1, "ok"));
    });
}

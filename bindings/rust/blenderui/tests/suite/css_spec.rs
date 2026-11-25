//! Cascade, selector and value rules checked against the CSS specifications: each page paints a
//! box red when the rule holds.

use blenderui::prelude::*;

struct Page(&'static str, fn() -> Div);

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        (self.1)().child(style().child(self.0))
    }
}

/// The color (0xRRGGBB) at the middle of the `k`th 10 x 10 box down the page.
fn colors(css: &'static str, body: fn() -> Div, boxes: usize) -> Vec<u32> {
    let mut out = Vec::new();
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(100.0), px(200.0)),
            })),
            ..Default::default()
        };
        let win = cx.open_window(opts, |_, cx| cx.new(|_| Page(css, body))).unwrap();
        let img = cx.draw(win.id);
        let s = img.0 as f32 / 100.0;
        out = (0..boxes).map(|k| img.2[((10 * k + 5) as f32 * s) as usize * img.0 + (5.0 * s) as usize]).collect();
    });
    out
}

const RED: u32 = 0xff0000;

#[test]
fn important_reverses_layers_and_takes_any_spelling() {
    let got = colors(
        "html, body { margin: 0; background: #000 }
         body > div { width: 10px; height: 10px }
         @layer base { #a { background: #f00 !important } }
         #a { background: #00f !important }
         #b { background: #f00 ! important }
         #b { background: #00f }
         #c { background: #f00 !IMPORTANT }
         #c { background: #00f }",
        || body().child(div().id("a")).child(div().id("b")).child(div().id("c")),
        3,
    );
    assert_eq!(got, [RED; 3], "a layered !important beats an unlayered one; `! important` and `!IMPORTANT` count");
}

#[test]
fn has_counts_its_most_specific_argument() {
    // `.p:has(#dc)` is (1, 1, 0): it beats the later `.p.q` (0, 2, 0)
    let got = colors(
        "html, body { margin: 0; background: #000 }
         body > div { width: 10px; height: 10px }
         .p:has(#dc) { background: #f00 }
         .p.q { background: #00f }",
        || body().child(div().class("p q").child(div().id("dc"))),
        1,
    );
    assert_eq!(got, [RED]);
}

#[test]
fn scope_is_the_scoping_root() {
    let got = colors(
        "html, body { margin: 0; background: #000 }
         body > div { width: 10px; height: 10px }
         @scope (#s) { :scope { background: #f00 } }",
        || body().child(div().id("s")),
        1,
    );
    assert_eq!(got, [RED]);
}

#[test]
fn empty_sees_the_children() {
    let got = colors(
        "html, body { margin: 0; background: #000 }
         body > div { width: 10px; height: 10px; color: #f00 }
         div { background: #f00 }
         div:empty { background: #00f }
         #e:empty { background: #f00 }",
        || body().child(div().child("x")).child(div().child(span())).child(div().id("e")),
        3,
    );
    assert_eq!(got, [RED; 3], "text and element children are not empty; no children is");
}

#[test]
fn has_reaches_following_siblings() {
    let got = colors(
        "html, body { margin: 0; background: #000 }
         body > div { width: 10px; height: 10px }
         #b:has(+ .y) { background: #f00 }
         #c:has(~ .z) { background: #f00 }
         #d { background: #f00 }
         #d:has(+ .nope) { background: #00f }",
        || body().child(div().id("b")).child(div().class("y"))
            .child(div().id("c")).child(div()).child(div().class("z"))
            .child(div().id("d")).child(div()),
        8,
    );
    assert_eq!((got[0], got[2], got[5]), (RED, RED, RED));
}

#[test]
fn rgb_channels_clamp() {
    let got = colors(
        "html, body { margin: 0; background: #000 }
         body > div { width: 10px; height: 10px }
         div { background: rgb(300 -20 0) }",
        || body().child(div()),
        1,
    );
    assert_eq!(got, [RED]);
}

#[test]
fn rem_is_the_root_font_size() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(200.0), px(100.0)),
            })),
            ..Default::default()
        };
        let win = cx.open_window(opts, |_, cx| cx.new(|_| Page(
            "html { font-size: 10px } html, body { margin: 0 }
             div { height: 4px }
             #a { width: calc(2rem + 6px) }
             #b { font-size: 20px; width: calc(1em + 1rem) }
             #c { width: 1rlh }",
            || body().child(div().id("a")).child(div().id("b")).child(div().id("c")),
        ))).unwrap();
        cx.draw(win.id);
        let mut w = |id: &str| cx.element_bounds(win.id, id).unwrap().size.width;
        assert_eq!((w("a"), w("b"), w("c")), (26.0, 30.0, 13.0));
    });
}

/// The Rust parser's specificity against the Lean model's: `#eval vectors` in
/// `proofs/Bui/Specificity.lean` prints these (the model's theorems cover every rule they use).
#[test]
fn specificity_matches_the_lean_model() {
    let cases: [(&str, (u16, u16, u16)); 13] = [
        ("div", (0, 0, 1)),
        ("p#a.b", (1, 1, 1)),
        ("ul li.x :hover", (0, 2, 2)),
        (".p:has(#d)", (1, 1, 0)),
        (":has(.x, #y.z)", (1, 1, 0)),
        (":is(p, #q)", (1, 0, 0)),
        (":not(.a, .b.c)", (0, 2, 0)),
        (":where(#w).k", (0, 1, 0)),
        (":nth-child(2n+1)", (0, 1, 0)),
        (":nth-child(2n+1 of .x, #y)", (1, 1, 0)),
        ("p::before", (0, 0, 2)),
        ("*[data-x]", (0, 1, 0)),
        (":is(:has(#deep))", (1, 0, 0)),
    ];
    for (sel, want) in cases {
        let got = blenderui::css::Selector::parse(sel).unwrap_or_else(|| panic!("{sel} parses")).specificity;
        assert_eq!(got, want, "{sel}");
    }
}

#[test]
fn nth_last_child_of_counts_the_following_siblings() {
    let got = colors(
        "html, body { margin: 0; background: #000 }
         body > div { width: 10px; height: 10px; background: #00f }
         #a:nth-last-child(4 of .x) { background: #f00 }
         #b:nth-last-child(3 of .x) { background: #f00 }
         #c:nth-last-child(1 of body > .x) { background: #f00 }",
        || body().child(div().id("a").class("x")).child(div().id("b").class("x"))
            .child(div().class("x")).child(div()).child(div().id("c").class("x")),
        5,
    );
    // #a and #b count the .x after them (not the plain div); #c through a combinator naming its parent
    assert_eq!((got[0], got[1], got[4]), (RED, RED, RED));
}

/// Boxes lay out at fractional positions, as in a browser (`getBoundingClientRect` gives 47.5),
/// and paint with each edge on its nearest device pixel: no half-blended column at 47.5.
#[test]
fn fractional_layout_with_snapped_edges() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(200.0), px(20.0)),
            })),
            ..Default::default()
        };
        let win = cx.open_window(opts, |_, cx| cx.new(|_| Page(
            "html, body { margin: 0; background: #000 }
             #row { display: flex; height: 10px; gap: 10px }
             #g1 { flex: 1; background: #f00 } #g2 { flex: 3; background: #00f }",
            || body().child(div().id("row").child(div().id("g1")).child(div().id("g2"))),
        ))).unwrap();
        let img = cx.draw(win.id);
        let w = |cx: &mut App, id: &str| cx.element_bounds(win.id, id).unwrap();
        assert_eq!(w(cx, "g1").size.width, 47.5);
        assert_eq!((w(cx, "g2").origin.x, w(cx, "g2").size.width), (57.5, 142.5));
        let s = img.0 / 200;
        let at = |x: usize| img.2[5 * s * img.0 + x * s];
        assert_eq!((at(47), at(48), at(57), at(58)), (RED, 0x000000, 0x000000, 0x0000ff),
                   "edges at 47.5 and 57.5 round to device pixels 48 and 58");
    });
}

/// `subgrid`: cards whose headers differ in height still line their bodies up, because the cards'
/// rows are the grid's, sized by every card's items; a card's padding is added at its edges.
#[test]
fn subgrid_rows_line_up_across_cards() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(300.0), px(200.0)),
            })),
            ..Default::default()
        };
        fn card(n: usize, head: &'static str) -> Div {
            div().id(format!("c{n}")).class("card")
                .child(div().id(format!("h{n}")).style(head))
                .child(div().id(format!("b{n}")).class("body"))
        }
        let win = cx.open_window(opts, |_, cx| cx.new(|_| Page(
            "html, body { margin: 0 }
             #grid { display: grid; grid-template-columns: repeat(3, 100px) }
             .card { display: grid; grid-row: span 2; grid-template-rows: subgrid; padding: 2px }
             .body { height: 15px }",
            || body().child(div().id("grid")
                .child(card(0, "height: 10px")).child(card(1, "height: 30px")).child(card(2, "height: 20px"))),
        ))).unwrap();
        cx.draw(win.id);
        let b = |cx: &mut App, id: &str| cx.element_bounds(win.id, id).unwrap();
        for n in 0..3 {
            let (head, body, card) = (b(cx, &format!("h{n}")), b(cx, &format!("b{n}")), b(cx, &format!("c{n}")));
            assert_eq!(head.origin.y, 2.0, "card {n}: its padding is above its header");
            assert_eq!(body.origin.y, 32.0, "card {n}: the first row is the tallest header (30) plus the padding");
            assert_eq!((card.origin.y, card.size.height), (0.0, 49.0), "card {n}: the two rows it spans");
            assert_eq!(card.origin.x, 100.0 * n as f32);
        }
    });
}

/// `subgrid` columns: a form's rows share the label column, sized by the widest label; their items
/// auto-place inside the row, and a nested element moves with its item.
#[test]
fn subgrid_columns_share_the_label_column() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(200.0), px(100.0)),
            })),
            ..Default::default()
        };
        fn row(n: usize, label: &'static str) -> Div {
            div().class("row")
                .child(div().id(format!("l{n}")).style(label))
                .child(div().id(format!("f{n}")).class("field").child(div().id(format!("i{n}")).class("inner")))
        }
        let win = cx.open_window(opts, |_, cx| cx.new(|_| Page(
            "html, body { margin: 0 }
             #form { display: grid; grid-template-columns: auto 1fr; column-gap: 4px }
             .row { display: grid; grid-column: 1 / -1; grid-template-columns: subgrid; height: 10px }
             .inner { margin-left: 3px; width: 5px; height: 5px }",
            || body().child(div().id("form").child(row(0, "width: 20px")).child(row(1, "width: 50px"))),
        ))).unwrap();
        cx.draw(win.id);
        let b = |cx: &mut App, id: &str| cx.element_bounds(win.id, id).unwrap();
        for n in 0..2 {
            let (field, inner) = (b(cx, &format!("f{n}")), b(cx, &format!("i{n}")));
            assert_eq!((field.origin.x, field.size.width), (54.0, 146.0), "row {n}: after the widest label and the gap");
            assert_eq!((inner.origin.x, inner.origin.y), (57.0, 10.0 * n as f32), "row {n}: the nested box moves with it");
        }
    });
}

/// Anchor positioning: `anchor()` and `anchor-size()` follow the anchor's box, `position-area`
/// puts a box in a region around it (centred on it across a spanned axis), and
/// `position-try-fallbacks: flip-block` moves a box that would overflow to the other side.
#[test]
fn anchored_boxes_follow_their_anchors() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(200.0), px(100.0)),
            })),
            ..Default::default()
        };
        let win = cx.open_window(opts, |_, cx| cx.new(|_| Page(
            "html, body { margin: 0 }
             #btn { anchor-name: --btn; margin: 40px 0 0 30px; width: 50px; height: 20px }
             #low { anchor-name: --low; position: absolute; top: 80px; left: 100px; width: 10px; height: 10px }
             #tip { position: absolute; position-anchor: --btn; top: anchor(bottom); left: anchor(left);
                    width: anchor-size(width); height: 10px }
             #pop { position: absolute; position-anchor: --btn; position-area: top; width: 20px; height: 10px }
             #nudge { position: absolute; top: calc(anchor(--btn top) - 4px); left: anchor(--btn right, 0px);
                      width: 5px; height: 5px }
             #flip { position: absolute; position-anchor: --low; top: anchor(bottom); left: anchor(left);
                     width: 10px; height: 30px; position-try-fallbacks: flip-block }",
            || body().child(div().id("btn")).child(div().id("low")).child(div().id("tip"))
                .child(div().id("pop")).child(div().id("nudge")).child(div().id("flip")),
        ))).unwrap();
        // the first layout has no anchors to read: they are placed by the next
        for _ in 0..3 {
            cx.draw(win.id);
        }
        let b = |cx: &mut App, id: &str| {
            let r = cx.element_bounds(win.id, id).unwrap();
            (r.origin.x, r.origin.y, r.size.width, r.size.height)
        };
        assert_eq!(b(cx, "tip"), (30.0, 60.0, 50.0, 10.0), "under the button, as wide as it");
        assert_eq!(b(cx, "pop"), (45.0, 30.0, 20.0, 10.0), "above it, centred on it");
        assert_eq!(b(cx, "nudge"), (80.0, 36.0, 5.0, 5.0), "calc(anchor() - 4px)");
        assert_eq!(b(cx, "flip"), (100.0, 50.0, 10.0, 30.0), "no room below: flipped above");
    });
}

struct Hero {
    moved: bool,
}

impl Render for Hero {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        body()
            .child(div().id("hero").class(if self.moved { "b" } else { "a" }))
            .child(style().child("html, body { margin: 0; background: #fff }
                #hero { view-transition-name: hero; background: #f00; position: absolute; top: 0 }
                .a { left: 0; width: 20px; height: 20px } .b { left: 100px; width: 40px; height: 40px }
                ::view-transition-group(*) { animation-duration: 1s; animation-timing-function: linear }"))
    }
}

/// A view transition: the named box's group moves and grows from its old box to its new one
/// (the live box is hidden meanwhile), and the page shows the new state once it ends.
#[test]
fn view_transition_moves_the_named_box() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(200.0), px(100.0)),
            })),
            ..Default::default()
        };
        let h = cx.open_window(opts, |_, cx| cx.new(|_| Hero { moved: false })).unwrap();
        let win = h.id;
        cx.draw(win);
        h.update(cx, |v, window, cx| {
            window.start_view_transition(cx);
            v.moved = true;
            cx.notify();
        });
        cx.draw(win);
        cx.advance_clock(std::time::Duration::from_millis(500));
        let img = cx.draw(win);
        let s = img.0 / 200;
        let at = |img: &(usize, usize, Vec<u32>), x: usize, y: usize| img.2[y * s * img.0 + x * s] & 0xffffff;
        assert_eq!(at(&img, 65, 15), RED, "halfway: the group is at (50, 0), 30 x 30");
        assert_eq!(at(&img, 110, 35), 0xffffff, "the live box is hidden while it moves");
        cx.advance_clock(std::time::Duration::from_millis(600));
        let img = cx.draw(win);
        assert_eq!(at(&img, 110, 35), RED, "over: the box is where the new state puts it");
        assert_eq!(at(&img, 65, 15), 0xffffff);
    });
}

/// Only CSS white space (space, tab, newline ...) separates words: a no-break or ideographic space
/// is part of its word, even right after a collapsed space (this used to slice inside it and panic).
#[test]
fn no_break_spaces_stay_inside_words() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(200.0), px(50.0)),
            })),
            ..Default::default()
        };
        let win = cx.open_window(opts, |_, cx| cx.new(|_| Page(
            "html, body { margin: 0 } #p { width: 30px }",
            || body().child(div().id("p").child(span().child("x")).child(" \u{3000}y\u{a0}z\u{202f}w\u{2007}v")),
        ))).unwrap();
        cx.draw(win.id);
        let p = cx.element_bounds(win.id, "p").unwrap();
        assert_eq!(p.size.height, 42.0, "two lines: `x`, then the no-break run as one word");
    });
}

/// Animating `box-shadow` from `none` to a shadow pads the empty list (this indexed the empty one).
#[test]
fn box_shadow_animates_from_none() {
    Application::headless().run(|cx| {
        let win = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Page(
            "#t { width: 10px; height: 10px; animation: grow 2s linear both }
             @keyframes grow { from { box-shadow: none } to { box-shadow: lime 10px 0 } }",
            || body().child(div().id("t")),
        ))).unwrap();
        cx.draw(win.id);
        cx.advance_clock(std::time::Duration::from_millis(500));
        cx.draw(win.id);
    });
}

/// The bounds of the elements `ids` on a `w` x `h` page.
fn bounds_of(css: &'static str, body: fn() -> Div, (w, h): (f32, f32), ids: &[&str]) -> Vec<Bounds<f32>> {
    let mut out = Vec::new();
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.0), px(0.0)), size: size(px(w), px(h)) })),
            ..Default::default()
        };
        let win = cx.open_window(opts, |_, cx| cx.new(|_| Page(css, body))).unwrap();
        cx.draw(win.id);
        out = ids.iter().map(|id| cx.element_bounds(win.id, id).unwrap()).collect();
    });
    out
}

/// Grid lanes (`display: grid-lanes`): each item goes to the lane where it ends up highest (the
/// first from the lane after the last item's, within `fit-tolerance`), and the container is as tall
/// as its longest lane.
#[test]
fn grid_lanes_stack_items_in_the_shortest_lane() {
    let b = bounds_of(
        "html, body { margin: 0 }
         #g { display: grid-lanes; grid-template-columns: repeat(3, 100px); fit-tolerance: 0 }",
        || body().child(div().id("g")
            .child(div().id("i0").style("height: 50px")).child(div().id("i1").style("height: 20px"))
            .child(div().id("i2").style("height: 30px")).child(div().id("i3").style("height: 10px"))
            .child(div().id("i4").style("height: 10px; grid-column: 1"))),
        (300.0, 200.0), &["g", "i0", "i1", "i2", "i3", "i4"],
    );
    let at = |k: usize| (b[k].origin.x, b[k].origin.y);
    assert_eq!((at(1), at(2), at(3)), ((0.0, 0.0), (100.0, 0.0), (200.0, 0.0)), "the first row of lanes");
    assert_eq!(at(4), (100.0, 20.0), "the shortest lane is the second");
    assert_eq!(at(5), (0.0, 50.0), "a definite lane stays");
    assert_eq!(b[0].size.height, 60.0, "the longest lane");
}

/// `grid-lanes-pack: dense` fills the highest gap an item fits in, above the ends of the lanes.
#[test]
fn grid_lanes_dense_packing_fills_gaps() {
    let b = bounds_of(
        "html, body { margin: 0 }
         #g { display: grid-lanes; grid-template-columns: repeat(3, 50px); grid-lanes-pack: dense; fit-tolerance: 0 }
         #g > div { height: 20px }",
        || body().child(div().id("g")
            .child(div()).child(div()).child(div().style("grid-column: span 3"))
            .child(div().id("last"))),
        (300.0, 200.0), &["last"],
    );
    assert_eq!((b[0].origin.x, b[0].origin.y), (100.0, 0.0), "the gap beside the first two");
}

/// Two `<br>`s in a row make an empty line, as tall as a line.
#[test]
fn consecutive_line_breaks_make_an_empty_line() {
    let b = bounds_of(
        "html, body { margin: 0 } #p { font-size: 10px; line-height: 20px }",
        || body().child(div().id("p").child("a").child(br()).child(br()).child("b")),
        (200.0, 100.0), &["p"],
    );
    assert_eq!(b[0].size.height, 60.0, "a, the empty line, b");
}

/// `white-space: pre-wrap` keeps every space between words (and `break-spaces` too).
#[test]
fn pre_wrap_keeps_every_space() {
    let b = bounds_of(
        "html, body { margin: 0 } span { display: inline-block } #one, #three { white-space: pre-wrap }
         #broken { white-space: break-spaces }",
        || body().child(span().id("one").child("a b")).child(div()).child(span().id("three").child("a   b"))
            .child(div()).child(span().id("broken").child("a   b")),
        (200.0, 100.0), &["one", "three", "broken"],
    );
    assert!(b[1].size.width > b[0].size.width + 1.0, "three spaces are wider than one");
    // (each space after the first is a piece of its own there, each rounded up to a pixel)
    assert!((b[1].size.width - b[2].size.width).abs() <= 1.0, "break-spaces keeps them too");
}

/// A block with an `aspect-ratio` is at least as tall as its content (CSS Sizing 4 §5.2).
#[test]
fn aspect_ratio_box_grows_to_its_content() {
    let b = bounds_of(
        "html, body { margin: 0 } #r { width: 100px; aspect-ratio: 2 / 1 } #small { width: 100px; aspect-ratio: 2 / 1 }",
        || body().child(div().id("r").child(div().style("height: 80px"))).child(div().id("small").child(div().style("height: 10px"))),
        (200.0, 300.0), &["r", "small"],
    );
    assert_eq!(b[0].size.height, 80.0, "its content is taller than the ratio's 50");
    assert_eq!(b[1].size.height, 50.0, "the ratio's height");
}

/// A uniform border is painted inside the border box, not half a width further in.
#[test]
fn borders_paint_inside_the_border_box() {
    Application::headless().run(|cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.0), px(0.0)), size: size(px(100.0), px(100.0)) })),
            ..Default::default()
        };
        let win = cx.open_window(opts, |_, cx| cx.new(|_| Page(
            "html, body { margin: 0; background: white } #b { margin: 10px; width: 40px; height: 40px; border: 8px solid black }",
            || body().child(div().id("b")),
        ))).unwrap();
        let (w, _, px) = cx.draw(win.id);
        let at = |x: usize, y: usize| px[y * w + x] & 0xff_ffff;
        assert_eq!(at(10, 30), 0, "the border's outer edge is the box's");
        assert_eq!(at(17, 30), 0, "8px wide");
        assert_eq!(at(18, 30), 0xff_ffff, "then the padding box");
    });
}

/// An SVG background without a natural size is sized from its ratio (`contain` for `auto`), or
/// fills the area without one (CSS Images 3 §5.3).
#[test]
fn svg_backgrounds_size_from_their_ratio() {
    use blenderui::style::{BgSize, ImageStyle, Natural};
    let auto = ImageStyle::default();
    let ratio = Natural { width: None, height: None, ratio: Some(0.5) };
    assert_eq!(auto.tile((100.0, 300.0), ratio), (100.0, 200.0), "contain");
    assert_eq!(auto.tile((100.0, 300.0), Natural::default()), (100.0, 300.0), "the area");
    let wide = ImageStyle { bg_size: BgSize::Cover, ..ImageStyle::default() };
    assert_eq!(wide.tile((100.0, 300.0), ratio), (150.0, 300.0), "cover");
    let width_only = Natural { width: Some(8.0), height: None, ratio: None };
    assert_eq!(auto.tile((100.0, 300.0), width_only), (8.0, 300.0), "its width, the area's height");
}

/// Grid lanes: an auto-placed item counts towards the size of every lane it could go in, so a wide
/// first item widens the second lane too (CSS Grid 3 § "Grid Axis Track Sizing").
#[test]
fn grid_lanes_size_every_lane_from_auto_items() {
    let b = bounds_of(
        "html, body { margin: 0 }
         #g { display: grid-lanes; grid-template-columns: auto auto; width: max-content; fit-tolerance: 0 }",
        || body().child(div().id("g")
            .child(div().id("wide").style("width: 100px; height: 10px"))
            .child(div().id("narrow").style("width: 20px; height: 10px"))),
        (400.0, 200.0), &["g", "narrow"],
    );
    assert_eq!(b[1].origin.x, 100.0, "the second lane is as wide as the first item");
    assert_eq!(b[0].size.width, 200.0, "both lanes are");
}

/// A subgrid sharing only its columns keeps rows of its own: its items stack in them, sized by its
/// parent's columns.
#[test]
fn subgrid_in_one_axis_keeps_its_own_rows() {
    let b = bounds_of(
        "html, body { margin: 0 }
         #g { display: grid; grid-template-columns: 60px 40px }
         #s { display: grid; grid-column: 1 / 3; grid-template-columns: subgrid; row-gap: 5px }
         #s > div { height: 10px }",
        || body().child(div().id("g").child(div().id("s")
            .child(div().id("a")).child(div().id("b")).child(div().id("c")))),
        (300.0, 200.0), &["s", "b", "c"],
    );
    assert_eq!((b[1].origin.x, b[1].size.width), (60.0, 40.0), "the second column is the parent's");
    assert_eq!((b[2].origin.x, b[2].origin.y), (0.0, 15.0), "the third item starts its second row");
    assert_eq!(b[0].size.height, 25.0, "two rows and the gap");
}

/// A class the page's own CSS styles is the page's: it doesn't also compile as a Tailwind utility
/// (`hidden` would be `display: none`).
#[test]
fn page_classes_win_over_tailwind() {
    let b = bounds_of(
        "html, body { margin: 0 } .hidden { width: 30px; height: 10px }",
        || body().child(div().id("h").class("hidden")),
        (200.0, 100.0), &["h"],
    );
    assert_eq!((b[0].size.width, b[0].size.height), (30.0, 10.0));
}

/// `min-width: min-content` holds a box at its content's narrowest, past its `width`; `contain:
/// size` sizes a box by its `contain-intrinsic-size`, whatever it holds.
#[test]
fn min_content_minimums_and_size_containment() {
    let b = bounds_of(
        "html, body { margin: 0 }
         #m { width: 10px; min-width: min-content }
         #c { float: left; contain: size; contain-intrinsic-size: 40px 30px }",
        || body().child(div().id("m").child(div().style("width: 50px; height: 5px")))
            .child(div().id("c").child(div().style("width: 200px; height: 200px"))),
        (300.0, 300.0), &["m", "c"],
    );
    assert_eq!(b[0].size.width, 50.0, "as wide as its content");
    assert_eq!((b[1].size.width, b[1].size.height), (40.0, 30.0), "its intrinsic size, not its content's");
}

/// Inline layout: Chinese wraps between characters (UAX #14), `word-break: keep-all` keeps it on one
/// line; a `<span>`'s padding takes room on its line; spaces at a line's end hang (they don't make a
/// line that fits wrap).
/// An inline-block keeps its own height when its text gives it a baseline.
#[test]
fn inline_block_keeps_its_size() {
    let b = bounds_of(
        "html, body { margin: 0 } #c { position: relative; width: 400px; height: 150px }
         #a { display: inline-block; width: 100px; height: 100px }",
        || body().child(div().id("c").child(div().id("a").child("anchor1"))),
        (800.0, 600.0), &["a"],
    );
    assert_eq!((b[0].size.width, b[0].size.height), (100.0, 100.0));
}

#[test]
fn inline_layout_breaks_like_a_browser() {
    let b = bounds_of(
        "html, body { margin: 0 } div { font: 10px/10px monospace }
         #cjk { width: 25px } #keep { width: 25px; word-break: keep-all }
         #pad { width: max-content } #pad span { padding: 0 10px }
         #fits { width: 30px }",
        || body().child(div().id("cjk").child("中文字中文")).child(div().id("keep").child("中文字中文"))
            .child(div().id("pad").child("a").child(span().child("b")))
            .child(div().id("fits").child("aa bb ").child(span().child("cc"))),
        (300.0, 300.0), &["cjk", "keep", "pad", "fits"],
    );
    assert!(b[0].size.height >= 20.0, "Chinese breaks between characters: {:?}", b[0].size);
    assert_eq!(b[1].size.height, 10.0, "keep-all: one line");
    let plain = bounds_of("html, body { margin: 0 } div { font: 10px/10px monospace; width: max-content }",
                          || body().child(div().id("p").child("a").child(span().child("b"))), (300.0, 300.0), &["p"]);
    assert_eq!(b[2].size.width, plain[0].size.width + 20.0, "the span's padding takes room");
    // 30px holds "aa bb" exactly (6px a character): the space after "bb" hangs, so it stays
    assert_eq!(b[3].size.height, 20.0, "two lines, not three: the space at the line's end hangs");
}

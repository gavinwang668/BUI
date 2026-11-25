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

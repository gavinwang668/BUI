//! Pixel effects (filter, backdrop-filter, mix-blend-mode, clip-path, mask,
//! rotate / skew / scale transforms), rounded dashed borders, border-image,
//! box and text shadows, radial gradients, per-corner radii, rounded `overflow`,
//! table column sizing, scroll snapping, overflow-wrap in inline text and
//! the capitalized Blender components, end to end on headless pages.

use blenderui::prelude::*;

fn open<V: Render>(cx: &mut App, w: f32, h: f32, v: V) -> WindowHandle<V> {
    let opts = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(w), px(h)),
        })),
        ..Default::default()
    };
    cx.open_window(opts, |_, cx| cx.new(|_| v)).unwrap()
}

struct Page {
    css: &'static str,
    body: fn() -> Div,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        (self.body)().child(style().child(self.css))
    }
}

fn page(cx: &mut App, w: f32, h: f32, css: &'static str, body: fn() -> Div)
        -> (WindowId, impl Fn(f32, f32) -> u32 + use<>) {
    let win = open(cx, w, h, Page { css, body });
    let img = cx.draw(win.id);
    let s = img.0 as f32 / w;
    (win.id, move |x: f32, y: f32| img.2[(y * s) as usize * img.0 + (x * s) as usize])
}

fn rgb_of(p: u32) -> (i32, i32, i32) {
    ((p >> 16 & 255) as i32, (p >> 8 & 255) as i32, (p & 255) as i32)
}

fn near(p: u32, q: u32, tol: i32) -> bool {
    let (a, b) = (rgb_of(p), rgb_of(q));
    (a.0 - b.0).abs() <= tol && (a.1 - b.1).abs() <= tol && (a.2 - b.2).abs() <= tol
}

const BASE: &str = "html, body { margin: 0; background: #000 } .b { position: absolute }";

#[test]
fn filters() {
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 300.0, 100.0, "
            html, body { margin: 0; background: #000 } .b { position: absolute; top: 10px; width: 40px; height: 40px }
            #g { left: 10px; background: #f00; filter: grayscale(1) }
            #i { left: 60px; background: #ff0000; filter: invert(1) }
            #o { left: 110px; background: #fff; filter: opacity(0.5) }
            #bl { left: 180px; background: #fff; filter: blur(4px) }
        ", || {
            body().child(div().id("g").class("b")).child(div().id("i").class("b"))
                .child(div().id("o").class("b")).child(div().id("bl").class("b"))
        });
        let (r, g, b) = rgb_of(at(30.0, 30.0));
        assert!((r - g).abs() <= 2 && (g - b).abs() <= 2 && r > 40 && r < 80, "grayscale red: {r} {g} {b}");
        assert!(near(at(80.0, 30.0), 0x00ffff, 3), "invert: {:06x}", at(80.0, 30.0));
        assert!(near(at(130.0, 30.0), 0x808080, 3), "opacity: {:06x}", at(130.0, 30.0));
        // blur spreads past the box and softens its edge
        assert!(rgb_of(at(176.0, 30.0)).0 > 10, "blurred outside: {:06x}", at(176.0, 30.0));
        assert!(rgb_of(at(181.0, 30.0)).0 < 250, "soft edge: {:06x}", at(181.0, 30.0));
        assert!(near(at(200.0, 30.0), 0xffffff, 8), "centre stays: {:06x}", at(200.0, 30.0));
    });
}

#[test]
fn transforms_rotate_scale_skew() {
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 300.0, 200.0, "
            html, body { margin: 0; background: #000 } .b { position: absolute; background: #0f0 }
            #r { left: 50px; top: 50px; width: 80px; height: 20px; transform: rotate(90deg) }
            #s { left: 200px; top: 20px; width: 20px; height: 20px; scale: 2 }
            #k { left: 150px; top: 150px; width: 40px; height: 20px; transform: skewX(45deg) }
        ", || body().child(div().id("r").class("b")).child(div().id("s").class("b")).child(div().id("k").class("b")));
        // a 80x20 bar rotated about its centre (90, 60) stands 20 wide, 80 tall
        assert_eq!(at(90.0, 30.0), 0x00ff00, "rotated bar reaches up");
        assert_eq!(at(60.0, 60.0), 0x000000, "and no longer reaches left");
        // scale(2) about the centre (210, 30): 20px grows to 40px
        assert_eq!(at(193.0, 30.0), 0x00ff00, "scaled box grows");
        // skewX(45deg) about the centre leans the top right, the bottom left
        // (y points down, so the top edge moves left and the bottom right)
        assert_eq!(at(143.0, 152.0), 0x00ff00, "top edge pushed left");
        assert_eq!(at(193.0, 168.0), 0x00ff00, "bottom edge pushed right");
    });
}

#[test]
fn clip_path_and_mask() {
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 300.0, 120.0, "
            html, body { margin: 0; background: #000 } .b { position: absolute; top: 10px; width: 80px; height: 80px; background: #f00 }
            #c { left: 10px; clip-path: circle(40%) }
            #p { left: 110px; clip-path: polygon(0 0, 100% 0, 0 100%) }
            #m { left: 210px; mask-image: linear-gradient(to right, #000, transparent) }
        ", || body().child(div().id("c").class("b")).child(div().id("p").class("b")).child(div().id("m").class("b")));
        assert_eq!(at(50.0, 50.0), 0xff0000, "circle centre");
        assert_eq!(at(13.0, 13.0), 0x000000, "circle corner clipped");
        assert_eq!(at(120.0, 20.0), 0xff0000, "triangle kept part");
        assert_eq!(at(180.0, 80.0), 0x000000, "triangle clipped part");
        let (l, r) = (rgb_of(at(215.0, 50.0)).0, rgb_of(at(285.0, 50.0)).0);
        assert!(l > 200 && r < 50, "mask fades left to right: {l} {r}");
    });
}

#[test]
fn blend_modes_and_backdrop_filter() {
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 300.0, 120.0, "
            html, body { margin: 0; background: #000 }
            .under { position: absolute; top: 10px; width: 60px; height: 60px; background: #ffff00 }
            #m { position: absolute; top: 10px; left: 10px; width: 60px; height: 60px; background: #00ffff; mix-blend-mode: multiply }
            #d { position: absolute; top: 10px; left: 110px; width: 60px; height: 60px; background: #808080; mix-blend-mode: difference }
            #bd { position: absolute; top: 10px; left: 210px; width: 60px; height: 60px; backdrop-filter: invert(1) }
        ", || {
            body().child(div().class("under").style("left: 10px")).child(div().class("under").style("left: 110px"))
                .child(div().class("under").style("left: 210px"))
                .child(div().id("m")).child(div().id("d")).child(div().id("bd"))
        });
        assert!(near(at(40.0, 40.0), 0x00ff00, 3), "yellow x cyan = green: {:06x}", at(40.0, 40.0));
        assert!(near(at(140.0, 40.0), 0x7f7f80, 3), "difference: {:06x}", at(140.0, 40.0));
        assert!(near(at(240.0, 40.0), 0x0000ff, 3), "inverted backdrop: {:06x}", at(240.0, 40.0));
    });
}

#[test]
fn rounded_dashed_borders_and_border_image() {
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 300.0, 120.0, "
            html, body { margin: 0; background: #000 }
            #d { position: absolute; left: 10px; top: 10px; width: 100px; height: 60px;
                 border: 4px dashed #f00; border-radius: 20px }
            #i { position: absolute; left: 150px; top: 10px; width: 100px; height: 60px; border: 10px solid;
                 border-image: linear-gradient(#0f0, #00f) 10 }
        ", || body().child(div().id("d")).child(div().id("i")));
        // a dashed rounded border: the corner region inside the radius stays empty
        assert_eq!(at(11.0, 11.0), 0x000000, "corner outside the curve");
        let top: Vec<bool> = (30..90).map(|x| at(x as f32 + 0.5, 12.0) == 0xff0000).collect();
        assert!(top.iter().any(|v| *v) && top.iter().any(|v| !*v), "dashes along the top");
        // the border image paints the border area, not the inside
        let edge = at(152.0, 40.0);
        assert!(edge != 0x000000, "border image drawn: {edge:06x}");
        assert_eq!(at(200.0, 40.0), 0x000000, "middle not filled without `fill`");
    });
}

#[test]
fn tables_size_columns_to_content() {
    Application::headless().run(|cx| {
        let (w, _) = page(cx, 600.0, 200.0, "
            body { margin: 0 } table { border-spacing: 0 } td { padding: 0 }
            #fixed { table-layout: fixed; width: 300px }
        ", || {
            body()
                .child(table().id("auto").child(tbody()
                    .child(tr().child(td().id("a1").child("x")).child(td().id("b1").child("a much longer cell")))
                    .child(tr().child(td().id("a2").child("y")).child(td().id("b2").child("z")))))
                .child(table().id("fixed").child(tbody()
                    .child(tr().child(td().id("f1").style("width: 100px").child("x")).child(td().id("f2").child("y")))))
        });
        let r = |id: &str, cx: &mut App| cx.element_bounds(w, id).unwrap();
        let (a1, b1, a2, b2) = (r("a1", cx), r("b1", cx), r("a2", cx), r("b2", cx));
        assert!(b1.size.width > 2.0 * a1.size.width, "columns follow content");
        assert_eq!(a1.size.width, a2.size.width, "a column lines up across rows");
        assert_eq!(b1.origin.x, b2.origin.x);
        let t = r("auto", cx);
        assert!(t.size.width < 400.0, "an auto table shrinks to its columns: {}", t.size.width);
        assert_eq!(r("f1", cx).size.width, 100.0, "fixed layout uses the first row's widths");
        assert_eq!(r("f2", cx).size.width, 200.0);
    });
}

#[test]
fn scroll_snap_mandatory() {
    Application::headless().run(|cx| {
        let (w, _) = page(cx, 200.0, 200.0, "
            body { margin: 0 }
            #s { height: 100px; overflow-y: scroll; scroll-snap-type: y mandatory }
            .p { height: 100px; scroll-snap-align: start }
        ", || {
            body().child(div().id("s").child(div().class("p")).child(div().id("p2").class("p")).child(div().class("p")))
        });
        cx.simulate_scroll(w, 50.0, 50.0, -1);
        cx.draw(w);
        // one wheel notch lands exactly on the next page: the second child at the top
        let p2 = cx.element_bounds(w, "p2").unwrap();
        assert_eq!(p2.origin.y, 0.0, "snapped to the second child: {p2:?}");
    });
}

#[test]
fn a_touch_drag_is_not_snapped_a_step() {
    Application::headless().run(|cx| {
        let (w, _) = page(cx, 200.0, 200.0, "
            body { margin: 0 }
            #s { height: 100px; overflow-y: scroll; scroll-snap-type: y mandatory }
            .p { height: 100px; scroll-snap-align: start }
        ", || {
            body().child(div().id("s").child(div().class("p")).child(div().id("p2").class("p")).child(div().class("p")))
        });
        cx.simulate_scroll_px(w, 50.0, 50.0, -10.0);
        cx.draw(w);
        // the content follows the finger 10 px, not a whole page
        let p2 = cx.element_bounds(w, "p2").unwrap();
        assert_eq!(p2.origin.y, 90.0, "moved as far as the finger: {p2:?}");
    });
}

#[test]
fn overflow_wrap_in_inline_text() {
    Application::headless().run(|cx| {
        let (w, _) = page(cx, 400.0, 400.0, "
            body { margin: 0 } p { width: 60px; margin: 0; line-height: 20px; overflow-wrap: anywhere }
        ", || {
            body().child(p().id("p").child("short ").child(b().child("aaaaaaaaaaaaaaaaaaaaaaaaaaaa")))
        });
        cx.draw(w); // widths settle after one more layout
        let h = cx.element_bounds(w, "p").unwrap().size.height;
        assert!(h >= 60.0, "the long bold word breaks across lines: {h}");
    });
}

struct Blender;

impl Render for Blender {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        rsx! {
            <body m-0>
                <Button id="bb" onClick={|| {}}>"Apply"</Button>
                <NumberSlider id="ns" min={0.0} max={10.0} />
                <Checkbox id="cb" checked={true} />
                <input id="plain" type="range" />
            </body>
        }
    }
}

#[test]
fn blender_components_are_elements() {
    Application::headless().run(|cx| {
        let win = open(cx, 300.0, 200.0, Blender);
        cx.draw(win.id);
        for id in ["bb", "ns", "cb", "plain"] {
            assert!(cx.element_bounds(win.id, id).is_some(), "{id} laid out");
        }
        // gpui syntax: the same components are functions
        let d = Checkbox().checked(true);
        let _ = d;
    });
}

#[allow(dead_code)]
const _BASE: &str = BASE;

#[test]
fn rounded_gradient_keeps_its_corners_in_a_layer() {
    // a turned or filtered box is drawn through a layer: its gradient is cut to the rounded box by
    // clips there too (an empty clip used to mean none: the whole gradient, square)
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 300.0, 140.0, "
            html, body { margin: 0; background: #000 }
            .b { position: absolute; top: 20px; width: 80px; height: 80px; border-radius: 40px;
                 background: linear-gradient(#f00, #00f) }
            #t { left: 20px; transform: rotate(45deg) } #f { left: 160px; filter: opacity(0.99) }
        ", || body().child(div().id("t").class("b")).child(div().id("f").class("b")));
        // inside the turned square, outside the circle it is
        assert_eq!(at(60.0, 11.0), 0x000000, "turned: the corner is cut");
        assert_ne!(at(60.0, 60.0), 0x000000, "turned: the middle shows");
        assert_eq!(at(164.0, 24.0), 0x000000, "in a layer: the corner is cut");
        assert_ne!(at(200.0, 60.0), 0x000000, "in a layer: the middle shows");
    });
}

#[test]
fn box_shadow_rings_and_blurs() {
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 400.0, 200.0, "
            html, body { margin: 0; background: #fff }
            #ring { position: absolute; left: 30px; top: 30px; width: 60px; height: 60px; border-radius: 50%;
                    box-shadow: 0 0 0 6px #f00 }
            #soft { position: absolute; left: 200px; top: 50px; width: 100px; height: 60px; background: #fff;
                    box-shadow: 0 0 20px #000 }
        ", || body().child(div().id("ring")).child(div().id("soft")));
        // the shadow is only outside the box (CSS Backgrounds 3 §7.1.1): an empty box shows its ring
        assert_eq!(at(60.0, 60.0), 0xffffff, "inside the empty box");
        assert!(near(at(60.0, 27.0), 0xff0000, 8), "the spread ring: {:06x}", at(60.0, 27.0));
        // a Gaussian falloff out from the edge: darkest by it, lighter step by step, no bands
        let row: Vec<i32> = (0..30).map(|d| rgb_of(at(301.0 + d as f32, 80.0)).0).collect();
        assert!(row[0] < 160 && row[29] > 245, "falls off: {row:?}");
        for w in row.windows(2) {
            assert!(w[1] >= w[0] && w[1] - w[0] <= 14, "smooth: {row:?}");
        }
        assert_eq!(at(250.0, 80.0), 0xffffff, "the box covers its own shadow");
    });
}

#[test]
fn radial_gradient_position_size_and_fade() {
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 400.0, 300.0, "
            html, body { margin: 0; background: #fff }
            #g { position: absolute; left: 0; top: 0; width: 400px; height: 300px;
                 background: radial-gradient(60px circle at 100px 90px, #ff0000, transparent) }
        ", || body().child(div().id("g")));
        assert!(near(at(100.0, 90.0), 0xff0000, 12), "centred where it says: {:06x}", at(100.0, 90.0));
        assert_eq!(at(200.0, 150.0), 0xffffff, "past its radius: nothing");
        // halfway out: red over white, never darkened by the transparent end's black
        let (r, g, b) = rgb_of(at(130.0, 90.0));
        assert!(r >= 250 && (g - b).abs() <= 3 && g > 80 && g < 200, "a premultiplied fade: {r} {g} {b}");
    });
}

#[test]
fn each_corner_has_its_own_radius() {
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 200.0, 240.0, "
            html, body { margin: 0; background: #000 }
            #d { position: absolute; left: 20px; top: 20px; width: 120px; height: 200px; background: #0f0;
                 border-radius: 56px 56px 6px 6px }
            #e { position: absolute; left: 150px; top: 20px; width: 40px; height: 40px;
                 border: 4px solid #f00; border-radius: 20px 0 0 0 }
        ", || body().child(div().id("d")).child(div().id("e")));
        assert_eq!(at(25.0, 25.0), 0x000000, "a 56px top corner is cut");
        assert_eq!(at(24.0, 216.0), 0x00ff00, "a 6px bottom corner nearly square");
        assert_eq!(at(187.0, 22.0), 0xff0000, "a square corner's border");
        assert_eq!(at(152.0, 22.0), 0x000000, "a round corner's border is cut");
    });
}

#[test]
fn overflow_hidden_rounds_what_it_holds() {
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 300.0, 140.0, "
            html, body { margin: 0; background: #000 }
            .box { position: absolute; top: 20px; width: 100px; height: 100px; border-radius: 30px; overflow: hidden }
            .in { width: 100px; height: 100px; background: #00f }
            #a { left: 20px } #c { left: 160px }
            .cv { position: absolute; left: 0; top: 0; width: 100px; height: 100px }
        ", || {
            body().child(div().id("a").class("box").child(div().class("in")))
                .child(div().id("c").class("box").child(canvas(|b, p| p.fill(b, rgb(0xff0000))).class("cv")))
        });
        assert_eq!(at(23.0, 23.0), 0x000000, "a child's square corner is cut");
        assert_eq!(at(70.0, 70.0), 0x0000ff, "its middle shows");
        assert_eq!(at(163.0, 117.0), 0x000000, "a canvas's corner is cut");
        assert_eq!(at(210.0, 70.0), 0xff0000, "its middle shows");
    });
}

#[test]
fn text_shadow_follows_the_text() {
    // each shadow is the text moved: its letters spaced as the text's, its blur a blur
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 400.0, 160.0, "
            html, body { margin: 0; background: #fff }
            #t { position: absolute; left: 10px; top: 10px; font-size: 30px; letter-spacing: 14px; color: #000;
                 text-shadow: 0 60px 0 #f00, 0 100px 4px #00f }
        ", || body().child(div().id("t").child("SPACED")));
        let span_of = |y0: f32, y1: f32, hit: &dyn Fn(u32) -> bool| {
            let xs: Vec<f32> = (0..400).map(|x| x as f32)
                .filter(|&x| (0..((y1 - y0) as i32)).any(|dy| hit(at(x, y0 + dy as f32)))).collect();
            (xs.first().copied().unwrap_or(0.0), xs.last().copied().unwrap_or(0.0))
        };
        let text = span_of(10.0, 50.0, &|p| rgb_of(p).0 < 80);
        let shadow = span_of(70.0, 110.0, &|p| { let (r, g, b) = rgb_of(p); r > 200 && g < 80 && b < 80 });
        assert!((text.0 - shadow.0).abs() <= 1.0 && (text.1 - shadow.1).abs() <= 1.0,
                "the shadow is as wide as the spaced text: {text:?} {shadow:?}");
        // a blurred shadow has no hard pixels and reaches past its letters
        let blue = (110..160).flat_map(|y| (0..400).map(move |x| (x as f32, y as f32)))
            .map(|(x, y)| rgb_of(at(x, y))).filter(|(r, g, b)| b > r && b > g).count();
        assert!(blue > 400, "the blurred shadow shows: {blue}");
    });
}

#[test]
fn an_anonymous_box_does_not_clip_again() {
    // a canvas after a block in an `overflow: hidden` box sits in an anonymous line box that carries
    // the box's style: the clip is the box's padding box, not cut again at the anonymous box
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 300.0, 200.0, "
            html, body { margin: 0; background: #000 }
            .lcd { position: absolute; left: 20px; top: 20px; width: 200px; height: 100px; overflow: hidden }
            .head { height: 20px }
            .viz { position: absolute; left: 10px; top: 50px; width: 180px; height: 36px }
        ", || body().child(div().class("lcd").child(div().class("head"))
            .child(canvas(|b, p| p.fill(b, rgb(0xff0000))).class("viz"))));
        assert_eq!(at(120.0, 85.0), 0xff0000, "the canvas paints");
    });
}

#[test]
fn translate_moves_inline_content_once() {
    // loose text after a block goes in an anonymous line box: the box's `translate` moves it once
    Application::headless().run(|cx| {
        let (_, moved) = page(cx, 300.0, 200.0, "
            html, body { margin: 0; background: #fff; color: #000; font-size: 20px }
            .t { position: absolute; left: 20px; top: 20px; translate: 0px 40px; width: 200px }
        ", || body().child(div().class("t").child(div().child("AAAA")).child("BBBB")));
        let (_, placed) = page(cx, 300.0, 200.0, "
            html, body { margin: 0; background: #fff; color: #000; font-size: 20px }
            .t { position: absolute; left: 20px; top: 60px; width: 200px }
        ", || body().child(div().class("t").child(div().child("AAAA")).child("BBBB")));
        for y in (0..200).step_by(2) {
            for x in (0..300).step_by(2) {
                assert_eq!(moved(x as f32, y as f32), placed(x as f32, y as f32), "pixel {x},{y}");
            }
        }
    });
}

#[test]
fn a_highlighted_span_keeps_every_word() {
    // the words of a text draw as one run from its first word; a later word's highlight (its inline
    // box's fragment) is painted with that word, so it must not cover the run's text
    Application::headless().run(|cx| {
        let (_, at) = page(cx, 400.0, 60.0, "
            html, body { margin: 0; background: #fff; color: #000; font-size: 20px }
            .hl { background: #b8cdf2 }
        ", || body().child(div().css("padding", "10px").child(span().class("hl").child("aaaa bbbb cccc")).child(" after")));
        // dark glyph pixels in the right half of the highlight (the second and third words)
        let ink = (40..140).flat_map(|x| (10..34).map(move |y| (x as f32, y as f32)))
            .filter(|&(x, y)| rgb_of(at(x, y)).0 < 90).count();
        assert!(ink > 40, "the later words show over their highlight: {ink} dark pixels");
    });
}

//! Morning: a pinch folds a full-screen page into its card in a stack of cards, and spreading
//! opens it again (cloned from `ref/testanim.mp4`). The page washes out, shrinks onto its card,
//! goes dark as the stack fades in behind it, and the stack zooms up to rest; its three little
//! cards fan into the card as they go.
//!
//! One number drives it all: `progress`, 0 the page, 1 the stack. A pinch on a touch screen or a
//! trackpad (`on_pinch`, or ctrl + wheel, which is how Windows trackpads pinch) scrubs it under the
//! fingers, and letting go finishes the way they were going; Space / Enter fold, Escape opens,
//! a click on the page folds and a click on its card opens.
//!
//! `cargo run --release --bin morning`, `-- --storyboard DIR` for headless frames.

use std::sync::OnceLock;

use blenderui::motion::MotionValue;
use blenderui::prelude::*;
use blenderui::vars;
use bui_demos::canvas::{rem, rem_css, Fit};
use bui_demos::ease::{ease_in, ease_out, lerp, progress};
use bui_demos::{arg, focus_root, run_with, size_arg, story::story, Script};

const DESIGN: (f32, f32) = (390.0, 844.0);
/// Seconds for a whole fold (the reference: the stack settles 1 s after the fingers close).
const FOLD: f32 = 1.0;
/// The page's card: its glow, which the page turns into as it folds.
const HOME_GLOW: (&str, f32) = ("#b06c8c", 0.85);
/// Progress per wheel line of a ctrl + wheel pinch.
const PINCH: f32 = 0.09;
/// Fingers closing to this scale fold the page all the way (it ends at about 0.65 of the screen).
const PINCH_FOLD: f32 = 0.65;
/// A pinch with no event for this many frames has let go (no "fingers up" comes from a trackpad).
const LET_GO: u32 = 8;

/// A box in design pixels.
#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

const fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

impl Rect {
    fn lerp(self, to: Self, t: f32) -> Self {
        rect(lerp(self.x, to.x, t), lerp(self.y, to.y, t), lerp(self.w, to.w, t), lerp(self.h, to.h, t))
    }

    fn scaled(self, k: f32) -> Self {
        rect(self.x * k, self.y * k, self.w * k, self.h * k)
    }

    fn place(self, d: Div) -> Div {
        d.css("left", rem(self.x)).css("top", rem(self.y)).css("width", rem(self.w)).css("height", rem(self.h))
    }
}

/// Where the fold is at `u` (0 the page, 1 the stack), read off the reference frame by frame.
struct Fold {
    /// The stack's zoom about the screen centre and its drop (it starts small and low).
    zoom: f32,
    drop: f32,
    stack_alpha: f32,
    /// The page's box on screen and how far it has become its card (0..1).
    page: Rect,
    card: f32,
    /// White wash, then the card's dark, over the page.
    wash: f32,
    dark: f32,
    blur: f32,
    /// The page's own chrome (status bar, the field at the bottom).
    chrome: f32,
}

const FULL: Rect = rect(0.0, 0.0, DESIGN.0, DESIGN.1);
const CENTRE: (f32, f32) = (DESIGN.0 / 2.0, DESIGN.1 / 2.0);
/// The page's card in the stack at rest.
const HOME: Rect = rect(12.0, 290.0, 366.0, 244.0);

impl Fold {
    fn at(u: f32) -> Self {
        // times as fractions of the second the fold takes (ms / 1000)
        let z = ease_out(progress(u, 0.44, 1.0));
        let (zoom, drop) = (lerp(0.72, 1.0, z), lerp(70.0, 0.0, z));
        let card = ease_in(progress(u, 0.0, 0.528));
        let target = Self::through(zoom, drop, HOME);
        let up = progress(u, 0.317, 0.484).min(1.0 - progress(u, 0.546, 0.85));
        Self {
            zoom,
            drop,
            stack_alpha: progress(u, 0.22, 0.484),
            page: FULL.lerp(target, card),
            card,
            wash: 0.6 * progress(u, 0.044, 0.37) * (1.0 - progress(u, 0.396, 0.546)),
            dark: progress(u, 0.37, 0.546),
            blur: 5.0 * up,
            chrome: 1.0 - progress(u, 0.07, 0.334),
        }
    }

    /// `r` (stack coordinates) on screen.
    fn through(zoom: f32, drop: f32, r: Rect) -> Rect {
        rect(CENTRE.0 + (r.x - CENTRE.0) * zoom, CENTRE.1 + (r.y - CENTRE.1) * zoom + drop, r.w * zoom, r.h * zoom)
    }

    /// A point of the page's layout at full size (`page`) and on its card (`card`, card
    /// coordinates), inside the page box now.
    fn inner(&self, page: Rect, card: Rect) -> Rect {
        page.lerp(card.scaled(self.zoom), self.card)
    }
}

/// The stack, top to bottom: title, box (stack coordinates), what is drawn in it.
const STACK: [(&str, Rect, Look); 6] = [
    ("Weekend Plans", rect(12.0, -74.0, 366.0, 76.0), Look::Pill),
    ("Twisted Blades Rogue Build", rect(12.0, 10.0, 366.0, 74.0), Look::Pill),
    ("Personality Quiz", rect(12.0, 92.0, 366.0, 190.0), Look::Quiz),
    ("Good Morning", HOME, Look::Home),
    ("Brainstorming Presents", rect(12.0, 542.0, 366.0, 92.0), Look::Pill),
    ("Brunch Recipe Ideas", rect(12.0, 642.0, 366.0, 210.0), Look::Brunch),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Pill,
    Quiz,
    Home,
    Brunch,
}

/// The page's three cards: full size on the page, then fanned on its card.
struct Snap {
    page: Rect,
    card: Rect,
    turn: (f32, f32),
}

const QUOTE: Snap = Snap { page: rect(44.0, 278.0, 150.0, 140.0), card: rect(92.0, 140.0, 76.0, 96.0), turn: (-3.0, -9.0) };
const FRIDAY: Snap = Snap { page: rect(70.0, 492.0, 160.0, 152.0), card: rect(138.0, 130.0, 80.0, 104.0), turn: (-1.0, -3.0) };
const PHOTO: Snap = Snap { page: rect(190.0, 322.0, 160.0, 196.0), card: rect(188.0, 120.0, 104.0, 108.0), turn: (4.0, 10.0) };

const CSS: &str = r"
html { height: 100%; background: #121013; }
body { margin: 0; height: 100%; overflow: hidden; background: #121013; color: #fff;
       font-family: 'Segoe UI', Inter, sans-serif; }
.abs { position: absolute; }
.screen { position: absolute; overflow: hidden; }
.stack { position: absolute; left: 0px; top: 0px; width: 390px; height: 844px; transform-origin: 195px 422px; }
.card { position: absolute; overflow: hidden; border-radius: 28px; background: #2e2a2f; }
.glow { position: absolute; left: 8%; width: 84%; top: 42%; height: 110%; border-radius: 50%; filter: blur(34px); }
.card h3 { position: absolute; left: 0px; right: 0px; margin: 0; text-align: center; font-size: 19px;
           font-weight: 700; color: #f2cfdc; letter-spacing: -0.2px; }
.pill h3 { top: 26px; color: #ece6ea; font-size: 17px; }
.doc { position: absolute; border-radius: 9px; background: #f4f1ee; box-shadow: 0 6px 18px rgba(0,0,0,0.35); }
.doc i { position: absolute; left: 12px; height: 4px; border-radius: 2px; background: #b9b2b6; }
.page { position: absolute; overflow: hidden; }
.paint { position: absolute; left: 0px; top: 0px; width: 100%; height: 100%; }
.peach { background: linear-gradient(172deg, #f8e6df 0%, #f3c9c3 40%, #efb7a8 100%); }
.blob { position: absolute; border-radius: 50%; }
.title { position: absolute; left: 0px; right: 0px; text-align: center; font-weight: 800; color: #fff;
         line-height: 0.95; letter-spacing: -1px; }
.snap { position: absolute; overflow: hidden; border-radius: 14px; background: #fbf8f4;
        box-shadow: 0 10px 26px rgba(120,60,60,0.22); }
.snap .in { position: absolute; left: 0px; top: 0px; transform-origin: 0px 0px; }
.quote .words { margin: 0; position: absolute; left: 14px; top: 14px; width: 124px; font-family: Georgia, serif;
           font-size: 15.5px; line-height: 1.18; color: #3a302c; }
.label { position: absolute; font-size: 10px; color: #9a8f8a; }
.friday .day { position: absolute; left: 16px; top: 18px; font-size: 26px; color: #2a2522; font-weight: 700; }
.friday .date { position: absolute; left: 16px; top: 48px; font-size: 26px; color: #5b5350; }
.chrome { position: absolute; left: 0px; top: 0px; width: 390px; height: 844px; }
.clock { position: absolute; left: 34px; top: 15px; font-size: 15px; font-weight: 600; color: #3b302e; }
.field { position: absolute; left: 16px; top: 772px; width: 358px; height: 44px; border-radius: 22px;
         background: rgba(255,255,255,0.42); color: #9b8c88; font-size: 14px; }
.field .ask { position: absolute; left: 44px; top: 12px; }
.field .plus { position: absolute; left: 18px; top: 8px; font-size: 20px; font-weight: 400; color: #8c7d79; }
.home { position: absolute; left: 128px; top: 829px; width: 134px; height: 5px; border-radius: 3px; }
.island { position: absolute; left: 134px; top: 11px; width: 122px; height: 35px; border-radius: 18px;
          background: #000; }
";

fn css() -> &'static str {
    static CSS_REM: OnceLock<String> = OnceLock::new();
    CSS_REM.get_or_init(|| rem_css(CSS))
}

struct Morning {
    progress: MotionValue,
    /// The frame of a pinch's last event and which way it went (+1 folding).
    pinch: Option<(u32, f32)>,
    frames: u32,
}

impl Morning {
    /// Runs the fold to `to` (0 or 1) at the reference's pace from wherever it is.
    fn go(&mut self, to: f32) {
        self.pinch = None;
        let duration = FOLD * (to - self.progress.get()).abs();
        drop(self.progress.animate(to, vars! { duration: duration, ease: "none" }));
    }

    fn key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        match e.keystroke.key.as_str() {
            "space" | "Return" | "KP_Enter" | "Down" => self.go(1.0),
            "Escape" | "Up" | "BackSpace" => self.go(0.0),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn folded(&self) -> bool {
        self.progress.get() > 0.5
    }

    /// The fingers moved: `fold` of the way towards the stack (negative: towards the page).
    fn scrub(&mut self, fold: f32) {
        self.progress.stop();
        self.progress.set((self.progress.get() + fold).clamp(0.0, 1.0));
        self.pinch = Some((self.frames, fold.signum()));
    }

    /// Fingers closing by `1 + delta` (a scale) fold in proportion: `PINCH_FOLD` is the whole way.
    fn pinch(&mut self, e: &PinchEvent) {
        self.scrub((1.0 + e.delta).max(0.01).ln() / PINCH_FOLD.ln());
    }

    /// ctrl + wheel: spreading (wheel away) opens, pinching folds.
    fn wheel(&mut self, e: &ScrollWheelEvent) {
        if e.modifiers.control {
            self.scrub(-e.pixels.map_or(e.delta, |p| p / 120.0) * PINCH);
        }
    }
}

fn card_paint(glow: &str, strength: f32) -> Div {
    div().class("paint").child(div().class("glow").css("background", glow).css("opacity", format!("{strength}")))
}

/// The little photo: a person in a yellow jacket under bare branches.
fn photo() -> Div {
    canvas(|b, p| {
        let (x, y, w, h) = (b.origin.x, b.origin.y, b.size.width, b.size.height);
        let at = |u: f32, v: f32| (x + u * w, y + v * h);
        p.fill(b, rgb(0x00d9_dbd7));
        p.fill(Bounds { origin: point(x, y + h * 0.55), size: size(w, h * 0.45) }, rgb(0x00b9_bcb4));
        for (i, (u0, v0, u1, v1)) in [(0.0, 0.1, 0.55, 0.35), (0.2, 0.0, 0.4, 0.42), (0.55, 0.05, 1.0, 0.3),
                                      (0.75, 0.0, 0.62, 0.38), (0.05, 0.4, 0.3, 0.2)].into_iter().enumerate() {
            p.line(&[at(u0, v0), at(u1, v1)], rgb(0x0040_3a36), if i < 2 { 2.2 } else { 1.4 });
        }
        p.polygon(&[at(0.22, 1.0), at(0.3, 0.62), at(0.5, 0.55), at(0.72, 0.62), at(0.82, 1.0)], rgb(0x00e8_b03c));
        p.oval(Bounds { origin: point(x + w * 0.38, y + h * 0.3), size: size(w * 0.25, h * 0.28) }, rgb(0x00e3_bda2), rgba(0), 0.0);
        p.oval(Bounds { origin: point(x + w * 0.36, y + h * 0.25), size: size(w * 0.29, h * 0.16) }, rgb(0x002b_221d), rgba(0), 0.0);
    })
    .class("paint")
}

/// Brunch from above: a plate, two yolks, oranges, herbs.
fn food() -> Div {
    canvas(|b, p| {
        let (x, y, w, h) = (b.origin.x, b.origin.y, b.size.width, b.size.height);
        let oval = |p: &mut Painter, u: f32, v: f32, rw: f32, rh: f32, c: u32| {
            p.oval(Bounds { origin: point(x + (u - rw) * w, y + (v - rh) * h), size: size(2.0 * rw * w, 2.0 * rh * h) },
                   rgb(c), rgba(0), 0.0);
        };
        p.fill(b, rgb(0x003a_2218));
        oval(p, 0.5, 0.62, 0.3, 0.46, 0x00ec_e5da);
        oval(p, 0.5, 0.62, 0.25, 0.38, 0x00f4_efe6);
        oval(p, 0.1, 0.3, 0.09, 0.16, 0x00d9_5f22);
        oval(p, 0.1, 0.3, 0.06, 0.11, 0x00f0_8a3c);
        oval(p, 0.9, 0.78, 0.08, 0.15, 0x00e0_7a2a);
        oval(p, 0.86, 0.25, 0.07, 0.12, 0x00a8_3a24);
        oval(p, 0.42, 0.55, 0.12, 0.2, 0x00ff_fbf2);
        oval(p, 0.42, 0.55, 0.05, 0.08, 0x00f6_a51c);
        oval(p, 0.6, 0.7, 0.12, 0.2, 0x00ff_fbf2);
        oval(p, 0.6, 0.7, 0.05, 0.08, 0x00f6_a51c);
        for (u, v) in [(0.3, 0.75), (0.52, 0.42), (0.7, 0.52), (0.35, 0.4), (0.62, 0.9)] {
            oval(p, u, v, 0.025, 0.04, 0x004e_7a2c);
        }
    })
    .class("paint")
}

/// The stack's cards, in screen coordinates for this frame.
fn stack_card(f: &Fold, i: usize) -> Div {
    let (title, at, look) = STACK[i];
    let card = Fold::through(f.zoom, f.drop, at).place(div().id(format!("card{i}")).class("card"))
        .css("border-radius", rem(28.0 * f.zoom)).css("z-index", format!("{}", i + 1));
    // the contents keep their rest layout, zoomed with the stack
    let mut inner = div().class("abs").css("left", "0").css("top", "0").css("width", rem(at.w)).css("height", rem(at.h))
        .css("transform-origin", "0 0").css("transform", format!("scale({})", f.zoom));
    inner = match look {
        Look::Pill => inner.class("pill").child(card_paint("#4a3f47", 0.45)),
        Look::Quiz => inner.child(card_paint("#86588a", 0.6))
            .child(rect(112.0, 128.0, 118.0, 80.0).place(div().class("doc").css("background", "#cfc8cb").css("transform", "rotate(-5deg)")))
            .child(rect(136.0, 128.0, 118.0, 80.0).place(div().class("doc").css("background", "#d9d3d5").css("transform", "rotate(5deg)")))
            .child(doc_lines(rect(124.0, 116.0, 118.0, 92.0))),
        Look::Home => inner.child(card_paint(HOME_GLOW.0, HOME_GLOW.1)),
        Look::Brunch => inner.child(card_paint("#8a5246", 0.7))
            .child(rect(104.0, 108.0, 158.0, 110.0).place(div().class("abs").css("overflow", "hidden")
                .css("border-radius", rem(12.0)).child(food()))),
    };
    // the page's own card is drawn by the page once it has folded onto it
    if look != Look::Home {
        inner = inner.child(h3().child(title));
    }
    card.child(inner)
}

fn doc_lines(at: Rect) -> Div {
    let mut d = at.place(div().class("doc"));
    for (k, w) in [86.0, 70.0, 92.0, 54.0, 78.0].into_iter().enumerate() {
        d = d.child(div().css("width", rem(w)).css("top", rem(12.0 + k as f32 * 13.0)).css("position", "absolute")
            .css("left", rem(12.0)).css("height", rem(4.0)).css("border-radius", rem(2.0)).css("background", "#bdb5b9"));
    }
    d
}

/// One of the page's cards at this frame: its page layout scaled into the box it has now.
fn snap(f: &Fold, s: &Snap, class: &'static str, content: Div) -> Div {
    let at = f.inner(s.page, s.card);
    let k = at.w / s.page.w;
    at.place(div().class("snap").class(class))
        .css("border-radius", rem(lerp(14.0, 9.0, f.card)))
        .css("transform", format!("rotate({}deg)", lerp(s.turn.0, s.turn.1, f.card)))
        .child(content.class("in").css("width", rem(s.page.w)).css("height", rem(s.page.h))
            .css("transform", format!("scale({k})")))
}

fn page(f: &Fold) -> Div {
    let bx = f.page;
    let t = f.card;
    let mut page = bx.place(div().id("page").class("page"))
        .css("border-radius", rem(lerp(0.0, 28.0 * f.zoom, t))).css("z-index", "20");
    if f.blur > 0.05 || f.wash > 0.01 {
        page = page.css("filter", format!("blur({}px) grayscale({})", f.blur, (f.wash * 1.4).min(1.0)));
    }
    // the warm page: peach, a magenta bloom on the right and an orange one low left
    let k = bx.w / DESIGN.0;
    let blob = |x: f32, y: f32, w: f32, h: f32, c: &str, blur: f32| {
        rect(x, y, w, h).scaled(k).place(div().class("blob")).css("background", c)
            .css("filter", format!("blur({}px)", blur * k))
    };
    let warm = div().class("paint peach")
        .child(blob(170.0, 40.0, 300.0, 520.0, "#e486cf", 70.0))
        .child(blob(-80.0, 250.0, 300.0, 340.0, "#f49a6c", 70.0))
        .child(blob(40.0, -120.0, 300.0, 260.0, "#fbeee8", 60.0));
    page = page.child(warm)
        .child(div().class("paint").css("background", "#f1eeee").css("opacity", format!("{}", f.wash)))
        .child(card_paint(HOME_GLOW.0, HOME_GLOW.1).css("background", "#2e2a2f").css("opacity", format!("{}", f.dark)));

    // "Good Morning": the page's headline becomes the card's title
    let size = lerp(72.0, 27.0 * f.zoom, t);
    let top = lerp(118.0, 22.0 * f.zoom, t);
    let tint = mix(0x00ff_ffff, 0x00f2_cfdc, f.dark);
    page = page.child(div().class("title").css("top", rem(top)).css("font-size", rem(size))
        .css("color", format!("#{tint:06x}")).css("opacity", format!("{}", 1.0 - 0.45 * f.wash / 0.6))
        .child("Good").child(br()).child("Morning"));

    page = page
        .child(snap(f, &QUOTE, "quote", div()
            .child(div().class("words").child("I thrive in new environments and welcome the adventures of learning."))
            .child(div().class("label").css("left", rem(14.0)).css("top", rem(114.0)).child("Daily Affirmation"))))
        .child(snap(f, &FRIDAY, "friday", div()
            .child(div().class("day").child("Friday")).child(div().class("date").child("Oct 27"))
            .child(div().class("label").css("left", rem(16.0)).css("top", rem(124.0)).child("Daily Overview"))))
        .child(snap(f, &PHOTO, "photo", div().child(photo())));

    if f.chrome > 0.0 {
        let o = format!("{}", f.chrome);
        let mut bars = div().class("abs bars").css("left", rem(318.0)).css("top", rem(19.0)).css("width", rem(20.0)).css("height", rem(11.0));
        for (i, h) in [4.0, 6.0, 8.5, 11.0].into_iter().enumerate() {
            bars = bars.child(i_().css("left", rem(i as f32 * 4.5)).css("height", rem(h)));
        }
        page = page.child(div().class("chrome").css("opacity", o).css("transform", format!("scale({k})"))
            .css("transform-origin", "0 0")
            .child(div().class("clock").child("11:01")).child(bars)
            .child(div().class("abs").css("left", rem(344.0)).css("top", rem(19.0)).css("width", rem(22.0))
                .css("height", rem(11.0)).css("border-radius", rem(3.0)).css("background", "#3b302e"))
            .child(div().class("field").child(div().class("plus").child("+")).child(div().class("ask").child("Share with Dot")))
            .child(div().class("home").css("background", "#2a2220")));
    }
    page
}

/// `a` to `b` (0xRRGGBB) by `t`.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 255) as f32, ((b >> s) & 255) as f32);
        (lerp(x, y, t).round() as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

fn i_() -> Div {
    div().css("position", "absolute").css("bottom", "0").css("width", rem(3.0)).css("border-radius", rem(1.0))
        .css("background", "#3b302e")
}

impl Render for Morning {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        w.request_animation_frame();
        focus_root(w, &mut self.frames);
        // a pinch that has gone quiet has let go: on to the end it was heading for
        if let Some((at, dir)) = self.pinch
            && self.frames - at > LET_GO
        {
            // a little way is enough to commit to the way the fingers went
            let u = self.progress.get();
            let fold = if dir > 0.0 { u > 0.2 } else { u > 0.8 };
            self.go(if fold { 1.0 } else { 0.0 });
        }
        let fit = Fit::new(w, DESIGN);
        let f = Fold::at(self.progress.get());

        let mut stack = div().class("stack").css("opacity", format!("{}", f.stack_alpha));
        if f.blur > 0.05 {
            stack = stack.css("filter", format!("blur({}px)", f.blur));
        }
        for i in 0..STACK.len() {
            stack = stack.child(stack_card(&f, i));
        }
        let page = page(&f).on_click(cx.listener(|this, _, _, cx| {
            if this.folded() {
                this.go(0.0);
            } else {
                this.go(1.0);
            }
            cx.notify();
        }));
        let screen = div().class("screen")
            .css("left", rem(fit.ox)).css("top", rem(fit.oy)).css("width", rem(DESIGN.0)).css("height", rem(DESIGN.1))
            .child(stack).child(page).child(div().class("island").css("z-index", "40"));

        html().css("font-size", fit.root_font())
            .child(head().child(style().child(css())))
            .child(body().id("root").attr("tabindex", "0")
                .on_key_down(cx.listener(Self::key))
                .on_pinch(cx.listener(|this, e: &PinchEvent, _, cx| {
                    this.pinch(e);
                    cx.notify();
                }))
                .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                    this.wheel(e);
                    cx.notify();
                }))
                .child(screen))
    }
}

/// The reference's timing: 120 ms at rest, the fold, rest; then the way back, then a pinch and a
/// spread.
fn storyboard(dir: String) -> Script {
    story(dir, move |s| {
        // a first frame, then the pointer comes in (the kit takes keys once it has)
        s.shot("start");
        s.tick(300);
        s.cx.simulate_mouse_move(s.win, 5.0, 5.0);
        // fold: frame n at n x 40 ms, numbered like the reference's (0001 = 0 ms)
        for n in 0..44u64 {
            if n == 3 {
                s.key("space");
            }
            s.shot(&format!("fold-{:04}", n + 1));
            s.tick(40);
        }
        s.key("Escape");
        for n in 0..26u64 {
            s.shot(&format!("open-{:04}", n + 1));
            s.tick(40);
        }
        // fingers closing a little (to 0.8): the page follows them, then on letting go folds
        for n in 0..30u64 {
            if n < 6 {
                s.cx.simulate_magnify(s.win, 195.0, 420.0, -0.037);
            }
            s.shot(&format!("pinch-{:04}", n + 1));
            s.tick(40);
        }
        // spreading them opens it again
        for n in 0..30u64 {
            if n < 6 {
                s.cx.simulate_magnify(s.win, 195.0, 420.0, 0.04);
            }
            s.shot(&format!("spread-{:04}", n + 1));
            s.tick(40);
        }
    })
}

fn main() {
    let (w, h) = size_arg(DESIGN);
    let make = || Morning { progress: MotionValue::new(0.0), pinch: None, frames: 0 };
    run_with(w, h, "BUI · Morning", make, arg("--storyboard").map(storyboard));
}

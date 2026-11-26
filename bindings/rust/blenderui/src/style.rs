//! Computed style and the one table that applies every CSS property.
//!
//! Layout properties land directly in a `taffy::Style`; paint and text
//! properties in [`Computed`]. Shorthands call the longhand handlers, so
//! each property has exactly one implementation.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hasher};
use std::sync::OnceLock;

use foldhash::quality::FixedState;
use taffy::geometry::MinMax;
use taffy::prelude::*;
use taffy::style::Overflow;

use crate::color::{parse_color, Rgba};
use crate::css::{Decl, Prop, Tok};
use crate::geometry::{Length, Units};

// ==========================================================================
// Computed style

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayKind {
    None,
    Block,
    Inline,
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    Contents,
    ListItem,
    Table,
    TableRowGroup,
    TableRow,
    TableCell,
    TableCaption,
}

impl DisplayKind {
    pub fn is_inline_level(self) -> bool {
        matches!(self, DisplayKind::Inline | DisplayKind::InlineBlock | DisplayKind::InlineFlex
                 | DisplayKind::InlineGrid)
    }
}

/// `user-select`: whether, and how, the text of an element can be selected.
/// `contain` is read as `text`; `auto` is resolved against the parent (see
/// [`Computed::inherit`]) and is `text` for ordinary content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UserSelect {
    #[default]
    Auto,
    None,
    Text,
    /// One click selects the whole element.
    All,
}

/// `app-region`: what an element is to a window whose title bar the app draws (not inherited:
/// an element is part of the nearest area around it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppRegion {
    #[default]
    Auto,
    /// The app's own, inside a drag area (Chromium's `no-drag`).
    NoDrag,
    Area(crate::window_kind::WindowControlArea),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
    /// The line's start (left in `ltr`, right in `rtl`): the initial value.
    Start,
    End,
}

impl TextAlign {
    /// Left, right, center or justify, in direction `rtl` or not.
    pub(crate) const fn physical(self, rtl: bool) -> TextAlign {
        match self {
            TextAlign::Start if rtl => TextAlign::Right,
            TextAlign::Start => TextAlign::Left,
            TextAlign::End if rtl => TextAlign::Left,
            TextAlign::End => TextAlign::Right,
            a => a,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
    PreWrap,
    PreLine,
    /// `break-spaces`: as `pre-wrap`, but lines may break after every preserved space.
    BreakSpaces,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextTransform {
    None,
    Upper,
    Lower,
    Capitalize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    Normal,
    Factor(f32),
    Px(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ListStyle {
    None,
    Disc,
    Circle,
    Square,
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VAlign {
    Baseline,
    Sub,
    Super,
    /// `top`: the top of the line box (`text-top` / `-bottom`, by the parent's text, are baseline).
    Top,
    /// `middle`: the middle of the line box.
    Middle,
    /// `bottom`: its bottom.
    Bottom,
}

/// `border-style` / `outline-style` (`none` and `hidden` clear the width instead).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

impl BorderStyle {
    fn parse(s: &str) -> Option<BorderStyle> {
        Some(match s {
            "solid" | "none" | "hidden" | "auto" => BorderStyle::Solid,
            "dashed" => BorderStyle::Dashed,
            "dotted" => BorderStyle::Dotted,
            "double" => BorderStyle::Double,
            "groove" => BorderStyle::Groove,
            "ridge" => BorderStyle::Ridge,
            "inset" => BorderStyle::Inset,
            "outset" => BorderStyle::Outset,
            _ => return None,
        })
    }
}

/// `text-decoration-style`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DecoStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

/// A box edge for `background-clip` / `background-origin`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BoxEdge {
    #[default]
    Border,
    Padding,
    Content,
}

impl BoxEdge {
    fn parse(s: &str) -> Option<BoxEdge> {
        Some(match s {
            "border-box" => BoxEdge::Border,
            "padding-box" => BoxEdge::Padding,
            "content-box" => BoxEdge::Content,
            _ => return None,
        })
    }
}

/// `counter-reset` / `counter-set` / `counter-increment` of one element.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CounterOps {
    pub reset: Vec<(Box<str>, i32)>,
    pub set: Vec<(Box<str>, i32)>,
    pub increment: Vec<(Box<str>, i32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub shape: GradientShape,
    /// Colour stops, with their positions on the gradient line when given (`50%`, `75px`,
    /// `calc(100% - 10px)`).
    pub stops: Vec<(Rgba, Option<LengthPercentage>)>,
}

/// Which way a gradient runs (CSS Images 3 §3.1, §3.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientShape {
    /// Along a line at `angle_deg` (0 = to top, 90 = to right).
    Linear { angle_deg: f32 },
    /// Out from a centre: a circle or an ellipse.
    Radial(Radial),
}

/// A `radial-gradient()`'s ending shape: a circle (one radius) or an ellipse, its size and its
/// centre (`at <position>`, against the box).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radial {
    pub circle: bool,
    pub size: RadialSize,
    pub at: [Pos; 2],
}

/// How big a radial gradient's ending shape is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RadialSize {
    /// `closest-side`, `farthest-side`, `closest-corner`, `farthest-corner` (the default).
    Extent { corner: bool, farthest: bool },
    /// The radii, x and y (a circle's are the same length); a percentage is of the box's
    /// width / height.
    Radii(Pos, Pos),
}

impl Gradient {
    /// The stops at fractions of a gradient line `len` px long (CSS Images 3 §3.5.3: a first or
    /// last stop without a position is at the start or end, a position before an earlier one is
    /// moved up to it, and stops without one are spaced evenly between the ones around them).
    pub fn resolved(&self, len: f32) -> Vec<(Rgba, f32)> {
        let n = self.stops.len();
        let len = len.max(1e-3);
        let mut at: Vec<Option<f32>> = self.stops.iter()
            // to 1/64 px: `calc(20%)` and `20px` on a 100px line are the same stop
            .map(|(_, p)| p.map(|p| (taffy::ResolveOrZero::resolve_or_zero(p, Some(len), crate::tree::resolve_calc) * 64.0).round() / 64.0 / len))
            .collect();
        if let Some(f) = at.first_mut() {
            f.get_or_insert(0.0);
        }
        if let Some(l) = at.last_mut() {
            l.get_or_insert(1.0);
        }
        let mut high = f32::NEG_INFINITY;
        for p in at.iter_mut().flatten() {
            high = high.max(*p);
            *p = high;
        }
        let mut out: Vec<(Rgba, f32)> = Vec::with_capacity(n);
        let mut k = 0;
        while k < n {
            if let Some(p) = at[k] {
                out.push((self.stops[k].0, p));
                k += 1;
                continue;
            }
            // a run of stops without positions, between the ones before and after
            let start = out.last().map_or(0.0, |s| s.1);
            let end = (k..n).find_map(|j| at[j].map(|p| (j, p))).unwrap_or((n, 1.0));
            let steps = (end.0 - k + 1) as f32;
            for j in k..end.0 {
                out.push((self.stops[j].0, start + (end.1 - start) * (j - k + 1) as f32 / steps));
            }
            k = end.0;
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Rgba,
    pub inset: bool,
}

/// A position along one axis: `frac` of the free space plus `px`
/// (`left` = 0, `center` = 0.5, `75%`, `10px`, `right 10px` ...).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pos {
    pub frac: f32,
    pub px: f32,
}

impl Pos {
    pub const START: Pos = Pos { frac: 0.0, px: 0.0 };
    pub const CENTER: Pos = Pos { frac: 0.5, px: 0.0 };

    /// Offset into `free` pixels of room.
    pub fn at(self, free: f32) -> f32 {
        free * self.frac + self.px
    }
}

/// `object-fit`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ObjectFit {
    #[default]
    Fill,
    Contain,
    Cover,
    None,
    ScaleDown,
}

impl ObjectFit {
    /// The kit's `BK_FIT_*`; a video's default (fill) letterboxes, as browsers do.
    pub(crate) const fn kit_video(self) -> i32 {
        match self {
            Self::Fill | Self::Contain => blenderui_sys::BK_FIT_CONTAIN,
            Self::Cover => blenderui_sys::BK_FIT_COVER,
            Self::None => blenderui_sys::BK_FIT_NONE,
            Self::ScaleDown => blenderui_sys::BK_FIT_SCALE_DOWN,
        }
    }
}

/// `background-size`: a size is a fraction of the box plus pixels; `None` = auto.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BgSize {
    #[default]
    Auto,
    Cover,
    Contain,
    Size(Option<Pos>, Option<Pos>),
}

/// Image placement: `object-*` for replaced content, `background-*` images.
/// Boxed in [`Computed::images`]: most elements have neither.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageStyle {
    pub object_fit: ObjectFit,
    pub object_position: [Pos; 2],
    pub bg_url: Option<std::sync::Arc<str>>,
    pub bg_size: BgSize,
    pub bg_position: [Pos; 2],
    /// repeat along x, y
    pub bg_repeat: (bool, bool),
    /// How each repeating axis repeats: `REPEAT_*` (`space`, `round`, or plain).
    pub bg_spacing: [u8; 2],
    /// `background-attachment: fixed`: positioned against the viewport.
    pub bg_fixed: bool,
    /// `background-attachment: local`: it scrolls with a scroll container's content.
    pub bg_local: bool,
}

impl Default for ImageStyle {
    fn default() -> Self {
        ImageStyle {
            object_fit: ObjectFit::Fill,
            object_position: [Pos::CENTER; 2],
            bg_url: None,
            bg_size: BgSize::Auto,
            bg_position: [Pos::START; 2],
            bg_repeat: (true, true),
            bg_spacing: [REPEAT_PLAIN; 2],
            bg_fixed: false,
            bg_local: false,
        }
    }
}

impl ImageStyle {
    /// Where an image of `natural` size goes in `area` with `object-fit` /
    /// `object-position` (it may overflow `area`; clip to it).
    pub fn object_rect(&self, area: (f32, f32, f32, f32), natural: (f32, f32)) -> (f32, f32, f32, f32) {
        let (x, y, w, h) = area;
        let (nw, nh) = (natural.0.max(1.0), natural.1.max(1.0));
        let (dw, dh) = match self.object_fit {
            ObjectFit::Fill => (w, h),
            ObjectFit::None => (nw, nh),
            ObjectFit::Contain | ObjectFit::Cover | ObjectFit::ScaleDown => {
                let s = if self.object_fit == ObjectFit::Cover { (w / nw).max(h / nh) } else { (w / nw).min(h / nh) };
                let s = if self.object_fit == ObjectFit::ScaleDown { s.min(1.0) } else { s };
                (nw * s, nh * s)
            }
        };
        (x + self.object_position[0].at(w - dw), y + self.object_position[1].at(h - dh), dw, dh)
    }

    /// One background tile's size for an image of `natural` size in `area` (CSS Backgrounds 3
    /// §3.9, with CSS Images 3 §5.3's default sizing for what `background-size` leaves `auto`).
    pub fn tile(&self, area: (f32, f32), natural: Natural) -> (f32, f32) {
        let (w, h) = area;
        // the largest (`contain`) or smallest (`cover`) size of ratio `r` that fits the area
        let fit = |r: f32, cover: bool| if (w / h > r) == cover { (w, w / r) } else { (h * r, h) };
        let Natural { width: nw, height: nh, ratio } = natural;
        let auto = || match (nw, nh, ratio) {
            (Some(a), Some(b), _) => (a, b),
            (Some(a), None, r) => (a, r.map_or(h, |r| a / r)),
            (None, Some(b), r) => (r.map_or(w, |r| b * r), b),
            (None, None, Some(r)) => fit(r, false),
            (None, None, None) => (w, h),
        };
        match self.bg_size {
            BgSize::Cover => ratio.map_or((w, h), |r| fit(r, true)),
            BgSize::Contain => ratio.map_or((w, h), |r| fit(r, false)),
            BgSize::Auto => auto(),
            BgSize::Size(sx, sy) => match (sx.map(|p| p.at(w)), sy.map(|p| p.at(h))) {
                (Some(a), Some(b)) => (a, b),
                (Some(a), None) => (a, ratio.map_or(nh.unwrap_or(h), |r| a / r)),
                (None, Some(b)) => (ratio.map_or(nw.unwrap_or(w), |r| b * r), b),
                (None, None) => auto(),
            },
        }
    }
}

/// `background-repeat` per axis (`ImageStyle::bg_spacing`): tiles side by side, as many whole tiles as
/// fit spaced out over the area, or tiles resized so a whole number of them fills it.
pub(crate) const REPEAT_PLAIN: u8 = 0;
pub(crate) const REPEAT_SPACE: u8 = 1;
pub(crate) const REPEAT_ROUND: u8 = 2;

/// An image's natural size (CSS Images 3 §5.1): an SVG may lack a width, a height, or its ratio.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Natural {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub ratio: Option<f32>,
}

impl Natural {
    /// A raster image's: `width` x `height` (at least a pixel each).
    pub fn of(width: f32, height: f32) -> Self {
        let (w, h) = (width.max(1.0), height.max(1.0));
        Self { width: Some(w), height: Some(h), ratio: Some(w / h) }
    }
}

/// The style of one element after the cascade.
#[derive(Clone, Debug)]
pub struct Computed {
    pub display: DisplayKind,
    pub layout: Style,
    pub order: i32,
    /// `grid-template-columns`/`-rows: subgrid`: the axes (`tree::SUBGRID_COLUMNS`, `SUBGRID_ROWS`)
    /// whose tracks are the parent grid's.
    pub subgrid: u8,
    /// `display: grid-lanes` ([`LANES_ON`]) and `grid-lanes-direction` (`tree::LANES_*` flags).
    pub grid_lanes: u8,
    /// `fit-tolerance` (`None`: `normal`, 1em).
    pub fit_tolerance: Option<LengthPercentage>,
    /// `aspect-ratio: auto <ratio>`: a replaced element's natural ratio wins over the one given.
    pub aspect_auto: bool,
    /// Sizing keywords on `min-width`, `min-height`, `max-width`, `max-height` (4 bits each, in that
    /// order: `crate::tree::SIZING_*`), which taffy has no room for there: the tree resolves them.
    pub size_kw: u16,
    /// Size containment (`contain: size` both bits, `inline-size` the width's: `crate::tree::CONTAIN_*`)
    /// and the content size it sizes the box as (`contain-intrinsic-width` / `-height`, 0 for `none`).
    pub contain: (u8, [f32; 2]),
    /// Which of `justify-content`, `align-content`, `justify-items`, `align-items` (`tree::FLOW_*`)
    /// are `flow-start` / `flow-end`: start / end, swapped where grid lanes reverse that axis.
    pub flow_align: u8,
    /// Which `justify-*` properties are `left` / `right` (`PHYS_*`): physical, swapped right to left.
    pub phys_align: u8,
    /// `margin-trim` (`TRIM_*`): the edges whose children's margins this container drops.
    pub margin_trim: u8,
    /// `display: run-in` (a block box, which the block after it takes in as its first inline box).
    pub run_in: bool,
    /// `wrap-inside: avoid`: lines break inside this inline box only when they must.
    pub wrap_avoid: bool,
    /// `white-space-trim` (`TRIM_WS_*`): white space this box discards around and inside it.
    pub ws_trim: u8,
    /// `visibility: collapse` (inherited with `visible`): a collapsed flex item takes no room in
    /// the main axis, only its cross size (a strut).
    pub collapsed: bool,
    /// `background-color` when it uses `currentcolor`: its value, resolved by [`resolve_current`].
    pub background_current: Option<Box<Tok>>,
    /// `word-space-transform` (inherited): what word separators U+200B and `<wbr>` become: none
    /// (0), U+0020 (1) or U+3000 (2).
    pub word_space: u8,
    /// An absolutely positioned box blockified from an inline display: its static position is
    /// an inline box's, where it would be in its line (CSS 2 10.3.7).
    pub inline_static: bool,
    /// `width` / `height: stretch(<length-percentage>)`: the size the margin box fills, made the
    /// box's size once its margins are known ([`resolve_stretch`]).
    pub stretch_fn: [Option<LengthPercentage>; 2],
    /// Anchor positioning (`anchor-name`, `anchor()` ...: `crate::anchor`), when the box uses it.
    pub anchor: Option<Box<crate::anchor::AnchorStyle>>,
    /// `view-transition-name` / `-class`, when the box has a name (`crate::transition`).
    pub view_transition: Option<Box<crate::transition::Named>>,
    // paint (not inherited)
    pub background: Rgba,
    pub gradient: Option<Gradient>,
    /// `object-*` / `background-image: url()` placement (None: defaults).
    pub images: Option<Box<ImageStyle>>,
    /// `container-type`: 0 normal, 1 inline-size, 2 size.
    pub container_type: u8,
    /// `container-name` (space-separated names).
    pub container_name: Option<Box<str>>,
    pub border_color: [Rgba; 4],
    pub border_drawn: [bool; 4],
    pub border_style: [BorderStyle; 4],
    pub radius: [Length; 4],
    pub shadows: Vec<Shadow>,
    pub outline: (f32, Rgba, f32),
    pub outline_style: BorderStyle,
    /// `background-clip` and `background-origin`.
    pub bg_clip: BoxEdge,
    pub bg_origin: BoxEdge,
    /// `clip: rect(top, right, bottom, left)` (absolutely positioned boxes;
    /// `auto` edges are None).
    pub clip_rect: Option<[Option<Length>; 4]>,
    /// `counter-*` operations (None: none).
    pub counters: Option<Box<CounterOps>>,
    /// Filters, blending, clip-path, masks, transforms beyond translate and
    /// border-image (None: none).
    pub effects: Option<Box<crate::effects::Effects>>,
    pub opacity: f32,
    pub z_index: Option<i32>,
    pub app_region: AppRegion,
    pub translate: (Length, Length),
    pub scale: f32,
    // inherited
    pub visible: bool,
    pub cursor: Option<i32>,
    pub pointer_events: bool,
    /// `user-select` (not inherited; `auto` takes `none` / `all` from the parent, see `inherit`).
    pub user_select: UserSelect,
    pub color: Rgba,
    pub accent: Option<Rgba>,
    pub font_size: f32,
    /// The root element's font size (`rem`): inherited by every element from the root.
    pub root_font_size: f32,
    pub weight: u16,
    pub italic: bool,
    pub underline: bool,
    pub line_through: bool,
    pub overline: bool,
    pub text_align: TextAlign,
    /// `text-transform: full-width` / `full-size-kana` (`linebreak::TRANSFORM_*`, inherited),
    /// with or without a case in `text_transform`.
    pub text_transform_extra: u8,
    /// `text-align-last` (`auto`: none, which follows `text-align`).
    pub text_align_last: Option<TextAlign>,
    pub line_height: LineHeight,
    pub white_space: WhiteSpace,
    pub text_transform: TextTransform,
    pub letter_spacing: f32,
    pub word_spacing: f32,
    pub text_indent: Length,
    pub list_style: ListStyle,
    pub ellipsis: bool,
    pub small_caps: bool,
    pub vertical_align: VAlign,
    pub font_family: String,
    /// The font family's engine id (see `text::font_id`); 0 = the UI font.
    pub font_id: u16,
    // tables, lists, quotes and text effects (inherited)
    pub border_spacing: (f32, f32),
    pub border_collapse: bool,
    pub caption_bottom: bool,
    /// `table-layout: fixed` (not inherited).
    pub table_fixed: bool,
    /// `unicode-bidi: bidi-override | isolate-override`: characters are laid
    /// out in `direction` order, ignoring their own directionality.
    pub bidi_override: bool,
    /// `scroll-snap-type`: axes (1 = x, 2 = y) and strictness.
    pub snap_axes: u8,
    pub snap_mandatory: bool,
    /// `scroll-snap-align` per axis (x, y): 0 none, 1 start, 2 center, 3 end.
    pub snap_align: (u8, u8),
    pub marker_inside: bool,
    /// `quotes`: None = the default curly quotes.
    pub quotes: Option<(String, String)>,
    /// `text-shadow`: the first on top.
    pub text_shadow: Vec<Shadow>,
    pub decoration_color: Option<Rgba>,
    pub decoration_style: DecoStyle,
    /// `text-decoration-thickness` in px (None: auto / from-font).
    pub decoration_thickness: Option<f32>,
    /// `text-underline-offset` in px (None: auto).
    pub underline_offset: Option<f32>,
    /// `empty-cells: hide` (inherited).
    pub empty_cells_hide: bool,
    /// `field-sizing: content` (not inherited): a textarea is as tall as its text, wrapped at its width.
    pub field_sizing_content: bool,
    /// `list-style-image: url()` (inherited).
    pub list_image: Option<std::sync::Arc<str>>,
    /// `word-break: break-all` (inherited).
    pub break_all: bool,
    /// `overflow-wrap: anywhere | break-word` (inherited).
    pub wrap_anywhere: bool,
    /// `word-break: keep-all`, `line-break: anywhere`, `hyphens: none`, `overflow-wrap: break-word`
    /// (`linebreak::WRAP_*`, inherited).
    pub text_wrap: u8,
    /// `tab-size` in spaces (inherited).
    pub tab_size: u8,
    /// Custom properties in scope (inherited), in the style store's var arena.
    pub vars: u32,
    /// `content` (for `::before` / `::after`): None = `normal` / `none`.
    pub content: Option<Box<[ContentPart]>>,
    /// `animation-*` / `transition-*` (None: none). Not inherited.
    pub anim: Option<std::sync::Arc<crate::anim::AnimSpec>>,
}

/// One piece of generated content.
#[derive(Clone, Debug, PartialEq)]
pub enum ContentPart {
    Text(String),
    /// `attr(name)`: the originating element's attribute.
    Attr(String),
    OpenQuote,
    CloseQuote,
    /// `counter(name, style)`.
    Counter(Box<str>, ListStyle),
    /// `counters(name, separator, style)`.
    Counters(Box<str>, Box<str>, ListStyle),
}

impl Default for Computed {
    fn default() -> Self {
        Computed {
            display: DisplayKind::Inline,
            layout: Style { display: Display::Block, ..Default::default() },
            order: 0,
            subgrid: 0,
            grid_lanes: 0,
            fit_tolerance: None,
            flow_align: 0,
            phys_align: 0,
            margin_trim: 0,
            run_in: false,
            wrap_avoid: false,
            ws_trim: 0,
            collapsed: false,
            background_current: None,
            word_space: 0,
            inline_static: false,
            stretch_fn: [None; 2],
            aspect_auto: false,
            size_kw: 0,
            contain: (0, [0.0; 2]),
            anchor: None,
            view_transition: None,
            background: Rgba::TRANSPARENT,
            gradient: None,
            images: None,
            container_type: 0,
            container_name: None,
            border_color: [Rgba::TRANSPARENT; 4],
            border_drawn: [true; 4],
            border_style: [BorderStyle::Solid; 4],
            radius: [Length::Px(0.0); 4],
            shadows: Vec::new(),
            outline: (0.0, Rgba::TRANSPARENT, 0.0),
            outline_style: BorderStyle::Solid,
            bg_clip: BoxEdge::Border,
            bg_origin: BoxEdge::Padding,
            clip_rect: None,
            counters: None,
            effects: None,
            opacity: 1.0,
            z_index: None,
            app_region: AppRegion::Auto,
            translate: (Length::Px(0.0), Length::Px(0.0)),
            scale: 1.0,
            visible: true,
            cursor: None,
            pointer_events: true,
            user_select: UserSelect::Auto,
            color: crate::color::rgb(0x000000), // initial: CanvasText
            accent: None,
            font_size: 13.0,
            root_font_size: crate::geometry::ROOT_FONT_SIZE,
            weight: 400,
            italic: false,
            underline: false,
            line_through: false,
            overline: false,
            text_align: TextAlign::Start,
            text_align_last: None,
            text_transform_extra: 0,
            line_height: LineHeight::Normal,
            white_space: WhiteSpace::Normal,
            text_transform: TextTransform::None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            text_indent: Length::Px(0.0),
            list_style: ListStyle::Disc,
            ellipsis: false,
            small_caps: false,
            vertical_align: VAlign::Baseline,
            font_family: String::new(),
            font_id: 0,
            border_spacing: (0.0, 0.0),
            border_collapse: false,
            caption_bottom: false,
            table_fixed: false,
            bidi_override: false,
            snap_axes: 0,
            snap_mandatory: false,
            snap_align: (0, 0),
            marker_inside: false,
            quotes: None,
            text_shadow: Vec::new(),
            decoration_color: None,
            decoration_style: DecoStyle::Solid,
            decoration_thickness: None,
            underline_offset: None,
            empty_cells_hide: false,
            field_sizing_content: false,
            list_image: None,
            break_all: false,
            wrap_anywhere: false,
            text_wrap: 0,
            tab_size: 8,
            vars: 0,
            content: None,
            anim: None,
        }
    }
}

impl Computed {
    /// A child's starting style: inherited properties from `parent`, the rest initial.
    /// What this element's lengths resolve against in `viewport`.
    pub(crate) fn units(&self, viewport: (f32, f32)) -> Units {
        Units { font_size: self.font_size, ch: crate::text::ch(self.font_id, self.font_size), lh: self.line_px(),
                root_font_size: self.root_font_size, viewport }
    }

    pub fn inherit(parent: &Computed) -> Computed {
        Computed {
            visible: parent.visible,
            cursor: parent.cursor,
            pointer_events: parent.pointer_events,
            // CSS UI 4: `auto` computes to the parent's `none` / `all`, else `text`
            user_select: match parent.user_select {
                UserSelect::None => UserSelect::None,
                UserSelect::All => UserSelect::All,
                _ => UserSelect::Auto,
            },
            color: parent.color,
            accent: parent.accent,
            font_size: parent.font_size,
            root_font_size: parent.root_font_size,
            weight: parent.weight,
            italic: parent.italic,
            text_align: parent.text_align,
            text_align_last: parent.text_align_last,
            text_transform_extra: parent.text_transform_extra,
            word_space: parent.word_space,
            collapsed: parent.collapsed,
            line_height: parent.line_height,
            white_space: parent.white_space,
            text_transform: parent.text_transform,
            letter_spacing: parent.letter_spacing,
            word_spacing: parent.word_spacing,
            text_indent: parent.text_indent,
            list_style: parent.list_style,
            small_caps: parent.small_caps,
            font_family: parent.font_family.clone(),
            font_id: parent.font_id,
            // text-decoration propagates to inline descendants in CSS
            underline: parent.underline,
            line_through: parent.line_through,
            overline: parent.overline,
            decoration_color: parent.decoration_color,
            decoration_style: parent.decoration_style,
            decoration_thickness: parent.decoration_thickness,
            underline_offset: parent.underline_offset,
            empty_cells_hide: parent.empty_cells_hide,
            list_image: parent.list_image.clone(),
            break_all: parent.break_all,
            wrap_anywhere: parent.wrap_anywhere,
            text_wrap: parent.text_wrap,
            tab_size: parent.tab_size,
            border_spacing: parent.border_spacing,
            border_collapse: parent.border_collapse,
            caption_bottom: parent.caption_bottom,
            marker_inside: parent.marker_inside,
            quotes: parent.quotes.clone(),
            text_shadow: parent.text_shadow.clone(),
            vars: parent.vars,
            layout: Style { display: Display::Block, direction: parent.layout.direction, ..Default::default() },
            ..Computed::default()
        }
    }

    pub(crate) fn effects_mut(&mut self) -> &mut crate::effects::Effects {
        self.effects.get_or_insert_with(Default::default)
    }

    fn images_mut(&mut self) -> &mut ImageStyle {
        self.images.get_or_insert_with(Default::default)
    }

    /// The grid-lanes settings of a `display: grid-lanes` box.
    pub(crate) fn lanes(&self) -> Option<crate::tree::Lanes> {
        (self.grid_lanes & LANES_ON != 0).then(|| crate::tree::Lanes {
            flags: self.grid_lanes & !LANES_ON,
            flow: self.flow_align,
            tolerance: self.fit_tolerance.unwrap_or_else(|| LengthPercentage::length(self.font_size)),
        })
    }

    /// Line box height in pixels.
    pub fn line_px(&self) -> f32 {
        match self.line_height {
            LineHeight::Normal => crate::text::normal_line(self.font_id, self.font_size),
            LineHeight::Factor(f) => (self.font_size * f).round(),
            LineHeight::Px(p) => p,
        }
    }

    /// `text-overflow: ellipsis` takes effect: it only applies to a block
    /// container that clips its overflow horizontally (CSS Overflow 3).
    pub fn ellipsizes(&self) -> bool {
        self.ellipsis && self.layout.overflow.x != taffy::Overflow::Visible
    }

    pub fn bold(&self) -> bool {
        self.weight >= 600
    }
}

// ==========================================================================
// Applying declarations

/// What a handler sees: the style being computed, its parent, units.
pub struct Ctx<'a> {
    pub c: &'a mut Computed,
    pub parent: &'a Computed,
    pub u: Units,
}

type Handler = fn(&mut Ctx, &[Tok]) -> bool;

/// Apply one declaration; false when the property or value isn't understood.
pub fn apply(c: &mut Computed, parent: &Computed, u: Units, d: &Decl) -> bool {
    let mut ctx = Ctx { c, parent, u };
    if d.prop.is_custom() {
        return true; // custom properties are collected by the cascade (`vars`)
    }
    if d.prop == Prop::UNKNOWN {
        return false;
    }
    if let [Tok::Ident(k)] = &*d.value {
        if d.prop.name() == "all" {
            let keep = (ctx.c.layout.direction, ctx.c.vars);
            *ctx.c = match k.as_ref() {
                "inherit" => ctx.parent.clone(),
                "initial" => Computed::default(),
                "unset" | "revert" | "revert-layer" => Computed::inherit(ctx.parent),
                _ => return false,
            };
            (ctx.c.layout.direction, ctx.c.vars) = keep;
            return true;
        }
        if matches!(k.as_ref(), "inherit" | "initial" | "unset" | "revert" | "revert-layer") {
            return global_keyword(&mut ctx, d.prop.name(), k);
        }
    }
    match HANDLERS.get(d.prop.0 as usize) {
        Some((_, h)) => h(&mut ctx, &d.value),
        None => true, // accepted, no effect
    }
}

/// `@supports (prop: value)`: whether this engine understands the declaration.
pub(crate) fn supports_decl(prop: &str, value: &str) -> bool {
    let d = Decl::new(prop.to_ascii_lowercase(), value);
    if d.prop.is_custom() {
        return true;
    }
    if d.prop == Prop::UNKNOWN {
        return false;
    }
    let parent = Computed::default();
    let mut c = Computed::default();
    apply(&mut c, &parent, Units::initial(16.0, (1000.0, 800.0)), &d)
}

/// The property index of `name` (run-time lookup for parsed CSS).
pub(crate) fn prop_index(name: &str) -> Prop {
    static MAP: OnceLock<KeyMap<&'static str, u16>> = OnceLock::new();
    let map = MAP.get_or_init(|| {
        HANDLERS.iter().map(|(n, _)| *n).chain(ACCEPTED.iter().copied()).enumerate()
            .map(|(i, n)| (n, i as u16)).collect()
    });
    match map.get(name) {
        Some(&i) => Prop(i),
        None if name.starts_with("--") => Prop::custom(name),
        None => Prop::UNKNOWN,
    }
}

/// Standard properties that parse and are accepted but have no effect in this
/// renderer (floats, bidi, generated content, background images, animation ...).
pub const ACCEPTED: &[&str] = &[
    "writing-mode", "will-change",
    "resize", "appearance", "caret-color", "scroll-behavior",
    "overscroll-behavior", "touch-action",
    "font-feature-settings", "font-kerning",
    "font-stretch", "text-rendering", "-webkit-font-smoothing", "-moz-osx-font-smoothing",
    "orphans", "widows", "page-break-before", "page-break-after", "page-break-inside",
    "break-before", "break-after", "break-inside", "columns", "column-count", "column-width",
    "column-rule", "perspective", "backface-visibility",
    "box-decoration-break", "image-rendering", "shape-outside",
    "text-justify",
    "all",
    // scrolling
    "scroll-margin", "scroll-margin-top", "scroll-margin-right", "scroll-margin-bottom",
    "scroll-margin-left", "scroll-margin-block", "scroll-margin-block-start", "scroll-margin-block-end",
    "scroll-margin-inline", "scroll-margin-inline-start", "scroll-margin-inline-end",
    "scroll-padding", "scroll-padding-top", "scroll-padding-right", "scroll-padding-bottom",
    "scroll-padding-left", "scroll-padding-block", "scroll-padding-block-start",
    "scroll-padding-block-end", "scroll-padding-inline", "scroll-padding-inline-start",
    "scroll-padding-inline-end", "scroll-snap-stop", "overscroll-behavior-x", "overscroll-behavior-y",
    "overscroll-behavior-block", "overscroll-behavior-inline", "overflow-anchor", "scrollbar-color",
    "scrollbar-gutter", "overflow-clip-margin",
    // fonts: OpenType features and synthesis
    "font-variant-ligatures", "font-variant-numeric", "font-variant-east-asian",
    "font-variant-alternates", "font-variant-position", "font-variant-emoji", "font-synthesis",
    "font-synthesis-weight", "font-synthesis-style", "font-synthesis-small-caps",
    "font-synthesis-position", "font-optical-sizing", "font-size-adjust", "font-variation-settings",
    "font-palette", "font-language-override", "font-width",
    // text details
    "text-underline-position", "text-decoration-skip-ink",
    "text-emphasis", "text-emphasis-color", "text-emphasis-style", "text-emphasis-position",
    "text-orientation", "text-combine-upright", "hanging-punctuation", "hyphenate-character",
    "line-clamp", "-webkit-line-clamp", "text-size-adjust", "-webkit-text-size-adjust",
    // sizing / containment / rendering hints
    "content-visibility",
    "interpolate-size", "color-scheme", "forced-color-adjust",
    "print-color-adjust", "color-adjust", "image-orientation", "image-resolution",
    "transform-box", "transform-style", "perspective-origin", "zoom", "caret", "caret-shape",
    "background-blend-mode", "column-span", "column-fill", "column-rule-color", "column-rule-style",
    "column-rule-width",
    // masking
    "mask-clip", "mask-origin",
    "mask-composite", "mask-type", "mask-border", "mask-border-source",
    "mask-border-slice", "mask-border-width", "mask-border-outset", "mask-border-repeat",
    "mask-border-mode",
];

/// Implemented or accepted.
pub fn is_supported(name: &str) -> bool {
    is_implemented(name) || ACCEPTED.contains(&name) || name.starts_with("--")
}

fn global_keyword(ctx: &mut Ctx, name: &str, keyword: &str) -> bool {
    if name == "font-family" {
        global_keyword(ctx, "-bui-font-id", keyword);
    }
    if name == "background-color" {
        ctx.c.background_current = if keyword == "inherit" { ctx.parent.background_current.clone() } else { None };
    }
    let src = if keyword == "inherit" || (keyword == "unset" && is_inherited(name)) {
        ctx.parent.clone()
    } else {
        Computed::default()
    };
    macro_rules! copy {
        ($($prop:literal => $($field:ident).+),* $(,)?) => {
            match name {
                $($prop => { ctx.c.$($field).+ = src.$($field).+.clone(); true })*
                _ => handlers().contains_key(name),
            }
        };
    }
    copy! {
        "color" => color, "font-size" => font_size, "font-weight" => weight,
        "font-style" => italic, "text-align" => text_align, "line-height" => line_height,
        "white-space" => white_space, "text-transform" => text_transform,
        "letter-spacing" => letter_spacing, "word-spacing" => word_spacing,
        "visibility" => visible, "cursor" => cursor, "list-style-type" => list_style,
        "background-color" => background, "opacity" => opacity, "display" => display,
        "width" => layout.size.width, "height" => layout.size.height,
        "font-family" => font_family, "accent-color" => accent,
        "-bui-font-id" => font_id, "user-select" => user_select,
        "align-items" => layout.align_items, "align-self" => layout.align_self,
        "align-content" => layout.align_content, "justify-items" => layout.justify_items,
        "justify-self" => layout.justify_self, "justify-content" => layout.justify_content,
        "flex-grow" => layout.flex_grow, "flex-shrink" => layout.flex_shrink,
        "flex-basis" => layout.flex_basis, "flex-direction" => layout.flex_direction,
        "flex-wrap" => layout.flex_wrap, "min-width" => layout.min_size.width,
        "min-height" => layout.min_size.height, "max-width" => layout.max_size.width,
        "max-height" => layout.max_size.height, "box-sizing" => layout.box_sizing,
        "aspect-ratio" => layout.aspect_ratio, "row-gap" => layout.gap.height,
        "column-gap" => layout.gap.width, "margin-top" => layout.margin.top,
        "margin-right" => layout.margin.right, "margin-bottom" => layout.margin.bottom,
        "margin-left" => layout.margin.left, "padding-top" => layout.padding.top,
        "padding-right" => layout.padding.right, "padding-bottom" => layout.padding.bottom,
        "padding-left" => layout.padding.left, "top" => layout.inset.top,
        "right" => layout.inset.right, "bottom" => layout.inset.bottom, "left" => layout.inset.left,
    }
}

fn is_inherited(name: &str) -> bool {
    INHERITED.contains(&name)
}

const INHERITED: &[&str] = &[
    "border-spacing", "border-collapse", "caption-side", "list-style-position", "quotes",
    "text-shadow", "empty-cells",
    "color", "cursor", "font", "font-family", "font-size", "font-style", "font-variant",
    "font-weight", "letter-spacing", "line-height", "list-style", "list-style-type",
    "text-align", "text-align-last", "text-indent", "text-transform", "visibility", "white-space", "word-spacing",
    "accent-color", "caret-color", "pointer-events", "list-style-image", "word-break",
    "overflow-wrap", "word-wrap", "tab-size", "text-underline-offset", "line-break", "hyphens", "text-autospace",
];

/// The one hasher for keys, caches and maps: foldhash, a word at a time (FNV took a byte at a
/// time). The fixed seed keeps keys the same for the whole run; the quality variant keeps
/// element keys, which are identities with no equality check, collision-free in practice.
pub(crate) type KeyState = FixedState;

/// A new hasher with the crate's fixed seed.
pub(crate) fn key_hasher() -> impl Hasher {
    KeyState::default().build_hasher()
}

pub(crate) type KeyMap<K, V> = HashMap<K, V, KeyState>;

/// A set with the same hasher as [`KeyMap`].
pub(crate) type KeySet<K> = HashSet<K, KeyState>;

fn handlers() -> &'static KeyMap<&'static str, Handler> {
    static MAP: OnceLock<KeyMap<&'static str, Handler>> = OnceLock::new();
    MAP.get_or_init(|| HANDLERS.iter().copied().collect())
}

/// Names of every property that changes layout or rendering.
pub fn implemented_properties() -> impl Iterator<Item = &'static str> {
    HANDLERS.iter().map(|(n, _)| *n)
}

pub fn is_implemented(name: &str) -> bool {
    handlers().contains_key(name)
}

// -- value helpers -----------------------------------------------------------------

pub(crate) fn length(t: &Tok) -> Option<Length> {
    use crate::calc::{is_math_fn, leaf, Calc};
    Some(match t {
        Tok::Num(v) if *v == 0.0 => Length::Px(0.0),
        Tok::Pct(p) => Length::Percent(*p),
        Tok::Ident(s) if s == "auto" => Length::Auto,
        Tok::Dim(v, unit) => match leaf(*v, unit)? {
            Calc::Px(x) => Length::Px(x),
            Calc::Em(x) => Length::Em(x),
            // the font's own: resolved with the element's units
            ch @ (Calc::Ch(_) | Calc::Lh(_)) => Length::Calc(crate::calc::intern(ch)),
            Calc::Rem(x) => Length::Rem(x),
            Calc::Vw(x) => Length::Vw(x),
            Calc::Vh(x) => Length::Vh(x),
            Calc::Vmin(x) => Length::Vmin(x),
            Calc::Vmax(x) => Length::Vmax(x),
            _ => return None, // angles, times
        },
        Tok::Calc(c) => Length::Calc(c),
        Tok::Func(fx) if is_math_fn(&fx.name) => Length::Calc(Calc::parse_fn(&fx.name, &fx.args)?),
        _ => return None,
    })
}

pub(crate) fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Num(v) => format!("{v}"),
        Tok::Dim(v, u) => format!("{v}{u}"),
        Tok::Pct(v) => format!("{v}%"),
        Tok::Ident(s) => s.to_string(),
        Tok::Hash(h) => format!("#{h}"),
        Tok::Str(s) => s.to_string(),
        Tok::Color(c) => c.to_css(),
        Tok::Func(fx) => format!(
            "{}({})",
            fx.name,
            fx.args.iter()
                .map(|a| a.iter().map(tok_text).collect::<Vec<_>>().join(" "))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Tok::Comma => ",".into(),
        Tok::Slash => "/".into(),
        Tok::Delim(c) => c.to_string(),
        Tok::Calc(c) => format!("{c:?}"),
    }
}

pub(crate) fn color(t: &Tok, current: Rgba) -> Option<Rgba> {
    match t {
        Tok::Color(c) => Some(*c),
        Tok::Ident(s) if s == "currentcolor" => Some(current),
        Tok::Ident(s) => crate::color4::system(s).or_else(|| parse_color(s)),
        Tok::Hash(_) => parse_color(&tok_text(t)),
        Tok::Func(fx) => crate::color4::color_fn(fx, current),
        _ => None,
    }
}

fn number(t: &Tok) -> Option<f32> {
    match t {
        Tok::Num(v) => Some(*v),
        Tok::Pct(p) => Some(*p / 100.0),
        _ => None,
    }
}

fn kw(v: &[Tok]) -> Option<&str> {
    v.first().and_then(Tok::ident)
}

fn px(ctx: &Ctx, l: Length) -> f32 {
    l.to_px(&ctx.u).unwrap_or(0.0)
}

fn lp(ctx: &Ctx, l: Length) -> LengthPercentage {
    match l {
        Length::Percent(p) => LengthPercentage::percent(p / 100.0),
        Length::Auto => LengthPercentage::length(0.0),
        Length::Calc(c) if c.has_pct() => LengthPercentage::calc(c.resolve(&ctx.u).as_ptr()),
        other => LengthPercentage::length(px(ctx, other)),
    }
}

fn lpa(ctx: &Ctx, l: Length) -> LengthPercentageAuto {
    match l {
        Length::Percent(p) => LengthPercentageAuto::percent(p / 100.0),
        Length::Auto => LengthPercentageAuto::auto(),
        Length::Calc(c) if c.has_pct() => LengthPercentageAuto::calc(c.resolve(&ctx.u).as_ptr()),
        other => LengthPercentageAuto::length(px(ctx, other)),
    }
}

fn dim(ctx: &Ctx, l: Length) -> Dimension {
    match l {
        Length::Percent(p) => Dimension::percent(p / 100.0),
        Length::Auto => Dimension::auto(),
        Length::Calc(c) if c.has_pct() => Dimension::calc(c.resolve(&ctx.u).as_ptr()),
        other => Dimension::length(px(ctx, other)),
    }
}

/// `width`-like values: lengths plus the sizing keywords.
fn dim_kw(ctx: &Ctx, v: &[Tok]) -> Option<Dimension> {
    match kw(v) {
        Some("none") => Some(Dimension::auto()),
        Some("min-content") => Some(Dimension::min_content()),
        Some("max-content") => Some(Dimension::max_content()),
        Some("fit-content") => Some(Dimension::fit_content()),
        Some("stretch" | "-webkit-fill-available" | "-moz-available" | "fill-available") => {
            Some(Dimension::stretch())
        }
        _ => match v.first()? {
            Tok::Func(fx) if fx.name == "fit-content" => {
                Some(match length(fx.args.first()?.first()?)? {
                    Length::Percent(p) => Dimension::fit_content_percent(p / 100.0),
                    Length::Calc(c) if c.has_pct() => Dimension::fit_content_calc(c.resolve(&ctx.u).as_ptr()),
                    l => Dimension::fit_content_px(px(ctx, l)),
                })
            }
            t => length(t).map(|l| dim(ctx, l)),
        },
    }
}

/// Whether colour value `t` depends on `currentcolor` (itself, or inside a function).
fn uses_currentcolor(t: &Tok) -> bool {
    match t {
        Tok::Ident(s) => s.eq_ignore_ascii_case("currentcolor"),
        Tok::Func(fx) => fx.args.iter().flatten().any(uses_currentcolor),
        _ => false,
    }
}

/// A `currentcolor` background ([`Computed::background_current`]) with the element's `color`.
pub(crate) fn resolve_current(c: &mut Computed) {
    if let Some(col) = c.background_current.as_deref().and_then(|t| color(t, c.color)) {
        c.background = col;
    }
}

/// `stretch(L)` sizes (css-sizing-4 6.1) made the box's: L less its margins (auto ones 0), and
/// less its padding and border for a `content-box` size, as a `calc()` when percentages are in it.
pub(crate) fn resolve_stretch(c: &mut Computed, u: &Units) {
    use taffy::CompactLength;
    let term = |v: CompactLength| match v.tag() {
        CompactLength::LENGTH_TAG => Some(format!("{}px", v.value())),
        CompactLength::PERCENT_TAG => Some(format!("{}%", v.value() * 100.0)),
        _ => None,
    };
    for k in 0..2 {
        let Some(l) = c.stretch_fn[k] else { continue };
        let s = &c.layout;
        let (m, p, b) = if k == 0 {
            ([s.margin.left, s.margin.right], [s.padding.left, s.padding.right], [s.border.left, s.border.right])
        } else {
            ([s.margin.top, s.margin.bottom], [s.padding.top, s.padding.bottom], [s.border.top, s.border.bottom])
        };
        let mut expr = term(l.into_raw()).unwrap_or_else(|| "0px".into());
        let mut less = m.iter().filter_map(|v| term(v.into_raw())).collect::<Vec<_>>();
        if s.box_sizing == BoxSizing::ContentBox {
            less.extend(p.iter().chain(&b).filter_map(|v| term(v.into_raw())));
        }
        for t in less {
            expr.push_str(" - ");
            expr.push_str(&t);
        }
        let toks = crate::css::tokenize(&format!("max(0px, {expr})"));
        let d = match toks.first().and_then(length) {
            Some(Length::Calc(c)) if c.has_pct() => Dimension::calc(c.resolve(u).as_ptr()),
            Some(l) => l.to_px(u).map_or(Dimension::auto(), Dimension::length),
            None => Dimension::auto(),
        };
        if k == 0 { c.layout.size.width = d } else { c.layout.size.height = d }
    }
}

/// `contain-intrinsic-*` values: each `none` (0), a length, or `auto` before either.
fn intrinsic_sizes(ctx: &Ctx, v: &[Tok]) -> Option<Vec<f32>> {
    v.iter().filter(|t| t.ident() != Some("auto")).map(|t| match t {
        t if t.ident() == Some("none") => Some(0.0),
        t => length(t).filter(|l| !matches!(l, Length::Percent(_))).map(|l| px(ctx, l).max(0.0)),
    }).collect::<Option<Vec<f32>>>().filter(|s| (1..=2).contains(&s.len()))
}

/// A `min-width`-like value in slot `slot` of `Computed::size_kw`: a sizing keyword is recorded there
/// (for the tree to resolve) and is `auto` to taffy.
fn min_max(ctx: &mut Ctx, slot: u16, v: &[Tok]) -> Option<LengthPercentageAuto> {
    // `fit-content(<length-percentage>)`: the keyword, its limit kept as the value
    if let [Tok::Func(fx)] = v
        && fx.name == "fit-content"
        && let [arg] = fx.args.as_slice()
        && let [t] = arg.as_slice()
        && let Some(l) = length(t).filter(|l| *l != Length::Auto && !matches!(l, Length::Percent(p) if *p < 0.0))
    {
        let shift = slot * 4;
        ctx.c.size_kw = (ctx.c.size_kw & !(0xf << shift)) | (crate::tree::SIZING_FIT_CONTENT << shift);
        return Some(lpa(ctx, l));
    }
    let code = match kw(v) {
        Some("min-content") => crate::tree::SIZING_MIN_CONTENT,
        Some("max-content") => crate::tree::SIZING_MAX_CONTENT,
        Some("fit-content") => crate::tree::SIZING_FIT_CONTENT,
        Some("stretch" | "-webkit-fill-available" | "-moz-available" | "fill-available") => crate::tree::SIZING_STRETCH,
        _ => 0,
    };
    let out = if code == 0 { lpa_kw(ctx, v) } else { Some(LengthPercentageAuto::auto()) };
    if out.is_some() {
        let shift = slot * 4;
        ctx.c.size_kw = (ctx.c.size_kw & !(0xf << shift)) | (code << shift);
    }
    out
}

/// `min-width`-like values (taffy has no sizing keywords there yet: they act as auto).
fn lpa_kw(ctx: &Ctx, v: &[Tok]) -> Option<LengthPercentageAuto> {
    match kw(v) {
        Some("none" | "min-content" | "max-content" | "fit-content" | "stretch") => {
            Some(LengthPercentageAuto::auto())
        }
        _ => v.first().and_then(length).map(|l| lpa(ctx, l)),
    }
}

/// The 1–4 value box shorthand: top right bottom left.
fn four<T: Copy>(v: &[T]) -> Option<[T; 4]> {
    Some(match v.len() {
        1 => [v[0]; 4],
        2 => [v[0], v[1], v[0], v[1]],
        3 => [v[0], v[1], v[2], v[1]],
        4 => [v[0], v[1], v[2], v[3]],
        _ => return None,
    })
}

fn lengths(v: &[Tok]) -> Option<Vec<Length>> {
    v.iter().filter(|t| !matches!(t, Tok::Slash)).map(length).collect()
}

// side index: 0 top, 1 right, 2 bottom, 3 left
fn set_margin(ctx: &mut Ctx, side: usize, l: Length) {
    let v = lpa(ctx, l);
    let m = &mut ctx.c.layout.margin;
    match side {
        0 => m.top = v,
        1 => m.right = v,
        2 => m.bottom = v,
        _ => m.left = v,
    }
}

fn set_padding(ctx: &mut Ctx, side: usize, l: Length) {
    let v = lp(ctx, l);
    let p = &mut ctx.c.layout.padding;
    match side {
        0 => p.top = v,
        1 => p.right = v,
        2 => p.bottom = v,
        _ => p.left = v,
    }
}

/// The box's anchor positioning style, made when a property first needs it.
fn anchoring<'c>(ctx: &'c mut Ctx) -> &'c mut crate::anchor::AnchorStyle {
    ctx.c.anchor.get_or_insert_with(Default::default)
}

/// `top`, `right`, `bottom`, `left`: a length, or `anchor()` (`auto` until resolved after layout).
fn inset_side(ctx: &mut Ctx, side: usize, v: &[Tok]) -> bool {
    if let Some(e) = crate::anchor::Edge::parse(v, ctx.parent.font_size) {
        anchoring(ctx).insets[side] = Some(e);
        set_inset(ctx, side, Length::Auto);
        return true;
    }
    let Some(l) = v.first().and_then(length) else { return false };
    if let Some(a) = ctx.c.anchor.as_deref_mut() {
        a.insets[side] = None;
    }
    set_inset(ctx, side, l);
    true
}

/// A size property (`width` .. `max-height`, in `AnchorStyle::sizes` order): `anchor-size()`, or what
/// `rest` parses.
fn size_prop(ctx: &mut Ctx, k: usize, v: &[Tok], rest: fn(&mut Ctx, &[Tok]) -> bool) -> bool {
    if k < 2 {
        // `stretch(<length-percentage>)`: auto until the cascade ends
        let arg = match v {
            [Tok::Func(fx)] if fx.name == "stretch" && fx.args.len() == 1 => match fx.args[0].as_slice() {
                [t] => Some(length(t).filter(|l| *l != Length::Auto).map(|l| lp(ctx, l))),
                _ => Some(None),
            },
            _ => None,
        };
        ctx.c.stretch_fn[k] = None;
        if let Some(arg) = arg {
            let Some(l) = arg else { return false };
            ctx.c.stretch_fn[k] = Some(l);
            if k == 0 { ctx.c.layout.size.width = Dimension::auto() } else { ctx.c.layout.size.height = Dimension::auto() }
            return true;
        }
    }
    if let Some(x) = crate::anchor::Extent::parse(v, ctx.parent.font_size) {
        anchoring(ctx).sizes[k] = Some(x);
        return true;
    }
    if !rest(ctx, v) {
        return false;
    }
    if let Some(a) = ctx.c.anchor.as_deref_mut() {
        a.sizes[k] = None;
    }
    true
}

/// `justify-self` (bit 1) / `align-self` (bit 2): records `anchor-center`.
fn anchor_center(ctx: &mut Ctx, v: &[Tok], bit: u8) {
    if kw(v) == Some("anchor-center") {
        anchoring(ctx).center |= bit;
    } else if let Some(a) = ctx.c.anchor.as_deref_mut() {
        a.center &= !bit;
    }
}

fn set_inset(ctx: &mut Ctx, side: usize, l: Length) {
    let v = lpa(ctx, l);
    let i = &mut ctx.c.layout.inset;
    match side {
        0 => i.top = v,
        1 => i.right = v,
        2 => i.bottom = v,
        _ => i.left = v,
    }
}

fn set_border_width(ctx: &mut Ctx, side: usize, w: f32) {
    let v = LengthPercentage::length(w);
    let b = &mut ctx.c.layout.border;
    match side {
        0 => b.top = v,
        1 => b.right = v,
        2 => b.bottom = v,
        _ => b.left = v,
    }
}

fn border_width_tok(ctx: &Ctx, t: &Tok) -> Option<f32> {
    match t.ident() {
        Some("thin") => Some(1.0),
        Some("medium") => Some(3.0),
        Some("thick") => Some(5.0),
        _ => length(t).map(|l| px(ctx, l)),
    }
}

const BORDER_STYLES: &[&str] = &["none", "hidden", "solid", "dashed", "dotted", "double",
                                 "groove", "ridge", "inset", "outset"];

/// `border[-side]: <width> || <style> || <color>` for the given sides.
fn border_sides(ctx: &mut Ctx, v: &[Tok], sides: &[usize]) -> bool {
    let (mut w, mut style, mut col) = (None, None, None);
    for t in v {
        if let Some(s) = t.ident().filter(|s| BORDER_STYLES.contains(s)) {
            style = Some(s.to_string());
        } else if let Some(c) = color(t, ctx.c.color).filter(|_| !matches!(t, Tok::Dim(..))) {
            col = Some(c);
        } else if let Some(px) = border_width_tok(ctx, t) {
            w = Some(px);
        } else {
            return false;
        }
    }
    let none = matches!(style.as_deref(), Some("none") | Some("hidden"));
    for &s in sides {
        let width = if none { 0.0 } else { w.unwrap_or(if style.is_some() { 3.0 } else { 0.0 }) };
        set_border_width(ctx, s, width);
        ctx.c.border_color[s] = col.unwrap_or(ctx.c.color);
        ctx.c.border_drawn[s] = !none;
        if let Some(bs) = style.as_deref().and_then(BorderStyle::parse) {
            ctx.c.border_style[s] = bs;
        } else if style.is_none() {
            ctx.c.border_style[s] = BorderStyle::Solid;
        }
    }
    true
}

fn per_side(v: &[Tok], mut f: impl FnMut(usize, &Tok) -> bool) -> bool {
    match four(&v.iter().collect::<Vec<_>>()) {
        Some(sides) => (0..4).all(|i| f(i, sides[i])),
        None => false,
    }
}

fn align(v: &str) -> Option<AlignItems> {
    Some(match v {
        "start" | "flow-start" => AlignItems::START,
        "end" | "flow-end" => AlignItems::END,
        "self-start" => AlignItems::SELF_START,
        "self-end" => AlignItems::SELF_END,
        "flex-start" => AlignItems::FLEX_START,
        "flex-end" => AlignItems::FLEX_END,
        "center" | "anchor-center" => AlignItems::CENTER,
        "baseline" | "first" | "last" => AlignItems::BASELINE,
        "stretch" => AlignItems::STRETCH,
        "normal" | "legacy" => AlignItems::NORMAL,
        _ => return None,
    })
}

fn align_content(v: &str) -> Option<AlignContent> {
    Some(match v {
        "start" | "flow-start" => AlignContent::START,
        "end" | "flow-end" => AlignContent::END,
        "flex-start" => AlignContent::FLEX_START,
        "flex-end" => AlignContent::FLEX_END,
        "center" => AlignContent::CENTER,
        "stretch" => AlignContent::STRETCH,
        "normal" => AlignContent::NORMAL,
        "space-between" => AlignContent::SPACE_BETWEEN,
        "space-around" => AlignContent::SPACE_AROUND,
        "space-evenly" => AlignContent::SPACE_EVENLY,
        "baseline" | "first" => AlignContent::START,
        _ => return None,
    })
}

/// `[safe | unsafe] <keyword>` (and `first baseline` / `last baseline`).
fn safety(v: &[Tok]) -> taffy::style::AlignmentSafety {
    use taffy::style::AlignmentSafety;
    match v.iter().filter_map(Tok::ident).next() {
        Some("safe") => AlignmentSafety::Safe,
        Some("unsafe") => AlignmentSafety::Unsafe,
        _ => AlignmentSafety::Default,
    }
}

/// Records in `flow_align` whether alignment value `v` (of the property of `bit`) is flow-relative.
fn flow_relative(c: &mut Ctx, v: &[Tok], bit: u8) {
    if v.iter().filter_map(Tok::ident).next_back().is_some_and(|k| k.starts_with("flow-")) {
        c.c.flow_align |= bit;
    } else {
        c.c.flow_align &= !bit;
    }
}

/// `left` / `right` (valid in the inline axis only, `physical`) are the physical sides: start and
/// end left to right, recorded by [`physical`] for `base_style` to swap them right to left.
/// `last baseline` takes its fallback alignment, `end` (taffy shares first baselines only; an
/// overflowing item overflows at the start, as browsers place it).
fn alignment<A>(v: &[Tok], physical: bool, start: A, end: A, keyword: fn(&str) -> Option<A>)
    -> Option<(A, taffy::style::AlignmentSafety)> {
    if v.iter().any(|t| t.ident() == Some("last")) {
        return (v.iter().filter_map(Tok::ident).next_back() == Some("baseline"))
            .then_some((end, taffy::style::AlignmentSafety::Default));
    }
    let a = match v.iter().filter_map(Tok::ident).filter(|s| *s != "safe" && *s != "unsafe").next_back()? {
        "left" if physical => start,
        "right" if physical => end,
        k => keyword(k)?,
    };
    Some((a, safety(v)))
}

fn kw_align(v: &[Tok], physical: bool) -> Option<AlignItems> {
    alignment(v, physical, AlignItems::START, AlignItems::END, align).map(|(a, s)| a.with_safety(s))
}

fn kw_align_content(v: &[Tok], physical: bool) -> Option<AlignContent> {
    alignment(v, physical, AlignContent::START, AlignContent::END, align_content).map(|(a, s)| a.with_safety(s))
}

/// Records in `phys_align` whether alignment value `v` (of the property of `bit`) is `left` / `right`.
fn physical(c: &mut Ctx, v: &[Tok], bit: u8) {
    if v.iter().filter_map(Tok::ident).next_back().is_some_and(|k| k == "left" || k == "right") {
        c.c.phys_align |= bit;
    } else {
        c.c.phys_align &= !bit;
    }
}

/// `white-space-trim` bits (CSS Text 4 3.3): discard the white space before the box, after it, and at
/// the inner edges of its content.
pub(crate) const TRIM_WS_BEFORE: u8 = 1;
pub(crate) const TRIM_WS_AFTER: u8 = 2;
pub(crate) const TRIM_WS_INNER: u8 = 4;

/// `margin-trim` bits (css-box-4 3): the container edges whose adjoining margins are trimmed.
pub(crate) const TRIM_BLOCK_START: u8 = 1;
pub(crate) const TRIM_BLOCK_END: u8 = 2;
pub(crate) const TRIM_INLINE_START: u8 = 4;
pub(crate) const TRIM_INLINE_END: u8 = 8;

/// `margin-trim`: `none`, `block`, `inline`, or any of the four edges once each.
fn margin_trim(v: &[Tok]) -> Option<u8> {
    let words: Vec<&str> = v.iter().map(Tok::ident).collect::<Option<_>>()?;
    match words.as_slice() {
        ["none"] => return Some(0),
        ["block"] => return Some(TRIM_BLOCK_START | TRIM_BLOCK_END),
        ["inline"] => return Some(TRIM_INLINE_START | TRIM_INLINE_END),
        _ => {}
    }
    let mut bits = 0;
    for w in words {
        let bit = match w {
            "block-start" => TRIM_BLOCK_START,
            "block-end" => TRIM_BLOCK_END,
            "inline-start" => TRIM_INLINE_START,
            "inline-end" => TRIM_INLINE_END,
            _ => return None,
        };
        if bits & bit != 0 {
            return None;
        }
        bits |= bit;
    }
    (bits != 0).then_some(bits)
}

/// `phys_align` bits: which `justify-*` property is `left` / `right`.
pub(crate) const PHYS_JUSTIFY_CONTENT: u8 = 1;
pub(crate) const PHYS_JUSTIFY_ITEMS: u8 = 2;
pub(crate) const PHYS_JUSTIFY_SELF: u8 = 4;

// -- grid ----------------------------------------------------------------------------

type Ident = String;
use taffy::style::{GridTemplateArea, GridTemplateAreas, GridTemplateComponent, GridTemplateRepetition,
                   RepetitionCount};

fn track_min(ctx: &Ctx, t: &Tok) -> Option<MinTrackSizingFunction> {
    Some(match t.ident() {
        Some("auto") => MinTrackSizingFunction::auto(),
        Some("min-content") => MinTrackSizingFunction::min_content(),
        Some("max-content") => MinTrackSizingFunction::max_content(),
        _ => match t {
            Tok::Dim(_, u) if u == "fr" => MinTrackSizingFunction::auto(),
            _ => match length(t)? {
                Length::Percent(p) => MinTrackSizingFunction::percent(p / 100.0),
                Length::Calc(c) if c.has_pct() => MinTrackSizingFunction::calc(c.resolve(&ctx.u).as_ptr()),
                l => MinTrackSizingFunction::length(px(ctx, l)),
            },
        },
    })
}

fn track_max(ctx: &Ctx, t: &Tok) -> Option<MaxTrackSizingFunction> {
    Some(match t.ident() {
        Some("auto") => MaxTrackSizingFunction::auto(),
        Some("min-content") => MaxTrackSizingFunction::min_content(),
        Some("max-content") => MaxTrackSizingFunction::max_content(),
        _ => match t {
            Tok::Dim(v, u) if u == "fr" => MaxTrackSizingFunction::fr(*v),
            _ => match length(t)? {
                Length::Percent(p) => MaxTrackSizingFunction::percent(p / 100.0),
                Length::Calc(c) if c.has_pct() => MaxTrackSizingFunction::calc(c.resolve(&ctx.u).as_ptr()),
                l => MaxTrackSizingFunction::length(px(ctx, l)),
            },
        },
    })
}

fn track(ctx: &Ctx, t: &Tok) -> Option<TrackSizingFunction> {
    match t {
        Tok::Func(fx) if fx.name == "minmax" && fx.args.len() == 2 => Some(MinMax {
            min: track_min(ctx, fx.args[0].first()?)?,
            max: track_max(ctx, fx.args[1].first()?)?,
        }),
        Tok::Func(fx) if fx.name == "fit-content" => Some(MinMax {
            min: MinTrackSizingFunction::auto(),
            max: match length(fx.args.first()?.first()?)? {
                Length::Percent(p) => MaxTrackSizingFunction::fit_content_percent(p / 100.0),
                Length::Calc(c) if c.has_pct() => {
                    MaxTrackSizingFunction::fit_content_calc(c.resolve(&ctx.u).as_ptr())
                }
                l => MaxTrackSizingFunction::fit_content_px(px(ctx, l)),
            },
        }),
        _ => Some(MinMax { min: track_min(ctx, t)?, max: track_max(ctx, t)? }),
    }
}

/// Records whether a `grid-template-*` value made axis `bit` a subgrid (`subgrid`, with or without
/// line names: [`track_list`] keeps those).
fn subgrid_axis(c: &mut Ctx, v: &[Tok], bit: u8) {
    if v.first().and_then(Tok::ident) == Some("subgrid") {
        c.c.subgrid |= bit;
    } else {
        c.c.subgrid &= !bit;
    }
}

type TrackList = (Vec<GridTemplateComponent<Ident>>, Vec<Vec<Ident>>);

/// A track list with its line names (`[a b] 1fr [c] repeat(2, [d] 10px)`):
/// tracks, and one name set per line between them.
fn track_list(ctx: &Ctx, v: &[Tok]) -> Option<TrackList> {
    if matches!(kw(v), Some("none") | Some("subgrid") | Some("masonry")) {
        return Some((Vec::new(), Vec::new()));
    }
    // `subgrid [a] [b c]`: a name set for each of its lines, from the first (no tracks: the parent's)
    if v.first().and_then(Tok::ident) == Some("subgrid") {
        let mut names: Vec<Vec<Ident>> = Vec::new();
        let mut open = false;
        for t in &v[1..] {
            match t {
                Tok::Delim('[') if !open => {
                    open = true;
                    names.push(Vec::new());
                }
                Tok::Delim(']') if open => open = false,
                t if open => names.last_mut()?.push(Ident::from(t.ident()?)),
                _ => return None,
            }
        }
        return (!open).then_some((Vec::new(), names));
    }
    let mut out = Vec::new();
    let mut names: Vec<Vec<Ident>> = vec![Vec::new()];
    let mut k = 0;
    while k < v.len() {
        match &v[k] {
            Tok::Delim('[') => {
                k += 1;
                while k < v.len() && !matches!(v[k], Tok::Delim(']')) {
                    if let Some(n) = v[k].ident() {
                        names.last_mut()?.push(Ident::from(n));
                    }
                    k += 1;
                }
            }
            Tok::Func(fx) if fx.name == "repeat" && fx.args.len() >= 2 => {
                let count = match fx.args[0].first()? {
                    Tok::Num(n) => RepetitionCount::Count(*n as u16),
                    Tok::Ident(s) if s == "auto-fill" => RepetitionCount::AutoFill,
                    Tok::Ident(s) if s == "auto-fit" => RepetitionCount::AutoFit,
                    _ => return None,
                };
                let inner: Vec<Tok> = fx.args[1..].iter().flatten().cloned().collect();
                let (tracks, inner_names) = track_list(ctx, &inner)?;
                let tracks: Vec<TrackSizingFunction> = tracks.into_iter().filter_map(|t| match t {
                    GridTemplateComponent::Single(t) => Some(t),
                    GridTemplateComponent::Repeat(_) => None,
                }).collect();
                let line_names = if inner_names.len() == tracks.len() + 1 { inner_names } else { Vec::new() };
                out.push(GridTemplateComponent::Repeat(GridTemplateRepetition { count, tracks, line_names }));
                names.push(Vec::new());
            }
            t => {
                out.push(GridTemplateComponent::Single(track(ctx, t)?));
                names.push(Vec::new());
            }
        }
        k += 1;
    }
    if names.iter().all(Vec::is_empty) {
        names.clear();
    }
    Some((out, names))
}

/// `grid-template-areas: "a a b" "c . b"`.
fn template_areas(v: &[Tok]) -> Option<Option<GridTemplateAreas<Ident>>> {
    if kw(v) == Some("none") {
        return Some(None);
    }
    let rows: Vec<Vec<String>> = v.iter().map(|t| match t {
        Tok::Str(s) => Some(s.split_whitespace().map(str::to_string).collect()),
        _ => None,
    }).collect::<Option<_>>()?;
    let cols = rows.first().map(Vec::len).unwrap_or(0);
    if rows.is_empty() || rows.iter().any(|r| r.len() != cols) {
        return None;
    }
    let mut areas: Vec<GridTemplateArea<Ident>> = Vec::new();
    for (ri, row) in rows.iter().enumerate() {
        for (ci, cell) in row.iter().enumerate() {
            if cell.chars().all(|c| c == '.') {
                continue;
            }
            let (r, c) = (ri as u16 + 1, ci as u16 + 1);
            match areas.iter_mut().find(|a| a.name == *cell) {
                Some(a) => {
                    a.row_start = a.row_start.min(r);
                    a.row_end = a.row_end.max(r + 1);
                    a.column_start = a.column_start.min(c);
                    a.column_end = a.column_end.max(c + 1);
                }
                None => areas.push(GridTemplateArea {
                    name: Ident::from(cell.as_str()),
                    row_start: r,
                    row_end: r + 1,
                    column_start: c,
                    column_end: c + 1,
                }),
            }
        }
    }
    Some(Some(GridTemplateAreas { areas, row_count: rows.len() as u16, column_count: cols as u16 }))
}

fn grid_place(toks: &[Tok]) -> Option<GridPlacement<Ident>> {
    let mut span = false;
    let (mut num, mut name) = (None, None);
    for t in toks {
        match t {
            Tok::Ident(s) if s == "span" => span = true,
            Tok::Ident(s) if s == "auto" && toks.len() == 1 => return Some(GridPlacement::Auto),
            Tok::Ident(s) => name = Some(Ident::from(s.as_ref())),
            Tok::Num(n) => num = Some(*n as i32),
            _ => return None,
        }
    }
    Some(match (span, num, name) {
        (true, n, None) => GridPlacement::Span(n.unwrap_or(1).max(1) as u16),
        (true, n, Some(id)) => GridPlacement::NamedSpan(id, n.unwrap_or(1).max(1) as u16),
        (false, Some(n), None) => taffy::style_helpers::line(n as i16),
        (false, n, Some(id)) => GridPlacement::NamedLine(id, n.unwrap_or(0) as i16),
        _ => return None,
    })
}

fn grid_line(v: &[Tok]) -> Option<Line<GridPlacement<Ident>>> {
    let parts: Vec<&[Tok]> = v.split(|t| matches!(t, Tok::Slash)).collect();
    let start = grid_place(parts[0])?;
    let end = match parts.get(1) {
        Some(p) => grid_place(p)?,
        // a lone area name: its -end line closes it
        None => match &start {
            GridPlacement::NamedLine(id, 0) => GridPlacement::NamedLine(id.clone(), 0),
            _ => GridPlacement::Auto,
        },
    };
    Some(Line { start, end })
}

// -- backgrounds, shadows, fonts -----------------------------------------------------------

pub(crate) fn gradient(ctx: &Ctx, t: &Tok) -> Option<Gradient> {
    let Tok::Func(fx) = t else { return None };
    let (f, args) = (&fx.name, &fx.args);
    if !f.contains("gradient") {
        return None;
    }
    let radial = f.contains("radial");
    let mut shape = if radial { GradientShape::Radial(radial_shape(ctx, &[])?) } else { GradientShape::Linear { angle_deg: 180.0 } };
    let mut first = 0;
    if let Some(a) = args.first() {
        match a.first() {
            Some(Tok::Dim(v, u)) if !radial && (u == "deg" || u == "turn" || u == "rad") => {
                let angle_deg = match u.as_ref() {
                    "turn" => v * 360.0,
                    "rad" => v.to_degrees(),
                    _ => *v,
                };
                shape = GradientShape::Linear { angle_deg };
                first = 1;
            }
            Some(Tok::Ident(s)) if !radial && s == "to" => {
                let dirs: Vec<&str> = a[1..].iter().filter_map(Tok::ident).collect();
                let angle_deg = match dirs.as_slice() {
                    ["top"] => 0.0,
                    ["right"] => 90.0,
                    ["left"] => 270.0,
                    ["bottom"] => 180.0,
                    [a, b] if (*a == "top" || *b == "top") => {
                        if dirs.contains(&"right") { 45.0 } else { 315.0 }
                    }
                    _ => if dirs.contains(&"right") { 135.0 } else { 225.0 },
                };
                shape = GradientShape::Linear { angle_deg };
                first = 1;
            }
            // `circle`, `ellipse 40px 20px`, `farthest-side at 20% 30%`, `at center`: no colour in it
            _ if radial && !a.iter().any(|t| color(t, ctx.c.color).is_some()) => {
                shape = GradientShape::Radial(radial_shape(ctx, a)?);
                first = 1;
            }
            _ => {}
        }
    }
    // `red`, `red 20%`, `red 10px`, or `red 0 75px` (two stops of one colour)
    let mut stops: Vec<(Rgba, Option<LengthPercentage>)> = Vec::new();
    for stop in &args[first..] {
        let Some(c) = stop.iter().find_map(|t| color(t, ctx.c.color)) else { continue };
        let at: Vec<LengthPercentage> = stop.iter().filter_map(|t| length(t).map(|l| lp(ctx, l))).take(2).collect();
        if at.is_empty() {
            stops.push((c, None));
        }
        stops.extend(at.into_iter().map(|a| (c, Some(a))));
    }
    (!stops.is_empty()).then_some(Gradient { shape, stops })
}

/// A `radial-gradient()`'s first argument: `[circle | ellipse] [<extent> | <radii>] [at <position>]`
/// in any order before `at` (CSS Images 3 §3.2.1). Empty: an ellipse to the farthest corner, centred.
fn radial_shape(c: &Ctx, v: &[Tok]) -> Option<Radial> {
    let (head, at) = match v.iter().position(|t| t.ident() == Some("at")) {
        Some(k) => (&v[..k], position(c, &v[k + 1..])?),
        None => (v, [Pos::CENTER; 2]),
    };
    let mut circle = None;
    let mut extent = None;
    let mut radii: Vec<Pos> = Vec::new();
    for t in head {
        match t.ident() {
            Some("circle") => circle = Some(true),
            Some("ellipse") => circle = Some(false),
            Some("closest-side") => extent = Some((false, false)),
            Some("farthest-side") => extent = Some((false, true)),
            Some("closest-corner") => extent = Some((true, false)),
            Some("farthest-corner") => extent = Some((true, true)),
            Some(_) => return None,
            None => radii.push(match length(t)? {
                Length::Percent(p) => Pos { frac: p / 100.0, px: 0.0 },
                l => Pos { frac: 0.0, px: l.to_px(&c.u)? },
            }),
        }
    }
    // one radius is a circle's (and a length, not a percentage); two, an ellipse's
    let circle = circle.unwrap_or(radii.len() == 1);
    let size = match radii.as_slice() {
        [] => {
            let (corner, farthest) = extent.unwrap_or((true, true));
            RadialSize::Extent { corner, farthest }
        }
        [r] if circle && r.frac == 0.0 && extent.is_none() => RadialSize::Radii(*r, *r),
        [rx, ry] if !circle && extent.is_none() => RadialSize::Radii(*rx, *ry),
        _ => return None,
    };
    Some(Radial { circle, size, at })
}

fn shadows(ctx: &Ctx, v: &[Tok]) -> Option<Vec<Shadow>> {
    if kw(v) == Some("none") {
        return Some(Vec::new());
    }
    v.split(|t| matches!(t, Tok::Comma))
        .map(|part| {
            let mut nums = Vec::new();
            // no colour: currentcolor
            let mut c = ctx.c.color;
            let mut inset = false;
            for t in part {
                if t.ident() == Some("inset") {
                    inset = true;
                } else if let Some(l) = length(t) {
                    nums.push(px(ctx, l));
                } else {
                    c = color(t, ctx.c.color)?;
                }
            }
            if nums.len() < 2 {
                return None;
            }
            Some(Shadow {
                x: nums[0],
                y: nums[1],
                blur: *nums.get(2).unwrap_or(&0.0),
                spread: *nums.get(3).unwrap_or(&0.0),
                color: c,
                inset,
            })
        })
        .collect()
}

fn font_size(ctx: &Ctx, t: &Tok) -> Option<f32> {
    let base = ctx.parent.font_size;
    Some(match t.ident() {
        Some("xx-small") => 9.0,
        Some("x-small") => 10.0,
        Some("small") => 13.0 * 0.89,
        Some("medium") => 13.0,
        Some("large") => 13.0 * 1.2,
        Some("x-large") => 13.0 * 1.5,
        Some("xx-large") => 13.0 * 2.0,
        Some("xxx-large") => 13.0 * 3.0,
        Some("smaller") => base / 1.2,
        Some("larger") => base * 1.2,
        _ => match length(t)? {
            Length::Percent(p) => base * p / 100.0,
            Length::Em(e) => base * e,
            other => other.to_px(&ctx.u)?,
        },
    })
}

fn font_weight(ctx: &Ctx, t: &Tok) -> Option<u16> {
    Some(match t {
        Tok::Num(n) => *n as u16,
        Tok::Ident(s) => match s.as_ref() {
            "normal" => 400,
            "bold" => 700,
            "bolder" => (ctx.parent.weight + 300).min(900),
            "lighter" => ctx.parent.weight.saturating_sub(300).max(100),
            _ => return None,
        },
        _ => return None,
    })
}

fn line_height(t: &Tok, ctx: &Ctx) -> Option<LineHeight> {
    Some(match t {
        Tok::Ident(s) if s == "normal" => LineHeight::Normal,
        Tok::Num(n) => LineHeight::Factor(*n),
        Tok::Pct(p) => LineHeight::Factor(p / 100.0),
        _ => match length(t)? {
            // `calc(1.25 / 0.875)` (Tailwind's `text-sm`) is a number, a factor of the font size
            Length::Calc(c) if c.is_number() => LineHeight::Factor(c.eval_units(&ctx.u, 0.0)),
            Length::Em(e) => LineHeight::Px(e * ctx.c.font_size),
            other => LineHeight::Px(other.to_px(&ctx.u)?),
        },
    })
}

/// `display: grid-lanes` in [`Computed::grid_lanes`].
const LANES_ON: u8 = 0x80;

fn display(ctx: &mut Ctx, v: &[Tok]) -> bool {
    let words: Vec<&str> = v.iter().filter_map(Tok::ident).collect();
    let lanes = matches!(words.as_slice(), ["grid-lanes" | "inline-grid-lanes"] | ["block" | "inline", "grid-lanes"]);
    let k = match words.as_slice() {
        ["grid-lanes"] | ["block", "grid-lanes"] => DisplayKind::Grid,
        ["inline-grid-lanes"] | ["inline", "grid-lanes"] => DisplayKind::InlineGrid,
        ["none"] => DisplayKind::None,
        ["block"] | ["block", "flow"] => DisplayKind::Block,
        ["flow-root"] | ["block", "flow-root"] => {
            ctx.c.layout.display = Display::FlowRoot;
            DisplayKind::Block
        }
        ["inline"] | ["inline", "flow"] | ["ruby"] | ["ruby-text"] | ["ruby-base"] => {
            DisplayKind::Inline
        }
        ["inline-block"] | ["inline", "flow-root"] => DisplayKind::InlineBlock,
        ["flex"] | ["block", "flex"] => DisplayKind::Flex,
        ["inline-flex"] | ["inline", "flex"] => DisplayKind::InlineFlex,
        ["grid"] | ["block", "grid"] => DisplayKind::Grid,
        ["inline-grid"] | ["inline", "grid"] => DisplayKind::InlineGrid,
        ["contents"] => DisplayKind::Contents,
        ["run-in"] | ["run-in", "flow"] => DisplayKind::Block,
        ["list-item"] | ["block", "list-item"] => DisplayKind::ListItem,
        ["table"] | ["inline-table"] => DisplayKind::Table,
        ["table-row-group"] | ["table-header-group"] | ["table-footer-group"] => {
            DisplayKind::TableRowGroup
        }
        ["table-row"] => DisplayKind::TableRow,
        ["table-cell"] | ["table-column"] | ["table-column-group"] => DisplayKind::TableCell,
        ["table-caption"] => DisplayKind::TableCaption,
        _ => return false,
    };
    ctx.c.display = k;
    ctx.c.run_in = v.first().and_then(Tok::ident) == Some("run-in");
    ctx.c.grid_lanes = if lanes { ctx.c.grid_lanes | LANES_ON } else { ctx.c.grid_lanes & !LANES_ON };
    true
}

/// `grid-lanes-direction: normal | [row | column] && fill-reverse? && track-reverse?`.
fn lanes_direction(c: &mut Ctx, v: &[Tok]) -> bool {
    use crate::tree::{FILL_REVERSE, LANES_COLUMN, LANES_ROW, TRACK_REVERSE};
    let mut f = 0;
    for t in v {
        let bit = match t.ident() {
            Some("normal") if v.len() == 1 => 0,
            Some("row") if f & (LANES_ROW | LANES_COLUMN) == 0 => LANES_ROW,
            Some("column") if f & (LANES_ROW | LANES_COLUMN) == 0 => LANES_COLUMN,
            Some("fill-reverse") if f & FILL_REVERSE == 0 => FILL_REVERSE,
            Some("track-reverse") if f & TRACK_REVERSE == 0 => TRACK_REVERSE,
            _ => return false,
        };
        f |= bit;
    }
    if v.len() > 1 && f & (LANES_ROW | LANES_COLUMN) == 0 {
        return false;
    }
    let keep = LANES_ON | crate::tree::TOLERANCE_INFINITE | crate::tree::DENSE;
    c.c.grid_lanes = (c.c.grid_lanes & keep) | f;
    true
}

/// `fit-tolerance: normal | infinite | <length-percentage [0,∞]>`.
fn fit_tolerance(c: &mut Ctx, v: &[Tok]) -> bool {
    let inf = crate::tree::TOLERANCE_INFINITE;
    match (v, kw(v)) {
        (_, Some("normal")) => c.c.fit_tolerance = None,
        (_, Some("infinite")) => {
            c.c.grid_lanes |= inf;
            return true;
        }
        ([t], _) => match length(t) {
            Some(l) => {
                let l = lp(c, l);
                let raw = l.into_raw();
                if !raw.is_plain_calc() && raw.value() < 0.0 {
                    return false;
                }
                c.c.fit_tolerance = Some(l);
            }
            None => return false,
        },
        _ => return false,
    }
    c.c.grid_lanes &= !inf;
    true
}

/// The kit's I-beam cursor (`cursor: text`, and the pointer over selectable text).
pub(crate) const CURSOR_TEXT: i32 = 3;

const CURSORS: &[(&str, i32)] = &[
    ("auto", -1), ("default", 0), ("pointer", 5), ("text", CURSOR_TEXT), ("vertical-text", CURSOR_TEXT),
    ("crosshair", 4), ("move", 6), ("all-scroll", 6), ("grab", 6), ("grabbing", 6),
    ("ew-resize", 1), ("e-resize", 1), ("w-resize", 1), ("col-resize", 1),
    ("ns-resize", 2), ("n-resize", 2), ("s-resize", 2), ("row-resize", 2),
    ("none", 7), ("not-allowed", 0), ("wait", 0), ("progress", 0), ("help", 0),
    ("context-menu", 0), ("cell", 4), ("copy", 0), ("alias", 0), ("no-drop", 0),
    ("nesw-resize", 6), ("nwse-resize", 6), ("ne-resize", 6), ("nw-resize", 6),
    ("se-resize", 6), ("sw-resize", 6), ("zoom-in", 4), ("zoom-out", 4),
];

fn roman(mut n: usize, upper: bool) -> String {
    let table = [(1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"),
                 (50, "l"), (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")];
    let mut s = String::new();
    for (v, r) in table {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    if upper { s.to_uppercase() } else { s }
}

impl ListStyle {
    /// A counter value in this style, without the marker's trailing dot
    /// (`counter()` in `content`).
    pub fn format(self, n: i32) -> String {
        let alpha = |upper: bool| {
            if n < 1 {
                return n.to_string();
            }
            // bijective base 26: a..z, aa, ab ...
            let (mut k, mut out) = (n as u32, Vec::new());
            while k > 0 {
                k -= 1;
                out.push(((if upper { b'A' } else { b'a' }) + (k % 26) as u8) as char);
                k /= 26;
            }
            out.iter().rev().collect()
        };
        match self {
            ListStyle::None => String::new(),
            ListStyle::Disc => "\u{2022}".into(),
            ListStyle::Circle => "\u{25e6}".into(),
            ListStyle::Square => "\u{25aa}".into(),
            ListStyle::Decimal => n.to_string(),
            ListStyle::LowerAlpha => alpha(false),
            ListStyle::UpperAlpha => alpha(true),
            ListStyle::LowerRoman | ListStyle::UpperRoman if (1..4000).contains(&n) => {
                roman(n as usize, self == ListStyle::UpperRoman)
            }
            _ => n.to_string(),
        }
    }

    /// The marker text for the `index`-th (1-based) item.
    pub fn marker(self, index: usize) -> Option<String> {
        Some(match self {
            ListStyle::None => return None,
            ListStyle::Disc => "\u{2022}".into(),
            ListStyle::Circle => "\u{25e6}".into(),
            ListStyle::Square => "\u{25aa}".into(),
            ListStyle::Decimal => format!("{index}."),
            ListStyle::LowerAlpha => format!("{}.", (b'a' + ((index - 1) % 26) as u8) as char),
            ListStyle::UpperAlpha => format!("{}.", (b'A' + ((index - 1) % 26) as u8) as char),
            ListStyle::LowerRoman => format!("{}.", roman(index, false)),
            ListStyle::UpperRoman => format!("{}.", roman(index, true)),
        })
    }
}

fn list_style(s: &str) -> Option<ListStyle> {
    Some(match s {
        "none" => ListStyle::None,
        "disc" => ListStyle::Disc,
        "circle" => ListStyle::Circle,
        "square" => ListStyle::Square,
        "decimal" | "decimal-leading-zero" => ListStyle::Decimal,
        "lower-alpha" | "lower-latin" => ListStyle::LowerAlpha,
        "upper-alpha" | "upper-latin" => ListStyle::UpperAlpha,
        "lower-roman" => ListStyle::LowerRoman,
        "upper-roman" => ListStyle::UpperRoman,
        _ => return None,
    })
}

/// An angle in radians (`45deg`, `0.5turn`, `1rad`, `100grad`, a bare 0).
fn angle(t: &Tok) -> Option<f32> {
    match t {
        Tok::Num(n) if *n == 0.0 => Some(0.0),
        Tok::Dim(v, u) => Some(match u.as_ref() {
            "deg" => v.to_radians(),
            "rad" => *v,
            "turn" => v * std::f32::consts::TAU,
            "grad" => v * std::f32::consts::PI / 200.0,
            _ => return None,
        }),
        _ => None,
    }
}

fn add_len(a: Length, b: Length) -> Length {
    match (a, b) {
        (Length::Px(x), Length::Px(y)) => Length::Px(x + y),
        (Length::Px(x), o) | (o, Length::Px(x)) if x == 0.0 => o,
        (_, o) => o,
    }
}

/// A `transform` function list.
fn transform_list(c: &Ctx, v: &[Tok]) -> Option<Vec<crate::effects::TransformFn>> {
    use crate::effects::TransformFn as T;
    let mut out = Vec::new();
    for t in v {
        let Tok::Func(fx) = t else { return None };
        let args: Vec<&Tok> = fx.args.iter().flatten().filter(|t| !matches!(t, Tok::Comma)).collect();
        let num = |i: usize| args.get(i).and_then(|t| number(t));
        let len = |i: usize| args.get(i).and_then(|t| length(t));
        let ang = |i: usize| args.get(i).and_then(|t| angle(t));
        let _ = c;
        out.push(match fx.name.as_ref() {
            "translate" | "translate3d" => T::Translate(len(0)?, len(1).unwrap_or(Length::Px(0.0))),
            "translatex" => T::Translate(len(0)?, Length::Px(0.0)),
            "translatey" => T::Translate(Length::Px(0.0), len(0)?),
            "translatez" => continue,
            "scale" | "scale3d" => {
                let x = num(0).or_else(|| match args.first() { Some(Tok::Pct(p)) => Some(p / 100.0), _ => None })?;
                T::Scale(x, num(1).unwrap_or(x))
            }
            "scalex" => T::Scale(num(0)?, 1.0),
            "scaley" => T::Scale(1.0, num(0)?),
            "scalez" => continue,
            "rotate" | "rotatez" => T::Rotate(ang(0)?),
            "skew" => T::Skew(ang(0)?, ang(1).unwrap_or(0.0)),
            "skewx" => T::Skew(ang(0)?, 0.0),
            "skewy" => T::Skew(0.0, ang(0)?),
            "matrix" => {
                let m: Option<Vec<f32>> = (0..6).map(num).collect();
                let m = m?;
                T::Matrix([m[0], m[1], m[2], m[3], m[4], m[5]])
            }
            // 3D functions without a 2D meaning: no effect on the 2D plane
            "perspective" | "rotatex" | "rotatey" | "rotate3d" | "matrix3d" => continue,
            _ => return None,
        });
    }
    Some(out)
}

/// A `filter` / `backdrop-filter` value.
fn filter_list(c: &Ctx, v: &[Tok]) -> Option<Vec<crate::effects::Filter>> {
    use crate::effects::Filter as F;
    if kw(v) == Some("none") {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for t in v {
        let Tok::Func(fx) = t else { return None };
        let args: Vec<&Tok> = fx.args.iter().flatten().filter(|t| !matches!(t, Tok::Comma)).collect();
        // amounts: numbers or percentages, default 1
        let amount = |d: f32| match args.first() {
            None => Some(d),
            Some(Tok::Num(n)) => Some(*n),
            Some(Tok::Pct(p)) => Some(p / 100.0),
            _ => None,
        };
        out.push(match fx.name.as_ref() {
            "blur" => F::Blur(args.first().map_or(Some(0.0), |t| length(t).map(|l| px(c, l)))?),
            "brightness" => F::Brightness(amount(1.0)?),
            "contrast" => F::Contrast(amount(1.0)?),
            "grayscale" => F::Grayscale(amount(1.0)?),
            "invert" => F::Invert(amount(1.0)?),
            "opacity" => F::Opacity(amount(1.0)?),
            "saturate" => F::Saturate(amount(1.0)?),
            "sepia" => F::Sepia(amount(1.0)?),
            "hue-rotate" => F::HueRotate(args.first().map_or(Some(0.0), |t| angle(t))?.to_degrees()),
            "drop-shadow" => {
                let lens: Vec<f32> = args.iter().filter_map(|t| match t {
                    Tok::Dim(..) | Tok::Num(_) | Tok::Calc(_) => length(t).map(|l| px(c, l)),
                    _ => None,
                }).collect();
                let color = args.iter().find_map(|t| match t {
                    Tok::Dim(..) | Tok::Num(_) => None,
                    t => color(t, c.c.color),
                }).unwrap_or(c.c.color);
                match lens.as_slice() {
                    [x, y] => F::DropShadow { x: *x, y: *y, blur: 0.0, color },
                    [x, y, b, ..] => F::DropShadow { x: *x, y: *y, blur: *b, color },
                    _ => return None,
                }
            }
            _ => return None, // url(#svg-filter): not supported
        });
    }
    Some(out)
}

/// A `clip-path` basic shape.
fn clip_shape(c: &Ctx, t: &Tok) -> Option<crate::effects::ClipShape> {
    use crate::effects::{ClipShape as S, ShapeRadius as R};
    let Tok::Func(fx) = t else { return None };
    let flat: Vec<Tok> = fx.args.iter().flatten().cloned().collect();
    // split at `at` (circle / ellipse) and `round` (inset)
    let split = |word: &str| -> (Vec<Tok>, Vec<Tok>) {
        match flat.iter().position(|t| t.ident() == Some(word)) {
            Some(k) => (flat[..k].to_vec(), flat[k + 1..].to_vec()),
            None => (flat.clone(), Vec::new()),
        }
    };
    let radius = |t: &Tok| match t.ident() {
        Some("closest-side") => Some(R::Closest),
        Some("farthest-side") => Some(R::Farthest),
        _ => length(t).map(R::Len),
    };
    let center = |rest: &[Tok]| if rest.is_empty() { Some([Pos::CENTER; 2]) } else { position(c, rest) };
    Some(match fx.name.as_ref() {
        "inset" => {
            let (sides, round) = split("round");
            let l: Option<Vec<Length>> = sides.iter().map(length).collect();
            let s = four(&l?)?;
            let r = round.first().and_then(length).unwrap_or(Length::Px(0.0));
            S::Inset(s, r)
        }
        "circle" => {
            let (r, at) = split("at");
            S::Circle(r.first().map_or(Some(R::Closest), radius)?, center(&at)?)
        }
        "ellipse" => {
            let (r, at) = split("at");
            let rx = r.first().map_or(Some(R::Closest), radius)?;
            let ry = r.get(1).map_or(Some(R::Closest), radius)?;
            S::Ellipse(rx, ry, center(&at)?)
        }
        "polygon" => {
            let mut pts = Vec::new();
            for group in &fx.args {
                let ls: Vec<Length> = group.iter().filter_map(length).collect();
                if let [x, y] = ls.as_slice() {
                    pts.push((*x, *y));
                }
            }
            if pts.len() < 3 {
                return None;
            }
            S::Polygon(pts)
        }
        _ => return None,
    })
}

/// A mask / border-image source: a gradient or `url()` (PNG).
fn image_source(c: &Ctx, t: &Tok) -> Option<crate::effects::ImageSource> {
    if let Tok::Func(fx) = t {
        if fx.name == "url" {
            if let Some(Tok::Str(u)) = fx.args.first().and_then(|a| a.first()) {
                return Some(crate::effects::ImageSource::Url(u.as_ref().into()));
            }
            return None;
        }
    }
    gradient(c, t).map(crate::effects::ImageSource::Gradient)
}

fn four_sides<T: Copy>(v: &[T]) -> Option<[T; 4]> {
    Some(match v {
        [a] => [*a; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return None,
    })
}

/// `border-image-width` / `-outset`: numbers (border widths), lengths, `auto`.
fn side_values(v: &[Tok]) -> Option<[crate::effects::Side; 4]> {
    use crate::effects::Side;
    let vals: Option<Vec<Side>> = v.iter().map(|t| match t {
        Tok::Ident(k) if k.as_ref() == "auto" => Some(Side::Auto),
        Tok::Num(n) => Some(Side::Num(*n)),
        t => length(t).map(Side::Len),
    }).collect();
    four_sides(&vals?)
}

fn deco_style(s: &str) -> Option<DecoStyle> {
    Some(match s {
        "solid" => DecoStyle::Solid,
        "double" => DecoStyle::Double,
        "dotted" => DecoStyle::Dotted,
        "dashed" => DecoStyle::Dashed,
        "wavy" => DecoStyle::Wavy,
        _ => return None,
    })
}

/// `none | [<name> <integer>?]+` for the `counter-*` properties.
fn counter_list(v: &[Tok], default: i32) -> Option<Vec<(Box<str>, i32)>> {
    if kw(v) == Some("none") {
        return Some(Vec::new());
    }
    let mut out: Vec<(Box<str>, i32)> = Vec::new();
    for t in v {
        match t {
            Tok::Ident(s) => out.push((s.as_ref().into(), default)),
            Tok::Num(n) => out.last_mut()?.1 = *n as i32,
            Tok::Func(fx) if fx.name == "reversed" => {
                let name = fx.args.first().and_then(|a| a.first()).and_then(Tok::ident)?;
                out.push((name.into(), default));
            }
            _ => return None,
        }
    }
    (!out.is_empty()).then_some(out)
}

fn overflow(s: &str) -> Option<Overflow> {
    Some(match s {
        "visible" => Overflow::Visible,
        "hidden" => Overflow::Hidden,
        "clip" => Overflow::Clip,
        "scroll" | "auto" | "overlay" => Overflow::Scroll,
        _ => return None,
    })
}

/// Run another property's handler (shorthands, logical aliases).
fn delegate(ctx: &mut Ctx, name: &str, v: &[Tok]) -> bool {
    handlers().get(name).is_some_and(|h| h(ctx, v))
}

macro_rules! side_handlers {
    ($setter:ident, $top:literal, $right:literal, $bottom:literal, $left:literal) => {
        [
            ($top, (|c: &mut Ctx, v: &[Tok]| v.first().and_then(length).map(|l| $setter(c, 0, l)).is_some()) as Handler),
            ($right, (|c: &mut Ctx, v: &[Tok]| v.first().and_then(length).map(|l| $setter(c, 1, l)).is_some()) as Handler),
            ($bottom, (|c: &mut Ctx, v: &[Tok]| v.first().and_then(length).map(|l| $setter(c, 2, l)).is_some()) as Handler),
            ($left, (|c: &mut Ctx, v: &[Tok]| v.first().and_then(length).map(|l| $setter(c, 3, l)).is_some()) as Handler),
        ]
    };
}

/// The property table. A `const`, so style methods resolve property names to
/// indices at compile time ([`Prop::named`]).
pub(crate) const HANDLERS: &[(&str, Handler)] = &{
    const fn h(n: &'static str, f: Handler) -> (&'static str, Handler) {
        (n, f)
    }
    // The table: [box model, positioning, flexbox, grid, alignment, overflow,
    // background, border, effects, text, interaction]. Every entry changes
    // layout or rendering.
    let margin = side_handlers!(set_margin, "margin-top", "margin-right", "margin-bottom", "margin-left");
    let padding = side_handlers!(set_padding, "padding-top", "padding-right", "padding-bottom", "padding-left");
    [
        // -- box model --------------------------------------------------------------
        h("display", display),
        h("float", |c, v| {
            let rtl = c.c.layout.direction == taffy::style::Direction::Rtl;
            c.c.layout.float = match kw(v) {
                Some("left") => taffy::style::Float::Left,
                Some("right") => taffy::style::Float::Right,
                Some("inline-start") => if rtl { taffy::style::Float::Right } else { taffy::style::Float::Left },
                Some("inline-end") => if rtl { taffy::style::Float::Left } else { taffy::style::Float::Right },
                Some("none") => taffy::style::Float::None,
                _ => return false,
            };
            true
        }),
        h("clear", |c, v| {
            let rtl = c.c.layout.direction == taffy::style::Direction::Rtl;
            c.c.layout.clear = match kw(v) {
                Some("left") => taffy::style::Clear::Left,
                Some("right") => taffy::style::Clear::Right,
                Some("inline-start") => if rtl { taffy::style::Clear::Right } else { taffy::style::Clear::Left },
                Some("inline-end") => if rtl { taffy::style::Clear::Left } else { taffy::style::Clear::Right },
                Some("both" | "both-inline" | "both-block") => taffy::style::Clear::Both,
                Some("none") => taffy::style::Clear::None,
                _ => return false,
            };
            true
        }),
        h("direction", |c, v| {
            c.c.layout.direction = match kw(v) {
                Some("ltr") => taffy::style::Direction::Ltr,
                Some("rtl") => taffy::style::Direction::Rtl,
                _ => return false,
            };
            true
        }),
        h("contain", |c, v| {
            use taffy::style::Contain;
            use crate::tree::{CONTAIN_HEIGHT, CONTAIN_WIDTH};
            let mut bits = Contain::NONE;
            let mut size = 0;
            for w in v.iter().filter_map(Tok::ident) {
                (bits, size) = match w {
                    "none" => (Contain::NONE, 0),
                    "layout" => (bits | Contain::LAYOUT, size),
                    "paint" => (bits | Contain::PAINT, size),
                    "content" => (bits | Contain::CONTENT, size),
                    "strict" => (bits | Contain::CONTENT, CONTAIN_WIDTH | CONTAIN_HEIGHT),
                    "size" => (bits, CONTAIN_WIDTH | CONTAIN_HEIGHT),
                    "inline-size" => (bits, size | CONTAIN_WIDTH),
                    "style" => (bits, size),
                    _ => return false,
                };
            }
            c.c.layout.contain = bits;
            c.c.contain.0 = size;
            true
        }),
        // `none | <length> | auto <length> | auto none` (the remembered size, `auto`, is the length)
        h("contain-intrinsic-size", |c, v| {
            let sizes: Option<Vec<f32>> = intrinsic_sizes(c, v);
            match sizes.as_deref() {
                Some(&[w]) => c.c.contain.1 = [w, w],
                Some(&[w, h]) => c.c.contain.1 = [w, h],
                _ => return false,
            }
            true
        }),
        h("contain-intrinsic-width", |c, v| intrinsic_sizes(c, v).and_then(|s| s.first().copied()).map(|w| c.c.contain.1[0] = w).is_some()),
        h("contain-intrinsic-height", |c, v| intrinsic_sizes(c, v).and_then(|s| s.first().copied()).map(|h| c.c.contain.1[1] = h).is_some()),
        h("contain-intrinsic-inline-size", |c, v| delegate(c, "contain-intrinsic-width", v)),
        h("contain-intrinsic-block-size", |c, v| delegate(c, "contain-intrinsic-height", v)),
        h("box-sizing", |c, v| {
            c.c.layout.box_sizing = match kw(v) {
                Some("content-box") => BoxSizing::ContentBox,
                Some("border-box") => BoxSizing::BorderBox,
                _ => return false,
            };
            true
        }),
        h("width", |c, v| size_prop(c, 0, v, |c, v| dim_kw(c, v).map(|d| c.c.layout.size.width = d).is_some())),
        h("height", |c, v| size_prop(c, 1, v, |c, v| dim_kw(c, v).map(|d| c.c.layout.size.height = d).is_some())),
        h("min-width", |c, v| size_prop(c, 2, v, |c, v| min_max(c, 0, v).map(|d| c.c.layout.min_size.width = d).is_some())),
        h("min-height", |c, v| size_prop(c, 3, v, |c, v| min_max(c, 1, v).map(|d| c.c.layout.min_size.height = d).is_some())),
        h("max-width", |c, v| size_prop(c, 4, v, |c, v| min_max(c, 2, v).map(|d| c.c.layout.max_size.width = d).is_some())),
        h("max-height", |c, v| size_prop(c, 5, v, |c, v| min_max(c, 3, v).map(|d| c.c.layout.max_size.height = d).is_some())),
        h("inline-size", |c, v| delegate(c, "width", v)),
        h("block-size", |c, v| delegate(c, "height", v)),
        h("min-inline-size", |c, v| delegate(c, "min-width", v)),
        h("min-block-size", |c, v| delegate(c, "min-height", v)),
        h("max-inline-size", |c, v| delegate(c, "max-width", v)),
        h("max-block-size", |c, v| delegate(c, "max-height", v)),
        h("aspect-ratio", |c, v| {
            let nums: Vec<f32> = v.iter().filter_map(number).collect();
            // `auto && <ratio>` uses the ratio (on the box-sizing box, as taffy does, not only the
            // content box); a ratio with a zero term is degenerate, as `auto`
            let auto = v.iter().any(|t| t.ident() == Some("auto"));
            c.c.aspect_auto = auto;
            c.c.layout.aspect_ratio = match nums.as_slice() {
                [] if auto && v.len() == 1 => None,
                [w, h] => (*w > 0.0 && *h > 0.0).then(|| w / h),
                [r] => (*r > 0.0).then_some(*r),
                _ => return false,
            };
            true
        }),
        margin[0], margin[1], margin[2], margin[3],
        h("margin", |c, v| lengths(v).and_then(|l| four(&l)).map(|s| {
            (0..4).for_each(|i| set_margin(c, i, s[i]))
        }).is_some()),
        h("margin-inline-start", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "margin-right" } else { "margin-left" }, v)),
        h("margin-inline-end", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "margin-left" } else { "margin-right" }, v)),
        h("margin-block-start", |c, v| delegate(c, "margin-top", v)),
        h("margin-block-end", |c, v| delegate(c, "margin-bottom", v)),
        h("margin-inline", |c, v| if c.c.layout.direction == taffy::style::Direction::Rtl { pair(c, v, "margin-right", "margin-left") } else { pair(c, v, "margin-left", "margin-right") }),
        h("margin-block", |c, v| pair(c, v, "margin-top", "margin-bottom")),
        padding[0], padding[1], padding[2], padding[3],
        h("padding", |c, v| lengths(v).and_then(|l| four(&l)).map(|s| {
            (0..4).for_each(|i| set_padding(c, i, s[i]))
        }).is_some()),
        h("padding-inline-start", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "padding-right" } else { "padding-left" }, v)),
        h("padding-inline-end", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "padding-left" } else { "padding-right" }, v)),
        h("padding-block-start", |c, v| delegate(c, "padding-top", v)),
        h("padding-block-end", |c, v| delegate(c, "padding-bottom", v)),
        h("padding-inline", |c, v| if c.c.layout.direction == taffy::style::Direction::Rtl { pair(c, v, "padding-right", "padding-left") } else { pair(c, v, "padding-left", "padding-right") }),
        h("padding-block", |c, v| pair(c, v, "padding-top", "padding-bottom")),
        // -- positioning ------------------------------------------------------------
        h("position", |c, v| {
            c.c.layout.position = match kw(v) {
                Some("static") => Position::Static,
                Some("relative") => Position::Relative,
                Some("sticky") | Some("-webkit-sticky") => Position::Sticky,
                Some("absolute") => Position::Absolute,
                Some("fixed") => Position::Fixed,
                _ => return false,
            };
            true
        }),
        h("top", |c, v| inset_side(c, 0, v)),
        h("right", |c, v| inset_side(c, 1, v)),
        h("bottom", |c, v| inset_side(c, 2, v)),
        h("left", |c, v| inset_side(c, 3, v)),
        h("anchor-name", |c, v| {
            if kw(v) == Some("none") {
                if let Some(a) = c.c.anchor.as_deref_mut() {
                    a.names = Box::default();
                }
                return true;
            }
            let names: Option<Vec<Box<str>>> = v.iter().filter(|t| !matches!(t, Tok::Comma))
                .map(|t| t.ident().filter(|s| s.starts_with("--")).map(Box::from)).collect();
            match names {
                Some(n) if !n.is_empty() => {
                    anchoring(c).names = n.into();
                    true
                }
                _ => false,
            }
        }),
        h("position-visibility", |c, v| crate::anchor::parse_visibility(v).map(|b| anchoring(c).visibility = Some(b)).is_some()),
        h("position-anchor", |c, v| {
            let (default, implicit) = match kw(v) {
                Some("auto") => (None, Some(true)),
                Some("none") => (None, Some(false)),
                Some("normal") => (None, None),
                Some(n) if n.starts_with("--") => (Some(n.into()), None),
                _ => return false,
            };
            let a = anchoring(c);
            (a.default, a.implicit) = (default, implicit);
            true
        }),
        h("position-area", |c, v| match crate::anchor::Area::parse(v) {
            Some(area) => {
                if area.is_some() || c.c.anchor.is_some() {
                    anchoring(c).area = area;
                }
                true
            }
            None => false,
        }),
        h("position-try-fallbacks", |c, v| match crate::anchor::parse_fallbacks(v) {
            Some(f) => {
                if !f.is_empty() || c.c.anchor.is_some() {
                    anchoring(c).fallbacks = f;
                }
                true
            }
            None => false,
        }),
        h("position-try", |c, v| delegate(c, "position-try-fallbacks", v)),
        h("view-transition-name", |c, v| match crate::transition::Named::parse_name(v) {
            Some(None) => {
                c.c.view_transition = None;
                true
            }
            Some(Some(name)) => {
                let classes = c.c.view_transition.take().map(|n| n.classes).unwrap_or_default();
                c.c.view_transition = Some(Box::new(crate::transition::Named { name, classes }));
                true
            }
            None => false,
        }),
        h("view-transition-class", |c, v| match crate::transition::Named::parse_classes(v) {
            Some(classes) => {
                if let Some(n) = c.c.view_transition.as_deref_mut() {
                    n.classes = classes;
                }
                true
            }
            None => false,
        }),
        h("inset", |c, v| lengths(v).and_then(|l| four(&l)).map(|s| {
            (0..4).for_each(|i| set_inset(c, i, s[i]))
        }).is_some()),
        h("inset-inline-start", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "right" } else { "left" }, v)),
        h("inset-inline-end", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "left" } else { "right" }, v)),
        h("inset-block-start", |c, v| delegate(c, "top", v)),
        h("inset-block-end", |c, v| delegate(c, "bottom", v)),
        h("inset-inline", |c, v| if c.c.layout.direction == taffy::style::Direction::Rtl { pair(c, v, "right", "left") } else { pair(c, v, "left", "right") }),
        h("inset-block", |c, v| pair(c, v, "top", "bottom")),
        h("z-index", |c, v| {
            c.c.z_index = match v.first() {
                Some(Tok::Num(n)) => Some(*n as i32),
                Some(Tok::Ident(s)) if s == "auto" => None,
                _ => return false,
            };
            true
        }),
        // -- flexbox --------------------------------------------------------------------
        h("flex-direction", |c, v| {
            c.c.layout.flex_direction = match kw(v) {
                Some("row") => FlexDirection::Row,
                Some("row-reverse") => FlexDirection::RowReverse,
                Some("column") => FlexDirection::Column,
                Some("column-reverse") => FlexDirection::ColumnReverse,
                _ => return false,
            };
            true
        }),
        // `nowrap | wrap | wrap-reverse`, or `balance` with `wrap-reverse` or not, in either order
        h("flex-wrap", |c, v| {
            let words: Vec<&str> = v.iter().filter_map(Tok::ident).collect();
            if words.len() != v.len() || words.is_empty() {
                return false;
            }
            let has = |w: &str| words.contains(&w);
            c.c.layout.flex_wrap = match (words.len(), has("balance"), has("wrap-reverse")) {
                (1, false, false) if has("nowrap") => FlexWrap::NoWrap,
                (1, false, false) if has("wrap") => FlexWrap::Wrap,
                (1, false, true) => FlexWrap::WrapReverse,
                (1, true, _) => FlexWrap::Balance,
                (2, true, true) => FlexWrap::BalanceReverse,
                _ => return false,
            };
            true
        }),
        h("flex-line-count", |c, v| match v {
            [Tok::Num(n)] if *n >= 1.0 && n.fract() == 0.0 => {
                c.c.layout.flex_line_count = n.min(f32::from(u16::MAX)) as u16;
                true
            }
            _ => false,
        }),
        h("flex-flow", |c, v| {
            // the direction keyword alone; the rest is the wrap
            let (dir, wrap): (Vec<Tok>, Vec<Tok>) = v.iter().cloned()
                .partition(|t| t.ident().is_some_and(|w| w.starts_with("row") || w.starts_with("column")));
            dir.len() <= 1 && (dir.is_empty() || delegate(c, "flex-direction", &dir))
                && (wrap.is_empty() || delegate(c, "flex-wrap", &wrap))
        }),
        // non-negative <number>s
        h("flex-grow", |c, v| v.first().and_then(number).filter(|n| *n >= 0.0).map(|n| c.c.layout.flex_grow = n).is_some()),
        h("flex-shrink", |c, v| v.first().and_then(number).filter(|n| *n >= 0.0).map(|n| c.c.layout.flex_shrink = n).is_some()),
        h("flex-basis", |c, v| match kw(v) {
            Some("content") => {
                c.c.layout.flex_basis = Dimension::content();
                true
            }
            _ => dim_kw(c, v).map(|d| c.c.layout.flex_basis = d).is_some(),
        }),
        h("flex", |c, v| {
            if v.iter().any(|t| matches!(t, Tok::Num(n) if *n < 0.0)) {
                return false;
            }
            let l = &mut c.c.layout;
            match (kw(v), v) {
                (Some("none"), _) => (l.flex_grow, l.flex_shrink, l.flex_basis) = (0.0, 0.0, Dimension::auto()),
                (Some("auto"), _) => (l.flex_grow, l.flex_shrink, l.flex_basis) = (1.0, 1.0, Dimension::auto()),
                (Some("initial"), _) => (l.flex_grow, l.flex_shrink, l.flex_basis) = (0.0, 1.0, Dimension::auto()),
                (_, [Tok::Num(g)]) => (l.flex_grow, l.flex_shrink, l.flex_basis) = (*g, 1.0, Dimension::percent(0.0)),
                (_, [Tok::Num(g), Tok::Num(s)]) => (l.flex_grow, l.flex_shrink, l.flex_basis) = (*g, *s, Dimension::percent(0.0)),
                (_, [Tok::Num(g), Tok::Num(s), b]) => {
                    let Some(len) = length(b) else { return false };
                    let d = dim(c, len);
                    let l = &mut c.c.layout;
                    (l.flex_grow, l.flex_shrink, l.flex_basis) = (*g, *s, d);
                }
                (_, [Tok::Num(g), b]) => {
                    let Some(len) = length(b) else { return false };
                    let d = dim(c, len);
                    let l = &mut c.c.layout;
                    (l.flex_grow, l.flex_shrink, l.flex_basis) = (*g, 1.0, d);
                }
                (_, [b]) => {
                    let Some(len) = length(b) else { return false };
                    let d = dim(c, len);
                    let l = &mut c.c.layout;
                    (l.flex_grow, l.flex_shrink, l.flex_basis) = (1.0, 1.0, d);
                }
                _ => return false,
            }
            true
        }),
        // `word-space-transform: none | [space | ideographic-space] && auto-phrase?` (the phrase
        // finding, a dictionary's, is not done)
        h("word-space-transform", |c, v| {
            let words: Option<Vec<&str>> = v.iter().map(Tok::ident).collect();
            let Some(words) = words else { return false };
            let mode = match words.iter().copied().filter(|w| *w != "auto-phrase").collect::<Vec<_>>().as_slice() {
                ["none"] if words.len() == 1 => 0,
                ["space"] => 1,
                ["ideographic-space"] => 2,
                [] if words == ["auto-phrase"] => 0,
                _ => return false,
            };
            c.c.word_space = mode;
            true
        }),
        h("white-space-trim", |c, v| {
            let mut bits = 0;
            for t in v {
                let bit = match t.ident() {
                    Some("none") if v.len() == 1 => 0,
                    Some("discard-before") => TRIM_WS_BEFORE,
                    Some("discard-after") => TRIM_WS_AFTER,
                    Some("discard-inner") => TRIM_WS_INNER,
                    _ => return false,
                };
                if bits & bit != 0 {
                    return false;
                }
                bits |= bit;
            }
            c.c.ws_trim = bits;
            true
        }),
        h("wrap-inside", |c, v| match kw(v) {
            Some(k @ ("auto" | "avoid")) => {
                c.c.wrap_avoid = k == "avoid";
                true
            }
            _ => false,
        }),
        h("margin-trim", |c, v| margin_trim(v).map(|t| c.c.margin_trim = t).is_some()),
        h("order", |c, v| match v.first() {
            // an <integer>
            Some(Tok::Num(n)) if n.fract() == 0.0 => {
                c.c.order = *n as i32;
                true
            }
            _ => false,
        }),
        // -- alignment ------------------------------------------------------------------
        h("align-items", |c, v| kw_align(v, false).map(|a| {
            c.c.layout.align_items = a;
            flow_relative(c, v, crate::tree::FLOW_ALIGN_ITEMS);
        }).is_some()),
        h("align-self", |c, v| match kw(v) {
            Some("auto") => {
                c.c.layout.align_self = None;
                true
            }
            _ => {
                anchor_center(c, v, 2);
                kw_align(v, false).map(|a| c.c.layout.align_self = Some(a)).is_some()
            }
        }),
        h("justify-items", |c, v| kw_align(v, true).map(|a| {
            c.c.layout.justify_items = a;
            physical(c, v, PHYS_JUSTIFY_ITEMS);
            flow_relative(c, v, crate::tree::FLOW_JUSTIFY_ITEMS);
        }).is_some()),
        h("justify-self", |c, v| match kw(v) {
            Some("auto") => {
                c.c.layout.justify_self = None;
                true
            }
            _ => {
                anchor_center(c, v, 1);
                physical(c, v, PHYS_JUSTIFY_SELF);
                kw_align(v, true).map(|a| c.c.layout.justify_self = Some(a)).is_some()
            }
        }),
        h("align-content", |c, v| kw_align_content(v, false).map(|a| {
            c.c.layout.align_content = a;
            flow_relative(c, v, crate::tree::FLOW_ALIGN_CONTENT);
        }).is_some()),
        h("justify-content", |c, v| kw_align_content(v, true).map(|a| {
            c.c.layout.justify_content = a;
            physical(c, v, PHYS_JUSTIFY_CONTENT);
            flow_relative(c, v, crate::tree::FLOW_JUSTIFY_CONTENT);
        }).is_some()),
        h("place-items", |c, v| two_kw(c, v, "align-items", "justify-items")),
        h("place-content", |c, v| two_kw(c, v, "align-content", "justify-content")),
        h("place-self", |c, v| two_kw(c, v, "align-self", "justify-self")),
        h("gap", |c, v| {
            let Some(l) = lengths(v) else { return false };
            let (r, col) = match l.as_slice() {
                [a] => (*a, *a),
                [a, b] => (*a, *b),
                _ => return false,
            };
            c.c.layout.gap = Size { width: lp(c, col), height: lp(c, r) };
            true
        }),
        h("row-gap", |c, v| v.first().and_then(length).map(|l| c.c.layout.gap.height = lp(c, l)).is_some()),
        h("column-gap", |c, v| match kw(v) {
            Some("normal") => true,
            _ => v.first().and_then(length).map(|l| c.c.layout.gap.width = lp(c, l)).is_some(),
        }),
        h("grid-gap", |c, v| delegate(c, "gap", v)),
        h("grid-row-gap", |c, v| delegate(c, "row-gap", v)),
        h("grid-column-gap", |c, v| delegate(c, "column-gap", v)),
        // -- grid -------------------------------------------------------------------------
        h("grid-template-columns", |c, v| track_list(c, v).map(|(t, n)| {
            c.c.layout.grid_template_columns = t;
            c.c.layout.grid_template_column_names = n;
            subgrid_axis(c, v, crate::tree::SUBGRID_COLUMNS);
        }).is_some()),
        h("grid-template-rows", |c, v| track_list(c, v).map(|(t, n)| {
            c.c.layout.grid_template_rows = t;
            c.c.layout.grid_template_row_names = n;
            subgrid_axis(c, v, crate::tree::SUBGRID_ROWS);
        }).is_some()),
        h("grid-lanes-direction", lanes_direction),
        h("fit-tolerance", fit_tolerance),
        h("grid-lanes-pack", |c, v| {
            let dense = match kw(v) {
                Some("normal") => false,
                Some("dense") => true,
                _ => return false,
            };
            c.c.grid_lanes = if dense { c.c.grid_lanes | crate::tree::DENSE } else { c.c.grid_lanes & !crate::tree::DENSE };
            true
        }),
        h("grid-template-areas", |c, v| template_areas(v).map(|a| c.c.layout.grid_template_areas = a).is_some()),
        h("grid-template", |c, v| {
            let parts: Vec<&[Tok]> = v.split(|t| matches!(t, Tok::Slash)).collect();
            // "a a" 40px "b b" 1fr / auto 1fr: area strings, each with its row size
            if parts[0].iter().any(|t| matches!(t, Tok::Str(_))) {
                let strs: Vec<Tok> = parts[0].iter().filter(|t| matches!(t, Tok::Str(_))).cloned().collect();
                let mut rows: Vec<Tok> = Vec::new();
                for (k, t) in parts[0].iter().enumerate() {
                    if matches!(t, Tok::Str(_)) {
                        match parts[0].get(k + 1) {
                            Some(n) if !matches!(n, Tok::Str(_)) => rows.push(n.clone()),
                            _ => rows.push(Tok::Ident("auto".into())),
                        }
                    }
                }
                return delegate(c, "grid-template-areas", &strs)
                    && delegate(c, "grid-template-rows", &rows)
                    && parts.get(1).is_none_or(|cols| delegate(c, "grid-template-columns", cols));
            }
            match parts.as_slice() {
                [rows, cols] => delegate(c, "grid-template-rows", rows)
                    && delegate(c, "grid-template-columns", cols),
                _ => delegate(c, "grid-template-rows", v),
            }
        }),
        h("grid", |c, v| {
            // <rows> / <columns> | auto-flow [dense] <rows> / <columns> | <rows> / auto-flow <columns>
            let parts: Vec<&[Tok]> = v.split(|t| matches!(t, Tok::Slash)).collect();
            let [rows, cols] = parts.as_slice() else { return false };
            let flow = |p: &[Tok]| p.iter().any(|t| t.ident() == Some("auto-flow"));
            let dense = v.iter().any(|t| t.ident() == Some("dense"));
            let strip = |p: &[Tok]| p.iter().filter(|t| !matches!(t.ident(), Some("auto-flow" | "dense")))
                .cloned().collect::<Vec<Tok>>();
            if flow(rows) {
                c.c.layout.grid_auto_flow = if dense { GridAutoFlow::RowDense } else { GridAutoFlow::Row };
                let auto = strip(rows);
                (auto.is_empty() || delegate(c, "grid-auto-rows", &auto))
                    && delegate(c, "grid-template-columns", cols)
            } else if flow(cols) {
                c.c.layout.grid_auto_flow = if dense { GridAutoFlow::ColumnDense } else { GridAutoFlow::Column };
                let auto = strip(cols);
                (auto.is_empty() || delegate(c, "grid-auto-columns", &auto))
                    && delegate(c, "grid-template-rows", rows)
            } else {
                delegate(c, "grid-template", v)
            }
        }),
        h("grid-auto-columns", |c, v| v.iter().map(|t| track(c, t)).collect::<Option<Vec<_>>>()
            .map(|t| c.c.layout.grid_auto_columns = t).is_some()),
        h("grid-auto-rows", |c, v| v.iter().map(|t| track(c, t)).collect::<Option<Vec<_>>>()
            .map(|t| c.c.layout.grid_auto_rows = t).is_some()),
        h("grid-auto-flow", |c, v| {
            let words: Vec<&str> = v.iter().filter_map(Tok::ident).collect();
            let dense = words.contains(&"dense");
            c.c.layout.grid_auto_flow = match (words.contains(&"column"), dense) {
                (true, true) => GridAutoFlow::ColumnDense,
                (true, false) => GridAutoFlow::Column,
                (false, true) => GridAutoFlow::RowDense,
                (false, false) => GridAutoFlow::Row,
            };
            true
        }),
        h("grid-row", |c, v| grid_line(v).map(|l| c.c.layout.grid_row = l).is_some()),
        h("grid-column", |c, v| grid_line(v).map(|l| c.c.layout.grid_column = l).is_some()),
        h("grid-row-start", |c, v| grid_line(v).map(|l| c.c.layout.grid_row.start = l.start).is_some()),
        h("grid-row-end", |c, v| grid_line(v).map(|l| c.c.layout.grid_row.end = l.start).is_some()),
        h("grid-column-start", |c, v| grid_line(v).map(|l| c.c.layout.grid_column.start = l.start).is_some()),
        h("grid-column-end", |c, v| grid_line(v).map(|l| c.c.layout.grid_column.end = l.start).is_some()),
        h("grid-area", |c, v| {
            let parts: Vec<&[Tok]> = v.split(|t| matches!(t, Tok::Slash)).collect();
            let get = |i: usize| parts.get(i).and_then(|p| grid_line(p)).map(|l| l.start);
            // an omitted part copies the matching earlier one when that is a
            // name (`grid-area: head` = head / head / head / head), else auto
            let named = |p: &Option<GridPlacement<Ident>>| match p {
                Some(GridPlacement::NamedLine(id, 0)) => GridPlacement::NamedLine(id.clone(), 0),
                _ => GridPlacement::Auto,
            };
            match (get(0), get(1), get(2), get(3)) {
                (Some(rs), cs, re, ce) => {
                    let rs = Some(rs);
                    let cs = cs.or_else(|| Some(named(&rs)));
                    let re = re.unwrap_or_else(|| named(&rs));
                    let ce = ce.unwrap_or_else(|| named(&cs));
                    c.c.layout.grid_row = Line { start: rs.unwrap(), end: re };
                    c.c.layout.grid_column = Line { start: cs.unwrap(), end: ce };
                    true
                }
                _ => false,
            }
        }),
        // -- overflow ---------------------------------------------------------------------
        h("overflow-x", |c, v| kw(v).and_then(overflow).map(|o| c.c.layout.overflow.x = o).is_some()),
        h("overflow-y", |c, v| kw(v).and_then(overflow).map(|o| c.c.layout.overflow.y = o).is_some()),
        h("overflow", |c, v| {
            let o: Option<Vec<Overflow>> = v.iter().filter_map(Tok::ident).map(overflow).collect();
            match o.as_deref() {
                Some([a]) => c.c.layout.overflow = taffy::Point { x: *a, y: *a },
                Some([a, b]) => c.c.layout.overflow = taffy::Point { x: *a, y: *b },
                _ => return false,
            }
            true
        }),
        h("overflow-inline", |c, v| delegate(c, "overflow-x", v)),
        h("overflow-block", |c, v| delegate(c, "overflow-y", v)),
        h("scrollbar-width", |c, v| {
            c.c.layout.scrollbar_width = match kw(v) {
                Some("none") => 0.0,
                Some("thin") => 6.0,
                Some("auto") => 8.0,
                _ => return v.first().and_then(length).map(|l| c.c.layout.scrollbar_width = px(c, l)).is_some(),
            };
            true
        }),
        h("text-overflow", |c, v| {
            c.c.ellipsis = kw(v) == Some("ellipsis");
            true
        }),
        // -- background -------------------------------------------------------------------
        h("background-color", |c, v| {
            let Some(col) = v.first().and_then(|t| color(t, c.c.color)) else { return false };
            c.c.background = col;
            // a colour of `currentcolor` stays one: resolved with the element's own `color` once the
            // cascade ends, and inherited as such (CSS Color 4 6.4)
            c.c.background_current = v.first().filter(|t| uses_currentcolor(t)).map(|t| Box::new(t.clone()));
            true
        }),
        h("background-image", |c, v| match kw(v) {
            Some("none") => {
                c.c.gradient = None;
                if let Some(i) = c.c.images.as_mut() {
                    i.bg_url = None;
                }
                true
            }
            _ => {
                // the first url() and the first gradient of the layer list
                let mut ok = false;
                for t in v {
                    match t {
                        Tok::Func(fx) if fx.name == "url" => {
                            if let Some(Tok::Str(u)) = fx.args.first().and_then(|a| a.first()) {
                                if c.c.images.as_ref().is_none_or(|i| i.bg_url.is_none()) || !ok {
                                    c.c.images_mut().bg_url = Some(u.as_ref().into());
                                }
                                ok = true;
                            }
                        }
                        _ => {
                            if let Some(g) = gradient(c, t) {
                                c.c.gradient.get_or_insert(g);
                                ok = true;
                            }
                        }
                    }
                }
                ok
            }
        }),
        h("background-size", |c, v| {
            let first = v.split(|t| matches!(t, Tok::Comma)).next().unwrap_or(&[]);
            let size = match (kw(first), first) {
                (Some("cover"), _) => BgSize::Cover,
                (Some("contain"), _) => BgSize::Contain,
                (_, [a]) => BgSize::Size(size_pos(c, a), None),
                (_, [a, b]) => BgSize::Size(size_pos(c, a), size_pos(c, b)),
                _ => return false,
            };
            let size = match size {
                BgSize::Size(None, None) => BgSize::Auto,
                s => s,
            };
            c.c.images_mut().bg_size = size;
            true
        }),
        h("background-position", |c, v| {
            let first = v.split(|t| matches!(t, Tok::Comma)).next().unwrap_or(&[]);
            position(c, first).map(|p| c.c.images_mut().bg_position = p).is_some()
        }),
        h("background-position-x", |c, v| axis_pos(c, v, 0).map(|p| c.c.images_mut().bg_position[0] = p).is_some()),
        h("background-position-y", |c, v| axis_pos(c, v, 1).map(|p| c.c.images_mut().bg_position[1] = p).is_some()),
        h("background-repeat", |c, v| {
            let words: Vec<&str> = v.iter().take_while(|t| !matches!(t, Tok::Comma)).filter_map(Tok::ident).collect();
            let rep = |w: &str| match w {
                "repeat" => Some((true, REPEAT_PLAIN)),
                "space" => Some((true, REPEAT_SPACE)),
                "round" => Some((true, REPEAT_ROUND)),
                "no-repeat" => Some((false, REPEAT_PLAIN)),
                _ => None,
            };
            let (x, y) = match words.as_slice() {
                ["repeat-x"] => ((true, REPEAT_PLAIN), (false, REPEAT_PLAIN)),
                ["repeat-y"] => ((false, REPEAT_PLAIN), (true, REPEAT_PLAIN)),
                [a] => match rep(a) {
                    Some(r) => (r, r),
                    None => return false,
                },
                [a, b] => match (rep(a), rep(b)) {
                    (Some(a), Some(b)) => (a, b),
                    _ => return false,
                },
                _ => return false,
            };
            let st = c.c.images_mut();
            st.bg_repeat = (x.0, y.0);
            st.bg_spacing = [x.1, y.1];
            true
        }),
        h("object-fit", |c, v| {
            c.c.images_mut().object_fit = match kw(v) {
                Some("fill") => ObjectFit::Fill,
                Some("contain") => ObjectFit::Contain,
                Some("cover") => ObjectFit::Cover,
                Some("none") => ObjectFit::None,
                Some("scale-down") => ObjectFit::ScaleDown,
                _ => return false,
            };
            true
        }),
        h("container-type", |c, v| {
            c.c.container_type = match kw(v) {
                Some("normal") | Some("scroll-state") => 0,
                Some("inline-size") => 1,
                Some("size") => 2,
                _ => return false,
            };
            true
        }),
        h("container-name", |c, v| {
            let names: Vec<&str> = v.iter().filter_map(Tok::ident).collect();
            c.c.container_name = match names.as_slice() {
                [] => return false,
                ["none"] => None,
                n => Some(n.join(" ").into()),
            };
            true
        }),
        h("container", |c, v| {
            // <name>+ [/ <type>]
            let mut parts = v.split(|t| matches!(t, Tok::Slash));
            let names = parts.next().unwrap_or(&[]);
            let ok = delegate(c, "container-name", names);
            match parts.next() {
                Some(ty) => ok && delegate(c, "container-type", ty),
                None => ok,
            }
        }),
        h("object-position", |c, v| position(c, v).map(|p| c.c.images_mut().object_position = p).is_some()),
        h("background", |c, v| {
            // the first layer's image, position / size and repeat, then the colour
            let layer = v.split(|t| matches!(t, Tok::Comma)).next().unwrap_or(&[]);
            if let Some(k) = layer.iter().position(|t| matches!(t, Tok::Slash)) {
                let size: Vec<Tok> = layer[k + 1..].iter().take_while(|t| length(t).is_some()
                    || matches!(t.ident(), Some("auto" | "cover" | "contain"))).cloned().take(2).collect();
                delegate(c, "background-size", &size);
            }
            let pos: Vec<Tok> = layer.iter().take_while(|t| !matches!(t, Tok::Slash))
                .filter(|t| length(t).is_some() && !matches!(t, Tok::Num(n) if *n != 0.0)
                    || matches!(t.ident(), Some("left" | "right" | "top" | "bottom" | "center")))
                .cloned().collect();
            if !pos.is_empty() {
                delegate(c, "background-position", &pos);
            }
            let reps: Vec<Tok> = layer.iter().filter(|t| matches!(t.ident(),
                Some("repeat" | "no-repeat" | "repeat-x" | "repeat-y" | "space" | "round"))).cloned().collect();
            if !reps.is_empty() {
                delegate(c, "background-repeat", &reps);
            }
            let mut ok = !pos.is_empty() || !reps.is_empty();
            // <box> keywords: one sets origin and clip, two set origin then clip
            let boxes: Vec<BoxEdge> = layer.iter().filter_map(|t| t.ident().and_then(BoxEdge::parse)).collect();
            match boxes.as_slice() {
                [b] => (c.c.bg_origin, c.c.bg_clip) = (*b, *b),
                [o, k, ..] => (c.c.bg_origin, c.c.bg_clip) = (*o, *k),
                [] => {}
            }
            ok |= !boxes.is_empty();
            if layer.iter().any(|t| t.ident() == Some("fixed")) {
                c.c.images_mut().bg_fixed = true;
                ok = true;
            }
            if layer.iter().any(|t| t.ident() == Some("local")) {
                c.c.images_mut().bg_local = true;
                ok = true;
            }
            for t in v {
                if let Tok::Func(fx) = t {
                    if fx.name == "url" {
                        if let Some(Tok::Str(u)) = fx.args.first().and_then(|a| a.first()) {
                            c.c.images_mut().bg_url = Some(u.as_ref().into());
                            ok = true;
                        }
                        continue;
                    }
                }
                if let Some(g) = gradient(c, t) {
                    c.c.gradient = Some(g);
                    ok = true;
                } else if let Some(col) = color(t, c.c.color) {
                    c.c.background = col;
                    ok = true;
                } else if t.ident() == Some("none") {
                    c.c.background = Rgba::TRANSPARENT;
                    c.c.gradient = None;
                    ok = true;
                }
            }
            ok
        }),
        // -- border -------------------------------------------------------------------------
        h("border", |c, v| border_sides(c, v, &[0, 1, 2, 3])),
        h("border-top", |c, v| border_sides(c, v, &[0])),
        h("border-right", |c, v| border_sides(c, v, &[1])),
        h("border-bottom", |c, v| border_sides(c, v, &[2])),
        h("border-left", |c, v| border_sides(c, v, &[3])),
        h("border-inline", |c, v| border_sides(c, v, &[1, 3])),
        h("border-block", |c, v| border_sides(c, v, &[0, 2])),
        h("border-inline-start", |c, v| border_sides(c, v, if c.c.layout.direction == taffy::style::Direction::Rtl { &[1] } else { &[3] })),
        h("border-inline-end", |c, v| border_sides(c, v, if c.c.layout.direction == taffy::style::Direction::Rtl { &[3] } else { &[1] })),
        h("border-block-start", |c, v| border_sides(c, v, &[0])),
        h("border-block-end", |c, v| border_sides(c, v, &[2])),
        // logical longhands (horizontal-tb: block = top/bottom, inline = left/right)
        h("border-block-color", |c, v| pair(c, v, "border-top-color", "border-bottom-color")),
        h("border-block-style", |c, v| pair(c, v, "border-top-style", "border-bottom-style")),
        h("border-block-width", |c, v| pair(c, v, "border-top-width", "border-bottom-width")),
        h("border-inline-color", |c, v| pair(c, v, "border-left-color", "border-right-color")),
        h("border-inline-style", |c, v| pair(c, v, "border-left-style", "border-right-style")),
        h("border-inline-width", |c, v| pair(c, v, "border-left-width", "border-right-width")),
        h("border-block-start-color", |c, v| delegate(c, "border-top-color", v)),
        h("border-block-start-style", |c, v| delegate(c, "border-top-style", v)),
        h("border-block-start-width", |c, v| delegate(c, "border-top-width", v)),
        h("border-block-end-color", |c, v| delegate(c, "border-bottom-color", v)),
        h("border-block-end-style", |c, v| delegate(c, "border-bottom-style", v)),
        h("border-block-end-width", |c, v| delegate(c, "border-bottom-width", v)),
        h("border-inline-start-color", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "border-right-color" } else { "border-left-color" }, v)),
        h("border-inline-start-style", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "border-right-style" } else { "border-left-style" }, v)),
        h("border-inline-start-width", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "border-right-width" } else { "border-left-width" }, v)),
        h("border-inline-end-color", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "border-left-color" } else { "border-right-color" }, v)),
        h("border-inline-end-style", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "border-left-style" } else { "border-right-style" }, v)),
        h("border-inline-end-width", |c, v| delegate(c, if c.c.layout.direction == taffy::style::Direction::Rtl { "border-left-width" } else { "border-right-width" }, v)),
        h("border-width", |c, v| per_side(v, |i, t| border_width_tok(c, t).map(|w| set_border_width(c, i, w)).is_some())),
        h("border-top-width", |c, v| v.first().and_then(|t| border_width_tok(c, t)).map(|w| set_border_width(c, 0, w)).is_some()),
        h("border-right-width", |c, v| v.first().and_then(|t| border_width_tok(c, t)).map(|w| set_border_width(c, 1, w)).is_some()),
        h("border-bottom-width", |c, v| v.first().and_then(|t| border_width_tok(c, t)).map(|w| set_border_width(c, 2, w)).is_some()),
        h("border-left-width", |c, v| v.first().and_then(|t| border_width_tok(c, t)).map(|w| set_border_width(c, 3, w)).is_some()),
        h("border-color", |c, v| per_side(v, |i, t| color(t, c.c.color).map(|col| c.c.border_color[i] = col).is_some())),
        h("border-top-color", |c, v| v.first().and_then(|t| color(t, c.c.color)).map(|col| c.c.border_color[0] = col).is_some()),
        h("border-right-color", |c, v| v.first().and_then(|t| color(t, c.c.color)).map(|col| c.c.border_color[1] = col).is_some()),
        h("border-bottom-color", |c, v| v.first().and_then(|t| color(t, c.c.color)).map(|col| c.c.border_color[2] = col).is_some()),
        h("border-left-color", |c, v| v.first().and_then(|t| color(t, c.c.color)).map(|col| c.c.border_color[3] = col).is_some()),
        h("border-style", |c, v| per_side(v, |i, t| border_style(c, i, t))),
        h("border-top-style", |c, v| v.first().is_some_and(|t| border_style(c, 0, t))),
        h("border-right-style", |c, v| v.first().is_some_and(|t| border_style(c, 1, t))),
        h("border-bottom-style", |c, v| v.first().is_some_and(|t| border_style(c, 2, t))),
        h("border-left-style", |c, v| v.first().is_some_and(|t| border_style(c, 3, t))),
        h("border-radius", |c, v| {
            let first: Vec<Tok> = v.split(|t| matches!(t, Tok::Slash)).next().unwrap_or(&[]).to_vec();
            lengths(&first).and_then(|l| four(&l)).map(|s| c.c.radius = s).is_some()
        }),
        h("border-top-left-radius", |c, v| v.first().and_then(length).map(|l| c.c.radius[0] = l).is_some()),
        h("border-top-right-radius", |c, v| v.first().and_then(length).map(|l| c.c.radius[1] = l).is_some()),
        h("border-bottom-right-radius", |c, v| v.first().and_then(length).map(|l| c.c.radius[2] = l).is_some()),
        h("border-bottom-left-radius", |c, v| v.first().and_then(length).map(|l| c.c.radius[3] = l).is_some()),
        h("border-start-start-radius", |c, v| delegate(c, "border-top-left-radius", v)),
        h("border-start-end-radius", |c, v| delegate(c, "border-top-right-radius", v)),
        h("border-end-start-radius", |c, v| delegate(c, "border-bottom-left-radius", v)),
        h("border-end-end-radius", |c, v| delegate(c, "border-bottom-right-radius", v)),
        h("outline", |c, v| {
            let (mut w, mut col, mut ok) = (c.c.outline.0.max(3.0), c.c.color, true);
            for t in v {
                if t.ident().is_some_and(|s| s == "none") {
                    w = 0.0;
                } else if let Some(s) = t.ident().filter(|s| BORDER_STYLES.contains(s) || *s == "auto") {
                    c.c.outline_style = BorderStyle::parse(s).unwrap_or_default();
                } else if let Some(cc) = color(t, c.c.color).filter(|_| !matches!(t, Tok::Dim(..))) {
                    col = cc;
                } else if let Some(px) = border_width_tok(c, t) {
                    w = px;
                } else {
                    ok = false;
                }
            }
            c.c.outline.0 = w;
            c.c.outline.1 = col;
            ok
        }),
        h("outline-width", |c, v| v.first().and_then(|t| border_width_tok(c, t)).map(|w| c.c.outline.0 = w).is_some()),
        h("outline-color", |c, v| v.first().and_then(|t| color(t, c.c.color)).map(|col| c.c.outline.1 = col).is_some()),
        h("outline-style", |c, v| match kw(v) {
            Some("none") => {
                c.c.outline.0 = 0.0;
                true
            }
            Some(s) => BorderStyle::parse(s).map(|b| c.c.outline_style = b).is_some(),
            None => false,
        }),
        h("outline-offset", |c, v| v.first().and_then(length).map(|l| c.c.outline.2 = px(c, l)).is_some()),
        // -- effects ------------------------------------------------------------------------
        h("opacity", |c, v| v.first().and_then(number).map(|n| c.c.opacity = n.clamp(0.0, 1.0)).is_some()),
        h("visibility", |c, v| {
            c.c.visible = match kw(v) {
                Some("visible") => true,
                Some("hidden") | Some("collapse") => false,
                _ => return false,
            };
            c.c.collapsed = kw(v) == Some("collapse");
            true
        }),
        h("box-shadow", |c, v| shadows(c, v).map(|s| c.c.shadows = s).is_some()),
        h("transform", |c, v| {
            if kw(v) == Some("none") {
                c.c.translate = (Length::Px(0.0), Length::Px(0.0));
                c.c.scale = 1.0;
                if let Some(e) = c.c.effects.as_mut() {
                    e.transform.clear();
                }
                return true;
            }
            let Some(fns) = transform_list(c, v) else { return false };
            use crate::effects::TransformFn as T;
            // translations alone stay a layout offset; anything else is drawn
            // through the element's layer
            if fns.iter().all(|f| matches!(f, T::Translate(..))) {
                let mut t = (Length::Px(0.0), Length::Px(0.0));
                for f in &fns {
                    if let T::Translate(x, y) = f {
                        t = (add_len(t.0, *x), add_len(t.1, *y));
                    }
                }
                c.c.translate = t;
                if let Some(e) = c.c.effects.as_mut() {
                    e.transform.clear();
                }
            } else {
                c.c.translate = (Length::Px(0.0), Length::Px(0.0));
                c.c.effects_mut().transform = fns;
            }
            true
        }),
        h("scale", |c, v| {
            let nums: Vec<f32> = v.iter().filter_map(|t| match t {
                Tok::Num(n) => Some(*n),
                Tok::Pct(p) => Some(*p / 100.0),
                _ => None,
            }).collect();
            let (x, y) = match (kw(v), nums.as_slice()) {
                (Some("none"), _) => (1.0, 1.0),
                (_, [s]) => (*s, *s),
                (_, [x, y, ..]) => (*x, *y),
                _ => return false,
            };
            c.c.scale = 1.0;
            c.c.effects_mut().scale2 = ((x, y) != (1.0, 1.0)).then_some((x, y));
            true
        }),
        h("rotate", |c, v| match (kw(v), v.last().and_then(angle)) {
            (Some("none"), _) => {
                c.c.effects_mut().rotate = 0.0;
                true
            }
            (_, Some(a)) => {
                c.c.effects_mut().rotate = a;
                true
            }
            _ => false,
        }),
        h("transform-origin", |c, v| {
            let xy: Vec<Tok> = v.iter().take(2).cloned().collect();
            position(c, &xy).map(|p| c.c.effects_mut().origin = Some(p)).is_some()
        }),
        // -- filters, blending, clipping, masking -------------------------------------
        h("filter", |c, v| filter_list(c, v).map(|f| c.c.effects_mut().filter = f).is_some()),
        h("backdrop-filter", |c, v| filter_list(c, v).map(|f| c.c.effects_mut().backdrop = f).is_some()),
        h("-webkit-backdrop-filter", |c, v| delegate(c, "backdrop-filter", v)),
        h("mix-blend-mode", |c, v| kw(v).and_then(crate::effects::Blend::parse)
            .map(|b| c.c.effects_mut().blend = b).is_some()),
        h("isolation", |c, v| match kw(v) {
            Some(k @ ("auto" | "isolate")) => {
                c.c.effects_mut().isolate = k == "isolate";
                true
            }
            _ => false,
        }),
        h("clip-path", |c, v| {
            if kw(v) == Some("none") {
                c.c.effects_mut().clip_path = None;
                return true;
            }
            let mut shape = None;
            let mut edge = BoxEdge::Border;
            for t in v {
                match t {
                    Tok::Ident(k) => match BoxEdge::parse(k) {
                        Some(e) => edge = e,
                        None if matches!(k.as_ref(), "margin-box" | "fill-box" | "stroke-box" | "view-box") => {}
                        None => return false,
                    },
                    Tok::Func(_) => match clip_shape(c, t) {
                        Some(sh) => shape = Some(sh),
                        None => return false,
                    },
                    _ => return false,
                }
            }
            c.c.effects_mut().clip_path = Some((shape.unwrap_or(crate::effects::ClipShape::Box), edge));
            true
        }),
        h("-webkit-clip-path", |c, v| delegate(c, "clip-path", v)),
        h("mask-image", |c, v| {
            let first = v.split(|t| matches!(t, Tok::Comma)).next().unwrap_or(&[]);
            let src = match first {
                [Tok::Ident(k)] if k.as_ref() == "none" => None,
                [t] => match image_source(c, t) {
                    Some(src) => Some(src),
                    None => return false,
                },
                _ => return false,
            };
            c.c.effects_mut().mask.get_or_insert_with(Default::default).image = src;
            true
        }),
        h("mask-mode", |c, v| match kw(v) {
            Some(k @ ("alpha" | "luminance" | "match-source")) => {
                c.c.effects_mut().mask.get_or_insert_with(Default::default).luminance = k == "luminance";
                true
            }
            _ => false,
        }),
        h("mask-size", |c, v| {
            // reuse background-size's parser on a scratch style
            let mut tmp = Computed::default();
            let parent = Computed::default();
            let mut cx = Ctx { c: &mut tmp, parent: &parent, u: c.u };
            if !delegate(&mut cx, "background-size", v) {
                return false;
            }
            let size = tmp.images.map(|i| i.bg_size).unwrap_or_default();
            c.c.effects_mut().mask.get_or_insert_with(Default::default).size = size;
            true
        }),
        h("mask-position", |c, v| {
            let first = v.split(|t| matches!(t, Tok::Comma)).next().unwrap_or(&[]);
            position(c, first).map(|p| c.c.effects_mut().mask.get_or_insert_with(Default::default).position = p)
                .is_some()
        }),
        h("mask-repeat", |c, v| {
            let words: Vec<&str> = v.iter().take_while(|t| !matches!(t, Tok::Comma)).filter_map(Tok::ident).collect();
            let rep = |w: &str| matches!(w, "repeat" | "space" | "round");
            let r = match words.as_slice() {
                ["repeat-x"] => (true, false),
                ["repeat-y"] => (false, true),
                [a] => (rep(a), rep(a)),
                [a, b] => (rep(a), rep(b)),
                _ => return false,
            };
            c.c.effects_mut().mask.get_or_insert_with(Default::default).repeat = r;
            true
        }),
        h("mask", |c, v| {
            if kw(v) == Some("none") {
                c.c.effects_mut().mask = None;
                return true;
            }
            let layer = v.split(|t| matches!(t, Tok::Comma)).next().unwrap_or(&[]);
            let mut ok = false;
            for t in layer {
                if let Ok(src) = image_source(c, t).ok_or(()) {
                    c.c.effects_mut().mask.get_or_insert_with(Default::default).image = Some(src);
                    ok = true;
                }
            }
            if let Some(k) = layer.iter().position(|t| matches!(t, Tok::Slash)) {
                let size: Vec<Tok> = layer[k + 1..].iter().take_while(|t| length(t).is_some()
                    || matches!(t.ident(), Some("auto" | "cover" | "contain"))).cloned().take(2).collect();
                delegate(c, "mask-size", &size);
            }
            let pos: Vec<Tok> = layer.iter().take_while(|t| !matches!(t, Tok::Slash))
                .filter(|t| length(t).is_some() && !matches!(t, Tok::Num(n) if *n != 0.0)
                    || matches!(t.ident(), Some("left" | "right" | "top" | "bottom" | "center")))
                .cloned().collect();
            if !pos.is_empty() {
                delegate(c, "mask-position", &pos);
            }
            let reps: Vec<Tok> = layer.iter().filter(|t| matches!(t.ident(),
                Some("repeat" | "no-repeat" | "repeat-x" | "repeat-y" | "space" | "round"))).cloned().collect();
            if !reps.is_empty() {
                delegate(c, "mask-repeat", &reps);
            }
            for k in layer.iter().filter_map(Tok::ident) {
                if matches!(k, "alpha" | "luminance") {
                    c.c.effects_mut().mask.get_or_insert_with(Default::default).luminance = k == "luminance";
                }
            }
            ok
        }),
        h("-webkit-mask", |c, v| delegate(c, "mask", v)),
        h("-webkit-mask-image", |c, v| delegate(c, "mask-image", v)),
        h("-webkit-mask-size", |c, v| delegate(c, "mask-size", v)),
        h("-webkit-mask-position", |c, v| delegate(c, "mask-position", v)),
        h("-webkit-mask-repeat", |c, v| delegate(c, "mask-repeat", v)),
        // -- border-image ---------------------------------------------------------------
        h("border-image-source", |c, v| match v {
            [Tok::Ident(k)] if k.as_ref() == "none" => {
                c.c.effects_mut().border_image.get_or_insert_with(Default::default).source = None;
                true
            }
            [t] => image_source(c, t).map(|s| c.c.effects_mut().border_image.get_or_insert_with(Default::default)
                .source = Some(s)).is_some(),
            _ => false,
        }),
        h("border-image-slice", |c, v| {
            let fill = v.iter().any(|t| t.ident() == Some("fill"));
            let vals: Option<Vec<crate::effects::Side>> = v.iter().filter(|t| t.ident() != Some("fill"))
                .map(|t| match t {
                    Tok::Num(n) => Some(crate::effects::Side::Num(*n)),
                    Tok::Pct(p) => Some(crate::effects::Side::Len(Length::Percent(*p))),
                    _ => None,
                }).collect();
            let Some(sides) = vals.as_deref().and_then(four_sides) else { return false };
            let b = c.c.effects_mut().border_image.get_or_insert_with(Default::default);
            b.slice = sides;
            b.fill = fill;
            true
        }),
        h("border-image-width", |c, v| side_values(v).map(|s| c.c.effects_mut().border_image
            .get_or_insert_with(Default::default).width = s).is_some()),
        h("border-image-outset", |c, v| side_values(v).map(|s| c.c.effects_mut().border_image
            .get_or_insert_with(Default::default).outset = s).is_some()),
        h("border-image-repeat", |c, v| {
            let tile = |w: &str| Some(match w {
                "stretch" => crate::effects::Tile::Stretch,
                "repeat" => crate::effects::Tile::Repeat,
                "round" => crate::effects::Tile::Round,
                "space" => crate::effects::Tile::Space,
                _ => return None,
            });
            let words: Option<Vec<crate::effects::Tile>> = v.iter().map(|t| t.ident().and_then(tile)).collect();
            let r = match words.as_deref() {
                Some([a]) => (*a, *a),
                Some([a, b]) => (*a, *b),
                _ => return false,
            };
            c.c.effects_mut().border_image.get_or_insert_with(Default::default).repeat = r;
            true
        }),
        h("border-image", |c, v| {
            if kw(v) == Some("none") {
                c.c.effects_mut().border_image = None;
                return true;
            }
            // <source> || <slice> [/ <width> [/ <outset>]] || <repeat>
            let mut ok = false;
            let mut rest: Vec<Tok> = Vec::new();
            for t in v {
                if matches!(t, Tok::Func(_)) && image_source(c, t).is_some() {
                    ok |= delegate(c, "border-image-source", std::slice::from_ref(t));
                } else if matches!(t.ident(), Some("stretch" | "repeat" | "round" | "space")) {
                    let reps: Vec<Tok> = v.iter().filter(|t| matches!(t.ident(),
                        Some("stretch" | "repeat" | "round" | "space"))).cloned().collect();
                    delegate(c, "border-image-repeat", &reps);
                } else {
                    rest.push(t.clone());
                }
            }
            let parts: Vec<&[Tok]> = rest.split(|t| matches!(t, Tok::Slash)).collect();
            if let Some(slice) = parts.first().filter(|p| !p.is_empty()) {
                ok &= delegate(c, "border-image-slice", slice);
            }
            if let Some(w) = parts.get(1).filter(|p| !p.is_empty()) {
                delegate(c, "border-image-width", w);
            }
            if let Some(o) = parts.get(2).filter(|p| !p.is_empty()) {
                delegate(c, "border-image-outset", o);
            }
            ok
        }),
        h("translate", |c, v| {
            let l = lengths(v).unwrap_or_default();
            c.c.translate = (*l.first().unwrap_or(&Length::Px(0.0)), *l.get(1).unwrap_or(&Length::Px(0.0)));
            !l.is_empty()
        }),
        // -- text ---------------------------------------------------------------------------
        h("color", |c, v| v.first().and_then(|t| color(t, c.parent.color)).map(|col| c.c.color = col).is_some()),
        h("font-size", |c, v| v.first().and_then(|t| font_size(c, t)).map(|s| c.c.font_size = s).is_some()),
        h("font-weight", |c, v| v.first().and_then(|t| font_weight(c, t)).map(|w| c.c.weight = w).is_some()),
        h("font-style", |c, v| {
            c.c.italic = matches!(kw(v), Some("italic") | Some("oblique"));
            true
        }),
        h("font-family", |c, v| {
            c.c.font_family = v.iter().map(tok_text).collect::<Vec<_>>().join(" ");
            c.c.font_id = crate::text::font_id(&c.c.font_family);
            true
        }),
        h("font-variant", |c, v| {
            c.c.small_caps = kw(v) == Some("small-caps");
            true
        }),
        h("font-variant-caps", |c, v| delegate(c, "font-variant", v)),
        h("font", |c, v| {
            // [style] [variant] [weight] size[/line-height] family
            let mut i = 0;
            while i < v.len() {
                let t = &v[i];
                if let Some(s) = t.ident() {
                    match s {
                        "italic" | "oblique" => c.c.italic = true,
                        "small-caps" => c.c.small_caps = true,
                        "bold" | "bolder" | "lighter" => c.c.weight = font_weight(c, t).unwrap_or(400),
                        "normal" => {}
                        _ => break,
                    }
                } else if let Tok::Num(n) = t {
                    c.c.weight = *n as u16;
                } else if let Some(s) = font_size(c, t) {
                    c.c.font_size = s;
                    if matches!(v.get(i + 1), Some(Tok::Slash)) {
                        if let Some(lh) = v.get(i + 2).and_then(|t| line_height(t, c)) {
                            c.c.line_height = lh;
                        }
                        i += 2;
                    }
                    i += 1;
                    c.c.font_family = v[i..].iter().map(tok_text).collect::<Vec<_>>().join(" ");
                    c.c.font_id = crate::text::font_id(&c.c.font_family);
                    return true;
                } else {
                    return false;
                }
                i += 1;
            }
            false
        }),
        h("line-height", |c, v| v.first().and_then(|t| line_height(t, c)).map(|l| c.c.line_height = l).is_some()),
        h("text-align", |c, v| {
            let rtl = c.parent.layout.direction == taffy::style::Direction::Rtl;
            c.c.text_align = match kw(v) {
                Some("left" | "-webkit-left") => TextAlign::Left,
                Some("right" | "-webkit-right") => TextAlign::Right,
                Some("start") => TextAlign::Start,
                Some("end") => TextAlign::End,
                Some("center" | "-webkit-center") => TextAlign::Center,
                Some("justify") => TextAlign::Justify,
                // the parent's, its start and end as its direction has them
                Some("match-parent") => c.parent.text_align.physical(rtl),
                Some("justify-all") => {
                    c.c.text_align_last = Some(TextAlign::Justify);
                    TextAlign::Justify
                }
                _ => return false,
            };
            true
        }),
        h("text-align-last", |c, v| {
            c.c.text_align_last = match kw(v) {
                Some("auto") => None,
                Some("left") => Some(TextAlign::Left),
                Some("right") => Some(TextAlign::Right),
                Some("start") => Some(TextAlign::Start),
                Some("end") => Some(TextAlign::End),
                Some("center") => Some(TextAlign::Center),
                Some("justify") => Some(TextAlign::Justify),
                Some("match-parent") => Some(c.parent.text_align_last.unwrap_or(c.parent.text_align)
                    .physical(c.parent.layout.direction == taffy::style::Direction::Rtl)),
                _ => return false,
            };
            true
        }),
        h("text-decoration", |c, v| {
            // a shorthand: what it doesn't name goes back to the initial value
            c.c.decoration_style = DecoStyle::Solid;
            c.c.decoration_thickness = None;
            c.c.decoration_color = None;
            let words: Vec<&str> = v.iter().filter_map(Tok::ident).collect();
            c.c.underline = words.contains(&"underline");
            c.c.line_through = words.contains(&"line-through");
            c.c.overline = words.contains(&"overline");
            for t in v {
                let one = std::slice::from_ref(t);
                match t.ident() {
                    Some("underline" | "line-through" | "overline" | "none" | "blink" | "spelling-error"
                         | "grammar-error") => {}
                    Some(s) if deco_style(s).is_some() => c.c.decoration_style = deco_style(s).unwrap(),
                    _ if matches!(t, Tok::Dim(..) | Tok::Pct(_) | Tok::Calc(_))
                        || t.ident() == Some("from-font") => {
                        delegate(c, "text-decoration-thickness", one);
                    }
                    _ => {
                        if !delegate(c, "text-decoration-color", one) {
                            return false;
                        }
                    }
                }
            }
            true
        }),
        h("text-decoration-line", |c, v| {
            let words: Vec<&str> = v.iter().filter_map(Tok::ident).collect();
            c.c.underline = words.contains(&"underline");
            c.c.line_through = words.contains(&"line-through");
            c.c.overline = words.contains(&"overline");
            !words.is_empty()
        }),
        h("white-space", |c, v| {
            c.c.white_space = match kw(v) {
                Some("normal") => WhiteSpace::Normal,
                Some("nowrap") => WhiteSpace::NoWrap,
                Some("pre") => WhiteSpace::Pre,
                Some("pre-wrap") => WhiteSpace::PreWrap,
                Some("break-spaces") => WhiteSpace::BreakSpaces,
                Some("pre-line") => WhiteSpace::PreLine,
                _ => return false,
            };
            true
        }),
        // `text-wrap-mode || text-wrap-style`
        h("text-wrap", |c, v| {
            for t in v {
                let one = std::slice::from_ref(t);
                if !(delegate(c, "text-wrap-mode", one) || delegate(c, "text-wrap-style", one)) {
                    return false;
                }
            }
            true
        }),
        h("text-wrap-style", |c, v| {
            use crate::linebreak::WRAP_BALANCE;
            let balance = match kw(v) {
                Some("balance") => true,
                Some("auto" | "pretty" | "stable") => false,
                _ => return false,
            };
            c.c.text_wrap = (c.c.text_wrap & !WRAP_BALANCE) | if balance { WRAP_BALANCE } else { 0 };
            true
        }),
        // CSS Text 4 longhands of white-space: wrapping and collapsing
        h("text-wrap-mode", |c, v| {
            use WhiteSpace::*;
            c.c.white_space = match (kw(v), c.c.white_space) {
                (Some("wrap"), NoWrap) => Normal,
                (Some("wrap"), Pre) => PreWrap,
                (Some("wrap"), w) => w,
                (Some("nowrap"), Normal | PreLine) => NoWrap,
                (Some("nowrap"), PreWrap | BreakSpaces) => Pre,
                (Some("nowrap"), w) => w,
                _ => return false,
            };
            true
        }),
        h("white-space-collapse", |c, v| {
            use WhiteSpace::*;
            let wraps = !matches!(c.c.white_space, NoWrap | Pre);
            c.c.white_space = match kw(v) {
                Some("collapse" | "discard") => if wraps { Normal } else { NoWrap },
                Some("preserve" | "preserve-spaces") => if wraps { PreWrap } else { Pre },
                Some("break-spaces") => if wraps { BreakSpaces } else { Pre },
                Some("preserve-breaks") => if wraps { PreLine } else { Pre },
                _ => return false,
            };
            true
        }),
        // `none | [capitalize | uppercase | lowercase] || full-width || full-size-kana`
        h("text-transform", |c, v| {
            use crate::linebreak::{TRANSFORM_FULL_SIZE_KANA, TRANSFORM_FULL_WIDTH};
            let (mut case, mut extra) = (None, 0u8);
            for w in v.iter().map(Tok::ident) {
                let Some(w) = w else { return false };
                match w {
                    "none" if v.len() == 1 => case = Some(TextTransform::None),
                    "uppercase" if case.is_none() => case = Some(TextTransform::Upper),
                    "lowercase" if case.is_none() => case = Some(TextTransform::Lower),
                    "capitalize" if case.is_none() => case = Some(TextTransform::Capitalize),
                    "full-width" if extra & TRANSFORM_FULL_WIDTH == 0 => extra |= TRANSFORM_FULL_WIDTH,
                    "full-size-kana" if extra & TRANSFORM_FULL_SIZE_KANA == 0 => extra |= TRANSFORM_FULL_SIZE_KANA,
                    _ => return false,
                }
            }
            c.c.text_transform = case.unwrap_or(TextTransform::None);
            c.c.text_transform_extra = extra;
            true
        }),
        h("letter-spacing", |c, v| match kw(v) {
            Some("normal") => {
                c.c.letter_spacing = 0.0;
                true
            }
            _ => v.first().and_then(length).map(|l| c.c.letter_spacing = px(c, l)).is_some(),
        }),
        h("word-spacing", |c, v| match kw(v) {
            Some("normal") => {
                c.c.word_spacing = 0.0;
                true
            }
            _ => v.first().and_then(length).map(|l| c.c.word_spacing = px(c, l)).is_some(),
        }),
        h("text-indent", |c, v| v.first().and_then(length).map(|l| c.c.text_indent = l).is_some()),
        h("vertical-align", |c, v| {
            c.c.vertical_align = match kw(v) {
                Some("sub") => VAlign::Sub,
                Some("super") => VAlign::Super,
                Some("top") => VAlign::Top,
                Some("middle") => VAlign::Middle,
                Some("bottom") => VAlign::Bottom,
                _ => VAlign::Baseline,
            };
            true
        }),
        h("list-style-type", |c, v| kw(v).and_then(list_style).map(|l| c.c.list_style = l).is_some()),
        h("list-style-position", |c, v| match kw(v) {
            Some(p @ ("inside" | "outside")) => {
                c.c.marker_inside = p == "inside";
                true
            }
            _ => false,
        }),
        h("content", |c, v| {
            if matches!(kw(v), Some("none" | "normal")) {
                c.c.content = None;
                return true;
            }
            let mut parts = Vec::new();
            // `content: "x" / "alt text"`: the part after the slash is alt text
            let main = v.split(|t| matches!(t, Tok::Slash)).next().unwrap_or(&[]);
            for t in main {
                parts.push(match t {
                    Tok::Str(s) => ContentPart::Text(s.to_string()),
                    Tok::Ident(s) if s == "open-quote" => ContentPart::OpenQuote,
                    Tok::Ident(s) if s == "close-quote" => ContentPart::CloseQuote,
                    Tok::Ident(s) if s == "no-open-quote" || s == "no-close-quote" => continue,
                    Tok::Func(fx) if fx.name == "attr" => {
                        let Some(n) = fx.args.first().and_then(|a| a.first()).and_then(Tok::ident) else {
                            return false;
                        };
                        ContentPart::Attr(n.to_string())
                    }
                    Tok::Func(fx) if fx.name == "counter" || fx.name == "counters" => {
                        let args: Vec<&[Tok]> = fx.args.iter().map(|a| a.as_slice()).collect();
                        let Some(name) = args.first().and_then(|a| a.first()).and_then(Tok::ident) else {
                            return false;
                        };
                        let style_at = |k: usize| args.get(k).and_then(|a| a.first()).and_then(Tok::ident)
                            .and_then(list_style).unwrap_or(ListStyle::Decimal);
                        if fx.name == "counter" {
                            ContentPart::Counter(name.into(), style_at(1))
                        } else {
                            let sep = match args.get(1).and_then(|a| a.first()) {
                                Some(Tok::Str(s)) => s.as_ref().into(),
                                _ => ".".into(),
                            };
                            ContentPart::Counters(name.into(), sep, style_at(2))
                        }
                    }
                    // inline images in generated content: not drawn
                    Tok::Func(fx) if matches!(fx.name.as_ref(), "url" | "image" | "linear-gradient"
                                              | "image-set") => continue,
                    _ => return false,
                });
            }
            c.c.content = Some(parts.into_boxed_slice());
            true
        }),
        h("quotes", |c, v| {
            let strs: Vec<&str> = v.iter().filter_map(|t| match t {
                Tok::Str(s) => Some(s.as_ref()),
                _ => None,
            }).collect();
            match (kw(v), strs.as_slice()) {
                (Some("none"), _) => c.c.quotes = Some((String::new(), String::new())),
                (Some("auto"), _) => c.c.quotes = None,
                (_, [open, close, ..]) => c.c.quotes = Some((open.to_string(), close.to_string())),
                _ => return false,
            }
            true
        }),
        // `box-shadow`'s list, without `inset` or a spread (CSS Text Decoration 3 §4)
        h("text-shadow", |c, v| match shadows(c, v) {
            Some(l) if l.iter().all(|s| !s.inset && s.spread == 0.0) => {
                c.c.text_shadow = l;
                true
            }
            _ => false,
        }),
        h("text-decoration-color", |c, v| v.first().and_then(|t| color(t, c.c.color))
            .map(|col| c.c.decoration_color = Some(col)).is_some()),
        // -- tables ----------------------------------------------------------------------------
        h("border-spacing", |c, v| {
            let l: Option<Vec<f32>> = v.iter().map(|t| length(t).and_then(|l| l.to_px(&c.u))).collect();
            match l.as_deref() {
                Some([a]) => c.c.border_spacing = (*a, *a),
                Some([a, b]) => c.c.border_spacing = (*a, *b),
                _ => return false,
            }
            true
        }),
        h("scroll-snap-type", |c, v| {
            let words: Vec<&str> = v.iter().filter_map(Tok::ident).collect();
            if words == ["none"] {
                c.c.snap_axes = 0;
                return true;
            }
            let axes = match words.first() {
                Some(&"x" | &"inline") => 1,
                Some(&"y" | &"block") => 2,
                Some(&"both") => 3,
                _ => return false,
            };
            c.c.snap_axes = axes;
            c.c.snap_mandatory = words.get(1) == Some(&"mandatory");
            true
        }),
        h("scroll-snap-align", |c, v| {
            let one = |w: &str| Some(match w {
                "none" => 0,
                "start" => 1,
                "center" => 2,
                "end" => 3,
                _ => return None,
            });
            let words: Option<Vec<u8>> = v.iter().map(|t| t.ident().and_then(one)).collect();
            // one value: both axes; two: block (y) then inline (x)
            c.c.snap_align = match words.as_deref() {
                Some([a]) => (*a, *a),
                Some([y, x]) => (*x, *y),
                _ => return false,
            };
            true
        }),
        h("unicode-bidi", |c, v| match kw(v) {
            Some("normal" | "embed" | "isolate" | "plaintext") => {
                c.c.bidi_override = false;
                true
            }
            Some("bidi-override" | "isolate-override") => {
                c.c.bidi_override = true;
                true
            }
            _ => false,
        }),
        h("table-layout", |c, v| match kw(v) {
            Some(k @ ("auto" | "fixed")) => {
                c.c.table_fixed = k == "fixed";
                true
            }
            _ => false,
        }),
        h("border-collapse", |c, v| match kw(v) {
            Some(k @ ("collapse" | "separate")) => {
                c.c.border_collapse = k == "collapse";
                true
            }
            _ => false,
        }),
        h("caption-side", |c, v| match kw(v) {
            Some(k @ ("top" | "bottom" | "block-start" | "block-end")) => {
                c.c.caption_bottom = matches!(k, "bottom" | "block-end");
                true
            }
            _ => false,
        }),
        h("list-style", |c, v| {
            let mut ok = false;
            for t in v {
                match t {
                    Tok::Func(fx) if fx.name == "url" => ok |= delegate(c, "list-style-image", std::slice::from_ref(t)),
                    Tok::Ident(s) if s.as_ref() == "inside" || s.as_ref() == "outside" => {
                        ok |= delegate(c, "list-style-position", std::slice::from_ref(t))
                    }
                    Tok::Ident(s) => {
                        if let Some(l) = list_style(s) {
                            c.c.list_style = l;
                            ok = true;
                        }
                    }
                    _ => {}
                }
            }
            ok
        }),
        h("background-clip", |c, v| {
            let first = v.split(|t| matches!(t, Tok::Comma)).next().unwrap_or(&[]);
            match kw(first) {
                // `text`: no glyph masks here; keep the colour behind the text box
                Some("text") => true,
                Some(s) => BoxEdge::parse(s).map(|e| c.c.bg_clip = e).is_some(),
                None => false,
            }
        }),
        h("background-origin", |c, v| {
            let first = v.split(|t| matches!(t, Tok::Comma)).next().unwrap_or(&[]);
            kw(first).and_then(BoxEdge::parse).map(|e| c.c.bg_origin = e).is_some()
        }),
        h("background-attachment", |c, v| {
            let first = v.split(|t| matches!(t, Tok::Comma)).next().unwrap_or(&[]);
            match kw(first) {
                Some(k @ ("scroll" | "fixed" | "local")) => {
                    let fixed = k == "fixed";
                    if k != "scroll" || c.c.images.is_some() {
                        let st = c.c.images_mut();
                        (st.bg_fixed, st.bg_local) = (fixed, k == "local");
                    }
                    true
                }
                _ => false,
            }
        }),
        h("clip", |c, v| match v.first() {
            Some(Tok::Ident(s)) if s.as_ref() == "auto" => {
                c.c.clip_rect = None;
                true
            }
            Some(Tok::Func(fx)) if fx.name == "rect" => {
                // rect(top, right, bottom, left), commas optional
                let toks: Vec<&Tok> = fx.args.iter().flatten().filter(|t| !matches!(t, Tok::Comma)).collect();
                if toks.len() != 4 {
                    return false;
                }
                let mut r = [None; 4];
                for (k, t) in toks.iter().enumerate() {
                    if t.ident() == Some("auto") {
                        continue;
                    }
                    let Some(l) = length(t) else { return false };
                    r[k] = Some(l);
                }
                c.c.clip_rect = Some(r);
                true
            }
            _ => false,
        }),
        h("text-decoration-style", |c, v| kw(v).and_then(deco_style).map(|s| c.c.decoration_style = s).is_some()),
        h("text-decoration-thickness", |c, v| match (kw(v), v.first()) {
            (Some("auto" | "from-font"), _) => {
                c.c.decoration_thickness = None;
                true
            }
            // a percentage is of 1em
            (_, Some(Tok::Pct(p))) => {
                c.c.decoration_thickness = Some(c.c.font_size * p / 100.0);
                true
            }
            (_, Some(t)) => length(t).map(|l| c.c.decoration_thickness = Some(px(c, l))).is_some(),
            _ => false,
        }),
        h("text-underline-offset", |c, v| match (kw(v), v.first()) {
            (Some("auto"), _) => {
                c.c.underline_offset = None;
                true
            }
            (_, Some(Tok::Pct(p))) => {
                c.c.underline_offset = Some(c.c.font_size * p / 100.0);
                true
            }
            (_, Some(t)) => length(t).map(|l| c.c.underline_offset = Some(px(c, l))).is_some(),
            _ => false,
        }),
        // -- text breaking --------------------------------------------------------------------
        h("word-break", |c, v| {
            use crate::linebreak::WRAP_KEEP_ALL;
            c.c.text_wrap = (c.c.text_wrap & !WRAP_KEEP_ALL) | if kw(v) == Some("keep-all") { WRAP_KEEP_ALL } else { 0 };
            c.c.break_all = match kw(v) {
                Some("break-all") => true,
                Some("normal" | "keep-all" | "auto-phrase") => false,
                // deprecated: overflow-wrap: anywhere plus word-break: normal
                Some("break-word") => {
                    c.c.wrap_anywhere = true;
                    false
                }
                _ => return false,
            };
            true
        }),
        h("overflow-wrap", |c, v| {
            use crate::linebreak::WRAP_BREAK_WORD;
            c.c.text_wrap = (c.c.text_wrap & !WRAP_BREAK_WORD) | if kw(v) == Some("break-word") { WRAP_BREAK_WORD } else { 0 };
            c.c.wrap_anywhere = match kw(v) {
                Some("anywhere" | "break-word") => true,
                Some("normal") => false,
                _ => return false,
            };
            true
        }),
        h("word-wrap", |c, v| delegate(c, "overflow-wrap", v)),
        h("line-break", |c, v| {
            use crate::linebreak::{WRAP_ANYWHERE, WRAP_LOOSE, WRAP_STRICT};
            let bits = match kw(v) {
                Some("anywhere") => WRAP_ANYWHERE,
                Some("loose") => WRAP_LOOSE,
                Some("strict") => WRAP_STRICT,
                Some("auto" | "normal") => 0,
                _ => return false,
            };
            c.c.text_wrap = (c.c.text_wrap & !(WRAP_ANYWHERE | WRAP_LOOSE | WRAP_STRICT)) | bits;
            true
        }),
        h("text-autospace", |c, v| {
            use crate::linebreak::WRAP_NO_AUTOSPACE;
            let off = match kw(v) {
                Some("no-autospace") => true,
                Some(_) => false,
                None => return false,
            };
            c.c.text_wrap = (c.c.text_wrap & !WRAP_NO_AUTOSPACE) | if off { WRAP_NO_AUTOSPACE } else { 0 };
            true
        }),
        h("hyphens", |c, v| {
            use crate::linebreak::WRAP_NO_HYPHENS;
            let none = match kw(v) {
                Some("none") => true,
                Some("manual" | "auto") => false,
                _ => return false,
            };
            c.c.text_wrap = (c.c.text_wrap & !WRAP_NO_HYPHENS) | if none { WRAP_NO_HYPHENS } else { 0 };
            true
        }),
        h("tab-size", |c, v| match v.first() {
            Some(Tok::Num(n)) if *n >= 0.0 => {
                c.c.tab_size = n.round().min(64.0) as u8;
                true
            }
            // a length: as many spaces as fit
            Some(t) => length(t).map(|l| {
                let sp = (c.c.font_size * 0.3).max(1.0);
                c.c.tab_size = (px(c, l) / sp).round().clamp(0.0, 64.0) as u8;
            }).is_some(),
            None => false,
        }),
        h("list-style-image", |c, v| match v.first() {
            Some(Tok::Ident(s)) if s.as_ref() == "none" => {
                c.c.list_image = None;
                true
            }
            Some(Tok::Func(fx)) if fx.name == "url" => match fx.args.first().and_then(|a| a.first()) {
                Some(Tok::Str(u)) => {
                    c.c.list_image = Some(u.as_ref().into());
                    true
                }
                _ => false,
            },
            _ => false,
        }),
        h("field-sizing", |c, v| match kw(v) {
            Some(k @ ("content" | "fixed")) => {
                c.c.field_sizing_content = k == "content";
                true
            }
            _ => false,
        }),
        h("empty-cells", |c, v| match kw(v) {
            Some(k @ ("show" | "hide")) => {
                c.c.empty_cells_hide = k == "hide";
                true
            }
            _ => false,
        }),
        // -- counters ---------------------------------------------------------------------------
        h("counter-reset", |c, v| counter_list(v, 0).map(|l| c.c.counters.get_or_insert_with(Default::default).reset = l).is_some()),
        h("counter-set", |c, v| counter_list(v, 0).map(|l| c.c.counters.get_or_insert_with(Default::default).set = l).is_some()),
        h("counter-increment", |c, v| counter_list(v, 1).map(|l| c.c.counters.get_or_insert_with(Default::default).increment = l).is_some()),
        // -- interaction --------------------------------------------------------------------
        h("cursor", |c, v| {
            let name = v.iter().filter_map(Tok::ident).next_back();
            match name.and_then(|n| CURSORS.iter().find(|(k, _)| *k == n)) {
                Some((_, -1)) => c.c.cursor = None,
                Some((_, id)) => c.c.cursor = Some(*id),
                None => return false,
            }
            true
        }),
        h("user-select", |c, v| {
            c.c.user_select = match kw(v) {
                Some("auto") => UserSelect::Auto,
                Some("none") => UserSelect::None,
                Some("text" | "contain") => UserSelect::Text,
                Some("all") => UserSelect::All,
                _ => return false,
            };
            true
        }),
        h("-webkit-user-select", |c, v| delegate(c, "user-select", v)),
        h("-moz-user-select", |c, v| delegate(c, "user-select", v)),
        h("pointer-events", |c, v| {
            c.c.pointer_events = kw(v) != Some("none");
            true
        }),
        // Chromium's `drag` / `no-drag`, and BUI's window buttons
        h("app-region", |c, v| {
            use crate::window_kind::WindowControlArea as A;
            c.c.app_region = match kw(v) {
                Some("none") => AppRegion::Auto,
                Some("no-drag") => AppRegion::NoDrag,
                Some("drag") => AppRegion::Area(A::Drag),
                Some("minimize") => AppRegion::Area(A::Min),
                Some("maximize") => AppRegion::Area(A::Max),
                Some("close") => AppRegion::Area(A::Close),
                _ => return false,
            };
            true
        }),
        h("-webkit-app-region", |c, v| delegate(c, "app-region", v)),
        h("accent-color", |c, v| match kw(v) {
            Some("auto") => {
                c.c.accent = None;
                true
            }
            _ => v.first().and_then(|t| color(t, c.c.color)).map(|col| c.c.accent = Some(col)).is_some(),
        }),
        // -- animations and transitions (run by anim.rs) -----------------------------------
        h("animation", crate::anim::h_animation),
        h("animation-name", crate::anim::h_animation_name),
        h("animation-duration", crate::anim::h_animation_duration),
        h("animation-timing-function", crate::anim::h_animation_timing),
        h("animation-delay", crate::anim::h_animation_delay),
        h("animation-iteration-count", crate::anim::h_animation_iterations),
        h("animation-direction", crate::anim::h_animation_direction),
        h("animation-fill-mode", crate::anim::h_animation_fill),
        h("animation-play-state", crate::anim::h_animation_play_state),
        h("transition", crate::anim::h_transition),
        h("transition-property", crate::anim::h_transition_property),
        h("transition-duration", crate::anim::h_transition_duration),
        h("transition-timing-function", crate::anim::h_transition_timing),
        h("transition-delay", crate::anim::h_transition_delay),
        h("transition-behavior", crate::anim::h_transition_behavior),
    ]
};


/// A `<position>` (`center`, `left 10px top`, `25% 75%`, `right 8px bottom 4px` ...)
/// as x and y.
fn position(c: &Ctx, v: &[Tok]) -> Option<[Pos; 2]> {
    let kwp = |s: &str| match s {
        "left" | "top" => Some(0.0),
        "center" => Some(0.5),
        "right" | "bottom" => Some(1.0),
        _ => None,
    };
    let val = |t: &Tok| -> Option<Pos> {
        match length(t)? {
            Length::Percent(p) => Some(Pos { frac: p / 100.0, px: 0.0 }),
            l => l.to_px(&c.u).map(|px| Pos { frac: 0.0, px }),
        }
    };
    let mut x: Option<Pos> = None;
    let mut y: Option<Pos> = None;
    let mut i = 0;
    let mut pending_center = 0;
    while i < v.len() {
        let t = &v[i];
        if let Some(word) = t.ident() {
            let f = kwp(word)?;
            // `right 10px top`: an offset from that edge, in the three- and four-value forms only
            // (`left 10px` is x and y)
            let off = v.get(i + 1).filter(|n| v.len() >= 3 && n.ident().is_none()).and_then(val);
            let p = match off {
                Some(o) => {
                    i += 1;
                    if f == 1.0 { Pos { frac: 1.0 - o.frac, px: -o.px } } else { o }
                }
                None => Pos { frac: f, px: 0.0 },
            };
            match word {
                "left" | "right" => x = Some(p),
                "top" | "bottom" => y = Some(p),
                _ => pending_center += 1,
            }
        } else {
            let p = val(t)?;
            if x.is_none() && pending_center == 0 {
                x = Some(p);
            } else {
                y = Some(p);
            }
        }
        i += 1;
    }
    for _ in 0..pending_center {
        if x.is_none() {
            x = Some(Pos::CENTER);
        } else if y.is_none() {
            y = Some(Pos::CENTER);
        }
    }
    if x.is_none() && y.is_none() {
        return None;
    }
    Some([x.unwrap_or(Pos::CENTER), y.unwrap_or(Pos::CENTER)])
}

/// `background-position-x` / `-y`.
fn axis_pos(c: &Ctx, v: &[Tok], axis: usize) -> Option<Pos> {
    let first: Vec<Tok> = v.iter().take_while(|t| !matches!(t, Tok::Comma)).cloned().collect();
    let p = position(c, &first)?;
    // a lone keyword/length lands on x; for -y take it from whichever axis it went to
    Some(if axis == 0 || matches!(first.first().and_then(Tok::ident), Some("top" | "bottom")) { p[axis] } else { p[0] })
}

/// One `background-size` component: `auto` is None.
fn size_pos(c: &Ctx, t: &Tok) -> Option<Pos> {
    match length(t)? {
        Length::Percent(p) => Some(Pos { frac: p / 100.0, px: 0.0 }),
        l => l.to_px(&c.u).map(|px| Pos { frac: 0.0, px }),
    }
}

fn pair(c: &mut Ctx, v: &[Tok], a: &str, b: &str) -> bool {
    let (x, y) = match v {
        [one] => (one.clone(), one.clone()),
        [x, y] => (x.clone(), y.clone()),
        _ => return false,
    };
    delegate(c, a, &[x]) && delegate(c, b, &[y])
}

fn two_kw(c: &mut Ctx, v: &[Tok], a: &str, b: &str) -> bool {
    let (x, y) = match v {
        [one] => (one.clone(), one.clone()),
        [x, y] => (x.clone(), y.clone()),
        _ => return false,
    };
    delegate(c, a, &[x]) && delegate(c, b, &[y])
}

fn border_style(c: &mut Ctx, side: usize, t: &Tok) -> bool {
    match t.ident() {
        Some(s) if BORDER_STYLES.contains(&s) => {
            let none = s == "none" || s == "hidden";
            c.c.border_drawn[side] = !none;
            if none {
                set_border_width(c, side, 0.0);
            }
            c.c.border_style[side] = BorderStyle::parse(s).unwrap_or_default();
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::parse_declarations;

    fn computed(css: &str) -> Computed {
        let parent = Computed::default();
        let mut c = Computed::inherit(&parent);
        let u = Units::initial(13.0, (800.0, 600.0));
        for d in parse_declarations(css) {
            assert!(apply(&mut c, &parent, u, &d), "not applied: {}", d.prop);
        }
        c
    }

    #[test]
    fn box_model_and_flex() {
        let c = computed("display: flex; padding: 4px 8px; margin: 0 auto; flex: 1; gap: 2px 6px;
                          width: 50%; border: 2px solid #f00; border-radius: 4px");
        assert_eq!(c.display, DisplayKind::Flex);
        assert_eq!(c.layout.padding.left, LengthPercentage::length(8.0));
        assert_eq!(c.layout.margin.left, LengthPercentageAuto::auto());
        assert_eq!(c.layout.flex_grow, 1.0);
        assert_eq!(c.layout.size.width, Dimension::percent(0.5));
        assert_eq!(c.layout.border.top, LengthPercentage::length(2.0));
        assert_eq!(c.border_color[0], crate::color::rgb(0xff0000));
    }

    #[test]
    fn grid_and_text() {
        let c = computed("grid-template-columns: repeat(3, 1fr) 100px; grid-column: 1 / span 2;
                          font: italic bold 20px/1.5 serif; text-decoration: underline;
                          background: linear-gradient(to right, red, blue); box-shadow: 0 2px 4px #000");
        assert_eq!(c.layout.grid_template_columns.len(), 2);
        assert!(c.italic && c.bold() && c.underline && c.font_size == 20.0);
        assert_eq!(c.gradient.as_ref().unwrap().stops.len(), 2);
        assert_eq!(c.shadows.len(), 1);
    }

    /// A property is handled or accepted as having no effect, not both: the lookup takes the
    /// later, so one in both would never apply.
    #[test]
    fn handled_properties_are_not_also_ignored() {
        for name in ACCEPTED {
            assert!(!HANDLERS.iter().any(|(n, _)| n == name), "{name} is handled and ignored");
        }
    }

    /// Gradient stops: lengths and percentages, two positions in one stop, positions that go back
    /// moved up, and stops without one spaced evenly between their neighbours.
    #[test]
    fn gradient_stops_resolve_on_the_line() {
        let c = computed("background: linear-gradient(red 0 50px, blue 25%, green, white 80px)");
        let at: Vec<f32> = c.gradient.as_ref().unwrap().resolved(100.0).iter().map(|s| s.1).collect();
        assert_eq!(at, vec![0.0, 0.5, 0.5, 0.65, 0.8]);
    }

    /// `flex-wrap: balance` with `wrap-reverse` in either order, `flex-line-count`, and `flex-flow`.
    #[test]
    fn balanced_flex_wrapping() {
        assert_eq!(computed("flex-wrap: wrap-reverse balance").layout.flex_wrap, FlexWrap::BalanceReverse);
        assert_eq!(computed("flex-flow: column balance").layout.flex_wrap, FlexWrap::Balance);
        assert_eq!(computed("flex-line-count: 3").layout.flex_line_count, 3);
    }

    /// Sizing keywords on min / max sizes are kept for the tree (taffy has them as `auto`), and
    /// size containment with the size it sizes the box as.
    #[test]
    fn background_shorthand_sizes_a_gradient() {
        let c = computed("background: linear-gradient(red, red) 0 0/2ch 2ch no-repeat");
        let st = c.images.as_deref().expect("image style");
        assert!(c.gradient.is_some());
        assert_eq!(st.bg_repeat, (false, false));
        assert!(matches!(st.bg_size, BgSize::Size(Some(_), Some(_))), "{:?}", st.bg_size);
    }

    #[test]
    fn min_max_keywords_and_containment() {
        let c = computed("min-width: min-content; max-height: fit-content");
        assert_eq!(c.size_kw, crate::tree::SIZING_MIN_CONTENT | crate::tree::SIZING_FIT_CONTENT << 12);
        assert_eq!(c.layout.min_size.width, LengthPercentageAuto::auto());
        let c = computed("contain: inline-size; contain-intrinsic-size: auto 40px none");
        assert_eq!(c.contain, (crate::tree::CONTAIN_WIDTH, [40.0, 0.0]));
    }
}

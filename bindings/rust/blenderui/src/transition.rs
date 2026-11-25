//! View transitions (CSS View Transitions 1 and 2): [`Window::start_view_transition`] captures the
//! page as it is, the next render shows the new state, and the boxes with a `view-transition-name`
//! move and resize from their old place to their new one while the old pictures fade out over the
//! new ones (the page itself, `root`, cross-fades).
//!
//! The pseudo-element tree is drawn, not built: `::view-transition-group(name)`, `-old(name)` and
//! `-new(name)` take `animation-duration`, `-delay`, `-timing-function` and `animation: none`, by
//! name, `*` or `.class` (`view-transition-class`); their own `@keyframes` aren't run (each part
//! animates as the spec's defaults do). `:active-view-transition` doesn't match.
//!
//! [`Window::start_view_transition`]: crate::window::Window::start_view_transition

use std::sync::Arc;

use crate::anim::Timing;
use crate::css::{Decl, Tok};
use crate::geometry::{rect, Bounds};
use crate::layout::{Frame, StyleStore};
use crate::paint::{Image, Painter};

/// `view-transition-name` and `view-transition-class` of a box (`Computed::view_transition`).
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    /// The name; `None` for `auto` / `match-element` (the element's identity names it).
    pub name: Option<Box<str>>,
    pub classes: Box<[Box<str>]>,
}

impl Named {
    /// `view-transition-name`: `Some(None)` for `none`.
    pub(crate) fn parse_name(v: &[Tok]) -> Option<Option<Option<Box<str>>>> {
        match v {
            [t] => match t.ident()? {
                "none" => Some(None),
                "auto" | "match-element" => Some(Some(None)),
                n => Some(Some(Some(n.into()))),
            },
            _ => None,
        }
    }

    /// `view-transition-class: none | <ident>+`.
    pub(crate) fn parse_classes(v: &[Tok]) -> Option<Box<[Box<str>]>> {
        if v.len() == 1 && v[0].ident() == Some("none") {
            return Some(Box::default());
        }
        v.iter().map(|t| t.ident().map(Box::from)).collect()
    }
}

/// Which part of the pseudo-element tree a rule styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Group,
    ImagePair,
    Old,
    New,
}

/// `view-transition-class` names.
type Classes = Box<[Box<str>]>;

/// A rule for one of the transition's pseudo-elements (`Stylesheet::transitions`).
#[derive(Clone, Debug)]
pub(crate) struct PseudoRule {
    part: Part,
    /// What its argument selects: a name, `*`, and classes (`*.card`, `.card`, `hero`).
    name: Option<Box<str>>,
    classes: Box<[Box<str>]>,
    decls: Vec<Decl>,
}

impl PseudoRule {
    /// The rule `decls` under `selector`, when it is one of the transition's pseudo-elements.
    pub(crate) fn parse(selector: &str, decls: &[Decl]) -> Option<PseudoRule> {
        let at = selector.find("::view-transition")?;
        let rest = &selector[at + 2..];
        let open = rest.find('(');
        let part = match &rest[..open.unwrap_or(rest.len())] {
            "view-transition-group" => Part::Group,
            "view-transition-image-pair" => Part::ImagePair,
            "view-transition-old" => Part::Old,
            "view-transition-new" => Part::New,
            _ => return None,
        };
        let arg = open.and_then(|o| rest[o + 1..].split(')').next()).unwrap_or("*").trim();
        let mut bits = arg.split('.');
        let name = match bits.next()?.trim() {
            "" | "*" => None,
            n => Some(n.into()),
        };
        let classes = bits.map(|c| c.trim().into()).collect();
        Some(PseudoRule { part, name, classes, decls: decls.to_vec() })
    }

    fn selects(&self, name: &str, classes: &[Box<str>]) -> bool {
        self.name.as_deref().is_none_or(|n| n == name) && self.classes.iter().all(|c| classes.contains(c))
    }
}

/// How one part animates: when it starts and ends (seconds from the transition's start), and its easing.
#[derive(Clone, Debug, PartialEq)]
struct Timeline {
    delay: f32,
    duration: f32,
    easing: Timing,
}

impl Timeline {
    /// The spec's defaults: the user agent's 0.25 s `ease`.
    const DEFAULT: Timeline = Timeline { delay: 0.0, duration: 0.25, easing: crate::anim::EASE };

    /// The part `part` of `name` as the sheet's rules set it (later rules win).
    fn of(rules: &[PseudoRule], part: Part, name: &str, classes: &[Box<str>]) -> Timeline {
        let mut t = Timeline::DEFAULT;
        // the old and new pictures inherit the group's timing unless their own rules say otherwise
        let parts: &[Part] = match part {
            Part::Group => &[Part::Group],
            other => &[Part::Group, other],
        };
        for &p in parts {
            for r in rules.iter().filter(|r| r.part == p && r.selects(name, classes)) {
                for d in &r.decls {
                    t.take(d);
                }
            }
        }
        t
    }

    fn take(&mut self, d: &Decl) {
        let v: &[Tok] = &d.value;
        match d.prop.name() {
            "animation-duration" => self.duration = time(v).unwrap_or(self.duration),
            "animation-delay" => self.delay = time(v).unwrap_or(self.delay),
            "animation-timing-function" => {
                if let Some(e) = v.first().and_then(Timing::parse) {
                    self.easing = e;
                }
            }
            "animation" if v.first().and_then(Tok::ident) == Some("none") => self.duration = 0.0,
            "animation" => {
                // `<duration> [<easing>] [<delay>]` (names and the rest: the defaults' own)
                let mut times = v.iter().filter_map(|t| time(std::slice::from_ref(t)));
                if let Some(d) = times.next() {
                    self.duration = d;
                }
                if let Some(d) = times.next() {
                    self.delay = d;
                }
                if let Some(e) = v.iter().find_map(Timing::parse) {
                    self.easing = e;
                }
            }
            _ => {}
        }
    }

    /// Progress at `t` seconds, eased (0 before the delay, 1 after the end).
    fn at(&self, t: f32) -> f32 {
        if self.duration <= 0.0 {
            return if t >= self.delay { 1.0 } else { 0.0 };
        }
        self.easing.apply(((t - self.delay) / self.duration).clamp(0.0, 1.0), false)
    }

    fn end(&self) -> f32 {
        self.delay + self.duration
    }
}

fn time(v: &[Tok]) -> Option<f32> {
    match v.first()? {
        Tok::Dim(n, u) if u.eq_ignore_ascii_case("s") => Some(*n),
        Tok::Dim(n, u) if u.eq_ignore_ascii_case("ms") => Some(n / 1000.0),
        Tok::Num(n) if *n == 0.0 => Some(0.0),
        _ => None,
    }
}

/// A picture of a box (or of the page without its named boxes) and where it was.
struct Snap {
    image: Arc<Image>,
    at: Bounds<f32>,
}

/// One name's old and new pictures, and how its parts animate.
struct Pair {
    name: Box<str>,
    old: Option<Snap>,
    new: Option<Snap>,
    group: Timeline,
    fade_out: Timeline,
    fade_in: Timeline,
}

/// A view transition in progress (owned by its window).
#[derive(Default)]
pub(crate) struct Transition {
    root: Option<Snap>,
    root_fade: Option<Timeline>,
    pairs: Vec<Pair>,
    /// When the new state was first shown (window clock); `None` until then.
    start: Option<f64>,
    end: f32,
}

/// Each node's transition name in `frame` (an `auto` name is its key's), if it has one.
fn names<'f>(frame: &'f Frame, styles: &'f StyleStore) -> impl Iterator<Item = (usize, Box<str>, &'f [Box<str>])> + 'f {
    frame.nodes.iter().enumerate().filter_map(|(i, n)| {
        let v = styles[n.style].view_transition.as_deref()?;
        if n.taffy.is_none() || !n.visible {
            return None;
        }
        let name = v.name.clone().unwrap_or_else(|| format!("-bui-{:x}", n.key).into());
        Some((i, name, &*v.classes))
    })
}

/// For every node, whether it is a named box (`named`) or inside one.
fn hidden(frame: &Frame, named: &[bool]) -> Vec<bool> {
    let mut out: Vec<Option<bool>> = vec![None; frame.nodes.len()];
    let mut path = Vec::new();
    for i in 0..frame.nodes.len() {
        let mut k = Some(i);
        let v = loop {
            let Some(j) = k else { break false };
            if named[j] {
                break true;
            }
            if let Some(v) = out[j] {
                break v;
            }
            path.push(j);
            k = frame.nodes[j].parent;
        };
        for j in path.drain(..) {
            out[j] = Some(v);
        }
    }
    out.iter().zip(named).map(|(v, &n)| n || v.unwrap_or(false)).collect()
}

/// What paints a picture: the frame, its styles and the paint's caches.
pub(crate) struct Shot<'a> {
    pub frame: &'a Frame,
    pub styles: &'a StyleStore,
    pub text: &'a mut crate::text::TextCache,
    pub vp: (f32, f32),
    pub scale: f32,
    pub app: &'a crate::window::App,
}

impl Shot<'_> {
    /// The boxes `keep` selects, as one picture of `at`.
    fn take(&mut self, keep: &dyn Fn(usize) -> bool, at: Bounds<f32>) -> Option<Snap> {
        let image = crate::paint::snapshot(self.frame, self.styles, self.text, keep, at, self.vp, self.scale, self.app)?;
        Some(Snap { image, at })
    }
}

impl Transition {
    /// A transition from the page `shot` shows: pictures of it (without its named boxes) and of each named box.
    pub(crate) fn capture(shot: &mut Shot) -> Transition {
        let mut t = Transition::default();
        t.take_old(shot);
        t
    }

    fn take_old(&mut self, shot: &mut Shot) {
        let named_at: Vec<(usize, Box<str>)> = names(shot.frame, shot.styles).map(|(i, n, _)| (i, n)).collect();
        let mut named = vec![false; shot.frame.nodes.len()];
        for (i, _) in &named_at {
            named[*i] = true;
        }
        let inside = hidden(shot.frame, &named);
        let page = rect(0.0, 0.0, shot.vp.0, shot.vp.1);
        self.root = shot.take(&|i| !inside[i], page);
        for (i, name) in named_at {
            let at = shot.frame.nodes[i].bounds;
            let sub = subtree(shot.frame, i);
            let old = shot.take(&|k| sub[k], at);
            self.pairs.push(Pair { name, old, new: None, group: Timeline::DEFAULT, fade_out: Timeline::DEFAULT,
                                   fade_in: Timeline::DEFAULT });
        }
    }

    /// Paints the frame for time `now`: the new state's first frame takes its pictures and the
    /// timings; every frame hides the named boxes (`frame` is painted without them) and draws the
    /// transition over it. Returns `false` once it is over (nothing hidden, nothing drawn).
    pub(crate) fn paint(&mut self, shot: &mut Shot, rules: &[PseudoRule], now: f64, hide: &mut Vec<usize>) -> bool {
        if self.start.is_none() {
            self.start = Some(now);
            self.begin(shot, rules);
        }
        let t = (now - self.start.unwrap_or(now)) as f32;
        if t >= self.end {
            return false;
        }
        // the live named boxes are shown by their pictures
        let mut named = vec![false; shot.frame.nodes.len()];
        for (i, ..) in names(shot.frame, shot.styles) {
            named[i] = true;
        }
        for (i, h) in hidden(shot.frame, &named).into_iter().enumerate() {
            if h {
                hide.push(i);
            }
        }
        true
    }

    /// The new state's pictures and every part's timing.
    fn begin(&mut self, shot: &mut Shot, rules: &[PseudoRule]) {
        let mut classes_of: Vec<(Box<str>, Classes)> = Vec::new();
        for (i, name, classes) in names(shot.frame, shot.styles).collect::<Vec<_>>() {
            let at = shot.frame.nodes[i].bounds;
            let sub = subtree(shot.frame, i);
            let new = shot.take(&|k| sub[k], at);
            match self.pairs.iter_mut().find(|p| p.name == name) {
                Some(p) => p.new = new,
                None => self.pairs.push(Pair { name: name.clone(), old: None, new, group: Timeline::DEFAULT,
                                               fade_out: Timeline::DEFAULT, fade_in: Timeline::DEFAULT }),
            }
            classes_of.push((name, classes.into()));
        }
        let none: Classes = Box::default();
        self.end = 0.0;
        for p in &mut self.pairs {
            let classes = classes_of.iter().find(|(n, _)| *n == p.name).map_or(&none, |(_, c)| c);
            p.group = Timeline::of(rules, Part::Group, &p.name, classes);
            p.fade_out = Timeline::of(rules, Part::Old, &p.name, classes);
            p.fade_in = Timeline::of(rules, Part::New, &p.name, classes);
            self.end = self.end.max(p.group.end()).max(p.fade_out.end()).max(p.fade_in.end());
        }
        if self.root.is_some() {
            let fade = Timeline::of(rules, Part::Old, "root", &none);
            self.end = self.end.max(fade.end());
            self.root_fade = Some(fade);
        }
    }

    /// Draws the transition at `now` over what `p` holds.
    pub(crate) fn draw(&self, p: &mut Painter, now: f64) {
        let t = (now - self.start.unwrap_or(now)) as f32;
        if let (Some(root), Some(fade)) = (&self.root, &self.root_fade) {
            p.overlay(root.at, &root.image, 1.0 - fade.at(t));
        }
        let lerp = |a: f32, b: f32, k: f32| a + (b - a) * k;
        for pair in &self.pairs {
            // the group: from the old box to the new one; each picture fills its width
            let k = pair.group.at(t);
            let group = match (&pair.old, &pair.new) {
                (Some(a), Some(b)) => rect(lerp(a.at.origin.x, b.at.origin.x, k), lerp(a.at.origin.y, b.at.origin.y, k),
                                           lerp(a.at.size.width, b.at.size.width, k), lerp(a.at.size.height, b.at.size.height, k)),
                (Some(a), None) => a.at,
                (None, Some(b)) => b.at,
                (None, None) => continue,
            };
            let fill = |s: &Snap| {
                let h = if s.at.size.width > 0.0 { s.at.size.height * group.size.width / s.at.size.width } else { group.size.height };
                rect(group.origin.x, group.origin.y, group.size.width, h)
            };
            // the new picture under the old one: where both are opaque, the old fading out over
            // the new is the cross-fade
            if let Some(new) = &pair.new {
                p.overlay(fill(new), &new.image, if pair.old.is_some() { 1.0 } else { pair.fade_in.at(t) });
            }
            if let Some(old) = &pair.old {
                p.overlay(fill(old), &old.image, 1.0 - pair.fade_out.at(t));
            }
        }
    }
}

/// `in[k]`: whether node `k` is `root` or inside it.
fn subtree(frame: &Frame, root: usize) -> Vec<bool> {
    let mut mark = vec![false; frame.nodes.len()];
    mark[root] = true;
    hidden(frame, &mark)
}

#[cfg(test)]
mod tests {
    use super::{Part, PseudoRule, Timeline};

    #[test]
    fn pseudo_rules_select_by_name_and_class() {
        let r = PseudoRule::parse("::view-transition-old(*.card)", &[]).unwrap();
        assert_eq!(r.part, Part::Old);
        assert!(r.selects("a", &["card".into()]) && !r.selects("a", &[]));
        let r = PseudoRule::parse("html::view-transition-group(hero)", &[]).unwrap();
        assert!(r.part == Part::Group && r.selects("hero", &[]) && !r.selects("other", &[]));
        assert!(PseudoRule::parse("p::before", &[]).is_none());
    }

    #[test]
    fn timeline_eases_between_delay_and_end() {
        let t = Timeline { delay: 0.1, duration: 0.2, easing: crate::anim::Timing::Linear };
        assert_eq!((t.at(0.0), t.at(0.2), t.at(0.4)), (0.0, 0.5, 1.0));
    }
}

//! A WPT test or reference file as BUI elements: parsed by html5ever's tree builder (the HTML
//! parser's implied `<html>`/`<head>`/`<body>`, auto-closed `<p>`, entities ...), with its
//! stylesheets inlined and every URL made a path into the checkout. What the harness needs from
//! the file (its references, fuzzy tolerance, why it can't run) is read in the same walk. A page
//! with scripts runs them (`script.rs`) on its tree before it renders.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use blenderui::html::TAGS;
use blenderui::prelude::{Div, IntoElement, ParentElement, Styled};
use scraper::{Html, Node as Dom};

/// A reference the test must (`matches`) or must not look like.
pub struct Reference {
    pub path: PathBuf,
    pub matches: bool,
}

/// `<meta name="fuzzy">`: the largest channel difference and differing-pixel count allowed
/// (`maxDifference=0-2;totalPixels=0-300`, or the same as two bare ranges).
#[derive(Clone, Copy, Default)]
pub struct Fuzzy {
    pub max_diff: (u32, u32),
    pub pixels: (u32, u32),
}

impl Fuzzy {
    fn parse(content: &str) -> Option<Self> {
        // an optional `reference-url:` prefix names the reference it is for: it holds for all here
        let content = content.rsplit_once(':').map_or(content, |(_, r)| r);
        let mut parts = content.split(';').map(str::trim).filter(|p| !p.is_empty());
        let range = |part: &str| -> Option<(u32, u32)> {
            let v = part.split_once('=').map_or(part, |(_, v)| v).trim();
            match v.split_once('-') {
                Some((a, b)) => Some((a.trim().parse().ok()?, b.trim().parse().ok()?)),
                None => v.parse().ok().map(|n| (n, n)),
            }
        };
        let (mut max_diff, mut pixels) = (None, None);
        for (k, p) in parts.by_ref().take(2).enumerate() {
            // named, or by position: the difference first, then the pixel count
            let name = p.split_once('=').map(|(n, _)| n.trim());
            if name == Some("maxDifference") || (name.is_none() && k == 0) {
                max_diff = range(p);
            } else {
                pixels = range(p);
            }
        }
        Some(Self { max_diff: max_diff?, pixels: pixels? })
    }

    /// Whether a difference of at most `max` per channel over `count` pixels is allowed.
    pub const fn allows(self, max: u32, count: u32) -> bool {
        (max == 0 && count == 0)
            || (max >= self.max_diff.0 && max <= self.max_diff.1 && count >= self.pixels.0 && count <= self.pixels.1)
    }
}

/// What a browser's user agent sheet has that BUI's defaults don't, ahead of the page's sheets (so
/// any rule of theirs wins): BUI boxes are `border-box` by default (`docs/rust.md`), CSS's `content-box`.
/// Form controls stay `border-box`, as in browsers' sheets.
const BROWSER_DEFAULTS: &str = "*, ::before, ::after { box-sizing: content-box }     button, select, meter, progress, input:is([type=button], [type=submit], [type=reset], [type=color],     [type=checkbox], [type=radio], [type=search]) { box-sizing: border-box }";

/// A node of the document, ready to become elements every time the window renders. Elements are
/// numbered in document order (`serial`), which names them to the page's scripts.
#[derive(Clone)]
enum Node {
    Element { tag: &'static str, attrs: Vec<(Cow<'static, str>, Cow<'static, str>)>, style: Option<String>, children: Vec<Self>, serial: u32 },
    Text(String),
}

/// A loaded file: its elements, or why it can't run here.
pub struct Doc {
    root: Option<Node>,
    pub refs: Vec<Reference>,
    pub fuzzy: Option<Fuzzy>,
    /// Why the file can't be rendered faithfully (a nested browsing context ...).
    pub skip: Option<&'static str>,
    /// It has scripts: they run (`run_scripts`) before it renders, and its elements have ids
    /// (their own or `__w<serial>`) the harness finds them by.
    pub scripted: bool,
    /// What its scripts scrolled: (element id, x, y).
    pub scrolls: Vec<(String, f32, f32)>,
    /// The element its scripts focused (its id).
    pub focus: Option<String>,
    /// The popovers its scripts showed (their ids).
    pub popovers: Vec<String>,
    /// Its directory, and the next serial free.
    dir: PathBuf,
    serials: u32,
}

impl Doc {
    /// Load `file`, a page of the checkout at `wpt` (the root `/` URLs resolve against).
    pub fn load(wpt: &Path, file: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(file)?;
        let text = String::from_utf8_lossy(&bytes);
        let xml = file.extension().is_some_and(|e| e.eq_ignore_ascii_case("xht") || e.eq_ignore_ascii_case("xhtml"));
        let html = Html::parse_document(&if xml { xhtml_as_html(&text) } else { text.into_owned() });
        let dir = file.parent().unwrap_or(wpt);
        let mut cx = Load::new(wpt, dir, 0);
        let mut root = cx.node(*html.root_element());
        if let Some(Node::Element { children, .. }) = &mut root {
            children.insert(0, Node::Element { tag: "style", attrs: Vec::new(), style: None,
                                               children: vec![Node::Text(BROWSER_DEFAULTS.into())], serial: 0 });
        }
        Ok(Self { root, refs: cx.refs, fuzzy: cx.fuzzy, skip: cx.skip, scripted: cx.scripted, scrolls: Vec::new(), focus: None, popovers: Vec::new(),
                  dir: dir.to_path_buf(), serials: cx.serial + 1 })
    }

    /// The document as elements (built again for every render, as a view does).
    pub fn element(&self) -> Div {
        self.root.as_ref().and_then(|n| build(n, self.scripted)).unwrap_or_else(blenderui::html::html)
    }

    /// The same document, scripts not run (to lay it out for them).
    pub fn snapshot(&self) -> Self {
        Self { root: self.root.clone(), refs: Vec::new(), fuzzy: None, skip: None, scripted: self.scripted,
               scrolls: Vec::new(), focus: None, popovers: Vec::new(), dir: self.dir.clone(), serials: self.serials }
    }

    /// Each element's serial and the id it renders with (a scripted document's).
    pub fn ids(&self) -> Vec<(u32, String)> {
        fn walk(n: &Node, out: &mut Vec<(u32, String)>) {
            if let Node::Element { attrs, children, serial, .. } = n {
                out.push((*serial, id_of(attrs, *serial).into_owned()));
                for c in children {
                    walk(c, out);
                }
            }
        }
        let mut out = Vec::new();
        if let Some(r) = &self.root {
            walk(r, &mut out);
        }
        out
    }

    /// Run the page's scripts (`script.rs`) on its tree, `layout` being where its elements are
    /// (serial, [x, y, width, height]) before they run. Scripts that fail leave the tree as they
    /// left it, as in a browser.
    pub fn run_scripts(&mut self, wpt: &Path, layout: &[(u32, [f32; 4])]) {
        let Some(root) = &self.root else { return };
        let mut tree = String::new();
        json(root, &mut tree, true);
        let mut boxes = String::from("{");
        for (k, (s, b)) in layout.iter().enumerate() {
            let _ = write!(boxes, "{}\"{s}\":[{},{},{},{}]", if k == 0 { "" } else { "," }, b[0], b[1], b[2], b[3]);
        }
        boxes.push('}');
        let (owned_wpt, owned_dir) = (wpt.to_path_buf(), self.dir.clone());
        let parse = move |html: &str| {
            let frag = Html::parse_fragment(html);
            let mut cx = Load::new(&owned_wpt, &owned_dir, 0);
            let mut out = String::from("[");
            for (k, n) in frag.root_element().children().filter_map(|c| cx.node(c)).enumerate() {
                if k > 0 {
                    out.push(',');
                }
                json(&n, &mut out, false);
            }
            out.push(']');
            out
        };
        let Some(encoded) = crate::script::run(&tree, &boxes, self.serials, parse) else { return };
        let mut cx = Load::new(wpt, &self.dir, 0);
        let mut scrolls = Vec::new();
        let mut it = encoded.split('\u{1}');
        let focus = it.next().and_then(|s| s.parse::<u32>().ok());
        let shown: Vec<u32> = it.next().map(|s| s.split(',').filter_map(|k| k.parse().ok()).collect()).unwrap_or_default();
        if let Some(root) = cx.decode(&mut it, &mut scrolls) {
            self.root = Some(root);
            self.scrolls = scrolls;
            let ids = self.ids();
            self.focus = focus.and_then(|f| ids.iter().find(|(s, _)| *s == f)).map(|(_, id)| id.clone());
            self.popovers = ids.into_iter().filter(|(s, _)| shown.contains(s)).map(|(_, id)| id).collect();
        }
    }
}

/// The id element `serial` renders with: its own, or one for the harness to find it by.
fn id_of(attrs: &[(Cow<'static, str>, Cow<'static, str>)], serial: u32) -> Cow<'static, str> {
    attrs.iter().find(|(k, _)| k == "id").map_or_else(|| Cow::Owned(format!("__w{serial}")), |(_, v)| v.clone())
}

/// `n` as `dom.js` reads it: `{t, k, a, c}` elements (`k`, the serial, only `with_serial`: new
/// nodes get theirs there) and string texts.
fn json(n: &Node, out: &mut String, with_serial: bool) {
    match n {
        Node::Text(t) => quote(t, out),
        Node::Element { tag, attrs, style, children, serial } => {
            out.push_str("{\"t\":");
            quote(tag, out);
            if with_serial {
                let _ = write!(out, ",\"k\":{serial}");
            }
            out.push_str(",\"a\":[");
            let style = style.as_deref().map(|s| ("style", s));
            for (k, (name, value)) in attrs.iter().map(|(k, v)| (k.as_ref(), v.as_ref())).chain(style).enumerate() {
                out.push_str(if k == 0 { "[" } else { ",[" });
                quote(name, out);
                out.push(',');
                quote(value, out);
                out.push(']');
            }
            out.push_str("],\"c\":[");
            for (k, c) in children.iter().enumerate() {
                if k > 0 {
                    out.push(',');
                }
                json(c, out, with_serial);
            }
            out.push_str("]}");
        }
    }
}

/// `s` as a JSON string.
fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn build(n: &Node, ids: bool) -> Option<Div> {
    let Node::Element { tag, attrs, style, children, serial } = n else { return None };
    let mut d = Div::new(tag);
    if ids {
        d = d.id(id_of(attrs, *serial));
    }
    for (k, v) in attrs {
        // `#id` selectors match the element's identity
        d = if k == "id" { if ids { d } else { d.id(v.clone()) } } else { d.attr(k.clone(), v.clone()) };
    }
    if let Some(s) = style {
        d = d.style(s.as_str());
    }
    for c in children {
        d = match c {
            Node::Text(t) => d.child(t.clone()),
            Node::Element { .. } => match build(c, ids) {
                Some(e) => d.child(e.into_any_element()),
                None => d,
            },
        };
    }
    Some(d)
}

/// The walk that turns the DOM into [`Node`]s.
struct Load<'a> {
    wpt: &'a Path,
    dir: &'a Path,
    refs: Vec<Reference>,
    fuzzy: Option<Fuzzy>,
    skip: Option<&'static str>,
    scripted: bool,
    /// The last serial given.
    serial: u32,
}

impl<'a> Load<'a> {
    const fn new(wpt: &'a Path, dir: &'a Path, serial: u32) -> Self {
        Self { wpt, dir, refs: Vec::new(), fuzzy: None, skip: None, scripted: false, serial }
    }

    fn skip(&mut self, why: &'static str) {
        self.skip.get_or_insert(why);
    }

    fn node(&mut self, n: ego_tree::NodeRef<'_, Dom>) -> Option<Node> {
        match n.value() {
            Dom::Text(t) => Some(Node::Text(t.to_string())),
            Dom::Element(e) => {
                let name = e.name();
                let attr = |k: &str| e.attr(k);
                match name {
                    // its text, or its file's (`src`), to run (`run_scripts`)
                    "script" => {
                        self.scripted = true;
                        let text = match attr("src").and_then(|s| self.resolve(s)) {
                            Some(p) => script_file(&p),
                            None => n.children().filter_map(|c| c.value().as_text().map(|t| t.to_string())).collect(),
                        };
                        return Some(self.element(name, e.attrs().filter(|(k, _)| *k != "src"), vec![Node::Text(text)], None));
                    }
                    "iframe" | "frame" | "frameset" | "object" | "embed" => self.skip("nested document"),
                    // an inline load handler is a script too
                    "body" if attr("onload").is_some() => self.scripted = true,
                    "meta" => self.meta(attr("name").unwrap_or(""), attr("content").unwrap_or("")),
                    "link" => return self.link(attr("rel").unwrap_or(""), attr("href").unwrap_or("")),
                    _ => {}
                }
                let children = n.children().filter_map(|c| self.node(c)).collect();
                Some(self.element(name, e.attrs(), children, None))
            }
            _ => None,
        }
    }

    /// Element `name`: its URLs made paths and its style sheet's too (`serial`: its number, a new
    /// one when `None`).
    fn element<'v>(&mut self, name: &str, attrs: impl Iterator<Item = (&'v str, &'v str)>, mut children: Vec<Node>, serial: Option<u32>) -> Node {
        let mut out = Vec::new();
        let mut style = None;
        for (k, v) in attrs {
            match k {
                "style" => style = Some(self.css(v, self.dir)),
                "src" | "href" if !v.is_empty() => {
                    let p = self.resolve(v);
                    out.push((Cow::Owned(k.to_owned()), Cow::Owned(p.map_or_else(|| v.to_owned(), |p| slashes(&p)))));
                }
                _ => out.push((Cow::Owned(k.to_owned()), Cow::Owned(v.to_owned()))),
            }
        }
        if name == "style" {
            // the sheet's URLs are relative to the page
            let text: String = children.iter().filter_map(|c| match c {
                Node::Text(t) => Some(t.as_str()),
                Node::Element { .. } => None,
            }).collect();
            children = vec![Node::Text(self.css(&text, self.dir))];
        }
        let serial = serial.unwrap_or_else(|| {
            self.serial += 1;
            self.serial
        });
        Node::Element { tag: intern(name), attrs: out, style, children, serial }
    }

    /// A tree `dom.js` encoded (`__encode`), the scroll offsets its elements have put in `scrolls`.
    fn decode<'v>(&mut self, it: &mut impl Iterator<Item = &'v str>, scrolls: &mut Vec<(String, f32, f32)>) -> Option<Node> {
        if it.next()? == "T" {
            return Some(Node::Text(it.next()?.to_owned()));
        }
        let tag = it.next()?;
        let serial: u32 = it.next()?.parse().ok()?;
        let count: usize = it.next()?.parse().ok()?;
        let attrs = (0..count).map(|_| Some((it.next()?, it.next()?))).collect::<Option<Vec<_>>>()?;
        let scroll = it.next()?;
        let kids: usize = it.next()?.parse().ok()?;
        let children = (0..kids).map(|_| self.decode(it, scrolls)).collect::<Option<Vec<_>>>()?;
        if let Some((x, y)) = scroll.split_once(',') {
            let id = attrs.iter().find(|(k, _)| *k == "id").map_or_else(|| format!("__w{serial}"), |(_, v)| (*v).to_owned());
            scrolls.push((id, x.parse().ok()?, y.parse().ok()?));
        }
        Some(self.element(tag, attrs.into_iter(), children, Some(serial)))
    }

    fn meta(&mut self, name: &str, content: &str) {
        match name {
            "fuzzy" => self.fuzzy = Fuzzy::parse(content).or(self.fuzzy),
            "flags" => {
                for f in content.split_whitespace() {
                    match f {
                        "paged" => self.skip("paged media"),
                        "interact" | "userstyle" => self.skip("interaction"),
                        "animated" => self.skip("animation"),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// `<link>`: a stylesheet becomes the `<style>` it stands for; a reference is recorded.
    fn link(&mut self, rel: &str, href: &str) -> Option<Node> {
        let rel = rel.to_ascii_lowercase();
        let rels = || rel.split_whitespace();
        let matches = rels().any(|r| r == "match");
        if matches || rels().any(|r| r == "mismatch") {
            match self.resolve(href) {
                Some(path) => self.refs.push(Reference { path, matches }),
                None => self.skip("remote reference"),
            }
            return None;
        }
        if !rels().any(|r| r == "stylesheet") || rels().any(|r| r == "alternate") {
            return None;
        }
        let path = self.resolve(href)?;
        let Ok(text) = std::fs::read_to_string(&path) else {
            self.skip("missing stylesheet");
            return None;
        };
        let css = self.css(&text, path.parent().unwrap_or(self.wpt));
        self.serial += 1;
        Some(Node::Element { tag: "style", attrs: Vec::new(), style: None, children: vec![Node::Text(css)], serial: self.serial })
    }

    /// The file a URL names: relative to `self.dir`, or to the checkout's root when it starts
    /// with `/`. `None` for other schemes (`data:` stays a URL, `http:` is out of reach).
    fn resolve(&self, url: &str) -> Option<PathBuf> {
        resolve(self.wpt, self.dir, url)
    }

    /// `css` with its `url()`s made paths (relative to `dir`, the sheet's directory) and its
    /// `@import`s inlined.
    fn css(&mut self, css: &str, dir: &Path) -> String {
        let mut out = String::with_capacity(css.len());
        let mut rest = css;
        while let Some(at) = find_ci(rest, "url(").into_iter().chain(find_ci(rest, "@import")).min() {
            out.push_str(&rest[..at]);
            rest = &rest[at..];
            if rest.starts_with('@') {
                let end = rest.find(';').map_or(rest.len(), |e| e + 1);
                let target = rest["@import".len()..end].trim_end_matches(';').trim();
                let target = target.strip_prefix("url(").map_or(target, |t| t.split(')').next().unwrap_or(t));
                let target = target.split_whitespace().next().unwrap_or("").trim_matches(['"', '\'']);
                match resolve(self.wpt, dir, target).and_then(|p| std::fs::read_to_string(&p).ok().map(|t| (p, t))) {
                    Some((p, t)) => {
                        let inner = self.css(&t, p.parent().unwrap_or(self.wpt));
                        out.push_str(&inner);
                    }
                    None => self.skip("missing stylesheet"),
                }
                rest = &rest[end..];
                continue;
            }
            let Some(close) = rest.find(')') else { break };
            let arg = rest[4..close].trim().trim_matches(['"', '\'']);
            match resolve(self.wpt, dir, arg) {
                Some(p) => {
                    out.push_str("url(\"");
                    out.push_str(&slashes(&p));
                    out.push_str("\")");
                }
                None => out.push_str(&rest[..=close]),
            }
            rest = &rest[close + 1..];
        }
        out.push_str(rest);
        out
    }
}

/// An XHTML file (`.xht`, served as XML) as the HTML parser reads it the same: CDATA sections
/// become their text and `<div/>` an empty element (the HTML parser would leave it open).
fn xhtml_as_html(src: &str) -> String {
    const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"];
    // XML drops comments everywhere, a `<style>`'s text included (HTML keeps them there, as CSS)
    let mut bare = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(at) = rest.find("<!--") {
        bare.push_str(&rest[..at]);
        rest = rest[at..].find("-->").map_or("", |end| &rest[at + end + 3..]);
    }
    bare.push_str(rest);
    let src = bare.replace("<![CDATA[", "").replace("]]>", "");
    let mut out = String::with_capacity(src.len());
    let mut rest = src.as_str();
    while let Some(end) = rest.find("/>") {
        let head = &rest[..end];
        let tag = head.rfind('<').map(|lt| &head[lt + 1..]).and_then(|t| t.split(|c: char| c.is_whitespace()).next())
            .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':'));
        match tag {
            Some(t) if !VOID.contains(&t.to_ascii_lowercase().as_str()) => {
                out.push_str(head);
                out.push_str("></");
                out.push_str(t);
                out.push('>');
            }
            _ => out.push_str(&rest[..end + 2]),
        }
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    out
}

/// The source of script file `p` to run: testharness.js is its API doing nothing (`dom.js`).
fn script_file(p: &Path) -> String {
    match p.file_name().and_then(|n| n.to_str()) {
        Some("testharness.js") => "__testharness()".into(),
        Some("testharnessreport.js") => String::new(),
        _ => std::fs::read_to_string(p).unwrap_or_default(),
    }
}

/// See [`Load::resolve`]. A path into the checkout already is one (a tree back from its scripts).
fn resolve(wpt: &Path, dir: &Path, url: &str) -> Option<PathBuf> {
    let url = url.trim();
    let url = url.split(['?', '#']).next().unwrap_or("");
    if url.is_empty() || url.contains("://") || url.starts_with("data:") || url.starts_with("//") {
        return None;
    }
    if Path::new(url).starts_with(wpt) {
        return Some(PathBuf::from(url));
    }
    let mut out = if url.starts_with('/') { wpt.to_path_buf() } else { dir.to_path_buf() };
    for part in url.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    Some(out)
}

/// The first case-insensitive occurrence of the ASCII `needle` in `hay`.
fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    hay.as_bytes().windows(needle.len()).position(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

/// A path as CSS and the kit read it.
fn slashes(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// A tag name for the element's lifetime: the standard ones are the library's constants; others
/// (custom elements, SVG's) are leaked once each (tag names of a test run are a small, closed set).
fn intern(name: &str) -> &'static str {
    static OTHERS: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    if let Some((t, _)) = TAGS.iter().find(|(t, _)| *t == name) {
        return t;
    }
    let mut others = OTHERS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(t) = others.iter().find(|t| **t == name) {
        return t;
    }
    let t: &'static str = Box::leak(name.to_owned().into_boxed_str());
    others.push(t);
    t
}

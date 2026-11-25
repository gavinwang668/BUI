//! Text for inline layout (CSS Text 3 §4 and §5): white space processing, `text-transform`, and
//! where lines may break (UAX #14, `unicode-linebreak`, tailored by `word-break`, `line-break`,
//! `hyphens` and `overflow-wrap`). Pure functions: the builder (`layout/inline.rs`) runs them over a
//! block's whole inline content, so breaks across element boundaries follow the text around them.

use crate::style::{TextTransform, WhiteSpace};
use std::borrow::Cow;

/// `word-break: keep-all`: no breaks between letters (CJK included).
pub(crate) const WRAP_KEEP_ALL: u8 = 1;
/// `line-break: anywhere`: a break between any two characters.
pub(crate) const WRAP_ANYWHERE: u8 = 2;
/// `hyphens: none`: a soft hyphen is no break.
pub(crate) const WRAP_NO_HYPHENS: u8 = 4;
/// `overflow-wrap: break-word` (rather than `anywhere`): its last-resort breaks don't narrow the
/// min-content size.
pub(crate) const WRAP_BREAK_WORD: u8 = 8;

/// What an atomic inline is in a block's text (U+FFFC OBJECT REPLACEMENT CHARACTER).
pub(crate) const OBJECT: char = '\u{fffc}';

/// `text-wrap-style: balance`: lines as even as the line count allows.
pub(crate) const WRAP_BALANCE: u8 = 128;

/// `text-autospace: no-autospace`: no extra space between ideographs and Latin letters or digits.
pub(crate) const WRAP_NO_AUTOSPACE: u8 = 64;

/// `text-transform: full-width`: printable ASCII (and the space) as their fullwidth forms.
pub(crate) const TRANSFORM_FULL_WIDTH: u8 = 1;
/// `text-transform: full-size-kana`: small kana as their full-size letters.
pub(crate) const TRANSFORM_FULL_SIZE_KANA: u8 = 2;

/// The soft hyphen (U+00AD): a break that shows a hyphen when taken.
pub(crate) const SOFT_HYPHEN: char = '\u{ad}';

/// Document white space, which `white-space` collapses (CSS Text 3 §4.1): space, tab, segment
/// breaks, form feed. Other spaces (U+3000 ...) are characters of the text.
pub(crate) const fn collapsible(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

/// A space that hangs at the end of a line (CSS Text 3 §4.1.3): preserved spaces and the other
/// space separators (U+3000, U+2000-200A ...), not the no-break ones.
pub(crate) const fn hangs(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\u{1680}' | '\u{2000}'..='\u{2006}' | '\u{2008}'..='\u{200a}' | '\u{205f}' | '\u{3000}')
}

/// Whether white space collapses (`normal`, `nowrap`, `pre-line`).
pub(crate) const fn collapses(ws: WhiteSpace) -> bool {
    matches!(ws, WhiteSpace::Normal | WhiteSpace::NoWrap | WhiteSpace::PreLine)
}

/// Whether segment breaks (newlines) are kept as forced breaks.
pub(crate) const fn keeps_breaks(ws: WhiteSpace) -> bool {
    !matches!(ws, WhiteSpace::Normal | WhiteSpace::NoWrap)
}

/// Whether lines may wrap.
pub(crate) const fn wraps(ws: WhiteSpace) -> bool {
    !matches!(ws, WhiteSpace::NoWrap | WhiteSpace::Pre)
}

/// East Asian wide, fullwidth or halfwidth, and not Hangul (CSS Text 3 §4.1.2: a segment break
/// between two of these is removed, not made a space).
fn east_asian(c: char) -> bool {
    matches!(c as u32, 0x2e80..=0x303e | 0x3041..=0x33ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xa000..=0xa4cf
        | 0xf900..=0xfaff | 0xfe30..=0xfe4f | 0xff00..=0xffef | 0x20000..=0x3fffd)
}

/// An ideograph or kana (CSS Text 4 `text-autospace`: Han, Hiragana, Katakana, Bopomofo).
const fn ideograph(c: char) -> bool {
    matches!(c as u32, 0x3041..=0x30ff | 0x3100..=0x312f | 0x31a0..=0x31ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff
        | 0xf900..=0xfaff | 0x20000..=0x3fffd)
}

/// A letter or digit of a script that isn't East Asian (Latin, Greek, Cyrillic ...).
fn alphanumeric(c: char) -> bool {
    c.is_alphanumeric() && (c as u32) < 0x2e80
}

/// Whether `text-autospace: normal` puts 1/8 em between `a` and `b` (CSS Text 4 section 6.4:
/// `ideograph-alpha` and `ideograph-numeric`).
pub(crate) fn autospace(a: char, b: char) -> bool {
    (ideograph(a) && alphanumeric(b)) || (alphanumeric(a) && ideograph(b))
}

/// White space processing phase I and `text-transform` (CSS Text 3 §4.1.1, §2.1): with collapsing
/// white space, tabs and other white space become spaces and a segment break a space (or nothing,
/// next to a zero width space or between East Asian characters); preserved tabs advance to the next
/// multiple of `tab` spaces (with `None`, they stay: inline layout gives them their advance). Runs of
/// spaces collapse later, across elements. Borrows when nothing changes.
pub(crate) fn transform(s: &str, (t, extra): (TextTransform, u8), ws: WhiteSpace, tab: Option<u8>) -> Cow<'_, str> {
    // bidi formatting characters are default-ignorable: no glyph, no advance (CSS Text 3 4.1)
    let control = |c: char| matches!(c, '\u{200e}' | '\u{200f}' | '\u{061c}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
    if s.contains(control) {
        let bare: String = s.chars().filter(|&c| !control(c)).collect();
        return Cow::Owned(transform(&bare, (t, extra), ws, tab).into_owned());
    }
    let collapse = collapses(ws) && s.contains(['\n', '\t', '\r', '\x0c']);
    let expand_tabs = tab.is_some() && !collapses(ws) && s.contains('\t');
    let tab = tab.unwrap_or(8);
    if !collapse && !expand_tabs && t == TextTransform::None && extra == 0 {
        return Cow::Borrowed(s);
    }
    let s = if collapse {
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::with_capacity(s.len());
        for (k, &c) in chars.iter().enumerate() {
            match c {
                '\n' if ws == WhiteSpace::PreLine => out.push('\n'),
                '\n' => {
                    // the characters around the break, past the spaces next to it
                    let before = chars[..k].iter().rev().find(|c| !collapsible(**c)).copied();
                    let after = chars[k + 1..].iter().find(|c| !collapsible(**c)).copied();
                    let zwsp = before == Some('\u{200b}') || after == Some('\u{200b}');
                    let wide = before.is_some_and(east_asian) && after.is_some_and(east_asian);
                    if !zwsp && !wide {
                        out.push(' ');
                    }
                }
                '\t' | '\r' | '\x0c' => out.push(' '),
                c => out.push(c),
            }
        }
        out
    } else if expand_tabs {
        let mut out = String::with_capacity(s.len() + 16);
        let mut col = 0usize;
        for ch in s.chars() {
            match ch {
                '\t' if tab > 0 => {
                    let n = tab as usize - col % tab as usize;
                    out.extend(std::iter::repeat_n(' ', n));
                    col += n;
                }
                '\t' => {}
                '\n' => {
                    out.push('\n');
                    col = 0;
                }
                c => {
                    out.push(c);
                    col += 1;
                }
            }
        }
        out
    } else {
        s.to_string()
    };
    let cased = match t {
        TextTransform::None => s,
        TextTransform::Upper => s.to_uppercase(),
        TextTransform::Lower => s.to_lowercase(),
        TextTransform::Capitalize => capitalize(&s),
    };
    if extra == 0 {
        return Cow::Owned(cased);
    }
    Cow::Owned(cased.chars().map(|c| {
        let c = if extra & TRANSFORM_FULL_SIZE_KANA != 0 { full_size_kana(c) } else { c };
        if extra & TRANSFORM_FULL_WIDTH != 0 { full_width(c) } else { c }
    }).collect())
}

/// `full-width` (CSS Text 3 section 2.1): ASCII to its fullwidth form, the space to U+3000.
fn full_width(c: char) -> char {
    match c {
        ' ' => '\u{3000}',
        '!'..='~' => char::from_u32(c as u32 + 0xfee0).unwrap_or(c),
        c => c,
    }
}

/// `full-size-kana` (CSS Text 3 Appendix G): small kana to their full-size letters.
fn full_size_kana(c: char) -> char {
    let to = match c as u32 {
        // hiragana: the small ones sit just before their full-size letter
        0x3041 | 0x3043 | 0x3045 | 0x3047 | 0x3049 | 0x3063 | 0x3083 | 0x3085 | 0x3087 | 0x308e => c as u32 + 1,
        0x3095 => 0x304b,
        0x3096 => 0x3051,
        // katakana
        0x30a1 | 0x30a3 | 0x30a5 | 0x30a7 | 0x30a9 | 0x30c3 | 0x30e3 | 0x30e5 | 0x30e7 | 0x30ee => c as u32 + 1,
        0x30f5 => 0x30ab,
        0x30f6 => 0x30b1,
        // katakana phonetic extensions
        0x31f0 => 0x30af, 0x31f1 => 0x30b7, 0x31f2 => 0x30b9, 0x31f3 => 0x30c8, 0x31f4 => 0x30cc,
        0x31f5 => 0x30cf, 0x31f6 => 0x30d2, 0x31f7 => 0x30d5, 0x31f8 => 0x30d8, 0x31f9 => 0x30db,
        0x31fa => 0x30e0, 0x31fb => 0x30e9, 0x31fc => 0x30ea, 0x31fd => 0x30eb, 0x31fe => 0x30ec, 0x31ff => 0x30ed,
        // halfwidth katakana
        0xff67 => 0xff71, 0xff68 => 0xff72, 0xff69 => 0xff73, 0xff6a => 0xff74, 0xff6b => 0xff75,
        0xff6c => 0xff94, 0xff6d => 0xff95, 0xff6e => 0xff96, 0xff6f => 0xff82,
        // small ku, ke ... (Kana Extended-B / Small Kana Extension)
        0x1b132 => 0x3053, 0x1b150 => 0x3090, 0x1b151 => 0x3091, 0x1b152 => 0x3092,
        0x1b155 => 0x30b3, 0x1b164 => 0x30f0, 0x1b165 => 0x30f1, 0x1b166 => 0x30f2, 0x1b167 => 0x30f3,
        _ => return c,
    };
    char::from_u32(to).unwrap_or(c)
}

/// `text-transform: capitalize`: each word's first letter (a word starts after a space or
/// punctuation other than an apostrophe) upper-cased.
fn capitalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        if start && c.is_alphanumeric() {
            out.extend(c.to_uppercase());
            start = false;
            continue;
        }
        out.push(c);
        if c.is_whitespace() || (c.is_ascii_punctuation() && c != '\'' && c != '-') {
            start = true;
        } else if c.is_alphanumeric() {
            start = false;
        }
    }
    out
}

/// A break opportunity before byte `at` of a block's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opportunity {
    /// A line must end here (U+2028, a vertical tab ...).
    Forced,
    /// One may.
    Allowed,
    /// Only when nothing else on the line fits (`overflow-wrap`).
    LastResort,
    /// None (a cut between pieces that must stay on one line, such as before a tab).
    Glued,
}

/// `line-break: strict`: no break before small kana and the like (CJ), as UAX #14 has it.
pub(crate) const WRAP_STRICT: u8 = 16;
/// `line-break: loose`: breaks before iteration marks and the like (NS) and inseparables (IN) too.
pub(crate) const WRAP_LOOSE: u8 = 32;

/// The break opportunities in `s` (a block's inline content, U+FFFC for each atomic inline): UAX
/// #14's, then what the text's style says: `style(before, at)` gives, for the boundary between the
/// characters at those bytes, the `white-space` that rules it (the nearest common ancestor's of the
/// two, CSS Text 3 §5.1), and the `WRAP_*` flags, `word-break: break-all` and `overflow-wrap` of the
/// text before it. Each is (byte offset, kind); the end of `s` is not one.
pub(crate) fn opportunities(s: &str, style: impl Fn(usize, usize) -> (WhiteSpace, u8, bool, bool)) -> Vec<(usize, Opportunity)> {
    use unicode_linebreak::{break_property, BreakClass};
    let mut out: Vec<(usize, Opportunity)> = Vec::new();
    // an atomic inline breaks as an ideograph (CSS Text 3 section 5.1): one as long in UTF-8
    let ideographic: Cow<'_, str> = if s.contains(OBJECT) { Cow::Owned(s.replace(OBJECT, "\u{4e00}")) } else { Cow::Borrowed(s) };
    let uax: Vec<(usize, bool)> = unicode_linebreak::linebreaks(&ideographic)
        .filter(|&(at, _)| at < s.len())
        .map(|(at, o)| (at, o == unicode_linebreak::BreakOpportunity::Mandatory))
        .collect();
    let mut uax = uax.into_iter().peekable();
    let mut prev: Option<(usize, char)> = None;
    for (at, c) in s.char_indices() {
        let Some((p, before)) = prev.replace((at, c)) else { continue };
        let found = match uax.peek() {
            Some(&(u, forced)) if u == at => {
                uax.next();
                Some(forced)
            }
            _ => None,
        };
        let (ws, wrap, all, word) = style(p, at);
        let letters = before.is_alphanumeric() && c.is_alphanumeric();
        if found == Some(true) {
            out.push((at, Opportunity::Forced));
            continue;
        }
        if !wraps(ws) {
            continue;
        }
        // ... and a no-break space doesn't join it to the text (a narrow one or a word joiner does)
        let mut allowed = found.is_some() || matches!((before, c), (OBJECT, '\u{a0}') | ('\u{a0}', OBJECT));
        // UAX #14 is strict about small kana (CJ as NS); CSS's `normal` and `loose` break before them,
        // `loose` before iteration marks (NS) and inseparables (IN) too, but not after a space
        // (the break is before the space then)
        if !allowed && !collapsible(before) && !hangs(before) {
            let class = break_property(c as u32);
            let ideographic = matches!(break_property(before as u32), BreakClass::Ideographic | BreakClass::ConditionalJapaneseStarter
                | BreakClass::Alphabetic | BreakClass::NonStarter | BreakClass::Inseparable);
            allowed = ideographic && match class {
                BreakClass::ConditionalJapaneseStarter => wrap & WRAP_STRICT == 0,
                BreakClass::NonStarter | BreakClass::Inseparable => wrap & WRAP_LOOSE != 0,
                _ => false,
            };
        }
        if wrap & WRAP_ANYWHERE != 0 || (all && letters) {
            allowed = true;
        }
        if wrap & WRAP_KEEP_ALL != 0 && letters {
            allowed = false;
        }
        if before == SOFT_HYPHEN && wrap & WRAP_NO_HYPHENS != 0 {
            allowed = false;
        }
        // `break-spaces`: after each space, and its spaces don't hang (they are where it breaks)
        if ws == WhiteSpace::BreakSpaces && hangs(before) {
            allowed = true;
        }
        if allowed {
            out.push((at, Opportunity::Allowed));
        } else if word && !collapsible(before) && !collapsible(c) {
            out.push((at, Opportunity::LastResort));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> Vec<usize> {
        opportunities(s, |_, _| (WhiteSpace::Normal, 0, false, false)).into_iter()
            .filter(|o| o.1 == Opportunity::Allowed).map(|o| o.0).collect()
    }

    /// Breaks after spaces, between ideographs, after a hyphen; none before closing punctuation or
    /// inside a word.
    #[test]
    fn uax14_opportunities() {
        assert_eq!(at("aa bb"), vec![3]);
        assert_eq!(at("中文。字"), vec![3, 9], "between ideographs, not before the full stop");
        assert_eq!(at("well-known"), vec![5]);
        assert_eq!(at("a\u{a0}b"), Vec::<usize>::new(), "no-break space");
    }

    /// `keep-all` keeps letters together; `nowrap` text has none.
    #[test]
    fn style_tailoring() {
        let keep = opportunities("中文 字", |_, _| (WhiteSpace::Normal, WRAP_KEEP_ALL, false, false));
        assert_eq!(keep, vec![(7, Opportunity::Allowed)]);
        let none = opportunities("aa bb", |_, _| (WhiteSpace::NoWrap, 0, false, false));
        assert!(none.is_empty());
    }

    /// A segment break between ideographs is removed, else it's a space; capitalize starts words.
    #[test]
    fn white_space_and_transform() {
        assert_eq!(transform("中\n文", (TextTransform::None, 0), WhiteSpace::Normal, Some(8)), "中文");
        assert_eq!(transform("a\nb", (TextTransform::None, 0), WhiteSpace::Normal, Some(8)), "a b");
        assert_eq!(transform("hello (world)", (TextTransform::Capitalize, 0), WhiteSpace::Normal, Some(8)), "Hello (World)");
        assert_eq!(transform("a b", (TextTransform::Upper, TRANSFORM_FULL_WIDTH), WhiteSpace::Normal, Some(8)), "Ａ\u{3000}Ｂ");
        assert_eq!(transform("ぁッ", (TextTransform::None, TRANSFORM_FULL_SIZE_KANA), WhiteSpace::Normal, Some(8)), "あツ");
    }
}

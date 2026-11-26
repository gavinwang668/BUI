//! BUI Sheet: a spreadsheet made of Blender's UI. The window is a Blender screen (drag an area's
//! corner to split it, change an area's editor from its header): a Spreadsheet editor (the grid,
//! a toolbar header, a formula bar in its tool header, sheet tabs in its footer and a Properties
//! sidebar), a Chart editor and a Functions editor. Every control is a Blender widget: number
//! fields you drag or type math into (`2*pi`, `80*1.5`), dropdown menus, color swatches with
//! Blender's color picker, checkboxes, text fields.
//!
//! The grid is plain elements (thousands of Blender widgets would be slow); only what is on screen
//! is laid out. Formulas: `+ - * / ^ & %`, comparisons, cell refs (`A1`, `$A$1`, `Sales!B2`),
//! ranges, SUM AVERAGE MIN MAX COUNT IF ROUND ABS SQRT AND OR PI, recalculated on every change with
//! `#REF!`, `#DIV/0!`, `#NAME?`, `#VALUE!` and `#CYCLE!` errors.
//!
//! Mouse: click / drag to select (Shift extends), double-click to edit, drag a column header's edge
//! to resize, drag the fill handle to copy or continue a series, click a header to select its
//! row or column. Keys: arrows (Shift extends), Enter / Tab, F2 edit, type to replace, Esc cancel,
//! Delete clear, Ctrl+C / X / V, Ctrl+Z / Y, Ctrl+B bold, Ctrl+A all, Page Up / Down, Home.
//!
//! `cargo run --release --bin bui_sheet`, `-- --storyboard DIR` drives it headlessly.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::Arc;

use blenderui::prelude::*;
use blenderui::screen::{AreaRef, AreaSplit, Editor, Editors, Region, Screen};
use bui_demos::story::story;
use bui_demos::text::{edit, Edit};
use bui_demos::{arg, focus_root, run_with, size_arg, Script};
use bui_demos::color::{hex, parse_hex};

const COLS: u16 = 26;
const ROWS: u16 = 200;
const ROW_H: f32 = 24.0;
const HEAD_W: f32 = 46.0;
const COL_W: f32 = 92.0;
/// Rows laid out from the top one (the area clips what doesn't fit).
const VIEW_ROWS: u16 = 44;
/// Columns are laid out until they reach this far.
const VIEW_W: f32 = 1900.0;
/// The grid's size before the first frame measures it.
const VIEW_GUESS: (f32, f32) = (680.0, 700.0);
const UNDO: usize = 200;

// ------------------------------------------------------------------------------------------------
// The workbook

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum NumFmt {
    #[default]
    General,
    Number,
    Currency,
    Percent,
}

const NUM_FMTS: [(&str, NumFmt); 4] =
    [("General", NumFmt::General), ("Number", NumFmt::Number), ("Currency", NumFmt::Currency), ("Percent", NumFmt::Percent)];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Align {
    #[default]
    Auto,
    Left,
    Center,
    Right,
}

const ALIGNS: [(&str, Align); 4] = [("Auto", Align::Auto), ("Left", Align::Left), ("Center", Align::Center), ("Right", Align::Right)];

#[derive(Clone, Copy, PartialEq, Debug)]
struct Fmt {
    num: NumFmt,
    decimals: u8,
    bold: bool,
    align: Align,
    fill: Option<u32>,
    color: Option<u32>,
}

impl Default for Fmt {
    fn default() -> Self {
        Self { num: NumFmt::General, decimals: 2, bold: false, align: Align::Auto, fill: None, color: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
struct Cell {
    raw: String,
    fmt: Fmt,
}

#[derive(Clone)]
struct Sheet {
    name: String,
    cells: HashMap<(u16, u16), Cell>,
    widths: Vec<f32>,
}

impl Sheet {
    fn new(name: &str) -> Self {
        Self { name: name.to_string(), cells: HashMap::new(), widths: vec![COL_W; COLS as usize] }
    }

    fn raw(&self, at: (u16, u16)) -> &str {
        self.cells.get(&at).map_or("", |c| c.raw.as_str())
    }

    fn fmt(&self, at: (u16, u16)) -> Fmt {
        self.cells.get(&at).map(|c| c.fmt).unwrap_or_default()
    }

    /// Sets a cell's text, keeping its format; an empty, unformatted cell is dropped.
    fn set_raw(&mut self, at: (u16, u16), raw: String) {
        let cell = self.cells.entry(at).or_default();
        cell.raw = raw;
        if cell.raw.is_empty() && cell.fmt == Fmt::default() {
            self.cells.remove(&at);
        }
    }

    fn set_cell(&mut self, at: (u16, u16), cell: Cell) {
        if cell.raw.is_empty() && cell.fmt == Fmt::default() {
            self.cells.remove(&at);
        } else {
            self.cells.insert(at, cell);
        }
    }
}

#[derive(Clone)]
struct Book {
    sheets: Vec<Sheet>,
}

impl Book {
    fn sheet_index(&self, name: &str) -> Option<usize> {
        self.sheets.iter().position(|s| s.name.eq_ignore_ascii_case(name))
    }
}

/// Column letters: 0 = A, 25 = Z.
const fn col_name(c: u16) -> char {
    (b'A' + c as u8) as char
}

fn addr((r, c): (u16, u16)) -> String {
    format!("{}{}", col_name(c), r + 1)
}

/// `B12`, `$B$12` as (row, column, row absolute, column absolute).
fn parse_ref(w: &str) -> Option<(u16, u16, bool, bool)> {
    let b = w.as_bytes();
    let mut i = 0;
    let col_abs = b.first() == Some(&b'$');
    if col_abs {
        i += 1;
    }
    let letter = *b.get(i)?;
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    let c = u16::from(letter.to_ascii_uppercase() - b'A');
    i += 1;
    let row_abs = b.get(i) == Some(&b'$');
    if row_abs {
        i += 1;
    }
    let digits = &w[i..];
    if digits.is_empty() || !digits.bytes().all(|d| d.is_ascii_digit()) {
        return None;
    }
    let r: u32 = digits.parse().ok()?;
    (r >= 1 && r <= u32::from(ROWS) && c < COLS).then_some(((r - 1) as u16, c, row_abs, col_abs))
}

// ------------------------------------------------------------------------------------------------
// Formulas

#[derive(Clone, Debug, PartialEq)]
enum Val {
    Empty,
    Num(f64),
    Text(String),
    Bool(bool),
    Err(&'static str),
}

impl Val {
    fn num(&self) -> Result<f64, &'static str> {
        match self {
            Self::Num(n) => Ok(*n),
            Self::Empty => Ok(0.0),
            Self::Bool(b) => Ok(f64::from(u8::from(*b))),
            Self::Text(t) => literal_num(t).ok_or("#VALUE!"),
            Self::Err(e) => Err(e),
        }
    }

    fn truthy(&self) -> Result<bool, &'static str> {
        match self {
            Self::Bool(b) => Ok(*b),
            Self::Text(t) => Ok(!t.is_empty()),
            v => v.num().map(|n| n != 0.0),
        }
    }

    fn text(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Num(n) => general(*n),
            Self::Text(t) => t.clone(),
            Self::Bool(b) => if *b { "TRUE".into() } else { "FALSE".into() },
            Self::Err(e) => (*e).to_string(),
        }
    }
}

/// A typed number: `1,200`, `$45.50`, `12%`.
fn literal_num(t: &str) -> Option<f64> {
    let t = t.trim();
    let (t, pct) = t.strip_suffix('%').map_or((t, false), |s| (s, true));
    let t = t.strip_prefix('$').unwrap_or(t);
    let clean: String = t.chars().filter(|c| *c != ',').collect();
    if clean.is_empty() {
        return None;
    }
    clean.parse::<f64>().ok().map(|n| if pct { n / 100.0 } else { n })
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Word(String),
    Op(&'static str),
    Open,
    Close,
    Comma,
    Colon,
    Bang,
}

fn lex(s: &str) -> Option<Vec<Tok>> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let ch = b[i];
        match ch {
            b' ' | b'\t' => i += 1,
            b'(' => {
                out.push(Tok::Open);
                i += 1;
            }
            b')' => {
                out.push(Tok::Close);
                i += 1;
            }
            b',' | b';' => {
                out.push(Tok::Comma);
                i += 1;
            }
            b':' => {
                out.push(Tok::Colon);
                i += 1;
            }
            b'!' => {
                out.push(Tok::Bang);
                i += 1;
            }
            b'"' => {
                let end = s[i + 1..].find('"')? + i + 1;
                out.push(Tok::Str(s[i + 1..end].to_string()));
                i = end + 1;
            }
            b'0'..=b'9' | b'.' => {
                let mut j = i;
                while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
                    j += 1;
                }
                out.push(Tok::Num(s[i..j].parse().ok()?));
                i = j;
            }
            b'<' | b'>' => {
                let two = b.get(i + 1).copied();
                let op = match (ch, two) {
                    (b'<', Some(b'=')) => "<=",
                    (b'>', Some(b'=')) => ">=",
                    (b'<', Some(b'>')) => "<>",
                    (b'<', _) => "<",
                    _ => ">",
                };
                i += op.len();
                out.push(Tok::Op(op));
            }
            b'+' | b'-' | b'*' | b'/' | b'^' | b'&' | b'=' | b'%' => {
                out.push(Tok::Op(match ch {
                    b'+' => "+",
                    b'-' => "-",
                    b'*' => "*",
                    b'/' => "/",
                    b'^' => "^",
                    b'&' => "&",
                    b'=' => "=",
                    _ => "%",
                }));
                i += 1;
            }
            c if c.is_ascii_alphabetic() || c == b'$' || c == b'_' => {
                let mut j = i;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'$' || b[j] == b'_' || b[j] == b'.') {
                    j += 1;
                }
                out.push(Tok::Word(s[i..j].to_string()));
                i = j;
            }
            _ => return None,
        }
    }
    Some(out)
}

#[derive(Clone, Debug)]
enum Ex {
    Num(f64),
    Str(String),
    Bool(bool),
    /// A sheet (by name; none: this one) and a cell.
    Ref(Option<String>, (u16, u16)),
    Range(Option<String>, (u16, u16), (u16, u16)),
    Neg(Box<Self>),
    Pct(Box<Self>),
    Bin(&'static str, Box<Self>, Box<Self>),
    Call(String, Vec<Self>),
    Bad(&'static str),
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }

    fn op(&mut self, ops: &[&str]) -> Option<&'static str> {
        match self.peek() {
            Some(Tok::Op(o)) if ops.contains(o) => {
                let o = *o;
                self.i += 1;
                Some(o)
            }
            _ => None,
        }
    }

    fn level(&mut self, ops: &[&str], next: fn(&mut Self) -> Ex) -> Ex {
        let mut a = next(self);
        while let Some(o) = self.op(ops) {
            let b = next(self);
            a = Ex::Bin(o, Box::new(a), Box::new(b));
        }
        a
    }

    fn compare(&mut self) -> Ex {
        self.level(&["=", "<>", "<", ">", "<=", ">="], Self::concat)
    }

    fn concat(&mut self) -> Ex {
        self.level(&["&"], Self::add)
    }

    fn add(&mut self) -> Ex {
        self.level(&["+", "-"], Self::mul)
    }

    fn mul(&mut self) -> Ex {
        self.level(&["*", "/"], Self::pow)
    }

    fn pow(&mut self) -> Ex {
        self.level(&["^"], Self::unary)
    }

    fn unary(&mut self) -> Ex {
        if self.op(&["-"]).is_some() {
            return Ex::Neg(Box::new(self.unary()));
        }
        if self.op(&["+"]).is_some() {
            return self.unary();
        }
        let mut e = self.primary();
        while self.op(&["%"]).is_some() {
            e = Ex::Pct(Box::new(e));
        }
        e
    }

    fn reference(&mut self, sheet: Option<String>, w: &str) -> Ex {
        let Some((r, c, ..)) = parse_ref(w) else { return Ex::Bad("#REF!") };
        if self.peek() == Some(&Tok::Colon) {
            self.i += 1;
            let Some(Tok::Word(w2)) = self.next() else { return Ex::Bad("#REF!") };
            let Some((r2, c2, ..)) = parse_ref(&w2) else { return Ex::Bad("#REF!") };
            return Ex::Range(sheet, (r.min(r2), c.min(c2)), (r.max(r2), c.max(c2)));
        }
        Ex::Ref(sheet, (r, c))
    }

    fn primary(&mut self) -> Ex {
        match self.next() {
            Some(Tok::Num(n)) => Ex::Num(n),
            Some(Tok::Str(s)) => Ex::Str(s),
            Some(Tok::Open) => {
                let e = self.compare();
                if self.next() == Some(Tok::Close) { e } else { Ex::Bad("#ERROR!") }
            }
            Some(Tok::Word(w)) => {
                if self.peek() == Some(&Tok::Bang) {
                    self.i += 1;
                    let Some(Tok::Word(cell)) = self.next() else { return Ex::Bad("#REF!") };
                    return self.reference(Some(w), &cell);
                }
                if self.peek() == Some(&Tok::Open) {
                    self.i += 1;
                    let mut args = Vec::new();
                    if self.peek() == Some(&Tok::Close) {
                        self.i += 1;
                        return Ex::Call(w.to_ascii_uppercase(), args);
                    }
                    loop {
                        args.push(self.compare());
                        match self.next() {
                            Some(Tok::Comma) => {}
                            Some(Tok::Close) => break,
                            _ => return Ex::Bad("#ERROR!"),
                        }
                    }
                    return Ex::Call(w.to_ascii_uppercase(), args);
                }
                match w.to_ascii_uppercase().as_str() {
                    "TRUE" => Ex::Bool(true),
                    "FALSE" => Ex::Bool(false),
                    _ if parse_ref(&w).is_some() => self.reference(None, &w),
                    _ => Ex::Bad("#NAME?"),
                }
            }
            _ => Ex::Bad("#ERROR!"),
        }
    }
}

fn parse_formula(src: &str) -> Ex {
    let Some(t) = lex(src) else { return Ex::Bad("#ERROR!") };
    let mut p = Parser { t, i: 0 };
    let e = p.compare();
    if p.i < p.t.len() { Ex::Bad("#ERROR!") } else { e }
}

/// Every cell's value, per sheet.
#[derive(Default)]
struct Vals {
    sheets: Vec<HashMap<(u16, u16), Val>>,
}

impl Vals {
    fn get(&self, s: usize, at: (u16, u16)) -> &Val {
        const EMPTY: &Val = &Val::Empty;
        self.sheets.get(s).and_then(|m| m.get(&at)).unwrap_or(EMPTY)
    }
}

struct Calc<'a> {
    book: &'a Book,
    memo: Vec<HashMap<(u16, u16), Val>>,
    busy: HashSet<(usize, u16, u16)>,
}

impl Calc<'_> {
    fn cell(&mut self, s: usize, at: (u16, u16)) -> Val {
        if let Some(v) = self.memo[s].get(&at) {
            return v.clone();
        }
        if !self.busy.insert((s, at.0, at.1)) {
            return Val::Err("#CYCLE!");
        }
        let raw = self.book.sheets[s].raw(at);
        let v = if let Some(f) = raw.strip_prefix('=') {
            let e = parse_formula(f);
            match self.eval(s, &e) {
                // a formula that is a range alone shows its first cell
                Val::Empty => Val::Num(0.0),
                v => v,
            }
        } else if raw.trim().is_empty() {
            Val::Empty
        } else if let Some(n) = literal_num(raw) {
            Val::Num(n)
        } else if raw.eq_ignore_ascii_case("true") || raw.eq_ignore_ascii_case("false") {
            Val::Bool(raw.eq_ignore_ascii_case("true"))
        } else {
            Val::Text(raw.to_string())
        };
        self.busy.remove(&(s, at.0, at.1));
        self.memo[s].insert(at, v.clone());
        v
    }

    fn sheet(&self, here: usize, name: Option<&String>) -> Option<usize> {
        name.map_or(Some(here), |n| self.book.sheet_index(n))
    }

    /// The values a function argument stands for (a range: all its cells).
    fn flat(&mut self, s: usize, e: &Ex, out: &mut Vec<Val>) {
        if let Ex::Range(name, a, b) = e {
            let Some(t) = self.sheet(s, name.as_ref()) else {
                out.push(Val::Err("#REF!"));
                return;
            };
            for r in a.0..=b.0 {
                for c in a.1..=b.1 {
                    out.push(self.cell(t, (r, c)));
                }
            }
        } else {
            out.push(self.eval(s, e));
        }
    }

    fn eval(&mut self, s: usize, e: &Ex) -> Val {
        match e {
            Ex::Num(n) => Val::Num(*n),
            Ex::Str(t) => Val::Text(t.clone()),
            Ex::Bool(b) => Val::Bool(*b),
            Ex::Bad(err) => Val::Err(err),
            Ex::Ref(name, at) => match self.sheet(s, name.as_ref()) {
                Some(t) => self.cell(t, *at),
                None => Val::Err("#REF!"),
            },
            Ex::Range(name, a, _) => match self.sheet(s, name.as_ref()) {
                Some(t) => self.cell(t, *a),
                None => Val::Err("#REF!"),
            },
            Ex::Neg(a) => match self.eval(s, a).num() {
                Ok(n) => Val::Num(-n),
                Err(e) => Val::Err(e),
            },
            Ex::Pct(a) => match self.eval(s, a).num() {
                Ok(n) => Val::Num(n / 100.0),
                Err(e) => Val::Err(e),
            },
            Ex::Bin(op, a, b) => {
                let (a, b) = (self.eval(s, a), self.eval(s, b));
                binary(op, &a, &b)
            }
            Ex::Call(name, args) => self.call(s, name, args),
        }
    }

    fn call(&mut self, s: usize, name: &str, args: &[Ex]) -> Val {
        let mut vals = Vec::new();
        let numbers = |vals: &[Val]| -> Result<Vec<f64>, &'static str> {
            let mut out = Vec::new();
            for v in vals {
                match v {
                    Val::Num(n) => out.push(*n),
                    Val::Bool(b) => out.push(f64::from(u8::from(*b))),
                    Val::Err(e) => return Err(e),
                    Val::Empty | Val::Text(_) => {}
                }
            }
            Ok(out)
        };
        let num_arg = |this: &mut Self, i: usize| -> Result<f64, &'static str> {
            args.get(i).map_or(Ok(0.0), |a| this.eval(s, a).num())
        };
        let result: Result<Val, &'static str> = match name {
            "SUM" | "AVERAGE" | "MIN" | "MAX" | "COUNT" => {
                for a in args {
                    self.flat(s, a, &mut vals);
                }
                numbers(&vals).and_then(|ns| match name {
                    "SUM" => Ok(Val::Num(ns.iter().sum())),
                    "COUNT" => Ok(Val::Num(ns.len() as f64)),
                    "AVERAGE" if ns.is_empty() => Err("#DIV/0!"),
                    "AVERAGE" => Ok(Val::Num(ns.iter().sum::<f64>() / ns.len() as f64)),
                    "MIN" => Ok(Val::Num(ns.iter().copied().fold(f64::INFINITY, f64::min).min(if ns.is_empty() { 0.0 } else { f64::INFINITY }))),
                    _ => Ok(Val::Num(ns.iter().copied().fold(f64::NEG_INFINITY, f64::max).max(if ns.is_empty() { 0.0 } else { f64::NEG_INFINITY }))),
                })
            }
            "AND" | "OR" => {
                for a in args {
                    self.flat(s, a, &mut vals);
                }
                let mut acc = name == "AND";
                let mut err = None;
                for v in &vals {
                    match v.truthy() {
                        Ok(b) if name == "AND" => acc &= b,
                        Ok(b) => acc |= b,
                        Err(e) => err = Some(e),
                    }
                }
                err.map_or(Ok(Val::Bool(acc)), Err)
            }
            "IF" => match args.first().map(|c| self.eval(s, c).truthy()) {
                Some(Ok(true)) => Ok(args.get(1).map_or(Val::Bool(true), |a| self.eval(s, a))),
                Some(Ok(false)) => Ok(args.get(2).map_or(Val::Bool(false), |a| self.eval(s, a))),
                Some(Err(e)) => Err(e),
                None => Err("#VALUE!"),
            },
            "ROUND" => num_arg(self, 0).and_then(|x| {
                let d = num_arg(self, 1)?;
                let k = 10f64.powi(d as i32);
                Ok(Val::Num((x * k).round() / k))
            }),
            "ABS" => num_arg(self, 0).map(|x| Val::Num(x.abs())),
            "SQRT" => num_arg(self, 0).and_then(|x| if x < 0.0 { Err("#NUM!") } else { Ok(Val::Num(x.sqrt())) }),
            "PI" => Ok(Val::Num(std::f64::consts::PI)),
            _ => Err("#NAME?"),
        };
        result.unwrap_or_else(Val::Err)
    }
}

fn binary(op: &str, a: &Val, b: &Val) -> Val {
    use std::cmp::Ordering::{Equal, Greater, Less};
    if let Val::Err(e) = a {
        return Val::Err(e);
    }
    if let Val::Err(e) = b {
        return Val::Err(e);
    }
    if op == "&" {
        return Val::Text(a.text() + &b.text());
    }
    if matches!(op, "=" | "<>" | "<" | ">" | "<=" | ">=") {
        let ord = match (a.num(), b.num()) {
            (Ok(x), Ok(y)) if !matches!((a, b), (Val::Text(_), _) | (_, Val::Text(_))) => x.partial_cmp(&y),
            _ => Some(a.text().to_lowercase().cmp(&b.text().to_lowercase())),
        };
        let Some(o) = ord else { return Val::Err("#VALUE!") };
        return Val::Bool(match op {
            "=" => o == Equal,
            "<>" => o != Equal,
            "<" => o == Less,
            ">" => o == Greater,
            "<=" => o != Greater,
            _ => o != Less,
        });
    }
    let (x, y) = match (a.num(), b.num()) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(e), _) | (_, Err(e)) => return Val::Err(e),
    };
    match op {
        "+" => Val::Num(x + y),
        "-" => Val::Num(x - y),
        "*" => Val::Num(x * y),
        "/" if y == 0.0 => Val::Err("#DIV/0!"),
        "/" => Val::Num(x / y),
        _ => Val::Num(x.powf(y)),
    }
}

/// Every cell of every sheet, evaluated.
fn compute(book: &Book) -> Vals {
    let mut calc = Calc { book, memo: vec![HashMap::new(); book.sheets.len()], busy: HashSet::new() };
    for (s, sheet) in book.sheets.iter().enumerate() {
        for &at in sheet.cells.keys() {
            calc.cell(s, at);
        }
    }
    Vals { sheets: calc.memo }
}

/// A formula moved by (`dr`, `dc`): relative refs follow, `$` ones stay (as a copy or a fill does).
fn shift_refs(raw: &str, dr: i32, dc: i32) -> String {
    let Some(body) = raw.strip_prefix('=') else { return raw.to_string() };
    let b = body.as_bytes();
    let mut out = String::from("=");
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let end = body[i + 1..].find('"').map_or(b.len(), |e| e + i + 2);
            out.push_str(&body[i..end]);
            i = end;
            continue;
        }
        let starts = (b[i].is_ascii_alphabetic() || b[i] == b'$') && (i == 0 || !b[i - 1].is_ascii_alphanumeric());
        if !starts {
            out.push(b[i] as char);
            i += 1;
            continue;
        }
        let mut j = i;
        while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'$') {
            j += 1;
        }
        let word = &body[i..j];
        let call = b.get(j) == Some(&b'(') || b.get(j) == Some(&b'!');
        match parse_ref(word).filter(|_| !call) {
            Some((r, c, ra, ca)) => {
                let nr = if ra { i32::from(r) } else { i32::from(r) + dr };
                let nc = if ca { i32::from(c) } else { i32::from(c) + dc };
                if nr < 0 || nc < 0 || nr >= i32::from(ROWS) || nc >= i32::from(COLS) {
                    out.push_str("#REF!");
                } else {
                    let _ = write!(out, "{}{}{}{}", if ca { "$" } else { "" }, col_name(nc as u16), if ra { "$" } else { "" }, nr + 1);
                }
            }
            None => out.push_str(word),
        }
        i = j;
    }
    out
}

// ------------------------------------------------------------------------------------------------
// Showing values

/// A number as General shows it: an integer as is, else up to ten significant digits.
fn general(n: f64) -> String {
    if !n.is_finite() {
        return "#NUM!".into();
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return format!("{n:.0}");
    }
    let s = format!("{n:.10}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `1234567.891` with `d` decimals and thousands separators.
fn grouped(n: f64, d: u8) -> String {
    let s = format!("{:.*}", d as usize, n.abs());
    let (int, frac) = s.split_once('.').map_or((s.as_str(), ""), |(a, b)| (a, b));
    let mut out = String::with_capacity(s.len() + 4);
    if n < 0.0 {
        out.push('-');
    }
    for (k, ch) in int.chars().enumerate() {
        if k > 0 && (int.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    out
}

fn show(v: &Val, f: Fmt) -> String {
    match (v, f.num) {
        (Val::Num(n), NumFmt::General) => general(*n),
        (Val::Num(n), NumFmt::Number) => grouped(*n, f.decimals),
        (Val::Num(n), NumFmt::Currency) => {
            let g = grouped(n.abs(), f.decimals);
            if *n < 0.0 { format!("-${g}") } else { format!("${g}") }
        }
        (Val::Num(n), NumFmt::Percent) => format!("{}%", grouped(n * 100.0, f.decimals)),
        (v, _) => v.text(),
    }
}

// ------------------------------------------------------------------------------------------------
// The app

#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag {
    Select,
    Fill((u16, u16)),
    Resize { col: u16, x0: f32, w0: f32 },
}

#[derive(Clone, Copy, PartialEq)]
struct Chart {
    sheet: usize,
    a: (u16, u16),
    b: (u16, u16),
    kind: usize,
    width: f64,
    colors: [u32; 3],
}

const CHART_KINDS: [&str; 4] = ["Bars", "Lines", "Area", "Pie"];
const PALETTE: [u32; 6] = [0x4772b3, 0xe0913a, 0x5aa85a, 0xc9567a, 0x8b6bd1, 0x3fb5b0];

/// What the view shows besides the workbook.
#[derive(Clone)]
struct Ui {
    sheet: usize,
    anchor: (u16, u16),
    cur: (u16, u16),
    top: u16,
    left: u16,
    /// Typing into the active cell (F2 or a key); `keep` keeps arrows in the text (F2).
    draft: Option<String>,
    drag: Option<Drag>,
    chart: Chart,
    sidebar: bool,
    try_src: String,
    status: String,
    /// Bumped by each paste, fill and sort: keys the flash over what changed.
    flash: u32,
    flash_rect: Option<((u16, u16), (u16, u16))>,
}

struct Clip {
    from: (u16, u16),
    cells: Vec<Vec<Cell>>,
}

struct SheetApp {
    book: Arc<Book>,
    vals: Arc<Vals>,
    ui: Ui,
    undo: Vec<Arc<Book>>,
    redo: Vec<Arc<Book>>,
    clip: Option<Clip>,
    frames: u32,
    /// The grid's size on screen (as the last frame placed it): what scrolling keeps the cursor in.
    view: (f32, f32),
}

/// What an editor factory builds from: the state when the view rendered.
struct Snap {
    book: Arc<Book>,
    vals: Arc<Vals>,
    ui: Ui,
    weak: WeakEntity<SheetApp>,
    can_undo: bool,
    can_redo: bool,
}

impl SheetApp {
    fn new() -> Self {
        let book = Arc::new(seed());
        let vals = Arc::new(compute(&book));
        let chart = Chart { sheet: 0, a: (1, 0), b: (6, 6), kind: 0, width: 0.72, colors: [PALETTE[0], PALETTE[1], PALETTE[2]] };
        Self {
            book,
            vals,
            ui: Ui { sheet: 0, anchor: (2, 1), cur: (2, 1), top: 0, left: 0, draft: None, drag: None, chart, sidebar: true,
                     try_src: "=SUM(Budget!H3:H8)/6".into(), status: "Ready".into(), flash: 0, flash_rect: None },
            undo: Vec::new(),
            redo: Vec::new(),
            clip: None,
            frames: 0,
            view: VIEW_GUESS,
        }
    }

    fn sheet(&self) -> &Sheet {
        &self.book.sheets[self.ui.sheet]
    }

    /// Every edit goes through here: the old book is kept for undo, the values are evaluated again.
    fn change(&mut self, f: impl FnOnce(&mut Book, usize)) {
        self.undo.push(self.book.clone());
        if self.undo.len() > UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        let s = self.ui.sheet;
        f(Arc::make_mut(&mut self.book), s);
        self.vals = Arc::new(compute(&self.book));
    }

    fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(&mut self.book, prev));
        self.after_history("Undo");
        true
    }

    fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(&mut self.book, next));
        self.after_history("Redo");
        true
    }

    fn after_history(&mut self, what: &str) {
        self.ui.sheet = self.ui.sheet.min(self.book.sheets.len() - 1);
        self.vals = Arc::new(compute(&self.book));
        self.ui.draft = None;
        self.ui.status = what.to_string();
    }

    /// The selection: top-left and bottom-right.
    fn sel(&self) -> ((u16, u16), (u16, u16)) {
        sel_of(&self.ui)
    }

    fn select(&mut self, at: (u16, u16), extend: bool) {
        self.commit();
        self.ui.cur = (at.0.min(ROWS - 1), at.1.min(COLS - 1));
        if !extend {
            self.ui.anchor = self.ui.cur;
        }
        self.reveal();
    }

    /// Rows that fit in the grid below its header.
    fn rows_shown(&self) -> u16 {
        (((self.view.1 - ROW_H) / ROW_H).floor() as u16).max(1)
    }

    /// Scrolls so the cursor shows.
    fn reveal(&mut self) {
        let (r, c) = self.ui.cur;
        let rows = self.rows_shown();
        if r < self.ui.top {
            self.ui.top = r;
        } else if r >= self.ui.top + rows {
            self.ui.top = r + 1 - rows;
        }
        let keep_w = (self.view.0 - HEAD_W).max(COL_W);
        if c < self.ui.left {
            self.ui.left = c;
        } else {
            let mut left = self.ui.left;
            let w = &self.book.sheets[self.ui.sheet].widths;
            while left < c && w[left as usize..=c as usize].iter().sum::<f32>() > keep_w {
                left += 1;
            }
            self.ui.left = left;
        }
    }

    fn step(&mut self, dr: i32, dc: i32, extend: bool) {
        let (r, c) = self.ui.cur;
        let r = (i32::from(r) + dr).clamp(0, i32::from(ROWS) - 1) as u16;
        let c = (i32::from(c) + dc).clamp(0, i32::from(COLS) - 1) as u16;
        self.select((r, c), extend);
    }

    /// The draft goes into the active cell.
    fn commit(&mut self) {
        if let Some(d) = self.ui.draft.take() {
            let at = self.ui.cur;
            if self.sheet().raw(at) != d {
                self.change(|b, s| b.sheets[s].set_raw(at, d));
                self.ui.status = format!("{} set", addr(at));
            }
        }
    }

    fn set_cell_raw(&mut self, at: (u16, u16), raw: String) {
        if self.sheet().raw(at) != raw {
            self.change(|b, s| b.sheets[s].set_raw(at, raw));
        }
    }

    /// Changes the format of every cell selected.
    fn format(&mut self, what: &str, f: impl Fn(&mut Fmt)) {
        let (a, b) = self.sel();
        self.change(|book, s| {
            let sh = &mut book.sheets[s];
            for r in a.0..=b.0 {
                for c in a.1..=b.1 {
                    let mut cell = sh.cells.get(&(r, c)).cloned().unwrap_or_default();
                    f(&mut cell.fmt);
                    sh.set_cell((r, c), cell);
                }
            }
        });
        self.ui.status = what.to_string();
    }

    fn clear(&mut self) {
        let (a, b) = self.sel();
        self.change(|book, s| {
            for r in a.0..=b.0 {
                for c in a.1..=b.1 {
                    book.sheets[s].set_raw((r, c), String::new());
                }
            }
        });
        self.ui.status = "Cleared".into();
    }

    fn copy(&mut self, cut: bool) {
        let (a, b) = self.sel();
        let sh = self.sheet();
        let cells = (a.0..=b.0).map(|r| (a.1..=b.1).map(|c| sh.cells.get(&(r, c)).cloned().unwrap_or_default()).collect()).collect();
        self.clip = Some(Clip { from: a, cells });
        let n = (b.0 - a.0 + 1) * (b.1 - a.1 + 1);
        if cut {
            self.clear();
        }
        self.ui.status = format!("{} {n} cells", if cut { "Cut" } else { "Copied" });
    }

    fn paste(&mut self) {
        let Some(clip) = self.clip.as_ref() else { return };
        let to = self.ui.cur;
        let (dr, dc) = (i32::from(to.0) - i32::from(clip.from.0), i32::from(to.1) - i32::from(clip.from.1));
        let mut put = Vec::new();
        for (i, row) in clip.cells.iter().enumerate() {
            for (j, cell) in row.iter().enumerate() {
                let at = (to.0 + i as u16, to.1 + j as u16);
                if at.0 < ROWS && at.1 < COLS {
                    put.push((at, Cell { raw: shift_refs(&cell.raw, dr, dc), fmt: cell.fmt }));
                }
            }
        }
        let end = put.last().map_or(to, |p| p.0);
        self.change(|b, s| put.into_iter().for_each(|(at, cell)| b.sheets[s].set_cell(at, cell)));
        self.ui.anchor = to;
        self.ui.cur = end;
        self.flash((to, end), "Pasted");
    }

    fn flash(&mut self, rect: ((u16, u16), (u16, u16)), what: &str) {
        self.ui.flash += 1;
        self.ui.flash_rect = Some(rect);
        self.ui.status = what.to_string();
    }

    /// The fill handle let go at `to`: down or right from the selection, a series where its
    /// numbers step evenly, else copies (formulas moved).
    fn fill(&mut self, to: (u16, u16)) {
        let (a, b) = self.sel();
        let down = to.0 > b.0 && (to.0 - b.0) >= to.1.saturating_sub(b.1);
        let right = !down && to.1 > b.1;
        if !down && !right {
            return;
        }
        let sh = self.sheet();
        let vals = &self.vals;
        let s = self.ui.sheet;
        let mut put = Vec::new();
        // each line along the fill: a column (down) or a row (right)
        let lines: Vec<u16> = if down { (a.1..=b.1).collect() } else { (a.0..=b.0).collect() };
        for line in lines {
            let src: Vec<(u16, u16)> = if down { (a.0..=b.0).map(|r| (r, line)).collect() } else { (a.1..=b.1).map(|c| (line, c)).collect() };
            let nums: Vec<f64> = src.iter().filter_map(|&at| {
                let raw = sh.raw(at);
                (!raw.starts_with('=')).then(|| literal_num(raw)).flatten()
            }).collect();
            let series = nums.len() == src.len() && nums.len() >= 2;
            let stepv = if series { nums[nums.len() - 1] - nums[nums.len() - 2] } else { 0.0 };
            let span = if down { b.0 + 1..=to.0 } else { b.1 + 1..=to.1 };
            for (k, pos) in span.enumerate() {
                let at = if down { (pos, line) } else { (line, pos) };
                let from = src[k % src.len()];
                let fmt = sh.fmt(from);
                let raw = if series {
                    general(nums[nums.len() - 1] + stepv * (k as f64 + 1.0))
                } else {
                    let (dr, dc) = (i32::from(at.0) - i32::from(from.0), i32::from(at.1) - i32::from(from.1));
                    shift_refs(sh.raw(from), dr, dc)
                };
                put.push((at, Cell { raw, fmt }));
            }
            let _ = vals.get(s, src[0]);
        }
        self.change(|book, s| put.into_iter().for_each(|(at, cell)| book.sheets[s].set_cell(at, cell)));
        let end = if down { (to.0, b.1) } else { (b.0, to.1) };
        self.ui.anchor = a;
        self.ui.cur = end;
        self.flash((a, end), "Filled");
    }

    /// The selected rows, sorted by the active cell's column.
    fn sort(&mut self, desc: bool) {
        let (a, b) = self.sel();
        if a.0 == b.0 {
            self.ui.status = "Select rows to sort".into();
            return;
        }
        let key_col = self.ui.cur.1;
        let s = self.ui.sheet;
        let sh = self.sheet();
        let mut rows: Vec<(Val, Vec<Cell>)> = (a.0..=b.0).map(|r| {
            (self.vals.get(s, (r, key_col)).clone(), (a.1..=b.1).map(|c| sh.cells.get(&(r, c)).cloned().unwrap_or_default()).collect())
        }).collect();
        rows.sort_by(|x, y| {
            let o = match (&x.0, &y.0) {
                (Val::Num(p), Val::Num(q)) => p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal),
                (Val::Num(_), _) => std::cmp::Ordering::Less,
                (_, Val::Num(_)) => std::cmp::Ordering::Greater,
                (p, q) => p.text().to_lowercase().cmp(&q.text().to_lowercase()),
            };
            if desc { o.reverse() } else { o }
        });
        self.change(|book, s| {
            for (i, (_, cells)) in rows.into_iter().enumerate() {
                for (j, cell) in cells.into_iter().enumerate() {
                    book.sheets[s].set_cell((a.0 + i as u16, a.1 + j as u16), cell);
                }
            }
        });
        self.flash((a, b), if desc { "Sorted Z to A" } else { "Sorted A to Z" });
    }

    fn add_sheet(&mut self) {
        let n = self.book.sheets.len() + 1;
        self.change(|b, _| b.sheets.push(Sheet::new(&format!("Sheet{n}"))));
        self.go_sheet(self.book.sheets.len() - 1);
    }

    fn go_sheet(&mut self, i: usize) {
        self.commit();
        self.ui.sheet = i.min(self.book.sheets.len() - 1);
        self.ui.anchor = (0, 0);
        self.ui.cur = (0, 0);
        self.ui.top = 0;
        self.ui.left = 0;
        self.ui.status = format!("Sheet {}", self.sheet().name);
    }

    fn key(&mut self, e: &KeyDownEvent) -> bool {
        let k = &e.keystroke;
        let (ctrl, shift) = (k.modifiers.control, k.modifiers.shift);
        if let Some(draft) = self.ui.draft.as_mut() {
            match k.key.as_str() {
                "Tab" => {
                    self.commit();
                    self.step(0, if shift { -1 } else { 1 }, false);
                }
                "Up" | "Down" => {
                    self.commit();
                    self.step(if k.key == "Up" { -1 } else { 1 }, 0, false);
                }
                _ => match edit(draft, k, 400) {
                    Edit::Submit => {
                        self.commit();
                        self.step(if shift { -1 } else { 1 }, 0, false);
                    }
                    Edit::Leave => {
                        self.ui.draft = None;
                        self.ui.status = "Edit cancelled".into();
                    }
                    Edit::Typed => {}
                    Edit::Ignored => return false,
                },
            }
            return true;
        }
        let page = i32::from(self.rows_shown()).max(2) - 1;
        match (k.key.as_str(), ctrl) {
            ("z" | "Z", true) => return self.undo(),
            ("y" | "Y", true) => return self.redo(),
            ("c" | "C", true) => self.copy(false),
            ("x" | "X", true) => self.copy(true),
            ("v" | "V", true) => self.paste(),
            ("b" | "B", true) => {
                let bold = !self.sheet().fmt(self.ui.cur).bold;
                self.format(if bold { "Bold" } else { "Not bold" }, |f| f.bold = bold);
            }
            ("a" | "A", true) => {
                self.ui.anchor = (0, 0);
                self.ui.cur = (ROWS - 1, COLS - 1);
            }
            ("Home", true) => self.select((0, 0), shift),
            ("Up", _) => self.step(-1, 0, shift),
            ("Down", _) => self.step(1, 0, shift),
            ("Left", _) => self.step(0, -1, shift),
            ("Right", _) => self.step(0, 1, shift),
            ("Page_Up" | "Prior", _) => self.step(-page, 0, shift),
            ("Page_Down" | "Next", _) => self.step(page, 0, shift),
            ("Home", _) => self.select((self.ui.cur.0, 0), shift),
            ("Return" | "KP_Enter", _) => self.step(if shift { -1 } else { 1 }, 0, false),
            ("Tab", _) => self.step(0, if shift { -1 } else { 1 }, false),
            ("F2", _) => self.ui.draft = Some(self.sheet().raw(self.ui.cur).to_string()),
            ("Delete" | "BackSpace", _) => self.clear(),
            _ => match &k.key_char {
                // typing replaces the cell
                Some(ch) if !ctrl && !ch.chars().any(char::is_control) => {
                    self.ui.anchor = self.ui.cur;
                    self.ui.draft = Some(ch.clone());
                }
                _ => return false,
            },
        }
        true
    }

    /// A press on a cell: it is on screen already, so nothing scrolls under the pointer.
    fn press(&mut self, at: (u16, u16), shift: bool) {
        self.commit();
        self.ui.cur = at;
        if !shift {
            self.ui.anchor = at;
        }
        self.ui.drag = Some(Drag::Select);
    }

    fn enter(&mut self, at: (u16, u16)) -> bool {
        match self.ui.drag {
            Some(Drag::Select) if self.ui.cur != at => {
                self.ui.cur = at;
                true
            }
            Some(Drag::Fill(t)) if t != at => {
                self.ui.drag = Some(Drag::Fill(at));
                true
            }
            _ => false,
        }
    }

    fn release(&mut self) -> bool {
        match self.ui.drag.take() {
            Some(Drag::Fill(to)) => {
                self.fill(to);
                true
            }
            Some(_) => true,
            None => false,
        }
    }
}

fn sel_of(ui: &Ui) -> ((u16, u16), (u16, u16)) {
    let (a, b) = (ui.anchor, ui.cur);
    ((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1)))
}

fn seed() -> Book {
    let mut budget = Sheet::new("Budget");
    let put = |sh: &mut Sheet, at: &str, raw: &str, f: Fmt| {
        let (r, c, ..) = parse_ref(at).expect("seed ref");
        sh.set_cell((r, c), Cell { raw: raw.to_string(), fmt: f });
    };
    let head = Fmt { bold: true, fill: Some(0x3b4a63), ..Fmt::default() };
    let money = Fmt { num: NumFmt::Currency, decimals: 0, ..Fmt::default() };
    let title = Fmt { bold: true, color: Some(0x8cb4ff), ..Fmt::default() };
    put(&mut budget, "A1", "Little Moth Studio · 2026 budget", title);
    let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];
    put(&mut budget, "A2", "Category", head);
    for (i, m) in months.iter().enumerate() {
        put(&mut budget, &format!("{}2", col_name(1 + i as u16)), m, Fmt { align: Align::Right, ..head });
    }
    put(&mut budget, "H2", "Total", Fmt { align: Align::Right, ..head });
    put(&mut budget, "I2", "Share", Fmt { align: Align::Right, ..head });
    let rows: [(&str, [u32; 6]); 6] = [
        ("Render farm", [1800, 2100, 2650, 2400, 3100, 3300]),
        ("Software", [640, 640, 640, 720, 720, 720]),
        ("Workstations", [0, 4200, 0, 0, 3900, 0]),
        ("Freelancers", [3200, 2800, 4100, 3600, 4400, 5200]),
        ("Asset store", [180, 260, 90, 310, 220, 150]),
        ("Coffee", [95, 110, 120, 105, 130, 140]),
    ];
    for (i, (name, v)) in rows.iter().enumerate() {
        let r = 3 + i;
        put(&mut budget, &format!("A{r}"), name, Fmt::default());
        for (j, x) in v.iter().enumerate() {
            put(&mut budget, &format!("{}{r}", col_name(1 + j as u16)), &x.to_string(), money);
        }
        put(&mut budget, &format!("H{r}"), &format!("=SUM(B{r}:G{r})"), Fmt { bold: true, ..money });
        put(&mut budget, &format!("I{r}"), &format!("=H{r}/$H$9"), Fmt { num: NumFmt::Percent, decimals: 1, ..Fmt::default() });
    }
    put(&mut budget, "A9", "Total", head);
    for j in 0..7u16 {
        let c = col_name(1 + j);
        put(&mut budget, &format!("{c}9"), &format!("=SUM({c}3:{c}8)"), Fmt { bold: true, fill: Some(0x3b4a63), ..money });
    }
    put(&mut budget, "I9", "=SUM(I3:I8)", Fmt { num: NumFmt::Percent, decimals: 0, ..head });
    put(&mut budget, "A11", "Average month", Fmt::default());
    put(&mut budget, "B11", "=ROUND(AVERAGE(B9:G9), 0)", money);
    put(&mut budget, "A12", "Busiest month", Fmt::default());
    put(&mut budget, "B12", "=MAX(B9:G9)", money);
    put(&mut budget, "A13", "Under 40k?", Fmt::default());
    put(&mut budget, "B13", "=IF(H9<40000, \"on track\", \"over budget\")", Fmt::default());
    put(&mut budget, "A14", "Render share", Fmt::default());
    put(&mut budget, "B14", "=H3/H9", Fmt { num: NumFmt::Percent, decimals: 1, ..Fmt::default() });
    budget.widths[0] = 150.0;

    let mut sales = Sheet::new("Sales");
    put(&mut sales, "A1", "Course sales by region", title);
    for (i, h) in ["Region", "Q1", "Q2", "Q3", "Q4", "Year", "Growth"].iter().enumerate() {
        put(&mut sales, &format!("{}2", col_name(i as u16)), h, head);
    }
    let regions: [(&str, [u32; 4]); 5] = [
        ("North", [12_400, 13_900, 15_100, 17_800]),
        ("South", [9_800, 9_100, 10_600, 12_200]),
        ("East", [14_300, 15_700, 14_900, 18_300]),
        ("West", [7_600, 8_800, 11_200, 12_900]),
        ("Online", [21_000, 24_600, 27_300, 33_100]),
    ];
    for (i, (name, q)) in regions.iter().enumerate() {
        let r = 3 + i;
        put(&mut sales, &format!("A{r}"), name, Fmt::default());
        for (j, x) in q.iter().enumerate() {
            put(&mut sales, &format!("{}{r}", col_name(1 + j as u16)), &x.to_string(), money);
        }
        put(&mut sales, &format!("F{r}"), &format!("=SUM(B{r}:E{r})"), Fmt { bold: true, ..money });
        put(&mut sales, &format!("G{r}"), &format!("=E{r}/B{r}-1"), Fmt { num: NumFmt::Percent, decimals: 1, ..Fmt::default() });
    }
    put(&mut sales, "A8", "All", head);
    for c in ['B', 'C', 'D', 'E', 'F'] {
        put(&mut sales, &format!("{c}8"), &format!("=SUM({c}3:{c}7)"), Fmt { bold: true, fill: Some(0x3b4a63), ..money });
    }
    sales.widths[0] = 110.0;

    let mut summary = Sheet::new("Summary");
    put(&mut summary, "A1", "Across sheets", title);
    put(&mut summary, "A3", "Spent (Budget)", Fmt::default());
    put(&mut summary, "B3", "=Budget!H9", money);
    put(&mut summary, "A4", "Earned (Sales)", Fmt::default());
    put(&mut summary, "B4", "=Sales!F8", money);
    put(&mut summary, "A5", "Margin", head);
    put(&mut summary, "B5", "=B4-B3", Fmt { bold: true, ..money });
    put(&mut summary, "A6", "Margin %", Fmt::default());
    put(&mut summary, "B6", "=B5/B4", Fmt { num: NumFmt::Percent, decimals: 1, ..Fmt::default() });
    put(&mut summary, "A7", "Verdict", Fmt::default());
    put(&mut summary, "B7", "=IF(AND(B5>0, B6>0.5), \"healthy\", \"watch it\") & \" (\" & ROUND(B6*100, 0) & \"%)\"", Fmt::default());
    summary.widths[0] = 140.0;
    summary.widths[1] = 160.0;
    Book { sheets: vec![budget, sales, summary] }
}

// ------------------------------------------------------------------------------------------------
// Drawing

const CSS: &str = r"
.scr { width: 100vw; height: 100vh; }
bui-region { background: #303030; }
bui-region[region=sidebar] { background: #2b2b2b; border-left: 1px solid #1f1f1f; overflow-y: auto; }
bui-region[region=footer] { background: #2e2e2e; border-top: 1px solid #1f1f1f; }
bui-region[region=tool_header] { background: #353535; border-bottom: 1px solid #1f1f1f; }
.hdr { display: flex; align-items: center; gap: 6px; height: 100%; padding: 0 8px; white-space: nowrap; }
.hdr .sep { width: 1px; height: 16px; background: #4a4a4a; margin: 0 4px; }
.hdr .lbl { color: #9a9a9a; font-size: 11px; }
.fx { display: flex; align-items: center; gap: 6px; height: 100%; padding: 0 8px; }
.fx .fxl { color: #8cb4ff; font-style: italic; font-weight: 700; width: 18px; text-align: center; }
.wrap { position: relative; width: 100%; height: 100%; overflow: hidden; background: #262626; user-select: none; }
.grid { display: grid; grid-auto-rows: 24px; font-size: 12px; }
.c { padding: 0 6px; line-height: 23px; white-space: nowrap; overflow: hidden; color: #dedede; background: #2c2c2c;
     border-right: 1px solid #383838; border-bottom: 1px solid #383838; font-variant-numeric: tabular-nums; }
.c.n { text-align: right; }
.c.mid { text-align: center; }
.c.end { text-align: right; }
.c.start { text-align: left; }
.c.b { font-weight: 700; }
.c.in { background: #34405a; }
.c.err { color: #ff7b6e; }
.c.edit { background: #1d1d1d; color: #ffffff; }
.ch, .rh, .corner { background: #3a3a3a; color: #a6a6a6; text-align: center; line-height: 23px; font-size: 11px;
     border-right: 1px solid #2a2a2a; border-bottom: 1px solid #2a2a2a; font-variant-numeric: tabular-nums; }
.ch { position: relative; }
.ch.on, .rh.on { background: #4a5a78; color: #ffffff; }
.grip { position: absolute; right: -4px; top: 0px; width: 8px; height: 24px; cursor: col-resize; }
.grip:hover { background: rgba(140,180,255,0.35); }
.box { position: absolute; pointer-events: none; }
.selbox { border: 1px solid #6b96e6; background: rgba(71,114,179,0.10); }
.active { border: 2px solid #9cc0ff; }
.fillbox { border: 1px dashed #9cc0ff; }
.flash { background: rgba(140,180,255,0.35); animation: flash 700ms cubic-bezier(0.2, 0, 0, 1) forwards; }
.handle { position: absolute; width: 9px; height: 9px; background: #9cc0ff; border: 2px solid #262626; cursor: crosshair; }
.caret { display: inline-block; width: 1px; height: 14px; background: #ffffff; vertical-align: middle; margin-left: 1px;
         animation: blink 1s steps(1) infinite; }
.foot { display: flex; align-items: center; height: 100%; gap: 2px; padding: 0 6px; font-size: 12px; }
.tab { padding: 0 14px; height: 22px; line-height: 22px; border-radius: 4px 4px 0 0; color: #a8a8a8; background: #333333; cursor: pointer;
       transition-property: background-color, color; transition-duration: 120ms; }
.tab:hover { background: #3d3d3d; color: #dddddd; }
.tab.on { background: #4a5a78; color: #ffffff; }
.stats { margin-left: auto; color: #a8a8a8; font-variant-numeric: tabular-nums; }
.status { color: #8cb4ff; margin-left: 12px; }
.side { padding: 6px; font-size: 12px; }
.side details { margin-bottom: 6px; }
.row { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin: 4px 0; }
.row .k { color: #9a9a9a; }
.row .v { color: #e6e6e6; font-variant-numeric: tabular-nums; text-align: right; overflow: hidden; text-overflow: ellipsis; }
.chartwrap { width: 100%; height: 100%; background: #262626; }
.help { padding: 10px 12px; font-size: 12px; overflow-y: scroll; height: 100%; box-sizing: border-box; }
.help code { color: #8cb4ff; }
.help .res { margin: 6px 0 12px; padding: 6px 8px; background: #1f1f1f; border-radius: 4px; font-variant-numeric: tabular-nums; }
.help p { margin: 4px 0 8px; color: #b8b8b8; text-wrap: pretty; }
@keyframes blink { 50% { opacity: 0; } }
@keyframes flash { to { opacity: 0; } }
";

impl Snap {
    fn sheet(&self) -> &Sheet {
        &self.book.sheets[self.ui.sheet]
    }

    /// The x of column `c`'s left edge in the grid (from the row headers' left).
    fn col_x(&self, c: u16) -> f32 {
        let w = &self.sheet().widths;
        if c >= self.ui.left {
            HEAD_W + w[self.ui.left as usize..c as usize].iter().sum::<f32>()
        } else {
            HEAD_W - w[c as usize..self.ui.left as usize].iter().sum::<f32>()
        }
    }

    fn row_y(&self, r: u16) -> f32 {
        ROW_H + (f32::from(r) - f32::from(self.ui.top)) * ROW_H
    }

    fn rect_of(&self, a: (u16, u16), b: (u16, u16)) -> (f32, f32, f32, f32) {
        let x0 = self.col_x(a.1);
        let x1 = self.col_x(b.1) + self.sheet().widths[b.1 as usize];
        let y0 = self.row_y(a.0);
        let y1 = self.row_y(b.0) + ROW_H;
        (x0, y0, x1 - x0, y1 - y0)
    }

    fn grid(&self) -> Div {
        let w = self.weak;
        let sh = self.sheet();
        let s = self.ui.sheet;
        let (a, b) = sel_of(&self.ui);
        let mut cols = Vec::new();
        let mut x = 0.0;
        for c in self.ui.left..COLS {
            if x > VIEW_W {
                break;
            }
            cols.push(c);
            x += sh.widths[c as usize];
        }
        let mut template = format!("{HEAD_W}px");
        for &c in &cols {
            let _ = write!(template, " {}px", sh.widths[c as usize]);
        }
        let mut g = div().class("grid").css("grid-template-columns", template)
            .child(div().id("corner").class("corner").child("◢").on_pointer_down(w.listener(|this: &mut SheetApp, _: &PointerEvent, _, cx| {
                this.commit();
                this.ui.anchor = (0, 0);
                this.ui.cur = (ROWS - 1, COLS - 1);
                cx.notify();
            })));
        for &c in &cols {
            let on = c >= a.1 && c <= b.1;
            g = g.child(div().id(format!("ch{c}")).class(if on { "ch on" } else { "ch" }).child(col_name(c).to_string())
                .on_pointer_down(w.listener(move |this: &mut SheetApp, e: &PointerEvent, _, cx| {
                    this.commit();
                    if !e.modifiers.shift {
                        this.ui.anchor = (0, c);
                    }
                    this.ui.cur = (ROWS - 1, c);
                    cx.notify();
                }))
                .child(div().id(format!("grip{c}")).class("grip")
                    .on_pointer_down(w.listener(move |this: &mut SheetApp, e: &PointerEvent, win, cx| {
                        let w0 = this.sheet().widths[c as usize];
                        this.ui.drag = Some(Drag::Resize { col: c, x0: e.position.x.0, w0 });
                        win.set_pointer_capture(cx);
                        cx.stop_propagation();
                    }))
                    .on_pointer_move(w.listener(move |this: &mut SheetApp, e: &PointerEvent, _, cx| {
                        if let Some(Drag::Resize { col, x0, w0 }) = this.ui.drag {
                            let nw = (w0 + e.position.x.0 - x0).clamp(28.0, 600.0);
                            let s = this.ui.sheet;
                            // live while dragging: the book is changed in place, one undo step at the end
                            Arc::make_mut(&mut this.book).sheets[s].widths[col as usize] = nw;
                            cx.notify();
                        }
                    }))
                    .on_pointer_up(w.listener(move |this: &mut SheetApp, _: &PointerEvent, win, cx| {
                        if let Some(Drag::Resize { col, w0, .. }) = this.ui.drag.take() {
                            let s = this.ui.sheet;
                            let nw = this.sheet().widths[col as usize];
                            Arc::make_mut(&mut this.book).sheets[s].widths[col as usize] = w0;
                            this.change(|b, _| b.sheets[s].widths[col as usize] = nw);
                            this.ui.status = format!("Column {} is {nw:.0} px", col_name(col));
                        }
                        win.release_pointer_capture(cx);
                        cx.notify();
                    }))));
        }
        let last = (self.ui.top + VIEW_ROWS).min(ROWS);
        for r in self.ui.top..last {
            let on = r >= a.0 && r <= b.0;
            g = g.child(div().id(format!("rh{r}")).class(if on { "rh on" } else { "rh" }).child((r + 1).to_string())
                .on_pointer_down(w.listener(move |this: &mut SheetApp, e: &PointerEvent, _, cx| {
                    this.commit();
                    if !e.modifiers.shift {
                        this.ui.anchor = (r, 0);
                    }
                    this.ui.cur = (r, COLS - 1);
                    cx.notify();
                })));
            let mut k = 0;
            while k < cols.len() {
                let span = self.spill((r, cols[k]), &cols[k + 1..], (a, b));
                g = g.child(self.cell((r, cols[k]), (a, b), s, &cols[k..=k + span]));
                k += 1 + span;
            }
        }
        g
    }

    /// How many of the empty cells after `at` (`next`) its text runs on into, as a spreadsheet
    /// lets left-aligned text spill: as many as it needs, none that hold anything or are selected.
    fn spill(&self, at: (u16, u16), next: &[u16], (a, b): ((u16, u16), (u16, u16))) -> usize {
        let sh = self.sheet();
        let f = sh.fmt(at);
        let Val::Text(t) = self.vals.get(self.ui.sheet, at) else { return 0 };
        if !matches!(f.align, Align::Auto | Align::Left) || (self.ui.draft.is_some() && at == self.ui.cur) {
            return 0;
        }
        // about how wide the text draws (12 px type, a little wider bold)
        let need = t.chars().count() as f32 * if f.bold { 7.4 } else { 6.9 } + 12.0;
        let mut have = sh.widths[at.1 as usize];
        let mut n = 0;
        for &c in next {
            if have >= need {
                break;
            }
            let free = sh.raw((at.0, c)).is_empty() && sh.fmt((at.0, c)).fill.is_none()
                && !(at.0 >= a.0 && at.0 <= b.0 && c >= a.1 && c <= b.1) && (at.0, c) != self.ui.cur;
            if !free {
                break;
            }
            have += sh.widths[c as usize];
            n += 1;
        }
        n
    }

    /// One cell, running on over `cols` (itself, then the empty ones its text spills into).
    fn cell(&self, at: (u16, u16), (a, b): ((u16, u16), (u16, u16)), s: usize, cols: &[u16]) -> Div {
        let w = self.weak;
        let f = self.sheet().fmt(at);
        let v = self.vals.get(s, at);
        let editing = self.ui.draft.as_ref().filter(|_| at == self.ui.cur);
        let mut class = String::from("c");
        let num = matches!(v, Val::Num(_));
        class.push_str(match f.align {
            Align::Auto if num => " n",
            Align::Auto | Align::Left => " start",
            Align::Center => " mid",
            Align::Right => " end",
        });
        if f.bold {
            class.push_str(" b");
        }
        if matches!(v, Val::Err(_)) {
            class.push_str(" err");
        }
        let inside = at.0 >= a.0 && at.0 <= b.0 && at.1 >= a.1 && at.1 <= b.1 && a != b;
        if inside {
            class.push_str(" in");
        }
        if editing.is_some() {
            class.push_str(" edit start");
        }
        let id = format!("c{}_{}", at.0, at.1);
        let mut d = div().id(id.clone()).class(class);
        if cols.len() > 1 {
            d = d.css("grid-column", format!("span {}", cols.len()));
        }
        // which of the columns it covers a press at window x is in
        let spans: Vec<(u16, f32)> = if cols.len() > 1 { cols.iter().map(|&c| (c, self.sheet().widths[c as usize])).collect() } else { Vec::new() };
        let column_at = move |win: &Window, x: f32| -> (u16, u16) {
            let Some(bx) = win.bounds_of(&id).map(|b| b.origin.x) else { return at };
            let mut edge = bx;
            for &(c, wd) in &spans {
                edge += wd;
                if x < edge {
                    return (at.0, c);
                }
            }
            at
        };
        if editing.is_none() {
            if let Some(fill) = f.fill {
                d = d.css("background", if inside { hex(blend(fill, 0x4772b3)) } else { hex(fill) });
            }
            if let Some(col) = f.color {
                d = d.css("color", hex(col));
            }
        }
        d = match editing {
            Some(text) => d.child(span().child(text.clone())).child(span().class("caret")),
            None => d.child(show(v, f)),
        };
        d.on_pointer_down(w.listener(move |this: &mut SheetApp, e: &PointerEvent, win, cx| {
            this.press(column_at(win, e.position.x.0), e.modifiers.shift);
            cx.notify();
        }))
        .on_pointer_enter(w.listener(move |this: &mut SheetApp, _: &PointerEvent, _, cx| {
            if this.enter(at) {
                cx.notify();
            }
        }))
        .on_double_click(w.listener(move |this: &mut SheetApp, _: &ClickEvent, _, cx| {
            this.select(at, false);
            this.ui.draft = Some(this.sheet().raw(at).to_string());
            cx.notify();
        }))
    }

    /// The grid with the selection drawn over it.
    fn main(&self) -> Div {
        let w = self.weak;
        let (a, b) = sel_of(&self.ui);
        let (x, y, wd, ht) = self.rect_of(a, b);
        let place = |d: Div, (x, y, w, h): (f32, f32, f32, f32)| {
            d.css("left", format!("{x}px")).css("top", format!("{y}px")).css("width", format!("{w}px")).css("height", format!("{h}px"))
        };
        let mut wrap = div().id("wrap").class("wrap").child(self.grid())
            .child(place(div().class("box selbox"), (x, y, wd, ht)))
            .child(place(div().class("box active"), self.rect_of(self.ui.cur, self.ui.cur)))
            .on_scroll_wheel(w.listener(|this: &mut SheetApp, e: &ScrollWheelEvent, _, cx| {
                let lines = (-e.delta * 3.0).round() as i32;
                if e.modifiers.shift {
                    this.ui.left = (i32::from(this.ui.left) + lines.signum()).clamp(0, i32::from(COLS) - 1) as u16;
                } else {
                    this.ui.top = (i32::from(this.ui.top) + lines).clamp(0, i32::from(ROWS) - 10) as u16;
                }
                cx.notify();
                cx.stop_propagation();
            }));
        if let Some(Drag::Fill(t)) = self.ui.drag {
            let end = if t.0 > b.0 && t.0 - b.0 >= t.1.saturating_sub(b.1) { (t.0, b.1) } else { (b.0, t.1.max(b.1)) };
            wrap = wrap.child(place(div().class("box fillbox"), self.rect_of(a, end)));
        }
        if let Some((fa, fb)) = self.ui.flash_rect {
            wrap = wrap.child(place(div().id(format!("flash{}", self.ui.flash)).class("box flash"), self.rect_of(fa, fb)));
        }
        if self.ui.draft.is_none() {
            wrap = wrap.child(div().id("fill").class("handle")
                .css("left", format!("{}px", x + wd - 6.0)).css("top", format!("{}px", y + ht - 6.0))
                .on_pointer_down(w.listener(|this: &mut SheetApp, _: &PointerEvent, _, cx| {
                    this.ui.drag = Some(Drag::Fill(this.ui.cur));
                    cx.stop_propagation();
                    cx.notify();
                })));
        }
        wrap
    }

    fn toolbar(&self) -> Div {
        let w = self.weak;
        let f = self.sheet().fmt(self.ui.cur);
        let fmt_dd = NUM_FMTS.iter().fold(Dropdown().id("numfmt").css("width", "104px")
            .value(NUM_FMTS.iter().find(|(_, n)| *n == f.num).map_or("General", |(l, _)| *l)), |d, (l, _)| d.child(option().child(*l)));
        let align_dd = ALIGNS.iter().fold(Dropdown().id("align").css("width", "84px")
            .value(ALIGNS.iter().find(|(_, x)| *x == f.align).map_or("Auto", |(l, _)| *l)), |d, (l, _)| d.child(option().child(*l)));
        div().class("hdr")
            .child(Button().id("undo").child("Undo").disabled(!self.can_undo)
                .on_click(w.listener(|this: &mut SheetApp, _: &ClickEvent, _, cx| if this.undo() { cx.notify() })))
            .child(Button().id("redo").child("Redo").disabled(!self.can_redo)
                .on_click(w.listener(|this: &mut SheetApp, _: &ClickEvent, _, cx| if this.redo() { cx.notify() })))
            .child(div().class("sep"))
            .child(Checkbox().id("bold").checked(f.bold).child("Bold")
                .on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                    let on = e.checked;
                    this.format(if on { "Bold" } else { "Not bold" }, |f| f.bold = on);
                    cx.notify();
                })))
            .child(span().class("lbl").child("Align"))
            .child(align_dd.on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                let al = ALIGNS[(e.number as usize).min(3)].1;
                this.format("Aligned", |f| f.align = al);
                cx.notify();
            })))
            .child(span().class("lbl").child("Format"))
            .child(fmt_dd.on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                let n = NUM_FMTS[(e.number as usize).min(3)].1;
                this.format("Number format", |f| f.num = n);
                cx.notify();
            })))
            .child(span().class("lbl").child("Decimals"))
            .child(NumberField().id("decimals").min(0.0).max(6.0).step(1.0).value(f.decimals.to_string()).css("width", "70px")
                .on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                    let d = e.number.round().clamp(0.0, 6.0) as u8;
                    this.format("Decimals", |f| f.decimals = d);
                    cx.notify();
                })))
            .child(div().class("sep"))
            .child(span().class("lbl").child("Fill"))
            .child(ColorSwatch().id("fill").value(hex(f.fill.unwrap_or(0x2c2c2c))).css("width", "40px")
                .on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                    let c = parse_hex(&e.value);
                    this.format("Fill", |f| f.fill = c);
                    cx.notify();
                })))
            .child(span().class("lbl").child("Text"))
            .child(ColorSwatch().id("ink").value(hex(f.color.unwrap_or(0xdedede))).css("width", "40px")
                .on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                    let c = parse_hex(&e.value);
                    this.format("Text color", |f| f.color = c);
                    cx.notify();
                })))
            .child(Button().id("nofill").child("Plain")
                .on_click(w.listener(|this: &mut SheetApp, _: &ClickEvent, _, cx| {
                    this.format("Plain", |f| *f = Fmt::default());
                    cx.notify();
                })))
            .child(div().class("sep"))
            .child(Button().id("sortasc").child("Sort A→Z")
                .on_click(w.listener(|this: &mut SheetApp, _: &ClickEvent, _, cx| {
                    this.sort(false);
                    cx.notify();
                })))
            .child(Button().id("sortdesc").child("Sort Z→A")
                .on_click(w.listener(|this: &mut SheetApp, _: &ClickEvent, _, cx| {
                    this.sort(true);
                    cx.notify();
                })))
            .child(Button().id("tochart").child("Chart Selection")
                .on_click(w.listener(|this: &mut SheetApp, _: &ClickEvent, _, cx| {
                    let (a, b) = this.sel();
                    this.ui.chart.sheet = this.ui.sheet;
                    this.ui.chart.a = a;
                    this.ui.chart.b = b;
                    this.ui.status = format!("Charting {}:{}", addr(a), addr(b));
                    cx.notify();
                })))
            .child(Button().id("sidebar").child(if self.ui.sidebar { "Hide Sidebar" } else { "Sidebar" })
                .on_click(w.listener(|this: &mut SheetApp, _: &ClickEvent, _, cx| {
                    this.ui.sidebar = !this.ui.sidebar;
                    cx.notify();
                })))
    }

    fn formula_bar(&self) -> Div {
        let w = self.weak;
        let (a, b) = sel_of(&self.ui);
        let name = if a == b { addr(a) } else { format!("{}:{}", addr(a), addr(b)) };
        let raw = self.ui.draft.clone().unwrap_or_else(|| self.sheet().raw(self.ui.cur).to_string());
        div().class("fx")
            .child(TextField().id("name").value(name).css("width", "96px")
                .on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, win, cx| {
                    // the keyboard goes back to the grid, as after any jump
                    win.focus_id("root");
                    let mut parts = e.value.split(':');
                    if let Some((r, c, ..)) = parts.next().and_then(parse_ref) {
                        this.select((r, c), false);
                        if let Some((r2, c2, ..)) = parts.next().and_then(parse_ref) {
                            this.ui.cur = (r2, c2);
                        }
                        this.ui.status = format!("Went to {}", e.value.to_uppercase());
                    }
                    cx.notify();
                })))
            .child(span().class("fxl").child("ƒx"))
            .child(TextField().id("formula").value(raw).placeholder("Type a value or =formula").css("flex", "1")
                .on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, win, cx| {
                    win.focus_id("root");
                    this.ui.draft = None;
                    let at = this.ui.cur;
                    this.set_cell_raw(at, e.value.clone());
                    this.ui.status = format!("{} set", addr(at));
                    cx.notify();
                })))
    }

    fn footer(&self) -> Div {
        let w = self.weak;
        let mut d = div().class("foot");
        for (i, sh) in self.book.sheets.iter().enumerate() {
            d = d.child(div().id(format!("tab{i}")).class(if i == self.ui.sheet { "tab on" } else { "tab" }).child(sh.name.clone())
                .on_click(w.listener(move |this: &mut SheetApp, _: &ClickEvent, _, cx| {
                    this.go_sheet(i);
                    cx.notify();
                })));
        }
        d = d.child(Button().id("addsheet").child("+").css("width", "28px")
            .on_click(w.listener(|this: &mut SheetApp, _: &ClickEvent, _, cx| {
                this.add_sheet();
                cx.notify();
            })));
        let (sum, count, avg) = self.stats();
        d.child(span().class("status").child(self.ui.status.clone()))
            .child(span().class("stats").child(format!("Sum {}   Average {}   Count {count}", grouped(sum, 2), avg.map_or("—".into(), |a| grouped(a, 2)))))
    }

    /// Sum, count and mean of the numbers selected.
    fn stats(&self) -> (f64, usize, Option<f64>) {
        let (a, b) = sel_of(&self.ui);
        let (mut sum, mut n) = (0.0, 0);
        let m = &self.vals.sheets[self.ui.sheet];
        for (at, v) in m {
            if at.0 >= a.0 && at.0 <= b.0 && at.1 >= a.1 && at.1 <= b.1
                && let Val::Num(x) = v
            {
                sum += x;
                n += 1;
            }
        }
        (sum, n, (n > 0).then(|| sum / n as f64))
    }

    fn sidebar(&self) -> Div {
        let w = self.weak;
        let at = self.ui.cur;
        let sh = self.sheet();
        let f = sh.fmt(at);
        let v = self.vals.get(self.ui.sheet, at);
        let kind = match v {
            Val::Empty => "Empty",
            Val::Num(_) => "Number",
            Val::Text(_) => "Text",
            Val::Bool(_) => "Logical",
            Val::Err(_) => "Error",
        };
        let row = |k: &str, val: String| div().class("row").child(span().class("k").child(k.to_string())).child(span().class("v").child(val));
        let (sum, count, avg) = self.stats();
        let (a, b) = sel_of(&self.ui);
        let mins = self.vals.sheets[self.ui.sheet].iter()
            .filter(|(p, _)| p.0 >= a.0 && p.0 <= b.0 && p.1 >= a.1 && p.1 <= b.1)
            .filter_map(|(_, v)| if let Val::Num(x) = v { Some(*x) } else { None })
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| (lo.min(x), hi.max(x)));
        div().class("side")
            .child(details().open(true).child(summary().child("Cell"))
                .child(row("Address", addr(at)))
                .child(row("Kind", kind.into()))
                .child(row("Shows", show(v, f)))
                .child(row("Value", v.text()))
                .child(row("Raw", sh.raw(at).to_string())))
            .child(details().open(true).child(summary().child("Selection"))
                .child(row("Range", format!("{}:{}", addr(a), addr(b))))
                .child(row("Count", count.to_string()))
                .child(row("Sum", grouped(sum, 2)))
                .child(row("Average", avg.map_or("—".into(), |x| grouped(x, 2))))
                .child(row("Min / Max", if count > 0 { format!("{} / {}", grouped(mins.0, 0), grouped(mins.1, 0)) } else { "—".into() })))
            .child(details().open(true).child(summary().child("Column"))
                .child(div().class("row").child(span().class("k").child(format!("Width of {}", col_name(at.1))))
                    .child(NumberField().id("colw").min(28.0).max(600.0).step(1.0).value(format!("{:.0}", sh.widths[at.1 as usize]))
                        .css("width", "110px")
                        .on_change(w.listener(move |this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                            let nw = e.number.clamp(28.0, 600.0) as f32;
                            let c = this.ui.cur.1;
                            this.change(|b, s| b.sheets[s].widths[c as usize] = nw);
                            this.ui.status = format!("Column {} is {nw:.0} px", col_name(c));
                            cx.notify();
                        }))))
                .child(p().child("Drag the field, or double-click it and type math: 92*1.5")))
            .child(details().open(true).child(summary().child("Sheet"))
                .child(div().class("row").child(span().class("k").child("Name"))
                    .child(TextField().id("sheetname").value(sh.name.clone()).css("width", "130px")
                        .on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                            let name: String = e.value.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                            if !name.is_empty() && this.book.sheet_index(&name).is_none() {
                                this.change(|b, s| b.sheets[s].name.clone_from(&name));
                                this.ui.status = format!("Renamed to {name}");
                            } else {
                                this.ui.status = "That name is taken or empty".into();
                            }
                            cx.notify();
                        }))))
                .child(row("Cells in use", sh.cells.len().to_string()))
                .child(row("Undo steps", if self.can_undo { "yes" } else { "none" }.to_string())))
    }

    fn sheet_editor(&self) -> Editor {
        Editor::new(self.main())
            .header(self.toolbar())
            .tool_header(Region::new(self.formula_bar()).size(30).overlap(false))
            .footer(Region::new(self.footer()).size(28).overlap(false))
            .sidebar(Region::new(self.sidebar()).size(250).overlap(false).hidden(!self.ui.sidebar))
    }

    fn chart_editor(&self) -> Editor {
        let w = self.weak;
        let ch = self.ui.chart;
        let data = chart_data(&self.book, &self.vals, ch);
        let kind_dd = CHART_KINDS.iter().fold(Dropdown().id("ckind").css("width", "90px").value(CHART_KINDS[ch.kind]), |d, k| d.child(option().child(*k)));
        let mut head = div().class("hdr")
            .child(span().class("lbl").child(format!("{}!{}:{}", self.book.sheets.get(ch.sheet).map_or("?", |s| s.name.as_str()), addr(ch.a), addr(ch.b))))
            .child(kind_dd.on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                this.ui.chart.kind = (e.number as usize).min(CHART_KINDS.len() - 1);
                cx.notify();
            })))
            .child(span().class("lbl").child(if ch.kind == 1 { "Line" } else { "Width" }))
            .child(NumberSlider().id("cwidth").min(0.1).max(1.0).step(0.05).value(format!("{:.2}", ch.width)).css("width", "120px")
                .on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                    this.ui.chart.width = e.number.clamp(0.1, 1.0);
                    cx.notify();
                })));
        for k in 0..3 {
            head = head.child(ColorSwatch().id(format!("ccol{k}")).value(hex(ch.colors[k])).css("width", "34px")
                .on_change(w.listener(move |this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                    if let Some(c) = parse_hex(&e.value) {
                        this.ui.chart.colors[k] = c;
                        cx.notify();
                    }
                })));
        }
        let colors = ch.colors;
        let (kind, width) = (ch.kind, ch.width as f32);
        let body = div().class("chartwrap").child(canvas(move |b, p| draw_chart(b, p, &data, kind, width, colors)).css("width", "100%").css("height", "100%"));
        Editor::new(body).header(head)
    }

    fn help_editor(&self) -> Editor {
        let w = self.weak;
        let mut calc_book = (*self.book).clone();
        // the formula is tried in a cell of its own, on the sheet shown
        let probe = (ROWS - 1, COLS - 1);
        calc_book.sheets[self.ui.sheet].set_raw(probe, self.ui.try_src.clone());
        let v = compute(&calc_book).get(self.ui.sheet, probe).clone();
        let fun = |name: &str, sig: &str, what: &str| {
            details().child(summary().child(name.to_string())).child(p().child(code().child(sig.to_string()))).child(p().child(what.to_string()))
        };
        let body = div().class("help")
            .child(h3().child("Try a formula"))
            .child(TextField().id("try").value(self.ui.try_src.clone()).css("width", "100%")
                .on_change(w.listener(|this: &mut SheetApp, e: &ChangeEvent, _, cx| {
                    this.ui.try_src = if e.value.starts_with('=') { e.value.clone() } else { format!("={}", e.value) };
                    cx.notify();
                })))
            .child(div().class("res").child(format!("= {}", show(&v, Fmt { num: NumFmt::Number, ..Fmt::default() }))))
            .child(fun("SUM", "=SUM(B3:G3, 100)", "Adds numbers and ranges; text is skipped."))
            .child(fun("AVERAGE", "=AVERAGE(B9:G9)", "The mean of the numbers; #DIV/0! when there are none."))
            .child(fun("MIN / MAX", "=MAX(Sales!B3:E7)", "Smallest or largest number. Other sheets by name."))
            .child(fun("COUNT", "=COUNT(A1:Z200)", "How many cells hold numbers."))
            .child(fun("IF", "=IF(H9<40000, \"ok\", \"over\")", "One of two values by a test (= <> < > <= >=)."))
            .child(fun("AND / OR", "=AND(B5>0, B6>0.5)", "Logic over tests."))
            .child(fun("ROUND / ABS / SQRT", "=ROUND(B6*100, 1)", "Number helpers; PI() too."))
            .child(fun("Text", "=\"Total: \" & H9", "& joins text; 15% is 0.15."))
            .child(fun("Errors", "=1/0  =A1+  =NOPE(1)  =C5 in C5", "#DIV/0!, #ERROR!, #NAME?, #CYCLE!, #REF!"));
        Editor::new(body)
    }
}

/// a over b, half and half: a fill under the selection tint.
fn blend(a: u32, b: u32) -> u32 {
    let ch = |s: u32| u32::midpoint((a >> s) & 255, (b >> s) & 255);
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

struct ChartData {
    labels: Vec<String>,
    series: Vec<(String, Vec<f64>)>,
}

/// The chart's range: a first row of labels and a first column of series names when they are text.
fn chart_data(book: &Book, vals: &Vals, ch: Chart) -> ChartData {
    let s = ch.sheet.min(book.sheets.len() - 1);
    let (a, b) = (ch.a, ch.b);
    let is_text = |at| matches!(vals.get(s, at), Val::Text(_) | Val::Empty);
    let head_row = (a.1..=b.1).skip(1).any(|c| matches!(vals.get(s, (a.0, c)), Val::Text(_)));
    let head_col = (a.0..=b.0).skip(usize::from(head_row)).all(|r| is_text((r, a.1)));
    let r0 = a.0 + u16::from(head_row);
    let c0 = a.1 + u16::from(head_col);
    let labels = (c0..=b.1).map(|c| if head_row { vals.get(s, (a.0, c)).text() } else { col_name(c).to_string() }).collect();
    let series = (r0..=b.0).map(|r| {
        let name = if head_col { vals.get(s, (r, a.1)).text() } else { format!("Row {}", r + 1) };
        (name, (c0..=b.1).map(|c| vals.get(s, (r, c)).num().unwrap_or(0.0)).collect())
    }).collect();
    ChartData { labels, series }
}

/// A round step for an axis to `max` in about five.
fn nice_step(max: f64) -> f64 {
    let raw = (max / 5.0).max(1e-9);
    let mag = 10f64.powf(raw.log10().floor());
    let n = raw / mag;
    mag * if n <= 1.0 { 1.0 } else if n <= 2.0 { 2.0 } else if n <= 5.0 { 5.0 } else { 10.0 }
}

const fn color_of(colors: [u32; 3], i: usize) -> u32 {
    if i < 3 { colors[i] } else { PALETTE[i % PALETTE.len()] }
}

fn draw_chart(b: Bounds<f32>, p: &mut Painter, d: &ChartData, kind: usize, width: f32, colors: [u32; 3]) {
    let (x0, y0, w, h) = (b.origin.x, b.origin.y, b.size.width, b.size.height);
    let text = rgb(0xb8b8b8);
    if d.series.is_empty() || d.labels.is_empty() || w < 60.0 || h < 60.0 {
        p.text(x0 + 16.0, y0 + 16.0, "Select a range and press Chart Selection", 13.0, text);
        return;
    }
    // the legend along the top
    let mut lx = x0 + 56.0;
    for (i, (name, _)) in d.series.iter().enumerate() {
        p.quad(Bounds { origin: point(lx, y0 + 14.0), size: size(10.0, 10.0) }, 2.0, rgb(color_of(colors, i)), Rgba::TRANSPARENT, 0.0);
        p.text(lx + 14.0, y0 + 11.0, name, 11.0, text);
        lx += 22.0 + name.len() as f32 * 6.5;
    }
    let plot = Bounds { origin: point(x0 + 56.0, y0 + 40.0), size: size(w - 76.0, h - 70.0) };
    if kind == 3 {
        // pie of each series' total
        let totals: Vec<f64> = d.series.iter().map(|(_, v)| v.iter().sum::<f64>().max(0.0)).collect();
        let sum: f64 = totals.iter().sum();
        let r = plot.size.width.min(plot.size.height) / 2.0 * width.clamp(0.3, 1.0);
        let (cx, cy) = (plot.origin.x + plot.size.width / 2.0, plot.origin.y + plot.size.height / 2.0);
        let mut at = 90.0f32;
        for (i, t) in totals.iter().enumerate() {
            let ext = (t / sum.max(1e-9) * 360.0) as f32;
            p.arc(Bounds { origin: point(cx - r, cy - r), size: size(2.0 * r, 2.0 * r) }, at, -ext, rgb(color_of(colors, i)));
            let mid = (at - ext / 2.0).to_radians();
            let pct = t / sum.max(1e-9) * 100.0;
            if pct > 4.0 {
                p.text(cx + mid.cos() * r * 0.62 - 14.0, cy - mid.sin() * r * 0.62 - 6.0, &format!("{pct:.0}%"), 12.0, rgb(0xffffff));
            }
            at -= ext;
        }
        return;
    }
    let max = d.series.iter().flat_map(|(_, v)| v.iter().copied()).fold(0.0f64, f64::max);
    let step = nice_step(max);
    let top = (max / step).ceil().max(1.0) * step;
    let y_of = |v: f64| plot.origin.y + plot.size.height - (v / top) as f32 * plot.size.height;
    // grid lines and their values
    let mut v = 0.0;
    while v <= top + step * 0.01 {
        let y = y_of(v);
        p.line(&[(plot.origin.x, y), (plot.origin.x + plot.size.width, y)], rgb(if v == 0.0 { 0x5a5a5a } else { 0x343434 }), 1.0);
        p.text(x0 + 6.0, y - 7.0, &grouped(v, 0), 11.0, text);
        v += step;
    }
    let n = d.labels.len();
    let slot = plot.size.width / n as f32;
    for (k, l) in d.labels.iter().enumerate() {
        p.text(plot.origin.x + slot * (k as f32 + 0.5) - l.len() as f32 * 3.2, plot.origin.y + plot.size.height + 6.0, l, 11.0, text);
    }
    let ns = d.series.len();
    match kind {
        0 => {
            let group = slot * width;
            let bar = group / ns as f32;
            for (i, (_, vals)) in d.series.iter().enumerate() {
                for (k, v) in vals.iter().enumerate() {
                    let x = plot.origin.x + slot * k as f32 + (slot - group) / 2.0 + bar * i as f32;
                    let y = y_of(v.max(0.0));
                    let h = plot.origin.y + plot.size.height - y;
                    p.quad(Bounds { origin: point(x + 0.5, y), size: size((bar - 1.0).max(1.0), h) }, 2.0f32.min(bar / 3.0),
                           rgb(color_of(colors, i)), Rgba::TRANSPARENT, 0.0);
                }
            }
        }
        _ => {
            for (i, (_, vals)) in d.series.iter().enumerate() {
                let pts: Vec<(f32, f32)> = vals.iter().enumerate().map(|(k, v)| (plot.origin.x + slot * (k as f32 + 0.5), y_of(*v))).collect();
                let c = rgb(color_of(colors, i));
                if kind == 2 {
                    let mut poly = pts.clone();
                    poly.push((pts[pts.len() - 1].0, y_of(0.0)));
                    poly.push((pts[0].0, y_of(0.0)));
                    p.polygon(&poly, Rgba { a: 0.28, ..c });
                }
                p.line(&pts, c, 1.0 + width * 3.0);
                for &(x, y) in &pts {
                    p.oval(Bounds { origin: point(x - 3.0, y - 3.0), size: size(6.0, 6.0) }, c, rgb(0x262626), 1.0);
                }
            }
        }
    }
}

impl Render for SheetApp {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        focus_root(w, &mut self.frames);
        if let Some(b) = w.bounds_of("wrap") {
            self.view = (b.size.width, b.size.height);
        }
        let snap = Arc::new(Snap {
            book: self.book.clone(),
            vals: self.vals.clone(),
            ui: self.ui.clone(),
            weak: cx.weak_entity(),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        });
        let (s1, s2, s3) = (snap.clone(), snap.clone(), snap);
        let editors = Editors::new()
            .editor("Spreadsheet", "SPREADSHEET", move |_: &AreaRef| s1.sheet_editor())
            .editor("Chart", "GRAPH", move |_: &AreaRef| s2.chart_editor())
            .editor("Functions", "TEXT", move |_: &AreaRef| s3.help_editor());
        app().id("root").attr("tabindex", "0")
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
                // keys typed into a text field reach the page too: the grid leaves those alone. A key
                // the grid takes is not Blender's as well (a hovered field would paste Ctrl+V too)
                if !w.text_field_focused() && this.key(e) {
                    cx.prevent_default();
                    cx.notify();
                }
            }))
            .on_pointer_up(cx.listener(|this, _: &PointerEvent, _, cx| if this.release() { cx.notify() }))
            .child(style().child(CSS))
            .child(Screen().editors(editors).class("scr")
                .layout(AreaSplit::v(0.68, "Spreadsheet", AreaSplit::h(0.52, "Chart", "Functions"))))
    }
}

/// Types into the grid, a formula, a drag-select, a format, copy and paste, the fill handle, a
/// column resize, another sheet, errors, undo.
fn storyboard(dir: String) -> Script {
    story(dir, |s| {
        s.play("00-open", &[0, 300]);
        s.cx.simulate_mouse_move(s.win, 5.0, 5.0);
        // B15: a number, then a formula
        s.click("c14_1");
        s.type_text("1234.5");
        s.play("01-typing", &[0]);
        s.key("Return");
        s.type_text("=B15*2+SUM(B3:B8)");
        s.play("02-formula-typed", &[0]);
        s.key("Return");
        s.play("03-formula-done", &[0]);
        // select B3:G8 by dragging
        let (x0, y0) = s.centre("c2_1");
        let (x1, y1) = s.centre("c7_6");
        s.cx.simulate_drag(s.win, (x0, y0), (x1, y1));
        s.play("04-drag-select", &[0]);
        s.click("bold");
        s.play("05-bold", &[0]);
        // the Name box goes anywhere: copy the totals H3:H5, paste them at J3
        let goto = |s: &mut bui_demos::story::Story, to: &str| {
            s.click("name");
            s.tick(100);
            // Ctrl+A selects the field's text (the grid leaves keys to a widget under the pointer)
            s.cx.simulate_key_with(s.win, "a", "a", true, false);
            s.type_text(to);
            s.key("Return");
            s.tick(400);
        };
        s.tick(400);
        goto(s, "H3:H5");
        s.cx.simulate_key_with(s.win, "c", "c", true, false);
        goto(s, "J3");
        s.cx.simulate_key_with(s.win, "v", "v", true, false);
        s.play("06-paste", &[0, 200]);
        // a series: 10, 20 then the fill handle down four rows
        goto(s, "A16");
        s.type_text("10");
        s.key("Return");
        s.type_text("20");
        s.key("Return");
        goto(s, "A16:A17");
        s.play("07-before-fill", &[0]);
        let (hx, hy) = s.centre("fill");
        let (tx, ty) = s.centre("c20_0");
        s.cx.simulate_drag(s.win, (hx, hy), (tx, ty));
        s.play("08-filled", &[0, 300]);
        // widen column C by its header's edge
        let g = s.bounds("grip2");
        let (gx, gy) = (g.origin.x + g.size.width / 2.0, g.origin.y + g.size.height / 2.0);
        s.cx.simulate_drag(s.win, (gx, gy), (gx + 70.0, gy));
        s.play("09-resized", &[0]);
        // errors
        s.tick(400);
        s.click("c23_1");
        s.type_text("=1/0");
        s.key("Return");
        s.type_text("=B25");
        s.key("Return");
        s.type_text("=NOPE(2)");
        s.key("Return");
        s.play("10-errors", &[0]);
        // the chart of a sales range, then another sheet
        s.click("tab1");
        s.play("11-sales", &[0]);
        let (x0, y0) = s.centre("c1_0");
        let (x1, y1) = s.centre("c6_4");
        s.cx.simulate_drag(s.win, (x0, y0), (x1, y1));
        s.click("tochart");
        s.play("12-chart", &[0]);
        s.click("tab2");
        s.play("13-summary", &[0]);
        s.cx.simulate_key_with(s.win, "z", "z", true, false);
        s.play("14-undo", &[0]);
        s.click("addsheet");
        s.play("15-new-sheet", &[0, 400]);
    })
}

fn main() {
    let (w, h) = size_arg((1440.0, 900.0));
    run_with(w, h, "BUI Sheet", SheetApp::new, arg("--storyboard").map(storyboard));
}

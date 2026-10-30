// Copyright (c) 2026 Stephen Roe

//! A file's formulas as a workbook's.
//!
//! A sheet becomes a worksheet whose first row is the header, so a column
//! and a row of a sheet become a cell: `Revenue` on its own row is `B3`,
//! `[Revenue; *-1]` is `B2`, `[Revenue; 0]` is `$B$2`, and `Sales.Revenue`
//! is `$B$2:$B$13`. Each cell is written for the row it is on. A formula
//! that a workbook has no way to say is not written, and the reason is
//! given: the cell holds its value instead.

use omasheet_engine::omx::ast::{BinOp, Lit, UnOp};
use omasheet_engine::omx::check::Program;
use omasheet_engine::omx::date;
use omasheet_engine::omx::funcs::MathFn;
use omasheet_engine::omx::ir::{Bound, ColSel, Func, Ir, Node, Offset, RowSel};
use omasheet_engine::omx::types::S;
use omasheet_engine::value::format_rat;
use omasheet_engine::{Engine, Value};

/// Why a formula was not written.
pub type Why = &'static str;

const VECTOR: Why = "values were written in place of formulas that work on a whole column at once";
const CONDITION: Why =
    "values were written in place of formulas that pick rows by more than `Column == value`";
const SELECTION: Why =
    "values were written in place of formulas that pick rows by a range value or a list";
const SORTED: Why =
    "values were written in place of formulas that put rows in an order of their own";
const ROW: Why = "values were written in place of formulas that work out which row to read";
const NO_ROWS: Why = "values were written in place of formulas that read no rows";
const COLUMNS: Why =
    "values were written in place of formulas that read columns that are not side by side";
const FALLBACK: Why = "values were written in place of formulas that use `//` on a calculation";
const TYPE: Why =
    "values were written in place of formulas on values whose type a workbook cannot tell";
const OPERATOR: Why = "values were written in place of formulas that use `in` or a range";
// A workbook's `BITAND` and the rest take only 0 to 2^48 - 1, and a formula
// cannot say beforehand that every row's numbers are among them.
const BITWISE: Why = "values were written in place of formulas that use a bitwise operator";
const FUNCTION: Why =
    "values were written in place of formulas that use a function a workbook has not got";
const LITERAL: Why =
    "values were written in place of formulas with a number or text a workbook cannot hold";
const OTHER_SHEET: Why =
    "values were written in place of formulas that read a sheet that is not exported";
const TOO_LONG: Why = "values were written in place of formulas too long for a workbook";
const SEQUENCE: Why = "values were written in place of sequences made with `...`";
const PARTITION: Why =
    "values were written in place of formulas that count rows within a partition with `by`";

/// The most characters a workbook's formula may have.
const LONGEST: usize = 8000;
/// The most characters of text written into a formula.
const LONGEST_TEXT: usize = 255;

/// What a part of a formula gives, as far as a workbook needs to know: it
/// counts dates in days and times in fractions of a day.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Number,
    Text,
    Bool,
    Date,
    Time,
    DateTime,
    Unknown,
}

/// How a part of a formula is written, for `//`.
#[derive(Clone, Copy, PartialEq)]
enum Form {
    Calc,
    /// One cell.
    Cell,
    /// A row found by a condition, which may find none.
    Lookup,
}

/// How tightly a part of a formula holds together.
const COMPARE: u8 = 1;
const ADD: u8 = 3;
const MUL: u8 = 4;
const POW: u8 = 5;
const NEG: u8 = 6;
const ATOM: u8 = 9;

#[derive(Clone)]
struct Expr {
    text: String,
    prec: u8,
    kind: Kind,
    form: Form,
}

impl Expr {
    fn new(text: String, prec: u8, kind: Kind) -> Expr {
        Expr {
            text,
            prec,
            kind,
            form: Form::Calc,
        }
    }

    fn atom(text: String, kind: Kind) -> Expr {
        Expr::new(text, ATOM, kind)
    }

    /// In brackets if it holds together less tightly than `prec`.
    fn within(&self, prec: u8) -> String {
        if self.prec < prec {
            format!("({})", self.text)
        } else {
            self.text.clone()
        }
    }
}

/// Where a formula is: its sheet and row, how many rows a condition has
/// put between it and what is being read, and the arguments of the function
/// it is the body of.
#[derive(Clone, Copy)]
struct Cx<'e> {
    sheet: usize,
    row: usize,
    depth: usize,
    args: &'e [Option<Expr>],
}

/// The rows a selection reads: one, or from one to another. `true` marks a
/// row that is the same whichever row asks.
enum Rows<'n> {
    One(i64, bool),
    Span((i64, bool), (i64, bool)),
    Where(&'n Node),
}

type Scalar = Result<Option<Expr>, Why>;

/// The column letters of a workbook: `A`, `Z`, `AA`.
fn letters(col: usize) -> String {
    let mut out = String::new();
    let mut n = col + 1;
    while n > 0 {
        out.insert(0, (b'A' + ((n - 1) % 26) as u8) as char);
        n = (n - 1) / 26;
    }
    out
}

fn kind_of(s: S) -> Kind {
    match s {
        S::Int | S::Hex | S::Octal | S::Binary | S::Decimal | S::Fraction | S::Percent | S::Float => Kind::Number,
        S::String | S::Url | S::Email | S::Uuid => Kind::Text,
        S::Boolean => Kind::Bool,
        S::Date => Kind::Date,
        S::Time => Kind::Time,
        S::DateTime => Kind::DateTime,
        S::Any => Kind::Unknown,
    }
}

/// A whole number written in the formula, such as the `1` of `*-1`.
fn whole(program: &Program, node: &Node) -> Option<i64> {
    match &node.kind {
        Ir::Lit(Lit::Int(n, _)) => Some(*n),
        Ir::Unary(UnOp::Neg, inner) => whole(program, inner).map(|n| -n),
        Ir::Const(i) => whole(program, &program.consts[*i].node),
        Ir::Single(inner) => whole(program, inner),
        _ => None,
    }
}

/// The sheet, rows and columns a selection reads; no columns is all of
/// them. `None` if it is not a selection from a sheet.
fn target(node: &Node) -> Option<(usize, Option<&RowSel>, Vec<usize>)> {
    match &node.kind {
        Ir::Column { sheet, col } => Some((*sheet, None, vec![*col])),
        Ir::Single(inner) => target(inner),
        Ir::Index { base, rows, cols } => match (&base.kind, cols) {
            (Ir::Sheet(t), ColSel::All) => Some((*t, Some(rows), Vec::new())),
            (Ir::Sheet(t), ColSel::One(c)) => Some((*t, Some(rows), vec![*c])),
            (Ir::Sheet(t), ColSel::Many(many)) => Some((*t, Some(rows), many.clone())),
            (Ir::Column { sheet, col }, ColSel::All) => Some((*sheet, Some(rows), vec![*col])),
            _ => None,
        },
        Ir::Field { base, col } => {
            let (t, rows, cols) = target(base)?;
            (cols.is_empty() || cols.contains(col)).then(|| (t, rows, vec![*col]))
        }
        _ => None,
    }
}

/// Writes the formulas of a file for a workbook.
pub struct Writer<'a> {
    pub program: &'a Program,
    pub engine: &'a Engine<'a>,
    /// The worksheet of each sheet, or `None` for one that is not exported.
    pub titles: Vec<Option<String>>,
}

impl Writer<'_> {
    /// The formula of the cell on row `row` of `sheet`, as a workbook writes
    /// it, without its `=`. `None` if it comes to nothing there.
    pub fn formula(&self, node: &Node, sheet: usize, row: usize) -> Result<Option<String>, Why> {
        let cx = Cx {
            sheet,
            row,
            depth: 0,
            args: &[],
        };
        match self.scalar(node, &cx)? {
            Some(e) if e.text.len() > LONGEST => Err(TOO_LONG),
            found => Ok(found.map(|e| e.text)),
        }
    }

    /// The worksheet of a sheet, written before a cell of it: nothing for
    /// the sheet the formula is in.
    fn worksheet(&self, t: usize, cx: &Cx) -> Result<String, Why> {
        let title = self.titles[t].as_ref().ok_or(OTHER_SHEET)?;
        Ok(if t == cx.sheet {
            String::new()
        } else {
            format!("'{}'!", title.replace('\'', "''"))
        })
    }

    /// One cell, or nothing if the row is outside the sheet.
    fn cell(&self, cx: &Cx, t: usize, c: usize, (r, fixed): (i64, bool)) -> Scalar {
        let sheet = &self.program.sheets[t];
        if r < 0 || r >= sheet.nrows as i64 {
            return Ok(None);
        }
        let worksheet = self.worksheet(t, cx)?;
        let d = if fixed { "$" } else { "" };
        let kind = match self.engine.cell_shown(t, c, r as usize) {
            Value::Int(_, _)
            | Value::Decimal(_)
            | Value::Fraction(_)
            | Value::Percent(_)
            | Value::Float(_) => Kind::Number,
            Value::String(..) => Kind::Text,
            Value::Boolean(_) => Kind::Bool,
            Value::Date(_) => Kind::Date,
            Value::Time(_) => Kind::Time,
            Value::DateTime(_) | Value::Zoned(_) => Kind::DateTime,
            Value::Null => kind_of(sheet.cols[c].ty),
            _ => Kind::Unknown,
        };
        Ok(Some(Expr {
            text: format!("{worksheet}{d}{}{d}{}", letters(c), r + 2),
            prec: ATOM,
            kind,
            form: Form::Cell,
        }))
    }

    /// A block of cells: columns side by side, over the rows from one to
    /// another.
    fn area(
        &self,
        cx: &Cx,
        t: usize,
        cols: &[usize],
        (lo, hi): ((i64, bool), (i64, bool)),
    ) -> Result<String, Why> {
        let sheet = &self.program.sheets[t];
        let (first, last) = match cols {
            [] if !sheet.cols.is_empty() => (0, sheet.cols.len() - 1),
            [first, .., last] if cols.windows(2).all(|w| w[1] == w[0] + 1) => (*first, *last),
            [one] => (*one, *one),
            _ => return Err(COLUMNS),
        };
        let (lo, hi) = (
            (lo.0.max(0), lo.1),
            (hi.0.min(sheet.nrows as i64 - 1), hi.1),
        );
        if lo.0 > hi.0 {
            return Err(NO_ROWS);
        }
        let worksheet = self.worksheet(t, cx)?;
        let end = |c: usize, (r, fixed): (i64, bool)| {
            let d = if fixed { "$" } else { "" };
            format!("{d}{}{d}{}", letters(c), r + 2)
        };
        Ok(format!("{worksheet}{}:{}", end(first, lo), end(last, hi)))
    }

    /// The whole of a column.
    fn column(&self, cx: &Cx, t: usize, c: usize) -> Result<String, Why> {
        let last = self.program.sheets[t].nrows as i64 - 1;
        self.area(cx, t, &[c], ((0, true), (last, true)))
    }

    fn rows<'n>(&self, sel: Option<&'n RowSel>, t: usize, cx: &Cx) -> Result<Rows<'n>, Why> {
        let n = self.program.sheets[t].nrows as i64;
        let fixed = |node: &Node| {
            let at = whole(self.program, node).ok_or(ROW)?;
            Ok::<_, Why>((if at < 0 { at + n } else { at }, true))
        };
        let moved = |off: &Offset| match &off.0 {
            None => Ok((cx.row as i64, false)),
            Some((negative, node)) => {
                let by = whole(self.program, node).ok_or(ROW)?;
                Ok::<_, Why>((cx.row as i64 + if *negative { -by } else { by }, false))
            }
        };
        Ok(match sel {
            None | Some(RowSel::All) => Rows::Span((0, true), (n - 1, true)),
            Some(RowSel::Pos(node)) => {
                let (at, _) = fixed(node)?;
                Rows::One(at, true)
            }
            Some(RowSel::Cursor(off)) => {
                let (at, _) = moved(off)?;
                Rows::One(at, false)
            }
            Some(RowSel::Range {
                lo,
                hi,
                after,
                exclusive,
            }) => {
                let end = |bound: &Bound| match bound {
                    Bound::Abs(node) => fixed(node),
                    Bound::Cursor(off) => moved(off),
                };
                let (lo, hi) = (end(lo)?, end(hi)?);
                Rows::Span(
                    (lo.0 + *after as i64, lo.1),
                    (hi.0 - *exclusive as i64, hi.1),
                )
            }
            Some(RowSel::Pred(node)) => Rows::Where(node),
            Some(RowSel::RangeVal(_) | RowSel::Mask(_)) => return Err(SELECTION),
            // The row before within a partition is a different number of
            // rows back on each row, which a reference copied down is not.
            Some(RowSel::CursorBy { .. }) => return Err(PARTITION),
            // A sheet's own order is not a thing a cell formula can say.
            Some(RowSel::Order { .. }) => return Err(SORTED),
        })
    }

    /// A condition as columns of the sheet and what each is to equal:
    /// `Kind == x and not Done` is `Kind` equal to `x`, and `Done` to `FALSE`.
    fn criteria(&self, node: &Node, cx: &Cx, out: &mut Vec<(usize, Expr)>) -> Result<(), Why> {
        let tested = |n: &Node| match &n.kind {
            Ir::RowCol { depth: 0, col } => Some(*col),
            _ => None,
        };
        let outer = Cx {
            depth: cx.depth + 1,
            ..*cx
        };
        match &node.kind {
            Ir::Binary(BinOp::And, a, b) => {
                self.criteria(a, cx, out)?;
                self.criteria(b, cx, out)?;
            }
            Ir::Binary(BinOp::Eq, a, b) => {
                let (col, other) = match (tested(a), tested(b)) {
                    (Some(col), None) => (col, b),
                    (None, Some(col)) => (col, a),
                    _ => return Err(CONDITION),
                };
                let value = self
                    .scalar(other, &outer)
                    .map_err(|_| CONDITION)?
                    .ok_or(CONDITION)?;
                out.push((col, value));
            }
            Ir::Unary(UnOp::Not, inner) => {
                let col = tested(inner).ok_or(CONDITION)?;
                out.push((col, Expr::atom("FALSE".into(), Kind::Bool)));
            }
            _ => {
                let col = tested(node).ok_or(CONDITION)?;
                out.push((col, Expr::atom("TRUE".into(), Kind::Bool)));
            }
        }
        Ok(())
    }

    /// `sum`, `avg`, `min`, `max` or `count` of a selection or a list.
    fn aggregate(&self, func: Func, args: &[Node], cx: &Cx) -> Scalar {
        let [arg] = args else { return Err(VECTOR) };
        let name = match func {
            Func::Sum => "SUM",
            Func::Mean => "AVERAGE",
            Func::Min => "MIN",
            Func::Max => "MAX",
            // `count` skips what is empty and `len` does not, which is
            // COUNTA and ROWS.
            Func::Len => "ROWS",
            _ => "COUNTA",
        };
        if let Ir::VecLit(items) = &arg.kind {
            let mut parts = Vec::new();
            let mut kind = None;
            for item in items {
                // What is empty is left out, as the file leaves it out.
                if let Some(e) = self.scalar(item, cx)? {
                    kind =
                        Some(kind.map_or(e.kind, |k| if k == e.kind { k } else { Kind::Unknown }));
                    parts.push(e.text);
                }
            }
            if parts.is_empty() || matches!(func, Func::Count | Func::Len) {
                return Err(VECTOR);
            }
            let kind = kind.unwrap_or(Kind::Unknown);
            return Ok(Some(Expr::atom(
                format!("{name}({})", parts.join(",")),
                kind,
            )));
        }
        let (t, sel, cols) = target(arg).ok_or(VECTOR)?;
        let sheet = &self.program.sheets[t];
        let kind = match (func, cols.as_slice()) {
            (Func::Count | Func::Len, _) => Kind::Number,
            (_, [c, ..]) => kind_of(sheet.cols[*c].ty),
            _ => return Err(VECTOR),
        };
        let text = match self.rows(sel, t, cx)? {
            Rows::One(at, fixed) => {
                let block = self.area(cx, t, &cols, ((at, fixed), (at, fixed)))?;
                format!("{name}({block})")
            }
            Rows::Span(lo, hi) => {
                if cols.is_empty() {
                    return Err(VECTOR);
                }
                format!("{name}({})", self.area(cx, t, &cols, (lo, hi))?)
            }
            Rows::Where(cond) => {
                let mut tests = Vec::new();
                self.criteria(cond, cx, &mut tests)?;
                let mut parts = Vec::new();
                for (col, value) in &tests {
                    parts.push(self.column(cx, t, *col)?);
                    parts.push(value.text.clone());
                }
                match (func, cols.as_slice()) {
                    (Func::Len, []) => format!("COUNTIFS({})", parts.join(",")),
                    (Func::Count, [c]) => {
                        let read = self.column(cx, t, *c)?;
                        format!("COUNTIFS({},{read},\"<>\")", parts.join(","))
                    }
                    (Func::Sum | Func::Mean, [c]) => {
                        let read = self.column(cx, t, *c)?;
                        let name = if func == Func::Sum {
                            "SUMIFS"
                        } else {
                            "AVERAGEIFS"
                        };
                        format!("{name}({read},{})", parts.join(","))
                    }
                    _ => return Err(CONDITION),
                }
            }
        };
        Ok(Some(Expr::atom(text, kind)))
    }

    /// One value read from a sheet: a cell, or the row a condition finds.
    fn read(&self, node: &Node, cx: &Cx) -> Scalar {
        let (t, sel, cols) = target(node).ok_or(VECTOR)?;
        let [c] = cols.as_slice() else {
            return Err(VECTOR);
        };
        match self.rows(sel, t, cx)? {
            Rows::One(at, fixed) => self.cell(cx, t, *c, (at, fixed)),
            Rows::Span(..) => Err(VECTOR),
            Rows::Where(cond) => {
                let mut tests = Vec::new();
                self.criteria(cond, cx, &mut tests)?;
                let [(key, value)] = tests.as_slice() else {
                    return Err(CONDITION);
                };
                let (read, keys) = (self.column(cx, t, *c)?, self.column(cx, t, *key)?);
                Ok(Some(Expr {
                    text: format!("INDEX({read},MATCH({},{keys},0))", value.text),
                    prec: ATOM,
                    kind: kind_of(self.program.sheets[t].cols[*c].ty),
                    form: Form::Lookup,
                }))
            }
        }
    }

    fn literal(&self, lit: &Lit) -> Result<Expr, Why> {
        let number = |text: String| {
            let prec = if text.starts_with('-') { NEG } else { ATOM };
            Expr::new(text, prec, Kind::Number)
        };
        let ymd = |days: i32| {
            let (y, m, d) = date::civil_from_days(days as i64);
            format!("DATE({y},{m},{d})")
        };
        // A time is held in nanoseconds, and a workbook's `TIME` takes the
        // three parts of the clock.
        let hms = |nanos: i64| {
            let (h, m, s, _) = date::hms(nanos);
            format!("TIME({h},{m},{s})")
        };
        Ok(match lit {
            // A workbook holds fifteen digits.
            Lit::Int(n, _) if n.to_string().trim_start_matches('-').len() <= 15 => {
                number(n.to_string())
            }
            Lit::Int(_, _) => return Err(LITERAL),
            Lit::Decimal(r) | Lit::Percent(r) if !r.is_long() => number(format_rat(r)),
            Lit::Decimal(r) | Lit::Percent(r) => number(r.to_plain_string()),
            Lit::Float(f) if f.is_finite() => {
                let size = f.abs();
                number(if size == 0.0 || (1e-5..1e15).contains(&size) {
                    format!("{f}")
                } else {
                    format!("{f:E}")
                })
            }
            Lit::Float(_) => return Err(LITERAL),
            Lit::String(s) if s.chars().count() <= LONGEST_TEXT => {
                Expr::atom(format!("\"{}\"", s.replace('"', "\"\"")), Kind::Text)
            }
            Lit::String(_) => return Err(LITERAL),
            Lit::Boolean(b) => Expr::atom(if *b { "TRUE" } else { "FALSE" }.into(), Kind::Bool),
            Lit::Date(d) => Expr::atom(ymd(*d), Kind::Date),
            Lit::Time(t) => Expr::atom(hms(*t), Kind::Time),
            Lit::DateTime(t) => {
                let (days, time) = date::split_datetime(*t);
                if time == 0 {
                    Expr::atom(ymd(days), Kind::DateTime)
                } else {
                    Expr::new(format!("{}+{}", ymd(days), hms(time)), ADD, Kind::DateTime)
                }
            }
        })
    }

    /// `+ - * / **`. A workbook counts a time in days where a file counts
    /// it in seconds.
    fn arithmetic(&self, op: BinOp, a: Expr, b: Expr) -> Result<Expr, Why> {
        use Kind::{Date, DateTime, Number, Time};
        let clock = |k: Kind| matches!(k, Time | DateTime);
        let seconds = |e: &Expr| format!("{}/86400", e.within(POW));
        let (sym, prec) = match op {
            BinOp::Add => ("+", ADD),
            BinOp::Sub => ("-", ADD),
            BinOp::Mul => ("*", MUL),
            BinOp::Div => ("/", MUL),
            _ => ("^", POW),
        };
        let plain = |kind: Kind| {
            // The left of `^` too: a workbook reads `2^3^2` from the left.
            let left = if op == BinOp::Pow { prec + 1 } else { prec };
            Expr::new(
                format!("{}{sym}{}", a.within(left), b.within(prec + 1)),
                prec,
                kind,
            )
        };
        Ok(match (op, a.kind, b.kind) {
            (_, Number, Number) => plain(Number),
            (BinOp::Add, Date, Number) | (BinOp::Sub, Date, Number) => plain(Date),
            (BinOp::Add, Number, Date) => plain(Date),
            (BinOp::Add, Date, Time) | (BinOp::Add, Time, Date) => plain(DateTime),
            (BinOp::Sub, Date, Date) => plain(Number),
            (BinOp::Add | BinOp::Sub, k, Number) if clock(k) => {
                Expr::new(format!("{}{sym}{}", a.within(ADD), seconds(&b)), ADD, k)
            }
            (BinOp::Add, Number, k) if clock(k) => {
                Expr::new(format!("{}+{}", seconds(&a), b.within(ADD + 1)), ADD, k)
            }
            (BinOp::Sub, k, j) if clock(k) && k == j => Expr::atom(
                format!("ROUND(({}-{})*86400,0)", a.within(ADD), b.within(ADD + 1)),
                Number,
            ),
            _ => return Err(TYPE),
        })
    }

    fn call(&self, func: Func, args: &[Node], cx: &Cx) -> Scalar {
        if matches!(
            func,
            Func::Sum | Func::Mean | Func::Min | Func::Max | Func::Count | Func::Len
        ) {
            return self.aggregate(func, args, cx);
        }
        let named = |name: &str, kind: Kind| Ok(Some(Expr::atom(format!("{name}()"), kind)));
        match func {
            Func::Today => return named("TODAY", Kind::Date),
            Func::Now => return named("NOW", Kind::DateTime),
            Func::Pi => return named("PI", Kind::Number),
            Func::E => return Ok(Some(Expr::atom("EXP(1)".into(), Kind::Number))),
            _ => {}
        }
        // `round(x, n)` is Excel's own `ROUND(x,n)`.
        if matches!(func, Func::RoundDigits) {
            let [x, places] = args else {
                return Err(FUNCTION);
            };
            let (Some(x), Some(n)) = (self.scalar(x, cx)?, self.scalar(places, cx)?) else {
                return Ok(None);
            };
            if x.kind != Kind::Number || n.kind != Kind::Number {
                return Err(TYPE);
            }
            return Ok(Some(Expr::atom(
                format!("ROUND({},{})", x.text, n.text),
                Kind::Number,
            )));
        }
        let [arg] = args else { return Err(FUNCTION) };
        let Some(x) = self.scalar(arg, cx)? else {
            return Ok(None);
        };
        let of = |name: &str, kind: Kind| Expr::atom(format!("{name}({})", x.text), kind);
        let number = x.kind == Kind::Number;
        let dated = matches!(x.kind, Kind::Date | Kind::DateTime);
        let timed = matches!(x.kind, Kind::Time | Kind::DateTime);
        Ok(Some(match func {
            Func::Year if dated => of("YEAR", Kind::Number),
            Func::Month if dated => of("MONTH", Kind::Number),
            Func::Day if dated => of("DAY", Kind::Number),
            // A file counts the days of the week from Monday.
            Func::Weekday if dated => Expr::atom(format!("WEEKDAY({},2)", x.text), Kind::Number),
            Func::Hour if timed => of("HOUR", Kind::Number),
            Func::Minute if timed => of("MINUTE", Kind::Number),
            Func::Second if timed => of("SECOND", Kind::Number),
            Func::DateOf | Func::To(S::Date) if x.kind == Kind::DateTime => of("INT", Kind::Date),
            Func::TimeOf | Func::To(S::Time) if x.kind == Kind::DateTime => {
                Expr::atom(format!("MOD({},1)", x.text), Kind::Time)
            }
            Func::To(S::DateTime) if x.kind == Kind::Date => Expr {
                kind: Kind::DateTime,
                ..x.clone()
            },
            Func::To(S::Int | S::Hex | S::Octal | S::Binary) if number => {
                of("TRUNC", Kind::Number)
            }
            Func::To(S::Decimal | S::Percent | S::Float) if number => x.clone(),
            Func::To(S::Int | S::Decimal | S::Float) if x.kind == Kind::Bool => {
                Expr::atom(format!("IF({},1,0)", x.text), Kind::Number)
            }
            Func::To(S::Boolean) if number => {
                Expr::new(format!("{}<>0", x.within(COMPARE + 1)), COMPARE, Kind::Bool)
            }
            Func::Math(m) if number => {
                let name = match m {
                    MathFn::Abs => "ABS",
                    MathFn::Sign => "SIGN",
                    MathFn::Sqrt => "SQRT",
                    MathFn::Exp => "EXP",
                    MathFn::Ln => "LN",
                    MathFn::Log10 => "LOG10",
                    MathFn::Sin => "SIN",
                    MathFn::Cos => "COS",
                    MathFn::Tan => "TAN",
                    MathFn::Arcsin => "ASIN",
                    MathFn::Arccos => "ACOS",
                    MathFn::Arctan => "ATAN",
                    MathFn::Sinh => "SINH",
                    MathFn::Cosh => "COSH",
                    MathFn::Tanh => "TANH",
                    MathFn::Radians => "RADIANS",
                    MathFn::Degrees => "DEGREES",
                    // A half goes away from zero, as in a file.
                    MathFn::Round => {
                        return Ok(Some(Expr::atom(
                            format!("ROUND({},0)", x.text),
                            Kind::Number,
                        )));
                    }
                    // `INT` goes down, whatever the sign.
                    MathFn::Floor => "INT",
                    MathFn::Ceil => {
                        return Ok(Some(Expr::new(
                            format!("-INT(-{})", x.within(ATOM)),
                            NEG,
                            Kind::Number,
                        )));
                    }
                    MathFn::Log2 => {
                        return Ok(Some(Expr::atom(format!("LOG({},2)", x.text), Kind::Number)));
                    }
                };
                of(name, Kind::Number)
            }
            Func::Year
            | Func::Month
            | Func::Day
            | Func::Weekday
            | Func::Hour
            | Func::Minute
            | Func::Second
            | Func::DateOf
            | Func::TimeOf
            | Func::Math(_)
                if x.kind == Kind::Unknown =>
            {
                return Err(TYPE);
            }
            _ => return Err(FUNCTION),
        }))
    }

    /// A part of a formula that gives one value. `None` if it is known to
    /// give nothing on this row, as the row before the first does.
    fn scalar(&self, node: &Node, cx: &Cx) -> Scalar {
        match &node.kind {
            Ir::Lit(lit) => self.literal(lit).map(Some),
            // A sequence knows its own row, which a workbook formula
            // copied down cannot say.
            Ir::RowIndex { .. } | Ir::Empty => Err(SEQUENCE),
            // Only an aggregate of `agg` reads a group, and one is never
            // written as a formula.
            Ir::GroupCol(_) => Err(OTHER_SHEET),
            Ir::Const(i) => {
                let apart = Cx {
                    depth: 0,
                    args: &[],
                    ..*cx
                };
                self.scalar(&self.program.consts[*i].node, &apart)
            }
            // A column of the row the formula is on. Inside a condition, a
            // column of the row being tested cannot be read on its own.
            Ir::RowCol { depth, col } if *depth == cx.depth => {
                self.cell(cx, cx.sheet, *col, (cx.row as i64, false))
            }
            Ir::RowCol { .. } => Err(CONDITION),
            Ir::Arg(i) => cx.args.get(*i).cloned().ok_or(FUNCTION),
            Ir::Single(inner) => self.scalar(inner, cx),
            Ir::Column { .. } | Ir::Index { .. } | Ir::Field { .. } => self.read(node, cx),
            Ir::Unary(UnOp::Neg, inner) => {
                let Some(x) = self.scalar(inner, cx)? else {
                    return Ok(None);
                };
                if x.kind != Kind::Number {
                    return Err(TYPE);
                }
                Ok(Some(Expr::new(
                    format!("-{}", x.within(ATOM)),
                    NEG,
                    Kind::Number,
                )))
            }
            Ir::Unary(UnOp::Not, inner) => Ok(self
                .scalar(inner, cx)?
                .map(|x| Expr::atom(format!("NOT({})", x.text), Kind::Bool))),
            Ir::Unary(UnOp::UpTo, _) => Err(OPERATOR),
            Ir::Unary(UnOp::BitNot, _) => Err(BITWISE),
            Ir::Binary(op, ..) if op.is_bitwise() => Err(BITWISE),
            Ir::Binary(BinOp::Fallback, a, b) => {
                let (a, b) = (self.scalar(a, cx)?, self.scalar(b, cx)?);
                let Some(a) = a else { return Ok(b) };
                let kind = match &b {
                    Some(b) if b.kind != a.kind => Kind::Unknown,
                    _ => a.kind,
                };
                let other = b.map_or("\"\"".to_string(), |b| b.text);
                let text = match a.form {
                    Form::Cell => format!("IF(ISBLANK({0}),{other},{0})", a.text),
                    Form::Lookup => format!("IFERROR({},{other})", a.text),
                    Form::Calc => return Err(FALLBACK),
                };
                Ok(Some(Expr::atom(text, kind)))
            }
            Ir::Binary(op, ..) if *op == BinOp::In || op.range_ends().is_some() => Err(OPERATOR),
            Ir::Binary(op, a, b) => {
                let (Some(a), Some(b)) = (self.scalar(a, cx)?, self.scalar(b, cx)?) else {
                    return Ok(None);
                };
                if op.is_arithmetic() {
                    return self.arithmetic(*op, a, b).map(Some);
                }
                let both =
                    |name: &str| Expr::atom(format!("{name}({},{})", a.text, b.text), Kind::Bool);
                let text = a.kind == Kind::Text || b.kind == Kind::Text;
                let compare = |sym: &str| {
                    let sides = (a.within(COMPARE + 1), b.within(COMPARE + 1));
                    Expr::new(format!("{}{sym}{}", sides.0, sides.1), COMPARE, Kind::Bool)
                };
                Ok(Some(match op {
                    BinOp::And => both("AND"),
                    BinOp::Or => both("OR"),
                    // A workbook's `=` takes `a` and `A` for the same text.
                    BinOp::Eq if text => both("EXACT"),
                    BinOp::Ne if text => {
                        Expr::atom(format!("NOT({})", both("EXACT").text), Kind::Bool)
                    }
                    BinOp::Eq => compare("="),
                    BinOp::Ne => compare("<>"),
                    BinOp::Lt => compare("<"),
                    BinOp::Gt => compare(">"),
                    BinOp::Le => compare("<="),
                    BinOp::Ge => compare(">="),
                    _ => return Err(OPERATOR),
                }))
            }
            Ir::If(cond, a, b) => {
                let Some(cond) = self.scalar(cond, cx)? else {
                    return Ok(None);
                };
                let (a, b) = (self.scalar(a, cx)?, self.scalar(b, cx)?);
                let kind = match (&a, &b) {
                    (Some(a), Some(b)) if a.kind == b.kind => a.kind,
                    _ => Kind::Unknown,
                };
                let part = |e: Option<Expr>| e.map_or("\"\"".to_string(), |e| e.text);
                Ok(Some(Expr::atom(
                    format!("IF({},{},{})", cond.text, part(a), part(b)),
                    kind,
                )))
            }
            Ir::Call(func, args) => self.call(*func, args, cx),
            Ir::Apply { args, body, .. } => {
                let mut given = Vec::with_capacity(args.len());
                for arg in args {
                    given.push(self.scalar(arg, cx)?);
                }
                let inside = Cx {
                    args: &given,
                    ..*cx
                };
                self.scalar(body, &inside)
            }
            Ir::VecLit(_) | Ir::Sheet(_) => Err(VECTOR),
            Ir::Error => Err(FUNCTION),
        }
    }
}

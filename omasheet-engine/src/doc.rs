// Copyright (c) 2026 Stephen Roe

//! An editable file for interactive front ends.
//!
//! The `.omx` text stays the single source of truth: every edit rewrites the
//! text and the file is then re-read and re-evaluated. A [`Snapshot`] is what
//! a grid shows: for each cell, the value to display and the source to edit.
//!
//! Unlike the command line, a document is evaluated even when it has errors,
//! so that one mistyped formula does not blank the rest of the grid; cells
//! that could not be calculated show as errors.

use crate::Options;
use crate::datefmt::Names;
use crate::eval::Engine;
use crate::table::Copying;
use crate::value::{Value, arith, delocalised, format_exact, format_styled};
use omasheet_omx::ast::{BinOp, Expr, ExprKind, Lit, UnOp};
use omasheet_omx::colfmt::{Format, closing};
use omasheet_omx::date::{self, Style};
use omasheet_omx::file::{FileAst, SchemaLine, SheetDecl, ident_len, parse_file, split_cells};
use omasheet_omx::parser::parse_expr;
use omasheet_omx::types::Base;
use omasheet_omx::{ColKind, Diagnostic, S, Sources, Span, compile, text};
use std::cmp::Ordering;
use std::sync::Arc;

const UNDO_LIMIT: usize = 200;

/// A sheet whose rows are not its own text and are more than this many is
/// paged: its cells are formatted as the grid shows them rather than all at
/// once (D80). One the text holds is formatted whole, since the text it is
/// edited through is that size already.
pub const PAGED_ROWS: usize = 10_000;

/// How many rows of a paged sheet's selection [`Document::summary`] reads
/// at a time. Small in tests, so that a selection of the Titanic file
/// crosses blocks.
const SUMMARY_ROWS: usize = if cfg!(test) { 100 } else { 16_384 };

/// A fresh file: one small empty sheet to type into.
pub const BLANK: &str = "sheet Sheet1\n\n| A   | B   | C   |\n| --- | --- | --- |\n|     |     |     |\n|     |     |     |\n|     |     |     |\n|     |     |     |\n|     |     |     |\n";

#[derive(Debug, Default, Clone)]
pub struct Snapshot {
    pub sheets: Vec<SheetSnap>,
    pub consts: Vec<ConstSnap>,
    pub funcs: Vec<FuncSnap>,
    pub problems: Vec<Problem>,
}

#[derive(Debug, Clone)]
pub struct SheetSnap {
    pub name: String,
    pub columns: Vec<ColumnSnap>,
    /// Every row, formatted; empty for a paged sheet, whose rows are asked
    /// for with [`Document::rows`].
    pub rows: Vec<Vec<CellSnap>>,
    /// How many rows the sheet has, paged or not.
    pub nrows: usize,
    /// Its rows are formatted as they are asked for, not held here: a big
    /// sheet whose rows are a file's or another sheet's (D80).
    pub paged: bool,
    /// The Parquet file the rows are read from, for a `table`; `None` for a
    /// sheet that holds its own rows. A `view` is read-only: the file
    /// is never written back, so no edit of it is taken (D49).
    pub source: Option<String>,
    /// The sheet is written as `:=` lines alone, so it has no text behind a
    /// cell and no row of its own to change: it is shown and not edited,
    /// as a table is.
    pub derived: bool,
    /// The expression a sheet computed from one is written with, `Sales |>
    /// head(3)`, for the tab to mark it and say what it is. `None` for a
    /// sheet whose rows are its own or a file's (D90).
    pub view: Option<String>,
    /// The sheet is made by `group_by` and `agg`, or by `join`: rows of
    /// its own, and a formula a sort cannot be added to the end of, so it
    /// is sorted in the grid alone, as a table is.
    pub grouped: bool,
}

#[derive(Debug, Clone)]
pub struct ColumnSnap {
    pub name: String,
    /// The column type, or empty when it is not known.
    pub ty: String,
    /// The type is written in the file, as `Name : Type`, and its cells
    /// are held to it.
    pub declared: bool,
    /// The `:=` expression of a computed column; `None` for a data column.
    pub formula: Option<String>,
    /// How its values are shown, as its type line writes it: `'.2f`,
    /// `%d %b %Y`.
    pub format: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CellSnap {
    /// The calculated value.
    pub value: Value,
    /// The value as shown in the grid.
    pub display: String,
    /// The value in full, as a file writes it, when it is shown otherwise.
    pub exact: Option<String>,
    /// Whether it is shown with fewer digits than it has, which grouping
    /// them, or the locale's point, does not make it.
    pub shortened: bool,
    /// What the cell holds in the file: a literal, or `= expression`. For a
    /// computed column, the column's expression.
    pub source: String,
    pub numeric: bool,
    pub formula: bool,
    /// Why the cell could not be calculated, if it could not.
    pub error: Option<String>,
}

/// What a block of cells adds up to.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// How many of the cells hold a value.
    pub count: usize,
    /// The sum and the mean of the numbers among them, as text; `None`
    /// if there are no numbers.
    pub numbers: Option<(String, String)>,
}

/// A function the file defines.
#[derive(Debug, Clone)]
pub struct FuncSnap {
    pub name: String,
    /// How it is called, such as `Margin(revenue, cost)`.
    pub usage: String,
    /// The expression it stands for.
    pub source: String,
    /// The comment written above it.
    pub doc: String,
}

#[derive(Debug, Clone)]
pub struct ConstSnap {
    pub name: String,
    pub source: String,
    pub display: String,
    /// The value in full, when it is shown with fewer digits than it has.
    pub exact: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Problem {
    pub line: usize,
    pub column: usize,
    pub message: String,
    /// The cell the problem is in, as (sheet, row, column), for going
    /// straight to it. `None` for a problem no cell claimed: one in the
    /// text around the sheets, or in a sheet that did not compile.
    pub cell: Option<(usize, usize, usize)>,
}

/// The grid's order of each sorted table, shared between the document and
/// the steps of its history, so that a step keeps an order without copying
/// it.
type Orders = std::collections::HashMap<String, Arc<Vec<usize>>>;

/// A state of the document to undo or redo to: its text, and the order the
/// grid shows each table's rows in, which a sort changes without the text.
struct Step {
    text: String,
    orders: Orders,
}

/// A table's file the document could hold whole, and what that would take
/// (D157): the columns its sheet reads, and their size in memory as the
/// file's footer gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToHold {
    pub path: std::path::PathBuf,
    pub columns: Vec<String>,
    pub bytes: usize,
}

pub struct Document {
    text: String,
    snapshot: Snapshot,
    undo: Vec<Step>,
    redo: Vec<Step>,
    style: Style,
    /// What the locale calls the days and months, for a column's format.
    names: Names,
    /// Show date-times on the clocks of this machine.
    local: bool,
    /// The directory a `table`'s source is read relative to: the directory
    /// of the file, not the one the program was started in. `None` is the
    /// working directory. See [`Document::set_base`].
    base: Option<std::path::PathBuf>,
    /// The order a table's rows are shown in, by sheet name, where one has
    /// been sorted: the rows of its file, by number. A table's rows are not
    /// the text's, so this is a view of them -- it is not written or saved --
    /// and the grid, a copy and the summary of a selection all read the
    /// snapshot it orders. A sort is a step to undo, as an edit is.
    orders: Orders,
    /// The tables' files held whole in memory, by path (D157). Kept across
    /// rebuilds, and dropped when the text no longer names the file or the
    /// file has changed. See [`Document::to_hold`] and [`Document::hold`].
    whole: crate::table::Store,
    /// Whether a table over a CSV with no copy yet waits for one or shows
    /// its sample meanwhile.
    copying: Copying,
    /// How many edits have been committed. See [`Document::edit_count`].
    edits: usize,
    /// The engine of the last rebuild, kept so that a paged sheet's cells
    /// can be calculated and formatted when they are shown (D80).
    engine: Option<Engine<'static>>,
    /// Why each computed column of each sheet failed, for a paged sheet's
    /// cells, which are made after the rebuild that knew.
    column_errors: Vec<Vec<Option<String>>>,
    /// More rows than this and a sheet the text does not hold is paged.
    /// [`PAGED_ROWS`], but for a test.
    paged_rows: usize,
}

impl Default for Document {
    fn default() -> Self {
        Document::from_text(BLANK)
    }
}

/// A sheet's expression without the `|> sort(…)` it ends in, if it ends in
/// one: a sort's arguments are names and a bare word, so its parentheses
/// hold no others.
fn without_last_sort(expr: &str) -> &str {
    let trimmed = expr.trim_end();
    let Some(body) = trimmed.strip_suffix(')') else {
        return trimmed;
    };
    let Some(open) = body.rfind('(') else {
        return trimmed;
    };
    let head = body[..open].trim_end();
    let Some(head) = head.strip_suffix("sort") else {
        return trimmed;
    };
    // `sort` itself, and not the end of a longer name such as `resort`.
    if head.ends_with(|c: char| c.is_alphanumeric() || c == '_') {
        return trimmed;
    }
    match head.trim_end().strip_suffix("|>") {
        Some(rest) => rest.trim_end(),
        None => trimmed,
    }
}

/// The narrowest kind of text every cell of a column reads as. `None`
/// where they are not all text, not all one kind of it, or there are none.
///
/// This is the app's own reading, for what a column's type is shown as
/// where the file does not say. The checker is not told and the file is not
/// changed, so the column is still held to `String`: a column that gains an
/// odd value later is never in breach of a type it did not declare.
fn narrower_text_type(rows: &[Vec<CellSnap>], col: usize) -> Option<&'static str> {
    // One pass, and it gives up as soon as no kind is left: this is read
    // again for every column each time the file is rebuilt, which is once
    // per edit.
    let (mut url, mut email, mut uuid) = (true, true, true);
    let mut held_any = false;
    for row in rows {
        let held = match row.get(col).map(|cell| &cell.value) {
            Some(Value::String(held, _)) => held,
            // An empty cell says nothing either way.
            Some(Value::Null) => continue,
            _ => return None,
        };
        held_any = true;
        url = url && text::is_url(held);
        email = email && text::is_email(held);
        uuid = uuid && text::is_uuid(held);
        if !(url || email || uuid) {
            return None;
        }
    }
    if !held_any {
        return None;
    }
    // A `mailto:` address reads as both a web address and an email address,
    // so the web address is taken first.
    match (url, email, uuid) {
        (true, ..) => Some("Url"),
        (_, true, _) => Some("Email"),
        (.., true) => Some("Uuid"),
        _ => None,
    }
}

/// A value as a cell shows it where its column has no format.
fn shown_text(engine: &Engine, v: &Value) -> String {
    match v {
        Value::Sheet(_) => "<sheet>".to_string(),
        Value::Row(_) => "<row>".to_string(),
        other => engine.show(other, false),
    }
}

/// Why a sheet's data takes no edit, if it takes none: it is calculated
/// from an expression, or it is a `view` of a Parquet file (D49).
pub fn read_only_why(snap: &SheetSnap) -> Option<String> {
    if let Some(expr) = &snap.view {
        return Some(format!(
            "sheet `{}` is calculated from `{expr}`: change the sheets it reads, or its own formula",
            snap.name
        ));
    }
    let path = snap.source.as_ref()?;
    Some(format!(
        "view `{}` is read-only, reading `{path}`: import that file to make edits",
        snap.name
    ))
}

/// What a number shown with fewer digits than it has is in full.
fn exact_text(v: &Value, quoted: bool, shown: &str, style: &Style) -> Option<String> {
    let full = match v {
        // Two's complement is the whole number, not one cut short.
        Value::Int(_, b) if *b != Base::Dec => return None,
        Value::Int(_, _)
        | Value::Decimal(_)
        | Value::Percent(_)
        | Value::Float(_)
        | Value::Complex(..)
        | Value::Vector(_) => format_exact(v, quoted, style),
        _ => return None,
    };
    (full != shown).then_some(full)
}

/// A cell as the grid shows it: its value formatted as its column says.
fn cell_snap(
    engine: &Engine,
    style: &Style,
    format: Option<&Format>,
    value: Value,
    source: String,
    error: Option<String>,
) -> CellSnap {
    let failed = matches!(value, Value::Error);
    // A column's format shows its numbers as it says, so one it rounds is
    // not one shown short.
    let display = match format {
        Some(_) => engine.show_in(&value, false, format),
        None => shown_text(engine, &value),
    };
    let full = exact_text(&value, false, &display, style);
    CellSnap {
        shortened: format.is_none()
            && full
                .as_ref()
                .is_some_and(|f| *f != delocalised(&display, style)),
        exact: full,
        display,
        numeric: value.is_numeric(),
        value,
        formula: source.starts_with('='),
        error: error.or_else(|| failed.then(|| "this cell could not be calculated".to_string())),
        source,
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    inner.start >= outer.start && inner.start <= outer.end
}

/// A date or time typed the way `style` writes it (`31/1/2025`, `9:30 pm`),
/// as a file writes it, for a column of type `ty`. `None` if it is not one.
fn iso_entry(style: &Style, ty: &str, input: &str) -> Option<String> {
    let text = input.trim();
    let date = || style.parse_date(text).map(date::format);
    let time = || style.parse_time(text).map(date::format_time);
    let datetime = || style.parse_datetime(text).map(date::format_datetime);
    match ty {
        "Date" => date(),
        "Time" => time(),
        // A date alone is the start of that day.
        "Datetime" => datetime().or_else(|| Some(format!("{}T00:00", date()?))),
        // With no type to go by, only what cannot be anything else: a date
        // needs its year in full.
        "" => {
            let full_year = text
                .split(|c: char| !c.is_ascii_digit())
                .any(|part| part.len() == 4);
            datetime().or_else(date).filter(|_| full_year).or_else(time)
        }
        _ => None,
    }
}

/// What can be typed into a cell of a column of type `ty`, with an example
/// written the way `style` reads it. Empty when the type is not known.
pub fn entry_hint(style: &Style, ty: &str) -> String {
    // A day past the twelfth, so the order of day and month is plain.
    let day = date::from_ymd(2025, 1, 31).unwrap_or(0);
    let time = date::time_from(17, 5, 0).unwrap_or(0);
    let also_iso = if style.is_iso() { "" } else { " or ISO" };
    match ty {
        "Integer" => "Integer: a whole number, e.g. 42".into(),
        "Decimal" => "Decimal: an exact decimal number, e.g. 19.99 or 20%".into(),
        "Percent" => "Percent: an exact number shown as a percentage, e.g. 20%".into(),
        "Float" => "Float: a floating-point number, e.g. 1.5, 2e-3 or Inf".into(),
        "Complex" => "Complex: a real and an imaginary part, e.g. 3+4i or 2i".into(),
        "Boolean" => "Boolean: true or false".into(),
        "String" => "String".into(),
        "Url" => "Url: a web address, e.g. https://example.com".into(),
        "Email" => "Email: an email address, e.g. name@example.com".into(),
        "Uuid" => "Uuid: a UUID, e.g. 7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f".into(),
        "Date" => format!(
            "Date: {}, e.g. {}{also_iso}",
            style.date_pattern(),
            style.date(day)
        ),
        "Time" => format!("Time: e.g. {}", style.time(time)),
        "Datetime" => format!(
            "Datetime: e.g. {}{also_iso}",
            style.datetime(day as i64 * date::DAY_MICROS + time / date::NANOS_PER_MICRO)
        ),
        "Duration" => "Duration: PnDTnHnMnS, e.g. PT1H30M".into(),
        _ => String::new(),
    }
}

/// A row or column picked by counting, as a formula writes it.
#[derive(Clone, Copy, PartialEq)]
enum Pick {
    /// `*`: the formula's own row or column.
    Here,
    /// `*+n` or `*-n`.
    Off(i64),
    /// A number; a negative one counts from the end.
    At(i64),
}

impl Pick {
    fn of(e: &Expr) -> Option<Pick> {
        let int = |e: &Expr| match &e.kind {
            ExprKind::Lit(Lit::Int(k, _)) => Some(*k),
            _ => None,
        };
        match &e.kind {
            ExprKind::Cursor => Some(Pick::Here),
            ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub), l, r)
                if matches!(l.kind, ExprKind::Cursor) =>
            {
                let by = int(r)?;
                Some(Pick::Off(if *op == BinOp::Sub { -by } else { by }))
            }
            ExprKind::Unary(UnOp::Neg, a) => int(a).map(|k| Pick::At(-k)),
            _ => int(e).map(Pick::At),
        }
    }

    /// The position picked out of `n`, from a formula at `own`. It may be
    /// off either end.
    fn at(self, own: Option<usize>, n: usize) -> Option<i64> {
        Some(match self {
            Pick::Here => own? as i64,
            Pick::Off(d) => own? as i64 + d,
            Pick::At(k) if k < 0 => k + n as i64,
            Pick::At(k) => k,
        })
    }

    /// Position `to` of `n`, written the way this pick was, from a formula
    /// at `own`.
    fn write(self, to: i64, own: Option<usize>, n: usize) -> String {
        match self {
            Pick::At(k) if k < 0 && to < n as i64 => (to - n as i64).to_string(),
            Pick::At(_) => to.to_string(),
            Pick::Here | Pick::Off(_) => offset_text(to - own.unwrap_or(0) as i64),
        }
    }
}

/// `*`, `*+2` or `*-1`.
fn offset_text(by: i64) -> String {
    match by {
        0 => "*".to_string(),
        _ => format!("*{by:+}"),
    }
}

/// The two ends of a range, and whether it starts after the first and stops
/// short of the second.
fn range_ends(e: &Expr) -> Option<(&Expr, &Expr, bool, bool)> {
    match &e.kind {
        ExprKind::Binary(op, lo, hi) => {
            let (after, exclusive) = op.range_ends()?;
            Some((lo, hi, after, exclusive))
        }
        _ => None,
    }
}

/// How an index names its columns.
enum ColPart<'a> {
    /// Every column.
    All,
    /// The first slot of `[columns; rows]`.
    Slot(&'a Expr),
    /// A name after the index or before it: `T[0].Name`, `T.Name[0]`.
    Field(&'a str, Span),
}

/// One index into a sheet: `T[columns; rows]` and its other spellings.
struct Site<'a> {
    /// The sheet as written; `None` for an index with no sheet name.
    sheet: Option<&'a str>,
    row: Option<&'a Expr>,
    col: ColPart<'a>,
    /// Inside a slot of another index, where the current row is a row of
    /// that index and not the formula's own.
    nested: bool,
    /// The whole index as written.
    at: Span,
}

/// The sheet an index is into, if its base says so: `Some(None)` for an
/// index with no sheet name.
fn base_sheet(e: &Expr) -> Option<Option<&str>> {
    match &e.kind {
        ExprKind::Own => Some(None),
        ExprKind::Name(name) => Some(Some(name)),
        _ => None,
    }
}

/// Every index into a sheet in `e`, and every name that stands alone, which
/// in a cell may be a column of the cell's own row.
fn sites<'a>(e: &'a Expr, nested: bool, out: &mut Vec<Site<'a>>, names: &mut Vec<(&'a str, Span)>) {
    match &e.kind {
        ExprKind::Lit(_) | ExprKind::Cursor | ExprKind::Own => {}
        // A back-reference names no column: it is this one.
        ExprKind::Back(_) => {}
        ExprKind::Seq { seeds, rule, limit } => {
            for seed in seeds {
                sites(seed, nested, out, names);
            }
            if let Some(rule) = rule {
                sites(rule, nested, out, names);
            }
            sites(limit, nested, out, names);
        }
        ExprKind::Name(name) => {
            if !nested {
                names.push((name, e.span));
            }
        }
        ExprKind::Unary(_, a) | ExprKind::Named(_, _, a) | ExprKind::By(a, _) => {
            sites(a, nested, out, names)
        }
        ExprKind::Binary(_, a, b) => {
            sites(a, nested, out, names);
            sites(b, nested, out, names);
        }
        ExprKind::If(a, b, c) => {
            sites(a, nested, out, names);
            sites(b, nested, out, names);
            sites(c, nested, out, names);
        }
        ExprKind::VecLit(items) | ExprKind::Call(_, _, items) => {
            for item in items {
                sites(item, nested, out, names);
            }
        }
        ExprKind::Field(a, name, span) => match &a.kind {
            // `T[0].Name`
            ExprKind::Index(base, slots)
                if slots.len() == 2 && slots[0].is_none() && base_sheet(base).is_some() =>
            {
                out.push(Site {
                    sheet: base_sheet(base).flatten(),
                    row: slots[1].as_ref(),
                    col: ColPart::Field(name, *span),
                    nested,
                    at: e.span,
                });
                if let Some(slot) = &slots[1] {
                    sites(slot, true, out, names);
                }
            }
            // `T.Name`: a sheet and its column, not a name on its own.
            ExprKind::Name(_) => {}
            _ => sites(a, nested, out, names),
        },
        ExprKind::Index(base, slots) => {
            if let Some(sheet) = base_sheet(base) {
                out.push(Site {
                    sheet,
                    row: slots.get(1).and_then(Option::as_ref),
                    col: match slots.first() {
                        Some(Some(slot)) => ColPart::Slot(slot),
                        _ => ColPart::All,
                    },
                    nested,
                    at: e.span,
                });
            } else if let ExprKind::Field(inner, name, span) = &base.kind
                && let ExprKind::Name(sheet) = &inner.kind
                && slots.len() == 1
            {
                // `T.Name[0]`
                out.push(Site {
                    sheet: Some(sheet),
                    row: slots[0].as_ref(),
                    col: ColPart::Field(name, *span),
                    nested,
                    at: e.span,
                });
            } else {
                sites(base, nested, out, names);
            }
            for slot in slots.iter().flatten() {
                sites(slot, true, out, names);
            }
        }
    }
}

/// Every place in `e` that the name `name` stands: alone, or as what is
/// indexed or read from.
fn named(e: &Expr, name: &str, out: &mut Vec<Span>) {
    match &e.kind {
        ExprKind::Lit(_) | ExprKind::Cursor | ExprKind::Own | ExprKind::Back(_) => {}
        ExprKind::Seq { seeds, rule, limit } => {
            for seed in seeds {
                named(seed, name, out);
            }
            if let Some(rule) = rule {
                named(rule, name, out);
            }
            named(limit, name, out);
        }
        ExprKind::Name(n) => {
            if n == name {
                out.push(e.span);
            }
        }
        ExprKind::Unary(_, a) | ExprKind::Named(_, _, a) | ExprKind::By(a, _) => {
            named(a, name, out)
        }
        ExprKind::Binary(_, l, r) => {
            named(l, name, out);
            named(r, name, out);
        }
        ExprKind::If(c, a, b) => {
            named(c, name, out);
            named(a, name, out);
            named(b, name, out);
        }
        ExprKind::VecLit(items) | ExprKind::Call(_, _, items) => {
            for item in items {
                named(item, name, out);
            }
        }
        ExprKind::Index(base, slots) => {
            named(base, name, out);
            for slot in slots.iter().flatten() {
                named(slot, name, out);
            }
        }
        ExprKind::Field(base, _, _) => named(base, name, out),
    }
}

/// Where a formula is: its sheet, and its row and column if it is in a cell.
#[derive(Clone, Copy)]
struct Place {
    sheet: Option<usize>,
    cell: Option<(usize, usize)>,
}

/// Every formula of a file, with spans into the text of the file.
fn formulas(ast: &FileAst) -> Vec<(Place, Expr)> {
    let mut out = Vec::new();
    let nowhere = Place {
        sheet: None,
        cell: None,
    };
    for c in &ast.consts {
        out.extend(c.expr.clone().map(|e| (nowhere, e)));
    }
    for f in &ast.funcs {
        out.extend(f.expr.clone().map(|e| (nowhere, e)));
    }
    for (t, decl) in ast.sheets.iter().enumerate() {
        for (r, row) in decl.rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if let Some(body) = cell.text.strip_prefix('=')
                    && let Ok(e) = parse_expr(body, 0, cell.span.start as usize + 1)
                {
                    let cell = Some((r, c));
                    out.push((
                        Place {
                            sheet: Some(t),
                            cell,
                        },
                        e,
                    ));
                }
            }
        }
        let place = Place {
            sheet: Some(t),
            cell: None,
        };
        for c in &decl.computed {
            out.extend(c.expr.clone().map(|e| (place, e)));
        }
    }
    out
}

/// Whether `site`, in a formula at `place`, is an index into sheet `t`; and
/// if so, whether a `*` in it is the formula's own cell.
fn into_sheet(site: &Site, place: Place, t: usize, name: &str) -> Option<bool> {
    let home = place.sheet == Some(t) && !site.nested;
    match site.sheet {
        Some(sheet) if sheet == name => Some(home && place.cell.is_some()),
        None if home => Some(place.cell.is_some()),
        _ => None,
    }
}

/// The names of the columns of a sheet, in order: those of the header row,
/// then any formula column it fails to name.
fn column_names(decl: &SheetDecl) -> Vec<&str> {
    let named = |name: &str| decl.header.iter().any(|h| h.0 == name);
    let header = decl.header.iter().map(|h| h.0.as_str());
    let rest = decl.computed.iter().map(|c| c.name.as_str());
    header.chain(rest.filter(|name| !named(name))).collect()
}

/// What a row holds for a formula column.
const FORMULA_CELL: &str = "*";

/// `(start, end, replacement)` in the text of a file.
type Edit = (usize, usize, String);

/// Note that `e` is to read `new`, unless it does already.
fn put(edits: &mut Vec<Edit>, text: &str, span: Span, new: String) {
    let (s, e) = (span.start as usize, span.end as usize);
    if text.get(s..e).is_some_and(|old| old != new) {
        edits.push((s, e, new));
    }
}

/// `text` with `edits` made, and the sheets they fall in lined up again.
fn rewritten(text: &str, ast: &FileAst, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    edits.dedup_by_key(|e| e.0);
    let mut out = text.to_string();
    let mut touched: Vec<&str> = Vec::new();
    for (s, e, new) in &edits {
        let owner = ast.sheets.iter().rfind(|t| t.span.start as usize <= *s);
        if let Some(t) = owner.filter(|t| !touched.contains(&t.name.as_str())) {
            touched.push(&t.name);
        }
        out.replace_range(*s..*e, new);
    }
    for name in touched {
        let ast = parse_file(&out, 0, &mut Vec::new());
        if let Some(decl) = ast.sheets.iter().find(|t| t.name == name) {
            out = Grid::of(decl).write(decl, &out);
        }
    }
    out
}

/// Keep one slot on the rows or columns it picks while they are rearranged:
/// `place[k]` is where the k-th ends up, of `count` in all once any are
/// added, and `own` is where the formula is.
/// A single pick stays on its row or column; a range follows its two ends.
fn follow_slot(
    slot: &Expr,
    own: Option<usize>,
    place: &[usize],
    count: usize,
    text: &str,
    edits: &mut Vec<Edit>,
) {
    let n = place.len();
    let moved = |k: i64| usize::try_from(k).ok().and_then(|k| place.get(k)).copied();
    let new_own = own.and_then(|k| place.get(k)).copied();
    if let Some((lo, hi, after, exclusive)) = range_ends(slot) {
        let (Some(first), Some(last)) = (Pick::of(lo), Pick::of(hi)) else {
            return;
        };
        let (skip, past) = (i64::from(after), i64::from(exclusive));
        let ends = first
            .at(own, n)
            .map(|k| k + skip)
            .zip(last.at(own, n).map(|k| k - past));
        let Some((a, b)) = ends.filter(|(a, b)| a <= b) else {
            return;
        };
        let (Some(a), Some(b)) = (moved(a), moved(b)) else {
            return;
        };
        let (a, b) = (a.min(b) as i64, a.max(b) as i64);
        let to = last.write(b + past, new_own, count);
        if matches!(first, Pick::At(_)) && a < skip {
            // Nothing comes before the first to start after: start on it.
            let op = if exclusive { "..^" } else { ".." };
            let from = first.write(a, new_own, count);
            put(edits, text, slot.span, format!("{from}{op}{to}"));
        } else {
            put(edits, text, lo.span, first.write(a - skip, new_own, count));
            put(edits, text, hi.span, to);
        }
    } else if let Some(pick) = Pick::of(slot).filter(|p| *p != Pick::Here)
        && let Some(to) = pick.at(own, n).and_then(moved)
    {
        put(
            edits,
            text,
            slot.span,
            pick.write(to as i64, new_own, count),
        );
    }
}

/// The edits that keep every row or column picked by counting on the row or
/// column it was, when those of sheet `t` are rearranged so that `place[k]`
/// is where the k-th ends up, of `count` in all. An offset in a formula column
/// is left alone:
/// one formula serves every row, so `*-1` goes on meaning the row before.
fn follow_move(
    ast: &FileAst,
    text: &str,
    t: usize,
    rows: bool,
    place: &[usize],
    count: usize,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    let name = ast.sheets[t].name.as_str();
    for (at, expr) in formulas(ast) {
        let mut found = Vec::new();
        sites(&expr, false, &mut found, &mut Vec::new());
        for site in found {
            let Some(relative) = into_sheet(&site, at, t, name) else {
                continue;
            };
            let slot = match (rows, &site.col) {
                (true, _) => site.row,
                (false, ColPart::Slot(slot)) => Some(*slot),
                (false, _) => None,
            };
            let own = at
                .cell
                .filter(|_| relative)
                .map(|(r, c)| if rows { r } else { c });
            if let Some(slot) = slot {
                follow_slot(slot, own, place, count, text, &mut edits);
            }
        }
    }
    edits
}

/// A block of cells: its rows and its columns.
type Block = (std::ops::Range<usize>, std::ops::Range<usize>);

/// The column a site names, if it names one: its index, how it is written
/// (`None` for a name) and where.
fn one_column(
    site: &Site,
    own: Option<usize>,
    cols: &[&str],
) -> Option<(usize, Option<Pick>, Span)> {
    let named = |name: &str, span: Span| Some((cols.iter().position(|c| *c == name)?, None, span));
    match &site.col {
        ColPart::All => None,
        ColPart::Field(name, span) => named(name, *span),
        ColPart::Slot(slot) => match &slot.kind {
            ExprKind::Name(name) => named(name, slot.span),
            _ => {
                let pick = Pick::of(slot)?;
                let at = usize::try_from(pick.at(own, cols.len())?).ok()?;
                (at < cols.len()).then_some((at, Some(pick), slot.span))
            }
        },
    }
}

/// The one cell of a sheet that `site` reads, if it reads just one: its row
/// and column, each with how it is written and where.
#[allow(clippy::type_complexity)]
fn one_cell(
    site: &Site,
    own: Option<(usize, usize)>,
    nrows: usize,
    cols: &[&str],
) -> Option<((usize, Pick, Span), (usize, Option<Pick>, Span))> {
    let slot = site.row?;
    let pick = Pick::of(slot)?;
    let row = usize::try_from(pick.at(own.map(|o| o.0), nrows)?).ok()?;
    let col = one_column(site, own.map(|o| o.1), cols)?;
    Some(((row, pick, slot.span), col))
}

/// The edits that keep each formula reading a cell of `block`, cut from
/// sheet `t`, on that cell once it is `by` rows and columns away. Only a
/// reference to one cell follows it: a range or a whole column reads the
/// place, and so does a formula column.
fn follow_cut(ast: &FileAst, text: &str, t: usize, block: &Block, by: (i64, i64)) -> Vec<Edit> {
    follow_cells(ast, text, t, block, &|r, c| {
        (r as i64 + by.0, c as i64 + by.1)
    })
}

/// Where a cell of a block goes: its row and column once put down.
type Whither<'a> = &'a dyn Fn(usize, usize) -> (i64, i64);

/// As `follow_cut`, for a block whose cells each go where `map` says, as
/// when they are turned about.
fn follow_cells(ast: &FileAst, text: &str, t: usize, block: &Block, map: Whither) -> Vec<Edit> {
    let mut edits = Vec::new();
    let decl = &ast.sheets[t];
    let cols = column_names(decl);
    let nrows = decl.rows.len();
    let inside = |r: usize, c: usize| block.0.contains(&r) && block.1.contains(&c);
    for (at, expr) in formulas(ast) {
        let (mut found, mut names) = (Vec::new(), Vec::new());
        sites(&expr, false, &mut found, &mut names);
        for site in found {
            let Some(relative) = into_sheet(&site, at, t, &decl.name) else {
                continue;
            };
            let own = at.cell.filter(|_| relative);
            let Some(((r, row, row_span), (c, col, col_span))) = one_cell(&site, own, nrows, &cols)
            else {
                continue;
            };
            if !inside(r, c) {
                continue;
            }
            let (to_row, to_col) = map(r, c);
            if to_row != r as i64 {
                put(
                    &mut edits,
                    text,
                    row_span,
                    row.write(to_row, own.map(|o| o.0), nrows),
                );
            }
            if to_col != c as i64 {
                let new = match col {
                    Some(pick) => Some(pick.write(to_col, own.map(|o| o.1), cols.len())),
                    None => usize::try_from(to_col)
                        .ok()
                        .and_then(|c| cols.get(c))
                        .map(|name| name.to_string()),
                };
                if let Some(new) = new {
                    put(&mut edits, text, col_span, new);
                }
            }
        }
        // A column named on its own in a cell is that column of its row.
        let Some((r, _)) = at.cell.filter(|_| at.sheet == Some(t)) else {
            continue;
        };
        for (name, span) in names {
            let Some(c) = cols
                .iter()
                .position(|c| *c == name)
                .filter(|c| inside(r, *c))
            else {
                continue;
            };
            let (to_row, to_col) = map(r, c);
            if let Some(new) = usize::try_from(to_col).ok().and_then(|c| cols.get(c)) {
                let new = match to_row - r as i64 {
                    0 => new.to_string(),
                    rows => format!("[{new}; {}]", offset_text(rows)),
                };
                put(&mut edits, text, span, new);
            }
        }
    }
    edits
}

/// One slot of a formula that has moved `by` along its rows or columns: an
/// offset is counted again so that it is the row or column it was. A plain
/// `*` is the formula's own, wherever it is, and a range that ends on one
/// goes with the formula as it is.
fn shift_slot(slot: &Expr, by: i64, text: &str, edits: &mut Vec<Edit>) {
    let single = range_ends(slot).is_none();
    let shift = |e: &Expr, edits: &mut Vec<Edit>| match Pick::of(e) {
        Some(Pick::Off(d)) => put(edits, text, e.span, offset_text(d - by)),
        // A plain `*` read of a cell that is not moving: it is that row
        // still, and so is no longer the formula's own.
        Some(Pick::Here) if by != 0 && single => put(edits, text, e.span, offset_text(-by)),
        _ => {}
    };
    match range_ends(slot) {
        Some((lo, hi, ..)) => {
            let here = |e: &Expr| Pick::of(e) == Some(Pick::Here);
            if !here(lo) && !here(hi) {
                shift(lo, edits);
                shift(hi, edits);
            }
        }
        None => shift(slot, edits),
    }
}

/// Where cell (`r`, `c`) of `block` goes when it is put down turned about
/// with its first cell at `to`: its row is a column, and its column a row.
fn turn_at(block: &Block, to: (usize, usize), r: usize, c: usize) -> (i64, i64) {
    (
        (to.0 + c) as i64 - block.1.start as i64,
        (to.1 + r) as i64 - block.0.start as i64,
    )
}

/// What a formula cut or turned, and put down on a different cell, does with
/// a read of a cell outside its block (D221, D222).
#[derive(Clone, Copy)]
enum Outside {
    /// It goes on reading the same cell, counted again from the new place
    /// by this many rows and columns.
    Rebase((i64, i64)),
    /// It reads the same cell, as a reference to it: a row by number.
    Freeze,
}

/// A read of one cell fixed on the cell it read: its row as a number, and
/// its column by name where it counted from its own. A reference as the
/// paste of one writes it (D210), whichever cell it is put down in.
fn freeze_cell(
    edits: &mut Vec<Edit>,
    text: &str,
    (r, row, row_span): (usize, Pick, Span),
    (c, col, col_span): (usize, Option<Pick>, Span),
    cols: &[&str],
) {
    if !matches!(row, Pick::At(_)) {
        put(edits, text, row_span, r.to_string());
    }
    if let Some(pick) = col
        && !matches!(pick, Pick::At(_))
        && let Some(name) = cols.get(c)
    {
        put(edits, text, col_span, (*name).to_string());
    }
}

/// The source of a cell of `block` put down turned about, where `map` says:
/// what it reads in the block it still reads, at the cell's new place. A row
/// it reads by counting, or a column of its own row, is counted again from
/// there. What it reads of a range or a column is the place, as for a move.
fn turned_cell(
    cell: &str,
    from: (usize, usize),
    block: &Block,
    map: Whither,
    sheet: &str,
    nrows: usize,
    cols: &[&str],
) -> String {
    let Some(Ok(expr)) = cell.strip_prefix('=').map(|body| parse_expr(body, 0, 1)) else {
        return cell.to_string();
    };
    let (now_row, now_col) = map(from.0, from.1);
    let by = (now_row - from.0 as i64, now_col - from.1 as i64);
    let own = (usize::try_from(now_row).ok(), usize::try_from(now_col).ok());
    let inside = |r: usize, c: usize| block.0.contains(&r) && block.1.contains(&c);
    let (mut found, mut names) = (Vec::new(), Vec::new());
    sites(&expr, false, &mut found, &mut names);
    let mut edits = Vec::new();
    for site in found {
        if site.nested || site.sheet.is_some_and(|name| name != sheet) {
            continue;
        }
        let one = one_cell(&site, Some(from), nrows, cols);
        // What it reads outside the block it goes on reading, as a
        // reference to that cell: an offset means nothing once the cells
        // are turned about.
        if let Some((row, col)) = one
            && !inside(row.0, col.0)
        {
            freeze_cell(&mut edits, cell, row, col, cols);
            continue;
        }
        if let Some(((r, row, row_span), (c, col, col_span))) = one {
            let (to_row, to_col) = map(r, c);
            put(&mut edits, cell, row_span, row.write(to_row, own.0, nrows));
            match col {
                Some(pick) => put(
                    &mut edits,
                    cell,
                    col_span,
                    pick.write(to_col, own.1, cols.len()),
                ),
                None => {
                    if let Some(name) = usize::try_from(to_col).ok().and_then(|c| cols.get(c)) {
                        put(&mut edits, cell, col_span, name.to_string());
                    }
                }
            }
            continue;
        }
        if let Some(slot) = site.row {
            shift_slot(slot, by.0, cell, &mut edits);
        }
        if let ColPart::Slot(slot) = site.col {
            shift_slot(slot, by.1, cell, &mut edits);
        }
    }
    // A column named on its own is that column of the cell's own row: in
    // the block it is the cell it was, and outside it the row it was in.
    for (name, span) in names {
        let Some(c) = cols.iter().position(|c| *c == name) else {
            continue;
        };
        let (name, rows) = if inside(from.0, c) {
            let (to_row, to_col) = map(from.0, c);
            let Some(name) = usize::try_from(to_col).ok().and_then(|c| cols.get(c)) else {
                continue;
            };
            (*name, to_row - now_row)
        } else {
            put(&mut edits, cell, span, format!("[{name}; {}]", from.0));
            continue;
        };
        let new = match rows {
            0 => name.to_string(),
            rows => format!("[{name}; {}]", offset_text(rows)),
        };
        put(&mut edits, cell, span, new);
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = cell.to_string();
    for (s, e, new) in edits {
        out.replace_range(s..e, &new);
    }
    out
}

/// A formula copied from `sheet` and put down on a different sheet, with
/// what it read there made to say so, so that it reads what it read: an
/// index with no sheet name is into `sheet`, and a column of its own row,
/// named alone, is `sheet`'s column of that row (D214). `cols` are the
/// columns of `sheet`. Anything else is as it was.
pub fn qualified(cell: &str, sheet: &str, cols: &[String]) -> String {
    let Some(Ok(expr)) = cell.strip_prefix('=').map(|body| parse_expr(body, 0, 1)) else {
        return cell.to_string();
    };
    let (mut found, mut names) = (Vec::new(), Vec::new());
    sites(&expr, false, &mut found, &mut names);
    let mut edits: Vec<Edit> = Vec::new();
    for site in &found {
        if site.nested || site.sheet.is_some() {
            continue;
        }
        let start = site.at.start as usize;
        edits.push((start, start, sheet.to_string()));
    }
    for (name, span) in &names {
        if cols.iter().any(|c| c == name) {
            put(&mut edits, cell, *span, format!("{sheet}[{name}; *]"));
        }
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = cell.to_string();
    for (s, e, new) in edits {
        out.replace_range(s..e, &new);
    }
    out
}

/// A block of cells copied from a sheet, as a formula in it needs to know
/// where it came from.
pub struct CopiedFrom {
    /// The sheet it was on.
    pub sheet: String,
    /// Every column of that sheet, in order.
    pub cols: Vec<String>,
    /// How many rows that sheet has.
    pub nrows: usize,
    /// The rows and the columns the block covered.
    pub rows: std::ops::Range<usize>,
    pub across: std::ops::Range<usize>,
}

/// A formula of a block copied from `src` and put down at `to` on a sheet
/// whose columns are `target`, from `from` where it was: on the same sheet
/// or on a different one. A relative read stays relative, and a fixed one is
/// left as it is (D219); the sheet pasted on is the one it reads (D220). A
/// column name read of the block's own cells is the column it landed in
/// (D226), where the row is the formula's own or counted from it.
pub fn copied_cell(
    cell: &str,
    from: (usize, usize),
    src: &CopiedFrom,
    to: (usize, usize),
    target: &[String],
) -> String {
    let Some(Ok(expr)) = cell.strip_prefix('=').map(|body| parse_expr(body, 0, 1)) else {
        return cell.to_string();
    };
    let cols: Vec<&str> = src.cols.iter().map(String::as_str).collect();
    let by = to.1 as i64 - from.1 as i64;
    let inside = |r: usize, c: usize| src.rows.contains(&r) && src.across.contains(&c);
    let landed = |c: usize| {
        usize::try_from(c as i64 + by)
            .ok()
            .and_then(|c| target.get(c))
            .cloned()
    };
    let (mut found, mut names) = (Vec::new(), Vec::new());
    sites(&expr, false, &mut found, &mut names);
    let mut edits: Vec<Edit> = Vec::new();
    for site in &found {
        if site.nested || site.sheet.is_some() {
            continue;
        }
        if let Some(((r, row, _), (c, None, col_span))) =
            one_cell(site, Some(from), src.nrows, &cols)
            && inside(r, c)
            && matches!(row, Pick::Here | Pick::Off(_))
            && let Some(name) = landed(c)
        {
            put(&mut edits, cell, col_span, name);
        }
    }
    for (name, span) in &names {
        if let Some(c) = cols.iter().position(|c| c == name)
            && inside(from.0, c)
            && let Some(name) = landed(c)
        {
            put(&mut edits, cell, *span, name);
        }
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = cell.to_string();
    for (s, e, new) in edits {
        out.replace_range(s..e, &new);
    }
    out
}

/// A formula of a block copied from another sheet, put down at `to` on a
/// sheet whose columns are `target`, from `from` where it was. What it
/// read inside the block it still reads, at the copy: the cell it stood
/// for, in the sheet it is on now. What it read outside the block it still
/// reads where it was, on the sheet it came from, which is made to say so.
/// A range or a column reads the place, and so counts as outside (D214).
pub fn carried(
    cell: &str,
    from: (usize, usize),
    src: &CopiedFrom,
    to: (usize, usize),
    target: &[String],
    target_nrows: usize,
) -> String {
    let by = (to.0 as i64 - from.0 as i64, to.1 as i64 - from.1 as i64);
    let map = |r: usize, c: usize| (r as i64 + by.0, c as i64 + by.1);
    carried_by(
        cell,
        from,
        src,
        to,
        target,
        target_nrows,
        &map,
        Outside::Rebase(by),
    )
}

/// As `carried`, for a block put down turned about with its first cell at
/// `origin`: the cell at `from` is put down at `to`, and what it read
/// inside the block it reads where that went.
pub fn carried_turned(
    cell: &str,
    from: (usize, usize),
    src: &CopiedFrom,
    origin: (usize, usize),
    to: (usize, usize),
    target: &[String],
    target_nrows: usize,
) -> String {
    let block: Block = (src.rows.clone(), src.across.clone());
    let map = |r: usize, c: usize| turn_at(&block, origin, r, c);
    carried_by(
        cell,
        from,
        src,
        to,
        target,
        target_nrows,
        &map,
        Outside::Freeze,
    )
}

/// `carried`, with `map` saying where each cell of the block went, and
/// `outside` what becomes of a read of a cell outside it.
#[allow(clippy::too_many_arguments)]
fn carried_by(
    cell: &str,
    from: (usize, usize),
    src: &CopiedFrom,
    to: (usize, usize),
    target: &[String],
    target_nrows: usize,
    map: Whither,
    outside: Outside,
) -> String {
    let Some(Ok(expr)) = cell.strip_prefix('=').map(|body| parse_expr(body, 0, 1)) else {
        return cell.to_string();
    };
    let cols: Vec<&str> = src.cols.iter().map(String::as_str).collect();
    let inside = |r: usize, c: usize| src.rows.contains(&r) && src.across.contains(&c);
    let (mut found, mut names) = (Vec::new(), Vec::new());
    sites(&expr, false, &mut found, &mut names);
    let mut edits: Vec<Edit> = Vec::new();
    for site in &found {
        // One that names its sheet says what it reads, and one inside the
        // slot of another is read as that index reads it.
        if site.nested || site.sheet.is_some() {
            continue;
        }
        let one = one_cell(site, Some(from), src.nrows, &cols);
        if let Some(((r, row, row_span), (c, col, col_span))) = one
            && inside(r, c)
        {
            let (tr, tc) = map(r, c);
            let column = match col {
                Some(pick) => Some(pick.write(tc, Some(to.1), target.len())),
                None => usize::try_from(tc)
                    .ok()
                    .and_then(|c| target.get(c))
                    .cloned(),
            };
            if let Some(column) = column {
                put(
                    &mut edits,
                    cell,
                    row_span,
                    row.write(tr, Some(to.0), target_nrows),
                );
                put(&mut edits, cell, col_span, column);
                continue;
            }
        }
        match outside {
            Outside::Freeze => {
                if let Some((row, col)) = one {
                    freeze_cell(&mut edits, cell, row, col, &cols);
                }
            }
            Outside::Rebase(by) => {
                if let Some(slot) = site.row {
                    shift_slot(slot, by.0, cell, &mut edits);
                }
                if let ColPart::Slot(slot) = site.col {
                    shift_slot(slot, by.1, cell, &mut edits);
                }
            }
        }
        let start = site.at.start as usize;
        edits.push((start, start, src.sheet.clone()));
    }
    // A column named on its own is that column of the formula's own row: in
    // the block it is the cell it was, now in the column it landed in, and
    // outside it the row it was on, in the sheet it was on.
    for (name, span) in &names {
        let Some(c) = cols.iter().position(|c| c == name) else {
            continue;
        };
        let (land_row, land_col) = map(from.0, c);
        let landed = usize::try_from(land_col)
            .ok()
            .and_then(|c| target.get(c))
            .filter(|_| inside(from.0, c));
        let new = match landed {
            Some(name) => match land_row - to.0 as i64 {
                0 => name.clone(),
                rows => format!("[{name}; {}]", offset_text(rows)),
            },
            None if matches!(outside, Outside::Freeze) => {
                format!("{}[{name}; {}]", src.sheet, from.0)
            }
            None => format!(
                "{}[{name}; {}]",
                src.sheet,
                offset_text(from.0 as i64 - to.0 as i64)
            ),
        };
        put(&mut edits, cell, *span, new);
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = cell.to_string();
    for (s, e, new) in edits {
        out.replace_range(s..e, &new);
    }
    out
}

/// The source of a cell of `block`, cut from `from` in a sheet and put down
/// `by` rows and columns away. What its formula reads by an offset it still
/// reads: a move sideways counts the columns again, and a move up or down
/// the rows. A cell that was cut along with it is as far away as it was.
fn moved_cell(
    cell: &str,
    from: (usize, usize),
    block: &Block,
    by: (i64, i64),
    sheet: &str,
    nrows: usize,
    cols: &[&str],
) -> String {
    let Some(Ok(expr)) = cell.strip_prefix('=').map(|body| parse_expr(body, 0, 1)) else {
        return cell.to_string();
    };
    let (mut found, mut names) = (Vec::new(), Vec::new());
    sites(&expr, false, &mut found, &mut names);
    let mut edits = Vec::new();
    // A column named on its own is that column of the cell's own row: cut
    // with the block it is the cell it was, in the column it landed in, and
    // outside it the row it was in.
    for (name, span) in names {
        let Some(c) = cols.iter().position(|c| *c == name) else {
            continue;
        };
        if block.0.contains(&from.0) && block.1.contains(&c) {
            if let Some(new) = usize::try_from(c as i64 + by.1)
                .ok()
                .and_then(|c| cols.get(c))
            {
                put(&mut edits, cell, span, new.to_string());
            }
        } else if by.0 != 0 {
            put(
                &mut edits,
                cell,
                span,
                format!("[{name}; {}]", offset_text(-by.0)),
            );
        }
    }
    for site in found {
        if site.nested || site.sheet.is_some_and(|name| name != sheet) {
            continue;
        }
        let one = one_cell(&site, Some(from), nrows, cols);
        if let Some(((r, row, row_span), (c, col, col_span))) = one
            && block.0.contains(&r)
            && block.1.contains(&c)
        {
            // Cut together: an offset is right as it is, and a number or a
            // name follows the cell.
            if let Pick::At(_) = row {
                put(
                    &mut edits,
                    cell,
                    row_span,
                    row.write(r as i64 + by.0, None, nrows),
                );
            }
            let to = c as i64 + by.1;
            match col {
                Some(pick @ Pick::At(_)) => {
                    put(&mut edits, cell, col_span, pick.write(to, None, cols.len()));
                }
                None => {
                    if let Some(name) = usize::try_from(to).ok().and_then(|c| cols.get(c)) {
                        put(&mut edits, cell, col_span, name.to_string());
                    }
                }
                _ => {}
            }
            continue;
        }
        if let Some(slot) = site.row {
            shift_slot(slot, by.0, cell, &mut edits);
        }
        if let ColPart::Slot(slot) = site.col {
            shift_slot(slot, by.1, cell, &mut edits);
        }
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = cell.to_string();
    for (s, e, new) in edits {
        out.replace_range(s..e, &new);
    }
    out
}

/// Make arbitrary input safe to store as one cell: a single line that does
/// not split into several cells.
fn clean_cell(input: &str) -> String {
    // A newline is kept: a cell may hold one, written as the `\n` escape the
    // lexer already reads. Every other control character becomes a space,
    // there being nothing in the file to write it as.
    let text: String = input
        .chars()
        .map(|c| match c {
            '\n' => '\n',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let text = text.trim();
    if text.is_empty() || (!text.contains('\n') && split_cells(text, 0, 0).len() == 1) {
        return text.to_string();
    }
    format!(
        "\"{}\"",
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

/// A diagnostic as one line, its help after it.
fn said(d: &omasheet_omx::Diagnostic) -> String {
    match &d.help {
        Some(help) => format!("{} \u{2014} {help}", d.message),
        None => d.message.clone(),
    }
}

/// A name with any `_001` it was copied with taken off.
fn copy_base(name: &str) -> &str {
    match name.rsplit_once('_') {
        Some((base, n))
            if n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit()) && !base.is_empty() =>
        {
            base
        }
        _ => name,
    }
}

fn valid_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || ident_len(name) != name.len() {
        return Err(format!(
            "`{name}` is not a valid name: use letters, digits and `_`, starting with a letter"
        ));
    }
    Ok(name)
}

/// The header and rows of a sheet as text, plus where each row came from.
struct Grid {
    header: Vec<String>,
    /// Which columns of the header row are formula columns: their cells
    /// are `*`.
    formula: Vec<bool>,
    /// `(original row index, cells)`; `None` for a new row.
    rows: Vec<(Option<usize>, Vec<String>)>,
}

impl Grid {
    fn of(t: &SheetDecl) -> Grid {
        let formula = |name: &String| t.computed.iter().any(|c| &c.name == name);
        Grid {
            header: t.header.iter().map(|(name, _)| name.clone()).collect(),
            formula: t.header.iter().map(|(name, _)| formula(name)).collect(),
            rows: t
                .rows
                .iter()
                .enumerate()
                .map(|(i, row)| (Some(i), row.iter().map(|c| c.text.clone()).collect()))
                .collect(),
        }
    }

    fn blank_row(&self) -> Vec<String> {
        let blank = |formula: &bool| if *formula { FORMULA_CELL } else { "" };
        self.formula.iter().map(|f| blank(f).to_string()).collect()
    }

    /// Rewrite the sheet's lines in `text` as a Markdown table, aligned,
    /// touching nothing else: each line between outer pipes, `| a | b |`,
    /// and a `| --- |` line under the header row.
    fn write(&self, t: &SheetDecl, text: &str) -> String {
        // A sheet with no header row has no table in the text: there is
        // nothing to line up, and no line to write it over.
        if t.header.is_empty() {
            return text.to_string();
        }
        let n = self.header.len();
        // At least as wide as the `---` that Markdown asks for.
        let mut widths: Vec<usize> = self
            .header
            .iter()
            .map(|h| h.chars().count().max(3))
            .collect();
        for (_, row) in &self.rows {
            for (k, cell) in row.iter().enumerate().take(n) {
                widths[k] = widths[k].max(cell.chars().count());
            }
        }
        let line = |cells: &[String]| {
            let padded: Vec<String> = cells
                .iter()
                .enumerate()
                .map(|(k, c)| format!("{c:<width$}", width = widths[k]))
                .collect();
            format!("| {} |", padded.join(" | "))
        };
        let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
        let rule = line(&rule);

        // (start, end, replacement), applied from the end of the text back.
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        let range = |s: Span| (s.start as usize, s.end as usize);
        let (hs, he) = range(t.header_line);
        edits.push((hs, he, line(&self.header)));
        let mut anchor = he;
        if let Some(sep) = t.separator_line {
            let (s, e) = range(sep);
            edits.push((s, e, rule));
            anchor = e;
        } else {
            edits.push((he, he, format!("\n{rule}")));
        }
        let mut kept = vec![false; t.row_lines.len()];
        for origin in self.rows.iter().filter_map(|(o, _)| *o) {
            if let Some(k) = kept.get_mut(origin) {
                *k = true;
            }
        }
        for (i, span) in t.row_lines.iter().enumerate() {
            if !kept[i] {
                // Remove the line and its line ending.
                let (s, e) = range(*span);
                let end = if text[e..].starts_with("\r\n") {
                    e + 2
                } else if text[e..].starts_with('\n') {
                    e + 1
                } else {
                    e
                };
                edits.push((s, end, String::new()));
            }
        }
        // New rows go after the line of the row they follow.
        let mut pending = String::new();
        for (origin, cells) in &self.rows {
            match origin {
                Some(i) => {
                    if !pending.is_empty() {
                        edits.push((anchor, anchor, std::mem::take(&mut pending)));
                    }
                    let (s, e) = range(t.row_lines[*i]);
                    edits.push((s, e, line(cells)));
                    anchor = e;
                }
                None => {
                    pending.push('\n');
                    pending.push_str(&line(cells));
                }
            }
        }
        if !pending.is_empty() {
            edits.push((anchor, anchor, pending));
        }
        // Written once, front to back: replacing in the text edit by edit
        // would move the rest of it each time a line changed length. The
        // edits do not overlap, and those at one place go in the order made.
        let mut order: Vec<usize> = (0..edits.len()).collect();
        order.sort_by_key(|&k| (edits[k].0, k));
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        for k in order {
            let (s, e, ref replacement) = edits[k];
            out.push_str(&text[at..s]);
            out.push_str(replacement);
            at = e;
        }
        out.push_str(&text[at..]);
        out
    }
}

/// `text` with the lines of every sheet aligned, as an edit in the grid
/// leaves them: each column as wide as its widest cell, the `|` of every
/// line in step. Text that does not parse is left as it is, and so is a
/// sheet with a row longer than its header row.
fn aligned(text: &str) -> String {
    let mut diags = Vec::new();
    let ast = parse_file(text, 0, &mut diags);
    if !diags.is_empty() {
        return text.to_string();
    }
    let mut sheets: Vec<&SheetDecl> = ast.sheets.iter().collect();
    // From the last back, so the lines of those before stay where they were.
    sheets.sort_by_key(|t| std::cmp::Reverse(t.header_line.start));
    let mut out = text.to_string();
    for t in sheets {
        if t.rows.iter().all(|row| row.len() <= t.header.len()) {
            out = Grid::of(t).write(t, &out);
        }
    }
    out
}

/// `text` with the format of type line `l` set to `format`, or taken off.
fn reformatted(text: &str, l: &SchemaLine, format: Option<&Format>) -> String {
    let end = l.span.end as usize;
    let colon = end + text[end..].find(':').unwrap_or(0);
    let rest = &text[colon + 1..];
    let ty_at = colon + 1 + (rest.len() - rest.trim_start_matches([' ', '\t']).len());
    let ty_end = ty_at + ident_len(&text[ty_at..]);
    let old_end = match text[ty_end..].starts_with('[') {
        true => closing(&text[ty_end + 1..]).map_or(ty_end, |k| ty_end + k + 2),
        false => ty_end,
    };
    let mut out = text.to_string();
    let new = format.map(|f| format!("[{f}]")).unwrap_or_default();
    out.replace_range(ty_end..old_end, &new);
    out
}

/// `text` with the type line of column `name` of a sheet set to `ty`, or
/// taken out. Type lines whose colons are in line stay so.
fn retyped(text: &str, decl: &SheetDecl, name: &str, ty: Option<S>) -> String {
    let line_start = |at: usize| text[..at].rfind('\n').map_or(0, |k| k + 1);
    // Where the colon of a type line is, and its type.
    let parts = |l: &SchemaLine| {
        let end = l.span.end as usize;
        let colon = end + text[end..].find(':').unwrap_or(0);
        let rest = &text[colon + 1..];
        let ty_at = colon + 1 + (rest.len() - rest.trim_start_matches([' ', '\t']).len());
        (colon, ty_at, ty_at + ident_len(&text[ty_at..]))
    };
    let existing = decl.schema.iter().find(|l| l.name == name);
    let mut out = text.to_string();

    let Some(ty) = ty else {
        let Some(l) = existing else { return out };
        let start = line_start(l.span.start as usize);
        let end = text[start..]
            .find('\n')
            .map_or(text.len(), |k| start + k + 1);
        out.replace_range(start..end, "");
        // The last of them takes its blank line with it.
        let alone = decl.schema.len() == 1;
        if alone && out[start..].starts_with('\n') && out[..start].ends_with("\n\n") {
            out.remove(start);
        }
        return out;
    };
    if let Some(l) = existing {
        let (_, ty_at, mut ty_end) = parts(l);
        // A format the new type does not take goes with the old type.
        if l.format.as_ref().is_some_and(|f| !f.takes(ty))
            && text[ty_end..].starts_with('[')
            && let Some(close) = closing(&text[ty_end + 1..])
        {
            ty_end += close + 2;
        }
        out.replace_range(ty_at..ty_end, &ty.to_string());
        return out;
    }

    // How far each colon is from the start of its name.
    let offsets: Vec<usize> = decl
        .schema
        .iter()
        .map(|l| parts(l).0 - l.span.start as usize)
        .collect();
    let in_line = offsets.windows(2).all(|w| w[0] == w[1]);
    let mut offset = name.len() + 1;
    let mut edits: Vec<(usize, String)> = Vec::new();
    if in_line && let Some(&theirs) = offsets.first() {
        // A longer name moves the others out to it.
        for l in decl.schema.iter().filter(|_| offset > theirs) {
            edits.push((l.span.end as usize, " ".repeat(offset - theirs)));
        }
        offset = offset.max(theirs);
    }
    let line = format!("{name:<width$} : {ty}\n", width = offset - 1);
    // In the order of the header row; with no type lines yet, above it.
    let place = |n: &str| decl.header.iter().position(|h| h.0 == n);
    let here = place(name).unwrap_or(usize::MAX);
    let next = decl
        .schema
        .iter()
        .find(|l| place(&l.name).unwrap_or(usize::MAX) > here);
    match (next, decl.schema.last()) {
        (Some(l), _) => edits.push((line_start(l.span.start as usize), line)),
        (None, Some(l)) => {
            let end = l.span.end as usize;
            match text[end..].find('\n') {
                Some(k) => edits.push((end + k + 1, line)),
                None => edits.push((text.len(), format!("\n{}", line.trim_end()))),
            }
        }
        (None, None) => edits.push((
            line_start(decl.header_line.start as usize),
            format!("{line}\n"),
        )),
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    for (at, insert) in edits {
        out.insert_str(at, &insert);
    }
    out
}

/// A filter worked out as far as the document has to: what is left is
/// Polars' to run, which a worker can, as none of it is the document's.
/// See [`Document::filter_job`].
pub struct FilterJob {
    rows: usize,
    /// The columns' masks answered here, a row at a time.
    masks: Vec<Vec<bool>>,
    /// The columns' filters lowered to Polars, each with its frame.
    plans: Vec<(polars::prelude::DataFrame, polars::prelude::Expr)>,
    /// The grid's order of the rows, where it has been sorted.
    order: Option<Arc<Vec<usize>>>,
}

impl FilterJob {
    /// How many rows the sheet has: what the work is over.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The rows shown, in the grid's order, or `None` where Polars fails on
    /// a filter, which [`Document::shown_rows`] then answers on its own.
    pub fn run(self) -> Option<Vec<usize>> {
        let n = self.rows;
        let mut keep = vec![true; n];
        let mut and = |mask: Vec<bool>| {
            for (k, m) in keep.iter_mut().zip(mask) {
                *k &= m;
            }
        };
        for mask in self.masks {
            and(mask);
        }
        for (frame, expr) in self.plans {
            and(crate::eval::run_filter_plan(frame, expr)?);
        }
        let order = self.order.as_deref();
        Some(
            (0..n)
                .filter(|&p| keep[order.map_or(p, |o| o[p])])
                .collect(),
        )
    }
}

/// A table's sort worked out as far as the document has to: the column in
/// the order shown, for a worker to put in order. See
/// [`Document::sort_job`] and [`Document::finish_sort`].
pub struct SortJob {
    sheet: usize,
    col: usize,
    desc: bool,
    shown: polars::prelude::Series,
    /// The grid's order when the job was made: the sort is of that, and is
    /// not applied over another.
    before: Option<Arc<Vec<usize>>>,
}

impl SortJob {
    /// The places of the rows as shown, in the order of the column: a
    /// stable `arg_sort`, as [`Document::sort_rows`] takes it.
    pub fn run(&self) -> Vec<usize> {
        self.shown
            .arg_sort(polars::prelude::SortOptions {
                descending: self.desc,
                nulls_last: self.desc,
                maintain_order: true,
                multithreaded: true,
                limit: None,
            })
            .into_no_null_iter()
            .map(|i| i as usize)
            .collect()
    }
}

/// How a column of a [`SummaryJob`] is added up.
enum Kind {
    Whole,
    Float,
    /// Counted, with no numbers.
    Counted,
}

/// A selection's summary worked out as far as the document has to: its
/// columns as Polars holds them, for a worker to add up. See
/// [`Document::summary_job`].
pub struct SummaryJob {
    rows: std::ops::Range<usize>,
    order: Option<Arc<Vec<usize>>>,
    columns: Vec<(polars::prelude::Column, Kind)>,
}

/// What a [`SummaryJob`] added up: how many values, how many of them
/// numbers, and their sum.
pub struct Added {
    count: usize,
    numbers: usize,
    sum: Sum,
}

enum Sum {
    Whole(i128),
    Float(f64),
}

impl Added {
    /// The sum as a value, or `None` past what a whole number holds.
    fn sum(&self) -> Option<Value> {
        Some(match self.sum {
            Sum::Whole(w) => Value::int(i64::try_from(w).ok()?),
            Sum::Float(f) => Value::Float(f),
        })
    }
}

impl SummaryJob {
    /// How many rows it covers: what the work is over.
    pub fn rows(&self) -> usize {
        self.rows.len()
    }

    /// Add it up. `None` where Polars will not give a column as expected.
    pub fn run(self) -> Option<Added> {
        use polars::prelude::{DataType, IdxCa, StaticArray};
        let rows = self.rows;
        let (mut count, mut numbers) = (0usize, 0usize);
        let mut ints = Vec::new();
        let mut floats = Vec::new();
        let mut kinds = Vec::new();
        for (column, kind) in self.columns {
            let column = match &self.order {
                Some(order) => {
                    let idx: Vec<_> = order[rows.clone()].iter().map(|&r| r as _).collect();
                    column.take(&IdxCa::from_vec("".into(), idx)).ok()?
                }
                None => column.slice(rows.start as i64, rows.len()),
            };
            let held = column.len() - column.null_count();
            match kind {
                Kind::Whole => {
                    ints.push(
                        column
                            .cast(&DataType::Int64)
                            .ok()?
                            .i64()
                            .ok()?
                            .rechunk()
                            .into_owned(),
                    );
                    kinds.push(false);
                    numbers += held;
                }
                Kind::Float => {
                    floats.push(
                        column
                            .cast(&DataType::Float64)
                            .ok()?
                            .f64()
                            .ok()?
                            .rechunk()
                            .into_owned(),
                    );
                    kinds.push(true);
                    numbers += held;
                }
                Kind::Counted => {}
            }
            count += held;
        }
        let ints: Vec<_> = ints
            .iter()
            .map(|ca| ca.downcast_iter().next())
            .collect::<Option<_>>()?;
        let floats: Vec<_> = floats
            .iter()
            .map(|ca| ca.downcast_iter().next())
            .collect::<Option<_>>()?;
        let mut sum = Sum::Whole(0);
        for r in 0..rows.len() {
            let (mut i, mut f) = (ints.iter(), floats.iter());
            for &float in &kinds {
                sum = if float {
                    match (f.next()?.get(r), &sum) {
                        (None, _) => continue,
                        (Some(v), Sum::Whole(w)) => Sum::Float(*w as f64 + v),
                        (Some(v), Sum::Float(s)) => Sum::Float(s + v),
                    }
                } else {
                    match (i.next()?.get(r), &sum) {
                        (None, _) => continue,
                        (Some(v), Sum::Whole(w)) => Sum::Whole(w + v as i128),
                        (Some(v), Sum::Float(s)) => Sum::Float(s + v as f64),
                    }
                };
            }
        }
        Some(Added {
            count,
            numbers,
            sum,
        })
    }
}

impl Document {
    /// A document that shows dates and times as the file writes them.
    pub fn from_text(text: &str) -> Document {
        Document::new(text, Style::ISO, Names::default(), false, Copying::Wait)
    }

    /// A document for a person to look at: dates and times are shown in
    /// `style`, and date-times on the clocks of this machine even when the
    /// file is in another time zone. See [`Document::set_style`].
    pub fn with_style(text: &str, style: Style) -> Document {
        Document::with_locale(text, style, Names::default())
    }

    /// As [`Document::with_style`], with dates shown by a column's format
    /// in the language of `names`.
    pub fn with_locale(text: &str, style: Style, names: Names) -> Document {
        Document::new(text, style, names, true, Copying::Wait)
    }

    /// As [`Document::with_locale`], for a screen that should never wait
    /// for a big CSV to be copied: a table over one shows the sample of its
    /// first rows until the copy is written (open-a-big-csv D153), which
    /// [`Document::to_convert`] says is wanted and
    /// [`Document::converted`] takes up once it is.
    pub fn on_screen(text: &str, style: Style, names: Names) -> Document {
        Document::new(text, style, names, true, Copying::Sample)
    }

    fn new(text: &str, style: Style, names: Names, local: bool, copying: Copying) -> Document {
        let mut doc = Document {
            text: text.to_string(),
            snapshot: Snapshot::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            style,
            names,
            local,
            base: None,
            orders: Default::default(),
            whole: Default::default(),
            edits: 0,
            engine: None,
            column_errors: Vec::new(),
            paged_rows: PAGED_ROWS,
            copying,
        };
        doc.rebuild();
        doc
    }

    /// The directory a `table`'s source is read relative to, which the
    /// front end knows and the text does not: a document is re-read from
    /// its own text, and what a `table` names is beside the file.
    pub fn set_base(&mut self, dir: impl Into<std::path::PathBuf>) {
        self.base = Some(dir.into());
        self.rebuild();
    }

    /// Page a sheet of more than `rows` rows the text does not hold, in
    /// place of [`PAGED_ROWS`]: for a test to page a small file.
    #[doc(hidden)]
    pub fn set_paged_rows(&mut self, rows: usize) {
        self.paged_rows = rows;
        self.rebuild();
    }

    /// How dates and times are shown, and how they may be typed into a cell.
    /// The text of the file stays ISO whatever the style.
    pub fn set_style(&mut self, style: Style) {
        self.style = style;
        self.rebuild();
    }

    pub fn style(&self) -> &Style {
        &self.style
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Every table's file that is paged and not yet held whole, with what
    /// holding it would take (D157). Whether to hold it is the caller's to
    /// decide, against the memory there is, and the reading is the caller's
    /// to do, off the thread that draws: [`crate::table::read_whole`] then
    /// [`Document::hold`].
    pub fn to_hold(&self) -> Vec<ToHold> {
        let Some(engine) = &self.engine else {
            return Vec::new();
        };
        let base = self.base.clone().unwrap_or_else(|| ".".into());
        let mut out: Vec<ToHold> = Vec::new();
        for (sheet, snap) in engine.program().sheets.iter().zip(&self.snapshot.sheets) {
            let Some(source) = &sheet.source else {
                continue;
            };
            let path = base.join(&source.path);
            // A CSV not yet copied is shown from its sample, which is small.
            if crate::table::is_csv(&path) && crate::table::copy_of(&path).is_none() {
                continue;
            }
            let path = self.file_of(&path);
            if !snap.paged || self.whole.contains_key(&path) || out.iter().any(|h| h.path == path) {
                continue;
            }
            // The columns the file is read for: the sheet's own, by the name
            // the file gives them.
            let columns: Vec<String> = sheet
                .cols
                .iter()
                .filter(|c| matches!(c.kind, ColKind::Frame))
                .map(|c| c.from.clone().unwrap_or_else(|| c.name.clone()))
                .collect();
            if let Ok(bytes) = crate::table::estimate(&path, &columns) {
                out.push(ToHold {
                    path,
                    columns,
                    bytes,
                });
            }
        }
        out
    }

    /// Hold the file at `path` whole, as a worker read it, and rebuild over
    /// it, so that its columns are given from memory. `false`, and nothing
    /// held, where the text no longer names the file, it has changed since
    /// it was read, or it is held already.
    pub fn hold(&mut self, path: std::path::PathBuf, whole: crate::table::Whole) -> bool {
        if self.whole.contains_key(&path)
            || crate::table::identity(&path).as_ref() != Some(&whole.identity)
            || !self.to_hold().iter().any(|h| h.path == path)
        {
            return false;
        }
        self.whole.insert(path, whole);
        self.rebuild();
        true
    }

    /// The file a table's source at `path` is read from: itself, or for a
    /// CSV its copy, or its sample while there is none.
    fn file_of(&self, path: &std::path::Path) -> std::path::PathBuf {
        if !crate::table::is_csv(path) {
            return path.to_path_buf();
        }
        crate::table::copy_of(path).unwrap_or_else(|| path.to_path_buf())
    }

    /// The file a table names, for a file it is read from: a CSV, for its
    /// copy in the cache; any other file is itself. For what to call it.
    pub fn source_file(&self, read: &std::path::Path) -> std::path::PathBuf {
        let base = self.base.clone().unwrap_or_else(|| ".".into());
        self.engine
            .iter()
            .flat_map(|e| e.program().sheets.iter())
            .filter_map(|s| s.source.as_ref())
            .map(|source| base.join(&source.path))
            .find(|named| {
                crate::table::is_csv(named) && crate::table::copy_of(named).as_deref() == Some(read)
            })
            .unwrap_or_else(|| read.to_path_buf())
    }

    /// The CSV files this document's tables read that have no copy yet, and
    /// are shown from their samples meanwhile: each for a worker to
    /// [`crate::table::convert`], and then [`Document::converted`].
    pub fn to_convert(&self) -> Vec<std::path::PathBuf> {
        let Some(engine) = &self.engine else {
            return Vec::new();
        };
        let base = self.base.clone().unwrap_or_else(|| ".".into());
        let mut out: Vec<std::path::PathBuf> = Vec::new();
        for sheet in &engine.program().sheets {
            let Some(source) = &sheet.source else {
                continue;
            };
            let path = base.join(&source.path);
            if crate::table::is_csv(&path)
                && crate::table::copy_of(&path).is_none()
                && !out.contains(&path)
            {
                out.push(path);
            }
        }
        out
    }

    /// A CSV's copy has been written: its tables are read from it now,
    /// whole, where they were shown from the sample. Whether anything
    /// changed.
    pub fn converted(&mut self, csv: &std::path::Path) -> bool {
        if crate::table::copy_of(csv).is_none() {
            return false;
        }
        self.rebuild();
        true
    }

    /// Whether the file at `path` is held whole.
    pub fn holds(&self, path: &std::path::Path) -> bool {
        self.whole.contains_key(path)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        let now = self.restore(previous);
        self.redo.push(now);
        self.rebuild();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        let now = self.restore(next);
        self.undo.push(now);
        self.rebuild();
        true
    }

    /// Put the document back to `step`, giving the state it was in.
    fn restore(&mut self, step: Step) -> Step {
        Step {
            text: std::mem::replace(&mut self.text, step.text),
            orders: std::mem::replace(&mut self.orders, step.orders),
        }
    }

    /// Keep the state the document was in as the step an edit undoes to.
    fn push_step(&mut self, previous: Step) {
        self.undo.push(previous);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.edits += 1;
    }

    /// How many steps there are to undo: a mark for [`Document::join_since`].
    pub fn undo_mark(&self) -> usize {
        self.undo.len()
    }

    /// How many edits have been made, undo and redo not being edits. What
    /// tells an edit from a redo, which both leave one more step to undo.
    pub fn edit_count(&self) -> usize {
        self.edits
    }

    /// Make every step taken since `mark` one step, undone at once.
    pub fn join_since(&mut self, mark: usize) {
        if self.undo.len() > mark + 1 {
            self.undo.truncate(mark + 1);
        }
    }

    /// Replace the text as one undoable step. Returns whether it changed.
    fn commit(&mut self, text: String) -> bool {
        if text == self.text {
            return false;
        }
        let previous = Step {
            text: std::mem::replace(&mut self.text, text),
            orders: self.orders.clone(),
        };
        self.push_step(previous);
        self.rebuild();
        true
    }

    /// The text with the lines of every sheet aligned, as the source is
    /// shown to be edited by hand.
    pub fn aligned_text(&self) -> String {
        aligned(&self.text)
    }

    /// Replace the whole text, as written by hand, as one undoable step,
    /// with the lines of every sheet aligned. Returns whether it changed.
    pub fn set_text(&mut self, text: &str) -> bool {
        self.commit(aligned(text))
    }

    /// Make several edits as one undoable step. Returns whether they
    /// changed anything.
    pub fn as_one(&mut self, edits: impl FnOnce(&mut Self)) -> bool {
        let before = self.text.clone();
        let steps = self.undo.len();
        edits(self);
        let changed = self.text != before;
        // The first of the edits left the text as it was to be undone to.
        if self.undo.get(steps).map(|s| &s.text) == Some(&before) {
            self.undo.truncate(steps + usize::from(changed));
        }
        changed
    }

    fn ast(&self) -> FileAst {
        parse_file(&self.text, 0, &mut Vec::new())
    }

    /// The declaration of the snapshot's sheet `sheet`.
    fn sheet_decl<'a>(&self, ast: &'a FileAst, sheet: usize) -> Option<&'a SheetDecl> {
        let name = &self.snapshot.sheets.get(sheet)?.name;
        ast.sheets.iter().find(|t| &t.name == name)
    }

    /// Why sheet `sheet` takes no edit, if it takes none: its rows are a
    /// Parquet file's, and nothing is ever written back to that file (D49).
    /// `None` for a sheet that holds its own rows. Every edit of such a
    /// sheet is refused, and a front end can say so before one is tried.
    pub fn read_only(&self, sheet: usize) -> Option<String> {
        read_only_why(self.snapshot.sheets.get(sheet)?)
    }

    /// Why the cells of column `col` of sheet `sheet` take no edit, if they
    /// take none: a sheet calculated from an expression has no cell of its
    /// own, and the data columns of a `view` are its file's. A formula column
    /// appended to a view, a decorator, is the file's own text and is
    /// edited by its formula, as any other is.
    pub fn read_only_cell(&self, sheet: usize, col: usize) -> Option<String> {
        let snap = self.snapshot.sheets.get(sheet)?;
        let decorator = snap.source.is_some()
            && snap.view.is_none()
            && snap.columns.get(col).is_some_and(|c| c.formula.is_some());
        if decorator {
            None
        } else {
            self.read_only(sheet)
        }
    }

    /// Why sheet `sheet` has no cell to take an edit: a table's rows are in
    /// a file, and a sheet of `:=` lines alone has no rows of its own. Its
    /// formulas are its text, so those are edited as any other sheet's.
    fn no_cells(&self, sheet: usize) -> Option<String> {
        if let Some(why) = self.read_only(sheet) {
            return Some(why);
        }
        let snap = self.snapshot.sheets.get(sheet)?;
        snap.derived.then(|| {
            format!(
                "sheet `{}` has no rows of its own: every column is calculated",
                snap.name
            )
        })
    }

    /// `text` with the grid of sheet `sheet` changed.
    fn grid_edited(
        &self,
        text: &str,
        sheet: usize,
        change: impl FnOnce(&mut Grid),
    ) -> Option<String> {
        if self.no_cells(sheet).is_some() {
            return None;
        }
        let ast = parse_file(text, 0, &mut Vec::new());
        let decl = self.sheet_decl(&ast, sheet)?;
        let mut grid = Grid::of(decl);
        change(&mut grid);
        Some(grid.write(decl, text))
    }

    fn edit_grid(&mut self, sheet: usize, change: impl FnOnce(&mut Grid)) -> bool {
        match self.grid_edited(&self.text, sheet, change) {
            Some(text) => self.commit(text),
            None => false,
        }
    }

    /// Set cells of one sheet in a single undoable step. Cells are
    /// `(row, column, text)`; rows past the end are added. Cells in computed
    /// columns are skipped.
    pub fn set_cells(&mut self, sheet: usize, cells: &[(usize, usize, String)]) -> bool {
        match self.cells_set(&self.text, sheet, cells) {
            Some(text) => self.commit(text),
            None => false,
        }
    }

    /// `text` with cells of one sheet set, as `set_cells` sets them.
    fn cells_set(
        &self,
        text: &str,
        sheet: usize,
        cells: &[(usize, usize, String)],
    ) -> Option<String> {
        let style = self.style;
        let columns = self
            .snapshot
            .sheets
            .get(sheet)
            .map(|t| t.columns.as_slice());
        let types: Vec<String> = columns
            .map(|c| c.iter().map(|c| c.ty.clone()).collect())
            .unwrap_or_default();
        self.grid_edited(text, sheet, |grid| {
            for (row, col, text) in cells {
                if *col >= grid.header.len() || grid.formula[*col] {
                    continue;
                }
                while grid.rows.len() <= *row {
                    let blank = grid.blank_row();
                    grid.rows.push((None, blank));
                }
                let ty = types.get(*col).map_or("", String::as_str);
                // The file keeps the text as it was entered (D186): a column's
                // type shows it in its own format, and a type is never set
                // by what is typed into a cell (D191).
                grid.rows[*row].1[*col] =
                    iso_entry(&style, ty, text).unwrap_or_else(|| clean_cell(text));
            }
        })
    }

    /// Put down `block`, the sources of cells cut from `from` in a sheet, at
    /// `to` in the same sheet, in one undoable step. Formulas go on reading
    /// what they read: those put down count their offsets again, and those
    /// that read a cell that was cut follow it to where it now is.
    pub fn paste_cut(
        &mut self,
        sheet: usize,
        from: (usize, usize),
        to: (usize, usize),
        block: &[Vec<String>],
    ) -> bool {
        let ast = self.ast();
        let Some(name) = self.snapshot.sheets.get(sheet).map(|t| t.name.clone()) else {
            return false;
        };
        let Some(t) = ast.sheets.iter().position(|t| t.name == name) else {
            return false;
        };
        let width = block.iter().map(Vec::len).max().unwrap_or(0);
        let cut: Block = (from.0..from.0 + block.len(), from.1..from.1 + width);
        let by = (to.0 as i64 - from.0 as i64, to.1 as i64 - from.1 as i64);
        let decl = &ast.sheets[t];
        let cols = column_names(decl);
        let mut cells = Vec::new();
        for (i, line) in block.iter().enumerate() {
            for (j, cell) in line.iter().enumerate() {
                let at = (from.0 + i, from.1 + j);
                let moved = moved_cell(cell, at, &cut, by, &name, decl.rows.len(), &cols);
                cells.push((to.0 + i, to.1 + j, moved));
            }
        }
        let edits = follow_cut(&ast, &self.text, t, &cut, by);
        let followed = rewritten(&self.text, &ast, edits);
        match self.cells_set(&followed, sheet, &cells) {
            Some(text) => self.commit(text),
            None => false,
        }
    }

    /// A block copied from `from` and put down turned about at `to`, as the
    /// rows of columns: each cell's formula still reads what it read, now at
    /// the place it has been put. Short rows are filled out.
    pub fn turn_block(
        &self,
        sheet: usize,
        from: (usize, usize),
        to: (usize, usize),
        block: &[Vec<String>],
    ) -> Vec<Vec<String>> {
        let ast = self.ast();
        self.turned(&ast, sheet, from, to, block).1
    }

    /// As `paste_cut`, but turned about: the cells are put down as the rows
    /// of columns, and a formula elsewhere that read one of them reads it
    /// where it has gone.
    pub fn paste_cut_turned(
        &mut self,
        sheet: usize,
        from: (usize, usize),
        to: (usize, usize),
        block: &[Vec<String>],
    ) -> bool {
        let ast = self.ast();
        let (Some(t), turned) = self.turned(&ast, sheet, from, to, block) else {
            return false;
        };
        let width = block.iter().map(Vec::len).max().unwrap_or(0);
        let cut: Block = (from.0..from.0 + block.len(), from.1..from.1 + width);
        let map = |r: usize, c: usize| turn_at(&cut, to, r, c);
        let edits = follow_cells(&ast, &self.text, t, &cut, &map);
        let followed = rewritten(&self.text, &ast, edits);
        let mut cells = Vec::new();
        for (j, line) in turned.iter().enumerate() {
            for (i, cell) in line.iter().enumerate() {
                cells.push((to.0 + j, to.1 + i, cell.clone()));
            }
        }
        match self.cells_set(&followed, sheet, &cells) {
            Some(text) => self.commit(text),
            None => false,
        }
    }

    /// The index of sheet `sheet` in `ast`, and a block turned about.
    fn turned(
        &self,
        ast: &FileAst,
        sheet: usize,
        from: (usize, usize),
        to: (usize, usize),
        block: &[Vec<String>],
    ) -> (Option<usize>, Vec<Vec<String>>) {
        let width = block.iter().map(Vec::len).max().unwrap_or(0);
        let name = self.snapshot.sheets.get(sheet).map(|t| t.name.clone());
        let t = name
            .as_ref()
            .and_then(|name| ast.sheets.iter().position(|t| &t.name == name));
        let cut: Block = (from.0..from.0 + block.len(), from.1..from.1 + width);
        let map = |r: usize, c: usize| turn_at(&cut, to, r, c);
        let turned = (0..width)
            .map(|j| {
                block
                    .iter()
                    .enumerate()
                    .map(|(i, line)| {
                        let cell = line.get(j).cloned().unwrap_or_default();
                        match (t, &name) {
                            (Some(t), Some(name)) => {
                                let decl = &ast.sheets[t];
                                let cols = column_names(decl);
                                turned_cell(
                                    &cell,
                                    (from.0 + i, from.1 + j),
                                    &cut,
                                    &map,
                                    name,
                                    decl.rows.len(),
                                    &cols,
                                )
                            }
                            _ => cell,
                        }
                    })
                    .collect()
            })
            .collect();
        (t, turned)
    }

    /// How many rows sheet `sheet` has, paged or not.
    pub fn row_count(&self, sheet: usize) -> usize {
        self.snapshot.sheets.get(sheet).map_or(0, |t| t.nrows)
    }

    /// `count` rows of sheet `sheet` from row `from`, as the grid shows
    /// them: out of the snapshot for a sheet it holds, and calculated and
    /// formatted now for a paged one (D80). Rows past the end are not there.
    pub fn rows(&self, sheet: usize, from: usize, count: usize) -> Vec<Vec<CellSnap>> {
        let Some(t) = self.snapshot.sheets.get(sheet) else {
            return Vec::new();
        };
        let end = from.saturating_add(count).min(t.nrows);
        if from >= end {
            return Vec::new();
        }
        if !t.paged {
            return t.rows[from..end].to_vec();
        }
        let Some(engine) = &self.engine else {
            return Vec::new();
        };
        let file_rows: Vec<usize> = (from..end).map(|r| self.file_row(sheet, r)).collect();
        let program = engine.program();
        let mut columns: Vec<std::vec::IntoIter<Value>> = (0..t.columns.len())
            .map(|c| engine.rows_shown(sheet, c, &file_rows).into_iter())
            .collect();
        (from..end)
            .map(|_| {
                t.columns
                    .iter()
                    .zip(&mut columns)
                    .enumerate()
                    .map(|(c, (column, values))| {
                        let value = values.next().unwrap_or(Value::Error);
                        let failed = matches!(value, Value::Error);
                        let source = column
                            .formula
                            .as_ref()
                            .map_or(String::new(), |f| format!("= {f}"));
                        let error = self
                            .column_errors
                            .get(sheet)
                            .and_then(|errors| errors.get(c).cloned().flatten())
                            .filter(|_| failed);
                        let format = program.sheets[sheet].cols[c].format.as_ref();
                        cell_snap(engine, &self.style, format, value, source, error)
                    })
                    .collect()
            })
            .collect()
    }

    /// The row of the source that is shown at row `r` of sheet `sheet`:
    /// where a table has been sorted in the grid, the row it was before the
    /// sort, which is the row a formula reads (D215).
    pub fn source_row(&self, sheet: usize, r: usize) -> usize {
        if self.snapshot.sheets.get(sheet).is_none() {
            return r;
        }
        self.file_row(sheet, r)
    }

    /// The row of the file shown at row `r` of a paged sheet: where it has
    /// been sorted in the grid, the order says, and otherwise it is `r`.
    fn file_row(&self, sheet: usize, r: usize) -> usize {
        let t = &self.snapshot.sheets[sheet];
        match self.orders.get(&t.name) {
            Some(order) if order.len() == t.nrows => order.get(r).copied().unwrap_or(r),
            _ => r,
        }
    }

    /// Every value of column `col` of a paged sheet, in the order shown,
    /// with the text each is shown as: what a sort and a filter read where
    /// the snapshot holds no rows.
    fn paged_column(&self, sheet: usize, col: usize) -> Vec<(Value, String)> {
        let Some(engine) = &self.engine else {
            return Vec::new();
        };
        let n = self.row_count(sheet);
        let values = engine.column_shown(sheet, col).unwrap_or_else(|| {
            let all: Vec<usize> = (0..n).collect();
            engine.rows_shown(sheet, col, &all)
        });
        let format = engine.program().sheets[sheet].cols[col].format.as_ref();
        let shown = |v: &Value| match format {
            Some(_) => engine.show_in(v, false, format),
            None => shown_text(engine, v),
        };
        (0..n)
            .map(|r| {
                let v = values
                    .get(self.file_row(sheet, r))
                    .cloned()
                    .unwrap_or(Value::Error);
                let text = shown(&v);
                (v, text)
            })
            .collect()
    }

    /// The count, sum and average of the block of cells from `(top, left)`
    /// to `(bottom, right)`. Numbers add up exactly, as they do in a formula.
    pub fn summary(
        &self,
        sheet: usize,
        (top, left): (usize, usize),
        (bottom, right): (usize, usize),
    ) -> Option<Summary> {
        let t = self.snapshot.sheets.get(sheet)?;
        let (mut count, mut numbers) = (0, 0);
        let mut sum = Value::int(0.into());
        let mut add = |value: &Value| {
            if !matches!(value, Value::Null | Value::Error) {
                count += 1;
            }
            if value.is_numeric() {
                numbers += 1;
                sum = arith(BinOp::Add, &sum, value).ok()?;
            }
            Some(())
        };
        let end = (bottom + 1).min(t.nrows);
        let cols = left..(right + 1).min(t.columns.len());
        if !t.paged {
            for row in t.rows.get(top..end).unwrap_or_default() {
                for cell in row.get(cols.clone()).unwrap_or_default() {
                    add(&cell.value)?;
                }
            }
        } else if let Some(fast) = self.summary_native(sheet, top..end, cols.clone()) {
            (count, numbers, sum) = fast;
        } else if let Some(engine) = &self.engine {
            // A paged sheet's selection can be the whole of a big file, so
            // only the selected columns are read, as values and not as the
            // cells the grid draws, and a block of rows at a time, so that
            // what is held is one block however much is selected. Within a
            // block the values are added row by row, as a held sheet's are,
            // so the two add floats in the same order and agree exactly.
            if self.orders.contains_key(&t.name) && end > top {
                // Sorted, a block's rows are anywhere in the file, and each
                // block would read the span between them again. Asking for
                // the first row and the last reads each column whole, once,
                // and keeps it, so every block is answered from that.
                let ends = [0, t.nrows - 1];
                for c in cols.clone() {
                    engine.rows_shown(sheet, c, &ends);
                }
            }
            let mut from = top;
            while from < end {
                let to = (from + SUMMARY_ROWS).min(end);
                let file_rows: Vec<usize> = (from..to).map(|r| self.file_row(sheet, r)).collect();
                let columns: Vec<Vec<Value>> = cols
                    .clone()
                    .map(|c| engine.rows_shown(sheet, c, &file_rows))
                    .collect();
                for i in 0..file_rows.len() {
                    for column in &columns {
                        add(column.get(i).unwrap_or(&Value::Error))?;
                    }
                }
                from = to;
            }
        }
        let numbers = if numbers == 0 {
            None
        } else {
            let mean = arith(BinOp::Div, &sum, &Value::int(numbers as i64)).ok()?;
            // To two places: it is a glance, not the figure.
            let show = |v: &Value| {
                let v = crate::value::round_digits(v, 2).unwrap_or_else(|_| v.clone());
                format_styled(&v, false, &self.style)
            };
            Some((show(&sum), show(&mean)))
        };
        Some(Summary { count, numbers })
    }

    /// [`Document::summary`] of a big selection of a paged sheet, added up
    /// over the Polars arrays: the count, how many numbers, and their sum.
    /// See [`SummaryJob`].
    fn summary_native(
        &self,
        sheet: usize,
        rows: std::ops::Range<usize>,
        cols: std::ops::Range<usize>,
    ) -> Option<(usize, usize, Value)> {
        let added = self.summary_job(sheet, rows, cols)?.run()?;
        Some((added.count, added.numbers, added.sum()?))
    }

    /// [`Document::summary`] of a big selection of a paged sheet, as a job
    /// for a worker to [`SummaryJob::run`] and [`Document::summary_from`] to
    /// say. The numbers are added in the order the one-by-one path adds
    /// them, row by row and across, and as `arith` adds them: whole numbers
    /// exactly until the first float, and from then on as floats, so the
    /// answer is the same. `None`, and the values are added one by one, for
    /// a small selection, which reads only what it covers, and for columns
    /// this does not read alike: exact numbers other than whole ones, and a
    /// column whose values are converted to the sheet's type.
    pub fn summary_job(
        &self,
        sheet: usize,
        rows: std::ops::Range<usize>,
        cols: std::ops::Range<usize>,
    ) -> Option<SummaryJob> {
        use polars::prelude::DataType;
        let t = self.snapshot.sheets.get(sheet)?;
        let rows = rows.start..rows.end.min(t.nrows);
        if !t.paged || rows.len() <= SUMMARY_ROWS {
            return None;
        }
        let engine = self.engine.as_ref()?;
        let order = self
            .orders
            .get(&t.name)
            .filter(|o| o.len() == t.nrows)
            .cloned();
        let mut columns = Vec::new();
        for c in cols.start..cols.end.min(t.columns.len()) {
            let column = engine.frame_values(sheet, c)?;
            let ty = engine.program().sheets[sheet].cols[c].ty;
            let kind = match (ty, column.dtype()) {
                (S::Int, d) if d.is_integer() && !matches!(d, DataType::UInt64) => Kind::Whole,
                (S::Float, DataType::Float32 | DataType::Float64) => Kind::Float,
                // Counted, with no numbers. A date-time is out of a sheet's
                // reach only millions of years away, so each held is a value.
                (S::String, DataType::String)
                | (S::Boolean, DataType::Boolean)
                | (S::Date, DataType::Date)
                | (S::Time, DataType::Time)
                | (S::Datetime, DataType::Datetime(_, None)) => Kind::Counted,
                _ => return None,
            };
            columns.push((column, kind));
        }
        Some(SummaryJob {
            rows,
            order,
            columns,
        })
    }

    /// What a [`SummaryJob`] added up, as [`Document::summary`] says it.
    pub fn summary_from(&self, added: Added) -> Option<Summary> {
        let (count, numbers, sum) = (added.count, added.numbers, added.sum()?);
        let numbers = if numbers == 0 {
            None
        } else {
            let mean = arith(BinOp::Div, &sum, &Value::int(numbers as i64)).ok()?;
            // To two places: it is a glance, not the figure.
            let show = |v: &Value| {
                let v = crate::value::round_digits(v, 2).unwrap_or_else(|_| v.clone());
                format_styled(&v, false, &self.style)
            };
            Some((show(&sum), show(&mean)))
        };
        Some(Summary { count, numbers })
    }

    pub fn set_cell(&mut self, sheet: usize, row: usize, col: usize, text: &str) -> bool {
        // A sheet calculated from an expression has one formula, as a
        // calculated column does, and no cells of its own to take an edit:
        // whichever cell the cursor was in, the edit is of that formula.
        if self
            .snapshot
            .sheets
            .get(sheet)
            .is_some_and(|t| t.view.is_some())
        {
            return self.set_sheet_formula(sheet, text);
        }
        let is_computed = self
            .snapshot
            .sheets
            .get(sheet)
            .and_then(|t| t.columns.get(col))
            .is_some_and(|c| c.formula.is_some());
        if is_computed {
            return self.set_column_formula(sheet, col, text);
        }
        self.set_cells(sheet, &[(row, col, text.to_string())])
    }

    /// Declare the type of columns `cols` of a sheet as `ty`, the name of a
    /// type, or with `None` leave them to take their type from their cells.
    /// One undoable step. A cell that does not fit is left to be an error.
    pub fn set_column_types(&mut self, sheet: usize, cols: &[usize], ty: Option<&str>) -> bool {
        if self.no_cells(sheet).is_some() {
            return false;
        }
        let ty = match ty.map(S::from_name) {
            Some(None) => return false,
            Some(found) => found,
            None => None,
        };
        let Some(snap) = self.snapshot.sheets.get(sheet) else {
            return false;
        };
        let names: Vec<String> = cols
            .iter()
            .filter_map(|&c| snap.columns.get(c))
            .map(|c| c.name.clone())
            .collect();
        let mut text = self.text.clone();
        for name in &names {
            let ast = parse_file(&text, 0, &mut Vec::new());
            let Some(decl) = self.sheet_decl(&ast, sheet) else {
                return false;
            };
            text = retyped(&text, decl, name, ty);
        }
        self.commit(text)
    }

    /// Show the values of columns `cols` of a sheet by `format`, `'.2f` or
    /// `%d %b %Y`, or with no `format` as before. A column whose type is
    /// taken from its cells is declared as it. Refused, changing nothing,
    /// where a format does not read or a column's type does not take it,
    /// with what to write instead.
    pub fn set_column_format(
        &mut self,
        sheet: usize,
        cols: &[usize],
        format: Option<&str>,
    ) -> Result<(), String> {
        if let Some(why) = self.no_cells(sheet) {
            return Err(why);
        }
        let snap = self.snapshot.sheets.get(sheet).ok_or_else(String::new)?;
        let mut columns = Vec::new();
        for column in cols.iter().filter_map(|&c| snap.columns.get(c)) {
            let ty = S::from_name(&column.ty).unwrap_or(S::Any);
            let Some(takes) = Format::conversions_of(ty) else {
                return Err(format!("{} takes no format", column.name));
            };
            let found = match format {
                Some(text) => match Format::parse(text, ty).ok().filter(|f| f.takes(ty)) {
                    Some(f) => Some(f),
                    None => {
                        let takes = takes.replace('`', "");
                        return Err(format!("Not a format: {text}\n{ty} takes {takes}"));
                    }
                },
                None => None,
            };
            columns.push((column.name.clone(), ty, found));
        }
        let mut text = self.text.clone();
        for (name, ty, format) in &columns {
            let ast = parse_file(&text, 0, &mut Vec::new());
            let decl = self.sheet_decl(&ast, sheet).ok_or_else(String::new)?;
            // Declared already, it is as it was; otherwise it is now.
            if !decl.schema.iter().any(|l| &l.name == name) {
                text = retyped(&text, decl, name, Some(*ty));
            }
            let ast = parse_file(&text, 0, &mut Vec::new());
            let decl = self.sheet_decl(&ast, sheet).ok_or_else(String::new)?;
            let line = decl.schema.iter().find(|l| &l.name == name);
            let line = line.ok_or_else(String::new)?;
            text = reformatted(&text, line, format.as_ref());
        }
        self.commit(text);
        Ok(())
    }

    /// Insert `count` empty rows before row `at`.
    pub fn insert_rows(&mut self, sheet: usize, at: usize, count: usize) -> bool {
        self.edit_grid(sheet, |grid| {
            let at = at.min(grid.rows.len());
            for _ in 0..count {
                let blank = grid.blank_row();
                grid.rows.insert(at, (None, blank));
            }
        })
    }

    pub fn delete_rows(&mut self, sheet: usize, first: usize, count: usize) -> bool {
        self.edit_grid(sheet, |grid| {
            let end = (first + count).min(grid.rows.len());
            if first < end {
                grid.rows.drain(first..end);
            }
        })
    }

    /// Delete `count` columns starting at `first`, with their cells or
    /// formulas and their type lines, in one undoable step. A sheet keeps at
    /// least one column, a computed one as readily as a data one. A column
    /// that a formula picks by counting is still the column it was; a
    /// formula that reads a deleted column is left to be shown as an error.
    pub fn delete_columns(
        &mut self,
        sheet: usize,
        first: usize,
        count: usize,
    ) -> Result<(), String> {
        if let Some(why) = self.no_cells(sheet) {
            return Err(why);
        }
        let Some(snap) = self.snapshot.sheets.get(sheet) else {
            return Err("there is no such sheet".into());
        };
        let n = snap.columns.len();
        if count == 0 || first + count > n {
            return Err("there is no such column".into());
        }
        if count == n {
            return Err("a sheet keeps at least one column".into());
        }
        let gone = first..first + count;
        let kept = |k: &usize| !gone.contains(k);
        let name = |k: usize| snap.columns[k].name.clone();
        let names: Vec<String> = gone.clone().map(name).collect();
        let order: Vec<String> = (0..n).filter(kept).map(name).collect();
        // Those that stay close up; those that go are past the end.
        let place: Vec<usize> = (0..n)
            .map(|k| match k {
                k if k < first => k,
                k if gone.contains(&k) => n - count + (k - first),
                k => k - count,
            })
            .collect();
        let mut text = self.followed_to(sheet, false, &place, n - count);

        let ast = parse_file(&text, 0, &mut Vec::new());
        let Some(decl) = self.sheet_decl(&ast, sheet) else {
            return Err("there is no such sheet".into());
        };
        // Whole lines to take out: type lines, and `Name := expression`.
        let line = |s: usize, e: usize| {
            let start = text[..s].rfind('\n').map_or(0, |k| k + 1);
            let end = text[e..].find('\n').map_or(text.len(), |k| e + k + 1);
            (start, end)
        };
        let schema = decl.schema.iter().filter(|l| names.contains(&l.name));
        let formulas = decl.computed.iter().filter(|c| names.contains(&c.name));
        let mut cuts: Vec<(usize, usize)> = schema
            .map(|l| line(l.span.start as usize, l.span.end as usize))
            .chain(formulas.map(|c| line(c.span.start as usize, c.expr_span.end as usize)))
            .collect();
        cuts.sort_unstable();
        for (s, e) in cuts.into_iter().rev() {
            text.replace_range(s..e, "");
        }
        let Some(text) = self.arranged(&text, sheet, &order) else {
            return Err("there is no such sheet".into());
        };
        self.commit(text);
        Ok(())
    }

    /// `text` with the columns of sheet `sheet` in the order of `order`,
    /// their names; a column of the header row that is not among them is
    /// taken out. The header row names every column, and the cells of a
    /// formula column are `*`.
    fn arranged(&self, text: &str, sheet: usize, order: &[String]) -> Option<String> {
        let ast = parse_file(text, 0, &mut Vec::new());
        let decl = self.sheet_decl(&ast, sheet)?;
        let formula = |name: &String| decl.computed.iter().any(|c| &c.name == name);
        let column = |name: &String| decl.header.iter().position(|h| &h.0 == name);

        let mut grid = Grid::of(decl);
        let pick = |cells: &[String]| -> Vec<String> {
            let cell = |name: &String| match column(name) {
                Some(k) => cells.get(k).cloned().unwrap_or_default(),
                None => FORMULA_CELL.to_string(),
            };
            order.iter().map(cell).collect()
        };
        for (_, row) in &mut grid.rows {
            *row = pick(row);
        }
        grid.header = order.to_vec();
        grid.formula = order.iter().map(formula).collect();
        Some(grid.write(decl, text))
    }

    /// Add an empty data column at the end of the header row.
    pub fn add_column(&mut self, sheet: usize, name: &str) -> Result<(), String> {
        if let Some(why) = self.no_cells(sheet) {
            return Err(why);
        }
        let name = valid_name(name)?.to_string();
        let Some(snap) = self.snapshot.sheets.get(sheet) else {
            return Err("there is no such sheet".into());
        };
        if snap.columns.iter().any(|c| c.name == name) {
            return Err(format!(
                "sheet `{}` already has a column `{name}`",
                snap.name
            ));
        }
        // A formula column that the header row fails to name moves along one.
        let n = snap.columns.len();
        let ast = self.ast();
        let named = self.sheet_decl(&ast, sheet).map_or(n, |d| d.header.len());
        let place: Vec<usize> = (0..n).map(|k| if k < named { k } else { k + 1 }).collect();
        let followed = self.followed_to(sheet, false, &place, n + 1);
        let text = self.grid_edited(&followed, sheet, |grid| {
            // A lone column written without outer pipes stored its empty
            // cells as `""`.
            if grid.header.len() == 1 {
                for (_, row) in &mut grid.rows {
                    if row[0] == "\"\"" {
                        row[0].clear();
                    }
                }
            }
            grid.header.push(name);
            grid.formula.push(false);
            for (_, row) in &mut grid.rows {
                row.push(String::new());
            }
        });
        if let Some(text) = text {
            self.commit(text);
        }
        Ok(())
    }

    /// Rename a column, and every reference to it, in one undoable step.
    pub fn rename_column(&mut self, sheet: usize, col: usize, name: &str) -> Result<(), String> {
        if let Some(why) = self.read_only(sheet) {
            return Err(why);
        }
        let new = valid_name(name)?.to_string();
        let Some(snap) = self.snapshot.sheets.get(sheet) else {
            return Err("there is no such sheet".into());
        };
        let Some(old) = snap.columns.get(col).map(|c| c.name.clone()) else {
            return Err("there is no such column".into());
        };
        if new == old {
            return Ok(());
        }
        if snap.columns.iter().any(|c| c.name == new) {
            return Err(format!(
                "sheet `{}` already has a column `{new}`",
                snap.name
            ));
        }
        let taken = self.snapshot.consts.iter().any(|c| c.name == new)
            || self.snapshot.sheets.iter().any(|t| t.name == new);
        if taken {
            return Err(format!("`{new}` is already a constant or a sheet"));
        }
        let ast = self.ast();
        let Some(decl) = self.sheet_decl(&ast, sheet) else {
            return Err("there is no such sheet".into());
        };

        // The declaration: the header cell or `Name :=`, and its type line.
        let header = decl.header.iter().filter(|h| h.0 == old).map(|h| h.1);
        let computed = decl.computed.iter().filter(|c| c.name == old);
        let schema = decl.schema.iter().filter(|l| l.name == old);
        let mut spans: Vec<Span> = header
            .chain(computed.map(|c| c.span))
            .chain(schema.map(|l| l.span))
            .collect();
        // Every place a formula names this column, as the checker resolved it.
        let (program, _) = compile(&self.text, 0);
        let column = program
            .sheets
            .iter()
            .position(|t| t.name == decl.name)
            .and_then(|t| {
                Some((
                    t,
                    program.sheets[t].cols.iter().position(|c| c.name == old)?,
                ))
            });
        let uses = |p: &omasheet_omx::Program| {
            let mut found: Vec<Span> = Vec::new();
            if let Some((t, c)) = column {
                let refs = p.col_refs.iter().filter(|r| (r.1, r.2) == (t, c));
                for r in refs {
                    if !found.contains(&r.0) {
                        found.push(r.0);
                    }
                }
            }
            found
        };
        let before = uses(&program);
        spans.extend(before.iter().copied());
        let mut starts: Vec<usize> = spans
            .iter()
            .map(|s| s.start as usize)
            .filter(|s| self.text[*s..].starts_with(&old))
            .collect();
        starts.sort_unstable();
        starts.dedup();
        if starts.is_empty() {
            return Err(format!("column `{old}` cannot be renamed"));
        }
        let mut text = self.text.clone();
        let mut touched: Vec<&str> = Vec::new();
        for &s in starts.iter().rev() {
            let owner = ast.sheets.iter().rfind(|t| t.span.start as usize <= s);
            if let Some(t) = owner.filter(|t| !touched.contains(&t.name.as_str())) {
                touched.push(&t.name);
            }
            text.replace_range(s..s + old.len(), &new);
        }
        // The new name must not be one that a formula already reads as
        // something else, or that formula would silently change meaning.
        if uses(&compile(&text, 0).0).len() != before.len() {
            return Err(format!(
                "renaming to `{new}` would change what a formula refers to"
            ));
        }
        // Line the changed sheets up again.
        for name in touched {
            let ast = parse_file(&text, 0, &mut Vec::new());
            if let Some(decl) = ast.sheets.iter().find(|t| t.name == name) {
                text = Grid::of(decl).write(decl, &text);
            }
        }
        self.commit(text);
        Ok(())
    }

    /// The text with every row or column of sheet `sheet` that a formula
    /// picks by counting still the one it was, once they are rearranged so
    /// that the k-th is `order[k]`.
    fn followed(&self, sheet: usize, rows: bool, order: &[usize]) -> String {
        let mut place = vec![0; order.len()];
        for (new, &old) in order.iter().enumerate() {
            place[old] = new;
        }
        self.followed_to(sheet, rows, &place, order.len())
    }

    /// The same, for the k-th going to `place[k]` of `count` in all: more
    /// than there were, when some are being added.
    fn followed_to(&self, sheet: usize, rows: bool, place: &[usize], count: usize) -> String {
        let ast = self.ast();
        let name = self.snapshot.sheets.get(sheet).map(|t| t.name.as_str());
        let Some(t) = ast
            .sheets
            .iter()
            .position(|t| Some(t.name.as_str()) == name)
        else {
            return self.text.clone();
        };
        let edits = follow_move(&ast, &self.text, t, rows, place, count);
        rewritten(&self.text, &ast, edits)
    }

    /// Make the steps taken since there were `mark` to undo into one.
    fn squash(&mut self, mark: usize) {
        self.undo.truncate(mark + 1);
    }

    /// Add an empty data column as column `at`, in one undoable step.
    pub fn insert_column(&mut self, sheet: usize, at: usize, name: &str) -> Result<(), String> {
        if let Some(why) = self.no_cells(sheet) {
            return Err(why);
        }
        let mark = self.undo.len();
        let ast = self.ast();
        let named = self.sheet_decl(&ast, sheet).map(|d| d.header.len());
        self.add_column(sheet, name)?;
        // It was put at the end of the header row.
        let last = self.snapshot.sheets[sheet].columns.len() - 1;
        let from = named.unwrap_or(last).min(last);
        self.move_columns(sheet, from, 1, at.min(last))?;
        self.squash(mark);
        Ok(())
    }

    /// Add `Name := expression` as column `at`, in one undoable step.
    pub fn insert_computed(
        &mut self,
        sheet: usize,
        at: usize,
        name: &str,
        expr: &str,
    ) -> Result<(), String> {
        let mark = self.undo.len();
        self.add_computed(sheet, name, expr)?;
        let last = self.snapshot.sheets[sheet].columns.len() - 1;
        self.move_columns(sheet, last, 1, at.min(last))?;
        self.squash(mark);
        Ok(())
    }

    /// Move `count` rows starting at `first` so that the first of them
    /// becomes row `to`, in one undoable step. The rows keep their cells.
    /// A row that a formula picks by counting is still the row it was: a
    /// cell's `*-1` is counted again, and so is a number anywhere in the
    /// file. A formula column goes on reading the row before, in the new
    /// order.
    pub fn move_rows(
        &mut self,
        sheet: usize,
        first: usize,
        count: usize,
        to: usize,
    ) -> Result<(), String> {
        if let Some(why) = self.no_cells(sheet) {
            return Err(why);
        }
        let Some(snap) = self.snapshot.sheets.get(sheet) else {
            return Err("there is no such sheet".into());
        };
        let n = snap.rows.len();
        if count == 0 || first + count > n || to + count > n {
            return Err("there is no such row".into());
        }
        if to == first {
            return Ok(());
        }
        let mut order: Vec<usize> = (0..n).collect();
        let block: Vec<usize> = order.drain(first..first + count).collect();
        order.splice(to..to, block);
        let followed = self.followed(sheet, true, &order);
        let text = self.grid_edited(&followed, sheet, |grid| {
            // Each line of the sheet keeps its place and takes the cells of
            // the row that now belongs there.
            let cells: Vec<Vec<String>> = grid.rows.iter().map(|(_, row)| row.clone()).collect();
            for (slot, &from) in grid.rows.iter_mut().zip(&order) {
                if let Some(row) = cells.get(from) {
                    slot.1 = row.clone();
                }
            }
        });
        if let Some(text) = text {
            self.commit(text);
        }
        Ok(())
    }

    /// Put the rows of a sheet in the order of column `col`, in one
    /// undoable step: the rows are rearranged in the text, as a sort in
    /// Excel rearranges them in the worksheet, and stay so once the sort is
    /// done with. The order is the `sort` stage's: ties keep the order they
    /// came in, and an empty cell is smallest, so first ascending and last
    /// descending (D95). A formula that picks a row by counting is
    /// followed, as it is by [`Document::move_rows`].
    pub fn sort_rows(&mut self, sheet: usize, col: usize, desc: bool) -> Result<(), String> {
        match self.snapshot.sheets.get(sheet) {
            Some(t) if t.grouped => return self.sort_table(sheet, col, desc),
            Some(t) if t.view.is_some() => return self.sort_view(sheet, col, desc),
            Some(t) if t.source.is_some() => return self.sort_table(sheet, col, desc),
            _ => {}
        }
        if let Some(why) = self.no_cells(sheet) {
            return Err(why);
        }
        let Some(snap) = self.snapshot.sheets.get(sheet) else {
            return Err("there is no such sheet".into());
        };
        if col >= snap.columns.len() {
            return Err("there is no such column".into());
        }
        let mut order: Vec<usize> = (0..snap.rows.len()).collect();
        order.sort_by(|&a, &b| {
            let o = crate::value::sort_order(&snap.rows[a][col].value, &snap.rows[b][col].value)
                .unwrap_or(Ordering::Equal);
            if desc { o.reverse() } else { o }
        });
        if order.iter().enumerate().all(|(k, &from)| k == from) {
            return Ok(());
        }
        let followed = self.followed(sheet, true, &order);
        let text = self.grid_edited(&followed, sheet, |grid| {
            let cells: Vec<Vec<String>> = grid.rows.iter().map(|(_, row)| row.clone()).collect();
            for (slot, &from) in grid.rows.iter_mut().zip(&order) {
                if let Some(row) = cells.get(from) {
                    slot.1 = row.clone();
                }
            }
        });
        if let Some(text) = text {
            self.commit(text);
        }
        Ok(())
    }

    /// The rows of sheet `sheet` a filter shows, in the grid's order: those
    /// each filtered column's expression is true of, `*` in it being the
    /// row's cell in that column (filter-by-expression D160). A filter is
    /// the grid's view of a sheet, so it changes nothing. A table's is
    /// answered by Polars over the column where it lowers (D162). The
    /// first expression that fails is the answer instead, as
    /// `Column: why`.
    pub fn shown_rows(
        &self,
        sheet: usize,
        filters: &[(String, String)],
    ) -> Result<Vec<usize>, String> {
        let job = self.filter_job(sheet, filters)?;
        match job.run() {
            Some(rows) => Ok(rows),
            None => self.shown_rows_here(sheet, filters),
        }
    }

    /// [`Document::shown_rows`] as a job, for a worker to
    /// [`FilterJob::run`]: each column's filter checked and lowered here,
    /// and answered here where it does not lower. The first that fails is
    /// the answer instead, as `Column: why`.
    pub fn filter_job(
        &self,
        sheet: usize,
        filters: &[(String, String)],
    ) -> Result<FilterJob, String> {
        let mut job = FilterJob {
            rows: 0,
            masks: Vec::new(),
            plans: Vec::new(),
            order: None,
        };
        let Some(t) = self.snapshot.sheets.get(sheet) else {
            return Ok(job);
        };
        job.rows = t.nrows;
        job.order = self
            .orders
            .get(&t.name)
            .filter(|o| o.len() == t.nrows)
            .cloned();
        let Some(engine) = self.engine.as_ref() else {
            return Ok(job);
        };
        for (name, text) in filters.iter().filter(|(_, text)| !text.trim().is_empty()) {
            let Some(c) = t.columns.iter().position(|c| &c.name == name) else {
                continue;
            };
            let node = omasheet_omx::check::compile_filter(engine.program(), sheet, c, text)
                .map_err(|d| format!("{name}: {}", said(&d[0])))?;
            match engine.filter_plan(sheet, &node) {
                Some(plan) => job.plans.push(plan),
                None => {
                    let mask = engine
                        .filter_mask(sheet, &node)
                        .map_err(|why| format!("{name}: {why}"))?;
                    if mask.len() != t.nrows {
                        return Err(format!("{name}: the filter did not cover the sheet's rows"));
                    }
                    job.masks.push(mask);
                }
            }
        }
        Ok(job)
    }

    /// [`Document::shown_rows`] answered here alone, each filter lowered
    /// where it can be and otherwise a row at a time.
    fn shown_rows_here(
        &self,
        sheet: usize,
        filters: &[(String, String)],
    ) -> Result<Vec<usize>, String> {
        let Some(t) = self.snapshot.sheets.get(sheet) else {
            return Ok(Vec::new());
        };
        let n = t.nrows;
        let Some(engine) = self.engine.as_ref() else {
            return Ok((0..n).collect());
        };
        let mut keep = vec![true; n];
        for (name, text) in filters.iter().filter(|(_, text)| !text.trim().is_empty()) {
            let Some(c) = t.columns.iter().position(|c| &c.name == name) else {
                continue;
            };
            let node = omasheet_omx::check::compile_filter(engine.program(), sheet, c, text)
                .map_err(|d| format!("{name}: {}", said(&d[0])))?;
            let mask = engine
                .filter_mask(sheet, &node)
                .map_err(|why| format!("{name}: {why}"))?;
            if mask.len() != n {
                return Err(format!("{name}: the filter did not cover the sheet's rows"));
            }
            for (k, m) in keep.iter_mut().zip(mask) {
                *k &= m;
            }
        }
        let order = self.orders.get(&t.name).filter(|o| o.len() == n);
        Ok((0..n)
            .filter(|&p| keep[order.map_or(p, |o| o[p])])
            .collect())
    }

    /// Whether `text` is a filter column `col` of sheet `sheet` takes:
    /// checked, and no row read (filter-by-expression D172). `Err` says
    /// what is wrong with it, as the popup shows it.
    pub fn check_filter(&self, sheet: usize, col: usize, text: &str) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }
        let Some(engine) = self.engine.as_ref() else {
            return Ok(());
        };
        omasheet_omx::check::compile_filter(engine.program(), sheet, col, text)
            .map(|_| ())
            .map_err(|d| said(&d[0]))
    }

    /// A sheet calculated from an expression is sorted by its expression:
    /// it comes to end in `|> sort(Column)`, or `|> sort(Column, desc)`, in
    /// place of a sort it ended in already. So the sort is written and
    /// saved, as a sheet of its own rows is by rearranging them.
    fn sort_view(&mut self, sheet: usize, col: usize, desc: bool) -> Result<(), String> {
        let Some(t) = self.snapshot.sheets.get(sheet) else {
            return Err("there is no such sheet".into());
        };
        let (Some(view), Some(column)) = (t.view.clone(), t.columns.get(col)) else {
            return Err("there is no such column".into());
        };
        let name = column.name.clone();
        let base = without_last_sort(&view);
        let dir = if desc { ", desc" } else { "" };
        let expr = format!("{base} |> sort({name}{dir})");
        if expr == view {
            return Ok(());
        }
        if self.set_sheet_formula(sheet, &expr) {
            Ok(())
        } else {
            Err("the sheet cannot be sorted".into())
        }
    }

    /// A table is sorted in the grid alone: its rows are its file's, which
    /// is never written. The rows are put in order from the order they are
    /// shown in, so ties keep that order and two sorts compose.
    fn sort_table(&mut self, sheet: usize, col: usize, desc: bool) -> Result<(), String> {
        let Some(t) = self.snapshot.sheets.get(sheet) else {
            return Err("there is no such sheet".into());
        };
        if col >= t.columns.len() {
            return Err("there is no such column".into());
        }
        let shown = self
            .order_by_polars(sheet, col, desc)
            .unwrap_or_else(|| self.order_by_values(sheet, col, desc));
        self.put_in_order(sheet, shown)
    }

    /// [`Document::sort_rows`] of a table, as a job for a worker to
    /// [`SortJob::run`] and then [`Document::finish_sort`]: the column, in
    /// the order shown. `None` where the sheet is not a table or Polars
    /// does not hold the column, and `sort_rows` sorts it here.
    pub fn sort_job(&self, sheet: usize, col: usize, desc: bool) -> Option<SortJob> {
        let t = self.snapshot.sheets.get(sheet)?;
        t.source.as_ref()?;
        let column = self.engine.as_ref()?.frame_column(sheet, col)?;
        let n = t.nrows;
        if column.len() != n {
            return None;
        }
        let series = column.as_materialized_series();
        let before = self.orders.get(&t.name).filter(|o| o.len() == n).cloned();
        let shown = match &before {
            Some(order) => {
                let idx: Vec<polars::prelude::IdxSize> = order
                    .iter()
                    .map(|&r| r as polars::prelude::IdxSize)
                    .collect();
                series.take_slice(&idx).ok()?
            }
            None => series.clone(),
        };
        Some(SortJob {
            sheet,
            col,
            desc,
            shown,
            before,
        })
    }

    /// Put a table in the order a [`SortJob`] found, `shown`: unless the
    /// table has changed since the job was made, and then it is sorted
    /// here, as [`Document::sort_rows`] would.
    pub fn finish_sort(&mut self, job: SortJob, shown: Vec<usize>) -> Result<(), String> {
        let current = self.snapshot.sheets.get(job.sheet).map(|t| {
            (
                t.nrows,
                self.orders
                    .get(&t.name)
                    .filter(|o| o.len() == t.nrows)
                    .cloned(),
            )
        });
        let unchanged = match current {
            Some((n, order)) => {
                n == shown.len()
                    && match (&order, &job.before) {
                        (None, None) => true,
                        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                        _ => false,
                    }
            }
            None => false,
        };
        if !unchanged {
            return self.sort_rows(job.sheet, job.col, job.desc);
        }
        self.put_in_order(job.sheet, shown)
    }

    /// Show a table's rows in the order `shown`, the places of the rows as
    /// they are shown now: one step to undo.
    fn put_in_order(&mut self, sheet: usize, shown: Vec<usize>) -> Result<(), String> {
        let t = &self.snapshot.sheets[sheet];
        let n = t.nrows;
        let before = self.orders.get(&t.name).filter(|o| o.len() == n);
        let order: Vec<usize> = match before {
            Some(before) => shown.iter().map(|&p| before[p]).collect(),
            None => shown.clone(),
        };
        // Already in this order: nothing to do, and no step to undo.
        if shown.iter().enumerate().all(|(k, &p)| k == p) {
            return Ok(());
        }
        let name = t.name.clone();
        let previous = Step {
            text: self.text.clone(),
            orders: self.orders.clone(),
        };
        self.push_step(previous);
        let rows = std::mem::take(&mut self.snapshot.sheets[sheet].rows);
        if !rows.is_empty() {
            let mut rows: Vec<Option<Vec<CellSnap>>> = rows.into_iter().map(Some).collect();
            self.snapshot.sheets[sheet].rows =
                shown.iter().filter_map(|&p| rows[p].take()).collect();
        }
        self.orders.insert(name, Arc::new(order));
        Ok(())
    }

    /// The rows of a table's sheet as shown, in the order column `col` puts
    /// them, by their place in what is shown now: a stable sort, so ties
    /// keep the order a sort before gave them. A Polars `arg_sort` of the
    /// column (D158), which orders as [`crate::value::sort_order`] does: an
    /// empty cell first ascending and last descending, a `NaN` greatest,
    /// text by its bytes. `None` where the column is not one Polars holds,
    /// for [`Document::order_by_values`] to sort.
    fn order_by_polars(&self, sheet: usize, col: usize, desc: bool) -> Option<Vec<usize>> {
        Some(self.sort_job(sheet, col, desc)?.run())
    }

    /// [`Document::order_by_polars`], a value at a time: for a column
    /// Polars does not hold, and the measure the other is held to.
    fn order_by_values(&self, sheet: usize, col: usize, desc: bool) -> Vec<usize> {
        let t = &self.snapshot.sheets[sheet];
        let paged: Vec<Value> = match t.paged {
            true => self
                .paged_column(sheet, col)
                .into_iter()
                .map(|(v, _)| v)
                .collect(),
            false => Vec::new(),
        };
        let value = |r: usize| match t.paged {
            true => &paged[r],
            false => &t.rows[r][col].value,
        };
        let mut shown: Vec<usize> = (0..t.nrows).collect();
        shown.sort_by(|&a, &b| {
            let o = crate::value::sort_order(value(a), value(b)).unwrap_or(Ordering::Equal);
            if desc { o.reverse() } else { o }
        });
        shown
    }

    /// Move `count` columns starting at `first` so that the first of them
    /// becomes column `to`, in one undoable step. Data columns and formula
    /// columns go anywhere among each other. A column that a formula picks
    /// by counting, from its own (`[*-1; *]`) or by number, is still the
    /// column it was.
    pub fn move_columns(
        &mut self,
        sheet: usize,
        first: usize,
        count: usize,
        to: usize,
    ) -> Result<(), String> {
        if let Some(why) = self.no_cells(sheet) {
            return Err(why);
        }
        let Some(snap) = self.snapshot.sheets.get(sheet) else {
            return Err("there is no such sheet".into());
        };
        let n = snap.columns.len();
        if count == 0 || first + count > n || to + count > n {
            return Err("there is no such column".into());
        }
        if to == first {
            return Ok(());
        }
        let mut order: Vec<usize> = (0..n).collect();
        let block: Vec<usize> = order.drain(first..first + count).collect();
        order.splice(to..to, block);
        let names: Vec<String> = order
            .iter()
            .map(|&k| snap.columns[k].name.clone())
            .collect();

        let followed = self.followed(sheet, false, &order);
        let Some(text) = self.arranged(&followed, sheet, &names) else {
            return Err("the columns cannot be moved".into());
        };
        self.commit(text);
        Ok(())
    }

    /// Add `Name := expression` to a sheet, as its last column: the header
    /// row names it, with `*` for its cells.
    pub fn add_computed(&mut self, sheet: usize, name: &str, expr: &str) -> Result<(), String> {
        if let Some(why) = self.read_only(sheet) {
            return Err(why);
        }
        let name = valid_name(name)?.to_string();
        let expr = expr.trim().trim_start_matches('=').trim();
        if expr.is_empty() {
            return Err("a computed column needs an expression".into());
        }
        let Some(n) = self.snapshot.sheets.get(sheet).map(|t| t.columns.len()) else {
            return Err("there is no such sheet".into());
        };
        // A column counted from the end is one further from it.
        let place: Vec<usize> = (0..n).collect();
        let followed = self.followed_to(sheet, false, &place, n + 1);
        let ast = parse_file(&followed, 0, &mut Vec::new());
        let Some(decl) = self.sheet_decl(&ast, sheet) else {
            return Err("there is no such sheet".into());
        };
        if self.snapshot.sheets[sheet]
            .columns
            .iter()
            .any(|c| c.name == name)
        {
            return Err(format!(
                "sheet `{}` already has a column `{name}`",
                decl.name
            ));
        }
        let mut grid = Grid::of(decl);
        // A lone column written without outer pipes stored its empty
        // cells as `""`.
        if grid.header.len() == 1 {
            for (_, row) in &mut grid.rows {
                if row[0] == "\"\"" {
                    row[0].clear();
                }
            }
        }
        grid.header.push(name.clone());
        grid.formula.push(true);
        for (_, row) in &mut grid.rows {
            row.push(FORMULA_CELL.to_string());
        }
        let mut text = grid.write(decl, &followed);

        let ast = parse_file(&text, 0, &mut Vec::new());
        let Some(decl) = self.sheet_decl(&ast, sheet) else {
            return Err("there is no such sheet".into());
        };
        let rows_end = decl
            .row_lines
            .last()
            .or(decl.separator_line.as_ref())
            .map_or(decl.header_line.end, |s| s.end) as usize;
        let computed_end = decl.computed.iter().map(|c| c.expr_span.end as usize).max();
        let (at, gap) = match computed_end {
            Some(end) if end > rows_end => (end, "\n"),
            _ => (rows_end, "\n\n"),
        };
        text.insert_str(at, &format!("{gap}{name} := {}", expr.replace('\n', " ")));
        self.commit(text);
        Ok(())
    }

    /// Change the expression of a computed column.
    /// The expression a sheet is calculated from, changed in place. One
    /// undoable step, as a column's formula is (D90).
    pub fn set_sheet_formula(&mut self, sheet: usize, expr: &str) -> bool {
        // A formula may be written over several lines, the file reading a
        // line that is indented as a continuation of the one above. So a
        // newline is kept, and what follows it is indented to say so.
        let expr = expr.trim().trim_start_matches(":=").trim();
        let expr = expr
            .lines()
            .map(str::trim_end)
            .collect::<Vec<&str>>()
            .join("\n    ");
        if expr.is_empty() {
            return false;
        }
        // Only a sheet that has one: a table's rows come from its file, and
        // a sheet of its own rows has them in the text.
        if self
            .snapshot
            .sheets
            .get(sheet)
            .is_none_or(|t| t.view.is_none())
        {
            return false;
        }
        let ast = self.ast();
        let Some(span) = self
            .sheet_decl(&ast, sheet)
            .and_then(|decl| decl.formula.as_ref())
            .map(|f| f.expr_span)
        else {
            return false;
        };
        let mut text = self.text.clone();
        text.replace_range(span.start as usize..span.end as usize, &expr);
        self.commit(text)
    }

    pub fn set_column_formula(&mut self, sheet: usize, col: usize, expr: &str) -> bool {
        // A decorator on a view takes its formula; only a sheet calculated
        // from an expression has none of its own.
        if self.read_only_cell(sheet, col).is_some() {
            return false;
        }
        let expr = expr
            .trim()
            .trim_start_matches('=')
            .trim()
            .replace('\n', " ");
        if expr.is_empty() {
            return false;
        }
        let ast = self.ast();
        let Some(decl) = self.sheet_decl(&ast, sheet) else {
            return false;
        };
        let Some(name) = self.snapshot.sheets[sheet]
            .columns
            .get(col)
            .map(|c| &c.name)
        else {
            return false;
        };
        let Some(cd) = decl.computed.iter().find(|c| &c.name == name) else {
            return false;
        };
        let mut text = self.text.clone();
        text.replace_range(
            cd.expr_span.start as usize..cd.expr_span.end as usize,
            &expr,
        );
        self.commit(text)
    }

    pub fn set_const(&mut self, index: usize, expr: &str) -> bool {
        let expr = expr
            .trim()
            .trim_start_matches('=')
            .trim()
            .replace('\n', " ");
        let ast = self.ast();
        let Some(name) = self.snapshot.consts.get(index).map(|c| &c.name) else {
            return false;
        };
        let Some(cd) = ast.consts.iter().find(|c| &c.name == name) else {
            return false;
        };
        if expr.is_empty() {
            return false;
        }
        let mut text = self.text.clone();
        text.replace_range(
            cd.expr_span.start as usize..cd.expr_span.end as usize,
            &expr,
        );
        self.commit(text)
    }

    pub fn add_const(&mut self, name: &str, expr: &str) -> Result<(), String> {
        let name = valid_name(name)?.to_string();
        let expr = expr.trim().trim_start_matches('=').trim();
        if expr.is_empty() {
            return Err("a constant needs an expression".into());
        }
        let taken = self.snapshot.consts.iter().any(|c| c.name == name)
            || self.snapshot.sheets.iter().any(|t| t.name == name);
        if taken {
            return Err(format!("the name `{name}` is already used"));
        }
        // Constants go first, before the first sheet.
        let ast = self.ast();
        let at = ast
            .sheets
            .first()
            .map(|t| {
                self.text[..t.span.start as usize]
                    .rfind('\n')
                    .map_or(0, |i| i + 1)
            })
            .unwrap_or(self.text.len());
        let mut text = self.text.clone();
        text.insert_str(
            at,
            &format!("const {name} = {}\n\n", expr.replace('\n', " ")),
        );
        self.commit(text);
        Ok(())
    }

    /// Append a new sheet with one empty row.
    pub fn add_sheet(&mut self, name: &str, formula: Option<&str>) -> Result<(), String> {
        self.insert_sheet(usize::MAX, name, formula)
    }

    /// Put a new sheet in before sheet `at`, or last if there is none
    /// there. It goes above the comment written over that sheet.
    pub fn insert_sheet(
        &mut self,
        at: usize,
        name: &str,
        formula: Option<&str>,
    ) -> Result<(), String> {
        let name = valid_name(name)?.to_string();
        let taken = self.snapshot.consts.iter().any(|c| c.name == name)
            || self.snapshot.sheets.iter().any(|t| t.name == name);
        if taken {
            return Err(format!("the name `{name}` is already used"));
        }
        // A sheet calculated from an expression has no grid of its own:
        // its rows are the view the expression gives (D90).
        let block = match formula.map(str::trim).filter(|f| !f.is_empty()) {
            Some(formula) => format!("sheet {name} := {formula}\n"),
            None => format!(
                "sheet {name}\n\n| A   | B   | C   |\n| --- | --- | --- |\n|     |     |     |\n|     |     |     |\n|     |     |     |\n"
            ),
        };
        let ast = self.ast();
        let text = match self.sheet_decl(&ast, at) {
            Some(decl) => {
                let mut start = self.line_start(decl.span.start as usize);
                while start > 0 {
                    let above = self.line_start(start - 1);
                    if !self.text[above..start].starts_with('#') {
                        break;
                    }
                    start = above;
                }
                format!("{}{block}\n{}", &self.text[..start], &self.text[start..])
            }
            None => {
                let mut text = self.text.trim_end().to_string();
                if !text.is_empty() {
                    text.push_str("\n\n");
                }
                text + &block
            }
        };
        self.commit(text);
        Ok(())
    }

    /// Where the line holding byte `at` starts.
    fn line_start(&self, at: usize) -> usize {
        self.text[..at].rfind('\n').map_or(0, |k| k + 1)
    }

    /// Give sheet `sheet` another name, in its own line and wherever a
    /// formula names it.
    pub fn rename_sheet(&mut self, sheet: usize, name: &str) -> Result<(), String> {
        let new = valid_name(name)?.to_string();
        let ast = self.ast();
        let Some(decl) = self.sheet_decl(&ast, sheet) else {
            return Err("there is no such sheet".into());
        };
        let old = decl.name.clone();
        if new == old {
            return Ok(());
        }
        let taken = self.snapshot.consts.iter().any(|c| c.name == new)
            || self.snapshot.sheets.iter().any(|t| t.name == new);
        if taken {
            return Err(format!("the name `{new}` is already used"));
        }
        let mut spans = vec![decl.span];
        for (_, e) in formulas(&ast) {
            named(&e, &old, &mut spans);
        }
        let mut starts: Vec<usize> = spans
            .iter()
            .map(|s| s.start as usize)
            .filter(|s| self.text[*s..].starts_with(&old))
            .collect();
        starts.sort_unstable();
        starts.dedup();
        let mut text = self.text.clone();
        let mut touched: Vec<String> = Vec::new();
        for &s in starts.iter().rev() {
            let owner = ast.sheets.iter().rfind(|t| t.span.start as usize <= s);
            if let Some(t) = owner {
                let name = if t.name == old { &new } else { &t.name };
                if !touched.contains(name) {
                    touched.push(name.clone());
                }
            }
            text.replace_range(s..s + old.len(), &new);
        }
        // A formula must read what it read: a name that now means something
        // else, or nothing, is a rename not to make.
        if compile(&text, 0).1.len() != compile(&self.text, 0).1.len() {
            return Err(format!(
                "renaming to `{new}` would change what a formula refers to"
            ));
        }
        // Line the changed sheets up again.
        for name in touched {
            let ast = parse_file(&text, 0, &mut Vec::new());
            if let Some(decl) = ast.sheets.iter().find(|t| t.name == name) {
                text = Grid::of(decl).write(decl, &text);
            }
        }
        self.commit(text);
        Ok(())
    }

    /// Take sheet `sheet` out of the file: its line, its types, its rows
    /// and its formulas. A formula elsewhere that reads it is left to be
    /// shown as an error.
    pub fn delete_sheet(&mut self, sheet: usize) -> Result<(), String> {
        let ast = self.ast();
        let Some(decl) = self.sheet_decl(&ast, sheet) else {
            return Err("there is no such sheet".into());
        };
        let (start, mut end) = self.sheet_lines(decl);
        // The blank lines under it go with it.
        while let Some(rest) = self.text[end..].split_inclusive('\n').next() {
            if !rest.trim().is_empty() {
                break;
            }
            end += rest.len();
        }
        let mut text = format!("{}{}", &self.text[..start], &self.text[end..]);
        if end >= self.text.len() {
            text = text.trim_end().to_string();
            if !text.is_empty() {
                text.push('\n');
            }
        }
        self.commit(text);
        Ok(())
    }

    /// Where a sheet's lines start, and end: its own line, its types, its
    /// rows and its formulas, up to the end of the last of them.
    fn sheet_lines(&self, decl: &SheetDecl) -> (usize, usize) {
        let start = self.line_start(decl.span.start as usize);
        let ends = [decl.span, decl.header_line]
            .into_iter()
            .chain(decl.separator_line)
            .chain(decl.row_lines.iter().copied())
            .chain(decl.schema.iter().map(|l| l.span))
            .chain(decl.computed.iter().map(|c| c.expr_span));
        let last = ends.map(|s| s.end as usize).max().unwrap_or(start);
        let end = self.text[last..]
            .find('\n')
            .map_or(self.text.len(), |k| last + k + 1);
        (start, end)
    }

    /// Move sheet `from` to be sheet `to`, with the comment written over it.
    pub fn move_sheet(&mut self, from: usize, to: usize) -> Result<(), String> {
        let ast = self.ast();
        let Some(decl) = self.sheet_decl(&ast, from) else {
            return Err("there is no such sheet".into());
        };
        if from == to {
            return Ok(());
        }
        let (mut start, mut end) = self.sheet_lines(decl);
        while start > 0 {
            let above = self.line_start(start - 1);
            if !self.text[above..start].starts_with('#') {
                break;
            }
            start = above;
        }
        let block = self.text[start..end].trim_end().to_string() + "\n";
        // The blank lines under it go with it.
        while let Some(rest) = self.text[end..].split_inclusive('\n').next() {
            if !rest.trim().is_empty() {
                break;
            }
            end += rest.len();
        }
        let rest = format!("{}{}", &self.text[..start], &self.text[end..]);
        let ast = parse_file(&rest, 0, &mut Vec::new());
        let text = match ast.sheets.get(to) {
            Some(next) => {
                let mut at = rest[..next.span.start as usize]
                    .rfind('\n')
                    .map_or(0, |k| k + 1);
                while at > 0 {
                    let above = rest[..at - 1].rfind('\n').map_or(0, |k| k + 1);
                    if !rest[above..at].starts_with('#') {
                        break;
                    }
                    at = above;
                }
                format!("{}{block}\n{}", &rest[..at], &rest[at..])
                    .trim_end()
                    .to_string()
                    + "\n"
            }
            None => {
                let mut text = rest.trim_end().to_string();
                if !text.is_empty() {
                    text.push_str("\n\n");
                }
                text + &block
            }
        };
        self.commit(text);
        Ok(())
    }

    /// Make sheet `to` a copy of sheet `from`: its columns, types, rows and
    /// formulas, under `from`'s name made free with `_001`, `_002`...
    /// The name it was given.
    pub fn paste_sheet(&mut self, from: usize, to: usize) -> Result<String, String> {
        let ast = self.ast();
        let (Some(source), Some(target)) = (self.sheet_decl(&ast, from), self.sheet_decl(&ast, to))
        else {
            return Err("there is no such sheet".into());
        };
        if from == to {
            return Ok(source.name.clone());
        }
        let taken = |name: &str| {
            self.snapshot.consts.iter().any(|c| c.name == name)
                || self
                    .snapshot
                    .sheets
                    .iter()
                    .enumerate()
                    .any(|(k, t)| k != to && t.name == name)
        };
        let base = copy_base(&source.name);
        let name = (1..)
            .map(|n| format!("{base}_{n:03}"))
            .find(|name| !taken(name))
            .unwrap_or_default();
        let (s0, s1) = self.sheet_lines(source);
        let (t0, t1) = self.sheet_lines(target);
        // Its own line names it: the name after `sheet`.
        let mut block = self.text[s0..s1].to_string();
        let at = source.span.start as usize - s0;
        let named = block[at..].find(&source.name).map(|k| at + k);
        let Some(k) = named else {
            return Err("the sheet's name was not found".into());
        };
        block.replace_range(k..k + source.name.len(), &name);
        if !block.ends_with('\n') {
            block.push('\n');
        }
        let text = format!("{}{block}{}", &self.text[..t0], &self.text[t1..]);
        self.commit(text);
        Ok(name)
    }

    fn rebuild(&mut self) {
        let mut sources = Sources::new();
        let src = sources.add("file", self.text.as_str());
        let (mut program, mut diags) = compile(&self.text, src);
        // Read whatever else is wrong: a mistake in one sheet is no reason
        // for a table beside it to lose its rows.
        let base = self.base.clone().unwrap_or_else(|| ".".into());
        let tables =
            crate::table::read_holding(&mut program, &base, &mut diags, &self.whole, self.copying);
        // A file the text no longer names, or one changed since it was read,
        // is not held any longer.
        let named: std::collections::HashSet<std::path::PathBuf> = program
            .sheets
            .iter()
            .filter_map(|s| s.source.as_ref())
            .map(|source| self.file_of(&base.join(&source.path)))
            .collect();
        self.whole.retain(|path, w| {
            named.contains(path) && crate::table::identity(path).as_ref() == Some(&w.identity)
        });
        // A view's row count is settled before the grid is built from it.
        let view_options = Options {
            base: self.base.clone(),
            style: self.style,
            local: self.local,
            ..Options::default()
        };
        let tables = crate::resolve_views(&mut program, &view_options, tables);
        // Kept after the rebuild, for a paged sheet's rows (D80).
        let engine = Engine::owning(
            program,
            Options {
                style: self.style,
                names: self.names.clone(),
                local: self.local,
                ..Options::default()
            },
            tables,
        );
        engine.run();
        diags.extend(engine.take_diags());
        let program = engine.program();
        let ast = self.ast();

        // A cell that does not fit its column says so, and what the column
        // takes, written the way the user types it.
        for sheet in &program.sheets {
            let Some(decl) = ast.sheets.iter().find(|d| d.name == sheet.name) else {
                continue;
            };
            for (cell, col) in decl.rows.iter().flat_map(|row| row.iter().zip(&sheet.cols)) {
                let hint = entry_hint(&self.style, &col.ty.to_string());
                let misfits = diags
                    .iter_mut()
                    .filter(|d| d.span == cell.span && d.message.ends_with("` literal"));
                for d in misfits.filter(|_| !hint.is_empty()) {
                    d.message = format!(
                        "{} has type {}, {} does not match",
                        col.name, col.ty, cell.text
                    );
                    d.help = Some(hint.clone());
                }
            }
        }

        let message = |d: &Diagnostic| match &d.help {
            Some(help) => format!("{} ({help})", d.message),
            None => d.message.clone(),
        };
        let within = |span: Span| diags.iter().find(|d| contains(span, d.span)).map(message);
        let show = |v: &Value| shown_text(&engine, v);
        let exact =
            |v: &Value, quoted: bool, shown: &str| exact_text(v, quoted, shown, &self.style);
        let mut column_errors = Vec::new();

        let mut snapshot = Snapshot::default();
        // Where each problem is, for the problems list to go there. Only a
        // cell that holds one is kept, so this stays as short as the list.
        let mut located: Vec<(Span, usize, usize, usize)> = Vec::new();
        for (t, sheet) in program.sheets.iter().enumerate() {
            let decl = ast.sheets.iter().find(|d| d.name == sheet.name);
            let mut columns = Vec::new();
            let mut column_errors_here = Vec::new();
            let mut column_spans = Vec::new();
            for col in sheet.cols.iter() {
                let computed = decl
                    .filter(|_| matches!(col.kind, ColKind::Computed(_)))
                    .and_then(|d| d.computed.iter().find(|c| c.name == col.name));
                columns.push(ColumnSnap {
                    name: col.name.clone(),
                    ty: if col.ty == S::Any {
                        String::new()
                    } else {
                        col.ty.to_string()
                    },
                    declared: col.declared,
                    formula: computed.map(|c| {
                        self.text[c.expr_span.start as usize..c.expr_span.end as usize].to_string()
                    }),
                    format: col.format.as_ref().map(|f| f.to_string()),
                });
                column_spans.push(computed.map(|ce| ce.expr_span));
                column_errors_here.push(computed.and_then(|ce| within(ce.expr_span)));
            }
            // A big sheet the text does not hold is formatted a page at a
            // time, as the grid asks for it, and none of it here (D80).
            let paged = sheet.nrows > self.paged_rows && decl.is_none_or(|d| d.rows.is_empty());
            // Column-wise before row-wise (D75): a column the dataframe
            // holds is walked once here, where asking for it a cell at a
            // time would find its chunk once per row. A column it does not
            // hold is `None` and is read a cell at a time below, which for
            // one already calculated is quicker than collecting it first.
            let mut shown: Vec<Option<Vec<Value>>> = (0..sheet.cols.len())
                .map(|c| (!paged).then(|| engine.column_shown(t, c)).flatten())
                .collect();
            let nrows = if paged { 0 } else { sheet.nrows };
            let mut rows = Vec::with_capacity(nrows);
            for r in 0..nrows {
                let mut row = Vec::with_capacity(sheet.cols.len());
                for (c, column) in columns.iter().enumerate() {
                    // Taken rather than copied: the snapshot below keeps
                    // the value, and nothing reads this column again.
                    let value = match &mut shown[c] {
                        Some(values) => std::mem::take(&mut values[r]),
                        None => engine.cell_shown(t, c, r),
                    };
                    let failed = matches!(value, Value::Error);
                    let src_cell = decl
                        .filter(|d| c < d.header.len())
                        .and_then(|d| d.rows.get(r))
                        .and_then(|row| row.get(c));
                    let (source, error) = match (&column.formula, src_cell) {
                        (Some(formula), _) => {
                            // One formula is one span over every row, so
                            // the rows that failed are where a problem in
                            // it can be gone to, and row 0 would be a cell
                            // that is perfectly well.
                            if let Some(span) = column_spans[c].filter(|_| failed) {
                                located.push((span, t, r, c));
                            }
                            (
                                format!("= {formula}"),
                                failed.then(|| column_errors_here[c].clone()).flatten(),
                            )
                        }
                        (None, Some(cell)) => {
                            let error = within(cell.span);
                            if error.is_some() {
                                located.push((cell.span, t, r, c));
                            }
                            // A whole number in a column of a base is
                            // edited in that base, however it was written,
                            // `-0xff` for a negative one (D132).
                            let based = match &value {
                                Value::Int(n, b)
                                    if *b != Base::Dec
                                        && omasheet_omx::convert::parse_literal(&cell.text)
                                            .is_some() =>
                                {
                                    Some(b.write(*n))
                                }
                                _ => None,
                            };
                            (based.unwrap_or_else(|| cell.text.clone()), error)
                        }
                        (None, None) => (String::new(), None),
                    };
                    let format = sheet.cols[c].format.as_ref();
                    row.push(cell_snap(
                        &engine,
                        &self.style,
                        format,
                        value,
                        source,
                        error,
                    ));
                }
                rows.push(row);
            }
            // A column of addresses says so in its header, where the file
            // leaves its type to the cells. It stays undeclared, so it is
            // shown muted and nothing holds its cells to it.
            for (c, column) in columns.iter_mut().enumerate() {
                if column.declared || column.formula.is_some() || column.ty != "String" {
                    continue;
                }
                if let Some(found) = narrower_text_type(&rows, c) {
                    column.ty = found.to_string();
                }
            }
            // A column left to its cells is as wide as they are: one that
            // holds `Inf`, `-Inf` or `NaN`, which a division by zero comes
            // to, is a `Float` whatever exact numbers are beside it.
            for (c, column) in columns.iter_mut().enumerate() {
                let exact = sheet.cols[c].ty.is_exact() && !sheet.cols[c].ty.is_int();
                if column.declared || !exact {
                    continue;
                }
                let infinite =
                    |cell: &CellSnap| matches!(cell.display.as_str(), "Inf" | "-Inf" | "NaN");
                if rows.iter().any(|row| infinite(&row[c])) {
                    column.ty = S::Float.to_string();
                }
            }
            // A table sorted in the grid is shown in that order, while its
            // file still has as many rows as the order numbers. A paged
            // sheet takes the order as its rows are asked for.
            let rows = match self.orders.get(&sheet.name) {
                Some(order)
                    if (sheet.source.is_some()
                        || sheet.group.is_some()
                        || sheet.join.is_some())
                        && order.len() == rows.len() =>
                {
                    let mut slots: Vec<Option<Vec<CellSnap>>> =
                        rows.into_iter().map(Some).collect();
                    order
                        .iter()
                        .filter_map(|&r| slots.get_mut(r)?.take())
                        .collect()
                }
                _ => rows,
            };
            column_errors.push(column_errors_here);
            snapshot.sheets.push(SheetSnap {
                name: sheet.name.clone(),
                columns,
                nrows: sheet.nrows,
                paged,
                rows,
                source: sheet.source.as_ref().map(|s| s.path.clone()),
                derived: sheet.rows_from.is_some(),
                view: decl.and_then(|d| d.formula.as_ref()).map(|f| {
                    self.text[f.expr_span.start as usize..f.expr_span.end as usize]
                        .trim()
                        .to_string()
                }),
                grouped: sheet.group.is_some() || sheet.join.is_some(),
            });
        }
        for (i, c) in program.consts.iter().enumerate() {
            let decl = ast.consts.iter().find(|d| d.name == c.name);
            let value = engine.const_value(i).unwrap_or(Value::Error);
            let display = match &value {
                _ if c.columns.is_some() => {
                    format!(
                        "[{}]",
                        c.columns
                            .iter()
                            .flatten()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
                Value::Null => "empty".to_string(),
                Value::Vector(_) | Value::String(..) => engine.show(&value, true),
                other => show(other),
            };
            snapshot.consts.push(ConstSnap {
                name: c.name.clone(),
                source: decl.map_or(String::new(), |d| {
                    self.text[d.expr_span.start as usize..d.expr_span.end as usize].to_string()
                }),
                exact: exact(&value, true, &display)
                    .filter(|f| *f != delocalised(&display, &self.style)),
                display,
                error: decl.and_then(|d| within(d.expr_span)),
            });
        }
        for f in &program.funcs {
            snapshot.funcs.push(FuncSnap {
                name: f.name.clone(),
                usage: f.usage(),
                source: f.source.clone(),
                doc: f.doc.clone(),
            });
        }
        // A formula's span is one for every row it failed in, so each
        // problem takes a cell of its own: without that, every failure in
        // one column would be gone to at the same row.
        let mut taken = vec![false; located.len()];
        snapshot.problems = diags
            .iter()
            .map(|d| {
                let (line, column) = sources.line_col(d.span.src, d.span.start);
                let at = located
                    .iter()
                    .enumerate()
                    .find(|(i, (span, ..))| !taken[*i] && contains(*span, d.span))
                    .map(|(i, _)| i);
                if let Some(i) = at {
                    taken[i] = true;
                }
                Problem {
                    line,
                    column,
                    message: message(d),
                    cell: at.map(|i| {
                        let (_, t, r, c) = located[i];
                        (t, r, c)
                    }),
                }
            })
            .collect();
        self.snapshot = snapshot;
        self.column_errors = column_errors;
        self.engine = Some(engine);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALES: &str = "# sales\nconst Rate = 20%\n\nsheet Sales\n\n| Month | Revenue | Cost | Profit | Tax |\n| ----- | ------- | ---- | ------ | --- |\n| Jan   | 100     | 60   | *      | *   |\n| Feb   | 120     | 70   | *      | *   |\n\nProfit := Revenue - Cost\nTax := Profit * Rate\n";

    fn col(doc: &Document, sheet: usize, col: usize) -> Vec<String> {
        doc.snapshot().sheets[sheet]
            .rows
            .iter()
            .map(|r| r[col].display.clone())
            .collect()
    }

    #[test]
    fn a_division_by_zero_makes_a_column_left_to_its_cells_a_float() {
        let doc =
            Document::from_text("sheet T\n\n| U | V |\n| - | - |\n| =1/0 | 1.5 |\n| 2.5 | 2 |\n");
        let columns = &doc.snapshot().sheets[0].columns;
        assert_eq!(columns[0].ty, "Float");
        assert_eq!(columns[1].ty, "Decimal");
        assert_eq!(col(&doc, 0, 0), ["Inf", "2.5"]);
        // A declared column keeps its type, and its cell is the error.
        let doc = Document::from_text("sheet T\n\nU : Decimal\n\n| U |\n| - |\n| =1/0 |\n");
        assert_eq!(doc.snapshot().sheets[0].columns[0].ty, "Decimal");
    }

    #[test]
    fn a_value_written_as_a_fraction_makes_a_fraction_column() {
        let mut doc = Document::from_text("sheet T\n\nA | B\n1 | \n2 | 1/2\n");
        assert!(doc.set_cell(0, 0, 1, "1/3"));
        assert_eq!(doc.snapshot().sheets[0].columns[1].ty, "Fraction");
        assert_eq!(col(&doc, 0, 1), ["1/3", "1/2"]);
        // The column is inferred from its cell, not declared, and the cell
        // is kept as it was typed.
        assert!(!doc.text().contains("Fraction"), "{}", doc.text());
        assert!(doc.text().contains("1/3"), "{}", doc.text());
        // A decimal typed beside it makes the column a decimal one, a
        // fraction and a decimal being a decimal.
        assert!(doc.set_cell(0, 1, 1, "0.25"));
        assert_eq!(doc.snapshot().sheets[0].columns[1].ty, "Decimal");
        // Each cell keeps the format its own contents give it (D192).
        assert_eq!(col(&doc, 0, 1), ["1/3", "0.25"]);
        // So does a whole number and a part, proper or not.
        let doc = Document::from_text("sheet T\n\nA\n1 1/2\n5 7/3\n11/3\n");
        assert_eq!(doc.snapshot().sheets[0].columns[0].ty, "Fraction");
        assert_eq!(col(&doc, 0, 0), ["1 1/2", "7 1/3", "3 2/3"]);
        // A decimal alone is a `Decimal`, as is a fraction in a declared one.
        let doc = Document::from_text("sheet T\n\nA\n0.5\n0.25\n");
        assert_eq!(doc.snapshot().sheets[0].columns[0].ty, "Decimal");
        let doc = Document::from_text("sheet T\n\nA : Decimal\n\nA\n1/3\n");
        assert_eq!(doc.snapshot().sheets[0].columns[0].ty, "Decimal");
        // The same for a percentage: the first cell gives the type that is
        // shown, a later one widens it, and the file declares nothing (D194).
        let mut doc = Document::from_text("sheet T\n\nA\n\n\n");
        assert!(doc.set_cell(0, 0, 0, "23%"));
        let column = &doc.snapshot().sheets[0].columns[0];
        assert!(!column.declared && column.ty == "Percent");
        assert!(doc.set_cell(0, 1, 0, "1/2"));
        assert_eq!(doc.snapshot().sheets[0].columns[0].ty, "Decimal");
        assert_eq!(col(&doc, 0, 0), ["23%", "1/2"]);
        assert!(!doc.text().contains("Decimal"), "{}", doc.text());
        // A fraction among text is text.
        let mut doc = Document::from_text("sheet T\n\nA\nabc\n\n");
        assert!(doc.set_cell(0, 1, 0, "1/3"));
        assert!(!doc.text().contains("Fraction"), "{}", doc.text());
    }

    /// A sheet of `rows` empty computed cells in one column, filled by
    /// the sequence `seq`.
    fn seq(seq: &str, rows: usize) -> Vec<String> {
        let cells = "| * |\n".repeat(rows);
        let text = format!("sheet S\n\n| N |\n| - |\n{cells}\nN := {seq}\n");
        let doc = Document::from_text(&text);
        assert!(
            doc.snapshot().problems.is_empty(),
            "{seq}: {:?}",
            doc.snapshot().problems
        );
        col(&doc, 0, 0)
    }

    #[test]
    fn a_sequence_takes_an_arithmetic_or_geometric_step_from_its_terms() {
        assert_eq!(seq("1, 3 ... Inf", 4), ["1", "3", "5", "7"]);
        assert_eq!(seq("1, 2, 4 ... Inf", 4), ["1", "2", "4", "8"]);
        // Two terms are a difference, so a constant ratio needs three.
        assert_eq!(
            seq("1, 1/3 ... Inf", 3),
            ["1", "0.33333333333333333333…", "-0.33333333333333333333…"]
        );
        // A ratio of thirds cannot be inferred: `1/3` is rounded at the
        // widest scale, and multiplying it out does not give `1/9` back,
        // so the terms read as neither arithmetic nor geometric. Ratios of
        // numbers that end, such as halves, are inferred as before.
        assert_eq!(
            seq("1, 0.5, 0.25 ... Inf", 4),
            ["1", "0.5", "0.25", "0.125"]
        );
        // One term alone steps by one.
        assert_eq!(seq("5 ... Inf", 3), ["5", "6", "7"]);
    }

    #[test]
    fn a_sequence_may_give_its_own_rule_through_back_references() {
        assert_eq!(
            seq("0, 1, $-1 + $-2 ... Inf", 7),
            ["0", "1", "1", "2", "3", "5", "8"]
        );
        assert_eq!(seq("1, $-1 * 3 ... Inf", 4), ["1", "3", "9", "27"]);
    }

    #[test]
    fn a_sequence_stops_at_its_limit_and_leaves_the_rest_empty() {
        assert_eq!(seq("1, 3 ... 9", 7), ["1", "3", "5", "7", "9", "", ""]);
        // One that steps over its limit stops before it.
        assert_eq!(seq("1, 3 ... 10", 7), ["1", "3", "5", "7", "9", "", ""]);
        assert_eq!(seq("20, 15 ... 0", 6), ["20", "15", "10", "5", "0", ""]);
    }

    #[test]
    fn a_sequence_of_text_counts_letters_and_digits_up() {
        assert_eq!(seq(r#"'A', 'B' ... Inf"#, 3), ["A", "B", "C"]);
        // It carries at the end of the alphabet, as a workbook's columns do.
        assert_eq!(seq(r#"'Y' ... Inf"#, 4), ["Y", "Z", "AA", "AB"]);
        assert_eq!(seq(r#"'AY' ... 'BA'"#, 4), ["AY", "AZ", "BA", ""]);
        // Only the last run of letters and digits moves.
        assert_eq!(seq(r#"'job-A9' ... Inf"#, 2), ["job-A9", "job-B0"]);
    }

    #[test]
    fn a_sequence_of_named_days_or_months_goes_round() {
        assert_eq!(
            seq(r#"'Monday', 'Tuesday' ... Inf"#, 3),
            ["Monday", "Tuesday", "Wednesday"]
        );
        // Sunday is followed by Monday, and December by January.
        assert_eq!(seq(r#"'Sat', 'Sun' ... Inf"#, 3), ["Sat", "Sun", "Mon"]);
        assert_eq!(seq(r#"'Nov', 'Dec' ... Inf"#, 3), ["Nov", "Dec", "Jan"]);
        assert_eq!(
            seq(r#"'January', 'March' ... Inf"#, 3),
            ["January", "March", "May"]
        );
        // A month with its year carries, and the way it is written is kept.
        assert_eq!(
            seq(r#"'Nov-2026', 'Dec-2026' ... Inf"#, 3),
            ["Nov-2026", "Dec-2026", "Jan-2027"]
        );
        assert_eq!(seq(r#"'mon', 'tue' ... Inf"#, 2), ["mon", "tue"]);
        assert_eq!(
            seq(r#"'Monday' ... 'Wednesday'"#, 4),
            ["Monday", "Tuesday", "Wednesday", ""]
        );
        // Text that is no such name still counts up letter by letter.
        assert_eq!(seq(r#"'AY' ... Inf"#, 3), ["AY", "AZ", "BA"]);
    }

    #[test]
    fn a_sequence_of_dates_or_times_steps_by_days_or_seconds() {
        assert_eq!(
            seq("2025-01-01, 2025-01-08 ... Inf", 3),
            ["2025-01-01", "2025-01-08", "2025-01-15"]
        );
        assert_eq!(seq("09:00, 09:30 ... Inf", 3), ["09:00", "09:30", "10:00"]);
        assert_eq!(
            seq("2025-01-30 ... 2025-02-01", 4),
            ["2025-01-30", "2025-01-31", "2025-02-01", ""]
        );
    }

    #[test]
    fn dates_a_month_apart_step_by_months_rather_than_days() {
        assert_eq!(
            seq("2025-01-01, 2025-02-01, 2025-03-01 ... Inf", 4),
            ["2025-01-01", "2025-02-01", "2025-03-01", "2025-04-01"]
        );
        // The day counted from is the largest given, so the end of a short
        // month goes on to the end of a long one rather than drifting back.
        assert_eq!(
            seq("2025-01-31, 2025-02-28 ... Inf", 4),
            ["2025-01-31", "2025-02-28", "2025-03-31", "2025-04-30"]
        );
        assert_eq!(
            seq("2024-11-30, 2024-12-31 ... Inf", 4),
            ["2024-11-30", "2024-12-31", "2025-01-31", "2025-02-28"]
        );
        // Quarters, where the days between them vary.
        assert_eq!(
            seq("2025-01-15, 2025-04-15 ... Inf", 3),
            ["2025-01-15", "2025-04-15", "2025-07-15"]
        );
        // Days still win where the day of the month does not line up.
        assert_eq!(
            seq("2025-01-01, 2025-01-08 ... Inf", 3),
            ["2025-01-01", "2025-01-08", "2025-01-15"]
        );
        // One date alone steps by a day, as every other sequence steps by one.
        assert_eq!(
            seq("2025-01-30 ... Inf", 3),
            ["2025-01-30", "2025-01-31", "2025-02-01"]
        );
    }

    #[test]
    fn a_date_takes_a_duration_in_months_weeks_or_years() {
        assert_eq!(
            seq("2025-01-15, $-1 + 1.month ... Inf", 3),
            ["2025-01-15", "2025-02-15", "2025-03-15"]
        );
        // `month_end` holds a sequence on the end of each month from any start.
        assert_eq!(
            seq("month_end(2025-01-10), month_end($-1 + 1.month) ... Inf", 4),
            ["2025-01-31", "2025-02-28", "2025-03-31", "2025-04-30"]
        );
        assert_eq!(
            seq("2025-01-01, $-1 + 2.week ... Inf", 3),
            ["2025-01-01", "2025-01-15", "2025-01-29"]
        );
        // A leap day a year on is clamped, and months before go back.
        assert_eq!(
            seq("2024-02-29, $-1 + 1.year ... Inf", 2),
            ["2024-02-29", "2025-02-28"]
        );
        assert_eq!(
            seq("2025-03-31, $-1 - 1.month ... Inf", 3),
            ["2025-03-31", "2025-02-28", "2025-01-28"]
        );
    }

    #[test]
    fn a_sequence_that_cannot_be_worked_out_is_reported() {
        let bad = |seq: &str| {
            let text = format!("sheet S\n\n| N |\n| - |\n| * |\n| * |\n\nN := {seq}\n");
            let doc = Document::from_text(&text);
            doc.snapshot()
                .problems
                .first()
                .map(|p| p.message.clone())
                .unwrap_or_default()
        };
        assert!(bad("1, 2, 5 ... Inf").contains("neither an arithmetic nor a geometric"));
        assert!(bad("2025-01-01, 2025-01-05, 2025-01-20 ... Inf").contains("evenly spaced"));
        assert!(bad("$-1 + 1 ... Inf").contains("term to start from"));
    }

    #[test]
    fn snapshot_shows_values_and_sources() {
        let doc = Document::from_text(SALES);
        let t = &doc.snapshot().sheets[0];
        assert_eq!(t.name, "Sales");
        assert_eq!(t.columns.len(), 5);
        assert_eq!(t.columns[3].formula.as_deref(), Some("Revenue - Cost"));
        assert_eq!(col(&doc, 0, 3), ["40", "50"]);
        assert_eq!(t.rows[0][1].source, "100");
        assert_eq!(t.rows[0][3].source, "= Revenue - Cost");
        assert!(t.rows[0][1].numeric && !t.rows[0][0].numeric);
        assert_eq!(doc.snapshot().consts[0].display, "20%");
        assert!(doc.snapshot().problems.is_empty());
    }

    #[test]
    fn sorting_rearranges_the_rows_in_one_undoable_step() {
        let file = "sheet T\n\nName | N\nb | 2\na | 3\nc |\nd | 2\n";
        let mut doc = Document::from_text(file);
        // Ascending: the empty cell first, the tie in the order it came.
        doc.sort_rows(0, 1, false).expect("sorts");
        assert_eq!(col(&doc, 0, 0), ["c", "b", "d", "a"]);
        // The text itself is rearranged, so it stays so once saved.
        assert!(
            doc.text()
                .contains("| c    |     |\n| b    | 2   |\n| d    | 2   |\n| a    | 3   |")
        );
        doc.sort_rows(0, 1, true).expect("sorts");
        assert_eq!(col(&doc, 0, 0), ["a", "b", "d", "c"]);
        // Two steps, each undone on its own; an edit is counted, and an
        // undo or a redo is not.
        assert_eq!(doc.edit_count(), 2);
        assert!(doc.undo() && doc.undo());
        assert_eq!(doc.text(), file);
        assert!(doc.redo() && doc.undo());
        assert_eq!(doc.edit_count(), 2);
        // Sorting what is sorted adds no step.
        doc.sort_rows(0, 0, false).expect("sorts");
        let steps = doc.can_undo();
        doc.sort_rows(0, 0, false).expect("sorts");
        assert!(steps);
        assert!(doc.undo());
        assert!(!doc.can_undo());
    }

    #[test]
    fn a_calculated_sheet_is_sorted_by_its_expression() {
        let file = "sheet T\n\nName | N\nb | 2\na | 3\nc | 1\n\nsheet V := T |> head(3)\n";
        let mut doc = Document::from_text(file);
        doc.sort_rows(1, 1, true).expect("sorts");
        assert!(
            doc.text()
                .contains("sheet V := T |> head(3) |> sort(N, desc)"),
            "{}",
            doc.text()
        );
        assert_eq!(col(&doc, 1, 0), ["a", "b", "c"]);
        // A second sort takes the place of the first rather than following it.
        doc.sort_rows(1, 0, false).expect("sorts");
        assert!(
            doc.text()
                .contains("sheet V := T |> head(3) |> sort(Name)\n"),
            "{}",
            doc.text()
        );
        assert_eq!(col(&doc, 1, 0), ["a", "b", "c"]);
        // One step each, and the rows of `T` itself untouched.
        assert_eq!(col(&doc, 0, 0), ["b", "a", "c"]);
        assert!(doc.undo() && doc.undo());
        assert_eq!(doc.text(), file);
        assert_eq!(without_last_sort("T |> resort(N)"), "T |> resort(N)");
    }

    #[test]
    fn a_decorator_on_a_view_takes_a_formula_and_the_data_does_not() {
        let text = "view Passengers := dataframe['titanic.parquet']\n\nName : String\nFare : Float\n\nDouble := Fare * 2\n";
        let mut doc = Document::from_text(text);
        doc.set_base(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples"));
        let names: Vec<_> = doc.snapshot().sheets[0]
            .columns
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(names, ["Name", "Fare", "Double"]);
        // The file's columns take no edit, and say why; the decorator does.
        assert!(
            doc.read_only_cell(0, 0)
                .is_some_and(|w| w.contains("read-only"))
        );
        assert!(doc.read_only_cell(0, 1).is_some());
        assert_eq!(doc.read_only_cell(0, 2), None);
        assert!(!doc.set_cell(0, 0, 1, "7"));
        assert!(doc.set_column_formula(0, 2, "Fare * 3"));
        assert!(
            doc.text().contains("Double := Fare * 3\n"),
            "{}",
            doc.text()
        );
        // A sheet calculated from an expression has no cell of its own.
        let calc = format!("{text}\nsheet Top := Passengers |> head(2)\n");
        let mut doc = Document::from_text(&calc);
        doc.set_base(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples"));
        assert!(doc.read_only_cell(1, 0).is_some());
    }

    #[test]
    fn the_source_row_of_a_sorted_view_is_the_file_row() {
        let table =
            "view Passengers := dataframe['titanic.parquet']\n\nName : String\nFare : Float\n";
        let mut doc = Document::from_text(table);
        doc.set_base(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples"));
        // Unsorted, the row shown is the row of the file.
        assert_eq!(doc.source_row(0, 0), 0);
        assert_eq!(doc.source_row(0, 7), 7);
        // Sorted in the grid, the top row shown is another row of the file.
        doc.sort_rows(0, 1, true).expect("sorts");
        assert_ne!(doc.source_row(0, 0), 0);
        let at = doc.source_row(0, 0);
        assert_eq!(doc.snapshot().sheets[0].rows[0][1].display, "512.3292");
        // A sheet that does not exist is left as it was.
        assert_eq!(doc.source_row(9, 4), 4);
        let _ = at;
    }

    #[test]
    fn a_formula_copied_to_another_sheet_is_made_to_read_where_it_read() {
        let cols: Vec<String> = ["A", "B"].iter().map(|c| c.to_string()).collect();
        let q = |cell: &str| qualified(cell, "T", &cols);
        // An index with no sheet name is into the sheet it came from.
        assert_eq!(q("= [A; *-1] * 2"), "= T[A; *-1] * 2");
        assert_eq!(q("=[B; 3]"), "=T[B; 3]");
        // A column named on its own is that column of the cell's own row.
        assert_eq!(q("= A + B"), "= T[A; *] + T[B; *]");
        // A name that is no column of the sheet is left, and so is a sheet
        // already named, and anything that is not a formula.
        assert_eq!(q("= A * Rate"), "= T[A; *] * Rate");
        assert_eq!(q("= U[A; 1] + [B; 2]"), "= U[A; 1] + T[B; 2]");
        assert_eq!(q("5"), "5");
        assert_eq!(q("= 1 +"), "= 1 +");
        // Formed again, it reads the same.
        assert_eq!(q(&q("= [A; 1] + B")), q("= [A; 1] + B"));
    }

    #[test]
    fn a_table_is_sorted_in_the_grid_alone() {
        let table = "view Passengers := dataframe['titanic.parquet']\n\nSurvived : Integer\nPclass : Integer\nName : String\nSex : String\nAge : Float\nFare : Float\nSiblings : Integer from 'Siblings/Spouses Aboard'\nParents : Integer from 'Parents/Children Aboard'\n";
        let mut doc = Document::from_text(table);
        doc.set_base(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples"));
        doc.sort_rows(0, 5, true).expect("sorts");
        let rows = &doc.snapshot().sheets[0].rows;
        assert_eq!(rows.len(), 887);
        assert_eq!(rows[0][5].display, "512.3292");
        assert_eq!(rows[0][2].display, "Miss. Anna Ward");
        // Nothing is written, so there is nothing to save; but the sort is
        // a step, so there is something to undo.
        assert_eq!(doc.text(), table);
        assert!(doc.can_undo());
        // The order holds through a rebuild, and a second sort starts from
        // it: the ties at 512.3292 stay in the order the first one gave.
        doc.set_base(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples"));
        assert_eq!(
            doc.snapshot().sheets[0].rows[0][2].display,
            "Miss. Anna Ward"
        );
        doc.sort_rows(0, 1, false).expect("sorts");
        let rows = &doc.snapshot().sheets[0].rows;
        assert_eq!(rows[0][1].display, "1");
        assert_eq!(rows[0][2].display, "Miss. Anna Ward");
    }

    #[test]
    fn a_table_sort_is_a_step_to_undo_and_redo() {
        let text = "view Passengers := dataframe['titanic.parquet']\n\nSurvived : Integer\nPclass : Integer\nName : String\nSex : String\nAge : Float\nFare : Float\nSiblings : Integer from 'Siblings/Spouses Aboard'\nParents : Integer from 'Parents/Children Aboard'\n\nsheet Notes\n\nA\n1\n";
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
        // Held whole, and paged as a big table is.
        for paged in [false, true] {
            let mut doc = Document::from_text(text);
            doc.set_base(base);
            if paged {
                doc.set_paged_rows(100);
            }
            assert_eq!(doc.snapshot().sheets[0].paged, paged);
            let names = |doc: &Document| -> Vec<String> {
                doc.rows(0, 0, 3)
                    .into_iter()
                    .map(|r| r[2].display.clone())
                    .collect()
            };
            let unsorted = names(&doc);
            let edits = doc.edit_count();

            doc.sort_rows(0, 5, true).expect("sorts");
            let by_fare = names(&doc);
            assert_ne!(by_fare, unsorted, "paged {paged}");
            assert_eq!(doc.edit_count(), edits + 1);
            // The same sort again changes nothing and is no step.
            doc.sort_rows(0, 5, true).expect("sorts");
            assert_eq!(doc.undo_mark(), 1);

            assert!(doc.undo());
            assert_eq!(names(&doc), unsorted, "undone, paged {paged}");
            assert!(!doc.can_undo());
            assert!(doc.redo());
            assert_eq!(names(&doc), by_fare, "redone, paged {paged}");

            // A sort, an edit of the text, and a second sort, undone one at
            // a time: each takes back its own step and only that.
            assert!(doc.set_cell(1, 0, 0, "2"));
            let edited = doc.text().to_string();
            assert_ne!(edited, text);
            doc.sort_rows(0, 2, false).expect("sorts");
            let by_name = names(&doc);
            assert_ne!(by_name, by_fare);
            assert_eq!(doc.text(), edited, "a sort writes nothing");
            assert!(doc.undo());
            assert_eq!(names(&doc), by_fare);
            assert_eq!(doc.text(), edited);
            assert!(doc.undo());
            assert_eq!(names(&doc), by_fare, "the edit's undo keeps the order");
            assert_eq!(doc.text(), text);
            assert!(doc.undo());
            assert_eq!(names(&doc), unsorted);
            // And forward again, through all three.
            assert!(doc.redo() && doc.redo() && doc.redo());
            assert_eq!(names(&doc), by_name);
            assert_eq!(doc.text(), edited);
        }
    }

    #[test]
    fn a_filter_shows_the_rows_its_expressions_are_true_of() {
        let file = "sheet T\n\nName | N\nb | 2\na | 10\nc |\nd | 2\n";
        let doc = Document::from_text(file);
        let shown = |filters: &[(&str, &str)]| {
            let filters: Vec<(String, String)> = filters
                .iter()
                .map(|(n, e)| (n.to_string(), e.to_string()))
                .collect();
            doc.shown_rows(0, &filters)
        };
        assert_eq!(shown(&[]), Ok(vec![0, 1, 2, 3]));
        assert_eq!(shown(&[("N", "* == 2")]), Ok(vec![0, 3]));
        // An empty cell fails every comparison (D165), and is asked for.
        assert_eq!(shown(&[("N", "* != 2")]), Ok(vec![1]));
        assert_eq!(shown(&[("N", "* != 2 or * == ''")]), Ok(vec![1, 2]));
        assert_eq!(shown(&[("N", "* != 2 or *.is_null()")]), Ok(vec![1, 2]));
        assert_eq!(shown(&[("N", "0 < * <= 5")]), Ok(vec![0, 3]));
        assert_eq!(shown(&[("N", "N > 5")]), Ok(vec![1]));
        // Any function the directory has under a Polars name (D162).
        assert_eq!(shown(&[("N", "sqrt(*) > 3")]), Ok(vec![1]));
        assert_eq!(shown(&[("N", "floor(* / 3) == 3")]), Ok(vec![1]));
        assert_eq!(shown(&[("N", "abs(* - 10) < 5")]), Ok(vec![1]));
        assert_eq!(
            shown(&[("Name", "*.starts_with('a'..'b')")]),
            Ok(vec![0, 1])
        );
        assert_eq!(shown(&[("N", "*.is_first_distinct()")]), Ok(vec![0, 1, 2]));
        // Each column's filter, all at once.
        assert_eq!(
            shown(&[("N", "* == 2 or * > 5"), ("Name", "* != 'b'")]),
            Ok(vec![1, 3])
        );
        // A column the sheet has not got, and an empty expression, hide nothing.
        assert_eq!(
            shown(&[("Gone", "* == 1"), ("N", "  ")]),
            Ok(vec![0, 1, 2, 3])
        );
        // What is wrong is said, by column, before a row is read.
        let wrong = shown(&[("N", "* == 'x'")]).unwrap_err();
        assert!(wrong.starts_with("N: "), "{wrong}");
        assert!(shown(&[("N", "* >")]).unwrap_err().starts_with("N: "));
        let other = shown(&[("N", "Name == 'b'")]).unwrap_err();
        assert!(other.contains("its own column"), "{other}");
        assert!(
            shown(&[("N", "* + 1")])
                .unwrap_err()
                .contains("true or false")
        );
        assert!(
            shown(&[("N", "* > today()")])
                .unwrap_err()
                .contains("cannot call `today`")
        );
        // And checked alone, as the popup does with each key.
        assert_eq!(doc.check_filter(0, 1, "0 < * <= 5"), Ok(()));
        assert_eq!(
            doc.check_filter(0, 1, "*.year == 2025").map_err(|_| ()),
            Err(())
        );
        assert!(
            doc.check_filter(0, 0, "*.matches('(')")
                .unwrap_err()
                .contains("regex")
        );
    }

    #[test]
    fn a_paged_table_shows_what_a_held_one_does() {
        let text = "view Passengers := dataframe['titanic.parquet']\n\nSurvived : Integer\nPclass : Integer\nName : String\nSex : String\nAge : Float\nFare : Float\nSiblings : Integer from 'Siblings/Spouses Aboard'\nParents : Integer from 'Parents/Children Aboard'\n\nTwice := Fare * 2\n\nsheet Firsts := Passengers |> filter(Pclass == 1)\n";
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
        let mut held = Document::from_text(text);
        held.set_base(base);
        let mut paged = Document::from_text(text);
        paged.set_base(base);
        paged.set_paged_rows(100);
        let shown = |doc: &Document,
                     sheet: usize,
                     from: usize,
                     count: usize|
         -> Vec<Vec<(String, String, Option<String>)>> {
            doc.rows(sheet, from, count)
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|c| (c.display, c.source, c.error))
                        .collect()
                })
                .collect()
        };
        for sheet in 0..2 {
            let (h, p) = (
                &held.snapshot().sheets[sheet],
                &paged.snapshot().sheets[sheet],
            );
            assert!(!h.paged && p.paged, "sheet {sheet}");
            assert!(p.rows.is_empty(), "a paged sheet holds no rows");
            assert_eq!(h.nrows, p.nrows);
            assert_eq!(paged.row_count(sheet), h.rows.len());
            let n = h.nrows;
            // The start, a block across the middle, and past the end.
            for (from, count) in [(0, 30), (n / 2 - 7, 30), (n - 5, 30), (n, 5)] {
                assert_eq!(
                    shown(&paged, sheet, from, count),
                    shown(&held, sheet, from, count),
                    "{sheet} {from}"
                );
            }
            assert_eq!(
                paged.summary(sheet, (3, 0), (90, 8)),
                held.summary(sheet, (3, 0), (90, 8))
            );
        }
        let hide = vec![("Sex".to_string(), "* != 'male'".to_string())];
        assert_eq!(paged.shown_rows(0, &hide), held.shown_rows(0, &hide));
        // Sorted in the grid, by a column of the file and by a formula.
        for col in [5, 8] {
            held.sort_rows(0, col, true).unwrap();
            paged.sort_rows(0, col, true).unwrap();
            assert_eq!(
                shown(&paged, 0, 0, 40),
                shown(&held, 0, 0, 40),
                "sorted by {col}"
            );
            assert_eq!(
                shown(&paged, 0, 850, 40),
                shown(&held, 0, 850, 40),
                "sorted by {col}"
            );
            assert_eq!(paged.shown_rows(0, &hide), held.shown_rows(0, &hide));
        }
    }

    #[test]
    fn a_paged_summary_answers_as_a_held_one_reading_only_what_is_selected() {
        let text = "view Passengers := dataframe['titanic.parquet']\n\nSurvived : Integer\nPclass : Integer\nName : String\nSex : String\nAge : Float\nFare : Float\nSiblings : Integer from 'Siblings/Spouses Aboard'\nParents : Integer from 'Parents/Children Aboard'\n\nTwice := Fare * 2\n\nsheet Firsts := Passengers |> filter(Pclass == 1)\n";
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
        let mut held = Document::from_text(text);
        held.set_base(base);
        let mut paged = Document::from_text(text);
        paged.set_base(base);
        paged.set_paged_rows(100);

        // One column of a fresh document is the only column read for it.
        let before = paged.engine.as_ref().unwrap().reads();
        let n = held.snapshot().sheets[0].nrows;
        assert_eq!(
            paged.summary(0, (0, 5), (n - 1, 5)),
            held.summary(0, (0, 5), (n - 1, 5))
        );
        let read: Vec<_> = paged.engine.as_ref().unwrap().reads()[before.len()..].to_vec();
        assert!(
            read.iter().all(|&rc| rc == (0, 5)),
            "a summary of Fare read {read:?}"
        );

        let blocks = |doc: &Document, sheet: usize| {
            let n = doc.snapshot().sheets[sheet].nrows;
            [
                ((0, 0), (n - 1, 8)),       // everything, across many blocks
                ((150, 4), (640, 6)),       // starts and ends inside a block
                ((99, 5), (100, 5)),        // either side of a block's edge
                ((n - 3, 0), (n + 50, 20)), // past the last row and column
                ((n, 0), (n + 5, 8)),       // no rows at all
            ]
        };
        for sheet in 0..2 {
            for (from, to) in blocks(&held, sheet) {
                let got = paged.summary(sheet, from, to);
                assert_eq!(got, held.summary(sheet, from, to), "{sheet} {from:?}");
                assert!(got.is_some());
            }
        }
        // Sorted in the grid, by a column of the file and by a formula: the
        // same rows in a new order, so the same count and the same sum.
        for col in [5, 8] {
            held.sort_rows(0, col, false).unwrap();
            paged.sort_rows(0, col, false).unwrap();
            for (from, to) in blocks(&held, 0) {
                assert_eq!(
                    paged.summary(0, from, to),
                    held.summary(0, from, to),
                    "sorted by {col}, {from:?}"
                );
            }
        }
    }

    #[test]
    fn a_table_held_whole_answers_as_one_read_from_its_file() {
        // A copy of its own, since the file is changed below.
        let dir = std::env::temp_dir().join(format!("omasheet-hold-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("titanic.parquet");
        std::fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/titanic.parquet"),
            &file,
        )
        .unwrap();
        let text = "view Passengers := dataframe['titanic.parquet']\n\nSurvived : Integer\nPclass : Integer\nName : String\nSex : String\nAge : Float\nFare : Float\nSiblings : Integer from 'Siblings/Spouses Aboard'\nParents : Integer from 'Parents/Children Aboard'\n\nTwice := Fare * 2\n";
        let open = || {
            let mut doc = Document::from_text(text);
            doc.set_base(&dir);
            doc.set_paged_rows(100);
            doc
        };
        let (on_disk, mut held) = (open(), open());

        // What there is to hold: the file's eight columns, by the names the
        // file gives them, and not the formula.
        let wanted = held.to_hold();
        assert_eq!(wanted.len(), 1);
        let want = &wanted[0];
        assert_eq!(want.path, file);
        assert_eq!(want.columns.len(), 8);
        assert!(
            want.columns
                .contains(&"Siblings/Spouses Aboard".to_string())
        );
        let whole = crate::table::read_whole(&want.path, &want.columns).unwrap();
        // The estimate errs high, and not by much more than the text costs.
        let size = whole.frame.estimated_size();
        assert!(want.bytes >= size, "estimate {} < {size}", want.bytes);
        assert!(want.bytes < size * 3, "estimate {} vs {size}", want.bytes);

        assert!(held.hold(want.path.clone(), whole.clone()));
        assert!(held.holds(&file));
        assert!(held.to_hold().is_empty(), "nothing more to hold");
        assert!(!held.hold(want.path.clone(), whole.clone()), "held once");
        let n = on_disk.snapshot().sheets[0].nrows;
        let shown = |doc: &Document, from, count| -> Vec<Vec<String>> {
            doc.rows(0, from, count)
                .into_iter()
                .map(|row| row.into_iter().map(|c| c.display).collect())
                .collect()
        };
        for (from, count) in [(0, 30), (n / 2, 30), (n - 5, 30)] {
            assert_eq!(shown(&held, from, count), shown(&on_disk, from, count));
        }
        assert_eq!(
            held.summary(0, (0, 0), (n - 1, 8)),
            on_disk.summary(0, (0, 0), (n - 1, 8))
        );
        let filter = vec![(
            on_disk.snapshot().sheets[0].columns[5].name.clone(),
            "*.is_first_distinct()".to_string(),
        )];
        assert_eq!(held.shown_rows(0, &filter), on_disk.shown_rows(0, &filter));
        // Rows not asked for yet come from memory: with the file taken away
        // they are still there, until a rebuild lets the file go.
        let expected = shown(&on_disk, n / 4, 30);
        let away = dir.join("away.parquet");
        let not_held = open();
        std::fs::rename(&file, &away).unwrap();
        assert_eq!(shown(&held, n / 4, 30), expected);
        assert_ne!(shown(&not_held, n / 4, 30), expected, "the file is gone");
        std::fs::rename(&away, &file).unwrap();

        // The file changed on disk: what was held is not given out again,
        // and a frame read before the change is not taken.
        let mut bytes = std::fs::read(&file).unwrap();
        bytes.extend_from_slice(b"\0");
        std::fs::write(dir.join("other.parquet"), &bytes).unwrap();
        std::fs::rename(dir.join("other.parquet"), &file).unwrap();
        held.set_paged_rows(100);
        assert!(!held.holds(&file), "a changed file is let go");
        assert!(!held.hold(want.path.clone(), whole));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A directory holding `edge.parquet`: 300 rows of the values that
    /// order and show awkwardly, NaN, both infinities, -0.0 beside 0.0,
    /// nulls, empty and accented text, dates, date-times, booleans and
    /// decimals, with ties of each.
    fn edge_file(name: &str) -> std::path::PathBuf {
        use polars::prelude::*;
        // A file of the values that order awkwardly: NaN, both infinities,
        // -0.0 beside 0.0, nulls, empty and accented text, and ties of each.
        let dir = std::env::temp_dir().join(format!("omasheet-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let n = 300;
        let floats = [
            1.5,
            f64::NAN,
            -0.0,
            0.0,
            2.0,
            f64::NEG_INFINITY,
            f64::INFINITY,
            -3.25,
        ];
        let texts = ["b", "", "B", "é", "a", "ab", "a", "Z"];
        let f: Vec<Option<f64>> = (0..n)
            .map(|i| (i % 9 != 4).then(|| floats[i % floats.len()]))
            .collect();
        let int: Vec<Option<i64>> = (0..n)
            .map(|i| (i % 11 != 0).then_some(((i * 7) % 13) as i64 - 6))
            .collect();
        let text: Vec<Option<&str>> = (0..n)
            .map(|i| (i % 10 != 3).then(|| texts[i % texts.len()]))
            .collect();
        let day: Vec<Option<i32>> = (0..n)
            .map(|i| (i % 12 != 5).then_some(((i * 37) % 50) as i32 + 19_000))
            .collect();
        let at: Vec<Option<i64>> = (0..n)
            .map(|i| (i % 13 != 6).then_some(((i * 53) % 40) as i64 * 3_600_000_000))
            .collect();
        let yes: Vec<Option<bool>> = (0..n).map(|i| (i % 7 != 2).then_some(i % 3 == 0)).collect();
        let mut frame = DataFrame::new(vec![
            Column::new("F".into(), f),
            Column::new("I".into(), int),
            Column::new("S".into(), text),
            Column::new("D".into(), day).cast(&DataType::Date).unwrap(),
            Column::new("T".into(), at)
                .cast(&DataType::Datetime(TimeUnit::Microseconds, None))
                .unwrap(),
            Column::new("B".into(), yes),
            Column::new(
                "M".into(),
                (0..n).map(|i| (i % 17) as f64 / 4.0).collect::<Vec<_>>(),
            )
            .cast(&DataType::Decimal(Some(10), Some(2)))
            .unwrap(),
        ])
        .unwrap();
        ParquetWriter::new(std::fs::File::create(dir.join("edge.parquet")).unwrap())
            .finish(&mut frame)
            .unwrap();
        dir
    }

    #[test]
    fn a_table_sorts_by_polars_in_the_order_it_sorts_by_values() {
        let dir = edge_file("sort");
        let titanic = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
        let cases = [
            (
                "view T := dataframe['edge.parquet']\n",
                dir.to_str().unwrap(),
            ),
            (
                "view Passengers := dataframe['titanic.parquet']\n\nSurvived : Integer\nPclass : Integer\nName : String\nSex : String\nAge : Float\nFare : Float\nSiblings : Integer from 'Siblings/Spouses Aboard'\nParents : Integer from 'Parents/Children Aboard'\n\nTwice := Fare * 2\n",
                titanic,
            ),
        ];
        for (text, base) in cases {
            for paged in [false, true] {
                let mut doc = Document::from_text(text);
                doc.set_base(base);
                if paged {
                    doc.set_paged_rows(100);
                }
                assert_eq!(doc.snapshot().sheets[0].paged, paged);
                let cols = doc.snapshot().sheets[0].columns.len();
                assert!(cols >= 7, "{text}");
                // From the file's order, and again after a sort by the second
                // column, which a later sort's ties keep.
                for first in [None, Some(1)] {
                    if let Some(c) = first {
                        doc.sort_rows(0, c, true).unwrap();
                    }
                    for col in 0..cols {
                        for desc in [false, true] {
                            let polars = doc.order_by_polars(0, col, desc);
                            let name = &doc.snapshot().sheets[0].columns[col].name;
                            assert!(polars.is_some(), "{name} is held by Polars");
                            assert_eq!(
                                polars.unwrap(),
                                doc.order_by_values(0, col, desc),
                                "{name} desc {desc} paged {paged} after {first:?}"
                            );
                        }
                    }
                }
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_paged_filter_shows_what_a_held_one_does() {
        let dir = edge_file("filter");
        let titanic = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
        let cases = [
            (
                "view T := dataframe['edge.parquet']\n",
                dir.to_str().unwrap(),
            ),
            (
                "view Passengers := dataframe['titanic.parquet']\n\nSurvived : Integer\nPclass : Integer\nName : String\nSex : String\nAge : Float\nFare : Float\nSiblings : Integer from 'Siblings/Spouses Aboard'\nParents : Integer from 'Parents/Children Aboard'\n\nTwice := Fare * 2\n",
                titanic,
            ),
        ];
        for (text, base) in cases {
            let mut held = Document::from_text(text);
            held.set_base(base);
            let mut paged = Document::from_text(text);
            paged.set_base(base);
            paged.set_paged_rows(100);
            assert!(paged.snapshot().sheets[0].paged && !held.snapshot().sheets[0].paged);
            for sorted in [false, true] {
                if sorted {
                    held.sort_rows(0, 1, true).unwrap();
                    paged.sort_rows(0, 1, true).unwrap();
                }
                let columns = held.snapshot().sheets[0].columns.clone();
                for column in &columns {
                    let mut tests = vec![
                        "*.is_null()",
                        "not *.is_null()",
                        "*.is_duplicated()",
                        "*.is_first_distinct()",
                    ];
                    match column.ty.as_str() {
                        "Integer" | "Int" | "Decimal" => {
                            tests.extend(["* > 1", "0 < * <= 2", "* in [0, 1]", "* != 1"])
                        }
                        "Float" => tests.extend(["* > 1.5", "0.5 < * <= 30.5", "*.is_finite()"]),
                        "String" => tests.extend([
                            "*.starts_with('A')",
                            "*.starts_with('A'..'M')",
                            "*.contains('a')",
                            "*.to_lowercase().ends_with('e')",
                            "*.len_chars() > 3",
                            "* in ['male', 'b']",
                        ]),
                        _ => {}
                    }
                    for test in tests {
                        let filter = vec![(column.name.clone(), test.to_string())];
                        let want = held.shown_rows(0, &filter);
                        assert!(want.is_ok(), "{} {test}: {want:?}", column.name);
                        assert_eq!(
                            paged.shown_rows(0, &filter),
                            want,
                            "{} {test} sorted {sorted}",
                            column.name
                        );
                    }
                }
                // Two columns filtered at once.
                let two = vec![
                    (columns[0].name.clone(), "not *.is_null()".to_string()),
                    (columns[2].name.clone(), "*.is_first_distinct()".to_string()),
                ];
                assert_eq!(paged.shown_rows(0, &two), held.shown_rows(0, &two));
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_filter_reads_nanoseconds_and_milliseconds_as_a_sheet_does() {
        use polars::prelude::*;
        let dir = std::env::temp_dir().join(format!("omasheet-units-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // One nanosecond before 1970 is 1969-12-31 floored, not 1970-01-01
        // truncated; then a day in March 2019, and an empty cell.
        let ns = [Some(-1i64), Some(1_552_608_000_000_000_123), None];
        let ms = [Some(-1i64), Some(1_552_608_000_000), None];
        let mut frame = DataFrame::new(vec![
            Column::new("N".into(), ns)
                .cast(&DataType::Datetime(TimeUnit::Nanoseconds, None))
                .unwrap(),
            Column::new("M".into(), ms)
                .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
                .unwrap(),
        ])
        .unwrap();
        ParquetWriter::new(std::fs::File::create(dir.join("units.parquet")).unwrap())
            .finish(&mut frame)
            .unwrap();
        for paged in [false, true] {
            let mut doc = Document::from_text("view T := dataframe['units.parquet']\n");
            doc.set_base(dir.to_str().unwrap());
            if paged {
                doc.set_paged_rows(1);
            }
            for c in ["N", "M"] {
                let shown = |e: &str| doc.shown_rows(0, &[(c.to_string(), e.to_string())]);
                assert_eq!(shown("*.day == 31"), Ok(vec![0]), "{c} paged {paged}");
                assert_eq!(shown("*.year == 2019"), Ok(vec![1]), "{c} paged {paged}");
                assert_eq!(shown("* >= 2019-03-15T00:00"), Ok(vec![1]), "{c}");
                assert_eq!(shown("* < 1970-01-01T00:00"), Ok(vec![0]), "{c}");
                assert_eq!(shown("*.is_null()"), Ok(vec![2]), "{c}");
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_csv_table_shows_its_sample_and_then_its_copy_alike() {
        let dir = std::env::temp_dir().join(format!("omasheet-csvt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: the one test that sets it; no other reads a CSV.
        unsafe { std::env::set_var("OMASHEET_CACHE", dir.join("cache")) };
        let mut csv = String::from("Name,N,X,Day,At\n");
        for i in 0..1500 {
            let name = if i % 7 == 0 {
                "\"Smith, J\"".to_string()
            } else {
                format!("n{i}")
            };
            let n = if i % 11 == 0 {
                String::new()
            } else {
                (i % 13).to_string()
            };
            csv.push_str(&format!(
                "{name},{n},{}.5,2021-06-{:02},2021-06-01 00:{:02}:{:02}\n",
                i % 5,
                i % 28 + 1,
                i % 60,
                i % 59
            ));
        }
        std::fs::write(dir.join("x.csv"), &csv).unwrap();
        let text = "view T := dataframe['x.csv']\n";
        let open = |screen: bool| {
            let mut doc = if screen {
                Document::on_screen(text, Style::ISO, Names::default())
            } else {
                Document::from_text(text)
            };
            doc.set_base(&dir);
            doc
        };
        let types = |doc: &Document| -> Vec<String> {
            doc.snapshot().sheets[0]
                .columns
                .iter()
                .map(|c| c.ty.clone())
                .collect()
        };
        let shown = |doc: &Document| -> Vec<Vec<String>> {
            doc.rows(0, 0, 1500)
                .into_iter()
                .map(|row| row.into_iter().map(|c| c.display).collect())
                .collect()
        };

        // On a screen, the sample at once, and the copy wanted.
        let mut screen = open(true);
        assert!(
            screen.snapshot().problems.is_empty(),
            "{:?}",
            screen.snapshot().problems
        );
        assert_eq!(screen.snapshot().sheets[0].nrows, crate::table::SAMPLE_ROWS);
        assert_eq!(
            types(&screen),
            ["String", "Integer", "Float", "Date", "Datetime"]
        );
        let csv_path = dir.join("x.csv");
        assert_eq!(screen.to_convert(), vec![csv_path.clone()]);
        let sample = shown(&screen);
        assert_eq!(
            sample[0],
            ["Smith, J", "", "0.5", "2021-06-01", "2021-06-01T00:00"]
        );

        // Copied, it is the whole file, and its first rows are the sample's.
        crate::table::convert(&csv_path).unwrap();
        assert!(screen.converted(&csv_path));
        assert!(screen.to_convert().is_empty());
        assert_eq!(screen.snapshot().sheets[0].nrows, 1500);
        assert_eq!(
            types(&screen),
            ["String", "Integer", "Float", "Date", "Datetime"]
        );
        assert_eq!(shown(&screen)[..sample.len()], sample[..]);

        // Read where it waits for the copy, the same rows.
        let waited = open(false);
        assert_eq!(shown(&waited), shown(&screen));
        // And the copy filters and sums as a Parquet table does.
        assert_eq!(
            screen
                .shown_rows(0, &[("At".into(), "*.minute == 0".into())])
                .unwrap()
                .len(),
            25
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_mistake_in_one_sheet_leaves_a_table_its_rows() {
        let table = "view Passengers := dataframe['titanic.parquet']\n\nSurvived : Integer\nPclass : Integer\nName : String\nSex : String\nAge : Float\nFare : Float\nSiblings : Integer from 'Siblings/Spouses Aboard'\nParents : Integer from 'Parents/Children Aboard'\n";
        let mut doc = Document::from_text(table);
        doc.set_base(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples"));
        let rows = doc.snapshot().sheets[0].rows.len();
        assert_eq!(rows, 887);
        // A sheet beside it that cannot be calculated.
        let wrong = format!("{table}\nsheet Dearest := Passengers |> sort(Nothing)\n");
        assert!(doc.set_text(&wrong));
        assert!(!doc.snapshot().problems.is_empty());
        let passengers = doc
            .snapshot()
            .sheets
            .iter()
            .find(|s| s.name == "Passengers")
            .expect("the table is still a sheet");
        assert_eq!(passengers.rows.len(), rows, "the table lost its rows");
    }

    #[test]
    fn setting_the_text_is_one_step() {
        let mut doc = Document::from_text(SALES);
        assert!(!doc.set_text(SALES));
        assert!(!doc.can_undo());
        assert!(doc.set_text(&SALES.replace("| 100 ", "| 300 ")));
        assert_eq!(col(&doc, 0, 3), ["240", "50"]);
        // Text that is wrong is kept as it is, and said to be wrong.
        let wrong = SALES.replace("Revenue - Cost", "Revenue - Nothing");
        assert!(doc.set_text(&wrong));
        assert_eq!(doc.text(), wrong);
        assert!(!doc.snapshot().problems.is_empty());
        assert!(doc.undo() && doc.undo());
        assert_eq!(doc.text(), SALES);
    }

    #[test]
    fn a_prefixed_value_does_not_set_a_column_type() {
        let text =
            "sheet Regs\n\n| Name | Addr |\n| ---- | ---- |\n| a    |      |\n| b    |      |\n";
        let mut doc = Document::from_text(text);
        // The first cell gives the type that is shown, and the file says
        // nothing about it: only a type line or setting the type does
        // (D191), and the file keeps the text as it was entered (D186).
        assert!(doc.set_cell(0, 0, 1, "0xFF"));
        assert!(!doc.text().contains("Addr :"), "{}", doc.text());
        assert!(doc.text().contains("| 0xFF "), "{}", doc.text());
        let column = &doc.snapshot().sheets[0].columns[1];
        assert!(!column.declared && column.ty == "Hex");
        // The next cell widens the type, and neither cell is altered.
        assert!(doc.set_cell(0, 1, 1, "255"));
        assert!(!doc.text().contains("Addr :"), "{}", doc.text());
        assert_eq!(doc.snapshot().sheets[0].columns[1].ty, "Integer");
        assert_eq!(col(&doc, 0, 1), ["0xff", "255"]);
        // Setting the type puts one format on every cell, and the file
        // still has what was entered.
        assert!(doc.set_column_types(0, &[1], Some("Hex")));
        assert_eq!(col(&doc, 0, 1), ["0xff", "0xff"]);
        assert!(doc.text().contains("| 255 "), "{}", doc.text());
        // An `Integer` entered in a `Binary` column is shown in binary: a
        // way to convert a base on the fly.
        let mut doc = Document::from_text("sheet T\n\nA : Binary\n\n| A |\n| - |\n|   |\n");
        assert!(doc.set_cell(0, 0, 0, "15"));
        assert_eq!(col(&doc, 0, 0), ["0b1111"]);
        assert!(doc.text().contains("| 15 "), "{}", doc.text());
        // A plain number in a declared `Hex` column is edited in hex, and a
        // negative one is written so that it reads back (D132).
        let doc = Document::from_text("sheet T\n\nA : Hex\n\n| A   |\n| --- |\n| 255 |\n| -1  |\n");
        let rows = &doc.snapshot().sheets[0].rows;
        assert_eq!(
            (rows[0][0].source.as_str(), rows[1][0].source.as_str()),
            ("0xff", "-0x1")
        );
        assert_eq!(rows[1][0].exact, None);
        let mut doc = Document::from_text(SALES);
        assert!(doc.set_cell(0, 0, 1, "0x10"));
        assert!(!doc.text().contains(": Hex"), "{}", doc.text());
    }

    #[test]
    fn a_column_type_is_set_and_cleared() {
        let mut doc = Document::from_text(SALES);
        assert!(!doc.snapshot().sheets[0].columns[1].declared);
        // The first type line goes above the header row.
        assert!(doc.set_column_types(0, &[2], Some("Decimal")));
        assert!(
            doc.text()
                .contains("sheet Sales\n\nCost : Decimal\n\n| Month | Revenue")
        );
        let column = &doc.snapshot().sheets[0].columns[2];
        assert!(column.declared && column.ty == "Decimal");
        // The rest follow the header row, their colons in line.
        assert!(doc.set_column_types(0, &[0, 3], Some("String")));
        assert!(
            doc.text()
                .contains("\n\nMonth  : String\nCost   : Decimal\nProfit : String\n\n| Month | ")
        );
        // A formula that does not fit its type is an error, not refused.
        assert!(!doc.snapshot().problems.is_empty());
        assert!(doc.set_column_types(0, &[3], Some("Decimal")));
        assert!(doc.text().contains("Profit : Decimal\n"));
        assert!(doc.snapshot().problems.is_empty());
        assert!(!doc.set_column_types(0, &[3], Some("Decimal")));
        assert!(!doc.set_column_types(0, &[3], Some("Whole")));
        // Cleared one at a time, and the last takes its blank line.
        assert!(doc.set_column_types(0, &[0, 3], None));
        assert!(
            doc.text()
                .contains("sheet Sales\n\nCost   : Decimal\n\n| Month | ")
        );
        assert!(doc.set_column_types(0, &[1, 2], None));
        assert_eq!(doc.text(), SALES);
        assert!(!doc.set_column_types(0, &[2], None));
        assert!(doc.undo() && doc.undo());
        assert!(doc.text().contains("Profit : Decimal\n"));
    }

    #[test]
    fn a_format_stays_with_a_type_that_takes_it() {
        let src = SALES.replace("sheet Sales\n\n", "sheet Sales\n\nCost : Decimal['.2f]\n\n");
        let mut doc = Document::from_text(&src);
        assert!(doc.snapshot().problems.is_empty());
        assert!(doc.set_column_types(0, &[2], Some("Float")));
        assert!(doc.text().contains("Cost : Float['.2f]\n"));
        // An `Integer` takes no `f`, so the format goes with the old type.
        assert!(doc.set_column_types(0, &[2], Some("Integer")));
        assert!(doc.text().contains("Cost : Integer\n"));
        assert!(doc.snapshot().problems.is_empty());
    }

    #[test]
    fn a_formatted_column_shows_its_format() {
        let src = "sheet T\n\nA : Decimal['.2f]\nB : Integer[#x]\n\nA         | B   | C\n12345.678 | 255 | 12345.678\n";
        let doc = Document::with_style(src, Style::ISO.numbers(',', '.'));
        let snap = doc.snapshot();
        assert!(snap.problems.is_empty());
        let row = &snap.sheets[0].rows[0];
        assert_eq!(row[0].display, "12.345,68");
        assert!(!row[0].shortened);
        // The editor starts from the cell as written, in full.
        assert_eq!(row[0].source, "12345.678");
        assert_eq!(row[1].display, "0xff");
        // A column with no format is shown as before.
        assert_eq!(row[2].display, "12.345,678");
    }

    #[test]
    fn a_date_column_shows_its_format() {
        let src = "sheet T\n\nDue : Date[%a %d %b %Y]\nAt : Time[%I:%M %p]\nWhen : Datetime[%d/%m %H:%M %%]\n\nDue        | At    | When\n2025-01-31 | 17:05 | 2025-01-31T09:30:15\n";
        let doc = Document::from_text(src);
        let snap = doc.snapshot();
        assert!(snap.problems.is_empty(), "{:?}", snap.problems);
        let row = &snap.sheets[0].rows[0];
        let shown: Vec<_> = row.iter().map(|c| c.display.as_str()).collect();
        assert_eq!(shown, ["Fri 31 Jan 2025", "05:05 PM", "31/01 09:30 %"]);
        assert_eq!(row[0].source, "2025-01-31");
        assert_eq!(
            snap.sheets[0].columns[2].format.as_deref(),
            Some("%d/%m %H:%M %%")
        );
    }

    #[test]
    fn a_date_format_shows_the_files_zone() {
        let src = "zone Europe/London\n\nsheet T\n\nAt : Datetime[%H:%M %Z %z %s]\n\nAt\n2025-07-01T12:00\n2025-01-01T12:00\n";
        let doc = Document::from_text(src);
        assert!(doc.snapshot().problems.is_empty());
        assert_eq!(
            col(&doc, 0, 0),
            ["12:00 BST +0100 1751367600", "12:00 GMT +0000 1735732800"]
        );
    }

    #[test]
    fn a_date_format_is_set_and_kept() {
        let src = "sheet T\n\nDue\n2025-01-31\n";
        let mut doc = Document::from_text(src);
        assert_eq!(
            doc.set_column_format(0, &[0], Some("%d %H")),
            Err(
                "Not a format: %d %H\nDate takes the conversions of a date, as %d %b %Y"
                    .to_string()
            )
        );
        assert!(doc.set_column_format(0, &[0], Some("%d %b %Y %]")).is_ok());
        assert!(
            doc.text().contains("Due : Date[%d %b %Y %]]\n"),
            "{}",
            doc.text()
        );
        assert_eq!(col(&doc, 0, 0), ["31 Jan 2025 ]"]);
        // A `Datetime` takes what a `Date` does; a `Time` does not.
        assert!(doc.set_column_types(0, &[0], Some("Datetime")));
        assert!(
            doc.text().contains("Due : Datetime[%d %b %Y %]]\n"),
            "{}",
            doc.text()
        );
        assert!(doc.set_column_types(0, &[0], Some("Time")));
        assert!(doc.text().contains("Due : Time\n"), "{}", doc.text());
    }

    #[test]
    fn a_column_format_is_set_and_cleared() {
        let mut doc = Document::from_text(SALES);
        // `f` is not a conversion of a whole number, and nothing is set.
        assert_eq!(
            doc.set_column_format(0, &[1], Some("'.2f")),
            Err("Not a format: '.2f\nInteger takes d, i, x, X, o, b or B".to_string())
        );
        assert!(doc.set_column_format(0, &[1], Some("q")).is_err());
        assert_eq!(
            doc.set_column_format(0, &[0], Some("d")),
            Err("Month takes no format".to_string())
        );
        assert_eq!(doc.text(), SALES);
        // An undeclared column is declared as the type its cells give it.
        assert!(doc.set_column_format(0, &[1, 2], Some("05d")).is_ok());
        assert!(
            doc.text()
                .contains("sheet Sales\n\nRevenue : Integer[05d]\nCost    : Integer[05d]\n\n"),
            "{}",
            doc.text()
        );
        assert_eq!(col(&doc, 0, 1), ["00100", "00120"]);
        let column = &doc.snapshot().sheets[0].columns[1];
        assert_eq!(column.format.as_deref(), Some("05d"));
        assert!(column.declared);
        assert!(doc.set_column_format(0, &[1], Some("#x")).is_ok());
        assert!(doc.text().contains("Revenue : Integer[#x]\n"));
        assert!(doc.set_column_format(0, &[1], None).is_ok());
        assert!(doc.text().contains("Revenue : Integer\n"));
        assert_eq!(col(&doc, 0, 1), ["100", "120"]);
    }

    #[test]
    fn editing_a_cell_changes_one_line() {
        let mut doc = Document::from_text(SALES);
        assert!(doc.set_cell(0, 1, 1, "130"));
        assert_eq!(doc.text(), SALES.replace("120", "130"));
        assert_eq!(col(&doc, 0, 3), ["40", "60"]);
        assert!(doc.undo());
        assert_eq!(doc.text(), SALES);
        assert!(doc.redo());
        assert_eq!(col(&doc, 0, 4), ["8", "12"]);
    }

    #[test]
    fn formula_cells_and_wider_cells() {
        let mut doc = Document::from_text(SALES);
        doc.set_cell(0, 1, 1, "= Sales[Revenue; *-1] * 2");
        assert_eq!(col(&doc, 0, 1), ["100", "200"]);
        assert!(
            doc.text()
                .contains("Jan   | 100                       | 60")
        );
        assert!(doc.text().contains("# sales\nconst Rate = 20%"));
        assert!(
            doc.text()
                .ends_with("Profit := Revenue - Cost\nTax := Profit * Rate\n")
        );
    }

    #[test]
    fn rows_can_be_added_pasted_and_deleted() {
        let mut doc = Document::from_text(SALES);
        doc.set_cells(
            0,
            &[
                (2, 0, "Mar".into()),
                (2, 1, "150".into()),
                (2, 2, "80".into()),
                (3, 0, "Apr".into()),
            ],
        );
        assert_eq!(col(&doc, 0, 0), ["Jan", "Feb", "Mar", "Apr"]);
        assert_eq!(col(&doc, 0, 3), ["40", "50", "70", ""]);
        assert!(
            doc.text().contains(
                "| Feb   | 120     | 70   | *      | *   |\n| Mar   | 150     | 80   | *      | *   |\n| Apr   |         |      | *      | *   |\n\nProfit"
            )
        );
        doc.insert_rows(0, 0, 1);
        assert_eq!(col(&doc, 0, 0), ["", "Jan", "Feb", "Mar", "Apr"]);
        doc.delete_rows(0, 0, 2);
        assert_eq!(col(&doc, 0, 0), ["Feb", "Mar", "Apr"]);
        assert!(
            doc.text()
                .contains("| Month | Revenue | Cost | Profit | Tax |\n| ----- | ------- | ---- | ------ | --- |\n| Feb")
        );
    }

    #[test]
    fn columns_constants_and_sheets() {
        let mut doc = Document::from_text(SALES);
        doc.add_column(0, "Note").unwrap();
        assert!(doc.add_column(0, "Note").is_err());
        assert!(doc.add_column(0, "bad name").is_err());
        // A new column is the last one.
        doc.set_cell(0, 0, 5, "a | b");
        assert_eq!(doc.snapshot().sheets[0].rows[0][5].display, "a | b");
        doc.add_computed(0, "Margin", "Profit / Revenue").unwrap();
        let names: Vec<_> = doc.snapshot().sheets[0]
            .columns
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(
            names,
            [
                "Month", "Revenue", "Cost", "Profit", "Tax", "Note", "Margin"
            ]
        );
        // A column of no declared type takes the type of its values.
        assert_eq!(col(&doc, 0, 6), ["0.4", "0.41666666666666666666…"]);
        assert_eq!(doc.snapshot().sheets[0].columns[6].ty, "Decimal");
        // One that does not end is shown cut and marked, so there is more
        // of it to show.
        let exact = |doc: &Document| doc.snapshot().sheets[0].rows[1][6].exact.clone();
        assert_eq!(
            exact(&doc).as_deref(),
            Some("0.416666666666666666666666666667")
        );
        doc.set_cell(0, 0, 6, "Profit / Cost");
        assert_eq!(
            col(&doc, 0, 6),
            ["0.66666666666666666666…", "0.71428571428571428571…"]
        );
        doc.set_const(0, "50%");
        assert_eq!(col(&doc, 0, 4), ["20", "25"]);
        doc.add_const("Extra", "Sales.Revenue.sum()").unwrap();
        assert_eq!(doc.snapshot().consts[1].display, "220");
        doc.add_sheet("Notes", None).unwrap();
        assert_eq!(doc.snapshot().sheets[1].rows.len(), 3);
        assert!(
            doc.snapshot().problems.is_empty(),
            "{:?}",
            doc.snapshot().problems
        );
    }

    #[test]
    fn renaming_a_column_follows_its_references() {
        let text = format!(
            "{SALES}\nsheet Top\n\nKind | Value                                 | Revenue\nSum  | = Sales.Revenue.sum()                 | *\nBig  | = Sales[Revenue; Revenue > 100].sum() | *\n\nRevenue := Value\n"
        );
        let mut doc = Document::from_text(&text);
        let before = col(&doc, 0, 3);
        doc.rename_column(0, 1, "Income").unwrap();
        let renamed = doc.text().to_string();
        assert!(renamed.contains(
            "| Month | Income | Cost | Profit | Tax |\n| ----- | ------ | ---- | ------ | --- |\n| Jan   | 100    | 60   | *      | *   |\n"
        ));
        assert!(renamed.contains("Profit := Income - Cost"));
        assert!(renamed.contains("= Sales.Income.sum()"));
        assert!(renamed.contains("= Sales[Income; Income > 100].sum()"));
        // Another sheet's own column of the same name is left alone.
        assert!(renamed.contains("Revenue := Value"));
        assert!(
            doc.snapshot().problems.is_empty(),
            "{:?}",
            doc.snapshot().problems
        );
        assert_eq!(col(&doc, 0, 3), before);
        assert_eq!(col(&doc, 1, 1), ["220", "120"]);

        // A computed column.
        doc.rename_column(0, 3, "Gain").unwrap();
        assert!(
            doc.text()
                .contains("Gain := Income - Cost\nTax := Gain * Rate")
        );
        assert!(doc.snapshot().problems.is_empty());

        assert!(doc.rename_column(0, 1, "Cost").is_err());
        assert!(doc.rename_column(0, 1, "Rate").is_err());
        assert!(doc.rename_column(0, 1, "bad name").is_err());
        // `Value` inside `Sales[...]` would be captured by the renamed column.
        let capture =
            format!("{SALES}\nsheet Top\n\nValue\n100\n\nN := Sales[; Revenue > Value].len()\n");
        let mut other = Document::from_text(&capture);
        assert!(other.rename_column(0, 1, "Value").is_err());
        assert_eq!(other.text(), capture);
        // Each rename is one undo step.
        doc.undo();
        assert_eq!(doc.text(), renamed);
        doc.undo();
        assert_eq!(doc.text(), text);
    }

    #[test]
    fn functions_are_listed_and_follow_a_renamed_column() {
        let text = "# Everything sold.\nfn Sold() = Sales.Revenue.sum()\nfn Net(a, b) = a - b\n\n"
            .to_string()
            + "sheet Sales\n\nMonth | Revenue | Cost | Profit\nJan   | 100     | 60   | *\n\nProfit := Net(Revenue, Cost)\n";
        let mut doc = Document::from_text(&text);
        assert!(doc.snapshot().problems.is_empty());
        let funcs = &doc.snapshot().funcs;
        assert_eq!(funcs[0].usage, "Sold()");
        assert_eq!(funcs[0].doc, "Everything sold.");
        assert_eq!(funcs[1].usage, "Net(a, b)");
        assert_eq!(funcs[1].source, "a - b");
        assert_eq!(col(&doc, 0, 3), ["40"]);

        doc.rename_column(0, 1, "Income").unwrap();
        assert!(doc.text().contains("fn Sold() = Sales.Income.sum()"));
        assert!(doc.text().contains("Profit := Net(Income, Cost)"));
        assert!(doc.snapshot().problems.is_empty());
    }

    #[test]
    fn a_sheet_of_formulas_alone_is_shown_and_not_edited() {
        // There is no text behind a cell of such a sheet and no row of its
        // own to change, so every edit of it is refused and the file is left
        // as it was.
        let text = "sheet Data\n\n| Month | Revenue |\n| ----- | ------- |\n| Jan   | 10000   |\n| Feb   | 12000   |\n\nsheet Analysis\n\nTwice := Data[Revenue; *] * 2\n";
        let mut doc = Document::from_text(text);
        assert!(doc.snapshot().problems.is_empty());
        assert_eq!(col(&doc, 1, 0), ["20000", "24000"]);
        assert!(doc.snapshot().sheets[1].derived);
        let why = doc.no_cells(1).unwrap();
        assert!(why.contains("Analysis"), "{why}");
        assert!(doc.read_only(1).is_none());
        assert!(!doc.set_cells(1, &[(0, 0, "9".to_string())]));
        assert!(!doc.insert_rows(1, 0, 1));
        assert!(!doc.delete_rows(1, 0, 1));
        assert_eq!(doc.add_column(1, "Extra").unwrap_err(), why);
        assert_eq!(doc.insert_column(1, 0, "Extra").unwrap_err(), why);
        assert_eq!(doc.delete_columns(1, 0, 1).unwrap_err(), why);
        assert!(!doc.set_column_types(1, &[0], Some("Integer")));
        assert_eq!(doc.text(), text);

        // Its formulas are its text, so they are edited as any sheet's: the
        // expression, the name, and another column after it. Typing into a
        // cell of a computed column is how the app reaches the formula, and
        // every column of this sheet is one.
        assert!(doc.set_cell(1, 0, 0, "Data[Revenue; *] * 3"));
        assert_eq!(col(&doc, 1, 0), ["30000", "36000"]);
        doc.rename_column(1, 0, "Thrice").unwrap();
        assert!(doc.text().contains("Thrice := Data[Revenue; *] * 3\n"));
        doc.add_computed(1, "Half", "Thrice / 6").unwrap();
        assert_eq!(
            doc.text(),
            "sheet Data\n\n| Month | Revenue |\n| ----- | ------- |\n| Jan   | 10000   |\n| Feb   | 12000   |\n\nsheet Analysis\n\nThrice := Data[Revenue; *] * 3\nHalf := Thrice / 6\n"
        );
        assert_eq!(col(&doc, 1, 1), ["5000", "6000"]);
        assert!(doc.snapshot().problems.is_empty());
        // One step each to undo, back to the file as it was.
        for _ in 0..3 {
            assert!(doc.undo());
        }
        assert_eq!(doc.text(), text);

        // The sheet that holds the rows is edited as ever, and the derived
        // one follows it.
        assert!(doc.set_cell(0, 1, 1, "20000"));
        assert_eq!(col(&doc, 1, 0), ["20000", "40000"]);
    }

    #[test]
    fn columns_can_be_deleted() {
        let text = "const Rate = 20%\n\nsheet Sales\n\nCost : Int\nRevenue : Int\n\n\
                    | Month | Revenue | Cost | Note       | Profit | Tax |\n| ----- | ------- | ---- | ---------- | ------ | --- |\n| Jan   | 100     | 60   | a          | *      | *   |\n| Feb   | 120     | 70   | = [*-1; *] | *      | *   |\n\n\
                    Profit := Revenue - Cost\nTax := Profit * Rate\n";
        let mut doc = Document::from_text(text);
        let names = |doc: &Document| -> Vec<String> {
            let columns = &doc.snapshot().sheets[0].columns;
            columns.iter().map(|c| c.name.clone()).collect()
        };
        // A data column goes with its cells and its type line; the formula
        // that counts columns still reads the one to its left.
        doc.delete_columns(0, 0, 1).unwrap();
        assert_eq!(names(&doc), ["Revenue", "Cost", "Note", "Profit", "Tax"]);
        assert!(
            doc.text().contains(
                "| Revenue | Cost | Note       | Profit | Tax |\n| ------- | ---- | ---------- | ------ | --- |\n| 100     | 60   | a          | *      | *   |\n| 120     | 70   | = [*-1; *] | *      | *   |\n"
            ),
            "{}",
            doc.text()
        );
        assert_eq!(col(&doc, 0, 2), ["a", "70"]);
        assert!(doc.snapshot().problems.is_empty());
        // A formula column goes with its line. What read it shows an error.
        doc.delete_columns(0, 3, 1).unwrap();
        assert_eq!(names(&doc), ["Revenue", "Cost", "Note", "Tax"]);
        assert!(!doc.text().contains("Profit :="));
        assert!(doc.snapshot().problems[0].message.contains("Profit"));
        // One step each to undo.
        assert!(doc.undo());
        assert!(doc.snapshot().problems.is_empty());
        assert!(doc.undo());
        assert_eq!(doc.text(), text);

        // Several at once, with their type lines, from the middle.
        doc.delete_columns(0, 1, 2).unwrap();
        assert_eq!(names(&doc), ["Month", "Note", "Profit", "Tax"]);
        assert!(!doc.text().contains("Cost : Int") && !doc.text().contains("Revenue : Int"));
        assert!(doc.undo());
        assert_eq!(doc.text(), text);

        // Both kinds at once.
        doc.delete_columns(0, 3, 2).unwrap();
        assert_eq!(names(&doc), ["Month", "Revenue", "Cost", "Tax"]);
        assert!(!doc.text().contains("Note") && !doc.text().contains("Profit :="));
        assert!(doc.undo());
        assert_eq!(doc.text(), text);

        // Every data column may go: a sheet of nothing but computed
        // columns keeps the rows it had, each cell a `*`.
        doc.delete_columns(0, 0, 4).unwrap();
        assert_eq!(names(&doc), ["Profit", "Tax"]);
        assert_eq!(doc.snapshot().sheets[0].rows.len(), 2);
        assert!(
            doc.text().contains(
                "| Profit | Tax |\n| ------ | --- |\n| *      | *   |\n| *      | *   |\n"
            ),
            "{}",
            doc.text()
        );
        assert!(doc.undo());
        assert_eq!(doc.text(), text);

        // Not every column, and not off the sheet.
        assert!(
            doc.delete_columns(0, 0, 6)
                .unwrap_err()
                .contains("at least one")
        );
        assert!(doc.delete_columns(0, 5, 2).is_err());
        assert_eq!(doc.text(), text);
        // Down to one column, an empty cell is still a row.
        let mut two = Document::from_text("sheet T\n\nA | B\n  | 1\nx | 2\n");
        two.delete_columns(0, 1, 1).unwrap();
        assert_eq!(
            two.text(),
            "sheet T\n\n| A   |\n| --- |\n|     |\n| x   |\n"
        );
        assert_eq!(two.snapshot().sheets[0].rows.len(), 2);
    }

    #[test]
    fn errors_do_not_blank_the_grid() {
        let mut doc = Document::from_text(SALES);
        doc.set_cell(0, 0, 1, "= Revnue + 1");
        let t = &doc.snapshot().sheets[0];
        assert_eq!(t.rows[0][1].display, "#ERROR");
        assert!(t.rows[0][1].error.as_ref().unwrap().contains("Revnue"));
        assert_eq!(t.rows[1][3].display, "50");
        assert_eq!(doc.snapshot().problems.len(), 1);
        assert_eq!(doc.snapshot().problems[0].line, 8);
        // And the problem says which cell holds it, to be gone to.
        assert_eq!(doc.snapshot().problems[0].cell, Some((0, 0, 1)));
    }

    #[test]
    fn a_kind_of_text_says_which_cell_does_not_read_as_one() {
        let text = "sheet Contacts\n\nContact : Email\n\n\
                    Contact | Note\nname@example.com | a\nnope | b\n";
        let doc = Document::from_text(text);
        let rows = &doc.snapshot().sheets[0].rows;
        assert_eq!(rows[0][0].display, "name@example.com");
        assert_eq!(rows[1][0].display, "#ERROR");
        // The message is the app's own, with what can be typed as its help.
        assert_eq!(
            rows[1][0].error.as_deref(),
            Some(
                "Contact has type Email, nope does not match \
                 (Email: an email address, e.g. name@example.com)"
            )
        );
        // The rest of the sheet is still there.
        assert_eq!(rows[1][1].display, "b");
        // And the problem names the cell: sheet 0, row 1, column 0.
        let problems = &doc.snapshot().problems;
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].cell, Some((0, 1, 0)));
    }

    #[test]
    fn a_failed_conversion_is_gone_to_at_the_row_that_failed() {
        let text = "sheet Contacts\n\n\
                    Written | Checked\nname@example.com | *\nnope | *\nalso bad | *\n\n\
                    Checked := Written.Email\n";
        let doc = Document::from_text(text);
        let rows = &doc.snapshot().sheets[0].rows;
        assert_eq!(rows[0][1].display, "name@example.com");
        assert_eq!(rows[1][1].display, "#ERROR");
        assert_eq!(rows[2][1].display, "#ERROR");
        // Two failures, each gone to at the row it happened in, and neither
        // at row 0, which holds a cell that is perfectly well.
        let problems = &doc.snapshot().problems;
        let cells: Vec<_> = problems.iter().map(|p| p.cell).collect();
        assert_eq!(cells, [Some((0, 1, 1)), Some((0, 2, 1))]);
    }

    #[test]
    fn a_column_of_addresses_offers_the_kind_of_text_it_holds() {
        let text = "sheet T\n\n\
             Contact | Site | Ref | Mixed | Count\n\
             a@example.com | https://example.com | 7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f | a@example.com | 7\n\
             b@example.co.uk | https://example.org/x | 00000000-0000-0000-0000-000000000000 | nope | 12\n";
        let doc = Document::from_text(text);
        let shown: Vec<&str> = doc.snapshot().sheets[0]
            .columns
            .iter()
            .map(|column| column.ty.as_str())
            .collect();
        // Each column says in its header what its cells read as, and a
        // column with one odd cell, or of numbers, says what it did before.
        assert_eq!(shown, ["Email", "Url", "Uuid", "String", "Integer"]);
        // Nothing is declared by saying so: the header is shown muted, and
        // the file has no type line to go with it.
        assert!(doc.snapshot().sheets[0].columns.iter().all(|c| !c.declared));
        assert!(!doc.text().contains(": Email"));
    }

    #[test]
    fn an_empty_cell_does_not_stop_a_kind_of_text_being_offered() {
        let text = "sheet T\n\n\
                    Contact | Link\n\
                    a@example.com | mailto:a@example.com\n\
                    | mailto:b@example.com\n";
        let doc = Document::from_text(text);
        let ty = |c: usize| doc.snapshot().sheets[0].columns[c].ty.clone();
        assert_eq!(ty(0), "Email");
        // A `mailto:` address reads as both, and the web address wins.
        assert_eq!(ty(1), "Url");
        // A column with nothing in it says nothing.
        let empty = Document::from_text("sheet T\n\nA | B\n| \n");
        assert_eq!(empty.snapshot().sheets[0].columns[0].ty, "");
    }

    #[test]
    fn the_formula_of_a_calculated_sheet_is_edited_as_a_columns_is() {
        let text = "sheet Sales\n\nMonth | Revenue\nJan | 100\nFeb | 120\nMar | 150\n\n\
                    sheet Top := Sales |> head(1)\n";
        let mut doc = Document::from_text(text);
        assert_eq!(doc.snapshot().sheets[1].rows.len(), 1);
        // An edit of any of its cells is an edit of the sheet's formula,
        // as one of a calculated column's cells is of the column's.
        assert!(doc.set_cell(1, 0, 0, ":= Sales |> head(2)"));
        assert_eq!(doc.snapshot().sheets[1].rows.len(), 2);
        assert!(doc.text().contains("sheet Top := Sales |> head(2)"));
        // The mark is optional, the formula being what it is either way.
        assert!(doc.set_cell(1, 0, 0, "Sales |> head(3)"));
        assert_eq!(doc.snapshot().sheets[1].rows.len(), 3);
        // And one undoable step each time.
        assert!(doc.undo());
        assert_eq!(doc.snapshot().sheets[1].rows.len(), 2);
        // A sheet with no formula has none to set.
        assert!(!doc.set_sheet_formula(0, "Sales |> head(1)"));
    }

    #[test]
    fn each_kind_of_text_is_shown_an_example_that_reads_as_one() {
        let example = |ty| {
            let hint = entry_hint(&Style::ISO, ty);
            hint.split("e.g. ")
                .nth(1)
                .unwrap_or_else(|| panic!("no example in `{hint}`"))
                .to_string()
        };
        assert!(omasheet_omx::text::is_url(&example("Url")));
        assert!(omasheet_omx::text::is_email(&example("Email")));
        assert!(omasheet_omx::text::is_uuid(&example("Uuid")));
    }

    #[test]
    fn dates_follow_the_style_but_the_text_stays_iso() {
        use omasheet_omx::date::Order;
        let text = "sheet Log\n\nDay : Date\nAt : Time\nSeen : Datetime\n\n\
                    Day | At | Seen | Note\n2026-10-08 | 17:47 | 2026-10-08T17:47 | a\n";
        let mut doc = Document::from_text(text);
        let row = |doc: &Document, r: usize| -> Vec<String> {
            let cells = &doc.snapshot().sheets[0].rows[r];
            cells.iter().map(|c| c.display.clone()).collect()
        };
        assert_eq!(
            row(&doc, 0),
            ["2026-10-08", "17:47", "2026-10-08T17:47", "a"]
        );

        doc.set_style(Style::new(Order::Dmy, '/', false));
        assert_eq!(
            row(&doc, 0),
            ["08/10/2026", "17:47", "08/10/2026 17:47", "a"]
        );
        assert_eq!(doc.snapshot().sheets[0].rows[0][0].source, "2026-10-08");
        assert_eq!(doc.text(), text);

        // Typed the British way, stored the ISO way.
        doc.set_cells(
            0,
            &[
                (1, 0, "9/10/26".to_string()),
                (1, 1, "5:05 pm".to_string()),
                (1, 2, "9/10/2026 8:00".to_string()),
                (1, 3, "1/2/3".to_string()),
            ],
        );
        assert!(
            doc.text()
                .contains("2026-10-09 | 17:05 | 2026-10-09T08:00 | 1/2/3")
        );
        assert_eq!(
            row(&doc, 1),
            ["09/10/2026", "17:05", "09/10/2026 08:00", "1/2/3"]
        );
        doc.set_cell(0, 1, 2, "10/10/2026");
        assert!(doc.text().contains("2026-10-10T00:00"));
        assert!(doc.snapshot().problems.is_empty());

        // The same keys mean another day in the United States.
        doc.set_style(Style::new(Order::Mdy, '/', true));
        assert_eq!(row(&doc, 0)[2], "10/08/2026 5:47 PM");
        doc.set_cell(0, 0, 0, "3/4/2026");
        assert!(doc.text().contains("2026-03-04 | 17:47"));

        // A column with no type takes only what must be a date or a time.
        let mut loose = Document::from_text("sheet T\n\nA | B\n  |\n  |\n");
        loose.set_style(Style::new(Order::Dmy, '/', false));
        loose.set_cells(
            0,
            &[
                (0, 0, "8/10/2026".to_string()),
                (0, 1, "9:30".to_string()),
                (1, 0, "= 1/2".to_string()),
                (1, 1, "10.11.12".to_string()),
            ],
        );
        assert!(loose.text().contains("2026-10-08 | 09:30"));

        // A whole number and a part is kept as it was written, and makes the
        // column a `Fraction` one, which shows it the same way.
        let mut mixed = Document::from_text("sheet T\n\nN : String\n\nA | N\n  |\n  |\n");
        mixed.set_cells(
            0,
            &[
                (0, 0, " 2 3/4".to_string()),
                (1, 0, "-1 1/3".to_string()),
                (0, 1, "2 3/4".to_string()),
            ],
        );
        assert!(mixed.text().contains("2 3/4"), "{}", mixed.text());
        assert!(mixed.text().contains("-1 1/3"));
        assert!(!mixed.text().contains("2.75"), "{}", mixed.text());
        let cell = &mixed.snapshot().sheets[0].rows[0][0];
        assert_eq!(
            (cell.display.as_str(), cell.source.as_str()),
            ("2 3/4", "2 3/4")
        );
        assert_eq!(cell.exact, None);
        assert!(loose.text().contains("= 1/2"));
        assert!(loose.text().contains("10.11.12"));
    }

    #[test]
    fn the_source_is_aligned() {
        let ragged = "sheet T\n\nA | Bee\n100 | x\n# a note\n2 |\n\nsheet U\n\nC|D\n1|22\n";
        let mut doc = Document::from_text(ragged);
        // A Markdown table: outer pipes, a `---` line, every `|` in step.
        let aligned = "sheet T\n\n| A   | Bee |\n| --- | --- |\n| 100 | x   |\n# a note\n| 2   |     |\n\n\
                       sheet U\n\n| C   | D   |\n| --- | --- |\n| 1   | 22  |\n";
        assert_eq!(doc.aligned_text(), aligned);
        // Shown aligned, but not changed until it is edited.
        assert_eq!(doc.text(), ragged);
        assert!(doc.set_text(ragged));
        assert_eq!(doc.text(), aligned);
        // Read back as it was: the same cells, the empty one too.
        assert_eq!(col(&doc, 0, 1), ["x", ""]);
        assert_eq!(col(&doc, 1, 1), ["22"]);
        assert!(doc.snapshot().problems.is_empty());
        // One column is a table too, with an empty cell as a row.
        let one = Document::from_text("sheet T\n\n| Name |\n|---|\n| Ana |\n|  |\n");
        assert_eq!(col(&one, 0, 0), ["Ana", ""]);
        assert_eq!(
            one.aligned_text(),
            "sheet T\n\n| Name |\n| ---- |\n| Ana  |\n|      |\n"
        );
        // Text that does not parse is left as it is.
        let broken = "sheet T\n\nA : Whole\n\nA | B\n1 | 2\n";
        assert_eq!(Document::from_text(broken).aligned_text(), broken);
    }

    #[test]
    fn entry_hints_follow_the_style() {
        let us = Style::new(date::Order::Mdy, '/', true);
        assert_eq!(
            entry_hint(&us, "Date"),
            "Date: MM/DD/YYYY, e.g. 01/31/2025 or ISO"
        );
        assert_eq!(entry_hint(&us, "Time"), "Time: e.g. 5:05 PM");
        assert_eq!(
            entry_hint(&us, "Datetime"),
            "Datetime: e.g. 01/31/2025 5:05 PM or ISO"
        );
        assert_eq!(
            entry_hint(&Style::ISO, "Datetime"),
            "Datetime: e.g. 2025-01-31T17:05"
        );
        assert_eq!(
            entry_hint(&us, "Duration"),
            "Duration: PnDTnHnMnS, e.g. PT1H30M"
        );
        assert_eq!(entry_hint(&us, ""), "");
        // A cell that does not fit says what would.
        let mut doc = Document::with_style("sheet T\n\nOn : Date\n\nOn\n2025-01-31\n", us);
        doc.set_cell(0, 0, 0, "soon");
        assert_eq!(
            doc.snapshot().sheets[0].rows[0][0].error.as_deref(),
            Some(
                "On has type Date, soon does not match (Date: MM/DD/YYYY, e.g. 01/31/2025 or ISO)"
            )
        );
        assert!(
            doc.snapshot().problems[0]
                .message
                .contains("On has type Date")
        );
        // Every example is accepted as an entry for its type.
        for ty in ["Date", "Time", "Datetime"] {
            let hint = entry_hint(&us, ty);
            let example = hint
                .split("e.g. ")
                .nth(1)
                .unwrap()
                .trim_end_matches(" or ISO");
            assert!(iso_entry(&us, ty, example).is_some(), "{hint}");
        }
    }

    #[test]
    fn a_file_in_another_zone_is_shown_on_local_clocks() {
        // A zone whose clocks this machine does not keep, wherever it is.
        let zone = match crate::zone::Zone::system().from_utc(0) {
            Some(32_400) => "America/New_York",
            _ => "Asia/Tokyo",
        };
        let text = format!("zone {zone}\n\nsheet T\n\nAt\n2025-07-15T12:00\n");
        // As the file writes it, for tools.
        let plain = Document::from_text(&text);
        assert_eq!(
            plain.snapshot().sheets[0].rows[0][0].display,
            "2025-07-15T12:00"
        );
        // For a person, on this machine's clocks; the source is untouched.
        let mut doc = Document::with_style(&text, Style::ISO);
        let cell = doc.snapshot().sheets[0].rows[0][0].clone();
        assert_eq!(cell.source, "2025-07-15T12:00");
        assert_ne!(cell.display, "2025-07-15T12:00");
        // What is typed is in the file's zone, as the source shows it.
        doc.set_cell(0, 0, 0, "2025-07-15T13:00");
        assert!(doc.text().contains("\n| 2025-07-15T13:00 |\n"));

        let bad = Document::from_text("zone Mars/Base\n\nsheet T\n\nA\n1\n");
        assert!(bad.snapshot().problems[0].message.contains("no time zone"));
        assert_eq!(bad.snapshot().sheets[0].rows[0][0].display, "1");
    }

    #[test]
    fn a_block_of_cells_is_summed_exactly() {
        let text = "sheet T\n\nName | A | B\nx | 1 | 0.5\ny | 2 |\nz | = 1/3 | 1e2\n";
        let doc = Document::from_text(text);
        let sum = |from, to| doc.summary(0, from, to).unwrap();
        let numbers = |s: &str, a: &str| Some((s.to_string(), a.to_string()));
        // Exact numbers add up exactly, and are shown as cells are.
        let a = sum((0, 1), (2, 1));
        assert_eq!((a.count, a.numbers), (3, numbers("3.33", "1.11")));
        // Text counts but does not add; an empty cell does neither.
        let top = sum((0, 0), (1, 2));
        assert_eq!((top.count, top.numbers), (5, numbers("3.5", "1.17")));
        // One Num makes the total a Num.
        let b = sum((0, 2), (2, 2));
        assert_eq!((b.count, b.numbers), (2, numbers("100.5", "50.25")));
        let names = sum((0, 0), (2, 0));
        assert_eq!((names.count, names.numbers), (3, None));
        // A block that runs off the sheet is cut to it.
        assert_eq!(sum((2, 1), (9, 9)).count, 2);
        assert!(doc.summary(5, (0, 0), (1, 1)).is_none());
    }

    #[test]
    fn columns_can_be_moved() {
        let mut doc = Document::from_text(SALES);
        let names = |doc: &Document| -> Vec<String> {
            let columns = &doc.snapshot().sheets[0].columns;
            columns.iter().map(|c| c.name.clone()).collect()
        };
        assert_eq!(names(&doc), ["Month", "Revenue", "Cost", "Profit", "Tax"]);

        // A data column, with its cells, and the sheet lined up again.
        doc.move_columns(0, 0, 1, 2).unwrap();
        assert_eq!(names(&doc), ["Revenue", "Cost", "Month", "Profit", "Tax"]);
        assert!(
            doc.text()
                .contains("| Revenue | Cost | Month | Profit | Tax |\n| ------- | ---- | ----- | ------ | --- |\n| 100     | 60   | Jan   | *      | *   |\n| 120     | 70   | Feb   | *      | *   |\n")
        );
        assert_eq!(col(&doc, 0, 2), ["Jan", "Feb"]);
        assert_eq!(col(&doc, 0, 3), ["40", "50"]);
        // Two at once, back to the left.
        doc.move_columns(0, 1, 2, 0).unwrap();
        assert_eq!(names(&doc), ["Cost", "Month", "Revenue", "Profit", "Tax"]);
        // One step to undo.
        assert!(doc.undo());
        assert_eq!(names(&doc), ["Revenue", "Cost", "Month", "Profit", "Tax"]);

        // A formula column, among the formula columns.
        doc.move_columns(0, 4, 1, 3).unwrap();
        assert_eq!(names(&doc), ["Revenue", "Cost", "Month", "Tax", "Profit"]);
        assert!(
            doc.text().contains(
                "| Revenue | Cost | Month | Tax | Profit |\n| ------- | ---- | ----- | --- | ------ |\n| 100     | 60   | Jan   | *   | *      |\n"
            )
        );
        assert_eq!(col(&doc, 0, 3), ["8", "10"]);
        assert!(doc.snapshot().problems.is_empty());

        // Not off the sheet.
        let before = doc.text().to_string();
        assert!(doc.move_columns(0, 3, 2, 4).is_err());
        assert!(doc.move_columns(0, 9, 1, 0).is_err());
        assert!(doc.move_columns(0, 1, 1, 1).is_ok());
        assert_eq!(doc.text(), before);
    }

    #[test]
    fn formula_columns_go_among_data_columns() {
        let text = "const Rate = 20%\n\nsheet Sales\n\n\
                    | Month | Revenue | Cost | Left                  | Profit | Tax |\n\
                    | ----- | ------- | ---- | --------------------- | ------ | --- |\n\
                    | Jan   | 100     | 60   | = [*-1; *] + [*+1; *] | *      | *   |\n\
                    | Feb   | 120     | 70   | 5                     | *      | *   |\n\n\
                    Profit := Revenue - Cost\nTax := Profit * Rate\n";
        let mut doc = Document::from_text(text);
        let names = |doc: &Document| -> Vec<String> {
            let columns = &doc.snapshot().sheets[0].columns;
            columns.iter().map(|c| c.name.clone()).collect()
        };
        assert!(doc.snapshot().problems.is_empty());
        assert_eq!(col(&doc, 0, 3), ["100", "5"]);

        // A formula column to the left of a data column. The cell that
        // counted columns from itself still reads the two it read, and the
        // `Name := expression` lines stay as they are.
        doc.move_columns(0, 4, 1, 2).unwrap();
        assert_eq!(
            names(&doc),
            ["Month", "Revenue", "Profit", "Cost", "Left", "Tax"]
        );
        let now = doc.text();
        assert!(
            now.contains(
                "| Month | Revenue | Profit | Cost | Left                  | Tax |\n\
                 | ----- | ------- | ------ | ---- | --------------------- | --- |\n\
                 | Jan   | 100     | *      | 60   | = [*-1; *] + [*-2; *] | *   |\n\
                 | Feb   | 120     | *      | 70   | 5                     | *   |\n"
            ),
            "{now}"
        );
        assert!(now.contains("\nProfit := Revenue - Cost\nTax := Profit * Rate\n"));
        assert_eq!(col(&doc, 0, 2), ["40", "50"]);
        assert_eq!(col(&doc, 0, 4), ["100", "5"]);
        assert!(doc.snapshot().problems.is_empty());
        // Its cells show its formula, and typing in one changes the formula.
        assert_eq!(
            doc.snapshot().sheets[0].rows[1][2].source,
            "= Revenue - Cost"
        );
        doc.set_cell(0, 1, 2, "= Revenue - Cost - 1");
        assert!(doc.text().contains("Profit := Revenue - Cost - 1\n"));
        assert_eq!(col(&doc, 0, 2), ["39", "49"]);
        assert!(doc.undo());

        // A new row has `*` there, and pasting over the column leaves it.
        doc.insert_rows(0, 2, 1);
        assert!(
            doc.text()
                .contains("\n|       |         | *      |      |                       | *   |\n")
        );
        doc.set_cells(
            0,
            &[(2, 1, "9".into()), (2, 2, "7".into()), (2, 3, "4".into())],
        );
        assert_eq!(col(&doc, 0, 2), ["40", "50", "5"]);
        assert!(doc.undo() && doc.undo());

        // A data column to the right of every formula column.
        doc.move_columns(0, 0, 1, 5).unwrap();
        assert_eq!(
            names(&doc),
            ["Revenue", "Profit", "Cost", "Left", "Tax", "Month"]
        );
        assert_eq!(col(&doc, 0, 4), ["8", "10"]);
        assert_eq!(col(&doc, 0, 5), ["Jan", "Feb"]);
        assert!(doc.undo());

        // A new column is the last, and a new formula column is named in
        // the header row, with its `*`s.
        let among = doc.text().to_string();
        doc.add_column(0, "Note").unwrap();
        doc.add_computed(0, "Net", "Profit - Tax").unwrap();
        assert_eq!(
            names(&doc),
            [
                "Month", "Revenue", "Profit", "Cost", "Left", "Tax", "Note", "Net"
            ]
        );
        assert!(doc.text().contains("| Tax | Note | Net |\n"));
        assert!(doc.text().contains("| *   |      | *   |\n"));
        assert!(
            doc.text()
                .contains("\nTax := Profit * Rate\nNet := Profit - Tax\n")
        );
        assert_eq!(col(&doc, 0, 7), ["32", "40"]);
        // Renamed with its formula, and deleted with it.
        doc.rename_column(0, 2, "Gain").unwrap();
        assert!(doc.text().contains("| Revenue | Gain | Cost |"));
        assert!(
            doc.text()
                .contains("\nGain := Revenue - Cost\nTax := Gain * Rate\n")
        );
        doc.delete_columns(0, 2, 1).unwrap();
        assert_eq!(
            names(&doc),
            ["Month", "Revenue", "Cost", "Left", "Tax", "Note", "Net"]
        );
        assert!(!doc.text().contains("Gain :=") && !doc.text().contains("| Gain"));
        assert!(doc.undo() && doc.undo() && doc.undo() && doc.undo());
        assert_eq!(doc.text(), among);
        assert!(doc.undo());
        assert_eq!(doc.text(), text);

        // Written by hand: a cell of a formula column that is not `*`, and
        // a formula column that the header row does not name.
        let bad = Document::from_text("sheet T\n\nA | B\n1 | 2\n\nB := A + 1\n");
        let problems = &bad.snapshot().problems;
        assert!(
            problems[0].message.contains("its cells are written `*`"),
            "{problems:?}"
        );
        let unnamed = Document::from_text("sheet T\n\nA\n1\n\nB := A + 1\n");
        let problems = &unnamed.snapshot().problems;
        assert!(
            problems[0].message.contains("is not in the header row"),
            "{problems:?}"
        );
        // It is still shown, and moving any column names it.
        assert_eq!(col(&unnamed, 0, 1), ["2"]);
        let mut fixed = unnamed;
        fixed.move_columns(0, 1, 1, 0).unwrap();
        assert_eq!(
            fixed.text(),
            "sheet T\n\n| B   | A   |\n| --- | --- |\n| *   | 1   |\n\nB := A + 1\n"
        );
        assert!(fixed.snapshot().problems.is_empty());
    }

    #[test]
    fn cut_cells_keep_what_they_read() {
        let cols = ["A", "B", "C", "D"];
        let moved =
            |cell: &str, from, block: Block, by| moved_cell(cell, from, &block, by, "T", 6, &cols);
        // Sideways counts the columns again, and up or down the rows.
        assert_eq!(
            moved("= [*-2; *] * 10", (2, 3), (2..3, 3..4), (0, -3)),
            "= [*+1; *] * 10"
        );
        assert_eq!(
            moved("= [B; *-1] + 10", (2, 3), (2..3, 3..4), (2, 0)),
            "= [B; *-3] + 10"
        );
        assert_eq!(
            moved("= T[*-1; *-1]", (2, 3), (2..3, 3..4), (1, -1)),
            "= T[*; *-2]"
        );
        // A plain `*` read of a cell that is not cut is that cell still, and
        // a range ending on one goes as it is.
        assert_eq!(
            moved("= [*; *-1]", (2, 3), (2..3, 3..4), (0, -2)),
            "= [*+2; *-1]"
        );
        assert_eq!(
            moved("= sum([B; *-2..*])", (2, 3), (2..3, 3..4), (2, 0)),
            "= sum([B; *-2..*])"
        );
        assert_eq!(
            moved("= sum([B; *-2..*-1])", (2, 3), (2..3, 3..4), (2, 0)),
            "= sum([B; *-4..*-3])"
        );
        // A number says where it means already, and a name alone or a `*`
        // is the row it was in (D221).
        assert_eq!(
            moved("= [1; 0] + [B; *] + B", (2, 3), (2..3, 3..4), (2, -2)),
            "= [1; 0] + [B; *-2] + [B; *-2]"
        );
        // Cells cut together stay as far apart; a number or a name follows. A
        // cell not cut with it is read where it is, a row up from here.
        assert_eq!(
            moved(
                "= [*-1; *] + [*-3; *] + [C; 2]",
                (2, 3),
                (2..3, 2..4),
                (1, -2)
            ),
            "= [*-1; *] + [*-1; *-1] + [A; 3]"
        );
        assert_eq!(moved("*-1", (2, 3), (2..3, 3..4), (0, -2)), "*-1");
    }

    #[test]
    fn formulas_follow_a_cut_cell() {
        let text = "const K = T[C; 1]\n\nsheet T\n\nA | B | C | D                                    | E\n  |   | 1 | = [*-1; *] + C                       | *\n  |   | 5 | = [*-1; *] * 10 + [2; *-1] + T[C; *] | *\n  |   |   | = sum(T.C)                           | *\n\n\
                    E := C * 2\n\nsheet U\n\nX\n= T[2; 1] + T[; 1].C + T.C[1]\n";
        let mut doc = Document::from_text(text);
        assert_eq!(col(&doc, 0, 3), ["2", "56", "6"]);
        // The 5 goes from C to A and down a row, as a cut and a paste do it.
        doc.set_cell(0, 1, 2, "");
        doc.paste_cut(0, (1, 2), (2, 0), &[vec!["5".to_string()]]);
        let now = doc.text();
        assert!(now.contains("const K = T[A; 2]\n"), "{now}");
        assert!(
            now.contains("| = [*-3; *+1] * 10 + [2; *-1] + T[A; *+1]"),
            "{now}"
        );
        assert!(now.contains("= T[0; 2] + T[; 2].A + T.A[2]"), "{now}");
        // A formula column and a whole column read the place.
        assert!(now.contains("E := C * 2\n"), "{now}");
        assert!(now.contains("= sum(T.C)"), "{now}");
        assert_eq!(col(&doc, 0, 3), ["2", "56", "1"]);
        assert_eq!(col(&doc, 1, 0), ["15"]);
        assert!(doc.snapshot().problems.is_empty());

        // A column named on its own follows too.
        let mut doc = Document::from_text("sheet T\n\nA | B | C\n  | 5 | = B * 2\n  |   |\n");
        doc.set_cell(0, 0, 1, "");
        doc.paste_cut(0, (0, 1), (1, 0), &[vec!["5".to_string()]]);
        assert!(doc.text().contains("| = [A; *+1] * 2"), "{}", doc.text());
        assert_eq!(col(&doc, 0, 2), ["10", ""]);

        // Put down on the cell it reads, a formula is an error, not refused.
        let mut doc = Document::from_text("sheet T\n\nA | B\n1 | = [*-1; *]\n");
        doc.set_cell(0, 0, 1, "");
        assert!(doc.paste_cut(0, (0, 1), (0, 0), &[vec!["= [*-1; *]".to_string()]]));
        assert!(
            doc.text().contains("\n| = [*; *] |     |\n"),
            "{}",
            doc.text()
        );
        assert!(!doc.snapshot().problems.is_empty());
    }

    #[test]
    fn edits_as_one_undo_as_one() {
        let text = "sheet T\n\n| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";
        let mut doc = Document::from_text(text);
        assert!(doc.as_one(|doc| {
            doc.set_cell(0, 0, 0, "");
            doc.set_cell(0, 0, 1, "1");
        }));
        assert_eq!(col(&doc, 0, 1), ["1"]);
        assert!(doc.undo());
        assert_eq!(doc.text(), text);
        assert!(!doc.undo());
        assert!(!doc.as_one(|_| {}));
        // Edits that come to nothing leave nothing to undo.
        assert!(!doc.as_one(|doc| {
            doc.set_cell(0, 0, 0, "");
            doc.set_cell(0, 0, 0, "1");
        }));
        assert!(!doc.undo());
    }

    #[test]
    fn a_sheet_moves_with_its_comment() {
        let text = "# One\nsheet T\n\n| A |\n| - |\n| 1 |\n\nsheet U\n\n| X |\n| - |\n| 2 |\n\n# Three\nsheet V\n\n| Y |\n| - |\n| 3 |\n";
        let order = |doc: &Document| {
            doc.snapshot()
                .sheets
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>()
        };
        let mut doc = Document::from_text(text);
        doc.move_sheet(0, 2).unwrap();
        assert_eq!(order(&doc), ["U", "V", "T"]);
        assert!(doc.text().contains("# One\nsheet T"));
        assert!(doc.snapshot().problems.is_empty());
        doc.move_sheet(2, 1).unwrap();
        assert_eq!(order(&doc), ["U", "T", "V"]);
        assert!(doc.text().contains("| 1 |\n\n# Three\nsheet V"));
        doc.move_sheet(1, 0).unwrap();
        assert_eq!(doc.text(), text);
    }

    #[test]
    fn a_sheet_pasted_onto_another_replaces_it_under_its_name_copied() {
        let text = "sheet T\n\n| A   | B   |\n| --- | --- |\n| 1   | 2   |\n\nB := A + 1\n\n# Kept\nsheet U\n\n| X |\n| - |\n| 9 |\n";
        let mut doc = Document::from_text(text);
        assert_eq!(doc.paste_sheet(0, 1), Ok("T_001".to_string()));
        let columns = &doc.snapshot().sheets[1].columns;
        assert_eq!(
            columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["A", "B"]
        );
        assert_eq!(col(&doc, 1, 1), ["2"]);
        assert_eq!(doc.snapshot().sheets[1].name, "T_001");
        assert!(doc.text().contains("# Kept\nsheet T_001"));
        // Pasted again, onto the copy: the next free name.
        assert_eq!(doc.paste_sheet(1, 0), Ok("T_002".to_string()));
        assert!(doc.undo());
        assert!(doc.undo());
        assert_eq!(doc.text(), text);
    }

    #[test]
    fn steps_joined_since_a_mark_undo_as_one() {
        let text = "sheet T\n\n| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";
        let mut doc = Document::from_text(text);
        let mark = doc.undo_mark();
        doc.set_cell(0, 0, 0, "3");
        doc.rename_column(0, 1, "B_001").unwrap();
        doc.join_since(mark);
        assert!(doc.undo());
        assert_eq!(doc.text(), text);
        assert!(!doc.undo());
    }

    #[test]
    fn moved_rows_keep_a_range_that_starts_after() {
        let text = "sheet T\n\nName | N | M\na | 1 |\nb | 2 |\nc | 3 |\n\
                    d | 4 | = sum([N; 0^..2]) + sum([N; ^2])\n";
        let mut doc = Document::from_text(text);
        assert_eq!(col(&doc, 0, 2), ["", "", "", "8"]);
        // `b` to the bottom: the range still starts after `a`, and the
        // first two are whichever rows are first.
        doc.move_rows(0, 1, 1, 3).unwrap();
        let now = doc.text();
        assert!(now.contains("| = sum([N; 0^..3]) + sum([N; ^2])"), "{now}");
        // `a` to the bottom: there is no row left to start after.
        doc.move_rows(0, 0, 1, 3).unwrap();
        let now = doc.text();
        assert!(now.contains("| = sum([N; 0..2]) + sum([N; ^2])"), "{now}");
        assert!(doc.snapshot().problems.is_empty());
    }

    #[test]
    fn moved_rows_keep_what_reads_them() {
        let text = "const First = T[N; 0]\n\nsheet T\n\nName | N | M                                   | Sum\na    | 1 |                                     | *\nb    | 2 |                                     | *\nc    | 3 | = [N; *-1] + 10                     | *\n\
                    d    | 4 | = [N; 0] + sum([N; 0..1]) + [N; -3] | *\n\nSum := N + ([N; *-1] // 0)\n";
        let mut doc = Document::from_text(text);
        assert_eq!(col(&doc, 0, 2), ["", "", "12", "6"]);
        assert_eq!(col(&doc, 0, 3), ["1", "3", "5", "7"]);
        // `b` to the bottom: what read it still does, and the range follows
        // its ends, taking in the rows now between them.
        doc.move_rows(0, 1, 1, 3).unwrap();
        assert_eq!(col(&doc, 0, 0), ["a", "c", "d", "b"]);
        let now = doc.text();
        assert!(now.contains("| = [N; *+2] + 10"), "{now}");
        assert!(
            now.contains("| = [N; 0] + sum([N; 0..3]) + [N; -1]"),
            "{now}"
        );
        assert!(now.contains("const First = T[N; 0]\n"), "{now}");
        assert_eq!(col(&doc, 0, 2), ["", "12", "13", ""]);
        // The formula column still reads the row before, in the new order.
        assert!(now.contains("Sum := N + ([N; *-1] // 0)\n"), "{now}");
        assert_eq!(col(&doc, 0, 3), ["1", "4", "7", "6"]);
        assert!(doc.snapshot().problems.is_empty());
        // `a` down one: the number follows it, in the constant too.
        doc.move_rows(0, 0, 1, 1).unwrap();
        assert!(
            doc.text().contains("const First = T[N; 1]\n"),
            "{}",
            doc.text()
        );
        assert!(
            doc.text().contains("| = [N; 1] + sum([N; 1..3]) + [N; -1]"),
            "{}",
            doc.text()
        );
        assert!(doc.undo());
        assert!(doc.text().contains("const First = T[N; 0]\n"));
    }

    #[test]
    fn the_example_of_references_holds_through_moves_and_cuts() {
        // The checks that must hold through anything: `cell` and `own`.
        let failed = |doc: &Document| col(doc, 2, 2)[..2].join(" ");
        let cut = |doc: &mut Document, from: (usize, usize), to: (usize, usize)| {
            let source = doc.snapshot().sheets[0].rows[from.0][from.1].source.clone();
            doc.set_cell(0, from.0, from.1, "");
            doc.paste_cut(0, from, to, &[vec![source]]);
        };
        let text = include_str!("../../examples/refs.omx");
        let mut doc = Document::from_text(text);
        assert_eq!(col(&doc, 2, 2), ["0", "0", "0", "0"]);

        // Rows and columns, moved about.
        doc.move_rows(0, 3, 4, 10).unwrap();
        assert_eq!(failed(&doc), "0 0", "{}", doc.text());
        doc.move_rows(0, 0, 1, 19).unwrap();
        doc.move_rows(0, 15, 5, 2).unwrap();
        assert_eq!(failed(&doc), "0 0", "{}", doc.text());
        doc.move_columns(0, 1, 2, 5).unwrap();
        assert_eq!(failed(&doc), "0 0", "{}", doc.text());
        doc.move_columns(0, 7, 1, 0).unwrap();
        doc.move_columns(0, 8, 1, 9).unwrap();
        assert_eq!(failed(&doc), "0 0", "{}", doc.text());
        assert!(doc.snapshot().problems.is_empty());

        // A number that much else reads, cut and put down elsewhere; then a
        // formula, into the gap it left.
        let mut doc = Document::from_text(text);
        cut(&mut doc, (0, 1), (10, 3));
        assert!(doc.text().contains("const Corner  = Grid[3; 10]\n"));
        assert_eq!(failed(&doc), "0 0", "{}", doc.text());
        cut(&mut doc, (17, 7), (0, 1));
        assert!(
            doc.text().contains("= [*+2; *+14] + [*+3; *+15]"),
            "{}",
            doc.text()
        );
        assert_eq!(failed(&doc), "0 0", "{}", doc.text());
        // And all of it moved again.
        doc.move_rows(0, 0, 3, 12).unwrap();
        doc.move_columns(0, 3, 1, 1).unwrap();
        assert_eq!(failed(&doc), "0 0", "{}", doc.text());
    }

    #[test]
    fn columns_can_be_put_in_and_copied() {
        let text = "const K = T[1; 0] + T[-1; 0]\n\nsheet T\n\nA | B | C                                  | S\n1 | 2 | = [*-1; *] * 10 + [0; *] + [-3; *] | *\n3 | 4 | = [*-2; *]                         | *\n\n\
                    S := A + [1; *]\n";
        let names = |doc: &Document| -> Vec<String> {
            let columns = &doc.snapshot().sheets[0].columns;
            columns.iter().map(|c| c.name.clone()).collect()
        };
        let mut doc = Document::from_text(text);
        assert_eq!(col(&doc, 0, 2), ["23", "3"]);
        // A new column to the left of B: what counted B or S still does.
        doc.insert_column(0, 1, "N").unwrap();
        assert_eq!(names(&doc), ["A", "N", "B", "C", "S"]);
        let now = doc.text();
        assert!(now.contains("const K = T[2; 0] + T[-1; 0]\n"), "{now}");
        assert!(
            now.contains("| = [*-1; *] * 10 + [0; *] + [-3; *]"),
            "{now}"
        );
        assert!(now.contains("| = [*-3; *]"), "{now}");
        assert!(now.contains("S := A + [2; *]\n"), "{now}");
        assert_eq!(col(&doc, 0, 3), ["23", "3"]);
        assert_eq!(col(&doc, 0, 4), ["3", "7"]);
        // One step to undo.
        assert!(doc.undo());
        assert_eq!(doc.text(), text);
        assert!(!doc.can_undo());

        // A formula column, before the others.
        doc.insert_computed(0, 0, "D", "A * 2").unwrap();
        assert_eq!(names(&doc), ["D", "A", "B", "C", "S"]);
        let now = doc.text();
        assert!(now.contains("const K = T[2; 0] + T[-1; 0]\n"), "{now}");
        assert!(now.contains("\n| D   | A   | B   | C "), "{now}");
        assert!(now.contains("\n| *   | 1   | 2   | = "), "{now}");
        assert_eq!(col(&doc, 0, 0), ["2", "6"]);
        assert_eq!(col(&doc, 0, 3), ["23", "3"]);
        assert_eq!(col(&doc, 0, 4), ["3", "7"]);
        assert!(doc.undo());
        assert_eq!(doc.text(), text);
    }

    #[test]
    fn rows_can_be_moved() {
        let text = "sheet T\n\nName | N               | Sum\na    | 1               | *\nb    | 2               | *\nc    | = [N; *-1] + 10 | *\nd    | 4               | *\n\n\
                    Sum := N + ([N; *-1] // 0)\n";
        let mut doc = Document::from_text(text);
        assert_eq!(col(&doc, 0, 2), ["1", "3", "14", "16"]);
        // One row down, with its formula; the running column follows.
        doc.move_rows(0, 0, 1, 2).unwrap();
        assert_eq!(col(&doc, 0, 0), ["b", "c", "a", "d"]);
        assert!(
            doc.text()
                .contains("| Name | N               | Sum |\n| ---- | --------------- | --- |\n| b    | 2               | *   |\n| c    | = [N; *-1] + 10 | *   |\n| a    | 1               | *   |\n| d    | 4               | *   |\n")
        );
        assert_eq!(col(&doc, 0, 1), ["2", "12", "1", "4"]);
        assert_eq!(col(&doc, 0, 2), ["2", "14", "13", "5"]);
        // Two at once, to the top; and one step to undo.
        doc.move_rows(0, 2, 2, 0).unwrap();
        assert_eq!(col(&doc, 0, 0), ["a", "d", "b", "c"]);
        assert!(doc.undo());
        assert_eq!(col(&doc, 0, 0), ["b", "c", "a", "d"]);
        // Not off the sheet.
        let before = doc.text().to_string();
        assert!(doc.move_rows(0, 3, 2, 0).is_err());
        assert!(doc.move_rows(0, 0, 2, 3).is_err());
        assert!(doc.move_rows(0, 1, 1, 1).is_ok());
        assert_eq!(doc.text(), before);
    }

    #[test]
    fn sheets_can_be_put_in_renamed_and_deleted() {
        let text = "const K = Sales.Revenue.sum()\n\n# The sales.\nsheet Sales\n\nMonth | Revenue\nJan   | 100\nFeb   | 120\n\n\
                    sheet Summary\n\nLabel | Total                     | Twice\nAll   | = Sales[Revenue; 0] + K | *\n\nTwice := Sales.Revenue.sum() * 2\n";
        let mut doc = Document::from_text(text);
        let names = |doc: &Document| -> Vec<String> {
            doc.snapshot()
                .sheets
                .iter()
                .map(|t| t.name.clone())
                .collect()
        };

        // Before a sheet, above its comment; after the last, at the end.
        doc.insert_sheet(0, "First", None).unwrap();
        doc.insert_sheet(2, "Middle", None).unwrap();
        doc.insert_sheet(4, "Last", None).unwrap();
        assert_eq!(names(&doc), ["First", "Sales", "Middle", "Summary", "Last"]);
        assert!(
            doc.text().contains("\n\n# The sales.\nsheet Sales\n"),
            "{}",
            doc.text()
        );
        assert!(doc.insert_sheet(0, "Sales", None).is_err());
        assert!(doc.insert_sheet(0, "K", None).is_err());
        assert!(
            doc.snapshot().problems.is_empty(),
            "{:?}",
            doc.snapshot().problems
        );

        // A new name is followed wherever the sheet is read.
        doc.rename_sheet(1, "Takings").unwrap();
        assert_eq!(names(&doc)[1], "Takings");
        assert!(
            doc.text().contains("const K = Takings.Revenue.sum()"),
            "{}",
            doc.text()
        );
        assert!(
            doc.text().contains("= Takings[Revenue; 0] + K"),
            "{}",
            doc.text()
        );
        assert!(
            doc.text().contains("Twice := Takings.Revenue.sum() * 2"),
            "{}",
            doc.text()
        );
        assert!(
            doc.snapshot().problems.is_empty(),
            "{:?}",
            doc.snapshot().problems
        );
        assert_eq!(doc.snapshot().sheets[3].rows[0][1].display, "320");
        assert!(doc.rename_sheet(1, "Summary").is_err());
        assert!(doc.rename_sheet(1, "K").is_err());
        assert!(doc.rename_sheet(1, "bad name").is_err());
        assert!(doc.undo());
        assert_eq!(names(&doc)[1], "Sales");

        // Deleting takes the whole of it, and nothing of its neighbours.
        doc.delete_sheet(2).unwrap();
        doc.delete_sheet(3).unwrap();
        doc.delete_sheet(0).unwrap();
        assert_eq!(names(&doc), ["Sales", "Summary"]);
        assert_eq!(doc.text(), text);
        assert!(doc.delete_sheet(2).is_err());
        doc.delete_sheet(1).unwrap();
        doc.delete_sheet(0).unwrap();
        assert!(names(&doc).is_empty());
        assert_eq!(
            doc.text(),
            "const K = Sales.Revenue.sum()\n\n# The sales.\n"
        );
    }

    #[test]
    fn blank_document_is_valid() {
        let mut doc = Document::default();
        assert!(doc.snapshot().problems.is_empty());
        assert_eq!(doc.snapshot().sheets[0].rows.len(), 5);
        doc.set_cell(0, 0, 0, "1");
        doc.set_cell(0, 0, 1, "= A + 1");
        assert_eq!(doc.snapshot().sheets[0].rows[0][1].display, "2");
    }
}

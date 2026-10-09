// Copyright (c) 2026 Stephen Roe

//! An editable sheet for interactive front ends.
//!
//! The `.omx` text stays the single source of truth: every edit rewrites the
//! text and the sheet is then re-read and re-evaluated. A [`Snapshot`] is what
//! a grid shows: for each cell, the value to display and the source to edit.
//!
//! Unlike the command line, a document is evaluated even when it has errors,
//! so that one mistyped formula does not blank the rest of the grid; cells
//! that could not be calculated show as errors.

use crate::Options;
use crate::eval::Engine;
use crate::value::{Value, arith, format_exact, format_scalar};
use num_traits::ToPrimitive;
use omasheet_omx::ast::{BinOp, Expr, ExprKind, Lit, UnOp};
use omasheet_omx::date::{self, Style};
use omasheet_omx::parser::parse_expr;
use omasheet_omx::sheet::{SheetAst, TableDecl, ident_len, parse_sheet, split_cells};
use omasheet_omx::{ColKind, Diagnostic, S, Sources, Span, compile};

const UNDO_LIMIT: usize = 200;

/// A fresh sheet: one small empty table to type into.
pub const BLANK: &str = "table Sheet1\n\nA | B | C\n  |   |\n  |   |\n  |   |\n  |   |\n  |   |\n";

#[derive(Debug, Default, Clone)]
pub struct Snapshot {
    pub tables: Vec<TableSnap>,
    pub consts: Vec<ConstSnap>,
    pub funcs: Vec<FuncSnap>,
    pub problems: Vec<Problem>,
}

#[derive(Debug, Clone)]
pub struct TableSnap {
    pub name: String,
    pub columns: Vec<ColumnSnap>,
    pub rows: Vec<Vec<CellSnap>>,
}

#[derive(Debug, Clone)]
pub struct ColumnSnap {
    pub name: String,
    /// The column type, or empty when it is not known.
    pub ty: String,
    /// The `:=` expression of a computed column; `None` for a data column.
    pub formula: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CellSnap {
    /// The calculated value.
    pub value: Value,
    /// The value as shown in the grid.
    pub display: String,
    /// The value in full, when it is shown with fewer digits than it has.
    pub exact: Option<String>,
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
    /// The sum and the average of the numbers among them, as text; `None`
    /// if there are no numbers.
    pub numbers: Option<(String, String)>,
}

/// A function the sheet defines.
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
}

pub struct Document {
    text: String,
    snapshot: Snapshot,
    undo: Vec<String>,
    redo: Vec<String>,
    style: Style,
    /// Show date-times on the clocks of this machine.
    local: bool,
}

impl Default for Document {
    fn default() -> Self {
        Document::from_text(BLANK)
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    inner.start >= outer.start && inner.start <= outer.end
}

/// A date or time typed the way `style` writes it (`31/1/2025`, `9:30 pm`),
/// as a sheet writes it, for a column of type `ty`. `None` if it is not one.
fn iso_entry(style: &Style, ty: &str, input: &str) -> Option<String> {
    let text = input.trim();
    let date = || style.parse_date(text).map(date::format);
    let time = || style.parse_time(text).map(date::format_time);
    let datetime = || style.parse_datetime(text).map(date::format_datetime);
    match ty {
        "Date" => date(),
        "Time" => time(),
        // A date alone is the start of that day.
        "DateTime" => datetime().or_else(|| Some(format!("{}T00:00", date()?))),
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
        "Int" => "Int: a whole number, e.g. 42".into(),
        "Ratio" => "Ratio: an exact number, e.g. 19.99, 20% or 1/7".into(),
        "Num" => "Num: a floating-point number, e.g. 1.5 or 2e-3".into(),
        "Complex" => "Complex: e.g. 3+4i or 2.5i".into(),
        "Bool" => "Bool: true or false".into(),
        "Text" => "Text".into(),
        "Date" => format!(
            "Date: {}, e.g. {}{also_iso}",
            style.date_pattern(),
            style.date(day)
        ),
        "Time" => format!("Time: e.g. {}", style.time(time)),
        "DateTime" => format!(
            "DateTime: e.g. {}{also_iso}",
            style.datetime(day as i64 * 86_400 + time as i64)
        ),
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
            ExprKind::Lit(Lit::Int(k)) => k.to_i64(),
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

/// One index into a table: `T[columns; rows]` and its other spellings.
struct Site<'a> {
    /// The table as written; `None` for an index with no table name.
    table: Option<&'a str>,
    row: Option<&'a Expr>,
    col: ColPart<'a>,
    /// Inside a slot of another index, where the current row is a row of
    /// that index and not the formula's own.
    nested: bool,
}

/// The table an index is into, if its base says so: `Some(None)` for an
/// index with no table name.
fn base_table(e: &Expr) -> Option<Option<&str>> {
    match &e.kind {
        ExprKind::Own => Some(None),
        ExprKind::Name(name) => Some(Some(name)),
        _ => None,
    }
}

/// Every index into a table in `e`, and every name that stands alone, which
/// in a cell may be a column of the cell's own row.
fn sites<'a>(e: &'a Expr, nested: bool, out: &mut Vec<Site<'a>>, names: &mut Vec<(&'a str, Span)>) {
    match &e.kind {
        ExprKind::Lit(_) | ExprKind::Cursor | ExprKind::Own => {}
        ExprKind::Name(name) => {
            if !nested {
                names.push((name, e.span));
            }
        }
        ExprKind::Unary(_, a) => sites(a, nested, out, names),
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
                if slots.len() == 2 && slots[0].is_none() && base_table(base).is_some() =>
            {
                out.push(Site {
                    table: base_table(base).flatten(),
                    row: slots[1].as_ref(),
                    col: ColPart::Field(name, *span),
                    nested,
                });
                if let Some(slot) = &slots[1] {
                    sites(slot, true, out, names);
                }
            }
            // `T.Name`: a table and its column, not a name on its own.
            ExprKind::Name(_) => {}
            _ => sites(a, nested, out, names),
        },
        ExprKind::Index(base, slots) => {
            if let Some(table) = base_table(base) {
                out.push(Site {
                    table,
                    row: slots.get(1).and_then(Option::as_ref),
                    col: match slots.first() {
                        Some(Some(slot)) => ColPart::Slot(slot),
                        _ => ColPart::All,
                    },
                    nested,
                });
            } else if let ExprKind::Field(inner, name, span) = &base.kind
                && let ExprKind::Name(table) = &inner.kind
                && slots.len() == 1
            {
                // `T.Name[0]`
                out.push(Site {
                    table: Some(table),
                    row: slots[0].as_ref(),
                    col: ColPart::Field(name, *span),
                    nested,
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

/// Where a formula is: its table, and its row and column if it is in a cell.
#[derive(Clone, Copy)]
struct Place {
    table: Option<usize>,
    cell: Option<(usize, usize)>,
}

/// Every formula of a sheet, with spans into the text of the sheet.
fn formulas(ast: &SheetAst) -> Vec<(Place, Expr)> {
    let mut out = Vec::new();
    let nowhere = Place {
        table: None,
        cell: None,
    };
    for c in &ast.consts {
        out.extend(c.expr.clone().map(|e| (nowhere, e)));
    }
    for f in &ast.funcs {
        out.extend(f.expr.clone().map(|e| (nowhere, e)));
    }
    for (t, decl) in ast.tables.iter().enumerate() {
        for (r, row) in decl.rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if let Some(body) = cell.text.strip_prefix('=')
                    && let Ok(e) = parse_expr(body, 0, cell.span.start as usize + 1)
                {
                    let cell = Some((r, c));
                    out.push((
                        Place {
                            table: Some(t),
                            cell,
                        },
                        e,
                    ));
                }
            }
        }
        let place = Place {
            table: Some(t),
            cell: None,
        };
        for c in &decl.computed {
            out.extend(c.expr.clone().map(|e| (place, e)));
        }
    }
    out
}

/// Whether `site`, in a formula at `place`, is an index into table `t`; and
/// if so, whether a `*` in it is the formula's own cell.
fn into_table(site: &Site, place: Place, t: usize, name: &str) -> Option<bool> {
    let home = place.table == Some(t) && !site.nested;
    match site.table {
        Some(table) if table == name => Some(home && place.cell.is_some()),
        None if home => Some(place.cell.is_some()),
        _ => None,
    }
}

/// The names of the columns of a table, data columns first.
fn column_names(decl: &TableDecl) -> Vec<&str> {
    let data = decl.header.iter().map(|h| h.0.as_str());
    data.chain(decl.computed.iter().map(|c| c.name.as_str()))
        .collect()
}

/// `(start, end, replacement)` in the text of a sheet.
type Edit = (usize, usize, String);

/// Note that `e` is to read `new`, unless it does already.
fn put(edits: &mut Vec<Edit>, text: &str, span: Span, new: String) {
    let (s, e) = (span.start as usize, span.end as usize);
    if text.get(s..e).is_some_and(|old| old != new) {
        edits.push((s, e, new));
    }
}

/// `text` with `edits` made, and the tables they fall in lined up again.
fn rewritten(text: &str, ast: &SheetAst, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    edits.dedup_by_key(|e| e.0);
    let mut out = text.to_string();
    let mut touched: Vec<&str> = Vec::new();
    for (s, e, new) in &edits {
        let owner = ast.tables.iter().rfind(|t| t.span.start as usize <= *s);
        if let Some(t) = owner.filter(|t| !touched.contains(&t.name.as_str())) {
            touched.push(&t.name);
        }
        out.replace_range(*s..*e, new);
    }
    for name in touched {
        let ast = parse_sheet(&out, 0, &mut Vec::new());
        if let Some(decl) = ast.tables.iter().find(|t| t.name == name) {
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
/// column it was, when those of table `t` are rearranged so that `place[k]`
/// is where the k-th ends up, of `count` in all. An offset in a formula column
/// is left alone:
/// one formula serves every row, so `*-1` goes on meaning the row before.
fn follow_move(
    ast: &SheetAst,
    text: &str,
    t: usize,
    rows: bool,
    place: &[usize],
    count: usize,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    let name = ast.tables[t].name.as_str();
    for (at, expr) in formulas(ast) {
        let mut found = Vec::new();
        sites(&expr, false, &mut found, &mut Vec::new());
        for site in found {
            let Some(relative) = into_table(&site, at, t, name) else {
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

/// The one cell of a table that `site` reads, if it reads just one: its row
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
/// table `t`, on that cell once it is `by` rows and columns away. Only a
/// reference to one cell follows it: a range or a whole column reads the
/// place, and so does a formula column.
fn follow_cut(ast: &SheetAst, text: &str, t: usize, block: &Block, by: (i64, i64)) -> Vec<Edit> {
    let mut edits = Vec::new();
    let decl = &ast.tables[t];
    let cols = column_names(decl);
    let nrows = decl.rows.len();
    let inside = |r: usize, c: usize| block.0.contains(&r) && block.1.contains(&c);
    let column = |c: usize| {
        usize::try_from(c as i64 + by.1)
            .ok()
            .and_then(|c| cols.get(c))
    };
    for (at, expr) in formulas(ast) {
        let (mut found, mut names) = (Vec::new(), Vec::new());
        sites(&expr, false, &mut found, &mut names);
        for site in found {
            let Some(relative) = into_table(&site, at, t, &decl.name) else {
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
            if by.0 != 0 {
                let to = r as i64 + by.0;
                put(
                    &mut edits,
                    text,
                    row_span,
                    row.write(to, own.map(|o| o.0), nrows),
                );
            }
            if by.1 != 0 {
                let to = c as i64 + by.1;
                let new = match col {
                    Some(pick) => Some(pick.write(to, own.map(|o| o.1), cols.len())),
                    None => column(c).map(|name| name.to_string()),
                };
                if let Some(new) = new {
                    put(&mut edits, text, col_span, new);
                }
            }
        }
        // A column named on its own in a cell is that column of its row.
        let Some((r, _)) = at.cell.filter(|_| at.table == Some(t)) else {
            continue;
        };
        for (name, span) in names {
            let cut = cols
                .iter()
                .position(|c| *c == name)
                .filter(|c| inside(r, *c));
            if let Some(new) = cut.and_then(column) {
                let new = match by.0 {
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
    let shift = |e: &Expr, edits: &mut Vec<Edit>| {
        if let Some(Pick::Off(d)) = Pick::of(e) {
            put(edits, text, e.span, offset_text(d - by));
        }
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

/// The source of a cell of `block`, cut from `from` in a table and put down
/// `by` rows and columns away. What its formula reads by an offset it still
/// reads: a move sideways counts the columns again, and a move up or down
/// the rows. A cell that was cut along with it is as far away as it was.
fn moved_cell(
    cell: &str,
    from: (usize, usize),
    block: &Block,
    by: (i64, i64),
    table: &str,
    nrows: usize,
    cols: &[&str],
) -> String {
    let Some(Ok(expr)) = cell.strip_prefix('=').map(|body| parse_expr(body, 0, 1)) else {
        return cell.to_string();
    };
    let mut found = Vec::new();
    sites(&expr, false, &mut found, &mut Vec::new());
    let mut edits = Vec::new();
    for site in found {
        if site.nested || site.table.is_some_and(|name| name != table) {
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
    let text: String = input
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = text.trim();
    if text.is_empty() || split_cells(text, 0, 0).len() == 1 {
        return text.to_string();
    }
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
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

/// The header and rows of a table as text, plus where each row came from.
struct Grid {
    header: Vec<String>,
    /// `(original row index, cells)`; `None` for a new row.
    rows: Vec<(Option<usize>, Vec<String>)>,
}

impl Grid {
    fn of(t: &TableDecl) -> Grid {
        Grid {
            header: t.header.iter().map(|(name, _)| name.clone()).collect(),
            rows: t
                .rows
                .iter()
                .enumerate()
                .map(|(i, row)| (Some(i), row.iter().map(|c| c.text.clone()).collect()))
                .collect(),
        }
    }

    fn blank_row(&self) -> Vec<String> {
        // A lone empty cell would be a blank line, which is not a row.
        let cell = if self.header.len() == 1 { "\"\"" } else { "" };
        vec![cell.to_string(); self.header.len()]
    }

    /// Rewrite the table's lines in `text`, aligned, touching nothing else.
    fn write(&self, t: &TableDecl, text: &str) -> String {
        let n = self.header.len();
        let mut widths: Vec<usize> = self.header.iter().map(|h| h.chars().count()).collect();
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
            // A trailing empty cell still ends in its separator.
            padded.join(" | ").trim_end().to_string()
        };

        // (start, end, replacement), applied from the end of the text back.
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        let range = |s: Span| (s.start as usize, s.end as usize);
        let (hs, he) = range(t.header_line);
        edits.push((hs, he, line(&self.header)));
        let mut anchor = he;
        if let Some(sep) = t.separator_line {
            let (s, e) = range(sep);
            let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
            edits.push((s, e, rule.join("-|-")));
            anchor = e;
        }
        let kept: Vec<usize> = self.rows.iter().filter_map(|(o, _)| *o).collect();
        for (i, span) in t.row_lines.iter().enumerate() {
            if !kept.contains(&i) {
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
        let mut out = text.to_string();
        let mut order: Vec<usize> = (0..edits.len()).collect();
        order.sort_by(|&a, &b| edits[b].0.cmp(&edits[a].0).then(b.cmp(&a)));
        for k in order {
            let (s, e, ref replacement) = edits[k];
            out.replace_range(s..e, replacement);
        }
        out
    }
}

impl Document {
    /// A document that shows dates and times as the sheet writes them.
    pub fn from_text(text: &str) -> Document {
        Document::new(text, Style::ISO, false)
    }

    /// A document for a person to look at: dates and times are shown in
    /// `style`, and date-times on the clocks of this machine even when the
    /// sheet is in another time zone. See [`Document::set_style`].
    pub fn with_style(text: &str, style: Style) -> Document {
        Document::new(text, style, true)
    }

    fn new(text: &str, style: Style, local: bool) -> Document {
        let mut doc = Document {
            text: text.to_string(),
            snapshot: Snapshot::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            style,
            local,
        };
        doc.rebuild();
        doc
    }

    /// How dates and times are shown, and how they may be typed into a cell.
    /// The text of the sheet stays ISO whatever the style.
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
        self.redo.push(std::mem::replace(&mut self.text, previous));
        self.rebuild();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.text, next));
        self.rebuild();
        true
    }

    /// Replace the text as one undoable step. Returns whether it changed.
    fn commit(&mut self, text: String) -> bool {
        if text == self.text {
            return false;
        }
        self.undo.push(std::mem::replace(&mut self.text, text));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.rebuild();
        true
    }

    fn ast(&self) -> SheetAst {
        parse_sheet(&self.text, 0, &mut Vec::new())
    }

    /// The declaration of the snapshot's table `table`.
    fn table_decl<'a>(&self, ast: &'a SheetAst, table: usize) -> Option<&'a TableDecl> {
        let name = &self.snapshot.tables.get(table)?.name;
        ast.tables.iter().find(|t| &t.name == name)
    }

    /// `text` with the grid of table `table` changed.
    fn grid_edited(
        &self,
        text: &str,
        table: usize,
        change: impl FnOnce(&mut Grid),
    ) -> Option<String> {
        let ast = parse_sheet(text, 0, &mut Vec::new());
        let decl = self.table_decl(&ast, table)?;
        let mut grid = Grid::of(decl);
        change(&mut grid);
        Some(grid.write(decl, text))
    }

    fn edit_grid(&mut self, table: usize, change: impl FnOnce(&mut Grid)) -> bool {
        match self.grid_edited(&self.text, table, change) {
            Some(text) => self.commit(text),
            None => false,
        }
    }

    /// Set cells of one table in a single undoable step. Cells are
    /// `(row, column, text)`; rows past the end are added. Cells in computed
    /// columns are skipped.
    pub fn set_cells(&mut self, table: usize, cells: &[(usize, usize, String)]) -> bool {
        match self.cells_set(&self.text, table, cells) {
            Some(text) => self.commit(text),
            None => false,
        }
    }

    /// `text` with cells of one table set, as `set_cells` sets them.
    fn cells_set(
        &self,
        text: &str,
        table: usize,
        cells: &[(usize, usize, String)],
    ) -> Option<String> {
        let style = self.style;
        let types: Vec<String> = self
            .snapshot
            .tables
            .get(table)
            .map(|t| t.columns.iter().map(|c| c.ty.clone()).collect())
            .unwrap_or_default();
        self.grid_edited(text, table, |grid| {
            for (row, col, text) in cells {
                if *col >= grid.header.len() {
                    continue;
                }
                while grid.rows.len() <= *row {
                    let blank = grid.blank_row();
                    grid.rows.push((None, blank));
                }
                let single = grid.header.len() == 1;
                let ty = types.get(*col).map_or("", String::as_str);
                let cleaned = iso_entry(&style, ty, text).unwrap_or_else(|| clean_cell(text));
                grid.rows[*row].1[*col] = if cleaned.is_empty() && single {
                    "\"\"".to_string()
                } else {
                    cleaned
                };
            }
        })
    }

    /// Put down `block`, the sources of cells cut from `from` in a table, at
    /// `to` in the same table, in one undoable step. Formulas go on reading
    /// what they read: those put down count their offsets again, and those
    /// that read a cell that was cut follow it to where it now is.
    pub fn paste_cut(
        &mut self,
        table: usize,
        from: (usize, usize),
        to: (usize, usize),
        block: &[Vec<String>],
    ) -> bool {
        let ast = self.ast();
        let Some(name) = self.snapshot.tables.get(table).map(|t| t.name.clone()) else {
            return false;
        };
        let Some(t) = ast.tables.iter().position(|t| t.name == name) else {
            return false;
        };
        let width = block.iter().map(Vec::len).max().unwrap_or(0);
        let cut: Block = (from.0..from.0 + block.len(), from.1..from.1 + width);
        let by = (to.0 as i64 - from.0 as i64, to.1 as i64 - from.1 as i64);
        let decl = &ast.tables[t];
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
        match self.cells_set(&followed, table, &cells) {
            Some(text) => self.commit(text),
            None => false,
        }
    }

    /// The count, sum and average of the block of cells from `(top, left)`
    /// to `(bottom, right)`. Numbers add up exactly, as they do in a formula.
    pub fn summary(
        &self,
        table: usize,
        (top, left): (usize, usize),
        (bottom, right): (usize, usize),
    ) -> Option<Summary> {
        let rows = &self.snapshot.tables.get(table)?.rows;
        let values = rows
            .iter()
            .take(bottom + 1)
            .skip(top)
            .flat_map(|row| row.iter().take(right + 1).skip(left))
            .map(|cell| &cell.value);
        let (mut count, mut numbers) = (0, 0);
        let mut sum = Value::Int(0.into());
        for value in values {
            if !matches!(value, Value::Empty | Value::Error) {
                count += 1;
            }
            if value.is_numeric() {
                numbers += 1;
                sum = arith(BinOp::Add, &sum, value).ok()?;
            }
        }
        let numbers = if numbers == 0 {
            None
        } else {
            let avg = arith(BinOp::Div, &sum, &Value::Int(numbers.into())).ok()?;
            Some((format_scalar(&sum, false), format_scalar(&avg, false)))
        };
        Some(Summary { count, numbers })
    }

    pub fn set_cell(&mut self, table: usize, row: usize, col: usize, text: &str) -> bool {
        let is_computed = self
            .snapshot
            .tables
            .get(table)
            .and_then(|t| t.columns.get(col))
            .is_some_and(|c| c.formula.is_some());
        if is_computed {
            return self.set_column_formula(table, col, text);
        }
        self.set_cells(table, &[(row, col, text.to_string())])
    }

    /// Insert `count` empty rows before row `at`.
    pub fn insert_rows(&mut self, table: usize, at: usize, count: usize) -> bool {
        self.edit_grid(table, |grid| {
            let at = at.min(grid.rows.len());
            for _ in 0..count {
                let blank = grid.blank_row();
                grid.rows.insert(at, (None, blank));
            }
        })
    }

    pub fn delete_rows(&mut self, table: usize, first: usize, count: usize) -> bool {
        self.edit_grid(table, |grid| {
            let end = (first + count).min(grid.rows.len());
            if first < end {
                grid.rows.drain(first..end);
            }
        })
    }

    /// Delete `count` columns starting at `first`, with their cells or
    /// formulas and their type lines, in one undoable step. The columns must
    /// be all data columns or all formula columns, and a table keeps at least
    /// one data column. A formula that reads a deleted column is left as it
    /// is, to be shown as an error.
    pub fn delete_columns(
        &mut self,
        table: usize,
        first: usize,
        count: usize,
    ) -> Result<(), String> {
        let Some(snap) = self.snapshot.tables.get(table) else {
            return Err("there is no such table".into());
        };
        let n = snap.columns.len();
        if count == 0 || first + count > n {
            return Err("there is no such column".into());
        }
        let data = snap.columns.iter().filter(|c| c.formula.is_none()).count();
        let among_data = first + count <= data;
        if !among_data && first < data {
            return Err("delete data columns and formula columns separately".into());
        }
        if among_data && count >= data {
            return Err("a table keeps at least one data column".into());
        }
        let names: Vec<String> = snap.columns[first..first + count]
            .iter()
            .map(|c| c.name.clone())
            .collect();
        // Last among their own kind first, so that formulas which count
        // columns keep reading the ones that stay.
        let mark = self.undo.len();
        let end = if among_data { data } else { n };
        self.move_columns(table, first, count, end - count)?;

        let ast = self.ast();
        let Some(decl) = self.table_decl(&ast, table) else {
            return Err("there is no such table".into());
        };
        // Whole lines to take out: type lines, and `Name := expression`.
        let line = |s: usize, e: usize| {
            let start = self.text[..s].rfind('\n').map_or(0, |k| k + 1);
            let end = self.text[e..]
                .find('\n')
                .map_or(self.text.len(), |k| e + k + 1);
            (start, end)
        };
        let schema = decl.schema.iter().filter(|l| names.contains(&l.name));
        let mut cuts: Vec<(usize, usize)> = schema
            .map(|l| line(l.span.start as usize, l.span.end as usize))
            .collect();
        let mut text = if among_data {
            let mut grid = Grid::of(decl);
            let keep = data - count;
            grid.header.truncate(keep);
            for (_, row) in &mut grid.rows {
                row.truncate(keep);
                // A lone empty cell would be a blank line, which is not a row.
                if keep == 1 && row[0].is_empty() {
                    row[0] = "\"\"".to_string();
                }
            }
            grid.write(decl, &self.text)
        } else {
            let gone = decl.computed.iter().filter(|c| names.contains(&c.name));
            cuts.extend(gone.map(|c| line(c.span.start as usize, c.expr_span.end as usize)));
            self.text.clone()
        };
        // The lines to take out come before the rows, or are the whole
        // change, so their places in the text still hold.
        cuts.sort_unstable();
        for (s, e) in cuts.into_iter().rev() {
            text.replace_range(s..e, "");
        }
        self.commit(text);
        self.squash(mark);
        Ok(())
    }

    /// Add an empty data column at the end of the header row.
    pub fn add_column(&mut self, table: usize, name: &str) -> Result<(), String> {
        let name = valid_name(name)?.to_string();
        let Some(snap) = self.snapshot.tables.get(table) else {
            return Err("there is no such table".into());
        };
        if snap.columns.iter().any(|c| c.name == name) {
            return Err(format!(
                "table `{}` already has a column `{name}`",
                snap.name
            ));
        }
        // The formula columns each move along one.
        let n = snap.columns.len();
        let data = snap.columns.iter().filter(|c| c.formula.is_none()).count();
        let place: Vec<usize> = (0..n).map(|k| if k < data { k } else { k + 1 }).collect();
        let followed = self.followed_to(table, false, &place, n + 1);
        let text = self.grid_edited(&followed, table, |grid| {
            // A lone column stored its empty cells as `""`.
            if grid.header.len() == 1 {
                for (_, row) in &mut grid.rows {
                    if row[0] == "\"\"" {
                        row[0].clear();
                    }
                }
            }
            grid.header.push(name);
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
    pub fn rename_column(&mut self, table: usize, col: usize, name: &str) -> Result<(), String> {
        let new = valid_name(name)?.to_string();
        let Some(snap) = self.snapshot.tables.get(table) else {
            return Err("there is no such table".into());
        };
        let Some(old) = snap.columns.get(col).map(|c| c.name.clone()) else {
            return Err("there is no such column".into());
        };
        if new == old {
            return Ok(());
        }
        if snap.columns.iter().any(|c| c.name == new) {
            return Err(format!(
                "table `{}` already has a column `{new}`",
                snap.name
            ));
        }
        let taken = self.snapshot.consts.iter().any(|c| c.name == new)
            || self.snapshot.tables.iter().any(|t| t.name == new);
        if taken {
            return Err(format!("`{new}` is already a constant or a table"));
        }
        let ast = self.ast();
        let Some(decl) = self.table_decl(&ast, table) else {
            return Err("there is no such table".into());
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
            .tables
            .iter()
            .position(|t| t.name == decl.name)
            .and_then(|t| {
                Some((
                    t,
                    program.tables[t].cols.iter().position(|c| c.name == old)?,
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
            let owner = ast.tables.iter().rfind(|t| t.span.start as usize <= s);
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
        // Line the changed tables up again.
        for name in touched {
            let ast = parse_sheet(&text, 0, &mut Vec::new());
            if let Some(decl) = ast.tables.iter().find(|t| t.name == name) {
                text = Grid::of(decl).write(decl, &text);
            }
        }
        self.commit(text);
        Ok(())
    }

    /// The text with every row or column of table `table` that a formula
    /// picks by counting still the one it was, once they are rearranged so
    /// that the k-th is `order[k]`.
    fn followed(&self, table: usize, rows: bool, order: &[usize]) -> String {
        let mut place = vec![0; order.len()];
        for (new, &old) in order.iter().enumerate() {
            place[old] = new;
        }
        self.followed_to(table, rows, &place, order.len())
    }

    /// The same, for the k-th going to `place[k]` of `count` in all: more
    /// than there were, when some are being added.
    fn followed_to(&self, table: usize, rows: bool, place: &[usize], count: usize) -> String {
        let ast = self.ast();
        let name = self.snapshot.tables.get(table).map(|t| t.name.as_str());
        let Some(t) = ast
            .tables
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

    /// A name for a copy of column `name`: `Name_copy001`, or the first
    /// after it that is free.
    fn copy_name(&self, table: usize, name: &str) -> String {
        let snap = &self.snapshot;
        let taken = |n: &str| {
            snap.tables
                .get(table)
                .is_some_and(|t| t.columns.iter().any(|c| c.name == n))
                || snap.consts.iter().any(|c| c.name == n)
                || snap.tables.iter().any(|t| t.name == n)
        };
        (1..)
            .map(|k| format!("{name}_copy{k:03}"))
            .find(|n| !taken(n))
            .unwrap_or_default()
    }

    /// Add an empty data column as column `at`, or as the last data column
    /// if `at` is among the formula columns, in one undoable step.
    pub fn insert_column(&mut self, table: usize, at: usize, name: &str) -> Result<(), String> {
        let mark = self.undo.len();
        self.add_column(table, name)?;
        let columns = &self.snapshot.tables[table].columns;
        let last = columns.iter().filter(|c| c.formula.is_none()).count() - 1;
        self.move_columns(table, last, 1, at.min(last))?;
        self.squash(mark);
        Ok(())
    }

    /// Add `Name := expression` as column `at`, or as the first formula
    /// column if `at` is among the data columns, in one undoable step.
    pub fn insert_computed(
        &mut self,
        table: usize,
        at: usize,
        name: &str,
        expr: &str,
    ) -> Result<(), String> {
        let mark = self.undo.len();
        self.add_computed(table, name, expr)?;
        let columns = &self.snapshot.tables[table].columns;
        let data = columns.iter().filter(|c| c.formula.is_none()).count();
        let last = columns.len() - 1;
        self.move_columns(table, last, 1, at.clamp(data, last))?;
        self.squash(mark);
        Ok(())
    }

    /// Put copies of the columns `cols` of a table in at column `at`, in one
    /// undoable step. A data column is copied with its cells as they are
    /// written, and a formula column with its formula; each goes among its
    /// own kind, under a name like `Name_copy001`.
    pub fn insert_copies(&mut self, table: usize, cols: &[usize], at: usize) -> Result<(), String> {
        let Some(snap) = self.snapshot.tables.get(table) else {
            return Err("there is no such table".into());
        };
        if cols.iter().any(|c| *c >= snap.columns.len()) {
            return Err("there is no such column".into());
        }
        let mark = self.undo.len();
        let (mut cols, mut at) = (cols.to_vec(), at);
        for k in 0..cols.len() {
            let column = self.snapshot.tables[table].columns[cols[k]].clone();
            let name = self.copy_name(table, &column.name);
            let data = |doc: &Document| {
                let columns = &doc.snapshot.tables[table].columns;
                columns.iter().filter(|c| c.formula.is_none()).count()
            };
            // Where the copy goes, among its own kind.
            let to = match &column.formula {
                None => {
                    let to = at.min(data(self));
                    self.insert_column(table, to, &name)?;
                    to
                }
                Some(expr) => {
                    let to = at.max(data(self));
                    self.insert_computed(table, to, &name, expr)?;
                    to
                }
            };
            for col in &mut cols {
                if *col >= to {
                    *col += 1;
                }
            }
            if column.formula.is_none() {
                let from = cols[k];
                self.edit_grid(table, |grid| {
                    for (_, row) in &mut grid.rows {
                        if let Some(cell) = row.get(from).cloned() {
                            row[to] = cell;
                        }
                    }
                });
            }
            at = to + 1;
        }
        self.squash(mark);
        Ok(())
    }

    /// Add rows at row `row` of a table and put `block` in them from column
    /// `col`, in one undoable step.
    pub fn insert_cells(
        &mut self,
        table: usize,
        (row, col): (usize, usize),
        block: &[Vec<String>],
    ) -> bool {
        let mark = self.undo.len();
        if !self.insert_rows(table, row, block.len()) {
            return false;
        }
        let mut cells = Vec::new();
        for (i, line) in block.iter().enumerate() {
            for (j, text) in line.iter().enumerate() {
                cells.push((row + i, col + j, text.clone()));
            }
        }
        self.set_cells(table, &cells);
        self.squash(mark);
        true
    }

    /// Move `count` rows starting at `first` so that the first of them
    /// becomes row `to`, in one undoable step. The rows keep their cells.
    /// A row that a formula picks by counting is still the row it was: a
    /// cell's `*-1` is counted again, and so is a number anywhere in the
    /// sheet. A formula column goes on reading the row before, in the new
    /// order.
    pub fn move_rows(
        &mut self,
        table: usize,
        first: usize,
        count: usize,
        to: usize,
    ) -> Result<(), String> {
        let Some(snap) = self.snapshot.tables.get(table) else {
            return Err("there is no such table".into());
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
        let followed = self.followed(table, true, &order);
        let text = self.grid_edited(&followed, table, |grid| {
            // Each line of the table keeps its place and takes the cells of
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

    /// Move `count` columns starting at `first` so that the first of them
    /// becomes column `to`, in one undoable step. Data columns move among the
    /// data columns and formula columns among the formula columns. A column
    /// that a formula picks by counting, from its own (`[*-1; *]`) or by
    /// number, is still the column it was.
    pub fn move_columns(
        &mut self,
        table: usize,
        first: usize,
        count: usize,
        to: usize,
    ) -> Result<(), String> {
        let Some(snap) = self.snapshot.tables.get(table) else {
            return Err("there is no such table".into());
        };
        let n = snap.columns.len();
        if count == 0 || first + count > n || to + count > n {
            return Err("there is no such column".into());
        }
        if to == first {
            return Ok(());
        }
        // The data columns come first, in the order of the header row.
        let data = snap.columns.iter().filter(|c| c.formula.is_none()).count();
        let among_data = first + count <= data && to + count <= data;
        let among_formulas = first >= data && to >= data;
        if !among_data && !among_formulas {
            return Err("data columns stay before the formula columns".into());
        }
        let mut order: Vec<usize> = (0..n).collect();
        let block: Vec<usize> = order.drain(first..first + count).collect();
        order.splice(to..to, block);
        let names: Vec<String> = snap.columns.iter().map(|c| c.name.clone()).collect();

        let followed = self.followed(table, false, &order);
        let ast = parse_sheet(&followed, 0, &mut Vec::new());
        let Some(decl) = self.table_decl(&ast, table) else {
            return Err("there is no such table".into());
        };
        let text = if among_data {
            let mut grid = Grid::of(decl);
            let pick = |cells: &[String]| -> Vec<String> {
                order[..data]
                    .iter()
                    .map(|&k| cells.get(k).cloned().unwrap_or_default())
                    .collect()
            };
            grid.header = pick(&grid.header);
            for (_, row) in &mut grid.rows {
                *row = pick(row);
            }
            grid.write(decl, &followed)
        } else {
            // Each `Name := expression` keeps its place in the text and takes
            // the declaration that now belongs there.
            let range = |name: &str| {
                let c = decl.computed.iter().find(|c| c.name == name)?;
                Some((c.span.start as usize, c.expr_span.end as usize))
            };
            let slots: Option<Vec<(usize, usize)>> =
                names[data..].iter().map(|name| range(name)).collect();
            let Some(mut slots) = slots else {
                return Err("the formula columns cannot be moved".into());
            };
            let blocks: Option<Vec<&str>> = order[data..]
                .iter()
                .map(|&k| range(&names[k]).map(|(s, e)| &followed[s..e]))
                .collect();
            let Some(blocks) = blocks else {
                return Err("the formula columns cannot be moved".into());
            };
            slots.sort_unstable();
            let mut text = followed.clone();
            for (&(s, e), block) in slots.iter().zip(&blocks).rev() {
                text.replace_range(s..e, block);
            }
            text
        };
        self.commit(text);
        Ok(())
    }

    /// Add `Name := expression` to a table.
    pub fn add_computed(&mut self, table: usize, name: &str, expr: &str) -> Result<(), String> {
        let name = valid_name(name)?.to_string();
        let expr = expr.trim().trim_start_matches('=').trim();
        if expr.is_empty() {
            return Err("a computed column needs an expression".into());
        }
        let Some(n) = self.snapshot.tables.get(table).map(|t| t.columns.len()) else {
            return Err("there is no such table".into());
        };
        // A column counted from the end is one further from it.
        let place: Vec<usize> = (0..n).collect();
        let followed = self.followed_to(table, false, &place, n + 1);
        let ast = parse_sheet(&followed, 0, &mut Vec::new());
        let Some(decl) = self.table_decl(&ast, table) else {
            return Err("there is no such table".into());
        };
        if self.snapshot.tables[table]
            .columns
            .iter()
            .any(|c| c.name == name)
        {
            return Err(format!(
                "table `{}` already has a column `{name}`",
                decl.name
            ));
        }
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
        let mut text = followed.clone();
        text.insert_str(at, &format!("{gap}{name} := {}", expr.replace('\n', " ")));
        self.commit(text);
        Ok(())
    }

    /// Change the expression of a computed column.
    pub fn set_column_formula(&mut self, table: usize, col: usize, expr: &str) -> bool {
        let expr = expr
            .trim()
            .trim_start_matches('=')
            .trim()
            .replace('\n', " ");
        if expr.is_empty() {
            return false;
        }
        let ast = self.ast();
        let Some(decl) = self.table_decl(&ast, table) else {
            return false;
        };
        let Some(name) = self.snapshot.tables[table]
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
            || self.snapshot.tables.iter().any(|t| t.name == name);
        if taken {
            return Err(format!("the name `{name}` is already used"));
        }
        // Constants go first, before the first table.
        let ast = self.ast();
        let at = ast
            .tables
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

    /// Append a new table with one empty row.
    pub fn add_table(&mut self, name: &str) -> Result<(), String> {
        let name = valid_name(name)?.to_string();
        let taken = self.snapshot.consts.iter().any(|c| c.name == name)
            || self.snapshot.tables.iter().any(|t| t.name == name);
        if taken {
            return Err(format!("the name `{name}` is already used"));
        }
        let mut text = self.text.trim_end().to_string();
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&format!(
            "table {name}\n\nA | B | C\n  |   |\n  |   |\n  |   |\n"
        ));
        self.commit(text);
        Ok(())
    }

    fn rebuild(&mut self) {
        let mut sources = Sources::new();
        let src = sources.add("sheet", self.text.as_str());
        let (program, mut diags) = compile(&self.text, src);
        let engine = Engine::with_options(
            &program,
            Options {
                style: self.style,
                local: self.local,
                ..Options::default()
            },
        );
        engine.run();
        diags.extend(engine.take_diags());
        let ast = self.ast();

        // A cell that does not fit its column: say what the column takes,
        // written the way the user types it.
        for table in &program.tables {
            let Some(decl) = ast.tables.iter().find(|d| d.name == table.name) else {
                continue;
            };
            for (cell, col) in decl.rows.iter().flat_map(|row| row.iter().zip(&table.cols)) {
                let hint = entry_hint(&self.style, &col.ty.to_string());
                let misfits = diags
                    .iter_mut()
                    .filter(|d| d.span == cell.span && d.message.ends_with("` literal"));
                for d in misfits.filter(|_| !hint.is_empty()) {
                    d.message = format!("`{}` does not fit column `{}`", cell.text, col.name);
                    d.help = Some(format!(
                        "{hint}; to calculate a value, start the cell with `=`"
                    ));
                }
            }
        }

        let message = |d: &Diagnostic| match &d.help {
            Some(help) => format!("{} ({help})", d.message),
            None => d.message.clone(),
        };
        let within = |span: Span| diags.iter().find(|d| contains(span, d.span)).map(message);
        let show = |v: &Value| match v {
            Value::Table(_) => "<table>".to_string(),
            Value::Row(_) => "<row>".to_string(),
            other => engine.show(other, false),
        };
        // What a number shown with fewer digits than it has is in full.
        let exact = |v: &Value, quoted: bool, shown: &str| {
            let full = match v {
                Value::Ratio(_) | Value::Num(_) | Value::Complex(..) | Value::Vector(_) => {
                    format_exact(v, quoted, &self.style)
                }
                _ => return None,
            };
            (full != shown).then_some(full)
        };

        let mut snapshot = Snapshot::default();
        for (t, table) in program.tables.iter().enumerate() {
            let decl = ast.tables.iter().find(|d| d.name == table.name);
            let mut columns = Vec::new();
            let mut column_errors = Vec::new();
            for col in &table.cols {
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
                    formula: computed.map(|c| {
                        self.text[c.expr_span.start as usize..c.expr_span.end as usize].to_string()
                    }),
                });
                column_errors.push(computed.and_then(|c| within(c.expr_span)));
            }
            let mut rows = Vec::with_capacity(table.nrows);
            for r in 0..table.nrows {
                let mut row = Vec::with_capacity(table.cols.len());
                for (c, column) in columns.iter().enumerate() {
                    let value = engine.cell_shown(t, c, r);
                    let failed = matches!(value, Value::Error);
                    let src_cell = decl
                        .filter(|d| c < d.header.len())
                        .and_then(|d| d.rows.get(r))
                        .and_then(|row| row.get(c));
                    let (source, error) = match (&column.formula, src_cell) {
                        (Some(formula), _) => (
                            format!("= {formula}"),
                            failed.then(|| column_errors[c].clone()).flatten(),
                        ),
                        (None, Some(cell)) => (cell.text.clone(), within(cell.span)),
                        (None, None) => (String::new(), None),
                    };
                    let display = show(&value);
                    row.push(CellSnap {
                        exact: exact(&value, false, &display),
                        display,
                        numeric: value.is_numeric(),
                        value: value.clone(),
                        formula: source.starts_with('='),
                        error: error.or_else(|| {
                            failed.then(|| "this cell could not be calculated".to_string())
                        }),
                        source,
                    });
                }
                rows.push(row);
            }
            snapshot.tables.push(TableSnap {
                name: table.name.clone(),
                columns,
                rows,
            });
        }
        for (i, c) in program.consts.iter().enumerate() {
            let decl = ast.consts.iter().find(|d| d.name == c.name);
            let value = engine.const_value(i).unwrap_or(Value::Error);
            let display = match &value {
                Value::Empty => "empty".to_string(),
                Value::Vector(_) | Value::Text(_) => engine.show(&value, true),
                other => show(other),
            };
            snapshot.consts.push(ConstSnap {
                name: c.name.clone(),
                source: decl.map_or(String::new(), |d| {
                    self.text[d.expr_span.start as usize..d.expr_span.end as usize].to_string()
                }),
                exact: exact(&value, true, &display),
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
        snapshot.problems = diags
            .iter()
            .map(|d| {
                let (line, column) = sources.line_col(d.span.src, d.span.start);
                Problem {
                    line,
                    column,
                    message: message(d),
                }
            })
            .collect();
        self.snapshot = snapshot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALES: &str = "# sales\nconst Rate = 20%\n\ntable Sales\n\nMonth | Revenue | Cost\nJan   | 100     | 60\nFeb   | 120     | 70\n\nProfit := Revenue - Cost\nTax := Profit * Rate\n";

    fn col(doc: &Document, table: usize, col: usize) -> Vec<String> {
        doc.snapshot().tables[table]
            .rows
            .iter()
            .map(|r| r[col].display.clone())
            .collect()
    }

    #[test]
    fn snapshot_shows_values_and_sources() {
        let doc = Document::from_text(SALES);
        let t = &doc.snapshot().tables[0];
        assert_eq!(t.name, "Sales");
        assert_eq!(t.columns.len(), 5);
        assert_eq!(t.columns[3].formula.as_deref(), Some("Revenue - Cost"));
        assert_eq!(col(&doc, 0, 3), ["40", "50"]);
        assert_eq!(t.rows[0][1].source, "100");
        assert_eq!(t.rows[0][3].source, "= Revenue - Cost");
        assert!(t.rows[0][1].numeric && !t.rows[0][0].numeric);
        assert_eq!(doc.snapshot().consts[0].display, "0.2");
        assert!(doc.snapshot().problems.is_empty());
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
                "Feb   | 120     | 70\nMar   | 150     | 80\nApr   |         |\n\nProfit"
            )
        );
        doc.insert_rows(0, 0, 1);
        assert_eq!(col(&doc, 0, 0), ["", "Jan", "Feb", "Mar", "Apr"]);
        doc.delete_rows(0, 0, 2);
        assert_eq!(col(&doc, 0, 0), ["Feb", "Mar", "Apr"]);
        assert!(doc.text().contains("Month | Revenue | Cost\nFeb"));
    }

    #[test]
    fn columns_constants_and_tables() {
        let mut doc = Document::from_text(SALES);
        doc.add_column(0, "Note").unwrap();
        assert!(doc.add_column(0, "Note").is_err());
        assert!(doc.add_column(0, "bad name").is_err());
        doc.set_cell(0, 0, 3, "a | b");
        assert_eq!(doc.snapshot().tables[0].rows[0][3].display, "a | b");
        doc.add_computed(0, "Margin", "Profit / Revenue").unwrap();
        let names: Vec<_> = doc.snapshot().tables[0]
            .columns
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(
            names,
            [
                "Month", "Revenue", "Cost", "Note", "Profit", "Tax", "Margin"
            ]
        );
        assert_eq!(col(&doc, 0, 6), ["0.4", "0.41667…"]);
        let exact = |doc: &Document| doc.snapshot().tables[0].rows[1][6].exact.clone();
        assert_eq!(exact(&doc).as_deref(), Some("5/12"));
        doc.set_cell(0, 0, 6, "Profit / Cost");
        assert_eq!(col(&doc, 0, 6), ["0.66667…", "0.71429…"]);
        doc.set_const(0, "50%");
        assert_eq!(col(&doc, 0, 5), ["20", "25"]);
        doc.add_const("Extra", "Sales.Revenue.sum()").unwrap();
        assert_eq!(doc.snapshot().consts[1].display, "220");
        doc.add_table("Notes").unwrap();
        assert_eq!(doc.snapshot().tables[1].rows.len(), 3);
        assert!(
            doc.snapshot().problems.is_empty(),
            "{:?}",
            doc.snapshot().problems
        );
    }

    #[test]
    fn renaming_a_column_follows_its_references() {
        let text = format!(
            "{SALES}\ntable Top\n\nKind | Value\nSum  | = Sales.Revenue.sum()\nBig  | = Sales[Revenue; Revenue > 100].sum()\n\nRevenue := Value\n"
        );
        let mut doc = Document::from_text(&text);
        let before = col(&doc, 0, 3);
        doc.rename_column(0, 1, "Income").unwrap();
        let renamed = doc.text().to_string();
        assert!(renamed.contains("Month | Income | Cost\nJan   | 100    | 60\n"));
        assert!(renamed.contains("Profit := Income - Cost"));
        assert!(renamed.contains("= Sales.Income.sum()"));
        assert!(renamed.contains("= Sales[Income; Income > 100].sum()"));
        // Another table's own column of the same name is left alone.
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
            format!("{SALES}\ntable Top\n\nValue\n100\n\nN := Sales[; Revenue > Value].count()\n");
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
            + "table Sales\n\nMonth | Revenue | Cost\nJan   | 100     | 60\n\nProfit := Net(Revenue, Cost)\n";
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
    fn columns_can_be_deleted() {
        let text = "const Rate = 20%\n\ntable Sales\n\nCost : Int\nRevenue : Int\n\n\
                    Month | Revenue | Cost | Note\nJan   | 100     | 60   | a\nFeb   | 120     | 70   | = [*-1; *]\n\n\
                    Profit := Revenue - Cost\nTax := Profit * Rate\n";
        let mut doc = Document::from_text(text);
        let names = |doc: &Document| -> Vec<String> {
            let columns = &doc.snapshot().tables[0].columns;
            columns.iter().map(|c| c.name.clone()).collect()
        };
        // A data column goes with its cells and its type line; the formula
        // that counts columns still reads the one to its left.
        doc.delete_columns(0, 0, 1).unwrap();
        assert_eq!(names(&doc), ["Revenue", "Cost", "Note", "Profit", "Tax"]);
        assert!(
            doc.text().contains(
                "Revenue | Cost | Note\n100     | 60   | a\n120     | 70   | = [*-1; *]\n"
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

        // Not both kinds at once, not every data column, not off the table.
        assert!(
            doc.delete_columns(0, 3, 2)
                .unwrap_err()
                .contains("separately")
        );
        assert!(
            doc.delete_columns(0, 0, 4)
                .unwrap_err()
                .contains("at least one")
        );
        assert!(doc.delete_columns(0, 5, 2).is_err());
        assert_eq!(doc.text(), text);
        // Down to one column, an empty cell is still a row.
        let mut two = Document::from_text("table T\n\nA | B\n  | 1\nx | 2\n");
        two.delete_columns(0, 1, 1).unwrap();
        assert_eq!(two.text(), "table T\n\nA\n\"\"\nx\n");
        assert_eq!(two.snapshot().tables[0].rows.len(), 2);
    }

    #[test]
    fn errors_do_not_blank_the_grid() {
        let mut doc = Document::from_text(SALES);
        doc.set_cell(0, 0, 1, "= Revnue + 1");
        let t = &doc.snapshot().tables[0];
        assert_eq!(t.rows[0][1].display, "#ERROR");
        assert!(t.rows[0][1].error.as_ref().unwrap().contains("Revnue"));
        assert_eq!(t.rows[1][3].display, "50");
        assert_eq!(doc.snapshot().problems.len(), 1);
        assert_eq!(doc.snapshot().problems[0].line, 7);
    }

    #[test]
    fn dates_follow_the_style_but_the_text_stays_iso() {
        use omasheet_omx::date::Order;
        let text = "table Log\n\nDay : Date\nAt : Time\nSeen : DateTime\n\n\
                    Day | At | Seen | Note\n2026-10-08 | 17:47 | 2026-10-08T17:47 | a\n";
        let mut doc = Document::from_text(text);
        let row = |doc: &Document, r: usize| -> Vec<String> {
            let cells = &doc.snapshot().tables[0].rows[r];
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
        assert_eq!(doc.snapshot().tables[0].rows[0][0].source, "2026-10-08");
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
        let mut loose = Document::from_text("table T\n\nA | B\n  |\n  |\n");
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
        assert!(loose.text().contains("= 1/2"));
        assert!(loose.text().contains("10.11.12"));
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
            entry_hint(&us, "DateTime"),
            "DateTime: e.g. 01/31/2025 5:05 PM or ISO"
        );
        assert_eq!(
            entry_hint(&Style::ISO, "DateTime"),
            "DateTime: e.g. 2025-01-31T17:05"
        );
        assert_eq!(entry_hint(&us, ""), "");
        // A cell that does not fit says what would.
        let mut doc = Document::with_style("table T\n\nOn : Date\n\nOn\n2025-01-31\n", us);
        doc.set_cell(0, 0, 0, "soon");
        assert_eq!(
            doc.snapshot().tables[0].rows[0][0].error.as_deref(),
            Some(
                "`soon` does not fit column `On` (Date: MM/DD/YYYY, e.g. 01/31/2025 or ISO; \
                 to calculate a value, start the cell with `=`)"
            )
        );
        assert!(
            doc.snapshot().problems[0]
                .message
                .contains("does not fit column `On`")
        );
        // Every example is accepted as an entry for its type.
        for ty in ["Date", "Time", "DateTime"] {
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
    fn a_sheet_in_another_zone_is_shown_on_local_clocks() {
        // A zone whose clocks this machine does not keep, wherever it is.
        let zone = match crate::zone::Zone::system().from_utc(0) {
            Some(32_400) => "America/New_York",
            _ => "Asia/Tokyo",
        };
        let text = format!("zone {zone}\n\ntable T\n\nAt\n2025-07-15T12:00\n");
        // As the sheet writes it, for tools.
        let plain = Document::from_text(&text);
        assert_eq!(
            plain.snapshot().tables[0].rows[0][0].display,
            "2025-07-15T12:00"
        );
        // For a person, on this machine's clocks; the source is untouched.
        let mut doc = Document::with_style(&text, Style::ISO);
        let cell = doc.snapshot().tables[0].rows[0][0].clone();
        assert_eq!(cell.source, "2025-07-15T12:00");
        assert_ne!(cell.display, "2025-07-15T12:00");
        // What is typed is in the sheet's zone, as the source shows it.
        doc.set_cell(0, 0, 0, "2025-07-15T13:00");
        assert!(doc.text().contains("\n2025-07-15T13:00\n"));

        let bad = Document::from_text("zone Mars/Base\n\ntable T\n\nA\n1\n");
        assert!(bad.snapshot().problems[0].message.contains("no time zone"));
        assert_eq!(bad.snapshot().tables[0].rows[0][0].display, "1");
    }

    #[test]
    fn a_block_of_cells_is_summed_exactly() {
        let text = "table T\n\nName | A | B\nx | 1 | 0.5\ny | 2 |\nz | = 1/3 | 1e2\n";
        let doc = Document::from_text(text);
        let sum = |from, to| doc.summary(0, from, to).unwrap();
        let numbers = |s: &str, a: &str| Some((s.to_string(), a.to_string()));
        // Exact numbers add up exactly, and are shown as cells are.
        let a = sum((0, 1), (2, 1));
        assert_eq!((a.count, a.numbers), (3, numbers("3.33333…", "1.11111…")));
        // Text counts but does not add; an empty cell does neither.
        let top = sum((0, 0), (1, 2));
        assert_eq!((top.count, top.numbers), (5, numbers("3.5", "1.16667…")));
        // One Num makes the total a Num.
        let b = sum((0, 2), (2, 2));
        assert_eq!((b.count, b.numbers), (2, numbers("100.5", "50.25")));
        let names = sum((0, 0), (2, 0));
        assert_eq!((names.count, names.numbers), (3, None));
        // A block that runs off the table is cut to it.
        assert_eq!(sum((2, 1), (9, 9)).count, 2);
        assert!(doc.summary(5, (0, 0), (1, 1)).is_none());
    }

    #[test]
    fn columns_can_be_moved() {
        let mut doc = Document::from_text(SALES);
        let names = |doc: &Document| -> Vec<String> {
            let columns = &doc.snapshot().tables[0].columns;
            columns.iter().map(|c| c.name.clone()).collect()
        };
        assert_eq!(names(&doc), ["Month", "Revenue", "Cost", "Profit", "Tax"]);

        // A data column, with its cells, and the table lined up again.
        doc.move_columns(0, 0, 1, 2).unwrap();
        assert_eq!(names(&doc), ["Revenue", "Cost", "Month", "Profit", "Tax"]);
        assert!(
            doc.text()
                .contains("Revenue | Cost | Month\n100     | 60   | Jan\n120     | 70   | Feb\n")
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
            doc.text()
                .contains("\nTax := Profit * Rate\nProfit := Revenue - Cost\n")
        );
        assert_eq!(col(&doc, 0, 3), ["8", "10"]);
        assert!(doc.snapshot().problems.is_empty());

        // Not across the two kinds, and not off the table.
        let before = doc.text().to_string();
        assert!(
            doc.move_columns(0, 0, 1, 3)
                .unwrap_err()
                .contains("stay before the formula columns")
        );
        assert!(doc.move_columns(0, 3, 1, 1).is_err());
        assert!(doc.move_columns(0, 1, 2, 2).is_err());
        assert!(doc.move_columns(0, 9, 1, 0).is_err());
        assert!(doc.move_columns(0, 1, 1, 1).is_ok());
        assert_eq!(doc.text(), before);

        // A formula that counts columns from its own still reads the same
        // ones, wherever it and they end up.
        let mut doc =
            Document::from_text("table T\n\nA | B | C\n1 | 2 | = [*-1; *] * 10 + [*-2; *]\n");
        assert_eq!(col(&doc, 0, 2), ["21"]);
        doc.move_columns(0, 0, 1, 1).unwrap();
        assert!(
            doc.text()
                .contains("B | A | C\n2 | 1 | = [*-2; *] * 10 + [*-1; *]\n")
        );
        assert_eq!(col(&doc, 0, 2), ["21"]);
        doc.move_columns(0, 2, 1, 0).unwrap();
        assert!(
            doc.text()
                .contains("\n= [*+1; *] * 10 + [*+2; *] | 2 | 1\n")
        );
        assert_eq!(col(&doc, 0, 0), ["21"]);
        assert!(doc.snapshot().problems.is_empty());

        // So does one that picks a column by its number, wherever it is; a
        // range follows its two ends.
        let mut doc = Document::from_text(
            "const K = T[1; 0]\n\ntable T\n\nA | B | C | D\n1 | 2 | 4 | = [1; *] * 10 + [-3; *] + [0..1; *].A\n\n\
             E := [0; *] + T[2; 0]\n",
        );
        assert_eq!(col(&doc, 0, 3), ["25"]);
        assert_eq!(col(&doc, 0, 4), ["5"]);
        doc.move_columns(0, 0, 1, 2).unwrap();
        assert_eq!(names(&doc), ["B", "C", "A", "D", "E"]);
        assert!(doc.text().contains("const K = T[0; 0]\n"));
        assert!(
            doc.text()
                .contains("| = [0; *] * 10 + [-4; *] + [0..2; *].A\n")
        );
        assert!(doc.text().contains("E := [2; *] + T[1; 0]\n"));
        assert_eq!(col(&doc, 0, 3), ["25"]);
        assert_eq!(col(&doc, 0, 4), ["5"]);
        assert!(doc.snapshot().problems.is_empty());
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
        // A plain `*` is the cell's own, and a range ending on one goes as it is.
        assert_eq!(
            moved("= [*; *-1]", (2, 3), (2..3, 3..4), (0, -2)),
            "= [*; *-1]"
        );
        assert_eq!(
            moved("= sum([B; *-2..*])", (2, 3), (2..3, 3..4), (2, 0)),
            "= sum([B; *-2..*])"
        );
        assert_eq!(
            moved("= sum([B; *-2..*-1])", (2, 3), (2..3, 3..4), (2, 0)),
            "= sum([B; *-4..*-3])"
        );
        // Numbers and names say where they mean already.
        assert_eq!(
            moved("= [1; 0] + [B; *] + B", (2, 3), (2..3, 3..4), (2, -2)),
            "= [1; 0] + [B; *] + B"
        );
        // Cells cut together stay as far apart; a number or a name follows.
        assert_eq!(
            moved(
                "= [*-1; *] + [*-3; *] + [C; 2]",
                (2, 3),
                (2..3, 2..4),
                (1, -2)
            ),
            "= [*-1; *] + [*-1; *] + [A; 3]"
        );
        assert_eq!(moved("*-1", (2, 3), (2..3, 3..4), (0, -2)), "*-1");
    }

    #[test]
    fn formulas_follow_a_cut_cell() {
        let text = "const K = T[C; 1]\n\ntable T\n\nA | B | C | D\n  |   | 1 | = [*-1; *] + C\n  |   | 5 | = [*-1; *] * 10 + [2; *-1] + T[C; *]\n  |   |   | = sum(T.C)\n\n\
                    E := C * 2\n\ntable U\n\nX\n= T[2; 1] + T[; 1].C + T.C[1]\n";
        let mut doc = Document::from_text(text);
        assert_eq!(col(&doc, 0, 3), ["2", "56", "6"]);
        // The 5 goes from C to A and down a row, as a cut and a paste do it.
        doc.set_cell(0, 1, 2, "");
        doc.paste_cut(0, (1, 2), (2, 0), &[vec!["5".to_string()]]);
        let now = doc.text();
        assert!(now.contains("const K = T[A; 2]\n"), "{now}");
        assert!(
            now.contains("| = [*-3; *+1] * 10 + [2; *-1] + T[A; *+1]\n"),
            "{now}"
        );
        assert!(now.contains("= T[0; 2] + T[; 2].A + T.A[2]\n"), "{now}");
        // A formula column and a whole column read the place.
        assert!(now.contains("E := C * 2\n"), "{now}");
        assert!(now.contains("= sum(T.C)\n"), "{now}");
        assert_eq!(col(&doc, 0, 3), ["2", "56", "1"]);
        assert_eq!(col(&doc, 1, 0), ["15"]);
        assert!(doc.snapshot().problems.is_empty());

        // A column named on its own follows too.
        let mut doc = Document::from_text("table T\n\nA | B | C\n  | 5 | = B * 2\n  |   |\n");
        doc.set_cell(0, 0, 1, "");
        doc.paste_cut(0, (0, 1), (1, 0), &[vec!["5".to_string()]]);
        assert!(doc.text().contains("| = [A; *+1] * 2\n"), "{}", doc.text());
        assert_eq!(col(&doc, 0, 2), ["10", ""]);

        // Put down on the cell it reads, a formula is an error, not refused.
        let mut doc = Document::from_text("table T\n\nA | B\n1 | = [*-1; *]\n");
        doc.set_cell(0, 0, 1, "");
        assert!(doc.paste_cut(0, (0, 1), (0, 0), &[vec!["= [*-1; *]".to_string()]]));
        assert!(doc.text().contains("\n= [*; *] |\n"), "{}", doc.text());
        assert!(!doc.snapshot().problems.is_empty());
    }

    #[test]
    fn moved_rows_keep_a_range_that_starts_after() {
        let text = "table T\n\nName | N | M\na | 1 |\nb | 2 |\nc | 3 |\n\
                    d | 4 | = sum([N; 0^..2]) + sum([N; ^2])\n";
        let mut doc = Document::from_text(text);
        assert_eq!(col(&doc, 0, 2), ["", "", "", "8"]);
        // `b` to the bottom: the range still starts after `a`, and the
        // first two are whichever rows are first.
        doc.move_rows(0, 1, 1, 3).unwrap();
        let now = doc.text();
        assert!(
            now.contains("| = sum([N; 0^..3]) + sum([N; ^2])\n"),
            "{now}"
        );
        // `a` to the bottom: there is no row left to start after.
        doc.move_rows(0, 0, 1, 3).unwrap();
        let now = doc.text();
        assert!(now.contains("| = sum([N; 0..2]) + sum([N; ^2])\n"), "{now}");
        assert!(doc.snapshot().problems.is_empty());
    }

    #[test]
    fn moved_rows_keep_what_reads_them() {
        let text = "const First = T[N; 0]\n\ntable T\n\nName | N | M\na | 1 |\nb | 2 |\nc | 3 | = [N; *-1] + 10\n\
                    d | 4 | = [N; 0] + sum([N; 0..1]) + [N; -3]\n\nSum := N + ([N; *-1] // 0)\n";
        let mut doc = Document::from_text(text);
        assert_eq!(col(&doc, 0, 2), ["", "", "12", "6"]);
        assert_eq!(col(&doc, 0, 3), ["1", "3", "5", "7"]);
        // `b` to the bottom: what read it still does, and the range follows
        // its ends, taking in the rows now between them.
        doc.move_rows(0, 1, 1, 3).unwrap();
        assert_eq!(col(&doc, 0, 0), ["a", "c", "d", "b"]);
        let now = doc.text();
        assert!(now.contains("| = [N; *+2] + 10\n"), "{now}");
        assert!(
            now.contains("| = [N; 0] + sum([N; 0..3]) + [N; -1]\n"),
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
            doc.text()
                .contains("| = [N; 1] + sum([N; 1..3]) + [N; -1]\n"),
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
            let source = doc.snapshot().tables[0].rows[from.0][from.1].source.clone();
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
        let text = "const K = T[1; 0] + T[-1; 0]\n\ntable T\n\nA | B | C\n1 | 2 | = [*-1; *] * 10 + [0; *] + [-3; *]\n3 | 4 | = [*-2; *]\n\n\
                    S := A + [1; *]\n";
        let names = |doc: &Document| -> Vec<String> {
            let columns = &doc.snapshot().tables[0].columns;
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
            now.contains("| = [*-1; *] * 10 + [0; *] + [-3; *]\n"),
            "{now}"
        );
        assert!(now.contains("| = [*-3; *]\n"), "{now}");
        assert!(now.contains("S := A + [2; *]\n"), "{now}");
        assert_eq!(col(&doc, 0, 3), ["23", "3"]);
        assert_eq!(col(&doc, 0, 4), ["3", "7"]);
        // One step to undo.
        assert!(doc.undo());
        assert_eq!(doc.text(), text);
        assert!(!doc.can_undo());

        // A formula column, before the others or among them.
        doc.insert_computed(0, 0, "D", "A * 2").unwrap();
        assert_eq!(names(&doc), ["A", "B", "C", "D", "S"]);
        assert!(doc.text().contains("const K = T[1; 0] + T[-1; 0]\n"));
        assert_eq!(col(&doc, 0, 3), ["2", "6"]);
        assert!(doc.undo());

        // Copies: a data column with its cells as written, and a formula
        // column with its formula, each among its own kind.
        doc.insert_copies(0, &[2, 3], 0).unwrap();
        assert_eq!(names(&doc), ["C_copy001", "A", "B", "C", "S_copy001", "S"]);
        let now = doc.text();
        assert!(
            now.contains("S_copy001 := A + [2; *]\nS := A + [2; *]\n"),
            "{now}"
        );
        assert_eq!(col(&doc, 0, 4), ["3", "7"]);
        // The copy counts from where it is, so it reads other cells.
        assert_eq!(col(&doc, 0, 3), ["23", "3"]);
        assert!(
            doc.snapshot().tables[0].rows[1][0]
                .source
                .contains("[*-2; *]")
        );
        doc.insert_copies(0, &[3], 4).unwrap();
        assert_eq!(names(&doc)[3..5], ["C", "C_copy002"]);
        assert!(doc.undo());
        assert!(doc.undo());
        assert_eq!(doc.text(), text);

        // Copied cells, put in as new rows.
        doc.insert_cells(0, (1, 0), &[vec!["8".into(), "9".into()]]);
        assert_eq!(col(&doc, 0, 0), ["1", "8", "3"]);
        assert_eq!(col(&doc, 0, 1), ["2", "9", "4"]);
        assert!(doc.undo());
        assert_eq!(doc.text(), text);
    }

    #[test]
    fn rows_can_be_moved() {
        let text = "table T\n\nName | N\na | 1\nb | 2\nc | = [N; *-1] + 10\nd | 4\n\n\
                    Sum := N + ([N; *-1] // 0)\n";
        let mut doc = Document::from_text(text);
        assert_eq!(col(&doc, 0, 2), ["1", "3", "14", "16"]);
        // One row down, with its formula; the running column follows.
        doc.move_rows(0, 0, 1, 2).unwrap();
        assert_eq!(col(&doc, 0, 0), ["b", "c", "a", "d"]);
        assert!(
            doc.text()
                .contains("Name | N\nb    | 2\nc    | = [N; *-1] + 10\na    | 1\nd    | 4\n")
        );
        assert_eq!(col(&doc, 0, 1), ["2", "12", "1", "4"]);
        assert_eq!(col(&doc, 0, 2), ["2", "14", "13", "5"]);
        // Two at once, to the top; and one step to undo.
        doc.move_rows(0, 2, 2, 0).unwrap();
        assert_eq!(col(&doc, 0, 0), ["a", "d", "b", "c"]);
        assert!(doc.undo());
        assert_eq!(col(&doc, 0, 0), ["b", "c", "a", "d"]);
        // Not off the table.
        let before = doc.text().to_string();
        assert!(doc.move_rows(0, 3, 2, 0).is_err());
        assert!(doc.move_rows(0, 0, 2, 3).is_err());
        assert!(doc.move_rows(0, 1, 1, 1).is_ok());
        assert_eq!(doc.text(), before);
    }

    #[test]
    fn blank_document_is_valid() {
        let mut doc = Document::default();
        assert!(doc.snapshot().problems.is_empty());
        assert_eq!(doc.snapshot().tables[0].rows.len(), 5);
        doc.set_cell(0, 0, 0, "1");
        doc.set_cell(0, 0, 1, "= A + 1");
        assert_eq!(doc.snapshot().tables[0].rows[0][1].display, "2");
    }
}

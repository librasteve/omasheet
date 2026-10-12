// Copyright (c) 2026 Stephen Roe

//! Omasheet at its edges: XLSX, CSV and JSON.
//!
//! An import turns a file into `.omx` source, which is then a file like any
//! other. An export writes the calculated sheets of a file. Neither is a way
//! to save a file: each says, as notices, what it could not carry across.

pub mod csv;
pub mod formula;
pub mod json;
pub mod xlsx;

use omasheet_engine::omx::types::S;
use omasheet_engine::omx::{Program, Sources, compile};
use omasheet_engine::{Engine, Options, Value};

/// A file as `.omx` source, and what was changed or left behind on the way.
#[derive(Debug, Default)]
pub struct Imported {
    pub source: String,
    pub notices: Vec<String>,
}

/// One file an export wrote.
#[derive(Debug)]
pub struct Output {
    /// The sheet this file holds, when it holds one of several.
    pub sheet: Option<String>,
    pub bytes: Vec<u8>,
}

/// The files of an export, and what they do not hold exactly.
#[derive(Debug, Default)]
pub struct Exported {
    pub files: Vec<Output>,
    pub notices: Vec<String>,
}

/// One cell on its way into a file.
#[derive(Clone, Debug, PartialEq)]
pub enum Datum {
    Empty,
    /// Text that a cell reads as a value of this type: `19.99`, `2025-01-31`.
    Lit(String, S),
    Text(String),
    /// An OMX expression for the cell to work out.
    Formula(String),
}

impl Datum {
    /// What a cell reads `text` as: a number, a date, a time, `true` or
    /// `false`; anything else is text, and so is a quoted string or a
    /// formula, which stay as they are written.
    pub fn read(text: &str) -> Datum {
        use omasheet_engine::omx::ast::Lit;
        use omasheet_engine::omx::convert::{lit_type, parse_literal};
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Datum::Empty;
        }
        match parse_literal(trimmed) {
            Some(Lit::Text(_)) | None => Datum::Text(text.to_string()),
            Some(lit) => Datum::Lit(trimmed.to_string(), lit_type(&lit)),
        }
    }

    fn label(&self) -> &str {
        match self {
            Datum::Empty | Datum::Formula(_) => "",
            Datum::Lit(s, _) | Datum::Text(s) => s,
        }
    }
}

/// A sheet on its way into a file: a name, a header row and rows of cells.
/// The names are as the file gives them, not yet identifiers.
#[derive(Debug, Default)]
pub struct Grid {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Datum>>,
    /// Keep every row, so that a row stays where a formula looks for it.
    pub fixed: bool,
}

impl Grid {
    /// From rows of cells of which the first is the header.
    pub fn from_rows(name: &str, mut rows: Vec<Vec<Datum>>) -> Grid {
        let headers = if rows.is_empty() {
            Vec::new()
        } else {
            rows.remove(0)
                .iter()
                .map(|d| d.label().trim().to_string())
                .collect()
        };
        Grid {
            name: name.to_string(),
            headers,
            rows,
            fixed: false,
        }
    }
}

const RESERVED: &[&str] = &[
    "if", "then", "else", "and", "or", "not", "in", "true", "false", "Inf", "NaN", "sheet",
    "const", "fn", "zone",
];

/// `raw` as an OMX identifier: the letters, digits and `_` of it, with a
/// capital after each gap. Empty if it has none of them.
fn identifier(raw: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            let c = if gap && !out.is_empty() {
                c.to_ascii_uppercase()
            } else {
                c
            };
            out.push(c);
            gap = false;
        } else {
            gap = true;
        }
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if RESERVED.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

/// A name for `raw` that is an identifier and not one of `taken`, to which
/// it is added. `fallback` is used when `raw` has nothing to make a name of.
fn name_for(
    raw: &str,
    fallback: &str,
    taken: &mut Vec<String>,
    notices: &mut Vec<String>,
) -> String {
    let base = match identifier(raw) {
        empty if empty.is_empty() => fallback.to_string(),
        name => name,
    };
    let mut name = base.clone();
    let mut n = 2;
    while taken.contains(&name) {
        name = format!("{base}_{n}");
        n += 1;
    }
    if raw.trim().is_empty() {
        notices.push(format!("a column with no name was named `{name}`"));
    } else if name != raw {
        notices.push(format!("`{raw}` was renamed `{name}`"));
    }
    taken.push(name.clone());
    name
}

/// Text as a cell holds it: bare where a cell would read it back as the
/// same text, and otherwise in quotes. Bare text is Markdown, and what is
/// imported is not, so text that Markdown would read as markup is in quotes
/// too.
fn text_cell(text: &str) -> String {
    use omasheet_engine::omx::convert::parse_literal;
    let bare = Datum::read(text) == Datum::Text(text.to_string())
        && parse_literal(text.trim()).is_none()
        && text == text.trim()
        && omasheet_md::is_plain(text)
        && !text.starts_with(['=', '#', '*'])
        && !text.contains(['|', '"', '\n', '\r', '\t'])
        && !text.contains(":=")
        && !text.chars().all(|c| matches!(c, '-' | ':' | ' '))
        && !RESERVED[11..]
            .iter()
            .any(|k| text.strip_prefix(k).is_some_and(|r| r.starts_with(' ')));
    if bare {
        return text.to_string();
    }
    let mut out = String::from('"');
    for c in text.replace("\r\n", "\n").chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The type a column is declared to have, from the types of its cells:
/// `None` to leave it to the file, which reads exact numbers and text for
/// itself. Also whether the cells are of types that do not go together,
/// such as numbers and text: such a column is not declared, and each cell
/// is what it reads as.
fn column_type(cells: &[&Datum]) -> (Option<S>, bool) {
    let mut seen: Vec<S> = Vec::new();
    for cell in cells {
        let ty = match cell {
            Datum::Empty => continue,
            // What a formula gives is for the file to say.
            Datum::Formula(_) => return (None, false),
            Datum::Lit(_, ty) => *ty,
            Datum::Text(_) => S::Text,
        };
        if !seen.contains(&ty) {
            seen.push(ty);
        }
    }
    let exact = |s: &S| s.is_exact();
    match seen.as_slice() {
        [] | [S::Text] => (None, false),
        all if all.iter().all(exact) => (None, false),
        [one] => (Some(*one), false),
        all if all.iter().all(|s| exact(s) || *s == S::Num) => (Some(S::Num), false),
        _ => (None, true),
    }
}

/// The names a sheet and its columns are given.
#[derive(Clone, Debug)]
pub(crate) struct Names {
    pub sheet: String,
    pub columns: Vec<String>,
}

/// How many columns a grid has.
fn width(grid: &Grid) -> usize {
    grid.rows
        .iter()
        .map(Vec::len)
        .chain([grid.headers.len()])
        .max()
        .unwrap_or(0)
}

/// Identifiers for each grid and its columns, saying what was renamed.
pub(crate) fn names(grids: &[Grid], notices: &mut Vec<String>) -> Vec<Names> {
    let mut sheets = Vec::new();
    grids
        .iter()
        .enumerate()
        .map(|(g, grid)| {
            let sheet = name_for(&grid.name, &format!("Sheet{}", g + 1), &mut sheets, notices);
            let mut taken = Vec::new();
            let columns = (0..width(grid))
                .map(|c| {
                    let raw = grid.headers.get(c).map_or("", String::as_str);
                    name_for(raw, &format!("Column{}", c + 1), &mut taken, notices)
                })
                .collect();
            Names { sheet, columns }
        })
        .collect()
}

/// Where a formula was written: the bytes of the source it takes up.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Place {
    pub grid: usize,
    pub column: usize,
    /// `None` for the formula of a whole column.
    pub row: Option<usize>,
    pub start: usize,
    pub end: usize,
}

/// Write sheets as `.omx` source.
pub fn write_omx(grids: Vec<Grid>, notices: &mut Vec<String>) -> String {
    let names = names(&grids, notices);
    write_named(&grids, &names, notices).0
}

/// The formula every row of a column has, when they all have the same one.
fn column_formula(rows: &[Vec<Datum>], c: usize) -> Option<&str> {
    let Some(Datum::Formula(first)) = rows.first().map(|row| &row[c]) else {
        return None;
    };
    let same = |row: &Vec<Datum>| matches!(&row[c], Datum::Formula(f) if f == first);
    (rows.len() > 1 && rows.iter().all(same)).then_some(first.as_str())
}

/// Write sheets as `.omx` source under the names given, and say where each
/// formula went.
pub(crate) fn write_named(
    grids: &[Grid],
    names: &[Names],
    notices: &mut Vec<String>,
) -> (String, Vec<Place>) {
    let mut out = String::new();
    let mut places = Vec::new();
    for (g, grid) in grids.iter().enumerate() {
        let sheet = &names[g].sheet;
        let columns = &names[g].columns;
        let width = width(grid);
        if width == 0 {
            continue;
        }
        let cell = |row: &'_ Vec<Datum>, c: usize| row.get(c).cloned().unwrap_or(Datum::Empty);
        let mut rows: Vec<Vec<Datum>> = grid
            .rows
            .iter()
            .map(|row| (0..width).map(|c| cell(row, c)).collect())
            .collect();
        // A line with nothing on it is not a row.
        if width == 1 && !grid.fixed {
            let before = rows.len();
            rows.retain(|row| row[0] != Datum::Empty);
            if rows.len() < before {
                notices.push(format!("`{sheet}`: empty rows were left out"));
            }
        }

        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("sheet {sheet}\n\n"));
        let types: Vec<(Option<S>, bool)> = (0..width)
            .map(|c| column_type(&rows.iter().map(|row| &row[c]).collect::<Vec<_>>()))
            .collect();
        let declared: Vec<(&str, S)> = (0..width)
            .filter_map(|c| Some((columns[c].as_str(), types[c].0?)))
            .collect();
        let longest = declared
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0);
        for (name, ty) in &declared {
            out.push_str(&format!("{name:longest$} : {ty}\n"));
        }
        if !declared.is_empty() {
            out.push('\n');
        }
        for c in (0..width).filter(|c| types[*c].1) {
            notices.push(format!(
                "`{sheet}.{}` holds values of more than one type, and has no type of its own",
                columns[c]
            ));
        }

        let computed: Vec<Option<&str>> = (0..width).map(|c| column_formula(&rows, c)).collect();
        let mut lines: Vec<Vec<String>> = vec![columns.clone()];
        for row in &rows {
            lines.push(
                row.iter()
                    .enumerate()
                    .map(|(c, d)| match d {
                        _ if computed[c].is_some() => "*".to_string(),
                        Datum::Empty => String::new(),
                        Datum::Lit(text, _) => text.clone(),
                        Datum::Text(text) => text_cell(text),
                        Datum::Formula(f) => format!("= {f}"),
                    })
                    .collect(),
            );
        }
        let widths: Vec<usize> = (0..width)
            .map(|c| {
                lines
                    .iter()
                    .map(|l| l[c].chars().count())
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        for (l, line) in lines.iter().enumerate() {
            let mut text = String::new();
            for (c, cell) in line.iter().enumerate() {
                if c > 0 {
                    text.push_str(" | ");
                }
                if l > 0 && computed[c].is_none() && matches!(rows[l - 1][c], Datum::Formula(_)) {
                    places.push(Place {
                        grid: g,
                        column: c,
                        row: Some(l - 1),
                        start: out.len() + text.len(),
                        end: out.len() + text.len() + cell.len(),
                    });
                }
                text.push_str(cell);
                text.push_str(&" ".repeat(widths[c] - cell.chars().count()));
            }
            out.push_str(text.trim_end());
            out.push('\n');
        }
        let longest = (0..width)
            .filter(|c| computed[*c].is_some())
            .map(|c| columns[c].len())
            .max()
            .unwrap_or(0);
        for c in 0..width {
            let Some(formula) = computed[c] else { continue };
            if !places.iter().any(|p| p.grid == g && p.row.is_none()) {
                out.push('\n');
            }
            let line = format!("{:longest$} := {formula}\n", columns[c]);
            places.push(Place {
                grid: g,
                column: c,
                row: None,
                start: out.len(),
                end: out.len() + line.len(),
            });
            out.push_str(&line);
        }
    }
    (out, places)
}

/// A calculated file, for an export to read.
pub(crate) struct File<'a> {
    pub program: &'a Program,
    pub engine: &'a Engine<'a>,
    /// The sheets to export, by index.
    pub sheets: Vec<usize>,
}

impl File<'_> {
    pub fn name(&self, t: usize) -> &str {
        &self.program.sheets[t].name
    }

    /// `Sheet.Column`, for a notice.
    pub fn column(&self, t: usize, c: usize) -> String {
        let sheet = &self.program.sheets[t];
        format!("{}.{}", sheet.name, sheet.cols[c].name)
    }
}

/// Things an export could not write exactly, each with the columns it
/// happened in, in the order they were met.
#[derive(Default)]
pub(crate) struct Losses(Vec<(&'static str, Vec<String>)>);

impl Losses {
    pub fn add(&mut self, what: &'static str, column: String) {
        let at = match self.0.iter().position(|(w, _)| *w == what) {
            Some(at) => at,
            None => {
                self.0.push((what, Vec::new()));
                self.0.len() - 1
            }
        };
        if !self.0[at].1.contains(&column) {
            self.0[at].1.push(column);
        }
    }

    pub fn notices(self) -> Vec<String> {
        self.0
            .into_iter()
            .map(|(what, columns)| format!("{what}: {}", columns.join(", ")))
            .collect()
    }
}

pub(crate) const FAILED: &str = "cells that could not be calculated were left empty";

/// Compile and calculate a file, and hand it to `write`. A file that does
/// not compile is not exported: the error is its diagnostics.
pub(crate) fn with_file<T>(
    name: &str,
    text: &str,
    sheet: Option<&str>,
    options: Options,
    write: impl FnOnce(&File) -> Result<T, String>,
) -> Result<T, Vec<String>> {
    let mut sources = Sources::new();
    let src = sources.add(name, text);
    let (program, diags) = compile(text, src);
    if !diags.is_empty() {
        return Err(diags.iter().map(|d| sources.render(d)).collect());
    }
    let engine = Engine::with_options(&program, options);
    let zone = engine.take_diags();
    if !zone.is_empty() {
        return Err(zone.iter().map(|d| sources.render(d)).collect());
    }
    engine.run();
    let sheets = match sheet {
        None => (0..program.sheets.len()).collect(),
        Some(wanted) => match program.sheets.iter().position(|t| t.name == wanted) {
            Some(t) => vec![t],
            None => {
                let names: Vec<&str> = program.sheets.iter().map(|t| t.name.as_str()).collect();
                return Err(vec![format!(
                    "omasheet: {name} has no sheet `{wanted}`; its sheets are {}\n",
                    names.join(", ")
                )]);
            }
        },
    };
    if sheets.is_empty() {
        return Err(vec![format!("omasheet: {name} has no sheets to export\n")]);
    }
    let file = File {
        program: &program,
        engine: &engine,
        sheets,
    };
    write(&file).map_err(|e| vec![format!("omasheet: {e}\n")])
}

/// Whether a value is one a cell can hold and an export can write.
pub(crate) fn is_failure(v: &Value) -> bool {
    matches!(
        v,
        Value::Error | Value::Vector(_) | Value::Sheet(_) | Value::Row(_) | Value::Range { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_identifiers() {
        assert_eq!(identifier("Unit Price"), "UnitPrice");
        assert_eq!(identifier("unit price (net)"), "unitPriceNet");
        assert_eq!(identifier("2024"), "_2024");
        assert_eq!(identifier("in"), "in_");
        assert_eq!(identifier("%"), "");
    }

    #[test]
    fn quotes_text_a_cell_would_read_otherwise() {
        assert_eq!(text_cell("Jan"), "Jan");
        assert_eq!(text_cell("N/A"), "N/A");
        assert_eq!(text_cell("42"), "\"42\"");
        // A time has two digits each side of the `:`; anything else is text.
        assert_eq!(Datum::read("9:30"), Datum::Text("9:30".to_string()));
        assert_eq!(text_cell("9:30"), "9:30");
        assert_eq!(text_cell("09:30"), "\"09:30\"");
        assert_eq!(text_cell("=SUM(A1:A3)"), "\"=SUM(A1:A3)\"");
        assert_eq!(text_cell("a|b"), "\"a|b\"");
        assert_eq!(text_cell(" padded"), "\" padded\"");
        assert_eq!(text_cell("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(text_cell("sheet Sales"), "\"sheet Sales\"");
        assert_eq!(text_cell("---"), "\"---\"");
        // What is imported is not Markdown: markup is kept as it is.
        assert_eq!(text_cell("so *very* good"), "\"so *very* good\"");
        assert_eq!(text_cell("[a](b.md)"), "\"[a](b.md)\"");
        assert_eq!(text_cell("a `b` c"), "\"a `b` c\"");
        assert_eq!(text_cell("<b>x</b>"), "\"<b>x</b>\"");
        assert_eq!(text_cell("AT&amp;T"), "\"AT&amp;T\"");
        assert_eq!(
            text_cell("snake_case, 2 * 3, Ada & Co"),
            "snake_case, 2 * 3, Ada & Co"
        );
    }
}

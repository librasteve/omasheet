// Copyright (c) 2026 Stephen Roe

//! Omasheet at its edges: XLSX and CSV.
//!
//! An import turns a file into `.omx` source, which is then a sheet like any
//! other. An export writes the calculated tables of a sheet. Neither is a way
//! to save a sheet: each says, as notices, what it could not carry across.

pub mod csv;
pub mod xlsx;

use omasheet_engine::omx::types::S;
use omasheet_engine::omx::{Program, Sources, compile};
use omasheet_engine::{Engine, Options, Value};

/// A sheet as `.omx` source, and what was changed or left behind on the way.
#[derive(Debug, Default)]
pub struct Imported {
    pub source: String,
    pub notices: Vec<String>,
}

/// One file an export wrote.
#[derive(Debug)]
pub struct Output {
    /// The table this file holds, when it holds one of several.
    pub table: Option<String>,
    pub bytes: Vec<u8>,
}

/// The files of an export, and what they do not hold exactly.
#[derive(Debug, Default)]
pub struct Exported {
    pub files: Vec<Output>,
    pub notices: Vec<String>,
}

/// One cell on its way into a sheet.
#[derive(Clone, Debug, PartialEq)]
pub enum Datum {
    Empty,
    /// Text that a cell reads as a value of this type: `19.99`, `2025-01-31`.
    Lit(String, S),
    Text(String),
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
            Datum::Empty => "",
            Datum::Lit(s, _) | Datum::Text(s) => s,
        }
    }
}

/// A table on its way into a sheet: a name, a header row and rows of cells.
/// The names are as the file gives them, not yet identifiers.
#[derive(Debug, Default)]
pub struct Grid {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Datum>>,
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
        }
    }
}

const RESERVED: &[&str] = &[
    "if", "then", "else", "and", "or", "not", "in", "true", "false", "table", "const", "fn", "zone",
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
/// same text, and otherwise in quotes.
fn text_cell(text: &str) -> String {
    let bare = Datum::read(text) == Datum::Text(text.to_string())
        && text == text.trim()
        && !text.starts_with(['=', '#', '*'])
        && !text.contains(['|', '"', '\n', '\r', '\t'])
        && !text.contains(":=")
        && !text.chars().all(|c| matches!(c, '-' | ':' | ' '))
        && !RESERVED[9..]
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
/// `None` to leave it to the sheet, which reads exact numbers and text for
/// itself. Also whether the cells are of types that do not go together,
/// such as numbers and text: such a column is not declared, and each cell
/// is what it reads as.
fn column_type(cells: &[&Datum]) -> (Option<S>, bool) {
    let mut seen: Vec<S> = Vec::new();
    for cell in cells {
        let ty = match cell {
            Datum::Empty => continue,
            Datum::Lit(_, ty) => *ty,
            Datum::Text(_) => S::Text,
        };
        if !seen.contains(&ty) {
            seen.push(ty);
        }
    }
    let exact = |s: &S| matches!(s, S::Int | S::Ratio);
    match seen.as_slice() {
        [] | [S::Text] => (None, false),
        all if all.iter().all(exact) => (None, false),
        [one] => (Some(*one), false),
        all if all.iter().all(|s| exact(s) || *s == S::Num) => (Some(S::Num), false),
        _ => (None, true),
    }
}

/// Write tables as `.omx` source.
pub fn write_omx(grids: Vec<Grid>, notices: &mut Vec<String>) -> String {
    let mut out = String::new();
    let mut tables = Vec::new();
    for (g, grid) in grids.into_iter().enumerate() {
        let table = name_for(&grid.name, &format!("Table{}", g + 1), &mut tables, notices);
        let width = grid
            .rows
            .iter()
            .map(Vec::len)
            .chain([grid.headers.len()])
            .max()
            .unwrap_or(0);
        if width == 0 {
            continue;
        }
        let mut names = Vec::new();
        let columns: Vec<String> = (0..width)
            .map(|c| {
                let raw = grid.headers.get(c).map_or("", String::as_str);
                name_for(raw, &format!("Column{}", c + 1), &mut names, notices)
            })
            .collect();
        let cell = |row: &'_ Vec<Datum>, c: usize| row.get(c).cloned().unwrap_or(Datum::Empty);
        let mut rows: Vec<Vec<Datum>> = grid
            .rows
            .iter()
            .map(|row| (0..width).map(|c| cell(row, c)).collect())
            .collect();
        // A line with nothing on it is not a row.
        if width == 1 {
            let before = rows.len();
            rows.retain(|row| row[0] != Datum::Empty);
            if rows.len() < before {
                notices.push(format!("`{table}`: empty rows were left out"));
            }
        }

        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("table {table}\n\n"));
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
                "`{table}.{}` holds values of more than one type, and has no type of its own",
                columns[c]
            ));
        }

        let mut lines: Vec<Vec<String>> = vec![columns];
        for row in &rows {
            lines.push(
                row.iter()
                    .map(|d| match d {
                        Datum::Empty => String::new(),
                        Datum::Lit(text, _) => text.clone(),
                        Datum::Text(text) => text_cell(text),
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
        for line in lines {
            let cells: Vec<String> = line
                .iter()
                .enumerate()
                .map(|(c, text)| format!("{text}{}", " ".repeat(widths[c] - text.chars().count())))
                .collect();
            out.push_str(cells.join(" | ").trim_end());
            out.push('\n');
        }
    }
    out
}

/// A calculated sheet, for an export to read.
pub(crate) struct Sheet<'a> {
    pub program: &'a Program,
    pub engine: &'a Engine<'a>,
    /// The tables to export, by index.
    pub tables: Vec<usize>,
}

impl Sheet<'_> {
    pub fn name(&self, t: usize) -> &str {
        &self.program.tables[t].name
    }

    /// `Table.Column`, for a notice.
    pub fn column(&self, t: usize, c: usize) -> String {
        let table = &self.program.tables[t];
        format!("{}.{}", table.name, table.cols[c].name)
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

/// Compile and calculate a sheet, and hand it to `write`. A sheet that does
/// not compile is not exported: the error is its diagnostics.
pub(crate) fn with_sheet<T>(
    name: &str,
    text: &str,
    table: Option<&str>,
    options: Options,
    write: impl FnOnce(&Sheet) -> Result<T, String>,
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
    let tables = match table {
        None => (0..program.tables.len()).collect(),
        Some(wanted) => match program.tables.iter().position(|t| t.name == wanted) {
            Some(t) => vec![t],
            None => {
                let names: Vec<&str> = program.tables.iter().map(|t| t.name.as_str()).collect();
                return Err(vec![format!(
                    "omasheet: {name} has no table `{wanted}`; its tables are {}\n",
                    names.join(", ")
                )]);
            }
        },
    };
    if tables.is_empty() {
        return Err(vec![format!("omasheet: {name} has no tables to export\n")]);
    }
    let sheet = Sheet {
        program: &program,
        engine: &engine,
        tables,
    };
    write(&sheet).map_err(|e| vec![format!("omasheet: {e}\n")])
}

/// Whether a value is one a cell can hold and an export can write.
pub(crate) fn is_failure(v: &Value) -> bool {
    matches!(
        v,
        Value::Error | Value::Vector(_) | Value::Table(_) | Value::Row(_) | Value::Range { .. }
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
        assert_eq!(text_cell("=SUM(A1:A3)"), "\"=SUM(A1:A3)\"");
        assert_eq!(text_cell("a|b"), "\"a|b\"");
        assert_eq!(text_cell(" padded"), "\" padded\"");
        assert_eq!(text_cell("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(text_cell("table Sales"), "\"table Sales\"");
        assert_eq!(text_cell("---"), "\"---\"");
    }
}

// Copyright (c) 2026 Stephen Roe

//! JSON: the values of tables, each a list of rows by column name.

use crate::{
    Datum, Exported, FAILED, Grid, Imported, Losses, Output, is_failure, with_sheet, write_omx,
};
use omasheet_engine::omx::date::Style;
use omasheet_engine::omx::types::S;
use omasheet_engine::value::{format_exact, format_num, format_rat, to_f64};
use omasheet_engine::{Options, Value};
use serde_json::Value as Json;

const ROUNDED: &str = "exact rationals that do not end were written as decimals";
const NESTED: &str = "lists and objects inside a row were kept as text";
const SHAPE: &str = "it is not a list of rows, or an object holding one for each table by its name";

/// What a cell reads a JSON value as. A number is the number a cell would
/// read it as. JSON has no dates or times, so a string that is one is that;
/// so is one that is a `Complex`, `Inf` or `NaN`, which JSON has no numbers
/// for. Any other string is text, a number in quotes too.
fn datum(value: &Json, nested: &mut bool) -> Datum {
    match value {
        Json::Null => Datum::Empty,
        Json::Bool(b) => Datum::Lit(b.to_string(), S::Bool),
        Json::Number(n) => {
            let raw = n.to_string();
            match Datum::read(&raw) {
                read @ Datum::Lit(..) => read,
                // Written some way a cell does not read, such as `1E5`.
                _ => match raw.parse::<f64>() {
                    Ok(f) => Datum::Lit(format_num(f, true), S::Num),
                    Err(_) => Datum::Text(raw),
                },
            }
        }
        Json::String(s) => match Datum::read(s) {
            Datum::Lit(text, ty @ (S::Date | S::Time | S::DateTime | S::Complex)) => {
                Datum::Lit(text, ty)
            }
            Datum::Lit(text, S::Num) if text.ends_with("Inf") || text == "NaN" => {
                Datum::Lit(text, S::Num)
            }
            _ => Datum::Text(s.clone()),
        },
        Json::Array(_) | Json::Object(_) => {
            *nested = true;
            Datum::Text(value.to_string())
        }
    }
}

/// A table from a list of rows: each an object of values by column name, or
/// each a list of values, of which the first is the names.
fn grid(name: &str, rows: &[Json], notices: &mut Vec<String>) -> Result<Grid, String> {
    let mut nested = false;
    let mut grid = Grid {
        name: name.to_string(),
        ..Grid::default()
    };
    if rows.iter().all(Json::is_array) {
        let cells = |row: &Json, nested: &mut bool| -> Vec<Datum> {
            let row = row.as_array().map_or(&[][..], Vec::as_slice);
            row.iter().map(|v| datum(v, nested)).collect()
        };
        if let Some((names, rest)) = rows.split_first() {
            // The header row is names, whatever they look like.
            let label = |v: &Json| match v {
                Json::String(s) => s.trim().to_string(),
                Json::Null => String::new(),
                other => other.to_string(),
            };
            grid.headers = names.as_array().into_iter().flatten().map(label).collect();
            grid.rows = rest.iter().map(|row| cells(row, &mut nested)).collect();
        }
    } else {
        for (r, row) in rows.iter().enumerate() {
            let Json::Object(fields) = row else {
                return Err(format!(
                    "row {} of `{name}` is not an object of values by column name",
                    r + 1
                ));
            };
            let mut cells = vec![Datum::Empty; grid.headers.len()];
            for (key, value) in fields {
                let c = match grid.headers.iter().position(|h| h == key) {
                    Some(c) => c,
                    None => {
                        grid.headers.push(key.clone());
                        cells.push(Datum::Empty);
                        grid.headers.len() - 1
                    }
                };
                cells[c] = datum(value, &mut nested);
            }
            grid.rows.push(cells);
        }
    }
    if nested {
        notices.push(format!("`{name}`: {NESTED}"));
    }
    Ok(grid)
}

/// `.omx` source for a JSON file: a list of rows, which is a table called
/// `name`, the name of the file without `.json`; or an object of such
/// lists, each a table called by its key.
pub fn import(name: &str, bytes: &[u8]) -> Result<Imported, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "it is not UTF-8 text".to_string())?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let json: Json = serde_json::from_str(text).map_err(|e| format!("it is not JSON: {e}"))?;
    let mut imported = Imported::default();
    let grids = match &json {
        Json::Array(rows) => vec![grid(name, rows, &mut imported.notices)?],
        Json::Object(tables) if !tables.is_empty() => tables
            .iter()
            .map(|(key, rows)| match rows {
                Json::Array(rows) => grid(key, rows, &mut imported.notices),
                _ => Err(SHAPE.to_string()),
            })
            .collect::<Result<_, _>>()?,
        _ => return Err(SHAPE.to_string()),
    };
    if grids.iter().all(|g| g.headers.is_empty()) {
        return Err("it has no rows".to_string());
    }
    imported.source = write_omx(grids, &mut imported.notices);
    Ok(imported)
}

fn quoted(text: &str) -> String {
    Json::String(text.to_string()).to_string()
}

/// A value as JSON, and whether it was rounded to write it. A number is a
/// number, a `Percent` the number it stands for; a date, a time and a
/// `Complex` are strings as a sheet writes them, and so are `Inf` and
/// `NaN`.
fn field(v: &Value) -> (String, bool) {
    let num = |f: f64| {
        let text = format_num(f, true);
        if f.is_finite() { text } else { quoted(&text) }
    };
    match v {
        Value::Empty => ("null".to_string(), false),
        Value::Int(n) => (n.to_string(), false),
        Value::Decimal(r) | Value::Fraction(r) | Value::Percent(r) => {
            let exact = format_rat(r);
            if exact.contains('/') {
                (num(to_f64(r)), true)
            } else {
                (exact, false)
            }
        }
        Value::Num(f) => (num(*f), false),
        Value::Bool(b) => (b.to_string(), false),
        v => (quoted(&format_exact(v, false, &Style::ISO)), false),
    }
}

/// One JSON file for a sheet: an object holding each table by name as a
/// list of rows, each an object of values by column name. A table that is
/// named is that list alone.
pub fn export(
    name: &str,
    text: &str,
    table: Option<&str>,
    options: Options,
) -> Result<Exported, Vec<String>> {
    with_sheet(name, text, table, options, |sheet| {
        let mut losses = Losses::default();
        let alone = table.is_some();
        let indent = if alone { "  " } else { "    " };
        let mut lists = Vec::new();
        for &t in &sheet.tables {
            let table = &sheet.program.tables[t];
            let mut rows = Vec::with_capacity(table.nrows);
            for r in 0..table.nrows {
                let mut fields = Vec::with_capacity(table.cols.len());
                for (c, col) in table.cols.iter().enumerate() {
                    let value = sheet.engine.cell_shown(t, c, r);
                    let text = if is_failure(&value) {
                        losses.add(FAILED, sheet.column(t, c));
                        "null".to_string()
                    } else {
                        let (text, rounded) = field(&value);
                        if rounded {
                            losses.add(ROUNDED, sheet.column(t, c));
                        }
                        text
                    };
                    fields.push(format!("{}: {text}", quoted(&col.name)));
                }
                rows.push(format!("{indent}{{{}}}", fields.join(", ")));
            }
            // A row to a line, so that a change to one is a change to one.
            let close = &indent[2..];
            lists.push(if rows.is_empty() {
                "[]".to_string()
            } else {
                format!("[\n{}\n{close}]", rows.join(",\n"))
            });
        }
        let json = if alone {
            format!("{}\n", lists.concat())
        } else {
            let tables: Vec<String> = sheet
                .tables
                .iter()
                .zip(&lists)
                .map(|(&t, list)| format!("  {}: {list}", quoted(sheet.name(t))))
                .collect();
            format!("{{\n{}\n}}\n", tables.join(",\n"))
        };
        Ok(Exported {
            files: vec![Output {
                table: None,
                bytes: json.into_bytes(),
            }],
            notices: losses.notices(),
        })
    })
}

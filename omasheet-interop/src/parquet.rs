// Copyright (c) 2026 Stephen Roe

//! Parquet, in both directions.
//!
//! Since the types are the Polars dtypes, a column is written as the Parquet
//! type its own name stands for, rather than converted into something near
//! it: an `Int` is an `int64`, a `Decimal` a `decimal(38, s)`, a `Date` a
//! `date32`. On the way in, a column's type is taken from the file's schema
//! and never guessed from its values.
//!
//! With `--meta` (P34), what no format carries goes along in the file's
//! key-value metadata, under [`META`]: the file's zone, its constants and
//! functions, and the sheet's column types and formulas. It is written as
//! `.omx` source, the sheet with no rows, so it is read back by the same
//! parser as any file, and a reader that does not know it sees only the
//! columns.

use crate::{
    Datum, Exported, FAILED, Grid, Imported, Losses, Output, is_failure, text_cell, with_file,
    write_omx,
};
use omasheet_engine::dtype::type_of;
use omasheet_engine::omx::date::Style;
use omasheet_engine::omx::decimal::{Dec, MAX_PRECISION};
use omasheet_engine::omx::file::{SheetDecl, parse_file};
use omasheet_engine::omx::types::{Base, S};
use omasheet_engine::value::{format_exact, format_float};
use omasheet_engine::{Options, Value};
use polars::prelude::*;
use std::io::Cursor;

/// Said of a column whose values do not all fit the type it is written as.
/// A value no `Decimal(38, s)` holds — its point outside the scale a
/// Parquet decimal names — is written as a null. Public so that the app can
/// run an export, see this coming and ask before it writes the file.
pub const WIDE: &str = "values too wide for the column's type were written as null";

/// Said of a column Parquet has no type for, written as text instead.
const AS_TEXT: &str = "columns of no one type were written as text";

/// The number of seconds in a day, for a date-time.
const DAY: i64 = 86_400;

/// Microseconds in a second, the unit a date-time is written in.
const MICROS: i64 = 1_000_000;

/// Nanoseconds in a second, the unit a time of day is written in.
const NANOS: i64 = 1_000_000_000;

/// The exact number a value holds, if it is one.
fn exact(v: &Value) -> Option<Dec> {
    match v {
        Value::Int(n, _) => Some(Dec::from_i64(*n)),
        Value::Decimal(d) | Value::Percent(d) => Some(*d),
        _ => None,
    }
}

/// One sheet of a file as a Polars frame: a series for each column, of the
/// dtype the column's own type names.
fn frame(file: &crate::File, t: usize, losses: &mut Losses) -> Result<DataFrame, String> {
    let sheet = &file.program.sheets[t];
    let rows = sheet.nrows;
    let mut columns: Vec<Column> = Vec::with_capacity(sheet.cols.len());

    for (c, col) in sheet.cols.iter().enumerate() {
        let name: PlSmallStr = col.name.as_str().into();
        let cells: Vec<Value> = (0..rows)
            .map(|r| {
                let v = file.engine.cell_shown(t, c, r);
                if is_failure(&v) {
                    losses.add(FAILED, file.column(t, c));
                    Value::Null
                } else {
                    v
                }
            })
            .collect();

        // A whole column of nulls has no type of its own to write.
        let series = match col.ty {
            S::Int | S::Hex | S::Octal | S::Binary => {
                let v: Vec<Option<i64>> = cells
                    .iter()
                    .map(|v| match v {
                        Value::Int(n, _) => Some(*n),
                        Value::Null => None,
                        // A whole number past an `i64`, which an `Int64`
                        // column has no room for: a null, and said so.
                        _ => {
                            losses.add(WIDE, file.column(t, c));
                            None
                        }
                    })
                    .collect();
                Series::new(name, v)
            }
            S::Decimal | S::Percent => {
                // One scale for the whole column, as a Parquet decimal has:
                // the widest scale any value needs, but no wider than the
                // narrowest any of them can be written at: one long value
                // must not push the others past what they can hold.
                // Only the values a Parquet decimal can hold have a say in
                // its scale: one whose point sits outside `0..=38` is
                // written as a null below, and must not drag the scale of
                // the values around it down with it.
                let wanted = cells
                    .iter()
                    .filter_map(exact)
                    .map(|d| d.scale())
                    .filter(|s| (0..=MAX_PRECISION as i32).contains(s))
                    .max()
                    .unwrap_or(0);
                let room = cells
                    .iter()
                    .filter_map(exact)
                    .map(|d| d.max_scale())
                    .filter(|s| *s >= 0)
                    .min()
                    .unwrap_or(MAX_PRECISION as i32);
                // A Parquet decimal names a scale of `0..=38`, so a value
                // whose point sits outside that is written as a null.
                let scale = wanted.min(room).clamp(0, MAX_PRECISION as i32);
                let v: Vec<Option<i128>> = cells
                    .iter()
                    .map(|v| {
                        let d = exact(v)?;
                        match d.rescale(scale) {
                            // A value rounded away to nothing was too small
                            // for the column, as one that will not rescale
                            // at all was too large.
                            Some(r) if r.is_zero() && !d.is_zero() => {
                                losses.add(WIDE, file.column(t, c));
                                None
                            }
                            Some(r) => Some(r.unscaled()),
                            None => {
                                losses.add(WIDE, file.column(t, c));
                                None
                            }
                        }
                    })
                    .collect();
                // The values are already unscaled digits, so the series is
                // built as a decimal rather than cast to one: casting an
                // integer to a decimal scales it, which is not what these
                // are.
                let ca: Int128Chunked = v.into_iter().collect_ca(name);
                ca.into_decimal_unchecked(Some(MAX_PRECISION as usize), scale as usize)
                    .into_series()
            }
            S::Float => {
                let v: Vec<Option<f64>> = cells
                    .iter()
                    .map(|v| match v {
                        Value::Float(f) => Some(*f),
                        other => exact(other).map(|d| d.to_f64()),
                    })
                    .collect();
                Series::new(name, v)
            }
            S::Boolean => {
                let v: Vec<Option<bool>> = cells
                    .iter()
                    .map(|v| match v {
                        Value::Boolean(b) => Some(*b),
                        _ => None,
                    })
                    .collect();
                Series::new(name, v)
            }
            S::Date => {
                let v: Vec<Option<i32>> = cells
                    .iter()
                    .map(|v| match v {
                        Value::Date(d) => Some(*d),
                        _ => None,
                    })
                    .collect();
                Series::new(name, v)
                    .cast(&DataType::Date)
                    .map_err(|e| e.to_string())?
            }
            S::Time => {
                let v: Vec<Option<i64>> = cells
                    .iter()
                    .map(|v| match v {
                        // No scaling: a `Time` is held in the nanoseconds
                        // Polars holds one in.
                        Value::Time(ns) => Some(*ns),
                        _ => None,
                    })
                    .collect();
                Series::new(name, v)
                    .cast(&DataType::Time)
                    .map_err(|e| e.to_string())?
            }
            S::DateTime => {
                let v: Vec<Option<i64>> = cells
                    .iter()
                    .map(|v| match v {
                        Value::DateTime(us) => Some(*us),
                        Value::Zoned(z) => Some(z.wall),
                        _ => None,
                    })
                    .collect();
                Series::new(name, v)
                    .cast(&DataType::Datetime(TimeUnit::Microseconds, None))
                    .map_err(|e| e.to_string())?
            }
            // No one type: written the way a file writes it, as text.
            _ => {
                if !cells
                    .iter()
                    .all(|v| matches!(v, Value::Null | Value::String(..)))
                {
                    losses.add(AS_TEXT, file.column(t, c));
                }
                let v: Vec<Option<String>> = cells
                    .iter()
                    .map(|v| match v {
                        Value::Null => None,
                        other => Some(format_exact(other, false, &Style::ISO)),
                    })
                    .collect();
                Series::new(name, v)
            }
        };
        columns.push(series.into_column());
    }

    DataFrame::new(columns).map_err(|e| e.to_string())
}

/// One Parquet file for each sheet of a file, or for the one named.
/// The key under which `--meta` keeps the file's source, sheet by sheet.
pub const META: &str = "omasheet";

pub fn export(
    name: &str,
    text: &str,
    sheet: Option<&str>,
    options: Options,
) -> Result<Exported, Vec<String>> {
    write(name, text, sheet, options, false)
}

/// As [`export`], with what no format carries in the key-value metadata of
/// each file written (P34).
pub fn export_meta(
    name: &str,
    text: &str,
    sheet: Option<&str>,
    options: Options,
) -> Result<Exported, Vec<String>> {
    write(name, text, sheet, options, true)
}

fn write(
    name: &str,
    text: &str,
    sheet: Option<&str>,
    options: Options,
    meta: bool,
) -> Result<Exported, Vec<String>> {
    with_file(name, text, sheet, options, |file| {
        let mut exported = Exported::default();
        let mut losses = Losses::default();
        let several = file.sheets.len() > 1;
        for &t in &file.sheets {
            let mut df = frame(file, t, &mut losses)?;
            let mut bytes = Cursor::new(Vec::new());
            let kept = meta.then(|| {
                KeyValueMetadata::from_static(vec![(
                    META.to_string(),
                    skeleton(text, file.name(t), file),
                )])
            });
            ParquetWriter::new(&mut bytes)
                .with_key_value_metadata(kept)
                .finish(&mut df)
                .map_err(|e| e.to_string())?;
            exported.files.push(Output {
                sheet: several.then(|| file.name(t).to_string()),
                bytes: bytes.into_inner(),
            });
        }
        exported.notices = losses.notices();
        Ok(exported)
    })
}

/// The whole lines of `text` from `start` to `end`, without the last line
/// end.
fn lines(text: &str, start: usize, end: usize) -> &str {
    let from = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let to = text[end..].find('\n').map_or(text.len(), |i| end + i);
    text[from..to].trim_end()
}

/// What `--meta` keeps of sheet `name`: the file's zone, constants and
/// functions, then the sheet with its type lines and formulas and none of
/// its rows. A table's or a computed sheet's columns are written as a sheet
/// of its own, since it is the values that go with them now, and a `from`
/// is left off with the file it named.
fn skeleton(text: &str, name: &str, file: &crate::File) -> String {
    let ast = parse_file(text, 0, &mut Vec::new());
    let mut out = String::new();
    if let Some((zone, _)) = &ast.zone {
        out.push_str(&format!("zone {zone}\n\n"));
    }
    for c in &ast.consts {
        out.push_str(lines(text, c.span.start as usize, c.expr_span.end as usize));
        out.push('\n');
    }
    for f in &ast.funcs {
        out.push_str(lines(text, f.span.start as usize, f.expr_span.end as usize));
        out.push('\n');
    }
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str(&format!("sheet {name}\n\n"));
    let decl: Option<&SheetDecl> = ast.sheets.iter().find(|d| d.name == name);
    if let Some(decl) = decl
        && !decl.schema.is_empty()
    {
        for l in &decl.schema {
            let line = lines(text, l.span.start as usize, l.span.end as usize);
            let line = match l.from {
                Some(_) => line[..line.rfind(" from ").unwrap_or(line.len())].trim_end(),
                None => line,
            };
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }
    let Some(sheet) = file.program.sheets.iter().find(|s| s.name == name) else {
        return out;
    };
    let names: Vec<&str> = sheet.cols.iter().map(|c| c.name.as_str()).collect();
    out.push_str(&format!("| {} |\n", names.join(" | ")));
    let rule: Vec<String> = names.iter().map(|n| "-".repeat(n.len().max(3))).collect();
    out.push_str(&format!("| {} |\n", rule.join(" | ")));
    if let Some(decl) = decl
        && !decl.computed.is_empty()
    {
        out.push('\n');
        for c in &decl.computed {
            out.push_str(lines(text, c.span.start as usize, c.expr_span.end as usize));
            out.push('\n');
        }
    }
    out
}

/// One cell of a Parquet column, as the type its schema gives.
fn cell(value: AnyValue<'_>, ty: S) -> Datum {
    use omasheet_engine::omx::date;
    match value {
        AnyValue::Null => Datum::Empty,
        AnyValue::Boolean(b) => Datum::Lit(b.to_string(), S::Boolean),
        AnyValue::String(s) => Datum::Text(s.to_string()),
        AnyValue::StringOwned(s) => Datum::Text(s.to_string()),
        AnyValue::Int8(n) => Datum::Lit(n.to_string(), S::Int),
        AnyValue::Int16(n) => Datum::Lit(n.to_string(), S::Int),
        AnyValue::Int32(n) => Datum::Lit(n.to_string(), S::Int),
        AnyValue::Int64(n) => Datum::Lit(n.to_string(), S::Int),
        AnyValue::UInt8(n) => Datum::Lit(n.to_string(), S::Int),
        AnyValue::UInt16(n) => Datum::Lit(n.to_string(), S::Int),
        AnyValue::UInt32(n) => Datum::Lit(n.to_string(), S::Int),
        // A `u64` past an `i64` is still exact as a decimal.
        AnyValue::UInt64(n) => Datum::Lit(
            n.to_string(),
            if n > i64::MAX as u64 {
                S::Decimal
            } else {
                S::Int
            },
        ),
        AnyValue::Float32(f) => Datum::Lit(float_text(f as f64), S::Float),
        AnyValue::Float64(f) => Datum::Lit(float_text(f), S::Float),
        AnyValue::Decimal(unscaled, scale) => match Dec::new(unscaled, scale as i32) {
            Some(d) => Datum::Lit(d.to_plain_string(), S::Decimal),
            None => Datum::Empty,
        },
        AnyValue::Date(days) => Datum::Lit(date::format(days), S::Date),
        AnyValue::Datetime(n, unit, _) | AnyValue::DatetimeOwned(n, unit, _) => {
            let secs = match unit {
                TimeUnit::Nanoseconds => n.div_euclid(NANOS),
                TimeUnit::Microseconds => n.div_euclid(MICROS),
                TimeUnit::Milliseconds => n.div_euclid(1_000),
            };
            match i32::try_from(secs.div_euclid(DAY)) {
                Ok(_) => Datum::Lit(date::format_datetime(secs * MICROS), S::DateTime),
                Err(_) => Datum::Empty,
            }
        }
        AnyValue::Time(ns) => Datum::Lit(date::format_time(ns.div_euclid(NANOS) * NANOS), S::Time),
        // Anything else is written the way Polars shows it, as text.
        other => Datum::Lit(other.to_string(), ty),
    }
}

/// The source `--meta` kept, with the rows of `df` put under its header:
/// a computed column's cells are `*` again, and its values in the file are
/// passed over for its formula.
fn restored(kept: &str, df: &DataFrame) -> Result<Imported, String> {
    let ast = parse_file(kept, 0, &mut Vec::new());
    let Some(decl) = ast.sheets.first() else {
        return Err(format!("its `{META}` metadata holds no sheet"));
    };
    let computed = |name: &str| decl.computed.iter().any(|c| c.name == name);
    // A whole number in a `Hex`, `Octal` or `Binary` column is written in
    // its base, as the app writes one typed into it: the file holds only
    // the number.
    let base = |name: &str| {
        let ty = decl
            .schema
            .iter()
            .find(|l| l.name == name)
            .map_or(S::Any, |l| l.ty);
        Base::of(ty)
    };
    let mut lines: Vec<Vec<String>> = vec![decl.header.iter().map(|(h, _)| h.clone()).collect()];
    for r in 0..df.height() {
        let mut row = Vec::with_capacity(decl.header.len());
        for (name, _) in &decl.header {
            let cell = match df.column(name) {
                _ if computed(name) => "*".to_string(),
                Ok(column) => {
                    let ty = type_of(column.dtype()).unwrap_or(S::String);
                    match cell(column.get(r).map_err(|e| e.to_string())?, ty) {
                        Datum::Empty => String::new(),
                        Datum::Lit(text, S::Int) if base(name) != Base::Dec => {
                            match text.parse::<i64>() {
                                Ok(n) => base(name).write(n),
                                Err(_) => text,
                            }
                        }
                        Datum::Lit(text, _) => text,
                        Datum::Text(text) => text_cell(&text),
                        Datum::Formula(f) => format!("= {f}"),
                    }
                }
                // A column the file no longer has is left empty.
                Err(_) => String::new(),
            };
            row.push(cell);
        }
        lines.push(row);
    }
    let widths: Vec<usize> = (0..decl.header.len())
        .map(|c| {
            lines
                .iter()
                .map(|l| l[c].chars().count())
                .max()
                .unwrap_or(0)
                .max(3)
        })
        .collect();
    let line = |cells: &[String]| {
        let padded: Vec<String> = cells
            .iter()
            .zip(&widths)
            .map(|(cell, w)| format!("{cell}{}", " ".repeat(w - cell.chars().count())))
            .collect();
        format!("| {} |\n", padded.join(" | "))
    };
    let mut table = line(&lines[0]);
    table.push_str(&line(
        &widths.iter().map(|w| "-".repeat(*w)).collect::<Vec<_>>(),
    ));
    for row in &lines[1..] {
        table.push_str(&line(row));
    }
    // The header and its rule, which is all of the table `--meta` keeps.
    let start = decl.header_line.start as usize;
    let end = decl.separator_line.unwrap_or(decl.header_line).end as usize;
    let end = end + usize::from(kept[end..].starts_with('\n'));
    let source = format!("{}{table}{}", &kept[..start], &kept[end..]);
    Ok(Imported {
        source,
        notices: Vec::new(),
    })
}

/// A `Float` as a cell holds one: the shortest form that reads back as the
/// same `Float`, which is how a file writes one.
fn float_text(f: f64) -> String {
    format_float(f, true)
}

/// A Parquet file as `.omx` source: one sheet, named after the file. One
/// written with `--meta` is the source it keeps, with the file's rows put
/// in it.
pub fn import(name: &str, bytes: &[u8]) -> Result<Imported, String> {
    let mut reader = ParquetReader::new(Cursor::new(bytes.to_vec()));
    let kept = reader
        .get_metadata()
        .map_err(|e| e.to_string())?
        .key_value_metadata()
        .iter()
        .flatten()
        .find(|kv| kv.key == META)
        .and_then(|kv| kv.value.clone());
    let df = reader.finish().map_err(|e| e.to_string())?;
    if let Some(kept) = kept {
        return restored(&kept, &df);
    }
    let mut notices = Vec::new();

    // The schema says what each column is; the values are never consulted.
    let mut kept: Vec<(String, S)> = Vec::new();
    let mut nested = Vec::new();
    for column in df.get_columns() {
        match type_of(column.dtype()) {
            Some(ty) => kept.push((column.name().to_string(), ty)),
            None => nested.push(column.name().to_string()),
        }
    }
    if !nested.is_empty() {
        notices.push(format!(
            "columns Parquet holds in a shape a sheet cannot render were left out: {}",
            nested.join(", ")
        ));
    }

    let mut rows: Vec<Vec<Datum>> = Vec::with_capacity(df.height() + 1);
    rows.push(
        kept.iter()
            .map(|(name, _)| Datum::Text(name.clone()))
            .collect(),
    );
    for r in 0..df.height() {
        let mut row = Vec::with_capacity(kept.len());
        for (name, ty) in &kept {
            let column = df.column(name).map_err(|e| e.to_string())?;
            let value = column.get(r).map_err(|e| e.to_string())?;
            row.push(cell(value, *ty));
        }
        rows.push(row);
    }

    // Each cell carries the type its column's schema gave, so the schema
    // lines written come from the file and not from reading the values.
    let mut grid = Grid::from_rows(name, rows);
    grid.fixed = true;
    let source = write_omx(vec![grid], &mut notices);
    Ok(Imported { source, notices })
}

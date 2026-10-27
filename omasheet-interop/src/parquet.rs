// Copyright (c) 2026 Stephen Roe

//! Parquet, in both directions.
//!
//! Since the types are the Polars dtypes, a column is written as the Parquet
//! type its own name stands for, rather than converted into something near
//! it: an `Int` is an `int64`, a `Decimal` a `decimal(38, s)`, a `Date` a
//! `date32`. On the way in, a column's type is taken from the file's schema
//! and never guessed from its values.

use crate::{
    Datum, Exported, FAILED, Grid, Imported, Losses, Output, is_failure, with_file, write_omx,
};
use omasheet_engine::dtype::type_of;
use omasheet_engine::omx::date::Style;
use omasheet_engine::omx::decimal::{Dec, MAX_PRECISION};
use omasheet_engine::omx::types::S;
use omasheet_engine::value::{format_exact, format_float};
use omasheet_engine::{Options, Value};
use polars::prelude::*;
use std::io::Cursor;

/// Said of a column whose values do not all fit the type it is written as.
const WIDE: &str = "values too wide for the column's type were written as null";

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
        Value::Int(n) => Some(Dec::from_i64(*n)),
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
            S::Int => {
                let v: Vec<Option<i64>> = cells
                    .iter()
                    .map(|v| match v {
                        Value::Int(n) => Some(*n),
                        _ => None,
                    })
                    .collect();
                Series::new(name, v)
            }
            S::Decimal | S::Percent => {
                // One scale for the whole column, as a Parquet decimal has:
                // the widest scale any value needs, but no wider than the
                // narrowest any of them can be written at: one long value
                // must not push the others past what they can hold.
                let wanted = cells
                    .iter()
                    .filter_map(exact)
                    .map(|d| d.scale())
                    .max()
                    .unwrap_or(0);
                let room = cells
                    .iter()
                    .filter_map(exact)
                    .map(|d| d.max_scale())
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
pub fn export(
    name: &str,
    text: &str,
    sheet: Option<&str>,
    options: Options,
) -> Result<Exported, Vec<String>> {
    with_file(name, text, sheet, options, |file| {
        let mut exported = Exported::default();
        let mut losses = Losses::default();
        let several = file.sheets.len() > 1;
        for &t in &file.sheets {
            let mut df = frame(file, t, &mut losses)?;
            let mut bytes = Cursor::new(Vec::new());
            ParquetWriter::new(&mut bytes)
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

/// A `Float` as a cell holds one: the shortest form that reads back as the
/// same `Float`, which is how a file writes one.
fn float_text(f: f64) -> String {
    format_float(f, true)
}

/// A Parquet file as `.omx` source: one sheet, named after the file.
pub fn import(name: &str, bytes: &[u8]) -> Result<Imported, String> {
    let df = ParquetReader::new(Cursor::new(bytes.to_vec()))
        .finish()
        .map_err(|e| e.to_string())?;
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
            "columns Parquet holds in a shape a sheet has no room for were left out: {}",
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

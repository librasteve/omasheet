// Copyright (c) 2026 Stephen Roe

//! The rows of a `table`: a sheet whose rows live in Parquet files.
//!
//! The checker gives such a sheet its columns, from the schema lines in the
//! text, and a [`Source`] saying what to read. This fills in the rest: the
//! row count from the file's footer, each column's cells, and a diagnostic
//! where the text and the file disagree about a column.

use omasheet_engine::omx::Program;
use omasheet_engine::omx::ast::Lit;
use omasheet_engine::omx::check::{Cell, ColKind, Column, Source};
use omasheet_engine::omx::decimal::Dec;
use omasheet_engine::omx::diag::Diagnostic;
use omasheet_engine::omx::file::ident_len;
use omasheet_engine::omx::types::S;
use polars::prelude::*;
use std::fs::File;
use std::path::Path;

/// Seconds in a day, for a date-time past the end of a day.
const DAY: i64 = 86_400;

/// Microseconds in a second, the unit a date-time is written in.
const MICROS: i64 = 1_000_000;

/// Nanoseconds in a second, the unit a time of day is written in.
const NANOS: i64 = 1_000_000_000;

/// Fill in every sheet that reads its rows from elsewhere. `dir` is the
/// directory the file's own path is relative to, so that a source is read
/// from beside the `.omx` rather than from wherever the command was run.
pub fn read(program: &mut Program, dir: &Path, diags: &mut Vec<Diagnostic>) {
    for sheet in program.sheets.iter_mut() {
        let Some(source) = sheet.source.clone() else {
            continue;
        };
        let path = dir.join(&source.path);
        match frame(&path) {
            Ok(df) => fill(sheet, &df, &source, diags),
            Err(why) => diags.push(Diagnostic::new(
                source.span,
                format!("cannot read `{}`: {why}", source.path),
            )),
        }
    }
}

/// The row count of a source, from the footer alone: no row group is read. A
/// sheet's length is fixed before evaluation begins, and this is what fixes
/// it.
pub fn rows(dir: &Path, source: &Source) -> Result<usize, String> {
    let file = open(&dir.join(&source.path))?;
    ParquetReader::new(file)
        .num_rows()
        .map_err(|e| e.to_string())
}

/// The file a source names. A source names one file, so a missing one is
/// said to be missing rather than described as matching nothing.
fn open(path: &Path) -> Result<File, String> {
    File::open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => "there is no such file".to_string(),
        _ => e.to_string(),
    })
}

/// A source's file as a frame.
fn frame(path: &Path) -> Result<DataFrame, String> {
    ParquetReader::new(open(path)?)
        .finish()
        .map_err(|e| e.to_string())
}

/// The sheet's rows, and a diagnostic for each way the text and the file
/// disagree. The text is not corrected from the file and the file is not
/// read as the text says: a mismatch is reported and left (D29).
fn fill(
    sheet: &mut omasheet_engine::omx::check::Sheet,
    df: &DataFrame,
    source: &Source,
    diags: &mut Vec<Diagnostic>,
) {
    let held: Vec<(String, S)> = df
        .get_columns()
        .iter()
        .filter_map(|c| type_of(c.dtype()).map(|ty| (c.name().to_string(), ty)))
        .collect();

    // A table with no schema lines takes its columns from the file, so that
    // a source can be read before its schema has been written down.
    if sheet.cols.is_empty() {
        for (name, ty) in &held {
            sheet.cols.push(Column {
                name: name.clone(),
                span: source.span,
                ty: *ty,
                declared: false,
                kind: ColKind::Frame(Vec::new()),
                from: None,
            });
        }
    }

    for (name, ty) in &held {
        if !sheet.cols.iter().any(|c| held_as(c) == name) {
            // A name omasheet cannot write is reached by a `from` clause. A
            // column of nulls holds no type to offer, and a schema line
            // cannot be written without one, so the line is said rather
            // than shown.
            let bare = ident_len(name) == name.len();
            let help = match (*ty == S::Any, bare) {
                (true, true) => {
                    format!("add `{name}` to the table's schema, with the type the file will hold")
                }
                (true, false) => format!(
                    "add `{}` to the table's schema, with `from \"{name}\"` and the type the file will hold",
                    suggest(name)
                ),
                (false, true) => format!("add `{name} : {ty}` to the table's schema"),
                (false, false) => format!(
                    "add `{} : {ty} from \"{name}\"` to the table's schema",
                    suggest(name)
                ),
            };
            diags.push(
                Diagnostic::new(
                    source.span,
                    format!(
                        "the file has a column `{name}` that `{}` does not",
                        sheet.name
                    ),
                )
                .with_help(help),
            );
        }
    }

    let height = df.height();
    let names: Vec<String> = sheet.cols.iter().map(|c| held_as(c).to_string()).collect();
    for (c, col) in sheet.cols.iter_mut().enumerate() {
        if !matches!(col.kind, ColKind::Frame(_)) {
            continue;
        }
        let Ok(column) = df.column(&names[c]) else {
            diags.push(Diagnostic::new(
                col.span,
                format!("the file has no column `{}`", names[c]),
            ));
            col.kind = ColKind::Frame((0..height).map(|_| Cell::Invalid).collect());
            continue;
        };
        match type_of(column.dtype()) {
            Some(ty) if col.declared && !ty.assignable_to(col.ty) => diags.push(
                Diagnostic::new(
                    col.span,
                    format!(
                        "column `{}` is declared `{}` but the file holds `{ty}`",
                        col.name, col.ty
                    ),
                )
                .with_help("change the declared type, or the file"),
            ),
            Some(_) => {}
            None => diags.push(Diagnostic::new(
                col.span,
                format!(
                    "Parquet holds `{}` in a shape a sheet has no room for",
                    col.name
                ),
            )),
        }
        let mut cells = Vec::with_capacity(height);
        for r in 0..height {
            cells.push(match column.get(r) {
                Ok(value) => cell(value, col.ty),
                Err(_) => Cell::Invalid,
            });
        }
        col.kind = ColKind::Frame(cells);
    }
    sheet.nrows = height;
}

/// A name for a column the file holds under one omasheet cannot write: what
/// is left once the characters a name has no room for are dropped, with the
/// word after each of them capitalised, so that `Parents/Children Aboard` is
/// offered as `ParentsChildrenAboard` rather than as a part of itself.
fn suggest(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    // The first kept character begins a word, as every character after a
    // dropped run does.
    let mut word = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            if word {
                out.extend(c.to_uppercase());
            } else {
                out.push(c);
            }
            word = false;
        } else {
            word = true;
        }
    }
    match out.chars().next() {
        None => "Column".to_string(),
        // A name starts with a letter or an underscore, never a digit.
        Some(c) if c.is_ascii_digit() => format!("_{out}"),
        Some(_) => out,
    }
}

/// What the file calls a column: the name of its `from` clause, or the
/// column's own name where there is none.
fn held_as(col: &Column) -> &str {
    col.from.as_deref().unwrap_or(&col.name)
}

/// The omasheet type a Polars dtype is named by, or `None` for a shape a
/// sheet has no room for: a list, a struct, a map.
fn type_of(dtype: &DataType) -> Option<S> {
    Some(match dtype {
        DataType::Boolean => S::Boolean,
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => S::Int,
        DataType::Float32 | DataType::Float64 => S::Float,
        DataType::Decimal(..) => S::Decimal,
        DataType::String => S::String,
        DataType::Date => S::Date,
        DataType::Time => S::Time,
        DataType::Datetime(..) => S::DateTime,
        DataType::Null => S::Any,
        _ => return None,
    })
}

/// One value of a frame as a cell. The column's own type decides how an
/// exact number is held, so that a `Percent` column holds percentages and a
/// `Fraction` column fractions, as a sheet of text does.
fn cell(value: AnyValue<'_>, ty: S) -> Cell {
    let exact = |d: Dec| match ty {
        S::Percent => Cell::Lit(Lit::Percent(d)),
        _ => Cell::Lit(Lit::Decimal(d)),
    };
    let whole = |n: i64| match ty {
        S::Decimal | S::Fraction | S::Percent => exact(Dec::from_i64(n)),
        _ => Cell::Lit(Lit::Int(n)),
    };
    match value {
        AnyValue::Null => Cell::Empty,
        AnyValue::Boolean(b) => Cell::Lit(Lit::Boolean(b)),
        AnyValue::String(s) => Cell::Lit(Lit::String(s.to_string())),
        AnyValue::StringOwned(s) => Cell::Lit(Lit::String(s.to_string())),
        AnyValue::Int8(n) => whole(n as i64),
        AnyValue::Int16(n) => whole(n as i64),
        AnyValue::Int32(n) => whole(n as i64),
        AnyValue::Int64(n) => whole(n),
        AnyValue::UInt8(n) => whole(n as i64),
        AnyValue::UInt16(n) => whole(n as i64),
        AnyValue::UInt32(n) => whole(n as i64),
        // A `u64` past an `i64` is still exact as a decimal.
        AnyValue::UInt64(n) => match i64::try_from(n) {
            Ok(n) => whole(n),
            Err(_) => Dec::new(n as i128, 0).map_or(Cell::Invalid, exact),
        },
        AnyValue::Float32(f) => Cell::Lit(Lit::Float(f as f64)),
        AnyValue::Float64(f) => Cell::Lit(Lit::Float(f)),
        AnyValue::Decimal(unscaled, scale) => {
            Dec::new(unscaled, scale as u32).map_or(Cell::Invalid, exact)
        }
        AnyValue::Date(days) => Cell::Lit(Lit::Date(days)),
        AnyValue::Datetime(n, unit, _) | AnyValue::DatetimeOwned(n, unit, _) => {
            let secs = match unit {
                TimeUnit::Nanoseconds => n.div_euclid(NANOS),
                TimeUnit::Microseconds => n.div_euclid(MICROS),
                TimeUnit::Milliseconds => n.div_euclid(1_000),
            };
            match i32::try_from(secs.div_euclid(DAY)) {
                Ok(_) => Cell::Lit(Lit::DateTime(secs)),
                Err(_) => Cell::Invalid,
            }
        }
        AnyValue::Time(ns) => Cell::Lit(Lit::Time(ns.div_euclid(NANOS) as i32)),
        // Anything else is held as Polars shows it.
        other => Cell::Lit(Lit::String(other.to_string())),
    }
}

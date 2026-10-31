// Copyright (c) 2026 Stephen Roe

//! CSV: one sheet of values, read and written as text.

use crate::{
    Datum, Exported, FAILED, Grid, Imported, Losses, Output, is_failure, with_file, write_omx,
};
use omasheet_engine::omx::date::Style;
use omasheet_engine::value::{format_exact, format_rat};
use omasheet_engine::{Options, Value};

const ROUNDED: &str = "exact rationals that do not end were written as decimals";

/// `.omx` source for a CSV file. `name` is what the sheet is called, before
/// it is made an identifier: the name of the file without `.csv`.
pub fn import(name: &str, bytes: &[u8]) -> Result<Imported, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "it is not UTF-8 text".to_string())?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = ::csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for (i, record) in reader.records().enumerate() {
        let record = record.map_err(|e| format!("it is not CSV: {e}"))?;
        // The header row is names, whatever they look like.
        let read = |field: &str| match i {
            0 => Datum::Text(field.to_string()),
            _ => Datum::read(field),
        };
        rows.push(record.iter().map(read).collect());
    }
    if rows.is_empty() {
        return Err("it has no header row".to_string());
    }
    let mut imported = Imported::default();
    imported.source = write_omx(vec![Grid::from_rows(name, rows)], &mut imported.notices);
    Ok(imported)
}

/// A value as a CSV field, and whether it was rounded to write it. An
/// exact number is written in full: every digit it holds, which is every
/// digit there is of it.
fn field(v: &Value) -> (String, bool) {
    match v {
        // A `Fraction` goes out as the decimal it holds: a CSV is read by
        // anything, and `1/7` in one is not a number anywhere else.
        Value::Decimal(r) | Value::Fraction(r) => (format_rat(r), false),
        Value::Percent(_) => (format_exact(v, false, &Style::ISO), false),
        v => (format_exact(v, false, &Style::ISO), false),
    }
}

/// One CSV file for each sheet of a file, or for the one named.
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
            let sheet = &file.program.sheets[t];
            let mut writer = ::csv::Writer::from_writer(Vec::new());
            let fail = |e: ::csv::Error| e.to_string();
            writer
                .write_record(sheet.cols.iter().map(|c| c.name.as_str()))
                .map_err(fail)?;
            for r in 0..sheet.nrows {
                let mut record = Vec::with_capacity(sheet.cols.len());
                for c in 0..sheet.cols.len() {
                    let value = file.engine.cell_shown(t, c, r);
                    if is_failure(&value) {
                        losses.add(FAILED, file.column(t, c));
                        record.push(String::new());
                        continue;
                    }
                    let (text, rounded) = field(&value);
                    if rounded {
                        losses.add(ROUNDED, file.column(t, c));
                    }
                    record.push(text);
                }
                // A row of one empty field is written `""`, not a blank
                // line, which a reader would skip.
                writer.write_record(&record).map_err(fail)?;
            }
            let bytes = writer.into_inner().map_err(|e| e.to_string())?;
            exported.files.push(Output {
                sheet: several.then(|| file.name(t).to_string()),
                bytes,
            });
        }
        exported.notices = losses.notices();
        Ok(exported)
    })
}

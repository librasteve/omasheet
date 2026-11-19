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
    let mut grid = Grid::from_rows(name, rows);
    units_in_the_heads(&mut grid);
    let mut imported = Imported::default();
    imported.source = write_omx(vec![grid], &mut imported.notices);
    Ok(imported)
}

/// A head with a unit after it, `Revenue [USD]`, over numbers is a column of
/// that kind of quantity, `Revenue : Currency[USD]`: the way this program
/// writes one, and the numbers are read in the unit.
fn units_in_the_heads(grid: &mut Grid) {
    use omasheet_engine::omx::types::S;
    let width = grid.headers.len();
    let mut kinds: Vec<Option<String>> = vec![None; width];
    for c in 0..width {
        let head = grid.headers[c].clone();
        let Some((name, unit)) = head
            .strip_suffix(']')
            .and_then(|h| h.rsplit_once(" ["))
            .map(|(n, u)| (n.trim_end().to_string(), u.to_string()))
        else {
            continue;
        };
        let Ok(parsed) = siquant::Unit::parse(&unit, siquant::Scope::Any) else {
            continue;
        };
        let Some(kind) = siquant::Registry::builtin().infer_type(parsed.dims()) else {
            continue;
        };
        // Only numbers, or nothing, under it.
        let plain = grid.rows.iter().all(|row| match row.get(c) {
            None | Some(Datum::Empty) => true,
            Some(Datum::Lit(_, ty)) => ty.is_numeric() || matches!(ty, S::Scientific),
            _ => false,
        });
        if name.is_empty() || !plain {
            continue;
        }
        grid.headers[c] = name;
        kinds[c] = Some(format!("{}[{unit}]", kind.name()));
    }
    if kinds.iter().any(Option::is_some) {
        grid.kinds = kinds;
    }
}

const UNCERTAINTY: &str = "uncertainties were dropped, leaving the values";

/// A value as a CSV field, and whether it was rounded to write it. An
/// exact number is written in full: every digit it holds, which is every
/// digit there is of it.
fn field(v: &Value) -> (String, bool) {
    match v {
        Value::Decimal(r) => (format_rat(r), false),
        // A `Fraction` goes out as the fraction it is, `1/7`: exact, and read
        // back as a `Fraction` by this program.
        Value::Fraction(r) => (r.fraction_text(), false),
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
            // A column of quantities has its unit once, in its head, and plain
            // numbers under it, as a printed table has: `Revenue [USD]`.
            let units: Vec<Option<siquant::Unit>> =
                sheet.cols.iter().map(crate::quantity_unit).collect();
            writer
                .write_record(sheet.cols.iter().zip(&units).map(|(c, unit)| match unit {
                    Some(u) => format!("{} [{u}]", c.name),
                    None => c.name.clone(),
                }))
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
                    let in_unit = match (&units[c], &value) {
                        (Some(unit), Value::Quantity(q)) => q.convert(unit).ok(),
                        // A duration is microseconds: in seconds, then the unit.
                        (Some(unit), Value::Duration(us)) => {
                            let seconds = siquant::Unit::parse("s", siquant::Scope::Any).ok();
                            seconds
                                .and_then(|s| siquant::Quantity::exact(*us as f64 / 1e6, s).convert(unit).ok())
                        }
                        _ => None,
                    };
                    if let Some(q) = &in_unit {
                        if q.sigma() != 0.0 {
                            losses.add(UNCERTAINTY, file.column(t, c));
                        }
                        record.push(omasheet_engine::value::format_float(q.value(), true));
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

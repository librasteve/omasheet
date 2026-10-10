// Copyright (c) 2026 Stephen Roe

//! XLSX: a worksheet for each table. Every number in a workbook is a double,
//! so exact numbers are rounded on the way out and read back from the
//! shortest decimal on the way in.

use crate::{
    Datum, Exported, FAILED, Grid, Imported, Losses, Output, is_failure, with_sheet, write_omx,
};
use calamine::{Data, Reader, open_workbook_auto_from_rs};
use num_rational::BigRational;
use omasheet_engine::omx::convert::ratio_of;
use omasheet_engine::omx::date;
use omasheet_engine::omx::types::S;
use omasheet_engine::value::{format_exact, format_rat, to_f64};
use omasheet_engine::{Options, Value};
use rust_xlsxwriter::{Format, Workbook, Worksheet, XlsxError};
use std::io::Cursor;

/// Days from the day a workbook counts from, 1899-12-30, to 1970-01-01.
const EPOCH: i64 = 25_569;
const DAY: i64 = 86_400;

/// `A1`, `AB12`: where a cell is, for a notice.
fn a1(row: u32, col: u32) -> String {
    let mut letters = String::new();
    let mut n = col + 1;
    while n > 0 {
        letters.insert(0, (b'A' + ((n - 1) % 26) as u8) as char);
        n = (n - 1) / 26;
    }
    format!("{letters}{}", row + 1)
}

/// A number of a workbook as the exact number its shortest decimal is.
fn number(f: f64) -> Datum {
    match ratio_of(f) {
        Some(r) if r.is_integer() => Datum::Lit(r.to_integer().to_string(), S::Int),
        Some(r) => Datum::Lit(format_rat(&r), S::Ratio),
        None => Datum::Empty,
    }
}

/// A date serial as a date, a time of day or a date-time, by which parts
/// of it are there.
fn moment(serial: f64) -> Datum {
    let secs = (serial * DAY as f64).round() as i64;
    let (days, time) = (secs.div_euclid(DAY), secs.rem_euclid(DAY));
    if days == 0 {
        Datum::Lit(date::format_time(time as i32), S::Time)
    } else if time == 0 {
        Datum::Lit(date::format((days - EPOCH) as i32), S::Date)
    } else {
        Datum::Lit(date::format_datetime(secs - EPOCH * DAY), S::DateTime)
    }
}

/// `.omx` source for a workbook: a table for each worksheet.
pub fn import(bytes: &[u8]) -> Result<Imported, String> {
    let mut book = open_workbook_auto_from_rs(Cursor::new(bytes))
        .map_err(|e| format!("it is not a workbook: {e}"))?;
    let mut imported = Imported::default();
    let mut grids = Vec::new();
    for sheet in book.sheet_names() {
        let range = book
            .worksheet_range(&sheet)
            .map_err(|e| format!("worksheet `{sheet}` cannot be read: {e}"))?;
        let Some((top, left)) = range.start() else {
            imported
                .notices
                .push(format!("worksheet `{sheet}` is empty and was left out"));
            continue;
        };
        let mut failed = Vec::new();
        let mut rows: Vec<Vec<Datum>> = Vec::new();
        for (r, row) in range.rows().enumerate() {
            let mut cells = Vec::with_capacity(row.len());
            for (c, cell) in row.iter().enumerate() {
                cells.push(match cell {
                    Data::Empty => Datum::Empty,
                    Data::String(s) if s.trim().is_empty() => Datum::Empty,
                    Data::String(s) => Datum::Text(s.clone()),
                    Data::Int(n) => Datum::Lit(n.to_string(), S::Int),
                    Data::Float(f) => number(*f),
                    Data::Bool(b) => Datum::Lit(b.to_string(), S::Bool),
                    Data::DateTime(t) if t.is_duration() => number(t.as_f64() * DAY as f64),
                    Data::DateTime(t) => moment(t.as_f64()),
                    Data::DateTimeIso(s) | Data::DurationIso(s) => Datum::read(s),
                    Data::Error(_) => {
                        failed.push(a1(top + r as u32, left + c as u32));
                        Datum::Empty
                    }
                });
            }
            rows.push(cells);
        }
        // A column with a time on some of its dates is a column of
        // date-times.
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        for c in 0..width {
            let is =
                |row: &Vec<Datum>, ty| matches!(row.get(c), Some(Datum::Lit(_, t)) if *t == ty);
            if rows[1..].iter().any(|row| is(row, S::DateTime)) {
                for row in rows[1..].iter_mut().filter(|row| is(row, S::Date)) {
                    if let Datum::Lit(text, ty) = &mut row[c] {
                        text.push_str("T00:00");
                        *ty = S::DateTime;
                    }
                }
            }
        }
        let list = |cells: &[String]| {
            let more = match cells.len() {
                n if n > 8 => format!(" and {} more", n - 8),
                _ => String::new(),
            };
            format!("{}{more}", cells[..cells.len().min(8)].join(", "))
        };
        if !failed.is_empty() {
            imported.notices.push(format!(
                "worksheet `{sheet}`: cells holding an error were left empty: {}",
                list(&failed)
            ));
        }
        if let Ok(formulas) = book.worksheet_formula(&sheet) {
            let (ftop, fleft) = formulas.start().unwrap_or((0, 0));
            let cells: Vec<String> = formulas
                .used_cells()
                .filter(|(_, _, f)| !f.is_empty())
                .map(|(r, c, _)| a1(ftop + r as u32, fleft + c as u32))
                .collect();
            if !cells.is_empty() {
                imported.notices.push(format!(
                    "worksheet `{sheet}`: formulas were not translated, and their values were imported: {}",
                    list(&cells)
                ));
            }
        }
        grids.push(Grid::from_rows(&sheet, rows));
    }
    if grids.is_empty() {
        return Err("it has no worksheet with anything on it".to_string());
    }
    imported.source = write_omx(grids, &mut imported.notices);
    Ok(imported)
}

const ROUNDED: &str = "exact rationals were written as floating point";
const LARGE: &str = "integers too large for a workbook were rounded";
const AS_TEXT: &str = "values a workbook has no type for were written as text";
const ZONES: &str = "date-times in another time zone were written on the clocks of the sheet's";

/// The nearest double to an exact number, and whether it is that number.
fn double(r: &BigRational) -> (f64, bool) {
    let f = to_f64(r);
    (f, BigRational::from_float(f).as_ref() == Some(r))
}

struct Formats {
    date: Format,
    time: Format,
    datetime: Format,
}

fn write_cell(
    sheet: &mut Worksheet,
    formats: &Formats,
    (row, col): (u32, u16),
    value: &Value,
    mut lose: impl FnMut(&'static str),
) -> Result<(), XlsxError> {
    let day = DAY as f64;
    match value {
        Value::Empty => {}
        Value::Int(n) => {
            let (f, same) = double(&BigRational::from_integer(n.clone()));
            if !f.is_finite() {
                lose(AS_TEXT);
                sheet.write_string(row, col, n.to_string())?;
                return Ok(());
            }
            if !same {
                lose(LARGE);
            }
            sheet.write_number(row, col, f)?;
        }
        Value::Ratio(r) => {
            let (f, same) = double(r);
            if !same {
                lose(ROUNDED);
            }
            sheet.write_number(row, col, f)?;
        }
        Value::Num(f) if f.is_finite() => {
            sheet.write_number(row, col, *f)?;
        }
        Value::Text(s) => {
            sheet.write_string(row, col, &**s)?;
        }
        Value::Bool(b) => {
            sheet.write_boolean(row, col, *b)?;
        }
        Value::Date(d) => {
            let serial = (*d as i64 + EPOCH) as f64;
            sheet.write_number_with_format(row, col, serial, &formats.date)?;
        }
        Value::Time(t) => {
            sheet.write_number_with_format(row, col, *t as f64 / day, &formats.time)?;
        }
        Value::DateTime(t) => {
            let serial = *t as f64 / day + EPOCH as f64;
            sheet.write_number_with_format(row, col, serial, &formats.datetime)?;
        }
        Value::Zoned(z) => {
            lose(ZONES);
            let serial = z.home_wall() as f64 / day + EPOCH as f64;
            sheet.write_number_with_format(row, col, serial, &formats.datetime)?;
        }
        other => {
            lose(AS_TEXT);
            let text = format_exact(other, false, &date::Style::ISO);
            sheet.write_string(row, col, text)?;
        }
    }
    Ok(())
}

/// A workbook with a worksheet for each table of a sheet, or for the one
/// named.
pub fn export(
    name: &str,
    text: &str,
    table: Option<&str>,
    options: Options,
) -> Result<Exported, Vec<String>> {
    with_sheet(name, text, table, options, |sheet| {
        let fail = |e: XlsxError| e.to_string();
        let formats = Formats {
            date: Format::new().set_num_format("yyyy-mm-dd"),
            time: Format::new().set_num_format("hh:mm:ss"),
            datetime: Format::new().set_num_format("yyyy-mm-dd hh:mm:ss"),
        };
        let mut book = Workbook::new();
        let mut losses = Losses::default();
        let mut notices = Vec::new();
        let mut used: Vec<String> = Vec::new();
        for &t in &sheet.tables {
            let table = &sheet.program.tables[t];
            // A worksheet name is at most 31 characters.
            let mut title: String = table.name.chars().take(31).collect();
            let mut n = 2;
            while used.contains(&title) {
                let tail = format!("_{n}");
                title = table.name.chars().take(31 - tail.len()).collect::<String>() + &tail;
                n += 1;
            }
            if title != table.name {
                notices.push(format!("table `{}` is worksheet `{title}`", table.name));
            }
            used.push(title.clone());
            let page = book.add_worksheet();
            page.set_name(&title).map_err(fail)?;
            for (c, col) in table.cols.iter().enumerate() {
                page.write_string(0, c as u16, &col.name).map_err(fail)?;
            }
            for r in 0..table.nrows {
                for c in 0..table.cols.len() {
                    let value = sheet.engine.cell_shown(t, c, r);
                    if is_failure(&value) {
                        losses.add(FAILED, sheet.column(t, c));
                        continue;
                    }
                    let at = (r as u32 + 1, c as u16);
                    write_cell(page, &formats, at, &value, |what| {
                        losses.add(what, sheet.column(t, c))
                    })
                    .map_err(fail)?;
                }
            }
        }
        if table.is_none() && !sheet.program.consts.is_empty() {
            let names: Vec<&str> = sheet
                .program
                .consts
                .iter()
                .map(|c| c.name.as_str())
                .collect();
            notices.push(format!("constants are not exported: {}", names.join(", ")));
        }
        notices.extend(losses.notices());
        let bytes = book.save_to_buffer().map_err(fail)?;
        Ok(Exported {
            files: vec![Output { table: None, bytes }],
            notices,
        })
    })
}

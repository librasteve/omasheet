// Copyright (c) 2026 Stephen Roe

//! XLSX: a worksheet for each table. Every number in a workbook is a double,
//! so exact numbers are rounded on the way out and read back from the
//! shortest decimal on the way in.

use crate::{
    Datum, Exported, FAILED, Grid, Imported, Losses, Names, Output, Place, formula, is_failure,
    names, with_sheet, write_named,
};
use calamine::{Data, Reader, open_workbook_auto_from_rs};
use num_rational::BigRational;
use omasheet_engine::omx::check::{Cell, ColKind};
use omasheet_engine::omx::convert::fraction_of;
use omasheet_engine::omx::date;
use omasheet_engine::omx::types::S;
use omasheet_engine::omx::{Sources, compile};
use omasheet_engine::value::{format_exact, format_rat, to_f64};
use omasheet_engine::{Engine, Options, Value};
use rust_xlsxwriter::{Format, Workbook, Worksheet, XlsxError};
use std::io::Cursor;

mod write;

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
    match fraction_of(f) {
        Some(r) if r.is_integer() => Datum::Lit(r.to_integer().to_string(), S::Int),
        Some(r) => Datum::Lit(format_rat(&r), S::Decimal),
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

/// A formula of a workbook, and the cell it is in.
struct Formula {
    grid: usize,
    /// The row among the table's rows, and the column.
    row: usize,
    column: usize,
    /// `B3`, for a notice.
    cell: String,
    /// What the workbook last worked it out as.
    cached: Datum,
    /// The translation as one of a column filled down.
    filled: String,
    /// `today()` and the like: what it gives now is not what it gave then.
    volatile: bool,
}

/// A value as a number, if it is one.
fn real(v: &Value) -> Option<f64> {
    match v {
        Value::Int(n) => n.to_string().parse().ok(),
        Value::Decimal(r) | Value::Fraction(r) | Value::Percent(r) => Some(to_f64(r)),
        Value::Num(f) => Some(*f),
        Value::Bool(b) => Some(*b as u8 as f64),
        _ => None,
    }
}

/// Whether a formula gave what the workbook holds for its cell. Numbers
/// are the same when they agree as far as a workbook holds them.
fn agrees(cached: &Datum, v: &Value) -> bool {
    let text = match v {
        Value::Empty => String::new(),
        v => format_exact(v, false, &date::Style::ISO),
    };
    match cached {
        Datum::Empty | Datum::Formula(_) => text.is_empty(),
        Datum::Text(s) => *s == text || s.trim() == text,
        Datum::Lit(s, ty) if ty.is_numeric() => {
            match (s.replace('_', "").parse::<f64>(), real(v)) {
                (Ok(a), Some(b)) => (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1e-300),
                _ => *s == text,
            }
        }
        // A date among date-times was given a time of midnight.
        Datum::Lit(s, _) => *s == text || s.strip_suffix("T00:00") == Some(text.as_str()),
    }
}

const REFUSED: &str = "the sheet does not take its translation";
const DIFFERS: &str = "its translation gives another value than the workbook holds";

/// Put back the values of the formulas that do not hold up: those the
/// sheet does not check, and those that do not give what the workbook has.
/// Each goes into `dropped` with the reason.
fn settle(
    grids: &mut [Grid],
    names: &[Names],
    formulas: &[Formula],
    dropped: &mut Vec<(usize, &'static str)>,
) {
    let mut live: Vec<usize> = (0..formulas.len()).collect();
    let mut drop = |grids: &mut [Grid], live: &mut Vec<usize>, which: Vec<usize>, why| {
        for f in which {
            let formula = &formulas[f];
            grids[formula.grid].rows[formula.row][formula.column] = formula.cached.clone();
            live.retain(|l| *l != f);
            dropped.push((f, why));
        }
    };
    // Each round drops at least one formula, or is the last.
    while !live.is_empty() {
        let (source, places) = write_named(grids, names, &mut Vec::new());
        // The formulas written at a place: one cell's, or a whole column's.
        let at = |place: &Place| -> Vec<usize> {
            live.iter()
                .copied()
                .filter(|f| {
                    let formula = &formulas[*f];
                    formula.grid == place.grid
                        && formula.column == place.column
                        && place.row.is_none_or(|row| row == formula.row)
                })
                .collect()
        };
        let mut sources = Sources::new();
        let src = sources.add("workbook", &source);
        let (program, diags) = compile(&source, src);
        if !diags.is_empty() {
            let mut bad: Vec<usize> = Vec::new();
            for d in &diags {
                let start = d.span.start as usize;
                for place in places.iter().filter(|p| p.start <= start && start < p.end) {
                    bad.extend(at(place));
                }
            }
            bad.sort_unstable();
            bad.dedup();
            // An error with no formula to blame: none of them can be kept.
            if bad.is_empty() {
                bad = live.clone();
            }
            drop(grids, &mut live, bad, REFUSED);
            continue;
        }
        let engine = Engine::with_options(&program, Options::default());
        engine.take_diags();
        engine.run();
        let mut bad = Vec::new();
        for &f in &live {
            let formula = &formulas[f];
            let table = &names[formula.grid].table;
            let Some(t) = program.tables.iter().position(|t| t.name == *table) else {
                continue;
            };
            match engine.cell(t, formula.column, formula.row) {
                Ok(v) if formula.volatile || agrees(&formula.cached, &v) => {}
                _ => bad.push(f),
            }
        }
        if bad.is_empty() {
            break;
        }
        drop(grids, &mut live, bad, DIFFERS);
    }
}

/// `.omx` source for a workbook: a table for each worksheet.
pub fn import(bytes: &[u8]) -> Result<Imported, String> {
    let mut book = open_workbook_auto_from_rs(Cursor::new(bytes))
        .map_err(|e| format!("it is not a workbook: {e}"))?;
    let mut imported = Imported::default();
    let mut grids = Vec::new();
    // For each grid: its worksheet, and where its header starts.
    let mut origins: Vec<(String, u32, u32)> = Vec::new();
    let list = |cells: &[String]| {
        let more = match cells.len() {
            n if n > 8 => format!(" and {} more", n - 8),
            _ => String::new(),
        };
        format!("{}{more}", cells[..cells.len().min(8)].join(", "))
    };
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
        if !failed.is_empty() {
            imported.notices.push(format!(
                "worksheet `{sheet}`: cells holding an error were left empty: {}",
                list(&failed)
            ));
        }
        grids.push(Grid::from_rows(&sheet, rows));
        origins.push((sheet, top, left));
    }
    if grids.is_empty() {
        return Err("it has no worksheet with anything on it".to_string());
    }

    let names = names(&grids, &mut imported.notices);
    let sheets: Vec<formula::Sheet> = (0..grids.len())
        .map(|g| formula::Sheet {
            excel: origins[g].0.clone(),
            table: names[g].table.clone(),
            columns: names[g].columns.clone(),
            top: origins[g].1,
            left: origins[g].2,
            rows: grids[g].rows.len(),
        })
        .collect();
    // Each formula is translated, or has a reason why not.
    let mut formulas: Vec<Formula> = Vec::new();
    let mut untranslated: Vec<(usize, String, String)> = Vec::new();
    for (g, (sheet, top, left)) in origins.iter().enumerate() {
        let Ok(found) = book.worksheet_formula(sheet) else {
            continue;
        };
        let (ftop, fleft) = found.start().unwrap_or((0, 0));
        for (r, c, text) in found.used_cells().filter(|(_, _, f)| !f.is_empty()) {
            let (row, col) = (ftop + r as u32, fleft + c as u32);
            let cell = a1(row, col);
            // A formula in the header row, or outside what the table holds.
            let place = (row.checked_sub(top + 1), col.checked_sub(*left));
            let (Some(row_in), Some(column)) = place else {
                untranslated.push((g, "it is in the header row".into(), cell));
                continue;
            };
            let (row_in, column) = (row_in as usize, column as usize);
            let Some(cached) = grids[g]
                .rows
                .get(row_in)
                .and_then(|cells| cells.get(column))
            else {
                continue;
            };
            match formula::translate(text, &sheets, g, row) {
                Ok(translated) => {
                    formulas.push(Formula {
                        grid: g,
                        row: row_in,
                        column,
                        cell,
                        cached: cached.clone(),
                        filled: translated.filled,
                        volatile: translated.volatile,
                    });
                    grids[g].rows[row_in][column] = Datum::Formula(translated.text);
                }
                Err(why) => untranslated.push((g, why, cell)),
            }
        }
    }
    if !formulas.is_empty() {
        for grid in &mut grids {
            grid.fixed = true;
        }
    }
    // A column filled down with one formula: every row has it, and it is
    // the same written from each.
    for (g, grid) in grids.iter_mut().enumerate() {
        for c in 0..grid.headers.len() {
            let of: Vec<&Formula> = formulas
                .iter()
                .filter(|f| f.grid == g && f.column == c)
                .collect();
            let same = of.len() == grid.rows.len()
                && of.len() > 1
                && of.iter().all(|f| f.filled == of[0].filled);
            if same {
                for f in of {
                    grid.rows[f.row][c] = Datum::Formula(f.filled.clone());
                }
            }
        }
    }
    let mut dropped = Vec::new();
    settle(&mut grids, &names, &formulas, &mut dropped);
    for (f, why) in dropped {
        untranslated.push((formulas[f].grid, why.to_string(), formulas[f].cell.clone()));
    }
    for (g, (sheet, ..)) in origins.iter().enumerate() {
        let kept = formulas.iter().filter(|f| f.grid == g).count()
            - untranslated
                .iter()
                .filter(|(at, why, _)| *at == g && (why == REFUSED || why == DIFFERS))
                .count();
        if kept > 0 {
            let (were, s) = if kept == 1 {
                ("was", "")
            } else {
                ("were", "s")
            };
            imported.notices.push(format!(
                "worksheet `{sheet}`: {kept} formula{s} {were} translated"
            ));
        }
        // The reasons in the order they were met, each with its cells.
        let mut reasons: Vec<&str> = Vec::new();
        for (at, why, _) in &untranslated {
            if *at == g && !reasons.contains(&why.as_str()) {
                reasons.push(why);
            }
        }
        for reason in reasons {
            let cells: Vec<String> = untranslated
                .iter()
                .filter(|(at, why, _)| *at == g && why == reason)
                .map(|(_, _, cell)| cell.clone())
                .collect();
            imported.notices.push(format!(
                "worksheet `{sheet}`: values were imported in place of formulas, as {reason}: {}",
                list(&cells)
            ));
        }
    }
    imported.source = write_named(&grids, &names, &mut imported.notices).0;
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
    percent: Format,
}

/// What a cell of a workbook holds for a value.
enum Shown<'f> {
    Number(f64, Option<&'f Format>),
    Text(String),
    Bool(bool),
    /// A value a workbook has no type for, as text: no formula gives it.
    Written(String),
}

/// A value as a workbook holds it, or `None` for nothing.
fn shown<'f>(
    formats: &'f Formats,
    value: &Value,
    mut lose: impl FnMut(&'static str),
) -> Option<Shown<'f>> {
    let day = DAY as f64;
    Some(match value {
        Value::Empty => return None,
        Value::Int(n) => {
            let (f, same) = double(&BigRational::from_integer(n.clone()));
            if !f.is_finite() {
                lose(AS_TEXT);
                return Some(Shown::Written(n.to_string()));
            }
            if !same {
                lose(LARGE);
            }
            Shown::Number(f, None)
        }
        Value::Decimal(r) | Value::Fraction(r) => {
            let (f, same) = double(r);
            if !same {
                lose(ROUNDED);
            }
            Shown::Number(f, None)
        }
        Value::Percent(r) => {
            let (f, same) = double(r);
            if !same {
                lose(ROUNDED);
            }
            Shown::Number(f, Some(&formats.percent))
        }
        Value::Num(f) if f.is_finite() => Shown::Number(*f, None),
        Value::Text(s) => Shown::Text(s.to_string()),
        Value::Bool(b) => Shown::Bool(*b),
        Value::Date(d) => Shown::Number((*d as i64 + EPOCH) as f64, Some(&formats.date)),
        Value::Time(t) => Shown::Number(*t as f64 / day, Some(&formats.time)),
        Value::DateTime(t) => {
            Shown::Number(*t as f64 / day + EPOCH as f64, Some(&formats.datetime))
        }
        Value::Zoned(z) => {
            lose(ZONES);
            let serial = z.home_wall() as f64 / day + EPOCH as f64;
            Shown::Number(serial, Some(&formats.datetime))
        }
        other => {
            lose(AS_TEXT);
            Shown::Written(format_exact(other, false, &date::Style::ISO))
        }
    })
}

/// Write a cell: its value, or its formula with the value as its result,
/// so that the workbook shows it before anything works it out again.
fn write_cell(
    sheet: &mut Worksheet,
    (row, col): (u32, u16),
    shown: Shown,
    formula: Option<String>,
) -> Result<(), XlsxError> {
    let Some(formula) = formula else {
        match shown {
            Shown::Number(f, None) => sheet.write_number(row, col, f)?,
            Shown::Number(f, Some(format)) => {
                sheet.write_number_with_format(row, col, f, format)?
            }
            Shown::Text(s) | Shown::Written(s) => sheet.write_string(row, col, s)?,
            Shown::Bool(b) => sheet.write_boolean(row, col, b)?,
        };
        return Ok(());
    };
    let (result, format) = match shown {
        Shown::Number(f, format) => (f.to_string(), format),
        Shown::Text(s) | Shown::Written(s) => (s, None),
        Shown::Bool(b) => (if b { "TRUE" } else { "FALSE" }.to_string(), None),
    };
    let formula = rust_xlsxwriter::Formula::new(formula).set_result(result);
    match format {
        Some(format) => sheet.write_formula_with_format(row, col, formula, format)?,
        None => sheet.write_formula(row, col, formula)?,
    };
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
            percent: Format::new().set_num_format("0.00%"),
        };
        let mut book = Workbook::new();
        let mut losses = Losses::default();
        let mut notices = Vec::new();
        let mut used: Vec<String> = Vec::new();
        // Every worksheet is named before any is written, as a formula may
        // read a table that comes after its own.
        let mut titles: Vec<Option<String>> = vec![None; sheet.program.tables.len()];
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
            titles[t] = Some(title);
        }
        let writer = write::Writer {
            program: sheet.program,
            engine: sheet.engine,
            titles,
        };
        let mut formulas = 0;
        for &t in &sheet.tables {
            let table = &sheet.program.tables[t];
            let page = book.add_worksheet();
            let title = writer.titles[t].as_deref().unwrap_or(&table.name);
            page.set_name(title).map_err(fail)?;
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
                    let Some(shown) = shown(&formats, &value, |what| {
                        losses.add(what, sheet.column(t, c))
                    }) else {
                        continue;
                    };
                    let node = match &table.cols[c].kind {
                        ColKind::Computed(node) => Some(node),
                        ColKind::Data(cells) => match cells.get(r) {
                            Some(Cell::Formula(node)) => Some(node),
                            _ => None,
                        },
                    };
                    // A value with no type in a workbook has no formula there.
                    let node = node.filter(|_| !matches!(shown, Shown::Written(_)));
                    let formula = match node.map(|node| writer.formula(node, t, r)) {
                        Some(Ok(formula)) => formula,
                        Some(Err(why)) => {
                            losses.add(why, sheet.column(t, c));
                            None
                        }
                        None => None,
                    };
                    formulas += formula.is_some() as usize;
                    write_cell(page, (r as u32 + 1, c as u16), shown, formula).map_err(fail)?;
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
        if formulas > 0 {
            let s = if formulas == 1 { "" } else { "s" };
            notices.push(format!("{formulas} formula{s} written"));
        }
        notices.extend(losses.notices());
        let bytes = book.save_to_buffer().map_err(fail)?;
        Ok(Exported {
            files: vec![Output { table: None, bytes }],
            notices,
        })
    })
}

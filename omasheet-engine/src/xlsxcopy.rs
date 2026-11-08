// Copyright (c) 2026 Stephen Roe

//! A sheet of an XLSX workbook turned into a Parquet copy, streamed
//! (`open-a-big-csv` D152, D153): the cells of the sheet are read one at a
//! time from its XML, never the sheet whole, and written a row group at a
//! time, so memory is a batch of rows whatever the sheet's size. Only the
//! workbook's table of shared strings is held, as `calamine` holds it.
//!
//! A source names one sheet with `book.xlsx#Sheet`, and a bare `book.xlsx` is
//! the first sheet with anything in it. The sheet is read as `omasheet
//! import` reads it, so a table and an import agree: the first row of what
//! is used is the names; a blank cell is a null, and so is a cell holding an
//! error; a number that is a whole number is whole; a date is a `Date`, a
//! time of day a `Time`, a date with a time a `Datetime`, a duration a
//! `Duration`, and a column with a time on some of its dates is a column of
//! date-times. Text stays text, whatever it looks like. A row of the sheet
//! with nothing in it is a row of nulls, as it is in an import, since a
//! formula may be looking for it where it is.
//!
//! The sheet's formulas are not read: a cell is the value the workbook last
//! worked out. A table is values only, so a workbook with a formula in it is
//! not one to open as tables: [`has_formulas`] says, by reading the sheet
//! for one, and the caller keeps the formulas by importing it instead.

use crate::copyframe::{self, Cell, Names, RowBuf, RowSource, Shape};
use calamine::{DataRef, Reader, Xlsx, open_workbook};
use std::cell::Cell as Slot;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Days from the day a workbook counts from, 1899-12-30, to 1970-01-01.
const EPOCH: i64 = 25_569;
const DAY: i64 = 86_400;

/// What the source says when a cell is left of the column the sheet was
/// taken to start at, which `convert` answers by starting again from it.
const LEFT_OF_IT: &str = "omasheet: a cell left of the sheet's stated start";

type Book = Xlsx<BufReader<File>>;

fn open(path: &Path) -> Result<Book, String> {
    open_workbook(path).map_err(|e| format!("it is not a workbook: {e}"))
}

/// The sheets of the workbook at `path` that have a cell in them, by name,
/// in the workbook's order. A sheet with nothing in it is not a table.
pub fn sheets(path: &Path) -> Result<Vec<String>, String> {
    let mut book = open(path)?;
    let mut found = Vec::new();
    for name in book.sheet_names() {
        let mut cells = book
            .worksheet_cells_reader(&name)
            .map_err(|e| format!("worksheet `{name}` cannot be read: {e}"))?;
        if cells
            .next_cell()
            .map_err(|e| format!("worksheet `{name}` cannot be read: {e}"))?
            .is_some()
        {
            found.push(name);
        }
    }
    if found.is_empty() {
        return Err("it has no rows".to_string());
    }
    Ok(found)
}

/// Whether any cell of the sheet `sheet` of the workbook at `path` holds a
/// formula. The whole sheet is read, as a formula in the last row is as much
/// one as a formula in the first, and stops at the first it finds; a shared
/// formula is a group, and what is read of a cell of it is not expanded.
pub fn has_formulas(path: &Path, sheet: &str) -> Result<bool, String> {
    let mut book = open(path)?;
    let mut cells = book
        .worksheet_cells_reader(sheet)
        .map_err(|e| format!("there is no worksheet `{sheet}` ({e})"))?;
    loop {
        let next = cells
            .next_cell_with_formula_metadata()
            .map_err(|e| format!("worksheet `{sheet}` cannot be read: {e}"))?;
        match next {
            Some(cell) if cell.formula.is_some() => return Ok(true),
            Some(_) => {}
            None => return Ok(false),
        }
    }
}

/// A workbook cell as the cell of a copy, as `omasheet import` reads it.
fn cell_of(value: &DataRef) -> Cell {
    match value {
        DataRef::Empty | DataRef::Error(_) => Cell::Null,
        DataRef::String(s) if s.trim().is_empty() => Cell::Null,
        DataRef::SharedString(s) if s.trim().is_empty() => Cell::Null,
        DataRef::String(s) => Cell::Str(s.clone()),
        DataRef::SharedString(s) => Cell::Str((*s).to_string()),
        DataRef::Int(n) => Cell::Int(*n),
        DataRef::Float(f) if !f.is_finite() => Cell::Null,
        // A workbook keeps every number as a float; one that is whole is
        // whole, as an import has it.
        DataRef::Float(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => Cell::Int(*f as i64),
        DataRef::Float(f) => Cell::Float(*f),
        DataRef::Bool(b) => Cell::Bool(*b),
        DataRef::DateTime(t) if t.is_duration() => {
            Cell::Duration((t.as_f64() * DAY as f64 * 1_000_000.0).round() as i64)
        }
        DataRef::DateTime(t) => moment(t.as_f64()),
        DataRef::DateTimeIso(s) | DataRef::DurationIso(s) => Cell::Str(s.clone()),
    }
}

/// A date serial as a date, a time of day or a date-time, by which parts of
/// it are there.
fn moment(serial: f64) -> Cell {
    let secs = (serial * DAY as f64).round() as i64;
    let (days, time) = (secs.div_euclid(DAY), secs.rem_euclid(DAY));
    if days == 0 {
        Cell::Time(time * 1_000_000_000)
    } else if time == 0 {
        Cell::Date((days - EPOCH) as i32)
    } else {
        Cell::Datetime((secs - EPOCH * DAY) * 1_000_000)
    }
}

/// The rows of one sheet of a workbook.
struct XlsxSource<'a> {
    path: &'a Path,
    sheet: Option<&'a str>,
    /// The column the sheet starts at, where it is not the one it says: set
    /// when a cell turns up to the left of it.
    left: Slot<Option<u32>>,
    /// The column of a cell found to the left of where the sheet was taken
    /// to start.
    found_left: Slot<Option<u32>>,
}

impl RowSource for XlsxSource<'_> {
    fn rows(
        &self,
        names: &mut Names,
        each: &mut dyn FnMut(Shape, &mut Names, &mut RowBuf) -> bool,
    ) -> Result<(), String> {
        let mut book = open(self.path)?;
        let name = match self.sheet {
            Some(sheet) => sheet.to_string(),
            None => sheets(self.path)?.remove(0),
        };
        let mut cells = book
            .worksheet_cells_reader(&name)
            .map_err(|e| format!("there is no worksheet `{name}` ({e})"))?;
        // The sheet states where its cells are, and a cell outside that is
        // for the next try, which starts from where it is.
        let left = self
            .left
            .get()
            .unwrap_or_else(|| cells.dimensions().start.1);
        let mut buf = RowBuf {
            cells: Vec::new(),
            unknown: false,
        };
        let mut current: Option<u32> = None;
        let empty = |buf: &mut RowBuf| {
            buf.cells.clear();
        };
        loop {
            let next = cells
                .next_cell()
                .map_err(|e| format!("worksheet `{name}` cannot be read: {e}"))?;
            let Some(cell) = next else {
                break;
            };
            let (row, col) = cell.get_position();
            if col < left {
                self.found_left.set(Some(col));
                return Err(LEFT_OF_IT.to_string());
            }
            match current {
                Some(at) if at == row => {}
                Some(at) => {
                    // The row before it is whole, and so is each row there
                    // is none of between.
                    if !each(Shape::List, names, &mut buf) {
                        return Ok(());
                    }
                    for _ in at + 1..row {
                        empty(&mut buf);
                        if !each(Shape::List, names, &mut buf) {
                            return Ok(());
                        }
                    }
                    empty(&mut buf);
                    current = Some(row);
                }
                None => current = Some(row),
            }
            let c = (col - left) as usize;
            if buf.cells.len() <= c {
                buf.cells.resize_with(c + 1, || None);
            }
            buf.cells[c] = Some(cell_of(cell.get_value()));
        }
        if current.is_some() {
            each(Shape::List, names, &mut buf);
        }
        Ok(())
    }
}

/// Read a source, starting again from the column a cell turned up at where
/// it was left of the one the sheet said it started at.
fn retrying<T>(
    source: &XlsxSource,
    mut run: impl FnMut(&XlsxSource) -> Result<T, String>,
) -> Result<T, String> {
    for _ in 0..3 {
        match run(source) {
            Err(e) if e == LEFT_OF_IT => {
                let Some(col) = source.found_left.take() else {
                    return Err(e);
                };
                source
                    .left
                    .set(Some(col.min(source.left.get().unwrap_or(col))));
            }
            other => return other,
        }
    }
    Err("its cells are not where it says they are".to_string())
}

/// The whole of the sheet `sheet` of the workbook at `path`, written as a
/// Parquet file at `target`.
pub fn convert(path: &Path, sheet: Option<&str>, target: &Path) -> Result<(), String> {
    let source = XlsxSource {
        path,
        sheet,
        left: Slot::new(None),
        found_left: Slot::new(None),
    };
    retrying(&source, |s| copyframe::convert(s, target))
}

/// The first `rows` rows of the sheet, typed by the first `infer` of them,
/// written as a Parquet file at `target`: what is shown until the whole is.
pub fn sample(
    path: &Path,
    sheet: Option<&str>,
    infer: usize,
    rows: usize,
    target: &Path,
) -> Result<(), String> {
    let source = XlsxSource {
        path,
        sheet,
        left: Slot::new(None),
        found_left: Slot::new(None),
    };
    retrying(&source, |s| copyframe::sample(s, infer, rows, target))
}

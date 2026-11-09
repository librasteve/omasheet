// Copyright (c) 2026 Stephen Roe

//! A workbook with formulas, as tables (`open-a-big-csv` D154).
//!
//! A table is values only, so a workbook with a formula in it is imported
//! whole, where the formulas are kept, unless every formula is one that a
//! table can say: the same formula filled down every row of a column, as a
//! calculated column of a sheet is. Such a column is not read from the
//! workbook but written as a `:=` line over the table, as `Family` is in
//! `analysis.omx`, and the rest of the sheet is read as it is without
//! formulas.
//!
//! This reads each sheet once for its formulas, a cell at a time, and holds
//! none of them: it keeps, for each column, how many cells have a formula,
//! whether they are one formula (`formula::relative`, which is the formula
//! with each cell it names as how far it is from the row it is on), and the
//! first. A column is *filled* when every data row has the formula and they
//! are the same. A formula anywhere else, or one that cannot be translated,
//! means the workbook is imported and not read as tables, with the reason.
//!
//! It is also held to what the workbook has. The sheets are copied, the
//! document is compiled and run over the copies, and each calculated column
//! is compared with the values the workbook holds in its cells: all of them,
//! so a translation that differs in any row is found. A column Polars will
//! not calculate whole cannot be compared and is a reason not to.
//!
//! What it gives is the `.omx` text, `view` lines with each sheet's columns
//! declared, in the order of the workbook, and a `:=` line where a column
//! is calculated. The copies it made stay in the cache, so the document
//! opens on them at once.

use crate::formula::{self, Worksheet};
use crate::{Grid, names};
use calamine::{Xlsx, open_workbook};
use omasheet_engine::dtype::type_of;
use omasheet_engine::eval::Attempt;
use omasheet_engine::omx::compile;
use omasheet_engine::omx::types::S;
use omasheet_engine::{Engine, Lowering, Options, table};
use polars::prelude::*;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// What a column of a sheet has by way of formulas.
#[derive(Default)]
struct Fill {
    /// How many cells of the column have a formula.
    count: usize,
    /// Whether they are all one formula, filled down.
    same: bool,
    /// The first of them, its row and its text.
    first: Option<(u32, String)>,
    /// What the first looks like by `formula::relative`.
    key: Option<String>,
}

/// What a read of a sheet found.
struct Scan {
    /// The row of the sheet's names, and the column it starts at.
    top: u32,
    left: u32,
    fills: Vec<Fill>,
}

/// Reads the sheet `name` for its formulas, a cell at a time.
fn scan(path: &Path, name: &str) -> Result<Scan, String> {
    let mut book: Xlsx<BufReader<File>> =
        open_workbook(path).map_err(|e| format!("it is not a workbook: {e}"))?;
    let mut cells = book
        .worksheet_cells_reader(name)
        .map_err(|e| format!("worksheet `{name}` cannot be read: {e}"))?;
    let left = cells.dimensions().start.1;
    let mut top: Option<u32> = None;
    let mut fills: Vec<Fill> = Vec::new();
    loop {
        let next = cells
            .next_cell_with_formula()
            .map_err(|e| format!("worksheet `{name}` cannot be read: {e}"))?;
        let Some(cell) = next else {
            break;
        };
        let (row, col) = cell.pos;
        if col < left {
            return Err(format!(
                "worksheet `{name}` has a cell left of where it starts"
            ));
        }
        let top = *top.get_or_insert(row);
        let Some(text) = cell.formula.filter(|f| !f.trim().is_empty()) else {
            continue;
        };
        if row == top {
            return Err(format!(
                "worksheet `{name}` has a formula in its header row"
            ));
        }
        let c = (col - left) as usize;
        if fills.len() <= c {
            fills.resize_with(c + 1, Fill::default);
        }
        let fill = &mut fills[c];
        fill.count += 1;
        let key = formula::relative(&text, row);
        if fill.count == 1 {
            fill.same = key.is_some();
            fill.key = key;
            fill.first = Some((row, text));
        } else if fill.key != key || key.is_none() {
            fill.same = false;
        }
    }
    Ok(Scan {
        top: top.unwrap_or(0),
        left,
        fills,
    })
}

/// A sheet's copy: where it is, and the columns it has and what they hold.
struct Copy {
    path: PathBuf,
    /// The columns' names in the copy, and their types.
    columns: Vec<(String, S)>,
    rows: usize,
}

fn copy_of(source: &Path) -> Result<Copy, String> {
    let path = table::convert(source)?;
    let mut reader = ParquetReader::new(File::open(&path).map_err(|e| e.to_string())?);
    let schema = reader.schema().map_err(|e| e.to_string())?;
    let mut columns = Vec::new();
    for (name, field) in schema.iter() {
        let dtype = DataType::from_arrow_field(field);
        let Some(ty) = type_of(&dtype) else {
            return Err(format!("column `{name}` holds a type a sheet cannot show"));
        };
        columns.push((name.to_string(), ty));
    }
    let rows = reader.num_rows().map_err(|e| e.to_string())?;
    Ok(Copy {
        path,
        columns,
        rows,
    })
}

/// A column of the workbook: read from the copy, or calculated.
enum Col {
    Data {
        raw: String,
        ty: S,
    },
    /// Calculated: what the workbook holds is in the copy under `raw`, and
    /// `filled` is the formula for every row.
    Calculated {
        raw: String,
        filled: String,
    },
}

/// The `.omx` text that reads the workbook at `path` as tables, with the
/// columns that are one formula filled down calculated; or the reason it
/// cannot, in which case it is to be imported.
pub(crate) fn plan(path: &Path) -> Result<String, String> {
    let full = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let full = full.to_string_lossy().to_string();
    let sheets = omasheet_engine::xlsxcopy::sheets(path)?;
    let source = |sheet: &str| PathBuf::from(format!("{full}#{sheet}"));

    // What each sheet holds, as a copy; and where its formulas are.
    let mut copies = Vec::new();
    let mut scans = Vec::new();
    for sheet in &sheets {
        copies.push(copy_of(&source(sheet))?);
        scans.push(scan(path, sheet)?);
    }

    // Names, as an import gives them, from the names the copies hold.
    let grids: Vec<Grid> = sheets
        .iter()
        .zip(&copies)
        .map(|(sheet, copy)| Grid {
            name: sheet.clone(),
            headers: copy.columns.iter().map(|(n, _)| n.clone()).collect(),
            ..Grid::default()
        })
        .collect();
    let named = names(&grids, &mut Vec::new());
    let worksheets: Vec<Worksheet> = (0..sheets.len())
        .map(|g| Worksheet {
            excel: sheets[g].clone(),
            sheet: named[g].sheet.clone(),
            columns: named[g].columns.clone(),
            top: scans[g].top,
            left: scans[g].left,
            rows: copies[g].rows,
        })
        .collect();

    // Which columns are calculated, and what they are.
    let mut layout: Vec<Vec<Col>> = Vec::new();
    for (g, sheet) in sheets.iter().enumerate() {
        let (copy, scan) = (&copies[g], &scans[g]);
        if scan.fills.len() > copy.columns.len() {
            return Err(format!(
                "worksheet `{sheet}` has formulas right of its columns"
            ));
        }
        let mut columns = Vec::new();
        for (c, (raw, ty)) in copy.columns.iter().enumerate() {
            let fill = scan.fills.get(c).filter(|f| f.count > 0);
            let Some(fill) = fill else {
                columns.push(Col::Data {
                    raw: raw.clone(),
                    ty: *ty,
                });
                continue;
            };
            let name = &named[g].columns[c];
            // Every row, and the same formula in each: a column filled down.
            if fill.count != copy.rows || copy.rows < 2 || !fill.same {
                return Err(format!(
                    "`{sheet}.{name}` has formulas that are not one formula down every row"
                ));
            }
            let (row, text) = fill.first.as_ref().expect("a first formula");
            let translated = formula::translate(text, &worksheets, g, *row).map_err(|why| {
                format!("the formula of `{sheet}.{name}` has no translation: {why}")
            })?;
            if translated.volatile {
                return Err(format!(
                    "the formula of `{sheet}.{name}` gives something else each time it is worked out"
                ));
            }
            columns.push(Col::Calculated {
                raw: raw.clone(),
                filled: translated.filled,
            });
        }
        layout.push(columns);
    }

    // The document.
    let mut text = String::new();
    for (g, columns) in layout.iter().enumerate() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&format!(
            "view {} := dataframe[{}]\n\n",
            named[g].sheet,
            omasheet_engine::omx::lexer::quoted(&format!("{full}#{}", sheets[g]))
        ));
        let longest = named[g].columns.iter().map(String::len).max().unwrap_or(0);
        for (c, column) in columns.iter().enumerate() {
            let name = &named[g].columns[c];
            match column {
                Col::Data { raw, ty } if raw == name => {
                    text.push_str(&format!("{name:longest$} : {ty}\n"));
                }
                Col::Data { raw, ty } => text.push_str(&format!(
                    "{name:longest$} : {ty} from {}\n",
                    omasheet_engine::omx::lexer::quoted(raw)
                )),
                Col::Calculated { filled, .. } => {
                    text.push_str(&format!("{name:longest$} := {filled}\n"));
                }
            }
        }
    }

    // Held to what the workbook has.
    if layout
        .iter()
        .any(|columns| columns.iter().any(|c| matches!(c, Col::Calculated { .. })))
    {
        verify(&text, &named, &copies, &layout)?;
    }
    Ok(text)
}

/// Calculates every calculated column over the copies and compares it with
/// the values the workbook holds, row for row.
fn verify(
    text: &str,
    named: &[crate::Names],
    copies: &[Copy],
    layout: &[Vec<Col>],
) -> Result<(), String> {
    let (mut program, mut diags) = compile(text, 0);
    if let Some(d) = diags.first() {
        return Err(format!("its translation does not check: {}", d.message));
    }
    let tables = table::read(&mut program, Path::new("."), &mut diags);
    if let Some(d) = diags.first() {
        return Err(format!("its sheets do not read: {}", d.message));
    }
    let program = program;
    // Whole columns whatever their length, which is what is compared.
    let options = Options {
        lowering: Lowering::Always,
        ..Options::default()
    };
    let engine = Engine::with_tables(&program, options, tables);
    engine.run();
    if let Some(d) = engine.take_diags().first() {
        return Err(format!("its translation does not work: {}", d.message));
    }
    let attempts = engine.attempts();
    for (g, columns) in layout.iter().enumerate() {
        let Some(t) = program.sheets.iter().position(|s| s.name == named[g].sheet) else {
            return Err(format!("the sheet `{}` was not read", named[g].sheet));
        };
        for (c, column) in columns.iter().enumerate() {
            let Col::Calculated { raw, .. } = column else {
                continue;
            };
            let name = &named[g].columns[c];
            let lowered = attempts
                .iter()
                .any(|(at, col, attempt)| *at == t && *col == c && *attempt == Attempt::Lowered);
            let computed = engine.frame_column(t, c).filter(|_| lowered);
            let Some(computed) = computed else {
                return Err(format!(
                    "the formula of `{}.{name}` is not one Polars calculates over the whole column, \
                     so it cannot be held to what the workbook has",
                    named[g].sheet
                ));
            };
            let held = ParquetReader::new(File::open(&copies[g].path).map_err(|e| e.to_string())?)
                .with_columns(Some(vec![raw.clone()]))
                .finish()
                .map_err(|e| e.to_string())?;
            let held = held.column(raw).map_err(|e| e.to_string())?;
            if !agree(&computed, held) {
                return Err(format!(
                    "the formula of `{}.{name}` does not give what the workbook holds",
                    named[g].sheet
                ));
            }
        }
    }
    Ok(())
}

/// Whether a calculated column is what the workbook holds: numbers the same
/// as far as a workbook holds them, anything else the same text, a blank
/// the same as a blank.
fn agree(computed: &Column_, held: &Column_) -> bool {
    if computed.len() != held.len() {
        return false;
    }
    let numeric = |c: &Column_| c.dtype().is_primitive_numeric() || c.dtype().is_decimal();
    if numeric(computed) && numeric(held) {
        let (Ok(a), Ok(b)) = (
            computed.cast(&DataType::Float64),
            held.cast(&DataType::Float64),
        ) else {
            return false;
        };
        let (Ok(a), Ok(b)) = (a.f64(), b.f64()) else {
            return false;
        };
        return a.into_iter().zip(b).all(|pair| match pair {
            (Some(x), Some(y)) => (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1e-300),
            (None, None) => true,
            _ => false,
        });
    }
    let (Ok(a), Ok(b)) = (
        computed.cast(&DataType::String),
        held.cast(&DataType::String),
    ) else {
        return false;
    };
    let (Ok(a), Ok(b)) = (a.str(), b.str()) else {
        return false;
    };
    a.into_iter().zip(b).all(|pair| match pair {
        (Some(x), Some(y)) => x.trim() == y.trim(),
        (None, None) => true,
        (None, Some(y)) | (Some(y), None) => y.trim().is_empty(),
    })
}

/// Polars' column, which this module's own `Column` is not.
type Column_ = polars::prelude::Column;

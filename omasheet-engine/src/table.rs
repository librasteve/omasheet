// Copyright (c) 2026 Stephen Roe

//! The rows of a `table`: a sheet whose rows live in Parquet files.
//!
//! The checker gives such a sheet its columns, from the schema lines in the
//! text, and a [`Source`] saying what to read. This fills in the rest: the
//! row count from the file's footer, each column's cells, and a diagnostic
//! where the text and the file disagree about a column.

use crate::dtype::type_of;
use omasheet_omx::Program;
use omasheet_omx::check::{ColKind, Column, Source};
use omasheet_omx::diag::Diagnostic;
use omasheet_omx::file::ident_len;
use omasheet_omx::types::S;
use polars::prelude::Column as PlColumn;
use polars::prelude::*;
use std::fs::File;
use std::path::Path;

/// What a column of a `table` holds.
pub enum Held {
    /// A plan that reads this one column of the source, and nothing else of
    /// it. Nothing is read until it is collected, and the projection is why
    /// a column nothing asks for is never read from the file at all. Boxed
    /// because a plan is 288 bytes beside a column that has none.
    Column(Box<LazyFrame>),
    /// The source has no such column. The mismatch is said once, as a
    /// diagnostic; every cell of it is then in error rather than empty,
    /// which is what a column of errors standing in for it gave.
    Absent,
}

impl Held {
    /// The column itself, read now. `None` where the file will not give it,
    /// which is a file changed or taken away since its schema was read.
    pub fn read(&self) -> Option<PlColumn> {
        match self {
            Held::Column(frame) => frame.clone().collect().ok()?.get_columns().first().cloned(),
            Held::Absent => None,
        }
    }
}

/// Every sheet's columns that come from a source, by sheet and column. A
/// column the text itself holds has no entry: it is read from the cells it
/// is written in.
#[derive(Default)]
pub struct Tables {
    sheets: Vec<Vec<Option<Held>>>,
}

impl Tables {
    /// Room for the sheets a program has, so that the usual case sets a
    /// column without growing anything.
    pub fn for_program(prog: &Program) -> Tables {
        Tables {
            sheets: prog
                .sheets
                .iter()
                .map(|s| (0..s.cols.len()).map(|_| None).collect())
                .collect(),
        }
    }

    pub fn get(&self, t: usize, c: usize) -> Option<&Held> {
        self.sheets.get(t)?.get(c)?.as_ref()
    }

    /// Hold `held` for one column, making room for it where a table gained
    /// its columns from the file rather than from the text.
    pub fn set(&mut self, t: usize, c: usize, held: Held) {
        if self.sheets.len() <= t {
            self.sheets.resize_with(t + 1, Vec::new);
        }
        let cols = &mut self.sheets[t];
        if cols.len() <= c {
            cols.resize_with(c + 1, || None);
        }
        cols[c] = Some(held);
    }
}

/// Read every sheet that takes its rows from elsewhere: the schema it
/// declares is checked against the file's, its row count is fixed from the
/// file's footer, and each of its columns gets a plan that reads that
/// column alone. `dir` is the directory the file's own path is relative to,
/// so that a source is read from beside the `.omx` rather than from
/// wherever the command was run.
///
/// No row group is read here. What is read is the schema and the row count,
/// both of which Parquet holds in its footer, so that opening a file of a
/// million rows and twenty columns reads neither the rows nor the columns —
/// and a column no formula mentions is never read at all.
pub fn read(program: &mut Program, dir: &Path, diags: &mut Vec<Diagnostic>) -> Tables {
    let mut frames = Tables::for_program(program);
    for (t, sheet) in program.sheets.iter_mut().enumerate() {
        let Some(source) = sheet.source.clone() else {
            continue;
        };
        let path = dir.join(&source.path);
        match scan(&path).and_then(|frame| {
            let height = rows(dir, &source)?;
            Ok((frame, height))
        }) {
            Ok((frame, height)) => fill(t, sheet, frame, height, &source, diags, &mut frames),
            Err(why) => diags.push(Diagnostic::new(
                source.span,
                format!("cannot read `{}`: {why}", source.path),
            )),
        }
    }
    // A sheet of formulas alone is as long as the sheets it reads, which for
    // a table is only now known.
    omasheet_omx::check::resolve_rows(program);
    frames
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

/// A plan that reads a source's file, with nothing read yet.
fn scan(path: &Path) -> Result<LazyFrame, String> {
    // The file is opened first so that a missing one is said to be missing
    // in the words `open` gives it, rather than in Polars'.
    open(path)?;
    LazyFrame::scan_parquet(
        PlPath::from_str(&path.to_string_lossy()),
        Default::default(),
    )
    .map_err(|e| e.to_string())
}

/// The sheet's rows, and a diagnostic for each way the text and the file
/// disagree. The text is not corrected from the file and the file is not
/// read as the text says: a mismatch is reported and left (D29).
fn fill(
    t: usize,
    sheet: &mut omasheet_omx::check::Sheet,
    mut frame: LazyFrame,
    height: usize,
    source: &Source,
    diags: &mut Vec<Diagnostic>,
    frames: &mut Tables,
) {
    let Ok(schema) = frame.collect_schema() else {
        diags.push(Diagnostic::new(
            source.span,
            format!("cannot read the schema of `{}`", source.path),
        ));
        return;
    };
    let held: Vec<(String, S)> = schema
        .iter()
        .filter_map(|(name, dtype)| type_of(dtype).map(|ty| (name.to_string(), ty)))
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
                kind: ColKind::Frame,
                from: None,
                format: None,
                elementwise: false,
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

    let schema = schema.clone();
    let names: Vec<String> = sheet.cols.iter().map(|c| held_as(c).to_string()).collect();
    for (c, col) in sheet.cols.iter_mut().enumerate() {
        if !matches!(col.kind, ColKind::Frame) {
            continue;
        }
        let Some(dtype) = schema.get(names[c].as_str()) else {
            diags.push(Diagnostic::new(
                col.span,
                format!("the file has no column `{}`", names[c]),
            ));
            frames.set(t, c, Held::Absent);
            continue;
        };
        match type_of(dtype) {
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
                    "Parquet holds `{}` in a shape a sheet cannot render",
                    col.name
                ),
            )),
        }
        // A plan that reads this column and nothing else of the file.
        // Nothing is read until a cell is asked for, and a column nothing
        // asks for is never read at all.
        let one = frame
            .clone()
            .select([polars::prelude::col(names[c].as_str())]);
        frames.set(t, c, Held::Column(Box::new(one)));
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

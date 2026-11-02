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
use std::path::{Path, PathBuf};

/// What a column of a `table` holds.
pub enum Held {
    /// The file this column is in and the name the file gives it. Nothing
    /// is read until a cell of it is asked for, and then this column alone
    /// is read, so a column nothing asks for is never read from the file at
    /// all.
    ///
    /// `whole` is the column already in memory, where the document holds
    /// its file whole (D157): then nothing is read from the file at all.
    Column {
        path: PathBuf,
        name: String,
        whole: Option<Box<PlColumn>>,
    },
    /// The source has no such column. The mismatch is said once, as a
    /// diagnostic; every cell of it is then in error rather than empty,
    /// which is what a column of errors standing in for it gave.
    Absent,
}

impl Held {
    /// The column itself, read now. `None` where the file will not give it,
    /// which is a file changed or taken away since its schema was read.
    pub fn read(&self) -> Option<PlColumn> {
        self.read_rows(None)
    }

    /// `len` rows of the column from row `start`, read now. Polars' reader
    /// takes the slice to the file, so a row group that holds none of these
    /// rows is not read (D78): the first five rows of a file of many row
    /// groups are read out of the first.
    pub fn read_range(&self, start: usize, len: usize) -> Option<PlColumn> {
        self.read_rows(Some((start, len)))
    }

    fn read_rows(&self, slice: Option<(usize, usize)>) -> Option<PlColumn> {
        match self {
            // A slice of a column held is a view of its buffers, not a copy.
            Held::Column {
                whole: Some(column),
                ..
            } => Some(match slice {
                None => (**column).clone(),
                Some((start, len)) => column.slice(start as i64, len),
            }),
            Held::Column { path, name, .. } => ParquetReader::new(open(path).ok()?)
                .with_columns(Some(vec![name.clone()]))
                .with_slice(slice)
                .finish()
                .ok()?
                .get_columns()
                .first()
                .cloned(),
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
    read_holding(program, dir, diags, &Store::new(), Copying::Wait)
}

/// [`read`], where some files are held whole in `store` (D157): a column of
/// one of them, while the file is still as it was when it was read, is
/// given from memory rather than read again.
pub fn read_holding(
    program: &mut Program,
    dir: &Path,
    diags: &mut Vec<Diagnostic>,
    store: &Store,
    copying: Copying,
) -> Tables {
    let mut frames = Tables::for_program(program);
    for (t, sheet) in program.sheets.iter_mut().enumerate() {
        let Some(source) = sheet.source.clone() else {
            continue;
        };
        // A CSV is read from its Parquet copy, or its sample meanwhile.
        match located(&dir.join(&source.path), copying).and_then(|path| {
            let frame = scan(&path)?;
            let height = rows(&path)?;
            Ok((frame, height, path))
        }) {
            Ok((frame, height, path)) => {
                let whole = store
                    .get(&path)
                    .filter(|w| identity(&path).as_ref() == Some(&w.identity))
                    .map(|w| &w.frame);
                fill(
                    t,
                    sheet,
                    (frame, &path, whole),
                    height,
                    &source,
                    diags,
                    &mut frames,
                )
            }
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
pub fn rows(path: &Path) -> Result<usize, String> {
    let file = open(path)?;
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
    (mut frame, path, whole): (LazyFrame, &Path, Option<&DataFrame>),
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
                    "add `{}` to the table's schema, with `from {}` and the type the file will hold",
                    suggest(name),
                    omasheet_omx::lexer::quoted(name)
                ),
                (false, true) => format!("add `{name} : {ty}` to the table's schema"),
                (false, false) => format!(
                    "add `{} : {ty} from {}` to the table's schema",
                    suggest(name),
                    omasheet_omx::lexer::quoted(name)
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
        // Where this column is, and nothing read of it: a cell asked for
        // reads it, and a column nothing asks for is never read at all.
        frames.set(
            t,
            c,
            Held::Column {
                path: path.to_path_buf(),
                name: names[c].clone(),
                whole: whole.and_then(|f| f.column(names[c].as_str()).ok().cloned().map(Box::new)),
            },
        );
    }
    sheet.nrows = height;
}

/// What tells a file from the same file changed: its length and when it
/// was last written. A file held whole is given from memory only while
/// these are as they were when it was read.
pub type Identity = (u64, Option<std::time::SystemTime>);

/// The identity of the file at `path` now, or `None` where there is none.
pub fn identity(path: &Path) -> Option<Identity> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

/// A table's file read whole, with the identity it had when it was read.
#[derive(Clone)]
pub struct Whole {
    pub identity: Identity,
    pub frame: DataFrame,
}

/// The files a document holds whole, by the path its sources name them by
/// (D157).
pub type Store = std::collections::HashMap<PathBuf, Whole>;

/// What it would take to hold columns `names` of the Parquet file at
/// `path` in memory, in bytes, from its footer alone: no row group is read.
///
/// A column of fixed width takes its width a row, and a bit a row to say
/// which are null, as Arrow holds it, and that is exact. Any other — text,
/// mostly — takes what the footer says it is uncompressed, and 16 bytes a
/// row more, which is what a Polars string view costs. That errs high:
/// a file of short strings is said to need more than it does, and so is
/// left on disk where it might have fitted, which is the mistake to make.
pub fn estimate(path: &Path, names: &[String]) -> Result<usize, String> {
    let mut reader = ParquetReader::new(open(path)?);
    let schema = reader.schema().map_err(|e| e.to_string())?;
    let meta = reader.get_metadata().map_err(|e| e.to_string())?.clone();
    let rows = meta.num_rows;
    let mut bytes = 0usize;
    for name in names {
        let Some(field) = schema.get(name.as_str()) else {
            continue;
        };
        let dtype = DataType::from_arrow_field(field);
        let validity = rows.div_ceil(8);
        bytes += match fixed_width(&dtype) {
            Some(0) => 0,
            Some(width) => rows * width + validity,
            None => {
                let raw: i64 = meta
                    .row_groups
                    .iter()
                    .filter_map(|g| g.columns_under_root_iter(name))
                    .flatten()
                    .map(|c| c.uncompressed_size())
                    .sum();
                usize::try_from(raw).unwrap_or(0) + rows * 16 + validity
            }
        };
    }
    Ok(bytes)
}

/// The bytes a row of a column of `dtype` takes, where every row takes the
/// same; `None` where they do not.
fn fixed_width(dtype: &DataType) -> Option<usize> {
    Some(match dtype {
        DataType::Null => 0,
        // A bit a row, which the validity's share already counts.
        DataType::Boolean => 0,
        DataType::Int8 | DataType::UInt8 => 1,
        DataType::Int16 | DataType::UInt16 => 2,
        DataType::Int32 | DataType::UInt32 | DataType::Float32 | DataType::Date => 4,
        DataType::Int64
        | DataType::UInt64
        | DataType::Float64
        | DataType::Datetime(..)
        | DataType::Duration(_)
        | DataType::Time => 8,
        DataType::Int128 | DataType::Decimal(..) => 16,
        _ => return None,
    })
}

/// Columns `names` of the Parquet file at `path`, read whole, with the
/// identity the file had when reading began: what a document holds of a
/// table that fits (D157). A file changed while it was read has a new
/// identity by the time it is held, so what was read is never given out.
/// For a worker: nothing here touches a document.
pub fn read_whole(path: &Path, names: &[String]) -> Result<Whole, String> {
    let identity = identity(path).ok_or_else(|| "there is no such file".to_string())?;
    let frame = ParquetReader::new(open(path)?)
        .with_columns(Some(names.to_vec()))
        .finish()
        .map_err(|e| e.to_string())?;
    Ok(Whole { identity, frame })
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

// ---- A CSV, read through a Parquet copy (open-a-big-csv D153) ------------

/// How a source that is not Parquet is found: from its copy, waited for if
/// there is none yet, or from the sample of its first rows meanwhile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Copying {
    /// Write the copy now if there is none, and read that: the command line
    /// and the tests, which have no screen to show a sample on.
    Wait,
    /// Read the copy if there is one, and otherwise the sample, written now
    /// if need be: the app, which writes the copy on a worker.
    Sample,
}

/// How many rows the sample of a CSV holds, shown while its copy is written.
pub const SAMPLE_ROWS: usize = 1_000;

/// The rows a CSV's types are worked out from, for the sample and the copy
/// alike, so the two have the same columns.
const INFER_ROWS: usize = 10_000;

/// Whether `path` names a CSV, which a table reads through a copy.
pub fn is_csv(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"))
}

/// Where copies are kept: `$OMASHEET_CACHE`, or `omasheet` in
/// `$XDG_CACHE_HOME`, or in `~/.cache`.
pub fn cache_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("OMASHEET_CACHE") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("omasheet")
}

/// The stem every copy of the CSV at `path` is named with, whatever its
/// state: a hash of where it is.
fn family(path: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let full = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut h = std::collections::hash_map::DefaultHasher::new();
    full.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// The name of the copy of the CSV at `path` as it is now, by its length
/// and when it was written: a CSV changed has a copy of another name.
fn copy_name(path: &Path) -> Option<String> {
    let (len, modified) = identity(path)?;
    let nanos = modified
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Some(format!("{}-{len}-{nanos}", family(path)))
}

/// The copy of the CSV at `path`, if one has been written for it as it is
/// now.
pub fn copy_of(path: &Path) -> Option<PathBuf> {
    let copy = cache_dir().join(format!("{}.parquet", copy_name(path)?));
    copy.is_file().then_some(copy)
}

/// The file a source is read from: the source itself, unless it is a CSV,
/// and then its copy, or as `copying` says where there is none yet.
pub fn located(path: &Path, copying: Copying) -> Result<PathBuf, String> {
    if !is_csv(path) {
        return Ok(path.to_path_buf());
    }
    if let Some(copy) = copy_of(path) {
        return Ok(copy);
    }
    match copying {
        Copying::Wait => convert(path),
        Copying::Sample => sample(path),
    }
}

/// The head of the CSV at `path`: its header and the rows its types are
/// worked out from, as bytes.
fn csv_head(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::BufRead;
    let mut head = Vec::new();
    let mut reader = std::io::BufReader::new(open(path)?);
    for _ in 0..=INFER_ROWS {
        if reader.read_until(b'\n', &mut head).map_err(|e| e.to_string())? == 0 {
            break;
        }
    }
    Ok(head)
}

/// The types of the CSV whose head is `head`, as the sample and the copy
/// both read it. Polars works out the numbers and the text; a text column
/// every value of which in the head is an ISO date, or an ISO date and
/// time, is read as one. Polars' own `try_parse_dates` would do that too,
/// but takes half a second over the head of a file of seven columns.
fn csv_schema(head: &[u8]) -> Result<SchemaRef, String> {
    let frame = CsvReadOptions::default()
        .with_has_header(true)
        .with_infer_schema_length(Some(INFER_ROWS))
        .into_reader_with_file_handle(std::io::Cursor::new(head))
        .finish()
        .map_err(|e| e.to_string())?;
    let date = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    let datetime =
        regex::Regex::new(r"^\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}(:\d{2}(\.\d{1,9})?)?$").unwrap();
    let mut schema = (**frame.schema()).clone();
    for column in frame.get_columns() {
        let Ok(texts) = column.str() else {
            continue;
        };
        let every = |re: &regex::Regex| {
            texts.null_count() < texts.len() && texts.into_iter().flatten().all(|t| re.is_match(t))
        };
        let dtype = if every(&date) {
            DataType::Date
        } else if every(&datetime) {
            DataType::Datetime(TimeUnit::Microseconds, None)
        } else {
            continue;
        };
        schema.with_column(column.name().clone(), dtype);
    }
    Ok(std::sync::Arc::new(schema))
}

/// A reader of the CSV at `path`, with the types `schema` gives it, or
/// worked out from every row where there is none.
fn csv_scan(path: &Path, schema: Option<SchemaRef>) -> Result<LazyFrame, String> {
    open(path)?;
    let reader =
        LazyCsvReader::new(PlPath::from_str(&path.to_string_lossy())).with_has_header(true);
    match schema {
        Some(schema) => reader.with_schema(Some(schema)),
        None => reader.with_infer_schema_length(None).with_try_parse_dates(true),
    }
    .finish()
    .map_err(|e| e.to_string())
}

/// Write `frame` to `target` through a file beside it, renamed into place
/// once whole, so that a copy half written is never read.
fn sink_to(frame: LazyFrame, target: &Path) -> Result<(), String> {
    let dir = target.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let partial = target.with_extension(format!("partial-{}", std::process::id()));
    let written = frame
        .sink_parquet(
            SinkTarget::Path(PlPath::from_str(&partial.to_string_lossy())),
            ParquetWriteOptions::default(),
            None,
            SinkOptions::default(),
        )
        .and_then(|f| f.collect_with_engine(Engine::Streaming));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&partial);
        return Err(e.to_string());
    }
    std::fs::rename(&partial, target).map_err(|e| e.to_string())
}

/// The first [`SAMPLE_ROWS`] rows of the CSV at `path`, written as a small
/// Parquet file to read until the copy is ready: what a table over a big
/// CSV shows in the moment after it is opened.
pub fn sample(path: &Path) -> Result<PathBuf, String> {
    let name = copy_name(path).ok_or_else(|| "there is no such file".to_string())?;
    let target = cache_dir().join(format!("{name}.sample.parquet"));
    if !target.is_file() {
        // Only the head of the file is given to Polars: given the file, its
        // reader sets about the whole of it however few rows are asked for.
        let head = csv_head(path)?;
        let mut frame = CsvReadOptions::default()
            .with_has_header(true)
            .with_schema(Some(csv_schema(&head)?))
            .with_n_rows(Some(SAMPLE_ROWS))
            .into_reader_with_file_handle(std::io::Cursor::new(head))
            .finish()
            .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(cache_dir()).map_err(|e| e.to_string())?;
        let partial = target.with_extension(format!("partial-{}", std::process::id()));
        let file = File::create(&partial).map_err(|e| e.to_string())?;
        ParquetWriter::new(file).finish(&mut frame).map_err(|e| e.to_string())?;
        std::fs::rename(&partial, &target).map_err(|e| e.to_string())?;
    }
    Ok(target)
}

/// Write the copy of the CSV at `path`, streamed so that the CSV is never
/// held whole, and take away the copies and samples of it as it was
/// before. For a worker: nothing here touches a document. Where a value
/// further down does not have the type the first rows gave its column,
/// the types are worked out again from every row.
pub fn convert(path: &Path) -> Result<PathBuf, String> {
    let name = copy_name(path).ok_or_else(|| "there is no such file".to_string())?;
    let target = cache_dir().join(format!("{name}.parquet"));
    // What a copy cut short left behind: the app closed while writing it.
    let stem = family(path);
    if let Ok(entries) = std::fs::read_dir(cache_dir()) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            if file.starts_with(&stem) && file.contains(".partial-") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    // Where a value further down does not have the type the head gave its
    // column, the types are worked out again from every row.
    let schema = csv_schema(&csv_head(path)?)?;
    if sink_to(csv_scan(path, Some(schema))?, &target).is_err() {
        sink_to(csv_scan(path, None)?, &target)?;
    }
    // Earlier copies of this CSV, and its sample, are of no more use.
    if let Ok(entries) = std::fs::read_dir(cache_dir()) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            if file.starts_with(&stem) && !file.starts_with(&format!("{name}.parquet")) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    Ok(target)
}

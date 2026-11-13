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
        /// Where a column read whole is kept for the next rebuild (D150),
        /// when the document that holds this has one.
        cache: Option<ColumnCache>,
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
        let Held::Column {
            path,
            name,
            whole: None,
            cache: Some(cache),
        } = self
        else {
            return self.read_rows(None);
        };
        // Taken from the cache where the file is as it was when the column
        // was put there, and put there when it is read (D150).
        let identity = identity(path)?;
        if let Some(column) = cache.get(path, name, &identity) {
            return Some(column);
        }
        let column = self.read_rows(None)?;
        let bytes = estimate(path, std::slice::from_ref(name))
            .unwrap_or_else(|_| column.as_materialized_series().estimated_size());
        cache.put(path, name, identity, column.clone(), bytes);
        Some(column)
    }

    /// `len` rows of the column from row `start`, read now. Polars' reader
    /// takes the slice to the file, so a row group that holds none of these
    /// rows is not read (D78): the first five rows of a file of many row
    /// groups are read out of the first.
    pub fn read_range(&self, start: usize, len: usize) -> Option<PlColumn> {
        self.read_rows(Some((start, len)))
    }

    /// Where each row group of the file lies, as a start and a length, for
    /// a column that is to be read from it a group at a time (D155): `None`
    /// for a column held in memory, and for a file of one row group, which
    /// a slice would read whole every time.
    pub fn groups(&self) -> Option<Vec<(usize, usize)>> {
        let Held::Column {
            path, whole: None, ..
        } = self
        else {
            return None;
        };
        let mut reader = ParquetReader::new(open(path).ok()?);
        let meta = reader.get_metadata().ok()?;
        let mut start = 0;
        let groups: Vec<_> = meta
            .row_groups
            .iter()
            .map(|g| {
                let at = (start, g.num_rows());
                start += g.num_rows();
                at
            })
            .collect();
        (groups.len() >= 2).then_some(groups)
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
    read_holding(program, dir, diags, &Store::new(), Copying::Wait, None)
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
    cache: Option<&ColumnCache>,
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
                    (frame, &path, whole, cache),
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
    (mut frame, path, whole, cache): (LazyFrame, &Path, Option<&DataFrame>, Option<&ColumnCache>),
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
    // A copy marks the columns that are percentages: decimals, shown as such.
    let percents = percent_columns(path);
    // And the kind of each column of quantities, `Mass[g]`.
    let kinds = quantity_kinds(path);
    let held: Vec<(String, S)> = schema
        .iter()
        .filter_map(|(name, dtype)| {
            type_of(dtype).map(|ty| {
                let ty = if percents.iter().any(|p| p == name.as_str()) {
                    S::Percent
                } else if let Some(q) = kinds
                    .get(name.as_str())
                    .and_then(|k| crate::dtype::quantity_kind(dtype, k))
                {
                    q
                } else {
                    ty
                };
                (name.to_string(), ty)
            })
        })
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
                zone: None,
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

    // A column the file holds in a zone shows on that zone's clocks.
    let zones: std::collections::HashMap<String, String> = schema
        .iter()
        .filter_map(|(name, dtype)| match dtype {
            DataType::Datetime(_, Some(tz)) => Some((name.to_string(), tz.to_string())),
            _ => None,
        })
        .collect();
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
        if col.zone.is_none()
            && let Some(tz) = zones.get(names[c].as_str())
        {
            col.zone = Some((tz.clone(), col.span));
        }
        let kind = kinds.get(names[c].as_str());
        // A column of quantities has the unit its kind gives it.
        if let (Some(kind), S::Quantity(id)) = (kind, col.ty)
            && col.format.is_none()
            && crate::dtype::quantity_kind(dtype, kind).is_some()
            && let Some(unit) = crate::dtype::kind_unit(kind)
            && let Ok(f) = omasheet_omx::colfmt::QuantityFormat::parse(unit, id)
        {
            col.format = Some(omasheet_omx::colfmt::Format::Quantity(f));
        }
        let found = kind
            .and_then(|k| crate::dtype::quantity_kind(dtype, k))
            .or_else(|| type_of(dtype));
        match found {
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
                cache: cache.cloned(),
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
///
/// A copy in the cache has no time in it. Each read of a copy touches it, so
/// that the pruning knows which was used last, and a time that changes every
/// time it is read would make every copy a different file at the next look:
/// the columns kept of it, and a table held whole from it, would be taken
/// for another file's. What a copy is a copy of is in its name, which says
/// the length and the time of the file it was made from.
pub fn identity(path: &Path) -> Option<Identity> {
    let meta = std::fs::metadata(path).ok()?;
    if path.parent() == Some(cache_dir().as_path()) {
        return Some((meta.len(), None));
    }
    Some((meta.len(), meta.modified().ok()))
}

/// The columns a document has read whole, kept from one rebuild to the next
/// (open-a-big-csv D150). An edit rebuilds the engine, and the engine's own
/// columns go with it; this belongs to the document, so a column read for
/// an aggregate, a sort or a filter is not read again from the file after
/// the next keystroke.
///
/// Keyed by the file and the column's name in it, and each entry carries
/// the identity the file had when the column was read, so a file changed
/// since is a miss and its old entry is replaced rather than given out.
/// Entries are dropped least recently used first once the bytes they take
/// pass the budget, which is a quarter of the machine's memory unless the
/// caller says otherwise, and a column that is by itself over the budget is
/// not kept. A column read for a range is never put here: it is not the
/// column.
///
/// The size of an entry is what [`estimate`] says from the footer, and not
/// Polars' own figure, which counts little of a text column (D157).
#[derive(Clone)]
pub struct ColumnCache(std::sync::Arc<std::sync::Mutex<CacheInner>>);

struct CacheInner {
    entries: std::collections::HashMap<(PathBuf, String), CacheEntry>,
    bytes: usize,
    budget: usize,
    /// What tables held whole take, which the budget gives way to (D157).
    reserved: usize,
    /// What an aggregate over a whole column of a file came to, by the file,
    /// the column and the function: the answer, kept from one rebuild to the next so that
    /// an edit does not add up a column of twenty million rows again
    /// (D155). Each carries the identity the file had, and is of no use to
    /// a file that is another now.
    answers: std::collections::HashMap<(PathBuf, String, String), Answer>,
    answer_hits: usize,
    tick: u64,
    hits: usize,
    misses: usize,
}

impl CacheInner {
    /// What the entries may take now: the budget less what is held whole.
    fn allowed(&self) -> usize {
        self.budget.saturating_sub(self.reserved)
    }

    /// Drop the least recently used until what is kept is allowed.
    fn trim(&mut self) {
        while self.bytes > self.allowed() {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some(gone) = self.entries.remove(&oldest) {
                self.bytes -= gone.bytes;
            }
        }
    }
}

struct Answer {
    identity: Identity,
    value: crate::eval::Scalar,
}

struct CacheEntry {
    identity: Identity,
    column: PlColumn,
    bytes: usize,
    used: u64,
}

/// What a cache has done and holds, for a test and for the status.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct CacheStats {
    /// What the entries may take now, after what is held whole.
    pub budget: usize,
    pub hits: usize,
    pub misses: usize,
    pub entries: usize,
    pub bytes: usize,
    /// How many aggregates are kept, and how many times one was given again
    /// instead of being added up.
    pub answers: usize,
    pub answer_hits: usize,
}

impl Default for ColumnCache {
    fn default() -> Self {
        ColumnCache::with_budget(physical_memory() / 4)
    }
}

impl ColumnCache {
    /// A cache that keeps at most `budget` bytes.
    pub fn with_budget(budget: usize) -> ColumnCache {
        ColumnCache(std::sync::Arc::new(std::sync::Mutex::new(CacheInner {
            entries: Default::default(),
            bytes: 0,
            budget,
            reserved: 0,
            answers: Default::default(),
            answer_hits: 0,
            tick: 0,
            hits: 0,
            misses: 0,
        })))
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, CacheInner> {
        // Nothing in here can be left half done by a panic.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The column, if it was kept and the file is as it was. A miss is
    /// counted here, because it is followed by the read it stands for.
    fn get(&self, path: &Path, name: &str, identity: &Identity) -> Option<PlColumn> {
        let mut inner = self.inner();
        inner.tick += 1;
        let tick = inner.tick;
        let key = (path.to_path_buf(), name.to_string());
        match inner.entries.get_mut(&key) {
            Some(entry) if &entry.identity == identity => {
                entry.used = tick;
                let column = entry.column.clone();
                inner.hits += 1;
                Some(column)
            }
            Some(_) => {
                // Changed since: of no use to anyone.
                if let Some(old) = inner.entries.remove(&key) {
                    inner.bytes -= old.bytes;
                }
                inner.misses += 1;
                None
            }
            None => {
                inner.misses += 1;
                None
            }
        }
    }

    /// Keep a column read whole, and drop the least recently used until
    /// what is kept is within the budget.
    fn put(&self, path: &Path, name: &str, identity: Identity, column: PlColumn, bytes: usize) {
        let mut inner = self.inner();
        if bytes > inner.allowed() {
            return;
        }
        inner.tick += 1;
        let used = inner.tick;
        let key = (path.to_path_buf(), name.to_string());
        if let Some(old) = inner.entries.insert(
            key,
            CacheEntry {
                identity,
                column,
                bytes,
                used,
            },
        ) {
            inner.bytes -= old.bytes;
        }
        inner.bytes += bytes;
        inner.trim();
    }

    /// Tell the cache what tables held whole take. Its budget gives way to
    /// that, and what no longer fits is dropped, least recently used first.
    pub fn reserve(&self, bytes: usize) {
        let mut inner = self.inner();
        inner.reserved = bytes;
        inner.trim();
    }

    /// Drop every column of the file at `path`: the file is held whole, and
    /// a column kept here as well would be the same data twice.
    pub fn evict_file(&self, path: &Path) {
        let mut inner = self.inner();
        let gone: Vec<_> = inner
            .entries
            .keys()
            .filter(|(p, _)| p == path)
            .cloned()
            .collect();
        for key in gone {
            if let Some(entry) = inner.entries.remove(&key) {
                inner.bytes -= entry.bytes;
            }
        }
    }

    /// Whether the column is kept, as the file is. Not a use of it.
    pub(crate) fn holds(&self, path: &Path, name: &str, identity: &Identity) -> bool {
        let inner = self.inner();
        let key = (path.to_path_buf(), name.to_string());
        inner
            .entries
            .get(&key)
            .is_some_and(|e| &e.identity == identity)
    }

    /// Let go of the column, if it is kept.
    pub(crate) fn drop_column(&self, path: &Path, name: &str) {
        let mut inner = self.inner();
        if let Some(entry) = inner
            .entries
            .remove(&(path.to_path_buf(), name.to_string()))
        {
            inner.bytes -= entry.bytes;
        }
    }

    /// What an aggregate over the column came to, where it was kept and the
    /// file is as it was.
    pub(crate) fn answer(
        &self,
        path: &Path,
        name: &str,
        key: &str,
        identity: &Identity,
    ) -> Option<crate::eval::Scalar> {
        let mut inner = self.inner();
        let kept = inner
            .answers
            .get(&(path.to_path_buf(), name.to_string(), key.to_string()))?;
        let value = (&kept.identity == identity).then(|| kept.value.clone())?;
        inner.answer_hits += 1;
        Some(value)
    }

    /// Keep what an aggregate over the column came to.
    pub(crate) fn put_answer(
        &self,
        path: &Path,
        name: &str,
        key: &str,
        identity: Identity,
        value: crate::eval::Scalar,
    ) {
        let mut inner = self.inner();
        // Each is a few bytes, but a file that is written again is another
        // copy with answers of its own: not for ever.
        if inner.answers.len() >= MOST_ANSWERS {
            inner.answers.clear();
        }
        inner.answers.insert(
            (path.to_path_buf(), name.to_string(), key.to_string()),
            Answer { identity, value },
        );
    }

    pub fn stats(&self) -> CacheStats {
        let inner = self.inner();
        CacheStats {
            budget: inner.allowed(),
            hits: inner.hits,
            misses: inner.misses,
            entries: inner.entries.len(),
            bytes: inner.bytes,
            answers: inner.answers.len(),
            answer_hits: inner.answer_hits,
        }
    }
}

/// The most aggregates a document keeps the answers of.
const MOST_ANSWERS: usize = 4096;

/// The machine's physical memory in bytes, or 4 GiB where it cannot be asked.
fn physical_memory() -> usize {
    #[cfg(unix)]
    {
        let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
        let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if pages > 0 && size > 0 {
            return pages as usize * size as usize;
        }
    }
    4 << 30
}

/// A table's file read whole, with the identity it had when it was read.
#[derive(Clone)]
pub struct Whole {
    pub identity: Identity,
    pub frame: DataFrame,
    /// What holding it takes, as [`estimate`] says from the footer, which is
    /// what the cache's budget gives way to (D150, D157).
    pub bytes: usize,
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
    let bytes = estimate(path, names).unwrap_or_else(|_| frame.estimated_size());
    Ok(Whole {
        identity,
        frame,
        bytes,
    })
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

/// The size above which a CSV or Parquet file is opened as a table over the
/// file rather than imported as text (open-a-big-csv D146): the app opens it
/// so, and `omasheet import` refuses it and prints the line that does.
pub const BIG_FILE: u64 = 4 * 1024 * 1024;

/// How many rows the sample of a CSV holds, shown while its copy is written.
pub const SAMPLE_ROWS: usize = 1_000;

/// The rows a CSV's types are worked out from, for the sample and the copy
/// alike, so the two have the same columns.
const INFER_ROWS: usize = 10_000;

/// What a source that a table reads through a copy is made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Csv,
    Json,
    Xlsx,
}

/// The kind a file is, by its extension.
fn kind_of_file(file: &Path) -> Option<Kind> {
    let ext = file.extension()?;
    if ext.eq_ignore_ascii_case("csv") {
        Some(Kind::Csv)
    } else if ext.eq_ignore_ascii_case("json") {
        Some(Kind::Json)
    } else if ext.eq_ignore_ascii_case("xlsx") || ext.eq_ignore_ascii_case("xlsm") {
        Some(Kind::Xlsx)
    } else {
        None
    }
}

/// A source's file, and the sheet of it that is named: `book.json#Sales` is
/// the sheet `Sales` of `book.json` (open-a-big-csv, `file#Sheet`). Only
/// where what is before the last `#` is a file of a kind with sheets, and
/// there is no file of the whole name, so a file called `a#b.json` is
/// itself.
pub fn split_sheet(path: &Path) -> (PathBuf, Option<String>) {
    if !path.exists() {
        let text = path.to_string_lossy();
        if let Some(at) = text.rfind('#') {
            let file = PathBuf::from(&text[..at]);
            let sheet = &text[at + 1..];
            if !sheet.is_empty() && kind_of_file(&file).is_some() {
                return (file, Some(sheet.to_string()));
            }
        }
    }
    (path.to_path_buf(), None)
}

/// What a source is, where it is one a table reads through a copy.
pub fn kind(path: &Path) -> Option<Kind> {
    kind_of_file(&split_sheet(path).0)
}

/// Whether `path` names a CSV, which a table reads through a copy.
pub fn is_csv(path: &Path) -> bool {
    kind(path) == Some(Kind::Csv)
}

/// Whether `path` names a source a table reads through a copy: a CSV, a JSON
/// file or an XLSX workbook.
pub fn is_converted(path: &Path) -> bool {
    kind(path).is_some()
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
    let (file, sheet) = split_sheet(path);
    let full = std::fs::canonicalize(&file).unwrap_or(file);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    full.hash(&mut h);
    sheet.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// The name of the copy of the CSV at `path` as it is now, by its length
/// and when it was written: a CSV changed has a copy of another name.
fn copy_name(path: &Path) -> Option<String> {
    let (len, modified) = identity(&split_sheet(path).0)?;
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
    if !is_converted(path) {
        return Ok(path.to_path_buf());
    }
    if let Some(copy) = copy_of(path) {
        touch(&copy);
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
        if reader
            .read_until(b'\n', &mut head)
            .map_err(|e| e.to_string())?
            == 0
        {
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
fn csv_schema(head: &[u8]) -> Result<CsvTypes, String> {
    let frame = CsvReadOptions::default()
        .with_has_header(true)
        .with_infer_schema_length(Some(INFER_ROWS))
        .into_reader_with_file_handle(std::io::Cursor::new(head))
        .finish()
        .map_err(|e| e.to_string())?;
    // The same head, every column as the text it is, for the columns that
    // Polars made floats of: an import reads `1.50` as an exact decimal, and
    // so does a table (open-a-big-csv 6.2).
    let texts = CsvReadOptions::default()
        .with_has_header(true)
        .with_infer_schema_length(Some(0))
        .into_reader_with_file_handle(std::io::Cursor::new(head))
        .finish()
        .map_err(|e| e.to_string())?;
    let date = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    let datetime =
        regex::Regex::new(r"^\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}(:\d{2}(\.\d{1,9})?)?$").unwrap();
    let plain = regex::Regex::new(r"^-?\d+(\.(\d+))?$").unwrap();
    let mut schema = (**frame.schema()).clone();
    let mut decimals = Vec::new();
    let mut hex = Vec::new();
    let mut percents = Vec::new();
    let mut literals = Vec::new();
    let time = regex::Regex::new(TIME).unwrap();
    let duration = regex::Regex::new(DURATION).unwrap();
    let underscored = regex::Regex::new(UNDERSCORED).unwrap();
    let percent = regex::Regex::new(r"^-?\d+(\.(\d+))?%$").unwrap();
    for column in frame.get_columns() {
        if matches!(column.dtype(), DataType::Float64)
            && let Ok(words) = texts
                .column(column.name().as_str())
                .and_then(|c| c.str().cloned())
        {
            // Every value a plain number, and the most places after the
            // point is the scale. Anything else (an exponent, `NaN`) is a
            // float, as it is to Polars.
            let (mut scale, mut all) = (0usize, true);
            for word in words.into_iter().flatten() {
                match plain.captures(word) {
                    Some(c) => scale = scale.max(c.get(2).map_or(0, |m| m.len())),
                    None => {
                        all = false;
                        break;
                    }
                }
            }
            if all && (1..=MOST_PLACES).contains(&scale) {
                schema.with_column(column.name().clone(), DataType::String);
                decimals.push((column.name().clone(), scale));
            }
            continue;
        }
        // Polars reads `TRUE` as a boolean, and an import reads it as text:
        // only `true` and `false` are booleans.
        if matches!(column.dtype(), DataType::Boolean)
            && let Ok(words) = texts
                .column(column.name().as_str())
                .and_then(|c| c.str().cloned())
            && !words
                .into_iter()
                .flatten()
                .all(|w| w == "true" || w == "false")
        {
            schema.with_column(column.name().clone(), DataType::String);
            continue;
        }
        let Ok(texts) = column.str() else {
            continue;
        };
        // A column of nothing but percentages (`7.5%`) is the decimals they
        // are (`0.075`), marked in the file so that it is shown as they are.
        if texts.null_count() < texts.len() {
            let mut places = Some(0usize);
            for word in texts.into_iter().flatten() {
                match percent.captures(word) {
                    Some(c) => {
                        places = places.map(|p| p.max(c.get(2).map_or(0, |m| m.len())));
                    }
                    None => {
                        places = None;
                        break;
                    }
                }
            }
            if let Some(places) = places
                && places + 2 <= MOST_PLACES
            {
                percents.push((column.name().clone(), places));
                continue;
            }
        }
        // A column of nothing but hexadecimal numbers is shown in lower
        // case, as an import shows them.
        if texts.null_count() < texts.len() && texts.into_iter().flatten().all(is_hex) {
            hex.push(column.name().clone());
            continue;
        }
        let every = |re: &regex::Regex| {
            texts.null_count() < texts.len() && texts.into_iter().flatten().all(|t| re.is_match(t))
        };
        for (re, literal) in [
            (&time, Literal::Time),
            (&duration, Literal::Duration),
            (&underscored, Literal::Underscored),
        ] {
            // Some of an underscored column's values have none: a column
            // with no underscore at all was whole numbers to Polars.
            let has = |t: &str| !matches!(literal, Literal::Underscored) || t.contains('_');
            if every(re) && texts.into_iter().flatten().any(has) {
                literals.push((column.name().clone(), literal));
            }
        }
        if literals.last().is_some_and(|(n, _)| n == column.name()) {
            continue;
        }
        let dtype = if every(&date) {
            DataType::Date
        } else if every(&datetime) {
            DataType::Datetime(TimeUnit::Microseconds, None)
        } else {
            continue;
        };
        schema.with_column(column.name().clone(), dtype);
    }
    Ok(CsvTypes {
        schema: std::sync::Arc::new(schema),
        decimals,
        percents,
        hex,
        literals,
    })
}

/// Whether `s` is a hexadecimal number as an import reads one: `0x1F`.
fn is_hex(s: &str) -> bool {
    s.strip_prefix("0x")
        .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// What the head of a CSV says its columns are, beyond what Polars infers:
/// the schema to read it with, the columns made decimals, the ones that are
/// percentages (made decimals too, and marked), and the hexadecimal ones.
struct CsvTypes {
    schema: SchemaRef,
    decimals: Decimals,
    percents: Decimals,
    hex: Vec<PlSmallStr>,
    literals: Vec<(PlSmallStr, Literal)>,
}

/// The other literals an import reads in a CSV cell, which a column made of
/// them is converted to: a time of day, an ISO duration, a whole number with
/// underscores (`1_000`).
#[derive(Clone, Copy)]
enum Literal {
    Time,
    Duration,
    Underscored,
}

/// An ISO 8601 duration as an import reads one, in days, hours, minutes and
/// seconds, with at least one of them.
const DURATION: &str = r"^P(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+(?:\.\d+)?)S)?)?$";

/// A time of day, `10:30`, `10:30:15` or `10:30:15.5`.
const TIME: &str = r"^([01]\d|2[0-3]):[0-5]\d(:[0-5]\d(\.\d{1,9})?)?$";

/// A whole number written with underscores between digits.
const UNDERSCORED: &str = r"^-?\d+(_\d+)*$";

/// The key of the file-level metadata that names, one to a line, the columns
/// of a copy that are percentages: a table shows them as such.
const PERCENT_KEY: &str = "omasheet.percent";

/// The columns of the copy at `path` that are marked as percentages.
fn percent_columns(path: &Path) -> Vec<String> {
    let Ok(file) = open(path) else {
        return Vec::new();
    };
    let mut reader = ParquetReader::new(file);
    let Ok(meta) = reader.get_metadata() else {
        return Vec::new();
    };
    meta.key_value_metadata
        .iter()
        .flatten()
        .find(|kv| kv.key == PERCENT_KEY)
        .and_then(|kv| kv.value.clone())
        .map(|v| v.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// The key prefix of the file-level metadata that gives the kind of a column
/// of quantities, by column: `omasheet.kind.Weight` is `Mass[g]`.
const KIND_PREFIX: &str = "omasheet.kind.";

/// The kind of each column of the file at `path` that has one.
fn quantity_kinds(path: &Path) -> std::collections::HashMap<String, String> {
    let Ok(file) = open(path) else {
        return Default::default();
    };
    let mut reader = ParquetReader::new(file);
    let Ok(meta) = reader.get_metadata() else {
        return Default::default();
    };
    meta.key_value_metadata
        .iter()
        .flatten()
        .filter_map(|kv| {
            Some((
                kv.key.strip_prefix(KIND_PREFIX)?.to_string(),
                kv.value.clone()?,
            ))
        })
        .collect()
}

/// The metadata that marks `percents` as percentages, for the writer.
fn percent_marks(percents: &Decimals) -> Option<KeyValueMetadata> {
    (!percents.is_empty()).then(|| {
        let names: Vec<&str> = percents.iter().map(|(n, _)| n.as_str()).collect();
        KeyValueMetadata::from_static(vec![(PERCENT_KEY.to_string(), names.join("\n"))])
    })
}

/// The most places after the point a column read as exact decimals has.
const MOST_PLACES: usize = 18;

/// The columns read as text and made decimals, by name, with their scale.
type Decimals = Vec<(PlSmallStr, usize)>;

/// The expression that makes the text column `name` the literal it is. A value
/// that is not one fails the conversion, and so the copy, which is worked out
/// again from every row, as text.
fn literal_column(name: &PlSmallStr, literal: Literal) -> Expr {
    let text = col(name.clone());
    match literal {
        Literal::Time => {
            // `10:30` has no seconds; they are put in, and the time read.
            let full = text
                .str()
                .replace(lit(r"^(\d{2}:\d{2})$"), lit("${1}:00"), false);
            full.str()
                .to_time(StrptimeOptions {
                    format: Some("%H:%M:%S%.f".into()),
                    strict: true,
                    ..Default::default()
                })
                .alias(name.clone())
        }
        Literal::Underscored => text
            .str()
            .replace_all(lit("_"), lit(""), true)
            .cast(DataType::Int64)
            .alias(name.clone()),
        Literal::Duration => {
            let part = |group: usize| {
                text.clone()
                    .str()
                    .extract(lit(DURATION), group)
                    .cast(DataType::Float64)
                    .fill_null(lit(0.0))
            };
            // A value that is not a duration, or has no component (`P`,
            // `PT`), fails the conversion: a column of `0`s and `x`s read
            // strictly as numbers, added in as the `0.0` it is.
            let valid = text
                .clone()
                .str()
                .contains(lit(DURATION), false)
                .and(text.clone().str().contains(lit(r"^PT?$"), false).not())
                .fill_null(lit(false));
            let guard = when(valid)
                .then(lit("0"))
                .otherwise(lit("x"))
                .strict_cast(DataType::Float64);
            let seconds = part(1) * lit(86_400.0)
                + part(2) * lit(3_600.0)
                + part(3) * lit(60.0)
                + part(4)
                + guard;
            // Half a microsecond up, then whole: durations here are not
            // negative.
            (seconds * lit(1_000_000.0) + lit(0.5))
                .cast(DataType::Int64)
                .cast(DataType::Duration(TimeUnit::Microseconds))
                .alias(name.clone())
        }
    }
}

/// `frame` with each of `decimals` made the exact decimal its text is. A
/// value with more places than the scale the head gave is turned into text
/// that is not a number, so that the conversion fails and the file is
/// worked out again from every row, rather than the digits being dropped.
fn decimal_columns(frame: LazyFrame, types: &CsvTypes) -> LazyFrame {
    let (decimals, hex) = (&types.decimals, &types.hex);
    // Only a value that is a hexadecimal number is changed, so a text further
    // down the column is left as it is.
    let lowered = hex.iter().map(|name| {
        let text = col(name.clone());
        when(
            text.clone()
                .str()
                .contains(lit(r"^0x[0-9a-fA-F]+$"), true)
                .fill_null(lit(false)),
        )
        .then(text.clone().str().to_lowercase())
        .otherwise(text)
        .alias(name.clone())
    });
    let exprs: Vec<Expr> = lowered
        .chain(
            types
                .literals
                .iter()
                .map(|(name, literal)| literal_column(name, *literal)),
        )
        .chain(types.percents.iter().map(|(name, places)| {
            // The text without its `%`, the decimal it is, and a hundredth of
            // that: exact, at two places more.
            let text = col(name.clone());
            let fits = text
                .clone()
                .str()
                .contains(lit(format!(r"^-?\d+(\.\d{{1,{places}}})?%$")), true)
                .fill_null(lit(true));
            (when(fits)
                .then(text.str().replace(lit("%"), lit(""), true))
                .otherwise(lit("not a number"))
                .str()
                .to_decimal(*places)
                .cast(DataType::Decimal(Some(38), Some(places + 4)))
                / lit(100i64).cast(DataType::Decimal(Some(38), Some(0))))
            .cast(DataType::Decimal(Some(38), Some(places + 2)))
            .alias(name.clone())
        }))
        .chain(decimals.iter().map(|(name, scale)| {
            let text = col(name.clone());
            let fits = text
                .clone()
                .str()
                .contains(lit(format!(r"^-?\d+(\.\d{{1,{scale}}})?$")), true)
                .fill_null(lit(true));
            when(fits)
                .then(text)
                .otherwise(lit("not a decimal"))
                .str()
                .to_decimal(*scale)
                .alias(name.clone())
        }))
        .collect();
    if exprs.is_empty() {
        frame
    } else {
        frame.with_columns(exprs)
    }
}

/// A reader of the CSV at `path`, with the types `schema` gives it, or
/// worked out from every row where there is none.
fn csv_scan(path: &Path, schema: Option<SchemaRef>) -> Result<LazyFrame, String> {
    open(path)?;
    let reader =
        LazyCsvReader::new(PlPath::from_str(&path.to_string_lossy())).with_has_header(true);
    match schema {
        Some(schema) => reader.with_schema(Some(schema)),
        None => reader
            .with_infer_schema_length(None)
            .with_try_parse_dates(true),
    }
    .finish()
    .map_err(|e| e.to_string())
}

/// Write `frame` to `target` through a file beside it, renamed into place
/// once whole, so that a copy half written is never read.
fn sink_to(frame: LazyFrame, target: &Path, marks: Option<KeyValueMetadata>) -> Result<(), String> {
    let dir = target.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let partial = target.with_extension(format!("partial-{}", std::process::id()));
    let written = frame
        .sink_parquet(
            SinkTarget::Path(PlPath::from_str(&partial.to_string_lossy())),
            ParquetWriteOptions {
                key_value_metadata: marks,
                ..Default::default()
            },
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
    let (file, sheet) = split_sheet(path);
    if !target.is_file() && matches!(kind(path), Some(Kind::Json | Kind::Xlsx)) {
        std::fs::create_dir_all(cache_dir()).map_err(|e| e.to_string())?;
        let partial = target.with_extension(format!("partial-{}", std::process::id()));
        let sampled = match kind(path) {
            Some(Kind::Xlsx) => {
                crate::xlsxcopy::sample(&file, sheet.as_deref(), INFER_ROWS, SAMPLE_ROWS, &partial)
            }
            _ => {
                crate::jsoncopy::sample(&file, sheet.as_deref(), INFER_ROWS, SAMPLE_ROWS, &partial)
            }
        };
        if let Err(e) = sampled {
            let _ = std::fs::remove_file(&partial);
            return Err(e);
        }
        std::fs::rename(&partial, &target).map_err(|e| e.to_string())?;
    } else if !target.is_file() {
        // Only the head of the file is given to Polars: given the file, its
        // reader sets about the whole of it however few rows are asked for.
        let head = csv_head(&file)?;
        let types = csv_schema(&head)?;
        let frame = CsvReadOptions::default()
            .with_has_header(true)
            .with_schema(Some(types.schema.clone()))
            .with_n_rows(Some(SAMPLE_ROWS))
            .into_reader_with_file_handle(std::io::Cursor::new(head))
            .finish()
            .map_err(|e| e.to_string())?;
        let mut frame = decimal_columns(frame.lazy(), &types)
            .collect()
            .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(cache_dir()).map_err(|e| e.to_string())?;
        let partial = target.with_extension(format!("partial-{}", std::process::id()));
        let file = File::create(&partial).map_err(|e| e.to_string())?;
        ParquetWriter::new(file)
            .with_key_value_metadata(percent_marks(&types.percents))
            .finish(&mut frame)
            .map_err(|e| e.to_string())?;
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
    // Where the child could not be started, the copy is made here.
    if let Some(program) = COPIER.get()
        && let Some(done) = convert_in_child(program, path)
    {
        return done;
    }
    convert_here(path)
}

/// The program that makes copies in a process of its own, where one is set
/// (`open-a-big-csv` 5.4). Polars' streaming engine keeps what it grew to
/// while writing a copy, about 460 MiB, for as long as its process lives; a
/// child gives it back to the machine as it exits.
static COPIER: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// The variable that makes a process a copier, holding the source to copy.
pub const COPY_CHILD: &str = "OMASHEET_COPY_CHILD";

/// Threads a child copies with: two keep it near 230 MB, for about 3.7 s a
/// million rows, where eight take about 500 MB.
const CHILD_THREADS: &str = "2";

/// From now on copies are made by running this program, as a child with
/// [`COPY_CHILD`] set; it must call [`run_child`] first thing. Called by
/// the app and the command line, not by tests or libraries, which copy in
/// their own process.
pub fn copy_in_children() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = COPIER.set(exe);
    }
}

/// `Some` if this process was started to make a copy: it makes it, says
/// where it is on stdout or why not on stderr, and the exit code is for
/// the caller to return.
pub fn run_child() -> Option<i32> {
    let source = std::env::var_os(COPY_CHILD)?;
    Some(match convert_here(Path::new(&source)) {
        Ok(target) => {
            println!("{}", target.display());
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    })
}

/// `None` if the child could not be started.
fn convert_in_child(program: &Path, path: &Path) -> Option<Result<PathBuf, String>> {
    let out = std::process::Command::new(program)
        .env(COPY_CHILD, path)
        .env("POLARS_MAX_THREADS", CHILD_THREADS)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if out.status.success() {
        let text = String::from_utf8_lossy(&out.stdout);
        return Some(match text.lines().last() {
            Some(line) if !line.is_empty() => Ok(PathBuf::from(line)),
            _ => Err("the copy left no file".to_string()),
        });
    }
    let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Some(Err(if why.is_empty() {
        "the copy stopped".to_string()
    } else {
        why
    }))
}

/// [`convert`], in this process.
pub fn convert_here(path: &Path) -> Result<PathBuf, String> {
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
    let (file, sheet) = split_sheet(path);
    match kind(path) {
        Some(kind @ (Kind::Json | Kind::Xlsx)) => {
            std::fs::create_dir_all(cache_dir()).map_err(|e| e.to_string())?;
            let partial = target.with_extension(format!("partial-{}", std::process::id()));
            let made = match kind {
                Kind::Xlsx => crate::xlsxcopy::convert(&file, sheet.as_deref(), &partial),
                _ => crate::jsoncopy::convert(&file, sheet.as_deref(), &partial),
            };
            if let Err(e) = made {
                let _ = std::fs::remove_file(&partial);
                return Err(e);
            }
            std::fs::rename(&partial, &target).map_err(|e| e.to_string())?;
        }
        _ => {
            // Where a value further down does not have the type the head
            // gave its column, the types are worked out again from every
            // row.
            let types = csv_schema(&csv_head(&file)?)?;
            let marks = percent_marks(&types.percents);
            let typed = csv_scan(&file, Some(types.schema.clone()))?;
            if sink_to(decimal_columns(typed, &types), &target, marks).is_err() {
                sink_to(csv_scan(&file, None)?, &target, None)?;
            }
        }
    }
    // The cache kept within its budget, the copy just written aside.
    prune(&cache_dir(), cache_budget(), &target);
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

/// What the cache of copies may take: 10 GB, or a tenth of the space free
/// on its disk where that is less (open-a-big-csv D153).
pub fn cache_budget() -> u64 {
    const MOST: u64 = 10 * 1_000_000_000;
    match free_space(&cache_dir()) {
        Some(free) => MOST.min(free / 10),
        None => MOST,
    }
}

/// The bytes free to a user on the disk that holds `dir`, or the nearest
/// folder above it that there is.
#[cfg(unix)]
fn free_space(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let dir = dir.ancestors().find(|d| d.exists())?;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is a NUL-terminated string and `stat` is ours to fill.
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    Some(stat.f_bavail as u64 * stat.f_frsize as u64)
}

#[cfg(not(unix))]
fn free_space(_dir: &Path) -> Option<u64> {
    None
}

/// Mark a copy as just used, which is what the cache keeps the most of.
fn touch(copy: &Path) {
    if let Ok(file) = std::fs::File::options().append(true).open(copy) {
        let _ = file.set_modified(std::time::SystemTime::now());
    }
}

/// Take copies out of the cache at `dir`, the least lately used first,
/// until what is left takes no more than `budget` bytes; `keep`, the copy
/// just written, stays whatever its size. A copy's sample goes with it.
/// What was taken out, by name.
pub fn prune(dir: &Path, budget: u64, keep: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut copies: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_name()?.to_str()?.to_string();
            let copy = name.ends_with(".parquet") && !name.ends_with(".sample.parquet");
            let meta = e.metadata().ok()?;
            copy.then_some(())?;
            Some((meta.modified().ok()?, meta.len(), path))
        })
        .collect();
    let mut total: u64 = copies.iter().map(|(_, len, _)| len).sum();
    copies.sort_by_key(|(used, _, _)| *used);
    let mut gone = Vec::new();
    for (_, len, path) in copies {
        if total <= budget {
            break;
        }
        if path == keep {
            continue;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
            let _ = std::fs::remove_file(path.with_extension("sample.parquet"));
            gone.push(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
    gone
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn the_cache_lets_the_least_lately_used_copies_go_first() {
        let dir = std::env::temp_dir().join(format!("omasheet-prune-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let now = SystemTime::now();
        // Three copies of 100 bytes, used an hour, a minute and a second
        // ago, the first with a sample beside it.
        let make = |name: &str, ago: u64| {
            let path = dir.join(name);
            std::fs::write(&path, [0u8; 100]).unwrap();
            let file = std::fs::File::options().append(true).open(&path).unwrap();
            file.set_modified(now - Duration::from_secs(ago)).unwrap();
            path
        };
        make("a.parquet", 3600);
        make("a.sample.parquet", 3600);
        let b = make("b.parquet", 60);
        let c = make("c.parquet", 1);
        // Within 250 bytes, the oldest goes, and its sample with it.
        assert_eq!(prune(&dir, 250, &c), ["a.parquet"]);
        assert!(!dir.join("a.sample.parquet").exists());
        // Within 50, the copy just written stays whatever its size.
        assert_eq!(prune(&dir, 50, &c), ["b.parquet"]);
        assert!(!b.exists() && c.exists());
        // Used again, a copy counts as the newest.
        touch(&c);
        assert!(std::fs::metadata(&c).unwrap().modified().unwrap() >= now);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_budget_is_ten_gigabytes_or_a_tenth_of_the_space_free() {
        let budget = cache_budget();
        assert!(budget > 0 && budget <= 10 * 1_000_000_000);
        if let Some(free) = free_space(Path::new("/")) {
            assert!(free > 0);
        }
    }
}

#[cfg(test)]
mod column_cache_tests {
    use super::*;

    fn column(name: &str, n: i64) -> PlColumn {
        PlColumn::new(name.into(), (0..n).collect::<Vec<i64>>())
    }

    fn id(len: u64) -> Identity {
        (len, None)
    }

    #[test]
    fn the_least_recently_used_column_goes_first_when_the_budget_is_passed() {
        let cache = ColumnCache::with_budget(100);
        let path = Path::new("a.parquet");
        cache.put(path, "a", id(1), column("a", 4), 40);
        cache.put(path, "b", id(1), column("b", 4), 40);
        // `a` is used again, so `b` is the older.
        assert!(cache.get(path, "a", &id(1)).is_some());
        cache.put(path, "c", id(1), column("c", 4), 40);
        assert!(cache.get(path, "b", &id(1)).is_none(), "b went");
        assert!(cache.get(path, "a", &id(1)).is_some());
        assert!(cache.get(path, "c", &id(1)).is_some());
        let stats = cache.stats();
        assert_eq!((stats.entries, stats.bytes), (2, 80));
    }

    #[test]
    fn a_column_over_the_budget_by_itself_is_not_kept() {
        let cache = ColumnCache::with_budget(100);
        let path = Path::new("a.parquet");
        cache.put(path, "big", id(1), column("big", 4), 101);
        assert_eq!(cache.stats().entries, 0);
        assert!(cache.get(path, "big", &id(1)).is_none());
    }

    #[test]
    fn a_column_of_a_file_changed_since_is_a_miss_and_is_dropped() {
        let cache = ColumnCache::with_budget(100);
        let path = Path::new("a.parquet");
        cache.put(path, "a", id(1), column("a", 4), 40);
        assert!(cache.get(path, "a", &id(2)).is_none());
        let stats = cache.stats();
        assert_eq!((stats.entries, stats.bytes, stats.misses), (0, 0, 1));
        // The same name in another file is another column.
        cache.put(path, "a", id(2), column("a", 4), 40);
        assert!(cache.get(Path::new("b.parquet"), "a", &id(2)).is_none());
        assert!(cache.get(path, "a", &id(2)).is_some());
    }

    #[test]
    fn putting_a_column_again_replaces_it_without_counting_it_twice() {
        let cache = ColumnCache::with_budget(100);
        let path = Path::new("a.parquet");
        cache.put(path, "a", id(1), column("a", 4), 40);
        cache.put(path, "a", id(2), column("a", 8), 60);
        let stats = cache.stats();
        assert_eq!((stats.entries, stats.bytes), (1, 60));
    }

    #[test]
    fn the_budget_gives_way_to_what_is_held_whole() {
        let cache = ColumnCache::with_budget(100);
        let path = Path::new("a.parquet");
        cache.put(path, "a", id(1), column("a", 4), 40);
        cache.put(path, "b", id(1), column("b", 4), 40);
        assert_eq!(cache.stats().budget, 100);
        // 70 held whole leaves 30, which neither column fits in.
        cache.reserve(70);
        let stats = cache.stats();
        assert_eq!((stats.budget, stats.entries, stats.bytes), (30, 0, 0));
        // And nothing over what is left is kept afterwards.
        cache.put(path, "a", id(1), column("a", 4), 40);
        assert_eq!(cache.stats().entries, 0);
        cache.put(path, "c", id(1), column("c", 4), 20);
        assert_eq!(cache.stats().entries, 1);
        // What is held whole going again gives the room back.
        cache.reserve(0);
        assert_eq!(cache.stats().budget, 100);
        // More held than the budget is a budget of nothing, not a wrap.
        cache.reserve(500);
        assert_eq!(cache.stats().budget, 0);
    }

    #[test]
    fn a_file_held_whole_has_none_of_its_columns_kept_as_well() {
        let cache = ColumnCache::with_budget(1000);
        let (a, b) = (Path::new("a.parquet"), Path::new("b.parquet"));
        cache.put(a, "x", id(1), column("x", 4), 40);
        cache.put(a, "y", id(1), column("y", 4), 40);
        cache.put(b, "x", id(1), column("x", 4), 40);
        cache.evict_file(a);
        let stats = cache.stats();
        assert_eq!((stats.entries, stats.bytes), (1, 40));
        assert!(cache.get(b, "x", &id(1)).is_some());
        assert!(cache.get(a, "x", &id(1)).is_none());
    }
}

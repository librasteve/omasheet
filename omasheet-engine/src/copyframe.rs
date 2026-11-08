// Copyright (c) 2026 Stephen Roe

//! What a streamed copy to Parquet has in common, whatever it is read from
//! (`open-a-big-csv` D152, D153): the types a column comes to, a cell read
//! once into one of them, builders that fill a batch of rows a column at a
//! time, a schema worked out from the first rows or from all of them, and a
//! writer that puts the rows out a row group at a time, so that memory is a
//! batch of rows whatever the size of the file.
//!
//! A reader gives it rows through [`RowSource`]; `jsoncopy` and `xlsxcopy`
//! are two.

use omasheet_omx::date;
use omasheet_omx::duration;
use polars::prelude::*;
use std::fs::File;
use std::path::Path;

/// How many rows are written as one row group: the size the CSV copies use.
pub(crate) const BATCH: usize = 262_144;

/// How many rows a copy's types are first worked out from, as a CSV's are.
pub(crate) const HEAD_ROWS: usize = 10_000;

/// Where the rows of a sheet come from: calls `each` for each, in order,
/// until it says to stop. A row is read into the buffer, whole, a cell to a
/// column, and `names` is where an object's keys find their columns.
pub(crate) trait RowSource {
    fn rows(
        &self,
        names: &mut Names,
        each: &mut dyn FnMut(Shape, &mut Names, &mut RowBuf) -> bool,
    ) -> Result<(), String>;
}

/// The type a column of a copy comes to, widening as values are seen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// Nothing but nulls so far.
    Nothing,
    Bool,
    Int,
    Float,
    Date,
    Datetime,
    Time,
    Duration,
    Str,
}

impl Kind {
    pub(crate) fn join(self, other: Kind) -> Kind {
        use Kind::*;
        match (self, other) {
            (a, b) if a == b => a,
            (Nothing, k) | (k, Nothing) => k,
            (Int, Float) | (Float, Int) => Float,
            // A date is the date-time at its midnight, so a column of dates
            // with a time on some is a column of date-times.
            (Date, Datetime) | (Datetime, Date) => Datetime,
            _ => Str,
        }
    }
}

/// One JSON value, read once and with nothing more made of it than the
/// column it goes to needs: no tree of values per row.
#[derive(Debug, PartialEq)]
pub(crate) enum Cell {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    /// A number as it was written, where it is neither of the above.
    Number(String),
    Str(String),
    Date(i32),
    Datetime(i64),
    /// Nanoseconds since midnight.
    Time(i64),
    /// Microseconds.
    Duration(i64),
    /// A list or an object, as the JSON of it.
    Nested(String),
}

impl Cell {
    pub(crate) fn kind(&self) -> Kind {
        match self {
            Cell::Null => Kind::Nothing,
            Cell::Bool(_) => Kind::Bool,
            Cell::Int(_) => Kind::Int,
            Cell::Float(_) => Kind::Float,
            Cell::Date(_) => Kind::Date,
            Cell::Datetime(_) => Kind::Datetime,
            Cell::Time(_) => Kind::Time,
            Cell::Duration(_) => Kind::Duration,
            Cell::Number(_) | Cell::Str(_) | Cell::Nested(_) => Kind::Str,
        }
    }

    /// A cell as it is in a column of text.
    pub(crate) fn text(self) -> Option<String> {
        match self {
            Cell::Null => None,
            Cell::Bool(b) => Some(b.to_string()),
            Cell::Int(i) => Some(i.to_string()),
            Cell::Float(f) => Some(f.to_string()),
            Cell::Number(s) | Cell::Str(s) | Cell::Nested(s) => Some(s),
            // A date or a time as it is written: only in a column that is
            // text because of the others, so the string was one.
            Cell::Date(d) => Some(date::format(d)),
            Cell::Datetime(us) => Some(date::format_datetime(us)),
            Cell::Time(ns) => Some(date::format_time(ns)),
            Cell::Duration(us) => Some(duration::format(us)),
        }
    }
}

/// The columns of a sheet by name, found as the rows are read.
pub(crate) struct Names {
    pub(crate) names: Vec<String>,
    index: std::collections::HashMap<String, usize>,
    /// A key that is not in `names` is not added but refused.
    frozen: bool,
}

impl Names {
    pub(crate) fn new(names: Vec<String>, frozen: bool) -> Names {
        let index = names
            .iter()
            .enumerate()
            .map(|(c, n)| (n.clone(), c))
            .collect();
        Names {
            names,
            index,
            frozen,
        }
    }

    pub(crate) fn column(&mut self, key: &str) -> Option<usize> {
        if let Some(&c) = self.index.get(key) {
            return Some(c);
        }
        if self.frozen {
            return None;
        }
        self.names.push(key.to_string());
        self.index.insert(key.to_string(), self.names.len() - 1);
        Some(self.names.len() - 1)
    }
}

/// The shape of a row, and what it holds, one cell to a column.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Shape {
    Object,
    List,
}

/// What a row of a sheet is read into, reused for each.
pub(crate) struct RowBuf {
    pub(crate) cells: Vec<Option<Cell>>,
    /// A key a row has that the columns do not, where they are fixed.
    pub(crate) unknown: bool,
}

/// The columns of a sheet and what each holds, found from its rows.
pub(crate) struct Schema {
    pub(crate) names: Vec<String>,
    pub(crate) kinds: Vec<Kind>,
    /// Rows are lists of values, the first of which is these names.
    pub(crate) listed: Option<bool>,
}

/// A name from the first row of a list of lists.
pub(crate) fn label(cell: Option<&Cell>) -> String {
    match cell {
        Some(Cell::Str(s)) => s.trim().to_string(),
        Some(Cell::Null) | None => String::new(),
        Some(Cell::Bool(b)) => b.to_string(),
        Some(Cell::Int(i)) => i.to_string(),
        Some(Cell::Float(f)) => f.to_string(),
        Some(Cell::Number(s) | Cell::Nested(s)) => s.clone(),
        Some(Cell::Date(d)) => date::format(*d),
        Some(Cell::Datetime(us)) => date::format_datetime(*us),
        Some(Cell::Time(ns)) => date::format_time(*ns),
        Some(Cell::Duration(us)) => duration::format(*us),
    }
}

impl Schema {
    /// The names the columns go by in the copy: each once, none empty.
    fn unique_names(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::with_capacity(self.names.len());
        for (c, name) in self.names.iter().enumerate() {
            let base = if name.trim().is_empty() {
                format!("Column{}", c + 1)
            } else {
                name.clone()
            };
            let mut candidate = base.clone();
            let mut n = 2;
            while out.contains(&candidate) {
                candidate = format!("{base}_{n}");
                n += 1;
            }
            out.push(candidate);
        }
        out
    }
}

/// The schema of a sheet from its first `limit` rows, or from all of them.
pub(crate) fn schema_of(src: &dyn RowSource, limit: Option<usize>) -> Result<Schema, String> {
    let mut names = Names::new(Vec::new(), false);
    let mut kinds: Vec<Kind> = Vec::new();
    let mut listed: Option<bool> = None;
    let mut seen = 0usize;
    let mut bad: Option<usize> = None;
    let mut header: Vec<String> = Vec::new();
    src.rows(
        &mut names,
        &mut |shape: Shape, names: &mut Names, buf: &mut RowBuf| {
            seen += 1;
            let is_list = shape == Shape::List;
            match listed {
                None => listed = Some(is_list),
                Some(was) if was != is_list => {
                    bad = Some(seen);
                    return false;
                }
                _ => {}
            }
            if is_list && seen == 1 {
                // The first is the names, whatever they look like.
                header = buf.cells.iter().map(|c| label(c.as_ref())).collect();
                kinds = vec![Kind::Nothing; header.len()];
                return true;
            }
            if kinds.len() < buf.cells.len() {
                kinds.resize(buf.cells.len().max(names.names.len()), Kind::Nothing);
            }
            for (c, cell) in buf.cells.iter().enumerate() {
                if let Some(cell) = cell {
                    kinds[c] = kinds[c].join(cell.kind());
                }
            }
            limit.is_none_or(|n| seen < n)
        },
    )?;
    if let Some(r) = bad {
        return Err(format!(
            "row {r} is not an object of values by column name, nor a list of values like \
             the rows before it"
        ));
    }
    let mut columns = if listed == Some(true) {
        header
    } else {
        names.names
    };
    if listed == Some(true) && kinds.len() > columns.len() {
        for c in columns.len()..kinds.len() {
            columns.push(format!("Column{}", c + 1));
        }
    }
    kinds.resize(columns.len(), Kind::Nothing);
    if columns.is_empty() {
        return Err("it has no rows".to_string());
    }
    Ok(Schema {
        names: columns,
        kinds,
        listed,
    })
}

/// One column of a batch, being filled.
enum Builder {
    Bool(Vec<Option<bool>>),
    Int(Vec<Option<i64>>),
    Float(Vec<Option<f64>>),
    Date(Vec<Option<i32>>),
    Datetime(Vec<Option<i64>>),
    Time(Vec<Option<i64>>),
    Duration(Vec<Option<i64>>),
    Str(Vec<Option<String>>),
}

impl Builder {
    fn new(kind: Kind) -> Builder {
        match kind {
            Kind::Bool => Builder::Bool(Vec::new()),
            Kind::Int => Builder::Int(Vec::new()),
            Kind::Float => Builder::Float(Vec::new()),
            Kind::Date => Builder::Date(Vec::new()),
            Kind::Datetime => Builder::Datetime(Vec::new()),
            Kind::Time => Builder::Time(Vec::new()),
            Kind::Duration => Builder::Duration(Vec::new()),
            // A column of nothing but nulls is text, which can hold them.
            Kind::Str | Kind::Nothing => Builder::Str(Vec::new()),
        }
    }

    /// Take a cell in, or say that it is not what the column holds.
    fn push(&mut self, cell: Option<Cell>) -> bool {
        let cell = cell.filter(|c| *c != Cell::Null);
        match (self, cell) {
            (Builder::Bool(v), None) => v.push(None),
            (Builder::Int(v), None) => v.push(None),
            (Builder::Float(v), None) => v.push(None),
            (Builder::Date(v), None) => v.push(None),
            (Builder::Datetime(v), None) => v.push(None),
            (Builder::Time(v), None) => v.push(None),
            (Builder::Duration(v), None) => v.push(None),
            (Builder::Str(v), None) => v.push(None),
            (Builder::Bool(v), Some(Cell::Bool(b))) => v.push(Some(b)),
            (Builder::Int(v), Some(Cell::Int(i))) => v.push(Some(i)),
            (Builder::Float(v), Some(Cell::Float(f))) => v.push(Some(f)),
            (Builder::Float(v), Some(Cell::Int(i))) => v.push(Some(i as f64)),
            (Builder::Date(v), Some(Cell::Date(d))) => v.push(Some(d)),
            (Builder::Datetime(v), Some(Cell::Datetime(us))) => v.push(Some(us)),
            // A date in a column of date-times is at its midnight.
            (Builder::Datetime(v), Some(Cell::Date(d))) => {
                v.push(Some(i64::from(d) * 86_400 * 1_000_000))
            }
            (Builder::Time(v), Some(Cell::Time(ns))) => v.push(Some(ns)),
            (Builder::Duration(v), Some(Cell::Duration(us))) => v.push(Some(us)),
            (Builder::Str(v), Some(cell)) => v.push(cell.text()),
            _ => return false,
        }
        true
    }

    fn len(&self) -> usize {
        match self {
            Builder::Bool(v) => v.len(),
            Builder::Int(v) => v.len(),
            Builder::Float(v) => v.len(),
            Builder::Date(v) => v.len(),
            Builder::Datetime(v) => v.len(),
            Builder::Time(v) => v.len(),
            Builder::Duration(v) => v.len(),
            Builder::Str(v) => v.len(),
        }
    }

    fn finish(&mut self, name: &str) -> Result<Column, String> {
        let name: PlSmallStr = name.into();
        let column = match self {
            Builder::Bool(v) => Column::new(name, std::mem::take(v)),
            Builder::Int(v) => Column::new(name, std::mem::take(v)),
            Builder::Float(v) => Column::new(name, std::mem::take(v)),
            Builder::Date(v) => Column::new(name, std::mem::take(v))
                .cast(&DataType::Date)
                .map_err(|e| e.to_string())?,
            Builder::Datetime(v) => Column::new(name, std::mem::take(v))
                .cast(&DataType::Datetime(TimeUnit::Microseconds, None))
                .map_err(|e| e.to_string())?,
            Builder::Time(v) => Column::new(name, std::mem::take(v))
                .cast(&DataType::Time)
                .map_err(|e| e.to_string())?,
            Builder::Duration(v) => Column::new(name, std::mem::take(v))
                .cast(&DataType::Duration(TimeUnit::Microseconds))
                .map_err(|e| e.to_string())?,
            Builder::Str(v) => Column::new(name, std::mem::take(v)),
        };
        Ok(column)
    }
}

/// A frame of the rows taken so far, and the builders emptied.
fn frame(names: &[String], builders: &mut [Builder]) -> Result<DataFrame, String> {
    let columns = names
        .iter()
        .zip(builders.iter_mut())
        .map(|(name, b)| b.finish(name))
        .collect::<Result<Vec<_>, _>>()?;
    DataFrame::new(columns).map_err(|e| e.to_string())
}

/// Why a write stopped short: the rows are not as the schema said (a value
/// of another type, a column that is new), which is for the schema to be
/// worked out again from every row, or an error that is not.
pub(crate) enum Stop {
    Widen,
    Failed(String),
}

impl From<String> for Stop {
    fn from(why: String) -> Stop {
        Stop::Failed(why)
    }
}

/// Writes the rows of the sheet `sheet`, up to `limit` of them, to a
/// Parquet file at `target`, typed by `schema`.
pub(crate) fn write(
    src: &dyn RowSource,
    schema: &Schema,
    limit: Option<usize>,
    target: &Path,
) -> Result<(), Stop> {
    let names = schema.unique_names();
    let mut builders: Vec<Builder> = schema.kinds.iter().map(|k| Builder::new(*k)).collect();
    let listed = schema.listed == Some(true);
    let empty = frame(&names, &mut builders)?;
    let file = File::create(target).map_err(|e| e.to_string())?;
    let mut writer = ParquetWriter::new(file)
        .with_row_group_size(Some(BATCH))
        .batched(empty.schema())
        .map_err(|e| e.to_string())?;
    // The keys of an object are the columns' as they were found; a key not
    // among them, in a row after the ones the schema was made from, is for
    // the schema to be made again.
    let mut keys = Names::new(schema.names.clone(), true);
    let mut stopped: Option<Stop> = None;
    let mut seen = 0usize;
    let mut in_batch = 0usize;
    src.rows(
        &mut keys,
        &mut |shape: Shape, _: &mut Names, buf: &mut RowBuf| {
            seen += 1;
            // The first row of a list of lists is the names, already taken.
            if listed && seen == 1 {
                return true;
            }
            if buf.unknown || (shape == Shape::List) != listed {
                stopped = Some(Stop::Widen);
                return false;
            }
            for (c, builder) in builders.iter_mut().enumerate() {
                let cell = buf.cells.get_mut(c).and_then(Option::take);
                if !builder.push(cell) {
                    stopped = Some(Stop::Widen);
                    return false;
                }
            }
            // A list with more values than there are columns is a column new.
            if buf.cells.len() > builders.len() {
                stopped = Some(Stop::Widen);
                return false;
            }
            in_batch += 1;
            if in_batch >= BATCH {
                in_batch = 0;
                if let Err(e) = frame(&names, &mut builders)
                    .and_then(|df| writer.write_batch(&df).map_err(|e| e.to_string()))
                {
                    stopped = Some(Stop::Failed(e));
                    return false;
                }
            }
            limit.is_none_or(|n| seen - usize::from(listed) < n)
        },
    )?;
    if let Some(stop) = stopped {
        return Err(stop);
    }
    if builders.first().is_some_and(|b| b.len() > 0) {
        let df = frame(&names, &mut builders)?;
        writer.write_batch(&df).map_err(|e| e.to_string())?;
    }
    writer.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// The whole of a source, written as a Parquet file at `target`. The types
/// are worked out from the first rows and the file written in one pass;
/// where a row further down does not fit them (a number among whole ones, a
/// column that is new) the types are worked out again from every row and it
/// is written again.
pub(crate) fn convert(src: &dyn RowSource, target: &Path) -> Result<(), String> {
    let head = schema_of(src, Some(HEAD_ROWS))?;
    match write(src, &head, None, target) {
        Ok(()) => Ok(()),
        Err(Stop::Failed(why)) => Err(why),
        Err(Stop::Widen) => {
            let all = schema_of(src, None)?;
            match write(src, &all, None, target) {
                Ok(()) => Ok(()),
                Err(Stop::Failed(why)) => Err(why),
                Err(Stop::Widen) => Err("its rows do not keep to one shape".to_string()),
            }
        }
    }
}

/// The first `rows` rows of a source, typed by the first `infer` of them,
/// written as a Parquet file at `target`: what is shown until the whole is.
pub(crate) fn sample(
    src: &dyn RowSource,
    infer: usize,
    rows: usize,
    target: &Path,
) -> Result<(), String> {
    let schema = schema_of(src, Some(infer))?;
    match write(src, &schema, Some(rows), target) {
        Ok(()) | Err(Stop::Widen) => Ok(()),
        Err(Stop::Failed(why)) => Err(why),
    }
}

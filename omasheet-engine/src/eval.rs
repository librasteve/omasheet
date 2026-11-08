// Copyright (c) 2026 Stephen Roe

//! Evaluation of a checked program.
//!
//! Cells are calculated on demand and remembered, so each is calculated once
//! and always after whatever it reads. [`Engine::run`] walks the program's
//! calculation order so that long chains (a running balance) are filled in
//! row by row rather than by deep recursion.

use crate::datefmt::{Instant, Moment, format_date};
use crate::dtype::{cell_of, column_of};
use crate::numfmt::format_number;
use crate::table::{ColumnCache, Held, Identity, Tables};
use crate::value::{
    RowRef, Value, View, arith, bitwise, compare, complement, equal, format_exact, format_styled,
    math, round_digits, sort_order,
};
use crate::zone::{Zone, Zoned, utc_now};
use crate::{Lowering, Options, ROW_THRESHOLD, lower};
use omasheet_omx::ast::Lit;
use omasheet_omx::ast::{BinOp, UnOp};
use omasheet_omx::check::Join;
use omasheet_omx::colfmt::Format;
use omasheet_omx::cycle;
use omasheet_omx::date;
use omasheet_omx::decimal::{Dec, SHOWN_PRECISION};
use omasheet_omx::funcs::MathFn;
use omasheet_omx::ir::{Bound, ColSel, Func, Ir, Node, Offset, RowSel};
use omasheet_omx::types::Base;
use omasheet_omx::{Cell, ColKind, Column, Diagnostic, Group, Program, S, Span};
use polars::prelude::Column as PlColumn;
use polars::prelude::{
    AnyValue, DataFrame, DataType, Expr, IntoLazy, JoinArgs, JoinType, MaintainOrderJoin,
    SortMultipleOptions, col,
};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

/// How many rows a grid's read of a big table's column goes past the rows
/// shown, on each side (`open-a-big-csv` 1.3). A screen is a few dozen rows
/// and a step of a scroll a few more, so this is many hundreds of frames
/// of scrolling in either direction before the next read.
/// Row groups read at once for an aggregate over a column too tall to hold
/// (D155): 8 of 262,144 rows is 2M rows of one column at a time.
const BATCH: usize = 8;

const READ_AHEAD: usize = 8_192;

/// The rows past the read-ahead that a column to be deferred is tried on:
/// a screen's worth, so that the try is of what the first screen will be.
const PROBE_ROWS: usize = 64;

/// Why an evaluation stopped.
pub enum Fail {
    Diag(Diagnostic),
    /// It read a cell that had already failed; that failure was reported.
    Poison,
}

type R<T = Value> = Result<T, Fail>;

/// The rows in scope: (sheet, row). The innermost row is last.
pub type Frames = Vec<(usize, usize)>;

#[derive(Clone)]
enum Slot {
    Pending,
    Busy,
    Done(Value),
}

/// What became of a column the first gate said could be calculated in one
/// step: see [`Engine::attempts`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attempt {
    /// Calculated by Polars, over the whole column.
    Lowered,
    /// Polars has no answer of its own for some part of it, so the engine
    /// calculated it row by row.
    Refused,
    /// Lowered, and then given up: the plan would not collect, so the
    /// result was discarded whole and the engine calculated the column
    /// row by row instead.
    Failed,
}

enum Rows {
    /// One position, or `None` for a cursor offset outside the sheet.
    One(Option<usize>),
    Many(Vec<usize>),
}

/// A group of rows of a grouped sheet: the sheet grouped and the rows.
type GroupRows = (usize, Rc<Vec<usize>>);
/// The rows of each group of a grouped sheet, a row of it each.
type Groups = Vec<Rc<Vec<usize>>>;
/// The values of an aggregate of a grouped sheet Polars took, one a group.
type AggValues = Rc<Vec<Value>>;
/// The rows of a sheet in partitions, one for each value of key columns:
/// `of[p]` is the partition of position `p` and its place in it, and each
/// partition lists its positions in the sheet's order.
struct Partition {
    of: Vec<(usize, usize)>,
    parts: Vec<Vec<usize>>,
}

/// The partitions worked out, by sheet and key columns.
type Partitions = HashMap<(usize, Vec<usize>), Rc<Partition>>;

/// The right sheet of a join and the row of it each row of the join reads.
type Paired = (usize, Rc<Vec<Option<usize>>>);

/// What a column function (`is_unique` and its kin) answers for each row of
/// a column, by the function, the sheet and the column.
type Marks = ((Func, usize, usize), Rc<Vec<bool>>);

/// The program an engine runs: lent to it for one run, as the command line
/// does, or held by it, so that an engine can be kept after the run and a
/// cell asked for when it is shown (D80).
enum Prog<'p> {
    Lent(&'p Program),
    Held(Box<Program>),
}

impl std::ops::Deref for Prog<'_> {
    type Target = Program;
    fn deref(&self) -> &Program {
        match self {
            Prog::Lent(p) => p,
            Prog::Held(p) => p,
        }
    }
}

pub struct Engine<'p> {
    prog: Prog<'p>,
    /// The columns of every `table`, as their files hold them. Empty for a
    /// program with no table, and for one whose sources were not read.
    tables: Tables,
    /// A column the dataframe holds and is the authority for: one
    /// calculated in one step by Polars. Its values are read the way a
    /// `table`'s columns are — a value is made into a cell as it is asked
    /// for — so a column nothing asks for is never read.
    held: RefCell<Vec<Vec<Option<PlColumn>>>>,
    /// And a copy of a column already calculated, made so that something
    /// else could be lowered over it. It is **not** read back as the
    /// column's values, and [`Engine::cell`] never looks here: a copy
    /// passes through a dtype, and a dtype does not carry everything a
    /// value does — text written bare in a cell is Markdown where a file is
    /// shown as a document, and no Polars string holds that.
    copies: RefCell<Vec<Vec<Option<PlColumn>>>>,
    /// A column calculated in one step that has not been, and is calculated
    /// for the rows that are shown a window at a time instead, until
    /// something asks for the whole of it (`Options::defer_rows`).
    deferred: RefCell<Vec<Vec<bool>>>,
    /// Some rows of a `table`'s column, read from the file for a row range
    /// asked for before the whole column was, and the row they start at.
    /// Kept apart from `held`, which is only ever the whole column: a range
    /// wanted whole afterwards is read again, so nothing that holds some
    /// rows can be asked for one it has not got (D78). The latest range
    /// read is the one kept.
    windows: RefCell<Vec<Vec<Option<Window>>>>,
    /// What became of each column that could have been calculated in one
    /// step, in the order they were tried.
    attempts: RefCell<Vec<(usize, usize, Attempt)>>,
    /// And of each aggregate over a whole column, by the column it was
    /// taken over. One column may be here more than once.
    aggregates: RefCell<Vec<(usize, usize, Attempt)>>,
    /// And of each condition picking rows of a sheet, by that sheet.
    filters: RefCell<Vec<(usize, Attempt)>>,
    /// What `is_unique` and its kin answer over a column of a sheet, by
    /// the function, the sheet and the column: worked out once for the
    /// column and looked up a row at a time.
    marks: RefCell<Vec<Marks>>,
    /// Which of a `table`'s columns have been read from its file, by sheet
    /// and column, in the order they were read. What is not here was never
    /// read.
    reads: RefCell<Vec<(usize, usize)>>,
    /// How many values those reads brought back: the sheet's row count for
    /// a column read whole, and the rows of the range for one read for a
    /// row range (D78). `tests/rowgroups.rs` watches it.
    rows_read: RefCell<usize>,
    /// How many sums and means of a column of floats were taken as a loop
    /// over the floats and not by a walk of values.
    float_totals: std::cell::Cell<usize>,
    /// A memoisation slot per cell, by sheet, column and row. A column's
    /// slots are made when a cell of it is first asked for: a sheet of a
    /// million rows whose columns all come from a file needs none of them,
    /// and merely opening such a file should not pay for them.
    cells: RefCell<Vec<Vec<Option<Vec<Slot>>>>>,
    consts: RefCell<Vec<Slot>>,
    /// The evaluated view of each sheet computed from an expression, once
    /// something has asked for one: a `Slot` so that a view reading itself
    /// is caught as any other cycle is (D90).
    views: RefCell<Vec<Slot>>,
    /// The rows of each group of a sheet computed by `group_by`, in the
    /// sheet its view groups, once the view has been evaluated: one entry a
    /// row of the grouped sheet.
    groups: RefCell<Vec<Option<Rc<Groups>>>>,
    /// The group an aggregate of `agg` is being worked out for, innermost
    /// last: the sheet grouped and the rows of the group.
    grouping: RefCell<Vec<GroupRows>>,
    /// The aggregates of a grouped sheet Polars took, by sheet and column,
    /// a value a group.
    agg_held: RefCell<Vec<Vec<Option<AggValues>>>>,
    /// The right sheet of each sheet computed by `join`, and the row of it
    /// each row of the join pairs with: none, for a left row a left join
    /// kept with nothing to pair it with.
    joined: RefCell<Vec<Option<Paired>>>,
    /// The partitions of a whole sheet by its key columns, for `*-1 by
    /// Region`: worked out once, so a column of such offsets is not a pass
    /// over the sheet for every cell.
    partitions: RefCell<Partitions>,
    /// The arguments of the file's own functions being evaluated, the
    /// innermost call last.
    args: RefCell<Vec<Vec<Value>>>,
    all_rows: Vec<Rc<Vec<usize>>>,
    all_cols: Vec<Rc<Vec<usize>>>,
    diags: RefCell<Vec<Diagnostic>>,
    options: Options,
    /// The zone the file's date-times are in.
    home: Rc<Zone>,
    /// The zone of this machine.
    local: Rc<Zone>,
    zones: RefCell<HashMap<String, Option<Rc<Zone>>>>,
    /// What `now()` gives.
    now: i64,
}

/// The complaint about a file that names a time zone there is none of.
pub fn unknown_zone(prog: &Program) -> Vec<Diagnostic> {
    let columns = prog
        .sheets
        .iter()
        .flat_map(|t| t.cols.iter())
        .filter_map(|c| c.zone.as_ref());
    prog.zone
        .iter()
        .chain(columns)
        .filter(|(name, _)| Zone::named(name).is_none())
        .map(|(name, span)| no_zone(name, *span))
        .collect()
}

fn no_zone(name: &str, span: Span) -> Diagnostic {
    Diagnostic::new(span, format!("there is no time zone `{name}`"))
        .with_help("use a name from the time zone database, such as `Europe/London` or `UTC`")
}

/// The column a lowered selection counts rows in. No omasheet column can
/// be called this, so it cannot be one of the sheet's own.
const ROW_INDEX: &str = "#row";
/// The rows of the two sheets of a join, in the frame Polars joins.
const LEFT_ROW: &str = "#left";
const RIGHT_ROW: &str = "#right";

fn fail<T>(span: Span, message: impl Into<String>) -> R<T> {
    Err(Fail::Diag(Diagnostic::new(span, message)))
}

/// One literal as a value of a column of this type. Any exact number in a
/// `Fraction` column is shown as a fraction; there is no fraction literal to
/// carry it, since `1/7` is a division.
fn lit_value(l: &Lit, ty: S) -> Value {
    match ty {
        S::Fraction => Value::from_lit(l).fraction(),
        _ => Value::from_lit(l),
    }
}

/// One value of a column the dataframe holds, as a cell. A row the column
/// has not got is in error rather than empty, as a column the file lacks is.
fn held_cell(column: &PlColumn, r: usize, ty: S) -> Cell {
    match column.get(r) {
        Ok(v) => cell_of(v, ty),
        Err(_) => Cell::Invalid,
    }
}

/// Some rows of a column read from its file, and the row they start at.
type Window = (usize, PlColumn);

/// A value an aggregate came to, in a form a document can keep between its
/// rebuilds and share with the engines it makes: no `Rc` in it. Only the
/// kinds of answer there are in plenty, numbers and dates and the like;
/// anything else is worked out each time.
#[derive(Clone, Debug)]
pub(crate) enum Scalar {
    Null,
    Int(i64, Base),
    Decimal(Dec),
    Fraction(Dec),
    Percent(Dec),
    Float(f64),
    Boolean(bool),
    Date(i32),
    Time(i64),
    Datetime(i64),
    Duration(i64),
    Text(String, bool),
}

impl Scalar {
    /// The value as one that can be kept, where it is one.
    pub(crate) fn of(v: &Value) -> Option<Scalar> {
        Some(match v {
            Value::Null => Scalar::Null,
            Value::Int(n, base) => Scalar::Int(*n, *base),
            Value::Decimal(d) => Scalar::Decimal(*d),
            Value::Fraction(d) => Scalar::Fraction(*d),
            Value::Percent(d) => Scalar::Percent(*d),
            Value::Float(f) => Scalar::Float(*f),
            Value::Boolean(b) => Scalar::Boolean(*b),
            Value::Date(d) => Scalar::Date(*d),
            Value::Time(t) => Scalar::Time(*t),
            Value::Datetime(us, None) => Scalar::Datetime(*us),
            Value::Duration(us) => Scalar::Duration(*us),
            Value::String(s, bare) => Scalar::Text(s.to_string(), *bare),
            _ => return None,
        })
    }

    pub(crate) fn value(&self) -> Value {
        match self {
            Scalar::Null => Value::Null,
            Scalar::Int(n, base) => Value::Int(*n, *base),
            Scalar::Decimal(d) => Value::Decimal(*d),
            Scalar::Fraction(d) => Value::Fraction(*d),
            Scalar::Percent(d) => Value::Percent(*d),
            Scalar::Float(f) => Value::Float(*f),
            Scalar::Boolean(b) => Value::Boolean(*b),
            Scalar::Date(d) => Value::Date(*d),
            Scalar::Time(t) => Value::Time(*t),
            Scalar::Datetime(us) => Value::Datetime(*us, None),
            Scalar::Duration(us) => Value::Duration(*us),
            Scalar::Text(s, bare) => Value::String(Rc::from(s.as_str()), *bare),
        }
    }
}

/// The sheet and the column of an aggregate's one argument.
fn args_sheet(args: &[Node]) -> usize {
    match args {
        [
            Node {
                kind: Ir::Column { sheet, .. },
                ..
            },
        ] => *sheet,
        _ => 0,
    }
}

fn args_col(args: &[Node]) -> usize {
    match args {
        [
            Node {
                kind: Ir::Column { col, .. },
                ..
            },
        ] => *col,
        _ => 0,
    }
}

/// One argument of an aggregate, as the aggregate walks it (D79, item 2 of
/// `hold-less-of-a-big-file`). A whole column the dataframe holds is kept
/// as that column and walked in place, so that summing twenty million
/// rows of it does not first build twenty million boxed values; anything
/// else is the values it evaluated to, shared rather than copied.
enum Items {
    One(Value),
    Many(Rc<Vec<Value>>),
    /// A column the dataframe is the authority for, in one chunk, and the
    /// sheet and column it is, whose memoised cells are consulted as it is
    /// walked: a cell already read is the value it was read as, as it is
    /// in `column_shown`.
    Column {
        series: polars::prelude::Series,
        len: usize,
        at: (usize, usize),
    },
}

impl Items {
    fn len(&self) -> usize {
        match self {
            Items::One(_) => 1,
            Items::Many(v) => v.len(),
            Items::Column { len, .. } => *len,
        }
    }

    /// Every value, in order, handed to `f`. Nothing here evaluates, so
    /// nothing here can fail: whatever could was settled when the items
    /// were taken.
    ///
    /// `f` must not reach back into the engine: the column's slots are
    /// borrowed while it is walked.
    fn each(&self, engine: &Engine, f: &mut impl FnMut(Value)) {
        match self {
            Items::One(v) => f(v.clone()),
            Items::Many(vs) => vs.iter().cloned().for_each(f),
            Items::Column {
                series,
                len,
                at: (t, c),
            } => {
                let cells = engine.cells.borrow();
                let slots = cells[*t][*c].as_deref();
                // `Series::iter` is over one chunk, so a column of several is
                // made one, which is a copy of it: what the loop over floats
                // does not need.
                let one;
                let series = if series.n_chunks() > 1 {
                    one = series.rechunk();
                    &one
                } else {
                    series
                };
                for (r, any) in series.iter().take(*len).enumerate() {
                    match slots.map(|s| &s[r]) {
                        Some(Slot::Done(v)) => f(v.clone()),
                        _ => f(engine.value_of(*t, *c, any)),
                    }
                }
            }
        }
    }
}

impl Engine<'static> {
    /// [`Engine::with_tables`], holding the program rather than borrowing
    /// it, for an engine kept beside what it calculated: the app keeps one
    /// to format the rows of a big table as the grid shows them (D80).
    pub fn owning(prog: Program, options: Options, tables: Tables) -> Engine<'static> {
        Engine::over(Prog::Held(Box::new(prog)), options, tables)
    }

    /// The program it runs.
    pub fn program(&self) -> &Program {
        &self.prog
    }
}

impl<'p> Engine<'p> {
    pub fn new(prog: &'p Program) -> Engine<'p> {
        Engine::with_options(prog, Options::default())
    }

    /// A file that names a time zone there is none of is evaluated in the
    /// zone of this machine, with a diagnostic to take before running.
    pub fn with_options(prog: &'p Program, options: Options) -> Engine<'p> {
        Engine::with_tables(prog, options, Tables::default())
    }

    /// [`Engine::with_options`], over the tables [`crate::table::read`]
    /// has already read. A program with a `table` and no tables here shows
    /// that sheet with no rows.
    pub fn with_tables(prog: &'p Program, options: Options, tables: Tables) -> Engine<'p> {
        Engine::over(Prog::Lent(prog), options, tables)
    }

    fn over(prog: Prog<'p>, options: Options, tables: Tables) -> Engine<'p> {
        let local = Rc::new(options.zone.clone().unwrap_or_else(Zone::system));
        let named = prog.zone.as_ref().and_then(|(name, _)| Zone::named(name));
        let home = named.map_or_else(|| local.clone(), Rc::new);
        let now = options
            .now
            .or_else(|| home.from_utc(utc_now()))
            .unwrap_or(0);
        // Moved in last: every other field is made from it.
        Engine {
            tables,
            held: RefCell::new(
                prog.sheets
                    .iter()
                    .map(|t| vec![None; t.cols.len()])
                    .collect(),
            ),
            windows: RefCell::new(
                prog.sheets
                    .iter()
                    .map(|t| vec![None; t.cols.len()])
                    .collect(),
            ),
            copies: RefCell::new(
                prog.sheets
                    .iter()
                    .map(|t| vec![None; t.cols.len()])
                    .collect(),
            ),
            deferred: RefCell::new(
                prog.sheets
                    .iter()
                    .map(|t| vec![false; t.cols.len()])
                    .collect(),
            ),
            views: RefCell::new(vec![Slot::Pending; prog.sheets.len()]),
            groups: RefCell::new(vec![None; prog.sheets.len()]),
            grouping: RefCell::new(Vec::new()),
            joined: RefCell::new(vec![None; prog.sheets.len()]),
            partitions: RefCell::new(HashMap::new()),
            agg_held: RefCell::new(
                prog.sheets
                    .iter()
                    .map(|t| vec![None; t.cols.len()])
                    .collect(),
            ),
            attempts: RefCell::new(Vec::new()),
            aggregates: RefCell::new(Vec::new()),
            filters: RefCell::new(Vec::new()),
            marks: RefCell::new(Vec::new()),
            reads: RefCell::new(Vec::new()),
            rows_read: RefCell::new(0),
            float_totals: std::cell::Cell::new(0),
            cells: RefCell::new(
                prog.sheets
                    .iter()
                    .map(|t| vec![None; t.cols.len()])
                    .collect(),
            ),
            consts: RefCell::new(vec![Slot::Pending; prog.consts.len()]),
            args: RefCell::new(Vec::new()),
            all_rows: prog
                .sheets
                .iter()
                .map(|t| Rc::new((0..t.nrows).collect()))
                .collect(),
            all_cols: prog
                .sheets
                .iter()
                .map(|t| Rc::new((0..t.cols.len()).collect()))
                .collect(),
            diags: RefCell::new(unknown_zone(&prog)),
            options,
            home,
            local,
            zones: RefCell::new(HashMap::new()),
            now,
            prog,
        }
    }

    /// Calculate every constant and cell, in dependency order.
    pub fn run(&self) {
        for group in &self.prog.order {
            match group {
                Group::Const(i) => {
                    let _ = self.const_value(*i);
                }
                Group::Cols { cols, reverse } => {
                    let Some(&(t, _)) = cols.first() else {
                        continue;
                    };
                    // A column whose values are in a file has nothing to
                    // calculate, so there is nothing here to do for it and
                    // no reason to read it: it is read when a cell of it is
                    // shown or a calculation asks for it, and a column
                    // nothing asks for is never read at all. A column of
                    // cells stays, a cell of it being able to hold a
                    // formula of its own.
                    if cols
                        .iter()
                        .all(|&(t, c)| matches!(self.prog.sheets[t].cols[c].kind, ColKind::Frame))
                    {
                        continue;
                    }
                    if self.try_defer(cols) || self.run_lowered(cols) {
                        continue;
                    }
                    let n = self.prog.sheets[t].nrows;
                    for k in 0..n {
                        let r = if *reverse { n - 1 - k } else { k };
                        for &(t, c) in cols {
                            if r < self.prog.sheets[t].nrows {
                                let _ = self.cell(t, c, r);
                            }
                        }
                    }
                }
            }
        }
    }

    /// What became of every column that could have been calculated in one
    /// step, as [`Lowering`] allowed it to be tried: what was lowered, what
    /// Polars had no answer for, and what was lowered and then given up. A
    /// column the row count left on the engine is not here, having never
    /// been tried; nor is one the first gate ruled out.
    pub fn attempts(&self) -> Vec<(usize, usize, Attempt)> {
        self.attempts.borrow().clone()
    }

    /// What became of every aggregate over a whole column, by the column it
    /// was taken over, in the order they were taken. A column aggregated
    /// twice is here twice.
    pub fn aggregates(&self) -> Vec<(usize, usize, Attempt)> {
        self.aggregates.borrow().clone()
    }

    /// And of every condition that picked rows of a sheet, by that sheet,
    /// in the order they were asked.
    pub fn filters(&self) -> Vec<(usize, Attempt)> {
        self.filters.borrow().clone()
    }

    /// Which of a `table`'s columns were read from its file, by sheet and
    /// column, in the order they were read. Each is read once and kept, so
    /// a column is here at most once, and a column nothing asked for is not
    /// here at all.
    pub fn reads(&self) -> Vec<(usize, usize)> {
        self.reads.borrow().clone()
    }

    /// How many values have been read from a `table`'s files, over all the
    /// columns read: the measure of how much of a Parquet an answer cost,
    /// where [`Engine::reads`] says only which columns it touched.
    pub fn rows_read(&self) -> usize {
        *self.rows_read.borrow()
    }

    /// Calculate a group of columns in one step, over the whole column, and
    /// say whether it was done. `false` leaves the group to the row loop,
    /// which is the only path that can say which row failed.
    ///
    /// A group of more than one column is two columns that read each other,
    /// which for same-row reads is a circular reference and for any other
    /// kind is not elementwise — so a group that can be lowered is one
    /// column, and that is what this looks for.
    fn run_lowered(&self, cols: &[(usize, usize)]) -> bool {
        if self.options.lowering == Lowering::Never {
            return false;
        }
        let &[(t, c)] = cols else {
            return false;
        };
        let col = &self.prog.sheets[t].cols[c];
        let ColKind::Computed(node) = &col.kind else {
            return false;
        };
        let nrows = self.prog.sheets[t].nrows;
        // A plan, an optimiser pass and a collect are slower than the row
        // loop for a sheet of ten rows (D73). An empty sheet has nothing to
        // calculate either way.
        if !col.elementwise
            || nrows == 0
            || (self.options.lowering == Lowering::Auto && nrows < ROW_THRESHOLD)
        {
            return false;
        }
        let Some((frame, expr)) = self.plan(t, c, node) else {
            self.attempts.borrow_mut().push((t, c, Attempt::Refused));
            return false;
        };
        // Whole or nothing: where the result is not one to believe it is
        // discarded, no diagnostic is filed for the attempt, and the group
        // is calculated again row by row, which is what produces a per-cell
        // error with the row in it (D67).
        let Some(column) = self.lowered_column(frame, expr, nrows) else {
            self.attempts.borrow_mut().push((t, c, Attempt::Failed));
            return false;
        };
        self.held.borrow_mut()[t][c] = Some(column);
        self.attempts.borrow_mut().push((t, c, Attempt::Lowered));
        true
    }

    /// What a plan comes to, or `None` where it is not an answer to believe.
    ///
    /// A null Polars cannot account for is not an answer. A decimal product
    /// wider than the 38 digits it is held in comes back as nothing at all
    /// rather than as an error, and so does a cast that did not take; the
    /// engine answers both, one by giving up its guard digits and the other
    /// by saying what went wrong in the row it went wrong in. So the plan
    /// carries a flag beside the answer: a row whose every column was there
    /// and whose answer is nothing. Where one is found the column is given
    /// up whole, the same as a plan that will not collect. A constant that
    /// is itself nothing trips it too, and the engine gives the same answer
    /// for that a little slower.
    ///
    /// Nothing of the frame is borrowed from here on: the plan is built and
    /// the columns owned before the collect, which Polars runs on a pool of
    /// its own threads (D67's `RefCell` discipline).
    fn lowered_column(&self, frame: DataFrame, expr: Expr, len: usize) -> Option<PlColumn> {
        let unaccounted = frame
            .get_column_names()
            .into_iter()
            .fold(expr.clone().is_null(), |flag, name| {
                flag.and(polars::prelude::col(name.clone()).is_not_null())
            })
            .alias(UNACCOUNTED);
        let column = frame
            .lazy()
            .select([expr, unaccounted])
            .collect()
            .ok()
            .filter(accounted_for)
            .and_then(|df| df.get_columns().first().cloned())
            .filter(|column| column.len() == len)?;
        // Polars gives arithmetic a dtype of precision `None`; the full 38
        // is what a Parquet write and the schema in the text agree on
        // (D72a).
        match column.dtype() {
            DataType::Decimal(None, Some(scale)) => {
                column.cast(&crate::dtype::decimal(*scale as u32)).ok()
            }
            _ => Some(column),
        }
    }

    /// Whether a column that can be calculated in one step is left to be
    /// calculated for the rows shown (`Options::defer_rows`), and so is
    /// not now. Only for a `table`'s sheet of more rows than the option
    /// says, and only where the plan takes the first window of it: a column
    /// that Polars will not do is the row loop's, now and whole, as ever.
    fn try_defer(&self, cols: &[(usize, usize)]) -> bool {
        let Some(limit) = self.options.defer_rows else {
            return false;
        };
        let &[(t, c)] = cols else {
            return false;
        };
        let sheet = &self.prog.sheets[t];
        let ColKind::Computed(node) = &sheet.cols[c].kind else {
            return false;
        };
        if self.options.lowering == Lowering::Never
            || sheet.source.is_none()
            || sheet.nrows <= limit
            || !sheet.cols[c].elementwise
        {
            return false;
        }
        self.deferred.borrow_mut()[t][c] = true;
        let to = sheet.nrows.min(READ_AHEAD + PROBE_ROWS);
        if self.computed_window(t, c, node, 0, to).is_some() {
            return true;
        }
        self.deferred.borrow_mut()[t][c] = false;
        false
    }

    /// Whether column `c` of sheet `t` has been calculated over the whole
    /// column and is held so, for a test of what is left to windows.
    pub fn holds_whole(&self, t: usize, c: usize) -> bool {
        self.held.borrow()[t][c].is_some()
    }

    /// A deferred column calculated whole after all, because something
    /// asked for the whole of it or for a cell the windows do not give. It
    /// is the lowering the engine would have done when it ran, or, where
    /// that will not take it, the row loop's, cell by cell.
    fn materialise(&self, t: usize, c: usize) {
        if !self.deferred.borrow()[t][c] {
            return;
        }
        self.deferred.borrow_mut()[t][c] = false;
        self.windows.borrow_mut()[t][c] = None;
        self.run_lowered(&[(t, c)]);
    }

    /// Rows `from..to` of a deferred column, calculated by Polars over the
    /// same rows of the columns it reads, and kept as the window of it.
    fn computed_window(
        &self,
        t: usize,
        c: usize,
        node: &Node,
        from: usize,
        to: usize,
    ) -> Option<PlColumn> {
        if let Some((start, window)) = &self.windows.borrow()[t][c]
            && *start <= from
            && to <= start + window.len()
        {
            return Some(window.slice((from - start) as i64, to - from));
        }
        let col = &self.prog.sheets[t].cols[c];
        if !col.elementwise || col.zone.is_some() {
            return None;
        }
        let column = |cc: usize| self.window_column(t, cc, from, to);
        let (frame, expr) = self.plan_with(t, c, node, &column)?;
        let window = self.lowered_column(frame, expr, to - from)?;
        self.windows.borrow_mut()[t][c] = Some((from, window.clone()));
        Some(window)
    }

    /// Rows `from..to` of column `c` of sheet `t` as a Polars column: a
    /// slice of what is held whole, the rows read from a file or
    /// calculated for a window, or a slice of the whole column where it
    /// can only be had whole.
    fn window_column(&self, t: usize, c: usize, from: usize, to: usize) -> Option<PlColumn> {
        let col = &self.prog.sheets[t].cols[c];
        let len = to - from;
        if col.zone.is_some() && !matches!(col.kind, ColKind::Frame) {
            return None;
        }
        if let Some(whole) = self.held.borrow()[t][c].clone() {
            return Some(whole.slice(from as i64, len));
        }
        match (&col.kind, self.tables.get(t, c)) {
            (ColKind::Frame, Some(held @ Held::Column { .. })) => {
                if let Some((start, window)) = &self.windows.borrow()[t][c]
                    && *start <= from
                    && to <= start + window.len()
                {
                    return Some(window.slice((from - start) as i64, len));
                }
                let window = held.read_range(from, len)?;
                self.keep_window(t, c, from, window.clone());
                Some(window)
            }
            (ColKind::Frame, _) => None,
            (ColKind::Computed(node), _) if self.deferred.borrow()[t][c] => {
                self.computed_window(t, c, node, from, to)
            }
            _ => self
                .pl_column(t, c)
                .map(|whole| whole.slice(from as i64, len)),
        }
    }

    /// The rows a deferred column shows, out of its window, calculating a
    /// window around them first where none of the windows holds them.
    fn deferred_rows(&self, t: usize, c: usize, rows: &[usize]) -> Option<Vec<Value>> {
        let (&lo, &hi) = (rows.iter().min()?, rows.iter().max()?);
        let col = &self.prog.sheets[t].cols[c];
        let ColKind::Computed(node) = &col.kind else {
            return None;
        };
        let covered = matches!(
            &self.windows.borrow()[t][c],
            Some((start, window)) if *start <= lo && hi < start + window.len()
        );
        if !covered {
            let nrows = self.prog.sheets[t].nrows;
            let from = lo.saturating_sub(READ_AHEAD);
            let to = (hi + 1).saturating_add(READ_AHEAD).min(nrows);
            self.computed_window(t, c, node, from, to)?;
        }
        let windows = self.windows.borrow();
        let (start, window) = windows[t][c].as_ref()?;
        Some(
            rows.iter()
                .map(
                    |&r| match r.checked_sub(*start).filter(|&i| i < window.len()) {
                        Some(i) => match held_cell(window, i, col.ty) {
                            Cell::Empty => Value::Null,
                            Cell::Lit(l) => self.lit_in(t, c, &l),
                            Cell::Markdown(s) => Value::String(Rc::from(s.as_str()), true),
                            Cell::Invalid | Cell::Formula(_) => Value::Error,
                        },
                        None => Value::Error,
                    },
                )
                .collect(),
        )
    }

    /// The rows of a sheet a condition keeps, picked by Polars in one pass
    /// rather than by evaluating the condition for each row. `None` leaves
    /// it to the engine.
    ///
    /// What comes back is positions in `rows`, which is what the engine's
    /// own selection answers with, so nothing downstream knows the
    /// difference: a selection still makes a view of row numbers and reads
    /// no cell.
    fn run_filter(&self, t: usize, rows: &[usize], node: &Node) -> Option<Vec<usize>> {
        if self.options.lowering == Lowering::Never {
            return None;
        }
        let nrows = self.prog.sheets[t].nrows;
        if nrows == 0 || (self.options.lowering == Lowering::Auto && nrows < ROW_THRESHOLD) {
            return None;
        }
        // The whole sheet, in its own order. A condition over a selection
        // already made would have to gather those rows first, which is a
        // copy of them, and is left for when there is a reason to pay it.
        if rows.len() != nrows || rows.iter().enumerate().any(|(p, &r)| p != r) {
            return None;
        }
        let refused = |engine: &Engine| {
            engine.filters.borrow_mut().push((t, Attempt::Refused));
            None
        };
        let column = |c| self.plan_column(t, c);
        let constant = |i| self.const_value(i).ok();
        let cx = lower::Cx::new(&column, &constant);
        let Some(out) = lower::expr(node, &cx) else {
            return refused(self);
        };
        // A condition answers true, false or nothing, here and in Polars,
        // and a row is kept only where it answers true.
        if out.dtype != DataType::Boolean {
            return refused(self);
        }
        let risk = cx.at_risk();
        let columns = cx.taken();
        // A condition reading no column is the same answer for every row,
        // and the engine has it without a plan.
        if columns.is_empty() {
            return refused(self);
        }
        let Ok(frame) = DataFrame::new(columns) else {
            return refused(self);
        };
        // A row an arithmetic of the condition might not fit is one Polars
        // cannot be believed for, so the engine picks the rows.
        if risk.is_some_and(|risk| any_at_risk(&frame, risk)) {
            self.filters.borrow_mut().push((t, Attempt::Failed));
            return None;
        }
        // `#row` is a name no omasheet column can have, so it cannot be one
        // of the sheet's own.
        let kept = frame
            .lazy()
            .with_row_index(ROW_INDEX, None)
            .filter(out.expr)
            .select([col(ROW_INDEX)])
            .collect()
            .ok()
            .and_then(|df| df.column(ROW_INDEX).ok()?.u32().ok().cloned());
        let Some(kept) = kept else {
            self.filters.borrow_mut().push((t, Attempt::Failed));
            return None;
        };
        let keep: Option<Vec<usize>> = kept.into_iter().map(|p| Some(p? as usize)).collect();
        self.filters.borrow_mut().push((t, Attempt::Lowered));
        keep
    }

    /// Which rows of sheet `t` a filter keeps, in the sheet's own order:
    /// `node` is a condition checked as a row formula of the sheet
    /// (filter-by-expression D162). Polars answers it over the columns in
    /// one pass wherever it lowers, however few the rows; what does not
    /// lower, or fails in Polars, is answered a row at a time (D163). A row
    /// is kept only where the condition is true. The first error a row
    /// meets is the answer instead.
    pub fn filter_mask(&self, t: usize, node: &Node) -> Result<Vec<bool>, String> {
        let nrows = self.prog.sheets[t].nrows;
        if let Some(mask) = self.lowered_mask(t, node) {
            return Ok(mask);
        }
        let mut mask = vec![false; nrows];
        for (r, kept) in mask.iter_mut().enumerate() {
            match self.eval(node, &mut vec![(t, r)]) {
                Ok(Value::Boolean(b)) => *kept = b,
                Ok(_) => {}
                Err(Fail::Diag(d)) => return Err(d.message),
                Err(Fail::Poison) => {}
            }
        }
        Ok(mask)
    }

    fn lowered_mask(&self, t: usize, node: &Node) -> Option<Vec<bool>> {
        let (frame, expr) = self.filter_plan(t, node)?;
        run_filter_plan(frame, expr)
    }

    /// A filter lowered to Polars, and the frame of the columns it reads:
    /// what [`run_filter_plan`] runs, which a worker can, since neither
    /// holds anything of the engine. `None` where the filter does not lower.
    pub fn filter_plan(&self, t: usize, node: &Node) -> Option<(DataFrame, Expr)> {
        let nrows = self.prog.sheets[t].nrows;
        let column = |c| self.plan_column(t, c);
        let constant = |i| self.const_value(i).ok();
        let cx = lower::Cx::new(&column, &constant);
        let out = lower::expr(node, &cx)?;
        if out.dtype != DataType::Boolean {
            return None;
        }
        let risk = cx.at_risk();
        let columns = cx.taken();
        if columns.is_empty() || columns.iter().any(|c| c.len() != nrows) {
            return None;
        }
        let frame = DataFrame::new(columns).ok()?;
        if risk.is_some_and(|risk| any_at_risk(&frame, risk)) {
            return None;
        }
        Some((frame, out.expr))
    }

    /// The rows of a whole sheet in the order its key columns put them,
    /// taken by Polars. `None` leaves it to the engine, which reads the
    /// keys a cell at a time; the attempt is recorded either way, as a
    /// condition's is, a sort being the other way a selection answers
    /// positions.
    fn run_sort(&self, t: usize, rows: &[usize], keys: &[usize], desc: bool) -> Option<Vec<usize>> {
        if self.options.lowering == Lowering::Never {
            return None;
        }
        let nrows = self.prog.sheets[t].nrows;
        if nrows == 0 || (self.options.lowering == Lowering::Auto && nrows < ROW_THRESHOLD) {
            return None;
        }
        // The whole sheet, in its own order, for the reason a condition is
        // held to the same: a selection already made would have to be
        // gathered first, which is a copy of it.
        if rows.len() != nrows || rows.iter().enumerate().any(|(p, &r)| p != r) {
            return None;
        }
        let refused = |engine: &Engine| {
            engine.filters.borrow_mut().push((t, Attempt::Refused));
            None
        };
        let mut columns: Vec<PlColumn> = Vec::with_capacity(keys.len());
        let mut by: Vec<Expr> = Vec::with_capacity(keys.len());
        for &c in keys {
            let Some(column) = self.pl_column(t, c) else {
                return refused(self);
            };
            let name = column.name().clone();
            if !columns.iter().any(|held| held.name() == &name) {
                columns.push(column);
            }
            by.push(col(name));
        }
        let Ok(frame) = DataFrame::new(columns) else {
            return refused(self);
        };
        // The same order the engine's own sort puts them in:
        // `maintain_order` for the ties of D56, and nulls where D95 has
        // them — first ascending, last descending, which is the empty cell
        // being the smallest thing there is either way.
        let options = SortMultipleOptions::default()
            .with_order_descending(desc)
            .with_nulls_last(desc)
            .with_maintain_order(true);
        let order = frame
            .lazy()
            .with_row_index(ROW_INDEX, None)
            .sort_by_exprs(by, options)
            .select([col(ROW_INDEX)])
            .collect()
            .ok()
            .and_then(|df| df.column(ROW_INDEX).ok()?.u32().ok().cloned());
        let Some(order) = order else {
            self.filters.borrow_mut().push((t, Attempt::Failed));
            return None;
        };
        let order: Option<Vec<usize>> = order.into_iter().map(|p| Some(p? as usize)).collect();
        self.filters.borrow_mut().push((t, Attempt::Lowered));
        order
    }

    /// The groups of a whole sheet by its key columns, and the aggregates of
    /// `aggs` over each group, taken by Polars in one `group_by`. `aggs` is
    /// each lowerable aggregate of the grouped sheet: its column there, its
    /// function and the column of `t` it is over. `None` leaves the groups
    /// to the engine, and an aggregate missing from what comes back is one
    /// left to the engine too.
    ///
    /// The groups come back in the order the engine's own put them in: by
    /// their keys, an empty key first (D95), each group's rows in the order
    /// they came in. Two keys are one group where the engine's comparison
    /// ties them, which for a decimal is to `SHOWN_PRECISION` (D86): a key of
    /// more significant digits than that is refused rather than rounded,
    /// keys of fewer being one group exactly where they are equal.
    fn run_group(
        &self,
        t: usize,
        rows: &[usize],
        keys: &[usize],
        aggs: &[(usize, Func, usize)],
    ) -> Option<(Groups, Vec<(usize, AggValues)>)> {
        if self.options.lowering == Lowering::Never {
            return None;
        }
        let nrows = self.prog.sheets[t].nrows;
        if nrows == 0 || (self.options.lowering == Lowering::Auto && nrows < ROW_THRESHOLD) {
            return None;
        }
        // The whole sheet, in its own order, for the reason a sort is held
        // to the same.
        if rows.len() != nrows || rows.iter().enumerate().any(|(p, &r)| p != r) {
            return None;
        }
        let refused = |engine: &Engine| {
            engine.filters.borrow_mut().push((t, Attempt::Refused));
            None
        };
        let mut columns: Vec<PlColumn> = Vec::new();
        let mut take = |c: usize| -> Option<(polars::prelude::PlSmallStr, DataType)> {
            let column = self.pl_column(t, c)?;
            let (name, dtype) = (column.name().clone(), column.dtype().clone());
            if !columns.iter().any(|held| held.name() == &name) {
                columns.push(column);
            }
            Some((name, dtype))
        };
        let mut by: Vec<Expr> = Vec::with_capacity(keys.len());
        let mut key_names = Vec::with_capacity(keys.len());
        for &c in keys {
            let Some((name, _)) = take(c) else {
                return refused(self);
            };
            by.push(col(name.clone()));
            key_names.push(name);
        }
        // Each aggregate's parts, named for its place in `aggs`: the answer,
        // what a mean divides by, the extremes that bound a sum, and whether
        // a floating-point `min` has a number to be the least of (D71a), as
        // `run_agg` asks of a whole column.
        let mut select = vec![col(ROW_INDEX)];
        let mut plans = Vec::with_capacity(aggs.len());
        for (k, &(_, func, c)) in aggs.iter().enumerate() {
            let Some((name, dtype)) = take(c) else {
                continue;
            };
            let mean = func == Func::Mean;
            let Some(expr) = lower::aggregate(if mean { Func::Sum } else { func }, &name, &dtype)
            else {
                continue;
            };
            let mut parts = vec![expr.alias(format!("a{k}"))];
            if mean {
                let Some(count) = lower::aggregate(Func::Count, &name, &dtype) else {
                    continue;
                };
                parts.push(count.alias(format!("c{k}")));
            }
            let total = matches!(func, Func::Sum | Func::Mean)
                .then(|| lower::exact_scale(&dtype))
                .flatten();
            if total.is_some() {
                let (Some(lo), Some(hi)) = (
                    lower::aggregate(Func::Min, &name, &dtype),
                    lower::aggregate(Func::Max, &name, &dtype),
                ) else {
                    continue;
                };
                parts.push(lo.alias(format!("l{k}")));
                parts.push(hi.alias(format!("h{k}")));
            }
            let guard = func == Func::Min && lower::float(&dtype);
            if guard {
                parts.push(lower::has_a_number(&name).alias(format!("n{k}")));
            }
            select.extend(parts);
            plans.push((k, mean, total, guard));
        }
        let Ok(frame) = DataFrame::new(columns) else {
            return refused(self);
        };
        let options = SortMultipleOptions::default()
            .with_nulls_last(false)
            .with_maintain_order(true);
        let Ok(out) = frame
            .lazy()
            .with_row_index(ROW_INDEX, None)
            .group_by(by.clone())
            .agg(select)
            .sort_by_exprs(by, options)
            .collect()
        else {
            self.filters.borrow_mut().push((t, Attempt::Failed));
            return None;
        };
        // A decimal key of more digits than are compared would be one group
        // here and might be two there, or the other way about.
        let limit = 10i128.pow(SHOWN_PRECISION);
        for name in &key_names {
            let column = out.column(name).ok()?;
            if matches!(column.dtype(), DataType::Decimal(..)) {
                for i in 0..column.len() {
                    if let Ok(AnyValue::Decimal(n, _)) = column.get(i)
                        && n.unsigned_abs() >= limit as u128
                    {
                        return refused(self);
                    }
                }
            }
        }
        let mut groups: Groups = Vec::with_capacity(out.height());
        let listed = out.column(ROW_INDEX).ok()?.list().ok()?.clone();
        for list in listed.into_iter() {
            let list = list?;
            let mut members: Vec<usize> = list
                .u32()
                .ok()?
                .into_iter()
                .map(|p| Some(rows[p? as usize]))
                .collect::<Option<_>>()?;
            // Polars keeps no promise about the order inside a group, and
            // the engine's is the order the rows came in.
            members.sort_unstable();
            groups.push(Rc::new(members));
        }
        self.filters.borrow_mut().push((t, Attempt::Lowered));
        let ty = |c: usize| self.prog.sheets[t].cols[c].ty;
        let mut held = Vec::new();
        'agg: for (k, mean, total, guard) in plans {
            let (at, func, c) = aggs[k];
            let get = |part: &str, i: usize| out.column(&format!("{part}{k}")).ok()?.get(i).ok();
            let mut values = Vec::with_capacity(groups.len());
            for (i, group) in groups.iter().enumerate() {
                let answer = (|| {
                    if guard && !matches!(get("n", i)?, AnyValue::Boolean(true)) {
                        return None;
                    }
                    if let Some(scale) = total
                        && !has_room(get("l", i)?, get("h", i)?, group.len(), scale)?
                    {
                        return None;
                    }
                    let count = match mean {
                        true => Some(get("c", i)?.try_extract::<i64>().ok()?),
                        false => None,
                    };
                    agg_value(func, ty(c), get("a", i)?, count)
                })();
                match answer {
                    Some(v) => values.push(v),
                    None => {
                        self.aggregates.borrow_mut().push((t, c, Attempt::Refused));
                        continue 'agg;
                    }
                }
            }
            self.aggregates.borrow_mut().push((t, c, Attempt::Lowered));
            held.push((at, Rc::new(values)));
        }
        Some((groups, held))
    }

    /// The pairs of rows of a join, taken by Polars: the left sheet and the
    /// right each whole and in its own order, and the pairs in the order the
    /// engine's own join gives them, the left rows' and then the right's
    /// (`maintain_order`, D85). An empty key matches nothing, which is
    /// Polars' own default. `None` leaves the join to the engine.
    fn run_join(
        &self,
        left: &crate::value::View,
        right: &crate::value::View,
        join: &Join,
    ) -> Option<Vec<(usize, Option<usize>)>> {
        if self.options.lowering == Lowering::Never {
            return None;
        }
        let whole = |view: &crate::value::View| {
            let n = self.prog.sheets[view.sheet].nrows;
            view.rows.len() == n && view.rows.iter().enumerate().all(|(p, &r)| p == r)
        };
        let (ln, rn) = (left.rows.len(), right.rows.len());
        if ln == 0
            || rn == 0
            || (self.options.lowering == Lowering::Auto && ln.max(rn) < ROW_THRESHOLD)
            || !whole(left)
            || !whole(right)
        {
            return None;
        }
        let refused = |engine: &Engine| {
            engine
                .filters
                .borrow_mut()
                .push((left.sheet, Attempt::Refused));
            None
        };
        // Each side's keys, under the names its frame holds them by; a
        // pair of keys of two dtypes is a comparison Polars does not make
        // as the engine does.
        let side = |sheet: usize, keys: &[usize]| -> Option<(Vec<PlColumn>, Vec<Expr>)> {
            let mut columns: Vec<PlColumn> = Vec::new();
            let mut on = Vec::new();
            for &c in keys {
                let column = self.pl_column(sheet, c)?;
                on.push(col(column.name().clone()));
                if !columns.iter().any(|held| held.name() == column.name()) {
                    columns.push(column);
                }
            }
            Some((columns, on))
        };
        let (Some((lcols, lon)), Some((rcols, ron))) = (
            side(left.sheet, &join.left_keys),
            side(right.sheet, &join.right_keys),
        ) else {
            return refused(self);
        };
        let dtype =
            |sheet: usize, c: usize| self.pl_column(sheet, c).map(|col| col.dtype().clone());
        for (&l, &r) in join.left_keys.iter().zip(join.right_keys.iter()) {
            if dtype(left.sheet, l)? != dtype(right.sheet, r)? {
                return refused(self);
            }
        }
        // A decimal key of more digits than are compared is equal here and
        // perhaps not there (D86), as `run_group` refuses one.
        let limit = 10i128.pow(SHOWN_PRECISION) as u128;
        for column in lcols.iter().chain(rcols.iter()) {
            if matches!(column.dtype(), DataType::Decimal(..)) {
                for i in 0..column.len() {
                    if let Ok(AnyValue::Decimal(n, _)) = column.get(i)
                        && n.unsigned_abs() >= limit
                    {
                        return refused(self);
                    }
                }
            }
        }
        let (Ok(lframe), Ok(rframe)) = (DataFrame::new(lcols), DataFrame::new(rcols)) else {
            return refused(self);
        };
        let how = if join.left {
            JoinType::Left
        } else {
            JoinType::Inner
        };
        let args = JoinArgs {
            maintain_order: MaintainOrderJoin::LeftRight,
            ..JoinArgs::new(how)
        };
        let Ok(out) = lframe
            .lazy()
            .with_row_index(LEFT_ROW, None)
            .join(
                rframe.lazy().with_row_index(RIGHT_ROW, None),
                lon,
                ron,
                args,
            )
            .select([col(LEFT_ROW), col(RIGHT_ROW)])
            .collect()
        else {
            self.filters
                .borrow_mut()
                .push((left.sheet, Attempt::Failed));
            return None;
        };
        let ls = out.column(LEFT_ROW).ok()?.u32().ok()?.clone();
        let rs = out.column(RIGHT_ROW).ok()?.u32().ok()?.clone();
        let pairs = ls
            .into_iter()
            .zip(&rs)
            .map(|(l, r)| Some((left.rows[l? as usize], r.map(|r| right.rows[r as usize]))))
            .collect::<Option<Vec<_>>>()?;
        self.filters
            .borrow_mut()
            .push((left.sheet, Attempt::Lowered));
        Some(pairs)
    }

    /// An aggregate over one whole column of a sheet, taken by Polars.
    /// `None` leaves it to the engine, which reads the column a value at a
    /// time; nothing is reported either way, as nothing can fail.
    ///
    /// The argument has to be a whole column and nothing else: a selection
    /// of rows, a vector, several arguments together, or a block all go to
    /// the engine. Those are section 6's remaining tasks rather than this
    /// one's.
    /// The row groups of column `c` of sheet `t`, as a start and a length,
    /// where the column is to be read a group at a time for an aggregate
    /// (D155): it is a file's own column of more than one row group, which
    /// nothing holds and the document's cache does not have, and it has the
    /// rows the sheet has. Reading it whole would hold a column that the
    /// answer to the aggregate then makes of no use.
    fn stream_groups(&self, t: usize, c: usize) -> Option<Vec<(usize, usize)>> {
        let col = &self.prog.sheets[t].cols[c];
        if !matches!(col.kind, ColKind::Frame)
            || col.zone.is_some()
            || self.held.borrow()[t][c].is_some()
            || self.cells.borrow()[t][c].is_some()
        {
            return None;
        }
        let Some(
            held @ Held::Column {
                path,
                name,
                whole: None,
                cache: Some(cache),
            },
        ) = self.tables.get(t, c)
        else {
            return None;
        };
        let identity = crate::table::identity(path)?;
        if cache.holds(path, name, &identity) {
            return None;
        }
        let groups = held.groups()?;
        (groups.iter().map(|g| g.1).sum::<usize>() == self.prog.sheets[t].nrows).then_some(groups)
    }

    /// One row group of column `c` of sheet `t`, read now.
    fn read_group(&self, t: usize, c: usize, (start, len): (usize, usize)) -> Option<PlColumn> {
        let column = self.tables.get(t, c)?.read_range(start, len)?;
        (column.len() == len).then(|| {
            *self.rows_read.borrow_mut() += len;
            column
        })
    }

    /// `groups` of column `c` of sheet `t`, each read on a thread of its
    /// own, in the order given. They are taken a batch at a time, so the most
    /// held is a batch of row groups, a few MB each.
    fn read_groups(&self, t: usize, c: usize, groups: &[(usize, usize)]) -> Option<Vec<PlColumn>> {
        let held = self.tables.get(t, c)?;
        let read = |&(start, len): &(usize, usize)| {
            held.read_range(start, len).filter(|col| col.len() == len)
        };
        let out: Vec<Option<PlColumn>> = if groups.len() == 1 {
            vec![read(&groups[0])]
        } else {
            std::thread::scope(|scope| {
                let jobs: Vec<_> = groups
                    .iter()
                    .map(|group| scope.spawn(move || read(group)))
                    .collect();
                jobs.into_iter()
                    .map(|job| job.join().ok().flatten())
                    .collect()
            })
        };
        let out: Option<Vec<PlColumn>> = out.into_iter().collect();
        if let Some(out) = &out {
            *self.rows_read.borrow_mut() += out.iter().map(|col| col.len()).sum::<usize>();
        }
        out
    }

    /// The one-row frame `select` gives over the whole of a column read a
    /// group at a time: each group's, then those put together as the
    /// aggregate puts them together (a sum of sums, the least of the least
    /// ones, a count of counts), so the plan that follows is the one for a
    /// column read whole.
    #[allow(clippy::too_many_arguments)]
    fn fold_groups(
        &self,
        t: usize,
        c: usize,
        groups: &[(usize, usize)],
        first: PlColumn,
        select: &[Expr],
        func: Func,
        dtype: &DataType,
    ) -> polars::prelude::PolarsResult<DataFrame> {
        use polars::prelude::{col, polars_err};
        let mut parts: Option<DataFrame> = None;
        let mut first = Some(first);
        for batch in groups.chunks(BATCH) {
            // The first group is already read, and the rest of its batch
            // is read beside it.
            let mut columns = Vec::with_capacity(batch.len());
            let rest = match first.take() {
                Some(column) => {
                    columns.push(column);
                    &batch[1..]
                }
                None => batch,
            };
            if !rest.is_empty() {
                columns.extend(
                    self.read_groups(t, c, rest).ok_or_else(
                        || polars_err!(ComputeError: "a row group could not be read"),
                    )?,
                );
            }
            for column in columns {
                let part = DataFrame::new(vec![column])?
                    .lazy()
                    .select(select)
                    .collect()?;
                match &mut parts {
                    None => parts = Some(part),
                    Some(all) => {
                        all.vstack_mut(&part)?;
                    }
                }
            }
        }
        self.reads.borrow_mut().push((t, c));
        let parts = parts.ok_or_else(|| polars_err!(ComputeError: "no row groups"))?;
        let agg = match func {
            Func::Min => col("agg").min(),
            Func::Max if lower::float(dtype) => col("agg").nan_max(),
            Func::Max => col("agg").max(),
            _ => col("agg").sum(),
        };
        let mut combine = vec![agg.alias("agg")];
        for (name, put) in [
            ("number", col("number").any(true)),
            ("count", col("count").sum()),
            ("lo", col("lo").min()),
            ("hi", col("hi").max()),
        ] {
            if parts.get_column_names().iter().any(|n| n.as_str() == name) {
                combine.push(put.alias(name));
            }
        }
        parts.lazy().select(combine).collect()
    }

    /// The total or mean of a column of floats read a group at a time, in
    /// the order and from the start [`Engine::float_total`] has, so that the
    /// answer is the same to the last bit: the running total is carried from
    /// one group to the next.
    fn stream_float(&self, func: Func, args: &[Node], span: Span) -> Option<R> {
        use polars::prelude::DataType;
        if !matches!(func, Func::Sum | Func::Mean) {
            return None;
        }
        let [arg] = args else {
            return None;
        };
        let Ir::Column { sheet: t, col: c } = arg.kind else {
            return None;
        };
        if self.prog.sheets[t].cols[c].ty != S::Float {
            return None;
        }
        let groups = self.stream_groups(t, c)?;
        let (mut total, mut present) = (0.0_f64, 0usize);
        for batch in groups.chunks(BATCH) {
            for column in self.read_groups(t, c, batch)? {
                let series = column.as_materialized_series();
                match series.dtype() {
                    DataType::Float64 => {
                        for chunk in series.f64().ok()?.downcast_iter() {
                            for x in chunk.iter().flatten() {
                                total += *x;
                                present += 1;
                            }
                        }
                    }
                    DataType::Float32 => {
                        for chunk in series.f32().ok()?.downcast_iter() {
                            for x in chunk.iter().flatten() {
                                total += f64::from(*x);
                                present += 1;
                            }
                        }
                    }
                    // Anything else is for the walk.
                    _ => return None,
                }
            }
        }
        if present == 0 {
            return None;
        }
        self.reads.borrow_mut().push((t, c));
        self.float_totals.set(self.float_totals.get() + 1);
        Some(match func {
            Func::Sum => Ok(Value::Float(total)),
            _ => arith(
                BinOp::Div,
                &Value::Float(total),
                &Value::int(present as i64),
            )
            .or_else(|m| fail(span, m)),
        })
    }

    fn run_agg(&self, func: Func, args: &[Node]) -> Option<Value> {
        if self.options.lowering == Lowering::Never {
            return None;
        }
        let [arg] = args else {
            return None;
        };
        let Ir::Column { sheet: t, col: c } = arg.kind else {
            return None;
        };
        let nrows = self.prog.sheets[t].nrows;
        if nrows == 0 || (self.options.lowering == Lowering::Auto && nrows < ROW_THRESHOLD) {
            return None;
        }
        let refused = |engine: &Engine| {
            engine
                .aggregates
                .borrow_mut()
                .push((t, c, Attempt::Refused));
            None
        };
        // A column of a file of many row groups, not held and not wanted
        // again, is taken a group at a time (D155), so what is held is a
        // group and not the column.
        let groups = self.stream_groups(t, c);
        let Some(column) = (match &groups {
            Some(groups) => self.read_group(t, c, groups[0]),
            None => self.pl_column(t, c),
        }) else {
            return refused(self);
        };
        let (name, dtype) = (column.name().clone(), column.dtype().clone());
        // `mean` is a sum over a count, and both halves lower where the
        // whole does not (D72): Polars' own `mean` answers an exact column
        // with an `f64`, but its `sum` and its `count` are exact, and the
        // division between them is done below as the row path does it.
        let mean = func == Func::Mean;
        let Some(expr) = lower::aggregate(if mean { Func::Sum } else { func }, &name, &dtype)
        else {
            return refused(self);
        };
        // D71a: `min` over a floating-point column of nothing but `NaN`s is
        // null in Polars and the `NaN` here, so the question is asked in the
        // same plan and the column handed back where the answer is no.
        let guard = func == Func::Min && lower::float(&dtype);
        let mut select = vec![expr.alias("agg")];
        if guard {
            select.push(lower::has_a_number(&name).alias("number"));
        }
        if mean {
            // How many values it is a mean of: the row path divides by the
            // values that are not empty, which is what `count` counts.
            let Some(count) = lower::aggregate(Func::Count, &name, &dtype) else {
                return refused(self);
            };
            select.push(count.alias("count"));
        }
        // Polars adds decimals in an `i128` and wraps on overflow rather
        // than saying so, so a tall column at a wide scale sums to the
        // wrong number silently: a hundred thousand quotients held at
        // `Decimal(38, 34)` have nowhere to put the total's integer part.
        // The plan asks for the two extremes as well, `|sum|` being at most
        // the rows times the wider of them, and where that bound has no
        // room the engine's fold is the only sum that is right. The bound
        // is checked against `Dec`'s 38 digits rather than an `i128`'s 39,
        // which refuses a shade early rather than ever wrapping.
        let total = matches!(func, Func::Sum | Func::Mean)
            .then(|| lower::exact_scale(&dtype))
            .flatten();
        if total.is_some() {
            let (Some(lo), Some(hi)) = (
                lower::aggregate(Func::Min, &name, &dtype),
                lower::aggregate(Func::Max, &name, &dtype),
            ) else {
                return refused(self);
            };
            select.push(lo.alias("lo"));
            select.push(hi.alias("hi"));
        }
        let out = match &groups {
            None => DataFrame::new(vec![column])
                .ok()?
                .lazy()
                .select(select)
                .collect(),
            Some(groups) => self.fold_groups(t, c, groups, column, &select, func, &dtype),
        };
        let Ok(out) = out else {
            self.aggregates.borrow_mut().push((t, c, Attempt::Failed));
            return None;
        };
        if guard
            && !matches!(
                out.column("number").ok()?.get(0),
                Ok(AnyValue::Boolean(true))
            )
        {
            return refused(self);
        }
        if let Some(scale) = total
            && !has_room(
                out.column("lo").ok()?.get(0).ok()?,
                out.column("hi").ok()?.get(0).ok()?,
                nrows,
                scale,
            )?
        {
            return refused(self);
        }
        let any = out.column("agg").ok()?.get(0).ok()?;
        let count = match mean {
            true => Some(
                out.column("count")
                    .ok()?
                    .get(0)
                    .ok()?
                    .try_extract::<i64>()
                    .ok()?,
            ),
            false => None,
        };
        let Some(value) = agg_value(func, self.prog.sheets[t].cols[c].ty, any, count) else {
            return refused(self);
        };
        self.aggregates.borrow_mut().push((t, c, Attempt::Lowered));
        Some(value)
    }

    /// The frame a column's formula is evaluated over and the one
    /// expression that calculates it, or `None` where Polars has no answer
    /// of its own for some part of it.
    fn plan(&self, t: usize, c: usize, node: &Node) -> Option<(DataFrame, Expr)> {
        let column = |cc| self.plan_column(t, cc);
        self.plan_with(t, c, node, &column)
    }

    /// [`Engine::plan`] over the columns `column` gives: whole ones, or the
    /// same rows of each, for a window.
    fn plan_with(
        &self,
        t: usize,
        c: usize,
        node: &Node,
        column: &dyn Fn(usize) -> Option<PlColumn>,
    ) -> Option<(DataFrame, Expr)> {
        let col = &self.prog.sheets[t].cols[c];
        let constant = |i| self.const_value(i).ok();
        let cx = lower::Cx::new(column, &constant);
        let out = lower::expr(node, &cx)?;
        // The same answer the row path's `fit` gives: a column that says
        // what it holds holds it, and a lowered result of another type is
        // one the two paths would disagree about.
        if col.declared && !out.ty()?.assignable_to(col.ty) {
            return None;
        }
        // A row whose result might not fit is nothing here, and a nothing
        // with every column present is the one `run_lowered` does not
        // believe: the column is then the engine's, whole.
        let risk = cx.at_risk();
        let columns = cx.taken();
        // An expression reading no column at all is one value repeated, and
        // the row loop has already memoised it by the second row.
        if columns.is_empty() {
            return None;
        }
        let frame = DataFrame::new(columns).ok()?;
        let expr = match risk {
            Some(risk) => polars::prelude::when(risk)
                .then(polars::prelude::lit(polars::prelude::NULL).cast(out.dtype.clone()))
                .otherwise(out.expr),
            None => out.expr,
        };
        Some((frame, expr.alias(col.name.as_str())))
    }

    /// Column `c` of sheet `t` as a Polars column, in the order of its rows:
    /// what [`Engine::pl_column`] gives, for the grid's operations over a
    /// whole column (D158). `None` where the column cannot be held as one,
    /// and then the caller works a value at a time, as before.
    pub fn frame_column(&self, t: usize, c: usize) -> Option<PlColumn> {
        self.pl_column(t, c)
    }

    /// [`Engine::frame_column`], where it is the column the grid shows: a
    /// `table`'s own column, whose cells are only ever read out of it, or
    /// one no cell of which has been evaluated on its own, which would then
    /// be what the grid shows of that cell. A grid's operation that reads
    /// values out of it with [`Engine::value_of`] gives what
    /// [`Engine::column_shown`] would.
    pub fn frame_values(&self, t: usize, c: usize) -> Option<PlColumn> {
        let column = self.pl_column(t, c)?;
        let own = matches!(self.prog.sheets[t].cols[c].kind, ColKind::Frame)
            && matches!(self.tables.get(t, c), Some(Held::Column { .. }));
        (own || self.cells.borrow()[t][c].is_none()).then_some(column)
    }

    /// One literal of column `c` of sheet `t`. In a column that declares a
    /// zone a date-time is the wall clock of that zone, so it is read as an
    /// instant there; a time the zone's clocks never show is left as it is.
    fn lit_in(&self, t: usize, c: usize, l: &Lit) -> Value {
        let col = &self.prog.sheets[t].cols[c];
        if let (Some((name, span)), Lit::Datetime(at)) = (&col.zone, l)
            && let Ok(zone) = self.zone(name, *span)
            // A file's own column holds the instant, as Polars does; the
            // cells of a column of the text hold the wall clock.
            && let Some(utc) = match col.kind {
                ColKind::Frame => Some(*at),
                _ => zone.to_utc(*at),
            }
            && let Some(z) = Zoned::at(utc, &zone, &self.home)
        {
            return Value::zoned(z);
        }
        lit_value(l, col.ty)
    }

    /// A value of column `c` of sheet `t`, as it is held by Polars, read as
    /// [`Engine::column_shown`] reads it.
    pub fn value_of(&self, t: usize, c: usize, value: AnyValue) -> Value {
        let ty = self.prog.sheets[t].cols[c].ty;
        match cell_of(value, ty) {
            Cell::Empty => Value::Null,
            Cell::Lit(l) => self.lit_in(t, c, &l),
            _ => Value::Error,
        }
    }

    /// A column for a lowered plan to read. That is [`Engine::pl_column`],
    /// and for a column of the text that declares a zone, the instants its
    /// cells are, with the zone on the dtype; the column is read only to be
    /// compared with another of the same zone, and nothing of it is read
    /// back out of the copy.
    fn plan_column(&self, t: usize, c: usize) -> Option<PlColumn> {
        let col = &self.prog.sheets[t].cols[c];
        let (Some((zone, _)), false) = (&col.zone, matches!(col.kind, ColKind::Frame)) else {
            return self.pl_column(t, c);
        };
        if let Some(copy) = self.copies.borrow()[t][c].clone() {
            return Some(copy);
        }
        let values: Vec<Value> = (0..self.prog.sheets[t].nrows)
            .map(|r| self.cell_shown(t, c, r))
            .collect();
        let built = crate::dtype::zoned_column(&col.name, zone, &values)?;
        self.copies.borrow_mut()[t][c] = Some(built.clone());
        Some(built)
    }

    /// One column of a sheet as the dataframe holds it: a `table`'s own
    /// column, a column already calculated in one step, or one built here
    /// out of calculated values. `None` for a column that has no one dtype
    /// to be held as, or that holds a value which does not fit the type the
    /// column says it has.
    fn pl_column(&self, t: usize, c: usize) -> Option<PlColumn> {
        // Asked for whole, a column left to its windows is calculated.
        self.materialise(t, c);
        // A declared zone puts the cells of the text on the clocks of that
        // zone, so the instant is not the number a dataframe would hold them
        // as; the values of such a column are the engine's own. A file's
        // own zoned column is held as the instants Polars holds.
        let col = &self.prog.sheets[t].cols[c];
        if col.zone.is_some() && !matches!(col.kind, ColKind::Frame) {
            return None;
        }
        // Cloning a Polars column clones the handles, not the buffers.
        if let Some(held) = self.held.borrow()[t][c].clone() {
            return Some(held);
        }
        match self.tables.get(t, c) {
            // The file's own column, read now and kept: the dataframe is
            // the authority for it, as it is for a lowered one.
            Some(held @ Held::Column { .. }) => {
                let column = held.read()?;
                self.reads.borrow_mut().push((t, c));
                *self.rows_read.borrow_mut() += column.len();
                self.held.borrow_mut()[t][c] = Some(column.clone());
                // The whole column answers every row a range did.
                self.windows.borrow_mut()[t][c] = None;
                return Some(column);
            }
            // The file has no such column, so every cell of it is in error.
            Some(Held::Absent) => return None,
            None => {}
        }
        if let Some(copy) = self.copies.borrow()[t][c].clone() {
            return Some(copy);
        }
        let col = &self.prog.sheets[t].cols[c];
        let values: Vec<Value> = (0..self.prog.sheets[t].nrows)
            .map(|r| self.cell_shown(t, c, r))
            .collect();
        // A column holding an error is not lowered over: an error is poison
        // with a diagnostic already filed, and a dataframe has no element
        // for one (D67).
        if values.iter().any(|v| matches!(v, Value::Error)) {
            return None;
        }
        let built = column_of(&col.name, col.ty, &values)?;
        self.copies.borrow_mut()[t][c] = Some(built.clone());
        Some(built)
    }

    /// Diagnostics raised during evaluation so far.
    pub fn take_diags(&self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.diags.borrow_mut())
    }

    /// Evaluate a standalone expression, reporting any failure.
    pub fn eval_top(&self, node: &Node) -> Option<Value> {
        match self.eval(node, &mut Vec::new()) {
            Ok(v) => Some(v),
            Err(Fail::Diag(d)) => {
                self.diags.borrow_mut().push(d);
                None
            }
            Err(Fail::Poison) => None,
        }
    }

    fn full_view(&self, t: usize) -> View {
        View {
            sheet: t,
            rows: self.all_rows[t].clone(),
            cols: self.all_cols[t].clone(),
        }
    }

    // ---- cells and constants ----------------------------------------------

    /// The value of one cell, calculating it if needed.
    pub fn cell(&self, t: usize, c: usize, r: usize) -> R {
        let col = &self.prog.sheets[t].cols[c];
        // A cell on its own is not a window's worth, and its neighbours
        // are likely to be asked for next.
        self.materialise(t, c);
        {
            let mut cells = self.cells.borrow_mut();
            let slots =
                cells[t][c].get_or_insert_with(|| vec![Slot::Pending; self.prog.sheets[t].nrows]);
            // Taken out of the slot, so that marking it `Busy` below is not
            // a second borrow of what the match is reading.
            match slots[r].clone() {
                Slot::Done(Value::Error) => return Err(Fail::Poison),
                Slot::Done(v) => return Ok(v),
                Slot::Busy => {
                    return fail(
                        col.span,
                        format!("circular reference in column `{}` at row {r}", col.name),
                    );
                }
                Slot::Pending => slots[r] = Slot::Busy,
            }
        }
        let formula = |node: &Node| {
            let v = self.eval(node, &mut vec![(t, r)])?;
            self.fit(v, col, node.span)
        };
        // One cell, however it is held. A row nothing reaches is empty, as a
        // short row in the text is.
        let value_of = |cell: Option<&Cell>| match cell {
            None | Some(Cell::Empty) => Ok(Value::Null),
            Some(Cell::Lit(l)) => Ok(self.lit_in(t, c, l)),
            Some(Cell::Markdown(s)) => Ok(Value::String(Rc::from(s.as_str()), true)),
            Some(Cell::Formula(node)) => formula(node),
            Some(Cell::Invalid) => Err(Fail::Poison),
        };
        // A value the dataframe is the authority for is made into a cell as
        // it is asked for, and goes through the same reading as a cell of
        // the text, so that the two cannot come to disagree. A copy made
        // for a plan is not here, so nothing a copy cannot carry is read
        // back out of one. The borrow is dropped before the cell is read:
        // reading one evaluates nothing, but a borrow held into `value_of`
        // would be a borrow held into the engine.
        let held = self.held.borrow()[t][c]
            .as_ref()
            .map(|column| held_cell(column, r, col.ty));
        let result = match (held, &col.kind) {
            // A column calculated in one step: its values are in the
            // dataframe, so the formula is not evaluated again here.
            (Some(cell), _) => value_of(Some(&cell)),
            (None, ColKind::Computed(node)) => formula(node),
            (None, ColKind::Data(cells)) => value_of(cells.get(r)),
            // A frame column's values are where the source holds them. The
            // column is read from the file the first time a cell of it is
            // asked for, and kept from then on; a file that will not give
            // it any more is in error rather than empty, as a column the
            // file never had is.
            // A view's cell is the source sheet's own, at the row the view
            // keeps: nothing of it is stored here (D91).
            (None, ColKind::View(source)) => match self.view(t) {
                Ok(view) => match view.rows.get(r) {
                    Some(&row) => self.cell(view.sheet, *source, row),
                    None => Ok(Value::Null),
                },
                Err(why) => Err(why),
            },
            // An aggregate is worked out over the rows of its group, which
            // the names of the grouped sheet's columns read.
            (None, ColKind::Agg(node)) => match self.group(t, r) {
                // Taken by Polars with the groups.
                Ok(Some(_)) if self.agg_held.borrow()[t][c].is_some() => {
                    let held = self.agg_held.borrow()[t][c].clone();
                    match held.and_then(|values| values.get(r).cloned()) {
                        Some(v) => self.fit(v, col, node.span),
                        None => Ok(Value::Null),
                    }
                }
                Ok(Some(rows)) => {
                    self.grouping.borrow_mut().push(rows);
                    let v = self.eval(node, &mut Vec::new());
                    self.grouping.borrow_mut().pop();
                    v.and_then(|v| self.fit(v, col, node.span))
                }
                Ok(None) => Ok(Value::Null),
                Err(why) => Err(why),
            },
            // A column of the right sheet of a join, at the row of it this
            // row pairs with, and empty where it pairs with none.
            (None, ColKind::Joined(source)) => match self.view(t) {
                Ok(_) => {
                    let paired = self.joined.borrow()[t].clone();
                    match paired.and_then(|(sheet, rows)| Some((sheet, (*rows.get(r)?)?))) {
                        Some((sheet, row)) => self.cell(sheet, *source, row),
                        None => Ok(Value::Null),
                    }
                }
                Err(why) => Err(why),
            },
            (None, ColKind::Frame) => {
                let held = match self.tables.get(t, c) {
                    Some(Held::Column { .. }) => Some(match self.windowed(t, c, r) {
                        // Read for a row range that holds this row.
                        Some(cell) => cell,
                        None => self
                            .pl_column(t, c)
                            .map_or(Cell::Invalid, |column| held_cell(&column, r, col.ty)),
                    }),
                    // The file has no such column: said once as a
                    // diagnostic, and in error here.
                    Some(Held::Absent) => Some(Cell::Invalid),
                    None => None,
                };
                value_of(held.as_ref())
            }
        };
        let value = match result {
            Ok(v) => v,
            Err(Fail::Diag(d)) => {
                self.diags.borrow_mut().push(d);
                Value::Error
            }
            Err(Fail::Poison) => Value::Error,
        };
        if let Some(slots) = self.cells.borrow_mut()[t][c].as_mut() {
            slots[r] = Slot::Done(value.clone());
        }
        match value {
            Value::Error => Err(Fail::Poison),
            v => Ok(v),
        }
    }

    /// Read `rows` of a `table`'s column out of its file, where they are
    /// fewer than the whole of it and the column has not been read yet: the
    /// span from the first of them to the last is read, and only the row
    /// groups that hold it (D78). A column already read whole, or a range
    /// already read that holds these rows, is left as it is.
    fn read_range(&self, t: usize, c: usize, rows: &[usize]) {
        self.read_range_ahead(t, c, rows, 0);
    }

    /// As [`Engine::read_range`], but reading `ahead` rows more on each side
    /// of the rows asked for. A grid asks for a screen at a time, a few rows
    /// further on at each step of a scroll, and a window that is exactly
    /// the screen is walked off the end of by the next step: every frame
    /// would read the row group again. With rows read ahead the next many
    /// frames are inside the window and read nothing. The window stays a
    /// range, never the column (D78, D150).
    fn read_range_ahead(&self, t: usize, c: usize, rows: &[usize], ahead: usize) {
        let Some((from, to)) = self.window_wanted(t, c, rows, ahead) else {
            return;
        };
        let Some(held) = self.tables.get(t, c) else {
            return;
        };
        // A file that will not give the range leaves the cells to the
        // whole read, which says so as it would have.
        let Some(window) = held.read_range(from, to - from) else {
            return;
        };
        self.keep_window(t, c, from, window);
    }

    /// The rows `from..to` that column `c` of sheet `t` has to read to show
    /// `rows` with `ahead` rows more on each side, or `None` where it has
    /// nothing to read: not a column of a file, held already, covering the
    /// rows already, or the rows are the whole sheet.
    fn window_wanted(
        &self,
        t: usize,
        c: usize,
        rows: &[usize],
        ahead: usize,
    ) -> Option<(usize, usize)> {
        let (&lo, &hi) = (rows.iter().min()?, rows.iter().max()?);
        let nrows = self.prog.sheets[t].nrows;
        if hi + 1 - lo >= nrows || self.held.borrow()[t][c].is_some() {
            return None;
        }
        let Some(Held::Column { .. }) = self.tables.get(t, c) else {
            return None;
        };
        if let Some((start, window)) = &self.windows.borrow()[t][c]
            && *start <= lo
            && hi < start + window.len()
        {
            return None;
        }
        Some((
            lo.saturating_sub(ahead),
            (hi + 1).saturating_add(ahead).min(nrows),
        ))
    }

    fn keep_window(&self, t: usize, c: usize, from: usize, window: PlColumn) {
        self.reads.borrow_mut().push((t, c));
        *self.rows_read.borrow_mut() += window.len();
        self.windows.borrow_mut()[t][c] = Some((from, window));
    }

    /// Read, on several threads, the windows that showing `rows` of sheet
    /// `t` needs of its columns, so that [`Engine::rows_shown`] finds each
    /// of them read (D80). A screen of a table of 24 columns reads 24
    /// windows, which are independent files' worth of decoding, and one
    /// after another they were most of the 50–70 ms a jump cost. The
    /// answers are the same ones: each window is what `rows_shown` would
    /// have read, only read beside the others. With fewer than two to read
    /// there is nothing to share and `rows_shown` reads it itself.
    pub fn prefetch_rows(&self, t: usize, rows: &[usize]) {
        let wanted: Vec<(usize, usize, usize, &Held)> = (0..self.prog.sheets[t].cols.len())
            .filter(|&c| {
                // The columns `rows_shown` reads as a range.
                matches!(self.prog.sheets[t].cols[c].kind, ColKind::Frame)
                    && self.cells.borrow()[t][c].is_none()
            })
            .filter_map(|c| {
                let (from, to) = self.window_wanted(t, c, rows, READ_AHEAD)?;
                Some((c, from, to, self.tables.get(t, c)?))
            })
            .collect();
        if wanted.len() < 2 {
            return;
        }
        let workers = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(wanted.len());
        let mut read: Vec<(usize, usize, Option<PlColumn>)> = Vec::with_capacity(wanted.len());
        std::thread::scope(|scope| {
            let handles: Vec<_> = wanted
                .chunks(wanted.len().div_ceil(workers))
                .map(|chunk| {
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .map(|&(c, from, to, held)| (c, from, held.read_range(from, to - from)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for handle in handles {
                // A thread that panicked read nothing: its columns are read
                // again by `rows_shown`, which says what went wrong.
                read.extend(handle.join().unwrap_or_default());
            }
        });
        for (c, from, window) in read {
            if let Some(window) = window {
                self.keep_window(t, c, from, window);
            }
        }
    }

    /// Row `r` of a `table`'s column, out of the range last read of it,
    /// and `None` where that range does not hold the row.
    fn windowed(&self, t: usize, c: usize, r: usize) -> Option<Cell> {
        let windows = self.windows.borrow();
        let (start, window) = windows[t][c].as_ref()?;
        let i = r.checked_sub(*start).filter(|&i| i < window.len())?;
        Some(held_cell(window, i, self.prog.sheets[t].cols[c].ty))
    }

    /// A cell for display: failures show as an error value.
    pub fn cell_shown(&self, t: usize, c: usize, r: usize) -> Value {
        match self.cell(t, c, r) {
            Ok(v) => v,
            Err(_) => Value::Error,
        }
    }

    /// Some rows of a column for display, as [`Engine::cell_shown`] gives
    /// each of them, for a grid that shows a big sheet a page at a time
    /// (D80).
    ///
    /// A `table`'s own column not yet read is read for these rows alone,
    /// out of the row groups that hold them, and its cells are made from
    /// that range without the memoisation slots a cell asked for one at a
    /// time would make — a slot for every row of the file, per column, is
    /// what a page of it is meant not to cost. Any other column, or a row
    /// the range read does not hold, is the cell as `cell_shown` has it.
    pub fn rows_shown(&self, t: usize, c: usize, rows: &[usize]) -> Vec<Value> {
        let col = &self.prog.sheets[t].cols[c];
        // A column left to its windows is calculated for these rows alone,
        // or, if its window will not be had, calculated whole.
        if self.deferred.borrow()[t][c] {
            if let Some(values) = self.deferred_rows(t, c, rows) {
                return values;
            }
            self.materialise(t, c);
        }
        let own = matches!(col.kind, ColKind::Frame)
            && matches!(self.tables.get(t, c), Some(Held::Column { .. }))
            && self.cells.borrow()[t][c].is_none();
        if !own {
            return rows.iter().map(|&r| self.cell_shown(t, c, r)).collect();
        }
        self.read_range_ahead(t, c, rows, READ_AHEAD);
        let value = |cell: Cell| match cell {
            Cell::Empty => Value::Null,
            Cell::Lit(l) => self.lit_in(t, c, &l),
            Cell::Markdown(s) => Value::String(Rc::from(s.as_str()), true),
            Cell::Invalid | Cell::Formula(_) => Value::Error,
        };
        rows.iter()
            .map(|&r| {
                let whole = self.held.borrow()[t][c].clone();
                match (whole, self.windowed(t, c, r)) {
                    (Some(column), _) => value(held_cell(&column, r, col.ty)),
                    (None, Some(cell)) => value(cell),
                    // Rows from one end of the file to the other are read
                    // as the whole column, which is then kept.
                    (None, None) => match self.pl_column(t, c) {
                        Some(column) => value(held_cell(&column, r, col.ty)),
                        None => Value::Error,
                    },
                }
            })
            .collect()
    }

    /// Every row of a column at once, where taking it at once is quicker
    /// than a cell at a time, and `None` where it is not (D75).
    ///
    /// `Some` is a column the dataframe is the authority for: one calculated
    /// in one step, or a `table`'s own. It is walked in order here rather
    /// than asked for a row at a time, because a Polars column is held in
    /// chunks, so asking it for row `r` finds the chunk that holds `r`
    /// first where walking it does not — and since task 6.2 a `table`'s
    /// column is not calculated by [`Engine::run`] at all, so the snapshot
    /// is what touches it first and pays that per row. Measured over a
    /// sheet whose columns all lower, this is 12% of the snapshot at 10⁵
    /// rows and 13% at 10⁶.
    ///
    /// `None` is every other column, and it is not a failure: those values
    /// are in the memoisation slots already, [`Engine::cell_shown`] is a
    /// slot away from them, and collecting a million of them into a vector
    /// first was measurably *slower* than reading them one at a time — 8%
    /// of the snapshot on the same sheet, 16% on `budget.omx`. So the
    /// caller reads those cell by cell, and this says which they are.
    ///
    /// Where it answers, the values are the same values: the same
    /// [`cell_of`] and the same reading of the literal it gives, and a cell
    /// already read is given back as it was read, so the two paths cannot
    /// come to disagree. What it does **not** do is fill the slots. A column
    /// the dataframe holds needs none — the dataframe is the authority, and
    /// reading it twice gives the same answer twice — and making them for a
    /// sheet of a million rows is the cost those slots are made per column
    /// to avoid.
    pub fn column_shown(&self, t: usize, c: usize) -> Option<Vec<Value>> {
        let sheet = &self.prog.sheets[t];
        let (nrows, ty) = (sheet.nrows, sheet.cols[c].ty);
        // The borrow is a statement of its own: `pl_column` borrows `held`
        // mutably, and a temporary held into the match below would still be
        // holding it.
        let got = self.held.borrow()[t][c].clone();
        let held = match got {
            Some(column) => Some(column),
            // A `table`'s column is read from its file now, as one cell of
            // it being asked for would have read it.
            None if matches!(self.tables.get(t, c), Some(Held::Column { .. })) => {
                self.pl_column(t, c)
            }
            None => None,
        };
        let column = held?;
        // In one chunk, which is what walking it in order needs. A column
        // of one chunk already — a lowered one, usually — is handed back as
        // it is; one read from a file of several row groups is a chunk per
        // group, and joining them is the one pass that asking for a row at
        // a time pays per row.
        let series = column.as_materialized_series().rechunk();
        let cells = self.cells.borrow();
        let slots = cells[t][c].as_deref();
        let mut values = Vec::with_capacity(nrows);
        for (r, value) in series.iter().take(nrows).enumerate() {
            values.push(match slots.map(|slots| &slots[r]) {
                // A cell already read is the value it was read as.
                Some(Slot::Done(v)) => v.clone(),
                // Nothing here evaluates, so nothing here can be in a
                // cycle; this is what `cell` would say if it were.
                Some(Slot::Busy) => Value::Error,
                _ => match cell_of(value, ty) {
                    Cell::Empty => Value::Null,
                    Cell::Lit(l) => self.lit_in(t, c, &l),
                    // `cell_of` gives one of the three.
                    _ => Value::Error,
                },
            });
        }
        // A row the column has not got is in error rather than empty, as
        // `held_cell` has it: a file that will not give a value is not a
        // file that gives nothing.
        values.resize(nrows, Value::Error);
        Some(values)
    }

    /// A formula's result must be one value of the column's declared type.
    /// A value in the dress of a column's type: an exact number in a
    /// `Percent` column is a percentage, in a `Fraction` column a fraction,
    /// and a percentage or a fraction in a plain number's column is the
    /// number it stands for. Anything else is left as it is, for `fit` to
    /// pass or refuse.
    fn fit(&self, v: Value, col: &Column, span: Span) -> R {
        if matches!(
            v,
            Value::Vector(_) | Value::Sheet(_) | Value::Row(_) | Value::Range { .. }
        ) {
            return fail(
                span,
                format!("a cell holds one value, but this is {}", v.kind()),
            );
        }
        // A cell wears its column's dress, inferred as much as declared.
        // `20% // 1` answers with whichever side it took, so the engine
        // used to give a column of percentages and plain numbers together,
        // where a lowered one — whose cells are read back at the column's
        // own type — gave one or the other throughout. A column has one
        // type, so it has one dress, and both paths put it on.
        let v = dressed(v, col);
        // A column with a zone shows an instant on that zone's clocks.
        let v = match (&col.zone, &v) {
            (Some((name, zspan)), Value::Datetime(_, None) | Value::Datetime(_, Some(_))) => {
                let zone = self.zone(name, *zspan)?;
                let utc = match &v {
                    Value::Datetime(wall, None) => self.home.to_utc(*wall),
                    Value::Datetime(_, Some(z)) => Some(z.utc),
                    _ => None,
                };
                match utc.and_then(|u| Zoned::at(u, &zone, &self.home)) {
                    Some(z) => Value::zoned(z),
                    None => return fail(span, "the date-time is out of range"),
                }
            }
            _ => v,
        };
        let ok = !col.declared
            || matches!(
                (&v, col.ty),
                (Value::Null, _)
                    | (_, S::Any)
                    | (
                        Value::Int(..),
                        S::Int | S::Hex | S::Octal | S::Binary | S::Decimal
                    )
                    | (Value::Decimal(_), S::Decimal)
                    | (Value::Fraction(_), S::Fraction)
                    | (Value::Percent(_), S::Percent)
                    | (Value::Float(_), S::Float)
                    | (Value::Complex(..), S::Complex)
                    | (Value::String(..), S::String | S::Url | S::Email | S::Uuid)
                    | (Value::Boolean(_), S::Boolean)
                    | (Value::Date(_), S::Date)
                    | (Value::Time(_), S::Time)
                    | (
                        Value::Datetime(_, None) | Value::Datetime(_, Some(_)),
                        S::Datetime
                    )
            );
        if ok {
            Ok(v)
        } else {
            // A division by zero is a `Float` however exact the numbers
            // divided, which an exact column cannot hold.
            let hint = match &v {
                Value::Float(f) if !f.is_finite() && col.ty.is_exact() => {
                    "; a division by zero is Inf or NaN, so declare the column `Float` \
                     or leave out its type, or guard the division"
                }
                _ => "",
            };
            fail(
                span,
                format!(
                    "column `{}` is declared `{}` but this is {}{hint}",
                    col.name,
                    col.ty,
                    v.kind()
                ),
            )
        }
    }

    /// The view of a sheet computed from an expression, evaluated once. The
    /// expression has no row around it, so it is evaluated as a const is.
    fn view(&self, t: usize) -> Result<crate::value::View, Fail> {
        let held = match &self.views.borrow()[t] {
            Slot::Done(Value::Sheet(view)) => Some(view.clone()),
            Slot::Done(_) => return Err(Fail::Poison),
            Slot::Busy => {
                return fail(
                    self.prog.sheets[t]
                        .view
                        .as_ref()
                        .map_or(Span::default(), |n| n.span),
                    format!(
                        "sheet `{}` is computed from itself",
                        self.prog.sheets[t].name
                    ),
                );
            }
            Slot::Pending => None,
        };
        if let Some(view) = held {
            return Ok(view);
        }
        let Some(node) = self.prog.sheets[t].view.as_ref() else {
            return Err(Fail::Poison);
        };
        self.views.borrow_mut()[t] = Slot::Busy;
        let value = match self.eval(node, &mut Vec::new()) {
            Ok(v) => v,
            Err(Fail::Diag(d)) => {
                self.diags.borrow_mut().push(d);
                Value::Error
            }
            Err(Fail::Poison) => Value::Error,
        };
        // A grouped sheet's view is the first row of each group, which is
        // where its key columns read, and the groups are kept for its
        // aggregates.
        let value = match (value, &self.prog.sheets[t].group) {
            (Value::Sheet(view), Some(keys)) => match self.grouped(t, &view, keys, node.span) {
                Ok((firsts, groups)) => {
                    self.groups.borrow_mut()[t] = Some(Rc::new(groups));
                    Value::Sheet(firsts)
                }
                Err(Fail::Diag(d)) => {
                    self.diags.borrow_mut().push(d);
                    Value::Error
                }
                Err(Fail::Poison) => Value::Error,
            },
            (value, _) => value,
        };
        // A joined sheet's view is the left row of each pair, which is
        // where its left columns read, and the right rows are kept for its
        // right ones.
        let value = match (value, &self.prog.sheets[t].join) {
            (Value::Sheet(view), Some(join)) => match self.joined(&view, join) {
                Ok((left, paired)) => {
                    self.joined.borrow_mut()[t] = Some(paired);
                    Value::Sheet(left)
                }
                Err(Fail::Diag(d)) => {
                    self.diags.borrow_mut().push(d);
                    Value::Error
                }
                Err(Fail::Poison) => Value::Error,
            },
            (value, _) => value,
        };
        self.views.borrow_mut()[t] = Slot::Done(value.clone());
        match value {
            Value::Sheet(view) => Ok(view),
            _ => Err(Fail::Poison),
        }
    }

    /// The rows of `view` in groups, one for each value of the key columns
    /// `keys` the rows hold, in the order a sort by the keys puts them (an
    /// empty key first, as D95 has it) and each group's rows in the order
    /// they came in. Two keys are one group where a sort would tie them,
    /// so a decimal is grouped as it is compared, to `SHOWN_PRECISION`
    /// (D86). Given back with the view of each group's first row.
    fn grouped(
        &self,
        t: usize,
        view: &crate::value::View,
        keys: &[usize],
        span: Span,
    ) -> Result<(crate::value::View, Groups), Fail> {
        let firsts = |groups: &Groups| crate::value::View {
            sheet: view.sheet,
            rows: Rc::new(groups.iter().map(|g| g[0]).collect()),
            cols: Rc::new(keys.to_vec()),
        };
        // Each aggregate that is a function of one column, which is what
        // Polars may take for every group at once.
        let aggs: Vec<(usize, Func, usize)> = self.prog.sheets[t]
            .cols
            .iter()
            .enumerate()
            .filter_map(|(at, col)| match &col.kind {
                ColKind::Agg(Node {
                    kind: Ir::Call(func, args),
                    ..
                }) => match args.as_slice() {
                    [
                        Node {
                            kind: Ir::GroupCol(c),
                            ..
                        },
                    ] => Some((at, *func, *c)),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        if let Some((groups, held)) = self.run_group(view.sheet, &view.rows, keys, &aggs) {
            let mut slots = self.agg_held.borrow_mut();
            for (at, values) in held {
                slots[t][at] = Some(values);
            }
            return Ok((firsts(&groups), groups));
        }
        let mut cells = Vec::with_capacity(view.rows.len());
        for &r in view.rows.iter() {
            let mut row = Vec::with_capacity(keys.len());
            for &c in keys {
                row.push(self.cell(view.sheet, c, r)?);
            }
            cells.push(row);
        }
        let order = ordered(&cells, false, span)?;
        let same = |a: usize, b: usize| {
            cells[a]
                .iter()
                .zip(cells[b].iter())
                .all(|(x, y)| matches!(sort_order(x, y), Ok(Ordering::Equal)))
        };
        let mut groups: Groups = Vec::new();
        let mut current: Vec<usize> = Vec::new();
        for (i, &p) in order.iter().enumerate() {
            if i > 0 && !same(order[i - 1], p) {
                groups.push(Rc::new(std::mem::take(&mut current)));
            }
            current.push(view.rows[p]);
        }
        if !current.is_empty() {
            groups.push(Rc::new(current));
        }
        Ok((firsts(&groups), groups))
    }

    /// The pairs of rows of `left` and of the sheet `join` joins it to whose
    /// keys are equal: each left row in its order, with each right row it
    /// matches in theirs, and a left join keeping a left row that matches
    /// none (D85). An empty key matches nothing, not even another empty
    /// key. Given back as the view of the left rows and the right row of
    /// each.
    fn joined(
        &self,
        left: &crate::value::View,
        join: &Join,
    ) -> Result<(crate::value::View, Paired), Fail> {
        let span = join.right.span;
        let right = match self.eval(&join.right, &mut Vec::new())? {
            Value::Sheet(view) => view,
            _ => return Err(Fail::Poison),
        };
        let pairs = match self.run_join(left, &right, join) {
            Some(pairs) => pairs,
            None => {
                let keys_of = |sheet: usize, keys: &[usize], r: usize| -> R<Vec<Value>> {
                    keys.iter().map(|&c| self.cell(sheet, c, r)).collect()
                };
                let mut rkeys = Vec::with_capacity(right.rows.len());
                for &r in right.rows.iter() {
                    rkeys.push(keys_of(right.sheet, &join.right_keys, r)?);
                }
                let order = ordered(&rkeys, false, span)?;
                let mut why = None;
                let mut against = |a: &[Value], b: &[Value]| {
                    for (x, y) in a.iter().zip(b.iter()) {
                        match sort_order(x, y) {
                            Ok(Ordering::Equal) => {}
                            Ok(o) => return o,
                            Err(m) => {
                                why.get_or_insert(m);
                                return Ordering::Equal;
                            }
                        }
                    }
                    Ordering::Equal
                };
                let mut pairs = Vec::new();
                for &l in left.rows.iter() {
                    let key = keys_of(left.sheet, &join.left_keys, l)?;
                    let before = pairs.len();
                    if !key.iter().any(|v| matches!(v, Value::Null)) {
                        // The right rows with this key are together in the
                        // sorted order, and in their own order among
                        // themselves, the sort being stable.
                        let first =
                            order.partition_point(|&p| against(&rkeys[p], &key) == Ordering::Less);
                        for &p in &order[first..] {
                            if against(&rkeys[p], &key) != Ordering::Equal {
                                break;
                            }
                            pairs.push((l, Some(right.rows[p])));
                        }
                    }
                    if join.left && pairs.len() == before {
                        pairs.push((l, None));
                    }
                }
                if let Some(m) = why {
                    return fail(span, m);
                }
                pairs
            }
        };
        let (rows, paired): (Vec<usize>, Vec<Option<usize>>) = pairs.into_iter().unzip();
        Ok((
            crate::value::View {
                sheet: left.sheet,
                rows: Rc::new(rows),
                cols: left.cols.clone(),
            },
            (right.sheet, Rc::new(paired)),
        ))
    }

    /// The rows `file_rows` of sheet `t` in partitions by the key columns
    /// `keys`: two rows are in one where a sort would tie their keys, an
    /// empty key being one partition of its own, as a group's is. Kept for
    /// a whole sheet in its own order, which is what a column of offsets
    /// over its own sheet asks for row after row.
    fn partition(
        &self,
        t: usize,
        file_rows: &[usize],
        keys: &[usize],
        span: Span,
    ) -> R<Rc<Partition>> {
        let whole = file_rows.len() == self.prog.sheets[t].nrows
            && file_rows.iter().enumerate().all(|(p, &r)| p == r);
        let key = (t, keys.to_vec());
        if whole && let Some(part) = self.partitions.borrow().get(&key) {
            return Ok(part.clone());
        }
        let mut cells = Vec::with_capacity(file_rows.len());
        for &r in file_rows {
            let mut row = Vec::with_capacity(keys.len());
            for &c in keys {
                row.push(self.cell(t, c, r)?);
            }
            cells.push(row);
        }
        let order = ordered(&cells, false, span)?;
        let same = |a: usize, b: usize| {
            cells[a]
                .iter()
                .zip(cells[b].iter())
                .all(|(x, y)| matches!(sort_order(x, y), Ok(Ordering::Equal)))
        };
        let mut of = vec![(0, 0); file_rows.len()];
        let mut parts: Vec<Vec<usize>> = Vec::new();
        for (i, &p) in order.iter().enumerate() {
            if i == 0 || !same(order[i - 1], p) {
                parts.push(Vec::new());
            }
            let g = parts.len() - 1;
            of[p] = (g, parts[g].len());
            parts[g].push(p);
        }
        let part = Rc::new(Partition { of, parts });
        if whole {
            self.partitions.borrow_mut().insert(key, part.clone());
        }
        Ok(part)
    }

    /// The sheet grouped and the rows of group `r` of grouped sheet `t`, or
    /// `None` past the last group.
    fn group(&self, t: usize, r: usize) -> Result<Option<GroupRows>, Fail> {
        let view = self.view(t)?;
        let groups = self.groups.borrow();
        Ok(groups[t]
            .as_ref()
            .and_then(|g| g.get(r).cloned())
            .map(|rows| (view.sheet, rows)))
    }

    /// The tables back, so that a probe engine can be made to settle what
    /// the checker could not and the real one can go on using them.
    pub fn into_tables(self) -> Tables {
        self.tables
    }

    /// How many rows a sheet has: a view's is the rows its expression keeps,
    /// which is not known until that is evaluated (D92).
    pub fn nrows(&self, t: usize) -> usize {
        match self.prog.sheets.get(t) {
            Some(sheet) if sheet.view.is_some() => self.view(t).map_or(0, |view| view.rows.len()),
            Some(sheet) => sheet.nrows,
            None => 0,
        }
    }

    pub fn const_value(&self, i: usize) -> R {
        let c = &self.prog.consts[i];
        match &self.consts.borrow()[i] {
            Slot::Done(Value::Error) => return Err(Fail::Poison),
            Slot::Done(v) => return Ok(v.clone()),
            Slot::Busy => {
                return fail(
                    c.node.span,
                    format!("circular reference in constant `{}`", c.name),
                );
            }
            Slot::Pending => {}
        }
        self.consts.borrow_mut()[i] = Slot::Busy;
        let value = match self.eval(&c.node, &mut Vec::new()) {
            Ok(v) => v,
            Err(Fail::Diag(d)) => {
                self.diags.borrow_mut().push(d);
                Value::Error
            }
            Err(Fail::Poison) => Value::Error,
        };
        self.consts.borrow_mut()[i] = Slot::Done(value.clone());
        match value {
            Value::Error => Err(Fail::Poison),
            v => Ok(v),
        }
    }

    /// A whole column as an aggregate walks it, where the dataframe holds it
    /// and walking it gives what [`Engine::column`] would: `None` sends the
    /// aggregate down the path of values, which is every other case.
    ///
    /// Only a column the dataframe is the authority for, which is one
    /// calculated in one step or a `table`'s own; and only where reading it
    /// cell by cell could not fail, since a failure has to surface where
    /// and as it did. So a column the file holds fewer rows of than the
    /// sheet, or one with a cell in error or being calculated, is left to
    /// the values.
    fn whole_column(&self, t: usize, c: usize) -> Option<Items> {
        let sheet = &self.prog.sheets[t];
        let nrows = sheet.nrows;
        let got = self.held.borrow()[t][c].clone();
        let column = match got {
            Some(column) => column,
            None if matches!(sheet.cols[c].kind, ColKind::Frame)
                && matches!(self.tables.get(t, c), Some(Held::Column { .. })) =>
            {
                self.pl_column(t, c)?
            }
            None => return None,
        };
        if column.len() < nrows {
            return None;
        }
        if let Some(slots) = self.cells.borrow()[t][c].as_deref()
            && slots
                .iter()
                .any(|s| matches!(s, Slot::Busy | Slot::Done(Value::Error)))
        {
            return None;
        }
        // In the chunks it is held in: a column read from a file of many row
        // groups is many chunks. The loop over floats takes them as they are;
        // the walk of values needs one, and makes it when it walks (below).
        Some(Items::Column {
            series: column.as_materialized_series().clone(),
            len: nrows,
            at: (t, c),
        })
    }

    /// The total of a whole column of floats, and how many of them were not
    /// empty, in the order the engine adds them: from `0.0`, a row at a
    /// time, each as the float it is. That is what the walk of values comes
    /// to for such a column (`arith` adds a float to the `0` it starts from
    /// as `0.0 + x`, and a float to a float as `x + y`; an empty cell is
    /// passed over and a `NaN` is a value that makes the total one), and
    /// so the answer is the same to the last bit, where adding them in
    /// another order, as Polars does, is not (D72).
    ///
    /// `None` leaves it to the walk: a column that is not a `Float` as the
    /// sheet has it, so that each cell would be read as another kind of
    /// number; one with cells already worked out, whose values are theirs;
    /// one not as long as the sheet; or one with nothing in it, whose total
    /// is a `0` and not a `0.0`.
    fn float_total(
        &self,
        series: &polars::prelude::Series,
        len: usize,
        (t, c): (usize, usize),
    ) -> Option<(f64, usize)> {
        use polars::prelude::DataType;
        if self.prog.sheets[t].cols[c].ty != S::Float
            || series.len() != len
            || self.cells.borrow()[t][c].is_some()
        {
            return None;
        }
        let (mut total, mut present) = (0.0_f64, 0usize);
        match series.dtype() {
            DataType::Float64 => {
                for chunk in series.f64().ok()?.downcast_iter() {
                    for x in chunk.iter().flatten() {
                        total += *x;
                        present += 1;
                    }
                }
            }
            DataType::Float32 => {
                for chunk in series.f32().ok()?.downcast_iter() {
                    for x in chunk.iter().flatten() {
                        total += f64::from(*x);
                        present += 1;
                    }
                }
            }
            _ => return None,
        }
        if present == 0 {
            return None;
        }
        self.float_totals.set(self.float_totals.get() + 1);
        Some((total, present))
    }

    /// How many sums and means of a column of floats were taken as a loop
    /// over the floats, which the answer cannot tell from a walk of values:
    /// for a test that it was the loop that gave it.
    pub fn float_totals(&self) -> usize {
        self.float_totals.get()
    }

    fn column(&self, t: usize, c: usize, rows: &[usize]) -> R {
        self.read_range(t, c, rows);
        let mut out = Vec::with_capacity(rows.len());
        for &r in rows {
            out.push(self.cell(t, c, r)?);
        }
        Ok(Value::Vector(Rc::new(out)))
    }

    // ---- expressions -------------------------------------------------------

    pub fn eval(&self, node: &Node, fr: &mut Frames) -> R {
        let span = node.span;
        match &node.kind {
            Ir::Error => Err(Fail::Poison),
            Ir::Lit(l) => Ok(Value::from_lit(l)),
            Ir::Const(i) => self.const_value(*i),
            Ir::Sheet(t) => Ok(Value::Sheet(self.full_view(*t))),
            Ir::Column { sheet, col } => self.column(*sheet, *col, &self.all_rows[*sheet]),
            Ir::RowCol { depth, col } => {
                let (t, r) = fr[fr.len() - 1 - depth];
                self.cell(t, *col, r)
            }
            Ir::GroupCol(col) => {
                let Some((t, rows)) = self.grouping.borrow().last().cloned() else {
                    return fail(span, "a column of a group outside `agg`");
                };
                self.column(t, *col, &rows)
            }
            Ir::RowIndex { depth } => {
                let (_, r) = fr[fr.len() - 1 - depth];
                Ok(Value::int(r as i64))
            }
            Ir::Empty => Ok(Value::Null),
            Ir::Unary(UnOp::UpTo, operand) => Ok(Value::Range {
                lo: 0,
                hi: self.int(operand, fr)?,
                after: false,
                exclusive: true,
            }),
            Ir::Unary(op, operand) => {
                let v = self.eval(operand, fr)?;
                let apply = |v: &Value| -> R {
                    match (op, v) {
                        (_, Value::Null) => Ok(Value::Null),
                        (UnOp::Not, Value::Boolean(b)) => Ok(Value::Boolean(!b)),
                        (UnOp::Neg, Value::Percent(r)) => r
                            .checked_neg()
                            .map(Value::Percent)
                            .map_or_else(|| fail(span, "the number is too large".to_string()), Ok),
                        (UnOp::Neg, Value::Fraction(r)) => r
                            .checked_neg()
                            .map(Value::Fraction)
                            .map_or_else(|| fail(span, "the number is too large".to_string()), Ok),
                        (UnOp::Neg, Value::Duration(t)) => {
                            t.checked_neg().map(Value::Duration).map_or_else(
                                || fail(span, "the duration is out of range".to_string()),
                                Ok,
                            )
                        }
                        (UnOp::Neg, v) if v.is_numeric() => {
                            let zero = match v {
                                Value::Int(_, b) => Value::Int(0, *b),
                                _ => Value::int(0),
                            };
                            arith(BinOp::Sub, &zero, v).or_else(|m| fail(span, m))
                        }
                        (UnOp::BitNot, v) => complement(v).or_else(|m| fail(span, m)),
                        (UnOp::Not, v) => fail(span, format!("cannot apply `not` to {}", v.kind())),
                        (UnOp::Neg, v) => fail(span, format!("cannot negate {}", v.kind())),
                        (UnOp::UpTo, v) => fail(span, format!("cannot count up to {}", v.kind())),
                    }
                };
                match &v {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(apply).collect::<R<Vec<_>>>()?,
                    ))),
                    single => apply(single),
                }
            }
            Ir::Binary(op, l, r) => self.binary(*op, l, r, span, fr),
            Ir::If(c, a, b) => match self.eval(c, fr)? {
                Value::Boolean(true) => self.eval(a, fr),
                Value::Boolean(false) => self.eval(b, fr),
                Value::Null => Ok(Value::Null),
                other => fail(
                    c.span,
                    format!(
                        "the condition of `if` must be Boolean, found {}",
                        other.kind()
                    ),
                ),
            },
            Ir::VecLit(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(self.eval(item, fr)?);
                }
                Ok(Value::Vector(Rc::new(out)))
            }
            Ir::Index { base, rows, cols } => {
                let b = self.eval(base, fr)?;
                self.index(b, rows, cols, span, fr)
            }
            Ir::Field { base, col } => match self.eval(base, fr)? {
                Value::Sheet(v) => self.column(v.sheet, *col, &v.rows),
                Value::Row(r) => self.cell(r.sheet, *col, r.row),
                Value::Null => Ok(Value::Null),
                other => fail(
                    span,
                    format!(
                        "a column can be read from a sheet or row, not {}",
                        other.kind()
                    ),
                ),
            },
            Ir::Call(func, args) => self.call(*func, args, span, fr),
            Ir::Apply { name, args, body } => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.eval(arg, fr)?);
                }
                self.args.borrow_mut().push(values);
                let result = self.eval(body, fr);
                self.args.borrow_mut().pop();
                // A failure inside the function is reported where it is called.
                result.map_err(|f| match f {
                    Fail::Diag(d) => Fail::Diag(Diagnostic {
                        span,
                        message: format!("in `{name}`: {}", d.message),
                        help: d.help,
                    }),
                    Fail::Poison => Fail::Poison,
                })
            }
            Ir::Arg(i) => match self.args.borrow().last().and_then(|a| a.get(*i)) {
                Some(v) => Ok(v.clone()),
                None => Err(Fail::Poison),
            },
            Ir::Single(inner) => match self.eval(inner, fr)? {
                Value::Vector(items) => match items.len() {
                    0 => Ok(Value::Null),
                    1 => Ok(items[0].clone()),
                    n => Err(Fail::Diag(
                        Diagnostic::new(
                            span,
                            format!("a cell holds one value, but this is a vector of {n}"),
                        )
                        .with_help("aggregate it, for example with `.sum()`"),
                    )),
                },
                other => Ok(other),
            },
        }
    }

    fn binary(&self, op: BinOp, l: &Node, r: &Node, span: Span, fr: &mut Frames) -> R {
        match op {
            BinOp::Fallback => {
                let left = match self.eval(l, fr)? {
                    Value::Vector(items) => match items.len() {
                        0 => Value::Null,
                        1 => items[0].clone(),
                        n => {
                            return Err(Fail::Diag(
                                Diagnostic::new(
                                    l.span,
                                    format!("expected at most one value before `//`, found {n}"),
                                )
                                .with_help("the lookup matched more than one row"),
                            ));
                        }
                    },
                    other => other,
                };
                match left {
                    Value::Null => self.eval(r, fr),
                    v => Ok(v),
                }
            }
            BinOp::Range | BinOp::RangeEx | BinOp::RangeFrom | BinOp::RangeBoth => {
                let lo = self.int(l, fr)?;
                let hi = self.int(r, fr)?;
                let (after, exclusive) = op.range_ends().unwrap_or_default();
                Ok(Value::Range {
                    lo,
                    hi,
                    after,
                    exclusive,
                })
            }
            BinOp::In => {
                let needle = self.eval(l, fr)?;
                if matches!(needle, Value::Null) {
                    return Ok(Value::Null);
                }
                match self.eval(r, fr)? {
                    Value::Vector(items) => {
                        for item in items.iter() {
                            if equal(&needle, item).or_else(|m| fail(span, m))? == Some(true) {
                                return Ok(Value::Boolean(true));
                            }
                        }
                        Ok(Value::Boolean(false))
                    }
                    Value::Range {
                        lo,
                        hi,
                        after,
                        exclusive,
                    } => Ok(Value::Boolean(match needle.as_i64() {
                        Some(n) => {
                            (n > lo || (n == lo && !after)) && (n < hi || (n == hi && !exclusive))
                        }
                        None => false,
                    })),
                    other => fail(
                        r.span,
                        format!("`in` needs a vector or range, found {}", other.kind()),
                    ),
                }
            }
            BinOp::And | BinOp::Or => {
                // Short-circuit when the left side settles it.
                let left = self.eval(l, fr)?;
                if let Value::Boolean(b) = left
                    && b == (op == BinOp::Or)
                {
                    return Ok(Value::Boolean(b));
                }
                let right = self.eval(r, fr)?;
                self.broadcast(op, &left, &right, span)
            }
            _ => {
                let left = self.eval(l, fr)?;
                let right = self.eval(r, fr)?;
                self.broadcast(op, &left, &right, span)
            }
        }
    }

    fn broadcast(&self, op: BinOp, a: &Value, b: &Value, span: Span) -> R {
        let one = |x: &Value, y: &Value| scalar_op(op, x, y).or_else(|m| fail(span, m));
        match (a, b) {
            (Value::Vector(x), Value::Vector(y)) => {
                if x.len() != y.len() {
                    return fail(
                        span,
                        format!(
                            "cannot apply `{}` to vectors of different lengths: {} and {}",
                            op.symbol(),
                            x.len(),
                            y.len()
                        ),
                    );
                }
                let out: R<Vec<_>> = x.iter().zip(y.iter()).map(|(p, q)| one(p, q)).collect();
                Ok(Value::Vector(Rc::new(out?)))
            }
            (Value::Vector(x), y) => {
                let out: R<Vec<_>> = x.iter().map(|p| one(p, y)).collect();
                Ok(Value::Vector(Rc::new(out?)))
            }
            (x, Value::Vector(y)) => {
                let out: R<Vec<_>> = y.iter().map(|q| one(x, q)).collect();
                Ok(Value::Vector(Rc::new(out?)))
            }
            (x, y) => one(x, y),
        }
    }

    fn int(&self, node: &Node, fr: &mut Frames) -> R<i64> {
        match self.eval(node, fr)?.as_i64() {
            Some(n) => Ok(n),
            None => fail(node.span, "expected a whole number here"),
        }
    }

    /// One of the tests Polars has that look at a value alone, or a text
    /// function they are written with (filter-by-expression D174). Empty
    /// of an empty cell, as Polars answers a null.
    fn test(&self, func: Func, args: &[Node], fr: &mut Frames) -> R {
        let pattern = match args.get(1) {
            Some(p) => Some(self.eval(p, fr)?),
            None => None,
        };
        // A regex is made once, not once a row.
        let regex = match (func, &pattern) {
            (Func::Matches, Some(Value::String(re, _))) => Some(
                regex::Regex::new(re)
                    .or_else(|e| fail(args[1].span, format!("this is not a regex: {e}")))?,
            ),
            _ => None,
        };
        let needles: Option<Vec<Rc<str>>> = match (func, &pattern) {
            (Func::ContainsAny, Some(Value::Vector(items))) => Some(
                items
                    .iter()
                    .filter_map(|v| match v {
                        Value::String(s, _) => Some(s.clone()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        };
        let leap = |days: i32| {
            let (y, _, _) = date::civil_from_days(days as i64);
            Value::Boolean((y % 4 == 0 && y % 100 != 0) || y % 400 == 0)
        };
        let one = |v: &Value| -> R {
            Ok(match (func, v) {
                (_, Value::Null) => Value::Null,
                (Func::IsFinite, Value::Float(f)) => Value::Boolean(f.is_finite()),
                (Func::IsInfinite, Value::Float(f)) => Value::Boolean(f.is_infinite()),
                // An exact number is never infinite.
                (Func::IsFinite, v) if v.is_numeric() => Value::Boolean(true),
                (Func::IsInfinite, v) if v.is_numeric() => Value::Boolean(false),
                (Func::IsLeapYear, Value::Date(d)) => leap(*d),
                (Func::IsLeapYear, Value::Datetime(t, None)) => leap(date::split_datetime(*t).0),
                (Func::IsLeapYear, Value::Datetime(_, Some(z))) => {
                    leap(date::split_datetime(z.wall).0)
                }
                (
                    Func::LenChars
                    | Func::ToLowercase
                    | Func::ToUppercase
                    | Func::StartsWith
                    | Func::EndsWith
                    | Func::Contains
                    | Func::Matches
                    | Func::ContainsAny,
                    Value::String(s, _),
                ) => match func {
                    Func::LenChars => Value::int(s.chars().count() as i64),
                    Func::ToLowercase => Value::text(&s.to_lowercase()),
                    Func::ToUppercase => Value::text(&s.to_uppercase()),
                    Func::Matches => regex
                        .as_ref()
                        .map_or(Value::Null, |re| Value::Boolean(re.is_match(s))),
                    Func::ContainsAny => needles.as_ref().map_or(Value::Null, |n| {
                        Value::Boolean(n.iter().any(|x| s.contains(&**x)))
                    }),
                    // Looking for nothing has no answer, as in Polars.
                    _ => match &pattern {
                        Some(Value::String(p, _)) => Value::Boolean(match func {
                            Func::StartsWith => s.starts_with(&**p),
                            Func::EndsWith => s.ends_with(&**p),
                            _ => s.contains(&**p),
                        }),
                        _ => Value::Null,
                    },
                },
                (_, other) => {
                    let wanted = match func {
                        Func::IsFinite | Func::IsInfinite => "a number",
                        Func::IsLeapYear => "a date",
                        _ => "text",
                    };
                    return fail(
                        args[0].span,
                        format!("expected {wanted}, found {}", other.kind()),
                    );
                }
            })
        };
        match self.eval(&args[0], fr)? {
            Value::Vector(items) => Ok(Value::Vector(Rc::new(
                items.iter().map(one).collect::<R<Vec<_>>>()?,
            ))),
            single => one(&single),
        }
    }

    /// `is_unique` and its kin: over a vector, each element's answer; over
    /// this row's cell, the row's answer out of its column's, which is
    /// worked out once.
    fn among(&self, func: Func, args: &[Node], fr: &mut Frames) -> R {
        if let [_, column, place] = args
            && let Ir::Column { sheet, col } = column.kind
        {
            let key = (func, sheet, col);
            let held = self
                .marks
                .borrow()
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, m)| m.clone());
            let found = match held {
                Some(found) => found,
                None => {
                    let items = match self.eval(column, fr)? {
                        Value::Vector(items) => items,
                        other => Rc::new(vec![other]),
                    };
                    let found = Rc::new(marks(func, &items));
                    self.marks.borrow_mut().push((key, found.clone()));
                    found
                }
            };
            let at = self
                .eval(place, fr)?
                .as_i64()
                .and_then(|r| usize::try_from(r).ok());
            return Ok(at
                .and_then(|r| found.get(r))
                .map_or(Value::Null, |&b| Value::Boolean(b)));
        }
        match self.eval(&args[0], fr)? {
            Value::Vector(items) => Ok(Value::Vector(Rc::new(
                marks(func, &items)
                    .into_iter()
                    .map(Value::Boolean)
                    .collect(),
            ))),
            // A value alone is the only one of its kind, and the first and
            // the last of it.
            _ => Ok(Value::Boolean(func != Func::IsDuplicated)),
        }
    }

    /// A call, where an aggregate over a whole column of a file is kept for
    /// the next time it is asked (D155): by the file and what it was when
    /// it was read, the column and its declared type, and the function. A
    /// document that edits a cell or a formula rebuilds, and without this
    /// every `const Total = Sheet.Amount.sum()` of it would add up twenty
    /// million rows again. What is kept is the answer, a value, and only
    /// the kinds of value that can be kept in a form the document shares
    /// between its rebuilds.
    fn call(&self, func: Func, args: &[Node], span: Span, fr: &mut Frames) -> R {
        let place = self.answer_place(func, args);
        if let Some((cache, path, name, identity, key)) = &place
            && let Some(kept) = cache.answer(path, name, key, identity)
        {
            self.aggregates
                .borrow_mut()
                .push((args_sheet(args), args_col(args), Attempt::Lowered));
            return Ok(kept.value());
        }
        // A column read only to be summed is not kept: its answer is, and
        // the column is 8 bytes a row for every aggregate of a session.
        let had = place
            .as_ref()
            .is_some_and(|(cache, path, name, identity, _)| cache.holds(path, name, identity));
        let at = (args_sheet(args), args_col(args));
        let was_held = place.is_none() || self.held.borrow()[at.0][at.1].is_some();
        let result = self.call_uncalculated(func, args, span, fr);
        if let (Some((cache, path, name, identity, key)), Ok(value)) = (place, &result)
            && let Some(kept) = Scalar::of(value)
        {
            cache.put_answer(&path, &name, &key, identity, kept);
            if !had {
                cache.drop_column(&path, &name);
            }
            // Nor is it held for the rest of this run: later cells that want
            // it read it again, from the cache if the budget has room.
            if !was_held {
                self.held.borrow_mut()[at.0][at.1] = None;
            }
        }
        result
    }

    /// Where an answer for `func` over `args` is kept, and what it is kept
    /// by, where it is one that can be: an aggregate of one whole column
    /// read from a file by a document that keeps its columns.
    fn answer_place(
        &self,
        func: Func,
        args: &[Node],
    ) -> Option<(ColumnCache, PathBuf, String, Identity, String)> {
        if !matches!(
            func,
            Func::Sum
                | Func::Mean
                | Func::Min
                | Func::Max
                | Func::Count
                | Func::Len
                | Func::Median
                | Func::Product
                | Func::First
                | Func::Last
        ) {
            return None;
        }
        let [arg] = args else {
            return None;
        };
        let Ir::Column { sheet: t, col: c } = arg.kind else {
            return None;
        };
        let col = &self.prog.sheets[t].cols[c];
        if !matches!(col.kind, ColKind::Frame) {
            return None;
        }
        let Some(Held::Column {
            path,
            name,
            cache: Some(cache),
            ..
        }) = self.tables.get(t, c)
        else {
            return None;
        };
        let identity = crate::table::identity(path)?;
        let key = format!("{func:?} of {}", col.ty);
        Some((cache.clone(), path.clone(), name.clone(), identity, key))
    }

    fn call_uncalculated(&self, func: Func, args: &[Node], span: Span, fr: &mut Frames) -> R {
        // An aggregate over a whole column the dataframe holds is Polars'
        // to take, which is a pass over a column rather than a value read
        // per row. Anything else falls through to the engine below.
        if let Some(done) = self.stream_float(func, args, span) {
            return done;
        }
        if let Some(value) = self.run_agg(func, args) {
            return Ok(value);
        }
        match func {
            Func::To(to) => {
                let convert =
                    |v: &Value| -> R { v.converted(to).or_else(|message| fail(span, message)) };
                match self.eval(&args[0], fr)? {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(convert).collect::<R<Vec<_>>>()?,
                    ))),
                    single => convert(&single),
                }
            }
            Func::Succ => {
                let next = |v: &Value| -> R {
                    match v {
                        Value::Null => Ok(Value::Null),
                        Value::String(s, md) => {
                            Ok(Value::String(Rc::from(succ_text(s).as_str()), *md))
                        }
                        Value::Date(d) => Ok(Value::Date(d + 1)),
                        // The next second, which is what it was before
                        // either was held any finer.
                        Value::Time(t) => Ok(Value::Time(t + date::NANOS)),
                        Value::Datetime(t, None) => Ok(Value::Datetime(t + date::MICROS, None)),
                        v if v.is_numeric() => {
                            let one = match v {
                                Value::Int(_, b) => Value::Int(1, *b),
                                _ => Value::int(1),
                            };
                            arith(BinOp::Add, v, &one).or_else(|m| fail(span, m))
                        }
                        other => fail(
                            span,
                            format!("`succ` takes a number or text, found {}", other.kind()),
                        ),
                    }
                };
                match self.eval(&args[0], fr)? {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(next).collect::<R<Vec<_>>>()?,
                    ))),
                    single => next(&single),
                }
            }
            Func::Cycle(kind) => {
                let step = match self.eval(&args[1], fr)? {
                    Value::Int(n, _) => n,
                    _ => 0,
                };
                match self.eval(&args[0], fr)? {
                    Value::Null => Ok(Value::Null),
                    Value::String(s, md) => match cycle::advance(kind, &s, step) {
                        Some(next) => Ok(Value::String(Rc::from(next.as_str()), md)),
                        None => fail(span, format!("`{s}` is not one of these names")),
                    },
                    other => fail(
                        span,
                        format!(
                            "a sequence of names steps through text, found {}",
                            other.kind()
                        ),
                    ),
                }
            }
            Func::AddMonths | Func::EndOfMonth | Func::StartOfMonth => {
                let months = match func {
                    Func::AddMonths => match self.eval(&args[1], fr)? {
                        Value::Null => return Ok(Value::Null),
                        Value::Int(n, _) => n,
                        other => {
                            return fail(
                                span,
                                format!("a number of months must be Int, found {}", other.kind()),
                            );
                        }
                    },
                    _ => 0,
                };
                // A sequence passes the day of the month to land on.
                let on = match args.get(2) {
                    Some(node) => match self.eval(node, fr)? {
                        Value::Int(n, _) => Some(n),
                        _ => None,
                    },
                    None => None,
                };
                let shift = |days: i32| match func {
                    Func::AddMonths => date::add_months(days, months, on),
                    Func::StartOfMonth => date::add_months(days, 0, Some(1)),
                    _ => date::end_of_month(days),
                };
                let moved = |v: &Value| -> R {
                    match v {
                        Value::Null => Ok(Value::Null),
                        Value::Date(d) => match shift(*d) {
                            Some(d) => Ok(Value::Date(d)),
                            None => fail(span, "this date falls outside the calendar".to_string()),
                        },
                        Value::Datetime(t, None) => {
                            let (days, time) = date::split_datetime(*t);
                            match shift(days) {
                                Some(d) => Ok(Value::Datetime(
                                    d as i64 * date::DAY_MICROS + time / date::NANOS_PER_MICRO,
                                    None,
                                )),
                                None => {
                                    fail(span, "this date falls outside the calendar".to_string())
                                }
                            }
                        }
                        other => fail(span, format!("expected a date, found {}", other.kind())),
                    }
                };
                match self.eval(&args[0], fr)? {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(moved).collect::<R<Vec<_>>>()?,
                    ))),
                    single => moved(&single),
                }
            }
            // The tests Polars has, a value at a time; empty of an empty
            // cell, as Polars answers a null (filter-by-expression D165).
            Func::IsFinite
            | Func::IsInfinite
            | Func::IsLeapYear
            | Func::LenChars
            | Func::ToLowercase
            | Func::ToUppercase
            | Func::StartsWith
            | Func::EndsWith
            | Func::Contains
            | Func::Matches
            | Func::ContainsAny => self.test(func, args, fr),
            Func::IsUnique | Func::IsDuplicated | Func::IsFirstDistinct | Func::IsLastDistinct => {
                self.among(func, args, fr)
            }
            Func::IsNull | Func::IsNan => {
                let absent = |v: &Value| -> R {
                    Ok(match (func, v) {
                        (Func::IsNull, v) => Value::Boolean(matches!(v, Value::Null)),
                        // An empty cell is missing, not a `NaN`, so it has no
                        // answer to this; Polars answers a null the same way.
                        (_, Value::Null) => Value::Null,
                        (_, v) => Value::Boolean(matches!(v, Value::Float(f) if f.is_nan())),
                    })
                };
                match self.eval(&args[0], fr)? {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(absent).collect::<R<Vec<_>>>()?,
                    ))),
                    single => absent(&single),
                }
            }
            Func::Today => Ok(Value::Date(date::split_datetime(self.now).0)),
            Func::Now => Ok(Value::Datetime(self.now, None)),
            Func::Complex => {
                let mut parts = [0.0; 2];
                for (part, arg) in parts.iter_mut().zip(args) {
                    *part = match self.eval(arg, fr)? {
                        Value::Null => return Ok(Value::Null),
                        Value::Int(n, _) => n as f64,
                        Value::Decimal(d) | Value::Fraction(d) | Value::Percent(d) => d.to_f64(),
                        Value::Float(f) => f,
                        other => {
                            return fail(
                                arg.span,
                                format!("`Complex` needs real numbers, found {}", other.kind()),
                            );
                        }
                    };
                }
                Ok(Value::Complex(parts[0], parts[1]))
            }
            Func::Pi => Ok(Value::Float(std::f64::consts::PI)),
            Func::E => Ok(Value::Float(std::f64::consts::E)),
            Func::Math(m) => {
                let apply =
                    |v: &Value| -> R { math(m, v).or_else(|message| fail(args[0].span, message)) };
                match self.eval(&args[0], fr)? {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(apply).collect::<R<Vec<_>>>()?,
                    ))),
                    single => apply(&single),
                }
            }
            Func::RoundDigits => {
                let x = self.eval(&args[0], fr)?;
                let places = match self.eval(&args[1], fr)? {
                    Value::Null => return Ok(Value::Null),
                    Value::Int(n, _) => n,
                    other => {
                        return fail(
                            args[1].span,
                            format!(
                                "expected a number of decimal places, found {}",
                                other.kind()
                            ),
                        );
                    }
                };
                let apply = |v: &Value| -> R {
                    round_digits(v, places).or_else(|message| fail(args[0].span, message))
                };
                match x {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(apply).collect::<R<Vec<_>>>()?,
                    ))),
                    single => apply(&single),
                }
            }
            Func::ToZone | Func::Utc | Func::Local | Func::Offset | Func::ZoneName => {
                let target = match func {
                    Func::ToZone => match self.eval(&args[1], fr)? {
                        Value::Null => return Ok(Value::Null),
                        Value::String(name, _) => Some(self.zone(&name, args[1].span)?),
                        other => {
                            return fail(
                                args[1].span,
                                format!("expected the name of a time zone, found {}", other.kind()),
                            );
                        }
                    },
                    Func::Utc => Some(self.zone("UTC", span)?),
                    Func::Local => Some(self.local.clone()),
                    _ => None,
                };
                let out_of_range = || fail(span, "the date-time is out of range");
                let one = |v: &Value| -> R {
                    // The instant, and the zone the value is in.
                    let (utc, wall, zone) = match v {
                        Value::Null => return Ok(Value::Null),
                        Value::Datetime(t, None) => match self.home.to_utc(*t) {
                            Some(utc) => (utc, *t, &self.home),
                            None => return out_of_range(),
                        },
                        Value::Datetime(_, Some(z)) => (z.utc, z.wall, &z.zone),
                        other => {
                            return fail(
                                args[0].span,
                                format!("expected a Datetime, found {}", other.kind()),
                            );
                        }
                    };
                    match (&target, func) {
                        // The file's own zone needs no label.
                        (Some(target), _) if **target == *self.home => self
                            .home
                            .from_utc(utc)
                            .map(|t| Value::Datetime(t, None))
                            .map_or_else(out_of_range, Ok),
                        (Some(target), _) => Zoned::at(utc, target, &self.home)
                            .map(Value::zoned)
                            .map_or_else(out_of_range, Ok),
                        (None, Func::Offset) => {
                            Ok(Value::int((wall - utc).div_euclid(date::MICROS)))
                        }
                        (None, _) => Ok(Value::text(zone.name())),
                    }
                };
                match self.eval(&args[0], fr)? {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(one).collect::<R<Vec<_>>>()?,
                    ))),
                    single => one(&single),
                }
            }
            Func::Total(unit) => {
                let total = |v: &Value| -> R {
                    match v {
                        Value::Null => Ok(Value::Null),
                        Value::Duration(n) => Ok(Value::exact(
                            Dec::from_i64(*n).checked_div(Dec::from_i64(unit)).unwrap(),
                        )),
                        other => fail(
                            args[0].span,
                            format!("expected a Duration, found {}", other.kind()),
                        ),
                    }
                };
                match self.eval(&args[0], fr)? {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(total).collect::<R<Vec<_>>>()?,
                    ))),
                    single => total(&single),
                }
            }
            Func::Year
            | Func::Month
            | Func::Day
            | Func::Weekday
            | Func::Quarter
            | Func::Week
            | Func::OrdinalDay
            | Func::Hour
            | Func::Minute
            | Func::Second
            | Func::Millisecond
            | Func::Microsecond
            | Func::Nanosecond
            | Func::DateOf
            | Func::TimeOf => {
                let part = |v: &Value| -> R {
                    let (days, time) = match v {
                        Value::Null => return Ok(Value::Null),
                        Value::Date(d) => (Some(*d), None),
                        Value::Time(t) => (None, Some(*t)),
                        Value::Datetime(t, None) => {
                            let (days, time) = date::split_datetime(*t);
                            (Some(days), Some(time))
                        }
                        Value::Datetime(_, Some(z)) => {
                            let (days, time) = date::split_datetime(z.wall);
                            (Some(days), Some(time))
                        }
                        other => {
                            return fail(
                                args[0].span,
                                format!("expected a date or time, found {}", other.kind()),
                            );
                        }
                    };
                    let ymd = days.map(|d| date::civil_from_days(d as i64));
                    let found = match func {
                        Func::Year => ymd.map(|(y, _, _)| y),
                        Func::Month => ymd.map(|(_, m, _)| m),
                        Func::Day => ymd.map(|(_, _, d)| d),
                        Func::Weekday => days.map(date::weekday),
                        Func::Quarter => ymd.map(|(_, m, _)| (m - 1) / 3 + 1),
                        Func::OrdinalDay => days.map(ordinal_day),
                        // The ISO week is the week of its Thursday, and the
                        // year it belongs to is that Thursday's year.
                        Func::Week => days.map(|d| {
                            let thursday = d + (4 - date::weekday(d)) as i32;
                            (ordinal_day(thursday) - 1) / 7 + 1
                        }),
                        Func::Hour => time.map(|t| date::hms(t).0),
                        Func::Minute => time.map(|t| date::hms(t).1),
                        Func::Second => time.map(|t| date::hms(t).2),
                        // What is left of a second, in the unit asked for.
                        Func::Millisecond => time.map(|t| date::hms(t).3 / (date::NANOS / 1_000)),
                        Func::Microsecond => time.map(|t| date::hms(t).3 / date::NANOS_PER_MICRO),
                        Func::Nanosecond => time.map(|t| date::hms(t).3),
                        Func::DateOf => {
                            return days
                                .map(Value::Date)
                                .ok_or(())
                                .or_else(|_| fail(args[0].span, "a Time has no date"));
                        }
                        _ => {
                            return time
                                .map(Value::Time)
                                .ok_or(())
                                .or_else(|_| fail(args[0].span, "a Date has no time"));
                        }
                    };
                    match found {
                        Some(n) => Ok(Value::int(n)),
                        None => fail(args[0].span, format!("{} has no such part", v.kind())),
                    }
                };
                match self.eval(&args[0], fr)? {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(part).collect::<R<Vec<_>>>()?,
                    ))),
                    single => part(&single),
                }
            }
            Func::Std | Func::Var | Func::Quantile => {
                let values = match self.eval(&args[0], fr)? {
                    Value::Null => return Ok(Value::Null),
                    Value::Vector(v) => v,
                    other => {
                        return fail(
                            args[0].span,
                            format!("expected a vector to aggregate, found {}", other.kind()),
                        );
                    }
                };
                let present: Vec<&Value> = values
                    .iter()
                    .filter(|v| !matches!(v, Value::Null))
                    .collect();
                let real = |v: &Value| -> Option<f64> {
                    match v {
                        Value::Int(i, _) => Some(*i as f64),
                        Value::Decimal(d) | Value::Fraction(d) | Value::Percent(d) => {
                            Some(d.to_f64())
                        }
                        Value::Float(f) => Some(*f),
                        _ => None,
                    }
                };
                let second = match args.get(1) {
                    None => None,
                    Some(a) => match self.eval(a, fr)? {
                        Value::Null => return Ok(Value::Null),
                        v => match real(&v) {
                            Some(n) => Some(n),
                            None => {
                                return fail(
                                    a.span,
                                    format!("expected a number, found {}", v.kind()),
                                );
                            }
                        },
                    },
                };
                if present.is_empty() {
                    return Ok(Value::Null);
                }
                if func == Func::Quantile {
                    let Some(q) = second else {
                        return fail(span, "`quantile` needs the fraction to take".to_string());
                    };
                    if !(0.0..=1.0).contains(&q) {
                        return fail(
                            args[1].span,
                            "the fraction of a `quantile` is from 0 to 1".to_string(),
                        );
                    }
                    let mut sorted = present.clone();
                    sorted.sort_by(|a, b| compare(a, b).ok().flatten().unwrap_or(Ordering::Equal));
                    // The nearest value, as Polars interpolates by default.
                    let at = (q * (sorted.len() - 1) as f64).round() as usize;
                    return Ok((*sorted[at.min(sorted.len() - 1)]).clone());
                }
                // One degree of freedom by default, the sample's, as Polars
                // has it.
                let ddof = second.unwrap_or(1.0).round() as i64;
                let n = present.len() as i64;
                if n - ddof <= 0 {
                    return Ok(Value::Null);
                }
                let fold = |op: BinOp, seed: Value, each: &dyn Fn(&Value) -> R| -> R {
                    let mut total = seed;
                    for v in &present {
                        let term = each(v)?;
                        total = arith(op, &total, &term).or_else(|m| fail(span, m))?;
                    }
                    Ok(total)
                };
                let total = fold(BinOp::Add, Value::int(0), &|v| Ok(v.clone()))?;
                let mean = arith(BinOp::Div, &total, &Value::int(n)).or_else(|m| fail(span, m))?;
                let squares = fold(BinOp::Add, Value::int(0), &|v| {
                    let d = arith(BinOp::Sub, v, &mean).or_else(|m| fail(span, m))?;
                    arith(BinOp::Mul, &d, &d).or_else(|m| fail(span, m))
                })?;
                let var = arith(BinOp::Div, &squares, &Value::int(n - ddof))
                    .or_else(|m| fail(span, m))?;
                if func == Func::Var {
                    Ok(var)
                } else {
                    math(MathFn::Sqrt, &var).or_else(|m| fail(span, m))
                }
            }
            Func::Sum
            | Func::Mean
            | Func::Min
            | Func::Max
            | Func::Count
            | Func::Len
            | Func::Median
            | Func::Product
            | Func::First
            | Func::Last => {
                // Every argument's values, taken together. A whole column
                // the dataframe holds is walked where it is rather than
                // copied out into values first, and a vector is shared
                // rather than copied (D79).
                let mut items: Vec<Items> = Vec::new();
                for arg in args {
                    if let Ir::Column { sheet: t, col: c } = arg.kind
                        && let Some(column) = self.whole_column(t, c)
                    {
                        items.push(column);
                        continue;
                    }
                    match self.eval(arg, fr)? {
                        Value::Vector(v) => items.push(Items::Many(v)),
                        Value::Sheet(v) if func == Func::Len && args.len() == 1 => {
                            return Ok(Value::int(v.rows.len() as i64));
                        }
                        // A block: its cells, row by row.
                        Value::Sheet(v) => {
                            let mut cells = Vec::with_capacity(v.rows.len() * v.cols.len());
                            for &r in v.rows.iter() {
                                for &c in v.cols.iter() {
                                    cells.push(self.cell(v.sheet, c, r)?);
                                }
                            }
                            items.push(Items::Many(Rc::new(cells)));
                        }
                        Value::Range {
                            lo,
                            hi,
                            after,
                            exclusive,
                        } => {
                            let lo = if after { lo + 1 } else { lo };
                            let hi = if exclusive { hi - 1 } else { hi };
                            items.push(Items::Many(Rc::new((lo..=hi).map(Value::int).collect())));
                        }
                        Value::Row(row) => {
                            let mut cells = Vec::with_capacity(row.cols.len());
                            for &c in row.cols.iter() {
                                cells.push(self.cell(row.sheet, c, row.row)?);
                            }
                            items.push(Items::Many(Rc::new(cells)));
                        }
                        Value::Null => {}
                        other if args.len() > 1 && !matches!(other, Value::Sheet(_)) => {
                            items.push(Items::One(other))
                        }
                        other => {
                            return fail(
                                arg.span,
                                format!("expected a vector to aggregate, found {}", other.kind()),
                            );
                        }
                    }
                }
                // A sum or a mean of a column of floats, taken as the walk
                // below takes it, and not by making a value of each cell.
                if matches!(func, Func::Sum | Func::Mean)
                    && let [Items::Column { series, len, at }] = items.as_slice()
                    && let Some((total, present)) = self.float_total(series, *len, *at)
                {
                    return match func {
                        Func::Sum => Ok(Value::Float(total)),
                        _ => arith(
                            BinOp::Div,
                            &Value::Float(total),
                            &Value::int(present as i64),
                        )
                        .or_else(|m| fail(span, m)),
                    };
                }
                // One walk over every value, nulls and all, in order: what
                // is not empty, what it is all of, and whatever `see`
                // gathers for the aggregate asked for. A column walked in
                // place converts each cell as it goes, so a second walk
                // would be a second conversion of every row.
                let (mut present, mut timed, mut percent, mut fraction) =
                    (0usize, true, true, true);
                let mut walk = |see: &mut dyn FnMut(Value)| -> Result<(), Fail> {
                    for item in &items {
                        // A cell in error poisons the column it is read
                        // from, as `cell` has it for a column of values.
                        let walked = matches!(item, Items::Column { .. });
                        let mut poisoned = false;
                        item.each(self, &mut |v| {
                            poisoned |= walked && matches!(v, Value::Error);
                            if !matches!(v, Value::Null) {
                                present += 1;
                                timed &= matches!(v, Value::Duration(_));
                                percent &= matches!(v, Value::Percent(_));
                                fraction &= matches!(v, Value::Fraction(_));
                            }
                            see(v);
                        });
                        if poisoned {
                            return Err(Fail::Poison);
                        }
                    }
                    Ok(())
                };
                // `op` over the values that are not empty, from `start`: the
                // first failure is the answer, and nothing after it counts.
                let fold = |total: &mut Result<Value, String>, op: BinOp, v: &Value| {
                    if let (Ok(t), false) = (&*total, matches!(v, Value::Null)) {
                        *total = arith(op, t, v);
                    }
                };
                // A sum from nothing: lengths of time add up to one, from
                // none, and anything else from `0`. Which it is is known only
                // once every value is seen, so both are kept while they can
                // be, and the one of time is dropped at the first value that
                // is not one.
                let mut by_int: Result<Value, String> = Ok(Value::int(0));
                let mut by_time: Option<Result<Value, String>> = Some(Ok(Value::Duration(0)));
                let mut summing = |v: Value| {
                    if matches!(v, Value::Null) {
                        return;
                    }
                    if !matches!(v, Value::Duration(_)) {
                        by_time = None;
                    }
                    if let Some(total) = &mut by_time {
                        fold(total, BinOp::Add, &v);
                    }
                    fold(&mut by_int, BinOp::Add, &v);
                };
                let mut sorted: Vec<Value> = Vec::new();
                let mut product: Result<Value, String> = Ok(Value::int(1));
                let mut first: Option<Value> = None;
                let mut last = Value::Null;
                let want = if func == Func::Min {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
                let mut best: Result<Option<Value>, String> = Ok(None);
                walk(&mut |v: Value| match func {
                    Func::Sum | Func::Mean => summing(v),
                    Func::Median if !matches!(v, Value::Null) => sorted.push(v),
                    Func::Product => fold(&mut product, BinOp::Mul, &v),
                    Func::First => {
                        first.get_or_insert(v);
                    }
                    Func::Last => last = v,
                    Func::Min | Func::Max if !matches!(v, Value::Null) => {
                        if let Ok(b) = &mut best {
                            match b {
                                None => *b = Some(v),
                                Some(held) => match compare(&v, held) {
                                    Ok(o) if o == Some(want) => *held = v,
                                    Ok(_) => {}
                                    Err(m) => best = Err(m),
                                },
                            }
                        }
                    }
                    _ => {}
                })?;
                // Lengths of time add up to one, from none.
                let timed = present > 0 && timed;
                // The sum and the average of percentages are percentages,
                // and of fractions fractions.
                let percent = present > 0 && percent;
                let fraction = present > 0 && fraction;
                let kept = |v: Value| match (percent, fraction) {
                    (true, _) => v.percent(),
                    (_, true) => v.fraction(),
                    _ => v,
                };
                let sum = || -> R {
                    match (timed, by_time) {
                        (true, Some(total)) => total,
                        _ => by_int,
                    }
                    .or_else(|m| fail(span, m))
                };
                match func {
                    Func::Count => Ok(Value::int(present as i64)),
                    // What is empty counts here, which is the whole
                    // difference between `len` and `count`.
                    Func::Len => Ok(Value::int(
                        items.iter().map(Items::len).sum::<usize>() as i64
                    )),
                    Func::Sum => sum().map(kept),
                    Func::Mean if present == 0 => Ok(Value::Null),
                    Func::Mean => arith(BinOp::Div, &sum()?, &Value::int(present as i64))
                        .map(kept)
                        .or_else(|m| fail(span, m)),
                    Func::Median if present == 0 => Ok(Value::Null),
                    Func::Median => {
                        sorted.sort_by(|a, b| {
                            compare(a, b).ok().flatten().unwrap_or(Ordering::Equal)
                        });
                        let n = sorted.len();
                        if n % 2 == 1 {
                            Ok(sorted.swap_remove(n / 2))
                        } else {
                            let pair = arith(BinOp::Add, &sorted[n / 2 - 1], &sorted[n / 2])
                                .or_else(|m| fail(span, m))?;
                            arith(BinOp::Div, &pair, &Value::int(2))
                                .map(kept)
                                .or_else(|m| fail(span, m))
                        }
                    }
                    Func::Product => product.or_else(|m| fail(span, m)),
                    // Empty or not: the first value is the first value.
                    Func::First => Ok(first.unwrap_or(Value::Null)),
                    Func::Last => Ok(last),
                    _ => Ok(best.or_else(|m| fail(span, m))?.unwrap_or(Value::Null)),
                }
            }
        }
    }

    // ---- indexing ----------------------------------------------------------

    fn offset(&self, off: &Offset, fr: &mut Frames) -> R<i64> {
        match &off.0 {
            None => Ok(0),
            Some((negative, node)) => {
                let n = self.int(node, fr)?;
                Ok(if *negative { -n } else { n })
            }
        }
    }

    /// Resolve a row selector to positions in a list of `len` rows. `sheet`
    /// gives the file rows behind each position, for conditions.
    fn rows(
        &self,
        sel: &RowSel,
        len: usize,
        sheet: Option<(usize, &[usize])>,
        span: Span,
        fr: &mut Frames,
    ) -> R<Rows> {
        let n = len as i64;
        let cursor = |fr: &Frames| match fr.first() {
            Some(&(_, r)) => Ok(r as i64),
            None => fail(span, "`*` needs a current row"),
        };
        let range = |lo: i64, hi: i64| {
            let (lo, hi) = (lo.max(0), hi.min(n - 1));
            Rows::Many((lo..=hi).map(|p| p as usize).collect())
        };
        let from_end = |p: i64| if p < 0 { p + n } else { p };
        Ok(match sel {
            RowSel::All => Rows::Many((0..len).collect()),
            RowSel::Pos(node) => {
                let p = self.int(node, fr)?;
                let at = from_end(p);
                if at < 0 || at >= n {
                    return fail(
                        node.span,
                        format!("row {p} is out of range: there are {len} rows"),
                    );
                }
                Rows::One(Some(at as usize))
            }
            RowSel::Cursor(off) => {
                let at = cursor(fr)? + self.offset(off, fr)?;
                Rows::One((at >= 0 && at < n).then_some(at as usize))
            }
            // The same move, counted among the rows of the cursor's own
            // partition: a row outside it, before its first or past its
            // last, is empty.
            RowSel::CursorBy { off, keys } => {
                let Some((t, file_rows)) = sheet else {
                    return fail(span, "`by` needs a sheet whose columns to partition by");
                };
                let p = cursor(fr)?;
                if p < 0 || p >= n {
                    return Ok(Rows::One(None));
                }
                let by = self.offset(off, fr)?;
                let part = self.partition(t, file_rows, keys, span)?;
                let (g, i) = part.of[p as usize];
                let at = i as i64 + by;
                Rows::One(
                    usize::try_from(at)
                        .ok()
                        .and_then(|at| part.parts[g].get(at).copied()),
                )
            }
            RowSel::Range {
                lo,
                hi,
                after,
                exclusive,
            } => {
                let bound = |b: &Bound, fr: &mut Frames| -> R<i64> {
                    Ok(match b {
                        Bound::Abs(node) => from_end(self.int(node, fr)?),
                        Bound::Cursor(off) => cursor(fr)? + self.offset(off, fr)?,
                    })
                };
                let lo = bound(lo, fr)?;
                let hi = bound(hi, fr)?;
                range(
                    if *after { lo + 1 } else { lo },
                    if *exclusive { hi - 1 } else { hi },
                )
            }
            RowSel::RangeVal(node) => match self.eval(node, fr)? {
                Value::Range {
                    lo,
                    hi,
                    after,
                    exclusive,
                } => {
                    let (lo, hi) = (from_end(lo), from_end(hi));
                    range(
                        if after { lo + 1 } else { lo },
                        if exclusive { hi - 1 } else { hi },
                    )
                }
                other => {
                    return fail(
                        node.span,
                        format!("expected a range, found {}", other.kind()),
                    );
                }
            },
            RowSel::Pred(node) => {
                let Some((t, file_rows)) = sheet else {
                    return fail(node.span, "a condition needs a sheet to select from");
                };
                if let Some(keep) = self.run_filter(t, file_rows, node) {
                    return Ok(Rows::Many(keep));
                }
                let mut keep = Vec::new();
                for (p, &r) in file_rows.iter().enumerate() {
                    fr.push((t, r));
                    let v = self.eval(node, fr);
                    fr.pop();
                    match v? {
                        Value::Boolean(true) => keep.push(p),
                        Value::Boolean(false) | Value::Null => {}
                        other => {
                            return fail(
                                node.span,
                                format!("a row condition must be Boolean, found {}", other.kind()),
                            );
                        }
                    }
                }
                Rows::Many(keep)
            }
            RowSel::Order { keys, desc } => {
                let Some((t, file_rows)) = sheet else {
                    return fail(span, "`sort` needs a sheet to order");
                };
                if let Some(order) = self.run_sort(t, file_rows, keys, *desc) {
                    return Ok(Rows::Many(order));
                }
                let mut cells = Vec::with_capacity(len);
                for &r in file_rows {
                    let mut row = Vec::with_capacity(keys.len());
                    for &c in keys {
                        row.push(self.cell(t, c, r)?);
                    }
                    cells.push(row);
                }
                Rows::Many(ordered(&cells, *desc, span)?)
            }
            RowSel::Mask(node) => {
                let Value::Vector(mask) = self.eval(node, fr)? else {
                    return fail(node.span, "expected a vector of Boolean");
                };
                if mask.len() != len {
                    return fail(
                        node.span,
                        format!(
                            "this mask has {} values but there are {len} rows",
                            mask.len()
                        ),
                    );
                }
                let mut keep = Vec::new();
                for (p, v) in mask.iter().enumerate() {
                    match v {
                        Value::Boolean(true) => keep.push(p),
                        Value::Boolean(false) | Value::Null => {}
                        other => {
                            return fail(
                                node.span,
                                format!("a mask must hold Boolean values, found {}", other.kind()),
                            );
                        }
                    }
                }
                Rows::Many(keep)
            }
        })
    }

    fn index(&self, base: Value, rows: &RowSel, cols: &ColSel, span: Span, fr: &mut Frames) -> R {
        match base {
            Value::Null => Ok(Value::Null),
            Value::Sheet(v) => {
                let picked = self.rows(rows, v.rows.len(), Some((v.sheet, &v.rows)), span, fr)?;
                let sub_cols = |cols: &ColSel| match cols {
                    ColSel::Many(list) => Rc::new(list.clone()),
                    _ => v.cols.clone(),
                };
                Ok(match (picked, cols) {
                    (Rows::One(None), _) => Value::Null,
                    (Rows::One(Some(p)), ColSel::One(c)) => self.cell(v.sheet, *c, v.rows[p])?,
                    (Rows::One(Some(p)), cols) => Value::Row(RowRef {
                        sheet: v.sheet,
                        row: v.rows[p],
                        cols: sub_cols(cols),
                    }),
                    (Rows::Many(ps), ColSel::One(c)) => {
                        let file_rows: Vec<usize> = ps.iter().map(|&p| v.rows[p]).collect();
                        self.column(v.sheet, *c, &file_rows)?
                    }
                    (Rows::Many(ps), cols) => Value::Sheet(View {
                        sheet: v.sheet,
                        rows: Rc::new(ps.iter().map(|&p| v.rows[p]).collect()),
                        cols: sub_cols(cols),
                    }),
                })
            }
            // A column's values are their own sort key, which the row
            // selector cannot reach: it is given the length of what it
            // selects from and not its values.
            Value::Vector(items) if matches!(rows, RowSel::Order { .. }) => {
                let RowSel::Order { desc, .. } = rows else {
                    unreachable!("matched just above")
                };
                let keys: Vec<Vec<Value>> = items.iter().map(|v| vec![v.clone()]).collect();
                let order = ordered(&keys, *desc, span)?;
                Ok(Value::Vector(Rc::new(
                    order.iter().map(|&p| items[p].clone()).collect(),
                )))
            }
            Value::Vector(items) => Ok(match self.rows(rows, items.len(), None, span, fr)? {
                Rows::One(None) => Value::Null,
                Rows::One(Some(p)) => items[p].clone(),
                Rows::Many(ps) => {
                    Value::Vector(Rc::new(ps.iter().map(|&p| items[p].clone()).collect()))
                }
            }),
            other => fail(span, format!("cannot index {}", other.kind())),
        }
    }

    // ---- display -----------------------------------------------------------

    /// The time zone called `name`, looked up once.
    fn zone(&self, name: &str, span: Span) -> R<Rc<Zone>> {
        let found = self
            .zones
            .borrow_mut()
            .entry(name.to_string())
            .or_insert_with(|| Zone::named(name).map(Rc::new))
            .clone();
        found.ok_or_else(|| Fail::Diag(no_zone(name, span)))
    }

    /// The file's own date-times in `v` on the clocks of this machine, when
    /// that was asked for and they differ.
    fn localised(&self, v: &Value) -> Option<Value> {
        if !self.options.local || *self.home == *self.local {
            return None;
        }
        match v {
            Value::Datetime(t, None) => {
                let wall = self.local.from_utc(self.home.to_utc(*t)?)?;
                Some(Value::Datetime(wall, None))
            }
            Value::Vector(items) => Some(Value::Vector(Rc::new(
                items
                    .iter()
                    .map(|x| self.localised(x).unwrap_or_else(|| x.clone()))
                    .collect(),
            ))),
            _ => None,
        }
    }

    /// A single value as text, with dates and times in the style asked for.
    pub fn show(&self, v: &Value, quoted: bool) -> String {
        let style = &self.options.style;
        let format = if self.options.exact {
            format_exact
        } else {
            format_styled
        };
        match self.localised(v) {
            Some(local) => format(&local, quoted, style),
            None => format(v, quoted, style),
        }
    }

    /// A cell's value as text, by its column's format where it has one and
    /// it shows the value. Shown in full, as `exact` asks, it has none.
    pub fn show_in(&self, v: &Value, quoted: bool, format: Option<&Format>) -> String {
        let style = match self.options.numbers {
            Some((point, sep)) => self.options.style.numbers(point, sep),
            None => self.options.style,
        };
        let shown = match format.filter(|_| !self.options.exact) {
            Some(Format::Number(f)) => format_number(v, f, &style),
            Some(Format::Date(f)) => self
                .moment(v)
                .and_then(|m| format_date(&m, f, &self.options.names)),
            None => None,
        };
        shown.unwrap_or_else(|| self.show(v, quoted))
    }

    /// A date or time as a date format shows it: a date-time on the clocks
    /// it is shown on, with where that is. `None` for any other value.
    fn moment(&self, v: &Value) -> Option<Moment> {
        let instant = |wall: i64, utc: i64, zone: &Zone| {
            Some(Instant {
                utc: utc.div_euclid(date::MICROS),
                offset: (wall - utc).div_euclid(date::MICROS),
                abbreviation: zone.abbreviation(utc)?,
            })
        };
        Some(match v {
            Value::Date(d) => Moment::date(*d),
            Value::Time(t) => Moment::time(*t),
            Value::Datetime(t, None) => {
                let (wall, zone) = match self.localised(v) {
                    Some(Value::Datetime(local, None)) => (local, &self.local),
                    _ => (*t, &self.home),
                };
                let at = zone.to_utc(wall).and_then(|utc| instant(wall, utc, zone));
                Moment::datetime(wall, at)
            }
            Value::Datetime(_, Some(z)) => {
                Moment::datetime(z.wall, instant(z.wall, z.utc, &z.zone))
            }
            _ => return None,
        })
    }

    /// The name of the time zone the file's date-times are in.
    pub fn zone_name(&self) -> &str {
        self.home.name()
    }

    /// A sheet as aligned text, in the same shape as `.omx` source.
    pub fn render_view(&self, v: &View) -> String {
        let sheet = &self.prog.sheets[v.sheet];
        let mut grid: Vec<Vec<String>> =
            vec![v.cols.iter().map(|&c| sheet.cols[c].name.clone()).collect()];
        let mut numeric = vec![true; v.cols.len()];
        for &r in v.rows.iter() {
            let mut line = Vec::with_capacity(v.cols.len());
            for (k, &c) in v.cols.iter().enumerate() {
                let value = self.cell_shown(v.sheet, c, r);
                if !matches!(value, Value::Null | Value::Error) && !value.is_numeric() {
                    numeric[k] = false;
                }
                // A row is a line, so a cell holding a newline is written
                // as the escape the file holds it by: the grid keeps its
                // shape and what is printed reads back as what it was.
                let text = self.show_in(&value, false, sheet.cols[c].format.as_ref());
                line.push(if text.contains(['\n', '\r', '\t']) {
                    format!(
                        "\"{}\"",
                        text.replace('\\', "\\\\")
                            .replace('"', "\\\"")
                            .replace('\r', "\n")
                            .replace('\n', "\\n")
                            .replace('\t', "\\t")
                    )
                } else {
                    text
                });
            }
            grid.push(line);
        }
        let widths: Vec<usize> = (0..v.cols.len())
            .map(|k| {
                grid.iter()
                    .map(|row| row[k].chars().count())
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        let mut out = String::new();
        for (i, row) in grid.iter().enumerate() {
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(k, text)| {
                    let pad = " ".repeat(widths[k] - text.chars().count());
                    if numeric[k] && i > 0 {
                        format!("{pad}{text}")
                    } else {
                        format!("{text}{pad}")
                    }
                })
                .collect();
            out.push_str(cells.join(" | ").trim_end());
            out.push('\n');
            if i == 0 {
                let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
                out.push_str(&rule.join("-|-"));
                out.push('\n');
            }
        }
        out
    }

    pub fn render_sheet(&self, t: usize) -> String {
        self.render_view(&self.full_view(t))
    }

    /// Any value as text, ending in a newline.
    pub fn render_value(&self, v: &Value) -> String {
        match v {
            Value::Sheet(view) => self.render_view(view),
            Value::Row(row) => self.render_view(&View {
                sheet: row.sheet,
                rows: Rc::new(vec![row.row]),
                cols: row.cols.clone(),
            }),
            Value::Null => "empty\n".to_string(),
            other => format!("{}\n", self.show(other, false)),
        }
    }
}

/// The positions `0..keys.len()` in the order their keys put them, each
/// key a row's cells in the key columns, compared in turn. The sort is
/// stable, so rows whose keys are equal keep the order they came in and
/// two sorts on different keys compose.
/// Whether a sum of `n` values between `lo` and `hi` has room in a `Dec`
/// at `scale`, which is what is asked before a Polars sum is believed:
/// Polars adds decimals in an `i128` and wraps on overflow rather than
/// saying so (D72d). `None` where the extremes are not decimals at all.
fn has_room(lo: AnyValue, hi: AnyValue, n: usize, scale: u32) -> Option<bool> {
    // An empty column has no extremes and sums to nothing, which has room
    // anywhere.
    let unscaled = |any: AnyValue| match any {
        AnyValue::Decimal(n, _) => Some(n),
        AnyValue::Null => Some(0),
        _ => None,
    };
    let widest = unscaled(lo)?
        .unsigned_abs()
        .max(unscaled(hi)?.unsigned_abs());
    Some(
        i128::try_from(widest)
            .ok()
            .and_then(|w| w.checked_mul(n as i128))
            .and_then(|bound| Dec::new(bound, scale as i32))
            .is_some(),
    )
}

/// What a Polars aggregate answered, as the value the row path gives:
/// `count` is how many values a `mean` divides by, its `any` being their
/// sum. `None` where it is not one the engine can take as it stands.
fn agg_value(func: Func, ty: S, any: AnyValue, count: Option<i64>) -> Option<Value> {
    // The dress a column wears is put back on, a Polars aggregate
    // answering a bare number (D69): the sum of percentages is a
    // percentage, as it is on the row path.
    let value = match func {
        // A count is a count of things, whatever the column holds.
        Func::Len | Func::Count => Value::int(any.try_extract::<i64>().ok()?),
        _ => Value::from_lit(match &cell_of(any, ty) {
            Cell::Lit(l) => l,
            _ => return None,
        }),
    };
    let value = match (ty, &value) {
        (S::Fraction, Value::Int(_, _) | Value::Decimal(_)) => value.fraction(),
        _ => value,
    };
    // The division `mean` is, taken here rather than by Polars so that it
    // is a `Dec`'s. The row path divides the sum it has already dressed and
    // puts the percentage back on after, a division being one of the
    // things that takes it off, so this does the same. Nothing to divide
    // by is a column of nothing, whose mean is empty.
    Some(match count {
        None => value,
        Some(0) => Value::Null,
        Some(n) => {
            let mean = arith(BinOp::Div, &value, &Value::int(n)).ok()?;
            match ty {
                S::Percent => mean.percent(),
                S::Fraction => mean.fraction(),
                _ => mean,
            }
        }
    })
}

fn ordered(keys: &[Vec<Value>], desc: bool, span: Span) -> R<Vec<usize>> {
    let mut why = None;
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by(|&a, &b| {
        for (x, y) in keys[a].iter().zip(keys[b].iter()) {
            match sort_order(x, y) {
                Ok(Ordering::Equal) => {}
                Ok(o) => return if desc { o.reverse() } else { o },
                Err(m) => {
                    why.get_or_insert(m);
                    return Ordering::Equal;
                }
            }
        }
        Ordering::Equal
    });
    match why {
        Some(m) => fail(span, m),
        None => Ok(order),
    }
}

/// The day of the year a date falls on, 1 to 366.
fn ordinal_day(days: i32) -> i64 {
    let (y, _, _) = date::civil_from_days(days as i64);
    days as i64 - date::days_from_civil(y, 1, 1) + 1
}

/// The flag a lowered column carries beside its answer: a row whose every
/// column was there and whose answer is nothing. The leading space is what
/// keeps it from any name a column of a sheet could have.
const UNACCOUNTED: &str = " unaccounted for";

/// Whether the test is true for any row of the frame, or cannot be had:
/// either way the plan is not one to believe.
fn any_at_risk(frame: &DataFrame, risk: Expr) -> bool {
    frame
        .clone()
        .lazy()
        .select([risk.any(true)])
        .collect()
        .ok()
        .and_then(|df| df.get_columns().first()?.bool().ok()?.get(0))
        .unwrap_or(true)
}

/// Whether every null in a lowered column is one the columns it read
/// account for. A flag that cannot be read at all counts as not.
fn accounted_for(out: &DataFrame) -> bool {
    out.column(UNACCOUNTED)
        .ok()
        .and_then(|flag| flag.as_materialized_series().bool().ok().map(|b| !b.any()))
        .unwrap_or(false)
}

/// A value in the dress its column's type wears. See [`Engine::fit`].
fn dressed(v: Value, col: &Column) -> Value {
    match col.ty {
        S::Percent if matches!(v, Value::Int(_, _) | Value::Decimal(_)) => v.percent(),
        S::Fraction if matches!(v, Value::Int(_, _) | Value::Decimal(_)) => v.fraction(),
        // A whole number wears its column's base, whichever it was
        // written or worked out in.
        t if t.is_int() || t == S::Decimal => match v {
            Value::Percent(r) | Value::Fraction(r) => Value::exact(r),
            v => v,
        }
        .rebased(Base::of(t)),
        // A type inferred from the formula cannot be violated by it, only
        // worn: a `Float` inferred from a float and a percentage holds the
        // plain number. A declared `Float` refuses a percentage as it
        // always has, which is the column saying what it is for.
        S::Float if !col.declared => match v {
            Value::Percent(r) | Value::Fraction(r) => Value::Float(r.to_f64()),
            v => v,
        },
        _ => v,
    }
}

fn scalar_op(op: BinOp, a: &Value, b: &Value) -> Result<Value, String> {
    let empty = matches!(a, Value::Null) || matches!(b, Value::Null);
    Ok(match op {
        _ if op.is_arithmetic() => arith(op, a, b)?,
        _ if op.is_bitwise() => bitwise(op, a, b)?,
        BinOp::Eq => equal(a, b)?.map_or(Value::Null, Value::Boolean),
        BinOp::Ne => equal(a, b)?.map_or(Value::Null, |e| Value::Boolean(!e)),
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => match compare(a, b)? {
            None => Value::Null,
            Some(o) => Value::Boolean(match op {
                BinOp::Lt => o == Ordering::Less,
                BinOp::Gt => o == Ordering::Greater,
                BinOp::Le => o != Ordering::Greater,
                _ => o != Ordering::Less,
            }),
        },
        BinOp::And | BinOp::Or => match (a, b) {
            (Value::Boolean(x), Value::Boolean(y)) => {
                Value::Boolean(if op == BinOp::And { *x && *y } else { *x || *y })
            }
            // Kleene's rule, which is Polars' and SQL's: an empty cell is an
            // unknown, and an unknown that cannot change the answer does not
            // decide it. So `false and` is false and `true or` is true
            // whichever side the empty cell falls on. Before this only the
            // left one settled it, by the short-circuit in `eval`, which
            // made `and` read differently each way round.
            (Value::Boolean(d), Value::Null) | (Value::Null, Value::Boolean(d))
                if *d == (op == BinOp::Or) =>
            {
                Value::Boolean(*d)
            }
            _ if empty => Value::Null,
            _ => {
                return Err(format!(
                    "`{}` needs Boolean on both sides, found {} and {}",
                    op.symbol(),
                    a.kind(),
                    b.kind()
                ));
            }
        },
        _ => return Err(format!("`{}` cannot be applied here", op.symbol())),
    })
}

/// The next text in the way letters and digits count up, as `AZ` to `BA`:
/// the rightmost run of letters or digits is raised by one, carrying to the
/// left, and widening when it runs off the end.
pub fn succ_text(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let Some(end) = chars.iter().rposition(char::is_ascii_alphanumeric) else {
        return format!("{s}1");
    };
    let mut start = end;
    while start > 0 && chars[start - 1].is_ascii_alphanumeric() {
        start -= 1;
    }
    let mut out = chars;
    let mut i = end;
    loop {
        let (next, carry) = match out[i] {
            '9' => ('0', true),
            'z' => ('a', true),
            'Z' => ('A', true),
            c => ((c as u8 + 1) as char, false),
        };
        out[i] = next;
        if !carry {
            break;
        }
        if i == start {
            out.insert(start, if next == '0' { '1' } else { next });
            break;
        }
        i -= 1;
    }
    out.into_iter().collect()
}

/// What `is_unique`, `is_duplicated`, `is_first_distinct` and
/// `is_last_distinct` answer for each of `items`: the items are sorted, so
/// that values alike are side by side, and each run of them is one kind.
/// An empty cell is a kind of its own, as a null is in Polars.
fn marks(func: Func, items: &[Value]) -> Vec<bool> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    let cmp = |a: &usize, b: &usize| {
        sort_order(&items[*a], &items[*b]).unwrap_or(std::cmp::Ordering::Equal)
    };
    order.sort_by(cmp);
    let mut out = vec![false; items.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && cmp(&order[start], &order[end]).is_eq() {
            end += 1;
        }
        // A stable sort keeps each run in the order of the rows.
        let run = &order[start..end];
        for (k, &i) in run.iter().enumerate() {
            out[i] = match func {
                Func::IsUnique => run.len() == 1,
                Func::IsDuplicated => run.len() > 1,
                Func::IsFirstDistinct => k == 0,
                _ => k == run.len() - 1,
            };
        }
        start = end;
    }
    out
}

/// Which rows of `frame` the lowered filter `expr` is true of, as a mask:
/// a plan of [`Engine::filter_plan`], run anywhere. `None` where Polars
/// fails on it, and the engine answers it a row at a time instead.
pub fn run_filter_plan(frame: DataFrame, expr: Expr) -> Option<Vec<bool>> {
    let nrows = frame.height();
    let kept = frame
        .lazy()
        .with_row_index(ROW_INDEX, None)
        .filter(expr)
        .select([col(ROW_INDEX)])
        .collect()
        .ok()?;
    let kept = kept.column(ROW_INDEX).ok()?.u32().ok()?.clone();
    let mut mask = vec![false; nrows];
    for r in kept.into_no_null_iter() {
        mask[r as usize] = true;
    }
    Some(mask)
}

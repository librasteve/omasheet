// Copyright (c) 2026 Stephen Roe

//! Evaluation of a checked program.
//!
//! Cells are calculated on demand and remembered, so each is calculated once
//! and always after whatever it reads. [`Engine::run`] walks the program's
//! calculation order so that long chains (a running balance) are filled in
//! row by row rather than by deep recursion.

use crate::dtype::{cell_of, column_of};
use crate::table::{Held, Tables};
use crate::value::{
    RowRef, Value, View, arith, compare, equal, format_exact, format_styled, math, round_digits,
};
use crate::zone::{Zone, Zoned, utc_now};
use crate::{Lowering, Options, ROW_THRESHOLD, lower};
use omasheet_omx::ast::Lit;
use omasheet_omx::ast::{BinOp, UnOp};
use omasheet_omx::cycle;
use omasheet_omx::date;
use omasheet_omx::decimal::Dec;
use omasheet_omx::funcs::MathFn;
use omasheet_omx::ir::{Bound, ColSel, Func, Ir, Node, Offset, RowSel};
use omasheet_omx::{Cell, ColKind, Column, Diagnostic, Group, Program, S, Span};
use polars::prelude::Column as PlColumn;
use polars::prelude::{AnyValue, DataFrame, DataType, Expr, IntoLazy, col};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

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

pub struct Engine<'p> {
    prog: &'p Program,
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
    /// What became of each column that could have been calculated in one
    /// step, in the order they were tried.
    attempts: RefCell<Vec<(usize, usize, Attempt)>>,
    /// And of each aggregate over a whole column, by the column it was
    /// taken over. One column may be here more than once.
    aggregates: RefCell<Vec<(usize, usize, Attempt)>>,
    /// And of each condition picking rows of a sheet, by that sheet.
    filters: RefCell<Vec<(usize, Attempt)>>,
    /// Which of a `table`'s columns have been read from its file, by sheet
    /// and column, in the order they were read. What is not here was never
    /// read.
    reads: RefCell<Vec<(usize, usize)>>,
    /// A memoisation slot per cell, by sheet, column and row. A column's
    /// slots are made when a cell of it is first asked for: a sheet of a
    /// million rows whose columns all come from a file needs none of them,
    /// and merely opening such a file should not pay for them.
    cells: RefCell<Vec<Vec<Option<Vec<Slot>>>>>,
    consts: RefCell<Vec<Slot>>,
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
pub fn unknown_zone(prog: &Program) -> Option<Diagnostic> {
    let (name, span) = prog.zone.as_ref()?;
    Zone::named(name).is_none().then(|| no_zone(name, *span))
}

fn no_zone(name: &str, span: Span) -> Diagnostic {
    Diagnostic::new(span, format!("there is no time zone `{name}`"))
        .with_help("use a name from the time zone database, such as `Europe/London` or `UTC`")
}

/// The column a lowered selection counts rows in. No omasheet column can
/// be called this, so it cannot be one of the sheet's own.
const ROW_INDEX: &str = "#row";

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
        let local = Rc::new(options.zone.clone().unwrap_or_else(Zone::system));
        let named = prog.zone.as_ref().and_then(|(name, _)| Zone::named(name));
        let home = named.map_or_else(|| local.clone(), Rc::new);
        let now = options
            .now
            .or_else(|| home.from_utc(utc_now()))
            .unwrap_or(0);
        Engine {
            prog,
            tables,
            held: RefCell::new(
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
            attempts: RefCell::new(Vec::new()),
            aggregates: RefCell::new(Vec::new()),
            filters: RefCell::new(Vec::new()),
            reads: RefCell::new(Vec::new()),
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
            diags: RefCell::new(unknown_zone(prog).into_iter().collect()),
            options,
            home,
            local,
            zones: RefCell::new(HashMap::new()),
            now,
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
                    if self.run_lowered(cols) {
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
        // A null Polars cannot account for is not an answer. A decimal
        // product wider than the 38 digits it is held in comes back as
        // nothing at all rather than as an error, and so does a cast that
        // did not take; the engine answers both, one by giving up its guard
        // digits and the other by saying what went wrong in the row it went
        // wrong in. So the plan carries a flag beside the answer: a row
        // whose every column was there and whose answer is nothing. Where
        // one is found the column is given up whole, the same as a plan
        // that will not collect. A constant that is itself nothing trips it
        // too, and the engine gives the same answer for that a little
        // slower.
        let unaccounted = frame
            .get_column_names()
            .into_iter()
            .fold(expr.clone().is_null(), |flag, name| {
                flag.and(polars::prelude::col(name.clone()).is_not_null())
            })
            .alias(UNACCOUNTED);
        // Nothing of the frame is borrowed from here on: the plan is built
        // and the columns owned before the collect, which Polars runs on a
        // pool of its own threads (D67's `RefCell` discipline).
        let collected = frame
            .lazy()
            .select([expr, unaccounted])
            .collect()
            .ok()
            .filter(accounted_for)
            .and_then(|df| df.get_columns().first().cloned());
        let Some(column) = collected.filter(|column| column.len() == nrows) else {
            // Whole or nothing: the result is discarded, no diagnostic is
            // filed for the attempt, and the group is calculated again row
            // by row — which is what produces a per-cell error with the row
            // in it (D67).
            self.attempts.borrow_mut().push((t, c, Attempt::Failed));
            return false;
        };
        // Polars gives arithmetic a dtype of precision `None`; the full 38
        // is what a Parquet write and the schema in the text agree on
        // (D72a).
        let column = match column.dtype() {
            DataType::Decimal(None, Some(scale)) => {
                let wanted = crate::dtype::decimal(*scale as u32);
                match column.cast(&wanted) {
                    Ok(stamped) => stamped,
                    Err(_) => {
                        self.attempts.borrow_mut().push((t, c, Attempt::Failed));
                        return false;
                    }
                }
            }
            _ => column,
        };
        self.held.borrow_mut()[t][c] = Some(column);
        self.attempts.borrow_mut().push((t, c, Attempt::Lowered));
        true
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
        let column = |c| self.pl_column(t, c);
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
        let columns = cx.taken();
        // A condition reading no column is the same answer for every row,
        // and the engine has it without a plan.
        if columns.is_empty() {
            return refused(self);
        }
        let Ok(frame) = DataFrame::new(columns) else {
            return refused(self);
        };
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

    /// An aggregate over one whole column of a sheet, taken by Polars.
    /// `None` leaves it to the engine, which reads the column a value at a
    /// time; nothing is reported either way, as nothing can fail.
    ///
    /// The argument has to be a whole column and nothing else: a selection
    /// of rows, a vector, several arguments together, or a block all go to
    /// the engine. Those are section 6's remaining tasks rather than this
    /// one's.
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
        let Some(column) = self.pl_column(t, c) else {
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
        let frame = DataFrame::new(vec![column]).ok()?;
        let Ok(out) = frame.lazy().select(select).collect() else {
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
        if let Some(scale) = total {
            // An empty column has no extremes and sums to nothing, which
            // has room anywhere.
            let unscaled = |key: &str| match out.column(key).ok()?.get(0).ok()? {
                AnyValue::Decimal(n, _) => Some(n),
                AnyValue::Null => Some(0),
                _ => None,
            };
            let widest = unscaled("lo")?
                .unsigned_abs()
                .max(unscaled("hi")?.unsigned_abs());
            let room = i128::try_from(widest)
                .ok()
                .and_then(|w| w.checked_mul(nrows as i128))
                .and_then(|bound| Dec::new(bound, scale as i32));
            if room.is_none() {
                return refused(self);
            }
        }
        let any = out.column("agg").ok()?.get(0).ok()?;
        // The dress a column wears is put back on, a Polars aggregate
        // answering a bare number (D69): the sum of percentages is a
        // percentage, as it is on the row path.
        let ty = self.prog.sheets[t].cols[c].ty;
        let value = match func {
            // A count is a count of things, whatever the column holds.
            Func::Len | Func::Count => Value::from_lit(&Lit::Int(any.try_extract::<i64>().ok()?)),
            _ => Value::from_lit(match &cell_of(any, ty) {
                Cell::Lit(l) => l,
                _ => return refused(self),
            }),
        };
        let value = match (ty, &value) {
            (S::Fraction, Value::Int(_) | Value::Decimal(_)) => value.fraction(),
            _ => value,
        };
        // The division `mean` is, taken here rather than by Polars so that
        // it is a `Dec`'s. The row path divides the sum it has already
        // dressed and puts the percentage back on after, a division being
        // one of the things that takes it off, so this does the same.
        // Nothing to divide by is a column of nothing, whose mean is empty.
        let value = match mean {
            false => value,
            true => match out
                .column("count")
                .ok()?
                .get(0)
                .ok()?
                .try_extract::<i64>()
                .ok()?
            {
                0 => Value::Null,
                n => {
                    let mean = arith(BinOp::Div, &value, &Value::Int(n)).ok()?;
                    match ty {
                        S::Percent => mean.percent(),
                        _ => mean,
                    }
                }
            },
        };
        self.aggregates.borrow_mut().push((t, c, Attempt::Lowered));
        Some(value)
    }

    /// The frame a column's formula is evaluated over and the one
    /// expression that calculates it, or `None` where Polars has no answer
    /// of its own for some part of it.
    fn plan(&self, t: usize, c: usize, node: &Node) -> Option<(DataFrame, Expr)> {
        let col = &self.prog.sheets[t].cols[c];
        let column = |cc| self.pl_column(t, cc);
        let constant = |i| self.const_value(i).ok();
        let cx = lower::Cx::new(&column, &constant);
        let out = lower::expr(node, &cx)?;
        // The same answer the row path's `fit` gives: a column that says
        // what it holds holds it, and a lowered result of another type is
        // one the two paths would disagree about.
        if col.declared && !out.ty()?.assignable_to(col.ty) {
            return None;
        }
        let columns = cx.taken();
        // An expression reading no column at all is one value repeated, and
        // the row loop has already memoised it by the second row.
        if columns.is_empty() {
            return None;
        }
        let frame = DataFrame::new(columns).ok()?;
        Some((frame, out.expr.alias(col.name.as_str())))
    }

    /// One column of a sheet as the dataframe holds it: a `table`'s own
    /// column, a column already calculated in one step, or one built here
    /// out of calculated values. `None` for a column that has no one dtype
    /// to be held as, or that holds a value which does not fit the type the
    /// column says it has.
    fn pl_column(&self, t: usize, c: usize) -> Option<PlColumn> {
        // Cloning a Polars column clones the handles, not the buffers.
        if let Some(held) = self.held.borrow()[t][c].clone() {
            return Some(held);
        }
        match self.tables.get(t, c) {
            // The file's own column, read now and kept: the dataframe is
            // the authority for it, as it is for a lowered one.
            Some(held @ Held::Column(_)) => {
                let column = held.read()?;
                self.reads.borrow_mut().push((t, c));
                self.held.borrow_mut()[t][c] = Some(column.clone());
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
            Some(Cell::Lit(l)) => Ok(lit_value(l, col.ty)),
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
            (None, ColKind::Frame) => {
                let held = match self.tables.get(t, c) {
                    Some(Held::Column(_)) => Some(
                        self.pl_column(t, c)
                            .map_or(Cell::Invalid, |column| held_cell(&column, r, col.ty)),
                    ),
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

    /// A cell for display: failures show as an error value.
    pub fn cell_shown(&self, t: usize, c: usize, r: usize) -> Value {
        match self.cell(t, c, r) {
            Ok(v) => v,
            Err(_) => Value::Error,
        }
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
            None if matches!(self.tables.get(t, c), Some(Held::Column(_))) => self.pl_column(t, c),
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
                    Cell::Lit(l) => lit_value(&l, ty),
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
        let ok = !col.declared
            || matches!(
                (&v, col.ty),
                (Value::Null, _)
                    | (_, S::Any)
                    | (Value::Int(_), S::Int | S::Decimal)
                    | (Value::Decimal(_), S::Decimal)
                    | (Value::Fraction(_), S::Fraction)
                    | (Value::Percent(_), S::Percent)
                    | (Value::Float(_), S::Float)
                    | (Value::String(..), S::String | S::Url | S::Email | S::Uuid)
                    | (Value::Boolean(_), S::Boolean)
                    | (Value::Date(_), S::Date)
                    | (Value::Time(_), S::Time)
                    | (Value::DateTime(_) | Value::Zoned(_), S::DateTime)
            );
        if ok {
            Ok(v)
        } else {
            fail(
                span,
                format!(
                    "column `{}` is declared `{}` but this is {}",
                    col.name,
                    col.ty,
                    v.kind()
                ),
            )
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

    fn column(&self, t: usize, c: usize, rows: &[usize]) -> R {
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
            Ir::RowIndex { depth } => {
                let (_, r) = fr[fr.len() - 1 - depth];
                Ok(Value::Int(r as i64))
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
                        (UnOp::Neg, v) if v.is_numeric() => {
                            arith(BinOp::Sub, &Value::Int(0), v).or_else(|m| fail(span, m))
                        }
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

    fn call(&self, func: Func, args: &[Node], span: Span, fr: &mut Frames) -> R {
        // An aggregate over a whole column the dataframe holds is Polars'
        // to take, which is a pass over a column rather than a value read
        // per row. Anything else falls through to the engine below.
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
                        Value::DateTime(t) => Ok(Value::DateTime(t + date::MICROS)),
                        v if v.is_numeric() => {
                            arith(BinOp::Add, v, &Value::Int(1)).or_else(|m| fail(span, m))
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
                    Value::Int(n) => n,
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
                        Value::Int(n) => n,
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
                        Value::Int(n) => Some(n),
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
                        Value::DateTime(t) => {
                            let (days, time) = date::split_datetime(*t);
                            match shift(days) {
                                Some(d) => Ok(Value::DateTime(
                                    d as i64 * date::DAY_MICROS + time / date::NANOS_PER_MICRO,
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
            Func::Now => Ok(Value::DateTime(self.now)),
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
                    Value::Int(n) => n,
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
                        Value::DateTime(t) => match self.home.to_utc(*t) {
                            Some(utc) => (utc, *t, &self.home),
                            None => return out_of_range(),
                        },
                        Value::Zoned(z) => (z.utc, z.wall, &z.zone),
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
                            .map(Value::DateTime)
                            .map_or_else(out_of_range, Ok),
                        (Some(target), _) => Zoned::at(utc, target, &self.home)
                            .map(|z| Value::Zoned(Rc::new(z)))
                            .map_or_else(out_of_range, Ok),
                        (None, Func::Offset) => {
                            Ok(Value::Int((wall - utc).div_euclid(date::MICROS)))
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
            | Func::DateOf
            | Func::TimeOf => {
                let part = |v: &Value| -> R {
                    let (days, time) = match v {
                        Value::Null => return Ok(Value::Null),
                        Value::Date(d) => (Some(*d), None),
                        Value::Time(t) => (None, Some(*t)),
                        Value::DateTime(t) => {
                            let (days, time) = date::split_datetime(*t);
                            (Some(days), Some(time))
                        }
                        Value::Zoned(z) => {
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
                        Some(n) => Ok(Value::Int(n)),
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
                        Value::Int(i) => Some(*i as f64),
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
                let total = fold(BinOp::Add, Value::Int(0), &|v| Ok(v.clone()))?;
                let mean = arith(BinOp::Div, &total, &Value::Int(n)).or_else(|m| fail(span, m))?;
                let squares = fold(BinOp::Add, Value::Int(0), &|v| {
                    let d = arith(BinOp::Sub, v, &mean).or_else(|m| fail(span, m))?;
                    arith(BinOp::Mul, &d, &d).or_else(|m| fail(span, m))
                })?;
                let var = arith(BinOp::Div, &squares, &Value::Int(n - ddof))
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
                // Every argument's values, taken together.
                let mut items: Vec<Value> = Vec::new();
                for arg in args {
                    match self.eval(arg, fr)? {
                        Value::Vector(v) => items.extend(v.iter().cloned()),
                        Value::Sheet(v) if func == Func::Len && args.len() == 1 => {
                            return Ok(Value::Int(v.rows.len() as i64));
                        }
                        // A block: its cells, row by row.
                        Value::Sheet(v) => {
                            for &r in v.rows.iter() {
                                for &c in v.cols.iter() {
                                    items.push(self.cell(v.sheet, c, r)?);
                                }
                            }
                        }
                        Value::Range {
                            lo,
                            hi,
                            after,
                            exclusive,
                        } => {
                            let lo = if after { lo + 1 } else { lo };
                            let hi = if exclusive { hi - 1 } else { hi };
                            items.extend((lo..=hi).map(Value::Int));
                        }
                        Value::Row(row) => {
                            for &c in row.cols.iter() {
                                items.push(self.cell(row.sheet, c, row.row)?);
                            }
                        }
                        Value::Null => {}
                        other if args.len() > 1 && !matches!(other, Value::Sheet(_)) => {
                            items.push(other)
                        }
                        other => {
                            return fail(
                                arg.span,
                                format!("expected a vector to aggregate, found {}", other.kind()),
                            );
                        }
                    }
                }
                let present: Vec<&Value> =
                    items.iter().filter(|v| !matches!(v, Value::Null)).collect();
                let sum = || -> R {
                    let mut total = Value::Int(0);
                    for v in &present {
                        total = arith(BinOp::Add, &total, v).or_else(|m| fail(span, m))?;
                    }
                    Ok(total)
                };
                // The sum and the average of percentages are percentages.
                let percent =
                    !present.is_empty() && present.iter().all(|v| matches!(v, Value::Percent(_)));
                let kept = |v: Value| if percent { v.percent() } else { v };
                match func {
                    Func::Count => Ok(Value::Int(present.len() as i64)),
                    // What is empty counts here, which is the whole
                    // difference between `len` and `count`.
                    Func::Len => Ok(Value::Int(items.len() as i64)),
                    Func::Sum => sum().map(kept),
                    Func::Mean if present.is_empty() => Ok(Value::Null),
                    Func::Mean => arith(BinOp::Div, &sum()?, &Value::Int(present.len() as i64))
                        .map(kept)
                        .or_else(|m| fail(span, m)),
                    Func::Median if present.is_empty() => Ok(Value::Null),
                    Func::Median => {
                        let mut sorted = present.clone();
                        sorted.sort_by(|a, b| {
                            compare(a, b).ok().flatten().unwrap_or(Ordering::Equal)
                        });
                        let n = sorted.len();
                        if n % 2 == 1 {
                            Ok((*sorted[n / 2]).clone())
                        } else {
                            let pair = arith(BinOp::Add, sorted[n / 2 - 1], sorted[n / 2])
                                .or_else(|m| fail(span, m))?;
                            arith(BinOp::Div, &pair, &Value::Int(2))
                                .map(kept)
                                .or_else(|m| fail(span, m))
                        }
                    }
                    Func::Product => {
                        let mut total = Value::Int(1);
                        for v in &present {
                            total = arith(BinOp::Mul, &total, v).or_else(|m| fail(span, m))?;
                        }
                        Ok(total)
                    }
                    // Empty or not: the first value is the first value.
                    Func::First => Ok(items.first().cloned().unwrap_or(Value::Null)),
                    Func::Last => Ok(items.last().cloned().unwrap_or(Value::Null)),
                    _ => {
                        let want = if func == Func::Min {
                            Ordering::Less
                        } else {
                            Ordering::Greater
                        };
                        let mut best: Option<&Value> = None;
                        for &v in &present {
                            best = match best {
                                None => Some(v),
                                Some(b) => {
                                    let o = compare(v, b).or_else(|m| fail(span, m))?;
                                    if o == Some(want) { Some(v) } else { Some(b) }
                                }
                            };
                        }
                        Ok(best.cloned().unwrap_or(Value::Null))
                    }
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
            Value::DateTime(t) => {
                let wall = self.local.from_utc(self.home.to_utc(*t)?)?;
                Some(Value::DateTime(wall))
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
                line.push(self.show(&value, false));
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

/// The day of the year a date falls on, 1 to 366.
fn ordinal_day(days: i32) -> i64 {
    let (y, _, _) = date::civil_from_days(days as i64);
    days as i64 - date::days_from_civil(y, 1, 1) + 1
}

/// The flag a lowered column carries beside its answer: a row whose every
/// column was there and whose answer is nothing. The leading space is what
/// keeps it from any name a column of a sheet could have.
const UNACCOUNTED: &str = " unaccounted for";

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
        S::Percent if matches!(v, Value::Int(_) | Value::Decimal(_)) => v.percent(),
        S::Fraction if matches!(v, Value::Int(_) | Value::Decimal(_)) => v.fraction(),
        S::Int | S::Decimal => match v {
            Value::Percent(r) | Value::Fraction(r) => Value::exact(r),
            v => v,
        },
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

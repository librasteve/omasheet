//! Evaluation of a checked program.
//!
//! Cells are calculated on demand and remembered, so each is calculated once
//! and always after whatever it reads. [`Engine::run`] walks the program's
//! calculation order so that long chains (a running balance) are filled in
//! row by row rather than by deep recursion.

use crate::Options;
use crate::value::{RowRef, Value, View, arith, compare, equal, format_styled, to_f64};
use crate::zone::{Zone, Zoned, utc_now};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use omasheet_omx::ast::{BinOp, UnOp};
use omasheet_omx::date;
use omasheet_omx::ir::{Bound, ColSel, Func, Ir, Node, Offset, RowSel};
use omasheet_omx::{Cell, ColKind, Column, Diagnostic, Group, Program, S, Span};
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

/// The rows in scope: (table, row). The innermost row is last.
pub type Frames = Vec<(usize, usize)>;

#[derive(Clone)]
enum Slot {
    Pending,
    Busy,
    Done(Value),
}

enum Rows {
    /// One position, or `None` for a cursor offset outside the table.
    One(Option<usize>),
    Many(Vec<usize>),
}

pub struct Engine<'p> {
    prog: &'p Program,
    cells: RefCell<Vec<Vec<Vec<Slot>>>>,
    consts: RefCell<Vec<Slot>>,
    all_rows: Vec<Rc<Vec<usize>>>,
    all_cols: Vec<Rc<Vec<usize>>>,
    diags: RefCell<Vec<Diagnostic>>,
    options: Options,
    /// The zone the sheet's date-times are in.
    home: Rc<Zone>,
    /// The zone of this machine.
    local: Rc<Zone>,
    zones: RefCell<HashMap<String, Option<Rc<Zone>>>>,
    /// What `now()` gives.
    now: i64,
}

/// The complaint about a sheet that names a time zone there is none of.
pub fn unknown_zone(prog: &Program) -> Option<Diagnostic> {
    let (name, span) = prog.zone.as_ref()?;
    Zone::named(name).is_none().then(|| no_zone(name, *span))
}

fn no_zone(name: &str, span: Span) -> Diagnostic {
    Diagnostic::new(span, format!("there is no time zone `{name}`"))
        .with_help("use a name from the time zone database, such as `Europe/London` or `UTC`")
}

fn fail<T>(span: Span, message: impl Into<String>) -> R<T> {
    Err(Fail::Diag(Diagnostic::new(span, message)))
}

impl<'p> Engine<'p> {
    pub fn new(prog: &'p Program) -> Engine<'p> {
        Engine::with_options(prog, Options::default())
    }

    /// A sheet that names a time zone there is none of is evaluated in the
    /// zone of this machine, with a diagnostic to take before running.
    pub fn with_options(prog: &'p Program, options: Options) -> Engine<'p> {
        let local = Rc::new(options.zone.clone().unwrap_or_else(Zone::system));
        let named = prog.zone.as_ref().and_then(|(name, _)| Zone::named(name));
        let home = named.map_or_else(|| local.clone(), Rc::new);
        let now = options
            .now
            .or_else(|| home.from_utc(utc_now()))
            .unwrap_or(0);
        Engine {
            prog,
            cells: RefCell::new(
                prog.tables
                    .iter()
                    .map(|t| vec![vec![Slot::Pending; t.nrows]; t.cols.len()])
                    .collect(),
            ),
            consts: RefCell::new(vec![Slot::Pending; prog.consts.len()]),
            all_rows: prog
                .tables
                .iter()
                .map(|t| Rc::new((0..t.nrows).collect()))
                .collect(),
            all_cols: prog
                .tables
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
                    let n = self.prog.tables[t].nrows;
                    for k in 0..n {
                        let r = if *reverse { n - 1 - k } else { k };
                        for &(t, c) in cols {
                            if r < self.prog.tables[t].nrows {
                                let _ = self.cell(t, c, r);
                            }
                        }
                    }
                }
            }
        }
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
            table: t,
            rows: self.all_rows[t].clone(),
            cols: self.all_cols[t].clone(),
        }
    }

    // ---- cells and constants ----------------------------------------------

    /// The value of one cell, calculating it if needed.
    pub fn cell(&self, t: usize, c: usize, r: usize) -> R {
        let col = &self.prog.tables[t].cols[c];
        match &self.cells.borrow()[t][c][r] {
            Slot::Done(Value::Error) => return Err(Fail::Poison),
            Slot::Done(v) => return Ok(v.clone()),
            Slot::Busy => {
                return fail(
                    col.span,
                    format!("circular reference in column `{}` at row {r}", col.name),
                );
            }
            Slot::Pending => {}
        }
        self.cells.borrow_mut()[t][c][r] = Slot::Busy;
        let formula = |node: &Node| {
            let v = self.eval(node, &mut vec![(t, r)])?;
            self.fit(v, col, node.span)
        };
        let result = match &col.kind {
            ColKind::Computed(node) => formula(node),
            ColKind::Data(cells) => match &cells[r] {
                Cell::Empty => Ok(Value::Empty),
                Cell::Lit(l) => Ok(Value::from_lit(l)),
                Cell::Formula(node) => formula(node),
                Cell::Invalid => Err(Fail::Poison),
            },
        };
        let value = match result {
            Ok(v) => v,
            Err(Fail::Diag(d)) => {
                self.diags.borrow_mut().push(d);
                Value::Error
            }
            Err(Fail::Poison) => Value::Error,
        };
        self.cells.borrow_mut()[t][c][r] = Slot::Done(value.clone());
        match value {
            Value::Error => Err(Fail::Poison),
            v => Ok(v),
        }
    }

    /// A cell for display: failures show as an error value.
    pub fn cell_shown(&self, t: usize, c: usize, r: usize) -> Value {
        self.cell(t, c, r).unwrap_or(Value::Error)
    }

    /// A formula's result must be one value of the column's declared type.
    fn fit(&self, v: Value, col: &Column, span: Span) -> R {
        if matches!(
            v,
            Value::Vector(_) | Value::Table(_) | Value::Row(_) | Value::Range { .. }
        ) {
            return fail(
                span,
                format!("a cell holds one value, but this is {}", v.kind()),
            );
        }
        let ok = !col.declared
            || matches!(
                (&v, col.ty),
                (Value::Empty, _)
                    | (_, S::Any)
                    | (Value::Int(_), S::Int | S::Rat)
                    | (Value::Rat(_), S::Rat)
                    | (Value::Num(_), S::Num)
                    | (Value::Complex(..), S::Complex)
                    | (Value::Text(_), S::Text)
                    | (Value::Bool(_), S::Bool)
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
            Ir::Table(t) => Ok(Value::Table(self.full_view(*t))),
            Ir::Column { table, col } => self.column(*table, *col, &self.all_rows[*table]),
            Ir::RowCol { depth, col } => {
                let (t, r) = fr[fr.len() - 1 - depth];
                self.cell(t, *col, r)
            }
            Ir::Unary(op, operand) => {
                let v = self.eval(operand, fr)?;
                let apply = |v: &Value| -> R {
                    match (op, v) {
                        (_, Value::Empty) => Ok(Value::Empty),
                        (UnOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                        (UnOp::Neg, v) if v.is_numeric() => {
                            arith(BinOp::Sub, &Value::Int(BigInt::from(0)), v)
                                .or_else(|m| fail(span, m))
                        }
                        (UnOp::Not, v) => fail(span, format!("cannot apply `not` to {}", v.kind())),
                        (UnOp::Neg, v) => fail(span, format!("cannot negate {}", v.kind())),
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
                Value::Bool(true) => self.eval(a, fr),
                Value::Bool(false) => self.eval(b, fr),
                Value::Empty => Ok(Value::Empty),
                other => fail(
                    c.span,
                    format!("the condition of `if` must be Bool, found {}", other.kind()),
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
                Value::Table(v) => self.column(v.table, *col, &v.rows),
                Value::Row(r) => self.cell(r.table, *col, r.row),
                Value::Empty => Ok(Value::Empty),
                other => fail(
                    span,
                    format!(
                        "a column can be read from a table or row, not {}",
                        other.kind()
                    ),
                ),
            },
            Ir::Call(func, args) => self.call(*func, args, span, fr),
            Ir::Single(inner) => match self.eval(inner, fr)? {
                Value::Vector(items) => match items.len() {
                    0 => Ok(Value::Empty),
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
                        0 => Value::Empty,
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
                    Value::Empty => self.eval(r, fr),
                    v => Ok(v),
                }
            }
            BinOp::Range | BinOp::RangeEx => {
                let lo = self.int(l, fr)?;
                let hi = self.int(r, fr)?;
                Ok(Value::Range {
                    lo,
                    hi,
                    exclusive: op == BinOp::RangeEx,
                })
            }
            BinOp::In => {
                let needle = self.eval(l, fr)?;
                if matches!(needle, Value::Empty) {
                    return Ok(Value::Empty);
                }
                match self.eval(r, fr)? {
                    Value::Vector(items) => {
                        for item in items.iter() {
                            if equal(&needle, item).or_else(|m| fail(span, m))? == Some(true) {
                                return Ok(Value::Bool(true));
                            }
                        }
                        Ok(Value::Bool(false))
                    }
                    Value::Range { lo, hi, exclusive } => Ok(Value::Bool(match needle.as_i64() {
                        Some(n) => n >= lo && (n < hi || (n == hi && !exclusive)),
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
                if let Value::Bool(b) = left
                    && b == (op == BinOp::Or)
                {
                    return Ok(Value::Bool(b));
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
        match func {
            Func::Approx => {
                let approx = |v: &Value| -> R {
                    Ok(match v {
                        Value::Empty => Value::Empty,
                        Value::Int(n) => Value::Num(n.to_f64().unwrap_or(f64::NAN)),
                        Value::Rat(r) => Value::Num(to_f64(r)),
                        Value::Num(f) => Value::Num(*f),
                        other => {
                            return fail(span, format!("cannot approximate {}", other.kind()));
                        }
                    })
                };
                match self.eval(&args[0], fr)? {
                    Value::Vector(items) => Ok(Value::Vector(Rc::new(
                        items.iter().map(approx).collect::<R<Vec<_>>>()?,
                    ))),
                    single => approx(&single),
                }
            }
            Func::Complex => {
                let mut parts = [0.0; 2];
                for (part, arg) in parts.iter_mut().zip(args) {
                    *part = match self.eval(arg, fr)? {
                        Value::Empty => return Ok(Value::Empty),
                        Value::Int(n) => n.to_f64().unwrap_or(f64::NAN),
                        Value::Rat(r) => to_f64(&r),
                        Value::Num(f) => f,
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
            Func::Today => Ok(Value::Date(date::split_datetime(self.now).0)),
            Func::Now => Ok(Value::DateTime(self.now)),
            Func::ToZone | Func::Utc | Func::Local | Func::Offset | Func::ZoneName => {
                let target = match func {
                    Func::ToZone => match self.eval(&args[1], fr)? {
                        Value::Empty => return Ok(Value::Empty),
                        Value::Text(name) => Some(self.zone(&name, args[1].span)?),
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
                        Value::Empty => return Ok(Value::Empty),
                        Value::DateTime(t) => match self.home.to_utc(*t) {
                            Some(utc) => (utc, *t, &self.home),
                            None => return out_of_range(),
                        },
                        Value::Zoned(z) => (z.utc, z.wall, &z.zone),
                        other => {
                            return fail(
                                args[0].span,
                                format!("expected a DateTime, found {}", other.kind()),
                            );
                        }
                    };
                    match (&target, func) {
                        // The sheet's own zone needs no label.
                        (Some(target), _) if **target == *self.home => self
                            .home
                            .from_utc(utc)
                            .map(Value::DateTime)
                            .map_or_else(out_of_range, Ok),
                        (Some(target), _) => Zoned::at(utc, target, &self.home)
                            .map(|z| Value::Zoned(Rc::new(z)))
                            .map_or_else(out_of_range, Ok),
                        (None, Func::Offset) => Ok(Value::Int(BigInt::from(wall - utc))),
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
            | Func::Hour
            | Func::Minute
            | Func::Second
            | Func::DateOf
            | Func::TimeOf => {
                let part = |v: &Value| -> R {
                    let (days, time) = match v {
                        Value::Empty => return Ok(Value::Empty),
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
                        Func::Hour => time.map(|t| t as i64 / 3600),
                        Func::Minute => time.map(|t| t as i64 / 60 % 60),
                        Func::Second => time.map(|t| t as i64 % 60),
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
                        Some(n) => Ok(Value::Int(BigInt::from(n))),
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
            Func::Sum | Func::Avg | Func::Min | Func::Max | Func::Count => {
                let items: Rc<Vec<Value>> = match self.eval(&args[0], fr)? {
                    Value::Vector(items) => items,
                    Value::Table(v) if func == Func::Count => {
                        return Ok(Value::Int(BigInt::from(v.rows.len())));
                    }
                    Value::Range { lo, hi, exclusive } => {
                        let hi = if exclusive { hi - 1 } else { hi };
                        Rc::new((lo..=hi).map(|n| Value::Int(BigInt::from(n))).collect())
                    }
                    Value::Empty => Rc::new(Vec::new()),
                    other => {
                        return fail(
                            args[0].span,
                            format!("expected a vector to aggregate, found {}", other.kind()),
                        );
                    }
                };
                let present: Vec<&Value> = items
                    .iter()
                    .filter(|v| !matches!(v, Value::Empty))
                    .collect();
                let sum = || -> R {
                    let mut total = Value::Int(BigInt::from(0));
                    for v in &present {
                        total = arith(BinOp::Add, &total, v).or_else(|m| fail(span, m))?;
                    }
                    Ok(total)
                };
                match func {
                    Func::Count => Ok(Value::Int(BigInt::from(present.len()))),
                    Func::Sum => sum(),
                    Func::Avg if present.is_empty() => Ok(Value::Empty),
                    Func::Avg => arith(
                        BinOp::Div,
                        &sum()?,
                        &Value::Int(BigInt::from(present.len())),
                    )
                    .or_else(|m| fail(span, m)),
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
                        Ok(best.cloned().unwrap_or(Value::Empty))
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

    /// Resolve a row selector to positions in a list of `len` rows. `table`
    /// gives the sheet rows behind each position, for conditions.
    fn rows(
        &self,
        sel: &RowSel,
        len: usize,
        table: Option<(usize, &[usize])>,
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
            RowSel::Range { lo, hi, exclusive } => {
                let bound = |b: &Bound, fr: &mut Frames| -> R<i64> {
                    Ok(match b {
                        Bound::Abs(node) => from_end(self.int(node, fr)?),
                        Bound::Cursor(off) => cursor(fr)? + self.offset(off, fr)?,
                    })
                };
                let lo = bound(lo, fr)?;
                let hi = bound(hi, fr)?;
                range(lo, if *exclusive { hi - 1 } else { hi })
            }
            RowSel::RangeVal(node) => match self.eval(node, fr)? {
                Value::Range { lo, hi, exclusive } => {
                    let hi = from_end(hi);
                    range(from_end(lo), if exclusive { hi - 1 } else { hi })
                }
                other => {
                    return fail(
                        node.span,
                        format!("expected a range, found {}", other.kind()),
                    );
                }
            },
            RowSel::Pred(node) => {
                let Some((t, sheet_rows)) = table else {
                    return fail(node.span, "a condition needs a table to select from");
                };
                let mut keep = Vec::new();
                for (p, &r) in sheet_rows.iter().enumerate() {
                    fr.push((t, r));
                    let v = self.eval(node, fr);
                    fr.pop();
                    match v? {
                        Value::Bool(true) => keep.push(p),
                        Value::Bool(false) | Value::Empty => {}
                        other => {
                            return fail(
                                node.span,
                                format!("a row condition must be Bool, found {}", other.kind()),
                            );
                        }
                    }
                }
                Rows::Many(keep)
            }
            RowSel::Mask(node) => {
                let Value::Vector(mask) = self.eval(node, fr)? else {
                    return fail(node.span, "expected a vector of Bool");
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
                        Value::Bool(true) => keep.push(p),
                        Value::Bool(false) | Value::Empty => {}
                        other => {
                            return fail(
                                node.span,
                                format!("a mask must hold Bool values, found {}", other.kind()),
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
            Value::Empty => Ok(Value::Empty),
            Value::Table(v) => {
                let picked = self.rows(rows, v.rows.len(), Some((v.table, &v.rows)), span, fr)?;
                let sub_cols = |cols: &ColSel| match cols {
                    ColSel::Many(list) => Rc::new(list.clone()),
                    _ => v.cols.clone(),
                };
                Ok(match (picked, cols) {
                    (Rows::One(None), _) => Value::Empty,
                    (Rows::One(Some(p)), ColSel::One(c)) => self.cell(v.table, *c, v.rows[p])?,
                    (Rows::One(Some(p)), cols) => Value::Row(RowRef {
                        table: v.table,
                        row: v.rows[p],
                        cols: sub_cols(cols),
                    }),
                    (Rows::Many(ps), ColSel::One(c)) => {
                        let sheet_rows: Vec<usize> = ps.iter().map(|&p| v.rows[p]).collect();
                        self.column(v.table, *c, &sheet_rows)?
                    }
                    (Rows::Many(ps), cols) => Value::Table(View {
                        table: v.table,
                        rows: Rc::new(ps.iter().map(|&p| v.rows[p]).collect()),
                        cols: sub_cols(cols),
                    }),
                })
            }
            Value::Vector(items) => Ok(match self.rows(rows, items.len(), None, span, fr)? {
                Rows::One(None) => Value::Empty,
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

    /// The sheet's own date-times in `v` on the clocks of this machine, when
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
        match self.localised(v) {
            Some(local) => format_styled(&local, quoted, style),
            None => format_styled(v, quoted, style),
        }
    }

    /// The name of the time zone the sheet's date-times are in.
    pub fn zone_name(&self) -> &str {
        self.home.name()
    }

    /// A table as aligned text, in the same shape as `.omx` source.
    pub fn render_view(&self, v: &View) -> String {
        let table = &self.prog.tables[v.table];
        let mut grid: Vec<Vec<String>> =
            vec![v.cols.iter().map(|&c| table.cols[c].name.clone()).collect()];
        let mut numeric = vec![true; v.cols.len()];
        for &r in v.rows.iter() {
            let mut line = Vec::with_capacity(v.cols.len());
            for (k, &c) in v.cols.iter().enumerate() {
                let value = self.cell_shown(v.table, c, r);
                if !matches!(value, Value::Empty | Value::Error) && !value.is_numeric() {
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

    pub fn render_table(&self, t: usize) -> String {
        self.render_view(&self.full_view(t))
    }

    /// Any value as text, ending in a newline.
    pub fn render_value(&self, v: &Value) -> String {
        match v {
            Value::Table(view) => self.render_view(view),
            Value::Row(row) => self.render_view(&View {
                table: row.table,
                rows: Rc::new(vec![row.row]),
                cols: row.cols.clone(),
            }),
            Value::Empty => "empty\n".to_string(),
            other => format!("{}\n", self.show(other, false)),
        }
    }
}

fn scalar_op(op: BinOp, a: &Value, b: &Value) -> Result<Value, String> {
    let empty = matches!(a, Value::Empty) || matches!(b, Value::Empty);
    Ok(match op {
        _ if op.is_arithmetic() => arith(op, a, b)?,
        BinOp::Eq => equal(a, b)?.map_or(Value::Empty, Value::Bool),
        BinOp::Ne => equal(a, b)?.map_or(Value::Empty, |e| Value::Bool(!e)),
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => match compare(a, b)? {
            None => Value::Empty,
            Some(o) => Value::Bool(match op {
                BinOp::Lt => o == Ordering::Less,
                BinOp::Gt => o == Ordering::Greater,
                BinOp::Le => o != Ordering::Greater,
                _ => o != Ordering::Less,
            }),
        },
        BinOp::And | BinOp::Or => match (a, b) {
            (Value::Bool(x), Value::Bool(y)) => {
                Value::Bool(if op == BinOp::And { *x && *y } else { *x || *y })
            }
            _ if empty => Value::Empty,
            _ => {
                return Err(format!(
                    "`{}` needs Bool on both sides, found {} and {}",
                    op.symbol(),
                    a.kind(),
                    b.kind()
                ));
            }
        },
        _ => return Err(format!("`{}` cannot be applied here", op.symbol())),
    })
}

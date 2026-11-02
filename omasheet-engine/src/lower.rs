// Copyright (c) 2026 Stephen Roe

//! An omasheet expression as a Polars one, or nothing at all.
//!
//! This is the second of the two gates a column passes to be calculated in
//! one step rather than row by row (D66). The first is
//! [`omasheet_omx::check::Column::elementwise`], which says every reference
//! the formula makes reaches the row it is evaluated for. This says Polars
//! has the same answer for every part of it.
//!
//! [`expr`] is total: an expression it does not handle is declined, never
//! approximated. Everything it declines the engine calculates as it always
//! did, so a gap here costs speed and never an answer — which is why the
//! set it handles is small and grows only against a test that the two paths
//! agree.
//!
//! What it does not handle, on purpose rather than for want of writing:
//!
//! - **Division and the mean of exact numbers** (D72). A Polars quotient has
//!   `max(x, y) + 4` digits after the point and cannot be given more than
//!   21, where `Dec` is only equal to itself down to 30; and a Polars mean
//!   over a decimal column comes back `f64`. Over floating-point numbers
//!   both are lowered — a quotient by `arith` and a mean by its `sum` and
//!   its `count` (D72b).
//! - **A percentage or a fraction**, which are an exact number in a dress
//!   rather than a dtype (D69). The dress is a column's, re-applied when a
//!   value is read back out of one, and what omasheet makes of a percentage
//!   met in the middle of a sum is its own rule rather than arithmetic.
//! - **Dates and times in arithmetic.** A whole number beside one counts
//!   days, or seconds, which is a scaling per type rather than an addition.
//!   The two are compared, though, both sides being whole numbers of the
//!   same unit.
//! - **A value with no dtype to be held as**: anything the checker typed
//!   `Any`, and a vector, a sheet or a row.
//! - **Most calls.** [`call`] says which are here and why the rest are not;
//!   the short of it is that a conversion, a rounding and a zone are
//!   omasheet's own rules rather than Polars', and that `abs`, `sign`,
//!   `round`, `floor`, `ceil`, `exp`, `log` and the trigonometric functions
//!   are Polars' but behind a feature this build does not enable.

use crate::Value;
use crate::dtype::{decimal, type_of};
use omasheet_omx::ast::{BinOp, Lit, UnOp};
use omasheet_omx::decimal::MAX_PRECISION;
use omasheet_omx::ir::{Func, Ir, Node};
use omasheet_omx::types::S;
use polars::prelude::Column as PlColumn;
use polars::prelude::*;
use std::cell::RefCell;

/// A lowered expression and the dtype it gives, which is what decides
/// whether the operation above it may be lowered in turn.
pub struct Out {
    pub expr: Expr,
    pub dtype: DataType,
}

impl Out {
    fn new(expr: Expr, dtype: DataType) -> Option<Out> {
        Some(Out { expr, dtype })
    }

    /// The omasheet type this gives, or `None` for a dtype no omasheet type
    /// names.
    pub fn ty(&self) -> Option<S> {
        type_of(&self.dtype)
    }
}

/// What a lowered expression needs from outside itself: the columns of the
/// sheet it runs over, and what a constant holds.
///
/// A column is asked for by index and comes back as the frame holds it, and
/// the ones asked for are remembered: the frame the expression is evaluated
/// over then holds exactly the columns it read, which is also how only the
/// columns a calculation needs are ever built.
pub struct Cx<'a> {
    column: &'a dyn Fn(usize) -> Option<PlColumn>,
    constant: &'a dyn Fn(usize) -> Option<Value>,
    taken: RefCell<Vec<PlColumn>>,
}

impl<'a> Cx<'a> {
    pub fn new(
        column: &'a dyn Fn(usize) -> Option<PlColumn>,
        constant: &'a dyn Fn(usize) -> Option<Value>,
    ) -> Cx<'a> {
        Cx {
            column,
            constant,
            taken: RefCell::new(Vec::new()),
        }
    }

    /// The columns the expression read, each once, in the order it asked
    /// for them.
    pub fn taken(self) -> Vec<PlColumn> {
        self.taken.into_inner()
    }

    fn read(&self, c: usize) -> Option<Out> {
        let column = (self.column)(c)?;
        let (name, dtype) = (column.name().clone(), column.dtype().clone());
        let mut taken = self.taken.borrow_mut();
        if !taken.iter().any(|held| held.name() == &name) {
            taken.push(column);
        }
        // A Parquet date-time in nanoseconds or milliseconds is read as the
        // microseconds a sheet holds, as `dtype::cell` reads each value:
        // nanoseconds floored, so a time before 1970 rounds down as there.
        let us = DataType::Datetime(TimeUnit::Microseconds, None);
        match dtype {
            DataType::Datetime(TimeUnit::Nanoseconds, None) => {
                // Polars' own cast, stepped down one where it went up.
                let ns = col(name.clone()).cast(DataType::Int64);
                let t = col(name).cast(us.clone()).cast(DataType::Int64);
                let floored = when((t.clone() * lit(1_000i64)).gt(ns))
                    .then(t.clone() - lit(1i64))
                    .otherwise(t);
                Out::new(floored.cast(us.clone()), us)
            }
            DataType::Datetime(TimeUnit::Milliseconds, None) => {
                Out::new(col(name).cast(us.clone()), us)
            }
            _ => Out::new(col(name), dtype),
        }
    }
}

/// An expression as one Polars expression over the whole column, or `None`
/// where Polars has not the same answer for some part of it.
pub fn expr(node: &Node, cx: &Cx) -> Option<Out> {
    match &node.kind {
        Ir::Lit(l) => literal(l),
        Ir::Const(i) => literal(&lit_of((cx.constant)(*i)?)?),
        // The row the formula is evaluated for, which is the only row a
        // lowered expression has: a reference reaching out of it is one the
        // first gate has already refused, and one reaching a row of an
        // enclosing sheet is a frame this expression is not over.
        Ir::RowCol { depth: 0, col } => cx.read(*col),
        Ir::Call(func, args) => call(*func, args, cx),
        Ir::Binary(BinOp::In, l, r) => within(expr(l, cx)?, r, cx),
        Ir::Unary(op, operand) => unary(*op, expr(operand, cx)?),
        Ir::Binary(op, l, r) => binary(*op, expr(l, cx)?, expr(r, cx)?),
        Ir::If(cond, yes, no) => {
            let cond = expr(cond, cx)?;
            if cond.dtype != DataType::Boolean {
                return None;
            }
            let (yes, no) = (expr(yes, cx)?, expr(no, cx)?);
            let (yes, no, dtype) = pair(yes, no)?;
            Out::new(when(cond.expr).then(yes).otherwise(no), dtype)
        }
        _ => None,
    }
}

/// An aggregate over one whole column as a Polars expression, or `None`
/// where Polars' answer is not the engine's.
///
/// `mean` is never here (D72): Polars answers it with an `f64` where the
/// engine answers exactly. The caller lowers it all the same, as a `sum`
/// and a `count` asked for here and divided by the engine, so the division
/// is a `Dec`'s own. Nor is any aggregate that adds floating-point numbers
/// up — `sum`, `product`, and `mean` again — because addition of them is
/// not associative, so a sum collected in parallel and a sum folded from
/// the left are not the same number. `min` and `max` do not care in which
/// order they look, so a float is no trouble to them.
pub fn aggregate(func: Func, name: &PlSmallStr, dtype: &DataType) -> Option<Expr> {
    let column = || col(name.clone());
    // A sum or an extreme of exact numbers widens to a decimal first, an
    // `Int64` included, for the reason `arith` widens: a `Dec` holds 38
    // digits where an `i64` holds 19, so a column whose total overflows
    // Polars' own addition still has an answer on the engine. Getting that
    // answer is the caller's part — 38 digits is not every total either,
    // and Polars wraps rather than refusing, so the caller bounds the sum
    // before it believes it (D72d).
    let exact = exact_scale(dtype).map(|scale| column().strict_cast(decimal(scale)));
    Some(match func {
        // How many rows there are, and how many of them hold a value:
        // Polars' `len` counts the rows and its `count` the values that are
        // not null, which is the same pair and the whole difference between
        // the two here.
        Func::Len => column().len(),
        Func::Count => column().count(),
        Func::Sum => exact?.sum(),
        // D71a, with `propagate_nans` on: omasheet holds a `NaN` to be
        // greatest, which makes its `max` exactly Polars' `nan_max` and its
        // `min` exactly Polars' default `min`. The two part company over a
        // column of nothing but `NaN`s, where the default `min` is null and
        // omasheet answers the `NaN`, and the caller is what detects that.
        Func::Max => match exact {
            Some(exact) => exact.max(),
            None if is_float(dtype) => column().nan_max(),
            None if temporal(dtype) => column().max(),
            None => return None,
        },
        Func::Min => match exact {
            Some(exact) => exact.min(),
            None if is_float(dtype) => column().min(),
            None if temporal(dtype) => column().min(),
            None => return None,
        },
        _ => return None,
    })
}

/// Whether a column holds a number at all, for the one case D71a leaves on
/// the engine: `min` over a floating-point column of nothing but `NaN`s.
pub fn has_a_number(name: &PlSmallStr) -> Expr {
    col(name.clone()).is_not_nan().any(true)
}

/// Whether this is a dtype a floating-point number is held as.
pub fn float(dtype: &DataType) -> bool {
    is_float(dtype)
}

/// A literal as a Polars one. An exact number is a `Decimal(38, s)`, which
/// is what a `Dec` was built to be.
fn literal(l: &Lit) -> Option<Out> {
    match l {
        Lit::Int(n, _) => Out::new(lit(*n), DataType::Int64),
        Lit::Decimal(d) => {
            // Polars names a scale of `0..=38`: a number held past that, as
            // `1e80` is, has no decimal to be and keeps its calculation on
            // the engine.
            let scale = u32::try_from(d.scale())
                .ok()
                .filter(|s| *s <= MAX_PRECISION)?;
            let dtype = decimal(scale);
            let value = AnyValue::Decimal(d.unscaled(), scale as usize);
            Out::new(lit(Scalar::new(dtype.clone(), value)), dtype)
        }
        Lit::Float(f) => Out::new(lit(*f), DataType::Float64),
        Lit::String(s) => Out::new(lit(s.clone()), DataType::String),
        Lit::Boolean(b) => Out::new(lit(*b), DataType::Boolean),
        Lit::Date(d) => Out::new(
            lit(Scalar::new(DataType::Date, AnyValue::Date(*d))),
            DataType::Date,
        ),
        Lit::Time(ns) => Out::new(
            lit(Scalar::new(DataType::Time, AnyValue::Time(*ns))),
            DataType::Time,
        ),
        Lit::DateTime(us) => {
            let dtype = DataType::Datetime(TimeUnit::Microseconds, None);
            let value = AnyValue::DatetimeOwned(*us, TimeUnit::Microseconds, None);
            Out::new(lit(Scalar::new(dtype.clone(), value)), dtype)
        }
        Lit::Duration(us) => {
            let dtype = DataType::Duration(TimeUnit::Microseconds);
            let value = AnyValue::Duration(*us, TimeUnit::Microseconds);
            Out::new(lit(Scalar::new(dtype.clone(), value)), dtype)
        }
        // A percentage is display, not a dtype (D69).
        Lit::Percent(_) => None,
    }
}

/// A calculated value as the literal it was written as, or `None` for one
/// no literal can write: a percentage or a fraction, whose dress belongs to
/// a column rather than to a value; a date-time in another zone, whose zone
/// a Polars column carries on its dtype and not per value (D70); and
/// nothing, which has no type to be a literal of.
fn lit_of(v: Value) -> Option<Lit> {
    Some(match v {
        Value::Int(n, b) => Lit::Int(n, b),
        Value::Decimal(d) => Lit::Decimal(d),
        Value::Float(f) => Lit::Float(f),
        Value::String(s, _) => Lit::String(s.to_string()),
        Value::Boolean(b) => Lit::Boolean(b),
        Value::Date(d) => Lit::Date(d),
        Value::Time(t) => Lit::Time(t),
        Value::DateTime(t) => Lit::DateTime(t),
        Value::Duration(t) => Lit::Duration(t),
        _ => return None,
    })
}

fn unary(op: UnOp, operand: Out) -> Option<Out> {
    match (op, exact_scale(&operand.dtype)) {
        // Negation of an exact number is `0 - x` on the engine, so it is
        // the same widening into a decimal here.
        (UnOp::Neg, Some(scale)) => {
            let dtype = decimal(scale);
            Out::new(-operand.expr.strict_cast(dtype.clone()), dtype)
        }
        (UnOp::Neg, None) if is_float(&operand.dtype) => Out::new(-operand.expr, DataType::Float64),
        (UnOp::Not, _) if operand.dtype == DataType::Boolean => {
            Out::new(operand.expr.not(), DataType::Boolean)
        }
        // Polars' `not` of an integer is its bitwise complement.
        (UnOp::BitNot, _) if operand.dtype == DataType::Int64 => {
            Out::new(operand.expr.not(), DataType::Int64)
        }
        // `^n` is a range, which no column holds.
        _ => None,
    }
}

/// A call of one argument as one Polars expression, or `None` where Polars
/// has not the same answer for it.
///
/// Most calls are not here, and two kinds of reason keep them out. Some are
/// omasheet's own and not Polars': every conversion reads and writes a
/// value the way the text does, `round` and the exact `floor` and `ceil`
/// work on a `Dec`, and a zone call answers from the engine's own tables.
/// The rest are Polars' but behind a feature this build does not enable —
/// `abs`, `sign`, `round`, `floor`, `ceil`, `exp`, `log` and every
/// trigonometric function among them — which is a size question (task 1.3)
/// rather than a semantic one.
fn call(func: Func, args: &[Node], cx: &Cx) -> Option<Out> {
    // The tests of text against text (filter-by-expression D174): both
    // sides text, and the answer a truth value, empty of an empty cell.
    if let Func::StartsWith | Func::EndsWith | Func::Contains | Func::Matches = func {
        let [text, part] = args else {
            return None;
        };
        let (text, part) = (expr(text, cx)?, expr(part, cx)?);
        if text.dtype != DataType::String || part.dtype != DataType::String {
            return None;
        }
        let s = text.expr.str();
        let test = match func {
            Func::StartsWith => s.starts_with(part.expr),
            Func::EndsWith => s.ends_with(part.expr),
            Func::Contains => s.contains_literal(part.expr),
            // Polars calls a regex match `contains`; strict, so that a bad
            // one fails rather than matching nothing.
            _ => s.contains(part.expr, true),
        };
        return Out::new(test, DataType::Boolean);
    }
    if func == Func::ContainsAny {
        let [text, parts] = args else {
            return None;
        };
        let text = expr(text, cx)?;
        let Ir::VecLit(items) = &parts.kind else {
            return None;
        };
        let parts: Vec<String> = items
            .iter()
            .map(|item| match &item.kind {
                Ir::Lit(Lit::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect::<Option<_>>()?;
        if text.dtype != DataType::String {
            return None;
        }
        let parts = lit(Series::new(PlSmallStr::EMPTY, parts)).implode();
        return Out::new(text.expr.str().contains_any(parts, false), DataType::Boolean);
    }
    // A value among its column's. Over this row's cell the checker gives
    // the column and the row's place as well, which Polars needs neither
    // of: its answer is a column's already.
    if let Func::IsUnique | Func::IsDuplicated | Func::IsFirstDistinct | Func::IsLastDistinct = func {
        let column = expr(args.first()?, cx)?;
        let kept = exact_scale(&column.dtype) == Some(0)
            || is_float(&column.dtype)
            || holdable(&column.dtype);
        if !kept {
            return None;
        }
        let test = match func {
            Func::IsUnique => column.expr.is_unique(),
            Func::IsDuplicated => column.expr.is_duplicated(),
            Func::IsFirstDistinct => column.expr.is_first_distinct(),
            _ => column.expr.is_last_distinct(),
        };
        return Out::new(test, DataType::Boolean);
    }
    let [arg] = args else {
        return None;
    };
    let out = expr(arg, cx)?;
    Some(match func {
        // Whether a cell is empty, which is the one call that asks about
        // the absence rather than the value, so every dtype answers it and
        // the answer is never itself empty.
        Func::IsNull => Out::new(out.expr.is_null(), DataType::Boolean)?,
        // Whether it holds a `NaN`, which only a floating-point column can.
        // Over any other the engine answers `false` for every value it
        // holds, and Polars has no answer at all, so it is left alone.
        Func::IsNan if is_float(&out.dtype) => Out::new(out.expr.is_nan(), DataType::Boolean)?,
        Func::IsFinite if is_float(&out.dtype) => {
            Out::new(out.expr.is_finite(), DataType::Boolean)?
        }
        Func::IsInfinite if is_float(&out.dtype) => {
            Out::new(out.expr.is_infinite(), DataType::Boolean)?
        }
        // An exact number is always finite, and empty stays empty.
        Func::IsFinite | Func::IsInfinite if exact_scale(&out.dtype).is_some() => Out::new(
            when(out.expr.is_not_null())
                .then(lit(func == Func::IsFinite))
                .otherwise(lit(NULL).cast(DataType::Boolean)),
            DataType::Boolean,
        )?,
        Func::IsLeapYear
            if matches!(
                out.dtype,
                DataType::Date | DataType::Datetime(TimeUnit::Microseconds, None)
            ) =>
        {
            Out::new(out.expr.dt().is_leap_year(), DataType::Boolean)?
        }
        Func::LenChars if out.dtype == DataType::String => Out::new(
            out.expr.str().len_chars().cast(DataType::Int64),
            DataType::Int64,
        )?,
        Func::ToLowercase if out.dtype == DataType::String => {
            Out::new(out.expr.str().to_lowercase(), DataType::String)?
        }
        Func::ToUppercase if out.dtype == DataType::String => {
            Out::new(out.expr.str().to_uppercase(), DataType::String)?
        }
        // A part of a date or a time. The dtype says which parts there are:
        // a `Date` has no hour and a `Time` no month, and the engine
        // reports either as an error, so a column with the wrong half is
        // not lowered rather than lowered to something that differs.
        // Polars counts the days of the week from Monday as `date::weekday`
        // does, and its week is the ISO week, which is the one the engine
        // works out from each Thursday.
        _ => return part(func, out),
    })
}

/// A date or time part, cast to the `Int64` a whole number is held as:
/// Polars answers these in an `Int32` or a `UInt32`, and a column of them
/// should be the width every other whole number in a sheet has.
fn part(func: Func, out: Out) -> Option<Out> {
    let dated = matches!(
        out.dtype,
        DataType::Date | DataType::Datetime(TimeUnit::Microseconds, None)
    );
    let timed = matches!(
        out.dtype,
        DataType::Time | DataType::Datetime(TimeUnit::Microseconds, None)
    );
    let dt = out.expr.clone().dt();
    let whole = |expr: Expr| Out::new(expr.cast(DataType::Int64), DataType::Int64);
    match func {
        Func::Year if dated => whole(dt.year()),
        Func::Month if dated => whole(dt.month()),
        Func::Day if dated => whole(dt.day()),
        Func::Weekday if dated => whole(dt.weekday()),
        Func::Quarter if dated => whole(dt.quarter()),
        Func::Week if dated => whole(dt.week()),
        Func::OrdinalDay if dated => whole(dt.ordinal_day()),
        Func::Hour if timed => whole(dt.hour()),
        Func::Minute if timed => whole(dt.minute()),
        Func::Second if timed => whole(dt.second()),
        // The day a date-time falls on, and the time of day it is at. Both
        // are a whole division of the microseconds since the epoch, and
        // before it that division has to floor rather than truncate, which
        // is why the dates these are tested over include one in 1969.
        Func::DateOf if dated => Out::new(out.expr.cast(DataType::Date), DataType::Date),
        Func::TimeOf if timed => Out::new(out.expr.cast(DataType::Time), DataType::Time),
        _ => None,
    }
}

/// `x in list` and `x.is_in(list)`: Polars' `is_in` over a list written
/// out, every element a literal of the needle's dtype, and `is_between`
/// over a range of whole numbers for a whole number. An empty needle is
/// empty and an empty element matches nothing, on both paths.
fn within(needle: Out, hay: &Node, cx: &Cx) -> Option<Out> {
    match &hay.kind {
        Ir::VecLit(items) => {
            let values: Vec<AnyValue<'static>> = items
                .iter()
                .map(|item| match &item.kind {
                    Ir::Lit(l) => element(l, &needle.dtype),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            let list =
                Series::from_any_values_and_dtype(PlSmallStr::EMPTY, &values, &needle.dtype, true)
                    .ok()?;
            Out::new(needle.expr.is_in(lit(list).implode(), false), DataType::Boolean)
        }
        Ir::Binary(op, lo, hi) if exact_scale(&needle.dtype) == Some(0) => {
            let (after, exclusive) = op.range_ends()?;
            let (lo, hi) = (expr(lo, cx)?, expr(hi, cx)?);
            between(needle, lo, hi, after, exclusive)
        }
        Ir::Unary(UnOp::UpTo, hi) if exact_scale(&needle.dtype) == Some(0) => {
            let hi = expr(hi, cx)?;
            between(needle, Out::new(lit(0i64), DataType::Int64)?, hi, false, true)
        }
        _ => None,
    }
}

fn between(needle: Out, lo: Out, hi: Out, after: bool, exclusive: bool) -> Option<Out> {
    if exact_scale(&lo.dtype) != Some(0) || exact_scale(&hi.dtype) != Some(0) {
        return None;
    }
    let closed = match (after, exclusive) {
        (false, false) => ClosedInterval::Both,
        (false, true) => ClosedInterval::Left,
        (true, false) => ClosedInterval::Right,
        (true, true) => ClosedInterval::None,
    };
    let int = |e: Expr| e.strict_cast(DataType::Int64);
    Out::new(
        int(needle.expr).is_between(int(lo.expr), int(hi.expr), closed),
        DataType::Boolean,
    )
}

/// A literal as an element of a list of `dtype`, or `None` where it is not
/// exactly one: a list of another dtype is the engine's to compare.
fn element(l: &Lit, dtype: &DataType) -> Option<AnyValue<'static>> {
    Some(match (l, dtype) {
        (Lit::Int(n, _), d) if exact_scale(d) == Some(0) => AnyValue::Int64(*n),
        (Lit::Float(f), DataType::Float64) => AnyValue::Float64(*f),
        (Lit::String(s), DataType::String) => AnyValue::StringOwned(s.as_str().into()),
        (Lit::Boolean(b), DataType::Boolean) => AnyValue::Boolean(*b),
        (Lit::Date(d), DataType::Date) => AnyValue::Date(*d),
        (Lit::DateTime(us), DataType::Datetime(TimeUnit::Microseconds, None)) => {
            AnyValue::Datetime(*us, TimeUnit::Microseconds, None)
        }
        (Lit::Time(ns), DataType::Time) => AnyValue::Time(*ns),
        _ => return None,
    })
}

fn binary(op: BinOp, l: Out, r: Out) -> Option<Out> {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => arith(op, l, r),
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => compare(op, l, r),
        // `a // b` is `b` where `a` is nothing. Polars has no short-circuit,
        // so it calculates `b` for every row where the engine calculates it
        // for some; a failure in it then fails the whole group, which falls
        // back to the engine and is calculated exactly as before.
        BinOp::Fallback => {
            let (l, r, dtype) = pair(l, r)?;
            Out::new(l.fill_null(r), dtype)
        }
        // Kleene's logic both sides now, so the two agree on an empty cell:
        // one that cannot change the answer does not decide it. Polars has
        // no short-circuit, so where the engine stops at a decisive left
        // operand this calculates both — the same answer, and the same
        // caveat as `//` above.
        BinOp::And | BinOp::Or => {
            if l.dtype != DataType::Boolean || r.dtype != DataType::Boolean {
                return None;
            }
            let expr = match op {
                BinOp::And => l.expr.and(r.expr),
                _ => l.expr.or(r.expr),
            };
            Out::new(expr, DataType::Boolean)
        }
        // Polars' `and`, `or` and `xor` of two integers are bitwise. The
        // shifts are left to the engine, which says where one goes too far.
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
            if l.dtype != DataType::Int64 || r.dtype != DataType::Int64 {
                return None;
            }
            let expr = match op {
                BinOp::BitAnd => l.expr.and(r.expr),
                BinOp::BitOr => l.expr.or(r.expr),
                _ => l.expr.xor(r.expr),
            };
            Out::new(expr, DataType::Int64)
        }
        _ => None,
    }
}

/// Arithmetic, where the two sides agree exactly with the engine's: `+`,
/// `-` and `*` on exact numbers held as a `Decimal(38, s)`, and the same
/// four on floating-point numbers.
fn arith(op: BinOp, l: Out, r: Out) -> Option<Out> {
    let apply = |l: Expr, r: Expr| match op {
        BinOp::Add => l + r,
        BinOp::Sub => l - r,
        BinOp::Mul => l * r,
        _ => l / r,
    };
    match (exact_scale(&l.dtype), exact_scale(&r.dtype)) {
        (Some(x), Some(y)) => {
            // The scale the engine would give, and the check D72 asks for:
            // a scale past the 38 digits a decimal holds is where `Dec`
            // gives up digits after the point and Polars does something
            // else, so the column is left on the engine.
            let scale = match op {
                BinOp::Add | BinOp::Sub => x.max(y),
                // A product needs as many digits as its two sides have
                // between them, and a `Decimal` column holds all 38 of them
                // whatever its scale, so the dtype cannot say that a
                // product of two of them fits. Polars answers one that does
                // not with nothing at all rather than an error, and that is
                // caught where the column comes back: a null none of the
                // columns it read accounts for is not believed, and the
                // engine calculates the column instead, giving up its guard
                // digits to keep the answer.
                BinOp::Mul => x + y,
                // A quotient cannot be had at the scale the engine gives
                // it: Polars offers at most 21 digits after the point and
                // `Dec` is equal to itself only down to 30 (D72).
                _ => return None,
            };
            if scale > MAX_PRECISION {
                return None;
            }
            // Each side widens to a decimal, an `Int64` included: a `Dec`
            // holds 38 digits where an `i64` holds 19, so a column of whole
            // numbers that overflows Polars' own addition still has an
            // answer on the engine.
            let (ld, rd) = match op {
                BinOp::Mul => (decimal(x), decimal(y)),
                _ => (decimal(scale), decimal(scale)),
            };
            let expr = apply(l.expr.strict_cast(ld), r.expr.strict_cast(rd));
            Out::new(expr, decimal(scale))
        }
        // A float on either side, and the other a number: both as `f64`,
        // which is what the engine does with the pair.
        (x, y) if (x.is_some() || is_float(&l.dtype)) && (y.is_some() || is_float(&r.dtype)) => {
            let expr = apply(
                l.expr.strict_cast(DataType::Float64),
                r.expr.strict_cast(DataType::Float64),
            );
            Out::new(expr, DataType::Float64)
        }
        _ => None,
    }
}

/// A comparison. Exact numbers are compared to 30 significant digits here
/// and to all 38 in Polars, which can only differ for a number holding more
/// than 30 — and the only operation that makes one is division, which is
/// not lowered. A `NaN` is greatest and equal to itself on both paths
/// (D56), and a comparison with nothing is nothing on both.
fn compare(op: BinOp, l: Out, r: Out) -> Option<Out> {
    let (l, r, _) = pair(l, r)?;
    let expr = match op {
        BinOp::Eq => l.eq(r),
        BinOp::Ne => l.neq(r),
        BinOp::Lt => l.lt(r),
        BinOp::Gt => l.gt(r),
        BinOp::Le => l.lt_eq(r),
        _ => l.gt_eq(r),
    };
    Out::new(expr, DataType::Boolean)
}

/// Two expressions at one dtype, for an operation that needs both sides to
/// be the same: the pair of them and the dtype they met at, or `None` where
/// they have no common dtype the engine would agree about.
fn pair(l: Out, r: Out) -> Option<(Expr, Expr, DataType)> {
    let dtype = match (exact_scale(&l.dtype), exact_scale(&r.dtype)) {
        (Some(x), Some(y)) => decimal(x.max(y)),
        (x, y) if (x.is_some() || is_float(&l.dtype)) && (y.is_some() || is_float(&r.dtype)) => {
            DataType::Float64
        }
        // Text, a truth value, a date, a time of day, a date-time in the
        // file's own zone: the same dtype on both sides and nothing to
        // convert. A zone on the dtype is one column one zone (D70), which
        // waits for the column that carries it.
        _ if l.dtype == r.dtype && holdable(&l.dtype) => l.dtype.clone(),
        _ => return None,
    };
    Some((
        l.expr.strict_cast(dtype.clone()),
        r.expr.strict_cast(dtype.clone()),
        dtype,
    ))
}

/// The scale an exact number is held at, or `None` for a dtype that is not
/// one. A whole number is an exact number of scale nothing.
pub fn exact_scale(dtype: &DataType) -> Option<u32> {
    match dtype {
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => Some(0),
        DataType::Decimal(_, scale) => Some(scale.unwrap_or(0) as u32),
        _ => None,
    }
}

fn is_float(dtype: &DataType) -> bool {
    matches!(dtype, DataType::Float32 | DataType::Float64)
}

/// Whether a column holds a date, a time, a timestamp or a duration. An
/// extreme of one
/// is the extreme of the whole number it is held as, which is the order
/// `value::compare` puts two of them in, so `min` and `max` of such a
/// column are Polars' to take. A `Datetime` of another unit or in a zone is
/// not one a sheet holds and is left alone.
fn temporal(dtype: &DataType) -> bool {
    match dtype {
        DataType::Datetime(unit, zone) => *unit == TimeUnit::Microseconds && zone.is_none(),
        DataType::Date | DataType::Time => true,
        DataType::Duration(unit) => *unit == TimeUnit::Microseconds,
        _ => false,
    }
}

/// Whether a dtype is one a sheet holds values of, with no conversion
/// either way: what [`crate::dtype::dtype`] names.
fn holdable(dtype: &DataType) -> bool {
    match dtype {
        DataType::Datetime(unit, zone) => *unit == TimeUnit::Microseconds && zone.is_none(),
        DataType::String | DataType::Boolean | DataType::Date | DataType::Time => true,
        DataType::Duration(unit) => *unit == TimeUnit::Microseconds,
        _ => false,
    }
}

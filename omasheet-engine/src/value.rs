// Copyright (c) 2026 Stephen Roe

//! Runtime values and exact arithmetic.
//!
//! Exact numbers are `Int`, `Decimal`, `Fraction` or `Percent`; the last
//! three hold the same fixed-point number and differ only in how it is
//! shown. Nothing here turns an exact number into a `Float` unless the other
//! operand already is one.

use crate::zone::{Zoned, format_offset};
use omasheet_omx::ast::{BinOp, Lit};
use omasheet_omx::convert;
use omasheet_omx::date;
use omasheet_omx::date::Style;
use omasheet_omx::decimal::{Dec, MAX_DENOMINATOR, SHOWN_PRECISION};
use omasheet_omx::funcs::MathFn;
use omasheet_omx::types::{Base, S};
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub enum Value {
    /// No value: a blank cell, or a row offset outside the sheet. SQL-style,
    /// so arithmetic with it is `Null` and aggregates pass over it.
    #[default]
    Null,
    /// A cell whose calculation failed; the diagnostic was already reported.
    Error,
    /// A whole number, and the base it is shown in.
    Int(i64, Base),
    /// An exact number that is not whole, shown as a decimal: `2.75`.
    Decimal(Dec),
    /// An exact number that is not whole, shown as a fraction: `1/3`, and
    /// `2 3/4` for `2.75`.
    Fraction(Dec),
    /// An exact number shown as a percentage: `0.2` for `20%`. It may be
    /// whole, `100%`.
    Percent(Dec),
    Float(f64),
    /// Text, and whether it was written bare in a cell, to be read as
    /// Markdown where a file is shown as a document. Text written in quotes,
    /// or made by a formula, is as it is.
    String(Rc<str>, bool),
    Boolean(bool),
    Date(i32),
    /// Nanoseconds since midnight, as Polars holds a `Time`.
    Time(i64),
    /// What the clocks of the file's time zone show.
    /// Microseconds since the start of 1970-01-01, on the clocks of the
    /// file's own zone, as Polars holds a `Datetime`.
    DateTime(i64),
    /// A date-time moved to another time zone.
    Zoned(Rc<Zoned>),
    Range {
        lo: i64,
        hi: i64,
        /// Leaves out its first end: `lo^..hi`.
        after: bool,
        exclusive: bool,
    },
    Vector(Rc<Vec<Value>>),
    Sheet(View),
    Row(RowRef),
}

/// Rows and columns of a sheet of the file, by index. Cells are read on demand, so
/// selecting never copies data.
#[derive(Clone, Debug)]
pub struct View {
    pub sheet: usize,
    pub rows: Rc<Vec<usize>>,
    pub cols: Rc<Vec<usize>>,
}

#[derive(Clone, Debug)]
pub struct RowRef {
    pub sheet: usize,
    pub row: usize,
    pub cols: Rc<Vec<usize>>,
}

impl Value {
    /// Whether this is text to be read as Markdown: see [`Value::String`].
    pub fn is_markdown(&self) -> bool {
        matches!(self, Value::String(_, true))
    }

    /// A whole number shown in decimal.
    pub fn int(n: i64) -> Value {
        Value::Int(n, Base::Dec)
    }

    /// A whole number shown in `base`; anything else as it is.
    pub fn rebased(self, base: Base) -> Value {
        match self {
            Value::Int(n, _) => Value::Int(n, base),
            other => other,
        }
    }

    pub fn text(s: &str) -> Value {
        Value::String(Rc::from(s), false)
    }

    /// An exact number: an `Int` when it is whole and fits one, and
    /// otherwise a `Decimal`.
    pub fn exact(r: Dec) -> Value {
        match r.to_i64() {
            Some(n) => Value::int(n),
            None => Value::Decimal(r),
        }
    }

    /// An exact number as a fraction, `2.75` as `2 3/4`; anything else as it
    /// is. It may be whole, and is shown plainly when it is: `3`, not `3/1`.
    pub fn fraction(self) -> Value {
        match self {
            Value::Int(n, _) => Value::Fraction(Dec::from_i64(n)),
            Value::Decimal(r) => Value::Fraction(r),
            other => other,
        }
    }

    /// An exact number as a percentage; anything else as it is.
    pub fn percent(self) -> Value {
        match self {
            Value::Int(n, _) => Value::Percent(Dec::from_i64(n)),
            Value::Decimal(r) | Value::Fraction(r) => Value::Percent(r),
            other => other,
        }
    }

    /// A percentage as the plain number it stands for; anything else as it
    /// is.
    pub fn plain(self) -> Value {
        match self {
            Value::Percent(r) | Value::Fraction(r) => Value::exact(r),
            other => other,
        }
    }

    pub fn from_lit(l: &Lit) -> Value {
        match l {
            Lit::Int(n, b) => Value::Int(*n, *b),
            Lit::Decimal(r) => Value::exact(*r),
            Lit::Percent(r) => Value::Percent(*r),
            Lit::Float(f) => Value::Float(*f),
            Lit::String(s) => Value::text(s),
            Lit::Boolean(b) => Value::Boolean(*b),
            Lit::Date(d) => Value::Date(*d),
            Lit::Time(t) => Value::Time(*t),
            Lit::DateTime(t) => Value::DateTime(*t),
        }
    }

    /// What kind of value this is, for messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Error => "an error",
            Value::Int(_, b) => match b {
                Base::Dec => "Integer",
                Base::Hex => "Hex",
                Base::Oct => "Octal",
                Base::Bin => "Binary",
            },
            Value::Decimal(_) => "Decimal",
            Value::Fraction(_) => "Fraction",
            Value::Percent(_) => "Percent",
            Value::Float(_) => "Float",
            Value::String(..) => "String",
            Value::Boolean(_) => "Boolean",
            Value::Date(_) => "Date",
            Value::Time(_) => "Time",
            Value::DateTime(_) | Value::Zoned(_) => "Datetime",
            Value::Range { .. } => "a range",
            Value::Vector(_) => "a vector",
            Value::Sheet(_) => "a sheet",
            Value::Row(_) => "a row",
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Value::Int(_, _)
                | Value::Decimal(_)
                | Value::Fraction(_)
                | Value::Percent(_)
                | Value::Float(_)
        )
    }

    /// A single value as a literal. A date-time in another zone is what its
    /// clocks show. `None` for empty, an error, or more than one value.
    fn to_lit(&self) -> Option<Lit> {
        Some(match self {
            Value::Int(n, b) => Lit::Int(*n, *b),
            // A `Fraction` is a `Decimal` shown another way; there is no
            // fraction literal to be had.
            Value::Decimal(r) | Value::Fraction(r) => Lit::Decimal(*r),
            Value::Percent(r) => Lit::Percent(*r),
            Value::Float(f) => Lit::Float(*f),
            Value::String(s, _) => Lit::String(s.to_string()),
            Value::Boolean(b) => Lit::Boolean(*b),
            Value::Date(d) => Lit::Date(*d),
            Value::Time(t) => Lit::Time(*t),
            Value::DateTime(t) => Lit::DateTime(*t),
            Value::Zoned(z) => Lit::DateTime(z.wall),
            _ => return None,
        })
    }

    /// This value as the type `to`: what `Int(x)`, `Text(x)` and the rest
    /// give. Empty stays empty.
    pub fn converted(&self, to: S) -> Result<Value, String> {
        let fits = match (self, to) {
            (Value::Null, _) | (_, S::Any) => true,
            (Value::Zoned(_), S::DateTime) | (Value::String(..), S::String) => true,
            // A whole `Decimal` is held as an `Int`.
            (Value::Int(_, b), to) if to.is_int() => *b == Base::of(to),
            (Value::Int(_, Base::Dec), S::Decimal) => true,
            (Value::Decimal(_), S::Decimal) => true,
            (Value::Fraction(_), S::Fraction) => true,
            (Value::Percent(_), S::Percent) => true,
            _ => false,
        };
        if fits {
            return Ok(self.clone());
        }
        // A `Fraction` is a `Decimal` shown as a fraction, so asking for one
        // is a change of display and not of value.
        if to == S::Fraction {
            return Ok(self.converted(S::Decimal)?.fraction());
        }
        let Some(lit) = self.to_lit() else {
            return Err(format!("{} cannot be made a `{to}`", self.kind()));
        };
        if to == S::String {
            return Ok(Value::text(&format_exact(self, false, &Style::ISO)));
        }
        convert::convert(&lit, to).map(|l| Value::from_lit(&l))
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n, _) => Some(*n),
            _ => None,
        }
    }
}

enum N {
    Exact(Dec),
    Float(f64),
}

fn number(v: &Value) -> Option<N> {
    Some(match v {
        Value::Int(n, _) => N::Exact(Dec::from_i64(*n)),
        Value::Decimal(r) | Value::Fraction(r) | Value::Percent(r) => N::Exact(*r),
        Value::Float(f) => N::Float(*f),
        _ => return None,
    })
}

pub fn to_f64(r: &Dec) -> f64 {
    r.to_f64()
}

fn real(n: &N) -> f64 {
    match n {
        N::Exact(r) => r.to_f64(),
        N::Float(f) => *f,
    }
}

/// `+ - * / **` on two single values. Empty in, empty out.
/// `+&`, `+|`, `+^`, `+<` or `+>` of two whole numbers, on their 64-bit two's
/// complement bits. Of two in one base the result is in that base, and of a
/// mix an `Integer` (D134); a shift is of the number on its left, and keeps
/// its base. `>>` fills with the sign (D137).
pub fn bitwise(op: BinOp, a: &Value, b: &Value) -> Result<Value, String> {
    let (&Value::Int(x, p), &Value::Int(y, q)) = (a, b) else {
        if matches!(a, Value::Null) || matches!(b, Value::Null) {
            return Ok(Value::Null);
        }
        return Err(format!(
            "`{}` needs whole numbers on both sides, found {} and {}",
            op.symbol(),
            a.kind(),
            b.kind()
        ));
    };
    let shift = || match u32::try_from(y) {
        Ok(n) if n < 64 => Ok(n),
        _ => Err(format!("cannot shift by {y} bits: a shift is of 0 to 63")),
    };
    let base = if p == q { p } else { Base::Dec };
    Ok(match op {
        BinOp::BitAnd => Value::Int(x & y, base),
        BinOp::BitOr => Value::Int(x | y, base),
        BinOp::BitXor => Value::Int(x ^ y, base),
        // Too large, as a product would be, where a bit is lost or the sign
        // turns.
        BinOp::Shl => {
            let n = shift()?;
            let out = x << n;
            if out >> n != x {
                return Err(TOO_LARGE.to_string());
            }
            Value::Int(out, p)
        }
        BinOp::Shr => Value::Int(x >> shift()?, p),
        _ => return Err(format!("`{}` is not a bitwise operator", op.symbol())),
    })
}

/// `+^n`: the bitwise complement of a whole number, `-n - 1`, in its base
/// (D135).
pub fn complement(v: &Value) -> Result<Value, String> {
    match v {
        Value::Null => Ok(Value::Null),
        Value::Int(n, b) => Ok(Value::Int(!n, *b)),
        other => Err(format!("cannot take the complement of {}", other.kind())),
    }
}

pub fn arith(op: BinOp, a: &Value, b: &Value) -> Result<Value, String> {
    if matches!(a, Value::Null) || matches!(b, Value::Null) {
        return Ok(Value::Null);
    }
    if let Some(shifted) = shift(op, a, b) {
        return shifted;
    }
    // Percentages add up to a percentage; anything else done with one gives
    // the plain number.
    if let (Value::Percent(x), Value::Percent(y), BinOp::Add | BinOp::Sub) = (a, b, op) {
        let sum = if op == BinOp::Add {
            x.checked_add(*y)
        } else {
            x.checked_sub(*y)
        };
        return match sum {
            Some(d) => Ok(Value::Percent(d)),
            None => Err(TOO_LARGE.to_string()),
        };
    }
    let (Some(x), Some(y)) = (number(a), number(b)) else {
        return Err(format!(
            "cannot apply `{}` to {} and {}",
            op.symbol(),
            a.kind(),
            b.kind()
        ));
    };
    // Whole numbers of one base give that base, but for a quotient, which
    // need not be whole; any other mix gives a decimal `Int`.
    let base = match (a, b) {
        (Value::Int(_, p), Value::Int(_, q)) if p == q && op != BinOp::Div => *p,
        _ => Base::Dec,
    };
    match (&x, &y) {
        // Anything done with a fraction gives a fraction.
        (N::Exact(x), N::Exact(y)) => exact_arith(op, x, y).map(|v| {
            if matches!(a, Value::Fraction(_)) || matches!(b, Value::Fraction(_)) {
                v.fraction()
            } else if let Value::Int(n, _) = v {
                Value::Int(n, base)
            } else {
                v
            }
        }),
        _ => {
            let (x, y) = (real(&x), real(&y));
            Ok(Value::Float(match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                BinOp::Mul => x * y,
                BinOp::Div => x / y,
                _ => x.powf(y),
            }))
        }
    }
}

/// `+` and `-` where a side is a date or time: a whole number counts days
/// next to a `Date` and seconds next to a `Time` or `Datetime`. `None` if
/// neither side is one, or the two do not combine.
fn shift(op: BinOp, a: &Value, b: &Value) -> Option<Result<Value, String>> {
    let minus = match op {
        BinOp::Add => false,
        BinOp::Sub => true,
        _ => return None,
    };
    let signed = |n: i64| if minus { n.checked_neg() } else { Some(n) };
    let date = |d: i32, n: &i64| {
        let days = signed(*n).and_then(|n| (d as i64).checked_add(n));
        match days.and_then(|d| i32::try_from(d).ok()) {
            Some(d) => Ok(Value::Date(d)),
            None => Err("the date is out of range".to_string()),
        }
    };
    // A whole number beside a date-time is that many seconds, which is the
    // promise the unit it is held in does not change.
    let datetime = |t: i64, n: &i64| {
        let moved = signed(*n)
            .and_then(|n| n.checked_mul(date::MICROS))
            .and_then(|n| t.checked_add(n));
        match moved {
            // Every date-time must have a day that is a valid date.
            Some(t) if i32::try_from(t.div_euclid(date::DAY_MICROS)).is_ok() => {
                Ok(Value::DateTime(t))
            }
            _ => Err("the date-time is out of range".to_string()),
        }
    };
    let zoned = |z: &Zoned, n: &i64| match signed(*n).and_then(|n| z.shifted(n)) {
        Some(z) => Ok(Value::Zoned(Rc::new(z))),
        None => Err("the date-time is out of range".to_string()),
    };
    // A time of day wraps round midnight, and a whole number beside one is
    // that many seconds.
    let time = |t: i64, n: &i64| {
        let n = (n % date::DAY) * date::NANOS;
        let n = if minus { -n } else { n };
        Ok(Value::Time((t + n).rem_euclid(date::DAY_NANOS)))
    };
    let int = |n: i64| Ok(Value::int(n));
    Some(match (a, b) {
        (Value::Date(d), Value::Int(n, _)) => date(*d, n),
        (Value::DateTime(t), Value::Int(n, _)) => datetime(*t, n),
        (Value::Zoned(z), Value::Int(n, _)) => zoned(z, n),
        (Value::Time(t), Value::Int(n, _)) => time(*t, n),
        (Value::Int(n, _), Value::Date(d)) if !minus => date(*d, n),
        (Value::Int(n, _), Value::DateTime(t)) if !minus => datetime(*t, n),
        (Value::Int(n, _), Value::Zoned(z)) if !minus => zoned(z, n),
        (Value::Int(n, _), Value::Time(t)) if !minus => time(*t, n),
        (Value::Date(d), Value::Time(t)) | (Value::Time(t), Value::Date(d)) if !minus => Ok(
            Value::DateTime(*d as i64 * date::DAY_MICROS + t / date::NANOS_PER_MICRO),
        ),
        // A difference is in seconds, whatever the two are held in.
        (Value::Date(x), Value::Date(y)) if minus => int(*x as i64 - *y as i64),
        (Value::Time(x), Value::Time(y)) if minus => int((x - y).div_euclid(date::NANOS)),
        (Value::DateTime(x), Value::DateTime(y)) if minus => int((x - y).div_euclid(date::MICROS)),
        (Value::Zoned(x), Value::Zoned(y)) if minus => {
            int((x.utc - y.utc).div_euclid(date::MICROS))
        }
        (Value::DateTime(x), Value::Zoned(y)) if minus => {
            int((x - y.home_wall()).div_euclid(date::MICROS))
        }
        (Value::Zoned(x), Value::DateTime(y)) if minus => {
            int((x.home_wall() - y).div_euclid(date::MICROS))
        }
        _ => return None,
    })
}

/// What a number too wide to hold is reported as.
const TOO_LARGE: &str = "the number is too large for the Decimal type";

/// An exact number divided by zero, which no exact number is: `Inf` or
/// `-Inf` by its sign, and `NaN` for zero.
fn over_zero(x: &Dec) -> f64 {
    if x.is_zero() {
        f64::NAN
    } else if x.is_negative() {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    }
}

fn exact_arith(op: BinOp, x: &Dec, y: &Dec) -> Result<Value, String> {
    let (x, y) = (*x, *y);
    let out = match op {
        BinOp::Add => x.checked_add(y),
        BinOp::Sub => x.checked_sub(y),
        BinOp::Mul => x.checked_mul(y),
        BinOp::Div => {
            if y.is_zero() {
                return Ok(Value::Float(over_zero(&x)));
            }
            x.checked_div(y)
        }
        _ => {
            if !y.is_integer() {
                return Err(
                    "`**` with a fractional exponent is not exact; use `.Float` on an operand"
                        .into(),
                );
            }
            let Some(e) = y.to_i64().and_then(|e| i32::try_from(e).ok()) else {
                return Err("the exponent is too large".into());
            };
            if x.is_zero() && e < 0 {
                return Ok(Value::Float(f64::INFINITY));
            }
            match e {
                0 => Some(Dec::ONE),
                e if e > 0 => x.powi(e.unsigned_abs()),
                e => x
                    .powi(e.unsigned_abs())
                    .and_then(|p| Dec::ONE.checked_div(p)),
            }
        }
    };
    out.map(Value::exact).ok_or_else(|| TOO_LARGE.to_string())
}

/// A function of one real number. An exact number stays exact where the
/// function allows it; otherwise the result is a `Num`.
pub fn math(m: MathFn, v: &Value) -> Result<Value, String> {
    let whole = |r: Dec| Value::exact(r);
    let x = match (v, m) {
        (Value::Null, _) => return Ok(Value::Null),
        (Value::Percent(r), MathFn::Abs) => {
            return r
                .checked_abs()
                .map(Value::Percent)
                .ok_or_else(|| TOO_LARGE.to_string());
        }
        (Value::Percent(_), _) => return math(m, &v.clone().plain()),
        (Value::Int(n, b), MathFn::Abs) => {
            return n
                .checked_abs()
                .map(|n| Value::Int(n, *b))
                .ok_or_else(|| TOO_LARGE.to_string());
        }
        (Value::Int(n, _), MathFn::Sign) => return Ok(Value::int(n.signum())),
        (Value::Int(_, _), _) if m.is_exact() => return Ok(v.clone()),
        (Value::Decimal(r), MathFn::Abs) => {
            return r
                .checked_abs()
                .map(Value::Decimal)
                .ok_or_else(|| TOO_LARGE.to_string());
        }
        (Value::Fraction(r), MathFn::Abs) => {
            return r
                .checked_abs()
                .map(Value::Fraction)
                .ok_or_else(|| TOO_LARGE.to_string());
        }
        (Value::Decimal(r) | Value::Fraction(r), MathFn::Sign) => {
            return Ok(Value::int(r.signum()));
        }
        (Value::Decimal(r) | Value::Fraction(r), MathFn::Round) => return Ok(whole(r.round())),
        (Value::Decimal(r) | Value::Fraction(r), MathFn::Floor) => return Ok(whole(r.floor())),
        (Value::Decimal(r) | Value::Fraction(r), MathFn::Ceil) => return Ok(whole(r.ceil())),
        (Value::Int(n, _), _) => *n as f64,
        (Value::Decimal(r) | Value::Fraction(r), _) => r.to_f64(),
        (Value::Float(f), _) => *f,
        (other, _) => {
            return Err(format!(
                "`{}` cannot be applied to {}",
                m.name(),
                other.kind()
            ));
        }
    };
    // Outside the function's domain it is as Raku has it: `sqrt(-1)` is
    // `NaN` and `ln(0)` is `-Inf`.
    Ok(Value::Float(m.apply(x)))
}

/// The nearest multiple of `scale`, which is what `round(x, 0.01)` means:
/// `round(x / scale) * scale`.
///
/// A number to `places` decimal places, as Polars rounds it: a half away
/// from zero, and a number already shown in fewer places left as it is,
/// since a `Decimal` carries its scale and widening it would only add
/// zeros. An `Int` is whole already, so it is itself.
pub fn round_digits(x: &Value, places: i64) -> Result<Value, String> {
    let places = i32::try_from(places.max(0)).unwrap_or(i32::MAX);
    let narrowed = |d: &Dec| d.rescale(places.min(d.scale())).unwrap_or(*d);
    Ok(match x {
        Value::Null => Value::Null,
        Value::Int(_, _) => x.clone(),
        Value::Decimal(d) => Value::Decimal(narrowed(d)),
        Value::Fraction(d) => Value::Fraction(narrowed(d)),
        Value::Percent(d) => Value::Percent(narrowed(d)),
        // An infinity and a `NaN` have no places to round to.
        Value::Float(f) if !f.is_finite() => Value::Float(*f),
        Value::Float(f) => {
            let factor = 10f64.powi(places.min(17));
            Value::Float((f * factor).round() / factor)
        }
        other => return Err(format!("cannot round {}", other.kind())),
    })
}

/// Whether a value is `NaN`, which is ordered greatest and equal to itself.
pub fn is_nan(v: &Value) -> bool {
    matches!(v, Value::Float(f) if f.is_nan())
}

/// Order two single values. `None` if either is empty.
pub fn compare(a: &Value, b: &Value) -> Result<Option<Ordering>, String> {
    Ok(Some(match (a, b) {
        (Value::Null, _) | (_, Value::Null) => return Ok(None),
        (Value::String(x, _), Value::String(y, _)) => x.cmp(y),
        (Value::Date(x), Value::Date(y)) => x.cmp(y),
        (Value::Time(x), Value::Time(y)) => x.cmp(y),
        (Value::DateTime(x), Value::DateTime(y)) => x.cmp(y),
        // Across time zones it is the instant that counts.
        (Value::Zoned(x), Value::Zoned(y)) => x.utc.cmp(&y.utc),
        (Value::DateTime(x), Value::Zoned(y)) => x.cmp(&y.home_wall()),
        (Value::Zoned(x), Value::DateTime(y)) => x.home_wall().cmp(y),
        (Value::Boolean(x), Value::Boolean(y)) => x.cmp(y),
        _ => match (number(a), number(b)) {
            (Some(N::Exact(x)), Some(N::Exact(y))) => x.cmp(&y),
            (Some(x), Some(y)) => {
                let (x, y) = (real(&x), real(&y));
                // A `NaN` is greatest and equal to itself, as Polars orders
                // it. Raku has it unordered, which would have kept every
                // column holding one off the dataframe.
                match (x.is_nan(), y.is_nan()) {
                    (true, true) => Ordering::Equal,
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                    (false, false) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
                }
            }
            _ => {
                return Err(format!("cannot compare {} and {}", a.kind(), b.kind()));
            }
        },
    }))
}

/// Order two single values for a sort, where every pair has an order:
/// an empty cell is smallest, so it comes first ascending and last
/// descending, as Polars orders a null. A `NaN` is greatest, by D56.
pub fn sort_order(a: &Value, b: &Value) -> Result<Ordering, String> {
    Ok(match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        _ => compare(a, b)?.unwrap_or(Ordering::Equal),
    })
}

/// Equality of two single values. `None` if either is empty.
pub fn equal(a: &Value, b: &Value) -> Result<Option<bool>, String> {
    Ok(compare(a, b)?.map(|o| o == Ordering::Equal))
}

/// The most characters a number is shown in, its sign aside.
pub const WIDTH: usize = 23;

/// What marks a number shown with fewer digits than it has.
pub const MORE: char = '…';

/// A number below this, other than zero, is small, and is shown in e notation.
const SMALL: f64 = 1e-15;

/// A number of this or more is large, and is shown in e notation.
const LARGE: f64 = 1e15;

/// How many digits after the point a `Percent` is rounded to.
pub const PERCENT_DIGITS: usize = 2;

/// An exact number as it is written: every digit of it, with no grouping and
/// no cutting. The guard digits are not part of the number, so they are not
/// written: see [`omasheet_omx::decimal`].
pub fn format_rat(r: &Dec) -> String {
    r.guarded().to_plain_string()
}

/// An exact number as a fraction of whole numbers, `1/3`, where one of a
/// small enough denominator comes to it. A whole one is written plainly.
pub fn format_fraction(r: &Dec) -> Option<String> {
    let (n, d) = r.to_fraction(MAX_DENOMINATOR)?;
    Some(if d == 1 {
        n.to_string()
    } else {
        format!("{n}/{d}")
    })
}

/// A fraction as a whole number and a part where it is more than one,
/// `2 3/4` for `2.75`, and as a plain fraction otherwise, `3/4`.
pub fn format_mixed(r: &Dec) -> Option<String> {
    let (n, d) = r.to_fraction(MAX_DENOMINATOR)?;
    if d == 1 {
        return Some(n.to_string());
    }
    let (whole, part) = (n / d, (n % d).abs());
    Some(if whole == 0 {
        format!("{n}/{d}")
    } else {
        format!("{whole} {part}/{d}")
    })
}

/// The exponent of e notation: `e+15`, `e-05`.
fn exponent(e: i64) -> String {
    format!("e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// A positive exact number in e notation, with every significant digit; cut
/// to [`WIDTH`] and marked with [`MORE`] where that is too wide.
fn scientific(r: &Dec) -> String {
    let (digits, e) = r.digits_and_exponent();
    let mantissa = match digits.split_at(1) {
        (first, "") => first.to_string(),
        (first, rest) => format!("{first}.{rest}"),
    };
    let exp = exponent(e);
    if mantissa.len() + exp.len() <= WIDTH {
        return format!("{mantissa}{exp}");
    }
    let keep = WIDTH - exp.len() - 1;
    format!("{}{MORE}{exp}", &mantissa[..keep])
}

/// How many characters `text` is.
fn width(text: &str) -> usize {
    text.chars().count()
}

/// What stands for a separator of digit groups until a number is shown in
/// a style, when it becomes the style's own. It is one character, as every
/// separator is, so that a number's width is known before then.
const GROUP: char = '\u{E000}';

/// A plain number with the digits before its point grouped by `sep` in
/// threes, where there are five of them or more: `12,345.678`.
fn grouped(plain: String, sep: Option<char>) -> String {
    let Some(sep) = sep else { return plain };
    let (whole, rest) = plain.split_at(plain.find('.').unwrap_or(plain.len()));
    if whole.len() < 5 || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return plain;
    }
    let mut out = String::new();
    for (k, digit) in whole.chars().enumerate() {
        if k > 0 && (whole.len() - k) % 3 == 0 {
            out.push(sep);
        }
        out.push(digit);
    }
    out + rest
}

/// A positive exact number, plain if it is neither small nor large and fits,
/// and otherwise in e notation or cut and marked.
fn shown_decimal(r: &Dec, sep: Option<char>) -> String {
    let r = &r.guarded();
    if r.is_zero() {
        return "0".into();
    }
    let e = r.magnitude();
    let medium = (-15..15).contains(&e);
    let plain = grouped(r.to_plain_string(), sep);
    if medium && width(&plain) <= WIDTH {
        return plain;
    }
    // A number of ordinary size that will not fit reads better cut and
    // marked, `0.33333333333333333333…`, than in e notation. Only the
    // genuinely small or large go to e notation.
    if medium {
        let digits: String = plain.chars().take(WIDTH - 1).collect();
        return format!("{digits}{MORE}");
    }
    scientific(r)
}

/// A positive exact number as a fraction, where one is to be had and it
/// fits: `2 3/4`, with the whole part grouped. `None` otherwise, and the
/// number is shown as a decimal instead.
fn shown_fraction(r: &Dec, sep: Option<char>) -> Option<String> {
    let mixed = format_mixed(&r.guarded())?;
    let mixed = match mixed.split_once(' ') {
        Some((whole, part)) => format!("{} {part}", grouped(whole.to_string(), sep)),
        None => mixed,
    };
    (width(&mixed) <= WIDTH).then_some(mixed)
}

/// How many digits after the point a long `Decimal` is given to, in full.
pub const LONG_DIGITS: usize = SHOWN_PRECISION as usize;

/// A `Decimal` whose shown form was cut, written out in full, for the app to
/// show when the cut form is pointed at. `None` for anything else, and for
/// one the shown form already gives whole.
pub fn long_decimal(v: &Value) -> Option<String> {
    let (Value::Decimal(r) | Value::Fraction(r)) = v else {
        return None;
    };
    let r = r.guarded();
    let plain = r.to_plain_string();
    let medium = (-15..15).contains(&r.magnitude());
    (medium && width(&plain) > WIDTH).then_some(plain)
}

/// A rounded decimal without the zeros, or the point, it ends with.
fn trimmed(text: String) -> String {
    if !text.contains('.') {
        return text;
    }
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A `Percent` as so many in a hundred. In full when `exact`, `12.5%`;
/// otherwise rounded to [`PERCENT_DIGITS`] digits after the point, `41.67%`,
/// and from `1e15%` in e notation, `1.43e+19%`. It is never marked with
/// [`MORE`]: rounding is the rule.
pub fn format_percent(r: &Dec, exact: bool) -> String {
    shown_percent(r, exact, None)
}

/// As [`format_percent`], with the digits grouped by `sep`.
fn shown_percent(r: &Dec, exact: bool, sep: Option<char>) -> String {
    let r = &r.guarded();
    let hundred = r.checked_mul(Dec::from_i64(100));
    if exact {
        return match hundred {
            Some(h) => format!("{}%", h.to_plain_string()),
            None => format!("{}%", r.to_plain_string()),
        };
    }
    // The size in hundredths, as digits and a power of ten, which is exact
    // even where multiplying by a hundred would not fit.
    let (digits, e) = r.digits_and_exponent();
    let e = e + 2;
    let shown = if e < 15 {
        let size = hundred.and_then(|h| h.checked_abs()).unwrap_or(Dec::ZERO);
        let rounded = size
            .rescale(PERCENT_DIGITS as i32)
            .unwrap_or(size)
            .to_plain_string();
        grouped(trimmed(rounded), sep)
    } else {
        let mantissa = match digits.split_at(1) {
            (first, "") => first.to_string(),
            (first, rest) => format!("{first}.{rest}"),
        };
        let rounded = Dec::parse(&mantissa)
            .and_then(|m| m.rescale(PERCENT_DIGITS as i32))
            .map(|m| m.to_plain_string())
            .unwrap_or(mantissa);
        format!("{}{}", trimmed(rounded), exponent(e))
    };
    let zero = shown.bytes().all(|b| !b.is_ascii_digit() || b == b'0');
    let sign = if r.is_negative() && !zero { "-" } else { "" };
    format!("{sign}{shown}%")
}

/// A `Num` as Raku writes one, with the fewest digits that give the same
/// `Num` back: a whole number as `100`, one with a fraction as `1.5`, and a
/// very large or very small one with an exponent, `1e+21` or `1.5e-07`.
/// Shown, it is plain from `1e-15` to `1e15` where that fits in [`WIDTH`],
/// and so never needs [`MORE`]; in full, as when `exact`, only from `1e-4`.
pub fn format_float(f: f64, exact: bool) -> String {
    shown_float(f, exact, None)
}

/// As [`format_float`], with the digits grouped by `sep`.
fn shown_float(f: f64, exact: bool, sep: Option<char>) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Inf" } else { "-Inf" }.into();
    }
    let size = f.abs();
    if exact && (size == 0.0 || (1e-4..LARGE).contains(&size)) {
        return format!("{f}");
    }
    let plain = grouped(format!("{size}"), sep);
    if !exact && (size == 0.0 || ((SMALL..LARGE).contains(&size) && width(&plain) <= WIDTH)) {
        return signed(f < 0.0, plain);
    }
    let scientific = format!("{f:e}");
    let (mantissa, e) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    format!("{mantissa}{}", exponent(e.parse().unwrap_or(0)))
}

/// A single value as text, the way a file shows it. `quoted` puts text in
/// quotes, as inside a vector.
pub fn format_scalar(v: &Value, quoted: bool) -> String {
    format_styled(v, quoted, &Style::ISO)
}

/// A single value as text, with dates and times in `style`.
pub fn format_styled(v: &Value, quoted: bool, style: &Style) -> String {
    format_value(v, quoted, style, false)
}

/// As [`format_styled`], with every `Num` in full: `1.4142135623730951`,
/// where it is shown as `1.41421…`. An exact number is always in full.
pub fn format_exact(v: &Value, quoted: bool, style: &Style) -> String {
    format_value(v, quoted, style, true)
}

/// What a number shown in `style` groups its digits by, until it is shown.
fn group(style: &Style) -> Option<char> {
    style.group.map(|_| GROUP)
}

/// A number shown as `style` writes one: its point the style's own, and its
/// digits grouped by the style's separator.
fn localised(text: String, style: &Style) -> String {
    let text = if style.point == '.' {
        text
    } else {
        text.replace('.', &style.point.to_string())
    };
    match style.group {
        Some(sep) => text.replace(GROUP, &sep.to_string()),
        None => text,
    }
}

/// As a number shown in `style` is written in a file: `12,345.5` and
/// `12.345,5` are `12345.5`.
pub fn delocalised(text: &str, style: &Style) -> String {
    text.chars()
        .filter(|&c| Some(c) != style.group)
        .map(|c| if c == style.point { '.' } else { c })
        .collect()
}

/// `text` with a `-` before it when `negative`.
fn signed(negative: bool, text: String) -> String {
    if negative { format!("-{text}") } else { text }
}

fn format_value(v: &Value, quoted: bool, style: &Style, exact: bool) -> String {
    match v {
        Value::Null => String::new(),
        Value::Error => "#ERROR".into(),
        // In full when `exact`, as a file writes it: `19.99`, and one that
        // does not end as `11/4`.
        Value::Int(n, b) if exact => b.write(*n),
        Value::Int(n, b) if *b != Base::Dec => b.show(*n),
        Value::Decimal(r) if exact => format_rat(r),
        // A `Fraction` in full is the fraction itself, which is the only way
        // to write one that does not end and read it back.
        Value::Fraction(r) if exact => format_fraction(r).unwrap_or_else(|| format_rat(r)),
        // Shown, in at most `WIDTH` characters and a sign: plain from
        // `1e-15` to `1e15`, and otherwise in e notation, `1e+60`.
        // Shown in the app, a plain number of five digits or more is grouped
        // in threes, `12,345`, and its point is the locale's.
        Value::Int(n, _) => localised(
            signed(
                n.is_negative(),
                shown_decimal(&Dec::from_i64(n.abs()), group(style)),
            ),
            style,
        ),
        Value::Decimal(r) => localised(
            signed(
                r.is_negative(),
                shown_decimal(&r.checked_abs().unwrap_or(*r), group(style)),
            ),
            style,
        ),
        // A fraction where there is one to be had, `2 3/4`, and the decimal
        // otherwise.
        Value::Fraction(r) => {
            let size = r.checked_abs().unwrap_or(*r);
            localised(
                signed(
                    r.is_negative(),
                    shown_fraction(&size, group(style))
                        .unwrap_or_else(|| shown_decimal(&size, group(style))),
                ),
                style,
            )
        }
        Value::Percent(r) if exact => format_percent(r, true),
        Value::Percent(r) => localised(shown_percent(r, false, group(style)), style),
        Value::Float(f) if exact => format_float(*f, true),
        Value::Float(f) => localised(shown_float(*f, false, group(style)), style),
        Value::String(s, _) if quoted => format!("{s:?}"),
        Value::String(s, _) => s.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Date(d) => style.date(*d),
        Value::Time(t) => style.time(*t),
        Value::DateTime(t) => style.datetime(*t),
        // `2025-01-31T09:30+09:00`, to tell it from the file's own zone.
        Value::Zoned(z) => format!(
            "{}{}{}",
            style.datetime(z.wall),
            if style.is_iso() { "" } else { " " },
            format_offset(z.offset())
        ),
        Value::Range {
            lo,
            hi,
            after,
            exclusive,
        } => {
            let op = match (after, exclusive) {
                (false, false) => "..",
                (false, true) => "..^",
                (true, false) => "^..",
                (true, true) => "^..^",
            };
            format!("{lo}{op}{hi}")
        }
        Value::Vector(items) => {
            let parts: Vec<String> = items
                .iter()
                .map(|x| match x {
                    Value::Null => "empty".to_string(),
                    other => format_value(other, true, style, exact),
                })
                .collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Sheet(_) => "<sheet>".into(),
        Value::Row(_) => "<row>".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omasheet_omx::date;

    fn rat(n: i64, d: i64) -> Dec {
        Dec::from_i64(n)
            .checked_div(Dec::from_i64(d))
            .expect("divides")
    }

    #[test]
    fn whole_numbers_in_a_base() {
        let hex = |n: i64| Value::Int(n, Base::Hex);
        let shown = |v: &Value| format_styled(v, false, &Style::ISO);
        assert_eq!(shown(&hex(255)), "0xff");
        // A negative one is its two's complement (D126), and written so
        // that it reads back as itself (D132).
        assert_eq!(shown(&hex(-255)), "0xffffffffffffff01");
        assert_eq!(format_exact(&hex(-255), false, &Style::ISO), "-0xff");
        assert_eq!(shown(&Value::Int(15, Base::Oct)), "0o17");
        assert_eq!(shown(&Value::Int(5, Base::Bin)), "0b101");
        // One base gives that base; a mix gives an `Integer`.
        let sum = arith(BinOp::Add, &hex(0xF0), &hex(0x0F)).unwrap();
        assert!(matches!(sum, Value::Int(255, Base::Hex)));
        let sum = arith(BinOp::Add, &hex(0xF0), &Value::int(1)).unwrap();
        assert!(matches!(sum, Value::Int(241, Base::Dec)));
        let bits = Value::Int(5, Base::Bin);
        let mixed = arith(BinOp::Mul, &hex(2), &bits).unwrap();
        assert!(matches!(mixed, Value::Int(10, Base::Dec)));
        assert!(matches!(
            hex(9).converted(S::Int),
            Ok(Value::Int(9, Base::Dec))
        ));
        assert!(matches!(
            Value::int(9).converted(S::Octal),
            Ok(Value::Int(9, Base::Oct))
        ));
    }

    #[test]
    fn bitwise_on_whole_numbers() {
        let hex = |n: i64| Value::Int(n, Base::Hex);
        let shown = |v: Result<Value, String>| format_styled(&v.unwrap(), false, &Style::ISO);
        assert_eq!(
            shown(bitwise(BinOp::BitAnd, &hex(0xF0), &hex(0x3C))),
            "0x30"
        );
        assert_eq!(shown(bitwise(BinOp::BitOr, &hex(0xF0), &hex(0x0F))), "0xff");
        // A mix is an `Integer` (D134); a shift keeps the base on its left.
        assert_eq!(
            shown(bitwise(BinOp::BitAnd, &hex(0xF0), &Value::int(60))),
            "48"
        );
        assert_eq!(
            shown(bitwise(BinOp::Shr, &hex(0xff), &Value::int(4))),
            "0xf"
        );
        // `+>` fills with the sign (D137), and the complement is `-n - 1`
        // (D135).
        assert_eq!(
            shown(bitwise(BinOp::Shr, &Value::int(-16), &Value::int(2))),
            "-4"
        );
        assert_eq!(shown(complement(&hex(0xff))), "0xffffffffffffff00");
        assert_eq!(shown(complement(&Value::int(5))), "-6");
        // Too far, too large, or not whole numbers (D138).
        assert!(bitwise(BinOp::Shl, &Value::int(1), &Value::int(64)).is_err());
        assert!(bitwise(BinOp::Shr, &Value::int(1), &Value::int(-1)).is_err());
        assert!(bitwise(BinOp::Shl, &Value::int(1), &Value::int(63)).is_err());
        assert!(bitwise(BinOp::BitAnd, &Value::Boolean(true), &Value::int(6)).is_err());
        assert!(matches!(
            bitwise(BinOp::BitAnd, &Value::Null, &Value::int(6)),
            Ok(Value::Null)
        ));
    }

    #[test]
    fn formats_rationals() {
        assert_eq!(format_rat(&rat(175, 4)), "43.75");
        assert_eq!(format_rat(&rat(1, 3)), "0.333333333333333333333333333333");
        assert_eq!(format_rat(&rat(-1, 8)), "-0.125");
        assert_eq!(format_rat(&rat(1, 20)), "0.05");
        assert_eq!(format_rat(&rat(-7, 3)), "-2.33333333333333333333333333333");
    }

    #[test]
    fn shows_rationals() {
        assert_eq!(format_rat(&rat(22, 7)), "3.14285714285714285714285714286");
        assert_eq!(format_rat(&rat(11, 4)), "2.75");
        assert_eq!(format_rat(&rat(-1, 3)), "-0.333333333333333333333333333333");
    }

    #[test]
    fn shows_fractions() {
        let shown = |n: i64, d: i64| format_scalar(&Value::Fraction(rat(n, d)), false);
        assert_eq!(shown(22, 7), "3 1/7");
        assert_eq!(shown(11, 4), "2 3/4");
        assert_eq!(shown(-11, 4), "-2 3/4");
        assert_eq!(shown(3, 4), "3/4");
        assert_eq!(shown(-1, 3), "-1/3");
        assert_eq!(shown(1000, 3), "333 1/3");
        assert_eq!(shown(1, 8), "1/8");
        // In full it is the fraction itself, which is how a file writes one.
        assert_eq!(
            format_exact(&Value::Fraction(rat(-11, 4)), false, &Style::ISO),
            "-11/4"
        );
        // One with no fraction of a small enough denominator is a decimal.
        assert_eq!(shown(1, 10_001), "0.00009999000099990000…");
        // Anything done with a fraction gives a fraction.
        let sum = arith(
            BinOp::Add,
            &Value::Fraction(rat(1, 3)),
            &Value::Decimal(rat(1, 6)),
        )
        .expect("adds");
        assert_eq!(format_scalar(&sum, false), "1/2");
    }

    #[test]
    fn shows_exact_numbers_in_23_characters() {
        let shown = |v: Value| format_scalar(&v, false);
        let int = |n: i64| Value::int(n);
        let dec = |s: &str| Value::Decimal(Dec::parse(s).expect("parses"));
        assert_eq!(shown(int(999_999_999_999_999)), "999999999999999");
        assert_eq!(shown(int(1_000_000_000_000_000)), "1e+15");
        assert_eq!(
            shown(int(123_456_789_012_345_678)),
            "1.23456789012345678e+17"
        );
        // Past an `i64` a whole number is still exact, as a decimal.
        assert_eq!(
            shown(dec("18446744073709551616")),
            "1.8446744073709551…e+19"
        );
        assert_eq!(
            shown(dec("-18446744073709551616")),
            "-1.8446744073709551…e+19"
        );
        assert_eq!(shown(dec("0.125")), "0.125");
        assert_eq!(shown(dec("0.000000000000001")), "0.000000000000001");
        // The widest and the narrowest a 38-digit decimal reaches.
        assert_eq!(shown(dec("1e-38")), "1e-38");
        assert_eq!(shown(dec(&"9".repeat(38))), "9.9999999999999999…e+37");
        assert_eq!(
            shown(dec("0.12345678901234567890123")),
            "0.12345678901234567890…"
        );
    }

    #[test]
    fn groups_digits_in_the_app() {
        let app = Style::ISO.numbers('.', ',');
        let shown = |v: Value| format_styled(&v, false, &app);
        let int = |n: i64| Value::int(n);
        assert_eq!(shown(int(2025)), "2025");
        assert_eq!(shown(int(12345)), "12,345");
        assert_eq!(shown(int(-1234567)), "-1,234,567");
        assert_eq!(shown(int(999_999_999_999_999)), "999,999,999,999,999");
        assert_eq!(shown(int(1_000_000_000_000_000)), "1e+15");
        assert_eq!(
            shown(Value::Decimal(rat(123_456_789, 10_000))),
            "12,345.6789"
        );
        assert_eq!(
            shown(Value::Decimal(
                Dec::parse("999999999999999.9999999").expect("parses")
            )),
            "999,999,999,999,999.99…"
        );
        assert_eq!(
            shown(Value::Percent(rat(12_345_678, 100_000))),
            "12,345.68%"
        );
        assert_eq!(shown(Value::Float(1_234_567.5)), "1,234,567.5");
        assert_eq!(shown(Value::Float(-0.000123)), "-0.000123");
        // Only the app groups them: a file, and `print`, do not.
        assert_eq!(format_scalar(&int(12345), false), "12345");
        assert_eq!(format_exact(&int(12345), false, &app), "12345");
    }

    #[test]
    fn shows_numbers_as_the_locale_does() {
        let de = Style::ISO.numbers(',', '.');
        let shown = |v: Value| format_styled(&v, false, &de);
        assert_eq!(shown(Value::Decimal(rat(123_455, 10))), "12.345,5");
        assert_eq!(shown(Value::Decimal(rat(1, 8))), "0,125");
        assert_eq!(shown(Value::Float(1.5e-20)), "1,5e-20");
        assert_eq!(shown(Value::Percent(rat(5, 12))), "41,67%");
        assert_eq!(delocalised("12.345,5", &de), "12345.5");
        // In full, as a file writes it, whatever the locale.
        assert_eq!(
            format_exact(&Value::Decimal(rat(123_455, 10)), false, &de),
            "12345.5"
        );
        let fr = Style::ISO.numbers(',', '\u{202F}');
        assert_eq!(
            format_styled(&Value::int(1_234_567.into()), false, &fr),
            "1\u{202F}234\u{202F}567"
        );
    }

    #[test]
    fn rounds_percentages_to_two_places() {
        let shown = |r| format_percent(&r, false);
        assert_eq!(shown(rat(5, 12)), "41.67%");
        assert_eq!(shown(rat(-5, 12)), "-41.67%");
        assert_eq!(shown(rat(1, 8)), "12.5%");
        assert_eq!(shown(rat(1, 5)), "20%");
        assert_eq!(shown(rat(1, 100_000_000)), "0%");
        assert_eq!(shown(rat(-1, 100_000_000)), "0%");
        assert_eq!(shown(rat(1, 20_000)), "0.01%");
        assert_eq!(shown(rat(10_000_000_000_000, 1)), "1e+15%");
        assert_eq!(shown(rat(1_000_000_000_000_000_000, 7)), "1.43e+19%");
        assert_eq!(
            format_percent(&rat(5, 12), true),
            "41.6666666666666666666666666667%"
        );
    }

    #[test]
    fn shows_nums_as_raku_does() {
        let shown: Vec<String> = [
            1.0,
            1.5,
            -0.25,
            0.1 + 0.2,
            1.0 / 3.0,
            1e14,
            1e15,
            123_456_789_012_345_678.0,
            1e-4,
            1e-5,
            1.5e-7,
            1e100,
            -0.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ]
        .iter()
        .map(|f| format_float(*f, true))
        .collect();
        assert_eq!(
            shown,
            [
                "1",
                "1.5",
                "-0.25",
                "0.30000000000000004",
                "0.3333333333333333",
                "100000000000000",
                "1e+15",
                "1.2345678901234568e+17",
                "0.0001",
                "1e-05",
                "1.5e-07",
                "1e+100",
                "-0",
                "Inf",
                "-Inf",
                "NaN",
            ]
        );
        // Shown, a `Num` is never rounded: plain from `1e-15` to `1e15`
        // where that fits in 23 characters, and in e notation otherwise.
        let shown = |f| format_float(f, false);
        assert_eq!(shown(1.5), "1.5");
        assert_eq!(shown(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(shown(-2.0 / 3.0), "-0.6666666666666666");
        assert_eq!(shown(1e-15), "0.000000000000001");
        assert_eq!(shown(9.999999999999999e-16), "9.999999999999999e-16");
        assert_eq!(shown(1.2345678e-15), "1.2345678e-15");
        assert_eq!(shown(999_999_999_999_999.9), "999999999999999.9");
        assert_eq!(shown(1e15), "1e+15");
        assert_eq!(shown(f64::MAX), "1.7976931348623157e+308");
    }

    #[test]
    fn converts() {
        let third = Value::Decimal(rat(-7, 3));
        let shown = |v: Result<Value, String>| format_exact(&v.unwrap(), true, &Style::ISO);
        assert_eq!(shown(third.converted(S::Int)), "-2");
        assert_eq!(
            shown(third.converted(S::String)),
            "\"-2.33333333333333333333333333333\""
        );
        assert_eq!(shown(Value::Float(0.1).converted(S::Decimal)), "0.1");
        assert_eq!(shown(Value::text("1/8").converted(S::Decimal)), "0.125");
        assert_eq!(
            shown(Value::Date(1).converted(S::DateTime)),
            "1970-01-02T00:00"
        );
        assert_eq!(shown(Value::Null.converted(S::Int)), "");
        assert!(Value::Date(1).converted(S::Int).is_err());
        assert!(
            Value::Vector(Rc::new(Vec::new()))
                .converted(S::Int)
                .is_err()
        );
    }

    #[test]
    fn division_is_exact_where_it_can_be() {
        let third = arith(BinOp::Div, &Value::int(1), &Value::int(3)).unwrap();
        assert!(matches!(third, Value::Decimal(_)));
        // A third is rounded at the widest scale there is, so taking it
        // back out again does not give one. This is the cost of a
        // fixed-point decimal in place of a rational, and is deliberate.
        let back = arith(BinOp::Mul, &third, &Value::int(3)).unwrap();
        assert_eq!(format_exact(&back, false, &Style::ISO), "1");
        // One that ends comes back exactly.
        let eighth = arith(BinOp::Div, &Value::int(1), &Value::int(8)).unwrap();
        let one = arith(BinOp::Mul, &eighth, &Value::int(8)).unwrap();
        assert!(matches!(one, Value::Int(n, _) if n == 1));
    }

    #[test]
    fn division_by_zero_is_a_num() {
        let over_zero = |n: i64| {
            let v = arith(BinOp::Div, &Value::int(n), &Value::int(0)).unwrap();
            assert!(matches!(v, Value::Float(_)));
            format_scalar(&v, false)
        };
        assert_eq!(over_zero(1), "Inf");
        assert_eq!(over_zero(-1), "-Inf");
        assert_eq!(over_zero(0), "NaN");
    }

    #[test]
    fn dates_and_times_shift() {
        let int = |n: i64| Value::int(n);
        let day = date::from_ymd(2025, 1, 31).unwrap();
        let shown = |op, a: &Value, b: &Value| format_scalar(&arith(op, a, b).unwrap(), false);
        assert_eq!(shown(BinOp::Add, &Value::Date(day), &int(1)), "2025-02-01");
        assert_eq!(
            shown(BinOp::Add, &int(-31), &Value::Date(day)),
            "2024-12-31"
        );
        assert_eq!(
            shown(BinOp::Sub, &Value::Date(day), &Value::Date(0)),
            "20119"
        );
        assert_eq!(
            shown(BinOp::Sub, &Value::Time(60 * date::NANOS), &int(120)),
            "23:59"
        );
        assert_eq!(
            shown(
                BinOp::Add,
                &Value::Date(day),
                &Value::Time(34_200 * date::NANOS)
            ),
            "2025-01-31T09:30"
        );
        let noon = arith(
            BinOp::Add,
            &Value::Date(day),
            &Value::Time(43_200 * date::NANOS),
        )
        .unwrap();
        assert_eq!(shown(BinOp::Add, &noon, &int(86_400)), "2025-02-01T12:00");
        assert_eq!(shown(BinOp::Sub, &noon, &noon), "0");
        assert!(arith(BinOp::Sub, &int(1), &Value::Date(day)).is_err());
        assert!(arith(BinOp::Mul, &Value::Date(day), &int(2)).is_err());
        assert!(arith(BinOp::Sub, &noon, &Value::Date(day)).is_err());
        assert!(arith(BinOp::Add, &Value::Date(i32::MAX), &int(1)).is_err());
    }

    #[test]
    fn num_is_contagious() {
        let v = arith(BinOp::Div, &Value::Float(1.0), &Value::int(3.into())).unwrap();
        assert!(matches!(v, Value::Float(_)));
    }
}

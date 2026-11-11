// Copyright (c) 2026 Stephen Roe

//! Runtime values and exact arithmetic.
//!
//! Exact numbers are `Int`, `Decimal`, `Fraction` or `Percent`; the last
//! three hold the same fixed-point number and differ only in how it is
//! shown. Nothing here turns an exact number into a `Float` unless the other
//! operand already is one.

use crate::zone::{Zoned, format_offset};
use omasheet_omx::ast::{BinOp, Lit};
use omasheet_omx::colfmt::{SciConv, SciFormat};
use omasheet_omx::convert;
use omasheet_omx::date;
use omasheet_omx::date::Style;
use omasheet_omx::decimal::{Dec, MAX_DENOMINATOR, SHOWN_PRECISION};
use omasheet_omx::duration;
use omasheet_omx::funcs::MathFn;
use omasheet_omx::types::{Base, QId, S};
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
    /// A value and its standard deviation, each a `Float`: `10.5(3)`,
    /// `10.5 ± 0.3`.
    Scientific(f64, f64),
    /// A number with an SI unit and, optionally, an uncertainty: `5kg`,
    /// `10.5(3)e-3kg` (add-si-units). Shared, since a unit is a list of terms.
    Quantity(Rc<siquant::Quantity>),
    /// A real and an imaginary part, each a `Float`: `3+4i`.
    Complex(f64, f64),
    /// Text, and whether it was written bare in a cell, to be read as
    /// Markdown where a file is shown as a document. Text written in quotes,
    /// or made by a formula, is as it is.
    String(Rc<str>, bool),
    Boolean(bool),
    Date(i32),
    /// Nanoseconds since midnight, as Polars holds a `Time`.
    Time(i64),
    /// Microseconds since the start of 1970-01-01 on the clocks it is
    /// shown on, as Polars holds a `Datetime`. With no zone, those are the
    /// clocks of the file's own zone; with one, the clocks of that zone,
    /// and the instant it is is in the [`Zoned`].
    Datetime(i64, Option<Rc<Zoned>>),
    /// Microseconds, as Polars holds a `Duration`: `PT1H30M`.
    Duration(i64),
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
    /// A date-time on the clocks of a zone other than the file's own.
    pub fn zoned(z: Zoned) -> Value {
        Value::Datetime(z.wall, Some(Rc::new(z)))
    }

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
            Lit::Fraction(r) => Value::Fraction(*r),
            Lit::Percent(r) => Value::Percent(*r),
            Lit::Float(f) => Value::Float(*f),
            Lit::Scientific(v, s) => Value::Scientific(*v, *s),
            Lit::Quantity(v, s, u) => crate::quantity::from_lit(*v, *s, u),
            Lit::Complex(re, im) => Value::Complex(*re, *im),
            Lit::String(s) => Value::text(s),
            Lit::Boolean(b) => Value::Boolean(*b),
            Lit::Date(d) => Value::Date(*d),
            Lit::Time(t) => Value::Time(*t),
            Lit::Datetime(t) => Value::Datetime(*t, None),
            Lit::Duration(t) => Value::Duration(*t),
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
            Value::Scientific(..) => "Scientific",
            Value::Quantity(_) => "Quantity",
            Value::Complex(..) => "Complex",
            Value::String(..) => "String",
            Value::Boolean(_) => "Boolean",
            Value::Date(_) => "Date",
            Value::Time(_) => "Time",
            Value::Datetime(_, None) | Value::Datetime(_, Some(_)) => "Datetime",
            Value::Duration(_) => "Duration",
            Value::Range { .. } => "a range",
            Value::Vector(_) => "a vector",
            Value::Sheet(_) => "a sheet",
            Value::Row(_) => "a row",
        }
    }

    /// A number with an SI unit.
    pub fn is_quantity(&self) -> bool {
        matches!(self, Value::Quantity(_))
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Value::Int(_, _)
                | Value::Decimal(_)
                | Value::Fraction(_)
                | Value::Percent(_)
                | Value::Float(_)
                | Value::Scientific(..)
                | Value::Complex(..)
        )
    }

    /// A single value as a literal. A date-time in another zone is what its
    /// clocks show. `None` for empty, an error, or more than one value.
    fn to_lit(&self) -> Option<Lit> {
        Some(match self {
            Value::Int(n, b) => Lit::Int(*n, *b),
            Value::Decimal(r) => Lit::Decimal(*r),
            Value::Fraction(r) => Lit::Fraction(*r),
            Value::Percent(r) => Lit::Percent(*r),
            Value::Float(f) => Lit::Float(*f),
            Value::Scientific(v, s) => Lit::Scientific(*v, *s),
            Value::Quantity(q) => crate::quantity::to_lit(q),
            Value::Complex(re, im) => Lit::Complex(*re, *im),
            Value::String(s, _) => Lit::String(s.to_string()),
            Value::Boolean(b) => Lit::Boolean(*b),
            Value::Date(d) => Lit::Date(*d),
            Value::Time(t) => Lit::Time(*t),
            Value::Datetime(t, None) => Lit::Datetime(*t),
            Value::Datetime(_, Some(z)) => Lit::Datetime(z.wall),
            Value::Duration(t) => Lit::Duration(*t),
            _ => return None,
        })
    }

    /// This value as the type `to`: what `Int(x)`, `Text(x)` and the rest
    /// give. Empty stays empty.
    pub fn converted(&self, to: S) -> Result<Value, String> {
        let fits = match (self, to) {
            (Value::Null, _) | (_, S::Any) => true,
            (Value::Datetime(_, Some(_)), S::Datetime) | (Value::String(..), S::String) => true,
            // A whole `Decimal` is held as an `Int`.
            (Value::Int(_, b), to) if to.is_int() => *b == Base::of(to),
            (Value::Int(_, Base::Dec), S::Decimal) => true,
            (Value::Decimal(_), S::Decimal) => true,
            (Value::Fraction(_), S::Fraction) => true,
            (Value::Percent(_), S::Percent) => true,
            (Value::Quantity(q), S::Quantity(id)) => crate::quantity::admitted(q, id),
            _ => false,
        };
        if fits {
            return Ok(self.clone());
        }
        // A `Duration` is a quantity of seconds, `PT90M.Quantity` is `5400s`,
        // and a quantity of time is a `Duration`, `5400s.Duration` is `PT1H30M`.
        match (self, to) {
            (Value::Duration(us), S::Quantity(id)) => {
                return if id == QId::ANY {
                    Ok(crate::quantity::from_duration(*us))
                } else {
                    Err(format!("a duration is not a `{}`", id.name()))
                };
            }
            (Value::Quantity(q), S::Duration) => return crate::quantity::as_duration(q),
            _ => {}
        }
        // A quantity with no dimension is a number; one with a unit is not.
        if let (Value::Quantity(q), S::Float | S::Scientific) = (self, to) {
            let (v, s) = q.to_number().map_err(|_| {
                format!(
                    "a quantity in {} is not a number; convert it with `to` to a unit \
                     with no dimension, or take its value in a unit",
                    q.unit()
                )
            })?;
            return Ok(if to == S::Scientific || s != 0.0 {
                Value::Scientific(v, s)
            } else {
                Value::Float(v)
            });
        }
        if let (Value::Quantity(q), S::Quantity(id)) = (self, to) {
            let name = id.name();
            return Err(format!("a quantity in {} is not a `{name}`", q.unit()));
        }
        // A `Fraction` is a `Decimal` shown as a fraction, so asking for one
        // is a change of display and not of value.
        if to == S::Fraction {
            return Ok(self.converted(S::Decimal)?.fraction());
        }
        // There is no uncertain complex number: it is the value alone.
        if let (Value::Scientific(v, _), S::Complex) = (self, to) {
            mark_degraded();
            return Ok(Value::Complex(*v, 0.0));
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
    Scientific(f64, f64),
    Complex(f64, f64),
}

/// What an uncertain value degrades from, when a calculation may give a
/// `Complex`: the message shown in the bottom bar.
pub const UNCERTAIN_COMPLEX: &str = "uncertain complex not supported";

thread_local! {
    static DEGRADED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether an uncertain value degraded to a `Float` since this was last
/// asked, and so the message [`UNCERTAIN_COMPLEX`] is to be shown. Asking
/// clears it.
pub fn take_degraded() -> bool {
    DEGRADED.with(|d| d.replace(false))
}

/// Note that an uncertain value degraded to a `Float`; see [`take_degraded`].
pub fn mark_degraded() {
    DEGRADED.with(|d| d.set(true));
}

fn number(v: &Value) -> Option<N> {
    Some(match v {
        Value::Int(n, _) => N::Exact(Dec::from_i64(*n)),
        Value::Decimal(r) | Value::Fraction(r) | Value::Percent(r) => N::Exact(*r),
        Value::Float(f) => N::Float(*f),
        Value::Scientific(v, s) => N::Scientific(*v, *s),
        Value::Complex(re, im) => N::Complex(*re, *im),
        _ => return None,
    })
}

/// A number's value and its uncertainty, which is zero for one that has
/// none; `None` for a `Complex`.
fn with_sigma(n: &N) -> Option<(f64, f64)> {
    Some(match n {
        N::Scientific(v, s) => (*v, *s),
        N::Complex(..) => return None,
        other => (real(other), 0.0),
    })
}

/// `+ - * / % **` of two values that carry an uncertainty, to first order
/// and with the two independent (D93, D94).
fn uncertain_arith(op: BinOp, (x, sx): (f64, f64), (y, sy): (f64, f64)) -> Value {
    // A term that is zero because its uncertainty is, whatever the slope.
    let term = |slope: f64, s: f64| if s == 0.0 { 0.0 } else { slope * s };
    let (value, sigma) = match op {
        BinOp::Add => (x + y, sx.hypot(sy)),
        BinOp::Sub => (x - y, sx.hypot(sy)),
        BinOp::Mul => (x * y, term(y, sx).hypot(term(x, sy))),
        BinOp::Div => (x / y, term(1.0 / y, sx).hypot(term(-x / (y * y), sy))),
        BinOp::Rem => {
            let q = (x / y).floor();
            (x - q * y, sx.hypot(term(q, sy)))
        }
        _ => {
            let value = x.powf(y);
            let wrt_x = term(y * x.powf(y - 1.0), sx);
            let wrt_y = term(value * x.ln(), sy);
            (value, wrt_x.hypot(wrt_y))
        }
    };
    Value::Scientific(value, sigma)
}

/// `value ± uncertainty`. Both sides are real numbers, and the uncertainty
/// is not negative.
pub fn plus_minus(a: &Value, b: &Value) -> Result<Value, String> {
    if matches!(a, Value::Null) || matches!(b, Value::Null) {
        return Ok(Value::Null);
    }
    if let Some(done) = crate::quantity::plus_minus(a, b) {
        return done;
    }
    let real_of = |v: &Value| match number(v) {
        Some(n @ (N::Exact(_) | N::Float(_))) => Some(real(&n)),
        _ => None,
    };
    let (Some(value), Some(sigma)) = (real_of(a), real_of(b)) else {
        return Err(format!(
            "`±` needs real numbers on both sides, found {} and {}",
            a.kind(),
            b.kind()
        ));
    };
    if sigma < 0.0 {
        return Err("the uncertainty after `±` cannot be negative".to_string());
    }
    Ok(Value::Scientific(value, sigma))
}

/// A function of an uncertain value, to first order (D94).
fn uncertain_math(m: MathFn, v: f64, s: f64) -> Value {
    let sigma = if s == 0.0 {
        0.0
    } else {
        (m.slope(v) * s).abs()
    };
    Value::Scientific(m.apply(v), sigma)
}

pub fn to_f64(r: &Dec) -> f64 {
    r.to_f64()
}

fn real(n: &N) -> f64 {
    match n {
        N::Exact(r) => r.to_f64(),
        N::Float(f) => *f,
        N::Scientific(v, _) => *v,
        N::Complex(re, _) => *re,
    }
}

fn complex(n: &N) -> (f64, f64) {
    match n {
        N::Complex(re, im) => (*re, *im),
        other => (real(other), 0.0),
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
    // A quantity of time beside a date, a time or a duration is a duration.
    let spanned = crate::quantity::as_span(op, a, b);
    let (a, b) = match &spanned {
        Some(Ok((x, y))) => (x, y),
        Some(Err(e)) => return Err(e.clone()),
        None => (a, b),
    };
    if let Some(done) = crate::quantity::arith(op, a, b) {
        return done;
    }
    if let Some(shifted) = shift(op, a, b) {
        return shifted;
    }
    if let Some(scaled) = scale(op, a, b) {
        return scaled;
    }
    let (Some(mut x), Some(mut y)) = (number(a), number(b)) else {
        return Err(format!(
            "cannot apply `{}` to {} and {}",
            op.symbol(),
            a.kind(),
            b.kind()
        ));
    };
    // An uncertain value beside a `Complex` is a `Float`: there is no
    // uncertain complex number, and the bottom bar says so.
    if matches!(x, N::Complex(..)) || matches!(y, N::Complex(..)) {
        for n in [&mut x, &mut y] {
            if let N::Scientific(v, _) = *n {
                *n = N::Float(v);
                mark_degraded();
            }
        }
    }
    // Whole numbers of one base give that base, but for a quotient, which
    // need not be whole; any other mix gives a decimal `Int`.
    let base = match (a, b) {
        (Value::Int(_, p), Value::Int(_, q)) if p == q && op != BinOp::Div => *p,
        _ => Base::Dec,
    };
    // A power is exact only for a whole exponent: any other is worked out
    // in floating point, as the type checker says.
    let floating = op == BinOp::Pow && !matches!(b, Value::Int(..));
    match (&x, &y) {
        // Two of one kind give that kind, so two fractions give a fraction and
        // two percentages a percentage; any other mix is the plain number.
        (N::Exact(x), N::Exact(y)) if !floating => exact_arith(op, x, y).map(|v| match (a, b) {
            (Value::Fraction(_), Value::Fraction(_)) => v.fraction(),
            (Value::Percent(_), Value::Percent(_)) => v.percent(),
            _ => match v {
                Value::Int(n, _) => Value::Int(n, base),
                v => v,
            },
        }),
        (N::Scientific(..), _) | (_, N::Scientific(..)) => match (with_sigma(&x), with_sigma(&y)) {
            (Some(p), Some(q)) => Ok(uncertain_arith(op, p, q)),
            _ => Err("uncertain complex not supported".to_string()),
        },
        (N::Complex(..), _) | (_, N::Complex(..)) => {
            let ((a, b), (c, d)) = (complex(&x), complex(&y));
            let (re, im) = match op {
                BinOp::Add => (a + c, b + d),
                BinOp::Sub => (a - c, b - d),
                BinOp::Mul => (a * c - b * d, a * d + b * c),
                BinOp::Div => {
                    let m = c * c + d * d;
                    ((a * c + b * d) / m, (b * c - a * d) / m)
                }
                // A whole exponent is repeated multiplication, as in Raku, so
                // that `(1+1i) ** 2` is `0+2i` and not a rounding of it.
                _ if d == 0.0 && c.fract() == 0.0 && c.abs() <= 1024.0 => {
                    let (mut base, mut n) = ((a, b), c.abs() as u32);
                    let mut out = (1.0, 0.0);
                    let times = |p: (f64, f64), q: (f64, f64)| {
                        (p.0 * q.0 - p.1 * q.1, p.0 * q.1 + p.1 * q.0)
                    };
                    while n > 0 {
                        if n & 1 == 1 {
                            out = times(out, base);
                        }
                        base = times(base, base);
                        n >>= 1;
                    }
                    if c < 0.0 {
                        let m = out.0 * out.0 + out.1 * out.1;
                        (out.0 / m, -out.1 / m)
                    } else {
                        out
                    }
                }
                _ => {
                    // z ** w = exp(w * ln z)
                    if a == 0.0 && b == 0.0 {
                        (0.0, 0.0)
                    } else {
                        let (ln_r, theta) = ((a * a + b * b).sqrt().ln(), b.atan2(a));
                        let (re, im) = (c * ln_r - d * theta, d * ln_r + c * theta);
                        (re.exp() * im.cos(), re.exp() * im.sin())
                    }
                }
            };
            Ok(Value::Complex(re, im))
        }
        _ => {
            let (x, y) = (real(&x), real(&y));
            // A negative base to a fractional power is complex (D212).
            if op == BinOp::Pow && x < 0.0 && y.is_finite() && y.fract() != 0.0 {
                return arith(op, &Value::Complex(x, 0.0), &Value::Complex(y, 0.0));
            }
            Ok(Value::Float(match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                BinOp::Mul => x * y,
                BinOp::Div => x / y,
                BinOp::Rem => x - (x / y).floor() * y,
                _ => x.powf(y),
            }))
        }
    }
}

/// `+` and `-` where a side is a date, a time or a duration: a whole number
/// counts days next to a `Date` and seconds next to a `Time` or `Datetime`,
/// and two of a kind differ by a `Duration`. `None` if neither side is one,
/// or the two do not combine.
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
                Ok(Value::Datetime(t, None))
            }
            _ => Err("the date-time is out of range".to_string()),
        }
    };
    let zoned = |z: &Zoned, n: &i64| match signed(*n).and_then(|n| z.shifted(n)) {
        Some(z) => Ok(Value::zoned(z)),
        None => Err("the date-time is out of range".to_string()),
    };
    // A time of day wraps round midnight, and a whole number beside one is
    // that many seconds.
    let time = |t: i64, n: &i64| {
        let n = (n % date::DAY) * date::NANOS;
        let n = if minus { -n } else { n };
        Ok(Value::Time((t + n).rem_euclid(date::DAY_NANOS)))
    };
    // A `Duration` beside a date is whole days of it, and beside a time of
    // day wraps round midnight as a whole number does.
    let by = |n: &i64| signed(*n).ok_or_else(|| "the duration is out of range".to_string());
    let on_date = |d: i32, n: &i64| {
        let days = by(n)?.div_euclid(date::DAY_MICROS);
        match i32::try_from(d as i64 + days) {
            Ok(d) => Ok(Value::Date(d)),
            Err(_) => Err("the date is out of range".to_string()),
        }
    };
    let on_datetime = |t: i64, n: &i64| match t.checked_add(by(n)?) {
        Some(t) if i32::try_from(t.div_euclid(date::DAY_MICROS)).is_ok() => {
            Ok(Value::Datetime(t, None))
        }
        _ => Err("the date-time is out of range".to_string()),
    };
    let on_zoned = |z: &Zoned, n: &i64| match z.moved(by(n)?) {
        Some(z) => Ok(Value::zoned(z)),
        None => Err("the date-time is out of range".to_string()),
    };
    let on_time = |t: i64, n: &i64| {
        let n = by(n)?.rem_euclid(date::DAY_MICROS) * date::NANOS_PER_MICRO;
        Ok(Value::Time((t + n).rem_euclid(date::DAY_NANOS)))
    };
    let span = |n: Option<i64>| match n {
        Some(n) => Ok(Value::Duration(n)),
        None => Err("the duration is out of range".to_string()),
    };
    Some(match (a, b) {
        (Value::Date(d), Value::Int(n, _)) => date(*d, n),
        (Value::Datetime(t, None), Value::Int(n, _)) => datetime(*t, n),
        (Value::Datetime(_, Some(z)), Value::Int(n, _)) => zoned(z, n),
        (Value::Time(t), Value::Int(n, _)) => time(*t, n),
        (Value::Int(n, _), Value::Date(d)) if !minus => date(*d, n),
        (Value::Int(n, _), Value::Datetime(t, None)) if !minus => datetime(*t, n),
        (Value::Int(n, _), Value::Datetime(_, Some(z))) if !minus => zoned(z, n),
        (Value::Int(n, _), Value::Time(t)) if !minus => time(*t, n),
        (Value::Date(d), Value::Time(t)) | (Value::Time(t), Value::Date(d)) if !minus => {
            Ok(Value::Datetime(
                *d as i64 * date::DAY_MICROS + t / date::NANOS_PER_MICRO,
                None,
            ))
        }
        (Value::Date(d), Value::Duration(n)) => on_date(*d, n),
        (Value::Datetime(t, None), Value::Duration(n)) => on_datetime(*t, n),
        (Value::Datetime(_, Some(z)), Value::Duration(n)) => on_zoned(z, n),
        (Value::Time(t), Value::Duration(n)) => on_time(*t, n),
        (Value::Duration(n), Value::Date(d)) if !minus => on_date(*d, n),
        (Value::Duration(n), Value::Datetime(t, None)) if !minus => on_datetime(*t, n),
        (Value::Duration(n), Value::Datetime(_, Some(z))) if !minus => on_zoned(z, n),
        (Value::Duration(n), Value::Time(t)) if !minus => on_time(*t, n),
        (Value::Duration(x), Value::Duration(y)) => span(signed(*y).and_then(|y| x.checked_add(y))),
        // A difference is a `Duration`, in microseconds whatever the two are
        // held in.
        (Value::Date(x), Value::Date(y)) if minus => {
            span((*x as i64 - *y as i64).checked_mul(date::DAY_MICROS))
        }
        (Value::Time(x), Value::Time(y)) if minus => span(Some((x - y) / date::NANOS_PER_MICRO)),
        (Value::Datetime(x, None), Value::Datetime(y, None)) if minus => span(x.checked_sub(*y)),
        (Value::Datetime(_, Some(x)), Value::Datetime(_, Some(y))) if minus => {
            span(x.utc.checked_sub(y.utc))
        }
        (Value::Datetime(x, None), Value::Datetime(_, Some(y))) if minus => {
            span(x.checked_sub(y.home_wall()))
        }
        (Value::Datetime(_, Some(x)), Value::Datetime(y, None)) if minus => {
            span(x.home_wall().checked_sub(*y))
        }
        _ => return None,
    })
}

/// `*` and `/` of a `Duration` by a number, to the nearest microsecond.
/// `None` if neither side is a duration, or the other is not a number.
fn scale(op: BinOp, a: &Value, b: &Value) -> Option<Result<Value, String>> {
    let (us, n) = match (op, a, b) {
        (BinOp::Mul | BinOp::Div, Value::Duration(us), n) => (*us, number(n)?),
        (BinOp::Mul, n, Value::Duration(us)) => (*us, number(n)?),
        _ => return None,
    };
    let out = match n {
        N::Exact(r) => {
            let us = Dec::from_i64(us);
            match op {
                BinOp::Div if r.is_zero() => {
                    return Some(Err("cannot divide a duration by zero".to_string()));
                }
                BinOp::Div => us.checked_div(r),
                _ => us.checked_mul(r),
            }
            .and_then(|d| d.round().to_i64())
        }
        N::Float(f) => {
            let out = if op == BinOp::Div {
                us as f64 / f
            } else {
                us as f64 * f
            };
            let fits = out.is_finite() && out.abs() < i64::MAX as f64;
            fits.then(|| out.round() as i64)
        }
        N::Scientific(f, _) => {
            let out = if op == BinOp::Div {
                us as f64 / f
            } else {
                us as f64 * f
            };
            let fits = out.is_finite() && out.abs() < i64::MAX as f64;
            fits.then(|| out.round() as i64)
        }
        N::Complex(..) => return Some(Err("a duration cannot be scaled by a Complex".to_string())),
    };
    Some(
        out.map(Value::Duration)
            .ok_or_else(|| "the duration is out of range".to_string()),
    )
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
        // What is left of a division that goes by whole steps down, so that
        // the result has the sign of the divisor: no exact number is a
        // remainder by zero, which is `NaN`.
        BinOp::Rem => {
            if y.is_zero() {
                return Ok(Value::Float(f64::NAN));
            }
            x.checked_div(y)
                .map(Dec::floor)
                .and_then(|q| q.checked_mul(y))
                .and_then(|m| x.checked_sub(m))
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
    if let Value::Quantity(q) = v {
        return crate::quantity::math(m, q);
    }
    let whole = |r: Dec| Value::exact(r);
    // These cast to a `Float` as `x.Float` does. A `Complex` is kept by
    // those that take one.
    if m.casts_to_float() {
        let cast = match v {
            Value::Boolean(_) | Value::String(..) => true,
            Value::Complex(..) => m.of_complex().is_none(),
            _ => false,
        };
        if cast {
            let x = v
                .converted(S::Float)
                .map_err(|_| format!("`{}` cannot be applied to {}", m.name(), v.kind()))?;
            return math(m, &x);
        }
    }
    let x = match (v, m) {
        (Value::Null, _) => return Ok(Value::Null),
        (Value::Percent(r), MathFn::Abs) => {
            return r
                .checked_abs()
                .map(Value::Percent)
                .ok_or_else(|| TOO_LARGE.to_string());
        }
        (Value::Percent(_), MathFn::Re | MathFn::Conj) => return Ok(v.clone()),
        (Value::Percent(_), _) => return math(m, &v.clone().plain()),
        (Value::Int(..) | Value::Decimal(_) | Value::Fraction(_), MathFn::Im) => {
            return Ok(Value::int(0));
        }
        (Value::Decimal(_) | Value::Fraction(_), MathFn::Re | MathFn::Conj) => {
            return Ok(v.clone());
        }
        (Value::Scientific(v, s), _) => return Ok(uncertain_math(m, *v, *s)),
        (Value::Complex(re, im), _) => return complex_math(m, *re, *im),
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
    // Outside the function's real domain the value is the principal complex
    // one: `sqrt(-4)` is `0+2i`. On its edge it is real, as `ln(0)` is `-Inf`.
    if m.leaves_domain(x) {
        return complex_math(m, x, 0.0);
    }
    Ok(Value::Float(m.apply(x)))
}

type C = (f64, f64);

fn c_mul(p: C, q: C) -> C {
    (p.0 * q.0 - p.1 * q.1, p.0 * q.1 + p.1 * q.0)
}

fn c_ln(z: C) -> C {
    (z.0.hypot(z.1).ln(), z.1.atan2(z.0))
}

/// The root with a real part that is not negative, the cut on the negative
/// real axis taken from the sign of the imaginary part, `-0` included.
fn c_sqrt(z: C) -> C {
    let r = z.0.hypot(z.1);
    let half = ((r - z.0) / 2.0).sqrt();
    (
        ((r + z.0) / 2.0).sqrt(),
        if z.1.is_sign_negative() { -half } else { half },
    )
}

/// `asinh z = ln(z + sqrt(z + i) sqrt(z - i))`. It is odd, so a negative real
/// part is worked on as its negation, which keeps the sum from cancelling.
fn c_asinh(z: C) -> C {
    if z.0.is_sign_negative() {
        let (re, im) = c_asinh((-z.0, -z.1));
        return (-re, -im);
    }
    let root = c_mul(c_sqrt((z.0, z.1 + 1.0)), c_sqrt((z.0, z.1 - 1.0)));
    c_ln((z.0 + root.0, z.1 + root.1))
}

/// `acosh z = ln(z + sqrt(z + 1) sqrt(z - 1))`.
fn c_acosh(z: C) -> C {
    // On the real segment inside (-1, 1) the real part is exactly zero, which
    // the logarithm of a modulus that is only nearly 1 would not give.
    if z.1 == 0.0 && z.0.abs() <= 1.0 {
        let angle = z.0.acos();
        return (
            0.0,
            if z.1.is_sign_negative() {
                -angle
            } else {
                angle
            },
        );
    }
    let root = c_mul(c_sqrt((z.0 + 1.0, z.1)), c_sqrt((z.0 - 1.0, z.1)));
    c_ln((z.0 + root.0, z.1 + root.1))
}

/// `atanh z = (ln(1 + z) - ln(1 - z)) / 2`.
fn c_atanh(z: C) -> C {
    let (a, b) = (c_ln((1.0 + z.0, z.1)), c_ln((1.0 - z.0, -z.1)));
    ((a.0 - b.0) / 2.0, (a.1 - b.1) / 2.0)
}

fn c_tan(z: C) -> C {
    let (x, y) = (2.0 * z.0, 2.0 * z.1);
    // `sinh y / cosh y` is `+-1` long before `cosh` overflows.
    let im = if y.abs() > 40.0 {
        y.signum()
    } else {
        y.sinh() / (x.cos() + y.cosh())
    };
    (x.sin() / (x.cos() + y.cosh()), im)
}

fn complex_math(m: MathFn, re: f64, im: f64) -> Result<Value, String> {
    let z = (re, im);
    let r = re.hypot(im);
    let ln10 = std::f64::consts::LN_10;
    let ln2 = std::f64::consts::LN_2;
    let half_pi = std::f64::consts::FRAC_PI_2;
    // `asin z = -i asinh(i z)`.
    let asin = |z: C| {
        let (a, b) = c_asinh((-z.1, z.0));
        (b, -a)
    };
    let c = match m {
        MathFn::Abs => return Ok(Value::Float(r)),
        MathFn::Re => return Ok(Value::Float(re)),
        MathFn::Im => return Ok(Value::Float(im)),
        MathFn::Arg => return Ok(Value::Float(im.atan2(re))),
        MathFn::Conj => (re, -im),
        MathFn::Sign if r == 0.0 => (0.0, 0.0),
        MathFn::Sign => (re / r, im / r),
        MathFn::Sqrt => c_sqrt(z),
        MathFn::Exp => {
            let e = re.exp();
            (e * im.cos(), e * im.sin())
        }
        MathFn::Ln => c_ln(z),
        MathFn::Log10 => {
            let l = c_ln(z);
            (l.0 / ln10, l.1 / ln10)
        }
        MathFn::Log2 => {
            let l = c_ln(z);
            (l.0 / ln2, l.1 / ln2)
        }
        MathFn::Sin => (re.sin() * im.cosh(), re.cos() * im.sinh()),
        MathFn::Cos => (re.cos() * im.cosh(), -(re.sin() * im.sinh())),
        MathFn::Tan => c_tan(z),
        MathFn::Sinh => (re.sinh() * im.cos(), re.cosh() * im.sin()),
        MathFn::Cosh => (re.cosh() * im.cos(), re.sinh() * im.sin()),
        // `tanh z = -i tan(i z)`.
        MathFn::Tanh => {
            let (a, b) = c_tan((-im, re));
            (b, -a)
        }
        MathFn::Arcsin => asin(z),
        MathFn::Arccos => {
            let s = asin(z);
            (half_pi - s.0, -s.1)
        }
        // `atan z = -i atanh(i z)`.
        MathFn::Arctan => {
            let (a, b) = c_atanh((-im, re));
            (b, -a)
        }
        MathFn::Arcsinh => c_asinh(z),
        MathFn::Arccosh => c_acosh(z),
        MathFn::Arctanh => c_atanh(z),
        MathFn::Round | MathFn::Floor | MathFn::Ceil | MathFn::Radians | MathFn::Degrees => {
            return Err(format!("`{}` is not defined for Complex", m.name()));
        }
    };
    Ok(Value::Complex(c.0, c.1))
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
        Value::Scientific(v, s) => {
            let factor = 10f64.powi(places.min(17));
            Value::Scientific((v * factor).round() / factor, *s)
        }
        other => return Err(format!("cannot round {}", other.kind())),
    })
}

/// Whether a value is `NaN`, which is ordered greatest and equal to itself.
pub fn is_nan(v: &Value) -> bool {
    matches!(v, Value::Float(f) if f.is_nan())
}

/// Order two single values. `None` if either is empty. Two numbers, one of
/// them uncertain, are equal when they differ by no more than their
/// uncertainties together (in quadrature), so `==` and `<=` hold for them.
pub fn compare(a: &Value, b: &Value) -> Result<Option<Ordering>, String> {
    if let Some(done) = crate::quantity::compare(a, b, true) {
        return done;
    }
    if let (Some(x), Some(y)) = (number(a), number(b))
        && (matches!(x, N::Scientific(..)) || matches!(y, N::Scientific(..)))
        && let (Some((vx, sx)), Some((vy, sy))) = (with_sigma(&x), with_sigma(&y))
        && !vx.is_nan()
        && !vy.is_nan()
        && (vx - vy).abs() <= sx.hypot(sy)
    {
        return Ok(Some(Ordering::Equal));
    }
    compare_exact(a, b)
}

/// Order two single values by their values alone, ignoring any uncertainty.
fn compare_exact(a: &Value, b: &Value) -> Result<Option<Ordering>, String> {
    if let Some(done) = crate::quantity::compare(a, b, false) {
        return done;
    }
    Ok(Some(match (a, b) {
        (Value::Null, _) | (_, Value::Null) => return Ok(None),
        (Value::String(x, _), Value::String(y, _)) => x.cmp(y),
        (Value::Date(x), Value::Date(y)) => x.cmp(y),
        (Value::Time(x), Value::Time(y)) => x.cmp(y),
        (Value::Datetime(x, None), Value::Datetime(y, None)) => x.cmp(y),
        (Value::Duration(x), Value::Duration(y)) => x.cmp(y),
        // Across time zones it is the instant that counts.
        (Value::Datetime(_, Some(x)), Value::Datetime(_, Some(y))) => x.utc.cmp(&y.utc),
        (Value::Datetime(x, None), Value::Datetime(_, Some(y))) => x.cmp(&y.home_wall()),
        (Value::Datetime(_, Some(x)), Value::Datetime(y, None)) => x.home_wall().cmp(y),
        (Value::Boolean(x), Value::Boolean(y)) => x.cmp(y),
        _ => match (number(a), number(b)) {
            (Some(N::Exact(x)), Some(N::Exact(y))) => x.cmp(&y),
            (Some(N::Complex(..)), _) | (_, Some(N::Complex(..))) => {
                return Err("Complex values have no order".into());
            }
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
        _ => compare_exact(a, b)?.unwrap_or(Ordering::Equal),
    })
}

/// Equality of two single values. `None` if either is empty.
pub fn equal(a: &Value, b: &Value) -> Result<Option<bool>, String> {
    // A `Complex` has no order but is equal to another when both of its
    // parts are, exactly (D202).
    if let (Some(x), Some(y)) = (number(a), number(b))
        && (matches!(x, N::Complex(..)) || matches!(y, N::Complex(..)))
    {
        return Ok(Some(complex(&x) == complex(&y)));
    }
    Ok(compare(a, b)?.map(|o| o == Ordering::Equal))
}

/// The most characters a number is shown in, its sign aside.
pub const WIDTH: usize = 23;

/// What marks a number shown with fewer digits than it has: a middle dot,
/// which is quieter on the page than the three dots of an ellipsis.
pub const MORE: char = '·';

/// How many digits after the point a `Decimal`, a `Float` or the parts of a
/// `Complex` are shown to: `0.333·`. A number with fewer has them all, and one
/// below a thousandth keeps the digits it has, so as not to show as nought.
pub const SHOWN_DIGITS: usize = 3;

/// A plain positive number rounded to [`SHOWN_DIGITS`] places after its point and
/// marked with [`MORE`]; `None` where it has no more than that, or is too small
/// for them to say anything.
fn cut_places(plain: &str) -> Option<String> {
    let (whole, fraction) = plain.split_once('.')?;
    let small = whole == "0" && fraction.chars().take(SHOWN_DIGITS).all(|c| c == '0');
    if fraction.len() <= SHOWN_DIGITS || small {
        return None;
    }
    // Rounded to the nearest, a half upwards, on the digits themselves: a
    // trip that comes back as 0.9999999999999999 reads as `1.000·`.
    let mut digits: Vec<u8> = whole
        .bytes()
        .chain(fraction.bytes().take(SHOWN_DIGITS))
        .collect();
    if fraction.as_bytes()[SHOWN_DIGITS] >= b'5' {
        let mut k = digits.len();
        loop {
            if k == 0 {
                digits.insert(0, b'1');
                break;
            }
            k -= 1;
            if digits[k] == b'9' {
                digits[k] = b'0';
            } else {
                digits[k] += 1;
                break;
            }
        }
    }
    let point = digits.len() - SHOWN_DIGITS;
    // A carry into a sixteenth digit is a number for e notation, not for this.
    if point > 15 {
        return None;
    }
    let text = String::from_utf8(digits).ok()?;
    Some(format!("{}.{}{MORE}", &text[..point], &text[point..]))
}

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
    let full = r.to_plain_string();
    if medium && let Some(cut) = cut_places(&full) {
        return grouped(cut, sep);
    }
    let plain = grouped(full, sep);
    if medium && width(&plain) <= WIDTH {
        return plain;
    }
    // A number of ordinary size that will not fit reads better cut and
    // marked, `0.33333333333333333333·`, than in e notation. Only the
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
    (medium && (width(&plain) > WIDTH || cut_places(&plain).is_some())).then_some(plain)
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
    float_to(f, exact, sep, true)
}

/// As [`shown_float`], to [`SHOWN_DIGITS`] places where `cut`.
fn float_to(f: f64, exact: bool, sep: Option<char>, cut: bool) -> String {
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
    let full = format!("{size}");
    let plain = match cut.then(|| cut_places(&full)).flatten() {
        Some(short) => grouped(short, sep),
        None => grouped(full, sep),
    };
    if !exact && (size == 0.0 || ((SMALL..LARGE).contains(&size) && width(&plain) <= WIDTH)) {
        return signed(f < 0.0, plain);
    }
    let scientific = format!("{f:e}");
    let (mantissa, e) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let mut e: i64 = e.parse().unwrap_or(0);
    let (sign, size) = match mantissa.strip_prefix('-') {
        Some(size) => ("-", size),
        None => ("", mantissa),
    };
    // The mantissa is cut like any other number, to three places; where
    // rounding carries it to ten it is one of the next power.
    let mantissa = match (cut && !exact).then(|| cut_places(size)).flatten() {
        Some(short) if short.starts_with("10.") => {
            e += 1;
            format!("1.{}{MORE}", "0".repeat(SHOWN_DIGITS))
        }
        Some(short) => short,
        None => size.to_string(),
    };
    format!("{sign}{mantissa}{}", exponent(e))
}

/// Shortened to `room` characters and marked with [`MORE`]: a number in e
/// notation in its mantissa, and any other at its end.
fn cut(text: String, room: usize) -> String {
    if width(&text) <= room {
        return text;
    }
    let first = |text: &str, n: usize| text.chars().take(n).collect::<String>();
    match text.split_once('e') {
        Some((mantissa, exp)) => {
            let keep = room.saturating_sub(exp.len() + 2).max(1);
            format!("{}{MORE}e{exp}", first(mantissa, keep))
        }
        None => format!("{}{MORE}", first(&text, room - 1)),
    }
}

/// A `Complex` as its parts are written: `3+4i`, `1.5-2i`. Shown, where that
/// is wider than [`WIDTH`], both parts are cut alike until it fits; as for
/// any number, a sign is not counted, and that of the imaginary part is
/// what joins the two.
fn format_complex(re: f64, im: f64, exact: bool, sep: Option<char>) -> String {
    let unsigned = |text: String| match text.strip_prefix('-') {
        Some(size) => ("-", size.to_string()),
        None => ("", text),
    };
    let (re_sign, mut re_text) = unsigned(shown_float(re, exact, sep));
    let (minus, mut im_text) = unsigned(shown_float(im, exact, sep));
    let plus = if minus.is_empty() { "+" } else { minus };
    // As Raku does, to keep the `i` apart from `Inf` and `NaN`.
    let i = if im.is_finite() { "i" } else { "\\i" };
    let room = WIDTH - 1 - i.len();
    let (re_size, im_size) = (width(&re_text), width(&im_text));
    if !exact && re_size + im_size > room {
        let half = room / 2;
        let (re_width, im_width) = if re_size <= half {
            (re_size, room - re_size)
        } else if im_size <= half {
            (room - im_size, im_size)
        } else {
            (half, half)
        };
        re_text = cut(re_text, re_width);
        im_text = cut(im_text, im_width);
    }
    // One mark for the whole number, after its `i`, where either part is cut.
    let cut = re_text.contains(MORE) || im_text.contains(MORE);
    let (re_text, im_text) = (re_text.replace(MORE, ""), im_text.replace(MORE, ""));
    let more = if cut { MORE.to_string() } else { String::new() };
    format!("{re_sign}{re_text}{plus}{im_text}{i}{more}")
}

/// A scientific value in the concise form of the physical tables,
/// `7.2973525643(11)e-3`: the uncertainty to two significant digits, in
/// brackets after the value cut at the same place. A value with no
/// uncertainty is a plain number, and a value or uncertainty that is not
/// finite is shown as it is (D96).
///
/// A column's format may ask for more (D100), in the terms of a number
/// format: `±` for `11.000 ± 0.082`; `.4f`, a fixed number of places, which
/// puts the uncertainty at that place whatever it is, so that a column's
/// brackets stand in line; `e`, an exponent of its own, or `e-3`, one for
/// every number, so that they are all of one size; and a sign, a width and
/// grouping, as for a `Float`. Asked for a conversion, a value with no
/// uncertainty is shown in the same form, as `11.0000(0)`.
pub fn format_uncertain(v: f64, s: f64, f: &SciFormat, sep: Option<char>) -> String {
    let fixed = f.places.is_some() || f.exponent.is_some() || f.conv != SciConv::General;
    if s == 0.0 && !fixed {
        return padded_to(
            f,
            String::new(),
            float_to(v, false, sep, false),
            String::new(),
        );
    }
    if !v.is_finite() || !s.is_finite() {
        let text = format!(
            "{} ± {}",
            float_to(v, false, sep, false),
            float_to(s, false, sep, false)
        );
        return padded_to(f, String::new(), text, String::new());
    }
    let s = s.abs();
    // Two significant digits of the uncertainty, and the place of the last.
    let (place, digits) = if s == 0.0 {
        (0, 0)
    } else {
        let mut place = s.log10().floor() as i32 - 1;
        let mut digits = (s / 10f64.powi(place)).round();
        if digits >= 100.0 {
            digits /= 10.0;
            place += 1;
        }
        (place, digits as i64)
    };
    let magnitude = if v != 0.0 {
        v.abs().log10().floor() as i32
    } else if s != 0.0 {
        s.log10().floor() as i32
    } else {
        0
    };
    // `f` is plain; `e` has an exponent, its own unless one is asked for;
    // and `g` is plain where the last digit is a decimal place and the number
    // is of a size to read, and otherwise a mantissa and an exponent.
    let exponent = match f.conv {
        SciConv::Fixed => 0,
        SciConv::Exp => f.exponent.unwrap_or(magnitude),
        SciConv::General if place <= 0 && (-2..15).contains(&magnitude) => 0,
        SciConv::General => magnitude,
    };
    let unit = 10f64.powi(exponent);
    let (mantissa, error) = (v / unit, s / unit);
    let derived = exponent - place;
    let decimals = match f.places {
        Some(n) => n as usize,
        // With no uncertainty to set the place, every digit the number has.
        None if s == 0.0 => format!("{mantissa}")
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len().min(17)),
        None => derived.max(0) as usize,
    };
    let k = 10f64.powi(decimals as i32);
    let mantissa = ((mantissa * k).round() / k).abs();
    let tail = if exponent == 0 && f.conv != SciConv::Exp {
        String::new()
    } else if f.upper {
        format!("E{exponent}")
    } else {
        format!("e{exponent}")
    };
    let thousands = |text: String| match sep.filter(|_| f.group) {
        Some(sep) => grouped_whole(&text, sep),
        None => text,
    };
    let sign = if v < 0.0 {
        "-"
    } else if f.plus {
        "+"
    } else if f.space {
        " "
    } else {
        ""
    };
    let number = thousands(format!("{mantissa:.decimals$}"));
    if f.plus_minus {
        let error = thousands(format!("{:.decimals$}", (error * k).round() / k));
        return if tail.is_empty() {
            padded_to(f, sign.into(), format!("{number} ± {error}"), String::new())
        } else {
            let head = format!("({sign}");
            padded_to(f, head, number, format!(" ± {error}){tail}"))
        };
    }
    // The two digits where the place is the uncertainty's own, and the
    // uncertainty at the place asked for where it is not.
    let digits = if f.places.is_none() && derived >= 0 {
        digits
    } else {
        (error * k).round() as i64
    };
    padded_to(f, sign.into(), number, format!("({digits}){tail}"))
}

/// A number's `head` (its sign), its `number` and the `rest` after it, made
/// up to the format's width: with noughts between the sign and the number
/// under `0`, and otherwise with spaces, before it or, under `-`, after it.
fn padded_to(f: &SciFormat, head: String, number: String, rest: String) -> String {
    let len = head.chars().count() + number.chars().count() + rest.chars().count();
    let width = f.width.unwrap_or(0);
    if len >= width {
        return head + &number + &rest;
    }
    let fill = width - len;
    if f.zero && !f.left {
        format!("{head}{}{number}{rest}", "0".repeat(fill))
    } else if f.left {
        format!("{head}{number}{rest}{}", " ".repeat(fill))
    } else {
        format!("{}{head}{number}{rest}", " ".repeat(fill))
    }
}

/// The digits before the point of `text` grouped in threes by `sep`.
fn grouped_whole(text: &str, sep: char) -> String {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (whole, rest) = text.split_at(end);
    let mut out = String::new();
    for (k, digit) in whole.chars().enumerate() {
        if k > 0 && (whole.len() - k) % 3 == 0 {
            out.push(sep);
        }
        out.push(digit);
    }
    out + rest
}

/// A scientific value as `style` shows a number: its point the style's own.
pub fn format_scientific(v: f64, s: f64, f: &SciFormat, style: &Style) -> String {
    localised(format_uncertain(v, s, f, group(style)), style)
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
/// where it is shown as `1.41421·`. An exact number is always in full.
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
        // In full, a file writes both parts: `10.5 ± 0.3`.
        Value::Scientific(v, s) if exact => {
            format!("{} ± {}", format_float(*v, true), format_float(*s, true))
        }
        Value::Scientific(v, s) => format_scientific(*v, *s, &SciFormat::default(), style),
        Value::Quantity(q) => crate::quantity::format(q, &SciFormat::default(), style, exact),
        Value::Complex(re, im) if exact => format_complex(*re, *im, true, None),
        Value::Complex(re, im) => localised(format_complex(*re, *im, false, group(style)), style),
        Value::String(s, _) if quoted => omasheet_omx::lexer::quoted(s),
        Value::String(s, _) => s.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Date(d) => style.date(*d),
        Value::Time(t) => style.time(*t),
        Value::Datetime(t, None) => style.datetime(*t),
        Value::Duration(t) => duration::format(*t),
        // `2025-01-31T09:30+09:00`, to tell it from the file's own zone.
        Value::Datetime(_, Some(z)) => format!(
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
        assert_eq!(shown(1, 10_001), "0.00009999000099990000·");
        // Two fractions give a fraction; a fraction with a decimal, a decimal.
        let sum = arith(
            BinOp::Add,
            &Value::Fraction(rat(1, 3)),
            &Value::Fraction(rat(1, 6)),
        )
        .expect("adds");
        assert_eq!(format_scalar(&sum, false), "1/2");
        let mixed = arith(
            BinOp::Add,
            &Value::Fraction(rat(1, 3)),
            &Value::Decimal(rat(1, 6)),
        )
        .expect("adds");
        assert_eq!(format_scalar(&mixed, false), "0.5");
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
            "1.8446744073709551·e+19"
        );
        assert_eq!(
            shown(dec("-18446744073709551616")),
            "-1.8446744073709551·e+19"
        );
        assert_eq!(shown(dec("0.125")), "0.125");
        assert_eq!(shown(dec("0.000000000000001")), "0.000000000000001");
        // The widest and the narrowest a 38-digit decimal reaches.
        assert_eq!(shown(dec("1e-38")), "1e-38");
        assert_eq!(shown(dec(&"9".repeat(38))), "9.9999999999999999·e+37");
        assert_eq!(shown(dec("0.12345678901234567890123")), "0.123·");
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
            "12,345.679·"
        );
        assert_eq!(
            shown(Value::Decimal(
                Dec::parse("999999999999999.9999999").expect("parses")
            )),
            "999,999,999,999,999.99·"
        );
        assert_eq!(
            shown(Value::Percent(rat(12_345_678, 100_000))),
            "12,345.68%"
        );
        assert_eq!(shown(Value::Complex(12345.0, -1.0)), "12,345-1i");
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
        let complex = |re, im| format_scalar(&Value::Complex(re, im), false);
        assert_eq!(complex(3.0, 4.0), "3+4i");
        assert_eq!(complex(1.5, -2.0), "1.5-2i");
        assert_eq!(complex(1e20, 1e-7), "1e+20+0.0000001i");
        assert_eq!(complex(1e20, 1e-20), "1e+20+1e-20i");
        assert_eq!(complex(0.0, f64::INFINITY), "0+Inf\\i");
        assert_eq!(complex(1.0 / 3.0, 1.0), "0.333+1i·");
        assert_eq!(complex(1.0 / 3.0, -1.0 / 3.0), "0.333-0.333i·");
        assert_eq!(
            complex(1.7976931348623157e308, 1.7976931348623157e308),
            "1.79e+308+1.79e+308i·"
        );
        // Shown, a `Num` is cut to three places and marked: plain from `1e-15` to `1e15`
        // where that fits in 23 characters, and in e notation otherwise.
        let shown = |f| format_float(f, false);
        // In e notation the mantissa is cut to three places too, and a carry
        // to ten is one of the next power.
        assert_eq!(shown(1.2246467991473532e-16), "1.225·e-16");
        assert_eq!(shown(-9.9996e-20), "-1.000·e-19");
        assert_eq!(shown(1.5e30), "1.5e+30");
        assert_eq!(
            format_scalar(&Value::Complex(1.2246467991473532e-16, 2.0), false),
            "1.225e-16+2i·"
        );
        assert_eq!(shown(1.5), "1.5");
        assert_eq!(shown(0.1 + 0.2), "0.300·");
        assert_eq!(shown(-2.0 / 3.0), "-0.667·");
        assert_eq!(shown(1e-15), "0.000000000000001");
        assert_eq!(shown(9.999999999999999e-16), "1.000·e-15");
        assert_eq!(shown(1.2345678e-15), "1.235·e-15");
        assert_eq!(shown(999_999_999_999_999.9), "999999999999999.9");
        assert_eq!(shown(1e15), "1e+15");
        assert_eq!(shown(f64::MAX), "1.798·e+308");
    }

    #[test]
    fn converts() {
        let third = Value::Decimal(rat(-7, 3));
        let shown = |v: Result<Value, String>| format_exact(&v.unwrap(), true, &Style::ISO);
        assert_eq!(shown(third.converted(S::Int)), "-2");
        assert_eq!(
            shown(third.converted(S::String)),
            "'-2.33333333333333333333333333333'"
        );
        assert_eq!(shown(Value::Float(0.1).converted(S::Decimal)), "0.1");
        assert_eq!(shown(Value::text("1/8").converted(S::Decimal)), "0.125");
        assert_eq!(
            shown(Value::Date(1).converted(S::Datetime)),
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
    fn two_of_a_kind_differ_by_a_duration() {
        let at = |h: i64, m: i64| Value::Time(date::time_from(h, m, 0).unwrap());
        let ymd = |y, m, d| Value::Date(date::from_ymd(y, m, d).unwrap());
        let shown = |op, a: &Value, b: &Value| format_scalar(&arith(op, a, b).unwrap(), false);
        let year = arith(BinOp::Sub, &ymd(2025, 3, 1), &ymd(2024, 3, 1)).unwrap();
        assert!(matches!(year, Value::Duration(n) if n == 365 * date::DAY_MICROS));
        assert_eq!(format_scalar(&year, false), "P365D");
        assert_eq!(shown(BinOp::Sub, &at(9, 0), &at(10, 30)), "-PT1H30M");
        let half = Value::Duration(90 * 60 * date::MICROS);
        assert_eq!(shown(BinOp::Add, &at(9, 0), &half), "10:30");
        assert_eq!(shown(BinOp::Add, &half, &at(23, 0)), "00:30");
        // Beside a date only the whole days count.
        let longer = Value::Duration(36 * 3_600 * date::MICROS);
        assert_eq!(shown(BinOp::Add, &ymd(2025, 1, 31), &longer), "2025-02-01");
        assert_eq!(shown(BinOp::Sub, &ymd(2025, 1, 31), &longer), "2025-01-29");
        let noon = Value::Datetime(date::datetime_from(0, 12, 0, 0).unwrap(), None);
        assert_eq!(shown(BinOp::Add, &noon, &longer), "1970-01-03T00:00");
        assert_eq!(shown(BinOp::Sub, &longer, &half), "P1DT10H30M");
        assert_eq!(compare(&half, &longer).unwrap(), Some(Ordering::Less));
        assert!(arith(BinOp::Add, &half, &Value::int(1)).is_err());
        assert!(arith(BinOp::Sub, &half, &at(9, 0)).is_err());
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
            "P20119D"
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
        assert_eq!(shown(BinOp::Sub, &noon, &noon), "PT0S");
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

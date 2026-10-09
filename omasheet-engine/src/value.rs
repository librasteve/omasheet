// Copyright (c) 2026 Stephen Roe

//! Runtime values and exact arithmetic.
//!
//! Exact numbers are `Int` (arbitrary precision) or `Ratio` (an
//! arbitrary-precision fraction that is not a whole number). Nothing here
//! turns an exact number into a `Num` unless the other operand already is one.

use crate::zone::{Zoned, format_offset};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use omasheet_omx::ast::{BinOp, Lit};
use omasheet_omx::convert;
use omasheet_omx::date::Style;
use omasheet_omx::funcs::MathFn;
use omasheet_omx::types::S;
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub enum Value {
    /// No value: a blank cell, or a row offset outside the table.
    #[default]
    Empty,
    /// A cell whose calculation failed; the diagnostic was already reported.
    Error,
    Int(BigInt),
    Ratio(BigRational),
    Num(f64),
    Complex(f64, f64),
    Text(Rc<str>),
    Bool(bool),
    Date(i32),
    Time(i32),
    /// What the clocks of the sheet's time zone show.
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
    Table(View),
    Row(RowRef),
}

/// Rows and columns of a sheet table, by index. Cells are read on demand, so
/// selecting never copies data.
#[derive(Clone, Debug)]
pub struct View {
    pub table: usize,
    pub rows: Rc<Vec<usize>>,
    pub cols: Rc<Vec<usize>>,
}

#[derive(Clone, Debug)]
pub struct RowRef {
    pub table: usize,
    pub row: usize,
    pub cols: Rc<Vec<usize>>,
}

impl Value {
    pub fn text(s: &str) -> Value {
        Value::Text(Rc::from(s))
    }

    /// An exact number, as an `Int` when it is whole.
    pub fn exact(r: BigRational) -> Value {
        if r.is_integer() {
            Value::Int(r.to_integer())
        } else {
            Value::Ratio(r)
        }
    }

    pub fn from_lit(l: &Lit) -> Value {
        match l {
            Lit::Int(n) => Value::Int(n.clone()),
            Lit::Ratio(r) => Value::exact(r.clone()),
            Lit::Num(f) => Value::Num(*f),
            Lit::Complex(re, im) => Value::Complex(*re, *im),
            Lit::Text(s) => Value::text(s),
            Lit::Bool(b) => Value::Bool(*b),
            Lit::Date(d) => Value::Date(*d),
            Lit::Time(t) => Value::Time(*t),
            Lit::DateTime(t) => Value::DateTime(*t),
        }
    }

    /// What kind of value this is, for messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Empty => "empty",
            Value::Error => "an error",
            Value::Int(_) => "Int",
            Value::Ratio(_) => "Ratio",
            Value::Num(_) => "Num",
            Value::Complex(..) => "Complex",
            Value::Text(_) => "Text",
            Value::Bool(_) => "Bool",
            Value::Date(_) => "Date",
            Value::Time(_) => "Time",
            Value::DateTime(_) | Value::Zoned(_) => "DateTime",
            Value::Range { .. } => "a range",
            Value::Vector(_) => "a vector",
            Value::Table(_) => "a table",
            Value::Row(_) => "a row",
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Value::Int(_) | Value::Ratio(_) | Value::Num(_) | Value::Complex(..)
        )
    }

    /// A single value as a literal. A date-time in another zone is what its
    /// clocks show. `None` for empty, an error, or more than one value.
    fn to_lit(&self) -> Option<Lit> {
        Some(match self {
            Value::Int(n) => Lit::Int(n.clone()),
            Value::Ratio(r) => Lit::Ratio(r.clone()),
            Value::Num(f) => Lit::Num(*f),
            Value::Complex(re, im) => Lit::Complex(*re, *im),
            Value::Text(s) => Lit::Text(s.to_string()),
            Value::Bool(b) => Lit::Bool(*b),
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
            (Value::Empty, _) | (_, S::Any) => true,
            (Value::Zoned(_), S::DateTime) | (Value::Text(_), S::Text) => true,
            // A whole `Ratio` is held as an `Int`.
            (Value::Int(_), S::Int | S::Ratio) => true,
            _ => false,
        };
        if fits {
            return Ok(self.clone());
        }
        let Some(lit) = self.to_lit() else {
            return Err(format!("{} cannot be made a `{to}`", self.kind()));
        };
        if to == S::Text {
            return Ok(Value::text(&format_exact(self, false, &Style::ISO)));
        }
        convert::convert(&lit, to).map(|l| Value::from_lit(&l))
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => n.to_i64(),
            _ => None,
        }
    }
}

enum N {
    Exact(BigRational),
    Num(f64),
    Complex(f64, f64),
}

fn number(v: &Value) -> Option<N> {
    Some(match v {
        Value::Int(n) => N::Exact(BigRational::from_integer(n.clone())),
        Value::Ratio(r) => N::Exact(r.clone()),
        Value::Num(f) => N::Num(*f),
        Value::Complex(re, im) => N::Complex(*re, *im),
        _ => return None,
    })
}

pub fn to_f64(r: &BigRational) -> f64 {
    r.to_f64().unwrap_or(f64::NAN)
}

fn real(n: &N) -> f64 {
    match n {
        N::Exact(r) => to_f64(r),
        N::Num(f) => *f,
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
pub fn arith(op: BinOp, a: &Value, b: &Value) -> Result<Value, String> {
    if matches!(a, Value::Empty) || matches!(b, Value::Empty) {
        return Ok(Value::Empty);
    }
    if let Some(shifted) = shift(op, a, b) {
        return shifted;
    }
    let (Some(x), Some(y)) = (number(a), number(b)) else {
        return Err(format!(
            "cannot apply `{}` to {} and {}",
            op.symbol(),
            a.kind(),
            b.kind()
        ));
    };
    match (&x, &y) {
        (N::Exact(x), N::Exact(y)) => exact_arith(op, x, y),
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
            Ok(Value::Num(match op {
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
/// next to a `Date` and seconds next to a `Time` or `DateTime`. `None` if
/// neither side is one, or the two do not combine.
fn shift(op: BinOp, a: &Value, b: &Value) -> Option<Result<Value, String>> {
    let minus = match op {
        BinOp::Add => false,
        BinOp::Sub => true,
        _ => return None,
    };
    let count = |n: &BigInt| {
        n.to_i64()
            .ok_or_else(|| "the number is too large".to_string())
    };
    let signed = |n: i64| if minus { n.checked_neg() } else { Some(n) };
    let date = |d: i32, n: &BigInt| {
        let days = signed(count(n)?).and_then(|n| (d as i64).checked_add(n));
        match days.and_then(|d| i32::try_from(d).ok()) {
            Some(d) => Ok(Value::Date(d)),
            None => Err("the date is out of range".to_string()),
        }
    };
    let datetime = |t: i64, n: &BigInt| match signed(count(n)?).and_then(|n| t.checked_add(n)) {
        // Every date-time must have a day that is a valid date.
        Some(t) if i32::try_from(t.div_euclid(86_400)).is_ok() => Ok(Value::DateTime(t)),
        _ => Err("the date-time is out of range".to_string()),
    };
    let zoned = |z: &Zoned, n: &BigInt| match signed(count(n)?).and_then(|n| z.shifted(n)) {
        Some(z) => Ok(Value::Zoned(Rc::new(z))),
        None => Err("the date-time is out of range".to_string()),
    };
    // A time of day wraps round midnight.
    let time = |t: i32, n: &BigInt| {
        let n = (n % BigInt::from(86_400)).to_i64().unwrap_or(0);
        let n = if minus { -n } else { n };
        Ok(Value::Time((t as i64 + n).rem_euclid(86_400) as i32))
    };
    let int = |n: i64| Ok(Value::Int(BigInt::from(n)));
    Some(match (a, b) {
        (Value::Date(d), Value::Int(n)) => date(*d, n),
        (Value::DateTime(t), Value::Int(n)) => datetime(*t, n),
        (Value::Zoned(z), Value::Int(n)) => zoned(z, n),
        (Value::Time(t), Value::Int(n)) => time(*t, n),
        (Value::Int(n), Value::Date(d)) if !minus => date(*d, n),
        (Value::Int(n), Value::DateTime(t)) if !minus => datetime(*t, n),
        (Value::Int(n), Value::Zoned(z)) if !minus => zoned(z, n),
        (Value::Int(n), Value::Time(t)) if !minus => time(*t, n),
        (Value::Date(d), Value::Time(t)) | (Value::Time(t), Value::Date(d)) if !minus => {
            Ok(Value::DateTime(*d as i64 * 86_400 + *t as i64))
        }
        (Value::Date(x), Value::Date(y)) if minus => int(*x as i64 - *y as i64),
        (Value::Time(x), Value::Time(y)) if minus => int(*x as i64 - *y as i64),
        (Value::DateTime(x), Value::DateTime(y)) if minus => int(x - y),
        (Value::Zoned(x), Value::Zoned(y)) if minus => int(x.utc - y.utc),
        (Value::DateTime(x), Value::Zoned(y)) if minus => int(x - y.home_wall()),
        (Value::Zoned(x), Value::DateTime(y)) if minus => int(x.home_wall() - y),
        _ => return None,
    })
}

fn exact_arith(op: BinOp, x: &BigRational, y: &BigRational) -> Result<Value, String> {
    Ok(Value::exact(match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div => {
            if y.is_zero() {
                return Err("division by zero".into());
            }
            x / y
        }
        _ => {
            if !y.is_integer() {
                return Err(
                    "`**` with a fractional exponent is not exact; use `.Num` on an operand".into(),
                );
            }
            let Some(e) = y.to_integer().to_i32().filter(|e| e.abs() <= 1_000_000) else {
                return Err("the exponent is too large".into());
            };
            if x.is_zero() && e < 0 {
                return Err("division by zero".into());
            }
            x.pow(e)
        }
    }))
}

/// A function of one real number. An exact number stays exact where the
/// function allows it; otherwise the result is a `Num`.
pub fn math(m: MathFn, v: &Value) -> Result<Value, String> {
    let whole = |r: BigRational| Value::Int(r.to_integer());
    let x = match (v, m) {
        (Value::Empty, _) => return Ok(Value::Empty),
        (Value::Int(n), MathFn::Abs) => return Ok(Value::Int(n.abs())),
        (Value::Int(n), MathFn::Sign) => return Ok(Value::Int(n.signum())),
        (Value::Int(_) | Value::Ratio(_), MathFn::Im) => return Ok(Value::Int(BigInt::from(0))),
        (Value::Ratio(_), MathFn::Re | MathFn::Conj) => return Ok(v.clone()),
        (Value::Complex(re, im), _) => return complex_math(m, *re, *im),
        (Value::Int(_), _) if m.is_exact() => return Ok(v.clone()),
        (Value::Ratio(r), MathFn::Abs) => return Ok(Value::Ratio(r.abs())),
        (Value::Ratio(r), MathFn::Sign) => return Ok(Value::Int(r.numer().signum())),
        (Value::Ratio(r), MathFn::Round) => return Ok(whole(r.round())),
        (Value::Ratio(r), MathFn::Floor) => return Ok(whole(r.floor())),
        (Value::Ratio(r), MathFn::Ceil) => return Ok(whole(r.ceil())),
        (Value::Int(n), _) => n.to_f64().unwrap_or(f64::NAN),
        (Value::Ratio(r), _) => to_f64(r),
        (Value::Num(f), _) => *f,
        (other, _) => {
            return Err(format!(
                "`{}` cannot be applied to {}",
                m.name(),
                other.kind()
            ));
        }
    };
    let y = m.apply(x);
    // Outside the function's domain, such as `sqrt(-1)` or `ln(0)`.
    if x.is_finite() && !y.is_finite() {
        return Err(format!("`{}` is not defined for {x:?}", m.name()));
    }
    Ok(Value::Num(y))
}

fn complex_math(m: MathFn, re: f64, im: f64) -> Result<Value, String> {
    let r = re.hypot(im);
    Ok(match m {
        MathFn::Abs => Value::Num(r),
        MathFn::Re => Value::Num(re),
        MathFn::Im => Value::Num(im),
        MathFn::Arg => Value::Num(im.atan2(re)),
        MathFn::Conj => Value::Complex(re, -im),
        // The root with a real part that is not negative.
        MathFn::Sqrt => {
            let half = ((r - re) / 2.0).sqrt();
            Value::Complex(((r + re) / 2.0).sqrt(), if im < 0.0 { -half } else { half })
        }
        MathFn::Exp => Value::Complex(re.exp() * im.cos(), re.exp() * im.sin()),
        MathFn::Ln if r == 0.0 => return Err("`ln` is not defined for zero".into()),
        MathFn::Ln => Value::Complex(r.ln(), im.atan2(re)),
        _ => return Err(format!("`{}` is not defined for Complex", m.name())),
    })
}

/// Order two single values. `None` if either is empty.
pub fn compare(a: &Value, b: &Value) -> Result<Option<Ordering>, String> {
    Ok(Some(match (a, b) {
        (Value::Empty, _) | (_, Value::Empty) => return Ok(None),
        (Value::Text(x), Value::Text(y)) => x.cmp(y),
        (Value::Date(x), Value::Date(y)) => x.cmp(y),
        (Value::Time(x), Value::Time(y)) => x.cmp(y),
        (Value::DateTime(x), Value::DateTime(y)) => x.cmp(y),
        // Across time zones it is the instant that counts.
        (Value::Zoned(x), Value::Zoned(y)) => x.utc.cmp(&y.utc),
        (Value::DateTime(x), Value::Zoned(y)) => x.cmp(&y.home_wall()),
        (Value::Zoned(x), Value::DateTime(y)) => x.home_wall().cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        _ => match (number(a), number(b)) {
            (Some(N::Exact(x)), Some(N::Exact(y))) => x.cmp(&y),
            (Some(N::Complex(..)), _) | (_, Some(N::Complex(..))) => {
                return Err("Complex values have no order".into());
            }
            (Some(x), Some(y)) => match real(&x).partial_cmp(&real(&y)) {
                Some(o) => o,
                None => return Ok(None),
            },
            _ => {
                return Err(format!("cannot compare {} and {}", a.kind(), b.kind()));
            }
        },
    }))
}

/// Equality of two single values. `None` if either is empty.
pub fn equal(a: &Value, b: &Value) -> Result<Option<bool>, String> {
    if let (Some(x), Some(y)) = (number(a), number(b))
        && (matches!(x, N::Complex(..)) || matches!(y, N::Complex(..)))
    {
        return Ok(Some(complex(&x) == complex(&y)));
    }
    Ok(compare(a, b)?.map(|o| o == Ordering::Equal))
}

/// How many digits a `Ratio` is shown with after the decimal point.
pub const RATIO_DIGITS: usize = 5;

/// What follows a number that is shown with fewer digits than it has.
pub const MORE: char = '…';

/// A whole number of `10 ** -scale`s as a decimal with `scale` digits after
/// the point.
fn decimal(scaled: &BigInt, negative: bool, scale: usize) -> String {
    let digits = scaled.abs().to_string();
    let digits = format!("{digits:0>width$}", width = scale + 1);
    let (whole, frac) = digits.split_at(digits.len() - scale);
    let sign = if negative { "-" } else { "" };
    if scale == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{frac}")
    }
}

/// An exact number in full: a terminating fraction as a decimal, anything
/// else as `n/d`.
pub fn format_rat(r: &BigRational) -> String {
    let (two, five, ten) = (BigInt::from(2), BigInt::from(5), BigInt::from(10));
    let mut rest = r.denom().clone();
    let (mut twos, mut fives) = (0usize, 0usize);
    while (&rest % &two).is_zero() {
        rest /= &two;
        twos += 1;
    }
    while (&rest % &five).is_zero() {
        rest /= &five;
        fives += 1;
    }
    if !rest.is_one() {
        return format!("{}/{}", r.numer(), r.denom());
    }
    let scale = twos.max(fives);
    let mut scaled = r.numer().clone();
    for _ in 0..scale {
        scaled *= &ten;
    }
    scaled /= r.denom();
    decimal(&scaled, r.is_negative(), scale)
}

/// An exact number as it is shown: a decimal with up to [`RATIO_DIGITS`]
/// digits after the point. One with more is rounded to that many, a half
/// away from zero, and marked with [`MORE`].
pub fn format_ratio(r: &BigRational) -> String {
    let unit = num_traits::pow(BigInt::from(10), RATIO_DIGITS);
    let scaled = r * BigRational::from_integer(unit);
    if scaled.is_integer() {
        return format_rat(r);
    }
    let rounded = scaled.round().to_integer();
    format!("{}{MORE}", decimal(&rounded, r.is_negative(), RATIO_DIGITS))
}

/// A single value as text, the way a sheet shows it. `quoted` puts text in
/// quotes, as inside a vector.
pub fn format_scalar(v: &Value, quoted: bool) -> String {
    format_styled(v, quoted, &Style::ISO)
}

/// A single value as text, with dates and times in `style`.
pub fn format_styled(v: &Value, quoted: bool, style: &Style) -> String {
    format_value(v, quoted, style, false)
}

/// As [`format_styled`], with every exact number in full: `1/3`, where it
/// is shown as `0.33333…`.
pub fn format_exact(v: &Value, quoted: bool, style: &Style) -> String {
    format_value(v, quoted, style, true)
}

fn format_value(v: &Value, quoted: bool, style: &Style, exact: bool) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Error => "#ERROR".into(),
        Value::Int(n) => n.to_string(),
        Value::Ratio(r) if exact => format_rat(r),
        Value::Ratio(r) => format_ratio(r),
        // Always with an exponent, to tell it from an exact number.
        Value::Num(f) if f.is_finite() => format!("{f:e}"),
        Value::Num(f) => format!("{f:?}"),
        Value::Complex(re, im) => {
            if *im < 0.0 || (*im == 0.0 && im.is_sign_negative()) {
                format!("{re:?}-{:?}i", -im)
            } else {
                format!("{re:?}+{im:?}i")
            }
        }
        Value::Text(s) if quoted => format!("{s:?}"),
        Value::Text(s) => s.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Date(d) => style.date(*d),
        Value::Time(t) => style.time(*t),
        Value::DateTime(t) => style.datetime(*t),
        // `2025-01-31T09:30+09:00`, to tell it from the sheet's own zone.
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
                    Value::Empty => "empty".to_string(),
                    other => format_value(other, true, style, exact),
                })
                .collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Table(_) => "<table>".into(),
        Value::Row(_) => "<row>".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omasheet_omx::date;

    fn rat(n: i64, d: i64) -> BigRational {
        BigRational::new(n.into(), d.into())
    }

    #[test]
    fn formats_rationals() {
        assert_eq!(format_rat(&rat(175, 4)), "43.75");
        assert_eq!(format_rat(&rat(1, 3)), "1/3");
        assert_eq!(format_rat(&rat(-1, 8)), "-0.125");
        assert_eq!(format_rat(&rat(1, 20)), "0.05");
        assert_eq!(format_rat(&rat(-7, 3)), "-7/3");
    }

    #[test]
    fn shows_rationals() {
        assert_eq!(format_ratio(&rat(175, 4)), "43.75");
        assert_eq!(format_ratio(&rat(1, 100_000)), "0.00001");
        assert_eq!(format_ratio(&rat(1, 3)), "0.33333…");
        assert_eq!(format_ratio(&rat(2, 3)), "0.66667…");
        assert_eq!(format_ratio(&rat(-7, 3)), "-2.33333…");
        assert_eq!(format_ratio(&rat(1, 64)), "0.01563…");
        assert_eq!(format_ratio(&rat(-1, 1_000_000)), "-0.00000…");
        assert_eq!(format_ratio(&rat(1_999_999, 1_000_000)), "2.00000…");
    }

    #[test]
    fn converts() {
        let third = Value::Ratio(rat(-7, 3));
        let shown = |v: Result<Value, String>| format_exact(&v.unwrap(), true, &Style::ISO);
        assert_eq!(shown(third.converted(S::Int)), "-2");
        assert_eq!(shown(third.converted(S::Ratio)), "-7/3");
        assert_eq!(shown(third.converted(S::Text)), "\"-7/3\"");
        assert_eq!(shown(Value::Num(0.1).converted(S::Ratio)), "0.1");
        assert_eq!(shown(Value::text("1/8").converted(S::Ratio)), "0.125");
        assert_eq!(
            shown(Value::Date(1).converted(S::DateTime)),
            "1970-01-02T00:00"
        );
        assert_eq!(shown(Value::Empty.converted(S::Int)), "");
        assert!(Value::Date(1).converted(S::Int).is_err());
        assert!(
            Value::Vector(Rc::new(Vec::new()))
                .converted(S::Int)
                .is_err()
        );
    }

    #[test]
    fn division_is_exact() {
        let third = arith(BinOp::Div, &Value::Int(1.into()), &Value::Int(3.into())).unwrap();
        assert!(matches!(third, Value::Ratio(_)));
        let one = arith(BinOp::Mul, &third, &Value::Int(3.into())).unwrap();
        assert!(matches!(one, Value::Int(n) if n.is_one()));
        assert!(arith(BinOp::Div, &Value::Int(1.into()), &Value::Int(0.into())).is_err());
    }

    #[test]
    fn dates_and_times_shift() {
        let int = |n: i64| Value::Int(n.into());
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
        assert_eq!(shown(BinOp::Sub, &Value::Time(60), &int(120)), "23:59");
        assert_eq!(
            shown(BinOp::Add, &Value::Date(day), &Value::Time(34_200)),
            "2025-01-31T09:30"
        );
        let noon = arith(BinOp::Add, &Value::Date(day), &Value::Time(43_200)).unwrap();
        assert_eq!(shown(BinOp::Add, &noon, &int(86_400)), "2025-02-01T12:00");
        assert_eq!(shown(BinOp::Sub, &noon, &noon), "0");
        assert!(arith(BinOp::Sub, &int(1), &Value::Date(day)).is_err());
        assert!(arith(BinOp::Mul, &Value::Date(day), &int(2)).is_err());
        assert!(arith(BinOp::Sub, &noon, &Value::Date(day)).is_err());
        assert!(arith(BinOp::Add, &Value::Date(i32::MAX), &int(1)).is_err());
    }

    #[test]
    fn num_is_contagious() {
        let v = arith(BinOp::Div, &Value::Num(1.0), &Value::Int(3.into())).unwrap();
        assert!(matches!(v, Value::Num(_)));
    }
}

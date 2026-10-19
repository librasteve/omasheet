// Copyright (c) 2026 Stephen Roe

//! Runtime values and exact arithmetic.
//!
//! Exact numbers are `Int` (arbitrary precision) or `Decimal` (an
//! arbitrary-precision exact number that is not whole). Nothing here turns
//! an exact number into a `Num` unless the other operand already is one.

use crate::zone::{Zoned, format_offset};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use omasheet_omx::ast::{BinOp, Lit};
use omasheet_omx::convert::{self, is_decimal};
use omasheet_omx::date::Style;
use omasheet_omx::funcs::MathFn;
use omasheet_omx::types::S;
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub enum Value {
    /// No value: a blank cell, or a row offset outside the sheet.
    #[default]
    Empty,
    /// A cell whose calculation failed; the diagnostic was already reported.
    Error,
    Int(BigInt),
    /// An exact number that is not whole and ends as a decimal, shown as
    /// one: `2.75`.
    Decimal(BigRational),
    /// An exact number shown as a percentage: `1/5` for `20%`. It may be
    /// whole, `100%`.
    Percent(BigRational),
    Num(f64),
    /// Text, and whether it was written bare in a cell, to be read as
    /// Markdown where a file is shown as a document. Text written in quotes,
    /// or made by a formula, is as it is.
    Text(Rc<str>, bool),
    Bool(bool),
    Date(i32),
    Time(i32),
    /// What the clocks of the file's time zone show.
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
    /// Whether this is text to be read as Markdown: see [`Value::Text`].
    pub fn is_markdown(&self) -> bool {
        matches!(self, Value::Text(_, true))
    }

    pub fn text(s: &str) -> Value {
        Value::Text(Rc::from(s), false)
    }

    /// An exact number: an `Int` when it is whole, and otherwise a
    /// `Decimal`, which need not end: `1/3` is shown as `0.333…`.
    pub fn exact(r: BigRational) -> Value {
        if r.is_integer() {
            Value::Int(r.to_integer())
        } else {
            Value::Decimal(r)
        }
    }

    /// An exact number as a percentage; anything else as it is.
    pub fn percent(self) -> Value {
        match self {
            Value::Int(n) => Value::Percent(BigRational::from_integer(n)),
            Value::Decimal(r) => Value::Percent(r),
            other => other,
        }
    }

    /// A percentage as the plain number it stands for; anything else as it
    /// is.
    pub fn plain(self) -> Value {
        match self {
            Value::Percent(r) => Value::exact(r),
            other => other,
        }
    }

    pub fn from_lit(l: &Lit) -> Value {
        match l {
            Lit::Int(n) => Value::Int(n.clone()),
            Lit::Decimal(r) => Value::exact(r.clone()),
            Lit::Percent(r) => Value::Percent(r.clone()),
            Lit::Num(f) => Value::Num(*f),
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
            Value::Decimal(_) => "Decimal",
            Value::Percent(_) => "Percent",
            Value::Num(_) => "Num",
            Value::Text(..) => "Text",
            Value::Bool(_) => "Bool",
            Value::Date(_) => "Date",
            Value::Time(_) => "Time",
            Value::DateTime(_) | Value::Zoned(_) => "DateTime",
            Value::Range { .. } => "a range",
            Value::Vector(_) => "a vector",
            Value::Sheet(_) => "a sheet",
            Value::Row(_) => "a row",
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Value::Int(_) | Value::Decimal(_) | Value::Percent(_) | Value::Num(_)
        )
    }

    /// A single value as a literal. A date-time in another zone is what its
    /// clocks show. `None` for empty, an error, or more than one value.
    fn to_lit(&self) -> Option<Lit> {
        Some(match self {
            Value::Int(n) => Lit::Int(n.clone()),
            Value::Decimal(r) => Lit::Decimal(r.clone()),
            Value::Percent(r) => Lit::Percent(r.clone()),
            Value::Num(f) => Lit::Num(*f),
            Value::Text(s, _) => Lit::Text(s.to_string()),
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
            (Value::Zoned(_), S::DateTime) | (Value::Text(..), S::Text) => true,
            // A whole `Decimal` is held as an `Int`.
            (Value::Int(_), S::Int | S::Decimal) => true,
            (Value::Decimal(_), S::Decimal) => true,
            (Value::Percent(_), S::Percent) => true,
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
}

fn number(v: &Value) -> Option<N> {
    Some(match v {
        Value::Int(n) => N::Exact(BigRational::from_integer(n.clone())),
        Value::Decimal(r) | Value::Percent(r) => N::Exact(r.clone()),
        Value::Num(f) => N::Num(*f),
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
    // Percentages add up to a percentage; anything else done with one gives
    // the plain number.
    if let (Value::Percent(x), Value::Percent(y), BinOp::Add | BinOp::Sub) = (a, b, op) {
        return Ok(Value::Percent(if op == BinOp::Add { x + y } else { x - y }));
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

/// An exact number divided by zero, which no exact number is: `Inf` or
/// `-Inf` by its sign, and `NaN` for zero.
fn over_zero(x: &BigRational) -> f64 {
    if x.is_zero() {
        f64::NAN
    } else if x.is_negative() {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    }
}

fn exact_arith(op: BinOp, x: &BigRational, y: &BigRational) -> Result<Value, String> {
    Ok(Value::exact(match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div => {
            if y.is_zero() {
                return Ok(Value::Num(over_zero(x)));
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
                return Ok(Value::Num(f64::INFINITY));
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
        (Value::Percent(r), MathFn::Abs) => return Ok(Value::Percent(r.abs())),
        (Value::Percent(_), _) => return math(m, &v.clone().plain()),
        (Value::Int(n), MathFn::Abs) => return Ok(Value::Int(n.abs())),
        (Value::Int(n), MathFn::Sign) => return Ok(Value::Int(n.signum())),
        (Value::Int(_), _) if m.is_exact() => return Ok(v.clone()),
        (Value::Decimal(r), MathFn::Abs) => return Ok(Value::Decimal(r.abs())),
        (Value::Decimal(r), MathFn::Sign) => {
            return Ok(Value::Int(r.numer().signum()));
        }
        (Value::Decimal(r), MathFn::Round) => return Ok(whole(r.round())),
        (Value::Decimal(r), MathFn::Floor) => return Ok(whole(r.floor())),
        (Value::Decimal(r), MathFn::Ceil) => return Ok(whole(r.ceil())),
        (Value::Int(n), _) => n.to_f64().unwrap_or(f64::NAN),
        (Value::Decimal(r), _) => to_f64(r),
        (Value::Num(f), _) => *f,
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
    Ok(Value::Num(m.apply(x)))
}

/// Whether a value is `NaN`, which is not equal to, less than or more than
/// anything, itself included.
pub fn is_nan(v: &Value) -> bool {
    matches!(v, Value::Num(f) if f.is_nan())
}

/// Order two single values. `None` if either is empty, or `NaN`.
pub fn compare(a: &Value, b: &Value) -> Result<Option<Ordering>, String> {
    Ok(Some(match (a, b) {
        (Value::Empty, _) | (_, Value::Empty) => return Ok(None),
        (Value::Text(x, _), Value::Text(y, _)) => x.cmp(y),
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

/// An exact number as it is written: one that ends as that decimal in
/// full, and any other as `n/d`.
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

/// `10 ** e` as an exact number.
fn power_of_ten(e: i64) -> BigRational {
    let p = BigRational::from_integer(num_traits::pow(BigInt::from(10), e.unsigned_abs() as usize));
    if e < 0 { p.recip() } else { p }
}

/// The power of ten of a positive exact number: `e` where `10 ** e <= r <
/// 10 ** (e + 1)`.
fn magnitude(r: &BigRational) -> i64 {
    let digits = |n: &BigInt| n.to_string().len() as i64;
    let e = digits(r.numer()) - digits(r.denom());
    if *r < power_of_ten(e) { e - 1 } else { e }
}

/// The exponent of e notation: `e+15`, `e-05`.
fn exponent(e: i64) -> String {
    format!("e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// A positive exact number in e notation, with every significant digit; cut
/// to [`WIDTH`] and marked with [`MORE`] where that is too wide, as one that
/// does not end always is.
fn scientific(r: &BigRational) -> String {
    let e = magnitude(r);
    // One digit more than can be shown tells whether any are left out.
    let scaled = r * power_of_ten(WIDTH as i64 - e);
    let whole = scaled.trunc().to_integer().to_string();
    let complete = scaled.is_integer();
    let digits = if complete {
        whole.trim_end_matches('0')
    } else {
        &whole
    };
    let mantissa = match digits.split_at(1) {
        (first, "") => first.to_string(),
        (first, rest) => format!("{first}.{rest}"),
    };
    let exp = exponent(e);
    if complete && mantissa.len() + exp.len() <= WIDTH {
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

/// A positive exact number that ends, plain if it is neither small nor large
/// and fits, and otherwise in e notation.
fn shown_decimal(r: &BigRational, sep: Option<char>) -> String {
    if r.is_zero() {
        return "0".into();
    }
    let medium = *r >= power_of_ten(-15) && *r < power_of_ten(15);
    // One that does not end is cut and marked: `0.3333333333333333333333…`.
    if !is_decimal(r) {
        if !medium {
            return scientific(r);
        }
        let digits: String = grouped(cut_decimal(r, WIDTH), sep)
            .chars()
            .take(WIDTH - 1)
            .collect();
        return format!("{digits}{MORE}");
    }
    let plain = grouped(format_rat(r), sep);
    if medium && width(&plain) <= WIDTH {
        plain
    } else {
        scientific(r)
    }
}

/// The first `places` digits after the point of a positive `r`, cut, not
/// rounded.
fn cut_decimal(r: &BigRational, places: usize) -> String {
    let scaled = (r * power_of_ten(places as i64)).trunc().to_integer();
    decimal(&scaled, false, places)
}

/// How many digits after the point a `Decimal` that does not end is given
/// to, in full.
pub const LONG_DIGITS: usize = 40;

/// A `Decimal` that does not end, as far as [`LONG_DIGITS`] digits after
/// the point, cut and marked: `2.3333333333333333333333333333333333333333…`.
/// `None` for anything else.
pub fn long_decimal(v: &Value) -> Option<String> {
    match v {
        Value::Decimal(r) if !is_decimal(r) => Some(format!(
            "{}{MORE}",
            signed(r.is_negative(), cut_decimal(&r.abs(), LONG_DIGITS))
        )),
        _ => None,
    }
}

/// A rounded decimal without the zeros, or the point, it ends with.
fn trimmed(text: String) -> String {
    if !text.contains('.') {
        return text;
    }
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `r` rounded to [`PERCENT_DIGITS`] digits after the point, a half away
/// from zero.
fn rounded(r: &BigRational) -> BigInt {
    let unit = BigRational::from_integer(num_traits::pow(BigInt::from(10), PERCENT_DIGITS));
    (r * unit).round().to_integer()
}

/// A `Percent` as so many in a hundred. In full when `exact`, `125/3%`;
/// otherwise rounded to [`PERCENT_DIGITS`] digits after the point, `41.67%`,
/// and from `1e15%` in e notation, `1.43e+19%`. It is never marked with
/// [`MORE`]: rounding is the rule.
pub fn format_percent(r: &BigRational, exact: bool) -> String {
    shown_percent(r, exact, None)
}

/// As [`format_percent`], with the digits grouped by `sep`.
fn shown_percent(r: &BigRational, exact: bool, sep: Option<char>) -> String {
    let hundred = r * BigRational::from_integer(BigInt::from(100));
    if exact {
        return format!("{}%", format_rat(&hundred));
    }
    let size = hundred.abs();
    let mut scaled = rounded(&size);
    let shown = if size < power_of_ten(15) {
        grouped(trimmed(decimal(&scaled, false, PERCENT_DIGITS)), sep)
    } else {
        let mut e = magnitude(&size);
        scaled = rounded(&(&size / power_of_ten(e)));
        // `9.999` rounds to ten, which is one of the next power.
        if scaled >= BigInt::from(1000) {
            e += 1;
            scaled = rounded(&(&size / power_of_ten(e)));
        }
        format!(
            "{}{}",
            trimmed(decimal(&scaled, false, PERCENT_DIGITS)),
            exponent(e)
        )
    };
    let sign = if hundred.is_negative() && !scaled.is_zero() {
        "-"
    } else {
        ""
    };
    format!("{sign}{shown}%")
}

/// A `Num` as Raku writes one, with the fewest digits that give the same
/// `Num` back: a whole number as `100`, one with a fraction as `1.5`, and a
/// very large or very small one with an exponent, `1e+21` or `1.5e-07`.
/// Shown, it is plain from `1e-15` to `1e15` where that fits in [`WIDTH`],
/// and so never needs [`MORE`]; in full, as when `exact`, only from `1e-4`.
pub fn format_num(f: f64, exact: bool) -> String {
    shown_num(f, exact, None)
}

/// As [`format_num`], with the digits grouped by `sep`.
fn shown_num(f: f64, exact: bool, sep: Option<char>) -> String {
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
        Value::Empty => String::new(),
        Value::Error => "#ERROR".into(),
        // In full when `exact`, as a file writes it: `19.99`, and one that
        // does not end as `11/4`.
        Value::Int(n) if exact => n.to_string(),
        Value::Decimal(r) if exact => format_rat(r),
        // Shown, in at most `WIDTH` characters and a sign: plain from
        // `1e-15` to `1e15`, and otherwise in e notation, `1e+60`.
        // Shown in the app, a plain number of five digits or more is grouped
        // in threes, `12,345`, and its point is the locale's.
        Value::Int(n) => localised(
            signed(
                n.is_negative(),
                shown_decimal(&BigRational::from_integer(n.abs()), group(style)),
            ),
            style,
        ),
        Value::Decimal(r) => localised(
            signed(r.is_negative(), shown_decimal(&r.abs(), group(style))),
            style,
        ),
        Value::Percent(r) if exact => format_percent(r, true),
        Value::Percent(r) => localised(shown_percent(r, false, group(style)), style),
        Value::Num(f) if exact => format_num(*f, true),
        Value::Num(f) => localised(shown_num(*f, false, group(style)), style),
        Value::Text(s, _) if quoted => format!("{s:?}"),
        Value::Text(s, _) => s.to_string(),
        Value::Bool(b) => b.to_string(),
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
                    Value::Empty => "empty".to_string(),
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
        assert_eq!(format_rat(&rat(22, 7)), "22/7");
        assert_eq!(format_rat(&rat(11, 4)), "2.75");
        assert_eq!(format_rat(&rat(-1, 3)), "-1/3");
    }

    #[test]
    fn shows_exact_numbers_in_23_characters() {
        let shown = |v: Value| format_scalar(&v, false);
        let int = |n: &str| Value::Int(n.parse().unwrap());
        let big = |e: usize| num_traits::pow(BigInt::from(10), e);
        assert_eq!(shown(int("999999999999999")), "999999999999999");
        assert_eq!(shown(Value::Int(big(15))), "1e+15");
        assert_eq!(shown(Value::Int(big(60))), "1e+60");
        assert_eq!(shown(int("123456789012345678")), "1.23456789012345678e+17");
        assert_eq!(
            shown(int("18446744073709551616")),
            "1.8446744073709551…e+19"
        );
        assert_eq!(
            shown(int("-18446744073709551616")),
            "-1.8446744073709551…e+19"
        );
        assert_eq!(shown(Value::Decimal(rat(1, 8))), "0.125");
        assert_eq!(
            shown(Value::Decimal(rat(1, 1_000_000_000_000_000))),
            "0.000000000000001"
        );
        assert_eq!(
            shown(Value::Decimal(BigRational::new(1.into(), big(43)))),
            "1e-43"
        );
        assert_eq!(
            shown(Value::Decimal(BigRational::new(
                "12345678901234567890123".parse().unwrap(),
                big(23)
            ))),
            "1.2345678901234567…e-01"
        );
    }

    #[test]
    fn groups_digits_in_the_app() {
        let app = Style::ISO.numbers('.', ',');
        let shown = |v: Value| format_styled(&v, false, &app);
        let int = |n: i64| Value::Int(n.into());
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
            shown(Value::Decimal(BigRational::new(
                "9999999999999999999999".parse().unwrap(),
                10_000_000.into()
            ))),
            "9.9999999999999999…e+14"
        );
        assert_eq!(
            shown(Value::Percent(rat(12_345_678, 100_000))),
            "12,345.68%"
        );
        assert_eq!(shown(Value::Num(1_234_567.5)), "1,234,567.5");
        assert_eq!(shown(Value::Num(-0.000123)), "-0.000123");
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
        assert_eq!(shown(Value::Num(1.5e-20)), "1,5e-20");
        assert_eq!(shown(Value::Percent(rat(5, 12))), "41,67%");
        assert_eq!(delocalised("12.345,5", &de), "12345.5");
        // In full, as a file writes it, whatever the locale.
        assert_eq!(
            format_exact(&Value::Decimal(rat(123_455, 10)), false, &de),
            "12345.5"
        );
        let fr = Style::ISO.numbers(',', '\u{202F}');
        assert_eq!(
            format_styled(&Value::Int(1_234_567.into()), false, &fr),
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
        assert_eq!(format_percent(&rat(5, 12), true), "125/3%");
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
        .map(|f| format_num(*f, true))
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
        let shown = |f| format_num(f, false);
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
        assert_eq!(shown(third.converted(S::Text)), "\"-7/3\"");
        assert_eq!(shown(Value::Num(0.1).converted(S::Decimal)), "0.1");
        assert_eq!(shown(Value::text("1/8").converted(S::Decimal)), "0.125");
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
        assert!(matches!(third, Value::Decimal(_)));
        let one = arith(BinOp::Mul, &third, &Value::Int(3.into())).unwrap();
        assert!(matches!(one, Value::Int(n) if n.is_one()));
    }

    #[test]
    fn division_by_zero_is_a_num() {
        let over_zero = |n: i64| {
            let v = arith(BinOp::Div, &Value::Int(n.into()), &Value::Int(0.into())).unwrap();
            assert!(matches!(v, Value::Num(_)));
            format_scalar(&v, false)
        };
        assert_eq!(over_zero(1), "Inf");
        assert_eq!(over_zero(-1), "-Inf");
        assert_eq!(over_zero(0), "NaN");
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

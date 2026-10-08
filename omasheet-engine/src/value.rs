//! Runtime values and exact arithmetic.
//!
//! Exact numbers are `Int` (arbitrary precision) or `Rat` (an
//! arbitrary-precision fraction that is not a whole number). Nothing here
//! turns an exact number into a `Num` unless the other operand already is one.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use omasheet_omx::ast::{BinOp, Lit};
use omasheet_omx::date;
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub enum Value {
    /// No value: a blank cell, or a row offset outside the table.
    Empty,
    /// A cell whose calculation failed; the diagnostic was already reported.
    Error,
    Int(BigInt),
    Rat(BigRational),
    Num(f64),
    Complex(f64, f64),
    Text(Rc<str>),
    Bool(bool),
    Date(i32),
    Range {
        lo: i64,
        hi: i64,
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
            Value::Rat(r)
        }
    }

    pub fn from_lit(l: &Lit) -> Value {
        match l {
            Lit::Int(n) => Value::Int(n.clone()),
            Lit::Rat(r) => Value::exact(r.clone()),
            Lit::Num(f) => Value::Num(*f),
            Lit::Text(s) => Value::text(s),
            Lit::Bool(b) => Value::Bool(*b),
            Lit::Date(d) => Value::Date(*d),
        }
    }

    /// What kind of value this is, for messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Empty => "empty",
            Value::Error => "an error",
            Value::Int(_) => "Int",
            Value::Rat(_) => "Rat",
            Value::Num(_) => "Num",
            Value::Complex(..) => "Complex",
            Value::Text(_) => "Text",
            Value::Bool(_) => "Bool",
            Value::Date(_) => "Date",
            Value::Range { .. } => "a range",
            Value::Vector(_) => "a vector",
            Value::Table(_) => "a table",
            Value::Row(_) => "a row",
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Value::Int(_) | Value::Rat(_) | Value::Num(_) | Value::Complex(..)
        )
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
        Value::Rat(r) => N::Exact(r.clone()),
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
                    "`**` with a fractional exponent is not exact; use `approx(...)` on an operand"
                        .into(),
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

/// Order two single values. `None` if either is empty.
pub fn compare(a: &Value, b: &Value) -> Result<Option<Ordering>, String> {
    Ok(Some(match (a, b) {
        (Value::Empty, _) | (_, Value::Empty) => return Ok(None),
        (Value::Text(x), Value::Text(y)) => x.cmp(y),
        (Value::Date(x), Value::Date(y)) => x.cmp(y),
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

/// A terminating fraction as a decimal, anything else as `n/d`.
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
    let digits = scaled.abs().to_string();
    let digits = format!("{digits:0>width$}", width = scale + 1);
    let (whole, frac) = digits.split_at(digits.len() - scale);
    let sign = if scaled.is_negative() { "-" } else { "" };
    if scale == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{frac}")
    }
}

/// A single value as text. `quoted` puts text in quotes, as inside a vector.
pub fn format_scalar(v: &Value, quoted: bool) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Error => "#ERROR".into(),
        Value::Int(n) => n.to_string(),
        Value::Rat(r) => format_rat(r),
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
        Value::Date(d) => date::format(*d),
        Value::Range { lo, hi, exclusive } => {
            format!("{lo}{}{hi}", if *exclusive { "..^" } else { ".." })
        }
        Value::Vector(items) => {
            let parts: Vec<String> = items
                .iter()
                .map(|x| match x {
                    Value::Empty => "empty".to_string(),
                    other => format_scalar(other, true),
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
    fn division_is_exact() {
        let third = arith(BinOp::Div, &Value::Int(1.into()), &Value::Int(3.into())).unwrap();
        assert!(matches!(third, Value::Rat(_)));
        let one = arith(BinOp::Mul, &third, &Value::Int(3.into())).unwrap();
        assert!(matches!(one, Value::Int(n) if n.is_one()));
        assert!(arith(BinOp::Div, &Value::Int(1.into()), &Value::Int(0.into())).is_err());
    }

    #[test]
    fn num_is_contagious() {
        let v = arith(BinOp::Div, &Value::Num(1.0), &Value::Int(3.into())).unwrap();
        assert!(matches!(v, Value::Num(_)));
    }
}

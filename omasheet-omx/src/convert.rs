// Copyright (c) 2026 Stephen Roe

//! Conversions between the types: what `Int(x)`, `Ratio(x)` and the rest
//! do.

use crate::ast::Lit;
use crate::lexer::{Tok, lex};
use crate::types::S;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};
use std::str::FromStr;

/// A value written on its own, such as `42`, `-1/7`, `3+4i` or
/// `2025-01-31`. `None` if the text is anything else.
pub fn parse_literal(text: &str) -> Option<Lit> {
    let toks = lex(text, 0, 0).ok()?;
    let toks: Vec<&Tok> = toks.iter().map(|t| &t.tok).collect();
    // A fraction of two whole numbers, `1/7`, is an exact number.
    let fraction = |n: &BigInt, d: &BigInt| {
        (!d.is_zero()).then(|| Lit::Ratio(BigRational::new(n.clone(), d.clone())))
    };
    // A complex number: `4i`, `3+4i`, `-1.5-2i`.
    let real = |t: &Tok| match t {
        Tok::Int(n) => n.to_f64(),
        Tok::Decimal(r) | Tok::Percent(r) => r.to_f64(),
        Tok::Num(f) => Some(*f),
        _ => None,
    };
    let (negative, rest) = match toks.as_slice() {
        [Tok::Minus, rest @ ..] => (true, rest),
        rest => (false, rest),
    };
    let sign = if negative { -1.0 } else { 1.0 };
    match rest {
        [Tok::Imag(im), Tok::Eof] => return Some(Lit::Complex(0.0, sign * im)),
        [re, op @ (Tok::Plus | Tok::Minus), Tok::Imag(im), Tok::Eof] => {
            let im = if **op == Tok::Minus { -im } else { *im };
            return Some(Lit::Complex(sign * real(re)?, im));
        }
        _ => {}
    }
    Some(match toks.as_slice() {
        [Tok::Int(n), Tok::Slash, Tok::Int(d), Tok::Eof] => fraction(n, d)?,
        [Tok::Minus, Tok::Int(n), Tok::Slash, Tok::Int(d), Tok::Eof] => fraction(&-n, d)?,
        [Tok::Int(n), Tok::Eof] => Lit::Int(n.clone()),
        [Tok::Decimal(r), Tok::Eof] => Lit::Decimal(r.clone()),
        [Tok::Percent(r), Tok::Eof] => Lit::Percent(r.clone()),
        [Tok::Num(f), Tok::Eof] => Lit::Num(*f),
        [Tok::Minus, Tok::Int(n), Tok::Eof] => Lit::Int(-n.clone()),
        [Tok::Minus, Tok::Decimal(r), Tok::Eof] => Lit::Decimal(-r.clone()),
        [Tok::Minus, Tok::Percent(r), Tok::Eof] => Lit::Percent(-r.clone()),
        [Tok::Minus, Tok::Num(f), Tok::Eof] => Lit::Num(-*f),
        [Tok::Str(s), Tok::Eof] => Lit::Text(s.clone()),
        [Tok::Date(d), Tok::Eof] => Lit::Date(*d),
        [Tok::Time(t), Tok::Eof] => Lit::Time(*t),
        [Tok::DateTime(t), Tok::Eof] => Lit::DateTime(*t),
        [Tok::True, Tok::Eof] => Lit::Bool(true),
        [Tok::False, Tok::Eof] => Lit::Bool(false),
        _ => return None,
    })
}

pub fn lit_type(l: &Lit) -> S {
    match l {
        Lit::Int(_) => S::Int,
        Lit::Decimal(_) => S::Decimal,
        Lit::Ratio(_) => S::Ratio,
        Lit::Percent(_) => S::Percent,
        Lit::Num(_) => S::Num,
        Lit::Complex(..) => S::Complex,
        Lit::Text(_) => S::Text,
        Lit::Bool(_) => S::Bool,
        Lit::Date(_) => S::Date,
        Lit::Time(_) => S::Time,
        Lit::DateTime(_) => S::DateTime,
    }
}

/// Whether an exact number ends when written as a decimal: `1/8` does, as
/// `0.125`, and `1/3` does not. It does if its denominator has no prime
/// factor but 2 and 5.
pub fn is_decimal(r: &BigRational) -> bool {
    let mut rest = r.denom().clone();
    for prime in [2, 5] {
        let prime = BigInt::from(prime);
        while (&rest % &prime).is_zero() {
            rest /= &prime;
        }
    }
    rest == BigInt::from(1)
}

/// The exact number a `Num` is shown as: `0.1`, not the binary fraction
/// nearest to it. `None` if it is not finite.
pub fn ratio_of(f: f64) -> Option<BigRational> {
    if !f.is_finite() {
        return None;
    }
    // The shortest decimal that reads back as the same `Num`.
    let text = format!("{f:e}");
    let (mantissa, exponent) = text.split_once('e')?;
    let (negative, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, mantissa),
    };
    let (whole, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = BigInt::from_str(&format!("{whole}{frac}")).ok()?;
    let power = exponent.parse::<i64>().ok()? - frac.len() as i64;
    let scale = num_traits::pow(BigInt::from(10), power.unsigned_abs() as usize);
    let r = if power >= 0 {
        BigRational::from_integer(digits * scale)
    } else {
        BigRational::new(digits, scale)
    };
    Some(if negative { -r } else { r })
}

/// The name of a type with `a` or `an` before it.
fn a(to: S) -> String {
    let article = if to == S::Int { "an" } else { "a" };
    format!("{article} {to}")
}

/// A value as the type `to`, as the conversion of that name gives it. A
/// fraction loses its fractional part as an `Int`; text is read the way a
/// cell is. The error says why it cannot be done. Writing a value as `Text`
/// is not done here: how it is written is the engine's business.
pub fn convert(lit: &Lit, to: S) -> Result<Lit, String> {
    let from = lit_type(lit);
    let no = || Err(format!("{from} cannot be made {}", a(to)));
    let one = |b: bool| BigInt::from(b as u8);
    let real = |l: &Lit| match l {
        Lit::Int(n) => n.to_f64(),
        Lit::Decimal(r) | Lit::Ratio(r) | Lit::Percent(r) => r.to_f64(),
        Lit::Num(f) => Some(*f),
        Lit::Bool(b) => Some(*b as u8 as f64),
        _ => None,
    };
    if from == to || to == S::Any {
        return Ok(lit.clone());
    }
    Ok(match (lit, to) {
        (Lit::Text(s), _) => {
            let read = parse_literal(s.trim()).filter(|l| !matches!(l, Lit::Text(_)));
            return read
                .and_then(|l| convert(&l, to).ok())
                .ok_or_else(|| format!("`{s}` is not {}", a(to)));
        }
        // A `Percent` is the number it stands for: `20%` is `0.2`.
        (Lit::Percent(r), S::Int | S::Decimal | S::Ratio | S::Bool) => {
            let plain = if r.is_integer() {
                Lit::Int(r.to_integer())
            } else if is_decimal(r) {
                Lit::Decimal(r.clone())
            } else {
                Lit::Ratio(r.clone())
            };
            return convert(&plain, to);
        }
        (Lit::Int(n), S::Percent) => Lit::Percent(BigRational::from_integer(n.clone())),
        (Lit::Decimal(r) | Lit::Ratio(r), S::Percent) => Lit::Percent(r.clone()),
        (Lit::Int(n), S::Decimal | S::Ratio) => Lit::Int(n.clone()),
        (Lit::Decimal(r) | Lit::Ratio(r), S::Int) => Lit::Int(r.trunc().to_integer()),
        // Every `Decimal` is a `Ratio`; a `Ratio` is a `Decimal` if it ends.
        (Lit::Decimal(r), S::Ratio) => Lit::Ratio(r.clone()),
        (Lit::Ratio(r), S::Decimal) if is_decimal(r) => Lit::Decimal(r.clone()),
        (Lit::Ratio(r), S::Decimal) => {
            return Err(format!(
                "{}/{} does not end as a decimal",
                r.numer(),
                r.denom()
            ));
        }
        (Lit::Num(f), S::Int | S::Decimal | S::Ratio | S::Percent) => match ratio_of(*f) {
            Some(r) if to == S::Percent => Lit::Percent(r),
            Some(r) if to == S::Int || r.is_integer() => Lit::Int(r.trunc().to_integer()),
            Some(r) if to == S::Decimal => Lit::Decimal(r),
            Some(r) => Lit::Ratio(r),
            None => return Err(format!("{f:?} has no exact value")),
        },
        (Lit::Bool(b), S::Int | S::Decimal | S::Ratio) => Lit::Int(one(*b)),
        (
            Lit::Int(_) | Lit::Decimal(_) | Lit::Ratio(_) | Lit::Percent(_) | Lit::Bool(_),
            S::Num,
        ) => match real(lit) {
            Some(f) => Lit::Num(f),
            None => return no(),
        },
        (
            Lit::Int(_) | Lit::Decimal(_) | Lit::Ratio(_) | Lit::Percent(_) | Lit::Num(_),
            S::Complex,
        ) => match real(lit) {
            Some(f) => Lit::Complex(f, 0.0),
            None => return no(),
        },
        (Lit::Int(n), S::Bool) => Lit::Bool(!n.is_zero()),
        (Lit::Decimal(r) | Lit::Ratio(r), S::Bool) => Lit::Bool(!r.is_zero()),
        (Lit::Num(f), S::Bool) => Lit::Bool(*f != 0.0),
        (Lit::DateTime(t), S::Date) => Lit::Date(crate::date::split_datetime(*t).0),
        (Lit::DateTime(t), S::Time) => Lit::Time(crate::date::split_datetime(*t).1),
        (Lit::Date(d), S::DateTime) => Lit::DateTime(*d as i64 * 86_400),
        _ => return no(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPES: [S; 11] = [
        S::Int,
        S::Decimal,
        S::Ratio,
        S::Percent,
        S::Num,
        S::Complex,
        S::Text,
        S::Bool,
        S::Date,
        S::Time,
        S::DateTime,
    ];

    fn lit(text: &str) -> Lit {
        parse_literal(text).unwrap()
    }

    #[test]
    fn converts_numbers() {
        assert_eq!(convert(&lit("19.99"), S::Int), Ok(lit("19")));
        assert_eq!(convert(&lit("-19.99"), S::Int), Ok(lit("-19")));
        assert_eq!(convert(&lit("1e-1"), S::Ratio), Ok(lit("1/10")));
        assert_eq!(convert(&lit("1e-1"), S::Decimal), Ok(lit("0.1")));
        assert_eq!(convert(&lit("1/8"), S::Decimal), Ok(lit("0.125")));
        assert_eq!(convert(&lit("0.125"), S::Ratio), Ok(lit("1/8")));
        assert_eq!(
            convert(&lit("1/3"), S::Decimal),
            Err("1/3 does not end as a decimal".to_string())
        );
        assert_eq!(convert(&lit("-2.5e3"), S::Ratio), Ok(lit("-2500")));
        assert_eq!(convert(&lit("1.5e3"), S::Int), Ok(lit("1500")));
        assert_eq!(convert(&lit("1/4"), S::Num), Ok(Lit::Num(0.25)));
        assert_eq!(convert(&lit("3"), S::Complex), Ok(Lit::Complex(3.0, 0.0)));
        assert_eq!(convert(&lit("true"), S::Int), Ok(lit("1")));
        assert_eq!(convert(&lit("0.5"), S::Bool), Ok(lit("true")));
        assert!(convert(&Lit::Num(f64::NAN), S::Int).is_err());
    }

    #[test]
    fn reads_text() {
        let text = |s: &str| Lit::Text(s.to_string());
        assert_eq!(convert(&text(" 42 "), S::Int), Ok(lit("42")));
        assert_eq!(convert(&text("19.99"), S::Int), Ok(lit("19")));
        assert_eq!(convert(&text("20%"), S::Ratio), Ok(lit("1/5")));
        assert_eq!(convert(&text("2025-01-31"), S::Date), Ok(lit("2025-01-31")));
        assert_eq!(
            convert(&text("abc"), S::Int),
            Err("`abc` is not an Int".to_string())
        );
        assert!(convert(&text("2025-01-31"), S::Int).is_err());
    }

    #[test]
    fn dates_and_times() {
        let moment = lit("2025-01-31T09:30");
        assert_eq!(convert(&moment, S::Date), Ok(lit("2025-01-31")));
        assert_eq!(convert(&moment, S::Time), Ok(lit("09:30")));
        assert_eq!(
            convert(&lit("2025-01-31"), S::DateTime),
            Ok(lit("2025-01-31T00:00"))
        );
        assert_eq!(
            convert(&lit("09:30"), S::Date),
            Err("Time cannot be made a Date".to_string())
        );
    }

    /// The checker and the conversion agree on what can be converted.
    #[test]
    fn agrees_with_the_types() {
        let samples = [
            lit("7"),
            lit("0.5"),
            // A fraction that ends: whether a `Ratio` is a `Decimal` depends
            // on the number, as whether text is a number depends on the text.
            lit("1/4"),
            lit("12.5%"),
            lit("1e-1"),
            lit("3+4i"),
            lit("true"),
            lit("2025-01-31"),
            lit("09:30"),
            lit("2025-01-31T09:30"),
        ];
        for sample in &samples {
            let from = lit_type(sample);
            for to in TYPES {
                // Text is another matter: it depends on what the text says.
                if to == S::Text {
                    continue;
                }
                assert_eq!(
                    convert(sample, to).is_ok(),
                    from.converts_to(to),
                    "{from} to {to}"
                );
            }
        }
    }
}

// Copyright (c) 2026 Stephen Roe

//! Conversions between the types: what `Integer(x)`, `Decimal(x)` and the rest
//! do.

use crate::ast::Lit;
use crate::decimal::Dec;
use crate::lexer::{Tok, lex};
use crate::types::{Base, S};

/// A whole number and a part written on their own, `2 3/4` or `-2 3/4`, as
/// the exact number they come to, `2.75`. The part is less than one. `None`
/// if the text is anything else.
pub fn parse_mixed(text: &str) -> Option<Dec> {
    let toks = lex(text, 0, 0).ok()?;
    let toks: Vec<&Tok> = toks.iter().map(|t| &t.tok).collect();
    let (negative, rest) = match toks.as_slice() {
        [Tok::Minus, rest @ ..] => (true, rest),
        rest => (false, rest),
    };
    let [
        Tok::Int(whole, Base::Dec),
        Tok::Int(n, Base::Dec),
        Tok::Slash,
        Tok::Int(d, Base::Dec),
        Tok::Eof,
    ] = rest
    else {
        return None;
    };
    let part = (n < d)
        .then(|| Dec::from_i64(*n).checked_div(Dec::from_i64(*d)))
        .flatten()?;
    let r = Dec::from_i64(*whole).checked_add(part)?;
    if negative { r.checked_neg() } else { Some(r) }
}

/// A value written on its own, such as `42`, `-1/7`, `2 3/4`, `3+4i` or
/// `2025-01-31`. `None` if the text is anything else.
pub fn parse_literal(text: &str) -> Option<Lit> {
    if let Some(r) = parse_mixed(text) {
        return Some(Lit::Decimal(r));
    }
    let toks = lex(text, 0, 0).ok()?;
    let toks: Vec<&Tok> = toks.iter().map(|t| &t.tok).collect();
    // A fraction of two whole numbers, `1/7`, is an exact number.
    let fraction = |n: i64, d: i64| {
        Dec::from_i64(n)
            .checked_div(Dec::from_i64(d))
            .map(Lit::Decimal)
    };
    Some(match toks.as_slice() {
        [
            Tok::Int(n, Base::Dec),
            Tok::Slash,
            Tok::Int(d, Base::Dec),
            Tok::Eof,
        ] => fraction(*n, *d)?,
        [
            Tok::Minus,
            Tok::Int(n, Base::Dec),
            Tok::Slash,
            Tok::Int(d, Base::Dec),
            Tok::Eof,
        ] => fraction(-*n, *d)?,
        [Tok::Int(n, b), Tok::Eof] => Lit::Int(*n, *b),
        [Tok::Decimal(r), Tok::Eof] => Lit::Decimal(*r),
        [Tok::Percent(r), Tok::Eof] => Lit::Percent(*r),
        [Tok::Float(f), Tok::Eof] => Lit::Float(*f),
        [Tok::Minus, Tok::Int(n, b), Tok::Eof] => Lit::Int(-*n, *b),
        [Tok::Minus, Tok::Decimal(r), Tok::Eof] => Lit::Decimal(r.checked_neg()?),
        [Tok::Minus, Tok::Percent(r), Tok::Eof] => Lit::Percent(r.checked_neg()?),
        [Tok::Minus, Tok::Float(f), Tok::Eof] => Lit::Float(-*f),
        [Tok::Str(s), Tok::Eof] => Lit::String(s.clone()),
        [Tok::Date(d), Tok::Eof] => Lit::Date(*d),
        [Tok::Time(t), Tok::Eof] => Lit::Time(*t),
        [Tok::DateTime(t), Tok::Eof] => Lit::DateTime(*t),
        [Tok::Duration(t), Tok::Eof] => Lit::Duration(*t),
        [Tok::Minus, Tok::Duration(t), Tok::Eof] => Lit::Duration(t.checked_neg()?),
        [Tok::True, Tok::Eof] => Lit::Boolean(true),
        [Tok::False, Tok::Eof] => Lit::Boolean(false),
        _ => return None,
    })
}

pub fn lit_type(l: &Lit) -> S {
    match l {
        Lit::Int(_, b) => b.ty(),
        Lit::Decimal(_) => S::Decimal,
        Lit::Percent(_) => S::Percent,
        Lit::Float(_) => S::Float,
        Lit::String(_) => S::String,
        Lit::Boolean(_) => S::Boolean,
        Lit::Date(_) => S::Date,
        Lit::Time(_) => S::Time,
        Lit::DateTime(_) => S::DateTime,
        Lit::Duration(_) => S::Duration,
    }
}

/// The name of a type with `a` or `an` before it.
fn a(to: S) -> String {
    let article = if matches!(to, S::Int | S::Octal | S::Email) {
        "an"
    } else {
        "a"
    };
    format!("{article} {to}")
}

/// A value as the type `to`, as the conversion of that name gives it. A
/// fraction loses its fractional part as an `Integer`; text is read the way a
/// cell is. The error says why it cannot be done. Writing a value as `Text`
/// is not done here: how it is written is the engine's business.
pub fn convert(lit: &Lit, to: S) -> Result<Lit, String> {
    let from = lit_type(lit);
    let no = || Err(format!("{from} cannot be made {}", a(to)));
    let real = |l: &Lit| match l {
        Lit::Int(n, _) => Some(*n as f64),
        Lit::Decimal(r) | Lit::Percent(r) => Some(r.to_f64()),
        Lit::Float(f) => Some(*f),
        Lit::Boolean(b) => Some(*b as u8 as f64),
        _ => None,
    };
    if from == to || to == S::Any {
        return Ok(lit.clone());
    }
    // The bases hold the same whole numbers, so a number goes through
    // decimal to be shown in another one.
    if let Lit::Int(n, b) = lit
        && *b != Base::Dec
    {
        return convert(&Lit::Int(*n, Base::Dec), to);
    }
    let base = Base::of(to);
    if base != Base::Dec {
        return match convert(lit, S::Int)? {
            Lit::Int(n, _) => Ok(Lit::Int(n, base)),
            _ => no(),
        };
    }
    Ok(match (lit, to) {
        // A narrower kind of text is the text itself, once it reads as one.
        // Surrounding space goes, as it does when text is read as a number.
        (Lit::String(s), S::Url | S::Email | S::Uuid) => {
            let s = s.trim();
            let reads = match to {
                S::Url => crate::text::is_url(s),
                S::Email => crate::text::is_email(s),
                _ => crate::text::is_uuid(s),
            };
            if !reads {
                return Err(format!("`{s}` is not {}", a(to)));
            }
            Lit::String(s.to_string())
        }
        (Lit::String(s), _) => {
            let read = parse_literal(s.trim()).filter(|l| !matches!(l, Lit::String(_)));
            return read
                .and_then(|l| convert(&l, to).ok())
                .ok_or_else(|| format!("`{s}` is not {}", a(to)));
        }
        // A `Percent` is the number it stands for: `20%` is `0.2`.
        (Lit::Percent(r), S::Int | S::Decimal | S::Boolean) => {
            let plain = match r.to_i64() {
                Some(n) => Lit::Int(n, Base::Dec),
                None => Lit::Decimal(*r),
            };
            return convert(&plain, to);
        }
        (Lit::Int(n, _), S::Percent) => Lit::Percent(Dec::from_i64(*n)),
        (Lit::Decimal(r), S::Percent) => Lit::Percent(*r),
        (Lit::Int(n, _), S::Decimal) => Lit::Int(*n, Base::Dec),
        (Lit::Decimal(r), S::Int) => match r.trunc().to_i64() {
            Some(n) => Lit::Int(n, Base::Dec),
            None => return Err("the number is too large for an Integer".to_string()),
        },
        (Lit::Float(f), S::Int | S::Decimal | S::Percent) => match Dec::from_f64(*f) {
            Some(r) if to == S::Percent => Lit::Percent(r),
            Some(r) if to == S::Int => match r.trunc().to_i64() {
                Some(n) => Lit::Int(n, Base::Dec),
                None => return Err("the number is too large for an Integer".to_string()),
            },
            Some(r) => match r.to_i64() {
                Some(n) => Lit::Int(n, Base::Dec),
                None => Lit::Decimal(r),
            },
            None => {
                let name = match f {
                    f if f.is_nan() => "NaN",
                    f if *f > 0.0 => "Inf",
                    _ => "-Inf",
                };
                return Err(format!("{name} has no exact value"));
            }
        },
        (Lit::Boolean(b), S::Int | S::Decimal) => Lit::Int(*b as i64, Base::Dec),
        (Lit::Int(..) | Lit::Decimal(_) | Lit::Percent(_) | Lit::Boolean(_), S::Float) => {
            match real(lit) {
                Some(f) => Lit::Float(f),
                None => return no(),
            }
        }
        (Lit::Int(n, _), S::Boolean) => Lit::Boolean(*n != 0),
        (Lit::Decimal(r), S::Boolean) => Lit::Boolean(!r.is_zero()),
        (Lit::Float(f), S::Boolean) => Lit::Boolean(*f != 0.0),
        (Lit::DateTime(t), S::Date) => Lit::Date(crate::date::split_datetime(*t).0),
        (Lit::DateTime(t), S::Time) => Lit::Time(crate::date::split_datetime(*t).1),
        (Lit::Date(d), S::DateTime) => Lit::DateTime(*d as i64 * crate::date::DAY_MICROS),
        _ => return no(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPES: [S; 15] = [
        S::Int,
        S::Hex,
        S::Octal,
        S::Binary,
        S::Decimal,
        S::Percent,
        S::Float,
        S::String,
        S::Boolean,
        S::Date,
        S::Time,
        S::DateTime,
        S::Url,
        S::Email,
        S::Uuid,
    ];

    fn lit(text: &str) -> Lit {
        parse_literal(text).unwrap()
    }

    #[test]
    fn converts_numbers() {
        assert_eq!(convert(&lit("19.99"), S::Int), Ok(lit("19")));
        assert_eq!(convert(&lit("-19.99"), S::Int), Ok(lit("-19")));
        assert_eq!(convert(&lit("1e-1"), S::Decimal), Ok(lit("0.1")));
        assert_eq!(convert(&lit("1/8"), S::Decimal), Ok(lit("0.125")));
        assert_eq!(convert(&lit("1/3"), S::Decimal), Ok(lit("1/3")));
        // A fraction of whole numbers is an exact `Decimal`.
        assert_eq!(
            lit("7/3"),
            Lit::Decimal(Dec::from_i64(7).checked_div(Dec::from_i64(3)).unwrap())
        );
        assert_eq!(convert(&lit("-22/7"), S::Int), Ok(lit("-3")));
        // A whole number and a part is the fraction they come to.
        assert_eq!(lit("2 3/4"), lit("11/4"));
        assert_eq!(lit("-2 3/4"), lit("-11/4"));
        assert_eq!(lit("0 1/3"), lit("1/3"));
        assert_eq!(lit("1_000 1/3"), lit("3001/3"));
        assert_eq!(parse_literal("2 7/4"), None);
        assert_eq!(parse_literal("2 3/0"), None);
        assert_eq!(parse_literal("2 3"), None);
        assert_eq!(parse_literal("1 2 3/4"), None);
        assert_eq!(convert(&lit("1.5e3"), S::Int), Ok(lit("1500")));
        assert_eq!(convert(&lit("1/4"), S::Float), Ok(Lit::Float(0.25)));
        assert_eq!(convert(&lit("true"), S::Int), Ok(lit("1")));
        assert_eq!(convert(&lit("0.5"), S::Boolean), Ok(lit("true")));
        assert!(convert(&Lit::Float(f64::NAN), S::Int).is_err());
        // The bases hold the same whole numbers.
        assert_eq!(convert(&lit("255"), S::Hex), Ok(Lit::Int(255, Base::Hex)));
        assert_eq!(convert(&lit("0xff"), S::Int), Ok(lit("255")));
        assert_eq!(convert(&lit("0xff"), S::Binary), Ok(lit("0b11111111")));
        assert_eq!(convert(&lit("19.99"), S::Octal), Ok(lit("0o23")));
        assert_eq!(lit("-0xff"), Lit::Int(-255, Base::Hex));
        assert_eq!(lit_type(&lit("0b101")), S::Binary);
    }

    #[test]
    fn reads_text() {
        let text = |s: &str| Lit::String(s.to_string());
        assert_eq!(convert(&text(" 42 "), S::Int), Ok(lit("42")));
        assert_eq!(convert(&text("19.99"), S::Int), Ok(lit("19")));
        assert_eq!(convert(&text("20%"), S::Decimal), Ok(lit("1/5")));
        assert_eq!(convert(&text("16:10"), S::Time), Ok(lit("16:10")));
        assert_eq!(convert(&text("2025-01-31"), S::Date), Ok(lit("2025-01-31")));
        assert_eq!(
            convert(&text("abc"), S::Int),
            Err("`abc` is not an Integer".to_string())
        );
        assert!(convert(&text("2025-01-31"), S::Int).is_err());
    }

    #[test]
    fn reads_the_narrower_kinds_of_text() {
        let text = |s: &str| Lit::String(s.to_string());
        let same = |s: &str, to| convert(&text(s), to) == Ok(text(s));
        assert!(same("https://example.com", S::Url));
        assert!(same("name@example.com", S::Email));
        assert!(same("7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f", S::Uuid));
        // Surrounding space goes, as it does when text is read as a number.
        assert_eq!(
            convert(&text(" name@example.com "), S::Email),
            Ok(text("name@example.com"))
        );
        assert_eq!(
            convert(&text("name"), S::Email),
            Err("`name` is not an Email".to_string())
        );
        assert_eq!(
            convert(&text("example.com"), S::Url),
            Err("`example.com` is not a Url".to_string())
        );
        // One kind of text is not another, and a number is none of them.
        assert!(convert(&text("https://example.com"), S::Email).is_err());
        assert!(convert(&lit("42"), S::Email).is_err());
        // Any of them is text, which is the engine's to write.
        assert_eq!(
            convert(&text("name@example.com"), S::String),
            Ok(text("name@example.com"))
        );
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
            lit("0xff"),
            lit("0.5"),
            // A fraction that ends: whether a `Fraction` is a `Decimal` depends
            // on the number, as whether text is a number depends on the text.
            lit("1/4"),
            lit("12.5%"),
            lit("1e-1"),
            lit("true"),
            lit("2025-01-31"),
            lit("09:30"),
            lit("2025-01-31T09:30"),
        ];
        for sample in &samples {
            let from = lit_type(sample);
            for to in TYPES {
                // Text is another matter: it depends on what the text says.
                if to == S::String {
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

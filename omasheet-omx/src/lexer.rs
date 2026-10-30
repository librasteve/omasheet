// Copyright (c) 2026 Stephen Roe

//! Tokens for OMX expressions.

use crate::date;
use crate::decimal::Dec;
use crate::diag::{Diagnostic, Span};
use crate::types::Base;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// A whole number, and the base it was written in.
    Int(i64, Base),
    Decimal(Dec),
    /// A number with a `%`, already divided by a hundred.
    Percent(Dec),
    Float(f64),
    Str(String),
    /// Days since 1970-01-01.
    Date(i32),
    /// Nanoseconds since midnight, as Polars holds a `Time`.
    Time(i64),
    /// Microseconds since the start of 1970-01-01, on the clocks of the
    /// file's own zone, as Polars holds a `Datetime`.
    DateTime(i64),
    Ident(String),
    If,
    Then,
    Else,
    And,
    Or,
    Not,
    In,
    True,
    False,
    Plus,
    Minus,
    /// `+&`, `+|` and `+^`: bitwise and, or and xor, as Raku writes them;
    /// `+^` before a number is its complement.
    PlusAmp,
    PlusPipe,
    PlusCaret,
    /// `+<` and `+>`: the shifts.
    PlusLt,
    PlusGt,
    Star,
    Slash,
    Pow,
    EqEq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    Assign,
    /// `:=`, which names an aggregate in `agg`.
    Define,
    SlashSlash,
    /// `...`, the sequence operator.
    DotDotDot,
    /// `$`, which starts a back-reference such as `$-1`.
    Dollar,
    DotDot,
    DotDotCaret,
    CaretDotDot,
    CaretDotDotCaret,
    Caret,
    Pipe,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Dot,
    Eof,
}

impl Tok {
    pub fn describe(&self) -> String {
        match self {
            Tok::Int(..) | Tok::Decimal(_) | Tok::Percent(_) | Tok::Float(_) => "a number".into(),
            Tok::Str(_) => "a string".into(),
            Tok::Date(_) => "a date".into(),
            Tok::Time(_) => "a time".into(),
            Tok::DateTime(_) => "a date-time".into(),
            Tok::Ident(name) => format!("`{name}`"),
            Tok::Eof => "the end of the expression".into(),
            other => format!("`{}`", other.text()),
        }
    }

    fn text(&self) -> &'static str {
        match self {
            Tok::If => "if",
            Tok::Then => "then",
            Tok::Else => "else",
            Tok::And => "and",
            Tok::Or => "or",
            Tok::Not => "not",
            Tok::In => "in",
            Tok::True => "true",
            Tok::False => "false",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::PlusAmp => "+&",
            Tok::PlusPipe => "+|",
            Tok::PlusCaret => "+^",
            Tok::PlusLt => "+<",
            Tok::PlusGt => "+>",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Pow => "**",
            Tok::EqEq => "==",
            Tok::NotEq => "!=",
            Tok::Lt => "<",
            Tok::Gt => ">",
            Tok::Le => "<=",
            Tok::Ge => ">=",
            Tok::Assign => "=",
            Tok::Define => ":=",
            Tok::SlashSlash => "//",
            Tok::DotDotDot => "...",
            Tok::Dollar => "$",
            Tok::DotDot => "..",
            Tok::DotDotCaret => "..^",
            Tok::CaretDotDot => "^..",
            Tok::CaretDotDotCaret => "^..^",
            Tok::Caret => "^",
            Tok::Pipe => "|>",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::Comma => ",",
            Tok::Semi => ";",
            Tok::Dot => ".",
            _ => "",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

/// Lex `text`, which starts at byte `base` of source `src`.
pub fn lex(text: &str, src: u32, base: usize) -> Result<Vec<Token>, Diagnostic> {
    let b = text.as_bytes();
    let span = |s: usize, e: usize| Span::new(src, base + s, base + e);
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        if let Some(found) = based(text, i) {
            let (tok, end) =
                found.map_err(|(msg, s, e)| Diagnostic::new(span(s, e), msg))?;
            out.push(Token {
                tok,
                span: span(start, end),
            });
            i = end;
            continue;
        }
        if c.is_ascii_digit() {
            let (tok, end) = number(text, i).map_err(|(msg, s, e)| {
                let d = Diagnostic::new(span(s, e), msg);
                if text[s..e].chars().any(|ch| ch.is_ascii_alphabetic()) {
                    d.with_help("units arrive in a later phase of Omasheet")
                } else {
                    d
                }
            })?;
            out.push(Token {
                tok,
                span: span(start, end),
            });
            i = end;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &text[start..i];
            let tok = match word {
                "if" => Tok::If,
                "then" => Tok::Then,
                "else" => Tok::Else,
                "and" => Tok::And,
                "or" => Tok::Or,
                "not" => Tok::Not,
                "in" => Tok::In,
                "true" => Tok::True,
                "false" => Tok::False,
                "Inf" | "NaN" => {
                    let f = if word == "Inf" {
                        f64::INFINITY
                    } else {
                        f64::NAN
                    };
                    Tok::Float(f)
                }
                _ => Tok::Ident(word.to_string()),
            };
            out.push(Token {
                tok,
                span: span(start, i),
            });
            continue;
        }
        if c == b'"' {
            let mut s = String::new();
            i += 1;
            loop {
                let Some(ch) = text[i..].chars().next() else {
                    return Err(Diagnostic::new(
                        span(start, b.len()),
                        "this string is never closed",
                    ));
                };
                i += ch.len_utf8();
                match ch {
                    '"' => break,
                    '\\' => {
                        let Some(esc) = text[i..].chars().next() else {
                            continue;
                        };
                        i += esc.len_utf8();
                        s.push(match esc {
                            'n' => '\n',
                            't' => '\t',
                            '"' => '"',
                            '\\' => '\\',
                            other => {
                                return Err(Diagnostic::new(
                                    span(i - other.len_utf8() - 1, i),
                                    format!("unknown escape `\\{other}`"),
                                )
                                .with_help("the escapes are \\\" \\\\ \\n and \\t"));
                            }
                        });
                    }
                    other => s.push(other),
                }
            }
            out.push(Token {
                tok: Tok::Str(s),
                span: span(start, i),
            });
            continue;
        }
        let rest = &text[i..];
        // A bitwise operator is `+` and its symbol with nothing between:
        // `a + ^5` is a sum and a range.
        let bitwise = match rest.as_bytes().get(1).filter(|_| c == b'+') {
            Some(b'&') => Some(Tok::PlusAmp),
            Some(b'|') => Some(Tok::PlusPipe),
            Some(b'^') => Some(Tok::PlusCaret),
            Some(b'<') => Some(Tok::PlusLt),
            Some(b'>') => Some(Tok::PlusGt),
            _ => None,
        };
        let (tok, len) = if let Some(tok) = bitwise {
            (tok, 2)
        } else if rest.starts_with("^..^") {
            (Tok::CaretDotDotCaret, 4)
        } else if rest.starts_with("^..") {
            (Tok::CaretDotDot, 3)
        } else if rest.starts_with("...") {
            (Tok::DotDotDot, 3)
        } else if rest.starts_with("..^") {
            (Tok::DotDotCaret, 3)
        } else if rest.starts_with("..") {
            (Tok::DotDot, 2)
        } else if rest.starts_with("**") {
            (Tok::Pow, 2)
        } else if rest.starts_with("==") {
            (Tok::EqEq, 2)
        } else if rest.starts_with("!=") {
            (Tok::NotEq, 2)
        } else if rest.starts_with("<=") {
            (Tok::Le, 2)
        } else if rest.starts_with(">=") {
            (Tok::Ge, 2)
        } else if rest.starts_with("//") {
            (Tok::SlashSlash, 2)
        } else if rest.starts_with("|>") {
            (Tok::Pipe, 2)
        } else if rest.starts_with(":=") {
            (Tok::Define, 2)
        } else {
            let tok = match c {
                b'+' => Tok::Plus,
                b'-' => Tok::Minus,
                b'*' => Tok::Star,
                b'/' => Tok::Slash,
                b'<' => Tok::Lt,
                b'>' => Tok::Gt,
                b'=' => Tok::Assign,
                b'(' => Tok::LParen,
                b')' => Tok::RParen,
                b'[' => Tok::LBracket,
                b']' => Tok::RBracket,
                b',' => Tok::Comma,
                b';' => Tok::Semi,
                b'.' => Tok::Dot,
                b'^' => Tok::Caret,
                b'$' => Tok::Dollar,
                _ => {
                    let ch = rest.chars().next().unwrap();
                    return Err(Diagnostic::new(
                        span(i, i + ch.len_utf8()),
                        format!("unexpected character `{ch}`"),
                    ));
                }
            };
            (tok, 1)
        };
        out.push(Token {
            tok,
            span: span(start, start + len),
        });
        i += len;
    }
    out.push(Token {
        tok: Tok::Eof,
        span: span(b.len(), b.len()),
    });
    Ok(out)
}

type NumErr = (String, usize, usize);

/// Lex a whole number written in hex, octal or binary, `0xff`, `0o17` or
/// `0b101`, if one starts at `i`. The prefix and the digits may be in either
/// case, and a `_` may stand among the digits (D131).
fn based(text: &str, i: usize) -> Option<Result<(Tok, usize), NumErr>> {
    let b = text.as_bytes();
    if b[i] != b'0' || i + 1 >= b.len() {
        return None;
    }
    let (base, name) = match b[i + 1].to_ascii_lowercase() {
        b'x' => (Base::Hex, "hex"),
        b'o' => (Base::Oct, "octal"),
        b'b' => (Base::Bin, "binary"),
        _ => return None,
    };
    let mut j = i + 2;
    let mut n: i64 = 0;
    let mut digits = 0;
    let mut too_large = false;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
        if b[j] != b'_' {
            let Some(d) = (b[j] as char).to_digit(base.radix()) else {
                let ch = b[j] as char;
                return Some(Err((format!("`{ch}` is not a {name} digit"), j, j + 1)));
            };
            digits += 1;
            match n.checked_mul(base.radix() as i64).and_then(|m| m.checked_add(d as i64)) {
                Some(m) => n = m,
                None => too_large = true,
            }
        }
        j += 1;
    }
    let written = &text[i..j];
    Some(if digits == 0 {
        Err((format!("`{written}` has no {name} digits"), i, j))
    } else if too_large {
        Err((format!("`{written}` is too large a number"), i, j))
    } else {
        Ok((Tok::Int(n, base), j))
    })
}

/// Lex a number, date, time or date-time starting at `i`. Returns the token and its
/// end.
fn number(text: &str, i: usize) -> Result<(Tok, usize), NumErr> {
    let b = text.as_bytes();
    let digit = |k: usize| k < b.len() && b[k].is_ascii_digit();
    let word = |k: usize| k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'_');

    // A date: dddd-dd-dd, or a date-time: dddd-dd-ddThh:mm or …Thh:mm:ss.
    let is_date = (0..10).all(|k| match k {
        4 | 7 => i + k < b.len() && b[i + k] == b'-',
        _ => digit(i + k),
    });
    let at = |k: usize, c: u8| k < b.len() && b[k] == c;
    let pair = |k: usize| digit(k) && digit(k + 1);
    // How far the digits of a fraction of a second run from `k`, and what
    // they come to in a unit of `places` digits. `None` where there is no
    // fraction; the error where there are more digits than the unit keeps.
    let fraction = |k: usize, places: u32, held: &str| -> Option<Result<(i64, usize), NumErr>> {
        if !(at(k, b'.') && digit(k + 1)) {
            return None;
        }
        let start = k + 1;
        let mut end = start;
        while digit(end) {
            end += 1;
        }
        let digits = &text[start..end];
        if digits.len() > places as usize {
            return Some(Err((
                format!(
                    "a {held} holds {places} digits of a second, so `.{digits}` is more of one than it can keep"
                ),
                k,
                end,
            )));
        }
        let scale = 10i64.pow(places - digits.len() as u32);
        Some(Ok((digits.parse::<i64>().unwrap() * scale, end)))
    };
    let mut seconds_of_datetime = false;
    let mut datetime_frac = 0i64;
    let mut time_end =
        if is_date && at(i + 10, b'T') && pair(i + 11) && at(i + 13, b':') && pair(i + 14) {
            seconds_of_datetime = at(i + 16, b':') && pair(i + 17);
            Some(i + if seconds_of_datetime { 19 } else { 16 })
        } else {
            None
        };
    // A date-time is held to the microsecond (D68), so six digits.
    let after_seconds = seconds_of_datetime
        .then(|| fraction(i + 19, 6, "date-time"))
        .flatten();
    if let Some(found) = after_seconds {
        let (micros, end) = found?;
        datetime_frac = micros;
        time_end = Some(end);
    }
    let end = time_end.unwrap_or(i + 10);
    if is_date && !word(end) {
        let part = |s: usize, e: usize| text[i + s..i + e].parse::<i64>().unwrap();
        let Some(days) = date::from_ymd(part(0, 4), part(5, 7), part(8, 10)) else {
            return Err((
                format!("`{}` is not a real date", &text[i..i + 10]),
                i,
                i + 10,
            ));
        };
        if time_end.is_none() {
            return Ok((Tok::Date(days), end));
        }
        let secs = if seconds_of_datetime { part(17, 19) } else { 0 };
        return match date::datetime_from(days, part(11, 13), part(14, 16), secs) {
            Some(t) => Ok((Tok::DateTime(t + datetime_frac), end)),
            None => Err((
                format!("`{}` is not a real time", &text[i + 11..end]),
                i + 11,
                end,
            )),
        };
    }

    // A time: hh:mm or hh:mm:ss, either with a fraction of a second.
    if pair(i) && at(i + 2, b':') && pair(i + 3) {
        let seconds = at(i + 5, b':') && pair(i + 6);
        let mut end = i + if seconds { 8 } else { 5 };
        // A time of day is held in nanoseconds (D68), so nine digits.
        let mut frac = 0i64;
        let after_seconds = seconds.then(|| fraction(end, 9, "time")).flatten();
        if let Some(found) = after_seconds {
            let (nanos, after) = found?;
            frac = nanos;
            end = after;
        }
        if !word(end) && !at(end, b':') {
            let part = |s: usize| text[i + s..i + s + 2].parse::<i64>().unwrap();
            let secs = if seconds { part(6) } else { 0 };
            return match date::time_from(part(0), part(3), secs) {
                Some(t) => Ok((Tok::Time(t + frac), end)),
                None => Err((format!("`{}` is not a real time", &text[i..end]), i, end)),
            };
        }
    }

    let mut j = i;
    while digit(j) || (j < b.len() && b[j] == b'_') {
        j += 1;
    }
    let mut frac_digits = 0;
    let mut is_decimal = false;
    if j < b.len() && b[j] == b'.' && digit(j + 1) {
        is_decimal = true;
        j += 1;
        while digit(j) || (j < b.len() && b[j] == b'_') {
            if b[j] != b'_' {
                frac_digits += 1;
            }
            j += 1;
        }
    }
    let mut is_float = false;
    if j < b.len() && (b[j] == b'e' || b[j] == b'E') {
        let mut k = j + 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
        if digit(k) {
            while digit(k) {
                k += 1;
            }
            is_float = true;
            j = k;
        }
    }
    let raw: String = text[i..j].chars().filter(|&c| c != '_').collect();
    let percent = j < b.len() && b[j] == b'%';
    let end = if percent { j + 1 } else { j };
    if word(end) {
        let mut k = end;
        while word(k) {
            k += 1;
        }
        return Err((format!("`{}` is not a number", &text[i..k]), i, k));
    }

    let tok = if is_float {
        let f: f64 = raw
            .parse()
            .map_err(|_| (format!("`{raw}` is not a number"), i, j))?;
        Tok::Float(if percent { f / 100.0 } else { f })
    } else if is_decimal || percent {
        let digits: String = raw.chars().filter(|&c| c != '.').collect();
        let scale = frac_digits + if percent { 2 } else { 0 };
        let unscaled: i128 = digits
            .parse()
            .map_err(|_| (format!("`{raw}` is too large a number"), i, j))?;
        let d = Dec::new(unscaled, scale)
            .ok_or_else(|| (format!("`{raw}` is too large a number"), i, j))?;
        if percent {
            Tok::Percent(d)
        } else {
            Tok::Decimal(d)
        }
    } else {
        // A whole number past `i64` is still exact as a decimal.
        match raw.parse::<i64>() {
            Ok(n) => Tok::Int(n, Base::Dec),
            Err(_) => {
                let unscaled: i128 = raw
                    .parse()
                    .map_err(|_| (format!("`{raw}` is too large a number"), i, j))?;
                let d = Dec::new(unscaled, 0)
                    .ok_or_else(|| (format!("`{raw}` is too large a number"), i, j))?;
                Tok::Decimal(d)
            }
        }
    };
    Ok((tok, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        lex(s, 0, 0).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn a_fraction_of_a_second() {
        // A time is nanoseconds of day, so nine digits of a second.
        assert_eq!(
            toks("09:30:00.5")[0],
            Tok::Time(date::time_from(9, 30, 0).unwrap() + 500_000_000)
        );
        assert_eq!(toks("00:00:00.000000001")[0], Tok::Time(1));
        // A date-time is microseconds, so six.
        let day = date::from_ymd(2025, 1, 31).unwrap();
        let noon = date::datetime_from(day, 12, 0, 0).unwrap();
        assert_eq!(
            toks("2025-01-31T12:00:00.5")[0],
            Tok::DateTime(noon + 500_000)
        );
        assert_eq!(
            toks("2025-01-31T12:00:00.000001")[0],
            Tok::DateTime(noon + 1)
        );
        // Past what the unit keeps is said rather than quietly dropped.
        let why = |text: &str| lex(text, 0, 0).unwrap_err().message;
        let said = why("2025-01-31T12:00:00.0000001");
        assert!(said.contains("a date-time holds 6 digits"), "{said}");
        let said = why("09:30:00.0000000001");
        assert!(said.contains("a time holds 9 digits"), "{said}");
        // The fraction goes with the seconds, so this is not one time.
        assert!(toks("09:30.5").len() > 2);
    }

    #[test]
    fn numbers() {
        assert_eq!(toks("42")[0], Tok::Int(42, Base::Dec));
        assert_eq!(
            toks("1_000_000.50")[0],
            Tok::Decimal(Dec::parse("1000000.50").unwrap())
        );
        assert_eq!(toks("20%")[0], Tok::Percent(Dec::parse("0.2").unwrap()));
        assert_eq!(toks("1e-100")[0], Tok::Float(1e-100));
        assert_eq!(
            toks("0..2"),
            vec![
                Tok::Int(0, Base::Dec),
                Tok::DotDot,
                Tok::Int(2, Base::Dec),
                Tok::Eof
            ]
        );
        assert!(lex("10m", 0, 0).is_err());
    }

    #[test]
    fn digit_separators() {
        // A `_` may stand anywhere in a number's digits, but not its exponent.
        assert_eq!(toks("10_00_000")[0], Tok::Int(1_000_000, Base::Dec));
        assert_eq!(toks("1_0")[0], Tok::Int(10, Base::Dec));
        assert_eq!(toks("0xFF_FF")[0], Tok::Int(65535, Base::Hex));
        assert_eq!(
            toks("0.000_1")[0],
            Tok::Decimal(Dec::parse("0.0001").unwrap())
        );
        assert!(lex("1e1_0", 0, 0).is_err());
    }

    #[test]
    fn bitwise_operators() {
        assert_eq!(
            toks("a +& b +| c +^ d +< 1 +> 2"),
            vec![
                Tok::Ident("a".into()),
                Tok::PlusAmp,
                Tok::Ident("b".into()),
                Tok::PlusPipe,
                Tok::Ident("c".into()),
                Tok::PlusCaret,
                Tok::Ident("d".into()),
                Tok::PlusLt,
                Tok::Int(1, Base::Dec),
                Tok::PlusGt,
                Tok::Int(2, Base::Dec),
                Tok::Eof
            ]
        );
        // With a space it is a sum and a range, as before.
        assert_eq!(toks("a + ^5")[1..3], [Tok::Plus, Tok::Caret]);
    }

    #[test]
    fn based_integers() {
        assert_eq!(toks("0xFF")[0], Tok::Int(255, Base::Hex));
        assert_eq!(toks("0Xff")[0], Tok::Int(255, Base::Hex));
        assert_eq!(toks("0o17")[0], Tok::Int(15, Base::Oct));
        assert_eq!(toks("0b101")[0], Tok::Int(5, Base::Bin));
        // A leading `0` alone is still decimal (D130).
        assert_eq!(toks("017")[0], Tok::Int(17, Base::Dec));
        assert_eq!(toks("0")[0], Tok::Int(0, Base::Dec));
        // The error is at the digit outside the base.
        let bad = lex("0b102", 0, 0).unwrap_err();
        assert_eq!((bad.span.start, bad.span.end), (4, 5));
        assert!(bad.message.contains("`2` is not a binary digit"));
        assert!(lex("0x", 0, 0).is_err());
        assert!(lex("0x_", 0, 0).is_err());
        assert!(lex("0x8000000000000000", 0, 0).is_err());
        assert_eq!(toks("0x7fffffffffffffff")[0], Tok::Int(i64::MAX, Base::Hex));
    }

    #[test]
    fn dates_and_operators() {
        assert_eq!(
            toks("2025-01-01")[0],
            Tok::Date(date::from_ymd(2025, 1, 1).unwrap())
        );
        assert!(lex("2025-02-30", 0, 0).is_err());
        let day = date::from_ymd(2025, 1, 1).unwrap();
        assert_eq!(
            toks("2025-01-01T09:30")[0],
            Tok::DateTime(date::datetime_from(day, 9, 30, 0).unwrap())
        );
        assert_eq!(
            toks("2025-01-01T09:30:15")[0],
            Tok::DateTime(date::datetime_from(day, 9, 30, 15).unwrap())
        );
        assert!(lex("2025-01-01T24:00", 0, 0).is_err());
        assert!(lex("2025-01-01T09", 0, 0).is_err());
        assert!(lex("2025-01-01T09:30:1", 0, 0).is_err());
        assert_eq!(
            toks("09:30")[0],
            Tok::Time((9 * 3600 + 30 * 60) * date::NANOS)
        );
        assert_eq!(toks("23:59:59")[0], Tok::Time(86_399 * date::NANOS));
        assert!(lex("24:00", 0, 0).is_err());
        assert!(lex("9:30", 0, 0).is_err());
        assert!(lex("09:30:1", 0, 0).is_err());
        assert_eq!(toks("a // b |> c ..^ d # note").len(), 8);
    }
}

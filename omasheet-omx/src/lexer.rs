// Copyright (c) 2026 Stephen Roe

//! Tokens for OMX expressions.

use crate::date;
use crate::diag::{Diagnostic, Span};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::pow::Pow;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Int(BigInt),
    Rat(BigRational),
    Num(f64),
    /// An imaginary number, `4i`.
    Imag(f64),
    Str(String),
    Date(i32),
    Time(i32),
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
    SlashSlash,
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
            Tok::Int(_) | Tok::Rat(_) | Tok::Num(_) | Tok::Imag(_) => "a number".into(),
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
            Tok::SlashSlash => "//",
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
        let (tok, len) = if rest.starts_with("^..^") {
            (Tok::CaretDotDotCaret, 4)
        } else if rest.starts_with("^..") {
            (Tok::CaretDotDot, 3)
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
    let time_end =
        if is_date && at(i + 10, b'T') && pair(i + 11) && at(i + 13, b':') && pair(i + 14) {
            let seconds = at(i + 16, b':') && pair(i + 17);
            Some(i + if seconds { 19 } else { 16 })
        } else {
            None
        };
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
        let secs = if end == i + 19 { part(17, 19) } else { 0 };
        return match date::datetime_from(days, part(11, 13), part(14, 16), secs) {
            Some(t) => Ok((Tok::DateTime(t), end)),
            None => Err((
                format!("`{}` is not a real time", &text[i + 11..end]),
                i + 11,
                end,
            )),
        };
    }

    // A time: hh:mm or hh:mm:ss.
    if pair(i) && at(i + 2, b':') && pair(i + 3) {
        let seconds = at(i + 5, b':') && pair(i + 6);
        let end = i + if seconds { 8 } else { 5 };
        if !word(end) && !at(end, b':') {
            let part = |s: usize| text[i + s..i + s + 2].parse::<i64>().unwrap();
            let secs = if seconds { part(6) } else { 0 };
            return match date::time_from(part(0), part(3), secs) {
                Some(t) => Ok((Tok::Time(t), end)),
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
    // An imaginary number: `4i`, `2.5i`.
    if !percent && j < b.len() && b[j] == b'i' && !word(j + 1) {
        let f: f64 = raw
            .parse()
            .map_err(|_| (format!("`{raw}i` is not a number"), i, j + 1))?;
        return Ok((Tok::Imag(f), j + 1));
    }
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
        Tok::Num(if percent { f / 100.0 } else { f })
    } else if is_decimal || percent {
        let digits: String = raw.chars().filter(|&c| c != '.').collect();
        let numer: BigInt = digits.parse().unwrap();
        let scale = frac_digits + if percent { 2 } else { 0 };
        Tok::Rat(BigRational::new(
            numer,
            Pow::pow(BigInt::from(10), scale as u32),
        ))
    } else {
        Tok::Int(raw.parse().unwrap())
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
    fn numbers() {
        assert_eq!(toks("42")[0], Tok::Int(42.into()));
        assert_eq!(
            toks("1_000_000.50")[0],
            Tok::Rat(BigRational::new(2_000_001.into(), 2.into()))
        );
        assert_eq!(
            toks("20%")[0],
            Tok::Rat(BigRational::new(1.into(), 5.into()))
        );
        assert_eq!(toks("1e-100")[0], Tok::Num(1e-100));
        assert_eq!(
            toks("0..2"),
            vec![
                Tok::Int(0.into()),
                Tok::DotDot,
                Tok::Int(2.into()),
                Tok::Eof
            ]
        );
        assert!(lex("10m", 0, 0).is_err());
        assert_eq!(toks("4i")[0], Tok::Imag(4.0));
        assert_eq!(
            toks("3+2.5i"),
            vec![Tok::Int(3.into()), Tok::Plus, Tok::Imag(2.5), Tok::Eof]
        );
        assert!(lex("2in", 0, 0).is_err());
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
        assert_eq!(toks("09:30")[0], Tok::Time(9 * 3600 + 30 * 60));
        assert_eq!(toks("23:59:59")[0], Tok::Time(86_399));
        assert!(lex("24:00", 0, 0).is_err());
        assert!(lex("9:30", 0, 0).is_err());
        assert!(lex("09:30:1", 0, 0).is_err());
        assert_eq!(toks("a // b |> c ..^ d # note").len(), 8);
    }
}

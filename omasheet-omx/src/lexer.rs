// Copyright (c) 2026 Stephen Roe

//! Tokens for OMX expressions.

use crate::date;
use crate::decimal::Dec;
use crate::diag::{Diagnostic, Span};
use crate::duration::{self, NotDuration};
use crate::types::Base;
use crate::units::{self, Tail};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// A whole number, and the base it was written in.
    Int(i64, Base),
    Decimal(Dec),
    /// A number with a `%`, already divided by a hundred.
    Percent(Dec),
    Float(f64),
    /// An imaginary number, `4i`.
    Imag(f64),
    /// A value and its uncertainty written together: `10.5(3)`, `7.297(11)e-3`.
    Scientific(f64, f64),
    /// A number and an SI unit written together: `5kg`, `10.5(3)e-3kg`,
    /// `9.81m.s^-2`, `20°C` (add-si-units D229). The uncertainty is zero
    /// where none was written.
    Quantity(f64, f64, siquant::Unit),
    /// `'…'`, a string in code (D168).
    Str(String),
    /// `"…"`, text as a table cell writes it. Code does not take it: the
    /// parser says to use single quotes, and a cell takes only this.
    Text(String),
    /// Days since 1970-01-01.
    Date(i32),
    /// Nanoseconds since midnight, as Polars holds a `Time`.
    Time(i64),
    /// Microseconds since the start of 1970-01-01, on the clocks of the
    /// file's own zone, as Polars holds a `Datetime`.
    Datetime(i64),
    /// Microseconds: `PT1H30M`.
    Duration(i64),
    Ident(String),
    If,
    Then,
    Else,
    And,
    Or,
    Xor,
    Nand,
    Nor,
    Not,
    In,
    True,
    False,
    Plus,
    Minus,
    /// `±`: a value and its uncertainty.
    PlusMinus,
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
    /// `%` on its own, the remainder; after a number it is a percentage.
    Rem,
    Pow,
    EqEq,
    /// `=~=`: equal to within a tolerance.
    Approx,
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
            Tok::Int(..)
            | Tok::Decimal(_)
            | Tok::Percent(_)
            | Tok::Float(_)
            | Tok::Imag(_)
            | Tok::Scientific(..) => "a number".into(),
            Tok::Quantity(..) => "a quantity".into(),
            Tok::Str(_) | Tok::Text(_) => "a string".into(),
            Tok::Date(_) => "a date".into(),
            Tok::Time(_) => "a time".into(),
            Tok::Datetime(_) => "a date-time".into(),
            Tok::Duration(_) => "a duration".into(),
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
            Tok::Xor => "xor",
            Tok::Nand => "nand",
            Tok::Nor => "nor",
            Tok::Not => "not",
            Tok::In => "in",
            Tok::True => "true",
            Tok::False => "false",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::PlusMinus => "±",
            Tok::PlusAmp => "+&",
            Tok::PlusPipe => "+|",
            Tok::PlusCaret => "+^",
            Tok::PlusLt => "+<",
            Tok::PlusGt => "+>",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Rem => "%",
            Tok::Pow => "**",
            Tok::EqEq => "==",
            Tok::Approx => "=~=",
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

/// `s` as a string in code, in single quotes (D168): what reads back as
/// `s`. Everything that writes OMX writes a string this way.
pub fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

/// Lex `text`, which starts at byte `base` of source `src`. A number may be
/// followed by an SI unit or a scale (`5kg`, `1k.`).
pub fn lex(text: &str, src: u32, base: usize) -> Result<Vec<Token>, Diagnostic> {
    lex_with(text, src, base, true)
}

/// Lex as [`lex`], but where `units` is false a number is never followed by
/// a unit or a scale: `5kg` is not a number, as before. What is imported is
/// read this way, so `12g` or `10m` in a CSV stays text (add-si-units D236).
pub fn lex_with(text: &str, src: u32, base: usize, units: bool) -> Result<Vec<Token>, Diagnostic> {
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
            // `0Bq` is zero becquerel, not a bad binary number: where what
            // follows a zero reads as a unit, that is read before the error.
            let found = match found {
                Err(e) if units => match number(text, i, units) {
                    Ok(quantity @ (Tok::Quantity(..), _)) => Ok(quantity),
                    _ => Err(e),
                },
                other => other,
            };
            let (tok, end) = found.map_err(|(msg, s, e)| Diagnostic::new(span(s, e), msg))?;
            out.push(Token {
                tok,
                span: span(start, end),
            });
            i = end;
            continue;
        }
        if c.is_ascii_digit() {
            let (tok, end) = number(text, i, units).map_err(|(msg, s, e)| {
                let d = Diagnostic::new(span(s, e), msg);
                if text[s..e].chars().any(|ch| ch.is_ascii_alphabetic()) {
                    d.with_help(
                        "a unit is written right after the number, in SI, as `5kg` or `9.81m.s^-2`",
                    )
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
        if let Some(found) = iso_duration(text, i) {
            let (tok, end) = found.map_err(|(msg, s, e)| Diagnostic::new(span(s, e), msg))?;
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
                "xor" => Tok::Xor,
                "nand" => Tok::Nand,
                "nor" => Tok::Nor,
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
                    // As an imaginary part the `i` is kept apart: `Inf\i`.
                    if b[i..].starts_with(b"\\i")
                        && !b
                            .get(i + 2)
                            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
                    {
                        i += 2;
                        Tok::Imag(f)
                    } else {
                        Tok::Float(f)
                    }
                }
                _ => Tok::Ident(word.to_string()),
            };
            out.push(Token {
                tok,
                span: span(start, i),
            });
            continue;
        }
        if c == b'\'' || c == b'"' {
            // `'…'` in code, `"…"` in a cell (D168); each escapes its own
            // quote, and either may stand in the other unescaped.
            let quote = c as char;
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
                    ch if ch == quote => break,
                    '\\' => {
                        let Some(esc) = text[i..].chars().next() else {
                            continue;
                        };
                        i += esc.len_utf8();
                        s.push(match esc {
                            'n' => '\n',
                            't' => '\t',
                            '\\' => '\\',
                            esc if esc == quote => quote,
                            other => {
                                return Err(Diagnostic::new(
                                    span(i - other.len_utf8() - 1, i),
                                    format!("unknown escape `\\{other}`"),
                                )
                                .with_help(format!("the escapes are \\{quote} \\\\ \\n and \\t")));
                            }
                        });
                    }
                    other => s.push(other),
                }
            }
            out.push(Token {
                tok: if quote == '"' {
                    Tok::Text(s)
                } else {
                    Tok::Str(s)
                },
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
        } else if rest.starts_with('±') {
            (Tok::PlusMinus, '±'.len_utf8())
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
        } else if rest.starts_with("=~=") {
            (Tok::Approx, 3)
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
                b'%' => Tok::Rem,
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
            match n
                .checked_mul(base.radix() as i64)
                .and_then(|m| m.checked_add(d as i64))
            {
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

/// Lex an ISO 8601 duration, `PT1H30M`, if one starts at `i`. A word that
/// starts with `P` but is not shaped as one, such as `Price` or `P1`, is
/// left to be a name.
fn iso_duration(text: &str, i: usize) -> Option<Result<(Tok, usize), NumErr>> {
    let b = text.as_bytes();
    let digit = |k: usize| b.get(k).is_some_and(u8::is_ascii_digit);
    if b[i] != b'P' || !(digit(i + 1) || (b.get(i + 1) == Some(&b'T') && digit(i + 2))) {
        return None;
    }
    let mut end = i;
    while end < b.len()
        && (b[end].is_ascii_alphanumeric() || b[end] == b'_' || (b[end] == b'.' && digit(end + 1)))
    {
        end += 1;
    }
    match duration::parse(&text[i..end]) {
        Ok(micros) => Some(Ok((Tok::Duration(micros), end))),
        Err(NotDuration::Says(msg)) => Some(Err((msg, i, end))),
        Err(NotDuration::Shape) => None,
    }
}

/// Lex a number, date, time or date-time starting at `i`. Returns the token and its
/// end.
fn number(text: &str, i: usize, units: bool) -> Result<(Tok, usize), NumErr> {
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
            Some(t) => Ok((Tok::Datetime(t + datetime_frac), end)),
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
    let mant_end = j;
    // The uncertainty in the last digits, `10.5(3)`: digits in brackets
    // with nothing between them and the number.
    let mut concise: Option<&str> = None;
    if j < b.len() && b[j] == b'(' {
        let mut k = j + 1;
        while digit(k) {
            k += 1;
        }
        if k > j + 1 && k < b.len() && b[k] == b')' {
            concise = Some(&text[j + 1..k]);
            j = k + 1;
        }
    }
    let exp_start = j;
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
    let raw: String = text[i..mant_end]
        .chars()
        .chain(text[exp_start..j].chars())
        .filter(|&c| c != '_')
        .collect();
    if let Some(digits) = concise {
        let exponent: i32 = match text[exp_start..j].get(1..) {
            Some(e) if is_float => e.parse().unwrap_or(0),
            _ => 0,
        };
        let value: f64 = raw
            .parse()
            .map_err(|_| (format!("`{raw}` is not a number"), i, j))?;
        let sigma: f64 = format!("{digits}e{}", exponent - frac_digits)
            .parse()
            .map_err(|_| (format!("`{raw}({digits})` is not a number"), i, j))?;
        if units
            && units::starts_one(text, j)
            && let Some((tail, end)) = units::tail(text, j)
        {
            return Ok((
                match tail {
                    Tail::Unit(unit) => Tok::Quantity(value, sigma, unit),
                    Tail::Scale { base, exp } => {
                        let m = units::power(base, exp);
                        Tok::Scientific(value * m, sigma * m)
                    }
                },
                end,
            ));
        }
        if word(j) {
            let mut k = j;
            while word(k) {
                k += 1;
            }
            return Err((format!("`{}` is not a number", &text[i..k]), i, k));
        }
        return Ok((Tok::Scientific(value, sigma), j));
    }
    let percent = j < b.len() && b[j] == b'%';
    let end = if percent { j + 1 } else { j };
    // An imaginary number: `4i`, `2.5i`.
    if !percent && j < b.len() && b[j] == b'i' && !word(j + 1) {
        let f: f64 = raw
            .parse()
            .map_err(|_| (format!("`{raw}i` is not a number"), i, j + 1))?;
        return Ok((Tok::Imag(f), j + 1));
    }
    // An angle in degrees, minutes and seconds: `45°30′30″`.
    if units
        && !percent
        && !is_float_exp(&raw)
        && let Some((unit, degrees, end)) = units::dms(text, i, j)
    {
        return Ok((Tok::Quantity(degrees, 0.0, unit), end));
    }
    if units
        && !percent
        && units::starts_one(text, j)
        && let Some((tail, uend)) = units::tail(text, j)
    {
        return match tail {
            Tail::Unit(unit) => {
                let v: f64 = raw
                    .parse()
                    .map_err(|_| (format!("`{raw}` is not a number"), i, j))?;
                Ok((Tok::Quantity(v, 0.0, unit), uend))
            }
            Tail::Scale { base, exp } => scaled(&raw, is_float, frac_digits, base, exp)
                .map(|tok| (tok, uend))
                .map_err(|said| (said, i, uend)),
        };
    }
    if word(end) {
        let mut k = end;
        while word(k) {
            k += 1;
        }
        let said = if percent {
            format!(
                "`{}` is not a number; a remainder is written with spaces, `7 % 3`",
                &text[i..k]
            )
        } else {
            format!("`{}` is not a number", &text[i..k])
        };
        return Err((said, i, k));
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

/// A number written with a scale, `1.5k.`, as the number it stands for: exact
/// for a whole number or a decimal (`1k.` is the integer 1000, `5m.` is the
/// decimal 0.005), a float for a float (add-si-units D231).
fn scaled(raw: &str, is_float: bool, frac_digits: i32, base: u8, exp: i16) -> Result<Tok, String> {
    let too_large = || format!("`{raw}` is too large a number with that scale");
    if is_float {
        let f: f64 = raw
            .parse()
            .map_err(|_| format!("`{raw}` is not a number"))?;
        return Ok(Tok::Float(f * units::power(base, exp)));
    }
    let digits: String = raw.chars().filter(|&c| c != '.').collect();
    let mut unscaled: i128 = digits.parse().map_err(|_| too_large())?;
    let mut scale = frac_digits;
    if base == 2 {
        let shift = u32::try_from(exp).map_err(|_| too_large())?;
        let factor = 2i128.checked_pow(shift).ok_or_else(too_large)?;
        unscaled = unscaled.checked_mul(factor).ok_or_else(too_large)?;
    } else {
        scale -= i32::from(exp);
        if scale < 0 {
            let factor = 10i128
                .checked_pow(scale.unsigned_abs())
                .ok_or_else(too_large)?;
            unscaled = unscaled.checked_mul(factor).ok_or_else(too_large)?;
            scale = 0;
        }
    }
    let d = Dec::new(unscaled, scale).ok_or_else(too_large)?;
    Ok(match d.to_i64() {
        Some(n) if scale == 0 => Tok::Int(n, Base::Dec),
        _ => Tok::Decimal(d),
    })
}

/// Does a number's text have an exponent (`1e3`)?
fn is_float_exp(raw: &str) -> bool {
    raw.contains(['e', 'E'])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        lex(s, 0, 0).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn strings_in_code_and_text_in_a_cell() {
        // Code quotes with `'`, a cell with `"` (D168); each escapes its own.
        assert_eq!(toks("'it\\'s'")[0], Tok::Str("it's".into()));
        assert_eq!(toks("'say \"hi\"'")[0], Tok::Str("say \"hi\"".into()));
        assert_eq!(toks("'a\\\\b\\n'")[0], Tok::Str("a\\b\n".into()));
        assert_eq!(toks("\"it's\"")[0], Tok::Text("it's".into()));
        let bad = lex("'a\\\"'", 0, 0).unwrap_err();
        assert!(bad.message.contains("unknown escape"), "{}", bad.message);
        assert!(lex("'open", 0, 0).is_err());
        // What writes code writes what reads back.
        for s in ["plain", "it's", "a\\b", "two\nlines", "say \"hi\""] {
            assert_eq!(toks(&quoted(s))[0], Tok::Str(s.into()), "{s}");
        }
    }

    #[test]
    fn an_uncertainty_in_brackets_or_after_a_plus_minus() {
        // The digits in brackets are the uncertainty in the last places.
        assert_eq!(toks("10.5(3)")[0], Tok::Scientific(10.5, 0.3));
        assert_eq!(toks("10(3)")[0], Tok::Scientific(10.0, 3.0));
        assert_eq!(toks("1.5(11)")[0], Tok::Scientific(1.5, 1.1));
        // An exponent after the brackets scales both.
        assert_eq!(
            toks("7.297_352_5643(11)e-3")[0],
            Tok::Scientific(7.2973525643e-3, 1.1e-12)
        );
        // A space, or anything but digits, makes it no such literal.
        assert_eq!(
            toks("10 (3)"),
            vec![
                Tok::Int(10, Base::Dec),
                Tok::LParen,
                Tok::Int(3, Base::Dec),
                Tok::RParen,
                Tok::Eof
            ]
        );
        assert!(
            lex("10.5(3.2)", 0, 0)
                .is_ok_and(|t| t[0].tok == Tok::Decimal(Dec::new(105, 1).unwrap()))
        );
        assert_eq!(toks("1 ± 2")[1], Tok::PlusMinus);
        // `+-` is a sum and a negation, as it always was.
        assert_eq!(toks("1+-2")[1..3], [Tok::Plus, Tok::Minus]);
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
            Tok::Datetime(noon + 500_000)
        );
        assert_eq!(
            toks("2025-01-31T12:00:00.000001")[0],
            Tok::Datetime(noon + 1)
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
        assert_eq!(toks("4i")[0], Tok::Imag(4.0));
        assert_eq!(
            toks("3+2.5i"),
            vec![Tok::Int(3, Base::Dec), Tok::Plus, Tok::Imag(2.5), Tok::Eof]
        );
        assert_eq!(toks("Inf\\i")[0], Tok::Imag(f64::INFINITY));
        assert!(lex("2in", 0, 0).is_err());
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
            Tok::Datetime(date::datetime_from(day, 9, 30, 0).unwrap())
        );
        assert_eq!(
            toks("2025-01-01T09:30:15")[0],
            Tok::Datetime(date::datetime_from(day, 9, 30, 15).unwrap())
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

    // ---- units and scales (add-si-units) ----

    fn quantity(s: &str) -> (f64, f64, String) {
        match toks(s).into_iter().next().unwrap() {
            Tok::Quantity(v, sigma, u) => (v, sigma, u.to_string()),
            other => panic!("`{s}` is {other:?}"),
        }
    }

    #[test]
    fn a_unit_after_a_number() {
        assert_eq!(quantity("5kg"), (5.0, 0.0, "kg".into()));
        assert_eq!(quantity("300cm"), (300.0, 0.0, "cm".into()));
        assert_eq!(quantity("9.81m.s^-2"), (9.81, 0.0, "m.s^-2".into()));
        assert_eq!(quantity("5kg.m^2.s^-2"), (5.0, 0.0, "kg.m^2.s^-2".into()));
        assert_eq!(quantity("1_000.5g"), (1000.5, 0.0, "g".into()));
        assert_eq!(quantity("20°C"), (20.0, 0.0, "°C".into()));
        assert_eq!(quantity("20\u{ba}C"), (20.0, 0.0, "°C".into()));
        assert_eq!(quantity("5\u{b5}m"), (5.0, 0.0, "μm".into()));
        assert_eq!(quantity("2.5Ω"), (2.5, 0.0, "Ω".into()));
        // An exponent, then a unit; and `E` is exa when no digit follows.
        assert_eq!(quantity("1e3m"), (1000.0, 0.0, "m".into()));
        assert_eq!(quantity("1Em"), (1.0, 0.0, "Em".into()));
        assert_eq!(quantity("1E3m"), (1000.0, 0.0, "m".into()));
        // The uncertainty in brackets comes before the unit.
        let (v, s, u) = quantity("10.5(3)e-3kg");
        assert!((v - 0.0105).abs() < 1e-15 && (s - 0.0003).abs() < 1e-15 && u == "kg");
        let (v, s, u) = quantity("10.5(3)g");
        assert_eq!((v, s, u.as_str()), (10.5, 0.3, "g"));
    }

    #[test]
    fn an_angle_in_degrees_minutes_seconds() {
        for (text, degrees) in [
            ("45°30′30″", 45.0 + 30.0 / 60.0 + 30.0 / 3600.0),
            ("45º30'30\"", 45.0 + 30.0 / 60.0 + 30.0 / 3600.0),
            ("12°30′", 12.5),
            ("0°0′1.5″", 1.5 / 3600.0),
        ] {
            let (v, s, u) = quantity(text);
            assert!(
                (v - degrees).abs() < 1e-12 && s == 0.0 && u == "°",
                "{text}"
            );
        }
        // A bare degree, and degrees Celsius, are as they were.
        assert_eq!(quantity("45°"), (45.0, 0.0, "°".into()));
        assert_eq!(quantity("20°C"), (20.0, 0.0, "°C".into()));
        // A minute or a second on its own is a unit.
        assert_eq!(quantity("30′"), (30.0, 0.0, "′".into()));
        // Not an angle: minutes of 60, or parts out of order.
        assert!(!matches!(toks("45°60′")[..], [Tok::Quantity(..)]));
    }

    #[test]
    fn a_unit_after_zero_is_not_a_bad_based_number() {
        assert_eq!(quantity("0Bq"), (0.0, 0.0, "Bq".into()));
        assert_eq!(quantity("0.5Bq"), (0.5, 0.0, "Bq".into()));
        // Real ones are still the based numbers they were.
        assert_eq!(toks("0b101")[0], Tok::Int(5, Base::Bin));
        assert_eq!(toks("0xff")[0], Tok::Int(255, Base::Hex));
        assert!(lex("0b2", 0, 0).is_err());
    }

    #[test]
    fn what_is_not_a_quantity() {
        // None of these lexes to a quantity token. (`5km/s` is the quantity
        // `5km`, a `/` and a name; that is not one literal, see convert.rs.)
        for bad in [
            "5 kg", "5m2", "5ft", "5L", "5min", "5kgx", "5k", "5m^", "10 m", "5Kg",
        ] {
            let ok = lex(bad, 0, 0)
                .map(|t| t.iter().any(|t| matches!(t.tok, Tok::Quantity(..))))
                .unwrap_or(false);
            assert!(!ok, "`{bad}` should not lex as a quantity");
        }
        assert!(lex("5m2", 0, 0).is_err());
        assert!(lex("5ft", 0, 0).is_err());
        assert!(lex("5 kg", 0, 0).is_ok()); // a number, then a name
        assert!(lex("2in", 0, 0).is_err());
        let e = lex("5ft", 0, 0).unwrap_err();
        assert!(e.help.as_deref().is_some_and(|h| h.contains("SI")), "{e:?}");
    }

    #[test]
    fn a_method_after_a_unit_stays_a_method() {
        assert_eq!(
            toks("5kg.abs()"),
            vec![
                Tok::Quantity(
                    5.0,
                    0.0,
                    siquant::Unit::parse("kg", siquant::Scope::Live).unwrap()
                ),
                Tok::Dot,
                Tok::Ident("abs".into()),
                Tok::LParen,
                Tok::RParen,
                Tok::Eof
            ]
        );
        let t = toks("5m.Float");
        assert!(matches!(t[0], Tok::Quantity(..)));
        assert_eq!(t[1], Tok::Dot);
        assert_eq!(t[2], Tok::Ident("Float".into()));
        // And a unit across a dot is one unit.
        assert_eq!(quantity("5N.m"), (5.0, 0.0, "N.m".into()));
        // A minus after a unit is a minus.
        let t = toks("5m-3m");
        assert!(matches!(t[0], Tok::Quantity(..)) && t[1] == Tok::Minus);
        assert!(matches!(t[2], Tok::Quantity(..)));
    }

    #[test]
    fn a_scale_reads_as_the_number() {
        assert_eq!(toks("1k.")[0], Tok::Int(1000, Base::Dec));
        assert_eq!(toks("5M.")[0], Tok::Int(5_000_000, Base::Dec));
        assert_eq!(toks("1.5k.")[0], Tok::Int(1500, Base::Dec));
        assert_eq!(toks("0.001k.")[0], Tok::Int(1, Base::Dec));
        assert_eq!(toks("5m.")[0], Tok::Decimal(Dec::parse("0.005").unwrap()));
        assert_eq!(
            toks("2.5u.".replace('u', "\u{3bc}").as_str())[0],
            Tok::Decimal(Dec::parse("0.0000025").unwrap())
        );
        assert_eq!(toks("1T.")[0], Tok::Int(1_000_000_000_000, Base::Dec));
        assert_eq!(toks("1da.")[0], Tok::Int(10, Base::Dec));
        assert_eq!(toks("1e3k.")[0], Tok::Float(1e6));
        // The uncertainty scales with it.
        assert_eq!(toks("2.5(3)k.")[0], Tok::Scientific(2500.0, 300.0));
        // A scale ends where the dot ends: what follows is the next token.
        assert_eq!(
            toks("1k.+2"),
            vec![
                Tok::Int(1000, Base::Dec),
                Tok::Plus,
                Tok::Int(2, Base::Dec),
                Tok::Eof
            ]
        );
        assert_eq!(
            toks("(1k.)"),
            vec![
                Tok::LParen,
                Tok::Int(1000, Base::Dec),
                Tok::RParen,
                Tok::Eof
            ]
        );
        // `m` is the metre, and `m.` is milli.
        assert!(matches!(toks("5m")[0], Tok::Quantity(..)));
        assert_eq!(toks("5m.")[0], Tok::Decimal(Dec::parse("0.005").unwrap()));
        // A scale that is not a prefix is no scale.
        assert!(lex("5x.", 0, 0).is_err());
        assert!(lex("5kg.", 0, 0).is_err() || !matches!(toks("5kg.")[0], Tok::Int(..)));
    }

    #[test]
    fn units_can_be_left_off() {
        // What is imported is lexed without units: `12g` stays text.
        assert!(lex_with("12g", 0, 0, false).is_err());
        assert!(lex_with("1k.", 0, 0, false).is_err());
        assert!(lex_with("12", 0, 0, false).is_ok());
        assert!(lex_with("0Bq", 0, 0, false).is_err());
        assert!(lex_with("12g", 0, 0, true).is_ok());
    }

    #[test]
    fn no_function_name_is_a_unit_symbol() {
        // `x.name` without brackets is a conversion; a name that was also a
        // unit would be read as part of the unit (add-si-units D237).
        use crate::funcs::FUNCTIONS;
        use siquant::{Scope, Unit};
        for f in FUNCTIONS {
            assert!(
                Unit::parse(f.name, Scope::Live).is_err(),
                "the function `{}` is also a unit symbol",
                f.name
            );
        }
        for t in [
            "Integer",
            "Int",
            "Hex",
            "Octal",
            "Binary",
            "Decimal",
            "Fraction",
            "Percent",
            "Float",
            "Scientific",
            "Complex",
            "String",
            "Date",
            "Time",
            "Datetime",
            "Duration",
            "Boolean",
            "Url",
            "Email",
            "Uuid",
            "Quantity",
        ] {
            assert!(
                Unit::parse(t, Scope::Live).is_err(),
                "the type `{t}` is a unit"
            );
        }
    }
}

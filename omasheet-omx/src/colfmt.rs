// Copyright (c) 2026 Stephen Roe

//! A column's format, after its type in brackets: a number format,
//! `Price : Decimal['.2f]`, or a date and time format, `Due : Date[%d %b %Y]`.

use std::fmt;

use crate::datefmt::DateFormat;
use crate::numfmt::{FormatError, NumFormat};
use crate::types::S;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Number(NumFormat),
    Date(DateFormat),
}

impl Format {
    /// Read a format for a column of type `ty`: a date and time format for a
    /// `Date`, `Time` or `Datetime`, and a number format for any other.
    pub fn parse(text: &str, ty: S) -> Result<Format, FormatError> {
        match ty {
            S::Date | S::Time | S::DateTime => DateFormat::parse(text).map(Format::Date),
            _ => NumFormat::parse(text).map(Format::Number),
        }
    }

    /// Whether a column of type `ty` takes this format.
    pub fn takes(&self, ty: S) -> bool {
        match self {
            Format::Number(f) => f.takes(ty),
            Format::Date(f) => f.takes(ty),
        }
    }

    /// What a column of type `ty` takes, for a diagnostic, or `None` for a
    /// type that takes no format.
    pub fn conversions_of(ty: S) -> Option<&'static str> {
        match ty {
            S::Date => Some("the conversions of a date, as `%d %b %Y`"),
            S::Time => Some("the conversions of a time, as `%H:%M`"),
            S::DateTime => Some("the conversions of a date and a time, as `%d %b %Y %H:%M`"),
            _ => NumFormat::conversions_of(ty),
        }
    }

    /// What a format is, for a diagnostic about one that does not read.
    pub fn help(ty: S) -> &'static str {
        match ty {
            S::Date | S::Time | S::DateTime => {
                "a date or time format is a C `strftime` format, as `%d %b %Y`"
            }
            _ => "a format is a C `printf` conversion without its `%`, as `'.2f`",
        }
    }
}

/// Where the format at the start of `text`, after its `[`, ends: the byte
/// of its `]`. A `]` after a `%` is part of it, `%]`, and so is a `%` after
/// one, `%%]`.
pub fn closing(text: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    while let Some((k, c)) = chars.next() {
        match c {
            '%' => {
                chars.next();
            }
            ']' => return Some(k),
            _ => {}
        }
    }
    None
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Format::Number(n) => n.fmt(f),
            Format::Date(d) => d.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_closing_bracket() {
        assert_eq!(closing("'.2f] from"), Some(4));
        assert_eq!(closing("%d %] x] y"), Some(7));
        assert_eq!(closing("100%%]"), Some(5));
        assert_eq!(closing("%d"), None);
    }

    #[test]
    fn reads_by_the_type() {
        assert!(matches!(Format::parse("%d", S::Date), Ok(Format::Date(_))));
        assert!(matches!(Format::parse("d", S::Int), Ok(Format::Number(_))));
        assert!(Format::parse("%d", S::Int).is_err());
        assert!(!Format::parse("%d", S::Date).unwrap().takes(S::Int));
    }
}

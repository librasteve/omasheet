// Copyright (c) 2026 Stephen Roe

//! A column's format, after its type in brackets: a number format,
//! `Price : Decimal['.2f]`, a date and time format, `Due : Date[%d %b %Y]`,
//! or `Length : Scientific[±.4e-3]`, which shows a value with its uncertainty
//! as `±`, with so many places, and with an exponent of its own.

use std::fmt;

use crate::datefmt::DateFormat;
use crate::numfmt::{FormatError, NumFormat};
use crate::types::S;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Number(NumFormat),
    Date(DateFormat),
    Scientific(SciFormat),
}

/// How a `Scientific` column shows its values: written `±`, `.4`, `e-3`, or
/// those together in that order, `±.4e-3`. A value is shown in the concise
/// form, `11.0000(82)`, unless `±` asks for `11.0000 ± 0.0082`; `.4` is the
/// places of the number, so that every cell of a column has the same and the
/// brackets stand in line; `e-3` is the exponent of the number, the same for
/// every cell. What is not named is worked out for each cell from its own
/// uncertainty.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SciFormat {
    pub plus_minus: bool,
    pub places: Option<u32>,
    pub exponent: Option<i32>,
}

/// The most places a number has.
const MOST_PLACES: u32 = 17;

impl SciFormat {
    /// Whether it names anything, other than the concise form.
    pub fn is_default(&self) -> bool {
        *self == SciFormat::default()
    }

    pub fn parse(text: &str) -> Result<SciFormat, FormatError> {
        let chars: Vec<char> = text.trim().chars().collect();
        let error = |at: usize, len: usize| FormatError {
            at,
            len: len.max(1),
            message: "a `Scientific` format is `±`, a number of places as `.4`, an exponent as \
                      `e-3`, or those together, as `±.4e-3`"
                .to_string(),
        };
        let mut f = SciFormat::default();
        let mut i = 0;
        let digits = |i: &mut usize| {
            let start = *i;
            while chars.get(*i).is_some_and(|c| c.is_ascii_digit()) {
                *i += 1;
            }
            chars[start..*i].iter().collect::<String>()
        };
        if chars.first() == Some(&'±') {
            f.plus_minus = true;
            i += 1;
        }
        if chars.get(i) == Some(&'.') {
            i += 1;
            let n = digits(&mut i);
            match n.parse::<u32>() {
                Ok(n) if n <= MOST_PLACES => f.places = Some(n),
                _ => return Err(error(i - n.len() - 1, n.len() + 1)),
            }
        }
        if chars.get(i) == Some(&'e') {
            let start = i;
            i += 1;
            let negative = match chars.get(i) {
                Some('-') => {
                    i += 1;
                    true
                }
                Some('+') => {
                    i += 1;
                    false
                }
                _ => false,
            };
            let n = digits(&mut i);
            match n.parse::<i32>() {
                Ok(n) if n <= 300 => f.exponent = Some(if negative { -n } else { n }),
                _ => return Err(error(start, i - start)),
            }
        }
        if i != chars.len() || (i == 0) {
            return Err(error(i, chars.len().saturating_sub(i)));
        }
        Ok(f)
    }
}

impl fmt::Display for SciFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.plus_minus {
            f.write_str("±")?;
        }
        if let Some(n) = self.places {
            write!(f, ".{n}")?;
        }
        if let Some(e) = self.exponent {
            write!(f, "e{e}")?;
        }
        Ok(())
    }
}

impl Format {
    /// Read a format for a column of type `ty`: a date and time format for a
    /// `Date`, `Time` or `Datetime`, and a number format for any other.
    pub fn parse(text: &str, ty: S) -> Result<Format, FormatError> {
        match ty {
            S::Scientific => SciFormat::parse(text).map(Format::Scientific),
            S::Date | S::Time | S::Datetime => DateFormat::parse(text).map(Format::Date),
            _ => NumFormat::parse(text).map(Format::Number),
        }
    }

    /// Whether a column of type `ty` takes this format.
    pub fn takes(&self, ty: S) -> bool {
        match self {
            Format::Number(f) => f.takes(ty),
            Format::Date(f) => f.takes(ty),
            Format::Scientific(_) => ty == S::Scientific,
        }
    }

    /// What a column of type `ty` takes, for a diagnostic, or `None` for a
    /// type that takes no format.
    pub fn conversions_of(ty: S) -> Option<&'static str> {
        match ty {
            S::Scientific => {
                Some("`±`, places as `.4`, an exponent as `e-3`, or together as `±.4e-3`")
            }
            S::Date => Some("the conversions of a date, as `%d %b %Y`"),
            S::Time => Some("the conversions of a time, as `%H:%M`"),
            S::Datetime => Some("the conversions of a date and a time, as `%d %b %Y %H:%M`"),
            _ => NumFormat::conversions_of(ty),
        }
    }

    /// What a format is, for a diagnostic about one that does not read.
    pub fn help(ty: S) -> &'static str {
        match ty {
            S::Scientific => {
                "a `Scientific` format is `±`, places as `.4`, an exponent as `e-3`, or together as `±.4e-3`"
            }
            S::Date | S::Time | S::Datetime => {
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
            Format::Scientific(s) => s.fmt(f),
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
    fn a_scientific_format_is_plus_minus_places_and_an_exponent() {
        let read = |t: &str| SciFormat::parse(t);
        assert_eq!(
            read("±"),
            Ok(SciFormat {
                plus_minus: true,
                ..Default::default()
            })
        );
        assert_eq!(
            read(".4"),
            Ok(SciFormat {
                places: Some(4),
                ..Default::default()
            })
        );
        assert_eq!(
            read("e-3"),
            Ok(SciFormat {
                exponent: Some(-3),
                ..Default::default()
            })
        );
        assert_eq!(
            read("±.3e-3"),
            Ok(SciFormat {
                plus_minus: true,
                places: Some(3),
                exponent: Some(-3)
            })
        );
        assert_eq!(read("e3").map(|f| f.exponent), Ok(Some(3)));
        // It is written as it was read.
        for text in ["±", ".4", "e-3", "±.3e-3", "e3"] {
            assert_eq!(read(text).unwrap().to_string(), text);
        }
        // Nothing, or anything else, is refused.
        for text in ["", "x", ".", ".4x", "e", ".99", "±±", "e-"] {
            assert!(read(text).is_err(), "{text}");
        }
        assert!(
            Format::parse("±.3", S::Scientific)
                .unwrap()
                .takes(S::Scientific)
        );
        assert!(Format::parse("±.3", S::Float).is_err());
    }

    #[test]
    fn reads_by_the_type() {
        assert!(matches!(Format::parse("%d", S::Date), Ok(Format::Date(_))));
        assert!(matches!(Format::parse("d", S::Int), Ok(Format::Number(_))));
        assert!(Format::parse("%d", S::Int).is_err());
        assert!(!Format::parse("%d", S::Date).unwrap().takes(S::Int));
    }
}

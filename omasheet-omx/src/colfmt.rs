// Copyright (c) 2026 Stephen Roe

//! A column's format, after its type in brackets: a number format,
//! `Price : Decimal['.2f]`, a date and time format, `Due : Date[%d %b %Y]`,
//! or `Length : Scientific[±.3e-3]`, which is a number format with `±` before
//! it for a value shown with its uncertainty as `value ± uncertainty`.

use std::fmt;

use crate::datefmt::DateFormat;
use crate::numfmt::{FormatError, MAX_WIDTH, NumFormat};
use crate::types::S;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Number(NumFormat),
    Date(DateFormat),
    Scientific(SciFormat),
}

/// How a `Scientific` column shows its values, as a `Float` column is given
/// its, by a C `printf` conversion without its `%` (D100), with `±` before it
/// for `value ± uncertainty` in place of the concise `value(nn)`:
/// `±+10.3e-3` is `±`, the flag `+`, a width of 10, 3 places, the
/// conversion `e` and an exponent of -3.
///
/// `f` is a fixed number of places, `e` an exponent, and `g` whichever of the
/// two reads better; `.3` is the places of the number, so that every cell of
/// a column has the same and the brackets stand in line. `e` takes an
/// exponent after it, `e-3`, the same for every cell. What is not named is
/// worked out for each cell from its own uncertainty.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SciFormat {
    /// `±`: written as `value ± uncertainty`.
    pub plus_minus: bool,
    /// `-`: on the left of its width, rather than the right.
    pub left: bool,
    /// `+`: a sign on every number, `+` on one that is not negative.
    pub plus: bool,
    /// ` `: a space where a number that is not negative has no sign.
    pub space: bool,
    /// `0`: the width made up with zeros after the sign, not spaces before.
    pub zero: bool,
    /// `'`: the whole part grouped in threes by the locale's separator.
    pub group: bool,
    pub width: Option<usize>,
    pub places: Option<u32>,
    pub conv: SciConv,
    /// `E` or `G` for `e` or `g` in capitals.
    pub upper: bool,
    /// The exponent of every number, after an `e`.
    pub exponent: Option<i32>,
}

/// How a number of a `Scientific` is sized.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SciConv {
    /// `g`, or none: plain where the number is of a size to read, and with
    /// an exponent where it is not.
    #[default]
    General,
    /// `f`: plain, with the places asked for.
    Fixed,
    /// `e`: a mantissa and an exponent, its own or the one asked for.
    Exp,
}

/// The most places a number has.
const MOST_PLACES: u32 = 17;

impl SciFormat {
    pub fn parse(text: &str) -> Result<SciFormat, FormatError> {
        let chars: Vec<char> = text.trim().chars().collect();
        let error = |at: usize, len: usize, message: &str| FormatError {
            at,
            len: len.max(1),
            message: message.to_string(),
        };
        let mut f = SciFormat::default();
        let mut i = 0;
        if chars.is_empty() {
            return Err(error(0, 0, "a format is empty"));
        }
        if chars.first() == Some(&'±') {
            f.plus_minus = true;
            i += 1;
        }
        let flags = i;
        while let Some(c) = chars.get(i) {
            match c {
                '-' => f.left = true,
                '+' => f.plus = true,
                ' ' => f.space = true,
                '0' => f.zero = true,
                '\'' => f.group = true,
                _ => break,
            }
            i += 1;
        }
        let digits = |i: &mut usize| {
            let start = *i;
            while chars.get(*i).is_some_and(|c| c.is_ascii_digit()) {
                *i += 1;
            }
            chars[start..*i].iter().collect::<String>()
        };
        let start = i;
        let n = digits(&mut i);
        if !n.is_empty() {
            match n.parse::<usize>() {
                Ok(n) if n <= MAX_WIDTH => f.width = Some(n),
                _ => {
                    return Err(error(
                        start,
                        n.len(),
                        &format!("a width is at most {MAX_WIDTH}"),
                    ));
                }
            }
        }
        if chars.get(i) == Some(&'.') {
            let start = i;
            i += 1;
            let n = digits(&mut i);
            // `.` alone is a precision of nought, as C has it.
            match if n.is_empty() {
                Ok(0)
            } else {
                n.parse::<u32>()
            } {
                Ok(n) if n <= MOST_PLACES => f.places = Some(n),
                _ => {
                    return Err(error(
                        start,
                        i - start,
                        &format!("a `Scientific` has at most {MOST_PLACES} places"),
                    ));
                }
            }
        }
        let named = i > flags;
        match chars.get(i) {
            None if !named => return Ok(f),
            None => {
                return Err(error(
                    i,
                    0,
                    "a format ends with its conversion, as `f`, `e` or `g`",
                ));
            }
            Some(c @ ('f' | 'F' | 'e' | 'E' | 'g' | 'G')) => {
                f.conv = match c.to_ascii_lowercase() {
                    'f' => SciConv::Fixed,
                    'e' => SciConv::Exp,
                    _ => SciConv::General,
                };
                f.upper = c.is_ascii_uppercase();
            }
            Some(c @ ('d' | 'i' | 'x' | 'X' | 'o' | 'b' | 'B')) => {
                return Err(error(
                    i,
                    1,
                    &format!(
                        "`{c}` is a conversion of a whole number: a `Scientific` takes `f`, `e` or `g`"
                    ),
                ));
            }
            Some(c) => {
                return Err(error(
                    i,
                    1,
                    &format!(
                        "`{c}` is not a conversion of a `Scientific`, which takes `f`, `e` or `g`"
                    ),
                ));
            }
        }
        i += 1;
        if f.conv == SciConv::Exp && matches!(chars.get(i), Some('-' | '+' | '0'..='9')) {
            let start = i;
            let negative = chars[i] == '-';
            if matches!(chars[i], '-' | '+') {
                i += 1;
            }
            let n = digits(&mut i);
            match n.parse::<i32>() {
                Ok(n) if n <= 300 => f.exponent = Some(if negative { -n } else { n }),
                _ => {
                    return Err(error(
                        start,
                        i - start,
                        "an exponent is a whole number, at most 300",
                    ));
                }
            }
        }
        if i != chars.len() {
            return Err(error(
                i,
                chars.len() - i,
                "a format ends at its conversion, or at the exponent of `e`",
            ));
        }
        if f.group && f.conv == SciConv::Exp {
            return Err(error(
                flags,
                i - flags,
                "`'` groups only `f` and `g`, not `e`",
            ));
        }
        Ok(f)
    }
}

impl fmt::Display for SciFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.plus_minus {
            f.write_str("±")?;
        }
        for (on, c) in [
            (self.left, '-'),
            (self.plus, '+'),
            (self.space, ' '),
            (self.zero, '0'),
            (self.group, '\''),
        ] {
            if on {
                write!(f, "{c}")?;
            }
        }
        if let Some(n) = self.width {
            write!(f, "{n}")?;
        }
        if let Some(n) = self.places {
            write!(f, ".{n}")?;
        }
        let letter = match (self.conv, self.upper) {
            (SciConv::Fixed, _) => Some('f'),
            (SciConv::Exp, false) => Some('e'),
            (SciConv::Exp, true) => Some('E'),
            (SciConv::General, false) => Some('g'),
            (SciConv::General, true) => Some('G'),
        };
        // `g` is understood where nothing else is said.
        let named = self.left
            || self.plus
            || self.space
            || self.zero
            || self.group
            || self.width.is_some()
            || self.places.is_some();
        if let Some(c) = letter.filter(|&c| named || c != 'g') {
            write!(f, "{c}")?;
        }
        if let Some(e) = self.exponent {
            write!(f, "{e}")?;
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
                Some("`f`, `e` or `g`, with `±` before them for `value ± uncertainty`")
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
                "a `Scientific` format is a C `printf` conversion without its `%`, with `±` before it, as `±.3e-3`"
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
    fn a_scientific_format_is_a_number_format_with_plus_minus() {
        let read = |t: &str| SciFormat::parse(t);
        assert_eq!(
            read("±"),
            Ok(SciFormat {
                plus_minus: true,
                ..Default::default()
            })
        );
        assert_eq!(
            read(".4f"),
            Ok(SciFormat {
                places: Some(4),
                conv: SciConv::Fixed,
                ..Default::default()
            })
        );
        assert_eq!(
            read("e-3"),
            Ok(SciFormat {
                conv: SciConv::Exp,
                exponent: Some(-3),
                ..Default::default()
            })
        );
        assert_eq!(
            read("±+10.3e-3"),
            Ok(SciFormat {
                plus_minus: true,
                plus: true,
                width: Some(10),
                places: Some(3),
                conv: SciConv::Exp,
                exponent: Some(-3),
                ..Default::default()
            })
        );
        assert_eq!(
            read("e").map(|f| (f.conv, f.exponent)),
            Ok((SciConv::Exp, None))
        );
        assert_eq!(
            read("E3").map(|f| (f.upper, f.exponent)),
            Ok((true, Some(3)))
        );
        assert_eq!(read("'.2f").map(|f| f.group), Ok(true));
        // It is written back the same, the flags in their order.
        for text in [
            "±", "e", ".4f", "e-3", "±.3e-3", "e3", "±+8.2f", "-08.1g", ".2E-4", "'.2f",
        ] {
            assert_eq!(read(text).unwrap().to_string(), text);
        }
        // Nothing, or anything else, is refused.
        for text in [
            "", "x", ".", ".4", "8", "+", ".4x", "±±", "e-", "d", ".2x", ".99f", "1000f", "'e",
            "fe", ".3f2",
        ] {
            assert!(read(text).is_err(), "{text}");
        }
        assert!(read(".2d").unwrap_err().message.contains("whole number"));
        assert!(
            Format::parse("±.3e", S::Scientific)
                .unwrap()
                .takes(S::Scientific)
        );
        assert!(Format::parse("±.3e", S::Float).is_err());
    }

    #[test]
    fn reads_by_the_type() {
        assert!(matches!(Format::parse("%d", S::Date), Ok(Format::Date(_))));
        assert!(matches!(Format::parse("d", S::Int), Ok(Format::Number(_))));
        assert!(Format::parse("%d", S::Int).is_err());
        assert!(!Format::parse("%d", S::Date).unwrap().takes(S::Int));
    }
}

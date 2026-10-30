// Copyright (c) 2026 Stephen Roe

//! Number formats: how a column of numbers is shown, written as a C `printf`
//! conversion without its `%`, `'.2f`, after the column's type in brackets:
//! `Price : Decimal['.2f]`.

use std::fmt;

use crate::types::S;

/// The widest a width or a precision may be written: enough for any number
/// a sheet shows, and few enough that one cannot ask for a cell of a
/// million spaces.
pub const MAX_WIDTH: usize = 999;

/// The letter that ends a conversion, which says how the number is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conv {
    /// `d` or `i`: a whole number in decimal.
    Dec,
    /// `x` or `X`: in hex, its letters in capitals when `upper`.
    Hex { upper: bool },
    /// `o`: in octal.
    Oct,
    /// `b` or `B`: in binary, as C23 has it.
    Bin { upper: bool },
    /// `f` or `F`: with a fixed number of places.
    Fixed { upper: bool },
    /// `e` or `E`: in e notation.
    Exp { upper: bool },
    /// `g` or `G`: fixed or e notation, whichever is the shorter.
    General { upper: bool },
}

impl Conv {
    fn from_letter(c: char) -> Option<Conv> {
        Some(match c {
            'd' | 'i' => Conv::Dec,
            'x' => Conv::Hex { upper: false },
            'X' => Conv::Hex { upper: true },
            'o' => Conv::Oct,
            'b' => Conv::Bin { upper: false },
            'B' => Conv::Bin { upper: true },
            'f' => Conv::Fixed { upper: false },
            'F' => Conv::Fixed { upper: true },
            'e' => Conv::Exp { upper: false },
            'E' => Conv::Exp { upper: true },
            'g' => Conv::General { upper: false },
            'G' => Conv::General { upper: true },
            _ => return None,
        })
    }

    /// A conversion of a whole number.
    pub fn is_integer(self) -> bool {
        matches!(
            self,
            Conv::Dec | Conv::Hex { .. } | Conv::Oct | Conv::Bin { .. }
        )
    }

    /// Hex, octal or binary, which show a negative number as its two's
    /// complement.
    pub fn is_radix(self) -> bool {
        self.is_integer() && self != Conv::Dec
    }
}

/// A number format, as read from between the brackets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumFormat {
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
    /// `#`: the point kept where no digit follows it, and `0x`, `0` or `0b`
    /// before hex, octal or binary.
    pub alt: bool,
    pub width: Option<usize>,
    pub precision: Option<usize>,
    pub conv: Conv,
    /// The format as it was written, to write it back the same.
    text: String,
}

/// Why a format does not read, and where in it, in characters from its start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatError {
    pub at: usize,
    pub len: usize,
    pub message: String,
}

fn error(at: usize, len: usize, message: impl Into<String>) -> FormatError {
    FormatError {
        at,
        len,
        message: message.into(),
    }
}

impl NumFormat {
    /// Read a format, as C reads one after its `%`: flags, a width, a
    /// precision and a conversion. What C leaves undefined is refused, as
    /// are the parts that have no meaning for the one number a cell holds:
    /// `*`, a length (`l`, `h`) and a position (`1$`).
    pub fn parse(text: &str) -> Result<NumFormat, FormatError> {
        let bytes = text.as_bytes();
        let mut k = 0;
        let mut f = NumFormat {
            left: false,
            plus: false,
            space: false,
            zero: false,
            group: false,
            alt: false,
            width: None,
            precision: None,
            conv: Conv::Dec,
            text: text.to_string(),
        };
        if text.is_empty() {
            return Err(error(0, 0, "a format is empty"));
        }
        while let Some(&b) = bytes.get(k) {
            match b {
                b'-' => f.left = true,
                b'+' => f.plus = true,
                b' ' => f.space = true,
                b'0' => f.zero = true,
                b'\'' => f.group = true,
                b'#' => f.alt = true,
                _ => break,
            }
            k += 1;
        }
        let number = |k: &mut usize| -> Result<Option<usize>, FormatError> {
            let start = *k;
            while bytes.get(*k).is_some_and(u8::is_ascii_digit) {
                *k += 1;
            }
            if *k == start {
                return Ok(None);
            }
            match text[start..*k].parse::<usize>() {
                Ok(n) if n <= MAX_WIDTH => Ok(Some(n)),
                _ => Err(error(
                    start,
                    *k - start,
                    format!("a width or precision is at most {MAX_WIDTH}"),
                )),
            }
        };
        f.width = number(&mut k)?;
        if bytes.get(k) == Some(&b'$') {
            return Err(error(
                k,
                1,
                "a format takes no position: there is one number",
            ));
        }
        if bytes.get(k) == Some(&b'.') {
            k += 1;
            // `.` alone is a precision of nought, as C has it.
            f.precision = Some(number(&mut k)?.unwrap_or(0));
        }
        let Some(c) = text[k..].chars().next() else {
            return Err(error(
                k,
                0,
                "a format ends with its conversion, as `d` or `f`",
            ));
        };
        let rest = k + c.len_utf8();
        match c {
            '*' => return Err(error(k, 1, "a width is written as a number, not `*`")),
            'h' | 'l' | 'L' | 'j' | 'z' | 't' => {
                return Err(error(
                    k,
                    1,
                    "a format takes no length: the type is the column's",
                ));
            }
            _ => {}
        }
        f.conv = Conv::from_letter(c).ok_or_else(|| {
            error(
                k,
                c.len_utf8(),
                format!("`{c}` is not a conversion of a number"),
            )
        })?;
        if rest < text.len() {
            return Err(error(
                rest,
                text.len() - rest,
                "a format ends at its conversion",
            ));
        }
        // Undefined in C, and so refused rather than given a meaning.
        if f.alt && f.conv == Conv::Dec {
            return Err(error(0, k, "`#` has no meaning for `d` or `i`"));
        }
        if f.group && (f.conv.is_radix() || matches!(f.conv, Conv::Exp { .. })) {
            return Err(error(
                0,
                k,
                format!("`'` groups only `d`, `i`, `f`, `F`, `g` and `G`, not `{c}`"),
            ));
        }
        Ok(f)
    }

    /// Whether a column of type `ty` takes this format: an `Integer` takes a
    /// conversion of a whole number, and a `Decimal` or a `Float` one of a
    /// number with places.
    pub fn takes(&self, ty: S) -> bool {
        match ty {
            S::Int => self.conv.is_integer(),
            S::Decimal | S::Float => !self.conv.is_integer(),
            _ => false,
        }
    }

    /// The conversions a column of type `ty` takes, for a diagnostic, or
    /// `None` for a type that takes no format.
    pub fn conversions_of(ty: S) -> Option<&'static str> {
        match ty {
            S::Int => Some("`d`, `i`, `x`, `X`, `o`, `b` or `B`"),
            S::Decimal | S::Float => Some("`f`, `F`, `e`, `E`, `g` or `G`"),
            _ => None,
        }
    }
}

impl fmt::Display for NumFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> NumFormat {
        NumFormat::parse(text).unwrap_or_else(|e| panic!("{text}: {e:?}"))
    }

    fn refused(text: &str) -> String {
        NumFormat::parse(text).expect_err(text).message
    }

    #[test]
    fn reads_flags_width_precision_and_conversion() {
        let f = read("+'015.2f");
        assert!(f.plus && f.group && f.zero && !f.left && !f.space && !f.alt);
        assert_eq!((f.width, f.precision), (Some(15), Some(2)));
        assert_eq!(f.conv, Conv::Fixed { upper: false });
        assert_eq!(f.to_string(), "+'015.2f");
        let f = read("-#8X");
        assert!(f.left && f.alt);
        assert_eq!(
            (f.width, f.precision, f.conv),
            (Some(8), None, Conv::Hex { upper: true })
        );
        assert_eq!(read(".f").precision, Some(0));
        assert_eq!(read("#b").conv, Conv::Bin { upper: false });
        assert_eq!(read("i").conv, Conv::Dec);
        assert_eq!(read(" G").conv, Conv::General { upper: true });
    }

    #[test]
    fn refuses_what_c_would_not_read() {
        assert!(refused("").contains("empty"));
        assert!(refused("8").contains("conversion"));
        assert!(refused(".2q").contains("`q`"));
        assert!(refused("s").contains("`s`"));
        assert!(refused("n").contains("`n`"));
        assert!(refused("*d").contains("`*`"));
        assert!(refused("ld").contains("length"));
        assert!(refused("1$d").contains("position"));
        assert!(refused("dd").contains("ends"));
        assert!(refused("1000d").contains("999"));
        assert!(refused("#d").contains("`#`"));
        assert!(refused("'x").contains("`'`"));
        assert!(refused("'e").contains("`'`"));
    }

    #[test]
    fn says_which_types_take_it() {
        assert!(read("x").takes(S::Int));
        assert!(!read("x").takes(S::Float));
        assert!(read(".2f").takes(S::Decimal));
        assert!(read("g").takes(S::Float));
        assert!(!read(".2f").takes(S::Int));
        assert!(!read("d").takes(S::String));
        assert!(!read("d").takes(S::Percent));
    }
}

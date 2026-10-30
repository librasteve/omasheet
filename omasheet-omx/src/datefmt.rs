// Copyright (c) 2026 Stephen Roe

//! Date and time formats: how a column of dates or times is shown, written
//! as a C `strftime` format after the column's type in brackets:
//! `Due : Date[%d %b %Y]`.
//!
//! Beyond C, a format takes the flags and width of glibc, `%-d`, and `%f`
//! for the fraction of a second, as `jiff` and `chrono` take them; `%]` is
//! a `]`, which would otherwise close the bracket.

use std::fmt;

use crate::numfmt::{FormatError, MAX_WIDTH};
use crate::types::S;

/// What a conversion needs of the value it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Needs {
    /// A day: `%d`, `%b`, `%Y`.
    Date,
    /// A time of day: `%H`, `%M`, `%f`.
    Time,
    /// An instant, a day and a time in a zone: `%c`, `%s`, `%z`, `%Z`.
    Instant,
    /// Nothing: `%n`, `%t`.
    Nothing,
}

/// What a conversion needs, or `None` for a letter that is not one.
pub fn needs(c: char) -> Option<Needs> {
    Some(match c {
        'a' | 'A' | 'b' | 'h' | 'B' | 'C' | 'd' | 'D' | 'e' | 'F' | 'g' | 'G' | 'j' | 'm' | 'u'
        | 'U' | 'V' | 'w' | 'W' | 'x' | 'y' | 'Y' => Needs::Date,
        'H' | 'I' | 'k' | 'l' | 'M' | 'p' | 'P' | 'r' | 'R' | 'S' | 'T' | 'X' | 'f' => Needs::Time,
        'c' | 's' | 'z' | 'Z' => Needs::Instant,
        'n' | 't' => Needs::Nothing,
        _ => return None,
    })
}

/// How a conversion is padded to its width, as glibc's flags say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pad {
    /// `-`: not at all, `%-d` is `5`.
    None,
    /// `_`: with spaces.
    Space,
    /// `0`: with zeros.
    Zero,
}

/// One part of a format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// Text shown as it is, with `%%` and `%]` as `%` and `]`.
    Text(String),
    Conv {
        pad: Option<Pad>,
        /// `^`: in capitals.
        upper: bool,
        /// `#`: the case of a name turned about, as glibc has it.
        swap: bool,
        width: Option<usize>,
        conv: char,
        /// The digits of `%3f`, `%6f` or `%9f`; `None` for `%f`, every one
        /// held.
        digits: Option<usize>,
    },
}

/// A date and time format, as read from between the brackets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DateFormat {
    pub items: Vec<Item>,
    /// Each conversion, where it starts and how long it is.
    spans: Vec<(char, usize, usize)>,
    /// The format as it was written, to write it back the same.
    text: String,
}

fn error(at: usize, len: usize, message: impl Into<String>) -> FormatError {
    FormatError {
        at,
        len,
        message: message.into(),
    }
}

impl DateFormat {
    /// Read a format as glibc's `strftime` reads one. What it would show as
    /// written, a `%` before a letter that is no conversion, is refused.
    pub fn parse(text: &str) -> Result<DateFormat, FormatError> {
        if text.is_empty() {
            return Err(error(0, 0, "a format is empty"));
        }
        let mut items = Vec::new();
        let mut spans = Vec::new();
        let mut plain = String::new();
        let mut chars = text.char_indices().peekable();
        while let Some((at, c)) = chars.next() {
            if c != '%' {
                plain.push(c);
                continue;
            }
            let (mut pad, mut upper, mut swap) = (None, false, false);
            while let Some(&(_, f)) = chars.peek() {
                match f {
                    '-' => pad = Some(Pad::None),
                    '_' => pad = Some(Pad::Space),
                    '0' => pad = Some(Pad::Zero),
                    '^' => upper = true,
                    '#' => swap = true,
                    _ => break,
                }
                chars.next();
            }
            let mut width = None;
            let start = chars.peek().map_or(text.len(), |&(k, _)| k);
            while chars.peek().is_some_and(|(_, d)| d.is_ascii_digit()) {
                chars.next();
            }
            let end = chars.peek().map_or(text.len(), |&(k, _)| k);
            if end > start {
                match text[start..end].parse::<usize>() {
                    Ok(n) if n <= MAX_WIDTH => width = Some(n),
                    _ => {
                        return Err(error(
                            start,
                            end - start,
                            format!("a width is at most {MAX_WIDTH}"),
                        ));
                    }
                }
            }
            // `E` and `O` ask for a locale's other forms, which glibc has
            // only for a few; they are read and the usual form is shown.
            while chars.peek().is_some_and(|&(_, m)| m == 'E' || m == 'O') {
                chars.next();
            }
            let Some((k, conv)) = chars.next() else {
                return Err(error(
                    at,
                    text.len() - at,
                    "a `%` ends with a conversion, as `%d`",
                ));
            };
            let plain_too = pad.is_none() && !upper && !swap && width.is_none();
            if matches!(conv, '%' | ']') && plain_too && k == at + 1 {
                plain.push(conv);
                continue;
            }
            // `%3f`: the width of a fraction is how many digits it has.
            let digits = match conv {
                'f' => match width {
                    None => None,
                    Some(n @ (3 | 6 | 9)) => Some(n),
                    Some(_) => {
                        return Err(error(
                            start,
                            end - start,
                            "a fraction of a second is `%f`, `%3f`, `%6f` or `%9f`",
                        ));
                    }
                },
                _ => None,
            };
            if needs(conv).is_none() {
                return Err(error(
                    k,
                    conv.len_utf8(),
                    format!("`{conv}` is not a conversion of a date or time"),
                ));
            }
            if !plain.is_empty() {
                items.push(Item::Text(std::mem::take(&mut plain)));
            }
            spans.push((conv, at, k + conv.len_utf8() - at));
            items.push(Item::Conv {
                pad,
                upper,
                swap,
                width: width.filter(|_| conv != 'f'),
                conv,
                digits,
            });
        }
        if !plain.is_empty() {
            items.push(Item::Text(plain));
        }
        Ok(DateFormat {
            items,
            spans,
            text: text.to_string(),
        })
    }

    fn convs(&self) -> impl Iterator<Item = char> + '_ {
        self.items.iter().filter_map(|i| match i {
            Item::Conv { conv, .. } => Some(*conv),
            Item::Text(_) => None,
        })
    }

    /// The first conversion a column of type `ty` does not take, or `None`
    /// where it takes them all. A `Date` has no time of day, a `Time` no day,
    /// and only a `Datetime` is an instant.
    pub fn refused(&self, ty: S) -> Option<char> {
        self.convs().find(|&c| {
            let needs = needs(c).unwrap_or(Needs::Nothing);
            match ty {
                S::Date => matches!(needs, Needs::Time | Needs::Instant),
                S::Time => matches!(needs, Needs::Date | Needs::Instant),
                S::DateTime => false,
                _ => true,
            }
        })
    }

    /// Whether a column of type `ty` takes this format.
    pub fn takes(&self, ty: S) -> bool {
        matches!(ty, S::Date | S::Time | S::DateTime) && self.refused(ty).is_none()
    }

    /// Where conversion `conv` first is in the text, in bytes from its
    /// `%`, and how long it is, for a diagnostic.
    pub fn span_of(&self, conv: char) -> (usize, usize) {
        self.spans
            .iter()
            .find(|(c, ..)| *c == conv)
            .map_or((0, self.text.len()), |&(_, at, len)| (at, len))
    }
}

impl fmt::Display for DateFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> DateFormat {
        DateFormat::parse(text).unwrap_or_else(|e| panic!("{text}: {e:?}"))
    }

    fn refused(text: &str) -> FormatError {
        DateFormat::parse(text).expect_err(text)
    }

    fn conv(c: char) -> Item {
        Item::Conv {
            pad: None,
            upper: false,
            swap: false,
            width: None,
            conv: c,
            digits: None,
        }
    }

    #[test]
    fn reads_conversions_and_text() {
        let f = read("%d %b %Y");
        assert_eq!(
            f.items,
            [
                conv('d'),
                Item::Text(" ".into()),
                conv('b'),
                Item::Text(" ".into()),
                conv('Y')
            ]
        );
        assert_eq!(f.to_string(), "%d %b %Y");
        assert_eq!(read("100%% at %]").items, [Item::Text("100% at ]".into())]);
        assert_eq!(read("on time").items, [Item::Text("on time".into())]);
    }

    #[test]
    fn reads_flags_widths_and_fractions() {
        assert_eq!(
            read("%-_^#10Ey").items,
            [Item::Conv {
                pad: Some(Pad::Space),
                upper: true,
                swap: true,
                width: Some(10),
                conv: 'y',
                digits: None,
            }]
        );
        let digits = |text: &str| match &read(text).items[..] {
            [Item::Conv { digits, width, .. }] => (*digits, *width),
            other => panic!("{other:?}"),
        };
        assert_eq!(digits("%f"), (None, None));
        assert_eq!(digits("%3f"), (Some(3), None));
        assert_eq!(digits("%9f"), (Some(9), None));
        assert_eq!(digits("%Od"), (None, None));
    }

    #[test]
    fn refuses_what_does_not_read() {
        assert!(refused("").message.contains("empty"));
        let e = refused("%d %q");
        assert_eq!((e.at, e.len), (4, 1));
        assert!(e.message.contains("`q`"));
        assert!(refused("%Y-%").message.contains("ends"));
        assert!(refused("%4f").message.contains("`%3f`"));
        assert!(refused("%1000Y").message.contains("999"));
    }

    #[test]
    fn says_which_types_take_it() {
        assert!(read("%d %b %Y").takes(S::Date));
        assert!(!read("%d %H:%M").takes(S::Date));
        assert_eq!(read("%d %H:%M").refused(S::Date), Some('H'));
        assert!(read("%I:%M %p").takes(S::Time));
        assert!(!read("%a %H").takes(S::Time));
        assert!(!read("%H %z").takes(S::Time));
        assert!(read("%c %z %Z %s %f").takes(S::DateTime));
        assert!(!read("%Y").takes(S::Int));
        assert!(read("%n").takes(S::Time));
    }

    #[test]
    fn finds_a_conversion() {
        let f = read("%d %-10H:%M");
        assert_eq!(f.span_of('H'), (3, 5));
        assert_eq!(f.span_of('d'), (0, 2));
    }
}

// Copyright (c) 2026 Stephen Roe

//! A column's format, after its type in brackets: a number format,
//! `Price : Decimal['.2f]`, a date and time format, `Due : Date[%d %b %Y]`,
//! or `Length : Scientific[±.3e-3]`, which is a number format with `±` before
//! it for a value shown with its uncertainty as `value ± uncertainty`.

use std::fmt;

use crate::datefmt::DateFormat;
use crate::numfmt::{FormatError, MAX_WIDTH, NumFormat};
use crate::types::{QId, S};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Number(NumFormat),
    Date(DateFormat),
    Scientific(SciFormat),
    Quantity(QuantityFormat),
    Boolean(BoolFormat),
    /// How a column of text that is wider than its column is shown.
    Text(TextFlow),
    /// How a `Duration` column shows its values other than as a number of a
    /// unit, which is a [`Format::Quantity`]: `iso` or `%H:%M:%S`.
    Duration(DurationStyle),
}

/// What a column of text does where a cell is wider than the column. Not
/// said, the text spills over the empty cells to its right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFlow {
    /// Cut off at the edge of the column.
    Clip,
    /// Carried on to more lines inside the column.
    Wrap,
}

impl TextFlow {
    fn parse(text: &str) -> Option<TextFlow> {
        match text.trim() {
            "clip" => Some(TextFlow::Clip),
            "wrap" => Some(TextFlow::Wrap),
            _ => None,
        }
    }
}

impl fmt::Display for TextFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TextFlow::Clip => "clip",
            TextFlow::Wrap => "wrap",
        })
    }
}

/// A `Duration` shown as a whole, rather than as a number in a unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationStyle {
    /// An ISO 8601 duration, `PT1H30M`.
    Iso,
    /// Signed hours, minutes and seconds, `+01:30:00`; the hours are not
    /// capped at 24.
    Clock,
    /// A count of seconds with its unit, `30s`. Only ever inferred.
    Seconds,
    /// Left to the cells: a column that declares no format shows a duration
    /// in the form its literals are written in, by the kinds of them it holds
    /// (`KIND_*`). The signed clock form is the most favoured, then seconds,
    /// and ISO the least.
    Auto(u8),
}

/// A duration written as `+17:04:10`.
pub const KIND_CLOCK: u8 = 1;
/// A duration written as a quantity of time, `10s`.
pub const KIND_SECONDS: u8 = 2;
/// A duration written as ISO 8601, `PT17H4M`.
pub const KIND_ISO: u8 = 4;

impl DurationStyle {
    /// The style that shows `micros`: itself, or for `Auto` the most
    /// favoured of the forms the column's literals are written in. Seconds
    /// beside ISO are shown as seconds only while under a minute, where a
    /// count of seconds still reads; past that, as the clock.
    pub fn resolve(self, micros: i64) -> DurationStyle {
        let DurationStyle::Auto(kinds) = self else {
            return self;
        };
        if kinds & KIND_CLOCK != 0 {
            DurationStyle::Clock
        } else if kinds & KIND_SECONDS != 0 {
            if kinds & KIND_ISO != 0 && micros.unsigned_abs() >= 60_000_000 {
                DurationStyle::Clock
            } else {
                DurationStyle::Seconds
            }
        } else {
            DurationStyle::Iso
        }
    }

    fn parse(text: &str) -> Option<DurationStyle> {
        match text.trim() {
            "iso" => Some(DurationStyle::Iso),
            "%H:%M:%S" => Some(DurationStyle::Clock),
            _ => None,
        }
    }
}

impl fmt::Display for DurationStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DurationStyle::Iso => "iso",
            DurationStyle::Clock => "%H:%M:%S",
            DurationStyle::Seconds => "s",
            DurationStyle::Auto(_) => "auto",
        })
    }
}

/// How a column of booleans is shown: any two different characters, the one
/// for true and then the one for false, as `10`, `xo`, `XO` or `✓✗`. Without
/// one it shows `true` and `false`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoolFormat {
    pub yes: char,
    pub no: char,
}

impl BoolFormat {
    pub fn parse(text: &str) -> Result<BoolFormat, FormatError> {
        let n = text.chars().count();
        let mut chars = text.chars();
        let (Some(yes), Some(no), None) = (chars.next(), chars.next(), chars.next()) else {
            return Err(FormatError {
                at: 0,
                len: n.max(1),
                message: format!("a `Boolean` format is two characters, and `{text}` is {n}"),
            });
        };
        // A `]` would end the format and a `%` is a date's, and a space or a
        // control character cannot be seen to be either.
        if let Some((at, c)) = [yes, no]
            .into_iter()
            .enumerate()
            .find(|&(_, c)| c == ']' || c == '%' || c.is_whitespace() || c.is_control())
        {
            return Err(FormatError {
                at,
                len: 1,
                message: format!("`{}` cannot be a symbol of a `Boolean`", c.escape_debug()),
            });
        }
        if yes == no {
            return Err(FormatError {
                at: 1,
                len: 1,
                message: format!("true and false are both `{yes}`"),
            });
        }
        Ok(BoolFormat { yes, no })
    }

    pub fn show(&self, b: bool) -> String {
        (if b { self.yes } else { self.no }).to_string()
    }
}

impl fmt::Display for BoolFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.yes, self.no)
    }
}

/// How a column of quantities is shown: in a unit of its dimension,
/// `Weight : Mass[g]`, and optionally by a number format as a `Scientific`
/// takes, after a `;`: `Mass[g; ±.3e-3]` (add-si-units D233). The unit is also
/// the unit the column holds its values in, so a bare number in it is read in
/// that unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantityFormat {
    /// `None` where the format names no unit, `Quantity[; .9s]`: each value
    /// is shown in its own unit, or the kind's own unit for a named kind.
    pub unit: Option<siquant::Unit>,
    pub number: Option<SciFormat>,
}

impl QuantityFormat {
    /// Read `text`, a unit and perhaps `;` and a number format, for a column
    /// of the quantity type `id`.
    pub fn parse(text: &str, id: QId) -> Result<QuantityFormat, FormatError> {
        let (unit_part, number_part) = match text.split_once(';') {
            Some((u, n)) => (u, Some(n)),
            None => (text, None),
        };
        let lead = |s: &str| s.chars().count() - s.trim_start().chars().count();
        let at = lead(unit_part);
        let unit_text = unit_part.trim();
        // `*m`: the unit with whichever prefix suits each value.
        let star = unit_text.starts_with('*');
        let (at, unit_text) = if star {
            (at + 1, unit_text[1..].trim_start())
        } else {
            (at, unit_text)
        };
        let error = |at: usize, len: usize, message: String| FormatError {
            at,
            len: len.max(1),
            message,
        };
        // The unit may be left out when a number format follows the `;`.
        if unit_text.is_empty() && number_part.is_none() && !star {
            return Err(error(0, 1, "needs a unit".to_string()));
        }
        if star && unit_text.is_empty() && id.named().is_none() {
            return Err(error(0, 1, "`*` needs a unit: write `*m`".to_string()));
        }
        let len = unit_text.chars().count();
        let unit = if unit_text.is_empty() {
            None
        } else {
            let unit = siquant::Unit::parse(unit_text, siquant::Scope::Any)
                .map_err(|_| error(at, len, format!("`{unit_text}` is not a unit")))?;
            if let Some(t) = id.named()
                && !t.admits(&unit)
            {
                return Err(error(
                    at,
                    len,
                    format!("must be a unit of {}", crate::units::kinds_of(t.dims())),
                ));
            }
            if star && !takes_si_prefixes(unit_text) {
                return Err(error(
                    at,
                    len,
                    "`*` is for a metric unit without its prefix, as `*g` for grams".to_string(),
                ));
            }
            Some(unit)
        };
        let number = match number_part {
            None => None,
            Some(n) => {
                let start = unit_part.chars().count() + 1 + lead(n);
                Some(SciFormat::parse_in(n, true).map_err(|mut e| {
                    e.at += start;
                    e
                })?)
            }
        };
        let number = if star {
            let mut n = number.unwrap_or_default();
            n.prefixed = true;
            n.unit_down = true;
            Some(n)
        } else {
            number
        };
        // A sign is for money: the kind `Currency`, or a `Quantity` of any.
        if let Some(n) = number
            && n.sign
            && id.named().is_some_and(|t| t.name() != "Currency")
        {
            let start = unit_part.chars().count() + 1;
            return Err(error(
                start,
                number_part.map_or(1, |s| s.chars().count()).max(1),
                "`$` is for a currency".to_string(),
            ));
        }
        // A named kind with no unit is shown in its preferred unit.
        let lifts = unit.is_some() || id.named().is_some();
        if let Some(n) = number {
            if n.unit_down && !n.sign && !lifts {
                return Err(error(
                    0,
                    unit_part.chars().count().max(1),
                    "`v` needs a unit: with none, each value is shown in its own".to_string(),
                ));
            }
            if n.gap && lifts && !n.unit_down {
                let start = unit_part.chars().count() + 1;
                return Err(error(
                    start,
                    number_part.map_or(1, |s| s.chars().count()).max(1),
                    "`_` spaces the unit in each row, which is shown in the head: write `v_`"
                        .to_string(),
                ));
            }
        }
        Ok(QuantityFormat { unit, number })
    }

    /// Whether the column shows its unit once, in its head, and its cells
    /// as numbers: a column that has a unit, unless `v` puts it in each row.
    /// `kind` is whether its type names a kind, which has a unit of its own.
    pub fn lifts(&self, kind: bool) -> bool {
        (self.unit.is_some() || kind) && !self.number.is_some_and(|n| n.unit_down)
    }
}

/// Whether `symbol` is a metric unit as the registry has it, with no prefix
/// of its own, and takes one: `g` and `m`, not `km` or `in`.
fn takes_si_prefixes(symbol: &str) -> bool {
    siquant::Registry::builtin().units().any(|u| {
        u.symbol == symbol
            && matches!(
                u.prefixable,
                siquant::Prefixable::Si | siquant::Prefixable::Both
            )
    })
}

impl fmt::Display for QuantityFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(n) = self.number.filter(|n| n.prefixed) {
            let n = SciFormat {
                prefixed: false,
                unit_down: false,
                ..n
            };
            f.write_str("*")?;
            if let Some(u) = &self.unit {
                write!(f, "{u}")?;
            }
            let rest = n.to_string();
            return if rest.is_empty() {
                Ok(())
            } else {
                write!(f, "; {rest}")
            };
        }
        match (&self.unit, &self.number) {
            (Some(u), Some(n)) => write!(f, "{u}; {n}"),
            (Some(u), None) => write!(f, "{u}"),
            (None, Some(n)) => write!(f, "; {n}"),
            (None, None) => Ok(()),
        }
    }
}

/// How a `Scientific` column shows its values, as a `Float` column is given
/// its, by a C `printf` conversion without its `%` (D100), with `±` before it
/// for `value ± uncertainty` in place of the concise `value(nn)`, or `~` for
/// the value alone:
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
    /// `~`: the value alone, with no uncertainty shown.
    pub value_only: bool,
    /// `_`: a quantity's unit set apart from its number by a space, so
    /// `6.022e+23 mol^-1`.
    pub gap: bool,
    /// `v`: a quantity's unit shown down in each row, `10.5g`, rather than
    /// once in the column's head. Only a quantity format takes it.
    pub unit_down: bool,
    /// `*` before a quantity's unit, `Length[*m]`: each value is shown with
    /// the prefix that suits it (`2mm`, `1.5km`), in the unit's own row.
    /// It puts the unit down, as `v` does; only a quantity format takes it.
    pub prefixed: bool,
    /// `^`: a quantity's unit written as the parser reads it, `m.s^-1`, where
    /// it is otherwise shown as the SI recommends, `m⋅s⁻¹`.
    pub ascii: bool,
    /// `$`: money shown with its sign before the number in each row, `€4.55`,
    /// and its code after where it has no sign, `4.55CHF`, rather than once
    /// in the column's head. It puts the unit down, as `v` does, and a
    /// currency format takes it.
    pub sign: bool,
    /// `$>`: as `$`, with the sign after the number, `144¥`; `_` then sets it
    /// apart by a space, `144 ¥`.
    pub sign_after: bool,
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
    /// `s`: as `g`, but the uncertainty leads. A value with an uncertainty
    /// is shown by it, `6.674 30(15)e-11`, whatever the precision; a value
    /// with none, which has no digits of its own to go by, is shown to the
    /// precision as significant digits: `.9s` is `1.05457182e-34`.
    Auto,
}

/// The most places a number has.
const MOST_PLACES: u32 = 17;

impl SciFormat {
    pub fn parse(text: &str) -> Result<SciFormat, FormatError> {
        SciFormat::parse_in(text, false)
    }

    /// As [`SciFormat::parse`], where `v` is read too if `quantity`.
    fn parse_in(text: &str, quantity: bool) -> Result<SciFormat, FormatError> {
        let chars: Vec<char> = text.trim().chars().collect();
        let error = |at: usize, len: usize, message: &str| FormatError {
            at,
            len: len.max(1),
            message: message.to_string(),
        };
        let mut f = SciFormat::default();
        let mut i = 0;
        if chars.is_empty() {
            return Err(error(0, 0, "empty"));
        }
        // `±`, `~`, `v` and `_` come first, each once, in that order.
        if chars.first() == Some(&'±') {
            f.plus_minus = true;
            i += 1;
        } else if chars.first() == Some(&'~') {
            f.value_only = true;
            i += 1;
        }
        if quantity && chars.get(i) == Some(&'v') {
            f.unit_down = true;
            i += 1;
        } else if quantity && chars.get(i) == Some(&'$') {
            f.unit_down = true;
            f.sign = true;
            i += 1;
            if chars.get(i) == Some(&'>') {
                f.sign_after = true;
                i += 1;
            }
        }
        if chars.get(i) == Some(&'_') {
            f.gap = true;
            i += 1;
        }
        if quantity && chars.get(i) == Some(&'^') {
            f.ascii = true;
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
                    return Err(error(start, n.len(), &format!("width at most {MAX_WIDTH}")));
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
                        &format!("at most {MOST_PLACES} places"),
                    ));
                }
            }
        }
        let named = i > flags;
        match chars.get(i) {
            None if !named => return Ok(f),
            None => {
                return Err(error(i, 0, "must end in 'e', 'f', 'g' or 's'"));
            }
            Some(c @ ('f' | 'F' | 'e' | 'E' | 'g' | 'G' | 's')) => {
                f.conv = match c.to_ascii_lowercase() {
                    'f' => SciConv::Fixed,
                    'e' => SciConv::Exp,
                    's' => SciConv::Auto,
                    _ => SciConv::General,
                };
                f.upper = c.is_ascii_uppercase();
            }
            Some(c @ ('d' | 'i' | 'x' | 'X' | 'o' | 'b' | 'B')) => {
                return Err(error(
                    i,
                    1,
                    &format!("`{c}` is for whole numbers: use 'e', 'f', 'g' or 's'"),
                ));
            }
            Some(c) => {
                return Err(error(i, 1, &format!("`{c}` is not 'e', 'f', 'g' or 's'")));
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
                    return Err(error(start, i - start, "exponent at most 300"));
                }
            }
        }
        if i != chars.len() {
            return Err(error(i, chars.len() - i, "nothing after the conversion"));
        }
        if f.group && f.conv == SciConv::Exp {
            return Err(error(flags, i - flags, "'  groups only 'f' and 'g'"));
        }
        Ok(f)
    }
}

impl fmt::Display for SciFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.plus_minus {
            f.write_str("±")?;
        }
        if self.value_only {
            f.write_str("~")?;
        }
        if self.sign {
            f.write_str("$")?;
            if self.sign_after {
                f.write_str(">")?;
            }
        } else if self.unit_down {
            f.write_str("v")?;
        }
        if self.gap {
            f.write_str("_")?;
        }
        if self.ascii {
            f.write_str("^")?;
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
            (SciConv::Auto, _) => Some('s'),
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
            S::Quantity(id) => QuantityFormat::parse(text, id).map(Format::Quantity),
            S::Duration => match DurationStyle::parse(text) {
                Some(style) => Ok(Format::Duration(style)),
                None => match QId::from_name("Duration") {
                    Some(id) => QuantityFormat::parse(text, id).map(Format::Quantity),
                    None => Err(FormatError {
                        at: 0,
                        len: 1,
                        message: "a `Duration` format is a unit of time".to_string(),
                    }),
                },
            },
            S::Date | S::Time | S::Datetime => DateFormat::parse(text).map(Format::Date),
            S::Boolean => BoolFormat::parse(text).map(Format::Boolean),
            S::String | S::Email | S::Url => TextFlow::parse(text).map(Format::Text).ok_or(
                FormatError {
                    at: 0,
                    len: text.len().max(1),
                    message: "a text format is `clip` or `wrap`".to_string(),
                },
            ),
            _ => NumFormat::parse(text).map(Format::Number),
        }
    }

    /// The format `text` for a value of type `ty`, or why there is none: it
    /// does not read, or the type does not take it.
    pub fn read_for(text: &str, ty: S) -> Result<Format, String> {
        let Some(takes) = Format::conversions_of(ty) else {
            return Err(format!("{ty} does not take a format"));
        };
        let f =
            Format::parse(text, ty).map_err(|e| format!("{} ({})", e.message, Format::help(ty)))?;
        if f.takes(ty) {
            Ok(f)
        } else {
            Err(format!(
                "{ty} does not take the format `{f}`: it takes {takes}"
            ))
        }
    }

    /// Whether a column of type `ty` takes this format.
    pub fn takes(&self, ty: S) -> bool {
        match self {
            Format::Number(f) => f.takes(ty),
            Format::Date(f) => f.takes(ty),
            Format::Scientific(_) => ty == S::Scientific,
            Format::Boolean(_) => ty == S::Boolean,
            Format::Text(_) => matches!(ty, S::String | S::Email | S::Url),
            Format::Duration(_) => ty == S::Duration,
            Format::Quantity(q) => match ty {
                S::Duration => q
                    .unit
                    .as_ref()
                    .is_none_or(|u| u.dims() == siquant::Dim::of(siquant::Base::Time)),
                S::Quantity(id) => id
                    .named()
                    .is_none_or(|t| q.unit.as_ref().is_none_or(|u| t.admits(u))),
                _ => false,
            },
        }
    }

    /// What a column of type `ty` takes, for a diagnostic, or `None` for a
    /// type that takes no format.
    pub fn conversions_of(ty: S) -> Option<&'static str> {
        match ty {
            S::Scientific => Some("`e`, `f`, `g` or `s`, with `±` first"),
            S::Quantity(_) => Some(
                "a unit of its dimension, as `g`, then `;` and a number format, or `;` and a number format alone",
            ),
            S::Duration => Some(
                "a unit of time, as `h`, then `;` and a number format; or `iso`, or `%H:%M:%S`",
            ),
            S::Date => Some("the conversions of a date, as `%d %b %Y`"),
            S::Boolean => Some("two different characters, as `10`, `xo` or `✓✗`"),
            S::String | S::Email | S::Url => Some("`clip` or `wrap`"),
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
            S::Quantity(_) => {
                "a quantity format is a unit, as `g`, which may be followed by `;` and a number format, as `g; ±.3e-3`"
            }
            S::Duration => {
                "a `Duration` format is a unit of time, as `h`, which may be followed by `;` and a number format, or `iso`, or `%H:%M:%S`"
            }
            S::Boolean => {
                "a `Boolean` format is two different characters: the symbol for true, then for false, as `xo` or `✓✗`"
            }
            S::String | S::Email | S::Url => {
                "a text format is `clip`, to cut a long cell off at its column, or `wrap`, to carry it on to more lines; with neither it spills over the empty cells to its right"
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
            Format::Duration(d) => d.fmt(f),
            Format::Quantity(q) => q.fmt(f),
            Format::Boolean(b) => b.fmt(f),
            Format::Text(t) => t.fmt(f),
        }
    }
}

/// A unit as the SI recommends it be written, `m⋅s⁻¹`, from the notation the
/// parser reads, `m.s^-1`: a dot operator between terms and exponents as
/// superscripts. A scale's own dot, `k.`, stays.
pub fn pretty_unit(unit: &str) -> String {
    let mut out = String::with_capacity(unit.len());
    let chars: Vec<char> = unit.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '.' if i > 0 && chars[i - 1] != '.' && chars.get(i + 1).is_some_and(|&n| n != '.') => {
                out.push('⋅');
            }
            '^' if chars
                .get(i + 1)
                .is_some_and(|&n| n == '-' || n.is_ascii_digit()) =>
            {
                i += 1;
                while let Some(&c) = chars.get(i) {
                    out.push(match c {
                        '-' => '⁻',
                        '0' => '⁰',
                        '1' => '¹',
                        '2' => '²',
                        '3' => '³',
                        '4' => '⁴',
                        '5' => '⁵',
                        '6' => '⁶',
                        '7' => '⁷',
                        '8' => '⁸',
                        '9' => '⁹',
                        _ => break,
                    });
                    i += 1;
                }
                continue;
            }
            c => out.push(c),
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_boolean_format_is_a_pair_of_symbols() {
        for (text, yes, no) in [("10", "1", "0"), ("xo", "x", "o"), ("XO", "X", "O")] {
            let Ok(Format::Boolean(b)) = Format::parse(text, S::Boolean) else {
                panic!("{text} should read");
            };
            assert_eq!((b.show(true), b.show(false)), (yes.into(), no.into()));
            assert_eq!(b.to_string(), text);
        }
        // Any two characters, but not the same, nor a `]`, a `%` or a gap.
        let Ok(Format::Boolean(b)) = Format::parse("✓✗", S::Boolean) else {
            panic!("✓✗ should read");
        };
        assert_eq!((b.show(true), b.show(false)), ("✓".into(), "✗".into()));
        assert_eq!(Format::parse("yn", S::Boolean).unwrap().to_string(), "yn");
        for bad in ["", "x", "xxo", "xx", "x]", "%x", "x ", "\tx"] {
            assert!(Format::parse(bad, S::Boolean).is_err(), "{bad:?}");
        }
        assert!(!Format::parse("xo", S::Boolean).unwrap().takes(S::Int));
    }

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
    fn the_s_conversion_and_a_quantity_format_with_no_unit() {
        let f = SciFormat::parse(".9s").unwrap();
        assert_eq!((f.conv, f.places), (SciConv::Auto, Some(9)));
        assert_eq!(f.to_string(), ".9s");
        assert!(SciFormat::parse("S").is_err());
        let any = QId::ANY;
        let q = QuantityFormat::parse("; .9s", any).unwrap();
        assert!(q.unit.is_none());
        assert_eq!(q.to_string(), "; .9s");
        assert!(QuantityFormat::parse("", any).is_err());
        let gap = QuantityFormat::parse("; _.9s", any).unwrap();
        assert!(gap.number.unwrap().gap);
        assert_eq!(gap.to_string(), "; _.9s");
        assert!(SciFormat::parse("_").unwrap().gap);
        assert_eq!(SciFormat::parse("±_.2f").unwrap().to_string(), "±_.2f");
        assert!(QuantityFormat::parse("g", any).unwrap().unit.is_some());
    }

    #[test]
    fn the_unit_is_in_the_head_unless_v_puts_it_in_the_rows() {
        let mass = QId::from_name("Mass").unwrap();
        // A unit, or a kind that has one, is lifted into the head.
        assert!(QuantityFormat::parse("g", mass).unwrap().lifts(false));
        assert!(QuantityFormat::parse("g; .1f", mass).unwrap().lifts(false));
        assert!(QuantityFormat::parse("; .1f", mass).unwrap().lifts(true));
        // `v` keeps it in each row, and is written back as read.
        let down = QuantityFormat::parse("g; v.1f", mass).unwrap();
        assert!(!down.lifts(false));
        assert_eq!(down.to_string(), "g; v.1f");
        assert_eq!(
            QuantityFormat::parse("g; v_.1f", mass).unwrap().to_string(),
            "g; v_.1f"
        );
        assert_eq!(
            QuantityFormat::parse("g; ±v.1f", mass).unwrap().to_string(),
            "g; ±v.1f"
        );
        // A quantity with no unit has one in each row already.
        assert!(
            !QuantityFormat::parse("; .9s", QId::ANY)
                .unwrap()
                .lifts(false)
        );
        let e = QuantityFormat::parse("; v.9s", QId::ANY).unwrap_err();
        assert!(e.message.contains("needs a unit"), "{}", e.message);
        // `_` spaces a unit in the row, so it needs `v` where there is one.
        let e = QuantityFormat::parse("g; _.1f", mass).unwrap_err();
        assert!(e.message.contains("v_"), "{}", e.message);
        assert!(QuantityFormat::parse("; _.9s", QId::ANY).is_ok());
        // Only a quantity takes it.
        assert!(SciFormat::parse("v.1f").is_err());
    }

    #[test]
    fn reads_by_the_type() {
        assert!(matches!(Format::parse("%d", S::Date), Ok(Format::Date(_))));
        assert!(matches!(Format::parse("d", S::Int), Ok(Format::Number(_))));
        assert!(Format::parse("%d", S::Int).is_err());
        assert!(!Format::parse("%d", S::Date).unwrap().takes(S::Int));
    }

    #[test]
    fn a_quantity_format_is_a_unit_and_perhaps_a_number_format() {
        let id = QId::from_name("Mass").unwrap();
        let ty = S::Quantity(id);
        for text in ["g", "kg", "mg; .1f", "g; ±.3e-3", "kg.m^2.s^-2"] {
            // Kinds that are not mass are refused for a Mass column.
            let read = Format::parse(text, ty);
            if text == "kg.m^2.s^-2" {
                assert!(read.is_err(), "{text}");
                continue;
            }
            let f = read.unwrap_or_else(|e| panic!("{text}: {e:?}"));
            assert!(f.takes(ty), "{text}");
            // What is written is what is read.
            assert_eq!(Format::parse(&f.to_string(), ty).unwrap(), f, "{text}");
        }
        let f = Format::parse("g", ty).unwrap();
        // It follows its column to a type that still takes it, and no further.
        assert!(f.takes(S::Quantity(QId::ANY)));
        assert!(!f.takes(S::Quantity(QId::from_name("Length").unwrap())));
        assert!(!f.takes(S::Float));
        let e = Format::parse("m", ty).unwrap_err();
        assert!(e.message.contains("unit of Mass"), "{}", e.message);
        assert_eq!(e.at, 0);
        // A number format's error is placed after the unit and the `;`.
        let e = Format::parse("g; x", ty).unwrap_err();
        assert_eq!(e.at, 3, "{e:?}");
    }

    #[test]
    fn a_dollar_sign_shows_money_by_its_sign() {
        let money = QId::from_name("Currency").unwrap();
        let f = QuantityFormat::parse("EUR; $", money).unwrap();
        assert!(f.number.unwrap().sign);
        // The sign is in each row, so the head holds no unit.
        assert!(!f.lifts(true));
        assert_eq!(f.to_string(), "EUR; $");
        let after = QuantityFormat::parse("JPY; $>_.0f", money).unwrap();
        assert!(after.number.unwrap().sign_after);
        assert_eq!(after.to_string(), "JPY; $>_.0f");
        assert_eq!(
            QuantityFormat::parse("USD; $'.2f", money)
                .unwrap()
                .to_string(),
            "USD; $'.2f"
        );
        // With no unit, each value by its own sign.
        assert!(QuantityFormat::parse("; $", QId::ANY).is_ok());
        // Only money has one.
        let mass = QId::from_name("Mass").unwrap();
        let e = QuantityFormat::parse("g; $", mass).unwrap_err();
        assert!(e.message.contains("currency"), "{}", e.message);
    }

    #[test]
    fn units_are_set_as_the_si_writes_them_and_a_caret_asks_for_the_ascii() {
        assert_eq!(pretty_unit("m.s^-1"), "m⋅s⁻¹");
        assert_eq!(pretty_unit("kg.m^2.s^-2"), "kg⋅m²⋅s⁻²");
        assert_eq!(pretty_unit("kg"), "kg");
        assert_eq!(pretty_unit("k."), "k.");
        let speed = QId::ANY;
        let f = QuantityFormat::parse("m.s^-1; ^", speed).unwrap();
        assert!(f.number.unwrap().ascii);
        assert_eq!(f.to_string(), "m.s^-1; ^");
        assert!(SciFormat::parse_in("v_^.1f", true).unwrap().ascii);
        // Only a quantity's format takes it.
        assert!(SciFormat::parse_in("^", false).is_err());
    }

    #[test]
    fn a_star_shows_each_value_in_the_prefix_that_suits_it() {
        let length = QId::from_name("Length").unwrap();
        let f = QuantityFormat::parse("*m", length).unwrap();
        assert!(f.number.unwrap().prefixed);
        // The unit is in each row, so the head holds none.
        assert!(!f.lifts(true));
        assert_eq!(f.to_string(), "*m");
        assert_eq!(
            QuantityFormat::parse("*m; .2f", length)
                .unwrap()
                .to_string(),
            "*m; .2f"
        );
        // A bare star of a kind is its preferred unit.
        assert!(QuantityFormat::parse("*", length).is_ok());
        assert!(QuantityFormat::parse("*", QId::ANY).is_err());
        // Only a metric unit with no prefix of its own takes it.
        assert!(QuantityFormat::parse("*km", length).is_err());
        assert!(QuantityFormat::parse("*in", length).is_err());
    }
}

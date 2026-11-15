// Copyright (c) 2026 Stephen Roe

//! Degrees, minutes and seconds of arc: `45°30′30″`.
//!
//! The entry form of an angle. A leading `-` makes it negative; the marks
//! are `°` `′` `″`, with `º`, `'` and `"` as the alternates a keyboard
//! has. Minutes and seconds are each under 60, and only the last part
//! written may have a decimal point: `45°30.5′` and `45°30′30.25″` are
//! right, `45.5°30′` is not. The result is a [`Quantity`] in degrees with no
//! uncertainty; the host reads any uncertainty and scale itself.

use crate::dim::{Base, Dim};
use crate::error::{Error, Result};
use crate::quantity::Quantity;
use crate::registry::{Registry, Scope};
use crate::unit::Unit;

fn bad<T>(at: usize, message: &str) -> Result<T> {
    Err(Error::Syntax {
        at,
        message: message.to_string(),
    })
}

/// The number at the start of `rest`, its text and what follows.
fn number(rest: &str) -> Option<(&str, &str)> {
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let mut end = digits;
    if rest[end..].starts_with('.') {
        let frac = rest[end + 1..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        if frac == 0 {
            return None;
        }
        end += 1 + frac;
    }
    Some((&rest[..end], &rest[end..]))
}

/// The mark at the start of `rest`, which of the three it is (0, 1, 2), and
/// what follows it.
fn mark(rest: &str) -> Option<(usize, &str)> {
    const MARKS: [(&str, usize); 7] = [
        ("°", 0),
        ("º", 0),
        ("′", 1),
        ("'", 1),
        ("″", 2),
        ("\"", 2),
        ("''", 2),
    ];
    // `''` before `'`: the longer mark wins.
    let mut found: Option<(&str, usize)> = None;
    for (text, which) in MARKS {
        if rest.starts_with(text) && found.is_none_or(|(t, _)| text.len() > t.len()) {
            found = Some((text, which));
        }
    }
    found.map(|(text, which)| (which, &rest[text.len()..]))
}

impl Quantity {
    /// An angle written in degrees, minutes and seconds, as `45°30′30″`,
    /// `-12°30′` or `0°0′1.5″`, in degrees.
    pub fn from_dms(text: &str) -> Result<Quantity> {
        let whole = text;
        let (negative, mut rest) = match text.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        let mut parts = [0.0f64; 3];
        let mut next = 0;
        let mut fractional = false;
        while !rest.is_empty() {
            let at = whole.len() - rest.len();
            if fractional {
                return bad(at, "only the last part may have a decimal point");
            }
            let Some((digits, after)) = number(rest) else {
                return bad(at, "a number is expected");
            };
            let Some((which, after)) = mark(after) else {
                return bad(
                    whole.len() - after.len(),
                    "a mark `°`, `′` or `″` is expected after the number",
                );
            };
            if which < next {
                return bad(at, "the parts go degrees, minutes, seconds, each once");
            }
            if next == 0 && which != 0 {
                return bad(at, "degrees come first: write `0°` for none");
            }
            let value: f64 = digits.parse().map_err(|_| Error::Overflow)?;
            if which > 0 && value >= 60.0 {
                return bad(at, "minutes and seconds are under 60");
            }
            fractional = digits.contains('.');
            parts[which] = value;
            next = which + 1;
            rest = after;
        }
        if next == 0 {
            return bad(0, "an angle such as `45°30′30″` is expected");
        }
        let degrees = parts[0] + parts[1] / 60.0 + parts[2] / 3600.0;
        let unit = Unit::parse("°", Scope::Live)?;
        Ok(Quantity::exact(
            if negative { -degrees } else { degrees },
            unit,
        ))
    }

    /// An angle as degrees, minutes and seconds, `45°30′30″`, with the
    /// seconds to `places` decimal places (at most 9); the result is rounded
    /// to them, so `59.9999″` at two places carries into the minute. The
    /// uncertainty is not shown.
    pub fn to_dms(&self, places: usize) -> Result<String> {
        let reg: &Registry = self.unit().registry();
        let degree = reg.parse_unit("°", Scope::Live)?;
        if self.unit().dims() != Dim::of(Base::Angle) {
            return Err(Error::DimensionMismatch {
                left: self.unit().dims(),
                right: Dim::of(Base::Angle),
            });
        }
        let places = places.min(9);
        let scale = 10u64.pow(places as u32);
        let degrees = self.convert(&degree)?.value();
        if !degrees.is_finite() {
            return Err(Error::Overflow);
        }
        let total = (degrees.abs() * 3600.0 * scale as f64).round();
        if total >= u64::MAX as f64 {
            return Err(Error::Overflow);
        }
        let total = total as u64;
        let (d, rest) = (total / (3600 * scale), total % (3600 * scale));
        let (m, s) = (rest / (60 * scale), rest % (60 * scale));
        let sign = if degrees < 0.0 && total > 0 { "-" } else { "" };
        let seconds = if places == 0 {
            format!("{}", s / scale)
        } else {
            format!("{}.{:0width$}", s / scale, s % scale, width = places)
        };
        Ok(format!("{sign}{d}°{m}′{seconds}″"))
    }
}

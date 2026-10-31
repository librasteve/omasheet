// Copyright (c) 2026 Stephen Roe

//! A length of time as a count of **microseconds**, the unit Polars holds a
//! `Duration("us")` in, written and shown as ISO 8601: `PT30M`, `PT1H30M`,
//! `P1DT6H`, `PT0.5S`, `P2W`, and `-PT30M` for a negative one (D22).
//!
//! Only the units of a fixed length are read: weeks, days, hours, minutes
//! and seconds. A year and a month are not a fixed count of microseconds, so
//! `P1Y` and `P1M` are refused rather than guessed at.

use crate::date::{DAY_MICROS, MICROS};

/// Why some text is not a duration.
#[derive(Debug, PartialEq)]
pub enum NotDuration {
    /// It is not shaped as one, and may be a name: `P1` or `Price`.
    Shape,
    /// It is one, but says why it cannot be held.
    Says(String),
}

/// The duration written as `text`, which starts with `P`.
pub fn parse(text: &str) -> Result<i64, NotDuration> {
    let b = text.as_bytes();
    if b.first() != Some(&b'P') {
        return Err(NotDuration::Shape);
    }
    // Each unit's letter, its length, and whether it is one of the time part
    // after the `T`. They are written in this order, each at most once.
    const UNITS: [(u8, i64, bool); 5] = [
        (b'W', 7 * DAY_MICROS, false),
        (b'D', DAY_MICROS, false),
        (b'H', 3_600 * MICROS, true),
        (b'M', 60 * MICROS, true),
        (b'S', MICROS, true),
    ];
    let mut i = 1;
    let mut timed = false;
    let mut next = 0;
    let mut total: i64 = 0;
    let mut parts = 0;
    let mut calendar = false;
    while i < b.len() {
        if b[i] == b'T' && !timed {
            timed = true;
            i += 1;
            if !b.get(i).is_some_and(u8::is_ascii_digit) {
                return Err(NotDuration::Shape);
            }
            continue;
        }
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return Err(NotDuration::Shape);
        }
        let whole = &text[start..i];
        let mut frac = "";
        if b.get(i) == Some(&b'.') {
            let at = i + 1;
            i = at;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if i == at {
                return Err(NotDuration::Shape);
            }
            frac = &text[at..i];
        }
        let Some(&letter) = b.get(i) else {
            return Err(NotDuration::Shape);
        };
        i += 1;
        if letter == b'Y' || (letter == b'M' && !timed) {
            calendar = true;
            parts += 1;
            continue;
        }
        let Some(k) = UNITS[next..]
            .iter()
            .position(|(l, _, t)| *l == letter && *t == timed)
        else {
            return Err(NotDuration::Shape);
        };
        let (_, unit, _) = UNITS[next + k];
        next += k + 1;
        parts += 1;
        if !frac.is_empty() && letter != b'S' {
            return Err(NotDuration::Says(format!(
                "only the seconds of a duration take a fraction, so `{whole}.{frac}{}` cannot be held",
                letter as char
            )));
        }
        if frac.len() > 6 {
            return Err(NotDuration::Says(format!(
                "a duration holds 6 digits of a second, so `.{frac}` is more of one than it can keep"
            )));
        }
        let micros = if frac.is_empty() {
            0
        } else {
            frac.parse::<i64>().unwrap() * 10i64.pow(6 - frac.len() as u32)
        };
        let part = whole
            .parse::<i64>()
            .ok()
            .and_then(|n| n.checked_mul(unit))
            .and_then(|n| n.checked_add(micros))
            .and_then(|n| total.checked_add(n));
        match part {
            Some(t) => total = t,
            None => {
                return Err(NotDuration::Says(format!(
                    "`{text}` is too long a duration"
                )));
            }
        }
    }
    if parts == 0 {
        return Err(NotDuration::Shape);
    }
    if calendar {
        return Err(NotDuration::Says(format!(
            "`{text}` has a year or a month in it, and a duration holds no calendar units"
        )));
    }
    Ok(total)
}

/// `micros` as ISO 8601, in days and the time after them: `P365D`,
/// `-PT1H30M`, `PT0.5S`, and `PT0S` for none. It reads back as the same
/// duration.
pub fn format(micros: i64) -> String {
    let sign = if micros < 0 { "-" } else { "" };
    let n = micros.unsigned_abs();
    let day = DAY_MICROS as u64;
    let (days, rest) = (n / day, n % day);
    let mut out = format!("{sign}P");
    if days > 0 {
        out += &format!("{days}D");
    }
    if rest > 0 || days == 0 {
        let second = MICROS as u64;
        let (h, m) = (rest / (3_600 * second), rest / (60 * second) % 60);
        let (s, frac) = (rest / second % 60, rest % second);
        out.push('T');
        if h > 0 {
            out += &format!("{h}H");
        }
        if m > 0 {
            out += &format!("{m}M");
        }
        if frac > 0 {
            let digits = format!("{frac:06}");
            out += &format!("{s}.{}S", digits.trim_end_matches('0'));
        } else if s > 0 || rest == 0 {
            out += &format!("{s}S");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600 * MICROS;

    #[test]
    fn reads_the_fixed_units() {
        assert_eq!(parse("PT30M"), Ok(30 * 60 * MICROS));
        assert_eq!(parse("PT1H30M"), Ok(HOUR * 3 / 2));
        assert_eq!(parse("P1DT6H"), Ok(DAY_MICROS + 6 * HOUR));
        assert_eq!(parse("PT0.5S"), Ok(MICROS / 2));
        assert_eq!(parse("P2W"), Ok(14 * DAY_MICROS));
        assert_eq!(parse("PT0S"), Ok(0));
    }

    #[test]
    fn a_name_is_not_a_duration() {
        for name in [
            "P", "PT", "P1", "Price", "PT1", "P1H", "PT1D", "P1D1W", "PT1M1H",
        ] {
            assert_eq!(parse(name), Err(NotDuration::Shape), "{name}");
        }
    }

    #[test]
    fn a_calendar_unit_is_refused() {
        for text in ["P1Y", "P1M", "P1Y2DT3H"] {
            assert!(matches!(parse(text), Err(NotDuration::Says(_))), "{text}");
        }
        assert!(matches!(parse("PT1.5H"), Err(NotDuration::Says(_))));
        assert!(matches!(parse("PT0.0000001S"), Err(NotDuration::Says(_))));
    }

    #[test]
    fn shown_as_it_is_written() {
        assert_eq!(format(365 * DAY_MICROS), "P365D");
        assert_eq!(format(-HOUR * 3 / 2), "-PT1H30M");
        assert_eq!(format(DAY_MICROS + 6 * HOUR), "P1DT6H");
        assert_eq!(format(MICROS / 2), "PT0.5S");
        assert_eq!(format(0), "PT0S");
        assert_eq!(format(61 * MICROS + 1), "PT1M1.000001S");
        for text in ["PT30M", "P1DT6H", "PT0.5S", "P14D", "PT1H0.25S"] {
            assert_eq!(format(parse(text).unwrap()), text);
        }
    }
}

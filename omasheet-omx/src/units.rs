// Copyright (c) 2026 Stephen Roe

//! What may follow a number: an SI unit or a scale (`add-si-units`, D229,
//! D231, D237).
//!
//! `5kg`, `9.81m.s^-2` and `20°C` are a number and a unit, read by the
//! `siquant` crate: `.`-separated terms, `^` and a signed integer exponent, no
//! space, no `/`. `1k.` is a scale, a prefix and a `.` on their own, and reads
//! as a number: 1000. This module finds where the unit or the scale ends; the
//! lexer makes the token.

use crate::types::QId;
use siquant::{Registry, Scope, Unit};

/// What was written after a number.
#[derive(Debug, PartialEq)]
pub enum Tail {
    /// A unit: the number is a quantity.
    Unit(Unit),
    /// A scale, `k.` or `Ki.`: the number times `base` to the `exp`.
    Scale { base: u8, exp: i16 },
}

/// Can `c` be part of a unit symbol? Letters, and the degree, minute and
/// second signs of an angle.
fn symbol_char(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '°' | '′' | '″')
}

/// The length in bytes of the run of symbol characters at the start of `s`.
fn run(s: &str) -> usize {
    s.char_indices()
        .find(|&(_, c)| !symbol_char(c))
        .map_or(s.len(), |(i, _)| i)
}

/// The digits at the start of `s`, in bytes.
fn digits(s: &str) -> usize {
    s.bytes().take_while(u8::is_ascii_digit).count()
}

/// An angle written in degrees, minutes and seconds, `45°30′30″`, that
/// starts at `text[start..]` with a number whose degree sign is at `at`: the
/// angle in degrees and the byte offset where it ends. `None` unless a
/// number follows the degree sign (`45°` and `20°C` are units, as ever).
pub fn dms(text: &str, start: usize, at: usize) -> Option<(Unit, f64, usize)> {
    let rest = &text[at..];
    let mark = rest.chars().next().filter(|c| matches!(c, '°' | 'º'))?;
    if !rest[mark.len_utf8()..].starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let reach = text[start..]
        .char_indices()
        .find(|&(_, c)| {
            !(c.is_ascii_digit() || matches!(c, '.' | '°' | 'º' | '′' | '\'' | '″' | '"'))
        })
        .map_or(text.len(), |(i, _)| start + i);
    // The longest run that is an angle: a trailing point or mark that
    // belongs to something else is left.
    let mut ends: Vec<usize> = text[start..reach]
        .char_indices()
        .filter(|&(_, c)| matches!(c, '°' | 'º' | '′' | '\'' | '″' | '"'))
        .map(|(i, c)| start + i + c.len_utf8())
        .collect();
    ends.reverse();
    ends.into_iter().find_map(|end| {
        let q = siquant::Quantity::from_dms(&text[start..end]).ok()?;
        Some((q.unit().clone(), q.value(), end))
    })
}

/// Is there something to read as a unit or a scale at `text[at..]`?
pub fn starts_one(text: &str, at: usize) -> bool {
    text[at..].chars().next().is_some_and(symbol_char)
}

/// The scale or the unit written at `text[at..]`, which follows a number, and
/// the byte offset where it ends. `None` if what follows is no unit and no
/// scale, which leaves the lexer to say the number is not one.
pub fn tail(text: &str, at: usize) -> Option<(Tail, usize)> {
    let rest = &text[at..];
    let first = run(rest);
    if first == 0 {
        return None;
    }

    // A prefix and a `.` on their own is a scale, unless something that
    // could continue a word, a call, a range or a unit follows the dot.
    if let Some(after) = rest[first..].strip_prefix('.') {
        let free = after
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '_' | '(' | '.')));
        if free && let Some((base, exp)) = Registry::builtin().scale(&rest[..=first]) {
            return Some((Tail::Scale { base, exp }, at + first + 1));
        }
    }

    // A unit: terms joined by `.`. A `.` goes on into another term only if
    // that term is not a call, so `5kg.abs()` is a quantity and a method.
    // Each place a unit could end is tried, longest first, so `5m.Float` is
    // the metre and a conversion.
    let mut ends: Vec<usize> = Vec::new();
    let mut pos = 0;
    loop {
        let symbol = run(&rest[pos..]);
        if symbol == 0 {
            break;
        }
        let mut end = pos + symbol;
        if rest[end..].starts_with('^') {
            let mut q = end + 1;
            if rest[q..].starts_with('-') {
                q += 1;
            }
            let d = digits(&rest[q..]);
            if d == 0 {
                break;
            }
            end = q + d;
        }
        ends.push(end);
        let Some(more) = rest[end..].strip_prefix('.') else {
            break;
        };
        let next = run(more);
        if next == 0 || more[next..].starts_with('(') {
            break;
        }
        pos = end + 1;
    }
    for &end in ends.iter().rev() {
        let word_goes_on = rest[end..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        if word_goes_on {
            continue;
        }
        if let Ok(unit) = Unit::parse(&rest[..end], Scope::Live) {
            return Some((Tail::Unit(unit), at + end));
        }
    }
    None
}

/// What a dimension is called: the names of the quantity types that have it
/// (`Mass`, `Energy/Torque`), or its symbols where none does.
pub fn kind_of(dims: siquant::Dim) -> String {
    let names: Vec<String> = Registry::builtin()
        .quantity_types_for(dims)
        .iter()
        .map(|t| t.name().to_string())
        .collect();
    if names.is_empty() {
        dims.to_string()
    } else {
        names.join("/")
    }
}

/// The unit a column of this type holds its values in, unless it says
/// another: the preferred unit of a named type, none for a `Quantity`.
pub fn preferred(id: QId) -> Option<Unit> {
    id.named()?.preferred_unit()
}

/// A value as a column of type `id` holds it, in the column's unit (D233).
/// `unit` is the unit the value was written in, `None` for a bare number,
/// which is read in the column's unit. Gives the value, uncertainty and unit
/// to hold, or `None` where it is held as it is; an error where a quantity
/// has the wrong dimension for a named type.
pub fn dress(
    value: f64,
    sigma: f64,
    unit: Option<&Unit>,
    id: QId,
    column: Option<&Unit>,
) -> Result<Option<(f64, f64, Unit)>, String> {
    match (unit, column) {
        (Some(u), column) => {
            if let Some(t) = id.named()
                && u.dims() != t.dims()
            {
                return Err(format!(
                    "a quantity in {u} is a {}, not a {}",
                    kind_of(u.dims()),
                    id.name()
                ));
            }
            match column {
                Some(c) if c != u => {
                    let q = siquant::Quantity::new(value, sigma, u.clone())
                        .and_then(|q| q.convert(c))
                        .map_err(|e| e.to_string())?;
                    Ok(Some((q.value(), q.sigma(), c.clone())))
                }
                _ => Ok(None),
            }
        }
        (None, Some(c)) => Ok(Some((value, sigma, c.clone()))),
        (None, None) => Ok(None),
    }
}

/// `base` to the power `exp` as a float.
pub fn power(base: u8, exp: i16) -> f64 {
    match base {
        2 => 2f64.powi(i32::from(exp)),
        // Read from text, so a power of ten is the nearest float to it.
        _ => format!("1e{exp}").parse().unwrap_or(f64::NAN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(text: &str) -> Option<(String, usize)> {
        match tail(text, 0)? {
            (Tail::Unit(u), end) => Some((u.to_string(), end)),
            _ => None,
        }
    }

    #[test]
    fn a_unit_ends_where_the_unit_ends() {
        assert_eq!(unit("kg"), Some(("kg".into(), 2)));
        assert_eq!(unit("kg + 1"), Some(("kg".into(), 2)));
        assert_eq!(unit("m.s^-2"), Some(("m.s^-2".into(), 6)));
        assert_eq!(unit("kg.m^2.s^-2)"), Some(("kg.m^2.s^-2".into(), 11)));
        assert_eq!(unit("°C"), Some(("°C".into(), 3)));
        assert_eq!(unit("μm"), Some(("μm".into(), 3)));
        // The exponent is attached to its term, and `-` only follows `^`.
        assert_eq!(unit("m^2-3m"), Some(("m^2".into(), 3)));
    }

    #[test]
    fn a_method_or_a_conversion_after_a_unit_is_not_part_of_it() {
        assert_eq!(unit("kg.abs()"), Some(("kg".into(), 2)));
        assert_eq!(unit("m.Float"), Some(("m".into(), 1)));
        assert_eq!(unit("m.to('g')"), Some(("m".into(), 1)));
        assert_eq!(unit("N.m.abs()"), Some(("N.m".into(), 3)));
        // But a unit after the dot is a unit.
        assert_eq!(unit("N.m"), Some(("N.m".into(), 3)));
        assert_eq!(unit("m.s"), Some(("m.s".into(), 3)));
    }

    #[test]
    fn what_is_not_a_unit() {
        for text in [
            "", " kg", "m2", "kgfoo", "ft", "k", "m^", "m^x", "5", "L", "1",
        ] {
            assert!(tail(text, 0).is_none(), "{text}");
        }
        assert!(tail("m s", 0).is_some()); // the metre; the space ends it
        assert_eq!(unit("m s"), Some(("m".into(), 1)));
    }

    #[test]
    fn a_prefix_and_a_dot_is_a_scale() {
        for (text, base, exp, end) in [
            ("k.", 10, 3, 2),
            ("k. + 1", 10, 3, 2),
            ("k.)", 10, 3, 2),
            ("m.", 10, -3, 2),
            ("T.", 10, 12, 2),
            ("da.", 10, 1, 3),
            ("μ.", 10, -6, 3),
        ] {
            assert_eq!(
                tail(text, 0),
                Some((Tail::Scale { base, exp }, end)),
                "{text}"
            );
        }
        // A dot that starts a word, a call, a range or another term is not
        // the end of a scale.
        for text in ["k.Float", "k.abs()", "k..5", "k.5"] {
            assert!(
                !matches!(tail(text, 0), Some((Tail::Scale { .. }, _))),
                "{text}"
            );
        }
    }

    #[test]
    fn powers() {
        assert_eq!(power(10, 3), 1000.0);
        assert_eq!(power(10, -3), 0.001);
        assert_eq!(power(2, 10), 1024.0);
    }
}

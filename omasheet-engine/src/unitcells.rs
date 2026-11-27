// Copyright (c) 2026 Stephen Roe

//! The cells to rewrite when a column of quantities is declared or its unit
//! changes: those with a unit of their own, to be bare numbers in the
//! column's, and bare numbers, to go on standing for what they did. A cell
//! is rewritten only where that can be done exactly.

use crate::dtype::quantity_of_text;
use omasheet_omx::S;
use omasheet_omx::ast::Lit;
use omasheet_omx::colfmt::{Format, pretty_unit, sub_unit_text};
use omasheet_omx::convert::{convert, parse_literal_with};
use omasheet_omx::file::{SheetDecl, split_cells};
use omasheet_omx::style::Style as CellStyle;
use siquant::{Quantity, Registry, Unit};

/// What a bare number stands for in a column that declares a unit.
#[derive(Clone, PartialEq)]
pub(crate) struct Meaning {
    ty: S,
    unit: Unit,
    /// `p` or `c`: a bare number is a hundredth of the unit.
    sub: Option<char>,
}

impl Meaning {
    /// That of column `name` as a sheet declares it, if it declares a unit.
    pub(crate) fn of(decl: &SheetDecl, name: &str) -> Option<Meaning> {
        let line = decl.schema.iter().find(|l| l.name == name)?;
        let format = match &line.format {
            Some(Format::Quantity(q)) => Some(q),
            _ => None,
        };
        let own = format.and_then(|q| q.unit.clone());
        let unit = match line.ty {
            S::Duration => own.or_else(|| Unit::parse("s", siquant::Scope::Any).ok()),
            S::Quantity(id) => own.or_else(|| omasheet_omx::units::preferred(id)),
            _ => None,
        }?;
        Some(Meaning {
            ty: line.ty,
            unit,
            sub: format.and_then(|q| q.number).and_then(|n| n.sub),
        })
    }

    fn scale(&self) -> f64 {
        if self.sub.is_some() { 0.01 } else { 1.0 }
    }

    /// The unit as it is said: `kg`, `m⋅s⁻¹`, `p⋅kWh⁻¹`.
    pub(crate) fn said(&self) -> String {
        let unit = self.unit.to_string();
        match self.sub {
            Some(c) => sub_unit_text(&unit, c),
            None => pretty_unit(&unit),
        }
    }

    /// Whether a bare number stands for the same here as there.
    pub(crate) fn same_numbers(&self, other: &Meaning) -> bool {
        self.unit == other.unit && self.sub.is_some() == other.sub.is_some()
    }
}

/// A bare number written as `text`: its value and uncertainty.
fn bare(text: &str) -> Option<(f64, f64)> {
    let lit = parse_literal_with(text, true)?;
    match lit {
        Lit::Scientific(v, s) => Some((v, s)),
        Lit::Int(..) | Lit::Decimal(_) | Lit::Fraction(_) | Lit::Float(_) => {
            match convert(&lit, S::Float) {
                Ok(Lit::Float(v)) => Some((v, 0.0)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// How many significant digits a number is written with.
fn digits(text: &str) -> usize {
    let mantissa = text.split(['e', 'E', '(', '±']).next().unwrap_or(text);
    let all: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    all.trim_start_matches('0').trim_end_matches('0').len()
}

/// `x` written in no more than `most` significant digits, or 12 if that is
/// more, where that is what it is and not a rounding of it.
fn short(x: f64, most: usize) -> Option<String> {
    let most = most.clamp(12, 14);
    let full = format!("{x:.14e}");
    let rounded: f64 = full.parse().ok()?;
    let held = (rounded - x).abs() <= x.abs() * 1e-14;
    (x.is_finite() && held && digits(&full) <= most).then(|| format!("{rounded}"))
}

/// The currencies a unit is in: an amount is not moved from one to another,
/// the rate between them being today's.
fn currencies(unit: &Unit) -> Vec<String> {
    unit.to_string()
        .split('.')
        .filter_map(|t| t.split('^').next())
        .filter(|c| Registry::builtin().rate(c).is_some())
        .map(str::to_string)
        .collect()
}

/// The cell `text`, which has a unit of its own, as a bare number in the
/// column that means `to`.
fn stripped(text: &str, to: &Meaning) -> Option<String> {
    let (v, s, unit) = quantity_of_text(text)?;
    if unit.dims() != to.unit.dims() || currencies(&unit) != currencies(&to.unit) {
        return None;
    }
    if unit == to.unit && to.sub.is_none() {
        // The number as it is written, uncertainty and all.
        let written = text
            .char_indices()
            .map(|(k, c)| text[..k + c.len_utf8()].trim_end())
            .filter(|head| bare(head) == Some((v, s)))
            .last();
        if let Some(head) = written {
            return Some(head.to_string());
        }
    }
    if s != 0.0 || digits(text) > 14 {
        return None;
    }
    let q = Quantity::new(v, 0.0, unit).ok()?.convert(&to.unit).ok()?;
    short(q.value() / to.scale(), digits(text)).filter(|t| bare(t).is_some())
}

/// The bare number `text`, which stood for what `from` says, written to
/// stand for the same in a column that means `to`: a number in its unit, or
/// where that cannot be exact, the number with the unit it had.
fn converted(text: &str, from: &Meaning, to: &Meaning) -> Option<String> {
    let (v, s) = bare(text)?;
    if from.unit.dims() != to.unit.dims() {
        return None;
    }
    if s == 0.0 && digits(text) <= 14 && currencies(&from.unit) == currencies(&to.unit) {
        let new = Quantity::new(v * from.scale(), 0.0, from.unit.clone())
            .ok()
            .and_then(|q| q.convert(&to.unit).ok())
            .and_then(|q| short(q.value() / to.scale(), digits(text)));
        if new.is_some() {
            return new;
        }
    }
    let number = match from.sub {
        None => text.to_string(),
        Some(_) if s == 0.0 => short(v * from.scale(), digits(text))?,
        Some(_) => return None,
    };
    let held = format!("{number}{}", from.unit);
    quantity_of_text(&held)
        .is_some_and(|(_, _, u)| u == from.unit)
        .then_some(held)
}

/// A cell to be rewritten: where it is, what it holds and what it would.
pub(crate) struct Rewrite {
    pub row: usize,
    pub col: usize,
    pub was: String,
    pub now: String,
    /// A bare number kept to its meaning, rather than a unit taken off.
    pub converts: bool,
}

/// The cells of column `col` to rewrite, its declaration having gone from
/// `from` to `to`.
pub(crate) fn rewrites(
    decl: &SheetDecl,
    col: usize,
    from: Option<&Meaning>,
    to: &Meaning,
) -> Vec<Rewrite> {
    let mut out = Vec::new();
    for (row, cells) in decl.rows.iter().enumerate() {
        let Some(cell) = cells.get(col) else { continue };
        let (style, text) = CellStyle::strip(&cell.text);
        let text = text.trim();
        if text.is_empty() || text.starts_with(['=', '"', '*']) {
            continue;
        }
        let (now, converts) = if bare(text).is_some() {
            match from {
                Some(from) if !from.same_numbers(to) => (converted(text, from, to), true),
                _ => continue,
            }
        } else {
            (stripped(text, to), false)
        };
        let Some(now) = now.filter(|n| n != text) else {
            continue;
        };
        // A rewrite must leave the line one of as many cells.
        if split_cells(&now, 0, 0).len() != 1 {
            continue;
        }
        out.push(Rewrite {
            row,
            col,
            was: text.to_string(),
            now: style.wrap(&now),
            converts,
        });
    }
    out
}

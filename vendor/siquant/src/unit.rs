// Copyright (c) 2026 Stephen Roe

//! A unit: an immutable, canonical list of prefixed unit terms.
//!
//! Compatibility between units is `dims` equality and nothing else; there is
//! no shadow of the definition, no "shortest name" rewriting and no type
//! inferred from the dimension.

use std::fmt;

use crate::dim::Dim;
use crate::error::{Error, Result};
use crate::registry::{PrefixId, Registry, Scope, UnitId};

/// One factor of a unit: a unit, with an optional prefix, to an exponent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Term {
    pub prefix: Option<PrefixId>,
    pub unit: UnitId,
    pub exp: i16,
}

/// A unit of measurement.
#[derive(Clone)]
pub struct Unit {
    reg: Registry,
    terms: Vec<Term>,
    dims: Dim,
    factor: f64,
    offset: f64,
}

impl Unit {
    /// The unit written as `text` in cell-text notation, in the built-in
    /// registry. `Scope::Live` allows only what may be written in a cell.
    pub fn parse(text: &str, scope: Scope) -> Result<Unit> {
        Registry::builtin().parse_unit(text, scope)
    }

    /// A unit from terms, normalised (merged, ordered, no zero exponents).
    pub(crate) fn from_terms(reg: &Registry, terms: Vec<Term>) -> Result<Unit> {
        let terms = reg.0.normalise(terms)?;
        let (dims, factor, offset) = reg.0.eval(&terms)?;
        Ok(Unit {
            reg: reg.clone(),
            terms,
            dims,
            factor,
            offset,
        })
    }

    /// The registry the unit belongs to.
    pub fn registry(&self) -> &Registry {
        &self.reg
    }

    /// The terms, in canonical order.
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    /// What the unit measures.
    pub fn dims(&self) -> Dim {
        self.dims
    }

    /// The size of the unit in the coherent SI unit of its dimension.
    pub fn factor(&self) -> f64 {
        self.factor
    }

    /// What is added, in the unit's own scale, before the factor (zero
    /// except for an affine unit such as `°C`).
    pub fn offset(&self) -> f64 {
        self.offset
    }

    /// Is this an affine unit, one with an offset?
    pub fn is_affine(&self) -> bool {
        self.offset != 0.0
    }

    /// Does this unit measure a pure number: no terms (`m.m^-1`, or
    /// [`Unit::one`]) or a scale (`k.`)?
    pub fn is_dimensionless(&self) -> bool {
        self.dims.is_dimensionless()
    }

    /// Is this a scale, a bare prefix such as `k.` (1000)?
    pub fn is_scale(&self) -> bool {
        matches!(self.terms.as_slice(), [t] if t.unit == self.reg.0.unity)
    }

    /// The unit of a plain number: no terms, factor one.
    pub fn one(reg: &Registry) -> Unit {
        Unit {
            reg: reg.clone(),
            terms: Vec::new(),
            dims: Dim::NONE,
            factor: 1.0,
            offset: 0.0,
        }
    }

    /// The product of two units. Affine units have no product.
    pub fn mul(&self, other: &Unit) -> Result<Unit> {
        let mut terms = self.terms.clone();
        terms.extend_from_slice(&other.terms);
        Unit::from_terms(&self.reg, terms)
    }

    /// The quotient of two units. Affine units have no quotient.
    pub fn div(&self, other: &Unit) -> Result<Unit> {
        self.mul(&other.invert()?)
    }

    /// One over the unit.
    pub fn invert(&self) -> Result<Unit> {
        self.pow(-1)
    }

    /// The unit to an integer power.
    pub fn pow(&self, n: i16) -> Result<Unit> {
        let terms = self
            .terms
            .iter()
            .map(|t| {
                Ok(Term {
                    exp: t.exp.checked_mul(n).ok_or(Error::Overflow)?,
                    ..*t
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Unit::from_terms(&self.reg, terms)
    }

    /// The `n`th root, when every exponent divides by `n`: `m^2` has a
    /// square root, `m` does not.
    pub fn root(&self, n: u16) -> Result<Unit> {
        let n = i16::try_from(n).map_err(|_| Error::Overflow)?;
        if n == 0 {
            return Err(Error::BadRoot("the root must be at least 1".to_string()));
        }
        let terms = self
            .terms
            .iter()
            .map(|t| {
                if t.exp % n == 0 {
                    Ok(Term {
                        exp: t.exp / n,
                        ..*t
                    })
                } else {
                    Err(Error::BadRoot(format!(
                        "{self} has an exponent not divisible by {n}"
                    )))
                }
            })
            .collect::<Result<Vec<_>>>()?;
        Unit::from_terms(&self.reg, terms)
    }

    /// An affine unit as the unit it is defined over (`°C` as `K`), so its
    /// quantities can be multiplied and added; any other unit unchanged.
    pub fn rebased(&self) -> Result<Unit> {
        match self.terms.as_slice() {
            [t] if self.is_affine() => {
                Unit::from_terms(&self.reg, self.reg.0.unit_def(t.unit).defn.clone())
            }
            // A scale as the plain number it stands for (`k.` as 1000).
            _ if self.is_scale() => Ok(Unit::one(&self.reg)),
            _ => Ok(self.clone()),
        }
    }

    /// `value`, written in this unit, written in `to`. The offsets apply to
    /// the value, and only to the value.
    pub fn convert_value(&self, value: f64, to: &Unit) -> Result<f64> {
        if self.dims != to.dims {
            return Err(Error::DimensionMismatch {
                left: self.dims,
                right: to.dims,
            });
        }
        // The same unit is the same number, with no rounding to 15 digits.
        if self == to {
            return Ok(value);
        }
        Ok(tidy(
            (value + self.offset) * self.factor / to.factor - to.offset,
        ))
    }

    /// How many `to` units one of this unit's size is, for a spread.
    pub fn spread_ratio(&self, to: &Unit) -> Result<f64> {
        if self.dims != to.dims {
            return Err(Error::DimensionMismatch {
                left: self.dims,
                right: to.dims,
            });
        }
        if self == to {
            return Ok(1.0);
        }
        Ok(tidy(self.factor / to.factor))
    }
}

/// A converted number to the 15 significant digits a float holds for sure,
/// so that `2g` in `mg` is `2000` and not `2000.0000000000002`. It is done
/// only where that moves the number by no more than a few of the last places,
/// which is the noise of the sum: a number that is not near a 15-digit one,
/// such as `90°` in radians, keeps all its digits.
fn tidy(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    match format!("{x:.14e}").parse::<f64>() {
        Ok(r) if (r - x).abs() <= 4.0 * f64::EPSILON * x.abs() => r,
        _ => x,
    }
}

impl PartialEq for Unit {
    fn eq(&self, other: &Unit) -> bool {
        self.reg.same(&other.reg) && self.terms == other.terms
    }
}

impl Eq for Unit {}

impl fmt::Display for Unit {
    /// The notation the parser reads: `kg.m^2.s^-2`, or `1` for no terms.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.terms.is_empty() {
            return f.write_str("1");
        }
        for (i, t) in self.terms.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            if let Some(p) = t.prefix {
                f.write_str(&self.reg.0.prefix_def(p).symbol)?;
            }
            if t.unit == self.reg.0.unity {
                // A scale: the prefix, then the `.` that makes it one.
                f.write_str(".")?;
                continue;
            }
            f.write_str(&self.reg.0.unit_def(t.unit).symbol)?;
            if t.exp != 1 {
                write!(f, "^{}", t.exp)?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unit({self})")
    }
}

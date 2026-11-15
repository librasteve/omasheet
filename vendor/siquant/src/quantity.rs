// Copyright (c) 2026 Stephen Roe

//! A measured value, and the named types that restrict it to a dimension.
//!
//! One generic type, [`Quantity`], carries a value, a 1-sigma uncertainty and
//! a unit. `Length` and `Force` are not types of their own: a
//! [`QuantityType`] is a name for a dimension, and "this is a `Length`" is
//! only [`QuantityType::admits`] being true of the unit.
//!
//! Uncertainty is propagated to first order, treating the operands as
//! independent (the rule omasheet uses): absolute uncertainties combine in
//! quadrature for a sum, and the partial derivatives do for a product.

use crate::dim::Dim;
use crate::error::{Error, Result};
use crate::registry::Registry;
use crate::unit::Unit;

/// A value with an uncertainty and a unit.
#[derive(Clone, Debug, PartialEq)]
pub struct Quantity {
    value: f64,
    sigma: f64,
    unit: Unit,
}

impl Quantity {
    /// A quantity; the uncertainty must not be negative (or NaN).
    pub fn new(value: f64, sigma: f64, unit: Unit) -> Result<Quantity> {
        if sigma.is_nan() || sigma < 0.0 {
            return Err(Error::NegativeSigma);
        }
        Ok(Quantity { value, sigma, unit })
    }

    /// A quantity with no uncertainty.
    pub fn exact(value: f64, unit: Unit) -> Quantity {
        Quantity {
            value,
            sigma: 0.0,
            unit,
        }
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn sigma(&self) -> f64 {
        self.sigma
    }

    pub fn unit(&self) -> &Unit {
        &self.unit
    }

    /// The same quantity in another unit of the same dimension. The offset
    /// of an affine unit applies to the value and not to the uncertainty.
    pub fn convert(&self, to: &Unit) -> Result<Quantity> {
        let value = self.unit.convert_value(self.value, to)?;
        let sigma = self.sigma * self.unit.spread_ratio(to)?;
        Ok(Quantity {
            value,
            sigma,
            unit: to.clone(),
        })
    }

    /// A quantity as a plain number: its value and uncertainty with the
    /// unit's size applied, so `1k.` is `(1000, 0)`. Only for a quantity
    /// with no dimension.
    pub fn to_number(&self) -> Result<(f64, f64)> {
        let plain = self.convert(&Unit::one(self.unit.registry()))?;
        Ok((plain.value, plain.sigma))
    }

    /// An affine quantity in the unit it is defined over (`20°C` as
    /// `293.15K`) and a scale as a plain number (`2k.` as `2000`); any
    /// other quantity unchanged.
    pub fn rebase(&self) -> Result<Quantity> {
        if self.unit.is_affine() || self.unit.is_scale() {
            self.convert(&self.unit.rebased()?)
        } else {
            Ok(self.clone())
        }
    }

    /// The sum. The right side is converted to the left's unit; if either
    /// side is affine both go through the unit it is defined over first.
    pub fn add(&self, other: &Quantity) -> Result<Quantity> {
        self.sum(other, 1.0)
    }

    /// The difference, as [`Quantity::add`]. Two temperatures differ by an
    /// interval in the unit of the left: `20°C - 10°C` is `10°C`.
    pub fn sub(&self, other: &Quantity) -> Result<Quantity> {
        self.sum(other, -1.0)
    }

    fn sum(&self, other: &Quantity, sign: f64) -> Result<Quantity> {
        let (a, b) = if self.unit.is_affine() || other.unit.is_affine() {
            (self.rebase()?, other.rebase()?)
        } else {
            (self.clone(), other.clone())
        };
        let b = b.convert(&a.unit)?;
        let sum = Quantity {
            value: a.value + sign * b.value,
            sigma: a.sigma.hypot(b.sigma),
            unit: a.unit,
        };
        if self.unit.is_affine() {
            if other.unit.is_affine() {
                // Two temperatures differ by a temperature interval, which
                // is as big in `°C` as in K: `10°C - 5°C` is `5°C`.
                if sign < 0.0 {
                    let f = self.unit.factor();
                    return Ok(Quantity {
                        value: sum.value / f,
                        sigma: sum.sigma / f,
                        unit: self.unit.clone(),
                    });
                }
            } else {
                // A temperature moved by an interval is a temperature in
                // the unit it began in: `10°C + 5K` is `15°C`.
                return sum.convert(&self.unit);
            }
        }
        Ok(sum)
    }

    /// The product, in the product of the two units: `5km * 2s` is `10km.s`.
    pub fn mul(&self, other: &Quantity) -> Result<Quantity> {
        let (a, b) = (self.rebase()?, other.rebase()?);
        Ok(Quantity {
            value: a.value * b.value,
            sigma: (b.value * a.sigma).hypot(a.value * b.sigma),
            unit: a.unit.mul(&b.unit)?,
        })
    }

    /// The quotient, in the quotient of the two units.
    pub fn div(&self, other: &Quantity) -> Result<Quantity> {
        let (a, b) = (self.rebase()?, other.rebase()?);
        if b.value == 0.0 {
            return Err(Error::DivideByZero);
        }
        Ok(Quantity {
            value: a.value / b.value,
            sigma: (a.sigma / b.value).hypot(a.value * b.sigma / (b.value * b.value)),
            unit: a.unit.div(&b.unit)?,
        })
    }

    /// The quantity to an integer power.
    pub fn pow(&self, n: i16) -> Result<Quantity> {
        let a = self.rebase()?;
        let k = i32::from(n);
        Ok(Quantity {
            value: a.value.powi(k),
            sigma: f64::from(k).abs() * a.value.abs().powi(k - 1) * a.sigma,
            unit: a.unit.pow(n)?,
        })
    }

    /// The `n`th root, when the unit has one.
    pub fn root(&self, n: u16) -> Result<Quantity> {
        let a = self.rebase()?;
        let unit = a.unit.root(n)?;
        let p = 1.0 / f64::from(n);
        if a.value < 0.0 && n.is_multiple_of(2) {
            return Err(Error::BadRoot("the value is negative".to_string()));
        }
        let value = a.value.abs().powf(p).copysign(a.value);
        let sigma = if a.sigma == 0.0 {
            0.0
        } else {
            p * a.value.abs().powf(p - 1.0) * a.sigma
        };
        if !sigma.is_finite() {
            return Err(Error::BadRoot(
                "the uncertainty is undefined at zero".to_string(),
            ));
        }
        Ok(Quantity { value, sigma, unit })
    }
}

/// A name for a dimension: `Length`, `Force`, `Energy`.
#[derive(Clone, Debug)]
pub struct QuantityType {
    reg: Registry,
    index: usize,
}

impl QuantityType {
    pub(crate) fn new(reg: Registry, index: usize) -> QuantityType {
        QuantityType { reg, index }
    }

    /// A small stable id, its position in the resource file, for a host that
    /// wants a `Copy` handle: see [`crate::Registry::quantity_type_by_id`].
    pub fn id(&self) -> u16 {
        u16::try_from(self.index).expect("fewer than 65536 quantity types")
    }

    /// The heading a menu files the type under: see [`crate::GROUPS`].
    pub fn group(&self) -> &str {
        &self.reg.0.types[self.index].group
    }

    /// The name, as a column declares it.
    pub fn name(&self) -> &str {
        &self.reg.0.types[self.index].name
    }

    /// The dimension it names.
    pub fn dims(&self) -> Dim {
        self.reg.0.types[self.index].dims
    }

    /// The unit to show a quantity of this type in, unless a column says
    /// otherwise.
    pub fn preferred_unit(&self) -> Option<Unit> {
        let terms = self.reg.0.types[self.index].unit.clone()?;
        Unit::from_terms(&self.reg, terms).ok()
    }

    /// Is this a unit a quantity of this type may have? Only the dimension
    /// matters: `km` and `m` are lengths, `N.m` is a `Torque` and an `Energy`.
    pub fn admits(&self, unit: &Unit) -> bool {
        unit.dims() == self.dims()
    }

    /// Is this quantity one of this type?
    pub fn admits_quantity(&self, q: &Quantity) -> bool {
        self.admits(q.unit())
    }
}

impl PartialEq for QuantityType {
    fn eq(&self, other: &QuantityType) -> bool {
        self.reg.same(&other.reg) && self.index == other.index
    }
}

// Copyright (c) 2026 Stephen Roe

//! Base dimensions and the exponent vector over them.
//!
//! The slot layout is fixed here, in code, and never inferred from a resource
//! file: `L M T I Θ N J A $ D`. Angle (`A`) is used from Stage 3, Currency
//! (`$`) is reserved, and Data (`D`) is used from Stage 2.

use std::fmt;

/// How many base dimensions there are.
pub const NUM_BASES: usize = 10;

/// A base dimension; its discriminant is its slot in a [`Dim`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Base {
    Length = 0,
    Mass = 1,
    Time = 2,
    Current = 3,
    Temperature = 4,
    Amount = 5,
    LuminousIntensity = 6,
    Angle = 7,
    Currency = 8,
    Data = 9,
}

impl Base {
    /// Every base dimension, in slot order.
    pub const ALL: [Base; NUM_BASES] = [
        Base::Length,
        Base::Mass,
        Base::Time,
        Base::Current,
        Base::Temperature,
        Base::Amount,
        Base::LuminousIntensity,
        Base::Angle,
        Base::Currency,
        Base::Data,
    ];

    /// Its slot in a [`Dim`].
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The name resource files use for it.
    pub const fn name(self) -> &'static str {
        match self {
            Base::Length => "Length",
            Base::Mass => "Mass",
            Base::Time => "Time",
            Base::Current => "Current",
            Base::Temperature => "Temperature",
            Base::Amount => "Amount",
            Base::LuminousIntensity => "LuminousIntensity",
            Base::Angle => "Angle",
            Base::Currency => "Currency",
            Base::Data => "Data",
        }
    }

    /// Its one-character symbol in a printed dimension.
    pub const fn symbol(self) -> &'static str {
        match self {
            Base::Length => "L",
            Base::Mass => "M",
            Base::Time => "T",
            Base::Current => "I",
            Base::Temperature => "Θ",
            Base::Amount => "N",
            Base::LuminousIntensity => "J",
            Base::Angle => "A",
            Base::Currency => "$",
            Base::Data => "D",
        }
    }

    /// The base dimension a resource file calls `name`.
    pub fn from_name(name: &str) -> Option<Base> {
        Base::ALL.into_iter().find(|b| b.name() == name)
    }
}

/// An exponent for each base dimension: what a unit measures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Dim([i8; NUM_BASES]);

impl Dim {
    /// No dimension: a pure number.
    pub const NONE: Dim = Dim([0; NUM_BASES]);

    /// One base dimension to the power one.
    pub fn of(base: Base) -> Dim {
        Dim::NONE.with(base, 1)
    }

    /// This dimension with the exponent of `base` set to `exp`.
    pub fn with(mut self, base: Base, exp: i8) -> Dim {
        self.0[base.index()] = exp;
        self
    }

    /// The exponent of `base`.
    pub fn exponent(&self, base: Base) -> i8 {
        self.0[base.index()]
    }

    /// Is this the dimension of a pure number?
    pub fn is_dimensionless(&self) -> bool {
        *self == Dim::NONE
    }

    /// The product of two dimensions, `None` if an exponent overflows.
    pub fn checked_add(self, other: Dim) -> Option<Dim> {
        let mut out = [0i8; NUM_BASES];
        for (slot, (a, b)) in out.iter_mut().zip(self.0.iter().zip(other.0.iter())) {
            *slot = a.checked_add(*b)?;
        }
        Some(Dim(out))
    }

    /// Every exponent times `k`, `None` if one overflows.
    pub fn checked_scale(self, k: i16) -> Option<Dim> {
        let mut out = [0i8; NUM_BASES];
        for (slot, a) in out.iter_mut().zip(self.0.iter()) {
            *slot = i8::try_from(i16::from(*a).checked_mul(k)?).ok()?;
        }
        Some(Dim(out))
    }
}

impl fmt::Display for Dim {
    /// `L.M.T^-2`, or `1` for no dimension.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut any = false;
        for base in Base::ALL {
            let e = self.exponent(base);
            if e == 0 {
                continue;
            }
            if any {
                f.write_str(".")?;
            }
            any = true;
            f.write_str(base.symbol())?;
            if e != 1 {
                write!(f, "^{e}")?;
            }
        }
        if !any {
            f.write_str("1")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_arithmetic() {
        let force = Dim::of(Base::Mass)
            .checked_add(Dim::of(Base::Length))
            .unwrap()
            .checked_add(Dim::of(Base::Time).checked_scale(-2).unwrap())
            .unwrap();
        assert_eq!(force.to_string(), "L.M.T^-2");
        assert_eq!(Dim::NONE.to_string(), "1");
        assert!(Dim::of(Base::Length).checked_scale(200).is_none());
    }

    #[test]
    fn names_round_trip() {
        for (i, b) in Base::ALL.into_iter().enumerate() {
            assert_eq!(b.index(), i);
            assert_eq!(Base::from_name(b.name()), Some(b));
        }
    }
}

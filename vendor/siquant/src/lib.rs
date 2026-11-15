// Copyright (c) 2026 Stephen Roe

//! SI units, prefixes and quantities with uncertainty.
//!
//! A unit is an immutable list of prefixed unit terms over a fixed vector of
//! base-dimension exponents; the units, the prefixes and the named quantity
//! types are defined in YAML files under `resources/`, embedded at build
//! time. A [`Quantity`] is a value, an uncertainty and a unit, and a named
//! type such as `Length` is that `Quantity` restricted to one dimension
//! ([`QuantityType`]).
//!
//! Cell text is deliberately strict: SI units only, written `kg.m^2.s^-2`
//! (see [`Unit::parse`] and [`Scope::Live`]), plus a bare prefix and a `.`,
//! `k.`, for a dimensionless scale (`1k.` is 1000). The crate does not read
//! the number in front of a unit; the host splits a cell into number and unit.
//!
//! The staged plan is in `docs/plan.md`: this is Stages 1 to 3 (SI units and SI
//! prefixes, data prefixes and angles), with durations to follow.

mod dim;
mod dms;
mod error;
mod parse;
mod quantity;
mod registry;
mod unit;

pub use dim::{Base, Dim, NUM_BASES};
pub use error::{Error, Result};
pub use quantity::{Quantity, QuantityType};
pub use registry::{
    GROUPS, PrefixId, PrefixInfo, PrefixKind, Prefixable, Registry, Scope, UnitId, UnitInfo,
};
pub use unit::{Term, Unit};

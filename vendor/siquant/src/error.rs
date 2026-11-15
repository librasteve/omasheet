// Copyright (c) 2026 Stephen Roe

//! The one error type of the crate.

use std::fmt;

use crate::dim::Dim;

/// Why a unit could not be read or an operation could not be done.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// The text is not shaped like a unit; `at` is a byte offset into it.
    Syntax { at: usize, message: String },
    /// No unit or prefixed unit has this symbol.
    UnknownUnit(String),
    /// The unit exists but is not allowed in cell text (a column unit only).
    NotLive(String),
    /// The symbol could be read more than one way.
    AmbiguousSymbol {
        symbol: String,
        candidates: Vec<String>,
    },
    /// An affine unit (such as `°C`) was used inside a compound unit.
    AffineInCompound(String),
    /// A scale (such as `k.`) was used inside a compound unit.
    ScaleInCompound(String),
    /// The two sides do not measure the same thing.
    DimensionMismatch { left: Dim, right: Dim },
    /// An exponent, dimension or factor is out of range.
    Overflow,
    /// An uncertainty must not be negative.
    NegativeSigma,
    /// Division by zero.
    DivideByZero,
    /// A root cannot be taken of this unit or value.
    BadRoot(String),
    /// A resource file is invalid.
    Resource(String),
}

/// The result of a siquant operation.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Syntax { at, message } => write!(f, "{message} (at byte {at})"),
            Error::UnknownUnit(s) => write!(f, "unknown unit `{s}`"),
            Error::NotLive(s) => write!(f, "`{s}` cannot be written in a cell"),
            Error::AmbiguousSymbol { symbol, candidates } => {
                write!(f, "`{symbol}` is ambiguous: {}", candidates.join(", "))
            }
            Error::AffineInCompound(s) => {
                write!(f, "`{s}` cannot be part of a compound unit")
            }
            Error::ScaleInCompound(s) => {
                write!(f, "the scale `{s}` cannot be part of a compound unit")
            }
            Error::DimensionMismatch { left, right } => {
                write!(f, "dimensions differ: {left} and {right}")
            }
            Error::Overflow => f.write_str("an exponent or factor is out of range"),
            Error::NegativeSigma => f.write_str("an uncertainty cannot be negative"),
            Error::DivideByZero => f.write_str("division by zero"),
            Error::BadRoot(s) => write!(f, "cannot take the root: {s}"),
            Error::Resource(s) => write!(f, "invalid resource: {s}"),
        }
    }
}

impl std::error::Error for Error {}

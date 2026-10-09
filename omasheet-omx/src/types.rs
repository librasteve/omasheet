// Copyright (c) 2026 Stephen Roe

//! Static types and shapes.

use std::fmt;
use std::rc::Rc;

/// The type of a single value. `Any` is "not known until evaluation".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S {
    Any,
    Int,
    Rat,
    Num,
    Complex,
    Text,
    Bool,
    Date,
    Time,
    DateTime,
}

impl S {
    /// A type that can be written in a column schema.
    pub fn from_name(name: &str) -> Option<S> {
        Some(match name {
            "Int" => S::Int,
            "Ratio" => S::Rat,
            "Num" => S::Num,
            "Complex" => S::Complex,
            "Text" => S::Text,
            "Date" => S::Date,
            "Time" => S::Time,
            "DateTime" => S::DateTime,
            "Bool" => S::Bool,
            _ => return None,
        })
    }

    pub fn is_numeric(self) -> bool {
        matches!(self, S::Any | S::Int | S::Rat | S::Num | S::Complex)
    }

    /// A date, a time of day, or both.
    pub fn is_temporal(self) -> bool {
        matches!(self, S::Date | S::Time | S::DateTime)
    }

    /// The type of `self + other`, or of `self - other` when `minus`, where
    /// at least one side is a date or time. A whole number counts days next
    /// to a `Date` and seconds next to a `Time` or `DateTime`. `None` if the
    /// two cannot be combined.
    pub fn shift(self, minus: bool, other: S) -> Option<S> {
        Some(match (self, other) {
            (S::Any, t) | (t, S::Any) if t.is_temporal() => S::Any,
            (t, S::Int) if t.is_temporal() => t,
            (S::Int, t) if t.is_temporal() && !minus => t,
            (S::Date, S::Time) | (S::Time, S::Date) if !minus => S::DateTime,
            (a, b) if a == b && a.is_temporal() && minus => S::Int,
            _ => return None,
        })
    }

    fn rank(self) -> u8 {
        match self {
            S::Int => 0,
            S::Rat => 1,
            S::Num => 2,
            _ => 3,
        }
    }

    /// The wider of two numeric types.
    pub fn wider(self, other: S) -> S {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    /// The common type of two values, or `None` if they cannot be mixed.
    pub fn join(self, other: S) -> Option<S> {
        if self == other {
            Some(self)
        } else if self == S::Any || other == S::Any {
            Some(S::Any)
        } else if self.is_numeric() && other.is_numeric() {
            Some(self.wider(other))
        } else {
            None
        }
    }

    /// Whether a value of type `self` may be stored where `to` is declared.
    pub fn assignable_to(self, to: S) -> bool {
        self == to || self == S::Any || to == S::Any || (self == S::Int && to == S::Rat)
    }
}

impl fmt::Display for S {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            S::Any => "Any",
            S::Int => "Int",
            S::Rat => "Ratio",
            S::Num => "Num",
            S::Complex => "Complex",
            S::Text => "Text",
            S::Bool => "Bool",
            S::Date => "Date",
            S::Time => "Time",
            S::DateTime => "DateTime",
        })
    }
}

/// How a reference reaches the column it reads, relative to the row that
/// contains the reference. Drives cycle detection and calculation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepKind {
    /// The same row.
    Same,
    /// Only rows strictly before the current row.
    Back,
    /// Only rows strictly after the current row.
    Fwd,
    /// Any rows.
    Whole,
}

/// A selection of rows and columns from one sheet table.
#[derive(Clone, Debug, PartialEq)]
pub struct TableTy {
    pub table: usize,
    pub cols: Rc<Vec<usize>>,
    pub rows: Option<usize>,
    pub kind: DepKind,
}

/// The shape and type of an expression.
#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Any,
    Scalar(S),
    Vector(S, Option<usize>),
    Range,
    Table(TableTy),
    Row(TableTy),
}

impl Ty {
    pub fn describe(&self) -> String {
        match self {
            Ty::Any => "a value of unknown type".into(),
            Ty::Scalar(S::Any) => "a single value".into(),
            Ty::Scalar(s) => format!("{s}"),
            Ty::Vector(S::Any, _) => "a vector".into(),
            Ty::Vector(s, _) => format!("a vector of {s}"),
            Ty::Range => "a range".into(),
            Ty::Table(_) => "a table".into(),
            Ty::Row(_) => "a row".into(),
        }
    }
}

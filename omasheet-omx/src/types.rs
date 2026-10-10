// Copyright (c) 2026 Stephen Roe

//! Static types and shapes.

use std::fmt;
use std::rc::Rc;

/// The type of a single value. `Any` is "not known until evaluation".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S {
    Any,
    Int,
    /// An exact number that ends when written as a decimal: `19.99`. Every
    /// `Int` is one, and every `Decimal` is a `Ratio`.
    Decimal,
    Ratio,
    /// An exact number shown as so many in a hundred: `20%` is `1/5`. It
    /// holds whatever a `Ratio` does.
    Percent,
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
            "Decimal" => S::Decimal,
            "Ratio" => S::Ratio,
            "Percent" => S::Percent,
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
        matches!(
            self,
            S::Any | S::Int | S::Decimal | S::Ratio | S::Percent | S::Num | S::Complex
        )
    }

    /// An exact number: `Int`, `Decimal`, `Ratio` or `Percent`.
    pub fn is_exact(self) -> bool {
        matches!(self, S::Int | S::Decimal | S::Ratio | S::Percent)
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
            S::Decimal => 1,
            S::Ratio | S::Percent => 2,
            S::Num => 3,
            _ => 4,
        }
    }

    /// The wider of two numeric types. A `Percent` next to another exact
    /// number is a `Ratio`.
    pub fn wider(self, other: S) -> S {
        let percent = self == S::Percent || other == S::Percent;
        if self != other && percent && self.is_exact() && other.is_exact() {
            S::Ratio
        } else if self.rank() >= other.rank() {
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

    /// Whether the conversion named `to` takes a value of type `self`. Text
    /// is read as whatever is asked for, and anything can be written as text.
    pub fn converts_to(self, to: S) -> bool {
        let number = self.is_exact() || self == S::Num;
        self == to
            || matches!(self, S::Any | S::Text)
            || match to {
                S::Any | S::Text => true,
                S::Int | S::Decimal | S::Ratio | S::Num => number || self == S::Bool,
                S::Percent | S::Complex | S::Bool => number,
                S::Date | S::Time => self == S::DateTime,
                S::DateTime => self == S::Date,
            }
    }

    /// Whether a value of type `self` may be stored where `to` is declared.
    pub fn assignable_to(self, to: S) -> bool {
        self == to
            || self == S::Any
            || to == S::Any
            // An `Int` is a `Decimal`, and a `Decimal` is a `Ratio`. A
            // `Percent` holds any of them, and is a `Ratio`.
            || (self.is_exact() && to.is_exact() && self.rank() <= to.rank())
    }
}

impl fmt::Display for S {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            S::Any => "Any",
            S::Int => "Int",
            S::Decimal => "Decimal",
            S::Ratio => "Ratio",
            S::Percent => "Percent",
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

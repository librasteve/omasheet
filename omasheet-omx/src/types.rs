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
}

impl S {
    /// A type that can be written in a column schema.
    pub fn from_name(name: &str) -> Option<S> {
        Some(match name {
            "Int" => S::Int,
            "Rat" => S::Rat,
            "Num" => S::Num,
            "Text" => S::Text,
            "Date" => S::Date,
            "Bool" => S::Bool,
            _ => return None,
        })
    }

    pub fn is_numeric(self) -> bool {
        matches!(self, S::Any | S::Int | S::Rat | S::Num | S::Complex)
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
            S::Rat => "Rat",
            S::Num => "Num",
            S::Complex => "Complex",
            S::Text => "Text",
            S::Bool => "Bool",
            S::Date => "Date",
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

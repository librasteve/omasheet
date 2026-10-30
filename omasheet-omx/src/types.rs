// Copyright (c) 2026 Stephen Roe

//! Static types and shapes.

use std::fmt;
use std::rc::Rc;

/// The type of a single value. `Any` is "not known until evaluation".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S {
    Any,
    Int,
    /// A whole number shown in base 16: `0xff`. It holds whatever an `Int`
    /// does.
    Hex,
    /// A whole number shown in base 8: `0o17`. It holds whatever an `Int`
    /// does.
    Octal,
    /// A whole number shown in base 2: `0b101`. It holds whatever an `Int`
    /// does.
    Binary,
    /// An exact number that ends when written as a decimal: `19.99`. Every
    /// `Int` is one.
    Decimal,
    /// An exact number shown as a fraction of whole numbers: `1/7`, and
    /// `2 3/4` for `2.75`. It holds whatever a `Decimal` does.
    Fraction,
    /// An exact number shown as so many in a hundred: `20%` is `1/5`. It
    /// holds whatever a `Decimal` does.
    Percent,
    Float,
    String,
    Boolean,
    Date,
    Time,
    DateTime,
    /// Text that reads as a web address: a scheme, then the rest of the
    /// address. It holds whatever a `String` does.
    Url,
    /// Text that reads as an email address: one `@`, with a name before it
    /// and a domain after. It holds whatever a `String` does.
    Email,
    /// Text that reads as a UUID: `7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f`.
    /// It holds whatever a `String` does.
    Uuid,
}

impl S {
    /// A type that can be written in a column schema.
    pub fn from_name(name: &str) -> Option<S> {
        Some(match name {
            "Integer" => S::Int,
            // `Int` is what this type was called before, and is still read
            // so that a file written then still opens. It is not written.
            "Int" => S::Int,
            "Hex" => S::Hex,
            "Octal" => S::Octal,
            "Binary" => S::Binary,
            "Decimal" => S::Decimal,
            "Fraction" => S::Fraction,
            "Percent" => S::Percent,
            "Float" => S::Float,
            "String" => S::String,
            "Date" => S::Date,
            "Time" => S::Time,
            "Datetime" => S::DateTime,
            "Boolean" => S::Boolean,
            "Url" => S::Url,
            "Email" => S::Email,
            "Uuid" => S::Uuid,
            _ => return None,
        })
    }

    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            S::Any
                | S::Int
                | S::Hex
                | S::Octal
                | S::Binary
                | S::Decimal
                | S::Fraction
                | S::Percent
                | S::Float
        )
    }

    /// A whole number, in whichever base it is shown.
    pub fn is_int(self) -> bool {
        matches!(self, S::Int | S::Hex | S::Octal | S::Binary)
    }

    /// An exact number: a whole number, `Decimal`, `Fraction` or `Percent`.
    pub fn is_exact(self) -> bool {
        self.is_int() || matches!(self, S::Decimal | S::Fraction | S::Percent)
    }

    /// A date, a time of day, or both.
    pub fn is_temporal(self) -> bool {
        matches!(self, S::Date | S::Time | S::DateTime)
    }

    /// Text, or one of the kinds of text that read as something narrower.
    pub fn is_text(self) -> bool {
        matches!(self, S::String | S::Url | S::Email | S::Uuid)
    }

    /// The type of `self + other`, or of `self - other` when `minus`, where
    /// at least one side is a date or time. A whole number counts days next
    /// to a `Date` and seconds next to a `Time` or `Datetime`. `None` if the
    /// two cannot be combined.
    pub fn shift(self, minus: bool, other: S) -> Option<S> {
        Some(match (self, other) {
            (S::Any, t) | (t, S::Any) if t.is_temporal() => S::Any,
            (t, i) if t.is_temporal() && i.is_int() => t,
            (i, t) if t.is_temporal() && i.is_int() && !minus => t,
            (S::Date, S::Time) | (S::Time, S::Date) if !minus => S::DateTime,
            (a, b) if a == b && a.is_temporal() && minus => S::Int,
            _ => return None,
        })
    }

    fn rank(self) -> u8 {
        match self {
            S::Int | S::Hex | S::Octal | S::Binary => 0,
            S::Decimal => 1,
            S::Fraction | S::Percent => 2,
            S::Float => 3,
            _ => 4,
        }
    }

    /// The wider of two numeric types. A `Percent` next to another kind of
    /// exact number is a `Decimal`, or a `Fraction` next to one. Whole
    /// numbers shown in two different bases are an `Int`.
    pub fn wider(self, other: S) -> S {
        let either = self == S::Percent || other == S::Percent;
        if self != other && self.is_int() && other.is_int() {
            S::Int
        } else if self != other && either && self.is_exact() && other.is_exact() {
            if self == S::Fraction || other == S::Fraction {
                S::Fraction
            } else {
                S::Decimal
            }
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
        } else if self.is_text() && other.is_text() {
            // Two kinds of text together are text, which both of them are.
            Some(S::String)
        } else {
            None
        }
    }

    /// Whether the conversion named `to` takes a value of type `self`. Text
    /// is read as whatever is asked for, and anything can be written as text.
    pub fn converts_to(self, to: S) -> bool {
        let number = self.is_exact() || self == S::Float;
        self == to
            || self == S::Any
            || self.is_text()
            || match to {
                S::Any | S::String => true,
                // Only text reads as one of these, and text is taken above.
                S::Url | S::Email | S::Uuid => false,
                S::Int | S::Hex | S::Octal | S::Binary | S::Decimal | S::Fraction | S::Float => {
                    number || self == S::Boolean
                }
                S::Percent | S::Boolean => number,
                S::Date | S::Time => self == S::DateTime,
                S::DateTime => self == S::Date,
            }
    }

    /// Whether a value of type `self` may be stored where `to` is declared.
    pub fn assignable_to(self, to: S) -> bool {
        self == to
            || self == S::Any
            || to == S::Any
            // An `Int` is any exact number. A `Decimal`, a `Fraction` and a
            // `Percent` hold the same numbers, shown in different ways, as
            // the whole numbers do in their bases.
            || (self.is_exact() && to.is_exact() && (!to.is_int() || self.is_int()))
            // A web address, an email address and a UUID are all text. Text
            // is not one of them until it has been read as one.
            || (self.is_text() && to == S::String)
    }
}

impl fmt::Display for S {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            S::Any => "Any",
            S::Int => "Integer",
            S::Hex => "Hex",
            S::Octal => "Octal",
            S::Binary => "Binary",
            S::Decimal => "Decimal",
            S::Fraction => "Fraction",
            S::Percent => "Percent",
            S::Float => "Float",
            S::String => "String",
            S::Boolean => "Boolean",
            S::Date => "Date",
            S::Time => "Time",
            S::DateTime => "Datetime",
            S::Url => "Url",
            S::Email => "Email",
            S::Uuid => "Uuid",
        })
    }
}

/// The base a whole number is written and shown in. Every base holds the
/// same numbers; it is the type that says which one a column shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Base {
    #[default]
    Dec,
    Hex,
    Oct,
    Bin,
}

impl Base {
    /// The base a type shows its whole numbers in: decimal for any but the
    /// three subtypes.
    pub fn of(s: S) -> Base {
        match s {
            S::Hex => Base::Hex,
            S::Octal => Base::Oct,
            S::Binary => Base::Bin,
            _ => Base::Dec,
        }
    }

    /// The type of a whole number written in this base.
    pub fn ty(self) -> S {
        match self {
            Base::Dec => S::Int,
            Base::Hex => S::Hex,
            Base::Oct => S::Octal,
            Base::Bin => S::Binary,
        }
    }

    pub fn radix(self) -> u32 {
        match self {
            Base::Dec => 10,
            Base::Hex => 16,
            Base::Oct => 8,
            Base::Bin => 2,
        }
    }

    /// What a number in this base is written after: `0x`, `0o` or `0b`,
    /// and nothing for decimal.
    pub fn prefix(self) -> &'static str {
        match self {
            Base::Dec => "",
            Base::Hex => "0x",
            Base::Oct => "0o",
            Base::Bin => "0b",
        }
    }

    /// `n` as it is written in this base, `-0xff` for -255. This reads back
    /// as the same number, where the two's complement it is shown as would
    /// not fit (D132).
    pub fn write(self, n: i64) -> String {
        let sign = if n < 0 { "-" } else { "" };
        let m = n.unsigned_abs();
        let digits = match self {
            Base::Dec => m.to_string(),
            Base::Hex => format!("{m:x}"),
            Base::Oct => format!("{m:o}"),
            Base::Bin => format!("{m:b}"),
        };
        format!("{sign}{}{digits}", self.prefix())
    }

    /// `n` as it is shown in this base: a negative one as its 64-bit two's
    /// complement, `0xffffffffffffff01` for -255 (D126).
    pub fn show(self, n: i64) -> String {
        match self {
            Base::Dec => n.to_string(),
            Base::Hex => format!("0x{n:x}"),
            Base::Oct => format!("0o{n:o}"),
            Base::Bin => format!("0b{n:b}"),
        }
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

/// A selection of rows and columns from one sheet of the file.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetTy {
    pub sheet: usize,
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
    Sheet(SheetTy),
    Row(SheetTy),
}

impl Ty {
    /// A whole number, or what may turn out to be one.
    pub fn is_whole(&self) -> bool {
        match self {
            Ty::Any | Ty::Scalar(S::Any) => true,
            Ty::Scalar(s) => s.is_int(),
            _ => false,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Ty::Any => "a value of unknown type".into(),
            Ty::Scalar(S::Any) => "a single value".into(),
            Ty::Scalar(s) => format!("{s}"),
            Ty::Vector(S::Any, _) => "a vector".into(),
            Ty::Vector(s, _) => format!("a vector of {s}"),
            Ty::Range => "a range".into(),
            Ty::Sheet(_) => "a sheet".into(),
            Ty::Row(_) => "a row".into(),
        }
    }
}

// Copyright (c) 2026 Stephen Roe

//! The checked form of an expression: every name is resolved to an index, so
//! evaluation never looks anything up by text.

use crate::ast::{BinOp, Lit, UnOp};
use crate::cycle::Cyc;
use crate::diag::Span;
use crate::funcs::MathFn;
use crate::types::S;

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: Ir,
    pub span: Span,
}

impl Node {
    pub fn new(kind: Ir, span: Span) -> Node {
        Node { kind, span }
    }

    pub fn error(span: Span) -> Node {
        Node {
            kind: Ir::Error,
            span,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Ir {
    /// An expression that failed to check; never evaluated.
    Error,
    Lit(Lit),
    Const(usize),
    Sheet(usize),
    /// A whole column of a sheet of the file.
    Column {
        sheet: usize,
        col: usize,
    },
    /// A column of the row `depth` frames out (0 is the innermost row).
    RowCol {
        depth: usize,
        col: usize,
    },
    /// The position of the row `depth` frames out, counting from zero.
    RowIndex {
        depth: usize,
    },
    /// No value: what a sequence gives once it is past its limit.
    Empty,
    Unary(UnOp, Box<Node>),
    Binary(BinOp, Box<Node>, Box<Node>),
    If(Box<Node>, Box<Node>, Box<Node>),
    VecLit(Vec<Node>),
    Index {
        base: Box<Node>,
        rows: RowSel,
        cols: ColSel,
    },
    /// A column of a sheet or row value.
    Field {
        base: Box<Node>,
        col: usize,
    },
    Call(Func, Vec<Node>),
    /// A call of a function the file defines: its body, in which `Arg(i)`
    /// stands for `args[i]`.
    Apply {
        name: String,
        args: Vec<Node>,
        body: Box<Node>,
    },
    /// An argument of the innermost `Apply`.
    Arg(usize),
    /// A vector used where one value is needed: empty gives empty, one
    /// element gives that element, more is an error.
    Single(Box<Node>),
}

/// A cursor position: the current row, optionally moved by an offset.
/// `Some((true, n))` is `* - n`.
#[derive(Clone, Debug)]
pub struct Offset(pub Option<(bool, Box<Node>)>);

#[derive(Clone, Debug)]
pub enum Bound {
    Abs(Box<Node>),
    Cursor(Offset),
}

#[derive(Clone, Debug)]
pub enum RowSel {
    All,
    /// One row by position; negative counts from the end.
    Pos(Box<Node>),
    /// One row relative to the current row; outside the sheet gives empty.
    Cursor(Offset),
    Range {
        lo: Bound,
        hi: Bound,
        /// Leaves out its first end: `lo^..hi`.
        after: bool,
        exclusive: bool,
    },
    /// A range value, such as a constant bound to `2..100`.
    RangeVal(Box<Node>),
    /// A condition evaluated for each candidate row.
    Pred(Box<Node>),
    /// A boolean vector with one element per row.
    Mask(Box<Node>),
}

#[derive(Clone, Debug)]
pub enum ColSel {
    All,
    One(usize),
    Many(Vec<usize>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Func {
    Sum,
    Mean,
    Min,
    Max,
    /// How many values are not empty.
    Count,
    /// How many rows a sheet has, or elements a vector has, empty or not.
    Len,
    /// The middle value, or the mean of the two middle ones.
    Median,
    /// The product of every value.
    Product,
    /// The first and last values, empty ones included.
    First,
    Last,
    /// The standard deviation and the variance, each taking the degrees of
    /// freedom to subtract: one by default, as Polars has it.
    Std,
    Var,
    /// The value a fraction of the way through the sorted values.
    Quantile,
    /// A value as another type: `Int(x)`, `Num(x)` and the rest.
    To(S),
    /// The next value after this one: a number plus one, or the next text
    /// in the way letters and digits count up, `AZ` to `BA`.
    Succ,
    /// A step through a run of names, `Monday` to `Tuesday`: the value and
    /// how many places to move it.
    Cycle(Cyc),
    /// A date or date-time a whole number of months on, clamped to the
    /// last day of a month too short for it.
    AddMonths,
    /// The last day of the month a date is in.
    EndOfMonth,
    /// The first day of the month a date is in.
    StartOfMonth,
    /// Whether a value is empty.
    IsNull,
    /// Whether a value is a `NaN`. Empty of an empty cell, which is a
    /// different kind of absence and has no answer to this.
    IsNan,
    Today,
    Now,
    Year,
    Month,
    Day,
    Weekday,
    /// 1 to 4 by the quarter of the year.
    Quarter,
    /// The ISO week of the year, 1 to 53.
    Week,
    /// The day of the year, 1 to 366.
    OrdinalDay,
    Hour,
    Minute,
    Second,
    Millisecond,
    Microsecond,
    Nanosecond,
    /// The day of a date-time.
    DateOf,
    /// The time of day of a date-time.
    TimeOf,
    /// The same instant on the clocks of a named time zone.
    ToZone,
    Utc,
    /// The same instant on the clocks of this machine.
    Local,
    /// Seconds ahead of UTC.
    Offset,
    ZoneName,
    /// A function of one real number, such as `sqrt`.
    Math(MathFn),
    /// To a number of decimal places: `round(x, 2)`, as Polars rounds.
    /// A half goes away from zero, and a number with fewer places than
    /// that is itself.
    RoundDigits,
    Pi,
    /// The base of the natural logarithm.
    E,
}

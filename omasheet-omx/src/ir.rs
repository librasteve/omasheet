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
    /// A column of the sheet being grouped, over the rows of the group an
    /// aggregate of `agg` is being worked out for.
    GroupCol(usize),
    /// The rows of the group `apply` is calling its function on, as a sheet.
    GroupSheet,
    /// `apply` of a function that gives rows of the sheet `base` is grouped
    /// from: `body` is called on each group's rows, and `cols` are the
    /// `keys` and then the columns the function gives.
    GroupApply {
        base: Box<Node>,
        keys: Vec<usize>,
        body: Box<Node>,
        cols: Vec<usize>,
        /// The sheet the function gives rows of, where that is not the one
        /// grouped.
        other: Option<usize>,
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
    /// One row relative to the current row among the rows whose `keys`
    /// hold what the current row's do, in the sheet's order: `*-1 by
    /// Region`. Outside that partition gives empty.
    CursorBy {
        off: Offset,
        keys: Vec<usize>,
    },
    Range {
        lo: Bound,
        hi: Bound,
        /// Leaves out its first end: `lo^..hi`.
        after: bool,
        exclusive: bool,
    },
    /// `block`: the rows that the current row heads, the ones under it
    /// with no heading level of their own, up to the end of its block.
    /// `levels` are the sheet's rows' heading levels, 0 for none. Empty for
    /// a row that heads no block.
    Block {
        levels: Vec<u8>,
        summary: Vec<bool>,
    },
    /// A range value, such as a constant bound to `2..100`.
    RangeVal(Box<Node>),
    /// A condition evaluated for each candidate row.
    Pred(Box<Node>),
    /// A boolean vector with one element per row.
    Mask(Box<Node>),
    /// Every row, in the order the key columns put them: a permutation
    /// rather than a selection, which is why a sort is a row selector at
    /// all. `keys` is empty over a column, whose values are their own key.
    Order {
        keys: Vec<usize>,
        desc: bool,
    },
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
    /// `x.type()` of a value whose type is not known until it is read: the
    /// kind of a quantity from its dimension.
    TypeOf,
    /// `Complex(re, im)`: a complex number from its two parts.
    Complex,
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
    /// Whether a number is neither infinite nor `NaN`, and whether it is
    /// infinite. An exact number is always finite.
    IsFinite,
    IsInfinite,
    /// Whether a date falls in a leap year.
    IsLeapYear,
    /// The number of characters in a text.
    LenChars,
    ToLowercase,
    ToUppercase,
    /// A value as text, in the format its second argument gives.
    Fmt,
    /// A random `Float` from 0 up to, not including, 1: Raku's `rand`.
    Rand,
    /// Values taken at random from a vector, as Raku's `roll` does, each put
    /// back after it is taken, and `pick`, which does not put it back.
    Roll,
    Pick,
    /// Whether a text starts with, ends with or holds another, literally.
    StartsWith,
    EndsWith,
    Contains,
    /// Whether a text matches a regex anywhere in it, as Polars'
    /// `str.contains` does.
    Matches,
    /// Whether a text holds any of a vector of texts.
    ContainsAny,
    /// Whether a value is the only one of its kind in its column, whether
    /// it is not, and whether it is the first or the last of its kind
    /// there. Over a vector, each element's answer. Over this row's cell,
    /// the arguments are the cell, its whole column and the row's place:
    /// the column is answered once and the row looked up in it.
    IsUnique,
    IsDuplicated,
    IsFirstDistinct,
    IsLastDistinct,
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
    /// A `Duration` as an exact number of days, hours, minutes or seconds:
    /// `total_hours` of `PT1H30M` is 1.5. The unit, in microseconds.
    Total(i64),
    /// A quantity in another unit of its dimension: `5kg.to('g')`. The unit
    /// is the second argument, as text (add-si-units D234).
    ToUnit,
    /// The unit of a quantity as text: `5kg.unit()` is `"kg"`.
    UnitOf,
    /// The number of a quantity in its own unit, as a `Scientific` that
    /// carries its uncertainty: `10.5(3)kg.value()` is `10.5(3)`.
    ValueOf,
    /// An angle as text in degrees, minutes and seconds: `45.5°.dms()` is
    /// `45°30′0″`. An optional second argument is the places of the seconds.
    Dms,
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
    /// A function down a column that reads the rows around each one: a
    /// running total, a difference, a rank, a rolling mean. See [`Spec`].
    Window(Spec),
    /// A `Decimal` result rounded to `s` places, half away from zero: the
    /// single rounding of a product, a quotient or a power, which are worked
    /// out exactly first. Its argument is that operation, so that an engine
    /// can divide straight to the scale rather than round twice.
    Settle(u8),
}

/// What a [`Func::Window`] does down a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Win {
    CumSum,
    CumMin,
    CumMax,
    CumProd,
    CumCount,
    Diff,
    PctChange,
    Rank(Ties),
    ForwardFill,
    BackwardFill,
    RollingSum,
    RollingMean,
    RollingMin,
    RollingMax,
    RollingStd,
}

/// How `rank` numbers values that are equal, as Polars' `method` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ties {
    Average,
    Min,
    Max,
    Dense,
    Ordinal,
}

/// A window function and the shape of its call. The arguments of the
/// call are: the column, the number (a window length or a lag, or zero
/// where there is none), the order key if `order`, one column for each of
/// `parts` partition keys and, when `row`, the place of this row, the
/// column being worked out once and this row's answer looked up in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub win: Win,
    pub desc: bool,
    pub order: bool,
    pub parts: u8,
    pub row: bool,
}

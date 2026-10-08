//! The checked form of an expression: every name is resolved to an index, so
//! evaluation never looks anything up by text.

use crate::ast::{BinOp, Lit, UnOp};
use crate::diag::Span;

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
    Table(usize),
    /// A whole column of a sheet table.
    Column {
        table: usize,
        col: usize,
    },
    /// A column of the row `depth` frames out (0 is the innermost row).
    RowCol {
        depth: usize,
        col: usize,
    },
    Unary(UnOp, Box<Node>),
    Binary(BinOp, Box<Node>, Box<Node>),
    If(Box<Node>, Box<Node>, Box<Node>),
    VecLit(Vec<Node>),
    Index {
        base: Box<Node>,
        rows: RowSel,
        cols: ColSel,
    },
    /// A column of a table or row value.
    Field {
        base: Box<Node>,
        col: usize,
    },
    Call(Func, Vec<Node>),
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
    /// One row relative to the current row; outside the table gives empty.
    Cursor(Offset),
    Range {
        lo: Bound,
        hi: Bound,
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
    Avg,
    Min,
    Max,
    Count,
    Approx,
    Complex,
}

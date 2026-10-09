//! Syntax trees for OMX expressions.

use crate::diag::Span;
use num_bigint::BigInt;
use num_rational::BigRational;

#[derive(Clone, Debug, PartialEq)]
pub enum Lit {
    Int(BigInt),
    Rat(BigRational),
    Num(f64),
    /// Real and imaginary parts.
    Complex(f64, f64),
    Text(String),
    Bool(bool),
    /// Days since 1970-01-01.
    Date(i32),
    /// Seconds since midnight.
    Time(i32),
    /// Seconds since the start of 1970-01-01.
    DateTime(i64),
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Lit(Lit),
    Name(String),
    /// `*`, the current row.
    Cursor,
    /// The table of the current row: the base of `[Revenue; *-1]`, an index
    /// written with no table name.
    Own,
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    VecLit(Vec<Expr>),
    /// `base[slot; slot]`; an empty slot is `None`.
    Index(Box<Expr>, Vec<Option<Expr>>),
    /// `base.Name`
    Field(Box<Expr>, String, Span),
    /// `name(args)`, `recv.name(args)` and `recv |> name(args)` all become a
    /// call with the receiver as the first argument.
    Call(String, Span, Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    In,
    Fallback,
    Range,
    RangeEx,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Pow => "**",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
            BinOp::In => "in",
            BinOp::Fallback => "//",
            BinOp::Range => "..",
            BinOp::RangeEx => "..^",
        }
    }

    pub fn is_arithmetic(self) -> bool {
        matches!(
            self,
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Pow
        )
    }

    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
        )
    }
}

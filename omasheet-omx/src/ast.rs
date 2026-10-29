// Copyright (c) 2026 Stephen Roe

//! Syntax trees for OMX expressions.

use crate::decimal::Dec;
use crate::diag::Span;

#[derive(Clone, Debug, PartialEq)]
pub enum Lit {
    Int(i64),
    /// Written with a decimal point: `19.99`.
    Decimal(Dec),
    /// Written with a `%`: `20%`, held as `0.2`.
    Percent(Dec),
    Float(f64),
    String(String),
    Boolean(bool),
    /// Days since 1970-01-01.
    Date(i32),
    /// Nanoseconds since midnight, as Polars holds a `Time`.
    Time(i64),
    /// Microseconds since the start of 1970-01-01, on the clocks of the
    /// file's own zone, as Polars holds a `Datetime`.
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
    /// The sheet of the current row: the base of `[Revenue; *-1]`, an index
    /// written with no sheet name.
    Own,
    /// `$-n`: this column `n` rows back, inside a sequence.
    Back(u32),
    /// `a, b, c ... limit`: a sequence that fills a column. The last term
    /// before `...` is the rule when it reads a back-reference, and a seed
    /// otherwise; with no rule the step is taken from the seeds.
    Seq {
        seeds: Vec<Expr>,
        rule: Option<Box<Expr>>,
        limit: Box<Expr>,
    },
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
    /// `Name := expr`, an argument given a name, as `agg` takes one.
    Named(String, Span, Box<Expr>),
}

/// Whether an expression reads a back-reference, which is what marks the
/// last term before `...` as the rule rather than another seed.
pub fn reads_back(e: &Expr) -> bool {
    let any = |es: &[Expr]| es.iter().any(reads_back);
    match &e.kind {
        ExprKind::Back(_) => true,
        ExprKind::Lit(_) | ExprKind::Name(_) | ExprKind::Cursor | ExprKind::Own => false,
        ExprKind::Unary(_, x) => reads_back(x),
        ExprKind::Binary(_, a, b) => reads_back(a) || reads_back(b),
        ExprKind::If(c, a, b) => reads_back(c) || reads_back(a) || reads_back(b),
        ExprKind::VecLit(items) => any(items),
        ExprKind::Index(base, slots) => reads_back(base) || slots.iter().flatten().any(reads_back),
        ExprKind::Field(base, _, _) => reads_back(base),
        ExprKind::Call(_, _, args) => any(args),
        ExprKind::Named(_, _, value) => reads_back(value),
        ExprKind::Seq { seeds, rule, limit } => {
            any(seeds) || rule.as_deref().is_some_and(reads_back) || reads_back(limit)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    /// `^n`: the range of the first `n`, from 0 to `n - 1`.
    UpTo,
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
    RangeFrom,
    RangeBoth,
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
            BinOp::RangeFrom => "^..",
            BinOp::RangeBoth => "^..^",
        }
    }

    /// For a range: whether it leaves out its first end, and its last.
    pub fn range_ends(self) -> Option<(bool, bool)> {
        match self {
            BinOp::Range => Some((false, false)),
            BinOp::RangeEx => Some((false, true)),
            BinOp::RangeFrom => Some((true, false)),
            BinOp::RangeBoth => Some((true, true)),
            _ => None,
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

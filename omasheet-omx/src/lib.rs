// Copyright (c) 2026 Stephen Roe

//! OMX: the Omasheet expression language and the `.omx` sheet format.
//!
//! This crate turns source text into a checked [`Program`]: lexing, parsing,
//! name resolution, type and shape checking, and dependency analysis. It does
//! not evaluate anything; that is `omasheet-engine`.

pub mod ast;
pub mod check;
pub mod convert;
pub mod date;
pub mod deps;
pub mod diag;
pub mod funcs;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod sheet;
pub mod types;

pub use check::{
    Cell, ColKind, Column, Const, Group, Program, Table, UserFn, compile, compile_expr,
};
pub use diag::{Diagnostic, Sources, Span};
pub use types::{S, Ty};

// Copyright (c) 2026 Stephen Roe

//! OMX: the Omasheet expression language and the `.omx` file format.
//!
//! This crate turns source text into a checked [`Program`]: lexing, parsing,
//! name resolution, type and shape checking, and dependency analysis. It does
//! not evaluate anything; that is `omasheet-engine`.

pub mod ast;
pub mod check;
pub mod colfmt;
pub mod convert;
pub mod cycle;
pub mod date;
pub mod datefmt;
pub mod decimal;
pub mod deps;
pub mod diag;
pub mod file;
pub mod funcs;
pub mod ir;
pub mod lexer;
pub mod numfmt;
pub mod parser;
pub mod text;
pub mod types;

pub use check::{
    Cell, ColKind, Column, Const, Group, Program, Sheet, UserFn, compile, compile_expr,
};
pub use decimal::Dec;
pub use diag::{Diagnostic, Sources, Span};
pub use types::{S, Ty};

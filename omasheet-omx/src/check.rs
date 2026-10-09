// Copyright (c) 2026 Stephen Roe

//! Semantic analysis: from syntax to a checked [`Program`].
//!
//! Resolves every name, checks types and shapes, decides what each cell is
//! (literal or formula), records what each constant and column reads, and
//! orders the calculation. Nothing is evaluated here.

use crate::ast::{BinOp, Expr, ExprKind, Lit, UnOp};
use crate::deps;
use crate::diag::{Diagnostic, Span};
use crate::funcs::{FUNCTIONS, MathFn};
use crate::ir::{Bound, ColSel, Func, Ir, Node, Offset, RowSel};
use crate::lexer::{Tok, lex};
use crate::parser::parse_expr;
use crate::sheet::{SheetAst, parse_sheet};
use crate::types::{DepKind, S, TableTy, Ty};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};
use std::mem;
use std::rc::Rc;

/// A checked sheet, ready to evaluate.
#[derive(Debug, Default)]
pub struct Program {
    /// The time zone the sheet names for its date-times, and where.
    pub zone: Option<(String, Span)>,
    pub tables: Vec<Table>,
    pub consts: Vec<Const>,
    /// The functions the sheet defines.
    pub funcs: Vec<UserFn>,
    /// Calculation order: each group only reads groups before it.
    pub order: Vec<Group>,
    /// Where the source names a column of a table: `(span, table, column)`.
    /// A formula checked more than once is listed more than once.
    pub col_refs: Vec<(Span, usize, usize)>,
}

#[derive(Debug)]
pub struct Table {
    pub name: String,
    pub cols: Vec<Column>,
    pub nrows: usize,
}

#[derive(Debug)]
pub struct Column {
    pub name: String,
    pub span: Span,
    pub ty: S,
    /// The type was written in the schema rather than inferred.
    pub declared: bool,
    pub kind: ColKind,
}

#[derive(Debug)]
pub enum ColKind {
    Data(Vec<Cell>),
    Computed(Node),
}

#[derive(Debug)]
pub enum Cell {
    Empty,
    Lit(Lit),
    Formula(Node),
    /// A cell that failed to check.
    Invalid,
}

#[derive(Debug)]
pub struct Const {
    pub name: String,
    pub node: Node,
    pub ty: Ty,
    stat: Option<Static>,
}

/// A function defined in the sheet with `func`. It is checked afresh at
/// each call, for the types of that call's arguments.
#[derive(Clone, Debug)]
pub struct UserFn {
    pub name: String,
    pub span: Span,
    pub params: Vec<String>,
    /// `None` if the body did not parse.
    pub body: Option<Rc<Expr>>,
    /// The source text of the body.
    pub source: String,
    /// The comment written above the definition.
    pub doc: String,
}

impl UserFn {
    /// How it is called, such as `Margin(revenue, cost)`.
    pub fn usage(&self) -> String {
        format!("{}({})", self.name, self.params.join(", "))
    }
}

#[derive(Debug)]
pub enum Group {
    Const(usize),
    /// Columns (table, column) calculated together, row by row.
    Cols {
        cols: Vec<(usize, usize)>,
        reverse: bool,
    },
}

/// A constant whose value is plain from its source: `3` or `3..7`.
#[derive(Clone, Copy, Debug)]
enum Static {
    Int(i64),
    Range(i64, i64, bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DepNode {
    Const(usize),
    Col(usize, usize),
}

type Dep = (DepNode, DepKind);

#[derive(Clone, Copy)]
enum ColSrc {
    /// Index into the header row.
    Data(usize),
    /// Index into the table's computed declarations.
    Computed(usize),
    /// Already checked (when compiling an expression against a program).
    Done,
}

struct ColInfo {
    name: String,
    span: Span,
    declared: Option<S>,
    src: ColSrc,
}

struct TableInfo {
    name: String,
    decl: usize,
    cols: Vec<ColInfo>,
    nrows: usize,
}

struct ConstInfo {
    name: String,
    span: Span,
    decl: usize,
    stat: Option<Static>,
}

enum St<T> {
    Pending,
    /// Being checked; carries the best type known so far.
    Busy(T),
    Done(T),
}

/// The rows an expression can see. The innermost row is last.
struct Scope {
    frames: Vec<usize>,
    /// `frames[0]` is the row that owns the expression (a cell's own row).
    own: bool,
    /// The column that owns the expression, in the table of `frames[0]`.
    col: Option<usize>,
}

struct RowPick {
    sel: RowSel,
    one: bool,
    len: Option<usize>,
    kind: DepKind,
}

impl RowPick {
    /// Rows that cannot be told until a function is called: the selector
    /// rests on a parameter. Nothing that depends on them is checked.
    fn unknown(span: Span) -> RowPick {
        RowPick {
            sel: RowSel::Pred(Box::new(Node::error(span))),
            one: false,
            len: None,
            kind: DepKind::Whole,
        }
    }

    fn is_unknown(&self) -> bool {
        matches!(&self.sel, RowSel::Pred(node) if matches!(node.kind, Ir::Error))
    }

    fn all(len: Option<usize>) -> RowPick {
        RowPick {
            sel: RowSel::All,
            one: false,
            len,
            kind: DepKind::Whole,
        }
    }
}

struct Checker<'a> {
    src: u32,
    text: &'a str,
    ast: Option<&'a SheetAst>,
    tables: Vec<TableInfo>,
    consts: Vec<ConstInfo>,
    funcs: Vec<UserFn>,
    /// The parameters of the function whose body is being checked: each
    /// name and the type of its argument, or `None` to check the body alone.
    params: Vec<(String, Option<Ty>)>,
    /// The functions whose bodies are being checked, outermost first.
    calling: Vec<usize>,
    col_state: Vec<Vec<St<S>>>,
    col_out: Vec<Vec<Option<ColKind>>>,
    col_deps: Vec<Vec<Vec<Dep>>>,
    const_state: Vec<St<Ty>>,
    const_out: Vec<Option<Node>>,
    const_deps: Vec<Vec<Dep>>,
    /// Diagnostics and reads of the expression being checked right now.
    cur: Vec<Diagnostic>,
    deps: Vec<Dep>,
    done: Vec<Diagnostic>,
    col_refs: Vec<(Span, usize, usize)>,
}

/// Compile a sheet. If any diagnostic is returned the program must not be
/// evaluated.
pub fn compile(text: &str, src: u32) -> (Program, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let ast = parse_sheet(text, src, &mut diags);
    let mut ck = Checker::for_sheet(&ast, text, src);
    for i in 0..ck.funcs.len() {
        ck.check_func(i);
    }
    for i in 0..ck.consts.len() {
        ck.ensure_const(i);
    }
    for t in 0..ck.tables.len() {
        for c in 0..ck.tables[t].cols.len() {
            ck.ensure_col(t, c);
        }
    }
    let order = ck.order();
    diags.append(&mut ck.done);
    diags.sort_by_key(|d| (d.span.start, d.span.end));
    (ck.finish(order), diags)
}

/// Compile one expression that may refer to the tables and constants of
/// `prog`. It has no current row.
pub fn compile_expr(prog: &Program, text: &str, src: u32) -> Result<(Node, Ty), Vec<Diagnostic>> {
    let expr = parse_expr(text, src, 0).map_err(|d| vec![d])?;
    let mut ck = Checker::for_program(prog, text, src);
    let mut scope = Scope {
        frames: Vec::new(),
        own: false,
        col: None,
    };
    let out = ck.lower(&expr, &mut scope);
    if ck.cur.is_empty() {
        Ok(out)
    } else {
        Err(ck.cur)
    }
}

fn static_of(e: &Expr) -> Option<Static> {
    fn int(e: &Expr) -> Option<i64> {
        match &e.kind {
            ExprKind::Lit(Lit::Int(n)) => n.to_i64(),
            ExprKind::Unary(UnOp::Neg, inner) => int(inner).map(|n| -n),
            _ => None,
        }
    }
    match &e.kind {
        ExprKind::Binary(op @ (BinOp::Range | BinOp::RangeEx), lo, hi) => {
            Some(Static::Range(int(lo)?, int(hi)?, *op == BinOp::RangeEx))
        }
        _ => int(e).map(Static::Int),
    }
}

fn has_cursor(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Cursor => true,
        ExprKind::Lit(_) | ExprKind::Name(_) | ExprKind::Own => false,
        ExprKind::Unary(_, a) | ExprKind::Field(a, ..) => has_cursor(a),
        ExprKind::Binary(_, a, b) => has_cursor(a) || has_cursor(b),
        ExprKind::If(a, b, c) => has_cursor(a) || has_cursor(b) || has_cursor(c),
        ExprKind::VecLit(items) | ExprKind::Call(_, _, items) => items.iter().any(has_cursor),
        // A cursor inside a nested index belongs to that index.
        ExprKind::Index(base, _) => has_cursor(base),
    }
}

/// `*`, `* + n` or `* - n`: the offset expression and whether it is negated.
fn cursor_form(e: &Expr) -> Option<Option<(bool, &Expr)>> {
    match &e.kind {
        ExprKind::Cursor => Some(None),
        ExprKind::Binary(op @ (BinOp::Add | BinOp::Sub), l, r)
            if matches!(l.kind, ExprKind::Cursor) =>
        {
            Some(Some((*op == BinOp::Sub, r)))
        }
        _ => None,
    }
}

fn lit_type(l: &Lit) -> S {
    match l {
        Lit::Int(_) => S::Int,
        Lit::Rat(_) => S::Rat,
        Lit::Num(_) => S::Num,
        Lit::Complex(..) => S::Complex,
        Lit::Text(_) => S::Text,
        Lit::Bool(_) => S::Bool,
        Lit::Date(_) => S::Date,
        Lit::Time(_) => S::Time,
        Lit::DateTime(_) => S::DateTime,
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut prev = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1].eq_ignore_ascii_case(&b[j - 1]) {
                0
            } else {
                1
            };
            let next = (prev + cost).min(row[j] + 1).min(row[j - 1] + 1);
            prev = row[j];
            row[j] = next;
        }
    }
    row[b.len()]
}

fn closest<'n>(name: &str, candidates: impl Iterator<Item = &'n str>) -> Option<&'n str> {
    candidates
        .map(|c| (edit_distance(name, c), c))
        .filter(|(d, c)| *d <= 2 && *d < name.len().max(c.len()))
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

impl<'a> Checker<'a> {
    fn empty(text: &'a str, src: u32) -> Checker<'a> {
        Checker {
            src,
            text,
            ast: None,
            tables: Vec::new(),
            consts: Vec::new(),
            funcs: Vec::new(),
            params: Vec::new(),
            calling: Vec::new(),
            col_state: Vec::new(),
            col_out: Vec::new(),
            col_deps: Vec::new(),
            const_state: Vec::new(),
            const_out: Vec::new(),
            const_deps: Vec::new(),
            cur: Vec::new(),
            deps: Vec::new(),
            done: Vec::new(),
            col_refs: Vec::new(),
        }
    }

    fn for_program(prog: &Program, text: &'a str, src: u32) -> Checker<'a> {
        let mut ck = Checker::empty(text, src);
        for (i, t) in prog.tables.iter().enumerate() {
            ck.col_state
                .push(t.cols.iter().map(|c| St::Done(c.ty)).collect());
            ck.tables.push(TableInfo {
                name: t.name.clone(),
                decl: i,
                nrows: t.nrows,
                cols: t
                    .cols
                    .iter()
                    .map(|c| ColInfo {
                        name: c.name.clone(),
                        span: c.span,
                        declared: None,
                        src: ColSrc::Done,
                    })
                    .collect(),
            });
        }
        for (i, c) in prog.consts.iter().enumerate() {
            ck.const_state.push(St::Done(c.ty.clone()));
            ck.consts.push(ConstInfo {
                name: c.name.clone(),
                span: c.node.span,
                decl: i,
                stat: c.stat,
            });
        }
        ck.funcs = prog.funcs.clone();
        ck
    }

    /// Collect declarations and report clashes; nothing is lowered yet.
    fn for_sheet(ast: &'a SheetAst, text: &'a str, src: u32) -> Checker<'a> {
        let mut ck = Checker::empty(text, src);
        ck.ast = Some(ast);

        for (decl, t) in ast.tables.iter().enumerate() {
            if ck.tables.iter().any(|x| x.name == t.name) {
                ck.done.push(Diagnostic::new(
                    t.span,
                    format!("table `{}` is already defined", t.name),
                ));
                continue;
            }
            let mut cols: Vec<ColInfo> = Vec::new();
            for (h, (name, span)) in t.header.iter().enumerate() {
                if cols.iter().any(|c| &c.name == name) {
                    ck.done.push(Diagnostic::new(
                        *span,
                        format!("column `{name}` appears twice in table `{}`", t.name),
                    ));
                }
                cols.push(ColInfo {
                    name: name.clone(),
                    span: *span,
                    declared: None,
                    src: ColSrc::Data(h),
                });
            }
            for (k, cd) in t.computed.iter().enumerate() {
                if let Some(other) = cols.iter().find(|c| c.name == cd.name) {
                    let what = match other.src {
                        ColSrc::Data(_) => "also has data in the header row",
                        _ => "is defined twice",
                    };
                    ck.done.push(Diagnostic::new(
                        cd.span,
                        format!("computed column `{}` {what}", cd.name),
                    ));
                    continue;
                }
                cols.push(ColInfo {
                    name: cd.name.clone(),
                    span: cd.span,
                    declared: None,
                    src: ColSrc::Computed(k),
                });
            }
            for line in &t.schema {
                match cols.iter_mut().find(|c| c.name == line.name) {
                    Some(c) if c.declared.is_some() => ck.done.push(Diagnostic::new(
                        line.span,
                        format!("the type of `{}` is declared twice", line.name),
                    )),
                    Some(c) => c.declared = Some(line.ty),
                    None => {
                        let mut d = Diagnostic::new(
                            line.span,
                            format!("table `{}` has no column `{}`", t.name, line.name),
                        );
                        if let Some(c) = closest(&line.name, cols.iter().map(|c| c.name.as_str())) {
                            d = d.with_help(format!("did you mean `{c}`?"));
                        }
                        ck.done.push(d);
                    }
                }
            }
            ck.col_state
                .push(cols.iter().map(|_| St::Pending).collect());
            ck.col_out.push(cols.iter().map(|_| None).collect());
            ck.col_deps.push(cols.iter().map(|_| Vec::new()).collect());
            ck.tables.push(TableInfo {
                name: t.name.clone(),
                decl,
                cols,
                nrows: t.rows.len(),
            });
        }

        for (decl, c) in ast.consts.iter().enumerate() {
            let clash = if ck.consts.iter().any(|x| x.name == c.name) {
                Some("constant")
            } else if ck.tables.iter().any(|t| t.name == c.name) {
                Some("table")
            } else {
                None
            };
            if let Some(what) = clash {
                ck.done.push(Diagnostic::new(
                    c.span,
                    format!("the name `{}` is already used by a {what}", c.name),
                ));
                continue;
            }
            ck.consts.push(ConstInfo {
                name: c.name.clone(),
                span: c.span,
                decl,
                stat: c.expr.as_ref().and_then(static_of),
            });
            ck.const_state.push(St::Pending);
            ck.const_out.push(None);
            ck.const_deps.push(Vec::new());
        }

        for f in &ast.funcs {
            if FUNCTIONS.iter().any(|b| b.name == f.name) {
                ck.done.push(Diagnostic::new(
                    f.span,
                    format!("`{}` is a built-in function", f.name),
                ));
                continue;
            }
            if ck.funcs.iter().any(|x| x.name == f.name) {
                ck.done.push(Diagnostic::new(
                    f.span,
                    format!("function `{}` is already defined", f.name),
                ));
                continue;
            }
            for (k, (name, span)) in f.params.iter().enumerate() {
                if f.params[..k].iter().any(|p| &p.0 == name) {
                    ck.done.push(Diagnostic::new(
                        *span,
                        format!("function `{}` has two parameters named `{name}`", f.name),
                    ));
                }
            }
            ck.funcs.push(UserFn {
                name: f.name.clone(),
                span: f.span,
                params: f.params.iter().map(|p| p.0.clone()).collect(),
                body: f.expr.clone().map(Rc::new),
                source: text[f.expr_span.start as usize..f.expr_span.end as usize].to_string(),
                doc: f.doc.clone(),
            });
        }
        ck
    }

    fn err(&mut self, span: Span, message: impl Into<String>) {
        self.cur.push(Diagnostic::new(span, message));
    }

    fn err_help(&mut self, span: Span, message: impl Into<String>, help: impl Into<String>) {
        self.cur
            .push(Diagnostic::new(span, message).with_help(help));
    }

    fn snippet(&self, span: Span) -> &str {
        self.text
            .get(span.start as usize..span.end as usize)
            .unwrap_or("")
            .trim()
    }

    fn find_col(&self, t: usize, name: &str) -> Option<usize> {
        self.tables[t].cols.iter().position(|c| c.name == name)
    }

    fn all_cols(&self, t: usize) -> Rc<Vec<usize>> {
        Rc::new((0..self.tables[t].cols.len()).collect())
    }

    // ---- lazily checked declarations --------------------------------------

    fn col_type(&mut self, t: usize, c: usize) -> S {
        if let Some(d) = self.tables[t].cols[c].declared {
            return d;
        }
        if matches!(self.col_state[t][c], St::Pending) {
            self.ensure_col(t, c);
        }
        match &self.col_state[t][c] {
            St::Busy(s) | St::Done(s) => *s,
            St::Pending => S::Any,
        }
    }

    fn const_type(&mut self, i: usize) -> Ty {
        if matches!(self.const_state[i], St::Pending) {
            self.ensure_const(i);
        }
        match &self.const_state[i] {
            St::Busy(t) | St::Done(t) => t.clone(),
            St::Pending => Ty::Any,
        }
    }

    fn ensure_const(&mut self, i: usize) {
        if !matches!(self.const_state[i], St::Pending) {
            return;
        }
        let Some(ast) = self.ast else { return };
        self.const_state[i] = St::Busy(Ty::Any);
        let saved = (mem::take(&mut self.cur), mem::take(&mut self.deps));
        let decl = &ast.consts[self.consts[i].decl];
        let (node, ty) = match &decl.expr {
            Some(e) => {
                let mut scope = Scope {
                    frames: Vec::new(),
                    own: false,
                    col: None,
                };
                self.lower(e, &mut scope)
            }
            None => (Node::error(decl.span), Ty::Any),
        };
        self.const_out[i] = Some(node);
        self.const_state[i] = St::Done(ty);
        self.const_deps[i] = mem::replace(&mut self.deps, saved.1);
        let mut found = mem::replace(&mut self.cur, saved.0);
        self.done.append(&mut found);
    }

    fn ensure_col(&mut self, t: usize, c: usize) {
        if !matches!(self.col_state[t][c], St::Pending) {
            return;
        }
        let Some(ast) = self.ast else { return };
        let decl = &ast.tables[self.tables[t].decl];
        let declared = self.tables[t].cols[c].declared;
        let saved = (mem::take(&mut self.cur), mem::take(&mut self.deps));

        let (kind, ty) = match self.tables[t].cols[c].src {
            ColSrc::Done => unreachable!("a checked program has no pending columns"),
            ColSrc::Computed(k) => {
                self.col_state[t][c] = St::Busy(declared.unwrap_or(S::Any));
                let cd = &decl.computed[k];
                match &cd.expr {
                    Some(e) => {
                        let (node, s) = self.lower_cell(e, t, c);
                        let s = self.fit_declared(s, declared, e.span, &cd.name);
                        (ColKind::Computed(node), s)
                    }
                    None => (
                        ColKind::Computed(Node::error(cd.span)),
                        declared.unwrap_or(S::Any),
                    ),
                }
            }
            ColSrc::Data(h) => {
                // Literal cells first, so that formulas in the column that
                // read the column see its literal type.
                let mut cells = Vec::with_capacity(decl.rows.len());
                let mut formulas = Vec::new();
                let mut inferred: Option<S> = None;
                let mut widen = |s: S| {
                    inferred = Some(match inferred {
                        None => s,
                        Some(prev) => prev.join(s).unwrap_or(S::Any),
                    });
                };
                for (r, row) in decl.rows.iter().enumerate() {
                    let cell = &row[h];
                    if cell.text.is_empty() {
                        cells.push(Cell::Empty);
                    } else if cell.text.starts_with('=') {
                        formulas.push(r);
                        cells.push(Cell::Invalid);
                    } else {
                        match self.literal_cell(&cell.text, cell.span, declared) {
                            Some(lit) => {
                                widen(lit_type(&lit));
                                cells.push(Cell::Lit(lit));
                            }
                            None => cells.push(Cell::Invalid),
                        }
                    }
                }
                self.col_state[t][c] = St::Busy(declared.or(inferred).unwrap_or(S::Any));
                let name = self.tables[t].cols[c].name.clone();
                for r in formulas {
                    let cell = &decl.rows[r][h];
                    let at = cell.span.start as usize + 1;
                    match parse_expr(&cell.text[1..], self.src, at) {
                        Ok(e) => {
                            let (node, s) = self.lower_cell(&e, t, c);
                            if declared.is_some() {
                                self.fit_declared(s, declared, e.span, &name);
                            } else if s != S::Any || inferred.is_none() {
                                inferred = Some(match inferred {
                                    None => s,
                                    Some(prev) => prev.join(s).unwrap_or(S::Any),
                                });
                            }
                            cells[r] = Cell::Formula(node);
                        }
                        Err(d) => self.cur.push(d),
                    }
                }
                (
                    ColKind::Data(cells),
                    declared.or(inferred).unwrap_or(S::Any),
                )
            }
        };

        self.col_out[t][c] = Some(kind);
        self.col_state[t][c] = St::Done(ty);
        self.col_deps[t][c] = mem::replace(&mut self.deps, saved.1);
        let mut found = mem::replace(&mut self.cur, saved.0);
        self.done.append(&mut found);
    }

    /// Check the body of a function on its own, for what is wrong with it
    /// whatever it is called with. Nothing is kept: each call checks it again.
    fn check_func(&mut self, f: usize) {
        let Some(body) = self.funcs[f].body.clone() else {
            return;
        };
        let saved = (mem::take(&mut self.cur), mem::take(&mut self.deps));
        let params = self.funcs[f]
            .params
            .iter()
            .map(|p| (p.clone(), None))
            .collect();
        let outer = mem::replace(&mut self.params, params);
        self.calling.push(f);
        let mut scope = Scope {
            frames: Vec::new(),
            own: false,
            col: None,
        };
        self.lower(&body, &mut scope);
        self.calling.pop();
        self.params = outer;
        self.deps = saved.1;
        let mut found = mem::replace(&mut self.cur, saved.0);
        self.done.append(&mut found);
    }

    /// A call of function `f`: its body, checked for these arguments. What
    /// is wrong inside the body is reported at the call.
    fn apply(
        &mut self,
        f: usize,
        name_span: Span,
        args: &[Expr],
        span: Span,
        sc: &mut Scope,
    ) -> (Node, Ty) {
        let fail = (Node::error(span), Ty::Any);
        let func = self.funcs[f].clone();
        if args.len() != func.params.len() {
            let want = func.params.len();
            self.err_help(
                span,
                format!(
                    "`{}` takes {want} argument{}, found {}",
                    func.name,
                    if want == 1 { "" } else { "s" },
                    args.len()
                ),
                format!("usage: `{}`", func.usage()),
            );
            return fail;
        }
        if self.calling.contains(&f) {
            self.err_help(
                name_span,
                format!("function `{}` calls itself", func.name),
                "a function cannot be defined in terms of itself",
            );
            return fail;
        }
        let mut nodes = Vec::new();
        let mut params = Vec::new();
        for (arg, param) in args.iter().zip(&func.params) {
            let (node, ty) = self.lower(arg, sc);
            // An argument that failed to check is unknown to the body.
            let known = !matches!(node.kind, Ir::Error);
            params.push((param.clone(), known.then_some(ty)));
            nodes.push(node);
        }
        // The definition already carries the complaint.
        let Some(body) = func.body else { return fail };

        let outer = mem::replace(&mut self.params, params);
        let reported = mem::take(&mut self.cur);
        self.calling.push(f);
        let mut scope = Scope {
            frames: Vec::new(),
            own: false,
            col: None,
        };
        let (body, ty) = self.lower(&body, &mut scope);
        self.calling.pop();
        self.params = outer;
        let inside = mem::replace(&mut self.cur, reported);
        for d in &inside {
            let mut at = Diagnostic::new(name_span, format!("in `{}`: {}", func.name, d.message));
            at.help = d.help.clone();
            if !self
                .cur
                .iter()
                .any(|x| x.span == at.span && x.message == at.message)
            {
                self.cur.push(at);
            }
        }
        if !inside.is_empty() {
            return fail;
        }
        (
            Node::new(
                Ir::Apply {
                    name: func.name,
                    args: nodes,
                    body: Box::new(body),
                },
                span,
            ),
            ty,
        )
    }

    /// Lower an expression that produces one cell of table `t`.
    fn lower_cell(&mut self, e: &Expr, t: usize, c: usize) -> (Node, S) {
        let mut scope = Scope {
            frames: vec![t],
            own: true,
            col: Some(c),
        };
        let (node, ty) = self.lower(e, &mut scope);
        match ty {
            Ty::Scalar(s) => (node, s),
            Ty::Any => (node, S::Any),
            Ty::Vector(s, _) => {
                let span = node.span;
                (Node::new(Ir::Single(Box::new(node)), span), s)
            }
            other => {
                self.err_help(
                    e.span,
                    format!("a cell holds one value, but this is {}", other.describe()),
                    "select one column and aggregate it, for example `.Revenue.sum()`",
                );
                (Node::error(e.span), S::Any)
            }
        }
    }

    fn fit_declared(&mut self, found: S, declared: Option<S>, span: Span, column: &str) -> S {
        let Some(want) = declared else { return found };
        if !found.assignable_to(want) {
            let help = match (found, want) {
                (S::Int | S::Rat, S::Num) => "use `approx(...)` to convert to Num",
                (S::Rat, S::Int) => "declare the column as `Ratio`",
                _ => "change the declared type or the formula",
            };
            self.err_help(
                span,
                format!("column `{column}` is declared `{want}` but this is `{found}`"),
                help,
            );
        }
        want
    }

    /// A cell without a leading `=`: a literal, never an expression.
    fn literal_cell(&mut self, text: &str, span: Span, declared: Option<S>) -> Option<Lit> {
        let parsed = self.parse_literal(text);
        let Some(want) = declared else {
            return Some(parsed.unwrap_or_else(|| Lit::Text(text.to_string())));
        };
        if want == S::Text {
            return Some(match parsed {
                Some(Lit::Text(s)) => Lit::Text(s),
                _ => Lit::Text(text.to_string()),
            });
        }
        let fitted = match (parsed, want) {
            (Some(l @ Lit::Int(_)), S::Int | S::Rat) => Some(l),
            (Some(l @ Lit::Rat(_)), S::Rat) => Some(l),
            (Some(Lit::Int(n)), S::Num) => n.to_f64().map(Lit::Num),
            (Some(Lit::Rat(r)), S::Num) => r.to_f64().map(Lit::Num),
            (Some(l @ Lit::Num(_)), S::Num) => Some(l),
            (Some(l @ Lit::Complex(..)), S::Complex) => Some(l),
            (Some(Lit::Int(n)), S::Complex) => n.to_f64().map(|x| Lit::Complex(x, 0.0)),
            (Some(Lit::Rat(r)), S::Complex) => r.to_f64().map(|x| Lit::Complex(x, 0.0)),
            (Some(Lit::Num(x)), S::Complex) => Some(Lit::Complex(x, 0.0)),
            (Some(l @ Lit::Date(_)), S::Date) => Some(l),
            (Some(l @ Lit::Time(_)), S::Time) => Some(l),
            (Some(l @ Lit::DateTime(_)), S::DateTime) => Some(l),
            (Some(l @ Lit::Bool(_)), S::Bool) => Some(l),
            _ => None,
        };
        if fitted.is_none() {
            let article = if want == S::Int { "an" } else { "a" };
            self.err_help(
                span,
                format!("`{text}` is not {article} `{want}` literal"),
                match want {
                    S::Int => "enter a whole number such as `42`, or start the cell with `=` for a formula",
                    S::Rat => "enter a number such as `19.99`, `20%` or `1/7`, or start the cell with `=` for a formula",
                    S::Num => "enter a number such as `1.5` or `2e-3`, or start the cell with `=` for a formula",
                    S::Complex => {
                        "enter a complex number such as `3+4i`, or start the cell with `=` for a formula"
                    }
                    S::Bool => "enter `true` or `false`, or start the cell with `=` for a formula",
                    S::Date => "enter a date such as `2025-01-31`, or start the cell with `=` for a formula",
                    S::Time => "enter a time such as `09:30` or `09:30:15`, or start the cell with `=` for a formula",
                    S::DateTime => "enter a date-time such as `2025-01-31T09:30`, or start the cell with `=` for a formula",
                    _ => "a formula cell starts with `=`",
                },
            );
        }
        fitted
    }

    fn parse_literal(&self, text: &str) -> Option<Lit> {
        let toks = lex(text, self.src, 0).ok()?;
        let toks: Vec<&Tok> = toks.iter().map(|t| &t.tok).collect();
        // A fraction of two whole numbers, `1/7`, is an exact number.
        let fraction = |n: &BigInt, d: &BigInt| {
            (!d.is_zero()).then(|| Lit::Rat(BigRational::new(n.clone(), d.clone())))
        };
        // A complex number: `4i`, `3+4i`, `-1.5-2i`.
        let real = |t: &Tok| match t {
            Tok::Int(n) => n.to_f64(),
            Tok::Rat(r) => r.to_f64(),
            Tok::Num(f) => Some(*f),
            _ => None,
        };
        let (negative, rest) = match toks.as_slice() {
            [Tok::Minus, rest @ ..] => (true, rest),
            rest => (false, rest),
        };
        let sign = if negative { -1.0 } else { 1.0 };
        match rest {
            [Tok::Imag(im), Tok::Eof] => return Some(Lit::Complex(0.0, sign * im)),
            [re, op @ (Tok::Plus | Tok::Minus), Tok::Imag(im), Tok::Eof] => {
                let im = if **op == Tok::Minus { -im } else { *im };
                return Some(Lit::Complex(sign * real(re)?, im));
            }
            _ => {}
        }
        Some(match toks.as_slice() {
            [Tok::Int(n), Tok::Slash, Tok::Int(d), Tok::Eof] => fraction(n, d)?,
            [Tok::Minus, Tok::Int(n), Tok::Slash, Tok::Int(d), Tok::Eof] => fraction(&-n, d)?,
            [Tok::Int(n), Tok::Eof] => Lit::Int(n.clone()),
            [Tok::Rat(r), Tok::Eof] => Lit::Rat(r.clone()),
            [Tok::Num(f), Tok::Eof] => Lit::Num(*f),
            [Tok::Minus, Tok::Int(n), Tok::Eof] => Lit::Int(-n.clone()),
            [Tok::Minus, Tok::Rat(r), Tok::Eof] => Lit::Rat(-r.clone()),
            [Tok::Minus, Tok::Num(f), Tok::Eof] => Lit::Num(-*f),
            [Tok::Str(s), Tok::Eof] => Lit::Text(s.clone()),
            [Tok::Date(d), Tok::Eof] => Lit::Date(*d),
            [Tok::Time(t), Tok::Eof] => Lit::Time(*t),
            [Tok::DateTime(t), Tok::Eof] => Lit::DateTime(*t),
            [Tok::True, Tok::Eof] => Lit::Bool(true),
            [Tok::False, Tok::Eof] => Lit::Bool(false),
            _ => return None,
        })
    }

    // ---- expressions -------------------------------------------------------

    fn lower(&mut self, e: &Expr, sc: &mut Scope) -> (Node, Ty) {
        let span = e.span;
        let fail = (Node::error(span), Ty::Any);
        match &e.kind {
            ExprKind::Lit(l) => (Node::new(Ir::Lit(l.clone()), span), Ty::Scalar(lit_type(l))),
            ExprKind::Name(name) => self.lower_name(name, span, sc),
            ExprKind::Own => {
                if !sc.own || sc.frames.is_empty() {
                    self.err_help(
                        span,
                        "an index with no table name needs a current table",
                        "name the table, for example `Sales[Revenue; 0]`",
                    );
                    return fail;
                }
                let t = sc.frames[0];
                let ty = Ty::Table(TableTy {
                    table: t,
                    cols: self.all_cols(t),
                    rows: Some(self.tables[t].nrows),
                    kind: DepKind::Whole,
                });
                (Node::new(Ir::Table(t), span), ty)
            }
            ExprKind::Cursor => {
                self.err_help(
                    span,
                    "`*` is the current row and can only be used as a row position",
                    "for example `Sales[Revenue; *-1]`; multiplication needs a value on both sides",
                );
                fail
            }
            ExprKind::Unary(op, operand) => {
                let (node, ty) = self.lower(operand, sc);
                let (s, vector) = match &ty {
                    Ty::Any => return (Node::new(Ir::Unary(*op, Box::new(node)), span), Ty::Any),
                    Ty::Scalar(s) => (*s, None),
                    Ty::Vector(s, n) => (*s, Some(*n)),
                    other => {
                        self.err(
                            span,
                            format!("cannot apply `{}` to {}", unop_text(*op), other.describe()),
                        );
                        return fail;
                    }
                };
                let ok = match op {
                    UnOp::Neg => s.is_numeric(),
                    UnOp::Not => matches!(s, S::Bool | S::Any),
                };
                if !ok {
                    self.err(span, format!("cannot apply `{}` to {s}", unop_text(*op)));
                    return fail;
                }
                let out = match vector {
                    Some(n) => Ty::Vector(s, n),
                    None => Ty::Scalar(s),
                };
                (Node::new(Ir::Unary(*op, Box::new(node)), span), out)
            }
            ExprKind::Binary(op, l, r) => self.lower_binary(*op, l, r, span, sc),
            ExprKind::If(c, a, b) => {
                let (cn, cty) = self.lower(c, sc);
                if !matches!(cty, Ty::Any | Ty::Scalar(S::Bool | S::Any)) {
                    self.err(
                        c.span,
                        format!(
                            "the condition of `if` must be Bool, found {}",
                            cty.describe()
                        ),
                    );
                }
                let (an, aty) = self.lower(a, sc);
                let (bn, bty) = self.lower(b, sc);
                let ty = match (&aty, &bty) {
                    (Ty::Scalar(x), Ty::Scalar(y)) => match x.join(*y) {
                        Some(s) => Ty::Scalar(s),
                        None => {
                            self.err(
                                span,
                                format!("the branches of `if` have different types: {x} and {y}"),
                            );
                            Ty::Any
                        }
                    },
                    (x, y) if x == y => x.clone(),
                    _ => Ty::Any,
                };
                (
                    Node::new(Ir::If(Box::new(cn), Box::new(an), Box::new(bn)), span),
                    ty,
                )
            }
            ExprKind::VecLit(items) => {
                let mut nodes = Vec::new();
                let mut elem: Option<S> = None;
                for item in items {
                    let (node, ty) = self.lower(item, sc);
                    let s = match ty {
                        Ty::Scalar(s) => s,
                        Ty::Any => S::Any,
                        other => {
                            self.err(
                                item.span,
                                format!(
                                    "a vector holds single values, but this is {}",
                                    other.describe()
                                ),
                            );
                            S::Any
                        }
                    };
                    elem = Some(match elem {
                        None => s,
                        Some(prev) => prev.join(s).unwrap_or_else(|| {
                            self.err(item.span, format!("this vector mixes {prev} and {s}"));
                            S::Any
                        }),
                    });
                    nodes.push(node);
                }
                let n = nodes.len();
                (
                    Node::new(Ir::VecLit(nodes), span),
                    Ty::Vector(elem.unwrap_or(S::Any), Some(n)),
                )
            }
            ExprKind::Index(base, slots) => {
                let (b, bty) = self.lower(base, sc);
                let slots: Vec<Option<&Expr>> = slots.iter().map(|s| s.as_ref()).collect();
                self.index(b, bty, &slots, span, sc)
            }
            ExprKind::Field(base, name, name_span) => {
                let (b, bty) = self.lower(base, sc);
                match bty {
                    Ty::Table(tt) | Ty::Row(tt) if self.col_in(&tt, name).is_none() => {
                        self.no_column(&tt, name, *name_span);
                        fail
                    }
                    Ty::Table(tt) => {
                        let c = self.col_in(&tt, name).unwrap();
                        self.col_refs.push((*name_span, tt.table, c));
                        self.deps.push((DepNode::Col(tt.table, c), tt.kind));
                        let s = self.col_type(tt.table, c);
                        let kind = if matches!(b.kind, Ir::Table(_)) {
                            Ir::Column {
                                table: tt.table,
                                col: c,
                            }
                        } else {
                            Ir::Field {
                                base: Box::new(b),
                                col: c,
                            }
                        };
                        (Node::new(kind, span), Ty::Vector(s, tt.rows))
                    }
                    Ty::Row(tt) => {
                        let c = self.col_in(&tt, name).unwrap();
                        self.col_refs.push((*name_span, tt.table, c));
                        self.deps.push((DepNode::Col(tt.table, c), tt.kind));
                        let s = self.col_type(tt.table, c);
                        (
                            Node::new(
                                Ir::Field {
                                    base: Box::new(b),
                                    col: c,
                                },
                                span,
                            ),
                            Ty::Scalar(s),
                        )
                    }
                    Ty::Any if matches!(b.kind, Ir::Error) => fail,
                    other => {
                        self.err(
                            *name_span,
                            format!(
                                "`.{name}` needs a table or a row, but this is {}",
                                other.describe()
                            ),
                        );
                        fail
                    }
                }
            }
            ExprKind::Call(name, name_span, args) => {
                self.lower_call(name, *name_span, args, span, sc)
            }
        }
    }

    fn col_in(&self, tt: &TableTy, name: &str) -> Option<usize> {
        tt.cols
            .iter()
            .copied()
            .find(|&c| self.tables[tt.table].cols[c].name == name)
    }

    fn no_column(&mut self, tt: &TableTy, name: &str, span: Span) {
        let table = &self.tables[tt.table];
        let mut d = Diagnostic::new(
            span,
            format!("table `{}` has no column `{name}`", table.name),
        );
        let names = tt.cols.iter().map(|&c| table.cols[c].name.as_str());
        if let Some(c) = closest(name, names) {
            d = d.with_help(format!("did you mean `{c}`?"));
        }
        self.cur.push(d);
    }

    fn lower_name(&mut self, name: &str, span: Span, sc: &mut Scope) -> (Node, Ty) {
        if let Some(i) = self.params.iter().position(|p| p.0 == name) {
            return match self.params[i].1.clone() {
                Some(ty) => (Node::new(Ir::Arg(i), span), ty),
                // Unknown until the function is called: nothing that
                // depends on it is checked.
                None => (Node::error(span), Ty::Any),
            };
        }
        for i in (0..sc.frames.len()).rev() {
            let t = sc.frames[i];
            if let Some(c) = self.find_col(t, name) {
                self.col_refs.push((span, t, c));
                let kind = if i == 0 && sc.own {
                    DepKind::Same
                } else {
                    DepKind::Whole
                };
                self.deps.push((DepNode::Col(t, c), kind));
                let s = self.col_type(t, c);
                let depth = sc.frames.len() - 1 - i;
                return (Node::new(Ir::RowCol { depth, col: c }, span), Ty::Scalar(s));
            }
        }
        if let Some(i) = self.consts.iter().position(|c| c.name == name) {
            self.deps.push((DepNode::Const(i), DepKind::Whole));
            let ty = self.const_type(i);
            return (Node::new(Ir::Const(i), span), ty);
        }
        if let Some(t) = self.tables.iter().position(|t| t.name == name) {
            let ty = Ty::Table(TableTy {
                table: t,
                cols: self.all_cols(t),
                rows: Some(self.tables[t].nrows),
                kind: DepKind::Whole,
            });
            return (Node::new(Ir::Table(t), span), ty);
        }

        let mut d = Diagnostic::new(span, format!("unknown name `{name}`"));
        let owner = self
            .tables
            .iter()
            .find(|t| t.cols.iter().any(|c| c.name == name));
        if let Some(t) = owner {
            d = d.with_help(format!(
                "`{name}` is a column of `{}`; write `{}.{name}` for the whole column",
                t.name, t.name
            ));
        } else {
            let frames = sc.frames.clone();
            let names = frames
                .iter()
                .flat_map(|&t| self.tables[t].cols.iter().map(|c| c.name.as_str()))
                .chain(self.params.iter().map(|p| p.0.as_str()))
                .chain(self.consts.iter().map(|c| c.name.as_str()))
                .chain(self.tables.iter().map(|t| t.name.as_str()));
            if let Some(c) = closest(name, names) {
                d = d.with_help(format!("did you mean `{c}`?"));
            }
        }
        self.cur.push(d);
        (Node::error(span), Ty::Any)
    }

    fn lower_binary(
        &mut self,
        op: BinOp,
        l: &Expr,
        r: &Expr,
        span: Span,
        sc: &mut Scope,
    ) -> (Node, Ty) {
        let (ln, lty) = self.lower(l, sc);
        let (rn, rty) = self.lower(r, sc);
        let poisoned = matches!(ln.kind, Ir::Error) || matches!(rn.kind, Ir::Error);
        let node = Node::new(Ir::Binary(op, Box::new(ln), Box::new(rn)), span);
        if poisoned {
            return (node, Ty::Any);
        }
        let fail = (Node::error(span), Ty::Any);
        let sym = op.symbol();

        match op {
            BinOp::Range | BinOp::RangeEx => {
                for (ty, e) in [(&lty, l), (&rty, r)] {
                    if !matches!(ty, Ty::Any | Ty::Scalar(S::Int | S::Any)) {
                        self.err(
                            e.span,
                            format!("the ends of a range must be Int, found {}", ty.describe()),
                        );
                        return fail;
                    }
                }
                return (node, Ty::Range);
            }
            BinOp::Fallback => {
                let left = match &lty {
                    Ty::Any => S::Any,
                    Ty::Scalar(s) | Ty::Vector(s, _) => *s,
                    other => {
                        self.err(
                            l.span,
                            format!("`//` needs a value on its left, found {}", other.describe()),
                        );
                        return fail;
                    }
                };
                let right = match &rty {
                    Ty::Any => S::Any,
                    Ty::Scalar(s) => *s,
                    other => {
                        self.err(
                            r.span,
                            format!(
                                "the fallback after `//` must be a single value, found {}",
                                other.describe()
                            ),
                        );
                        return fail;
                    }
                };
                // An unknown left side takes the type of its fallback.
                let s = if left == S::Any {
                    right
                } else {
                    match left.join(right) {
                        Some(s) => s,
                        None => {
                            self.err(
                                span,
                                format!("`//` cannot fall back from {left} to {right}"),
                            );
                            return fail;
                        }
                    }
                };
                return (node, Ty::Scalar(s));
            }
            BinOp::In => {
                let needle = match &lty {
                    Ty::Any => S::Any,
                    Ty::Scalar(s) => *s,
                    other => {
                        self.err(
                            l.span,
                            format!(
                                "`in` needs a single value on its left, found {}",
                                other.describe()
                            ),
                        );
                        return fail;
                    }
                };
                let hay = match &rty {
                    Ty::Any => S::Any,
                    Ty::Vector(s, _) => *s,
                    Ty::Range => S::Int,
                    other => {
                        self.err(
                            r.span,
                            format!(
                                "`in` needs a vector or range on its right, found {}",
                                other.describe()
                            ),
                        );
                        return fail;
                    }
                };
                if needle.join(hay).is_none() {
                    self.err(
                        span,
                        format!("cannot look for {needle} in a vector of {hay}"),
                    );
                    return fail;
                }
                return (node, Ty::Scalar(S::Bool));
            }
            _ => {}
        }

        // Element-wise operators: arithmetic, comparison, and / or.
        let mut parts = Vec::new();
        for ty in [&lty, &rty] {
            parts.push(match ty {
                Ty::Any => return (node, Ty::Any),
                Ty::Scalar(s) => (*s, None),
                Ty::Vector(s, n) => (*s, Some(*n)),
                other => {
                    self.err_help(
                        span,
                        format!("cannot apply `{sym}` to {}", other.describe()),
                        "select a column first, for example `Sales.Revenue`",
                    );
                    return fail;
                }
            });
        }
        let ((a, av), (b, bv)) = (parts[0], parts[1]);
        let unknown = a == S::Any || b == S::Any;
        let elem = if op.is_arithmetic() && (a.is_temporal() || b.is_temporal()) {
            let shifted = match op {
                BinOp::Add => a.shift(false, b),
                BinOp::Sub => a.shift(true, b),
                _ => None,
            };
            let Some(s) = shifted else {
                self.err_help(
                    span,
                    format!("cannot apply `{sym}` to {a} and {b}"),
                    "add or subtract whole days with a Date, whole seconds with a Time or \
                     DateTime; subtract two of the same kind; add a Date and a Time",
                );
                return fail;
            };
            s
        } else if op.is_arithmetic() {
            if !(a.is_numeric() && b.is_numeric()) {
                self.err(span, format!("cannot apply `{sym}` to {a} and {b}"));
                return fail;
            }
            if unknown {
                S::Any
            } else {
                match (op, a.wider(b)) {
                    (BinOp::Div | BinOp::Pow, S::Int) => S::Rat,
                    (_, w) => w,
                }
            }
        } else if op.is_comparison() {
            let equality = matches!(op, BinOp::Eq | BinOp::Ne);
            let ok = unknown
                || match a.join(b) {
                    Some(S::Complex | S::Bool) => equality,
                    Some(_) => true,
                    None => false,
                };
            if !ok {
                self.err(span, format!("cannot compare {a} and {b} with `{sym}`"));
                return fail;
            }
            S::Bool
        } else {
            if !matches!(a, S::Bool | S::Any) || !matches!(b, S::Bool | S::Any) {
                self.err(
                    span,
                    format!("`{sym}` needs Bool on both sides, found {a} and {b}"),
                );
                return fail;
            }
            S::Bool
        };
        let ty = match (av, bv) {
            (None, None) => Ty::Scalar(elem),
            (Some(n), None) | (None, Some(n)) => Ty::Vector(elem, n),
            (Some(Some(x)), Some(Some(y))) if x != y => {
                self.err(
                    span,
                    format!("cannot apply `{sym}` to vectors of different lengths: {x} and {y}"),
                );
                return fail;
            }
            (Some(x), Some(y)) => Ty::Vector(elem, x.or(y)),
        };
        (node, ty)
    }

    fn lower_call(
        &mut self,
        name: &str,
        name_span: Span,
        args: &[Expr],
        span: Span,
        sc: &mut Scope,
    ) -> (Node, Ty) {
        let fail = (Node::error(span), Ty::Any);
        let arity = |ck: &mut Checker, want: usize, usage: &str| {
            if args.len() == want {
                true
            } else {
                ck.err_help(
                    span,
                    format!(
                        "`{name}` takes {want} argument{}, found {}",
                        if want == 1 { "" } else { "s" },
                        args.len()
                    ),
                    format!("usage: `{usage}`"),
                );
                false
            }
        };
        match name {
            "sum" | "avg" | "min" | "max" | "count" => {
                if !arity(self, 1, &format!("Sales.Revenue.{name}()")) {
                    return fail;
                }
                let func = match name {
                    "sum" => Func::Sum,
                    "avg" => Func::Avg,
                    "min" => Func::Min,
                    "max" => Func::Max,
                    _ => Func::Count,
                };
                let (arg, aty) = self.lower(&args[0], sc);
                let (arg, s) = match aty {
                    Ty::Any => return (Node::new(Ir::Call(func, vec![arg]), span), Ty::Any),
                    Ty::Vector(s, _) => (arg, s),
                    Ty::Range => (arg, S::Int),
                    Ty::Table(_) if func == Func::Count => {
                        return (
                            Node::new(Ir::Call(func, vec![arg]), span),
                            Ty::Scalar(S::Int),
                        );
                    }
                    // The cells of one row, such as `[*-4..*-1; *]`.
                    Ty::Row(tt) => {
                        let mut s: Option<Option<S>> = None;
                        for &c in tt.cols.iter() {
                            self.deps.push((DepNode::Col(tt.table, c), tt.kind));
                            let ty = self.col_type(tt.table, c);
                            s = Some(match s {
                                None => Some(ty),
                                Some(so_far) => so_far.and_then(|x| x.join(ty)),
                            });
                        }
                        match s {
                            Some(Some(s)) => (arg, s),
                            None => (arg, S::Any),
                            Some(None) => {
                                self.err_help(
                                    args[0].span,
                                    format!("`{name}` needs values of one type, but the columns of this row differ"),
                                    "select columns of the same type, for example `[1..3; *]`",
                                );
                                return fail;
                            }
                        }
                    }
                    Ty::Table(tt) if tt.cols.len() == 1 => {
                        let c = tt.cols[0];
                        self.deps.push((DepNode::Col(tt.table, c), tt.kind));
                        let s = self.col_type(tt.table, c);
                        let arg_span = arg.span;
                        (
                            Node::new(
                                Ir::Field {
                                    base: Box::new(arg),
                                    col: c,
                                },
                                arg_span,
                            ),
                            s,
                        )
                    }
                    other => {
                        self.err_help(
                            args[0].span,
                            format!("`{name}` needs a vector, but this is {}", other.describe()),
                            "select one column, for example `Sales.Revenue`",
                        );
                        return fail;
                    }
                };
                let out = match func {
                    Func::Count => S::Int,
                    Func::Sum | Func::Avg if !s.is_numeric() => {
                        self.err(
                            args[0].span,
                            format!("cannot take the `{name}` of {s} values"),
                        );
                        return fail;
                    }
                    Func::Avg if s == S::Int => S::Rat,
                    Func::Min | Func::Max if matches!(s, S::Bool | S::Complex) => {
                        self.err(
                            args[0].span,
                            format!("{s} values have no order for `{name}`"),
                        );
                        return fail;
                    }
                    _ => s,
                };
                (Node::new(Ir::Call(func, vec![arg]), span), Ty::Scalar(out))
            }
            "approx" | "Num" => {
                if !arity(self, 1, &format!("{name}(1/3)")) {
                    return fail;
                }
                let (arg, aty) = self.lower(&args[0], sc);
                let ty = match aty {
                    Ty::Any => Ty::Any,
                    Ty::Scalar(s) if s.is_numeric() && s != S::Complex => Ty::Scalar(S::Num),
                    Ty::Vector(s, n) if s.is_numeric() && s != S::Complex => Ty::Vector(S::Num, n),
                    other => {
                        self.err(
                            args[0].span,
                            format!("`{name}` needs a number, but this is {}", other.describe()),
                        );
                        return fail;
                    }
                };
                (Node::new(Ir::Call(Func::Approx, vec![arg]), span), ty)
            }
            "Complex" => {
                if !arity(self, 2, "Complex(re, im)") {
                    return fail;
                }
                let mut nodes = Vec::new();
                for a in args {
                    let (node, ty) = self.lower(a, sc);
                    if !matches!(ty, Ty::Any | Ty::Scalar(S::Any | S::Int | S::Rat | S::Num)) {
                        self.err(
                            a.span,
                            format!(
                                "`Complex` needs real numbers, but this is {}",
                                ty.describe()
                            ),
                        );
                        return fail;
                    }
                    nodes.push(node);
                }
                (
                    Node::new(Ir::Call(Func::Complex, nodes), span),
                    Ty::Scalar(S::Complex),
                )
            }
            "today" | "now" => {
                if !arity(self, 0, &format!("{name}()")) {
                    return fail;
                }
                let (func, s) = if name == "today" {
                    (Func::Today, S::Date)
                } else {
                    (Func::Now, S::DateTime)
                };
                (Node::new(Ir::Call(func, Vec::new()), span), Ty::Scalar(s))
            }
            "year" | "month" | "day" | "weekday" | "hour" | "minute" | "second" | "date"
            | "time" => {
                if !arity(self, 1, &format!("Orders.Placed.{name}()")) {
                    return fail;
                }
                let (func, from, out) = match name {
                    "year" => (Func::Year, S::Date, S::Int),
                    "month" => (Func::Month, S::Date, S::Int),
                    "day" => (Func::Day, S::Date, S::Int),
                    "weekday" => (Func::Weekday, S::Date, S::Int),
                    "hour" => (Func::Hour, S::Time, S::Int),
                    "minute" => (Func::Minute, S::Time, S::Int),
                    "second" => (Func::Second, S::Time, S::Int),
                    "date" => (Func::DateOf, S::Date, S::Date),
                    _ => (Func::TimeOf, S::Time, S::Time),
                };
                let (arg, aty) = self.lower(&args[0], sc);
                let fits = |s: S| matches!(s, S::Any | S::DateTime) || s == from;
                let ty = match aty {
                    Ty::Any | Ty::Scalar(S::Any) => Ty::Scalar(out),
                    Ty::Scalar(s) if fits(s) => Ty::Scalar(out),
                    Ty::Vector(s, n) if fits(s) => Ty::Vector(out, n),
                    other => {
                        if !matches!(arg.kind, Ir::Error) {
                            self.err(
                                args[0].span,
                                format!(
                                    "`{name}` needs a {from} or a DateTime, but this is {}",
                                    other.describe()
                                ),
                            );
                        }
                        return fail;
                    }
                };
                (Node::new(Ir::Call(func, vec![arg]), span), ty)
            }
            "to_zone" | "utc" | "local" | "offset" | "zone" => {
                let takes_zone = name == "to_zone";
                let usage = if takes_zone {
                    "Orders.Placed.to_zone(\"Asia/Tokyo\")".to_string()
                } else {
                    format!("Orders.Placed.{name}()")
                };
                if !arity(self, if takes_zone { 2 } else { 1 }, &usage) {
                    return fail;
                }
                let (func, out) = match name {
                    "to_zone" => (Func::ToZone, S::DateTime),
                    "utc" => (Func::Utc, S::DateTime),
                    "local" => (Func::Local, S::DateTime),
                    "offset" => (Func::Offset, S::Int),
                    _ => (Func::ZoneName, S::Text),
                };
                let (arg, aty) = self.lower(&args[0], sc);
                let ty = match aty {
                    Ty::Any | Ty::Scalar(S::Any | S::DateTime) => Ty::Scalar(out),
                    Ty::Vector(S::Any | S::DateTime, n) => Ty::Vector(out, n),
                    other => {
                        if !matches!(arg.kind, Ir::Error) {
                            self.err_help(
                                args[0].span,
                                format!(
                                    "`{name}` needs a DateTime, but this is {}",
                                    other.describe()
                                ),
                                "a Date or a Time alone is in no time zone; add them to make a \
                                 DateTime",
                            );
                        }
                        return fail;
                    }
                };
                let mut nodes = vec![arg];
                if takes_zone {
                    let (zone, zty) = self.lower(&args[1], sc);
                    if !matches!(zty, Ty::Any | Ty::Scalar(S::Any | S::Text)) {
                        self.err_help(
                            args[1].span,
                            format!(
                                "`to_zone` needs the name of a time zone, but this is {}",
                                zty.describe()
                            ),
                            "for example `\"Europe/London\"`",
                        );
                        return fail;
                    }
                    nodes.push(zone);
                }
                (Node::new(Ir::Call(func, nodes), span), ty)
            }
            "filter" => {
                if !arity(self, 2, "Sales |> filter(Region == \"UK\")") {
                    return fail;
                }
                let (b, bty) = self.lower(&args[0], sc);
                if !matches!(bty, Ty::Table(_) | Ty::Any) {
                    self.err(
                        args[0].span,
                        format!("`filter` needs a table, but this is {}", bty.describe()),
                    );
                    return fail;
                }
                self.index(b, bty, &[None, Some(&args[1])], span, sc)
            }
            "select" => {
                if args.len() < 2 {
                    self.err_help(
                        span,
                        "`select` needs a table and at least one column",
                        "usage: `Sales |> select(Month, Revenue)`",
                    );
                    return fail;
                }
                let (b, bty) = self.lower(&args[0], sc);
                let Ty::Table(tt) = bty else {
                    if !matches!(b.kind, Ir::Error) {
                        self.err(
                            args[0].span,
                            format!("`select` needs a table, but this is {}", bty.describe()),
                        );
                    }
                    return fail;
                };
                let mut cols = Vec::new();
                for a in &args[1..] {
                    let ExprKind::Name(col) = &a.kind else {
                        self.err(a.span, "`select` takes column names");
                        return fail;
                    };
                    match self.col_in(&tt, col) {
                        Some(c) => {
                            self.col_refs.push((a.span, tt.table, c));
                            cols.push(c)
                        }
                        None => {
                            self.no_column(&tt, col, a.span);
                            return fail;
                        }
                    }
                }
                let ty = Ty::Table(TableTy {
                    table: tt.table,
                    cols: Rc::new(cols.clone()),
                    rows: tt.rows,
                    kind: tt.kind,
                });
                (
                    Node::new(
                        Ir::Index {
                            base: Box::new(b),
                            rows: RowSel::All,
                            cols: ColSel::Many(cols),
                        },
                        span,
                    ),
                    ty,
                )
            }
            "pi" | "e" => {
                if !arity(self, 0, &format!("{name}()")) {
                    return fail;
                }
                let func = if name == "pi" { Func::Pi } else { Func::E };
                (
                    Node::new(Ir::Call(func, Vec::new()), span),
                    Ty::Scalar(S::Num),
                )
            }
            _ if MathFn::from_name(name).is_some() => {
                if !arity(self, 1, &format!("{name}(x)")) {
                    return fail;
                }
                let Some(m) = MathFn::from_name(name) else {
                    return fail;
                };
                // An exact number stays exact where the function allows it.
                let out = |s: S| match s {
                    S::Any => Some(S::Any),
                    S::Complex => m.of_complex(),
                    S::Int | S::Rat if m.is_whole() => Some(S::Int),
                    S::Int | S::Rat if m.is_exact() => Some(s),
                    S::Int | S::Rat | S::Num => Some(S::Num),
                    _ => None,
                };
                let (arg, aty) = self.lower(&args[0], sc);
                let ty = match aty {
                    Ty::Any => Ty::Any,
                    Ty::Scalar(s) if out(s).is_some() => Ty::Scalar(out(s).unwrap_or(S::Any)),
                    Ty::Vector(s, n) if out(s).is_some() => Ty::Vector(out(s).unwrap_or(S::Any), n),
                    other => {
                        if !matches!(arg.kind, Ir::Error) {
                            self.err(
                                args[0].span,
                                format!("`{name}` cannot be applied to {}", other.describe()),
                            );
                        }
                        return fail;
                    }
                };
                (Node::new(Ir::Call(Func::Math(m), vec![arg]), span), ty)
            }
            _ if self.funcs.iter().any(|f| f.name == name) => {
                let Some(f) = self.funcs.iter().position(|f| f.name == name) else {
                    return fail;
                };
                self.apply(f, name_span, args, span, sc)
            }
            _ => {
                let known = FUNCTIONS
                    .iter()
                    .map(|f| f.name)
                    .chain(self.funcs.iter().map(|f| f.name.as_str()));
                let mut d = Diagnostic::new(name_span, format!("unknown function `{name}`"));
                d = match closest(name, known) {
                    Some(c) => d.with_help(format!("did you mean `{c}`?")),
                    None => d.with_help("the function directory lists every function"),
                };
                self.cur.push(d);
                fail
            }
        }
    }

    // ---- indexing ----------------------------------------------------------

    fn index(
        &mut self,
        b: Node,
        bty: Ty,
        slots: &[Option<&Expr>],
        span: Span,
        sc: &mut Scope,
    ) -> (Node, Ty) {
        let fail = (Node::error(span), Ty::Any);
        match bty {
            Ty::Table(tt) => {
                if slots.len() > 2 {
                    self.err(span, "a table has two dimensions: `[columns; rows]`");
                    return fail;
                }
                let direct = matches!(b.kind, Ir::Table(_));
                // `[columns; rows]`: the columns come first.
                let pick =
                    self.rowsel(slots.get(1).copied().flatten(), Some(tt.table), tt.rows, sc);
                if pick.is_unknown() {
                    return fail;
                }
                let kind = if direct { pick.kind } else { DepKind::Whole };
                let own = sc.col.filter(|_| sc.own).map(|c| (sc.frames[0], c));
                let Some(cols) = self.colsel(slots.first().copied().flatten(), &tt, own) else {
                    return fail;
                };
                let sub = |cols: Rc<Vec<usize>>, rows| TableTy {
                    table: tt.table,
                    cols,
                    rows,
                    kind,
                };
                let ty = match (&cols, pick.one) {
                    (ColSel::One(c), one) => {
                        self.deps.push((DepNode::Col(tt.table, *c), kind));
                        let s = self.col_type(tt.table, *c);
                        if one {
                            Ty::Scalar(s)
                        } else {
                            Ty::Vector(s, pick.len)
                        }
                    }
                    (ColSel::All, true) => Ty::Row(sub(tt.cols.clone(), None)),
                    (ColSel::All, false) => Ty::Table(sub(tt.cols.clone(), pick.len)),
                    (ColSel::Many(v), true) => Ty::Row(sub(Rc::new(v.clone()), None)),
                    (ColSel::Many(v), false) => Ty::Table(sub(Rc::new(v.clone()), pick.len)),
                };
                (
                    Node::new(
                        Ir::Index {
                            base: Box::new(b),
                            rows: pick.sel,
                            cols,
                        },
                        span,
                    ),
                    ty,
                )
            }
            Ty::Vector(s, n) => {
                if slots.len() != 1 {
                    self.err(span, "a vector has one dimension: `[position]`");
                    return fail;
                }
                let pick = self.rowsel(slots[0], None, n, sc);
                if pick.is_unknown() {
                    return fail;
                }
                let ty = if pick.one {
                    Ty::Scalar(s)
                } else {
                    Ty::Vector(s, pick.len)
                };
                (
                    Node::new(
                        Ir::Index {
                            base: Box::new(b),
                            rows: pick.sel,
                            cols: ColSel::All,
                        },
                        span,
                    ),
                    ty,
                )
            }
            Ty::Any if matches!(b.kind, Ir::Error) => fail,
            other => {
                self.err(span, format!("cannot index {}", other.describe()));
                fail
            }
        }
    }

    fn offset(&mut self, form: Option<(bool, &Expr)>, sc: &mut Scope) -> (Offset, Option<i64>) {
        let Some((negative, e)) = form else {
            return (Offset(None), Some(0));
        };
        let (node, ty) = self.lower(e, sc);
        if !matches!(ty, Ty::Any | Ty::Scalar(S::Int | S::Any)) {
            self.err(
                e.span,
                format!("a row offset must be Int, found {}", ty.describe()),
            );
        }
        let known = match static_of(e) {
            Some(Static::Int(n)) => Some(if negative { -n } else { n }),
            _ => None,
        };
        (Offset(Some((negative, Box::new(node)))), known)
    }

    fn rowsel(
        &mut self,
        slot: Option<&Expr>,
        table: Option<usize>,
        nrows: Option<usize>,
        sc: &mut Scope,
    ) -> RowPick {
        let Some(e) = slot else {
            return RowPick::all(nrows);
        };
        // Whether cursor offsets here are relative to the cell's own row in
        // its own table, which is what makes "previous row" safe to read.
        let own = sc.own && table == Some(sc.frames[0]);
        let need_row = |ck: &mut Checker, e: &Expr| {
            if !sc.own {
                let text = ck.snippet(e.span).to_string();
                ck.err_help(
                    e.span,
                    format!("`{text}` needs a current row"),
                    "`*` is the current row; an empty slot selects every row",
                );
                false
            } else {
                true
            }
        };

        if let Some(form) = cursor_form(e) {
            if !need_row(self, e) {
                return RowPick::all(nrows);
            }
            let (off, known) = self.offset(form, sc);
            let kind = match known {
                Some(0) if own => DepKind::Same,
                Some(n) if own && n < 0 => DepKind::Back,
                Some(n) if own && n > 0 => DepKind::Fwd,
                _ => DepKind::Whole,
            };
            return RowPick {
                sel: RowSel::Cursor(off),
                one: true,
                len: None,
                kind,
            };
        }

        if let ExprKind::Binary(op @ (BinOp::Range | BinOp::RangeEx), lo, hi) = &e.kind
            && (cursor_form(lo).is_some() || cursor_form(hi).is_some())
        {
            {
                if !need_row(self, e) {
                    return RowPick::all(nrows);
                }
                let exclusive = *op == BinOp::RangeEx;
                let mut bound = |ck: &mut Checker, e: &Expr| match cursor_form(e) {
                    Some(form) => {
                        let (off, known) = ck.offset(form, sc);
                        (Bound::Cursor(off), known)
                    }
                    None => {
                        let (node, ty) = ck.lower(e, sc);
                        if !matches!(ty, Ty::Any | Ty::Scalar(S::Int | S::Any)) {
                            ck.err(
                                e.span,
                                format!("the ends of a range must be Int, found {}", ty.describe()),
                            );
                        }
                        (Bound::Abs(Box::new(node)), None)
                    }
                };
                let (lo_b, lo_off) = bound(self, lo);
                let (hi_b, hi_off) = bound(self, hi);
                let last = hi_off.map(|n| if exclusive { n - 1 } else { n });
                let kind = match (lo_off, last) {
                    (_, Some(n)) if own && n < 0 => DepKind::Back,
                    (Some(n), _) if own && n > 0 => DepKind::Fwd,
                    _ => DepKind::Whole,
                };
                return RowPick {
                    sel: RowSel::Range {
                        lo: lo_b,
                        hi: hi_b,
                        exclusive,
                    },
                    one: false,
                    len: None,
                    kind,
                };
            }
        }

        if has_cursor(e) {
            self.err_help(
                e.span,
                "`*` can only be a row position, an offset from it, or the end of a range",
                "for example `[; *]`, `[; *-1]` or `[; 0..*]`",
            );
            return RowPick::all(nrows);
        }

        // A condition on the candidate row?
        let mut trial_errors = Vec::new();
        if let Some(t) = table {
            let (dmark, pmark) = (self.cur.len(), self.deps.len());
            sc.frames.push(t);
            let (node, ty) = self.lower(e, sc);
            sc.frames.pop();
            if ty == Ty::Scalar(S::Bool) {
                return RowPick {
                    sel: RowSel::Pred(Box::new(node)),
                    one: false,
                    len: None,
                    kind: DepKind::Whole,
                };
            }
            if ty == Ty::Any && self.cur.len() == dmark && self.unknown_params() {
                return RowPick::unknown(e.span);
            }
            trial_errors = self.cur.split_off(dmark);
            self.deps.truncate(pmark);
        }

        // Otherwise a position, a range or a mask, seen from outside.
        let dmark = self.cur.len();
        let (node, ty) = self.lower(e, sc);
        if self.cur.len() > dmark {
            if !trial_errors.is_empty() {
                self.cur.truncate(dmark);
                self.cur.append(&mut trial_errors);
            }
            return RowPick::all(nrows);
        }
        match ty {
            Ty::Scalar(S::Int) => {
                if let (Some(Static::Int(k)), Some(n)) = (static_of(e), nrows) {
                    let at = if k < 0 { k + n as i64 } else { k };
                    if at < 0 || at >= n as i64 {
                        self.err(
                            e.span,
                            format!(
                                "row {k} is out of range: there {} {n} row{}",
                                if n == 1 { "is" } else { "are" },
                                if n == 1 { "" } else { "s" }
                            ),
                        );
                    }
                }
                RowPick {
                    sel: RowSel::Pos(Box::new(node)),
                    one: true,
                    len: None,
                    kind: DepKind::Whole,
                }
            }
            Ty::Range => RowPick {
                sel: RowSel::RangeVal(Box::new(node)),
                one: false,
                len: None,
                kind: DepKind::Whole,
            },
            Ty::Vector(S::Bool, len) => {
                if let (Some(a), Some(b)) = (len, nrows)
                    && a != b
                {
                    self.err(
                        e.span,
                        format!("this mask has {a} values but there are {b} rows"),
                    );
                }
                RowPick {
                    sel: RowSel::Mask(Box::new(node)),
                    one: false,
                    len: None,
                    kind: DepKind::Whole,
                }
            }
            Ty::Any if self.unknown_params() => RowPick::unknown(e.span),
            other => {
                self.err_help(
                    e.span,
                    format!(
                        "a row selector must be a position, a range or a condition, found {}",
                        other.describe()
                    ),
                    "for example `[; 0]`, `[; 0..2]`, `[; *-1]` or `[; Region == \"UK\"]`",
                );
                RowPick::all(nrows)
            }
        }
    }

    /// Whether a function body is being checked for arguments not all known.
    fn unknown_params(&self) -> bool {
        self.params.iter().any(|p| p.1.is_none())
    }

    fn static_in(&self, e: &Expr) -> Option<Static> {
        if let ExprKind::Name(name) = &e.kind {
            if self.params.iter().any(|p| &p.0 == name) {
                return None;
            }
            return self.consts.iter().find(|c| &c.name == name)?.stat;
        }
        static_of(e)
    }

    /// `own` is the table and column of the formula being checked, if it
    /// is in a cell.
    fn colsel(
        &mut self,
        slot: Option<&Expr>,
        tt: &TableTy,
        own: Option<(usize, usize)>,
    ) -> Option<ColSel> {
        let Some(e) = slot else {
            return Some(ColSel::All);
        };
        // `*-4..*-1`: a range with an end counted from the formula's own
        // column.
        if let ExprKind::Binary(op @ (BinOp::Range | BinOp::RangeEx), lo, hi) = &e.kind
            && (cursor_form(lo).is_some() || cursor_form(hi).is_some())
        {
            let here = own
                .filter(|(t, _)| *t == tt.table)
                .and_then(|(_, c)| tt.cols.iter().position(|&x| x == c));
            let Some(here) = here else {
                self.err_help(
                    e.span,
                    "`*` as a column is the column of the formula, in its own table",
                    "name the columns, or count them from the first: `[1..3; *]`",
                );
                return None;
            };
            let n = tt.cols.len() as i64;
            let end = |ck: &mut Checker, e: &Expr| {
                let at = match cursor_form(e) {
                    Some(None) => Some(here as i64),
                    Some(Some((negative, by))) => match ck.static_in(by) {
                        Some(Static::Int(k)) => Some(here as i64 + if negative { -k } else { k }),
                        _ => None,
                    },
                    None => match ck.static_in(e) {
                        Some(Static::Int(k)) => Some(if k < 0 { k + n } else { k }),
                        _ => None,
                    },
                };
                if at.is_none() {
                    ck.err(e.span, "the ends of a column range must be whole numbers");
                }
                at
            };
            let (first, last) = (end(self, lo)?, end(self, hi)?);
            let last = if *op == BinOp::RangeEx {
                last - 1
            } else {
                last
            };
            if first < 0 || last >= n {
                self.err(
                    e.span,
                    format!(
                        "this range runs past the {} column",
                        if first < 0 { "first" } else { "last" }
                    ),
                );
                return None;
            }
            return Some(ColSel::Many(
                (first..=last).map(|k| tt.cols[k as usize]).collect(),
            ));
        }
        // `*`, `*+1`, `*-2`: a column counted from the formula's own.
        if let Some(form) = cursor_form(e) {
            let here = own
                .filter(|(t, _)| *t == tt.table)
                .and_then(|(_, c)| tt.cols.iter().position(|&x| x == c));
            let Some(here) = here else {
                self.err_help(
                    e.span,
                    "`*` as a column is the column of the formula, in its own table",
                    "name the column, for example `[Revenue; *-1]`",
                );
                return None;
            };
            let offset = match form {
                None => Some(0),
                Some((negative, by)) => match self.static_in(by) {
                    Some(Static::Int(k)) => Some(if negative { -k } else { k }),
                    _ => None,
                },
            };
            let Some(offset) = offset else {
                self.err(e.span, "a column offset must be a whole number");
                return None;
            };
            let at = here as i64 + offset;
            if at < 0 || at >= tt.cols.len() as i64 {
                self.err(
                    e.span,
                    format!(
                        "there is no column {} to the {} of this one",
                        offset.abs(),
                        if offset < 0 { "left" } else { "right" }
                    ),
                );
                return None;
            }
            return Some(ColSel::One(tt.cols[at as usize]));
        }
        if let ExprKind::Name(name) = &e.kind
            && let Some(c) = self.col_in(tt, name)
        {
            self.col_refs.push((e.span, tt.table, c));
            return Some(ColSel::One(c));
        }
        let n = tt.cols.len() as i64;
        let at = |k: i64| if k < 0 { k + n } else { k };
        let out_of_range = |ck: &mut Checker, k: i64| {
            ck.err(
                e.span,
                format!(
                    "column {k} is out of range: there {} {n} column{}",
                    if n == 1 { "is" } else { "are" },
                    if n == 1 { "" } else { "s" }
                ),
            );
            None
        };
        match self.static_in(e) {
            Some(Static::Int(k)) => {
                if at(k) < 0 || at(k) >= n {
                    return out_of_range(self, k);
                }
                Some(ColSel::One(tt.cols[at(k) as usize]))
            }
            Some(Static::Range(lo, hi, exclusive)) => {
                let (lo_at, hi_at) = (at(lo), if exclusive { at(hi) - 1 } else { at(hi) });
                if lo_at < 0 || lo_at >= n {
                    return out_of_range(self, lo);
                }
                if hi_at >= n {
                    return out_of_range(self, hi);
                }
                Some(ColSel::Many(
                    (lo_at..=hi_at).map(|k| tt.cols[k as usize]).collect(),
                ))
            }
            None => {
                if let ExprKind::Name(name) = &e.kind {
                    self.no_column(tt, name, e.span);
                } else {
                    self.err_help(
                        e.span,
                        "a column selector must be a column name, a position or a range",
                        "for example `[Revenue]`, `[0]` or `[1..3]`, before any `;`",
                    );
                }
                None
            }
        }
    }

    // ---- ordering ----------------------------------------------------------

    /// Order the calculation and report circular references.
    fn order(&mut self) -> Vec<Group> {
        let nconsts = self.consts.len();
        let mut nodes: Vec<DepNode> = (0..nconsts).map(DepNode::Const).collect();
        for (t, table) in self.tables.iter().enumerate() {
            nodes.extend((0..table.cols.len()).map(|c| DepNode::Col(t, c)));
        }
        let id = |n: DepNode| nodes.iter().position(|x| *x == n).unwrap();
        let edges: Vec<Vec<(usize, DepKind)>> = nodes
            .iter()
            .map(|n| {
                let deps = match *n {
                    DepNode::Const(i) => &self.const_deps[i],
                    DepNode::Col(t, c) => &self.col_deps[t][c],
                };
                let mut out: Vec<(usize, DepKind)> = Vec::new();
                for &(to, kind) in deps {
                    let edge = (id(to), kind);
                    if !out.contains(&edge) {
                        out.push(edge);
                    }
                }
                out
            })
            .collect();

        let mut groups = Vec::new();
        for scc in deps::analyse(&edges) {
            if let Some(cycle) = &scc.cycle {
                let one_table = cycle.iter().all(|&n| match (nodes[n], nodes[cycle[0]]) {
                    (DepNode::Col(a, _), DepNode::Col(b, _)) => a == b,
                    _ => false,
                });
                let label = |n: usize| match nodes[n] {
                    DepNode::Const(i) => self.consts[i].name.clone(),
                    DepNode::Col(t, c) if one_table => self.tables[t].cols[c].name.clone(),
                    DepNode::Col(t, c) => {
                        format!("{}.{}", self.tables[t].name, self.tables[t].cols[c].name)
                    }
                };
                let mut path: Vec<String> = cycle.iter().map(|&n| label(n)).collect();
                path.push(label(cycle[0]));
                let span = match nodes[cycle[0]] {
                    DepNode::Const(i) => self.consts[i].span,
                    DepNode::Col(t, c) => self.tables[t].cols[c].span,
                };
                self.done.push(
                    Diagnostic::new(span, format!("circular reference: {}", path.join(" → ")))
                        .with_help("a column may read its own earlier rows, such as `[; *-1]`, but not itself in the same row"),
                );
            }
            match nodes[scc.nodes[0]] {
                DepNode::Const(i) => groups.push(Group::Const(i)),
                DepNode::Col(..) => groups.push(Group::Cols {
                    cols: scc
                        .nodes
                        .iter()
                        .filter_map(|&n| match nodes[n] {
                            DepNode::Col(t, c) => Some((t, c)),
                            DepNode::Const(_) => None,
                        })
                        .collect(),
                    reverse: scc.reverse,
                }),
            }
        }
        groups
    }

    fn finish(mut self, order: Vec<Group>) -> Program {
        let mut tables = Vec::new();
        for (t, info) in mem::take(&mut self.tables).into_iter().enumerate() {
            let mut cols = Vec::new();
            for (c, col) in info.cols.into_iter().enumerate() {
                let ty = match &self.col_state[t][c] {
                    St::Done(s) | St::Busy(s) => *s,
                    St::Pending => S::Any,
                };
                cols.push(Column {
                    name: col.name,
                    span: col.span,
                    ty,
                    declared: col.declared.is_some(),
                    kind: self.col_out[t][c]
                        .take()
                        .unwrap_or(ColKind::Computed(Node::error(col.span))),
                });
            }
            tables.push(Table {
                name: info.name,
                cols,
                nrows: info.nrows,
            });
        }
        let mut consts = Vec::new();
        for (i, info) in mem::take(&mut self.consts).into_iter().enumerate() {
            let ty = match &self.const_state[i] {
                St::Done(t) | St::Busy(t) => t.clone(),
                St::Pending => Ty::Any,
            };
            consts.push(Const {
                name: info.name,
                node: self.const_out[i].take().unwrap_or(Node::error(info.span)),
                ty,
                stat: info.stat,
            });
        }
        Program {
            zone: self.ast.and_then(|ast| ast.zone.clone()),
            tables,
            consts,
            funcs: mem::take(&mut self.funcs),
            order,
            col_refs: mem::take(&mut self.col_refs),
        }
    }
}

fn unop_text(op: UnOp) -> &'static str {
    match op {
        UnOp::Neg => "-",
        UnOp::Not => "not",
    }
}

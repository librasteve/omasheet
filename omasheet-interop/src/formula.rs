// Copyright (c) 2026 Stephen Roe

//! A workbook's formulas as OMX.
//!
//! A worksheet becomes a sheet whose first row is the header, so a cell
//! reference becomes a column and a row of that sheet: `B3` is `Revenue` on
//! its own row, `[Revenue; *-1]` on the row below, and `[Revenue; 1]` when it
//! is written `B$3`. A formula that OMX has no way to say is not translated,
//! and the reason is given.

/// A worksheet as the sheet it becomes.
#[derive(Debug)]
pub struct Worksheet {
    /// The worksheet's name in the workbook.
    pub excel: String,
    /// The sheet's name in the file.
    pub sheet: String,
    pub columns: Vec<String>,
    /// The row and the column of the header's first cell.
    pub top: u32,
    pub left: u32,
    /// How many rows there are under the header.
    pub rows: usize,
}

/// A formula in OMX.
#[derive(Debug, PartialEq)]
pub struct Translated {
    pub text: String,
    /// The formula as one of a column filled down with it: every cell not
    /// held with a `$` is so many rows from the formula's own, so that each
    /// row of the column has the same text.
    pub filled: String,
    /// It gives something else each time it is worked out: `today()`.
    pub volatile: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Float(String),
    Str(String),
    /// A word: a function, a name, `TRUE`, or the letters of a column.
    Word(String),
    Cell(Cell),
    /// A worksheet's name, before the `!`.
    Worksheet(String),
    Op(&'static str),
    Open,
    Close,
    Comma,
    Colon,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cell {
    col: u32,
    row: u32,
    /// Written with a `$`: it stays where it is when the formula is copied.
    fixed: bool,
}

#[derive(Debug)]
enum Expr {
    Num(String),
    Str(String),
    Bool(bool),
    Cell(Option<String>, Cell),
    /// A block of cells, from one corner to the other.
    Block(Option<String>, Cell, Cell),
    /// Whole columns: `B:B`.
    Columns(Option<String>, u32, u32),
    Neg(Box<Expr>),
    Percent(Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

type R<T> = Result<T, String>;

const UNREADABLE: &str = "the formula could not be read";

fn letters(word: &str) -> Option<u32> {
    if word.is_empty() || word.len() > 3 || !word.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let n = word.bytes().fold(0u32, |n, b| {
        n * 26 + (b.to_ascii_uppercase() - b'A') as u32 + 1
    });
    Some(n - 1)
}

/// `B3`, `$B$3`: a cell, if `text` starts with one, and how long it is.
fn cell(text: &str) -> Option<(Cell, usize)> {
    let bytes = text.as_bytes();
    let mut i = usize::from(bytes.first() == Some(&b'$'));
    let start = i;
    while bytes.get(i).is_some_and(u8::is_ascii_alphabetic) {
        i += 1;
    }
    let col = letters(&text[start..i])?;
    let fixed = bytes.get(i) == Some(&b'$');
    i += usize::from(fixed);
    let digits = i;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let row: u32 = text[digits..i].parse().ok().filter(|n| *n > 0)?;
    // `LOG10(` is a function and `A1B` a name.
    let next = bytes.get(i).copied().unwrap_or(b' ');
    if next.is_ascii_alphanumeric() || matches!(next, b'_' | b'.' | b'(' | b'!') {
        return None;
    }
    Some((
        Cell {
            col,
            row: row - 1,
            fixed,
        },
        i,
    ))
}

fn lex(text: &str) -> R<Vec<Tok>> {
    let bytes = text.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let rest = &text[i..];
        if b.is_ascii_whitespace() {
            i += 1;
        } else if b == b'"' {
            // A quote in text is written twice.
            let mut out = String::new();
            let mut chars = rest[1..].char_indices().peekable();
            let mut end = None;
            while let Some((at, c)) = chars.next() {
                if c != '"' {
                    out.push(c);
                } else if chars.peek().is_some_and(|(_, c)| *c == '"') {
                    out.push('"');
                    chars.next();
                } else {
                    end = Some(at);
                    break;
                }
            }
            i += end.ok_or(UNREADABLE)? + 2;
            toks.push(Tok::Str(out));
        } else if b == b'\'' {
            let mut name = String::new();
            let mut at = 1;
            loop {
                let c = rest[at..].chars().next().ok_or(UNREADABLE)?;
                at += c.len_utf8();
                if c != '\'' {
                    name.push(c);
                } else if rest[at..].starts_with('\'') {
                    name.push('\'');
                    at += 1;
                } else {
                    break;
                }
            }
            if !rest[at..].starts_with('!') {
                return Err(UNREADABLE.into());
            }
            i += at + 1;
            toks.push(Tok::Worksheet(name));
        } else if b.is_ascii_digit()
            || (b == b'.' && rest[1..].starts_with(|c: char| c.is_ascii_digit()))
        {
            let mut j = i;
            while bytes
                .get(j)
                .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
            {
                j += 1;
            }
            if matches!(bytes.get(j), Some(b'E' | b'e')) {
                let mut k = j + 1;
                k += usize::from(matches!(bytes.get(k), Some(b'+' | b'-')));
                if bytes.get(k).is_some_and(u8::is_ascii_digit) {
                    while bytes.get(k).is_some_and(u8::is_ascii_digit) {
                        k += 1;
                    }
                    j = k;
                }
            }
            toks.push(Tok::Float(text[i..j].to_string()));
            i = j;
        } else if let Some((cell, len)) = cell(rest) {
            toks.push(Tok::Cell(cell));
            i += len;
        } else if b.is_ascii_alphabetic() || matches!(b, b'_' | b'$' | b'\\') {
            let mut j = i;
            while bytes.get(j).is_some_and(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'$' | b'\\')
            }) {
                j += 1;
            }
            let word = text[i..j].to_string();
            if bytes.get(j) == Some(&b'!') {
                toks.push(Tok::Worksheet(word));
                j += 1;
            } else {
                toks.push(Tok::Word(word));
            }
            i = j;
        } else {
            let two = rest.get(..2).unwrap_or("");
            let (tok, len) = match (two, b) {
                ("<>", _) => (Tok::Op("!="), 2),
                ("<=", _) => (Tok::Op("<="), 2),
                (">=", _) => (Tok::Op(">="), 2),
                (_, b'<') => (Tok::Op("<"), 1),
                (_, b'>') => (Tok::Op(">"), 1),
                (_, b'=') => (Tok::Op("=="), 1),
                (_, b'+') => (Tok::Op("+"), 1),
                (_, b'-') => (Tok::Op("-"), 1),
                (_, b'*') => (Tok::Op("*"), 1),
                (_, b'/') => (Tok::Op("/"), 1),
                (_, b'^') => (Tok::Op("**"), 1),
                (_, b'&') => (Tok::Op("&"), 1),
                (_, b'%') => (Tok::Op("%"), 1),
                (_, b'(') => (Tok::Open, 1),
                (_, b')') => (Tok::Close, 1),
                (_, b',') => (Tok::Comma, 1),
                (_, b':') => (Tok::Colon, 1),
                (_, b'{') => return Err("it holds a list of values in `{ }`".into()),
                (_, b'[') => return Err("it refers to a sheet of the workbook by name".into()),
                (_, b'#') => return Err("it holds an error value".into()),
                _ => return Err(UNREADABLE.into()),
            };
            toks.push(tok);
            i += len;
        }
    }
    Ok(toks)
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn next(&mut self) -> Option<Tok> {
        let tok = self.toks.get(self.at).cloned();
        self.at += 1;
        tok
    }

    fn op(&mut self, among: &[&str]) -> Option<&'static str> {
        match self.peek() {
            Some(Tok::Op(op)) if among.contains(op) => {
                let op = *op;
                self.at += 1;
                Some(op)
            }
            _ => None,
        }
    }

    /// Operators that take the operands to their left first, loosest first.
    fn binary(&mut self, level: usize) -> R<Expr> {
        const LEVELS: [&[&str]; 5] = [
            &["==", "!=", "<", ">", "<=", ">="],
            &["&"],
            &["+", "-"],
            &["*", "/"],
            &["**"],
        ];
        let Some(ops) = LEVELS.get(level) else {
            return self.percent();
        };
        let mut left = self.binary(level + 1)?;
        while let Some(op) = self.op(ops) {
            let right = self.binary(level + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn percent(&mut self) -> R<Expr> {
        let mut e = self.unary()?;
        while self.op(&["%"]).is_some() {
            e = Expr::Percent(Box::new(e));
        }
        Ok(e)
    }

    fn unary(&mut self) -> R<Expr> {
        match self.op(&["-", "+"]) {
            Some("-") => Ok(Expr::Neg(Box::new(self.unary()?))),
            Some(_) => self.unary(),
            None => self.primary(),
        }
    }

    fn primary(&mut self) -> R<Expr> {
        let worksheet = match self.peek() {
            Some(Tok::Worksheet(name)) => {
                let name = name.clone();
                self.at += 1;
                Some(name)
            }
            _ => None,
        };
        let tok = self.next().ok_or(UNREADABLE)?;
        if worksheet.is_some() && !matches!(tok, Tok::Cell(_) | Tok::Word(_)) {
            return Err(UNREADABLE.into());
        }
        match tok {
            Tok::Float(n) => Ok(Expr::Num(n)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Open => {
                let e = self.binary(0)?;
                match self.next() {
                    Some(Tok::Close) => Ok(e),
                    _ => Err(UNREADABLE.into()),
                }
            }
            Tok::Cell(from) => {
                if self.peek() != Some(&Tok::Colon) {
                    return Ok(Expr::Cell(worksheet, from));
                }
                self.at += 1;
                // The far corner may name its worksheet again.
                if matches!(self.peek(), Some(Tok::Worksheet(_))) {
                    self.at += 1;
                }
                match self.next() {
                    Some(Tok::Cell(to)) => Ok(Expr::Block(worksheet, from, to)),
                    _ => Err(UNREADABLE.into()),
                }
            }
            Tok::Word(word) => {
                if self.peek() == Some(&Tok::Open) && worksheet.is_none() {
                    self.at += 1;
                    let mut args = Vec::new();
                    if self.peek() == Some(&Tok::Close) {
                        self.at += 1;
                        return Ok(Expr::Call(word, args));
                    }
                    loop {
                        if matches!(self.peek(), Some(Tok::Comma | Tok::Close)) {
                            return Err("an argument is left out".into());
                        }
                        args.push(self.binary(0)?);
                        match self.next() {
                            Some(Tok::Comma) => {}
                            Some(Tok::Close) => return Ok(Expr::Call(word, args)),
                            _ => return Err(UNREADABLE.into()),
                        }
                    }
                }
                let column = |word: &str| letters(&word.replace('$', ""));
                if self.peek() == Some(&Tok::Colon)
                    && let Some(from) = column(&word)
                {
                    self.at += 1;
                    return match self.next() {
                        Some(Tok::Word(to)) => match column(&to) {
                            Some(to) => Ok(Expr::Columns(worksheet, from.min(to), from.max(to))),
                            None => Err(UNREADABLE.into()),
                        },
                        _ => Err(UNREADABLE.into()),
                    };
                }
                match word.to_ascii_uppercase().as_str() {
                    "TRUE" if worksheet.is_none() => Ok(Expr::Bool(true)),
                    "FALSE" if worksheet.is_none() => Ok(Expr::Bool(false)),
                    _ => Err(format!("it uses the name `{word}`")),
                }
            }
            Tok::Colon | Tok::Comma | Tok::Close | Tok::Op(_) | Tok::Worksheet(_) => {
                Err(UNREADABLE.into())
            }
        }
    }
}

fn parse(formula: &str) -> R<Expr> {
    let text = formula.trim().trim_start_matches('=');
    let mut parser = Parser {
        toks: lex(text)?,
        at: 0,
    };
    let e = parser.binary(0)?;
    if parser.at != parser.toks.len() {
        return Err(UNREADABLE.into());
    }
    Ok(e)
}

/// How tightly OMX holds an expression together; one inside a tighter one
/// is put in brackets.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Tight {
    If,
    Or,
    And,
    Not,
    Compare,
    Add,
    Multiply,
    Negate,
    Power,
    Whole,
}

/// A piece of OMX.
struct Omx {
    text: String,
    tight: Tight,
}

impl Omx {
    fn whole(text: impl Into<String>) -> Omx {
        Omx {
            text: text.into(),
            tight: Tight::Whole,
        }
    }

    /// The text, in brackets if it is looser than `tight`.
    fn at(&self, tight: Tight) -> String {
        if self.tight < tight {
            format!("({})", self.text)
        } else {
            self.text.clone()
        }
    }
}

/// What an expression gives: one value, or columns of them.
enum Out {
    One(Omx),
    Many(Vec<String>),
}

const RANGE: &str = "it uses a range of cells where one value is wanted";
const OUTSIDE: &str = "it refers to a cell outside the sheet";
const HEADER: &str = "it refers to the header row";

struct Scope<'a> {
    worksheets: &'a [Worksheet],
    here: usize,
    /// The row of the formula's cell among the sheet's rows.
    row: i64,
    /// Write it as one of a column filled down: see [`Translated::filled`].
    filled: bool,
    volatile: bool,
}

impl Scope<'_> {
    fn worksheet(&self, name: &Option<String>) -> R<usize> {
        let Some(name) = name else {
            return Ok(self.here);
        };
        self.worksheets
            .iter()
            .position(|s| s.excel.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("it refers to `{name}`, which was not imported"))
    }

    fn column(&self, s: usize, col: u32) -> R<&str> {
        let worksheet = &self.worksheets[s];
        col.checked_sub(worksheet.left)
            .and_then(|c| worksheet.columns.get(c as usize))
            .map(String::as_str)
            .ok_or_else(|| OUTSIDE.to_string())
    }

    /// The row a cell is on among its sheet's rows; `-1` is the header.
    fn index(&self, s: usize, cell: Cell) -> R<i64> {
        let k = cell.row as i64 - self.worksheets[s].top as i64 - 1;
        if k < -1 { Err(OUTSIDE.into()) } else { Ok(k) }
    }

    /// A row as a formula on this row writes it: a number when it is
    /// fixed, and otherwise so many rows from this one.
    ///
    /// A row of another sheet is a number too, unless the formula is one
    /// of a column filled down: one cell there is not meant as so many rows
    /// from one here.
    fn position(&self, s: usize, k: i64, fixed: bool) -> String {
        match k - self.row {
            _ if fixed || (s != self.here && !self.filled) => k.to_string(),
            0 => "*".into(),
            d if d > 0 => format!("*+{d}"),
            d => format!("*-{}", -d),
        }
    }

    fn select(&self, s: usize, column: &str, rows: &str) -> String {
        if s == self.here {
            format!("[{column}; {rows}]")
        } else {
            format!("{}[{column}; {rows}]", self.worksheets[s].sheet)
        }
    }

    fn cell(&self, worksheet: &Option<String>, cell: Cell) -> R<Omx> {
        let s = self.worksheet(worksheet)?;
        let column = self.column(s, cell.col)?;
        let k = self.index(s, cell)?;
        if k < 0 {
            return Err(HEADER.into());
        }
        if k >= self.worksheets[s].rows as i64 {
            return Err(OUTSIDE.into());
        }
        if s == self.here && k == self.row && !cell.fixed {
            return Ok(Omx::whole(column));
        }
        Ok(Omx::whole(self.select(
            s,
            column,
            &self.position(s, k, cell.fixed),
        )))
    }

    /// The cells of some columns between two rows, a vector for each
    /// column. A header and empty rows below the sheet are no part of it.
    fn block(&self, worksheet: &Option<String>, a: Cell, b: Cell) -> R<Vec<String>> {
        let s = self.worksheet(worksheet)?;
        let rows = self.worksheets[s].rows as i64;
        let (first, last) = if a.row <= b.row { (a, b) } else { (b, a) };
        let (mut lo, mut hi) = (self.index(s, first)?, self.index(s, last)?);
        let (mut lo_fixed, mut hi_fixed) = (first.fixed, last.fixed);
        if lo < 0 {
            (lo, lo_fixed) = (0, true);
        }
        if hi >= rows {
            (hi, hi_fixed) = (rows - 1, true);
        }
        if hi < lo {
            return Err(OUTSIDE.into());
        }
        // Every row of the column: but not when it only happens to be, for
        // this one of a column filled down.
        let all = lo == 0 && hi == rows - 1 && (!self.filled || (lo_fixed && hi_fixed));
        (a.col.min(b.col)..=a.col.max(b.col))
            .map(|col| {
                let column = self.column(s, col)?;
                Ok(if all {
                    format!("{}.{column}", self.worksheets[s].sheet)
                } else {
                    let rows = format!(
                        "{}..{}",
                        self.position(s, lo, lo_fixed),
                        self.position(s, hi, hi_fixed)
                    );
                    self.select(s, column, &rows)
                })
            })
            .collect()
    }

    /// `weekday(x)` for a `WEEKDAY(x)` that counts from Sunday, as a
    /// workbook does unless told otherwise.
    fn sunday_weekday(&mut self, e: &Expr) -> R<Option<String>> {
        let Expr::Call(name, args) = e else {
            return Ok(None);
        };
        let sunday = match args.as_slice() {
            [_] => true,
            [_, Expr::Num(n)] => n == "1",
            _ => false,
        };
        if !name.eq_ignore_ascii_case("WEEKDAY") || !sunday {
            return Ok(None);
        }
        Ok(Some(format!(
            "weekday({})",
            self.one(&args[0])?.at(Tight::Or)
        )))
    }

    fn one(&mut self, e: &Expr) -> R<Omx> {
        match self.expr(e)? {
            Out::One(omx) => Ok(omx),
            Out::Many(_) => Err(RANGE.into()),
        }
    }

    fn expr(&mut self, e: &Expr) -> R<Out> {
        Ok(Out::One(match e {
            Expr::Num(n) => Omx::whole(number(n)),
            Expr::Str(s) => Omx::whole(format!("{s:?}")),
            Expr::Bool(b) => Omx::whole(b.to_string()),
            Expr::Cell(worksheet, cell) => self.cell(worksheet, *cell)?,
            Expr::Block(worksheet, a, b) => return Ok(Out::Many(self.block(worksheet, *a, *b)?)),
            Expr::Columns(worksheet, from, to) => {
                let s = self.worksheet(worksheet)?;
                let columns: R<Vec<String>> = (*from..=*to)
                    .map(|col| {
                        Ok(format!(
                            "{}.{}",
                            self.worksheets[s].sheet,
                            self.column(s, col)?
                        ))
                    })
                    .collect();
                return Ok(Out::Many(columns?));
            }
            Expr::Neg(inner) => Omx {
                text: format!("-{}", self.one(inner)?.at(Tight::Power)),
                tight: Tight::Negate,
            },
            Expr::Percent(inner) => match &**inner {
                Expr::Num(n) => Omx::whole(format!("{}%", number(n))),
                other => Omx {
                    text: format!("{} / 100", self.one(other)?.at(Tight::Multiply)),
                    tight: Tight::Multiply,
                },
            },
            Expr::Binary("&", ..) => return Err("it joins text with `&`".into()),
            Expr::Binary(op @ ("==" | "!="), a, b)
                if let (Some(day), Some(n)) = (self.sunday_weekday(a)?, sunday_day(b)) =>
            {
                Omx {
                    text: format!("{day} {op} {n}"),
                    tight: Tight::Compare,
                }
            }
            Expr::Binary(op, a, b) => {
                let (a, b) = (self.one(a)?, self.one(b)?);
                // What is on the right is bracketed unless it holds tighter:
                // a workbook works from the left, and `**` in OMX from the
                // right.
                let (tight, left, right) = match *op {
                    "+" | "-" => (Tight::Add, Tight::Add, Tight::Multiply),
                    "*" | "/" => (Tight::Multiply, Tight::Multiply, Tight::Power),
                    "**" => (Tight::Power, Tight::Whole, Tight::Whole),
                    _ => (Tight::Compare, Tight::Add, Tight::Add),
                };
                Omx {
                    text: format!("{} {op} {}", a.at(left), b.at(right)),
                    tight,
                }
            }
            Expr::Call(name, args) => return self.call(name, args),
        }))
    }

    /// Each argument of a function that takes any number of values and
    /// ranges: `one` for a single value and `many` for a vector.
    fn spread(
        &mut self,
        args: &[Expr],
        one: impl Fn(&Omx) -> Option<String>,
        many: impl Fn(&str) -> String,
    ) -> R<Vec<String>> {
        let mut parts = Vec::new();
        for arg in args {
            match self.expr(arg)? {
                Out::One(omx) => parts.push(one(&omx).ok_or(RANGE)?),
                Out::Many(columns) => parts.extend(columns.iter().map(|c| many(c))),
            }
        }
        Ok(parts)
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> R<Out> {
        let upper = name.to_ascii_uppercase();
        let upper = upper.strip_prefix("_XLFN.").unwrap_or(&upper);
        let wrong = || format!("`{upper}` is not written as OMX can take it");
        let none = || format!("`{upper}` has no equivalent");
        let joined = |parts: Vec<String>, with: &str, tight: Tight| match parts.len() {
            0 => Err(wrong()),
            1 => Ok(Omx {
                text: parts[0].clone(),
                tight: Tight::Add,
            }),
            _ => Ok(Omx {
                text: parts.join(with),
                tight,
            }),
        };
        let omx = match (upper, args.len()) {
            ("SUM", _) => {
                let parts = self.spread(
                    args,
                    |v| Some(v.at(Tight::Multiply)),
                    |c| format!("{c}.sum()"),
                )?;
                joined(parts, " + ", Tight::Add)?
            }
            ("COUNT" | "COUNTA", _) => {
                let parts = self.spread(args, |_| None, |c| format!("{c}.count()"))?;
                joined(parts, " + ", Tight::Add)?
            }
            ("MIN" | "MAX" | "AVERAGE", _) => {
                let method = match upper {
                    "MIN" => "min",
                    "MAX" => "max",
                    _ => "avg",
                };
                let mut columns = 0;
                let mut parts = Vec::new();
                for arg in args {
                    match self.expr(arg)? {
                        Out::One(omx) => parts.push(omx.at(Tight::Or)),
                        Out::Many(many) => {
                            columns += many.len();
                            parts.extend(many.iter().map(|c| format!("{c}.{method}()")));
                        }
                    }
                }
                match (parts.len(), columns) {
                    (0, _) => return Err(wrong()),
                    (1, 1) => Omx::whole(parts.remove(0)),
                    // An average of averages is not the average of them all.
                    (_, c) if c > 0 && method == "avg" => return Err(wrong()),
                    _ => Omx::whole(format!("[{}].{method}()", parts.join(", "))),
                }
            }
            (
                "ABS" | "SQRT" | "SIGN" | "LN" | "EXP" | "LOG10" | "SIN" | "COS" | "TAN" | "ASIN"
                | "ACOS" | "ATAN" | "RADIANS" | "DEGREES" | "YEAR" | "MONTH" | "DAY" | "HOUR"
                | "MINUTE" | "SECOND",
                1,
            ) => Omx::whole(format!(
                "{}({})",
                upper.to_ascii_lowercase(),
                self.one(&args[0])?.at(Tight::Or)
            )),
            ("INT", 1) => Omx::whole(format!("floor({})", self.one(&args[0])?.at(Tight::Or))),
            ("TRUNC", 1) => Omx::whole(format!("{}.Int", self.one(&args[0])?.at(Tight::Whole))),
            ("DATEVALUE", 1) => {
                Omx::whole(format!("{}.Date", self.one(&args[0])?.at(Tight::Whole)))
            }
            // To so many digits is to the nearest power of ten: `ROUND(x,2)`
            // is `round(x, 0.01)`, and `ROUND(x,-2)` is `round(x, 100)`.
            ("ROUND", 1 | 2) => {
                let x = self.one(&args[0])?;
                let digits = |n: &str| n.parse::<i32>().ok().filter(|d| d.abs() <= 30);
                let digits = match args.get(1) {
                    None => 0,
                    Some(Expr::Num(n)) => digits(n).ok_or_else(wrong)?,
                    Some(Expr::Neg(inner)) => match &**inner {
                        Expr::Num(n) => -digits(n).ok_or_else(wrong)?,
                        _ => return Err(wrong()),
                    },
                    Some(_) => return Err(wrong()),
                };
                let x = x.at(Tight::Or);
                match digits {
                    0 => Omx::whole(format!("round({x})")),
                    d if d > 0 => {
                        Omx::whole(format!("round({x}, 0.{}1)", "0".repeat(d as usize - 1)))
                    }
                    d => Omx::whole(format!("round({x}, 1{})", "0".repeat(-d as usize))),
                }
            }
            ("POWER", 2) => Omx {
                text: format!(
                    "{} ** {}",
                    self.one(&args[0])?.at(Tight::Whole),
                    self.one(&args[1])?.at(Tight::Whole)
                ),
                tight: Tight::Power,
            },
            ("PI", 0) => Omx::whole("pi()"),
            ("TODAY" | "NOW", 0) => {
                self.volatile = true;
                Omx::whole(format!("{}()", upper.to_ascii_lowercase()))
            }
            // A workbook counts the days of the week from Sunday, and OMX
            // from Monday.
            ("WEEKDAY", 1 | 2) => {
                let day = format!("weekday({})", self.one(&args[0])?.at(Tight::Or));
                match args.get(1) {
                    None => from_sunday(&day),
                    Some(Expr::Num(n)) if n == "1" => from_sunday(&day),
                    Some(Expr::Num(n)) if n == "2" => Omx::whole(day),
                    Some(Expr::Num(n)) if n == "3" => Omx {
                        text: format!("{day} - 1"),
                        tight: Tight::Add,
                    },
                    Some(_) => return Err(wrong()),
                }
            }
            ("IF", 3) => {
                let test = self.one(&args[0])?.at(Tight::Or);
                let then = self.one(&args[1])?.at(Tight::Or);
                let other = self.one(&args[2])?.at(Tight::If);
                Omx {
                    text: format!("if {test} then {then} else {other}"),
                    tight: Tight::If,
                }
            }
            ("AND" | "OR", _) => {
                let (word, tight, inner) = if upper == "AND" {
                    (" and ", Tight::And, Tight::Not)
                } else {
                    (" or ", Tight::Or, Tight::And)
                };
                let mut parts = Vec::new();
                for arg in args {
                    parts.push(self.one(arg)?.at(inner));
                }
                match parts.len() {
                    0 => return Err(wrong()),
                    1 => Omx {
                        text: parts.remove(0),
                        tight: inner,
                    },
                    _ => Omx {
                        text: parts.join(word),
                        tight,
                    },
                }
            }
            ("NOT", 1) => Omx {
                text: format!("not {}", self.one(&args[0])?.at(Tight::Compare)),
                tight: Tight::Not,
            },
            // The cases in turn. With no default, the last case stands for
            // whatever is left.
            ("SWITCH", n) if n >= 3 => {
                // On days of the week, the cases are given OMX's numbers.
                let values = || args[1..].as_chunks::<2>().0.iter().map(|pair| &pair[0]);
                let days: Option<Vec<String>> = values().map(sunday_day).collect();
                let weekday = match (self.sunday_weekday(&args[0])?, days) {
                    (Some(day), Some(days)) => Some((day, days)),
                    _ => None,
                };
                let subject = match &weekday {
                    Some((day, _)) => day.clone(),
                    None => self.one(&args[0])?.at(Tight::Add),
                };
                let mut cases = Vec::new();
                for (i, pair) in args[1..].chunks(2).enumerate() {
                    match pair {
                        [value, result] => cases.push((
                            Some(match &weekday {
                                Some((_, days)) => format!("{subject} == {}", days[i]),
                                None => {
                                    format!("{subject} == {}", self.one(value)?.at(Tight::Add))
                                }
                            }),
                            self.one(result)?.at(Tight::Or),
                        )),
                        [default] => cases.push((None, self.one(default)?.at(Tight::Or))),
                        _ => unreachable!(),
                    }
                }
                chain(cases)
            }
            ("IFS", n) if n >= 2 && n % 2 == 0 => {
                let mut cases = Vec::new();
                for pair in args.chunks(2) {
                    let test = match &pair[0] {
                        Expr::Bool(true) => None,
                        other => Some(self.one(other)?.at(Tight::Or)),
                    };
                    cases.push((test, self.one(&pair[1])?.at(Tight::Or)));
                }
                chain(cases)
            }
            (
                "ABS" | "SQRT" | "INT" | "ROUND" | "IF" | "NOT" | "SWITCH" | "IFS" | "POWER" | "PI"
                | "TODAY" | "NOW" | "WEEKDAY" | "TRUNC" | "DATEVALUE" | "YEAR" | "MONTH" | "DAY",
                _,
            ) => return Err(wrong()),
            _ => return Err(none()),
        };
        Ok(Out::One(omx))
    }
}

/// `if a then x else if b then y else z`. A test of `None` is the default,
/// and so is the last case when there is no other.
fn chain(mut cases: Vec<(Option<String>, String)>) -> Omx {
    let (_, last) = cases.pop().unwrap_or_default();
    let mut text = String::new();
    for (test, result) in cases {
        match test {
            Some(test) => text.push_str(&format!("if {test} then {result} else ")),
            // A default before the end: what follows it is never reached.
            None => {
                text.push_str(&result);
                return Omx {
                    text,
                    tight: Tight::If,
                };
            }
        }
    }
    text.push_str(&last);
    Omx {
        text,
        tight: Tight::If,
    }
}

/// A workbook's number for a day of the week, 1 for Sunday, as OMX's, 1
/// for Monday.
fn sunday_day(e: &Expr) -> Option<String> {
    match e {
        Expr::Num(n) => match n.parse::<u32>() {
            Ok(n @ 1..=7) => Some(((n + 5) % 7 + 1).to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// A day of the week counted from Sunday as 1, from OMX's Monday as 1.
fn from_sunday(day: &str) -> Omx {
    Omx {
        text: format!("if {day} == 7 then 1 else {day} + 1"),
        tight: Tight::If,
    }
}

/// A number as OMX writes it: `.5` is `0.5`, and one with an exponent is
/// written out where it is whole.
fn number(n: &str) -> String {
    let n = n.to_ascii_lowercase();
    if n.contains('e') {
        return match n.parse::<f64>() {
            Ok(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{f}"),
            _ => n,
        };
    }
    let n = n.strip_suffix('.').unwrap_or(&n).to_string();
    if n.starts_with('.') {
        format!("0{n}")
    } else {
        n
    }
}

/// The formula of the cell on row `row` of worksheet `here`, in OMX. `row`
/// counts from the top of the worksheet, as `top` does. The error says why
/// the formula has no translation.
pub fn translate(formula: &str, worksheets: &[Worksheet], here: usize, row: u32) -> R<Translated> {
    let e = parse(formula)?;
    let mut scope = Scope {
        worksheets,
        here,
        row: row as i64 - worksheets[here].top as i64 - 1,
        filled: false,
        volatile: false,
    };
    let text = scope.one(&e)?.text;
    scope.filled = true;
    let filled = scope.one(&e)?.text;
    Ok(Translated {
        text,
        filled,
        volatile: scope.volatile,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worksheets() -> Vec<Worksheet> {
        let worksheet = |excel: &str, sheet: &str, columns: &[&str], rows| Worksheet {
            excel: excel.into(),
            sheet: sheet.into(),
            columns: columns.iter().map(|c| c.to_string()).collect(),
            top: 0,
            left: 0,
            rows,
        };
        vec![
            worksheet("Sales", "Sales", &["Month", "Revenue", "Cost", "Profit"], 3),
            worksheet("My Rates", "MyRates", &["Name", "Rate"], 2),
        ]
    }

    /// A formula in row 3 of `Sales`, its second row.
    fn omx(formula: &str) -> String {
        match translate(formula, &worksheets(), 0, 2) {
            Ok(t) => t.text,
            Err(why) => format!("no: {why}"),
        }
    }

    fn filled(formula: &str) -> String {
        translate(formula, &worksheets(), 0, 2).unwrap().filled
    }

    #[test]
    fn cells_are_columns_and_rows() {
        assert_eq!(omx("B3-C3"), "Revenue - Cost");
        assert_eq!(omx("=B3-C3"), "Revenue - Cost");
        assert_eq!(omx("B2"), "[Revenue; *-1]");
        assert_eq!(omx("B4+1"), "[Revenue; *+1] + 1");
        assert_eq!(omx("B$2"), "[Revenue; 0]");
        assert_eq!(omx("$B$3"), "[Revenue; 1]");
        assert_eq!(omx("$B3"), "Revenue");
        assert_eq!(omx("'My Rates'!B$2*B3"), "MyRates[Rate; 0] * Revenue");
        assert_eq!(omx("'My Rates'!B3"), "MyRates[Rate; 1]");
        assert_eq!(filled("'My Rates'!B3"), "MyRates[Rate; *]");
        assert_eq!(filled("B2+B$2"), "[Revenue; *-1] + [Revenue; 0]");
        assert_eq!(omx("B1"), format!("no: {HEADER}"));
        assert_eq!(omx("F3"), format!("no: {OUTSIDE}"));
        assert_eq!(omx("B9"), format!("no: {OUTSIDE}"));
        assert_eq!(
            omx("Other!A1"),
            "no: it refers to `Other`, which was not imported"
        );
    }

    #[test]
    fn ranges_are_vectors() {
        assert_eq!(omx("SUM(B2:B4)"), "Sales.Revenue.sum()");
        assert_eq!(omx("SUM(B:B)"), "Sales.Revenue.sum()");
        assert_eq!(omx("SUM(B1:B99)"), "Sales.Revenue.sum()");
        assert_eq!(omx("SUM(B$2:B3)"), "[Revenue; 0..*].sum()");
        assert_eq!(omx("SUM(B2:B3)"), "[Revenue; *-1..*].sum()");
        assert_eq!(omx("SUM('My Rates'!B2:B2)"), "MyRates[Rate; 0..0].sum()");
        // In a column filled down, the rows a range happens to take in.
        assert_eq!(filled("SUM(B$2:B4)"), "[Revenue; 0..*+1].sum()");
        assert_eq!(filled("SUM(B$2:B$4)"), "Sales.Revenue.sum()");
        assert_eq!(
            omx("SUM(B2:C4, 5)"),
            "Sales.Revenue.sum() + Sales.Cost.sum() + 5"
        );
        assert_eq!(omx("AVERAGE('My Rates'!B2:B3)"), "MyRates.Rate.avg()");
        assert_eq!(
            omx("MAX(B2:B4, C2:C4, 0)"),
            "[Sales.Revenue.max(), Sales.Cost.max(), 0].max()"
        );
        assert_eq!(omx("COUNT(B2:B4)"), "Sales.Revenue.count()");
        assert_eq!(omx("B2:B4"), format!("no: {RANGE}"));
        assert_eq!(omx("B2:B4*2"), format!("no: {RANGE}"));
    }

    #[test]
    fn operators_keep_their_order() {
        assert_eq!(omx("1+2*3"), "1 + 2 * 3");
        assert_eq!(omx("(1+2)*3"), "(1 + 2) * 3");
        assert_eq!(omx("1-(2-3)"), "1 - (2 - 3)");
        assert_eq!(omx("8/2/2"), "8 / 2 / 2");
        assert_eq!(omx("8/(2/2)"), "8 / (2 / 2)");
        // A workbook takes `^` from the left, and `-` before it.
        assert_eq!(omx("2^3^2"), "(2 ** 3) ** 2");
        assert_eq!(omx("-2^2"), "(-2) ** 2");
        assert_eq!(omx("-(2^2)"), "-2 ** 2");
        assert_eq!(omx("2*-3"), "2 * (-3)");
        assert_eq!(omx("B3<>C3"), "Revenue != Cost");
        assert_eq!(omx("B3=C3"), "Revenue == Cost");
        assert_eq!(omx("B3*10%"), "Revenue * 10%");
        assert_eq!(omx("B3%"), "Revenue / 100");
        assert_eq!(omx(".5+1E3"), "0.5 + 1000");
        assert_eq!(omx("\"a\"\"b\"=A3"), "\"a\\\"b\" == Month");
        assert_eq!(omx("A3&\"x\""), "no: it joins text with `&`");
    }

    #[test]
    fn functions() {
        assert_eq!(
            omx("IF(B3>C3,\"up\",\"down\")"),
            "if Revenue > Cost then \"up\" else \"down\""
        );
        assert_eq!(
            omx("IF(AND(B3>0,OR(C3>0,TRUE)),1,2)+1"),
            "(if Revenue > 0 and (Cost > 0 or true) then 1 else 2) + 1"
        );
        assert_eq!(omx("NOT(B3>0)"), "not Revenue > 0");
        assert_eq!(omx("ROUND(B3/3,2)"), "round(Revenue / 3, 0.01)");
        assert_eq!(omx("ROUND(B3,0)"), "round(Revenue)");
        assert_eq!(omx("ROUND(B3,1)"), "round(Revenue, 0.1)");
        assert_eq!(omx("ROUND(B3,-2)"), "round(Revenue, 100)");
        assert_eq!(omx("INT(B3)+ABS(C3)"), "floor(Revenue) + abs(Cost)");
        assert_eq!(omx("POWER(B3,2)"), "Revenue ** 2");
        assert_eq!(omx("YEAR(TODAY())"), "year(today())");
        assert_eq!(omx("DATEVALUE(A2)+1"), "[Month; *-1].Date + 1");
        assert_eq!(omx("WEEKDAY(A3,2)"), "weekday(Month)");
        // A workbook's Sunday is 1, and OMX's is 7.
        assert_eq!(
            omx("WEEKDAY(A3)"),
            "if weekday(Month) == 7 then 1 else weekday(Month) + 1"
        );
        assert_eq!(omx("WEEKDAY(A3)=1"), "weekday(Month) == 7");
        assert_eq!(
            omx("SWITCH(WEEKDAY(A3),1,\"Sun\",2,\"Mon\",\"other\")"),
            "if weekday(Month) == 7 then \"Sun\" else if weekday(Month) == 1 then \"Mon\" else \"other\""
        );
        assert_eq!(
            omx("_xlfn.SWITCH(B3,1,\"a\",2,\"b\",\"c\")"),
            "if Revenue == 1 then \"a\" else if Revenue == 2 then \"b\" else \"c\""
        );
        assert_eq!(
            omx("switch(B3,1,\"a\",2,\"b\")"),
            "if Revenue == 1 then \"a\" else \"b\""
        );
        assert_eq!(
            omx("IFS(B3>1,\"a\",TRUE,\"b\")"),
            "if Revenue > 1 then \"a\" else \"b\""
        );
        assert_eq!(omx("VLOOKUP(A3,B:C,2)"), "no: `VLOOKUP` has no equivalent");
        assert_eq!(omx("DATE(2026,6,30)"), "no: `DATE` has no equivalent");
        assert_eq!(
            omx("IF(B3>0,1)"),
            "no: `IF` is not written as OMX can take it"
        );
        assert_eq!(omx("Rate*2"), "no: it uses the name `Rate`");
        assert_eq!(omx("SUM(B2:B4"), format!("no: {UNREADABLE}"));
        assert!(translate("TODAY()", &worksheets(), 0, 2).unwrap().volatile);
    }
}

// Copyright (c) 2026 Stephen Roe

//! Recursive-descent parser for OMX expressions.
//!
//! Precedence, loosest first:
//! `|>`, `or`, `and`, `not`, `//`, comparison and `in`, `..` and `^n`, `+ -`, `* /`,
//! unary `-`, `**`, then postfix `[ ]`, `.Name` and calls.

use crate::ast::{BinOp, Expr, ExprKind, Lit, UnOp, reads_back};
use crate::decimal::Dec;
use crate::diag::{Diagnostic, Span};
use crate::lexer::{Tok, Token, lex};

type R<T> = Result<T, Diagnostic>;

/// Parse one expression from `text`, which starts at byte `base` of `src`.
pub fn parse_expr(text: &str, src: u32, base: usize) -> R<Expr> {
    let toks = lex(text, src, base)?;
    let mut p = Parser { toks, pos: 0 };
    if p.peek() == &Tok::Eof {
        return Err(Diagnostic::new(p.span(), "expected an expression"));
    }
    let parsed = p.seq().and_then(|e| {
        if p.peek() == &Tok::Eof {
            Ok(e)
        } else {
            Err(p.unexpected("an operator or the end of the expression"))
        }
    });
    parsed.map_err(|d| shift_or_fraction(&p.toks, text, base, d))
}

/// `a+<1/10>` reads as a shift, `+<`, and a `>` with nothing to compare. A
/// `+<` run straight into a number, with a `>` somewhere after it, is most
/// likely a sum with a fraction, so the error says how to write one.
fn shift_or_fraction(toks: &[Token], text: &str, base: usize, d: Diagnostic) -> Diagnostic {
    if d.help.is_some() {
        return d;
    }
    let touching = |t: &Token| {
        let at = (t.span.end as usize).saturating_sub(base);
        t.tok == Tok::PlusLt
            && text
                .as_bytes()
                .get(at)
                .is_some_and(|b| b.is_ascii_digit() || *b == b'-')
    };
    let Some(i) = toks.iter().position(touching) else {
        return d;
    };
    if !toks[i..].iter().any(|t| t.tok == Tok::Gt) {
        return d;
    }
    d.with_help("`+<` is a shift; to add a fraction put a space after the `+`, as in `a + <1/10>`")
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn span(&self) -> Span {
        self.toks[self.pos].span
    }

    fn prev_span(&self) -> Span {
        self.toks[self.pos.saturating_sub(1)].span
    }

    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == tok {
            self.bump();
            true
        } else {
            false
        }
    }

    fn unexpected(&self, wanted: &str) -> Diagnostic {
        let d = Diagnostic::new(
            self.span(),
            format!("expected {wanted}, found {}", self.peek().describe()),
        );
        if self.peek() == &Tok::Assign {
            d.with_help("use `==` to compare; `=` is not a comparison")
        } else {
            d
        }
    }

    fn expect(&mut self, tok: Tok, wanted: &str) -> R<()> {
        if self.eat(&tok) {
            Ok(())
        } else {
            Err(self.unexpected(wanted))
        }
    }

    fn ident(&mut self, wanted: &str) -> R<(String, Span)> {
        if let Tok::Ident(name) = self.peek().clone() {
            let span = self.bump().span;
            Ok((name, span))
        } else {
            Err(self.unexpected(wanted))
        }
    }

    /// A sequence, `a, b, rule ... limit`, or a plain expression. Only a
    /// whole formula may be one: inside brackets and arguments a comma
    /// still separates, so `sum(a, b)` and `[a, b]` are untouched.
    fn seq(&mut self) -> R<Expr> {
        let first = self.expr()?;
        if !matches!(self.peek(), Tok::Comma | Tok::DotDotDot) {
            return Ok(first);
        }
        let start = first.span;
        let mut terms = vec![first];
        while self.eat(&Tok::Comma) {
            terms.push(self.expr()?);
        }
        self.expect(Tok::DotDotDot, "`...` after the terms of a sequence")?;
        let limit = self.expr()?;
        // The last term is the rule when it reads a back-reference, and
        // another seed when it does not.
        let rule = terms
            .last()
            .is_some_and(reads_back)
            .then(|| Box::new(terms.pop().unwrap()));
        Ok(Expr {
            kind: ExprKind::Seq {
                seeds: terms,
                rule,
                limit: Box::new(limit),
            },
            span: start.to(self.prev_span()),
        })
    }

    fn expr(&mut self) -> R<Expr> {
        let mut lhs = self.or()?;
        while self.eat(&Tok::Pipe) {
            let (name, name_span) = self.ident("a function name after `|>`")?;
            self.expect(Tok::LParen, "`(`")?;
            let start = lhs.span;
            let mut args = vec![lhs];
            args.extend(self.args()?);
            lhs = Expr {
                kind: ExprKind::Call(name, name_span, args),
                span: start.to(self.prev_span()),
            };
        }
        Ok(lhs)
    }

    /// Arguments after an already-consumed `(`, through the closing `)`.
    fn args(&mut self) -> R<Vec<Expr>> {
        let mut args = Vec::new();
        if self.eat(&Tok::RParen) {
            return Ok(args);
        }
        loop {
            // `Name := expr`, an argument given a name: what `agg` names
            // the column of an aggregate by.
            if let Tok::Ident(name) = self.peek().clone()
                && self
                    .toks
                    .get(self.pos + 1)
                    .is_some_and(|t| t.tok == Tok::Define)
            {
                let name_span = self.bump().span;
                self.bump();
                let value = self.expr()?;
                let span = name_span.to(value.span);
                args.push(Expr {
                    kind: ExprKind::Named(name, name_span, Box::new(value)),
                    span,
                });
            } else {
                args.push(self.expr()?);
            }
            if self.eat(&Tok::Comma) {
                continue;
            }
            self.expect(Tok::RParen, "`,` or `)`")?;
            return Ok(args);
        }
    }

    /// A row slot's expression, with the `by Region, Year` after it if one
    /// follows: nothing else can follow an expression in a slot, so the
    /// word is not taken from a name anyone could have meant.
    fn by(&mut self, e: Expr) -> R<Expr> {
        if !matches!(self.peek(), Tok::Ident(w) if w == "by") {
            return Ok(e);
        }
        self.bump();
        let mut keys = vec![self.ident("a column name after `by`")?];
        while self.eat(&Tok::Comma) {
            keys.push(self.ident("a column name after `,`")?);
        }
        let span = e.span.to(self.prev_span());
        Ok(Expr {
            kind: ExprKind::By(Box::new(e), keys),
            span,
        })
    }

    fn binary(lhs: Expr, op: BinOp, rhs: Expr) -> Expr {
        let span = lhs.span.to(rhs.span);
        Expr {
            kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
            span,
        }
    }

    fn or(&mut self) -> R<Expr> {
        let mut lhs = self.and()?;
        while self.eat(&Tok::Or) {
            let rhs = self.and()?;
            lhs = Self::binary(lhs, BinOp::Or, rhs);
        }
        Ok(lhs)
    }

    fn and(&mut self) -> R<Expr> {
        let mut lhs = self.not()?;
        while self.eat(&Tok::And) {
            let rhs = self.not()?;
            lhs = Self::binary(lhs, BinOp::And, rhs);
        }
        Ok(lhs)
    }

    fn not(&mut self) -> R<Expr> {
        if self.peek() == &Tok::Not {
            let start = self.bump().span;
            let operand = self.not()?;
            let span = start.to(operand.span);
            return Ok(Expr {
                kind: ExprKind::Unary(UnOp::Not, Box::new(operand)),
                span,
            });
        }
        self.fallback()
    }

    fn fallback(&mut self) -> R<Expr> {
        let mut lhs = self.comparison()?;
        while self.eat(&Tok::SlashSlash) {
            let rhs = self.comparison()?;
            lhs = Self::binary(lhs, BinOp::Fallback, rhs);
        }
        Ok(lhs)
    }

    fn comparison(&mut self) -> R<Expr> {
        let lhs = self.range()?;
        let op = match self.peek() {
            Tok::EqEq => BinOp::Eq,
            Tok::NotEq => BinOp::Ne,
            Tok::Lt => BinOp::Lt,
            Tok::Gt => BinOp::Gt,
            Tok::Le => BinOp::Le,
            Tok::Ge => BinOp::Ge,
            Tok::In => BinOp::In,
            Tok::Assign => return Err(self.unexpected("an operator")),
            _ => return Ok(lhs),
        };
        self.bump();
        let rhs = self.range()?;
        if op == BinOp::In {
            return Ok(Self::binary(lhs, op, rhs));
        }
        // Comparisons chain, as in Raku (D166): `0 < x <= 10` is
        // `0 < x and x <= 10`. An expression has no effects, so the middle
        // one is written twice rather than held.
        let mut out = Self::binary(lhs, op, rhs.clone());
        let mut middle = rhs;
        while let Some(op) = match self.peek() {
            Tok::EqEq => Some(BinOp::Eq),
            Tok::NotEq => Some(BinOp::Ne),
            Tok::Lt => Some(BinOp::Lt),
            Tok::Gt => Some(BinOp::Gt),
            Tok::Le => Some(BinOp::Le),
            Tok::Ge => Some(BinOp::Ge),
            _ => None,
        } {
            self.bump();
            let next = self.range()?;
            let link = Self::binary(middle, op, next.clone());
            out = Self::binary(out, BinOp::And, link);
            middle = next;
        }
        Ok(out)
    }

    fn range(&mut self) -> R<Expr> {
        // `^5`: the first five, 0 to 4.
        if self.peek() == &Tok::Caret {
            let start = self.bump().span;
            let operand = self.additive()?;
            let span = start.to(operand.span);
            return Ok(Expr {
                kind: ExprKind::Unary(UnOp::UpTo, Box::new(operand)),
                span,
            });
        }
        let lhs = self.additive()?;
        let op = match self.peek() {
            Tok::DotDot => BinOp::Range,
            Tok::DotDotCaret => BinOp::RangeEx,
            Tok::CaretDotDot => BinOp::RangeFrom,
            Tok::CaretDotDotCaret => BinOp::RangeBoth,
            _ => return Ok(lhs),
        };
        self.bump();
        let rhs = self.additive()?;
        Ok(Self::binary(lhs, op, rhs))
    }

    fn additive(&mut self) -> R<Expr> {
        let mut lhs = self.multiplicative()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                // Bitwise or and xor bind as a sum does, as in Raku (D139).
                Tok::PlusPipe => BinOp::BitOr,
                Tok::PlusCaret => BinOp::BitXor,
                _ => return Ok(lhs),
            };
            self.bump();
            let rhs = self.multiplicative()?;
            lhs = Self::binary(lhs, op, rhs);
        }
    }

    fn multiplicative(&mut self) -> R<Expr> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                // And the bitwise and and the shifts as a product does.
                Tok::PlusAmp => BinOp::BitAnd,
                Tok::PlusLt => BinOp::Shl,
                Tok::PlusGt => BinOp::Shr,
                _ => return Ok(lhs),
            };
            self.bump();
            let rhs = self.unary()?;
            lhs = Self::binary(lhs, op, rhs);
        }
    }

    fn unary(&mut self) -> R<Expr> {
        let op = match self.peek() {
            Tok::Minus => UnOp::Neg,
            Tok::PlusCaret => UnOp::BitNot,
            _ => return self.power(),
        };
        let start = self.bump().span;
        let operand = self.unary()?;
        let span = start.to(operand.span);
        Ok(Expr {
            kind: ExprKind::Unary(op, Box::new(operand)),
            span,
        })
    }

    fn power(&mut self) -> R<Expr> {
        let base = self.postfix()?;
        if self.eat(&Tok::Pow) {
            let exponent = self.unary()?;
            return Ok(Self::binary(base, BinOp::Pow, exponent));
        }
        Ok(base)
    }

    fn postfix(&mut self) -> R<Expr> {
        let mut e = self.primary()?;
        loop {
            if self.eat(&Tok::LBracket) {
                let mut slots = Vec::new();
                loop {
                    let slot = if matches!(self.peek(), Tok::Semi | Tok::RBracket) {
                        None
                    } else {
                        let e = self.expr()?;
                        Some(self.by(e)?)
                    };
                    slots.push(slot);
                    if self.eat(&Tok::Semi) {
                        continue;
                    }
                    self.expect(Tok::RBracket, "`;` or `]`")?;
                    break;
                }
                let span = e.span.to(self.prev_span());
                e = Expr {
                    kind: ExprKind::Index(Box::new(e), slots),
                    span,
                };
            } else if self.eat(&Tok::Dot) {
                let (name, name_span) = self.ident("a column or method name after `.`")?;
                if self.eat(&Tok::LParen) {
                    let start = e.span;
                    let mut args = vec![e];
                    args.extend(self.args()?);
                    e = Expr {
                        kind: ExprKind::Call(name, name_span, args),
                        span: start.to(self.prev_span()),
                    };
                } else {
                    let span = e.span.to(name_span);
                    e = Expr {
                        kind: ExprKind::Field(Box::new(e), name, name_span),
                        span,
                    };
                }
            } else {
                return Ok(e);
            }
        }
    }

    /// A fraction written out: `<1/10>`, `<-3/4>`, `<2 3/4>` or `<3>`. It is
    /// a `Fraction` literal, as `1/10` is in a cell, where in a formula a bare
    /// `1/10` is a quotient.
    fn fraction(&mut self) -> R<Expr> {
        let start = self.span();
        self.bump();
        let negative = self.eat(&Tok::Minus);
        let whole = |t: &Tok| match t {
            Tok::Int(n, crate::types::Base::Dec) => Some(*n),
            _ => None,
        };
        let bad =
            |p: &Self| Err(p.unexpected("a fraction such as `<1/10>` or `<2 3/4>`, closed by `>`"));
        let Some(first) = whole(self.peek()) else {
            // A decimal on its own, `<0.5>`, is the fraction it is.
            if let Tok::Decimal(r) = self.peek().clone() {
                self.bump();
                return self.fraction_of(start, negative, Some(r));
            }
            return bad(self);
        };
        self.bump();
        // `<3>`, `<1/10>` or `<2 3/4>`.
        let value = if let Some(n) = whole(self.peek()) {
            self.bump();
            if !self.eat(&Tok::Slash) {
                return bad(self);
            }
            let Some(d) = whole(self.peek()) else {
                return bad(self);
            };
            let d_span = self.bump().span;
            let part = self.quotient(n, d, d_span)?;
            Dec::from_i64(first).checked_add(part)
        } else if self.eat(&Tok::Slash) {
            let Some(d) = whole(self.peek()) else {
                return bad(self);
            };
            let d_span = self.bump().span;
            Some(self.quotient(first, d, d_span)?)
        } else {
            Some(Dec::from_i64(first))
        };
        self.fraction_of(start, negative, value)
    }

    /// `n/d` as an exact number, or the error of a zero denominator.
    fn quotient(&self, n: i64, d: i64, span: Span) -> R<Dec> {
        if d == 0 {
            return Err(Diagnostic::new(
                span,
                "a fraction has a denominator that is not zero",
            ));
        }
        Dec::from_i64(n)
            .checked_div(Dec::from_i64(d))
            .ok_or_else(|| Diagnostic::new(span, "this fraction is too large"))
    }

    /// Close a fraction at `>`, as the `Fraction` of `value`.
    fn fraction_of(&mut self, start: Span, negative: bool, value: Option<Dec>) -> R<Expr> {
        if self.peek() != &Tok::Gt {
            return Err(self.unexpected("`>` to close the fraction"));
        }
        let end = self.bump().span;
        let span = start.to(end);
        let Some(value) = value.and_then(|v| if negative { v.checked_neg() } else { Some(v) })
        else {
            return Err(Diagnostic::new(span, "this fraction is too large"));
        };
        Ok(Expr {
            kind: ExprKind::Lit(Lit::Fraction(value)),
            span,
        })
    }

    fn primary(&mut self) -> R<Expr> {
        let span = self.span();
        let lit = |kind: Lit| ExprKind::Lit(kind);
        let kind = match self.peek().clone() {
            Tok::Int(n, b) => lit(Lit::Int(n, b)),
            Tok::Decimal(r) => lit(Lit::Decimal(r)),
            Tok::Percent(r) => lit(Lit::Percent(r)),
            Tok::Float(f) => lit(Lit::Float(f)),
            Tok::Str(s) => lit(Lit::String(s)),
            Tok::Text(s) => {
                return Err(Diagnostic::new(span, "strings in OMX are in single quotes")
                    .with_help(format!(
                        "write {}; double quotes are for text in a table cell",
                        crate::lexer::quoted(&s)
                    )));
            }
            Tok::Date(d) => lit(Lit::Date(d)),
            Tok::Time(t) => lit(Lit::Time(t)),
            Tok::Datetime(t) => lit(Lit::Datetime(t)),
            Tok::Duration(t) => lit(Lit::Duration(t)),
            Tok::True => lit(Lit::Boolean(true)),
            Tok::False => lit(Lit::Boolean(false)),
            Tok::Star => ExprKind::Cursor,
            Tok::Lt => return self.fraction(),
            Tok::Dollar => {
                self.bump();
                if !self.eat(&Tok::Minus) {
                    return Err(Diagnostic::new(
                        span.to(self.span()),
                        "a back-reference counts backwards",
                    )
                    .with_help("write `$-1` for the term before this one"));
                }
                let Tok::Int(n, _) = self.peek().clone() else {
                    return Err(self.unexpected("a whole number of terms to go back"));
                };
                self.bump();
                let Ok(n) = u32::try_from(n) else {
                    return Err(Diagnostic::new(
                        span.to(self.prev_span()),
                        "a back-reference goes back too far",
                    ));
                };
                if n == 0 {
                    return Err(Diagnostic::new(
                        span.to(self.prev_span()),
                        "a back-reference goes back at least one term",
                    )
                    .with_help("`$-1` is the term before this one"));
                }
                return Ok(Expr {
                    kind: ExprKind::Back(n),
                    span: span.to(self.prev_span()),
                });
            }
            Tok::Ident(name) => {
                self.bump();
                if self.eat(&Tok::LParen) {
                    let args = self.args()?;
                    return Ok(Expr {
                        kind: ExprKind::Call(name, span, args),
                        span: span.to(self.prev_span()),
                    });
                }
                return Ok(Expr {
                    kind: ExprKind::Name(name),
                    span,
                });
            }
            Tok::LParen => {
                self.bump();
                let inner = self.expr()?;
                self.expect(Tok::RParen, "`)`")?;
                return Ok(Expr {
                    kind: inner.kind,
                    span: span.to(self.prev_span()),
                });
            }
            Tok::LBracket => {
                self.bump();
                let mut items = Vec::new();
                // `[Revenue; *-1]`, with a `;` or a row cursor, is an index
                // into the current sheet; anything else is a vector.
                let mut first = None;
                if !matches!(self.peek(), Tok::Semi | Tok::RBracket) {
                    first = Some(self.expr()?);
                }
                let cursor = first.as_ref().is_some_and(|e| match &e.kind {
                    ExprKind::Cursor => true,
                    ExprKind::Binary(BinOp::Add | BinOp::Sub, l, _) => {
                        matches!(l.kind, ExprKind::Cursor)
                    }
                    _ => false,
                });
                if matches!(self.peek(), Tok::Semi)
                    || (cursor && !matches!(self.peek(), Tok::Comma))
                {
                    // `[*-1 by Region]`, a cursor and its partition alone.
                    let first = match first {
                        Some(e) if cursor => Some(self.by(e)?),
                        other => other,
                    };
                    let mut slots = vec![first];
                    while self.eat(&Tok::Semi) {
                        slots.push(if matches!(self.peek(), Tok::Semi | Tok::RBracket) {
                            None
                        } else {
                            let e = self.expr()?;
                            Some(self.by(e)?)
                        });
                    }
                    self.expect(Tok::RBracket, "`;` or `]`")?;
                    let own = Expr {
                        kind: ExprKind::Own,
                        span: Span::new(span.src, span.start as usize, span.start as usize),
                    };
                    return Ok(Expr {
                        kind: ExprKind::Index(Box::new(own), slots),
                        span: span.to(self.prev_span()),
                    });
                }
                if let Some(e) = first {
                    items.push(e);
                    while self.eat(&Tok::Comma) {
                        items.push(self.expr()?);
                    }
                }
                self.expect(Tok::RBracket, "`,` or `]`")?;
                return Ok(Expr {
                    kind: ExprKind::VecLit(items),
                    span: span.to(self.prev_span()),
                });
            }
            Tok::If => {
                self.bump();
                let cond = self.expr()?;
                self.expect(Tok::Then, "`then`")?;
                let then = self.expr()?;
                self.expect(Tok::Else, "`else`")?;
                let otherwise = self.or()?;
                let span = span.to(otherwise.span);
                return Ok(Expr {
                    kind: ExprKind::If(Box::new(cond), Box::new(then), Box::new(otherwise)),
                    span,
                });
            }
            _ => return Err(self.unexpected("a value")),
        };
        self.bump();
        Ok(Expr { kind, span })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> R<Expr> {
        parse_expr(s, 0, 0)
    }

    #[test]
    fn comparisons_chain() {
        // The shape without the spans, which differ between the two.
        let shown = |s: &str| {
            let text = format!("{:?}", parse(s).unwrap().kind);
            let mut out = String::new();
            let mut rest = text.as_str();
            while let Some(at) = rest.find("Span {") {
                out.push_str(&rest[..at]);
                rest = &rest[at + rest[at..].find('}').unwrap() + 1..];
            }
            out + rest
        };
        // `0 < x <= 10` is `0 < x and x <= 10`.
        let chained = shown("0 < x <= 10");
        assert_eq!(chained, shown("0 < x and x <= 10"));
        assert_eq!(shown("a < b < c < d"), shown("a < b and b < c and c < d"));
        assert_eq!(shown("a == b != c"), shown("a == b and b != c"));
        // `in` takes one side each.
        assert!(parse("a in b in c").is_err());
        assert!(parse("a < b in c").is_err());
    }

    #[test]
    fn code_takes_single_quotes() {
        assert!(matches!(
            parse("Name // 'Unknown'").unwrap().kind,
            ExprKind::Binary(BinOp::Fallback, ..)
        ));
        let err = parse("Name // \"Unknown\"").unwrap_err();
        assert_eq!(err.message, "strings in OMX are in single quotes");
        assert!(err.help.as_deref().unwrap().contains("'Unknown'"));
        assert_eq!((err.span.start, err.span.end), (8, 17));
    }

    #[test]
    fn shapes() {
        assert!(matches!(
            parse("Sales[Revenue; *-1] // 0").unwrap().kind,
            ExprKind::Binary(BinOp::Fallback, ..)
        ));
        assert!(matches!(
            parse("Sales[; 1]").unwrap().kind,
            ExprKind::Index(_, slots) if slots.len() == 2 && slots[0].is_none()
        ));
        assert!(matches!(
            parse("Sales |> filter(Region == 'UK') |> sum()").unwrap().kind,
            ExprKind::Call(name, _, args) if name == "sum" && args.len() == 1
        ));
        assert!(matches!(
            parse("-2 ** 2").unwrap().kind,
            ExprKind::Unary(UnOp::Neg, _)
        ));
        assert!(matches!(
            parse("0^..10").unwrap().kind,
            ExprKind::Binary(BinOp::RangeFrom, ..)
        ));
        assert!(matches!(
            parse("3 in ^5").unwrap().kind,
            ExprKind::Binary(BinOp::In, _, r) if matches!(r.kind, ExprKind::Unary(UnOp::UpTo, _))
        ));
        // The bitwise operators bind as Raku binds them (D139): a flag test
        // needs no brackets, and a shift goes before a mask.
        assert!(matches!(
            parse("F +& 4 == 4").unwrap().kind,
            ExprKind::Binary(BinOp::Eq, l, _) if matches!(l.kind, ExprKind::Binary(BinOp::BitAnd, ..))
        ));
        assert!(matches!(
            parse("a +| b +& c").unwrap().kind,
            ExprKind::Binary(BinOp::BitOr, _, r) if matches!(r.kind, ExprKind::Binary(BinOp::BitAnd, ..))
        ));
        assert!(matches!(
            parse("+^a +^ b").unwrap().kind,
            ExprKind::Binary(BinOp::BitXor, l, _) if matches!(l.kind, ExprKind::Unary(UnOp::BitNot, _))
        ));
    }

    #[test]
    fn single_equals_suggests_double() {
        let err = parse("Sales[; Region = 'UK']").unwrap_err();
        assert!(err.help.unwrap().contains("=="));
    }
}

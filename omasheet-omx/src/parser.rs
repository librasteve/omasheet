// Copyright (c) 2026 Stephen Roe

//! Recursive-descent parser for OMX expressions.
//!
//! Precedence, loosest first:
//! `|>`, `or`, `and`, `not`, `//`, comparison and `in`, `..` and `^n`, `+ -`, `* /`,
//! unary `-`, `**`, then postfix `[ ]`, `.Name` and calls.

use crate::ast::{BinOp, Expr, ExprKind, Lit, UnOp};
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
    let e = p.expr()?;
    if p.peek() != &Tok::Eof {
        return Err(p.unexpected("an operator or the end of the expression"));
    }
    Ok(e)
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
            args.push(self.expr()?);
            if self.eat(&Tok::Comma) {
                continue;
            }
            self.expect(Tok::RParen, "`,` or `)`")?;
            return Ok(args);
        }
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
        Ok(Self::binary(lhs, op, rhs))
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
                _ => return Ok(lhs),
            };
            self.bump();
            let rhs = self.unary()?;
            lhs = Self::binary(lhs, op, rhs);
        }
    }

    fn unary(&mut self) -> R<Expr> {
        if self.peek() == &Tok::Minus {
            let start = self.bump().span;
            let operand = self.unary()?;
            let span = start.to(operand.span);
            return Ok(Expr {
                kind: ExprKind::Unary(UnOp::Neg, Box::new(operand)),
                span,
            });
        }
        self.power()
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
                        Some(self.expr()?)
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

    fn primary(&mut self) -> R<Expr> {
        let span = self.span();
        let lit = |kind: Lit| ExprKind::Lit(kind);
        let kind = match self.peek().clone() {
            Tok::Int(n) => lit(Lit::Int(n)),
            Tok::Ratio(r) => lit(Lit::Ratio(r)),
            Tok::Num(f) => lit(Lit::Num(f)),
            Tok::Imag(f) => lit(Lit::Complex(0.0, f)),
            Tok::Str(s) => lit(Lit::Text(s)),
            Tok::Date(d) => lit(Lit::Date(d)),
            Tok::Time(t) => lit(Lit::Time(t)),
            Tok::DateTime(t) => lit(Lit::DateTime(t)),
            Tok::True => lit(Lit::Bool(true)),
            Tok::False => lit(Lit::Bool(false)),
            Tok::Star => ExprKind::Cursor,
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
                // into the current table; anything else is a vector.
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
                    let mut slots = vec![first];
                    while self.eat(&Tok::Semi) {
                        slots.push(if matches!(self.peek(), Tok::Semi | Tok::RBracket) {
                            None
                        } else {
                            Some(self.expr()?)
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
            parse("Sales |> filter(Region == \"UK\") |> sum()").unwrap().kind,
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
    }

    #[test]
    fn single_equals_suggests_double() {
        let err = parse("Sales[; Region = \"UK\"]").unwrap_err();
        assert!(err.help.unwrap().contains("=="));
    }
}

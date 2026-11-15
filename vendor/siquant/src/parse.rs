// Copyright (c) 2026 Stephen Roe

//! The unit notation, and nothing else:
//!
//! ```text
//! unit := term ('.' term)*
//! term := symbol ('^' '-'? digit+)?
//! ```
//!
//! `kg.m^2.s^-2`. No spaces, no `/`, no brackets, no superscripts, no
//! trailing-digit exponents (`m2`), no names or plurals.
//!
//! One more form: a prefix symbol and a `.` on their own, `k.`, is a scale,
//! a dimensionless number: `1k.` is 1000. The `.` makes it a prefix and
//! nothing else, so `m.` is milli and `T.` is tera, with no clash with the
//! metre or the tesla. It is a whole unit: `k.m` and `k.^2` are errors.
//!
//! A symbol is a unit symbol, or a prefix symbol and a prefixable unit
//! symbol. The exact unit
//! symbol wins over a prefix reading; [`crate::Registry`] refuses at load any
//! symbol that could be both, so nothing is ever shadowed.

use crate::error::{Error, Result};
use crate::registry::{Inner, PrefixId, Scope, UnitId};
use crate::unit::Term;

fn syntax<T>(at: usize, message: &str) -> Result<T> {
    Err(Error::Syntax {
        at,
        message: message.to_string(),
    })
}

impl Inner {
    /// The prefix and unit a symbol means.
    pub(crate) fn resolve(&self, symbol: &str, scope: Scope) -> Result<(Option<PrefixId>, UnitId)> {
        let live_only = scope == Scope::Live;
        if let Some(&id) = self.unit_by_symbol.get(symbol) {
            if live_only && !self.unit_def(id).live {
                return Err(Error::NotLive(symbol.to_string()));
            }
            return Ok((None, id));
        }
        let mut found: Vec<(PrefixId, UnitId)> = Vec::new();
        for (i, p) in self.prefixes.iter().enumerate() {
            for ps in p.all_symbols() {
                let Some(rest) = symbol.strip_prefix(ps).filter(|r| !r.is_empty()) else {
                    continue;
                };
                if let Some(&uid) = self.unit_by_symbol.get(rest)
                    && self.unit_def(uid).prefixable.allows(p.kind)
                {
                    found.push((PrefixId(i as u32), uid));
                }
            }
        }
        found.sort();
        found.dedup();
        match found.as_slice() {
            [] => Err(Error::UnknownUnit(symbol.to_string())),
            [(p, u)] => {
                if live_only && !self.unit_def(*u).live {
                    return Err(Error::NotLive(symbol.to_string()));
                }
                Ok((Some(*p), *u))
            }
            many => Err(Error::AmbiguousSymbol {
                symbol: symbol.to_string(),
                candidates: many
                    .iter()
                    .map(|(p, u)| {
                        format!(
                            "{}+{}",
                            self.prefix_def(*p).symbol,
                            self.unit_def(*u).symbol
                        )
                    })
                    .collect(),
            }),
        }
    }

    /// The normalised terms of a unit written as `text`.
    pub(crate) fn parse_terms(&self, text: &str, scope: Scope) -> Result<Vec<Term>> {
        if text.is_empty() {
            return syntax(0, "a unit is expected");
        }
        // A prefix and a `.` on their own is a scale: `k.` is 1000. The dot
        // makes it a prefix and nothing else, so `m.` is milli, not the metre.
        if let Some(body) = text.strip_suffix('.')
            && let Some(&p) = self.prefix_by_symbol.get(body)
        {
            return self.normalise(vec![Term {
                prefix: Some(p),
                unit: self.unity,
                exp: 1,
            }]);
        }
        let mut terms = Vec::new();
        let mut at = 0;
        for part in text.split('.') {
            terms.push(self.parse_term(part, at, scope)?);
            at += part.len() + 1;
        }
        self.normalise(terms)
    }

    fn parse_term(&self, part: &str, at: usize, scope: Scope) -> Result<Term> {
        if part.is_empty() {
            return syntax(at, "a unit symbol is expected between the `.`s");
        }
        if let Some(i) = part.find(char::is_whitespace) {
            return syntax(at + i, "spaces are not allowed; write kg.m, not kg m");
        }
        if let Some(i) = part.find('/') {
            return syntax(
                at + i,
                "`/` is not allowed; write a negative exponent, kg.m^-1",
            );
        }
        let (symbol, exp) = match part.split_once('^') {
            None => (part, 1),
            Some((symbol, digits)) => (symbol, parse_exponent(digits, at + symbol.len() + 1)?),
        };
        if symbol.is_empty() {
            return syntax(at, "a unit symbol is expected before `^`");
        }
        let (prefix, unit) = self.resolve(symbol, scope)?;
        Ok(Term { prefix, unit, exp })
    }
}

/// The exponent after a `^`: an optional `-` and digits, not zero.
fn parse_exponent(text: &str, at: usize) -> Result<i16> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return syntax(
            at,
            "an integer exponent is expected after `^`, as in m^2 or s^-1",
        );
    }
    match text.parse::<i16>() {
        Ok(0) => syntax(at, "an exponent of 0 is not allowed"),
        Ok(n) => Ok(n),
        Err(_) => Err(Error::Overflow),
    }
}

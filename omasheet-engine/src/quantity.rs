// Copyright (c) 2026 Stephen Roe

//! Quantities: numbers with SI units (`add-si-units`).
//!
//! A [`Value::Quantity`] holds a `siquant::Quantity`, a value, a 1-sigma
//! uncertainty and a unit. Arithmetic, comparison, conversion and the
//! propagation of the uncertainty are siquant's; this module adapts them to
//! values, and to the way omasheet reports a problem, as a message for the
//! cell. A result that has no dimension (`6m / 3m`) is a plain number, a
//! `Float`, or a `Scientific` where it has an uncertainty (D235).

use crate::value::Value;
use omasheet_omx::ast::{BinOp, Lit};
use omasheet_omx::colfmt::SciFormat;
use omasheet_omx::date::Style;
use omasheet_omx::funcs::MathFn;
use omasheet_omx::types::QId;
use siquant::{Quantity, Registry, Scope, Unit};
use std::cmp::Ordering;
use std::rc::Rc;

fn said(e: siquant::Error) -> String {
    e.to_string()
}

/// What a dimension is called: the names of the quantity types that have it
/// (`Mass`, `Energy/Torque`), or its symbols where none does.
fn kind_of(dims: siquant::Dim) -> String {
    let names: Vec<String> = Registry::builtin()
        .quantity_types_for(dims)
        .iter()
        .map(|t| t.name().to_string())
        .collect();
    if names.is_empty() {
        dims.to_string()
    } else {
        names.join("/")
    }
}

/// An error of `x` and `y`, `verb`ed, said with the units where it is that
/// their dimensions differ: `cannot add kg and m: Mass and Length`.
fn explain(verb: &str, x: &Quantity, y: &Quantity, e: siquant::Error) -> String {
    match e {
        siquant::Error::DimensionMismatch { .. } => format!(
            "cannot {verb} {} and {}: they measure {} and {}",
            x.unit(),
            y.unit(),
            kind_of(x.unit().dims()),
            kind_of(y.unit().dims())
        ),
        other => said(other),
    }
}

/// The unit of a plain number.
fn one() -> Unit {
    Unit::one(Registry::builtin())
}

/// A value as a quantity: a quantity itself, or a plain real number, which
/// is a quantity with no dimension. `None` for anything else.
fn lift(v: &Value) -> Option<Quantity> {
    Some(match v {
        Value::Quantity(q) => (**q).clone(),
        Value::Int(n, _) => Quantity::exact(*n as f64, one()),
        Value::Decimal(r) | Value::Fraction(r) | Value::Percent(r) => {
            Quantity::exact(r.to_f64(), one())
        }
        Value::Float(f) => Quantity::exact(*f, one()),
        Value::Scientific(v, s) => Quantity::new(*v, *s, one()).ok()?,
        _ => return None,
    })
}

/// A quantity as a value: a number where it has no dimension.
pub fn value(q: Quantity) -> Value {
    if q.unit().is_dimensionless() {
        match q.to_number() {
            Ok((v, 0.0)) => Value::Float(v),
            Ok((v, s)) => Value::Scientific(v, s),
            Err(_) => Value::Quantity(Rc::new(q)),
        }
    } else {
        Value::Quantity(Rc::new(q))
    }
}

/// The value a literal stands for.
pub fn from_lit(v: f64, s: f64, unit: &Unit) -> Value {
    match Quantity::new(v, s, unit.clone()) {
        Ok(q) => Value::Quantity(Rc::new(q)),
        Err(_) => Value::Error,
    }
}

/// A quantity as a literal.
pub fn to_lit(q: &Quantity) -> Lit {
    Lit::Quantity(q.value(), q.sigma(), q.unit().clone())
}

/// `a op b` where one side is a quantity; `None` where neither is, so the
/// numbers can take it.
pub fn arith(op: BinOp, a: &Value, b: &Value) -> Option<Result<Value, String>> {
    if !matches!(a, Value::Quantity(_)) && !matches!(b, Value::Quantity(_)) {
        return None;
    }
    Some(arith_of(op, a, b))
}

fn arith_of(op: BinOp, a: &Value, b: &Value) -> Result<Value, String> {
    let (Some(x), Some(y)) = (lift(a), lift(b)) else {
        return Err(format!(
            "cannot apply `{}` to {} and {}",
            op.symbol(),
            a.kind(),
            b.kind()
        ));
    };
    let verb = match op {
        BinOp::Add => "add",
        BinOp::Sub => "subtract",
        BinOp::Mul => "multiply",
        _ => "divide",
    };
    let out = match op {
        BinOp::Add => x.add(&y),
        BinOp::Sub => x.sub(&y),
        BinOp::Mul => x.mul(&y),
        BinOp::Div => x.div(&y),
        BinOp::Pow => {
            let n = match b {
                Value::Int(n, _) => i16::try_from(*n)
                    .map_err(|_| "the power of a quantity is out of range".to_string())?,
                _ => {
                    return Err(format!(
                        "a quantity takes a whole power, but this is {}",
                        b.kind()
                    ));
                }
            };
            if matches!(a, Value::Quantity(_)) {
                x.pow(n)
            } else {
                return Err("a power of a quantity needs the quantity as its base".to_string());
            }
        }
        _ => {
            return Err(format!(
                "`{}` cannot be applied to {} and {}",
                op.symbol(),
                a.kind(),
                b.kind()
            ));
        }
    };
    out.map(value).map_err(|e| explain(verb, &x, &y, e))
}

/// `a ± b` where `a` is a quantity: the uncertainty is a quantity of the same
/// dimension, or a plain number in the unit of `a`. `None` where `a` is not
/// a quantity.
pub fn plus_minus(a: &Value, b: &Value) -> Option<Result<Value, String>> {
    let Value::Quantity(x) = a else { return None };
    Some((|| {
        if x.sigma() != 0.0 {
            return Err("`±` needs a quantity with no uncertainty of its own on the left".into());
        }
        let sigma = match b {
            Value::Quantity(s) => {
                if s.sigma() != 0.0 {
                    return Err("the uncertainty after `±` cannot itself be uncertain".into());
                }
                s.convert(x.unit())
                    .map_err(|e| explain("use", x, s, e))?
                    .value()
            }
            other => match lift(other) {
                Some(n) if n.unit().is_dimensionless() => n.to_number().map_err(said)?.0,
                _ => {
                    return Err(format!(
                        "`±` needs a number or a quantity after it, found {}",
                        b.kind()
                    ));
                }
            },
        };
        if sigma < 0.0 {
            return Err("the uncertainty after `±` cannot be negative".to_string());
        }
        Quantity::new(x.value(), sigma, x.unit().clone())
            .map(|q| Value::Quantity(Rc::new(q)))
            .map_err(said)
    })())
}

/// The quantity with its value negated.
pub fn negate(q: &Quantity) -> Result<Value, String> {
    Quantity::new(-q.value(), q.sigma(), q.unit().clone())
        .map(|q| Value::Quantity(Rc::new(q)))
        .map_err(said)
}

/// A value and its uncertainty.
type Pair = (f64, f64);

/// Both sides in one unit: `(value, sigma)` of each. Affine units go
/// through the unit they are defined over.
fn aligned(a: &Quantity, b: &Quantity) -> Result<(Pair, Pair), String> {
    let (x, y) = if a.unit().is_affine() || b.unit().is_affine() {
        (a.rebase().map_err(said)?, b.rebase().map_err(said)?)
    } else {
        (a.clone(), b.clone())
    };
    let y = y
        .convert(x.unit())
        .map_err(|e| explain("compare", a, b, e))?;
    Ok(((x.value(), x.sigma()), (y.value(), y.sigma())))
}

/// Order two single values where one is a quantity. Two quantities of one
/// dimension are equal when they differ by no more than their uncertainties
/// together, as two `Scientific` values are, where `tolerant`. `None` where
/// neither is a quantity.
pub fn compare(a: &Value, b: &Value, tolerant: bool) -> Option<Result<Option<Ordering>, String>> {
    if !matches!(a, Value::Quantity(_)) && !matches!(b, Value::Quantity(_)) {
        return None;
    }
    Some((|| {
        if matches!(a, Value::Null) || matches!(b, Value::Null) {
            return Ok(None);
        }
        let (Value::Quantity(x), Value::Quantity(y)) = (a, b) else {
            return Err(format!("cannot compare {} and {}", a.kind(), b.kind()));
        };
        let ((vx, sx), (vy, sy)) = aligned(x, y)?;
        if tolerant && !vx.is_nan() && !vy.is_nan() && (vx - vy).abs() <= sx.hypot(sy) {
            return Ok(Some(Ordering::Equal));
        }
        Ok(Some(match (vx.is_nan(), vy.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => vx.partial_cmp(&vy).unwrap_or(Ordering::Equal),
        }))
    })())
}

/// A function of a quantity: those that keep or halve its unit.
pub fn math(m: MathFn, q: &Quantity) -> Result<Value, String> {
    match m.name() {
        "abs" => Quantity::new(q.value().abs(), q.sigma(), q.unit().clone())
            .map(|q| Value::Quantity(Rc::new(q)))
            .map_err(said),
        "sqrt" => q.root(2).map(value).map_err(said),
        "cbrt" => q.root(3).map(value).map_err(said),
        name => Err(format!(
            "`{name}` needs a number with no unit, but this is a quantity in {}; \
             `abs`, `sqrt` and `cbrt` take one",
            q.unit()
        )),
    }
}

/// `to(x, 'g')`: the quantity in another unit of its dimension.
pub fn to_unit(v: &Value, unit: &Value) -> Result<Value, String> {
    let Value::String(text, _) = unit else {
        return Err(format!(
            "expected the name of a unit, found {}",
            unit.kind()
        ));
    };
    let target =
        Unit::parse(text, Scope::Any).map_err(|e| format!("`{text}` is not a unit: {e}"))?;
    match v {
        Value::Null => Ok(Value::Null),
        Value::Quantity(q) => q.convert(&target).map(value).map_err(|e| match e {
            siquant::Error::DimensionMismatch { .. } => format!(
                "cannot convert {} to {target}: {} is a {} and {target} a {}",
                q.unit(),
                q.unit(),
                kind_of(q.unit().dims()),
                kind_of(target.dims())
            ),
            other => said(other),
        }),
        other => match lift(other) {
            // A plain number has no unit to convert from.
            Some(_) => Err(format!(
                "expected a quantity, found {}; write a unit after the number, `5kg`",
                other.kind()
            )),
            None => Err(format!("expected a quantity, found {}", other.kind())),
        },
    }
}

/// Whether a quantity may be held in a column of this type: any quantity in
/// a `Quantity` column, and in a named one a quantity of that dimension.
pub fn admitted(q: &Quantity, id: QId) -> bool {
    id.named().is_none_or(|t| t.admits_quantity(q))
}

/// A quantity as it is shown: the number as a `Scientific` is, then the
/// unit with no space. In full, as a file writes it: `5kg`, and `10.5 ±
/// 0.3kg` with an uncertainty.
pub fn format(q: &Quantity, f: &SciFormat, style: &Style, exact: bool) -> String {
    let unit = q.unit().to_string();
    let unit = if q.unit().is_dimensionless() {
        ""
    } else {
        &unit
    };
    if exact {
        let n = |x: f64| crate::value::format_float(x, true);
        return if q.sigma() == 0.0 {
            format!("{}{unit}", n(q.value()))
        } else {
            format!("{} ± {}{unit}", n(q.value()), n(q.sigma()))
        };
    }
    format!(
        "{}{unit}",
        crate::value::format_scientific(q.value(), q.sigma(), f, style)
    )
}

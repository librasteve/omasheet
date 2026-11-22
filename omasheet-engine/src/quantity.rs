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
use omasheet_omx::types::{QId, S};
use siquant::{Quantity, Registry, Scope, Unit};
use std::cmp::Ordering;
use std::rc::Rc;

fn said(e: siquant::Error) -> String {
    e.to_string()
}

use omasheet_omx::units::kind_of;

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
        Value::Decimal(r) | Value::Percent(r) => Quantity::exact(r.to_f64(), one()),
        Value::Fraction(r) => Quantity::exact(r.to_f64(), one()),
        Value::Float(f) => Quantity::exact(*f, one()),
        Value::Scientific(v, s) => Quantity::new(*v, *s, one()).ok()?,
        // A duration beside a quantity is so many seconds: `9560km / PT13H`.
        Value::Duration(us) => {
            Quantity::exact(*us as f64 / 1e6, Unit::parse("s", Scope::Live).ok()?)
        }
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

/// A quantity of time held in a `Duration` column: a whole number of
/// microseconds, or with an uncertainty, which that cannot hold, a quantity
/// of seconds. An error says what else it measures.
pub fn into_duration(q: &Quantity) -> Result<Value, String> {
    if q.sigma() == 0.0 {
        return as_duration(q);
    }
    let seconds = Unit::parse("s", Scope::Live).map_err(said)?;
    let (v, s, f) = (q.value(), q.sigma(), q.unit().factor());
    if q.unit().dims() != siquant::Dim::of(siquant::Base::Time) {
        return as_duration(q);
    }
    Quantity::new(v * f, s * f, seconds)
        .map(|q| Value::Quantity(Rc::new(q)))
        .map_err(said)
}

/// A `Duration` of `us` microseconds as a column of the format `f` shows it:
/// a number of its unit. `None` if the format names a unit that is not one
/// of time.
pub fn show_duration_in(
    us: i64,
    f: &omasheet_omx::colfmt::QuantityFormat,
    style: &Style,
) -> Option<String> {
    match from_duration(us) {
        Value::Quantity(q) => Some(show_in(&q, f, style)),
        _ => None,
    }
}

/// A `Duration` as the quantity of seconds it stands for: `PT90M` is `5400s`.
pub fn from_duration(us: i64) -> Value {
    match Unit::parse("s", Scope::Live) {
        Ok(unit) => Value::Quantity(Rc::new(Quantity::exact(us as f64 / 1e6, unit))),
        Err(_) => Value::Error,
    }
}

/// A quantity of time, `10s`, as a `Duration` in microseconds. An error says
/// what else it measures.
pub fn as_duration(q: &Quantity) -> Result<Value, String> {
    let time = siquant::Dim::of(siquant::Base::Time);
    if q.unit().dims() != time {
        return Err(format!(
            "a {} is not a length of time: only a quantity of time, as `10s`, \
             can be one",
            kind_of(q.unit().dims())
        ));
    }
    let seconds = q.value() * q.unit().factor();
    let micros = (seconds * 1e6).round();
    if !micros.is_finite() || micros.abs() >= 9.0e18 {
        return Err("the duration is out of range".to_string());
    }
    Ok(Value::Duration(micros as i64))
}

/// `+` and `-` of a quantity of time and a date, a time, a date-time or a
/// `Duration`: the quantity is taken as the `Duration` it stands for, so
/// `17:05:00 + 10s` is `17:05:10`. `None` where that is not what is asked.
pub fn as_span(op: BinOp, a: &Value, b: &Value) -> Option<Result<(Value, Value), String>> {
    if !matches!(op, BinOp::Add | BinOp::Sub) {
        return None;
    }
    let timed = |v: &Value| {
        matches!(
            v,
            Value::Date(_) | Value::Time(_) | Value::Datetime(..) | Value::Duration(_)
        )
    };
    let side = |v: &Value| match v {
        Value::Quantity(q) => as_duration(q),
        other => Ok(other.clone()),
    };
    // A duration with an uncertainty is a quantity of seconds, which can
    // be added to another, but cannot move a date or a time.
    let uncertain = |v: &Value| matches!(v, Value::Quantity(q) if q.sigma() != 0.0);
    match (a, b) {
        (q, t) | (t, q) if uncertain(q) && matches!(t, Value::Duration(_) | Value::Quantity(_)) => {
            None
        }
        (q, _) | (_, q) if uncertain(q) => Some(Err(
            "a duration with an uncertainty cannot move a date or a time, which are exact"
                .to_string(),
        )),
        (Value::Quantity(_), t) | (t, Value::Quantity(_)) if timed(t) => {
            Some(side(a).and_then(|x| side(b).map(|y| (x, y))))
        }
        _ => None,
    }
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
            // A root is the power `1/2`, `1/3` or `1/4`.
            if let Value::Fraction(r) = b
                && let Some((1, n @ 2..=4)) = r.parts()
            {
                return x
                    .root(n as u16)
                    .map(|root| {
                        // A root that is whole, `216000s³` to `60s`, is not
                        // left a hair off it by the floating point.
                        let whole = root.value().round();
                        let exact = whole.powi(n as i32) == x.value();
                        match Quantity::new(whole, root.sigma(), root.unit().clone()) {
                            Ok(snapped) if exact && whole != root.value() => value(snapped),
                            _ => value(root),
                        }
                    })
                    .map_err(|e| explain("take a root of", &x, &y, e));
            }
            let n = match b {
                Value::Int(n, _) => i16::try_from(*n)
                    .map_err(|_| "the power of a quantity is out of range".to_string())?,
                _ => {
                    return Err(format!(
                        "a quantity takes a whole power or a root, `1/2`, `1/3` or `1/4`, but this is {}",
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

/// A duration as the quantity of seconds it is, any other value as it is.
fn timed(v: &Value) -> Value {
    match v {
        Value::Duration(us) => from_duration(*us),
        other => other.clone(),
    }
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
        // A duration is a quantity of time beside one.
        let (a, b) = (timed(a), timed(b));
        let (Value::Quantity(x), Value::Quantity(y)) = (&a, &b) else {
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

/// `a =~= b` where one is a quantity: both in the left unit, equal to within
/// the tolerance of `=~=`, their uncertainties ignored. `None` where neither
/// is a quantity.
pub fn approx(a: &Value, b: &Value) -> Option<Result<Option<bool>, String>> {
    if !matches!(a, Value::Quantity(_)) && !matches!(b, Value::Quantity(_)) {
        return None;
    }
    let (a, b) = (&timed(a), &timed(b));
    let (Value::Quantity(x), Value::Quantity(y)) = (a, b) else {
        return Some(Err(format!("cannot compare {} and {}", a.kind(), b.kind())));
    };
    Some(aligned(x, y).map(|((vx, _), (vy, _))| {
        if vx.is_nan() || vy.is_nan() {
            return Some(false);
        }
        let zero = vx == 0.0 || vy == 0.0;
        Some(crate::value::within_tolerance(
            (vx - vy).abs(),
            (vx.abs(), vy.abs()),
            zero,
        ))
    }))
}

/// An angle of `value` radians, `sigma` uncertain.
pub fn radians(value: f64, sigma: f64) -> Value {
    match Unit::parse("rad", Scope::Live)
        .map_err(said)
        .and_then(|u| Quantity::new(value, sigma, u).map_err(said))
    {
        Ok(q) => Value::Quantity(Rc::new(q)),
        Err(_) => Value::Float(value),
    }
}

/// A function of a quantity: those that keep or halve its unit.
pub fn math(m: MathFn, q: &Quantity) -> Result<Value, String> {
    match m.name() {
        "abs" => Quantity::new(q.value().abs(), q.sigma(), q.unit().clone())
            .map(|q| Value::Quantity(Rc::new(q)))
            .map_err(said),
        "sqrt" => q.root(2).map(value).map_err(said),
        // An angle is a quantity the trigonometric functions take: they
        // work in radians, whatever the angle was written in, and the
        // uncertainty goes through the slope of the function.
        name @ ("sin" | "cos" | "tan")
            if q.unit().dims() == siquant::Dim::of(siquant::Base::Angle) =>
        {
            let rad = Unit::parse("rad", Scope::Live).map_err(said)?;
            let x = q.convert(&rad).map_err(said)?;
            let (v, slope) = match name {
                "sin" => (x.value().sin(), x.value().cos()),
                "cos" => (x.value().cos(), -x.value().sin()),
                _ => (x.value().tan(), 1.0 / x.value().cos().powi(2)),
            };
            let sigma = (slope * x.sigma()).abs();
            Ok(if sigma == 0.0 {
                Value::Float(v)
            } else {
                Value::Scientific(v, sigma)
            })
        }
        // The degrees of an angle, as a number: `degrees(arctan(1))` is 45.
        "degrees" if q.unit().dims() == siquant::Dim::of(siquant::Base::Angle) => {
            let rad = Unit::parse("rad", Scope::Live).map_err(said)?;
            let x = q.convert(&rad).map_err(said)?;
            let (v, s) = (x.value().to_degrees(), x.sigma().to_degrees());
            Ok(if s == 0.0 {
                Value::Float(v)
            } else {
                Value::Scientific(v, s)
            })
        }
        name @ ("sin" | "cos" | "tan") => Err(format!(
            "`{name}` takes an angle, such as `30°` or `1rad`, but this is a quantity in {} ({})",
            q.unit(),
            kind_of(q.unit().dims())
        )),
        name => Err(format!(
            "`{name}` needs a number with no unit, but this is a quantity in {}; \
             `abs` and `sqrt` take one",
            q.unit()
        )),
    }
}

/// `dms(x, places)`: an angle as text in degrees, minutes and seconds.
pub fn dms(v: &Value, places: usize) -> Result<Value, String> {
    match v {
        Value::Null => Ok(Value::Null),
        Value::Quantity(q) => q
            .to_dms(places)
            .map(|t| Value::text(&t))
            .map_err(|e| match e {
                siquant::Error::DimensionMismatch { .. } => format!(
                    "`dms` needs an angle, but this is a quantity in {} ({})",
                    q.unit(),
                    kind_of(q.unit().dims())
                ),
                other => said(other),
            }),
        other => Err(format!("`dms` needs an angle, found {}", other.kind())),
    }
}

/// `unit(x)` as text, or `value(x)` as a `Scientific`: the two halves of a
/// quantity.
pub fn part(v: &Value, unit: bool) -> Result<Value, String> {
    match v {
        Value::Null => Ok(Value::Null),
        Value::Quantity(q) if unit => Ok(Value::text(&q.unit().to_string())),
        Value::Quantity(q) => Ok(Value::Scientific(q.value(), q.sigma())),
        other => Err(format!(
            "`{}` needs a quantity, found {}",
            if unit { "unit" } else { "value" },
            other.kind()
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

/// `Length(x)`, `Length(x, 'km')`, `Duration(x, 'ms')`: a value as a quantity
/// kind or a `Duration`. A number, or text that reads as one (`'5km'`), is
/// taken in `unit`, or the kind's own unit (seconds for a `Duration`); a
/// quantity is converted to `unit` if it is given and is otherwise kept in the
/// unit it has. A quantity of another dimension is an error.
pub fn kind(v: &Value, to: S, unit: Option<&Unit>) -> Result<Value, String> {
    let v = match v {
        Value::Null => return Ok(Value::Null),
        Value::String(text, _) => {
            let text = text.trim();
            match omasheet_omx::convert::parse_literal_with(text, true) {
                Some(lit) => Value::from_lit(&lit),
                None => return Err(format!("`{text}` is not a number or a quantity")),
            }
        }
        other => other.clone(),
    };
    match to {
        S::Duration => match &v {
            Value::Duration(_) => Ok(v),
            Value::Quantity(q) => into_duration(q),
            number => {
                let n = lift(number)
                    .filter(|_| !matches!(number, Value::Duration(_)))
                    .ok_or_else(|| format!("{} cannot be made a `Duration`", number.kind()))?;
                let unit = match unit {
                    Some(u) => u.clone(),
                    None => Unit::parse("s", Scope::Live).map_err(said)?,
                };
                let q = Quantity::new(n.value(), n.sigma(), unit).map_err(said)?;
                into_duration(&q)
            }
        },
        S::Quantity(id) => {
            let v = match v {
                Value::Duration(us) => from_duration(us),
                other => other,
            };
            let already = matches!(v, Value::Quantity(_));
            if let (Some(u), Some(t)) = (unit, id.named())
                && u.dims() != t.dims()
            {
                return Err(format!("`{u}` is not a unit of {}", id.name()));
            }
            // A quantity keeps its own unit unless one is asked for.
            let column = match (unit, already) {
                (Some(u), _) => Some(u.clone()),
                (None, true) => None,
                (None, false) => column_unit(id),
            };
            match dress(v, id, column.as_ref())? {
                out @ Value::Quantity(_) => Ok(out),
                other => Err(format!(
                    "{} needs a unit to be a `{}`: write `{}(x, 'unit')`",
                    other.kind(),
                    id.name(),
                    id.name()
                )),
            }
        }
        other => Err(format!("`{other}` takes no unit")),
    }
}

/// The unit named by text, as `'km'`.
pub fn unit_named(v: &Value) -> Result<Unit, String> {
    let Value::String(text, _) = v else {
        return Err(format!("expected the name of a unit, found {}", v.kind()));
    };
    Unit::parse(text, Scope::Any).map_err(|e| format!("`{text}` is not a unit: {e}"))
}

/// `x.type()` of a quantity: its kind, or every kind its dimension has.
pub fn type_name(q: &Quantity) -> String {
    omasheet_omx::units::kinds_of(q.unit().dims())
}

/// Whether a quantity may be held in a column of this type: any quantity in
/// a `Quantity` column, and in a named one a quantity of that dimension.
pub fn admitted(q: &Quantity, id: QId) -> bool {
    id.named().is_none_or(|t| t.admits_quantity(q))
}

/// The unit a column of this type holds its values in, unless it says
/// another: the preferred unit of a named type, none for a `Quantity`.
pub fn column_unit(id: QId) -> Option<Unit> {
    omasheet_omx::units::preferred(id)
}

/// A value as a column of type `id` holds it, in the column's `unit`: a
/// quantity is converted to it, and a bare number is read in it (D233). A
/// quantity of the wrong dimension is an error. Anything else is left to be
/// refused by the column.
pub fn dress(v: Value, id: QId, column: Option<&Unit>) -> Result<Value, String> {
    let (value, sigma, unit) = match &v {
        Value::Quantity(q) => (q.value(), q.sigma(), Some(q.unit().clone())),
        number => match number {
            Value::Int(..)
            | Value::Decimal(_)
            | Value::Fraction(_)
            | Value::Percent(_)
            | Value::Float(_)
            | Value::Scientific(..) => match lift(number) {
                Some(n) => (n.value(), n.sigma(), None),
                None => return Ok(v),
            },
            _ => return Ok(v),
        },
    };
    match omasheet_omx::units::dress(value, sigma, unit.as_ref(), id, column)? {
        Some((value, sigma, unit)) => Quantity::new(value, sigma, unit)
            .map(|q| Value::Quantity(Rc::new(q)))
            .map_err(said),
        None => Ok(v),
    }
}

/// A quantity as its column's format shows it: in the format's unit, if it
/// can be, and its number as the format says.
pub fn show_in(q: &Quantity, f: &omasheet_omx::colfmt::QuantityFormat, style: &Style) -> String {
    let (converted, fits) = match &f.unit {
        Some(unit) => match q.convert(unit) {
            Ok(c) => (c, true),
            Err(_) => (q.clone(), false),
        },
        None => (q.clone(), false),
    };
    let mut number = f.number.unwrap_or_default();
    let converted = match &f.unit {
        Some(base) if fits && number.prefixed => with_best_prefix(&converted, base),
        _ => converted,
    };
    if let Some(code) = currency_code(&converted) {
        // Money is shown to the places it is spent in, unless the format says.
        if number.places.is_none()
            && number.conv == omasheet_omx::colfmt::SciConv::General
            && !number.plus_minus
            && !number.value_only
            && converted.sigma() == 0.0
        {
            number.conv = omasheet_omx::colfmt::SciConv::Fixed;
            number.places = Some(money_places(&code));
        }
        if number.sign {
            return render_money(&converted, &code, &number, style);
        }
    }
    // The unit is in the column's head where the column has one, unless a
    // value cannot be put in it, which keeps its own, so it is not mistaken.
    let head = fits && f.lifts(false);
    render(&converted, &number, style, false, !head)
}

/// `q` in `base` with the prefix, a power of a thousand, that gives it the
/// shortest form: `1500g` is `1.5kg` and `0.002m` is `2mm`. Zero and what
/// has no number stay in `base`, and a value past the prefixes takes the
/// last of them.
fn with_best_prefix(q: &Quantity, base: &Unit) -> Quantity {
    let Ok(in_base) = q.convert(base) else {
        return q.clone();
    };
    let v = in_base.value();
    if v == 0.0 || !v.is_finite() {
        return in_base;
    }
    // Data, the bit and the byte, takes the binary prefixes, powers of 1024,
    // `1536B` as `1.5KiB`; any other unit the powers of 1000.
    let binary = matches!(base.to_string().as_str(), "B" | "b");
    let (radix, per): (f64, i32) = if binary { (2.0, 10) } else { (10.0, 3) };
    let steps: Vec<_> = Registry::builtin()
        .prefixes()
        .filter(|p| p.base == if binary { 2 } else { 10 } && p.exp != 0 && i32::from(p.exp) % per == 0)
        .map(|p| (i32::from(p.exp), p.symbol.to_string()))
        .collect();
    let lowest = steps.iter().map(|s| s.0).min().unwrap_or(0).min(0);
    let highest = steps.iter().map(|s| s.0).max().unwrap_or(0).max(0);
    let want = ((v.abs().log(radix) / f64::from(per)).floor() as i32 * per).clamp(lowest, highest);
    let Some((_, symbol)) = steps.iter().find(|s| s.0 == want) else {
        return in_base;
    };
    Unit::parse(&format!("{}{base}", symbol), Scope::Any)
        .and_then(|u| in_base.convert(&u))
        .unwrap_or(in_base)
}

/// The code of a quantity of money in one currency, `EUR`: not a price a
/// kilogram, and not a quantity of anything else.
fn currency_code(q: &Quantity) -> Option<String> {
    let code = q.unit().to_string();
    if code.contains(['.', '^']) {
        return None;
    }
    Registry::builtin().rate(&code).map(|_| code)
}

/// How many places an amount of a currency is shown to by default: the
/// least that shows a hundredth of a dollar's worth, so two for the dollar,
/// the euro and the yen, and seven for a bitcoin; two at the least and eight
/// at the most.
pub fn money_places(code: &str) -> u32 {
    let dollars = Registry::builtin().rate(code).map_or(1.0, |r| 1.0 / r);
    (100.0 * dollars).log10().round().clamp(2.0, 8.0) as u32
}

/// An amount of money by its sign, `€4.55` and `-$5.00`, or by its code
/// after the number where it has no sign, `4.55CHF`.
fn render_money(q: &Quantity, code: &str, f: &SciFormat, style: &Style) -> String {
    let mut number = crate::value::format_scientific(q.value(), q.sigma(), f, style);
    if q.sigma() == 0.0 && !f.plus_minus {
        number = number.replacen("(0)", "", 1);
    }
    let gap = if f.gap { " " } else { "" };
    match Registry::builtin().currency_sign(code) {
        Some(sign) if f.sign_after => format!("{number}{gap}{sign}"),
        Some(sign) => match number.strip_prefix('-') {
            Some(digits) => format!("-{sign}{digits}"),
            None => format!("{sign}{number}"),
        },
        None => format!("{number}{gap}{code}"),
    }
}

/// A quantity as it is shown: the number as a `Scientific` is, then the
/// unit with no space. In full, as a file writes it: `5kg`, and `10.5 ±
/// 0.3kg` with an uncertainty.
pub fn format(q: &Quantity, f: &SciFormat, style: &Style, exact: bool) -> String {
    render(q, f, style, exact, true)
}

/// [`format`], with or without the unit after the number.
fn render(q: &Quantity, f: &SciFormat, style: &Style, exact: bool, with_unit: bool) -> String {
    let unit = q.unit().to_string();
    // Shown as the SI recommends, `m⋅s⁻¹`, unless `^` or the full form asks
    // for the notation the parser reads, `m.s^-1`.
    let unit = if exact || f.ascii {
        unit
    } else {
        omasheet_omx::colfmt::pretty_unit(&unit)
    };
    let unit = if q.unit().is_dimensionless() || !with_unit {
        String::new()
    } else if f.gap {
        // `_`: the unit set apart from its number, `6.022e+23 mol^-1`.
        format!(" {unit}")
    } else {
        unit
    };
    let unit = unit.as_str();
    if exact {
        let n = |x: f64| crate::value::format_float(x, true);
        return if q.sigma() == 0.0 {
            format!("{}{unit}", n(q.value()))
        } else {
            format!("{} ± {}{unit}", n(q.value()), n(q.sigma()))
        };
    }
    // With no uncertainty and no format, the number is shown as a `Float` is:
    // to three places, cut and marked, `-1.571rad·`.
    if q.sigma() == 0.0
        && (SciFormat {
            gap: false,
            ascii: false,
            unit_down: false,
            ..*f
        }) == SciFormat::default()
    {
        let number = crate::value::format_value(&Value::Float(q.value()), false, style, false);
        // The mark of a cut number goes last, after the unit, where a
        // display can set it apart: `1.571rad·`.
        // It follows an exponent, `6.022e+23·`, and goes after the unit all
        // the same, `6.022e+23mol^-1·`.
        return if number.contains(crate::value::MORE) {
            let digits = number.replace(crate::value::MORE, "");
            format!("{digits}{unit}{}", crate::value::MORE)
        } else {
            format!("{number}{unit}")
        };
    }
    let mut number = crate::value::format_scientific(q.value(), q.sigma(), f, style);
    // A quantity that has no uncertainty is not shown with `(0)` after the
    // number because a format asked for places: `218.3m.s^-1`, not
    // `218.3(0)m.s^-1`. `±` asks for it, as `218.3 ± 0.0`.
    if q.sigma() == 0.0 && !f.plus_minus {
        number = number.replacen("(0)", "", 1);
    }
    // The mark of a cut number goes last, after the unit, on the right.
    if number.contains(crate::value::MORE) {
        let digits = number.replace(crate::value::MORE, "");
        return format!("{digits}{unit}{}", crate::value::MORE);
    }
    format!("{number}{unit}")
}

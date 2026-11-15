// Copyright (c) 2026 Stephen Roe

//! The window functions: `cum_sum`, `diff`, `rank`, `rolling_mean` and the
//! rest, which give each row of a column a value read from the rows around
//! it. The column is cut into partitions (`by Account`), each partition is
//! put in the order of the order key (`Date`) with the file's order breaking
//! ties, the function is run down it, and every answer goes back on the row
//! it came from.

use std::cmp::Ordering;

use omasheet_omx::ast::BinOp;
use omasheet_omx::ir::{Spec, Ties, Win};

use crate::value::{Value, arith, compare, sort_order, to_float};

/// What `spec` gives for each of `values`. `n` is the lag or the window
/// length, `order` the order key (as long as `values`, or empty for the
/// file's order) and `parts` the partition keys.
pub fn run(
    spec: Spec,
    values: &[Value],
    n: i64,
    order: &[Value],
    parts: &[Vec<Value>],
) -> Result<Vec<Value>, String> {
    let len = values.len();
    for key in parts.iter().chain(std::iter::once(&order.to_vec())) {
        if !key.is_empty() && key.len() != len {
            return Err(
                "the order key and the partition columns must be as long as the column".into(),
            );
        }
    }
    let by_part = |a: usize, b: usize| -> Ordering {
        for key in parts {
            let o = sort_order(&key[a], &key[b]).unwrap_or(Ordering::Equal);
            if o != Ordering::Equal {
                return o;
            }
        }
        Ordering::Equal
    };
    let mut rows: Vec<usize> = (0..len).collect();
    // A stable sort: rows alike in key stay in the file's order.
    rows.sort_by(|&a, &b| {
        by_part(a, b).then_with(|| {
            if order.is_empty() {
                Ordering::Equal
            } else {
                sort_order(&order[a], &order[b]).unwrap_or(Ordering::Equal)
            }
        })
    });
    let mut out = vec![Value::Null; len];
    let mut start = 0;
    while start < rows.len() {
        let mut end = start + 1;
        while end < rows.len() && by_part(rows[start], rows[end]) == Ordering::Equal {
            end += 1;
        }
        let at = &rows[start..end];
        let seq: Vec<&Value> = at.iter().map(|&i| &values[i]).collect();
        let done = down(spec, &seq, n)?;
        for (&i, v) in at.iter().zip(done) {
            out[i] = v;
        }
        start = end;
    }
    Ok(out)
}

fn null(v: &Value) -> bool {
    matches!(v, Value::Null)
}

/// One partition, in order.
fn down(spec: Spec, xs: &[&Value], n: i64) -> Result<Vec<Value>, String> {
    let len = xs.len();
    Ok(match spec.win {
        Win::CumSum | Win::CumProd => {
            let op = if spec.win == Win::CumSum {
                BinOp::Add
            } else {
                BinOp::Mul
            };
            let mut acc: Option<Value> = None;
            let mut out = Vec::with_capacity(len);
            for x in xs {
                if null(x) {
                    out.push(Value::Null);
                    continue;
                }
                acc = Some(match acc {
                    None => (*x).clone(),
                    Some(a) => arith(op, &a, x)?,
                });
                out.push(acc.clone().unwrap_or(Value::Null));
            }
            out
        }
        Win::CumMin | Win::CumMax => {
            let want = if spec.win == Win::CumMin {
                Ordering::Less
            } else {
                Ordering::Greater
            };
            let mut acc: Option<Value> = None;
            let mut out = Vec::with_capacity(len);
            for x in xs {
                if null(x) {
                    out.push(Value::Null);
                    continue;
                }
                acc = Some(match acc {
                    None => (*x).clone(),
                    Some(a) => {
                        if compare(x, &a)? == Some(want) {
                            (*x).clone()
                        } else {
                            a
                        }
                    }
                });
                out.push(acc.clone().unwrap_or(Value::Null));
            }
            out
        }
        Win::CumCount => {
            let mut seen = 0;
            xs.iter()
                .map(|x| {
                    if !null(x) {
                        seen += 1;
                    }
                    Value::int(seen)
                })
                .collect()
        }
        Win::Diff | Win::PctChange => {
            let lag = usize::try_from(n).map_err(|_| "the lag must be a positive whole number")?;
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                if i < lag || null(xs[i]) || null(xs[i - lag]) {
                    out.push(Value::Null);
                } else if spec.win == Win::Diff {
                    out.push(arith(BinOp::Sub, xs[i], xs[i - lag])?);
                } else {
                    let (a, b) = to_float(xs[i])
                        .zip(to_float(xs[i - lag]))
                        .ok_or("`pct_change` needs real numbers")?;
                    out.push(Value::Float(a / b - 1.0));
                }
            }
            out
        }
        Win::ForwardFill | Win::BackwardFill => {
            let mut out: Vec<Value> = xs.iter().map(|x| (*x).clone()).collect();
            let order: Vec<usize> = if spec.win == Win::ForwardFill {
                (0..len).collect()
            } else {
                (0..len).rev().collect()
            };
            let mut last: Option<Value> = None;
            for i in order {
                if null(&out[i]) {
                    if let Some(l) = &last {
                        out[i] = l.clone();
                    }
                } else {
                    last = Some(out[i].clone());
                }
            }
            out
        }
        Win::Rank(ties) => rank(xs, ties, spec.desc)?,
        Win::RollingSum
        | Win::RollingMean
        | Win::RollingMin
        | Win::RollingMax
        | Win::RollingStd => {
            let w = usize::try_from(n)
                .ok()
                .filter(|w| *w >= 1)
                .ok_or("the window must be a positive whole number")?;
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                if i + 1 < w {
                    out.push(Value::Null);
                    continue;
                }
                let win = &xs[i + 1 - w..=i];
                if win.iter().any(|x| null(x)) {
                    out.push(Value::Null);
                    continue;
                }
                out.push(rolled(spec.win, win)?);
            }
            out
        }
    })
}

fn rolled(win: Win, xs: &[&Value]) -> Result<Value, String> {
    let sum = || -> Result<Value, String> {
        let mut acc = xs[0].clone();
        for x in &xs[1..] {
            acc = arith(BinOp::Add, &acc, x)?;
        }
        Ok(acc)
    };
    Ok(match win {
        Win::RollingSum => sum()?,
        Win::RollingMean => crate::eval::average(&sum()?, xs.len() as i64)?,
        Win::RollingMin | Win::RollingMax => {
            let want = if win == Win::RollingMin {
                Ordering::Less
            } else {
                Ordering::Greater
            };
            let mut best = xs[0];
            for x in &xs[1..] {
                if compare(x, best)? == Some(want) {
                    best = x;
                }
            }
            best.clone()
        }
        _ => {
            // The standard deviation of a sample, as Polars' default.
            if xs.len() < 2 {
                return Ok(Value::Null);
            }
            let fs: Vec<f64> = xs
                .iter()
                .map(|x| to_float(x).ok_or("`rolling_std` needs real numbers"))
                .collect::<Result<_, _>>()?;
            let mean = fs.iter().sum::<f64>() / fs.len() as f64;
            let var =
                fs.iter().map(|f| (f - mean) * (f - mean)).sum::<f64>() / (fs.len() - 1) as f64;
            Value::Float(var.sqrt())
        }
    })
}

fn rank(xs: &[&Value], ties: Ties, desc: bool) -> Result<Vec<Value>, String> {
    let mut at: Vec<usize> = (0..xs.len()).filter(|&i| !null(xs[i])).collect();
    let cmp = |a: &usize, b: &usize| {
        let o = sort_order(xs[*a], xs[*b]).unwrap_or(Ordering::Equal);
        if desc { o.reverse() } else { o }
    };
    at.sort_by(cmp);
    let mut out = vec![Value::Null; xs.len()];
    let mut start = 0;
    let mut dense = 0;
    while start < at.len() {
        let mut end = start + 1;
        while end < at.len() && cmp(&at[start], &at[end]).is_eq() {
            end += 1;
        }
        dense += 1;
        for (k, &i) in at[start..end].iter().enumerate() {
            out[i] = match ties {
                Ties::Average => Value::Float((start + 1 + end) as f64 / 2.0),
                Ties::Min => Value::int(start as i64 + 1),
                Ties::Max => Value::int(end as i64),
                Ties::Dense => Value::int(dense),
                Ties::Ordinal => Value::int((start + k) as i64 + 1),
            };
        }
        start = end;
    }
    Ok(out)
}

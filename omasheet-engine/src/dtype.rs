// Copyright (c) 2026 Stephen Roe

//! The boundary between an omasheet type and a Polars dtype: an omasheet
//! type is a name for one (`add-polars-dtypes`, D14), so this says which,
//! both ways, and converts one value across.

use crate::Value;
use omasheet_omx::ast::Lit;
use omasheet_omx::check::Cell;
use omasheet_omx::date::{DAY, MICROS, NANOS};
use omasheet_omx::decimal::{Dec, MAX_PRECISION};
use omasheet_omx::types::S;
use polars::prelude::Column as PlColumn;
use polars::prelude::*;

/// The omasheet type a Polars dtype is named by, or `None` for a shape a
/// sheet has no room for: a list, a struct, a map.
pub fn type_of(dtype: &DataType) -> Option<S> {
    Some(match dtype {
        DataType::Boolean => S::Boolean,
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => S::Int,
        DataType::Float32 | DataType::Float64 => S::Float,
        DataType::Decimal(..) => S::Decimal,
        DataType::String => S::String,
        DataType::Date => S::Date,
        DataType::Time => S::Time,
        DataType::Datetime(..) => S::DateTime,
        DataType::Null => S::Any,
        _ => return None,
    })
}

/// One value of a frame as a cell. The column's own type decides how an
/// exact number is held, so that a `Percent` column holds percentages and a
/// `Fraction` column fractions, as a sheet of text does.
pub fn cell_of(value: AnyValue<'_>, ty: S) -> Cell {
    let exact = |d: Dec| match ty {
        S::Percent => Cell::Lit(Lit::Percent(d)),
        _ => Cell::Lit(Lit::Decimal(d)),
    };
    let whole = |n: i64| match ty {
        S::Decimal | S::Fraction | S::Percent => exact(Dec::from_i64(n)),
        _ => Cell::Lit(Lit::Int(n)),
    };
    match value {
        AnyValue::Null => Cell::Empty,
        AnyValue::Boolean(b) => Cell::Lit(Lit::Boolean(b)),
        AnyValue::String(s) => Cell::Lit(Lit::String(s.to_string())),
        AnyValue::StringOwned(s) => Cell::Lit(Lit::String(s.to_string())),
        AnyValue::Int8(n) => whole(n as i64),
        AnyValue::Int16(n) => whole(n as i64),
        AnyValue::Int32(n) => whole(n as i64),
        AnyValue::Int64(n) => whole(n),
        AnyValue::UInt8(n) => whole(n as i64),
        AnyValue::UInt16(n) => whole(n as i64),
        AnyValue::UInt32(n) => whole(n as i64),
        // A `u64` past an `i64` is still exact as a decimal.
        AnyValue::UInt64(n) => match i64::try_from(n) {
            Ok(n) => whole(n),
            Err(_) => Dec::new(n as i128, 0).map_or(Cell::Invalid, exact),
        },
        AnyValue::Float32(f) => Cell::Lit(Lit::Float(f as f64)),
        AnyValue::Float64(f) => Cell::Lit(Lit::Float(f)),
        AnyValue::Decimal(unscaled, scale) => {
            Dec::new(unscaled, scale as i32).map_or(Cell::Invalid, exact)
        }
        AnyValue::Date(days) => Cell::Lit(Lit::Date(days)),
        // Still read to the whole second, as before the units changed: no
        // literal can write a fraction of one yet, so a value holding one
        // would show as something the file could not write back.
        AnyValue::Datetime(n, unit, _) | AnyValue::DatetimeOwned(n, unit, _) => {
            let secs = match unit {
                TimeUnit::Nanoseconds => n.div_euclid(NANOS),
                TimeUnit::Microseconds => n.div_euclid(MICROS),
                TimeUnit::Milliseconds => n.div_euclid(1_000),
            };
            match (
                i32::try_from(secs.div_euclid(DAY)),
                secs.checked_mul(MICROS),
            ) {
                (Ok(_), Some(micros)) => Cell::Lit(Lit::DateTime(micros)),
                _ => Cell::Invalid,
            }
        }
        AnyValue::Time(ns) => Cell::Lit(Lit::Time(ns.div_euclid(NANOS) * NANOS)),
        // Anything else is held as Polars shows it.
        other => Cell::Lit(Lit::String(other.to_string())),
    }
}

/// The Polars dtype an omasheet type is held as, with the scale an exact
/// number is held at. A percentage and a fraction are an exact number in a
/// dress rather than a dtype of their own (D69), so each is the decimal its
/// values are and [`cell_of`] puts the dress back on. A web address, an email
/// address and a UUID are text in a dress the same way. `None` only for a
/// column of anything, which no one dtype holds.
pub fn dtype(ty: S, scale: u32) -> Option<DataType> {
    Some(match ty {
        S::Int => DataType::Int64,
        S::Decimal | S::Fraction | S::Percent => decimal(scale),
        S::Float => DataType::Float64,
        S::String | S::Url | S::Email | S::Uuid => DataType::String,
        S::Boolean => DataType::Boolean,
        S::Date => DataType::Date,
        S::Time => DataType::Time,
        S::DateTime => DataType::Datetime(TimeUnit::Microseconds, None),
        S::Any => return None,
    })
}

/// `Decimal(38, scale)`: the dtype a `Dec` is, since a `Dec` was built to be
/// one. The precision is always the full 38, which is what a Parquet write
/// and the schema in the text agree on (D72a).
pub fn decimal(scale: u32) -> DataType {
    DataType::Decimal(Some(MAX_PRECISION as usize), Some(scale as usize))
}

/// A column's calculated values as the Polars column its type names, or
/// `None` where a value does not fit that type.
///
/// The twin of [`cell_of`], and the lossless one: where
/// `omasheet-interop`'s Parquet writer puts a null in place of a value too
/// wide for the column and says so as a loss, this gives nothing at all.
/// Its caller is deciding whether to lower a calculation, and a calculation
/// that would quietly drop a value is one to leave on the engine.
pub fn column_of(name: &str, ty: S, values: &[Value]) -> Option<PlColumn> {
    let name: PlSmallStr = name.into();
    let exact = |v: &Value| match v {
        Value::Int(n) => Some(Dec::from_i64(*n)),
        Value::Decimal(d) => Some(*d),
        _ => None,
    };
    // Every value or none of it: `collect` over an `Option` stops at the
    // first value the type has no room for.
    let all = |f: &dyn Fn(&Value) -> Option<Option<i64>>| -> Option<Vec<Option<i64>>> {
        values.iter().map(f).collect()
    };
    let null = |v: &Value| matches!(v, Value::Null);
    // The column as a `Decimal(38, s)`, `pick` saying which values it is
    // willing to read: one scale for the whole column, as a Polars decimal
    // has, which is the widest any value needs. A value that cannot be
    // written at that scale is not written at all, so the column is
    // declined.
    let decimals = |name: PlSmallStr, pick: &dyn Fn(&Value) -> Option<Dec>| -> Option<Series> {
        // A Polars decimal names a scale of `0..=38` and no other, so a
        // value whose point sits outside that is one this column cannot
        // hold: the scale is clamped, and the rescale below then declines.
        let scale = values
            .iter()
            .filter_map(pick)
            .map(|d| d.scale())
            .max()
            .unwrap_or(0)
            .clamp(0, MAX_PRECISION as i32);
        let digits: Vec<Option<i128>> = values
            .iter()
            .map(|v| match v {
                v if null(v) => Some(None),
                v => {
                    let d = pick(v)?;
                    let at = d.rescale(scale)?;
                    // A value rounded away to nothing is a value lost, and
                    // this column is the lossless one.
                    if at.is_zero() && !d.is_zero() {
                        return None;
                    }
                    Some(Some(at.unscaled()))
                }
            })
            .collect::<Option<_>>()?;
        // The values are unscaled digits already, so the series is built as
        // a decimal rather than cast to one: casting an integer to a
        // decimal scales it, which is not what these are.
        let ca: Int128Chunked = digits.into_iter().collect_ca(name);
        Some(
            ca.into_decimal_unchecked(Some(MAX_PRECISION as usize), scale as usize)
                .into_series(),
        )
    };
    let series = match ty {
        S::Int => Series::new(
            name,
            all(&|v| match v {
                Value::Int(n) => Some(Some(*n)),
                v if null(v) => Some(None),
                _ => None,
            })?,
        ),
        S::Decimal => decimals(name.clone(), &exact)?,
        // A percentage and a fraction are an exact number in a dress, and
        // the dress is display (D69): `cell_of` puts it back on from the
        // column's own type, which is what a table's column declared as one
        // already relies on. So such a column is held as the
        // `Decimal(38, s)` its values already are. Only the matching dress
        // is read — a bare number among them would come back wearing one,
        // which is not what the sheet of text says.
        S::Percent => decimals(name.clone(), &|v| match v {
            Value::Percent(d) => Some(*d),
            _ => None,
        })?,
        S::Fraction => decimals(name.clone(), &|v| match v {
            Value::Fraction(d) => Some(*d),
            _ => None,
        })?,
        S::Float => {
            let v: Vec<Option<f64>> = values
                .iter()
                .map(|v| match v {
                    Value::Float(f) => Some(Some(*f)),
                    v if null(v) => Some(None),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            Series::new(name, v)
        }
        S::String | S::Url | S::Email | S::Uuid => {
            let v: Vec<Option<&str>> = values
                .iter()
                .map(|v| match v {
                    Value::String(s, _) => Some(Some(&**s)),
                    v if null(v) => Some(None),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            Series::new(name, v)
        }
        S::Boolean => {
            let v: Vec<Option<bool>> = values
                .iter()
                .map(|v| match v {
                    Value::Boolean(b) => Some(Some(*b)),
                    v if null(v) => Some(None),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            Series::new(name, v)
        }
        S::Date => {
            let v: Vec<Option<i32>> = values
                .iter()
                .map(|v| match v {
                    Value::Date(d) => Some(Some(*d)),
                    v if null(v) => Some(None),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            Series::new(name, v).cast(&DataType::Date).ok()?
        }
        // A `Time` and a `DateTime` are held in the units Polars holds them
        // in, so neither is scaled on the way across.
        S::Time => Series::new(
            name,
            all(&|v| match v {
                Value::Time(ns) => Some(Some(*ns)),
                v if null(v) => Some(None),
                _ => None,
            })?,
        )
        .cast(&DataType::Time)
        .ok()?,
        S::DateTime => Series::new(
            name,
            all(&|v| match v {
                Value::DateTime(us) => Some(Some(*us)),
                v if null(v) => Some(None),
                _ => None,
            })?,
        )
        .cast(&DataType::Datetime(TimeUnit::Microseconds, None))
        .ok()?,
        // A column of anything: there is no one dtype to hold it as.
        S::Any => return None,
    };
    Some(series.into_column())
}

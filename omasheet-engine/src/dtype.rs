// Copyright (c) 2026 Stephen Roe

//! The boundary between an omasheet type and a Polars dtype: an omasheet
//! type is a name for one (`add-polars-dtypes`, D14), so this says which,
//! both ways, and converts one value across.

use omasheet_omx::ast::Lit;
use omasheet_omx::check::Cell;
use omasheet_omx::date::{DAY, MICROS, NANOS};
use omasheet_omx::decimal::Dec;
use omasheet_omx::types::S;
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
            Dec::new(unscaled, scale as u32).map_or(Cell::Invalid, exact)
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

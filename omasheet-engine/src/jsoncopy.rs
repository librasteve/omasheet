// Copyright (c) 2026 Stephen Roe

//! A JSON file turned into a Parquet copy, streamed (`open-a-big-csv` D152,
//! D153): the rows of a JSON file are read one at a time, never the file
//! whole, and written a row group at a time, so memory stays what a batch of
//! rows takes whatever the file's size.
//!
//! The shapes are those `omasheet import` reads: a list of rows, each an
//! object of values by column name or each a list of values of which the
//! first is the names; or an object of such lists, each a sheet called by
//! its key. A source names one of them with `file.json#Key`, and a bare
//! `file.json` is the first.
//!
//! It takes two passes, so that every row has been looked at before any is
//! written: the first works out each column's type from every value in it,
//! and the second writes. A column of whole numbers is `Int64`; any other
//! numbers, `Float64`; `true` and `false`, `Boolean`; a string that is an
//! ISO date or date-time in every cell, `Date` or `Datetime`; anything
//! else, text — a number among strings as it was written, a list or an
//! object as the JSON of it. A key a row has not got is a null. Numbers
//! are floats here where `omasheet import` makes them exact decimals, as
//! Polars reads a CSV's.

use crate::copyframe::{self, Cell, Names, RowBuf, RowSource, Shape};
#[cfg(test)]
use crate::copyframe::{BATCH, HEAD_ROWS};
use omasheet_omx::ast::Lit;
use omasheet_omx::convert::parse_literal;
#[cfg(test)]
use polars::prelude::*;
use serde::de::{DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::Value as Json;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// What the readers say to stop early: not a failure.
const STOP: &str = "omasheet: enough rows";

const SHAPE: &str = "it is not a list of rows, or an object holding one for each sheet by its name";

/// A string, as the cell it is: a date or a date and time where it is one.
fn text_cell(s: &str) -> Cell {
    match when(s) {
        Some(Lit::Date(days)) => Cell::Date(days),
        Some(Lit::Datetime(us)) => Cell::Datetime(us),
        _ => Cell::Str(s.to_string()),
    }
}

/// A number written as `raw`, as the cell it is.
fn number_cell(raw: &str) -> Cell {
    if !raw.contains(['.', 'e', 'E'])
        && let Ok(i) = raw.parse::<i64>()
    {
        return Cell::Int(i);
    }
    match raw.parse::<f64>() {
        Ok(f) if f.is_finite() => Cell::Float(f),
        _ => Cell::Number(raw.to_string()),
    }
}

/// serde_json's name for the key under which it hands a number, when it
/// keeps the digits as written (`arbitrary_precision`). A test holds it.
const NUMBER_TOKEN: &str = "$serde_json::private::Number";

/// A string read as a date or a date and time, where it is one. A cheap
/// look first, since most strings are neither.
fn when(s: &str) -> Option<Lit> {
    let s = s.trim();
    if !(8..=40).contains(&s.len()) || !s.as_bytes()[0].is_ascii_digit() || !s.contains('-') {
        return None;
    }
    // The plain `2024-03-05`, which is nearly every date, without the
    // general reading of a literal.
    if let Some(days) = iso_date(s) {
        return Some(Lit::Date(days));
    }
    match parse_literal(s) {
        Some(lit @ (Lit::Date(_) | Lit::Datetime(_))) => Some(lit),
        _ => None,
    }
}

/// `YYYY-MM-DD`, for a year from 1900 to 2200, as days since 1970-01-01;
/// anything else, or a date there is not, is left to the general reading.
fn iso_date(s: &str) -> Option<i32> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let digits = |from: usize, to: usize| -> Option<i64> {
        b[from..to].iter().try_fold(0i64, |n, c| {
            c.is_ascii_digit().then(|| n * 10 + i64::from(c - b'0'))
        })
    };
    let (y, m, d) = (digits(0, 4)?, digits(5, 7)?, digits(8, 10)?);
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let last = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1900..=2200).contains(&y) || d < 1 || d > last {
        return None;
    }
    // Howard Hinnant's days-from-civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i32::try_from(era * 146_097 + doe - 719_468).ok()
}

/// Reads one value as a [`Cell`].
struct CellSeed;

impl<'de> DeserializeSeed<'de> for CellSeed {
    type Value = Cell;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Cell, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for CellSeed {
    type Value = Cell;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Cell, E> {
        Ok(Cell::Null)
    }

    fn visit_none<E>(self) -> Result<Cell, E> {
        Ok(Cell::Null)
    }

    fn visit_bool<E>(self, b: bool) -> Result<Cell, E> {
        Ok(Cell::Bool(b))
    }

    fn visit_i64<E>(self, i: i64) -> Result<Cell, E> {
        Ok(Cell::Int(i))
    }

    fn visit_u64<E>(self, u: u64) -> Result<Cell, E> {
        Ok(match i64::try_from(u) {
            Ok(i) => Cell::Int(i),
            Err(_) => Cell::Float(u as f64),
        })
    }

    fn visit_f64<E>(self, f: f64) -> Result<Cell, E> {
        Ok(Cell::Float(f))
    }

    fn visit_str<E>(self, s: &str) -> Result<Cell, E> {
        Ok(text_cell(s))
    }

    fn visit_string<E>(self, s: String) -> Result<Cell, E> {
        Ok(match when(&s) {
            Some(_) => text_cell(&s),
            None => Cell::Str(s),
        })
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Cell, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<Json>()? {
            items.push(item);
        }
        Ok(Cell::Nested(Json::Array(items).to_string()))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Cell, A::Error> {
        // A number comes as a map of one key, serde_json's own, and its
        // digits as a string: read as both without making either one.
        let first = match map.next_key_seed(FirstKey)? {
            None => return Ok(Cell::Nested("{}".to_string())),
            Some(Some(key)) => key,
            Some(None) => return map.next_value_seed(NumberText),
        };
        let mut object = serde_json::Map::new();
        object.insert(first, map.next_value::<Json>()?);
        while let Some((key, value)) = map.next_entry::<String, Json>()? {
            object.insert(key, value);
        }
        Ok(Cell::Nested(Json::Object(object).to_string()))
    }
}

/// The first key of a map that is a value: `None` where it is the key a
/// number is handed under, and the key itself where it is an object's.
struct FirstKey;

impl<'de> DeserializeSeed<'de> for FirstKey {
    type Value = Option<String>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Option<String>, D::Error> {
        d.deserialize_str(self)
    }
}

impl<'de> Visitor<'de> for FirstKey {
    type Value = Option<String>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a key")
    }

    fn visit_str<E>(self, key: &str) -> Result<Option<String>, E> {
        Ok((key != NUMBER_TOKEN).then(|| key.to_string()))
    }
}

/// The digits of a number as they were written, read as the cell they are.
struct NumberText;

impl<'de> DeserializeSeed<'de> for NumberText {
    type Value = Cell;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Cell, D::Error> {
        d.deserialize_str(self)
    }
}

impl<'de> Visitor<'de> for NumberText {
    type Value = Cell;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a number")
    }

    fn visit_str<E>(self, digits: &str) -> Result<Cell, E> {
        Ok(number_cell(digits))
    }
}

/// Reads an object's key as the column it is, without making it a string
/// where the column is known.
struct KeySeed<'a>(&'a mut Names);

impl<'de> DeserializeSeed<'de> for KeySeed<'_> {
    type Value = Option<usize>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Option<usize>, D::Error> {
        d.deserialize_str(self)
    }
}

impl<'de> Visitor<'de> for KeySeed<'_> {
    type Value = Option<usize>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a key")
    }

    fn visit_str<E>(self, key: &str) -> Result<Option<usize>, E> {
        Ok(self.0.column(key))
    }
}

/// Reads one row of a sheet into a [`RowBuf`], and says what shape it was.
struct RowSeed<'a> {
    names: &'a mut Names,
    buf: &'a mut RowBuf,
}

impl<'de> DeserializeSeed<'de> for RowSeed<'_> {
    type Value = Shape;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Shape, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for RowSeed<'_> {
    type Value = Shape;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a row: an object, or a list of values")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Shape, A::Error> {
        for cell in self.buf.cells.iter_mut() {
            *cell = None;
        }
        self.buf.unknown = false;
        while let Some(column) = map.next_key_seed(KeySeed(self.names))? {
            match column {
                Some(c) => {
                    if self.buf.cells.len() <= c {
                        self.buf.cells.resize_with(c + 1, || None);
                    }
                    self.buf.cells[c] = Some(map.next_value_seed(CellSeed)?);
                }
                None => {
                    self.buf.unknown = true;
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(Shape::Object)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Shape, A::Error> {
        self.buf.cells.clear();
        self.buf.unknown = false;
        while let Some(cell) = seq.next_element_seed(CellSeed)? {
            self.buf.cells.push(Some(cell));
        }
        Ok(Shape::List)
    }
}

/// Calls `each` for the rows of the sheet `sheet` of the file, in order,
/// until it says to stop; the file is read through a buffer and never held.
fn rows(
    path: &Path,
    sheet: Option<&str>,
    names: &mut Names,
    mut each: impl FnMut(Shape, &mut Names, &mut RowBuf) -> bool,
) -> Result<(), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut de = serde_json::Deserializer::from_reader(BufReader::with_capacity(1 << 20, file));
    let mut buf = RowBuf {
        cells: Vec::new(),
        unknown: false,
    };
    let result = Top {
        sheet,
        names,
        buf: &mut buf,
        each: &mut each,
    }
    .deserialize(&mut de);
    match result {
        Ok(true) => Ok(()),
        Ok(false) => Err(SHAPE.to_string()),
        Err(e) if e.to_string().contains(STOP) => Ok(()),
        Err(e) => Err(format!("it is not JSON: {e}")),
    }
}

/// The top of the file: a list of rows, or an object of lists of rows.
/// `Ok(true)` is a sheet found and read.
struct Top<'a, F> {
    sheet: Option<&'a str>,
    names: &'a mut Names,
    buf: &'a mut RowBuf,
    each: &'a mut F,
}

impl<'de, F: FnMut(Shape, &mut Names, &mut RowBuf) -> bool> DeserializeSeed<'de> for Top<'_, F> {
    type Value = bool;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<bool, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de, F: FnMut(Shape, &mut Names, &mut RowBuf) -> bool> Visitor<'de> for Top<'_, F> {
    type Value = bool;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a list of rows, or an object of them")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<bool, A::Error> {
        // A bare list is the one sheet, whatever it is asked for by.
        Rows {
            names: self.names,
            buf: self.buf,
            each: self.each,
        }
        .visit_seq(seq)
        .map(|()| true)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<bool, A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            if self.sheet.is_none_or(|s| s == key) {
                // A value that is not a list is not a sheet.
                let read = map.next_value_seed(MaybeRows {
                    names: self.names,
                    buf: self.buf,
                    each: self.each,
                })?;
                if read {
                    // Everything after the sheet wanted is left unread.
                    return Err(serde::de::Error::custom(STOP));
                }
                if self.sheet.is_some() {
                    return Ok(false);
                }
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(false)
    }
}

/// A value that is a list of rows, read, or anything else, skipped.
struct MaybeRows<'a, F> {
    names: &'a mut Names,
    buf: &'a mut RowBuf,
    each: &'a mut F,
}

impl<'de, F: FnMut(Shape, &mut Names, &mut RowBuf) -> bool> DeserializeSeed<'de>
    for MaybeRows<'_, F>
{
    type Value = bool;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<bool, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de, F: FnMut(Shape, &mut Names, &mut RowBuf) -> bool> Visitor<'de> for MaybeRows<'_, F> {
    type Value = bool;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a list of rows")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<bool, A::Error> {
        Rows {
            names: self.names,
            buf: self.buf,
            each: self.each,
        }
        .visit_seq(seq)
        .map(|()| true)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<bool, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(false)
    }

    fn visit_bool<E>(self, _: bool) -> Result<bool, E> {
        Ok(false)
    }

    fn visit_i64<E>(self, _: i64) -> Result<bool, E> {
        Ok(false)
    }

    fn visit_u64<E>(self, _: u64) -> Result<bool, E> {
        Ok(false)
    }

    fn visit_f64<E>(self, _: f64) -> Result<bool, E> {
        Ok(false)
    }

    fn visit_str<E>(self, _: &str) -> Result<bool, E> {
        Ok(false)
    }

    fn visit_unit<E>(self) -> Result<bool, E> {
        Ok(false)
    }
}

/// The elements of a list, one at a time.
struct Rows<'a, F> {
    names: &'a mut Names,
    buf: &'a mut RowBuf,
    each: &'a mut F,
}

impl<'de, F: FnMut(Shape, &mut Names, &mut RowBuf) -> bool> Visitor<'de> for Rows<'_, F> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a list of rows")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        loop {
            let shape = seq.next_element_seed(RowSeed {
                names: self.names,
                buf: self.buf,
            })?;
            let Some(shape) = shape else {
                return Ok(());
            };
            if !(self.each)(shape, self.names, self.buf) {
                return Err(serde::de::Error::custom(STOP));
            }
        }
    }
}

/// The sheets of a JSON file.
#[derive(Debug, PartialEq, Eq)]
pub struct Book {
    /// Their names: the keys of an object whose values are lists, or the
    /// name of the file without `.json` for a bare list.
    pub sheets: Vec<String>,
    /// A bare list, which is the one sheet and is named by the file alone,
    /// without a `#`.
    pub bare: bool,
}

/// The sheets of the JSON file at `path`.
pub fn sheets(path: &Path) -> Result<Book, String> {
    use std::io::Read;
    let mut head = [0u8; 4096];
    let n = File::open(path)
        .and_then(|mut f| f.read(&mut head))
        .map_err(|e| e.to_string())?;
    let first = head[..n]
        .iter()
        .copied()
        .find(|b| !b.is_ascii_whitespace() && *b != 0xEF && *b != 0xBB && *b != 0xBF);
    if first == Some(b'[') {
        let stem = path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().to_string());
        return Ok(Book {
            sheets: vec![stem],
            bare: true,
        });
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut de = serde_json::Deserializer::from_reader(BufReader::with_capacity(1 << 20, file));
    let keys = Keys
        .deserialize(&mut de)
        .map_err(|e| format!("it is not JSON: {e}"))?;
    if keys.is_empty() {
        return Err(SHAPE.to_string());
    }
    Ok(Book {
        sheets: keys,
        bare: false,
    })
}

/// The keys of an object whose values are lists, the values skipped.
struct Keys;

impl<'de> DeserializeSeed<'de> for Keys {
    type Value = Vec<String>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Vec<String>, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Keys {
    type Value = Vec<String>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an object of lists of rows")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Vec<String>, A::Error> {
        let mut keys = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if map.next_value::<IsList>()?.0 {
                keys.push(key);
            }
        }
        Ok(keys)
    }
}

/// Whether a value is a list, its elements skipped without being read into
/// anything.
struct IsList(bool);

impl<'de> serde::Deserialize<'de> for IsList {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<IsList, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = IsList;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("any JSON")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<IsList, A::Error> {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(IsList(true))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<IsList, A::Error> {
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(IsList(false))
            }

            fn visit_bool<E>(self, _: bool) -> Result<IsList, E> {
                Ok(IsList(false))
            }

            fn visit_i64<E>(self, _: i64) -> Result<IsList, E> {
                Ok(IsList(false))
            }

            fn visit_u64<E>(self, _: u64) -> Result<IsList, E> {
                Ok(IsList(false))
            }

            fn visit_f64<E>(self, _: f64) -> Result<IsList, E> {
                Ok(IsList(false))
            }

            fn visit_str<E>(self, _: &str) -> Result<IsList, E> {
                Ok(IsList(false))
            }

            fn visit_unit<E>(self) -> Result<IsList, E> {
                Ok(IsList(false))
            }
        }
        d.deserialize_any(V)
    }
}

/// The rows of one sheet of a JSON file.
struct JsonSource<'a> {
    path: &'a Path,
    sheet: Option<&'a str>,
}

impl RowSource for JsonSource<'_> {
    fn rows(
        &self,
        names: &mut Names,
        each: &mut dyn FnMut(Shape, &mut Names, &mut RowBuf) -> bool,
    ) -> Result<(), String> {
        rows(self.path, self.sheet, names, |shape, names, buf| {
            each(shape, names, buf)
        })
    }
}

/// The whole of the sheet `sheet` of the JSON file at `path`, written as a
/// Parquet file at `target`. The types are worked out from the first rows
/// and the file written in one pass; where a row further down does not fit
/// them (a number among whole ones, a key that is new) the types are worked
/// out again from every row and it is written again.
pub fn convert(path: &Path, sheet: Option<&str>, target: &Path) -> Result<(), String> {
    copyframe::convert(&JsonSource { path, sheet }, target)
}

/// The first `rows` rows of the sheet, typed by the first `infer` of them,
/// written as a Parquet file at `target`: what is shown until the whole is.
pub fn sample(
    path: &Path,
    sheet: Option<&str>,
    infer: usize,
    rows: usize,
    target: &Path,
) -> Result<(), String> {
    copyframe::sample(&JsonSource { path, sheet }, infer, rows, target)
}

/// Whether a Parquet column of `dtype` is one a sheet can show, for a test.
#[cfg(test)]
fn shown(dtype: &DataType) -> bool {
    crate::dtype::type_of(dtype).is_some() || matches!(dtype, DataType::String)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(name: &str, json: &str, sheet: Option<&str>) -> Result<DataFrame, String> {
        let dir = std::env::temp_dir().join(format!("omasheet-json-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("in.json");
        std::fs::write(&file, json).unwrap();
        let target = dir.join("out.parquet");
        convert(&file, sheet, &target)?;
        let df = ParquetReader::new(File::open(&target).unwrap())
            .finish()
            .map_err(|e| e.to_string())?;
        std::fs::remove_dir_all(&dir).unwrap();
        Ok(df)
    }

    fn strs(df: &DataFrame, name: &str) -> Vec<Option<String>> {
        df.column(name)
            .unwrap()
            .as_materialized_series()
            .iter()
            .map(|v| match v {
                AnyValue::Null => None,
                AnyValue::String(s) => Some(s.to_string()),
                other => Some(other.to_string()),
            })
            .collect()
    }

    #[test]
    fn a_list_of_objects_is_a_table_typed_by_what_every_row_holds() {
        let df = written(
            "objects",
            r#"[
                {"id": 1, "name": "a", "price": 2.5, "ok": true, "on": "2024-03-05"},
                {"id": 2, "name": "b", "price": 3, "ok": false, "on": "2024-03-06"},
                {"id": 3, "name": null, "price": null, "on": null, "extra": "x"}
            ]"#,
            None,
        )
        .unwrap();
        assert_eq!(df.height(), 3);
        let names: Vec<_> = df
            .get_column_names()
            .iter()
            .map(|n| n.to_string())
            .collect();
        assert_eq!(names, ["id", "name", "price", "ok", "on", "extra"]);
        assert_eq!(df.column("id").unwrap().dtype(), &DataType::Int64);
        // A 2.5 and a 3: the column is a float, and holds both.
        assert_eq!(df.column("price").unwrap().dtype(), &DataType::Float64);
        assert_eq!(df.column("ok").unwrap().dtype(), &DataType::Boolean);
        assert_eq!(df.column("on").unwrap().dtype(), &DataType::Date);
        assert_eq!(df.column("name").unwrap().dtype(), &DataType::String);
        assert_eq!(
            strs(&df, "name"),
            [Some("a".into()), Some("b".into()), None]
        );
        // A key a row has not got is a null, as an explicit null is.
        assert_eq!(df.column("ok").unwrap().null_count(), 1);
        assert_eq!(df.column("price").unwrap().null_count(), 1);
        for column in df.get_columns() {
            assert!(shown(column.dtype()), "{:?}", column.dtype());
        }
    }

    #[test]
    fn mixed_values_make_a_text_column_and_nested_ones_are_kept_as_json() {
        let df = written(
            "mixed",
            r#"[{"a": 1, "b": [1, 2], "c": "2024-01-01T10:00:00"},
                {"a": "two", "b": {"k": true}, "c": "2024-01-02T11:30:00"}]"#,
            None,
        )
        .unwrap();
        assert_eq!(strs(&df, "a"), [Some("1".into()), Some("two".into())]);
        assert_eq!(
            strs(&df, "b"),
            [Some("[1,2]".into()), Some(r#"{"k":true}"#.into())]
        );
        assert!(matches!(
            df.column("c").unwrap().dtype(),
            DataType::Datetime(TimeUnit::Microseconds, None)
        ));
    }

    #[test]
    fn a_list_of_lists_takes_its_names_from_the_first_row() {
        let df = written(
            "lists",
            r#"[["id", "name"], [1, "a"], [2, "b"], [3]]"#,
            None,
        )
        .unwrap();
        assert_eq!(df.height(), 3);
        assert_eq!(df.column("id").unwrap().dtype(), &DataType::Int64);
        assert_eq!(df.column("name").unwrap().null_count(), 1);
    }

    #[test]
    fn an_object_of_lists_is_a_sheet_for_each_key_and_a_source_names_one() {
        let json = r#"{"Sales": [{"n": 1}, {"n": 2}], "note": "not a sheet",
                       "Costs": [{"c": "x"}, {"c": "y"}, {"c": "z"}]}"#;
        assert_eq!(written("a", json, Some("Costs")).unwrap().height(), 3);
        assert_eq!(written("b", json, Some("Sales")).unwrap().height(), 2);
        // Bare, it is the first.
        let first = written("c", json, None).unwrap();
        assert_eq!(first.get_column_names()[0].as_str(), "n");
        let err = written("d", json, Some("Nope"));
        assert!(err.is_err());
        // A key that is not a list is not a sheet.
        assert!(written("e", json, Some("note")).is_err());
    }

    #[test]
    fn the_sheets_of_a_file_are_its_lists() {
        let dir = std::env::temp_dir().join(format!("omasheet-json-{}-sheets", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let book = dir.join("book.json");
        std::fs::write(&book, r#"{"A": [{"x": 1}], "skip": 3, "B": [{"y": 2}]}"#).unwrap();
        assert_eq!(
            sheets(&book).unwrap(),
            Book {
                sheets: vec!["A".into(), "B".into()],
                bare: false
            }
        );
        let list = dir.join("rows.json");
        std::fs::write(&list, "\n  [{\"x\": 1}]").unwrap();
        assert_eq!(
            sheets(&list).unwrap(),
            Book {
                sheets: vec!["rows".into()],
                bare: true
            }
        );
        let bad = dir.join("bad.json");
        std::fs::write(&bad, "42").unwrap();
        assert!(sheets(&bad).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn what_is_not_rows_is_said_so() {
        assert!(written("n1", "42", None).unwrap_err().contains("not"));
        assert!(written("n2", "[]", None).unwrap_err().contains("no rows"));
        assert!(written("n3", "[{\"a\": 1}, 5]", None).is_err());
        assert!(written("n4", "[{\"a\": 1}, [1]]", None).is_err());
        assert!(
            written("n5", "[{\"a\": 1", None)
                .unwrap_err()
                .contains("JSON")
        );
    }

    #[test]
    fn more_rows_than_a_batch_are_written_in_row_groups_and_all_read_back() {
        let dir = std::env::temp_dir().join(format!("omasheet-json-{}-big", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("big.json");
        let n = BATCH + 1_000;
        let mut text = String::from("[");
        for i in 0..n {
            if i > 0 {
                text.push(',');
            }
            text.push_str(&format!("{{\"i\": {i}, \"s\": \"r{i}\"}}"));
        }
        text.push(']');
        std::fs::write(&file, text).unwrap();
        let target = dir.join("big.parquet");
        convert(&file, None, &target).unwrap();
        let mut reader = ParquetReader::new(File::open(&target).unwrap());
        assert_eq!(reader.get_metadata().unwrap().row_groups.len(), 2);
        let df = reader.finish().unwrap();
        assert_eq!(df.height(), n);
        let last = df.column("i").unwrap().get(n - 1).unwrap();
        assert_eq!(last, AnyValue::Int64(n as i64 - 1));
        // And the sample is its first rows, typed alike.
        let sample_path = dir.join("sample.parquet");
        sample(&file, None, 100, 50, &sample_path).unwrap();
        let s = ParquetReader::new(File::open(&sample_path).unwrap())
            .finish()
            .unwrap();
        assert_eq!(s.height(), 50);
        assert_eq!(s.column("i").unwrap().dtype(), &DataType::Int64);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_row_past_the_first_ones_that_does_not_fit_them_has_the_types_worked_out_again() {
        // Ten thousand rows of whole numbers and no `late`, then a number
        // that is not whole, a key that is new, and a string among the
        // whole numbers of another column.
        let mut text = String::from("[");
        for i in 0..(HEAD_ROWS + 500) {
            if i > 0 {
                text.push(',');
            }
            match i {
                n if n == HEAD_ROWS + 100 => text.push_str(&format!(
                    "{{\"i\": {n}, \"x\": 2.5, \"s\": 7, \"late\": \"new\"}}"
                )),
                n if n == HEAD_ROWS + 200 => {
                    text.push_str(&format!("{{\"i\": {n}, \"x\": 3, \"s\": \"seven\"}}"))
                }
                n => text.push_str(&format!("{{\"i\": {n}, \"x\": 1, \"s\": 7}}")),
            }
        }
        text.push(']');
        let df = written("widen", &text, None).unwrap();
        assert_eq!(df.height(), HEAD_ROWS + 500);
        let names: Vec<_> = df
            .get_column_names()
            .iter()
            .map(|n| n.to_string())
            .collect();
        assert_eq!(names, ["i", "x", "s", "late"]);
        assert_eq!(df.column("i").unwrap().dtype(), &DataType::Int64);
        // A float, holding the 2.5 and the whole numbers beside it.
        assert_eq!(df.column("x").unwrap().dtype(), &DataType::Float64);
        assert_eq!(
            df.column("x").unwrap().get(HEAD_ROWS + 100).unwrap(),
            AnyValue::Float64(2.5)
        );
        // Text, holding 7 as it was written and "seven".
        assert_eq!(df.column("s").unwrap().dtype(), &DataType::String);
        assert_eq!(strs(&df, "s")[0], Some("7".into()));
        assert_eq!(strs(&df, "s")[HEAD_ROWS + 200], Some("seven".into()));
        // The key that came late is a column, null where a row lacks it.
        assert_eq!(strs(&df, "late")[HEAD_ROWS + 100], Some("new".into()));
        assert_eq!(df.column("late").unwrap().null_count(), HEAD_ROWS + 499);
    }

    #[test]
    fn numbers_are_read_as_they_are_written() {
        // The key serde_json hands a number under is the one named here;
        // if a version of it changes it, this is the test that says so.
        let df = written(
            "numbers",
            r#"[{"a": 1, "b": -2, "c": 3.5, "d": 1e3, "e": 18446744073709551615, "f": 0.1},
                {"a": 9223372036854775807, "b": 0, "c": 1E-2, "d": 2.5E+2, "e": 1, "f": 5}]"#,
            None,
        )
        .unwrap();
        assert_eq!(df.column("a").unwrap().dtype(), &DataType::Int64);
        assert_eq!(df.column("b").unwrap().dtype(), &DataType::Int64);
        assert_eq!(df.column("c").unwrap().dtype(), &DataType::Float64);
        // 1e3 is not written as a whole number, so it is a float.
        assert_eq!(df.column("d").unwrap().dtype(), &DataType::Float64);
        // A whole number past what `i64` holds is a float, not an error.
        assert_eq!(df.column("e").unwrap().dtype(), &DataType::Float64);
        assert_eq!(df.column("f").unwrap().dtype(), &DataType::Float64);
        assert_eq!(
            df.column("a").unwrap().get(1).unwrap(),
            AnyValue::Int64(i64::MAX)
        );
        assert_eq!(
            df.column("c").unwrap().get(1).unwrap(),
            AnyValue::Float64(0.01)
        );
        assert_eq!(
            df.column("d").unwrap().get(0).unwrap(),
            AnyValue::Float64(1000.0)
        );
    }

    #[test]
    fn a_date_among_other_text_keeps_its_text_and_a_time_zone_less_date_time_is_one() {
        let df = written(
            "dates",
            r#"[{"d": "2024-03-05", "e": "2024-03-05T10:30:00.250"},
                {"d": "not a date", "e": "2024-03-06T00:00:01"}]"#,
            None,
        )
        .unwrap();
        // One that is not a date makes the column text, and the date is as
        // it was written.
        assert_eq!(
            strs(&df, "d"),
            [Some("2024-03-05".into()), Some("not a date".into())]
        );
        assert!(matches!(
            df.column("e").unwrap().dtype(),
            DataType::Datetime(TimeUnit::Microseconds, None)
        ));
    }

    #[test]
    fn the_quick_reading_of_a_date_is_the_general_ones() {
        // Every day from 1900 to 2200, and the days either side that are
        // not days, against the reading of a literal.
        let mut days = 0u32;
        for y in 1899..=2201 {
            for m in 0..=13 {
                for d in [0, 1, 15, 28, 29, 30, 31, 32] {
                    let s = format!("{y:04}-{m:02}-{d:02}");
                    let general = match parse_literal(&s) {
                        Some(Lit::Date(days)) => Some(days),
                        _ => None,
                    };
                    let quick = iso_date(&s);
                    // Where the quick one answers it is the same answer; it
                    // may leave a date to the general reading, never answer
                    // one the general reading does not.
                    if let Some(q) = quick {
                        assert_eq!(Some(q), general, "{s}");
                        days += 1;
                    }
                }
            }
        }
        assert!(days > 15_000, "it answers most dates: {days}");
    }
}

// Copyright (c) 2026 Stephen Roe

//! Task 8.1 of `add-columnar-core`: the kernel differential. A lowered
//! column is Polars' answer rather than the engine's, and the whole bet of
//! the columnar core is that the two are the same answer. This checks that
//! pair by pair: for every pair of column types and every operator between
//! them, a one-row-at-a-time engine column and the same column lowered to
//! Polars must agree, value for value and diagnostic for diagnostic.
//!
//! The grid is fixed and the edge values are written out, so a failure
//! names the triple that disagrees. A second pass fills the same grid from
//! a seeded generator, the seed printed on failure so the case it found can
//! be read back.

use omasheet_engine::eval::Attempt;
use omasheet_engine::omx::compile;
use omasheet_engine::{Engine, Lowering, Options, table, view_with};

/// Every type a column can be given, with the edge values a cell of one is
/// filled with. An empty cell is in every list: `Null` is the case most
/// worth asking about, since the two paths reach it by different rules.
const TYPES: [(&str, &[&str]); 10] = [
    (
        "Int",
        &["0", "1", "-1", "9223372036854775807", "-9223372036854775808", ""],
    ),
    (
        "Decimal",
        &[
            "0",
            "0.5",
            "-19.99",
            "99999999999999999999999999999999999999",
            "0.00000000000000000000000000000000000001",
            "",
        ],
    ),
    ("Fraction", &["0", "1/7", "2 3/4", "-1/3", ""]),
    ("Percent", &["0%", "20%", "-5%", "100%", ""]),
    ("Float", &["0", "1.5", "-2.5", "1e308", ""]),
    ("String", &["\"\"", "\"hi\"", "\"42\"", "\"-\"", ""]),
    ("Boolean", &["true", "false", ""]),
    ("Date", &["1970-01-01", "2025-01-31", "9999-12-31", ""]),
    ("Time", &["00:00", "09:30", "23:59:59", ""]),
    (
        "Datetime",
        &["1970-01-01T00:00", "2025-01-31T09:30", ""],
    ),
];

/// Every operator that stands between two cells. `in` takes a list on its
/// right and the four range operators make one, so none of them is an
/// operator between two columns and none is here.
const OPS: [&str; 15] = [
    "+", "-", "*", "/", "**", "==", "!=", "<", ">", "<=", ">=", "and", "or", "//", "~",
];

/// A one-column sheet over the rows given: `A` and `B` of the two types,
/// and `C` the operator between them.
fn sheet(ta: &str, tb: &str, op: &str, rows: &[(String, String)]) -> String {
    let mut text = format!("sheet K\n\nA : {ta}\nB : {tb}\n\n| A | B | C |\n");
    for (a, b) in rows {
        text.push_str(&format!("| {a} | {b} | * |\n"));
    }
    text.push_str(&format!("\nC := A {op} B\n"));
    text
}

/// Every pairing of one type's edge values with another's.
fn pairs(ea: &[&str], eb: &[&str]) -> Vec<(String, String)> {
    ea.iter()
        .flat_map(|a| eb.iter().map(move |b| (a.to_string(), b.to_string())))
        .collect()
}

/// What became of the sheet's one computed column when lowering was forced.
/// `None` where the file does not get as far as a column.
fn attempt(text: &str) -> Option<Attempt> {
    let (mut program, mut diags) = compile(text, 0);
    if !diags.is_empty() {
        return None;
    }
    let tables = table::read(&mut program, std::path::Path::new("."), &mut diags);
    let program = program;
    let engine = Engine::with_tables(
        &program,
        Options {
            lowering: Lowering::Always,
            exact: true,
            ..Options::default()
        },
        tables,
    );
    engine.run();
    engine.attempts().first().map(|a| a.2)
}

/// The sheet calculated both ways, and where the two differ: the first row
/// that does not match, named by what was in it. A whole grid is checked
/// before anything is reported, so one disagreement does not hide the rest.
fn difference(text: &str, rows: &[(String, String)]) -> Option<String> {
    let run = |lowering| {
        view_with(
            "kernel.omx",
            text,
            Options {
                lowering,
                exact: true,
                ..Options::default()
            },
        )
    };
    let engine = run(Lowering::Never);
    let polars = run(Lowering::Always);
    if engine.errors != polars.errors {
        return Some(format!(
            "reported {:?} by the engine and {:?} by Polars",
            engine.errors, polars.errors
        ));
    }
    // The rendered sheet is one line per row, under two lines of header.
    let lines = |out: &str| -> Vec<String> { out.lines().skip(4).map(str::to_string).collect() };
    let (ours, theirs) = (lines(&engine.output), lines(&polars.output));
    let (i, ours, theirs) = ours
        .iter()
        .zip(&theirs)
        .enumerate()
        .find(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| (i, a.clone(), b.clone()))?;
    let (a, b) = rows.get(i).cloned().unwrap_or_default();
    let blank = |s: String| if s.is_empty() { "<empty>".to_string() } else { s };
    Some(format!(
        "of {} and {} the engine says `{}` and Polars `{}`",
        blank(a),
        blank(b),
        ours.trim(),
        theirs.trim()
    ))
}

#[test]
fn every_operator_between_every_pair_of_types_agrees_either_way() {
    let mut lowered = 0;
    let mut refused = 0;
    let mut failed = 0;
    let mut differ = Vec::new();
    for (ta, ea) in TYPES {
        for (tb, eb) in TYPES {
            let rows = pairs(ea, eb);
            for op in OPS {
                let text = sheet(ta, tb, op, &rows);
                if let Some(how) = difference(&text, &rows) {
                    differ.push(format!("`{ta} {op} {tb}`: {how}"));
                }
                match attempt(&text) {
                    Some(Attempt::Lowered) => lowered += 1,
                    Some(Attempt::Refused) => refused += 1,
                    Some(Attempt::Failed) => failed += 1,
                    None => {}
                }
            }
        }
    }
    // A differential proves nothing where nothing lowers, so the grid says
    // how much of it Polars actually answered. A column lowered and then
    // given up on is the engine's answer in the end, and none of the grid
    // reaches that: `Failed` here would mean a plan that would not collect.
    eprintln!("lowered {lowered}, refused {refused}, given up {failed}");
    assert!(
        differ.is_empty(),
        "{} of the grid disagree:\n{}",
        differ.len(),
        differ.join("\n")
    );
    assert_eq!(failed, 0, "a lowered column would not collect");
    assert!(lowered > 0, "nothing in the grid lowered");
}

/// A seeded linear congruential generator, so a failure can be read back
/// from the seed alone. The constants are Numerical Recipes'.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 16
    }

    /// A number below `n`.
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// One of the values written for a type, or a value of its own where
    /// the type is one a generator can reach: the fixed list is the edges,
    /// and this is the middle ground between them.
    fn value(&mut self, ty: &str, edges: &[&str]) -> String {
        match ty {
            "Int" => format!("{}", self.next() as i64 - (1 << 31)),
            "Decimal" => {
                let digits = self.next() % 1_000_000_000;
                let scale = self.below(12) as usize;
                let sign = if self.below(2) == 0 { "-" } else { "" };
                let text = format!("{digits:0>width$}", width = scale + 1);
                let point = text.len() - scale;
                format!("{sign}{}.{}", &text[..point], &text[point..])
            }
            "Float" => {
                let f = self.next() as f64 / 1_000.0 - 1_000_000.0;
                format!("{f}")
            }
            "Percent" => format!("{}%", self.next() % 1_000),
            "Boolean" => format!("{}", self.below(2) == 0),
            "Date" => {
                let days = self.below(60_000) as i64;
                let (y, m, d) = omasheet_engine::omx::date::civil_from_days(days);
                format!("{y:04}-{m:02}-{d:02}")
            }
            // A fraction, a time and a string are left to their edges: the
            // forms a generator would reach are already written there.
            _ => edges[self.below(edges.len() as u64) as usize].to_string(),
        }
    }
}

#[test]
fn a_generated_grid_agrees_either_way() {
    // One seed, written down: a failure is read back by putting it here.
    const SEED: u64 = 0x5EED_0000_0000_0001;
    let mut lcg = Lcg(SEED);
    let mut differ = Vec::new();
    for (ta, ea) in TYPES {
        for (tb, eb) in TYPES {
            // Eight rows of generated values, with the empty cell kept in:
            // it is the one case a generator would otherwise never write.
            let mut rows: Vec<(String, String)> = (0..8)
                .map(|_| (lcg.value(ta, ea), lcg.value(tb, eb)))
                .collect();
            rows.push((String::new(), lcg.value(tb, eb)));
            rows.push((lcg.value(ta, ea), String::new()));
            for op in OPS {
                let text = sheet(ta, tb, op, &rows);
                if let Some(how) = difference(&text, &rows) {
                    differ.push(format!("`{ta} {op} {tb}`: {how}"));
                }
            }
        }
    }
    assert!(
        differ.is_empty(),
        "{} of the grid from seed {SEED:#x} disagree:\n{}",
        differ.len(),
        differ.join("\n")
    );
}

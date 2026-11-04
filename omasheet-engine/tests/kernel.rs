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
const TYPES: [(&str, &[&str]); 12] = [
    (
        "Integer",
        &[
            "0",
            "1",
            "-1",
            "9223372036854775807",
            "-9223372036854775808",
            "",
        ],
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
    ("String", &["''", "'hi'", "'42'", "'-'", ""]),
    ("Boolean", &["true", "false", ""]),
    ("Date", &["1970-01-01", "2025-01-31", "9999-12-31", ""]),
    ("Time", &["00:00", "09:30", "23:59:59", ""]),
    ("Datetime", &["1970-01-01T00:00", "2025-01-31T09:30", ""]),
    (
        "Duration",
        &["PT0S", "PT1H30M", "-PT30M", "P1DT6H", "PT0.5S", ""],
    ),
    // A column in a zone of its own, with the instants a zone makes
    // awkward: the hour London's clocks skip going forward, the hour they
    // show twice going back, and a time on each side of the change.
    (
        "Datetime(\"Europe/London\")",
        &[
            "2025-03-30T00:30",
            "2025-03-30T01:30",
            "2025-07-01T09:30",
            "2025-10-26T01:30",
            "",
        ],
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
    // `KERNEL_SHOW=1` prints both sheets whole where they differ, which is
    // how to see the row a line that differs only in its padding is about.
    if ours != theirs && std::env::var_os("KERNEL_SHOW").is_some() {
        eprintln!("ENGINE:\n{}\nPOLARS:\n{}", engine.output, polars.output);
    }
    let (i, ours, theirs) = ours
        .iter()
        .zip(&theirs)
        .enumerate()
        .find(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| (i, a.clone(), b.clone()))?;
    let (a, b) = rows.get(i).cloned().unwrap_or_default();
    let blank = |s: String| {
        if s.is_empty() {
            "<empty>".to_string()
        } else {
            s
        }
    };
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
            "Integer" => format!("{}", self.next() as i64 - (1 << 31)),
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

    /// As [`Lcg::value`], but reaching for the ends of what a type holds
    /// rather than the middle: a whole number as wide as an `i64`, a decimal
    /// of up to 38 digits at a scale anywhere in `0..=37`, and one so small
    /// that nearly every digit is after the point.
    fn wide(&mut self, ty: &str, edges: &[&str], cap: usize) -> String {
        match ty {
            "Integer" => match self.below(3) {
                0 => format!("{}", ((self.next() << 32) ^ self.next()) as i64),
                1 => format!("{}", i64::MAX - self.below(1_000) as i64),
                _ => format!("{}", i64::MIN + self.below(1_000) as i64),
            },
            "Decimal" => {
                // The column has one scale, the widest any value needs, and
                // every value has to fit at it or the column is declined and
                // never reaches Polars. So the column's scale is capped, and
                // a value has as many digits before the point as the cap
                // leaves room for: wide where the cap is low, and narrow
                // before a point that sits far to the right.
                let frac = match self.below(3) {
                    0 => 0,
                    1 => cap,
                    _ => self.below(cap as u64 + 1) as usize,
                };
                let whole = match self.below(3) {
                    0 => 0,
                    1 => 38 - cap,
                    _ => self.below((38 - cap) as u64 + 1) as usize,
                };
                let digit = |lcg: &mut Lcg| char::from(b'1' + lcg.below(9) as u8);
                let int_part = if whole == 0 {
                    "0".to_string()
                } else {
                    (0..whole).map(|_| digit(self)).collect()
                };
                let frac_part: String = (0..frac).map(|_| digit(self)).collect();
                let sign = if self.below(2) == 0 { "-" } else { "" };
                if frac == 0 {
                    format!("{sign}{int_part}")
                } else {
                    format!("{sign}{int_part}.{frac_part}")
                }
            }
            "Float" => match self.below(3) {
                0 => format!("{}e{}", self.below(9) + 1, self.below(300)),
                1 => format!("{}e-{}", self.below(9) + 1, self.below(300)),
                _ => format!("{}", self.next() as f64 / 1_000.0 - 1_000_000.0),
            },
            "Percent" => format!("{}%", self.next() % 1_000_000_000_000),
            _ => self.value(ty, edges),
        }
    }
}

#[test]
fn a_generated_grid_agrees_either_way() {
    // One seed, written down: a failure is read back by putting it here.
    const SEED: u64 = 0x5EED_0000_0000_0001;
    let mut differ = Vec::new();
    for (ta, ea) in TYPES {
        for (tb, eb) in TYPES {
            // Each pair has a generator of its own, so that a type added to
            // the list does not reshuffle the values every other pair gets.
            let mut lcg = Lcg(ta
                .bytes()
                .chain(*b"|")
                .chain(tb.bytes())
                .fold(SEED, |h, b| h.wrapping_mul(31).wrapping_add(b as u64)));
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

/// Found 2026-11-04 by the generated grid, when a type added to the list
/// moved the values it drew: a `Decimal` column times a wide `Integer`. The
/// engine was right and Polars wrapped the product silently, because it
/// multiplies at twice the larger scale before scaling the answer back, and
/// that passed what an `i128` holds although the answer itself did not.
/// `lower::arith` now estimates what Polars must hold and gives the column
/// to the engine when it is near the edge.
#[test]
fn a_wide_decimal_product_agrees_either_way() {
    let rows: Vec<(String, String)> = [
        ("-4.31345428", "232091885816215"),
        ("0.0218966386", "133674715537767"),
        ("1335897.6", "244800296630449"),
        ("46850.626", "128359535724483"),
        ("42107.9544", "196723984324582"),
        ("0.0870169905", "238171445948163"),
        ("-14.9488799", "242469729158024"),
        ("-3486289.1", "132537643048075"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    let text = sheet("Decimal", "Integer", "*", &rows);
    assert_eq!(difference(&text, &rows), None);
}

/// A column in a zone is lowered for one thing: comparing it with another
/// column of the same zone, which Polars does on the instants as the engine
/// does. Everything else about it stays on the engine, and the grid above
/// checks that each of those agrees by being the same path.
#[test]
fn zoned_columns_of_one_zone_are_compared_by_polars_and_agree() {
    let (ty, edges) = TYPES
        .iter()
        .find(|(t, _)| t.starts_with("Datetime(\""))
        .copied()
        .expect("a zoned type in the list");
    let rows = pairs(edges, edges);
    for op in ["==", "!=", "<", ">", "<=", ">="] {
        let text = sheet(ty, ty, op, &rows);
        assert_eq!(
            attempt(&text),
            Some(Attempt::Lowered),
            "`{op}` was not lowered"
        );
        assert_eq!(difference(&text, &rows), None, "`{op}` disagrees");
    }
    // Against a column on the file's own clocks it is compared by the zone
    // the engine works out, so that is left to the engine.
    let text = sheet(ty, "Datetime", "<", &pairs(edges, &["2025-07-01T09:30"]));
    assert_eq!(attempt(&text), Some(Attempt::Refused));
}

/// The generated grid again with operands at the ends of what each type
/// holds: whole numbers as wide as an `i64`, decimals of up to 38 digits and
/// down to `1e-37`, floats from `1e-300` to `1e300`. A lowered column is
/// Polars' answer, and Polars does not stop at the ends of a decimal: it
/// wraps, or rounds, or answers nothing, so this is where the two paths are
/// most likely to part.
///
/// It names every disagreement and fails on any, which is the point of it,
/// and is ignored until they are fixed. `KERNEL_SEED=<n>` draws another grid.
///
/// What forty seeds found on 2026-11-04, by kind:
///
/// - `+` and `-` between exact numbers: Polars answers `#ERROR` where the
///   engine, whose point moves when a result has no room, answers. Seen
///   where the sum needs the 39th digit at the column's shared scale.
/// - `*` between exact numbers (`Decimal`, `Integer`, `Percent`, any two):
///   Polars wraps the product to a plausible number. The engine is right.
/// - Any operator with a `Float` on one side (`+ - * /`): the last digit
///   differs, the two paths turning a `Decimal` into an `f64` differently.
/// - `//` with a `Float` on one side and an exact number on the other:
///   fixed the same day. Polars made the result a `Float`, which cut a
///   `Decimal` or an `Integer` to 16 digits, where the engine keeps the left
///   value as it is; such a fallback is no longer lowered.
///
/// `**`, `/` between exact numbers (which is not lowered), the comparisons
/// and the logical operators never disagreed.
#[test]
#[ignore = "finds standing disagreements at the ends of the decimal range; see above"]
fn a_grid_at_the_ends_of_the_ranges_agrees_either_way() {
    // `KERNEL_SEED=<number>` draws a different grid, to look further than the
    // one written down; the failure says which seed it was.
    let seed: u64 = std::env::var("KERNEL_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x5EED_0000_0000_00A1);
    let mut differ = Vec::new();
    let mut by_op: std::collections::BTreeMap<String, usize> = Default::default();
    for (ta, ea) in TYPES {
        for (tb, eb) in TYPES {
            let mut lcg = Lcg(ta
                .bytes()
                .chain(*b"|")
                .chain(tb.bytes())
                .fold(seed, |h, b| h.wrapping_mul(31).wrapping_add(b as u64)));
            // The scale each column of decimals is held at, drawn from the
            // whole range: 0 is whole numbers, 37 is almost all fraction.
            const CAPS: [usize; 6] = [0, 4, 10, 18, 28, 37];
            let (ca, cb) = (CAPS[lcg.below(6) as usize], CAPS[lcg.below(6) as usize]);
            let mut rows: Vec<(String, String)> = (0..12)
                .map(|_| (lcg.wide(ta, ea, ca), lcg.wide(tb, eb, cb)))
                .collect();
            rows.push((String::new(), lcg.wide(tb, eb, cb)));
            rows.push((lcg.wide(ta, ea, ca), String::new()));
            for op in OPS {
                let text = sheet(ta, tb, op, &rows);
                if let Some(how) = difference(&text, &rows) {
                    *by_op.entry(op.to_string()).or_default() += 1;
                    differ.push(format!("`{ta} {op} {tb}`: {how}"));
                }
            }
        }
    }
    eprintln!("disagreements by operator: {by_op:?}");
    assert!(
        differ.is_empty(),
        "{} of the grid disagree:\n{}",
        differ.len(),
        differ.join("\n")
    );
}

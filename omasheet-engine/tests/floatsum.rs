// Copyright (c) 2026 Stephen Roe

//! The sum and the mean of a column of floats, taken as a loop over the
//! floats and as a walk of a value for each, which have to be the same to
//! the last bit: adding floats in another order is a different answer, so the
//! loop adds them in the walk's order (`open-a-big-csv` 5.3).
//!
//! One engine takes the column as it is, which is the loop; another has
//! already worked out one cell of it, which is what the loop leaves to the
//! walk.

use omasheet_engine::omx::{compile, compile_expr};
use omasheet_engine::{Engine, Lowering, Options, Value, table};
use polars::prelude::*;

/// A tiny generator, so that the columns are the same on every machine.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
}

const ROWS: usize = 30_000;

/// The columns, each a list of what is in it, `None` for an empty cell.
fn columns() -> Vec<(&'static str, Vec<Option<f64>>)> {
    let mut rng = Lcg(0x9E37_79B9);
    // Magnitudes from 1e-8 to 1e12, in both signs: a sum of these depends on
    // the order, every time.
    let mut spread = |n: usize| -> Vec<Option<f64>> {
        (0..n)
            .map(|_| {
                let m = (rng.next() % 1_000_000) as f64 / 7.0;
                let e = 10f64.powi((rng.next() % 21) as i32 - 8);
                Some(if rng.next().is_multiple_of(2) {
                    m * e
                } else {
                    -m * e
                })
            })
            .collect()
    };
    let plain = spread(ROWS);
    let nulls: Vec<Option<f64>> = spread(ROWS)
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            if i.is_multiple_of(5) || i.is_multiple_of(977) {
                None
            } else {
                v
            }
        })
        .collect();
    let mut nan = spread(ROWS);
    nan[12_345] = Some(f64::NAN);
    let mut inf = spread(ROWS);
    inf[100] = Some(f64::INFINITY);
    inf[200] = Some(f64::NEG_INFINITY);
    let mut big_then_small = vec![Some(1e16), Some(1.0), Some(-1e16)];
    big_then_small.extend((0..ROWS - 3).map(|i| Some(if i % 2 == 0 { 1.0 } else { 0.5 })));
    vec![
        ("Plain", plain),
        ("Nulls", nulls),
        ("NotANumber", nan),
        ("Infinite", inf),
        ("NegZero", vec![Some(-0.0); ROWS]),
        ("OneThenLost", big_then_small),
        ("Empty", vec![None; ROWS]),
    ]
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a writable scratch directory");
    dir
}

/// What `expr` comes to over the column, in an engine that has worked out a
/// cell first or not, and whether the loop was what took it.
fn taken(dir: &std::path::Path, column: &str, expr: &str, cell_first: bool) -> (Value, usize) {
    let text = format!("view T := dataframe['f.parquet']\n\n{column} : Float\n");
    let (mut program, mut diags) = compile(&text, 0);
    assert!(diags.is_empty(), "{diags:?}");
    let tables = table::read(&mut program, dir, &mut diags);
    assert!(diags.is_empty(), "{diags:?}");
    let program = program;
    let engine = Engine::with_tables(
        &program,
        Options {
            base: Some(dir.to_path_buf()),
            // Polars' sum of floats is refused anyway (D72); this says it.
            lowering: Lowering::Never,
            ..Options::default()
        },
        tables,
    );
    engine.run();
    if cell_first {
        let _ = engine.cell(0, 0, 0);
    }
    let (node, _) = compile_expr(&program, expr, 1).expect("the expression checks");
    let value = engine.eval_top(&node).expect("a value");
    (value, engine.float_totals())
}

/// Equal as floats are for this: the same bits, or both a `NaN`.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => {
            x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())
        }
        (Value::Int(x, _), Value::Int(y, _)) => x == y,
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}

#[test]
fn the_sum_and_the_mean_of_floats_are_the_same_to_the_last_bit_as_a_walk_of_values() {
    let dir = scratch("floatsum");
    for (rows_per_group, kinds) in [(1_000usize, "many row groups"), (ROWS, "one")] {
        for (name, values) in columns() {
            let mut frame = df![name => values.clone()].expect("a frame");
            let mut file = std::fs::File::create(dir.join("f.parquet")).expect("a file");
            ParquetWriter::new(&mut file)
                .with_row_group_size(Some(rows_per_group))
                .finish(&mut frame)
                .expect("a Parquet");
            drop(file);
            for expr in ["T.%.sum()", "T.%.mean()"] {
                let expr = expr.replace('%', name);
                let (fast, by_loop) = taken(&dir, name, &expr, false);
                let (walked, by_walk) = taken(&dir, name, &expr, true);
                assert_eq!(by_walk, 0, "{name} {expr}: the walk is the walk");
                // A column of nothing is left to the walk, and the rest are
                // the loop's.
                let expect_loop = usize::from(name != "Empty");
                assert_eq!(
                    by_loop, expect_loop,
                    "{name} {expr} ({kinds}): the loop took it"
                );
                assert!(
                    same(&fast, &walked),
                    "{name} {expr} ({kinds}): {fast:?} against {walked:?}"
                );
            }
        }
    }
}

#[test]
fn a_column_the_sheet_does_not_call_a_float_is_left_to_the_walk() {
    // The loop adds floats as floats. A column that the sheet declares as
    // another kind of number is read by the walk, whatever the file holds.
    let dir = scratch("floatsum-declared");
    let mut frame = df!["V" => vec![Some(0.5f64); 1_000]].expect("a frame");
    let mut file = std::fs::File::create(dir.join("f.parquet")).expect("a file");
    ParquetWriter::new(&mut file)
        .finish(&mut frame)
        .expect("a Parquet");
    drop(file);
    for (declared, by_loop) in [("Float", 1), ("Decimal", 0), ("Integer", 0)] {
        let text = format!("view T := dataframe['f.parquet']\n\nV : {declared}\n");
        let (mut program, mut diags) = compile(&text, 0);
        let tables = table::read(&mut program, &dir, &mut diags);
        let program = program;
        let engine = Engine::with_tables(
            &program,
            Options {
                base: Some(dir.clone()),
                lowering: Lowering::Never,
                ..Options::default()
            },
            tables,
        );
        engine.run();
        let (node, _) = compile_expr(&program, "T.V.sum()", 1).expect("the expression checks");
        let _ = engine.eval_top(&node);
        assert_eq!(engine.float_totals(), by_loop, "declared {declared}");
    }
}

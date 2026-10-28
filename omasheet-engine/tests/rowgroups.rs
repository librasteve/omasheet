// Copyright (c) 2026 Stephen Roe

//! How much of a Parquet an answer costs: a `table` over a file of several
//! row groups, and a count of what comes back from it.
//!
//! A row group is the unit a Parquet is read in, so the count here is the
//! measurement task 6.2 is missing — the projection reaches the scan and
//! the limit does not, and nothing until now said so in a number.

use omasheet_engine::omx::{compile, compile_expr};
use omasheet_engine::{Engine, Lowering, Options, table};
use polars::prelude::*;

/// The fixture: five row groups of a thousand rows. A thousand is well
/// under the row group Polars writes by default, so the size has to be
/// asked for; five is enough that reading one and reading all of them are
/// different numbers.
const PER_GROUP: usize = 1_000;
const GROUPS: usize = 5;
const ROWS: usize = PER_GROUP * GROUPS;

/// A table over the fixture. Two columns, so that reading one and reading
/// both are different numbers as well.
const TABLE: &str = "\
table Numbers := dataframe[\"rowgroups.parquet\"]

Group : Int
Value : Int
";

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a writable scratch directory");
    dir
}

/// The fixture, written in `dir`. Each row says which row group it is in,
/// so that what came back can be told from where it came.
fn fixture(dir: &std::path::Path) -> std::path::PathBuf {
    let group: Vec<i64> = (0..ROWS).map(|r| (r / PER_GROUP) as i64).collect();
    let value: Vec<i64> = (0..ROWS).map(|r| r as i64).collect();
    let mut frame = df!["Group" => group, "Value" => value].expect("a frame of two columns");
    let path = dir.join("rowgroups.parquet");
    let mut file = std::fs::File::create(&path).expect("a writable file");
    ParquetWriter::new(&mut file)
        .with_row_group_size(Some(PER_GROUP))
        .finish(&mut frame)
        .expect("a written Parquet");
    path
}

/// What one expression cost, asked of the fixture: the values read out of
/// the file, and what the expression answered. `None` asks nothing, which
/// is the cost of opening the file and calculating the sheet.
fn cost(expr: Option<&str>) -> (usize, String) {
    let dir = scratch("rowgroups");
    fixture(&dir);
    let (mut program, mut diags) = compile(TABLE, 0);
    assert!(diags.is_empty(), "{diags:?}");
    let tables = table::read(&mut program, &dir, &mut diags);
    assert!(diags.is_empty(), "{diags:?}");
    let program = program;
    let engine = Engine::with_tables(
        &program,
        Options {
            base: Some(dir),
            // Forced, so that the row count cannot be what decides it: a
            // file of five thousand rows is over the threshold anyway.
            lowering: Lowering::Always,
            exact: true,
            ..Options::default()
        },
        tables,
    );
    engine.run();
    let answer = match expr {
        None => String::new(),
        Some(expr) => {
            let (node, _) = compile_expr(&program, expr, 1).expect("the expression checks");
            let value = engine.eval_top(&node).expect("a value");
            engine.show(&value, true)
        }
    };
    (engine.rows_read(), answer)
}

/// The row groups that many values came out of, the file being read from
/// its first row group onwards and one column at a time.
fn groups(rows_read: usize) -> usize {
    rows_read.div_ceil(PER_GROUP)
}

#[test]
fn the_fixture_is_a_real_multi_row_group_file() {
    // Not a test of omasheet: a test of the fixture, because every number
    // below means nothing if the writer put the rows in one group.
    let dir = scratch("rowgroups-fixture");
    let path = fixture(&dir);
    let file = std::fs::File::open(&path).expect("the file just written");
    let mut reader = ParquetReader::new(file);
    let metadata = reader.get_metadata().expect("a readable footer").clone();
    assert_eq!(metadata.num_rows, ROWS);
    assert_eq!(metadata.row_groups.len(), GROUPS);
    for group in &metadata.row_groups {
        assert_eq!(group.num_rows(), PER_GROUP);
    }
}

#[test]
fn opening_a_table_reads_no_row_group() {
    // The schema and the row count are in the footer, so a file of five
    // thousand rows opens and calculates for nothing.
    let (rows_read, _) = cost(None);
    assert_eq!(rows_read, 0);
    assert_eq!(groups(rows_read), 0);
}

#[test]
fn a_sum_reads_every_row_group_of_the_one_column() {
    // Every row group, which a sum has to have, and one column's worth of
    // them rather than two: the projection is what keeps `Group` out of it.
    let (rows_read, answer) = cost(Some("Numbers.Value.sum()"));
    assert_eq!(answer, "12497500");
    assert_eq!(rows_read, ROWS);
    assert_eq!(groups(rows_read), GROUPS);
}

#[test]
fn the_first_five_rows_are_read_out_of_every_row_group() {
    // Task 6.2's open half, in a number. The answer needs the first five
    // rows, which are the first row group's; the whole column is read to
    // give them, so five row groups are read where one would do.
    let (rows_read, answer) = cost(Some("(Numbers |> head(5)).Value"));
    assert_eq!(answer, "[0, 1, 2, 3, 4]");
    assert_eq!(rows_read, ROWS);
    assert_eq!(
        groups(rows_read),
        GROUPS,
        "the limit reached the scan: 6.2 is done, and this is its new figure"
    );
}

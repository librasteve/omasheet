// Copyright (c) 2026 Stephen Roe

//! Where the time goes: each phase of calculating a file, timed separately,
//! at four row counts and with lowering refused and forced (tasks 7.3 and
//! 7.4).
//!
//! It is a measurement and not an assertion, so it is ignored by default.
//! Run it in release, which is the only build whose figures mean anything:
//!
//! ```text
//! cargo test -p omasheet-engine --test timings --release -- --ignored --nocapture
//! ```
//!
//! The four phases are the ones the app pays on every keystroke, in the
//! order `Document::rebuild` pays them: `compile`, building the engine,
//! `run`, and the snapshot — every cell of every sheet formatted as it is
//! shown. The threshold D73 asks for is wherever `run` with lowering forced
//! first beats `run` with it refused by more than the rest of the rebuild
//! costs, which is what the table is printed to show.

use omasheet_engine::eval::Attempt;
use omasheet_engine::omx::{compile, compile_expr};
use omasheet_engine::{Engine, Lowering, Options, table};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The allocator, counting: what is held now and the most held since the
/// count was last reset, so that a calculation's peak memory is a figure
/// beside its time (`hold-less-of-a-big-file` 0.1). Two atomic additions an
/// allocation is all it costs the figures beside it.
struct Counting;

static HELD: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = HELD.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        HELD.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The most held above what was held when it was called, while `f` runs.
fn peak_of<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = HELD.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let out = f();
    (out, PEAK.load(Ordering::Relaxed).saturating_sub(base))
}

/// The row counts the threshold is looked for between.
const ROWS: [usize; 4] = [10, 1_000, 100_000, 1_000_000];

/// How many times each figure is taken. The quickest of them is the one
/// reported, as it is for any latency: a slower run has had something done
/// to it, and two runs of the same work came out 20% apart when this was
/// measured once each.
const RUNS: usize = 3;

/// `examples/budget.omx`'s `Sales` sheet grown to `rows` rows: the same
/// columns, the same four formulas, and the same constant.
///
/// Its `Summary` sheet is left out on purpose. Every line of it is an
/// aggregate or a lookup over the whole of `Sales`, none of which is lowered
/// yet — that is section 6 — so including it would time a row-wise scan of a
/// million rows and hide the phases this is asking about.
fn budget(rows: usize) -> String {
    let mut text = String::with_capacity(rows * 64);
    text.push_str(
        "const TaxRate = 20%\n\n\
         sheet Sales\n\n\
         Closed : Datetime\n\
         Margin : Percent\n\
         Growth : Percent\n\n\
         | Month | Closed | Revenue | Cost | Profit | Tax | Margin | Growth |\n\
         | ----- | ------ | ------- | ---- | ------ | --- | ------ | ------ |\n",
    );
    for r in 0..rows {
        // A month that cycles, a date-time that moves, and revenue and cost
        // that are not the same number twice running.
        let month = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"][r % 6];
        let day = 1 + (r % 28);
        let revenue = 10_000 + (r % 997) * 13;
        let cost = 6_000 + (r % 89) * 7;
        text.push_str(&format!(
            "| {month} | 2025-02-{day:02}T17:30 | {revenue} | {cost} | * | * | * | * |\n"
        ));
    }
    text.push_str(
        "\nProfit := Revenue - Cost\n\
         Tax    := Profit * TaxRate\n\
         Margin := Profit / Revenue\n\
         Growth := Revenue / Sales[Revenue; *-1] - 1\n",
    );
    text
}

/// `examples/budget.omx`'s `Summary` sheet, over whatever `Sales` holds: a
/// line per aggregate, and two over a selection.
///
/// The example's own `Best month` line is left out. It is
/// `Sales[; Profit == Sales.Profit.max()].Month`, and an aggregate inside a
/// condition is evaluated once per row with nothing memoising it, so
/// row-wise it is quadratic — at a million rows a million maxima of a
/// million values, which is not a measurement anybody waits for. The two
/// lines over a selection here ask what it asked without nesting.
///
/// Four of these are a whole column, which is what [`Engine::run_agg`]
/// takes; two are over a selection, which it does not, so the condition is
/// one Polars pass (task 6.1) and the sum of what it picked is the engine's.
const SUMMARY: &str = "\n\
     sheet Summary\n\n\
     | Item     | Value                                  |\n\
     | -------- | -------------------------------------- |\n\
     | Revenue  | = Sales.Revenue.sum()                  |\n\
     | Profit   | = Sales.Profit.sum()                   |\n\
     | Margin   | = Sales.Margin.mean()                  |\n\
     | Closed   | = Sales.Closed.max()                   |\n\
     | Big sum  | = Sales[; Profit > 10000].Revenue.sum() |\n\
     | Big rows | = Sales[; Profit > 10000].Revenue.len() |\n";

/// How many lowerable columns the wide sheet is measured at.
const COLS: [usize; 3] = [5, 20, 50];

/// A sheet of `rows` rows and `cols` columns, every one of them lowerable:
/// `C0 := Revenue - Cost`, and so on.
///
/// Short, it is the other shape, and the one the threshold is really for: a
/// plan is built, optimised and collected per column, so a small sheet pays
/// that cost as many times as it has columns where `budget.omx` pays it
/// once. Long, it is the shape the snapshot question needs, since every
/// column of it is one the dataframe holds.
fn wide(cols: usize, rows: usize) -> String {
    let names: Vec<String> = (0..cols).map(|c| format!("C{c}")).collect();
    let mut text = String::from("sheet Wide\n\n| Revenue | Cost |");
    for name in &names {
        text.push_str(&format!(" {name} |"));
    }
    text.push_str("\n| ------- | ---- |");
    for _ in &names {
        text.push_str(" --- |");
    }
    text.push('\n');
    for r in 0..rows {
        text.push_str(&format!("| {} | {} |", 10_000 + r * 13, 6_000 + r * 7));
        for _ in &names {
            text.push_str(" * |");
        }
        text.push('\n');
    }
    text.push('\n');
    for name in &names {
        text.push_str(&format!("{name} := Revenue - Cost\n"));
    }
    text
}

/// Which way the snapshot is taken: every cell asked for in turn, as
/// `Document::rebuild` did before task 7.2, or a column at a time, as it
/// does now (D75).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Snap {
    Cells,
    Columns,
}

struct Timing {
    compile: Duration,
    build: Duration,
    run: Duration,
    snapshot: Duration,
    lowered: usize,
    refused: usize,
    /// Aggregates over a whole column, taken and refused.
    aggs: (usize, usize),
    /// And conditions picking rows.
    picks: (usize, usize),
}

impl Timing {
    fn total(&self) -> Duration {
        self.compile + self.build + self.run + self.snapshot
    }

    /// The quickest each phase was over several runs. The phases are taken
    /// separately rather than the quickest run taken whole, so that one
    /// slow phase does not throw away good figures beside it.
    fn best(runs: Vec<Timing>) -> Timing {
        let least = |f: &dyn Fn(&Timing) -> Duration| runs.iter().map(f).min().unwrap_or_default();
        Timing {
            compile: least(&|t| t.compile),
            build: least(&|t| t.build),
            run: least(&|t| t.run),
            snapshot: least(&|t| t.snapshot),
            lowered: runs[0].lowered,
            refused: runs[0].refused,
            aggs: runs[0].aggs,
            picks: runs[0].picks,
        }
    }
}

/// One file calculated one way, with each phase timed on its own.
fn measure(text: &str, lowering: Lowering, snap: Snap) -> Timing {
    let start = Instant::now();
    let (program, diags) = compile(text, 0);
    let compile_time = start.elapsed();
    assert!(diags.is_empty(), "{diags:?}");

    let start = Instant::now();
    let engine = Engine::with_options(
        &program,
        Options {
            lowering,
            ..Options::default()
        },
    );
    let build = start.elapsed();

    let start = Instant::now();
    engine.run();
    let run = start.elapsed();

    // The snapshot: every cell of every sheet, formatted as it is shown.
    // `black_box` so that the formatting is not optimised away for being
    // thrown back. The formatting is the same work either way; what the two
    // styles differ in is how the value reaches it.
    let start = Instant::now();
    for (t, sheet) in program.sheets.iter().enumerate() {
        for c in 0..sheet.cols.len() {
            match snap {
                Snap::Cells => {
                    for r in 0..sheet.nrows {
                        std::hint::black_box(engine.show(&engine.cell_shown(t, c, r), false));
                    }
                }
                // As `Document::rebuild` does it: the column at once
                // where there is anything to gain by it, and a cell at a
                // time where there is not.
                Snap::Columns => match engine.column_shown(t, c) {
                    Some(values) => {
                        for value in values {
                            std::hint::black_box(engine.show(&value, false));
                        }
                    }
                    None => {
                        for r in 0..sheet.nrows {
                            std::hint::black_box(engine.show(&engine.cell_shown(t, c, r), false));
                        }
                    }
                },
            }
        }
    }
    let snapshot = start.elapsed();

    let attempts = engine.attempts();
    let aggregates = engine.aggregates();
    let filters = engine.filters();
    Timing {
        compile: compile_time,
        build,
        run,
        snapshot,
        lowered: attempts.iter().filter(|a| a.2 == Attempt::Lowered).count(),
        refused: attempts.iter().filter(|a| a.2 == Attempt::Refused).count(),
        aggs: (
            aggregates
                .iter()
                .filter(|a| a.2 == Attempt::Lowered)
                .count(),
            aggregates
                .iter()
                .filter(|a| a.2 != Attempt::Lowered)
                .count(),
        ),
        picks: (
            filters.iter().filter(|f| f.1 == Attempt::Lowered).count(),
            filters.iter().filter(|f| f.1 != Attempt::Lowered).count(),
        ),
    }
}

/// Milliseconds, to the tenth, which is the unit a keystroke is judged in.
fn ms(d: Duration) -> String {
    format!("{:.1}", d.as_secs_f64() * 1_000.0)
}

/// Polars starts a thread pool of its own the first time anything collects,
/// and whichever calculation is first pays for it. It is a one-off and not
/// what a keystroke costs, so it is measured on a file of its own and taken
/// out of the way before the table below is timed.
fn first_collect() -> Duration {
    let text = budget(2);
    let start = Instant::now();
    measure(&text, Lowering::Always, Snap::Columns);
    start.elapsed()
}

#[test]
#[ignore = "a measurement, not an assertion: run it in release with --ignored --nocapture"]
fn where_the_time_goes() {
    println!(
        "\nfirst collect, Polars' thread pool started with it: {} ms",
        ms(first_collect())
    );
    println!(
        "\n{:>9}  {:<7}  {:>9}  {:>9}  {:>9}  {:>9}  {:>9}  columns",
        "rows", "mode", "compile", "build", "run", "snapshot", "total"
    );
    for rows in ROWS {
        let text = budget(rows);
        for lowering in [Lowering::Never, Lowering::Always] {
            let t = Timing::best(
                (0..RUNS)
                    .map(|_| measure(&text, lowering, Snap::Columns))
                    .collect(),
            );
            println!(
                "{rows:>9}  {:<7}  {:>9}  {:>9}  {:>9}  {:>9}  {:>9}  {} lowered, {} refused",
                format!("{lowering:?}"),
                ms(t.compile),
                ms(t.build),
                ms(t.run),
                ms(t.snapshot),
                ms(t.total()),
                t.lowered,
                t.refused,
            );
        }
    }
    println!(
        "\nmilliseconds; `budget.omx`'s Sales sheet, its Summary left out; \
         the thread pool is warm throughout\n"
    );

    println!(
        "{:>9}  {:<7}  {:>9}  {:>9}  {:>9}  {:>9}  {:>9}  columns",
        "columns", "mode", "compile", "build", "run", "snapshot", "total"
    );
    for cols in COLS {
        let text = wide(cols, 10);
        for lowering in [Lowering::Never, Lowering::Always] {
            let t = Timing::best(
                (0..RUNS)
                    .map(|_| measure(&text, lowering, Snap::Columns))
                    .collect(),
            );
            println!(
                "{cols:>9}  {:<7}  {:>9}  {:>9}  {:>9}  {:>9}  {:>9}  {} lowered, {} refused",
                format!("{lowering:?}"),
                ms(t.compile),
                ms(t.build),
                ms(t.run),
                ms(t.snapshot),
                ms(t.total()),
                t.lowered,
                t.refused,
            );
        }
    }
    println!("\nmilliseconds; ten rows, every column lowerable\n");

    // Tasks 7.1 and 7.2: the snapshot alone, each style on an engine of
    // its own so that neither is reading what the other left behind. Two
    // shapes, because the two have nothing to do with each other: in
    // `budget.omx` one computed column of eight is lowered, so seven of
    // them are the row loop either way; in the wide sheet every column is
    // one the dataframe holds, which is the case D75 is about.
    println!(
        "{:>9}  {:<9}  {:<7}  {:>9}  {:>9}",
        "rows", "sheet", "mode", "cells", "columns"
    );
    for rows in ROWS {
        for (name, text) in [("budget", budget(rows)), ("wide", wide(5, rows))] {
            for lowering in [Lowering::Never, Lowering::Always] {
                let snapshot = |snap| {
                    Timing::best((0..RUNS).map(|_| measure(&text, lowering, snap)).collect())
                        .snapshot
                };
                println!(
                    "{rows:>9}  {name:<9}  {:<7}  {:>9}  {:>9}",
                    format!("{lowering:?}"),
                    ms(snapshot(Snap::Cells)),
                    ms(snapshot(Snap::Columns)),
                );
            }
        }
    }
    println!("\nmilliseconds; the snapshot alone, per cell against per column\n");

    // The shape the case for lowering rests on, which `budget.omx` is not:
    // every column of it lowers, so `run` is columnar throughout rather
    // than one column in four.
    println!(
        "{:>9}  {:<7}  {:>9}  {:>9}  {:>9}  {:>9}  columns",
        "rows", "mode", "compile", "run", "snapshot", "total"
    );
    for rows in ROWS {
        let text = wide(5, rows);
        for lowering in [Lowering::Never, Lowering::Always] {
            let t = Timing::best(
                (0..RUNS)
                    .map(|_| measure(&text, lowering, Snap::Columns))
                    .collect(),
            );
            println!(
                "{rows:>9}  {:<7}  {:>9}  {:>9}  {:>9}  {:>9}  {} lowered, {} refused",
                format!("{lowering:?}"),
                ms(t.compile),
                ms(t.run),
                ms(t.snapshot),
                ms(t.total()),
                t.lowered,
                t.refused,
            );
        }
    }
    println!("\nmilliseconds; five columns, every one of them lowerable\n");

    // The `Summary` sheet, which is what section 6 is for: an aggregate
    // over a whole column taken by Polars, and a condition picking rows in
    // one pass of it.
    println!(
        "{:>9}  {:<7}  {:>9}  {:>9}  {:>9}  {:>9}  aggregates / selections",
        "rows", "mode", "compile", "run", "snapshot", "total"
    );
    for rows in ROWS {
        let text = budget(rows) + SUMMARY;
        for lowering in [Lowering::Never, Lowering::Always] {
            let t = Timing::best(
                (0..RUNS)
                    .map(|_| measure(&text, lowering, Snap::Columns))
                    .collect(),
            );
            println!(
                "{rows:>9}  {:<7}  {:>9}  {:>9}  {:>9}  {:>9}  {} taken {} refused / {} taken {} refused",
                format!("{lowering:?}"),
                ms(t.compile),
                ms(t.run),
                ms(t.snapshot),
                ms(t.total()),
                t.aggs.0,
                t.aggs.1,
                t.picks.0,
                t.picks.1,
            );
        }
    }
    println!(
        "\nmilliseconds; `budget.omx`'s Sales and its Summary, four aggregates \
         over a whole column and two over a selection\n"
    );
}

/// The row counts a whole-column read is measured at (0.1).
const COLUMN_ROWS: [usize; 2] = [100_000, 1_000_000];

/// A table of `rows` rows in a Parquet written in `dir`: a whole number and
/// a float column, neither the same value twice running.
fn column_table(dir: &std::path::Path, rows: usize) {
    use polars::prelude::*;
    let whole: Vec<i64> = (0..rows).map(|r| (r % 9973) as i64).collect();
    let real: Vec<f64> = (0..rows).map(|r| (r % 9973) as f64 / 7.0).collect();
    let mut frame = df!["Whole" => whole, "Real" => real].expect("a frame");
    let mut file = std::fs::File::create(dir.join("column.parquet")).expect("a writable file");
    ParquetWriter::new(&mut file)
        .finish(&mut frame)
        .expect("a written Parquet");
}

/// One expression over the table, timed and with its peak memory, from a
/// fresh engine: the file's column is read inside it either way, so what
/// the two lowerings differ by is what the answer was made from.
fn column_cost(dir: &std::path::Path, expr: &str, lowering: Lowering) -> (Duration, usize, String) {
    let text = "view T := dataframe['column.parquet']\n\nWhole : Integer\nReal  : Float\n";
    let (mut program, mut diags) = compile(text, 0);
    assert!(diags.is_empty(), "{diags:?}");
    let tables = table::read(&mut program, dir, &mut diags);
    assert!(diags.is_empty(), "{diags:?}");
    let program = program;
    let engine = Engine::with_tables(
        &program,
        Options {
            base: Some(dir.to_path_buf()),
            lowering,
            ..Options::default()
        },
        tables,
    );
    engine.run();
    let (node, _) = compile_expr(&program, expr, 1).expect("the expression checks");
    let start = Instant::now();
    let (answer, peak) = peak_of(|| engine.eval_top(&node));
    let time = start.elapsed();
    let answer = answer.map_or("<error>".into(), |v| engine.show(&v, true));
    (time, peak, answer)
}

/// What a whole-column read costs as a `Value` a cell against the Polars
/// column it was built from (`hold-less-of-a-big-file` 0.1, which D79 waits
/// on): a `len()` and a `sum()` of a whole number and of a float column,
/// with the engine reading a value a cell and with Polars taking the
/// column. Time is the quickest of [`RUNS`]; memory is the peak above what
/// the engine held before the expression.
#[test]
#[ignore = "a measurement: run in release with --ignored --nocapture"]
fn a_column_as_values_against_the_column() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("column-width");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a writable scratch directory");
    println!(
        "\n{:>9}  {:<16}  {:<7}  {:>9}  {:>10}  answer",
        "rows", "expression", "mode", "time", "peak MiB"
    );
    for rows in COLUMN_ROWS {
        column_table(&dir, rows);
        for expr in [
            "T.Whole.len()",
            "T.Whole.sum()",
            "T.Real.len()",
            "T.Real.sum()",
        ] {
            for lowering in [Lowering::Never, Lowering::Always] {
                let runs: Vec<_> = (0..RUNS)
                    .map(|_| column_cost(&dir, expr, lowering))
                    .collect();
                let time = runs.iter().map(|r| r.0).min().unwrap_or_default();
                let peak = runs.iter().map(|r| r.1).min().unwrap_or_default();
                println!(
                    "{rows:>9}  {expr:<16}  {:<7}  {:>9}  {:>10.1}  {}",
                    format!("{lowering:?}"),
                    ms(time),
                    peak as f64 / (1024.0 * 1024.0),
                    runs[0].2,
                );
            }
        }
    }
    println!(
        "\nmilliseconds; a fresh engine each run, the file's column read inside \
         the expression either way\n"
    );
}

/// What a filter by expression costs on a big table (filter-by-expression
/// task 3.6): checking it as it is typed, and applying it, which Polars
/// does over the column. The files are the big ones of `demo_data`, not in
/// Git; a file that is not there is passed over. `OMASHEET_BIG` names
/// another, as a path.
#[test]
#[ignore = "a measurement: run in release with --ignored --nocapture"]
fn a_filter_on_a_big_table() {
    use omasheet_engine::Document;
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let mut files = vec![
        format!("{root}/demo_data/fhvhv_tripdata_2019-03.parquet"),
        format!("{root}/demo_data/yellow_tripdata_2026-01.parquet"),
    ];
    if let Ok(more) = std::env::var("OMASHEET_BIG") {
        files.insert(0, more);
    }
    for file in files.iter().filter(|f| std::path::Path::new(f).exists()) {
        let started = Instant::now();
        let doc = Document::from_text(&format!(
            "view T := dataframe[{}]\n",
            omasheet_engine::omx::lexer::quoted(file)
        ));
        let sheet = &doc.snapshot().sheets[0];
        println!(
            "\n{file}: {} rows, {} columns, opened in {}",
            sheet.nrows,
            sheet.columns.len(),
            ms(started.elapsed())
        );
        let pick = |types: &[&str]| {
            sheet
                .columns
                .iter()
                .position(|c| types.contains(&c.ty.as_str()))
        };
        let mut cases: Vec<(usize, &str)> = Vec::new();
        if let Some(c) = pick(&["Float", "Decimal"]) {
            cases.extend([(c, "* > 20"), (c, "10 < * <= 20"), (c, "*.is_null()")]);
        }
        if let Some(c) = pick(&["Integer", "Int"]) {
            cases.extend([(c, "* in [1, 2]"), (c, "* != 1")]);
        }
        if let Some(c) = pick(&["String"]) {
            cases.extend([
                (c, "*.starts_with('B'..'C')"),
                (c, "*.contains('02')"),
                (c, "*.matches('^HV000[34]$')"),
            ]);
        }
        if let Some(c) = pick(&["Datetime"]) {
            cases.extend([(c, "*.day == 15"), (c, "* >= 2019-03-15T00:00")]);
        }
        for (c, expr) in cases {
            let name = sheet.columns[c].name.clone();
            let checked = Instant::now();
            let check = doc.check_filter(0, c, expr);
            let check_time = checked.elapsed();
            let filter = vec![(name.clone(), expr.to_string())];
            let first = Instant::now();
            let rows = doc.shown_rows(0, &filter);
            let first_time = first.elapsed();
            let again = Instant::now();
            let _ = doc.shown_rows(0, &filter);
            let again_time = again.elapsed();
            println!(
                "  {name:<24} {expr:<28} check {:>8}  apply {:>9} (again {:>9})  {}",
                ms(check_time),
                ms(first_time),
                ms(again_time),
                match (&check, &rows) {
                    (Err(why), _) | (_, Err(why)) => format!("error: {why}"),
                    (Ok(()), Ok(rows)) => format!("{} rows", rows.len()),
                }
            );
        }
    }
}

/// open-a-big-csv 0.3 and 5.1: a big CSV opened for the app, shown from
/// its sample, then copied to Parquet and opened from the copy, and opened
/// again with the copy current. The copies go to a cache of their own.
#[test]
#[ignore = "a measurement, not an assertion: run it in release with --ignored --nocapture"]
fn a_big_csv_through_its_copy() {
    use omasheet_engine::Document;
    use omasheet_engine::table;
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let csv = std::env::var("OMASHEET_BIG_CSV")
        .unwrap_or_else(|_| format!("{root}/demo_data/fhvhv_tripdata_2021-06.csv"));
    let csv = std::path::PathBuf::from(csv);
    if !csv.exists() {
        return;
    }
    let cache = std::env::temp_dir().join(format!("omasheet-csv-{}", std::process::id()));
    // SAFETY: the one test of this binary that sets it, before any reads.
    unsafe { std::env::set_var("OMASHEET_CACHE", &cache) };
    let text = format!(
        "view T := dataframe[{}]\n",
        omasheet_engine::omx::lexer::quoted(&csv.to_string_lossy())
    );
    let names = omasheet_engine::datefmt::Names::default();
    let style = omasheet_engine::omx::date::Style::ISO;

    let started = Instant::now();
    table::sample(&csv).unwrap();
    println!("sample written in {}", ms(started.elapsed()));
    let started = Instant::now();
    let doc = Document::on_screen(&text, style, names.clone());
    let sample = doc.snapshot().sheets[0].nrows;
    println!(
        "\n{}: opened from its sample ({sample} rows) in {}",
        csv.display(),
        ms(started.elapsed())
    );
    assert_eq!(doc.to_convert(), vec![csv.clone()]);

    let started = Instant::now();
    let copy = table::convert(&csv).unwrap();
    let size = std::fs::metadata(&copy).unwrap().len();
    println!(
        "copied in {}, {} MB",
        ms(started.elapsed()),
        size / 1_000_000
    );

    let mut doc = doc;
    let started = Instant::now();
    assert!(doc.converted(&csv));
    println!(
        "read from the copy, {} rows, in {}",
        doc.snapshot().sheets[0].nrows,
        ms(started.elapsed())
    );

    let started = Instant::now();
    let again = Document::on_screen(&text, style, names);
    println!(
        "opened again, copy current: {} rows in {}",
        again.snapshot().sheets[0].nrows,
        ms(started.elapsed())
    );
    assert!(again.to_convert().is_empty());
    std::fs::remove_dir_all(&cache).unwrap();
}

/// Run what an open runs, over a small file of its own, so that the pages of
/// the program that an open touches are back in memory after a purge, which
/// drops them with everything else. What is timed after it is then the data
/// being cold and not the code (`open-a-big-csv` 5.1).
fn prime_code() {
    let dir = std::env::temp_dir().join(format!("omasheet-prime-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("prime.csv");
    let mut text = String::from("a,b,c\n");
    for i in 0..3_000 {
        text.push_str(&format!("{i},{}.5,x{i}\n", i % 7));
    }
    std::fs::write(&file, text).unwrap();
    let view = format!(
        "view T := dataframe[{}]\n",
        omasheet_engine::omx::lexer::quoted(&file.to_string_lossy())
    );
    table::sample(&file).unwrap();
    let names = omasheet_engine::datefmt::Names::default();
    let style = omasheet_engine::omx::date::Style::ISO;
    let _ = omasheet_engine::Document::on_screen(&view, style, names);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Drops the operating system's page cache, so the next read of a file is a
/// cold one. It needs privilege (`purge` on macOS, a write to
/// `/proc/sys/vm/drop_caches` on Linux), so it says whether it managed: a
/// run that cannot is reported as warm only, never as cold.
fn drop_page_cache() -> bool {
    use std::process::{Command, Stdio};
    let run = |program: &str, args: &[&str]| {
        Command::new(program)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    if cfg!(target_os = "macos") {
        run("purge", &[]) || run("sudo", &["-n", "purge"])
    } else {
        run("sh", &["-c", "sync; echo 3 > /proc/sys/vm/drop_caches"])
            || run(
                "sudo",
                &["-n", "sh", "-c", "sync; echo 3 > /proc/sys/vm/drop_caches"],
            )
    }
}

/// The process's peak resident set, in MiB. It only ever rises, so it is
/// printed once for each file and is the high-water mark of everything run
/// so far, mapped file pages included — which the counting allocator's
/// figure leaves out.
fn peak_rss_mib() -> f64 {
    #[cfg(unix)]
    {
        let mut usage = unsafe { std::mem::zeroed::<libc::rusage>() };
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } == 0 {
            // Bytes on macOS, kilobytes elsewhere.
            let scale = if cfg!(target_os = "macos") {
                1.0
            } else {
                1024.0
            };
            return usage.ru_maxrss as f64 * scale / (1024.0 * 1024.0);
        }
    }
    0.0
}

/// open-a-big-csv 0.1: the figures the change is measured against, taken
/// from the synthetic files `demo_data/generate.sh` makes, with the page
/// cache warm and, where it can be dropped, cold. For each file, in the
/// order the app pays them:
///
/// - **open**: the sample written and a `Document` drawn from it, which is
///   the time to the first screen (target ≤ 50 ms);
/// - **copy**: the streaming conversion to Parquet, with its size;
/// - **reopen**: a `Document` over the file with the copy current (≤ 50 ms);
/// - **sum, first**: `T.qty.sum()` over the copy, and **again** once the
///   engine has it (≤ 50 ms from the cache).
///
/// Time is wall time of one run, since a cold figure cannot be repeated;
/// memory is the peak the counting allocator saw above what was held before
/// (mapped file pages are not in it; `peak RSS` after each file is). The
/// scroll, jump and edit targets (5.2, 5.3) are frame times in the window
/// and are taken in the app. `OMASHEET_BENCH` names the sizes, default
/// `10mb,100mb,1.1gb`; a file that is not there is passed over.
///
/// ```text
/// demo_data/generate.sh
/// cargo test -p omasheet-engine --test timings --release -- --ignored --nocapture the_targets
/// ```
///
/// Cold needs privilege: run `sudo -v` first, or the cold rows say so.
#[test]
#[ignore = "a measurement, not an assertion: run it in release with --ignored --nocapture"]
fn the_targets_on_a_synthetic_csv() {
    use omasheet_engine::Document;
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../demo_data");
    let sizes = std::env::var("OMASHEET_BENCH").unwrap_or_else(|_| "10mb,100mb,1.1gb".into());
    // `OMASHEET_BENCH_EXT=json` takes the JSON files instead of the CSVs.
    let ext = std::env::var("OMASHEET_BENCH_EXT").unwrap_or_else(|_| "csv".into());
    let cache = std::env::temp_dir().join(format!("omasheet-bench-{}", std::process::id()));
    // SAFETY: the one test of this binary that sets it, before any reads.
    unsafe { std::env::set_var("OMASHEET_CACHE", &cache) };
    let names = omasheet_engine::datefmt::Names::default();
    let style = omasheet_engine::omx::date::Style::ISO;
    let cold_possible = drop_page_cache();
    println!(
        "\n{:<8} {:<5} {:<12} {:>10} {:>10}  note",
        "file", "cache", "step", "time", "peak MiB"
    );
    let row =
        |size: &str, cache_state: &str, step: &str, time: Duration, peak: usize, note: &str| {
            println!(
                "{size:<8} {cache_state:<5} {step:<12} {:>10} {:>10.1}  {note}",
                ms(time),
                peak as f64 / (1024.0 * 1024.0)
            );
        };
    for size in sizes.split(',') {
        let csv = std::path::PathBuf::from(format!("{root}/synthetic-{size}.{ext}"));
        if !csv.exists() {
            println!(
                "{size}: {} is not there; demo_data/generate.sh makes it",
                csv.display()
            );
            continue;
        }
        let text = format!(
            "view T := dataframe[{}]\n",
            omasheet_engine::omx::lexer::quoted(&csv.to_string_lossy())
        );
        for state in ["warm", "cold", "cold+code"] {
            if state.starts_with("cold") && !cold_possible {
                println!("{size:<8} cold  (page cache could not be dropped: needs sudo)");
                continue;
            }
            // Each state starts from nothing written, and from a cache the
            // next read of the file has to fetch from disk (cold) or has
            // just had read through (warm).
            let _ = std::fs::remove_dir_all(&cache);
            if state.starts_with("cold") {
                drop_page_cache();
                if state == "cold+code" {
                    prime_code();
                }
            } else {
                let _ = std::fs::read(&csv).map(|b| b.len());
            }

            let started = Instant::now();
            let (doc, peak) = peak_of(|| {
                table::sample(&csv).unwrap();
                Document::on_screen(&text, style, names.clone())
            });
            let sample = doc.snapshot().sheets[0].nrows;
            row(
                size,
                state,
                "open",
                started.elapsed(),
                peak,
                &format!("{sample} rows shown"),
            );

            let started = Instant::now();
            let (copy, peak) = peak_of(|| table::convert(&csv).unwrap());
            let bytes = std::fs::metadata(&copy).unwrap().len();
            row(
                size,
                state,
                "copy",
                started.elapsed(),
                peak,
                &format!("{} MB", bytes / 1_000_000),
            );
            drop(doc);

            if state.starts_with("cold") {
                drop_page_cache();
            }
            let started = Instant::now();
            let (again, peak) = peak_of(|| Document::on_screen(&text, style, names.clone()));
            row(
                size,
                state,
                "reopen",
                started.elapsed(),
                peak,
                &format!("{} rows", again.snapshot().sheets[0].nrows),
            );
            assert!(again.to_convert().is_empty());
            drop(again);

            // The first aggregate and the second, from one engine.
            let (mut program, mut diags) = compile(&text, 0);
            assert!(diags.is_empty(), "{diags:?}");
            let tables = table::read(&mut program, std::path::Path::new(root), &mut diags);
            assert!(diags.is_empty(), "{diags:?}");
            let program = program;
            let engine = Engine::with_tables(
                &program,
                Options {
                    base: Some(root.into()),
                    ..Options::default()
                },
                tables,
            );
            engine.run();
            let (node, _) =
                compile_expr(&program, "T.qty.sum()", 1).expect("the expression checks");
            if state.starts_with("cold") {
                drop_page_cache();
            }
            for step in ["sum, first", "sum, again"] {
                let started = Instant::now();
                let (answer, peak) = peak_of(|| engine.eval_top(&node));
                let answer = answer.map_or("<error>".into(), |v| engine.show(&v, true));
                row(
                    size,
                    state,
                    step,
                    started.elapsed(),
                    peak,
                    &format!("T.qty.sum() = {answer}"),
                );
            }
        }
        println!("{size:<8} peak RSS so far {:.0} MiB", peak_rss_mib());
    }
    println!(
        "\nmilliseconds, one run each; peak is the counting allocator's, above what \
         was held before the step\nscroll, jump and edit (5.2, 5.3) are frame times in \
         the app\n"
    );
    let _ = std::fs::remove_dir_all(&cache);
}

/// open-a-big-csv 1.3: a `table` over a big Parquet file, against the
/// targets, in the app's own terms. Opened as the app opens it
/// (`Document::on_screen`), then a screen of rows asked for as the grid asks
/// for them (`Document::rows`: 40 rows, every column, calculated and
/// formatted): the first screen, a jump to the last rows and to the middle,
/// and a scroll of 200 frames three rows at a time. Where the file would fit
/// in memory (D157: here under 2.5 GB to hold) it is then held whole, as the
/// app's worker does, and the same asked again, since that is what the grid
/// moves over to.
///
/// Targets: open to first screen ≤ 50 ms, and a frame ≤ 16 ms. This takes
/// the data's share of the frame; drawing it in the window is the app's,
/// and is looked at by hand. The files are those of `demo_data`; the 41M-row
/// NYC taxi year of `hold-less-of-a-big-file` is added by `OMASHEET_BIG`
/// (a path). Cold needs privilege to drop the page cache (`sudo -v`).
///
/// ```text
/// cargo test -p omasheet-engine --test timings --release -- --ignored --nocapture a_parquet_table
/// ```
#[test]
#[ignore = "a measurement, not an assertion: run it in release with --ignored --nocapture"]
fn a_parquet_table_against_the_targets() {
    use omasheet_engine::Document;
    const SCREEN: usize = 40;
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let mut files = vec![
        format!("{root}/demo_data/fhvhv_tripdata_2019-03.parquet"),
        format!("{root}/demo_data/yellow_tripdata_2026-01.parquet"),
    ];
    if let Ok(more) = std::env::var("OMASHEET_BIG") {
        files.insert(0, more);
    }
    let names = omasheet_engine::datefmt::Names::default();
    let style = omasheet_engine::omx::date::Style::ISO;
    let cold_possible = drop_page_cache();
    println!(
        "\n{:<6} {:<7} {:<22} {:>9} {:>9}  note",
        "cache", "state", "step", "time ms", "peak MiB"
    );
    for file in files.iter().filter(|f| std::path::Path::new(f).exists()) {
        println!("\n{file}");
        let text = format!(
            "view T := dataframe[{}]\n",
            omasheet_engine::omx::lexer::quoted(file)
        );
        for cache in ["warm", "cold"] {
            if cache == "cold" && !cold_possible {
                println!("cold   (page cache could not be dropped: needs sudo)");
                continue;
            }
            if cache == "cold" {
                drop_page_cache();
            } else {
                // Read through once, so that it is warm.
                let _ = std::fs::read(file).map(|b| b.len());
            }
            let row = |state: &str, step: &str, time: Duration, peak: usize, note: &str| {
                println!(
                    "{cache:<6} {state:<7} {step:<22} {:>9} {:>9.1}  {note}",
                    ms(time),
                    peak as f64 / (1024.0 * 1024.0)
                );
            };
            let started = Instant::now();
            let (mut doc, peak) = peak_of(|| Document::on_screen(&text, style, names.clone()));
            let nrows = doc.row_count(0);
            row(
                "paged",
                "open",
                started.elapsed(),
                peak,
                &format!("{nrows} rows"),
            );
            let mut held = false;
            for state in ["paged", "held"] {
                if state == "held" {
                    let wanted = doc.to_hold();
                    let Some(want) = wanted.first().filter(|h| h.bytes <= 2_500_000_000) else {
                        println!(
                            "{cache:<6} held    (not held: {} MB to hold)",
                            wanted.first().map_or(0, |h| h.bytes / 1_000_000)
                        );
                        break;
                    };
                    let started = Instant::now();
                    let (whole, peak) =
                        peak_of(|| table::read_whole(&want.path, &want.columns).unwrap());
                    let read_time = started.elapsed();
                    row("held", "read whole (worker)", read_time, peak, "");
                    // The swap is the part that runs on the thread that draws.
                    let path = want.path.clone();
                    let swapped = Instant::now();
                    held = doc.hold(path, whole);
                    row(
                        "held",
                        "hold: the swap (UI)",
                        swapped.elapsed(),
                        0,
                        &format!("held: {held}"),
                    );
                    if !held {
                        break;
                    }
                }
                let screen = |doc: &Document, from: usize| {
                    let started = Instant::now();
                    let (rows, peak) = peak_of(|| doc.rows(0, from, SCREEN));
                    assert!(!rows.is_empty());
                    (started.elapsed(), peak)
                };
                let (t, peak) = screen(&doc, 0);
                row(state, "first screen", t, peak, "");
                let (t, peak) = screen(&doc, nrows.saturating_sub(SCREEN));
                row(state, "jump to last rows", t, peak, "");
                let (t, peak) = screen(&doc, nrows / 2);
                row(state, "jump to middle", t, peak, "");
                let mut frames: Vec<Duration> = (0..200)
                    .map(|i| screen(&doc, nrows / 3 + i * 3).0)
                    .collect();
                frames.sort();
                row(
                    state,
                    "scroll, median frame",
                    frames[frames.len() / 2],
                    0,
                    &format!(
                        "worst {}, over 16 ms: {}",
                        ms(*frames.last().unwrap()),
                        frames
                            .iter()
                            .filter(|f| **f > Duration::from_millis(16))
                            .count()
                    ),
                );
            }
            let _ = held;
        }
    }
    println!("\nmilliseconds, one run each; a frame is `Document::rows` of {SCREEN} rows\n");
}

/// open-a-big-csv 2.4: what the document's cache of whole columns saves
/// once a column has been read whole, on the operations that read one: a
/// sort by it, a filter by it, and the rebuild after an edit, which starts
/// the engine again with none of its columns. Each step is timed and the
/// cache's hits and misses are printed after it, so that a step that reads
/// from the file shows as a miss and one that does not as a hit. Warm.
///
/// ```text
/// cargo test -p omasheet-engine --test timings --release -- --ignored --nocapture an_edit_over
/// ```
#[test]
#[ignore = "a measurement, not an assertion: run it in release with --ignored --nocapture"]
fn an_edit_over_a_big_table_reads_no_column_again() {
    use omasheet_engine::Document;
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let names = omasheet_engine::datefmt::Names::default();
    let style = omasheet_engine::omx::date::Style::ISO;
    println!(
        "\n{:<34} {:<24} {:>9}  cache after",
        "file", "step", "time ms"
    );
    for (file, column) in [
        ("fhvhv_tripdata_2019-03.parquet", "PULocationID"),
        ("yellow_tripdata_2026-01.parquet", "PULocationID"),
    ] {
        let path = format!("{root}/demo_data/{file}");
        if !std::path::Path::new(&path).exists() {
            continue;
        }
        let text = format!(
            "view T := dataframe[{}]\n\nPULocationID : Integer\nDOLocationID : Integer\n",
            omasheet_engine::omx::lexer::quoted(&path)
        );
        for kept in [false, true] {
            let mut doc = Document::on_screen(&text, style, names.clone());
            if !kept {
                // A budget of nothing keeps nothing: every edit reads again.
                doc.set_column_budget(0);
            }
            let file = format!("{file} ({})", if kept { "cache" } else { "no cache" });
            let c = doc.snapshot().sheets[0]
                .columns
                .iter()
                .position(|x| x.name == column)
                .expect("the column");
            let at = doc.snapshot().sheets[0].columns.len();
            let say = |doc: &Document, step: &str, took: Duration| {
                let s = doc.column_cache();
                println!(
                    "{file:<44} {step:<24} {:>9}  {} hits, {} misses, {} MiB",
                    ms(took),
                    s.hits,
                    s.misses,
                    s.bytes / (1024 * 1024)
                );
            };
            let started = Instant::now();
            doc.sort_rows(0, c, false).unwrap();
            say(&doc, "sort ascending", started.elapsed());
            let started = Instant::now();
            doc.sort_rows(0, c, true).unwrap();
            say(&doc, "sort descending", started.elapsed());
            let filter = vec![(column.to_string(), "* in [1, 2]".to_string())];
            let started = Instant::now();
            let rows = doc.shown_rows(0, &filter).map(|r| r.len());
            say(&doc, &format!("filter ({rows:?} rows)"), started.elapsed());
            let started = Instant::now();
            doc.insert_computed(0, at, "A", &format!("{column} + 1"))
                .unwrap();
            say(&doc, "edit: add a formula", started.elapsed());
            let started = Instant::now();
            doc.sort_rows(0, c, false).unwrap();
            say(&doc, "sort after the edit", started.elapsed());
            // The new column drawn: a screen of it, first at the start and then
            // a jump to the middle and a scroll on from there.
            let nrows = doc.row_count(0);
            let started = Instant::now();
            let _ = doc.rows(0, 0, 40);
            say(&doc, "first screen with it", started.elapsed());
            let started = Instant::now();
            let _ = doc.rows(0, nrows / 2, 40);
            say(&doc, "jump to the middle", started.elapsed());
            let started = Instant::now();
            let _ = doc.rows(0, nrows / 2 + 3, 40);
            say(&doc, "scroll three rows", started.elapsed());
        }
    }
    println!("\nmilliseconds, one run each, page cache warm\n");
}

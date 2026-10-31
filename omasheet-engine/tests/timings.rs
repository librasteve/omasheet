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
    let text = "table T := dataframe[\"column.parquet\"]\n\nWhole : Integer\nReal  : Float\n";
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

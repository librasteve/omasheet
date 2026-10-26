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
use omasheet_engine::omx::compile;
use omasheet_engine::{Engine, Lowering, Options};
use std::time::{Duration, Instant};

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

/// How many lowerable columns the wide sheet is measured at.
const COLS: [usize; 3] = [5, 20, 50];

/// A sheet of ten rows and `cols` columns, every one of them lowerable:
/// `C0 := Revenue - Cost`, and so on.
///
/// The other shape, and the one the threshold is really for. A plan is
/// built, optimised and collected per column, so a small sheet pays that
/// cost as many times as it has columns, where `budget.omx` pays it once.
fn wide(cols: usize) -> String {
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
    for r in 0..10 {
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

struct Timing {
    compile: Duration,
    build: Duration,
    run: Duration,
    snapshot: Duration,
    lowered: usize,
    refused: usize,
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
        }
    }
}

/// One file calculated one way, with each phase timed on its own.
fn measure(text: &str, lowering: Lowering) -> Timing {
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
    // thrown back.
    let start = Instant::now();
    for (t, sheet) in program.sheets.iter().enumerate() {
        for c in 0..sheet.cols.len() {
            for r in 0..sheet.nrows {
                std::hint::black_box(engine.show(&engine.cell_shown(t, c, r), false));
            }
        }
    }
    let snapshot = start.elapsed();

    let attempts = engine.attempts();
    Timing {
        compile: compile_time,
        build,
        run,
        snapshot,
        lowered: attempts.iter().filter(|a| a.2 == Attempt::Lowered).count(),
        refused: attempts.iter().filter(|a| a.2 == Attempt::Refused).count(),
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
    measure(&text, Lowering::Always);
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
            let t = Timing::best((0..RUNS).map(|_| measure(&text, lowering)).collect());
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
        let text = wide(cols);
        for lowering in [Lowering::Never, Lowering::Always] {
            let t = Timing::best((0..RUNS).map(|_| measure(&text, lowering)).collect());
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
}

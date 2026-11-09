// Copyright (c) 2026 Stephen Roe

//! Where the time and the memory go in opening a CSV the way it was opened
//! before `open-a-big-csv`: read into text, then into a `Datum` for every
//! cell, a `Grid`, `.omx` source, the parsed program, the evaluated engine,
//! and a snapshot that formats every cell (task 0.2). It is the *before*
//! that the 29× in `proposal.md` is attributed to, a phase at a time.
//!
//! The path is `csv::import` followed by `Document::on_screen`, taken
//! apart: `csv::import` is a read of the records and `Datum::read` of each
//! field, `Grid::from_rows`, and `write_omx`; the document is `compile`,
//! building and running the engine, and the snapshot. A last line runs the
//! whole of it again through `csv::import` and `Document::from_text`, which
//! is what the app does, as a check that the parts add up.
//!
//! It is a measurement and not an assertion, so it is ignored by default,
//! and release is the only build whose figures mean anything. The files are
//! the ones `demo_data/generate.sh` makes:
//!
//! ```text
//! cargo test -p omasheet-interop --test timings --release -- --ignored --nocapture
//! ```
//!
//! `OMASHEET_BENCH` names the sizes, default `10mb,100mb`. At 100 MB the
//! run holds several gigabytes, as it did for the figures in the proposal.

use omasheet_engine::omx::compile;
use omasheet_engine::{Document, Engine, Options};
use omasheet_interop::{Datum, Grid, write_omx};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The allocator, counting: what is held now, the most held since the
/// count was last reset (a phase's peak), and the most held ever since the
/// run began (the file's peak).
struct Counting;

static HELD: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static OVERALL: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = HELD.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
            OVERALL.fetch_max(now, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        HELD.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, layout, new_size) };
        if !q.is_null() {
            if new_size >= layout.size() {
                let now = HELD.fetch_add(new_size - layout.size(), Ordering::Relaxed) + new_size
                    - layout.size();
                PEAK.fetch_max(now, Ordering::Relaxed);
                OVERALL.fetch_max(now, Ordering::Relaxed);
            } else {
                HELD.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const MIB: f64 = 1024.0 * 1024.0;

/// What one phase cost: its time, the most it held above what was held
/// when it began, and what was held when it ended.
struct Phase {
    time: Duration,
    peak: usize,
    held: usize,
}

/// Runs `f` and takes its [`Phase`].
fn phase<T>(f: impl FnOnce() -> T) -> (T, Phase) {
    let base = HELD.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let start = Instant::now();
    let out = f();
    let time = start.elapsed();
    let peak = PEAK.load(Ordering::Relaxed).saturating_sub(base);
    (
        out,
        Phase {
            time,
            peak,
            held: HELD.load(Ordering::Relaxed),
        },
    )
}

/// The process's peak resident set, in MiB: the high-water mark of
/// everything run so far.
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
            return usage.ru_maxrss as f64 * scale / MIB;
        }
    }
    0.0
}

fn seconds(d: Duration) -> String {
    format!("{:.3}", d.as_secs_f64())
}

#[test]
#[ignore = "a measurement, not an assertion: run it in release with --ignored --nocapture"]
fn where_opening_a_csv_spends_its_time_and_memory() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../demo_data");
    let sizes = std::env::var("OMASHEET_BENCH").unwrap_or_else(|_| "10mb,100mb".into());
    println!(
        "\n{:<7} {:<18} {:>9} {:>11} {:>11}",
        "file", "phase", "seconds", "peak MiB", "held MiB"
    );
    for size in sizes.split(',') {
        let path = std::path::PathBuf::from(format!("{root}/synthetic-{size}.csv"));
        if !path.exists() {
            println!(
                "{size}: {} is not there; demo_data/generate.sh makes it",
                path.display()
            );
            continue;
        }
        let file_mib = std::fs::metadata(&path).unwrap().len() as f64 / MIB;
        OVERALL.store(HELD.load(Ordering::Relaxed), Ordering::Relaxed);
        let row = |name: &str, p: &Phase| {
            println!(
                "{size:<7} {name:<18} {:>9} {:>11.1} {:>11.1}",
                seconds(p.time),
                p.peak as f64 / MIB,
                p.held as f64 / MIB
            );
        };
        let before = HELD.load(Ordering::Relaxed);

        // csv::import, taken apart.
        let (bytes, p) = phase(|| std::fs::read(&path).unwrap());
        row("read", &p);

        let (rows, p) = phase(|| {
            let text = std::str::from_utf8(&bytes).unwrap();
            let text = text.strip_prefix('\u{feff}').unwrap_or(text);
            let mut reader = csv::ReaderBuilder::new()
                .has_headers(false)
                .flexible(true)
                .from_reader(text.as_bytes());
            let mut rows = Vec::new();
            for (i, record) in reader.records().enumerate() {
                let record = record.unwrap();
                let read = |field: &str| match i {
                    0 => Datum::Text(field.to_string()),
                    _ => Datum::read(field),
                };
                rows.push(record.iter().map(read).collect::<Vec<_>>());
            }
            rows
        });
        row("records, Datum", &p);

        let (grid, p) = phase(|| Grid::from_rows("synthetic", rows));
        row("Grid", &p);

        let mut notices = Vec::new();
        let (source, p) = phase(|| write_omx(vec![grid], &mut notices));
        row("write_omx", &p);
        // `csv::import` returns the source and lets go of the bytes.
        drop(bytes);
        let source_mib = source.len() as f64 / MIB;

        // Document::on_screen, taken apart.
        let ((program, diags), p) = phase(|| compile(&source, 0));
        assert!(diags.is_empty(), "{diags:?}");
        row("parse", &p);
        let mut parts = p.time;

        let (engine, p) = phase(|| {
            let engine = Engine::with_options(&program, Options::default());
            engine.run();
            engine
        });
        row("evaluate", &p);
        parts += p.time;

        let (snapshot, p) = phase(|| {
            // The snapshot: every cell of every sheet, formatted as it is
            // shown, a column at a time where the engine can, and kept, as
            // `Document` keeps the text of every cell it has shown.
            let mut cells: Vec<Vec<String>> = Vec::new();
            for (t, sheet) in program.sheets.iter().enumerate() {
                for c in 0..sheet.cols.len() {
                    cells.push(match engine.column_shown(t, c) {
                        Some(values) => values.iter().map(|v| engine.show(v, false)).collect(),
                        None => (0..sheet.nrows)
                            .map(|r| engine.show(&engine.cell_shown(t, c, r), false))
                            .collect(),
                    });
                }
            }
            cells
        });
        row("snapshot", &p);
        parts += p.time;

        // `Document::rebuild` parses the text a second time, into the AST
        // it reads each cell's source from.
        let (ast, p) =
            phase(|| omasheet_engine::omx::file::parse_file(&source, 0, &mut Vec::new()));
        row("AST, second parse", &p);
        parts += p.time;
        drop(ast);
        let held_by_parts = HELD.load(Ordering::Relaxed) - before;
        let overall = OVERALL.load(Ordering::Relaxed) - before;
        std::hint::black_box(&snapshot);
        drop(snapshot);
        drop(engine);
        drop(program);

        // `Document` over the same source: what it makes of it, whole.
        let (doc, p) = phase(|| Document::from_text(&source));
        row("Document::from_text", &p);
        println!(
            "{size:<7}   parse, evaluate, snapshot and AST come to {} s; Document takes {} s more, \
             in the rest of `rebuild`",
            seconds(parts),
            seconds(p.time.saturating_sub(parts))
        );
        std::hint::black_box(doc.snapshot().sheets.len());
        drop(doc);

        println!(
            "{size:<7} file {file_mib:.0} MiB, source {source_mib:.0} MiB; peak {:.0} MiB \
             ({:.1}× the file), held at the end {:.0} MiB, process peak RSS so far {:.0} MiB\n",
            overall as f64 / MIB,
            overall as f64 / MIB / file_mib,
            held_by_parts as f64 / MIB,
            peak_rss_mib()
        );
    }
    println!(
        "seconds, one run each, page cache warm; peak is the counting allocator's \
         above what the phase began with,\nheld is what is held when it ends\n"
    );
}

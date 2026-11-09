// Copyright (c) 2026 Stephen Roe

//! One CSV copied to Parquet as the app copies it (`table::convert`:
//! `scan_csv`, then `sink_parquet` on the streaming engine), in a process
//! of its own so that the resident set it peaks at is the copy's and
//! nothing else's (`open-a-big-csv` task 0.3). Run under the system's
//! `time`, which prints that peak:
//!
//! ```text
//! cargo build -p omasheet-engine --release --example convert_csv
//! /usr/bin/time -l target/release/examples/convert_csv demo_data/synthetic-1.1gb.csv
//! ```
//!
//! The copy goes to a cache of its own, which is taken away again unless a
//! second argument, `keep`, is given: the copy's path is then printed and it
//! is left, to look at (`row_groups` shows how it is laid out).

use std::time::Instant;

fn main() {
    let Some(csv) = std::env::args().nth(1) else {
        eprintln!("usage: convert_csv <file.csv>");
        std::process::exit(2);
    };
    let cache = std::env::temp_dir().join(format!("omasheet-convert-{}", std::process::id()));
    // SAFETY: before anything else reads it, and nothing else runs.
    unsafe { std::env::set_var("OMASHEET_CACHE", &cache) };
    let started = Instant::now();
    let copy = omasheet_engine::table::convert(std::path::Path::new(&csv)).expect("a copy");
    let took = started.elapsed();
    println!(
        "{csv}: copied in {:.0} ms, {} MB",
        took.as_secs_f64() * 1000.0,
        std::fs::metadata(&copy).expect("a file").len() / 1_000_000
    );
    if std::env::args().nth(2).as_deref() == Some("keep") {
        println!("kept: {}", copy.display());
    } else {
        let _ = std::fs::remove_dir_all(&cache);
    }
}

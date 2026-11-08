// Copyright (c) 2026 Stephen Roe

//! A session on a big CSV, as the targets of `open-a-big-csv` section 5
//! describe it: open it, copy it, sum every numeric column, edit a formula
//! column, and sum again. Each step is timed and the most memory the process
//! has had by then is printed, which is what the peak-memory target is read
//! from. Takes the seeded `demo_data/synthetic-*.csv` files:
//!
//! ```text
//! cargo run -p omasheet-engine --release --example session_csv -- demo_data/synthetic-1.1gb.csv
//! ```
//!
//! The copy goes to a cache of its own, which is taken away again.

use omasheet_engine::Document;
use omasheet_engine::datefmt::Names;
use omasheet_engine::omx::date::Style;
use std::time::Instant;

/// The physical memory the process holds now and the most it has held, in
/// MiB: the footprint macOS reports where there is one (what the machine
/// was asked for, not the file pages mapped), and the resident set
/// elsewhere, where only the most is known.
fn memory_mib() -> (f64, f64) {
    const MIB: f64 = 1024.0 * 1024.0;
    #[cfg(target_os = "macos")]
    {
        let mut info = unsafe { std::mem::zeroed::<libc::rusage_info_v4>() };
        let ok = unsafe {
            libc::proc_pid_rusage(
                libc::getpid(),
                libc::RUSAGE_INFO_V4,
                (&mut info as *mut libc::rusage_info_v4).cast(),
            )
        };
        if ok == 0 {
            return (
                info.ri_phys_footprint as f64 / MIB,
                info.ri_lifetime_max_phys_footprint as f64 / MIB,
            );
        }
    }
    let mut usage = unsafe { std::mem::zeroed::<libc::rusage>() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    let scale = if cfg!(target_os = "macos") {
        1.0
    } else {
        1024.0
    };
    let peak = usage.ru_maxrss as f64 * scale / MIB;
    (peak, peak)
}

fn main() {
    let Some(csv) = std::env::args().nth(1) else {
        eprintln!("usage: session_csv <file.csv>");
        std::process::exit(2);
    };
    let csv = std::fs::canonicalize(csv).expect("a file");
    let cache = std::env::temp_dir().join(format!("omasheet-session-{}", std::process::id()));
    // SAFETY: before anything else reads it, and nothing else runs.
    unsafe { std::env::set_var("OMASHEET_CACHE", &cache) };
    let text = format!(
        "view T := dataframe[{}]\n\nid : Integer\ndate : Date\nregion : String\nproduct : String\n\
         qty : Integer\nprice : Float\namount : Float\n\n\
         const SumId = T.id.sum()\nconst SumQty = T.qty.sum()\n\
         const SumPrice = T.price.sum()\nconst SumAmount = T.amount.sum()\n",
        omasheet_engine::omx::lexer::quoted(&csv.to_string_lossy())
    );
    println!(
        "{:<34} {:>10} {:>10} {:>16}",
        "step", "ms", "now MiB", "peak so far MiB"
    );
    let say = |step: &str, started: Instant| {
        let (now, peak) = memory_mib();
        println!(
            "{step:<34} {:>10.1} {:>10.0} {:>16.0}",
            started.elapsed().as_secs_f64() * 1000.0,
            now,
            peak
        );
    };
    let cached = |doc: &Document| {
        let s = doc.column_cache();
        println!(
            "    column cache: {} hits, {} misses, {} entries, {} MiB",
            s.hits,
            s.misses,
            s.entries,
            s.bytes / (1024 * 1024)
        );
    };
    println!(
        "{:<34} {:>10} {:>10.0} {:>16.0}",
        "(process start)",
        "",
        memory_mib().0,
        memory_mib().1
    );

    let started = Instant::now();
    let mut doc = Document::on_screen(&text, Style::ISO, Names::default());
    say("open (sample, sums of the sample)", started);

    let started = Instant::now();
    omasheet_engine::table::convert(&csv).expect("a copy");
    say("copy", started);

    let started = Instant::now();
    assert!(doc.converted(&csv));
    say("over the copy: sum every column", started);
    cached(&doc);
    for c in &doc.snapshot().consts {
        println!("    {} = {}", c.name, c.display);
    }
    let started = Instant::now();
    let nrows = doc.row_count(0);
    let _ = doc.rows(0, nrows / 2, 40);
    say("a screen from the middle", started);

    let at = doc.snapshot().sheets[0].columns.len();
    let started = Instant::now();
    doc.insert_computed(0, at, "Margin", "amount - price * qty")
        .expect("an edit");
    say("edit: add a formula column", started);
    cached(&doc);
    let started = Instant::now();
    let _ = doc.rows(0, nrows / 2, 40);
    say("  its visible cells", started);
    let started = Instant::now();
    doc.insert_computed(0, at + 1, "Again", "qty * 2")
        .expect("an edit");
    say("edit again (sums again)", started);
    let cache_stats = doc.column_cache();
    println!(
        "column cache: {} hits, {} misses, {} MiB held",
        cache_stats.hits,
        cache_stats.misses,
        cache_stats.bytes / (1024 * 1024)
    );
    let _ = std::fs::remove_dir_all(&cache);
}

// Copyright (c) 2026 Stephen Roe

//! The row groups of a Parquet file: how many rows and bytes each holds.
//! For finding why one part of a file reads slower than another
//! (`open-a-big-csv` 1.3).
//!
//! ```text
//! cargo run -p omasheet-engine --release --example row_groups -- file.parquet
//! ```

use polars::prelude::*;

fn main() {
    let path = std::env::args().nth(1).expect("usage: row_groups <file>");
    let file = std::fs::File::open(&path).expect("a file");
    let mut reader = ParquetReader::new(file);
    let meta = reader.get_metadata().expect("a footer").clone();
    println!(
        "{path}: {} rows, {} row groups",
        meta.num_rows,
        meta.row_groups.len()
    );
    let n = meta.row_groups.len();
    for (i, g) in meta.row_groups.iter().enumerate() {
        if i < 3 || i + 3 >= n || i == n / 2 {
            println!(
                "  group {i:>4}: {:>9} rows, {:>6.1} MB compressed, {:>6.1} MB uncompressed, {} columns",
                g.num_rows(),
                g.compressed_size() as f64 / 1e6,
                g.total_byte_size() as f64 / 1e6,
                g.n_columns()
            );
        }
    }
}

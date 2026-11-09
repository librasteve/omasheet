// Copyright (c) 2026 Stephen Roe

//! The synthetic CSV `open-a-big-csv` is measured against (task 0.1): rows
//! of `id,date,region,product,qty,price,amount`, made from a seed, so the
//! figures in `proposal.md` can be taken again on any machine.
//!
//! ```text
//! cargo run -p omasheet-engine --release --example gen_big_csv -- 10mb
//! cargo run -p omasheet-engine --release --example gen_big_csv -- 100mb
//! cargo run -p omasheet-engine --release --example gen_big_csv -- 1.1gb
//! cargo run -p omasheet-engine --release --example gen_big_csv -- <rows> <path> [seed]
//! ```
//!
//! With `--json` anywhere in the arguments the same rows are written as a
//! JSON list of objects, to `demo_data/synthetic-<size>.json` for a named
//! size, for `open-a-big-csv`'s JSON figures. With `--xlsx` they are one
//! sheet of a workbook, `synthetic-<size>.xlsx`, to at most the 1,048,575
//! rows a sheet holds, so `1.1gb` is as many as `100mb`. With `--formulas` too,
//! `amount` is the formula `=qty*price` filled down, with the value Excel
//! would hold for it, in `synthetic-<size>-formulas.xlsx`.
//!
//! A named size writes `demo_data/synthetic-<size>.csv` (not in Git). Every
//! size is a prefix of the same stream for a given seed, so the 10 MB file
//! is the first 211,000 rows of the 1.1 GB one: the rows in `proposal.md`
//! are 211k, 2.07M and 20.8M. `demo_data/generate.sh` makes all three.
//!
//! Nothing but the standard library: the generator is a SplitMix64 stream,
//! and a row is a function of its index and the stream alone, so there is
//! no dependence on the platform or on a crate's version.

use std::io::{BufWriter, Write};
use std::path::PathBuf;

const SEED: u64 = 0x0A5E_5EED;

const REGIONS: [&str; 6] = ["North", "South", "East", "West", "Central", "Overseas"];
const PRODUCTS: [&str; 12] = [
    "Widget", "Gadget", "Sprocket", "Gear", "Bracket", "Bearing", "Valve", "Flange", "Coupling",
    "Gasket", "Spindle", "Washer",
];

/// The sizes `proposal.md` names, as a number of rows.
fn preset(name: &str) -> Option<usize> {
    match name {
        "10mb" => Some(211_000),
        "100mb" => Some(2_070_000),
        "1.1gb" => Some(20_800_000),
        _ => None,
    }
}

struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (y + i64::from(m <= 2), m, d)
}

fn main() -> std::io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let xlsx = args.iter().any(|a| a == "--xlsx");
    let formulas = args.iter().any(|a| a == "--formulas");
    args.retain(|a| a != "--json" && a != "--xlsx" && a != "--formulas");
    let mut args = args.into_iter();
    let first = args.next().unwrap_or_else(|| usage());
    let (rows, path) = match preset(&first) {
        Some(rows) => {
            let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../demo_data");
            let ext = if xlsx {
                "xlsx"
            } else if json {
                "json"
            } else {
                "csv"
            };
            // The workbook with formulas is a file of its own.
            let tail = if xlsx && formulas { "-formulas" } else { "" };
            (
                rows,
                PathBuf::from(format!("{root}/synthetic-{first}{tail}.{ext}")),
            )
        }
        None => {
            let rows = first.replace('_', "").parse().unwrap_or_else(|_| usage());
            (rows, PathBuf::from(args.next().unwrap_or_else(|| usage())))
        }
    };
    let seed = args
        .next()
        .map_or(SEED, |s| s.parse().unwrap_or_else(|_| usage()));

    if xlsx {
        return workbook(rows.min(1_048_575), &path, seed, formulas);
    }
    // Written beside the target and renamed, so a half file is never taken
    // for a finished one.
    let partial = path.with_extension("part");
    let mut out = BufWriter::with_capacity(1 << 20, std::fs::File::create(&partial)?);
    if json {
        out.write_all(b"[")?;
    } else {
        out.write_all(b"id,date,region,product,qty,price,amount\n")?;
    }
    let mut rng = SplitMix(seed);
    // 2020-01-01 is day 18,262; the rows span three years.
    for id in 1..=rows {
        let (y, m, d) = civil(18_262 + (rng.next() % 1_096) as i64);
        let region = REGIONS[(rng.next() % REGIONS.len() as u64) as usize];
        let product = PRODUCTS[(rng.next() % PRODUCTS.len() as u64) as usize];
        let qty = 1 + rng.next() % 50;
        // Cents, 1.00 to 500.99, so `amount` is exact in two places.
        let cents = 100 + rng.next() % 50_000;
        let amount = qty * cents;
        if json {
            if id > 1 {
                out.write_all(b",\n")?;
            }
            write!(
                out,
                "{{\"id\":{id},\"date\":\"{y:04}-{m:02}-{d:02}\",\"region\":\"{region}\",\"product\":\"{product}\",\"qty\":{qty},\"price\":{}.{:02},\"amount\":{}.{:02}}}",
                cents / 100,
                cents % 100,
                amount / 100,
                amount % 100
            )?;
        } else {
            writeln!(
                out,
                "{id},{y:04}-{m:02}-{d:02},{region},{product},{qty},{}.{:02},{}.{:02}",
                cents / 100,
                cents % 100,
                amount / 100,
                amount % 100
            )?;
        }
    }
    if json {
        out.write_all(b"]\n")?;
    }
    out.flush()?;
    drop(out);
    std::fs::rename(&partial, &path)?;
    eprintln!(
        "{}: {rows} rows, {} MB",
        path.display(),
        std::fs::metadata(&path)?.len() / 1_000_000
    );
    Ok(())
}

fn usage() -> ! {
    eprintln!(
        "usage: gen_big_csv [--json] 10mb|100mb|1.1gb\n       gen_big_csv [--json] <rows> <path> [seed]"
    );
    std::process::exit(2)
}

/// The rows as one sheet of a workbook. The writer holds the sheet, a few
/// hundred MB at a million rows.
fn workbook(rows: usize, path: &PathBuf, seed: u64, formulas: bool) -> std::io::Result<()> {
    use rust_xlsxwriter::{ExcelDateTime, Format, Formula, Workbook};
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name("Sales").map_err(std::io::Error::other)?;
    let date = Format::new().set_num_format("yyyy-mm-dd");
    for (c, name) in ["id", "date", "region", "product", "qty", "price", "amount"]
        .iter()
        .enumerate()
    {
        sheet
            .write_string(0, c as u16, *name)
            .map_err(std::io::Error::other)?;
    }
    let mut rng = SplitMix(seed);
    for id in 1..=rows {
        let (y, m, d) = civil(18_262 + (rng.next() % 1_096) as i64);
        let region = REGIONS[(rng.next() % REGIONS.len() as u64) as usize];
        let product = PRODUCTS[(rng.next() % PRODUCTS.len() as u64) as usize];
        let qty = 1 + rng.next() % 50;
        let cents = 100 + rng.next() % 50_000;
        let r = id as u32;
        let err = |e: rust_xlsxwriter::XlsxError| std::io::Error::other(e);
        sheet.write_number(r, 0, id as f64).map_err(err)?;
        let day = ExcelDateTime::from_ymd(y as u16, m as u8, d as u8).map_err(err)?;
        sheet
            .write_datetime_with_format(r, 1, &day, &date)
            .map_err(err)?;
        sheet.write_string(r, 2, region).map_err(err)?;
        sheet.write_string(r, 3, product).map_err(err)?;
        sheet.write_number(r, 4, qty as f64).map_err(err)?;
        sheet
            .write_number(r, 5, cents as f64 / 100.0)
            .map_err(err)?;
        let amount = (qty * cents) as f64 / 100.0;
        if formulas {
            sheet
                .write_formula(
                    r,
                    6,
                    Formula::new(format!("=E{0}*F{0}", r + 1)).set_result(amount.to_string()),
                )
                .map_err(err)?;
        } else {
            sheet.write_number(r, 6, amount).map_err(err)?;
        }
    }
    let partial = path.with_extension("part");
    book.save(&partial).map_err(std::io::Error::other)?;
    std::fs::rename(&partial, path)?;
    eprintln!(
        "{}: {rows} rows, {} MB",
        path.display(),
        std::fs::metadata(path)?.len() / 1_000_000
    );
    Ok(())
}

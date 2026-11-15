// Copyright (c) 2026 Stephen Roe

//! The window functions (`polars-function-registry`): `cum_sum`, `diff`,
//! `rank`, `rolling_mean` and the rest, with an order key and a partition.

use omasheet_engine::{Options, eval_with, view_with};

fn exact() -> Options {
    Options {
        exact: true,
        ..Options::default()
    }
}

/// A ledger whose rows are not in date order, two accounts, one missing amount.
const LEDGER: &str = "\
sheet Tx

Date       | Account | Amount
2025-01-03 | A       | 10
2025-01-01 | A       | 5
2025-01-02 | B       | 7
2025-01-05 | A       |
2025-01-04 | B       | 20
2025-01-06 | A       | 1
";

/// `LEDGER` with one formula column `Name := expression`.
fn with(name: &str, formula: &str) -> String {
    let mut out = String::new();
    for line in LEDGER.lines() {
        out.push_str(line);
        if line.starts_with("Date") {
            out.push_str(&format!(" | {name}"));
        } else if line.starts_with("20") {
            out.push_str(" | *");
        }
        out.push('\n');
    }
    out.push_str(&format!("\n{name} := {formula}\n"));
    out
}

/// What a formula column holds, one row to a line.
fn column(name: &str, formula: &str) -> Vec<String> {
    let file = with(name, formula);
    let out = view_with("test.omx", &file, exact());
    assert!(out.ok(), "`{formula}` failed:\n{}", out.errors.join("\n"));
    let mut lines = out.output.lines().filter(|l| l.contains('|'));
    let header: Vec<&str> = lines.next().unwrap().split('|').map(str::trim).collect();
    let at = header.iter().position(|h| *h == name).unwrap();
    lines
        .skip(1)
        .map(|l| l.split('|').map(str::trim).nth(at).unwrap().to_string())
        .collect()
}

fn calc(file: &str, expr: &str) -> String {
    let out = eval_with(Some(("test.omx", file)), "<expression>", expr, exact());
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

#[test]
fn a_running_total_in_file_order() {
    assert_eq!(
        column("Run", "Amount.cum_sum()"),
        ["10", "15", "22", "", "42", "43"]
    );
}

#[test]
fn a_running_total_in_date_order() {
    // 01-01 5, 01-02 7, 01-03 10, 01-04 20, 01-05 (empty), 01-06 1
    assert_eq!(
        column("Run", "Amount.cum_sum(Date)"),
        ["22", "5", "12", "", "42", "43"]
    );
}

#[test]
fn a_running_total_for_each_account_in_date_order() {
    // A: 01-01 5, 01-03 10, 01-05 (empty), 01-06 1. B: 01-02 7, 01-04 20.
    assert_eq!(
        column("Run", "Amount.cum_sum(Date by Account)"),
        ["15", "5", "7", "", "27", "16"]
    );
}

#[test]
fn the_other_running_functions() {
    assert_eq!(
        column("X", "Amount.cum_max(Date)"),
        ["10", "5", "7", "", "20", "20"]
    );
    assert_eq!(
        column("X", "Amount.cum_min(Date)"),
        ["5", "5", "5", "", "5", "1"]
    );
    // In date order the empty cell is the fifth row, and is counted past.
    assert_eq!(
        column("X", "Amount.cum_count(Date)"),
        ["3", "1", "2", "4", "4", "5"]
    );
}

#[test]
fn a_difference_and_a_change() {
    assert_eq!(
        column("D", "Amount.diff(1, Date by Account)"),
        ["5", "", "", "", "13", ""]
    );
    assert_eq!(
        column("P", "Amount.pct_change(1, Date by Account)"),
        ["1", "", "", "", "1.8571428571428572", ""]
    );
}

#[test]
fn a_date_differs_by_a_duration() {
    let out = calc(LEDGER, "Tx.Date.diff()");
    assert!(out.contains("P"), "{out}");
}

#[test]
fn a_rank() {
    assert_eq!(
        column("R", "Amount.rank(min)"),
        ["4", "2", "3", "", "5", "1"]
    );
    assert_eq!(
        column("R", "Amount.rank(min, desc)"),
        ["2", "4", "3", "", "1", "5"]
    );
    assert_eq!(
        column("R", "Amount.rank(dense by Account)"),
        ["3", "2", "1", "", "2", "1"]
    );
}

#[test]
fn filling_and_rolling() {
    assert_eq!(
        column("F", "Amount.forward_fill(Date)"),
        ["10", "5", "7", "20", "20", "1"]
    );
    assert_eq!(
        column("F", "Amount.backward_fill(Date)"),
        ["10", "5", "7", "1", "20", "1"]
    );
    assert_eq!(
        column("S", "Amount.rolling_sum(2, Date)"),
        ["17", "", "12", "", "30", ""]
    );
    assert_eq!(
        column("M", "Amount.rolling_mean(2, Date)"),
        ["8.5", "", "6", "", "15", ""]
    );
}

#[test]
fn over_a_column_it_gives_a_column() {
    assert_eq!(
        calc(LEDGER, "Tx.Amount.cum_sum(Tx.Date)"),
        "[22, 5, 12, empty, 42, 43]"
    );
}

#[test]
fn mistakes_are_said() {
    let bad = |formula: &str| {
        let out = view_with("test.omx", &with("X", formula), exact());
        assert!(!out.ok(), "`{formula}` should fail");
        out.errors.join("\n")
    };
    assert!(bad("Account.cum_sum()").contains("needs numbers"));
    assert!(bad("Amount.rolling_mean()").contains("length of the window"));
    assert!(bad("Amount.cum_sum(Date by Nowhere)").contains("Nowhere"));
    assert!(bad("Amount.rank(sideways)").contains("sideways"));
}

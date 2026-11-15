// Copyright (c) 2026 Stephen Roe

//! The window functions calculated by Polars and by the engine, which have
//! to be the same (`polars-function-registry` 3.1): every function over a
//! column of whole numbers and a column of floats, with empty cells, rows out
//! of order, keys that tie and an empty partition key. The columns that
//! Polars will not take (an exact decimal, a percentage) are the engine's
//! alone, and are checked to be refused rather than guessed at.

use omasheet_engine::eval::Attempt;
use omasheet_engine::omx::compile;
use omasheet_engine::{Engine, Lowering, Options, table, view_with};

/// Twelve rows: dates out of order with ties, three partitions and an empty
/// one, empty amounts, and a float column with a repeated value.
const ROWS: &str = "\
| 2025-03-04 | A | 5    | 1.5  |
| 2025-03-01 | B | 7    | 2.25 |
| 2025-03-03 | A |      |      |
| 2025-03-01 | A | 10   | 1.5  |
| 2025-03-02 |   | -3   | -0.5 |
| 2025-03-03 | B | 7    | 4    |
| 2025-03-05 | A | 20   | 8    |
| 2025-03-02 | B |      | 2.25 |
| 2025-03-04 | B | 1    | 0.25 |
| 2025-03-02 | A | 10   | 3    |
| 2025-03-06 |   | 4    |      |
| 2025-03-03 | A | 2    | 6.5  |
";

fn sheet(types: &str, formula: &str) -> String {
    let mut text = format!("sheet W\n\nD : Date\nK : String\n{types}\n\n| D | K | I | F | R |\n");
    for line in ROWS.lines() {
        text.push_str(line);
        text.push_str(" * |\n");
    }
    text.push_str(&format!("\nR := {formula}\n"));
    text
}

fn run(text: &str, lowering: Lowering) -> (String, Vec<Attempt>) {
    let out = view_with(
        "w.omx",
        text,
        Options {
            lowering,
            exact: true,
            ..Options::default()
        },
    );
    assert!(out.ok(), "{text}\n{}", out.errors.join("\n"));
    let (mut program, mut diags) = compile(text, 0);
    assert!(diags.is_empty(), "{diags:?}");
    let tables = table::read(&mut program, std::path::Path::new("."), &mut diags);
    let engine = Engine::with_tables(
        &program,
        Options {
            lowering,
            exact: true,
            ..Options::default()
        },
        tables,
    );
    engine.run();
    let attempts = engine.attempts().iter().map(|a| a.2).collect();
    (out.output, attempts)
}

/// Both ways agree on `formula`, and Polars did it (or declined, if that is
/// what is `expected`).
fn agree(types: &str, formula: &str, lowered: bool) {
    let text = sheet(types, formula);
    let (engine, _) = run(&text, Lowering::Never);
    let (polars, attempts) = run(&text, Lowering::Always);
    assert_eq!(polars, engine, "`{formula}` differs:\n{text}");
    let want = if lowered {
        Attempt::Lowered
    } else {
        Attempt::Refused
    };
    assert_eq!(
        attempts.first().copied(),
        Some(want),
        "`{formula}` was {attempts:?}"
    );
}

const WHOLE: &str = "I : Integer\nF : Float";

#[test]
fn running_functions_of_whole_numbers_and_floats() {
    for col in ["I", "F"] {
        for args in ["", "D", "D by K", "K"] {
            // The partition alone is a window over the rows in file order.
            if args == "K" {
                continue;
            }
            agree(WHOLE, &format!("{col}.cum_sum({args})"), true);
            agree(WHOLE, &format!("{col}.cum_min({args})"), true);
            agree(WHOLE, &format!("{col}.cum_max({args})"), true);
            agree(WHOLE, &format!("{col}.cum_count({args})"), true);
        }
    }
    agree(WHOLE, "F.cum_prod(D by K)", true);
}

#[test]
fn differences_and_changes() {
    for col in ["I", "F"] {
        for args in ["", "D", "1, D by K", "2, D by K", "2"] {
            agree(WHOLE, &format!("{col}.diff({args})"), true);
            agree(WHOLE, &format!("{col}.pct_change({args})"), true);
        }
    }
}

#[test]
fn ranks() {
    for col in ["I", "F"] {
        for words in [
            "",
            "min",
            "max",
            "dense",
            "ordinal",
            "min, desc",
            "desc",
            "dense, desc",
        ] {
            agree(WHOLE, &format!("{col}.rank({words})"), true);
            let by = if words.is_empty() { "by K" } else { " by K" };
            agree(WHOLE, &format!("{col}.rank({words}{by})"), true);
        }
    }
}

#[test]
fn fills_and_rolling() {
    for col in ["I", "F"] {
        agree(WHOLE, &format!("{col}.forward_fill(D by K)"), true);
        agree(WHOLE, &format!("{col}.backward_fill(D)"), true);
        for w in [1, 2, 3] {
            agree(WHOLE, &format!("{col}.rolling_min({w}, D by K)"), true);
            agree(WHOLE, &format!("{col}.rolling_max({w}, D)"), true);
        }
    }
    for w in [1, 2, 3] {
        agree(WHOLE, &format!("I.rolling_sum({w}, D by K)"), true);
    }
    // A float window is added afresh by the engine and slid along by
    // Polars, and the mean and the deviation are the engine's alone.
    agree(WHOLE, "F.rolling_sum(2, D)", false);
    agree(WHOLE, "I.rolling_mean(2, D)", false);
    agree(WHOLE, "F.rolling_std(2, D)", false);
}

#[test]
fn a_percentage_is_a_decimal_in_a_dress() {
    // A percentage is held as the decimal it stands for, and the dress is
    // put back from the column's own type, so its window is Polars' too and
    // the answer is the same.
    agree("I : Decimal\nF : Percent", "F.cum_sum(D)", true);
    agree("I : Decimal\nF : Percent", "F.diff(1, D)", true);
}

#[test]
fn a_window_column_feeds_an_elementwise_one() {
    // `Share` reads the running total of the same row, which is lowered in
    // turn over the column that was.
    let text = format!(
        "{}\nShare := I / R\n",
        sheet(WHOLE, "I.cum_sum(D by K)")
            .replace("| R |", "| R | Share |")
            .replace(" * |\n", " * | * |\n")
    );
    let (engine, _) = run(&text, Lowering::Never);
    let (polars, _) = run(&text, Lowering::Always);
    assert_eq!(polars, engine);
}

#[test]
fn a_partition_alone_and_other_dtypes() {
    for col in ["I", "F"] {
        agree(WHOLE, &format!("{col}.cum_sum(by K)"), true);
        agree(WHOLE, &format!("{col}.rank(by K)"), true);
        agree(WHOLE, &format!("{col}.diff(by K)"), true);
    }
    // Dates and times are an order, so their extremes and their fills are
    // Polars'; text is filled too. A date's difference is a duration, the
    // engine's own rule.
    agree(WHOLE, "D.cum_max(D by K)", true);
    agree(WHOLE, "D.cum_min(by K)", true);
    agree(WHOLE, "K.forward_fill(D)", true);
    agree(WHOLE, "D.diff(1, D by K)", false);
}

#[test]
fn a_nan_and_an_overflow_are_left_to_the_engine() {
    // A `NaN` is ordered differently by the two, and a total that passes an
    // `i64` is moved to a wider number by the engine and wrapped by Polars.
    let nan = sheet(WHOLE, "F.cum_max(D)").replace("| 1.5  | * |", "| NaN  | * |");
    let (engine, _) = run(&nan, Lowering::Never);
    let (polars, attempts) = run(&nan, Lowering::Always);
    assert_eq!(polars, engine);
    assert_ne!(attempts.first(), Some(&Attempt::Lowered), "{attempts:?}");
    let big = sheet(WHOLE, "I.cum_sum(D)").replace("| 20   |", "| 9000000000000000000 |");
    let (engine, _) = run(&big, Lowering::Never);
    let (polars, attempts) = run(&big, Lowering::Always);
    assert_eq!(polars, engine);
    assert_ne!(attempts.first(), Some(&Attempt::Lowered), "{attempts:?}");
}

const DECIMALS: &str = "I : Integer\nF : Decimal(2)";

#[test]
fn a_decimal_column_is_lowered_where_polars_has_the_window() {
    // Polars cumulates, differs, ranks and fills a `Decimal(38, s)`; the
    // answers are the engine's to the digit, empty cells and ties included.
    for args in ["", "D", "D by K", "by K"] {
        agree(DECIMALS, &format!("F.cum_sum({args})"), true);
        agree(DECIMALS, &format!("F.cum_min({args})"), true);
        agree(DECIMALS, &format!("F.cum_max({args})"), true);
        agree(DECIMALS, &format!("F.cum_count({args})"), true);
    }
    agree(DECIMALS, "F.diff(1, D by K)", true);
    agree(DECIMALS, "F.diff(2, D)", true);
    agree(DECIMALS, "F.rank(min, desc by K)", true);
    agree(DECIMALS, "F.rank(dense)", true);
    agree(DECIMALS, "F.forward_fill(D by K)", true);
    agree(DECIMALS, "F.backward_fill(D)", true);
}

#[test]
fn what_polars_has_no_decimal_window_for_stays_on_the_engine() {
    agree(DECIMALS, "F.pct_change(1, D)", false);
    agree(DECIMALS, "F.rolling_min(2, D)", false);
    agree(DECIMALS, "F.rolling_sum(2, D)", false);
    agree(DECIMALS, "F.rolling_mean(2, D)", false);
}

#[test]
fn a_small_whole_power_of_a_decimal_is_lowered_and_rounded_once() {
    // `x ** n` is the base multiplied by itself, exactly, and rounded once
    // (decimal-and-fraction T19): the engine's answer to the digit.
    agree("I : Integer\nF : Decimal(2)", "F ** 2", true);
    agree("I : Integer\nF : Decimal(2)", "F ** 3", true);
    agree("I : Integer\nF : Decimal(10)", "F ** 2", true);
    // Past 38 digits, or a negative or large exponent, is the engine's.
    agree("I : Integer\nF : Decimal(10)", "F ** 4", false);
    agree("I : Integer\nF : Decimal(2)", "F ** 40", false);
}

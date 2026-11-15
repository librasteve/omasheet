// Copyright (c) 2026 Stephen Roe

//! A number too big (or too fine) for a `Decimal` is a `Fraction`: exact,
//! never rounded, never an error, and never lowered (`decimal-and-fraction`).

use omasheet_engine::{Options, eval_with, view_with};

fn exact() -> Options {
    Options {
        exact: true,
        ..Options::default()
    }
}

/// An expression against a file, in full.
fn ask(file: &str, expr: &str) -> String {
    let out = eval_with(Some(("t.omx", file)), "<expression>", expr, exact());
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

fn calc(expr: &str) -> String {
    let out = eval_with(None, "<expression>", expr, exact());
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

/// A googol in a column that declares nothing, and what is made of it.
const GOOGOL: &str = "\
sheet G

| Power | Big | Tiny | Product |
| ----- | --- | ---- | ------- |
| 100   | *   | *    | *       |

Big     := <10> ** Power
Tiny    := <1> / Big
Product := Big * Tiny
";

#[test]
fn a_googol_is_a_fraction_held_to_every_digit() {
    // 38 digits is all a Decimal holds; 101 are here, none of them lost.
    let big = ask(GOOGOL, "G.Big[0]");
    assert_eq!(big.len(), 101, "{big}");
    assert!(big.starts_with('1') && big[1..].bytes().all(|b| b == b'0'));
}

#[test]
fn its_reciprocal_is_held_too_and_they_cancel() {
    // 1 / 10**100 is a fraction no Decimal of any scale holds.
    let tiny = ask(GOOGOL, "G.Tiny[0]");
    assert_eq!(tiny, format!("1/1{}", "0".repeat(100)));
    assert_eq!(ask(GOOGOL, "G.Product[0]"), "1");
}

#[test]
fn nothing_is_rounded_where_a_decimal_would_be() {
    // A Decimal gives up the last digits of a quotient; a Fraction does not.
    assert_eq!(calc("1.0 / 3 * 3 == 1"), "false");
    assert_eq!(calc("1 / 3 * 3 == 1"), "true");
    assert_eq!(calc("<1> / 3 * 3 == 1"), "true");
    assert_eq!(calc("10 ** 100 + 1 - 10 ** 100"), "1");
    assert_eq!(calc("<1/10> ** 80 * 10 ** 80"), "1");
    assert_eq!(calc("<2/7> + <3/7> == <5/7>"), "true");
}

#[test]
fn a_decimal_of_more_than_eighteen_places_is_a_fraction() {
    let text = "sheet T\n\nA | B\n0.1234567890123456789012 | *\n\nB := A * 10 ** 22\n";
    assert_eq!(ask(text, "T.B[0]"), "1234567890123456789012");
}

#[test]
fn a_column_that_declares_a_decimal_refuses_it() {
    let text = "sheet G\n\nBig : Decimal\n\n| Power | Big |\n| ----- | --- |\n| 100   | *   |\n\nBig := 10 ** Power\n";
    let out = view_with("t.omx", text, exact());
    assert!(!out.ok(), "a Decimal cannot hold a googol");
    let said = out.errors.join("\n");
    assert!(
        said.contains("Decimal") && said.contains("Fraction"),
        "{said}"
    );
}

// Copyright (c) 2026 Stephen Roe

//! The scenarios of `openspec/changes/add-uncertainty`: a number with its
//! measurement uncertainty, written `10 ± 0.1` or `10.5(3)`.

use omasheet_engine::omx::compile;
use omasheet_engine::{Engine, Options, eval, eval_with, lint, view};

/// Evaluate an expression as the grid shows it.
fn shown(expr: &str) -> String {
    let out = eval(None, "<expression>", expr);
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

/// Evaluate an expression with every number in full, as a file writes it.
fn full(expr: &str) -> String {
    let options = Options {
        exact: true,
        ..Options::default()
    };
    let out = eval_with(None, "<expression>", expr, options);
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

fn fails(expr: &str) -> String {
    let out = eval(None, "<expression>", expr);
    assert!(!out.ok(), "`{expr}` should fail but gave {}", out.output);
    out.errors.join("\n")
}

/// The value and the uncertainty of an expression, as numbers.
fn parts(expr: &str) -> (f64, f64) {
    let text = full(expr);
    let (v, s) = text
        .split_once(" ± ")
        .unwrap_or_else(|| panic!("`{expr}` is {text}, not an uncertain value"));
    (v.parse().unwrap(), s.parse().unwrap())
}

fn near(got: (f64, f64), want: (f64, f64)) {
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * b.abs().max(1e-30);
    assert!(
        close(got.0, want.0) && close(got.1, want.1),
        "got {got:?}, wanted {want:?}"
    );
}

// ---- literals --------------------------------------------------------------

#[test]
fn the_plus_minus_literal() {
    assert_eq!(full("10 ± 0.1"), "10 ± 0.1");
    // `+-` is a sum and a negation, not an operator of its own.
    assert_eq!(full("10 +- 0.1"), "9.9");
    // `±` binds more tightly than anything, a power included.
    assert_eq!(full("10 ± 0.1 * 2"), "20 ± 0.2");
    assert_eq!(full("2 * 3 ± 0.1"), "6 ± 0.2");
    near(parts("10 ± 0.1 ** 2"), (100.0, 2.0));
    near(parts("2 ** 3 ± 0.1"), (8.0, 8.0 * 2f64.ln() * 0.1));
}

#[test]
fn a_negative_or_uncertain_uncertainty_is_refused() {
    assert!(fails("10 ± (0 - 1)").contains("negative"));
    assert!(fails("(10 ± 1) ± 1").contains("real numbers"));
}

#[test]
fn the_concise_literal() {
    assert_eq!(full("10.5(3)"), "10.5 ± 0.3");
    assert_eq!(full("10(3)"), "10 ± 3");
    assert_eq!(full("1.5(11)"), "1.5 ± 1.1");
    // An exponent scales both.
    near(parts("7.297_352_5643(11)e-3"), (7.2973525643e-3, 1.1e-12));
    // An uncertainty of zero is no uncertainty.
    assert_eq!(shown("10.5(0)"), "10.5");
    assert_eq!(full("-10.5(3)"), "-10.5 ± 0.3");
}

#[test]
fn only_digits_go_in_the_brackets() {
    assert!(!eval(None, "<expression>", "10.5(3.2)").ok());
    assert!(!eval(None, "<expression>", "10.5(x)").ok());
}

// ---- propagation -----------------------------------------------------------

#[test]
fn arithmetic_combines_in_quadrature() {
    near(parts("(10 ± 1) + (20 ± 2)"), (30.0, 5f64.sqrt()));
    near(parts("(10 ± 1) - (20 ± 2)"), (-10.0, 5f64.sqrt()));
    near(
        parts("(100 ± 2) * (0.2 ± 0.01)"),
        (20.0, 20.0 * (0.02f64.powi(2) + 0.05f64.powi(2)).sqrt()),
    );
    near(
        parts("(100 ± 2) / (50 ± 1)"),
        (2.0, 2.0 * (0.02f64.powi(2) + 0.02f64.powi(2)).sqrt()),
    );
}

#[test]
fn an_exact_operand_has_no_uncertainty() {
    assert_eq!(full("(10 ± 0.1) * 2"), "20 ± 0.2");
    assert_eq!(full("2 * (10 ± 0.1)"), "20 ± 0.2");
    assert_eq!(full("(10 ± 0.1) + 5"), "15 ± 0.1");
}

#[test]
fn operands_are_independent_even_when_they_are_the_same() {
    near(parts("(10 ± 1) - (10 ± 1)"), (0.0, 2f64.sqrt()));
    // Even the one cell, twice.
    let file = "sheet S\n\nX | Y\n--|--\n10 ± 1 | *\n\nY := X - X\n";
    let options = Options {
        exact: true,
        ..Options::default()
    };
    let out = omasheet_engine::view_with("test.omx", file, options);
    assert!(out.ok(), "{:?}", out.errors);
    assert!(
        out.output.contains("0 ± 1.4142135623730951"),
        "{}",
        out.output
    );
}

#[test]
fn functions_propagate_to_first_order() {
    near(
        parts("sin(0.5 ± 0.01)"),
        (0.5f64.sin(), 0.5f64.cos() * 0.01),
    );
    near(parts("ln(100 ± 1)"), (100f64.ln(), 0.01));
    near(parts("sqrt(4 ± 0.4)"), (2.0, 0.1));
    near(parts("exp(1 ± 0.1)"), (1f64.exp(), 1f64.exp() * 0.1));
    near(parts("(3 ± 0.1) ** 2"), (9.0, 0.6));
    near(parts("2 ** (3 ± 0.1)"), (8.0, 8.0 * 2f64.ln() * 0.1));
    // An exact argument stays exact.
    assert!(!full("sin(0.5)").contains('±'));
}

#[test]
fn a_comparison_uses_the_values() {
    assert_eq!(shown("(10 ± 1) > 9"), "true");
    assert_eq!(shown("(10 ± 1) == 10"), "true");
}

// ---- aggregation -----------------------------------------------------------

const LENGTHS: &str = "\
sheet Lengths

Length : Uncertain

Length
-----------
10 ± 0.1
11 ± 0.2
12 ± 0.1
";

fn lengths(expr: &str) -> String {
    let options = Options {
        exact: true,
        ..Options::default()
    };
    let out = eval_with(Some(("test.omx", LENGTHS)), "<expression>", expr, options);
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

#[test]
fn a_mean_and_a_sum_propagate() {
    let (v, s) = {
        let text = lengths("Lengths.Length.mean()");
        let (v, s) = text.split_once(" ± ").unwrap();
        (v.parse::<f64>().unwrap(), s.parse::<f64>().unwrap())
    };
    near((v, s), (11.0, 0.06f64.sqrt() / 3.0));
    let text = lengths("Lengths.Length.sum()");
    let (v, s) = text.split_once(" ± ").unwrap();
    near(
        (v.parse().unwrap(), s.parse().unwrap()),
        (33.0, 0.06f64.sqrt()),
    );
}

#[test]
fn a_maximum_keeps_its_uncertainty_and_a_spread_has_none() {
    assert_eq!(lengths("Lengths.Length.max()"), "12 ± 0.1");
    assert_eq!(lengths("Lengths.Length.min()"), "10 ± 0.1");
    assert!(!lengths("Lengths.Length.std()").contains('±'));
}

#[test]
fn a_group_sum_propagates() {
    let file = "\
sheet Items

Mass : Uncertain

Group | Mass
------|-----
a     | 10 ± 0.3
a     | 20 ± 0.4
b     | 5 ± 0.1

sheet Totals := Items |> group_by(Group) |> agg(Total := Mass.sum())
";
    let options = Options {
        exact: true,
        ..Options::default()
    };
    let out = omasheet_engine::view_with("test.omx", file, options);
    assert!(out.ok(), "{:?}", out.errors);
    assert!(out.output.contains("a     | 30 ± 0.5"), "{}", out.output);
    assert!(out.output.contains("b     |  5 ± 0.1"), "{}", out.output);
}

// ---- complex ---------------------------------------------------------------

/// A file of one cell calculated, and the warning the engine holds for it.
fn cell_with_warning(formula: &str) -> (String, Option<&'static str>) {
    let file = format!("sheet S\n\nV\n-\n= {formula}\n");
    let (program, diags) = compile(&file, 0);
    assert!(diags.is_empty(), "{diags:?}");
    let engine = Engine::with_options(&program, Options::default());
    engine.run();
    let value = engine.show(&engine.cell_shown(0, 0, 0), false);
    (value, engine.warning(0, 0, 0))
}

#[test]
fn an_uncertain_value_beside_a_complex_becomes_a_float() {
    let (value, warning) = cell_with_warning("(10 ± 1) + 3i");
    assert_eq!(value, "10+3i");
    assert_eq!(warning, Some("uncertain complex not supported"));
    // The same where a `Complex` is asked for.
    let (value, warning) = cell_with_warning("Complex(1 ± 0.1, 2)");
    assert_eq!(value, "1+2i");
    assert_eq!(warning, Some("uncertain complex not supported"));
    let (value, warning) = cell_with_warning("(1 ± 0.1).Complex");
    assert_eq!(value, "1+0i");
    assert_eq!(warning, Some("uncertain complex not supported"));
    // A real function of a negative number is `NaN`, as it is for a `Float`.
    let (value, warning) = cell_with_warning("sqrt(-4 ± 0.4)");
    assert_eq!(value, "NaN ± NaN");
    assert_eq!(warning, None);
    let (_, warning) = cell_with_warning("(10 ± 1) * 2");
    assert_eq!(warning, None);
    let (_, warning) = cell_with_warning("2i * 3");
    assert_eq!(warning, None);
}

#[test]
fn the_warning_belongs_to_the_cell_that_degraded() {
    // A cell that reads the degraded one is not itself warned.
    let file = "sheet S\n\nA | B\n--|--\n= (10 ± 1) + 3i | = A.re()\n";
    let (program, diags) = compile(file, 0);
    assert!(diags.is_empty(), "{diags:?}");
    let engine = Engine::with_options(&program, Options::default());
    engine.run();
    let _ = engine.cell_shown(0, 1, 0);
    assert!(engine.warning(0, 0, 0).is_some());
    assert!(engine.warning(0, 1, 0).is_none());
}

// ---- display ---------------------------------------------------------------

#[test]
fn the_default_display_is_the_concise_form() {
    // Two digits of uncertainty, and the value cut at the same place.
    assert_eq!(shown("(10 ± 0.1) + (1 ± 0.2)"), "11.00(22)");
    assert_eq!(shown("11 ± 0.0816"), "11.000(82)");
    assert_eq!(shown("7.2973525643e-3 ± 1.1e-12"), "7.2973525643(11)e-3");
    // No fallback to plus-minus when the uncertainty is the larger.
    assert_eq!(shown("0.5 ± 0.8"), "0.50(80)");
    // An uncertainty coarser than the units needs an exponent.
    assert_eq!(shown("1000 ± 200"), "1.00(20)e3");
    // No uncertainty is a plain number.
    assert_eq!(shown("10 ± 0"), "10");
}

#[test]
fn the_plus_minus_format_shows_value_plus_or_minus_uncertainty() {
    let file = "sheet S\n\nLength : Uncertain[±]\n\nLength\n------\n11 ± 0.0816\n";
    let out = view("test.omx", file);
    assert!(out.ok(), "{:?}", out.errors);
    assert!(out.output.contains("11.000 ± 0.082"), "{}", out.output);
}

#[test]
fn an_unknown_format_is_refused() {
    let errors = lint(
        "test.omx",
        "sheet S\n\nLength : Uncertain[x]\n\nLength\n------\n1\n",
    )
    .errors;
    assert!(errors.iter().any(|e| e.contains("`±`")), "{errors:?}");
}

// ---- columns ---------------------------------------------------------------

#[test]
fn an_uncertain_column_takes_the_three_spellings_and_exact_numbers() {
    let file = "\
sheet S

Length : Uncertain

Length
----------
10
10 ± 0.1
10.5(3)
";
    let out = view("test.omx", file);
    assert!(out.ok(), "{:?}", out.errors);
    assert!(out.output.contains("10.00(10)"), "{}", out.output);
    assert!(out.output.contains("10.50(30)"), "{}", out.output);
}

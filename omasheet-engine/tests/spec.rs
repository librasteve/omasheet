// Copyright (c) 2026 Stephen Roe

//! The scenarios of the Phase 1 specs
//! (`openspec/changes/archive/add-omasheet-core`)
//! and of `align-polars-vocabulary`, one test per scenario where it can be
//! checked from the outside.

use omasheet_engine::eval::Attempt;
use omasheet_engine::omx::check::ColKind;
use omasheet_engine::omx::date::MICROS;
use omasheet_engine::omx::date::{Order, Style};
use omasheet_engine::omx::{compile, compile_expr};
use omasheet_engine::{Engine, Lowering, Options, eval, eval_with, lint, view, view_with};

const SALES: &str = "\
sheet Sales

Month | Region | Revenue | Cost
Jan   | UK     | 100     | 60
Feb   | US     | 120     | 70
Mar   | UK     | 150     | 80
";

/// Exact numbers in full: these tests are about values, not their display.
fn exact() -> Options {
    Options {
        exact: true,
        ..Options::default()
    }
}

/// `SALES` with formula columns, each `Name := expression` on a line of
/// `formulas`: the header row names them, with `*` for their cells.
fn sales_with(formulas: &str) -> String {
    let names: Vec<&str> = formulas
        .lines()
        .filter_map(|line| line.split_once(":=").map(|(name, _)| name.trim()))
        .collect();
    let mut file = String::new();
    for (i, line) in SALES.lines().enumerate() {
        file.push_str(line);
        if line.contains('|') {
            for name in &names {
                file.push_str(" | ");
                file.push_str(if i == 2 { name } else { "*" });
            }
        }
        file.push('\n');
    }
    format!("{file}\n{formulas}")
}

/// Evaluate an expression with no file.
fn calc(expr: &str) -> String {
    let out = eval_with(None, "<expression>", expr, exact());
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

/// Evaluate an expression against a file.
fn ask(file: &str, expr: &str) -> String {
    let out = eval_with(Some(("test.omx", file)), "<expression>", expr, exact());
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

/// The first error from evaluating an expression.
fn calc_err(file: Option<&str>, expr: &str) -> String {
    let out = eval(file.map(|s| ("test.omx", s)), "<expression>", expr);
    assert!(!out.ok(), "`{expr}` should fail but gave {}", out.output);
    out.errors.join("\n")
}

fn shown(file: &str) -> String {
    let out = view_with("test.omx", file, exact());
    assert!(out.ok(), "view failed:\n{}", out.errors.join("\n"));
    out.output
}

fn lint_errors(file: &str) -> Vec<String> {
    lint("test.omx", file).errors
}

// ---- numerics ------------------------------------------------------------

#[test]
fn large_integers_keep_thirty_eight_digits_and_move_the_point() {
    // Past an `i64` a whole number is still exact, as a decimal.
    assert_eq!(calc("2 ** 64"), "18446744073709551616");
    let n = calc("10**37");
    assert_eq!(n.len(), 38);
    assert!(n.starts_with('1') && n[1..].chars().all(|c| c == '0'));
    // Past thirty-eight digits the point moves right of them, so a number
    // far larger than any Polars decimal is still an exact one, kept to the
    // digits it was given.
    assert_eq!(calc("10**1000"), "1e+1000");
    // Past where the scale itself reaches there is nowhere to put it, and
    // that is said rather than wrapped or quietly rounded.
    assert!(calc_err(None, "10**2000000000 * 10**2000000000").contains("too large"));
}

#[test]
fn large_decimals_keep_every_digit_they_can() {
    // Thirty-eight digits in all, so a number this size keeps seventeen
    // after the point and rounds there.
    assert_eq!(
        calc("999999999999999999999 / 7"),
        "142857142857142857142.714285714"
    );
}

#[test]
fn literals() {
    assert_eq!(calc("42.5 == 85/2"), "true");
    assert_eq!(calc("1_000_000.50 == 2000001/2"), "true");
    assert_eq!(calc("20% == 1/5"), "true");
    assert_eq!(calc("1e-100"), "1e-100");
}

#[test]
fn exact_arithmetic() {
    assert_eq!(calc("0.1 + 0.2"), "0.3");
    assert_eq!(calc("0.1 + 0.2 == 0.3"), "true");
    assert_eq!(calc("19.99 * 3 == 5997/100"), "true");
    // A decimal rounds at thirty-eight digits, and the last eight of those
    // are working room: they carry the rounding, and are neither compared
    // nor shown, so a third taken back out again is one.
    assert_eq!(calc("1 / 3 * 3"), "1");
    // One that ends comes back exactly.
    assert_eq!(calc("1 / 8 * 8"), "1");
    // Two ways of reaching one number reach the same answer, which is what
    // the guard digits are for: without them these differ in their last
    // digit, since a seventh of one and a seventh of two round opposite ways.
    assert_eq!(calc("2/7 + 3/7 == 5/7"), "true");
    assert_eq!(calc("1/7 + 1/7 == 2/7"), "true");
    assert_eq!(calc("1/3 + 1/3 + 1/3 == 1"), "true");
    assert_eq!(calc("1/7 * 7 == 1"), "true");
    assert_eq!(calc("1/6 + 1/3 == 1/2"), "true");
}

#[test]
fn floating_point_is_asked_for_never_assumed() {
    assert_eq!(calc("(1/3).Float"), "0.3333333333333333");
    assert_eq!(calc("Float(1) / 3"), "0.3333333333333333");
    // Exact arithmetic stays exact to the thirty digits it shows; it does
    // not fall back to floating point on its own.
    assert_eq!(
        calc("1/3 + 1/7 + 1/11 + 1/13 + 1/17"),
        "0.702846173434408728526375585199"
    );
    assert!(calc_err(None, "2 ** 0.5").contains("`.Float`"));
    assert!(calc_err(None, "approx(1/3)").contains("unknown function `approx`"));
}

/// `Decimal` is an exact number shown as a decimal, cut and marked where it
/// does not end.
#[test]
fn decimals() {
    // The literal says which: a decimal point or a fraction is a Decimal
    // and a `%` a Percent.
    for (expr, message) in [
        ("19.99 + true", "cannot apply `+` to Decimal and Boolean"),
        ("20% + true", "cannot apply `+` to Percent and Boolean"),
        // Sums, differences, products and quotients of decimals are
        // decimals, though a quotient need not end.
        (
            "19.99 * 3 - 0.5 + true",
            "cannot apply `+` to Decimal and Boolean",
        ),
        (
            "19.99 / 3 + true",
            "cannot apply `+` to Decimal and Boolean",
        ),
        (
            "19.99 + 1/7 + true",
            "cannot apply `+` to Decimal and Boolean",
        ),
    ] {
        let err = calc_err(None, expr);
        assert!(err.contains(message), "{expr}: {err}");
    }

    // A column of decimals and whole numbers is a Decimal column, and
    // takes them, and fractions and formulas that do not end, as decimals.
    let prices = "sheet P\n\nPrice : Decimal\n\nItem | Price | Half\n";
    assert!(
        lint_errors(&format!(
            "{prices}Tea | 4.50 | 1\nMug | 9 | 2\nTin | 20% | 3\n"
        ))
        .is_empty()
    );
    assert!(lint_errors(&format!("{prices}Tea | 1/7 | 1\n")).is_empty());
    assert!(lint_errors(&format!("{prices}Tea | = Half / 3 | 1\n")).is_empty());
    assert!(lint_errors(&format!("{prices}Tea | = Half * 1.5 | 1\n")).is_empty());

    // A power with a whole exponent written as it is keeps its base's type;
    // any other exponent may be negative, so the power is a Decimal.
    let powers = "sheet W\n\nN : Int\nD : Decimal\n\nN | D\n";
    assert!(lint_errors(&format!("{powers}= 2 ** 64 | = 1.1 ** 2\n")).is_empty());
    let errors = lint_errors(&format!("{powers}= 2 ** -1 | 1\n"));
    assert!(
        errors[0].contains("declared `Integer` but this is `Decimal`"),
        "{errors:?}"
    );
    assert!(lint_errors(&format!("{powers}2 | = 1.1 ** W[N; *]\n")).is_empty());
    assert_eq!(calc("2 ** 64"), "18446744073709551616");

    // A fraction goes where a Decimal is declared.
    let ratios = "sheet R\n\nShare : Decimal\n\nShare\n0.25\n1/7\n3\n= 0.5 * 0.5\n";
    assert!(lint_errors(ratios).is_empty());

    // One that does not end is shown cut and marked.
    assert_eq!(calc("(1/8).Decimal"), "0.125");
    assert_eq!(calc("(1.5e3).Decimal + 0.5"), "1500.5");
    assert_eq!(calc("\"20%\".Decimal"), "0.2");
    let shown = |expr: &str| {
        let out = eval(None, "<expression>", expr);
        assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
        out.output.trim_end().to_string()
    };
    assert_eq!(shown("(1/3).Decimal"), "0.33333333333333333333…");
    assert_eq!(shown("(1/3).Decimal * 3"), "1");
    let file =
        "sheet P\n\nHalf : Decimal\n\nN | Half\n8 | = (1 / N).Decimal\n3 | = (1 / N).Decimal\n";
    assert_eq!(ask(file, "P[Half; 0]"), "0.125");
    assert_eq!(ask(file, "P[Half; 1]"), "0.333333333333333333333333333333");

    // An average of decimals is a Decimal, as are their sum, least and
    // greatest.
    let file = "sheet P\n\nPrice\n4.50\n9\n0.25\n";
    assert!(calc_err(Some(file), "P.Price.sum() + true").contains("Decimal and Boolean"));
    assert!(calc_err(Some(file), "P.Price.max() + true").contains("Decimal and Boolean"));
    assert!(calc_err(Some(file), "P.Price.mean() + true").contains("Decimal and Boolean"));
    assert_eq!(ask(file, "P.Price.sum()"), "13.75");
}

/// A `Percent` is an exact number shown as so many in a hundred.
#[test]
fn percents() {
    assert_eq!(calc("20%"), "20%");
    assert_eq!(calc("-12.5%"), "-12.5%");
    assert_eq!(calc("20% == 0.2"), "true");

    // Percents add up to a Percent; anything else done with one gives the
    // plain number.
    assert_eq!(calc("20% + 5%"), "25%");
    assert_eq!(calc("20% - 5%"), "15%");
    assert_eq!(calc("-(20%)"), "-20%");
    assert_eq!(calc("abs(-20%)"), "20%");
    assert_eq!(calc("100 * 20%"), "20");
    assert_eq!(calc("20% * 50%"), "0.1");
    assert_eq!(calc("1 + 20%"), "1.2");
    assert_eq!(calc("20% / 3"), "0.0666666666666666666666666666667");
    assert_eq!(calc("[20%, 10%].sum()"), "30%");
    assert_eq!(
        calc("[20%, 10%, 10%].mean()"),
        "13.3333333333333333333333333333%"
    );
    assert_eq!(calc("[20%, 10%].max()"), "20%");
    assert_eq!(calc("[20%, 1/3].sum()"), "0.533333333333333333333333333333");
    for (expr, message) in [
        ("20% + 5% + true", "cannot apply `+` to Percent and Boolean"),
        ("20% * 2 + true", "cannot apply `+` to Decimal and Boolean"),
        (
            "20% + 0.5 + true",
            "cannot apply `+` to Decimal and Boolean",
        ),
        (
            "20% + 1.5e0 + true",
            "cannot apply `+` to Float and Boolean",
        ),
    ] {
        let err = calc_err(None, expr);
        assert!(err.contains(message), "{expr}: {err}");
    }

    // `.Percent` takes any number. It is rounded to two digits after the
    // point to be shown, and is still held in full.
    let show = |expr: &str| {
        let out = eval(None, "<expression>", expr);
        assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
        out.output.trim_end().to_string()
    };
    assert_eq!(show("(5/12).Percent"), "41.67%");
    assert_eq!(show("(-5/12).Percent"), "-41.67%");
    assert_eq!(show("(1/3).Percent"), "33.33%");
    assert_eq!(show("(2/3).Percent"), "66.67%");
    assert_eq!(show("[20%, 10%, 10%].mean()"), "13.33%");
    assert_eq!(show("(1/8).Percent"), "12.5%");
    assert_eq!(show("0.12345%"), "0.12%");
    assert_eq!(calc("0.12345%"), "0.12345%");
    assert_eq!(calc("(0.4).Percent"), "40%");
    assert_eq!(calc("2.Percent"), "200%");
    assert_eq!(calc("(1/8).Percent"), "12.5%");
    assert_eq!(calc("(5/12).Percent"), "41.6666666666666666666666666667%");
    // The last digit the twelfth left behind is a guard digit, so taking
    // the twelfth back out gives five.
    assert_eq!(calc("(5/12).Percent * 12"), "5");
    assert_eq!(
        calc("(5/12).Percent.String"),
        "41.6666666666666666666666666667%"
    );
    assert_eq!(calc("(1.5e-1).Percent"), "15%");
    assert_eq!(calc("\"20%\".Percent"), "20%");
    assert_eq!(calc("20%.Decimal"), "0.2");
    assert_eq!(calc("20%.Decimal"), "0.2");
    assert_eq!(calc("150%.Int"), "1");
    assert_eq!(calc("20%.Float"), "0.2");

    // A Percent column takes any exact number, and shows it as a percentage.
    let file = "sheet S\n\nMargin : Percent\nShare : Decimal\n\n\
                 Cost | Price | Rate | Margin | Share\n\
                 7 | 12 | 20% | * | 20%\n3 | 5 | 0.5 | * | 1/3\n\n\
                 Margin := (Price - Cost) / Price\n";
    assert!(lint_errors(file).is_empty());
    assert_eq!(
        ask(file, "S.Margin"),
        "[41.6666666666666666666666666667%, 40%]"
    );
    assert_eq!(
        ask(file, "S.Margin.mean()"),
        "40.8333333333333333333333333333%"
    );
    // A column of percentages is a Percent column; one with a plain number
    // among them is a Decimal column.
    assert!(calc_err(Some(file), "S[Rate; 0] + true").contains("Decimal and Boolean"));
    assert_eq!(ask(file, "S.Rate"), "[20%, 0.5]");
    // In a Decimal column a percentage is the number it stands for.
    assert_eq!(
        ask(file, "S.Share"),
        "[0.2, 0.333333333333333333333333333333]"
    );

    // What does not go in one.
    let rates = "sheet R\n\nRate : Percent\nWhole : Int\nEnds : Decimal\n\nRate | Whole | Ends\n";
    assert!(
        lint_errors(&format!(
            "{rates}20% | 1 | 20%\n0.4 | 2 | 0.5\n1/3 | 3 | 1\n"
        ))
        .is_empty()
    );
    let errors = lint_errors(&format!("{rates}hello | 1 | 1\n"));
    assert!(
        errors[0].contains("`hello` is not a `Percent` literal"),
        "{errors:?}"
    );
    let errors = lint_errors(&format!("{rates}= 1.5e0 | 1 | 1\n"));
    assert!(
        errors[0].contains("declared `Percent` but this is `Float`"),
        "{errors:?}"
    );
    let errors = lint_errors(&format!("{rates}20% | 20% | 1\n"));
    assert!(
        errors[0].contains("`20%` is not an `Integer` literal"),
        "{errors:?}"
    );
    // In a Decimal column a percentage is the number it stands for.
    let file = format!("{rates}20% | 1 | = Rate + 5%\n");
    assert!(lint_errors(&file).is_empty());
}

/// A `Fraction` is an exact number shown as a fraction of whole numbers.
#[test]
fn fractions() {
    let show = |expr: &str| {
        let out = eval(None, "<expression>", expr);
        assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
        out.output.trim_end().to_string()
    };
    // Shown as a fraction, with a whole part where there is one; in full it
    // is the fraction itself, which is how a file writes one.
    assert_eq!(show("(1/7).Fraction"), "1/7");
    assert_eq!(show("(22/7).Fraction"), "3 1/7");
    assert_eq!(show("(-22/7).Fraction"), "-3 1/7");
    assert_eq!(show("(0.125).Fraction"), "1/8");
    assert_eq!(show("(2.75).Fraction"), "2 3/4");
    assert_eq!(show("3.Fraction"), "3");
    assert_eq!(calc("(22/7).Fraction"), "22/7");
    assert_eq!(calc("(2.75).Fraction"), "11/4");
    // A fraction is recovered from the number, so one nobody wrote is found
    // too: no fraction of a seventh and a seventh was ever written down.
    assert_eq!(show("(1/7 + 1/7).Fraction"), "2/7");
    assert_eq!(show("(2/7 + 3/7).Fraction"), "5/7");
    assert_eq!(show("(1/3 + 1/6).Fraction"), "1/2");
    // Past the denominator it looks to, it is shown as a decimal.
    assert_eq!(show("(1/10000).Fraction"), "1/10000");
    assert_eq!(show("(1/10001).Fraction"), "0.00009999000099990000…");
    // A Fraction column shows its cells as fractions, whichever way they
    // were written, and anything worked out from one is a fraction too.
    let file = "sheet F\n\nYield : Fraction\n\nYield | Doubled | Half\n\
                1/7 | * | *\n2 3/4 | * | *\n0.5 | * | *\n3 | * | *\n\
                Doubled := Yield * 2\nHalf := Yield / 2\n";
    assert!(lint_errors(file).is_empty(), "{:?}", lint_errors(file));
    assert_eq!(ask(file, "F.Yield"), "[1/7, 11/4, 1/2, 3]");
    assert_eq!(ask(file, "F.Doubled"), "[2/7, 11/2, 1, 6]");
    assert_eq!(ask(file, "F.Half"), "[1/14, 11/8, 1/4, 3/2]");
    // It holds what a Decimal holds, so the two go either way.
    assert_eq!(calc("(1/7).Fraction == 1/7"), "true");
    assert_eq!(
        calc("(1/7).Fraction.Decimal"),
        "0.142857142857142857142857142857"
    );
    assert!(lint_errors("sheet F\n\nA : Fraction\nB : Decimal\n\nA | B\n1/3 | 1/3\n").is_empty());
    // What does not go in one.
    let errors = lint_errors("sheet F\n\nA : Fraction\n\nA\nhello\n");
    assert!(
        errors[0].contains("`hello` is not a `Fraction` literal"),
        "{errors:?}"
    );
}

#[test]
fn rationals_in_full() {
    assert_eq!(calc("175/4"), "43.75");
    assert_eq!(calc("1/3 + 1/3"), "0.666666666666666666666666666667");
    assert_eq!(
        calc("0.333333333333333333333333333333"),
        "0.333333333333333333333333333333"
    );
    // A third taken back out again is a hundred: the nines it leaves are
    // all in the guard digits.
    assert_eq!(calc("100/3 * 3"), "100");
    // A fraction of whole numbers is an exact `Decimal`, kept in lowest
    // terms, and one that is more than one is not split into parts.
    assert_eq!(
        calc("2.33333333333333333333333333333"),
        "2.33333333333333333333333333333"
    );
    // Thirty digits are shown, of the thirty-eight that are held, so a
    // literal written with more than thirty comes back to thirty.
    assert_eq!(
        calc("-3.14285714285714285714285714286"),
        "-3.14285714285714285714285714286"
    );
    assert_eq!(calc("10/6"), "1.66666666666666666666666666667");
    assert_eq!(calc("7/3 * 3"), "7");
    assert_eq!(calc("(7/3).Int"), "2");
    assert_eq!(calc("\"22/7\".Decimal"), "3.14285714285714285714285714286");
    // A whole number and a part may be written with a space, in a cell.
    assert_eq!(
        ask("sheet M\n\nA : Decimal\n\nA\n2 3/4\n", "M[A; 0]"),
        "2.75"
    );
    let file =
        "sheet T\n\nA : Decimal\n\nA | Twice\n7/3 | *\n-22/7 | *\n9/3 | *\n\nTwice := A * 2\n";
    assert!(lint_errors(file).is_empty());
    assert_eq!(
        ask(file, "T.A"),
        "[2.33333333333333333333333333333, -3.14285714285714285714285714286, 3]"
    );
    assert_eq!(
        ask(file, "T.Twice"),
        "[4.66666666666666666666666666667, -6.28571428571428571428571428571, 6]"
    );
    assert_eq!(ask(file, "T.A.sum()"), "2.19047619047619047619047619048");
}

#[test]
fn display_of_rationals() {
    let show = |expr: &str| {
        let out = eval(None, "<expression>", expr);
        assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
        out.output.trim_end().to_string()
    };
    // A Decimal that ends is shown as that decimal in full.
    assert_eq!(show("175/4"), "43.75");
    assert_eq!(show("1/100000"), "0.00001");
    assert_eq!(show("-1/64"), "-0.015625");
    assert_eq!(show("19.99 * 3"), "59.97");
    assert_eq!(show("100/3 * 3"), "100");
    // One that was rounded at the widest scale is cut and marked.
    assert_eq!(
        show("0.333333333333333333333333333333"),
        "0.33333333333333333333…"
    );
    assert_eq!(show("2/6 + 1/3"), "0.66666666666666666666…");
    assert_eq!(show("-7/3"), "-2.33333333333333333333…");
    assert_eq!(show("[1/3, 0.5]"), "[0.33333333333333333333…, 0.5]");
    // A Num is never rounded: shown with the fewest digits that give the
    // same Num back.
    assert_eq!(show("4.Float"), "4");
    assert_eq!(show("(1/3).Float"), "0.3333333333333333");
    assert_eq!(show("sqrt(2)"), "1.4142135623730951");
    assert_eq!(show("sqrt(2) * 1e20"), "1.4142135623730951e+20");
    assert_eq!(show("sqrt(2).String"), "1.4142135623730951");
    // Only the display: the value is exact still.
    assert_eq!(show("1/3 * 3 == 1"), "true");
    assert_eq!(show("(1/3).String"), "0.333333333333333333333333333333");
}

#[test]
fn conversions() {
    // Every type name converts to that type, written three ways.
    assert_eq!(calc("Int(19.99)"), "19");
    assert_eq!(calc("19.99.Int()"), "19");
    assert_eq!(calc("(-19.99).Int"), "-19");
    assert_eq!(calc("(10/4).Int + 1"), "3");
    assert_eq!(calc("Int(1.5e3)"), "1500");
    assert_eq!(calc("Int(true) + Int(false)"), "1");
    assert_eq!(calc("Decimal(1e-1)"), "0.1");
    assert_eq!(calc("Decimal(Float(1/3)) == 1/3"), "false");
    assert_eq!(calc("7.Decimal / 2"), "3.5");
    assert_eq!(calc("(1/4).Float"), "0.25");
    assert_eq!(calc("(1/3).String"), "0.333333333333333333333333333333");
    assert_eq!(calc("String(2025-01-31)"), "2025-01-31");
    assert_eq!(calc("Boolean(0)"), "false");
    assert_eq!(calc("0.5.Boolean"), "true");
    // Text is read the way a cell is.
    assert_eq!(calc("Int(\"42\") + 1"), "43");
    assert_eq!(calc("\"20%\".Decimal"), "0.2");
    assert_eq!(calc("Date(\"2025-01-31\") + 1"), "2025-02-01");
    assert_eq!(calc("Boolean(\"true\")"), "true");
    assert!(calc_err(None, "Int(\"abc\")").contains("`abc` is not an Int"));
    // Dates and times.
    assert_eq!(calc("2025-01-31T09:30.Date"), "2025-01-31");
    assert_eq!(calc("2025-01-31T09:30.Time"), "09:30");
    assert_eq!(calc("2025-01-31.Datetime"), "2025-01-31T00:00");
    // Each value of a vector, and empty stays empty.
    assert_eq!(ask(SALES, "(Sales.Revenue / 7).Int"), "[14, 17, 21]");
    assert_eq!(ask(SALES, "Sales.Revenue.Float.sum()"), "370");
    assert_eq!(ask("sheet T\n\nA | B\n1.5 |\n", "T.B.Int"), "[empty]");
    // What cannot be converted is caught before anything is calculated.
    assert!(calc_err(None, "Int(2025-01-31)").contains("`Int` cannot be applied to Date"));
    assert!(calc_err(Some(SALES), "Sales.Int").contains("has no column `Int`"));
}

// ---- omx-expressions -------------------------------------------------------

#[test]
fn reference_forms() {
    assert_eq!(ask(SALES, "Sales.Revenue"), "[100, 120, 150]");
    let err = calc_err(Some(SALES), "Revenue");
    assert!(err.contains("unknown name `Revenue`"), "{err}");
    assert!(err.contains("Sales.Revenue"), "{err}");
}

#[test]
fn selection() {
    assert_eq!(ask(SALES, "Sales[Revenue; 1]"), "120");
    assert_eq!(ask(SALES, "Sales[; 1].Revenue"), "120");
    assert_eq!(ask(SALES, "Sales[Revenue]"), "[100, 120, 150]");
    assert_eq!(
        ask(SALES, "Sales[; 2]"),
        "Month | Region | Revenue | Cost\n------|--------|---------|-----\nMar   | UK     |     150 |   80"
    );
    assert_eq!(
        ask(SALES, "Sales[2..3; 1..2]"),
        "Revenue | Cost\n--------|-----\n    120 |   70\n    150 |   80"
    );
}

#[test]
fn positional_indexing() {
    assert_eq!(ask(SALES, "Sales.Revenue[0]"), "100");
    assert_eq!(ask(SALES, "Sales.Revenue[-1]"), "150");
}

#[test]
fn ranges() {
    assert_eq!(ask(SALES, "Sales[; 0..2].len()"), "3");
    assert_eq!(ask(SALES, "Sales[; 0..^2].len()"), "2");
    assert_eq!(ask(SALES, "Sales[; 0^..2].len()"), "2");
    assert_eq!(ask(SALES, "Sales[; 0^..^2].len()"), "1");
    assert_eq!(ask(SALES, "Sales[; ^2].len()"), "2");
    assert_eq!(calc("sum(0^..10)"), "55");
    assert_eq!(calc("sum(^5)"), "10");
    assert_eq!(
        calc("[0 in 0^..10, 10 in 0^..10, 0 in ^5, 5 in ^5]"),
        "[false, true, true, false]"
    );
    let file = format!("const Rows = 1..2\nconst Cols = 2..3\n\n{SALES}");
    assert_eq!(
        ask(&file, "Sales[Cols; Rows]"),
        ask(SALES, "Sales[2..3; 1..2]")
    );
}

#[test]
fn row_cursor() {
    let file = sales_with(
        "Prev := Sales[Revenue; *-1]\nGrowth := Revenue / Sales[Revenue; *-1] - 1\nRunning := Sales[Revenue; 0..*].sum()\nWindow := Sales[Revenue; *-1..*].mean()\n",
    );
    assert_eq!(ask(&file, "Sales.Prev"), "[empty, 100, 120]");
    assert_eq!(ask(&file, "Sales.Growth"), "[empty, 0.2, 0.25]");
    assert_eq!(ask(&file, "Sales.Running"), "[100, 220, 370]");
    assert_eq!(ask(&file, "Sales.Window"), "[100, 110, 135]");
}

#[test]
fn cursor_needs_a_row() {
    let err = calc_err(Some(SALES), "Sales[Revenue; *-1]");
    assert!(err.contains("`*-1` needs a current row"), "{err}");
    let err = calc_err(Some(SALES), "Sales[Revenue; *]");
    assert!(err.contains("`*` needs a current row"), "{err}");
}

#[test]
fn cursor_survives_row_insertion() {
    let file = "sheet T\n\nV | Prev\n1 | *\n5 | *\n2 | *\n\nPrev := T[V; *-1]\n";
    assert_eq!(ask(file, "T.Prev"), "[empty, 1, 5]");
}

#[test]
fn predicate_selection() {
    assert_eq!(ask(SALES, "Sales[; Region == \"UK\"].Revenue.sum()"), "250");
    let dated = "sheet Sales\n\nDate | Region | Revenue\n2024-12-31 | UK | 1\n2025-03-01 | UK | 10\n2025-06-01 | US | 100\n2026-01-01 | UK | 1000\n";
    let expr = "Sales[;\n    Region == \"UK\" and\n    Date >= 2025-01-01 and\n    Date < 2026-01-01\n].Revenue.sum()";
    assert_eq!(ask(dated, expr), "10");
}

#[test]
fn datetime_columns() {
    let file = "sheet Log\n\nAt : Datetime\n\nAt | N\n2025-03-01T09:30 | 1\n2025-03-01T17:45:10 | 10\n2025-03-02T00:00 | 100\n";
    assert_eq!(
        ask(
            file,
            "Log[; At >= 2025-03-01T12:00 and At < 2025-03-02T00:00].N.sum()"
        ),
        "10"
    );
    assert_eq!(ask(file, "Log.At.max()"), "2025-03-02T00:00");
    assert_eq!(ask(file, "Log[At; 1]"), "2025-03-01T17:45:10");
    // An undeclared column infers Datetime from its cells.
    let inferred = file.replace("At : Datetime\n\n", "");
    assert_eq!(ask(&inferred, "Log.At.min()"), "2025-03-01T09:30");
    // A Date is not a Datetime.
    let err = calc_err(Some(file), "Log[; At >= 2025-03-01].N.sum()");
    assert!(err.contains("cannot compare Datetime and Date"), "{err}");
    let bad = "sheet Log\n\nAt : Datetime\n\nAt\n2025-03-01\n";
    let errs = lint_errors(bad);
    assert!(errs[0].contains("is not a `Datetime` literal"), "{errs:?}");
}

#[test]
fn time_columns() {
    let file = "sheet Log\n\nAt : Time\n\nAt | N\n09:30 | 1\n17:45:10 | 10\n00:00 | 100\n";
    assert_eq!(ask(file, "Log[; At >= 09:30 and At < 18:00].N.sum()"), "11");
    assert_eq!(ask(file, "Log.At.max()"), "17:45:10");
    assert_eq!(ask(file, "Log[At; 0]"), "09:30");
    // An undeclared column infers Time from its cells.
    let inferred = file.replace("At : Time\n\n", "");
    assert_eq!(ask(&inferred, "Log.At.min()"), "00:00");
    let err = calc_err(Some(file), "Log[; At >= 2025-03-01].N.sum()");
    assert!(err.contains("cannot compare Time and Date"), "{err}");
    let errs = lint_errors("sheet Log\n\nAt : Time\n\nAt\n9.30\n");
    assert!(errs[0].contains("is not a `Time` literal"), "{errs:?}");
    assert!(errs[0].contains("such as `09:30`"), "{errs:?}");
    assert!(calc_err(None, "24:00").contains("is not a real time"));
}

#[test]
fn date_and_time_arithmetic() {
    // A whole number is days next to a Date, seconds next to the others.
    assert_eq!(calc("2025-01-31 + 1"), "2025-02-01");
    assert_eq!(calc("7 + 2024-02-23"), "2024-03-01");
    assert_eq!(calc("2025-03-01 - 1"), "2025-02-28");
    assert_eq!(calc("2025-03-01 - 2024-03-01"), "365");
    assert_eq!(calc("2025-01-31T23:59 + 60"), "2025-02-01T00:00");
    assert_eq!(calc("2025-01-31T09:30 - 2025-01-30T09:00"), "88200");
    assert_eq!(calc("17:05 - 09:00"), "29100");
    // A time of day goes round midnight.
    assert_eq!(calc("23:30 + 3600"), "00:30");
    assert_eq!(calc("00:15 - 1800"), "23:45");
    assert_eq!(calc("2025-01-31 + 09:30"), "2025-01-31T09:30");
    assert_eq!(calc("09:30 + 2025-01-31"), "2025-01-31T09:30");
    assert_eq!(
        calc("[2025-01-01, 2025-12-31] + 1"),
        "[2025-01-02, 2026-01-01]"
    );

    for (expr, message) in [
        ("2025-01-31 * 2", "cannot apply `*` to Date and Int"),
        (
            "2025-01-31 + 2025-01-31",
            "cannot apply `+` to Date and Date",
        ),
        ("1 - 2025-01-31", "cannot apply `-` to Integer and Date"),
        ("2025-01-31 + 1.5", "cannot apply `+` to Date and Decimal"),
        ("2025-01-31 - 09:30", "cannot apply `-` to Date and Time"),
        (
            "2025-01-31T09:30 - 2025-01-31",
            "cannot apply `-` to Datetime and Date",
        ),
    ] {
        let err = calc_err(None, expr);
        assert!(err.contains(message), "{expr}: {err}");
    }

    let file = "sheet Jobs\n\nDue : Date\n\nStart | Days | Due\n2025-01-30 | 3 | = Start + Days\n";
    assert_eq!(ask(file, "Jobs[Due; 0]"), "2025-02-02");
    let bad = file.replace("Due : Date", "Due : Datetime");
    assert!(lint_errors(&bad)[0].contains("declared `Datetime` but this is `Date`"));
    // A conversion makes it fit.
    let fixed = bad.replace("= Start + Days", "= (Start + Days).Datetime");
    assert_eq!(ask(&fixed, "Jobs[Due; 0]"), "2025-02-02T00:00");
}

#[test]
fn date_and_time_parts() {
    assert_eq!(calc("2026-10-08.year()"), "2026");
    assert_eq!(calc("2026-10-08.month()"), "10");
    assert_eq!(calc("2026-10-08.day()"), "8");
    // Monday is 1 and Sunday is 7.
    assert_eq!(calc("2026-10-08.weekday()"), "4");
    assert_eq!(calc("weekday(2026-10-11)"), "7");
    assert_eq!(calc("17:47:09.hour()"), "17");
    assert_eq!(calc("17:47:09.minute()"), "47");
    assert_eq!(calc("17:47:09.second()"), "9");
    assert_eq!(calc("2026-10-08T17:47.date()"), "2026-10-08");
    assert_eq!(calc("2026-10-08T17:47.time()"), "17:47");
    assert_eq!(calc("2026-10-08T17:47.year()"), "2026");
    assert_eq!(calc("2026-10-08T17:47.hour()"), "17");
    assert_eq!(calc("[2025-01-31, 2026-02-28].month()"), "[1, 2]");
    assert!(calc_err(None, "09:30.year()").contains("`year` needs a Date or a Datetime"));
    assert!(calc_err(None, "2026-10-08.hour()").contains("`hour` needs a Time or a Datetime"));
    assert!(calc_err(None, "year(2026)").contains("but this is Int"));
}

#[test]
fn today_and_now_come_from_the_options() {
    let options = Options {
        now: Some((20_734 * 86_400 + 17 * 3600 + 47 * 60) * MICROS),
        ..Options::default()
    };
    let at = |expr: &str, options: &Options| {
        let options = options.clone();
        let out = eval_with(None, "<expression>", expr, options);
        assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
        out.output.trim_end().to_string()
    };
    assert_eq!(at("today()", &options), "2026-10-08");
    assert_eq!(at("now()", &options), "2026-10-08T17:47");
    assert_eq!(at("today() - 2026-01-01", &options), "280");
    assert_eq!(at("now() - (today() + 00:00)", &options), "64020");
    assert!(calc_err(None, "today(1)").contains("`today` takes 0 arguments"));
    // The clock of this machine otherwise.
    assert_eq!(calc("now().date() == today()"), "true");

    // The style changes how dates are shown, never how they are written.
    let gb = Options {
        style: Style::new(Order::Dmy, '/', false),
        ..options.clone()
    };
    let us = Options {
        style: Style::new(Order::Mdy, '/', true),
        ..options.clone()
    };
    assert_eq!(at("today()", &gb), "08/10/2026");
    assert_eq!(at("now()", &gb), "08/10/2026 17:47");
    assert_eq!(at("[today(), 2025-01-31]", &gb), "[08/10/2026, 31/01/2025]");
    assert_eq!(at("now()", &us), "10/08/2026 5:47 PM");
    assert_eq!(at("2026-10-08 + 00:00:05", &us), "10/08/2026 12:00:05 AM");
    let file = "const Start = 2025-01-31\n\nsheet T\n\nAt\n09:30\n";
    assert_eq!(
        view_with("test.omx", file, gb).output,
        "const Start = 31/01/2025\n\nsheet T\n\nAt\n-----\n09:30\n"
    );
    assert_eq!(
        shown(file),
        "const Start = 2025-01-31\n\nsheet T\n\nAt\n-----\n09:30\n"
    );
}

#[test]
fn index_with_no_sheet_name() {
    // In a sheet, `[...]` with a `;` or a row cursor is that sheet.
    let file = "sheet T\n\nV | Prev | Run | Same | First | All | Row\n1 | * | * | * | * | * | *\n5 | * | * | * | * | * | *\n2 | * | * | * | * | * | *\n\nPrev := [V; *-1]\nRun := [V; 0..*].sum()\nSame := [V; *]\nFirst := [V; 0]\nAll := [V; ].sum()\nRow := [; *-1].V // 0\n";
    assert_eq!(ask(file, "T.Prev"), "[empty, 1, 5]");
    assert_eq!(ask(file, "T.Run"), "[1, 6, 8]");
    assert_eq!(ask(file, "T.Same"), "[1, 5, 2]");
    assert_eq!(ask(file, "T.First"), "[1, 1, 1]");
    assert_eq!(ask(file, "T.All"), "[8, 8, 8]");
    assert_eq!(ask(file, "T.Row"), "[0, 1, 5]");
    // `*` as a column is the formula's own column, and counts from it.
    let grid = "sheet G\n\nA | B | C\n1 | = [*-1; *] + 1 | = [*-1; *] * 10\n5 | = [*; *-1] + [A; *] | = [*+0; *-1]\n";
    assert_eq!(ask(grid, "G.B"), "[2, 7]");
    assert_eq!(ask(grid, "G.C"), "[20, 20]");
    let off = lint_errors("sheet G\n\nA | B\n= [*-1; *] | 2\n");
    assert!(off[0].contains("no column 1 to the left"), "{off:?}");
    let err = calc_err(Some(grid), "G[*; 0]");
    assert!(err.contains("`*` as a column"), "{err}");
    // A vector literal is still a vector.
    assert_eq!(calc("[1, 2, 3].sum()"), "6");
    assert_eq!(calc("[4]"), "[4]");
    // Outside a sheet there is no sheet to mean.
    let err = calc_err(Some(file), "[V; 0]");
    assert!(err.contains("needs a current sheet"), "{err}");
}

#[test]
fn math_functions() {
    // Exact numbers stay exact where they can.
    assert_eq!(calc("abs(-7/2)"), "3.5");
    assert_eq!(
        calc("[round(5/2), round(-5/2), floor(-7/2), ceil(7/2)]"),
        "[3, -3, -4, 4]"
    );
    assert_eq!(calc("[sign(-3), sign(0), sign(1/2)]"), "[-1, 0, 1]");
    assert_eq!(calc("round(2.5e0)"), "3");
    // With a second argument, to that many decimal places, as Polars has
    // it. The number keeps its own type, and a half goes away from zero.
    assert_eq!(calc("round(3.14159, 2)"), "3.14");
    assert_eq!(calc("round(19.74, 1)"), "19.7");
    assert_eq!(
        calc("round([1.234, 1.235, -1.235], 2)"),
        "[1.23, 1.24, -1.24]"
    );
    assert_eq!(calc("round(pi(), 3)"), "3.142");
    assert_eq!(calc("(3.14159).round(3)"), "3.142");
    // Scaling by a power of ten keeps a Float and a Decimal in agreement
    // here, where dividing by the multiple would not have: 2.785 as a Float
    // is a shade over it, and 2.785 / 0.01 is 278.49999999999994.
    assert_eq!(calc("round(2.785, 2)"), "2.79");
    assert_eq!(calc("round(2.785e0, 2)"), "2.79");
    // A whole number has no places to lose, and an Inf none to round to.
    assert_eq!(calc("round(1234, 2)"), "1234");
    assert_eq!(calc("round(1/0, 2)"), "Inf");
    // The nearest hundred is said as the arithmetic it is.
    assert_eq!(calc("round(1250 / 100) * 100"), "1300");
    assert!(calc_err(None, "round(1, 2, 3)").contains("`round` takes 1 or 2 arguments"));
    assert!(calc_err(None, "round(1, 0.01)").contains("number of decimal places"));
    assert!(calc_err(None, "round(1, \"a\")").contains("number of decimal places"));
    // The rest give a Num.
    assert_eq!(calc("sqrt(16)"), "4");
    assert_eq!(calc("exp(0)"), "1");
    assert_eq!(calc("ln(exp(2))"), "2");
    assert_eq!(calc("[log10(1000), log2(8)]"), "[3, 3]");
    assert_eq!(calc("sin(0) + cos(0)"), "1");
    assert_eq!(calc("degrees(pi())"), "180");
    assert_eq!(calc("e()"), "2.718281828459045");
    assert_eq!(calc("ln(e())"), "1");
    assert_eq!(calc("e() == exp(1)"), "true");
    assert!(calc_err(None, "e(1)").contains("`e` takes 0 arguments"));
    assert_eq!(calc("round(sin(radians(30)) * 1000)"), "500");
    assert_eq!(calc("round(degrees(arctan(1)))"), "45");
    // Over a column, and as a method.
    assert_eq!(ask(SALES, "Sales.Revenue.sqrt().floor()"), "[10, 10, 12]");
    // Outside the domain, and on the wrong kind of value.
    assert_eq!(calc("[sqrt(-1), ln(0), arcsin(2)]"), "[NaN, -Inf, NaN]");
    assert!(calc_err(None, "sin(\"x\")").contains("`sin` cannot be applied to String"));
    assert!(calc_err(None, "sqr(4)").contains("did you mean `sqrt`?"));
    // Every function in the directory is one the checker knows.
    for f in omasheet_engine::omx::funcs::FUNCTIONS {
        let out = eval(None, "<expression>", &format!("{}()", f.name));
        assert!(
            !out.errors.join("\n").contains("unknown function"),
            "{}",
            f.name
        );
    }
}

#[test]
fn time_zones() {
    use omasheet_engine::zone::Zone;
    // A machine in London, whatever this one is set to.
    let london = Options {
        zone: Zone::named("Europe/London"),
        ..Options::default()
    };
    let with = |options: &Options, file: Option<&str>, expr: &str| {
        let file = file.map(|s| ("test.omx", s));
        let out = eval_with(file, "<expression>", expr, options.clone());
        assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
        out.output.trim_end().to_string()
    };
    let at = |expr: &str| with(&london, None, expr);

    // The same instant on other clocks, with daylight saving.
    assert_eq!(
        at("2025-07-15T12:00.convert_time_zone(\"Asia/Tokyo\")"),
        "2025-07-15T20:00+09:00"
    );
    assert_eq!(at("2025-07-15T12:00.utc()"), "2025-07-15T11:00+00:00");
    assert_eq!(at("2025-01-15T12:00.utc()"), "2025-01-15T12:00+00:00");
    assert_eq!(
        at("2025-07-15T12:00.convert_time_zone(\"America/St_Johns\")"),
        "2025-07-15T08:30-02:30"
    );
    assert_eq!(at("2025-07-15T12:00.base_utc_offset()"), "3600");
    assert_eq!(at("2025-01-15T12:00.base_utc_offset()"), "0");
    assert_eq!(at("2025-07-15T12:00.time_zone()"), "Europe/London");
    assert_eq!(at("2025-07-15T12:00.utc().time_zone()"), "UTC");
    assert_eq!(at("2025-07-15T12:00.utc().base_utc_offset()"), "0");
    // Back to the file's own zone it is an ordinary date-time again.
    assert_eq!(
        at(
            "2025-07-15T12:00.convert_time_zone(\"Asia/Tokyo\").convert_time_zone(\"Europe/London\")"
        ),
        "2025-07-15T12:00"
    );
    assert_eq!(at("2025-07-15T12:00.utc().local()"), "2025-07-15T12:00");

    // The parts are those of the zone it is in.
    assert_eq!(
        at("2025-07-15T20:00.convert_time_zone(\"Asia/Tokyo\").hour()"),
        "4"
    );
    assert_eq!(
        at("2025-07-15T20:00.convert_time_zone(\"Asia/Tokyo\").date()"),
        "2025-07-16"
    );
    // Seconds are added on its own clocks.
    assert_eq!(
        at("2025-07-15T12:00.convert_time_zone(\"Asia/Tokyo\") + 3600"),
        "2025-07-15T21:00+09:00"
    );
    // Across zones it is the instant that is compared and subtracted.
    assert_eq!(
        at("2025-07-15T12:00.convert_time_zone(\"Asia/Tokyo\") == 2025-07-15T12:00"),
        "true"
    );
    assert_eq!(
        at("2025-07-15T12:00.utc() < 2025-07-15T12:00:01.convert_time_zone(\"Asia/Tokyo\")"),
        "true"
    );
    assert_eq!(
        at("2025-07-15T13:00.convert_time_zone(\"Asia/Tokyo\") - 2025-07-15T12:00.utc()"),
        "3600"
    );
    assert_eq!(at("2025-07-15T13:00 - 2025-07-15T12:00.utc()"), "3600");
    // In one zone the clocks are taken at their word: the night the clocks go
    // forward has 24 hours on the wall and 23 in fact.
    assert_eq!(at("2025-03-30T12:00 - 2025-03-29T12:00"), "86400");
    assert_eq!(
        at("2025-03-30T12:00.utc() - 2025-03-29T12:00.utc()"),
        "82800"
    );
    assert_eq!(
        at("[2025-01-15T12:00, 2025-07-15T12:00].utc()"),
        "[2025-01-15T12:00+00:00, 2025-07-15T11:00+00:00]"
    );

    let out = eval_with(
        None,
        "<expression>",
        "2025-07-15T12:00.convert_time_zone(\"Mars/Base\")",
        london.clone(),
    );
    assert!(out.errors[0].contains("there is no time zone `Mars/Base`"));
    assert!(calc_err(None, "2025-07-15.utc()").contains("`utc` needs a Datetime"));
    assert!(calc_err(None, "now().convert_time_zone(3)").contains("needs the name of a time zone"));

    // A file names the zone its date-times are in.
    let file = "zone America/New_York\n\nsheet Calls\n\nAt | Tokyo\n2025-03-03T09:12 | *\n\n\
                 Tokyo := At.convert_time_zone(\"Asia/Tokyo\")\n";
    let ny = |expr: &str| with(&london, Some(file), expr);
    assert_eq!(ny("Calls[At; 0]"), "2025-03-03T09:12");
    assert_eq!(ny("Calls[At; 0].time_zone()"), "America/New_York");
    assert_eq!(ny("Calls[At; 0].base_utc_offset()"), "-18000");
    assert_eq!(ny("Calls[Tokyo; 0]"), "2025-03-03T23:12+09:00");
    assert_eq!(ny("Calls[At; 0].local()"), "2025-03-03T14:12+00:00");
    assert_eq!(ny("Calls[At; 0].utc() == Calls[At; 0]"), "true");
    // `now()` is on the file's clocks: five hours behind London in winter.
    assert_eq!(ny("now().time_zone()"), "America/New_York");
    assert_eq!(ny("now().local() == now()"), "true");
    let fixed = Options {
        now: Some(20_734 * 86_400 * MICROS),
        ..london.clone()
    };
    assert_eq!(with(&fixed, Some(file), "now()"), "2026-10-08T00:00");
    assert_eq!(
        with(&fixed, Some(file), "now().local()"),
        "2026-10-08T05:00+01:00"
    );

    // Shown on this machine's clocks only when that is asked for.
    let local = Options {
        local: true,
        ..london.clone()
    };
    assert_eq!(with(&local, Some(file), "Calls[At; 0]"), "2025-03-03T14:12");
    assert_eq!(with(&local, Some(file), "Calls.At"), "[2025-03-03T14:12]");
    assert_eq!(
        with(&local, Some(file), "Calls[Tokyo; 0]"),
        "2025-03-03T23:12+09:00"
    );
    assert_eq!(with(&local, Some(file), "Calls[At; 0].hour()"), "9");
    assert_eq!(
        view_with("test.omx", file, local).output,
        "sheet Calls\n\nAt               | Tokyo\n-----------------|-----------------------\n\
         2025-03-03T14:12 | 2025-03-03T23:12+09:00\n"
    );

    // The zone line is checked, and belongs before the first sheet.
    let errs = lint_errors("zone Mars/Base\n\nsheet T\n\nA\n1\n");
    assert!(
        errs[0].contains("test.omx:1:6: error: there is no time zone `Mars/Base`"),
        "{errs:?}"
    );
    assert!(!view("test.omx", "zone Mars/Base\n\nsheet T\n\nA\n1\n").ok());
    let errs = lint_errors("zone UTC\nzone Asia/Tokyo\n\nsheet T\n\nA\n1\n");
    assert!(
        errs[0].contains("the file already has a `zone`"),
        "{errs:?}"
    );
    assert!(lint_errors("zone UTC\n\nsheet T\n\nA\n1\n").is_empty());
    assert_eq!(ask("sheet T\n\nA\nzone UTC\n", "T[A; 0]"), "zone UTC");
}

#[test]
fn one_slot_selects_columns() {
    assert_eq!(ask(SALES, "Sales[Revenue]"), ask(SALES, "Sales.Revenue"));
    assert_eq!(ask(SALES, "Sales[Revenue; ]"), "[100, 120, 150]");
    assert_eq!(ask(SALES, "Sales[2]"), "[100, 120, 150]");
    assert_eq!(ask(SALES, "Sales[Revenue; 1]"), "120");
    assert_eq!(ask(SALES, "Sales[2; -1]"), "150");
    assert_eq!(ask(SALES, "Sales[; 1].Revenue"), "120");
    assert_eq!(
        ask(SALES, "Sales[2..3; 0]"),
        "Revenue | Cost\n--------|-----\n    100 |   60"
    );
    // Rows alone need the `;`.
    let err = calc_err(Some(SALES), "Sales[Region == \"UK\"]");
    assert!(
        err.contains("a column selector must be a column name"),
        "{err}"
    );
    assert_eq!(ask(SALES, "Sales[; Region == \"UK\"].Revenue.sum()"), "250");
    // A vector has one dimension, so its one slot is the element.
    assert_eq!(ask(SALES, "Sales.Revenue[1]"), "120");
    assert_eq!(ask(SALES, "Sales[Revenue][1]"), "120");
}

#[test]
fn lookup_from_another_sheet() {
    let file = "\
sheet Customers

ID | Name
1  | Ada
2  | Grace

sheet Sales

CustomerID | Amount | Customer
2          | 50     | *
7          | 20     | *
1          | 10     | *

Customer := Customers[; ID == CustomerID].Name // \"Unknown\"
";
    assert_eq!(
        ask(file, "Sales.Customer"),
        "[\"Grace\", \"Unknown\", \"Ada\"]"
    );
}

#[test]
fn operators() {
    let file = sales_with("Prev := Sales[Revenue; *-1] // 0\nHome := Region in [\"UK\", \"IE\"]\n");
    assert_eq!(ask(&file, "Sales.Prev"), "[0, 100, 120]");
    assert_eq!(ask(&file, "Sales.Home"), "[true, false, true]");
    let err = calc_err(Some(SALES), "Sales[; Region = \"UK\"]");
    assert!(err.contains("=="), "{err}");
}

#[test]
fn conditional_expression() {
    let file = "sheet T\n\nRevenue | Region | Net\n2000 | UK | *\n500 | UK | *\n\nNet :=\n  if Revenue > 1000 and Region in [\"UK\", \"US\"]\n  then Revenue * 90%\n  else Revenue\n";
    assert_eq!(ask(file, "T.Net"), "[1800, 500]");
}

#[test]
fn value_shapes() {
    assert!(ask(SALES, "Sales[; Region == \"UK\"]").starts_with("Month | Region"));
    assert_eq!(
        ask(SALES, "Sales[; Region == \"UK\"].Revenue"),
        "[100, 150]"
    );
    assert_eq!(ask(SALES, "Sales[; Region == \"UK\"].Revenue.sum()"), "250");
}

#[test]
fn aggregation_methods() {
    assert_eq!(ask(SALES, "Sales.Revenue.sum()"), "370");
    assert_eq!(
        ask(SALES, "Sales.Revenue.mean()"),
        "123.333333333333333333333333333"
    );
    assert_eq!(ask(SALES, "Sales.Revenue.min()"), "100");
    assert_eq!(ask(SALES, "Sales.Revenue.max()"), "150");
    assert_eq!(ask(SALES, "Sales.Revenue.count()"), "3");
}

#[test]
fn broadcasting() {
    assert_eq!(calc("[100, 200, 300] * 20%"), "[20, 40, 60]");
    assert_eq!(ask(SALES, "Sales.Revenue - Sales.Cost"), "[40, 50, 70]");
    assert_eq!(
        ask(SALES, "Sales[; Sales.Revenue > 110].Month"),
        "[\"Feb\", \"Mar\"]"
    );
    let err = calc_err(None, "[1, 2, 3] + [1, 2, 3, 4]");
    assert!(err.contains("different lengths"), "{err}");
}

#[test]
fn pipe_operator() {
    let piped = "Sales\n  |> filter(Region == \"UK\")\n  |> select(Revenue)\n  |> sum()";
    assert_eq!(
        ask(SALES, piped),
        ask(SALES, "Sales[; Region == \"UK\"].Revenue.sum()")
    );
}

/// A sheet whose keys tie and whose last Revenue is missing, which is
/// where the two sort questions are asked.
const TIES: &str = "\
sheet Sales

Month | Region | Revenue
Jan   | UK     | 120
Feb   | US     | 100
Mar   | UK     | 120
Apr   | UK     |
";

#[test]
fn sort_orders_a_sheet_by_a_column() {
    // Ascending by default, descending on the bare word, and the sheet
    // keeps every column and every row either way.
    assert_eq!(
        ask(SALES, "(Sales |> sort(Cost)).Month"),
        "[\"Jan\", \"Feb\", \"Mar\"]"
    );
    assert_eq!(
        ask(SALES, "Sales |> sort(Cost, desc) |> select(Month, Cost)"),
        "Month | Cost\n------|-----\nMar   |   80\nFeb   |   70\nJan   |   60"
    );
    assert_eq!(
        ask(SALES, "(Sales |> sort(Revenue, asc)).Revenue"),
        "[100, 120, 150]"
    );
    assert_eq!(ask(SALES, "Sales |> sort(Month) |> len()"), "3");
}

#[test]
fn sort_orders_a_column_by_its_own_values() {
    assert_eq!(ask(SALES, "Sales.Cost |> sort()"), "[60, 70, 80]");
    assert_eq!(ask(SALES, "Sales.Cost |> sort(desc)"), "[80, 70, 60]");
    assert_eq!(calc("[3, 1, 2] |> sort()"), "[1, 2, 3]");
    // A NaN is greatest, by D56, so it sorts last and an empty cell first.
    assert_eq!(calc("[1, NaN, 3] |> sort()"), "[1, 3, NaN]");
}

#[test]
fn sort_is_stable_and_takes_more_than_one_key() {
    // Jan and Mar tie on Revenue and keep the order they came in.
    assert_eq!(
        ask(TIES, "(Sales |> sort(Revenue)).Month"),
        "[\"Apr\", \"Feb\", \"Jan\", \"Mar\"]"
    );
    // Two keys, the second breaking the first's ties.
    assert_eq!(
        ask(TIES, "(Sales |> sort(Region, Revenue)).Month"),
        "[\"Apr\", \"Jan\", \"Mar\", \"Feb\"]"
    );
    // Two sorts compose into what a reader expects, the first key the
    // weaker one.
    assert_eq!(
        ask(TIES, "(Sales |> sort(Month) |> sort(Region)).Month"),
        "[\"Apr\", \"Jan\", \"Mar\", \"Feb\"]"
    );
}

#[test]
fn an_empty_cell_sorts_first() {
    // Smallest, as Polars orders a null: first ascending, last descending.
    assert_eq!(
        ask(TIES, "(Sales |> sort(Revenue)).Revenue"),
        "[empty, 100, 120, 120]"
    );
    assert_eq!(
        ask(TIES, "(Sales |> sort(Revenue, desc)).Month"),
        "[\"Jan\", \"Mar\", \"Feb\", \"Apr\"]"
    );
}

#[test]
fn sort_says_what_it_needs() {
    assert!(
        calc_err(Some(SALES), "Sales |> sort()").contains("needs a column to order a sheet by")
    );
    assert!(calc_err(Some(SALES), "Sales |> sort(Profit)").contains("has no column `Profit`"));
    assert!(calc_err(Some(SALES), "Sales.Cost |> sort(Cost)").contains("takes no column name"));
    assert!(calc_err(Some(SALES), "3 |> sort()").contains("needs a sheet or a column"));
}

/// Orders whose regions repeat, whose last region is missing and whose
/// dates fall in two months, to group by each.
const ORDERS: &str = "\
sheet Orders

Placed : Date

Region | Placed     | Qty | Price | Net
UK     | 2025-01-31 | 3   | 4.50  | *
US     | 2025-02-01 | 2   | 7.25  | *
UK     | 2025-02-03 | 6   | 9     | *
       | 2025-02-03 | 1   | 2     | *

Net := Qty * Price
";

/// The cells of sheet `name` as shown, from its header down.
fn sheet_of(file: &str, name: &str) -> String {
    let out = shown(file);
    let start = out.find(&format!("sheet {name}\n")).expect("no such sheet");
    let rest = &out[start..];
    let end = rest[1..].find("\nsheet ").map_or(rest.len(), |e| e + 1);
    rest[..end].trim_end().to_string()
}

#[test]
fn group_by_makes_a_row_a_group() {
    let file = format!(
        "{ORDERS}\nsheet ByRegion := Orders |> group_by(Region) |> agg(Qty.sum(), Net.mean(), Orders := Qty.len())\n"
    );
    // The keys first, then an aggregate a column named for its expression
    // or by `:=`. The groups are in the order of their keys, the empty one
    // first, and the computed `Net` is aggregated as any column is.
    assert_eq!(
        sheet_of(&file, "ByRegion"),
        "sheet ByRegion\n\n\
Region | Qty_sum | Net_mean | Orders
-------|---------|----------|-------
       |       1 |        2 |      1
UK     |       9 |    33.75 |      2
US     |       2 |     14.5 |      1"
    );
}

#[test]
fn group_by_takes_more_than_one_key_and_a_computed_one() {
    let file = format!(
        "{ORDERS}\nsheet ByDay := Orders |> filter(not is_null(Region)) |> group_by(Placed, Region) |> agg(Net.sum())\n\nsheet ByNet := Orders |> group_by(Net) |> agg(Qty.max())\n"
    );
    assert_eq!(
        sheet_of(&file, "ByDay"),
        "sheet ByDay\n\n\
Placed     | Region | Net_sum
-----------|--------|--------
2025-01-31 | UK     |    13.5
2025-02-01 | US     |    14.5
2025-02-03 | UK     |      54"
    );
    assert!(sheet_of(&file, "ByNet").contains("13.5 |       3"));
}

#[test]
fn a_grouped_sheet_is_a_sheet_like_any_other() {
    let file = format!(
        "{ORDERS}\nsheet ByRegion := Orders |> group_by(Region) |> agg(Net.sum())\n\nsheet Best := ByRegion |> sort(Net_sum, desc) |> head(1)\n"
    );
    assert!(sheet_of(&file, "Best").contains("UK     |    67.5"));
    assert_eq!(ask(&file, "ByRegion.Net_sum.sum()"), "84");
    assert_eq!(ask(&file, "ByRegion |> len()"), "3");
}

#[test]
fn agg_says_what_it_needs() {
    let with =
        |stage: &str| lint_errors(&format!("{ORDERS}\nsheet G := Orders |> {stage}\n")).join("\n");
    assert!(with("group_by(Region) |> agg(Qty)").contains("so it needs an aggregate"));
    assert!(
        with("group_by(Region) |> agg((Qty * 2).sum())").contains("needs a name for its column")
    );
    assert!(
        with("group_by(Region) |> agg(Qty.sum(), Qty.sum())")
            .contains("two columns named `Qty_sum`")
    );
    assert!(with("group_by(Country) |> agg(Qty.sum())").contains("has no column `Country`"));
    assert!(
        with("group_by(Region) |> agg(Qty.sum()) |> head(1)").contains("ends a sheet's formula")
    );
    assert!(
        calc_err(Some(ORDERS), "Orders |> group_by(Region)").contains("ends a sheet's formula")
    );
    assert!(calc_err(Some(ORDERS), "sum(X := 1)").contains("only inside `agg`"));
}

/// Targets for some of the months of `ORDERS`-like sales: one month twice,
/// one month there are no sales for, and one with no month at all.
const TARGETS: &str = "\
sheet Sales

| Month | Revenue |
| ----- | ------- |
| Jan   | 100     |
| Feb   | 120     |
| Mar   | 150     |
|       | 90      |

sheet Targets

| Month | Revenue | Owner |
| ----- | ------- | ----- |
| Feb   | 110     | Bob   |
| Jan   | 95      | Ann   |
| Feb   | 130     | Cy    |
| Apr   | 140     | Ann   |
|       | 1       | Nil   |
";

#[test]
fn join_pairs_the_rows_whose_keys_are_equal() {
    let file = format!("{TARGETS}\nsheet Both := Sales |> join(Targets, Month)\n");
    // A row for every match, Feb's two among them, in the left sheet's
    // order and then the right's; a name both have takes `_right`; and an
    // empty key matches nothing, not even another empty key.
    assert_eq!(
        sheet_of(&file, "Both"),
        "sheet Both\n\n\
Month | Revenue | Revenue_right | Owner
------|---------|---------------|------
Jan   |     100 |            95 | Ann
Feb   |     120 |           110 | Bob
Feb   |     120 |           130 | Cy"
    );
}

#[test]
fn a_left_join_keeps_a_row_that_matches_nothing() {
    let file = format!("{TARGETS}\nsheet Both := Sales |> join(Targets, Month, left)\n");
    assert_eq!(
        sheet_of(&file, "Both"),
        "sheet Both\n\n\
Month | Revenue | Revenue_right | Owner
------|---------|---------------|------
Jan   |     100 |            95 | Ann
Feb   |     120 |           110 | Bob
Feb   |     120 |           130 | Cy
Mar   |     150 |               |
      |      90 |               |"
    );
    // A joined sheet is a sheet like any other, by its name.
    assert_eq!(ask(&file, "Both.Revenue_right.sum()"), "335");
    assert_eq!(ask(&file, "Both |> len()"), "5");
}

#[test]
fn join_says_what_it_needs() {
    let with =
        |stage: &str| lint_errors(&format!("{TARGETS}\nsheet J := Sales |> {stage}\n")).join("\n");
    assert!(
        with("join(Targets)").contains("needs a sheet, a sheet to join it to and a key column")
    );
    assert!(with("join(Targets, left)").contains("needs a key column"));
    assert!(with("join(Targets, Owner)").contains("sheet `Sales` has no column `Owner`"));
    assert!(with("join(Targets.Owner, Month)").contains("joins a sheet to a sheet"));
    assert!(with("join(Targets, Month) |> head(1)").contains("ends a sheet's formula"));
}

/// Sales of two regions taking turns, and two with no region at all.
const REGIONS: &str = "\
sheet Sales

| Month | Region | Revenue | Before | After | Running |
| ----- | ------ | ------- | ------ | ----- | ------- |
| Jan   | UK     | 100     | *      | *     | *       |
| Jan   | US     | 80      | *      | *     | *       |
| Feb   | UK     | 120     | *      | *     | *       |
| Feb   |        | 5       | *      | *     | *       |
| Mar   | US     | 90      | *      | *     | *       |
| Mar   | UK     | 150     | *      | *     | *       |
| Apr   |        | 7       | *      | *     | *       |

Before  := [Revenue; *-1 by Region]
After   := [Revenue; *+1 by Region]
Running := Revenue + ([Running; *-1 by Region] // 0)
";

#[test]
fn a_cursor_offset_counts_within_its_partition() {
    // The row before and after among the rows of the same region, empty
    // past either end of it, and a running total per region made of the
    // one before. Rows with no region are a partition of their own, as an
    // empty key is a group of its own.
    assert_eq!(
        ask(REGIONS, "Sales.Before"),
        "[empty, empty, 100, empty, 80, 120, 5]"
    );
    assert_eq!(
        ask(REGIONS, "Sales.After"),
        "[120, 90, 150, 7, empty, empty, empty]"
    );
    assert_eq!(
        ask(REGIONS, "Sales.Running"),
        "[100, 80, 220, 5, 170, 370, 12]"
    );
    // Two keys at once: no region has two rows in a month, so nothing is
    // before anything. And the sheet named before the index.
    let two = REGIONS.replace(
        "[Revenue; *-1 by Region]",
        "[Revenue; *-1 by Region, Month]",
    );
    assert_eq!(ask(&two, "Sales.Before.count()"), "0");
    let named = REGIONS.replace("[Revenue; *-1 by Region]", "Sales[Revenue; *-1 by Region]");
    assert_eq!(
        ask(&named, "Sales.Before"),
        "[empty, empty, 100, empty, 80, 120, 5]"
    );
}

#[test]
fn by_says_what_it_needs() {
    let with = |formula: &str| {
        lint_errors(&REGIONS.replace("Before  := [Revenue; *-1 by Region]", formula)).join("\n")
    };
    assert!(with("Before := [Revenue; 0..* by Region].sum()").contains("counts a cursor offset"));
    assert!(with("Before := [Revenue; *-1 by Country]").contains("has no column `Country`"));
    assert!(
        calc_err(Some(REGIONS), "Sales[Revenue; *-1 by Region]").contains("needs a current row")
    );
}

// ---- file-format ----------------------------------------------------------

#[test]
fn minimal_sheet_and_optional_separator() {
    let plain =
        "sheet Sales\n\nMonth | Revenue | Cost\nJan   | 10000   | 6000\nFeb   | 12000   | 7000\n";
    let ruled = "sheet Sales\n\nMonth | Revenue | Cost\n------|---------|-----\nJan   | 10000   | 6000\nFeb   | 12000   | 7000\n";
    assert_eq!(shown(plain), shown(ruled));
    assert_eq!(ask(plain, "Sales.len()"), "2");
}

#[test]
fn ragged_row() {
    let errors = lint_errors("sheet Sales\n\nMonth | Revenue\nJan | 1 | 2\n");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].starts_with("test.omx:4:1:"), "{}", errors[0]);
    assert!(errors[0].contains("sheet `Sales`"), "{}", errors[0]);
}

#[test]
fn multiple_sheets() {
    let file = sales_with("sheet Summary\n\nLabel | Value\nTotal | = Sales.Revenue.sum()\n");
    assert_eq!(ask(&file, "Summary[Value; 0]"), "370");
    let errors = lint_errors("sheet Sales\n\nA\n1\n\nsheet Sales\n\nA\n2\n");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].starts_with("test.omx:6:"), "{}", errors[0]);
}

#[test]
fn column_schema() {
    let file =
        "sheet Items\n\nQty   : Int\nPrice : Decimal\n\nQty | Price\n2   | 19.99\n5   | 0.50\n";
    assert_eq!(ask(file, "Items[Price; 0] == 1999/100"), "true");
    assert_eq!(ask(file, "Items[Qty; 0] * Items[Price; 0]"), "39.98");
    let errors = lint_errors("sheet Items\n\nPrise : Decimal\n\nQty | Price\n2 | 1\n");
    assert!(errors[0].contains("no column `Prise`"), "{}", errors[0]);
}

#[test]
fn a_sheet_may_be_formulas_alone() {
    // Written as `:=` lines with no table of its own, a sheet has no cells
    // to write: its columns are those lines, in the order they are written,
    // and its length is the length of the sheets its formulas pick a row of
    // with `*`.
    let margin = format!(
        "{SALES}\nsheet Margin\n\nTake := Sales[Revenue; *] - Sales[Cost; *]\nOver := Take > 50\n"
    );
    assert_eq!(lint_errors(&margin), Vec::<String>::new());
    assert_eq!(ask(&margin, "Margin.Take"), "[40, 50, 70]");
    assert_eq!(ask(&margin, "Margin.Over"), "[false, false, true]");
    assert_eq!(
        shown(&margin).split("sheet Margin").nth(1).unwrap(),
        "\n\nTake | Over\n-----|------\n  40 | false\n  50 | false\n  70 | true\n"
    );
    // A row of the longest, where several are read; a row past the end of a
    // shorter one is empty, as any row outside a sheet is.
    let both = format!(
        "{SALES}\nsheet Two\n\nA | B\n1 | 2\n\nsheet Both\n\nPair := Sales[Revenue; *] + (Two[A; *] // 0)\n"
    );
    assert_eq!(ask(&both, "Both.Pair"), "[101, 120, 150]");
    // An aggregate reads a whole column rather than a row, so a sheet of
    // aggregates is one row long.
    let summary = format!("{SALES}\nsheet Summary\n\nTotal := Sales.Revenue.sum()\n");
    assert_eq!(
        shown(&summary).split("sheet Summary").nth(1).unwrap(),
        "\n\nTotal\n-----\n  370\n"
    );
    // The sheet is a sheet: what reads it selects and aggregates as ever.
    assert_eq!(ask(&margin, "Margin.Take.sum()"), "160");
    assert_eq!(ask(&margin, "Margin[Take; -1]"), "70");
}

#[test]
fn computed_columns() {
    let file = sales_with("Profit := Revenue - Cost\n");
    assert_eq!(ask(&file, "Sales.Profit"), "[40, 50, 70]");
    // The header row may name a formula column, to say where it goes among
    // the others. Its cells are written `*`.
    let among = "sheet T\n\nA | Twice | B | Sum\n1 | * | 10 | *\n2 | * | 20 | *\n\nTwice := A * 2\nSum := Twice + B\n";
    assert_eq!(lint_errors(among), Vec::<String>::new());
    assert_eq!(
        shown(among),
        "sheet T\n\nA | Twice | B  | Sum\n--|-------|----|----\n1 |     2 | 10 |  12\n2 |     4 | 20 |  24\n"
    );
    // Counting columns counts it where it is.
    assert_eq!(ask(among, "T[1; 1]"), "4");
    let errors = lint_errors("sheet T\n\nA | Profit\n1 | 2\n\nProfit := A\n");
    assert!(
        errors[0].contains("`Profit` is a formula column: its cells are written `*`"),
        "{}",
        errors[0]
    );
    // The header row has to name it.
    let unnamed = lint_errors("sheet T\n\nA\n1\n\nP := A\n");
    assert!(
        unnamed[0].contains("formula column `P` is not in the header row of sheet `T`"),
        "{}",
        unnamed[0]
    );
    assert!(
        unnamed[0].contains("write `*` in its cells"),
        "{}",
        unnamed[0]
    );
    let twice = lint_errors("sheet T\n\nA | P\n1 | *\n\nP := A\nP := A + 1\n");
    assert!(
        twice[0].contains("computed column `P` is defined twice"),
        "{}",
        twice[0]
    );
}

#[test]
fn cell_content() {
    // Text in an undeclared column.
    assert_eq!(ask(SALES, "Sales[Month; 0]"), "Jan");
    // A marked formula in a typed column.
    let file = "const TaxRate = 20%\n\nsheet T\n\nTax : Decimal\n\nRevenue | Tax\n100 | = Revenue * TaxRate\n";
    assert_eq!(ask(file, "T[Tax; 0]"), "20");
    // A fraction of two whole numbers is a Decimal literal, declared or not.
    let thirds = "sheet T\n\nA : Decimal\n\nA | B\n1/7 | 2/3\n-3/6 | 1/3\n4/2 | x\n";
    assert_eq!(
        ask(thirds, "T.A"),
        "[0.142857142857142857142857142857, -0.5, 2]"
    );
    assert_eq!(ask(thirds, "T[B; 0..1].sum()"), "1");
    assert!(
        lint_errors("sheet T\n\nA : Decimal\n\nA\n1/0\n")[0].contains("is not a `Decimal` literal")
    );
    assert_eq!(ask("sheet T\n\nA\n1/0\n", "T[A; 0]"), "1/0");
    // An exact number over zero is a Num: `Inf`, `-Inf`, or `NaN` for zero.
    assert_eq!(
        calc("[1/0, -1/0, 0/0, 1.5/0, 0 ** -1]"),
        "[Inf, -Inf, NaN, Inf, Inf]"
    );
    // They are written as they are shown, and text of the same is in quotes.
    assert_eq!(calc("[Inf, -Inf, NaN]"), "[Inf, -Inf, NaN]");
    assert_eq!(calc("[\"Inf\", \"NaN\"]"), "[\"Inf\", \"NaN\"]");
    assert_eq!(calc("[Inf - Inf, 1 / Inf, Inf + 1]"), "[NaN, 0, Inf]");
    assert_eq!(
        calc("[Inf == Inf, 1 < Inf, -Inf < 1]"),
        "[true, true, true]"
    );
    // A NaN is greatest and equal to itself, as Polars orders it.
    assert_eq!(
        calc("[NaN == NaN, NaN != NaN, NaN < 1, 1 <= NaN]"),
        "[true, false, false, true]"
    );
    assert_eq!(calc("[[1, NaN, 3].max(), [1, Inf, 3].max()]"), "[NaN, Inf]");
    // Least is the least of the numbers, since a NaN is greatest.
    assert_eq!(calc("[1, NaN, 3].min()"), "1");
    assert!(calc_err(None, "Inf.Int").contains("Inf has no exact value"));
    let cells = "sheet T\n\nN : Float\nT : String\n\nN | T | Any | Quoted\nInf | Inf | -Inf | \"NaN\"\nNaN | NaN | NaN | \"Inf\"\n";
    assert_eq!(ask(cells, "T.N"), "[Inf, NaN]");
    assert_eq!(
        ask(cells, "[T[Any; 0] < 0, T[Any; 1] != T[Any; 1]]"),
        "[true, false]"
    );
    assert_eq!(ask(cells, "T.T"), "[\"Inf\", \"NaN\"]");
    assert_eq!(ask(cells, "T.Quoted"), "[\"NaN\", \"Inf\"]");
    // An unmarked expression in a typed column is an error.
    let errors = lint_errors("sheet T\n\nTax : Decimal\n\nRevenue | Tax\n100 | Revenue * 2\n");
    assert!(
        errors[0].contains("is not a `Decimal` literal"),
        "{}",
        errors[0]
    );
    // An unmarked expression in a text column is text.
    let file = "sheet T\n\nNote : String\n\nRevenue | Note\n100 | Revenue - Cost\n200 | \"= see appendix\"\n";
    assert_eq!(
        ask(file, "T.Note"),
        "[\"Revenue - Cost\", \"= see appendix\"]"
    );
    // An empty cell.
    assert_eq!(ask("sheet T\n\nA | B\n1 |\n", "T[B; 0]"), "empty");
}

#[test]
fn constants() {
    let file = "const TaxRate = 20%\n\nsheet Sales\n\nRevenue | Tax\n100 | *\n250 | *\n\nTax := Revenue * TaxRate\n";
    assert_eq!(ask(file, "Sales.Tax"), "[20, 50]");
    assert_eq!(ask(file, "TaxRate == 1/5"), "true");
}

#[test]
fn a1_reference_is_not_special() {
    let errors = lint_errors("sheet T\n\nA | B\n1 | *\n\nB := B2\n");
    assert!(errors[0].contains("unknown name `B2`"), "{}", errors[0]);
}

// ---- evaluation ------------------------------------------------------------

#[test]
fn unresolved_name_is_a_compile_error() {
    let errors = lint_errors(
        "sheet Sales\n\nRevenue | Cost | Profit\n1 | 2 | *\n\nProfit := Revnue - Cost\n",
    );
    assert_eq!(errors.len(), 1);
    assert!(errors[0].starts_with("test.omx:6:11:"), "{}", errors[0]);
    assert!(
        errors[0].contains("did you mean `Revenue`?"),
        "{}",
        errors[0]
    );
}

#[test]
fn one_type_error_blocks_execution() {
    let file = "sheet Sales\n\nMonth | Revenue | Double | Bad\nJan | 100 | * | *\n\nDouble := Revenue * 2\nBad := Revenue + Month\n";
    let out = view("test.omx", file);
    assert_eq!(out.errors.len(), 1);
    assert!(
        out.errors[0].contains("cannot apply `+` to Integer and String"),
        "{}",
        out.errors[0]
    );
    assert!(
        out.output.is_empty(),
        "nothing may be calculated: {}",
        out.output
    );
}

#[test]
fn all_errors_reported_together() {
    let file = "sheet T\n\nName | N | A | B | C\nx | 1 | * | * | *\n\nA := N + Name\nB := Name * 2\nC := not N\n";
    assert_eq!(lint_errors(file).len(), 3);
}

#[test]
fn declaration_order_does_not_matter() {
    let file = "sheet T\n\nRevenue | Cost | Margin | Profit\n100 | 60 | * | *\n\nMargin := Profit / Revenue\nProfit := Revenue - Cost\n";
    assert_eq!(ask(file, "T.Margin"), "[0.4]");
}

#[test]
fn row_wise_self_reference() {
    let file = "sheet Ledger\n\nAmount | Balance\n10 | *\n-3 | *\n5 | *\n\nBalance := (Ledger[Balance; *-1] // 0) + Amount\n";
    assert_eq!(ask(file, "Ledger.Balance"), "[10, 7, 12]");
}

#[test]
fn cycle_detection() {
    let errors = lint_errors("sheet T\n\nX | A | B\n1 | * | *\n\nA := B + 1\nB := A + 1\n");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("A → B → A"), "{}", errors[0]);
}

// ---- cli (the parts that do not need a process) ----------------------------

#[test]
fn view_shows_computed_values() {
    let file = sales_with("Profit := Revenue - Cost\n");
    assert_eq!(
        shown(&file),
        "\
sheet Sales

Month | Region | Revenue | Cost | Profit
------|--------|---------|------|-------
Jan   | UK     |     100 |   60 |     40
Feb   | US     |     120 |   70 |     50
Mar   | UK     |     150 |   80 |     70
"
    );
}

#[test]
fn lint_clean_and_type_error() {
    assert!(lint_errors(SALES).is_empty());
    let errors = lint_errors(&sales_with("Bad := Revenue + Month\n"));
    assert_eq!(errors.len(), 1);
}

#[test]
fn diagnostics_are_located() {
    let file = "sheet T\n\nName | N | Bad\nx | 1 | *\n\n\n\n\n\n\n\n\n\nBad := N + Name\n";
    let errors = lint_errors(file);
    assert!(
        errors[0].starts_with("test.omx:14:8: error:"),
        "{}",
        errors[0]
    );
    assert!(errors[0].contains("14 | Bad := N + Name"), "{}", errors[0]);
    assert!(errors[0].contains("^^^^^^^^"), "{}", errors[0]);
}

// ---- custom functions ----------------------------------------------------

const FUNCS: &str = "\
const Rate = 20%

fn Margin(revenue, cost) = (revenue - cost) / revenue
fn WithTax(x) = x * (1 + Rate)
fn Twice(x) = WithTax(WithTax(x))
fn Total(t) = t.Revenue.sum()
fn NameOf(id) = Customers[; ID == id].Name // \"Unknown\"

sheet Customers

ID | Name
1  | Ada
2  | Grace

sheet Sales

Month | Revenue | Cost | Who | Margin | Gross | Twice | Buyer
Jan   | 100     | 60   | 1   | *      | *     | *     | *
Feb   | 120     | 90   | 9   | *      | *     | *     | *

Margin  := Margin(Revenue, Cost)
Gross   := Revenue.WithTax()
Twice   := Revenue |> Twice()
Buyer   := NameOf(Who)
";

#[test]
fn custom_function_in_a_computed_column() {
    assert_eq!(ask(FUNCS, "Sales.Margin"), "[0.4, 0.25]");
    assert_eq!(ask(FUNCS, "Sales.Buyer"), "[\"Ada\", \"Unknown\"]");
}

#[test]
fn custom_function_as_method_and_pipe_stage() {
    assert_eq!(ask(FUNCS, "Sales.Gross"), "[120, 144]");
    assert_eq!(ask(FUNCS, "Sales.Twice"), "[144, 172.8]");
    assert_eq!(ask(FUNCS, "100.WithTax()"), ask(FUNCS, "WithTax(100)"));
}

#[test]
fn custom_function_takes_sheets_and_vectors() {
    assert_eq!(ask(FUNCS, "Total(Sales)"), "220");
    assert_eq!(ask(FUNCS, "Total(Sales |> filter(Cost > 60))"), "120");
    assert_eq!(ask(FUNCS, "WithTax(Sales.Revenue)"), "[120, 144]");
}

#[test]
fn custom_function_does_not_see_the_callers_row() {
    let file =
        "fn Bad(x) = x + Cost\n\nsheet T\n\nRevenue | Cost | A\n1 | 2 | *\n\nA := Bad(Revenue)\n";
    let errors = lint_errors(file).join("\n");
    assert!(errors.contains("unknown name `Cost`"), "{errors}");
}

#[test]
fn custom_function_is_checked_for_each_call() {
    let err = calc_err(Some(FUNCS), "WithTax(\"ten\")");
    assert!(
        err.contains("in `WithTax`: cannot apply `*` to String"),
        "{err}"
    );
    let err = calc_err(Some(FUNCS), "Margin(1)");
    assert!(err.contains("`Margin` takes 2 arguments, found 1"), "{err}");
    assert!(err.contains("Margin(revenue, cost)"), "{err}");
    let err = calc_err(Some(FUNCS), "Margin(\"ten\", 1)");
    assert!(
        err.contains("in `Margin`: cannot apply `-` to String"),
        "{err}"
    );
    // Nothing is a tenth of nothing, and that is a value, not an error.
    assert_eq!(ask(FUNCS, "Margin(0, 1)"), "-Inf");
}

#[test]
fn custom_function_definitions_are_checked() {
    let errors = lint_errors("fn sum(x) = x\n").join("\n");
    assert!(errors.contains("`sum` is a built-in function"), "{errors}");
    let errors = lint_errors("fn F(x) = x\nfn F(y) = y\n").join("\n");
    assert!(
        errors.contains("function `F` is already defined"),
        "{errors}"
    );
    let errors = lint_errors("fn F(x, x) = x\n").join("\n");
    assert!(errors.contains("two parameters named `x`"), "{errors}");
    let errors = lint_errors("fn F(x) = F(x) + 1\n").join("\n");
    assert!(errors.contains("function `F` calls itself"), "{errors}");
    let errors = lint_errors("fn A(x) = B(x)\nfn B(x) = A(x)\n").join("\n");
    assert!(errors.contains("calls itself"), "{errors}");
    let errors = lint_errors("fn F(x) = x + Nope\n").join("\n");
    assert!(errors.contains("unknown name `Nope`"), "{errors}");
    assert!(lint_errors("fn F(t, n) = t[; n].Revenue + t[; Revenue > n].Cost.sum()\n").is_empty());
}

#[test]
fn a_column_range_counts_from_the_formulas_own_column() {
    let file = "sheet T\n\nN | A | B   | C | D | E | F\n\
                 x | 1 | 2   | 3 | 4 | = sum([*-4..*-1; *])     | = [*-5..^*-2; *].max()\n\
                 y | 1 | 2.5 | 3 | 4 | = sum([1..*-1; *])       | = mean([*-5..-4; *])\n\
                 z | 1 | 2   | 3 | 4 | = [*-4..*-1; *].count()  | = min([*-3..*-2; *])\n";
    assert_eq!(ask(file, "T.E"), "[10, 10.5, 4]");
    assert_eq!(ask(file, "T.F"), "[3, 2.16666666666666666666666666667, 3]");

    let errors = |cell: &str| {
        lint_errors(&format!("sheet T\n\nN | A | B | C\nx | 1 | 2 | {cell}\n")).join("\n")
    };
    assert!(errors("= sum([*-9..*-1; *])").contains("runs past the first column"));
    assert!(errors("= sum([*-1..*+1; *])").contains("runs past the last column"));
    assert!(errors("= sum([*-3..*-1; *])").contains("the columns of this row differ"));
    assert!(errors("= sum([*-1..*; *])").contains("circular reference"));
}

#[test]
fn aggregates_take_several_vectors_and_blocks_together() {
    assert_eq!(
        ask(SALES, "sum(Sales[Revenue; 0..1], Sales[Revenue; 2])"),
        "370"
    );
    assert_eq!(
        ask(SALES, "mean(Sales.Revenue, Sales.Cost)"),
        "96.6666666666666666666666666667"
    );
    assert_eq!(ask(SALES, "max(Sales[Cost; 0..1], 75)"), "75");
    assert_eq!(ask(SALES, "count(Sales.Revenue, Sales[Cost; 0])"), "4");
    // A block of cells, every one of them.
    assert_eq!(ask(SALES, "sum(Sales[2..3; 0..1])"), "350");
    assert_eq!(
        ask(SALES, "min(Sales[Revenue; 2], Sales[2..3; 0..1])"),
        "60"
    );

    let errors = lint_errors(&format!("{SALES}\nT := sum(Revenue, Month)\n")).join("\n");
    assert!(errors.contains("needs values of one type"), "{errors}");
    let errors = lint_errors(&format!("{SALES}\nT := sum(Sales[1..2; 0..1])\n")).join("\n");
    assert!(
        errors.contains("the columns of this block differ"),
        "{errors}"
    );
}

// The scenarios of `align-polars-vocabulary`: the names are Polars' names.

/// A sheet whose `Note` is empty in one row, for counting and for emptiness.
const NOTES: &str = "\
sheet T

Note | N
a    | 1
     | 2
c    | 3
d    | 4
";

#[test]
fn counting_values_and_counting_rows() {
    assert_eq!(ask(NOTES, "T.Note.count()"), "3");
    assert_eq!(ask(NOTES, "T.Note.len()"), "4");
    assert_eq!(ask(NOTES, "T.len()"), "4");
    assert_eq!(ask(NOTES, "T[; N > 2].len()"), "2");
    // One name asked two questions; now each has its own.
    let errors = calc_err(Some(NOTES), "T.count()");
    assert!(
        errors.contains("for the rows of a sheet, write `len`"),
        "{errors}"
    );
}

#[test]
fn the_spread_and_the_middle() {
    assert_eq!(calc("[1, 3, 2, 4].median()"), "2.5");
    assert_eq!(calc("[1, 3, 2].median()"), "2");
    assert_eq!(calc("[2, 3, 4].product()"), "24");
    assert_eq!(calc("[5, 6, 7].first()"), "5");
    assert_eq!(calc("[5, 6, 7].last()"), "7");
    // The textbook set: a population deviation of 2, and a sample's above it.
    let set = "[2, 4, 4, 4, 5, 5, 7, 9]";
    assert_eq!(calc(&format!("{set}.std(0)")), "2");
    assert_eq!(calc(&format!("{set}.var(0)")), "4");
    assert_eq!(calc(&format!("{set}.std()")), "2.138089935299395");
    // A quantile is taken at the nearest value, as Polars takes it, so it is
    // one of the values and keeps their type.
    assert_eq!(calc("[1, 2, 3, 4].quantile(0)"), "1");
    assert_eq!(calc("[1, 2, 3, 4].quantile(0.5)"), "3");
    assert_eq!(calc("[1, 2, 3, 4].quantile(1)"), "4");
    let errors = calc_err(None, "[1, 2].quantile(2)");
    assert!(errors.contains("from 0 to 1"), "{errors}");
}

#[test]
fn the_first_rows_the_last_and_the_one_before() {
    // Each is the selection it is equivalent to, for a sheet and a column.
    assert_eq!(ask(SALES, "(Sales |> head(2)).Month"), "[\"Jan\", \"Feb\"]");
    assert_eq!(ask(SALES, "(Sales |> tail(2)).Month"), "[\"Feb\", \"Mar\"]");
    assert_eq!(
        ask(SALES, "Sales |> head(2) |> select(Revenue) |> sum()"),
        "220"
    );
    assert_eq!(ask(SALES, "Sales.Revenue.head(2)"), "[100, 120]");
    assert_eq!(ask(SALES, "Sales.Revenue.tail(2)"), "[120, 150]");
    assert_eq!(
        ask(SALES, "Sales[Revenue; ^2]"),
        ask(SALES, "Sales.Revenue.head(2)")
    );
    // A shift is the cursor offset written as a function, so the row before
    // the first is empty.
    let file = sales_with("Prev := Sales.Revenue.shift(1)\nSame := Sales[Revenue; *-1]\n");
    assert_eq!(ask(&file, "Sales.Prev"), "[empty, 100, 120]");
    assert_eq!(ask(&file, "Sales.Prev"), ask(&file, "Sales.Same"));
    // It reads the cursor, so like `*` it belongs in a sheet's own formula.
    let errors = calc_err(Some(SALES), "Sales.Revenue.shift(1)");
    assert!(errors.contains("needs a current row"), "{errors}");
}

#[test]
fn empty_or_not() {
    assert_eq!(
        ask(NOTES, "T.Note.is_null()"),
        "[false, true, false, false]"
    );
    assert_eq!(
        ask(NOTES, "T.Note.is_not_null()"),
        "[true, false, true, true]"
    );
    // The function and the operator it stands for are the same thing, and
    // each takes one value.
    assert_eq!(ask(NOTES, "fill_null(T[Note; 1], \"none\")"), "none");
    assert_eq!(
        ask(NOTES, "T[Note; 1] // \"none\""),
        ask(NOTES, "fill_null(T[Note; 1], \"none\")")
    );
    assert_eq!(
        ask(NOTES, "is_in(T[N; 0], [1, 2])"),
        ask(NOTES, "T[N; 0] in [1, 2]")
    );
}

#[test]
fn a_nan_and_a_missing_value_are_different_absences() {
    // A NaN is a Float; an empty cell is a missing value. So `is_nan` of an
    // empty cell has no answer, where `is_null` of it has one.
    assert_eq!(calc("[is_nan(0/0), is_null(0/0)]"), "[true, false]");
    assert_eq!(calc("[is_nan(1), is_not_nan(1)]"), "[false, true]");
    // An infinity is a number, however large.
    assert_eq!(calc("[is_nan(1/0), is_not_nan(1/0)]"), "[false, true]");
    assert_eq!(ask(NOTES, "is_nan(T[Note; 1])"), "empty");
    assert_eq!(ask(NOTES, "is_null(T[Note; 1])"), "true");
    // Over a column, each cell in turn.
    assert_eq!(
        ask(NOTES, "T.Note.is_nan()"),
        "[false, empty, false, false]"
    );
}

#[test]
fn casting_is_converting() {
    // Four spellings of one conversion.
    assert_eq!(calc("cast(19.99, Integer)"), "19");
    assert_eq!(calc("Integer(19.99)"), "19");
    assert_eq!(calc("(19.99).Integer()"), "19");
    assert_eq!(calc("(19.99).Integer"), "19");
    // `Int` is what this type was called before, and is still read.
    assert_eq!(calc("(19.99).Int"), "19");
    assert_eq!(calc("cast(19.99, Int)"), "19");
    let errors = calc_err(None, "cast(1, Nope)");
    assert!(errors.contains("unknown type `Nope`"), "{errors}");
}

// ---- the kinds of text ---------------------------------------------------

#[test]
fn text_is_read_as_a_narrower_kind_of_text() {
    // The same four spellings a number's conversion has.
    let address = "name@example.com";
    assert_eq!(calc(&format!("cast(\"{address}\", Email)")), address);
    assert_eq!(calc(&format!("Email(\"{address}\")")), address);
    assert_eq!(calc(&format!("\"{address}\".Email()")), address);
    assert_eq!(calc(&format!("\"{address}\".Email")), address);
    assert_eq!(calc("\"https://example.com\".Url"), "https://example.com");
    let uuid = "7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f";
    assert_eq!(calc(&format!("\"{uuid}\".Uuid")), uuid);
    // Any of them is text, and goes where text is asked for.
    assert_eq!(calc(&format!("\"{address}\".Email.String")), address);
}

#[test]
fn text_that_does_not_read_as_one_says_so() {
    let errors = calc_err(None, "Email(\"nope\")");
    assert!(errors.contains("`nope` is not an Email"), "{errors}");
    let errors = calc_err(None, "Url(\"example.com\")");
    assert!(errors.contains("`example.com` is not a Url"), "{errors}");
    // One kind of text is not another, and a number is none of them.
    assert!(calc_err(None, "Uuid(\"https://example.com\")").contains("not a Uuid"));
    let errors = calc_err(None, "Email(42)");
    assert!(errors.contains("cannot be applied"), "{errors}");
}

/// The declared route: a cell that does not read as the column's kind of
/// text is a problem in the text, found before anything is calculated.
#[test]
fn a_column_declared_a_kind_of_text_takes_only_text_that_reads_as_one() {
    let file = |contact: &str| {
        format!("sheet Contacts\n\nContact : Email\n\n| Contact |\n| ------- |\n| {contact} |\n")
    };
    assert!(shown(&file("name@example.com")).contains("name@example.com"));
    for bad in ["nope", "42"] {
        let errors = lint_errors(&file(bad));
        assert!(
            errors
                .iter()
                .any(|e| e.contains(&format!("`{bad}` is not an `Email` literal"))),
            "{bad}: {errors:?}"
        );
    }
    // A `String` column still takes anything, as it always has.
    let text = "sheet Contacts\n\nContact : String\n\n| Contact |\n| ------- |\n| nope |\n";
    assert!(shown(text).contains("nope"));
}

/// The converting route, which is how a column of text that came in from a
/// file is cleaned: the cell that does not read says so and the rest of the
/// file is still calculated and still written out. The declared route above
/// stops the file instead, so these two are not the same thing.
#[test]
fn a_conversion_that_fails_marks_its_own_cell_and_leaves_the_file_calculating() {
    let file = "sheet Contacts\n\n| Written          | Checked |\n| ---------------- | ------- |\n| name@example.com | *       |\n| nope             | *       |\n\nChecked := Written.Email\n";
    let out = view_with("test.omx", file, exact());
    // The good row is calculated, the bad one is an error where it is.
    assert!(out.output.contains("name@example.com"), "{}", out.output);
    assert!(out.output.contains("#ERROR"), "{}", out.output);
    // And the failure is reported, without taking the file's output away.
    assert!(
        out.errors
            .iter()
            .any(|e| e.contains("`nope` is not an Email")),
        "{:?}",
        out.errors
    );
}

#[test]
fn a_fraction_of_a_second_is_written_and_read_back() {
    // A time keeps nine digits of a second, and shows the digits it holds
    // rather than the nine it could.
    assert_eq!(calc("09:30:00.5"), "09:30:00.5");
    assert_eq!(calc("09:30:00.123456789"), "09:30:00.123456789");
    assert_eq!(calc("00:00:00.000000001"), "00:00:00.000000001");
    // A date-time keeps six, that being the unit it is held in (D68).
    assert_eq!(calc("2025-01-31T09:30:00.5"), "2025-01-31T09:30:00.5");
    assert_eq!(
        calc("2025-01-31T09:30:00.123456"),
        "2025-01-31T09:30:00.123456"
    );
    // Without a fraction nothing changes: the seconds are shown only when
    // they are not zero, as before.
    assert_eq!(calc("09:30:00"), "09:30");
    assert_eq!(calc("2025-01-31T09:30:00"), "2025-01-31T09:30");
}

#[test]
fn a_second_past_what_the_unit_keeps_is_reported() {
    // A seventh digit on a date-time is said, not silently dropped.
    let errors = calc_err(None, "2025-01-31T09:30:00.0000001");
    assert!(errors.contains("a date-time holds 6 digits"), "{errors}");
    let errors = calc_err(None, "09:30:00.0000000001");
    assert!(errors.contains("a time holds 9 digits"), "{errors}");
}

#[test]
fn the_parts_of_a_second() {
    // Each is what is left after the second, in its own unit.
    assert_eq!(calc("09:30:00.5.millisecond()"), "500");
    assert_eq!(calc("09:30:00.5.microsecond()"), "500000");
    assert_eq!(calc("09:30:00.5.nanosecond()"), "500000000");
    assert_eq!(calc("09:30:00.123456789.nanosecond()"), "123456789");
    // The second itself is unchanged by the fraction beside it.
    assert_eq!(calc("09:30:15.5.second()"), "15");
    // A date-time answers too, to the microsecond it holds.
    assert_eq!(calc("2025-01-31T09:30:00.25.millisecond()"), "250");
    assert_eq!(calc("2025-01-31T09:30:00.25.microsecond()"), "250000");
    // Nothing after the second is zero, not empty.
    assert_eq!(calc("09:30:15.millisecond()"), "0");
}

const ROWS: &str = "sheet Sales\n\nMonth | Revenue\nJan | 100\nFeb | 120\nMar | 150\nApr | 90\n";

#[test]
fn a_sheet_may_be_calculated_from_another() {
    let file = format!("{ROWS}\nsheet Top := Sales |> head(3)\n");
    let out = shown(&file);
    // Its columns are the source's, and its rows are the ones kept.
    assert!(out.contains("sheet Top"), "{out}");
    let top = out.split("sheet Top").nth(1).unwrap();
    assert!(top.contains("Month | Revenue"), "{top}");
    assert!(top.contains("Jan"), "{top}");
    assert!(top.contains("Mar"), "{top}");
    assert!(!top.contains("Apr"), "{top}");
}

#[test]
fn a_calculated_sheet_takes_the_columns_its_expression_keeps() {
    let file = format!("{ROWS}\nsheet Big := Sales |> filter(Revenue > 100) |> select(Month)\n");
    let out = shown(&file);
    let big = out.split("sheet Big").nth(1).unwrap();
    // `select` says which columns, and the filter which rows: neither is
    // known from the text of this sheet, there being none.
    assert!(big.contains("Month"), "{big}");
    assert!(!big.contains("Revenue"), "{big}");
    assert!(big.contains("Feb") && big.contains("Mar"), "{big}");
    assert!(!big.contains("Jan"), "{big}");
}

#[test]
fn a_calculated_sheet_may_be_calculated_from_one() {
    let file = format!("{ROWS}\nsheet Top := Sales |> head(3)\n\nsheet One := Top |> head(1)\n");
    let out = shown(&file);
    let one = out.split("sheet One").nth(1).unwrap();
    assert!(one.contains("Jan"), "{one}");
    assert!(!one.contains("Feb"), "{one}");
}

#[test]
fn a_sheet_calculated_from_itself_is_reported() {
    let file = "sheet A := B |> head(1)\n\nsheet B := A |> head(1)\n";
    let out = view_with("test.omx", file, exact());
    assert!(
        out.errors.iter().any(|e| e.contains("itself")),
        "{:?}",
        out.errors
    );
}

#[test]
fn a_sheet_calculated_from_what_is_not_a_sheet_is_reported() {
    let file = format!("{ROWS}\nsheet Bad := Sales.Revenue\n");
    let errors = lint_errors(&file);
    assert!(
        errors.iter().any(|e| e.contains("a sheet needs a sheet")),
        "{errors:?}"
    );
}

#[test]
fn the_parts_of_a_year() {
    assert_eq!(calc("month_start(2025-02-10)"), "2025-02-01");
    assert_eq!(calc("month_end(2025-02-10)"), "2025-02-28");
    assert_eq!(calc("[2025-02-10, 2025-12-31].quarter()"), "[1, 4]");
    assert_eq!(calc("[2025-01-01, 2025-12-31].ordinal_day()"), "[1, 365]");
    // The ISO week belongs to the year its Thursday is in, so the last days
    // of 2025 are week 1 of 2026, and the first day of 2027 is week 53.
    assert_eq!(calc("2025-01-01.week()"), "1");
    assert_eq!(calc("2025-12-29.week()"), "1");
    assert_eq!(calc("2027-01-01.week()"), "53");
}

// ---- a columnar core: the two gates ---------------------------------------

/// A file whose columns are lowered or not for a reason each.
const GATES: &str = "\
const Rate = 20%

sheet Sales

| Month | Revenue | Cost | Profit | Tax | Share | Running | Big   |
| ----- | ------- | ---- | ------ | --- | ----- | ------- | ----- |
| Jan   | 100     | 60   | *      | *   | *     | *       | *     |
| Feb   | 120     | 70   | *      | *   | *     | *       | *     |
| Mar   | 150     | 80   | *      | *   | *     | *       | *     |

Profit  := Revenue - Cost
Tax     := Profit * Rate
Share   := Revenue / 7
Running := Profit + [Running; *-1] // 0
Big     := Revenue > Cost
";

/// A file calculated one way or the other, as an engine of its own: what
/// became of each column it could have lowered, and every cell of it as it
/// is shown.
fn calculated(file: &str, lowering: Lowering) -> (Vec<(String, Attempt)>, Vec<String>) {
    let (program, diags) = compile(file, 0);
    assert!(diags.is_empty(), "{diags:?}");
    let options = Options {
        lowering,
        exact: true,
        ..Options::default()
    };
    let engine = Engine::with_options(&program, options);
    engine.run();
    let attempts = engine
        .attempts()
        .into_iter()
        .map(|(t, c, a)| (program.sheets[t].cols[c].name.clone(), a))
        .collect();
    let mut shown = Vec::new();
    for (t, sheet) in program.sheets.iter().enumerate() {
        for c in 0..sheet.cols.len() {
            for r in 0..sheet.nrows {
                shown.push(engine.show(&engine.cell_shown(t, c, r), true));
            }
        }
    }
    (attempts, shown)
}

/// What became of one column, with lowering forced.
fn attempt(file: &str, name: &str) -> Option<Attempt> {
    calculated(file, Lowering::Always)
        .0
        .into_iter()
        .find(|(col, _)| col == name)
        .map(|(_, a)| a)
}

#[test]
fn an_elementwise_column_is_computed_in_one_step() {
    assert_eq!(attempt(GATES, "Profit"), Some(Attempt::Lowered));
    // And the values are the engine's, to the digit.
    assert_eq!(ask(GATES, "Sales.Profit"), "[40, 50, 70]");
}

#[test]
fn a_row_recursive_column_stays_on_the_engine() {
    // It reads the row before it, so the first gate never offers it.
    assert_eq!(attempt(GATES, "Running"), None);
    assert_eq!(ask(GATES, "Sales.Running"), "[0, 50, 120]");
}

#[test]
fn a_column_the_dataframe_has_no_answer_for() {
    // A quotient is scale 6 in Polars and every digit a decimal holds here.
    assert_eq!(attempt(GATES, "Share"), Some(Attempt::Refused));
    assert_eq!(
        ask(GATES, "Sales.Share"),
        "[14.2857142857142857142857142857, 17.1428571428571428571428571429, 21.4285714285714285714285714286]"
    );
}

#[test]
fn a_percentage_or_a_fraction_column_of_the_text_is_held_as_a_decimal() {
    // D69's dress is display, and `cell_of` puts it back on from the
    // column's own type, so such a column is held as the `Decimal(38, s)`
    // its values already are. `column_of` used to decline the pair, and
    // every aggregate over one was refused for want of a column to take.
    let file = "\
sheet T

Share : Percent
Part  : Fraction

| Item | Share | Part |
| ---- | ----- | ---- |
| a    | 20%   | 1/2  |
| b    | 10%   | 1/4  |
| c    | 30%   | 1/8  |
";
    for expr in [
        "T.Share.sum()",
        "T.Share.mean()",
        "T.Share.max()",
        "T.Part.sum()",
        "T.Part.mean()",
        "T.Part.min()",
    ] {
        let ran = both_ways(file, expr, None);
        assert_eq!(ran.aggregates, [Attempt::Lowered], "{expr}");
    }
}

#[test]
fn a_column_reading_a_percentage_column_is_lowered() {
    // The win is not the aggregate: a column of the text declared `Percent`
    // or `Fraction` is now a column any lowered expression can read, and
    // before this one it was a wall.
    let file = "\
sheet T

Rate : Percent

| Base | Rate | Fee |
| ---- | ---- | --- |
| 100  | 20%  | *   |
| 200  | 10%  | *   |

Fee := Base * Rate
";
    assert_eq!(attempt(file, "Fee"), Some(Attempt::Lowered));
    assert_eq!(ask(file, "T.Fee"), "[20, 20]");
}

#[test]
fn a_percentage_in_a_formula_is_not_lowered() {
    // `Rate` is `20%`, and what omasheet makes of a percentage met in the
    // middle of a sum is its own rule rather than arithmetic (D69). The
    // dress a column wears is another matter: it is put back on as a value
    // is read out of the dataframe, exactly as the row path puts it on.
    assert_eq!(attempt(GATES, "Tax"), Some(Attempt::Refused));
}

#[test]
fn a_comparison_is_lowered() {
    assert_eq!(attempt(GATES, "Big"), Some(Attempt::Lowered));
    assert_eq!(ask(GATES, "Sales.Big"), "[true, true, true]");
}

#[test]
fn a_file_calculates_the_same_either_way() {
    let rows = calculated(GATES, Lowering::Never).1;
    let columns = calculated(GATES, Lowering::Always).1;
    assert_eq!(rows, columns);
}

/// Every `examples/*.omx`, in the order their names sort in.
fn corpus() -> Vec<std::path::PathBuf> {
    let dir = std::path::Path::new("../examples");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .expect("the example corpus is beside the crates")
        .filter_map(|entry| {
            let path = entry.expect("a readable directory entry").path();
            (path.extension()? == "omx").then_some(path)
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no examples found");
    files
}

/// How a file in the corpus is viewed: beside itself, so a `table` finds
/// its source, and otherwise as the `print` command views it.
fn as_printed(lowering: Lowering) -> Options {
    Options {
        base: Some(std::path::PathBuf::from("../examples")),
        lowering,
        ..Options::default()
    }
}

#[test]
fn every_example_calculates_the_same_either_way() {
    for path in corpus() {
        let text = std::fs::read_to_string(&path).expect("a readable example");
        let name = path.file_name().expect("a file name").to_string_lossy();
        let never = view_with(&name, &text, as_printed(Lowering::Never));
        let always = view_with(&name, &text, as_printed(Lowering::Always));
        assert_eq!(never.output, always.output, "{name}");
        assert_eq!(never.errors, always.errors, "{name}");
    }
}

/// The committed `.expected` is the third party to that agreement. The
/// CLI's own golden test reads it under `Auto`, which for a sheet of ten
/// rows is the row path, so without this the lowered answer is only ever
/// compared with the other calculation and never with what is written
/// down.
#[test]
fn every_example_matches_its_expected_output_either_way() {
    for path in corpus() {
        let text = std::fs::read_to_string(&path).expect("a readable example");
        let name = path.file_name().expect("a file name").to_string_lossy();
        let expected = std::fs::read_to_string(path.with_extension("expected"))
            .expect("an example has its expected output beside it");
        for lowering in [Lowering::Never, Lowering::Auto, Lowering::Always] {
            let out = view_with(&name, &text, as_printed(lowering));
            assert!(
                out.errors.is_empty(),
                "{name} under {lowering:?}: {:?}",
                out.errors
            );
            assert_eq!(out.output, expected, "{name} under {lowering:?}");
        }
    }
}

/// How much of the corpus lowers: what became of every column the twelve
/// examples offer to Polars, with lowering forced.
///
/// Measured by hand three times before this, and the third measurement
/// corrected the second — a `grep` had been taking the first column of a
/// name from whichever example came first. The figures are pinned here so
/// that a change in what lowers says so, and printed under `--nocapture`
/// so the next measurement is read rather than taken.
#[test]
fn how_much_of_the_corpus_lowers() {
    let mut table = String::from("| Example | Offered | Lowered | Refused | Failed |\n");
    table.push_str("| --- | ---: | ---: | ---: | ---: |\n");
    let mut left = String::new();
    let (mut lowered, mut refused, mut failed) = (0, 0, 0);
    for path in corpus() {
        let text = std::fs::read_to_string(&path).expect("a readable example");
        let name = path.file_name().expect("a file name").to_string_lossy();
        let (program, mut diags) = compile(&text, 0);
        assert!(diags.is_empty(), "{name}: {diags:?}");
        let mut program = program;
        let dir = std::path::Path::new("../examples");
        let tables = omasheet_engine::table::read(&mut program, dir, &mut diags);
        assert!(diags.is_empty(), "{name}: {diags:?}");
        let program = program;
        let engine = Engine::with_tables(&program, as_printed(Lowering::Always), tables);
        engine.run();
        let attempts = engine.attempts();
        let count = |want| attempts.iter().filter(|(.., a)| *a == want).count();
        let (l, r, f) = (
            count(Attempt::Lowered),
            count(Attempt::Refused),
            count(Attempt::Failed),
        );
        table.push_str(&format!(
            "| {name} | {} | {l} | {r} | {f} |\n",
            attempts.len()
        ));
        // The name and the formula of each one left, which is what the
        // reasons in the task are read off: a grep for the name alone is
        // what got the measurement before this one wrong.
        for (t, c, a) in &attempts {
            if *a != Attempt::Lowered {
                let sheet = &program.sheets[*t];
                let column = &sheet.cols[*c];
                let formula = match &column.kind {
                    ColKind::Computed(node) => {
                        &text[node.span.start as usize..node.span.end as usize]
                    }
                    _ => "",
                };
                left.push_str(&format!(
                    "{a:?} {name} {}.{} := {formula}\n",
                    sheet.name, column.name
                ));
            }
        }
        (lowered, refused, failed) = (lowered + l, refused + r, failed + f);
    }
    let offered = lowered + refused + failed;
    table.push_str(&format!(
        "| **all of them** | **{offered}** | **{lowered}** | **{refused}** | **{failed}** |\n"
    ));
    println!("{table}\n{left}");
    // What task 8.3 records, and what the table there counts the reasons
    // for. A figure that moves is a change in coverage, which is worth
    // saying out loud rather than discovering at the next measurement:
    // read the two lists above against the table and write down what
    // moved.
    assert_eq!(
        (offered, lowered, refused, failed),
        (60, 17, 42, 1),
        "the corpus lowers differently than task 8.3 records"
    );
}

// ---- a columnar core: aggregates ------------------------------------------

/// What one expression came to: what it answers, and what became of every
/// aggregate it took and every condition it picked rows with.
struct Ran {
    shown: String,
    aggregates: Vec<Attempt>,
    filters: Vec<Attempt>,
}

/// One expression asked of a file, with lowering as given. `dir` is where
/// the file's sources are read from, for a `table`.
fn asked(file: &str, expr: &str, lowering: Lowering, dir: Option<&str>) -> Ran {
    let (mut program, mut diags) = compile(file, 0);
    assert!(diags.is_empty(), "{diags:?}");
    let base = dir.map(std::path::PathBuf::from);
    let tables = match &base {
        Some(dir) => omasheet_engine::table::read(&mut program, dir, &mut diags),
        None => Default::default(),
    };
    assert!(diags.is_empty(), "{diags:?}");
    let options = Options {
        base,
        lowering,
        exact: true,
        ..Options::default()
    };
    // A sheet calculated from an expression has its rows counted first,
    // as a file opened anywhere else does.
    let tables = omasheet_engine::resolve_views(&mut program, &options, tables);
    let program = program;
    let engine = Engine::with_tables(&program, options, tables);
    engine.run();
    let (node, _) = compile_expr(&program, expr, 1).expect("the expression checks");
    let shown = match engine.eval_top(&node) {
        Some(v) => engine.show(&v, true),
        None => "<error>".to_string(),
    };
    Ran {
        shown,
        aggregates: engine.aggregates().into_iter().map(|a| a.2).collect(),
        filters: engine.filters().into_iter().map(|f| f.1).collect(),
    }
}

/// The same expression both ways: it must answer the same, and what comes
/// back is what became of it when lowering was forced.
fn both_ways(file: &str, expr: &str, dir: Option<&str>) -> Ran {
    let rows = asked(file, expr, Lowering::Never, dir);
    let columns = asked(file, expr, Lowering::Always, dir);
    assert_eq!(
        rows.shown, columns.shown,
        "`{expr}` differs between the two paths"
    );
    columns
}

/// The titanic table of `examples/analysis.omx`, whose 887 rows are in a
/// Parquet file.
const TABLE: &str = "\
table Passengers := dataframe[\"titanic.parquet\"]

Survived : Int
Pclass   : Int
Name     : String
Sex      : String
Age      : Float
Fare     : Float
Siblings : Int from \"Siblings/Spouses Aboard\"
Parents  : Int from \"Parents/Children Aboard\"
";

#[test]
fn an_aggregate_over_a_frame_column_is_taken_by_polars() {
    let dir = Some("../examples");
    for (expr, answer) in [
        ("Passengers.Survived.sum()", "342"),
        ("Passengers.Survived.count()", "887"),
        ("Passengers.Survived.len()", "887"),
        ("Passengers.Age.max()", "80"),
        ("Passengers.Age.min()", "0.42"),
    ] {
        let ran = both_ways(TABLE, expr, dir);
        assert_eq!(ran.shown, answer, "{expr}");
        assert_eq!(ran.aggregates, [Attempt::Lowered], "{expr}");
    }
}

#[test]
fn a_mean_of_decimals_is_exact() {
    // D72: a Polars `mean` over a decimal column is an `f64` where the
    // engine answers exactly, so the `mean` itself is never lowered — but
    // the `sum` and the `count` it is made of both are, and the division
    // between them is the engine's own. `both_ways` is what says the two
    // paths agree to the digit.
    let dir = Some("../examples");
    let survived = both_ways(TABLE, "Passengers.Survived.mean()", dir);
    assert_eq!(survived.aggregates, [Attempt::Lowered]);
    // Over a floating-point column it stays on the engine, the sum half of
    // it being the one addition order matters to.
    let fare = both_ways(TABLE, "Passengers.Fare.mean()", dir);
    assert_eq!(fare.aggregates, [Attempt::Refused]);
}

#[test]
fn a_mean_of_a_column_of_nothing_is_nothing() {
    // The row path divides by the values that are not empty, so a column
    // with none has no mean. Polars sums such a column to zero rather than
    // to null, so it is the count that has to say so.
    let file = "\
sheet T

N : Decimal

| X | N |
| - | - |
| 1 |   |
| 2 |   |
";
    let empty = both_ways(file, "T.N.mean()", None);
    assert_eq!(empty.shown, "");
    assert_eq!(empty.aggregates, [Attempt::Lowered]);
}

#[test]
fn the_dress_a_column_wears_survives_a_lowered_mean() {
    // A division takes a percentage off, here as on the row path, so the
    // mean of a `Percent` column has it put back on afterwards. A fraction
    // needs no such thing: anything done with one gives one.
    let dir = Some("../examples");
    let percent = TABLE.replace("Survived : Int", "Survived : Percent");
    let ran = both_ways(&percent, "Passengers.Survived.mean()", dir);
    assert_eq!(ran.aggregates, [Attempt::Lowered]);

    let fraction = TABLE.replace("Survived : Int", "Survived : Fraction");
    let ran = both_ways(&fraction, "Passengers.Survived.mean()", dir);
    assert_eq!(ran.aggregates, [Attempt::Lowered]);
}

#[test]
fn an_extreme_of_a_date_or_a_time_is_taken_by_polars() {
    // An extreme of a temporal column is the extreme of the whole number it
    // is held as, which is the order the engine compares two of them in, so
    // there is nothing in the way of lowering it.
    let file = "\
sheet Stamps

Day   : Date
Clock : Time
When  : Datetime

| Day        | Clock    | When                |
| ---------- | -------- | ------------------- |
| 2024-03-01 | 09:30:00 | 2024-03-01T09:30:00 |
| 2023-12-25 | 23:59:00 | 2023-12-25T23:59:00 |
| 2025-01-15 | 00:01:00 | 2025-01-15T00:01:00 |
";
    for (expr, answer) in [
        ("Stamps.Day.max()", "2025-01-15"),
        ("Stamps.Day.min()", "2023-12-25"),
        ("Stamps.Clock.max()", "23:59"),
        ("Stamps.Clock.min()", "00:01"),
        ("Stamps.When.max()", "2025-01-15T00:01"),
        ("Stamps.When.min()", "2023-12-25T23:59"),
    ] {
        let ran = both_ways(file, expr, None);
        assert_eq!(ran.shown, answer, "{expr}");
        assert_eq!(ran.aggregates, [Attempt::Lowered], "{expr}");
    }
}

#[test]
fn polars_cannot_divide_exact_numbers_the_way_the_engine_does() {
    // Why D72 refuses an exact quotient, measured rather than asserted —
    // and pinned here because the version of this reason that was written
    // from the spike ("scale 6") was already out of date.
    //
    // Polars gives a quotient `max(x, y) + 4` digits after the point, from
    // an intermediate of `2 max(x, y) + 4` that has to fit in 38. So the
    // widest quotient it can give is 21 digits, at an input scale of 17 —
    // where the division answers *null*, not a number — and at 18 it is an
    // error. The engine needs 30: `Dec` ignores what is past the 30th digit
    // when it compares, and a third rescaled to 29 is no longer a third.
    // 21 < 30, so there is no pair of input scales that would do, and this
    // is a wall rather than a conservative choice.
    use omasheet_engine::dtype::decimal;
    use omasheet_omx::decimal::Dec;
    use polars::prelude::*;

    let third = |x: u32, y: u32| -> Result<DataType, String> {
        let one: Int128Chunked = [Some(10i128.pow(x))].into_iter().collect_ca("a".into());
        let three: Int128Chunked = [Some(3 * 10i128.pow(y))].into_iter().collect_ca("b".into());
        let frame = DataFrame::new(vec![
            one.into_decimal_unchecked(Some(38), x as usize)
                .into_series()
                .into_column(),
            three
                .into_decimal_unchecked(Some(38), y as usize)
                .into_series()
                .with_name("b".into())
                .into_column(),
        ])
        .expect("two columns of one row");
        let out = frame
            .lazy()
            .select([(col("a") / col("b")).alias("q")])
            .collect()
            .map_err(|e| e.to_string())?;
        let column = out.column("q").expect("the quotient");
        if matches!(column.get(0), Ok(AnyValue::Null)) {
            return Err("null".to_string());
        }
        Ok(column.dtype().clone())
    };
    for (x, y, scale) in [
        (0u32, 0u32, 4u32),
        (2, 2, 6),
        (2, 0, 6),
        (10, 2, 14),
        (14, 0, 18),
    ] {
        assert_eq!(
            third(x, y),
            Ok(DataType::Decimal(None, Some(scale as usize))),
            "Decimal(38, {x}) / Decimal(38, {y})"
        );
    }
    // The edge, and past it.
    assert_eq!(third(17, 0), Err("null".to_string()));
    assert!(third(18, 0).is_err());

    // And the 30 the engine needs, which is `SHOWN_PRECISION`: a third is
    // still a third rescaled to 30 and is not at 29.
    let third_exact = Dec::from_i64(1)
        .checked_div(Dec::from_i64(3))
        .expect("a third");
    assert_eq!(third_exact.rescale(30).expect("it fits"), third_exact);
    assert_ne!(third_exact.rescale(29).expect("it fits"), third_exact);
    // Narrowing is no way round it either: the guard digits have to be
    // there while the arithmetic happens, not merely when it is compared.
    // `2/7 + 3/7 == 5/7` is the property, and it fails if the two are
    // rescaled to 30 first.
    let seventh = |n: i64| {
        Dec::from_i64(n)
            .checked_div(Dec::from_i64(7))
            .expect("a seventh")
    };
    assert_eq!(
        seventh(2).checked_add(seventh(3)).expect("a sum"),
        seventh(5)
    );
    let narrowed = |n: i64| seventh(n).rescale(30).expect("it fits");
    assert_ne!(
        narrowed(2).checked_add(narrowed(3)).expect("a sum"),
        narrowed(5)
    );
    // `decimal` is what the lowering would have asked for, had there been
    // a scale to ask for.
    assert_eq!(decimal(30), DataType::Decimal(Some(38), Some(30)));
}

#[test]
fn a_sum_with_no_room_for_its_total_stays_on_the_engine() {
    // Polars adds decimals in an `i128` and wraps on overflow rather than
    // saying so, so the plan bounds the total before trusting it: `|sum|` is
    // at most the rows times the wider extreme, and four quotients held at
    // scale 38 are already past what 38 digits hold. The engine's fold has
    // an answer for them, and the bound is what hands it back. It is a bound
    // and not the sum, so it refuses a shade early — these four do add up to
    // less than one.
    let sheet = |rows: usize| {
        let mut text = String::from("sheet T\n\nShare : Decimal\n\n| N | Share |\n| - | ----- |\n");
        for r in 0..rows {
            text.push_str(&format!("| {} | * |\n", r + 1));
        }
        text.push_str("\nShare := 1 / (N + 2)\n");
        text
    };
    assert_eq!(
        both_ways(&sheet(2), "T.Share.sum()", None).aggregates,
        [Attempt::Lowered]
    );
    assert_eq!(
        both_ways(&sheet(4), "T.Share.sum()", None).aggregates,
        [Attempt::Refused]
    );
    // A `mean` is that same sum, so it is handed back with it.
    assert_eq!(
        both_ways(&sheet(4), "T.Share.mean()", None).aggregates,
        [Attempt::Refused]
    );
}

#[test]
fn a_sum_of_floats_stays_on_the_engine() {
    // Addition of floating-point numbers is not associative, so a sum
    // collected in parallel and a sum folded from the left are not the same
    // number. `max` and `min` do not care in which order they look.
    let dir = Some("../examples");
    assert_eq!(
        both_ways(TABLE, "Passengers.Fare.sum()", dir).aggregates,
        [Attempt::Refused]
    );
    assert_eq!(
        both_ways(TABLE, "Passengers.Fare.max()", dir).aggregates,
        [Attempt::Lowered]
    );
}

#[test]
fn the_dress_a_column_wears_survives_a_lowered_aggregate() {
    // D69: a Polars aggregate answers a bare number, and the wrapper is put
    // back on from the column's own type. `Survived` holds 0 and 1, and the
    // file's dtype is the same either way.
    let dir = Some("../examples");
    let percent = TABLE.replace("Survived : Int", "Survived : Percent");
    let ran = both_ways(&percent, "Passengers.Survived.sum()", dir);
    assert_eq!(ran.shown, "34200%");
    assert_eq!(ran.aggregates, [Attempt::Lowered]);

    let fraction = TABLE.replace("Survived : Int", "Survived : Fraction");
    let ran = both_ways(&fraction, "Passengers.Survived.sum()", dir);
    assert_eq!(ran.shown, "342");
    assert_eq!(ran.aggregates, [Attempt::Lowered]);
}

#[test]
fn the_least_of_a_column_of_nothing_but_nans_is_the_nan() {
    // D71a, the one case where Polars and omasheet disagree: its `min` over
    // a floating-point column holding no number is null, and omasheet
    // answers the `NaN`. So `min` is handed back and `max`, which is
    // Polars' `nan_max`, is not.
    let file = "\
sheet T

N : Float

| X | N |
| - | - |
| 1 | * |
| 2 | * |
| 3 | * |

N := (0/0).Float
";
    let least = both_ways(file, "T.N.min()", None);
    assert_eq!(least.shown, "NaN");
    assert_eq!(least.aggregates, [Attempt::Refused]);
    let most = both_ways(file, "T.N.max()", None);
    assert_eq!(most.shown, "NaN");
    assert_eq!(most.aggregates, [Attempt::Lowered]);
}

#[test]
fn an_aggregate_over_a_selection_stays_on_the_engine() {
    // The argument has to be a whole column: a selection of rows is section
    // 6's remaining work, not this task's.
    let dir = Some("../examples");
    let oldest = both_ways(TABLE, "Passengers[; Survived == 1].Age.max()", dir);
    assert_eq!(oldest.shown, "80");
    assert!(oldest.aggregates.is_empty());
    // And the selection is real: 342 of the 887 rows survived.
    let survivors = both_ways(TABLE, "Passengers[; Survived == 1].Age.len()", dir);
    assert_eq!(survivors.shown, "342");
    assert!(survivors.aggregates.is_empty());
}

// ---- a columnar core: selections ------------------------------------------

#[test]
fn a_condition_picks_rows_in_one_pass() {
    let dir = Some("../examples");
    for (expr, answer) in [
        ("Passengers[; Survived == 1].Age.len()", "342"),
        ("Passengers[; Pclass == 1].Name.len()", "216"),
        ("Passengers[; Age > 60].Age.len()", "26"),
        ("Passengers[; Sex == \"female\"].Age.len()", "314"),
    ] {
        let ran = both_ways(TABLE, expr, dir);
        assert_eq!(ran.shown, answer, "{expr}");
        assert_eq!(ran.filters, [Attempt::Lowered], "{expr}");
    }
}

#[test]
fn a_sort_is_lowered_and_lands_the_rows_where_the_engine_does() {
    let dir = Some("../examples");
    // The dearest fare aboard, reached by a sort rather than by `max`.
    let ran = both_ways(TABLE, "(Passengers |> sort(Fare, desc)).Fare[0]", dir);
    assert_eq!(ran.shown, "512.3292");
    assert_eq!(ran.filters, [Attempt::Lowered]);
    // Three passengers tie at that fare and both paths keep the order they
    // came in, which is what `maintain_order` is for.
    let ties = both_ways(
        TABLE,
        "(Passengers |> sort(Fare, desc) |> head(3)).Name",
        dir,
    );
    assert_eq!(
        ties.shown,
        "[\"Miss. Anna Ward\", \"Mr. Thomas Drake Martinez Cardeza\", \"Mr. Gustave J Lesurer\"]"
    );
    assert_eq!(ties.filters, [Attempt::Lowered]);
    // More than one key, and the sheet's own order where they both tie.
    let two = both_ways(TABLE, "(Passengers |> sort(Pclass, Fare)).Name[0]", dir);
    assert_eq!(two.shown, "\"Mr. William Harrison\"");
    assert_eq!(two.filters, [Attempt::Lowered]);
}

/// D95 on both paths: Polars places a null first only while the order is
/// ascending, so the lowered sort asks for `nulls_last` where the engine's
/// reversed comparison puts an empty cell last.
#[test]
fn an_empty_cell_sorts_first_on_both_paths() {
    let up = both_ways(TIES, "(Sales |> sort(Revenue)).Month", None);
    assert_eq!(up.shown, "[\"Apr\", \"Feb\", \"Jan\", \"Mar\"]");
    assert_eq!(up.filters, [Attempt::Lowered]);
    let down = both_ways(TIES, "(Sales |> sort(Revenue, desc)).Month", None);
    assert_eq!(down.shown, "[\"Jan\", \"Mar\", \"Feb\", \"Apr\"]");
    assert_eq!(down.filters, [Attempt::Lowered]);
}

#[test]
fn the_rows_a_condition_keeps_stay_in_their_order() {
    // A selection answers positions, so what follows it — `head`, a column
    // read, another selection — sees the sheet's own order either way.
    let dir = Some("../examples");
    let ran = both_ways(
        TABLE,
        "Passengers |> filter(Survived == 1) |> head(3) |> select(Name) |> len()",
        dir,
    );
    assert_eq!(ran.shown, "3");
    assert_eq!(ran.filters, [Attempt::Lowered]);
    let first = both_ways(TABLE, "Passengers[Name; Survived == 1][0]", dir);
    assert_eq!(
        first.shown,
        "\"Mrs. John Bradley (Florence Briggs Thayer) Cumings\""
    );
}

/// Every column of grouped sheet `sheet` both ways, as `both_ways` asks
/// of one expression, its cells joined a column a line; and what became
/// of the grouping and its aggregates when lowering was forced.
fn grouped_both_ways(file: &str, sheet: &str, cols: &[&str], dir: Option<&str>) -> Ran {
    let mut shown = Vec::new();
    let mut first = None;
    for c in cols {
        let ran = both_ways(file, &format!("{sheet}.{c}"), dir);
        shown.push(format!("{c}: {}", ran.shown));
        first.get_or_insert(ran);
    }
    let first = first.expect("a column");
    Ran {
        shown: shown.join("\n"),
        ..first
    }
}

#[test]
fn the_titanic_groups_alike_on_both_paths() {
    // The groups and every aggregate of them, the same to the digit, and
    // which of them Polars took: the grouping, and each aggregate but the
    // mean of a floating-point column, whose sum D72 keeps on the engine.
    let file = format!(
        "{TABLE}\nsheet G := Passengers |> group_by(Pclass, Sex) |> agg(Survived.sum(), Survived.mean(), Age.max(), Age.min(), Name.len(), Age.count(), Fare.mean(), Age.median())\n"
    );
    let cols = [
        "Pclass",
        "Sex",
        "Survived_sum",
        "Survived_mean",
        "Age_max",
        "Age_min",
        "Name_len",
        "Age_count",
        "Fare_mean",
        "Age_median",
    ];
    let ran = grouped_both_ways(&file, "G", &cols, Some("../examples"));
    assert!(
        ran.shown.contains("Survived_sum: [91, 45, 70, 17, 72, 47]"),
        "{}",
        ran.shown
    );
    assert_eq!(ran.filters, [Attempt::Lowered]);
    assert_eq!(
        ran.aggregates,
        [
            Attempt::Lowered,
            Attempt::Lowered,
            Attempt::Lowered,
            Attempt::Lowered,
            Attempt::Lowered,
            Attempt::Lowered,
        ]
    );
    // A computed key, the stages over the grouped sheet by name, and an
    // aggregate of an aggregate.
    let file = format!(
        "{TABLE}\nFamily := Siblings + Parents\n\nsheet F := Passengers |> group_by(Family) |> agg(Survived.sum())\n\nsheet Top := F |> sort(Survived_sum, desc) |> head(3)\n"
    );
    let ran = grouped_both_ways(
        &file,
        "Top",
        &["Family", "Survived_sum"],
        Some("../examples"),
    );
    assert!(ran.shown.starts_with("Family: [0, 1, 2]"), "{}", ran.shown);
    both_ways(&file, "F.Survived_sum.sum()", Some("../examples"));
}

/// Keys of every kind that a group could be made or broken by: an empty
/// one, a `NaN`, a zero of either sign, decimals and dates.
const KEYS: &str = "\
sheet K

Day  : Date
Real : Float

| Name | Text | Whole | Real | Exact | Day        | N |
| ---- | ---- | ----- | ---- | ----- | ---------- | - |
| a    | x    | 2     | 1.5  | 0.10  | 2025-01-02 | 1 |
| b    |      | 1     | NaN  | 0.1   | 2025-01-01 | 2 |
| c    | y    |       | 0.0  | 2.25  |            | 3 |
| d    | x    | 2     | -0.0 |       | 2025-01-02 | 4 |
| e    |      | 1     | NaN  | 2.250 | 2025-01-01 | 5 |
| f    | y    |       |      | 0.1   |            | 6 |
";

#[test]
fn every_kind_of_key_groups_alike_on_both_paths() {
    for key in [
        "Text",
        "Whole",
        "Real",
        "Exact",
        "Day",
        "Text, Whole",
        "Day, Real",
    ] {
        let file = format!(
            "{KEYS}\nsheet G := K |> group_by({key}) |> agg(N.sum(), N.len(), N.min(), N.max(), N.mean())\n"
        );
        let mut cols: Vec<&str> = key.split(", ").collect();
        cols.extend(["N_sum", "N_len", "N_min", "N_max", "N_mean"]);
        let ran = grouped_both_ways(&file, "G", &cols, None);
        assert_eq!(ran.filters, [Attempt::Lowered], "{key}");
        assert!(
            ran.aggregates.iter().all(|a| *a == Attempt::Lowered),
            "{key}: {:?}",
            ran.aggregates
        );
    }
}

#[test]
fn a_join_pairs_alike_on_both_paths() {
    for kind in ["", ", left"] {
        let file = format!("{TARGETS}\nsheet Both := Sales |> join(Targets, Month{kind})\n");
        let cols = ["Month", "Revenue", "Revenue_right", "Owner"];
        let ran = grouped_both_ways(&file, "Both", &cols, None);
        assert_eq!(ran.filters, [Attempt::Lowered], "{kind}");
    }
    // Keys of every kind, each joined to itself, which is where a `NaN`, a
    // zero of either sign and decimals of different scales meet their
    // like; and two keys at once.
    for key in ["Text", "Whole", "Real", "Exact", "Day", "Text, Whole"] {
        let file = format!(
            "{KEYS}\nsheet J := K |> select({key}, N) |> join(K |> select({key}, Name), {key}, left)\n"
        );
        let mut cols: Vec<&str> = key.split(", ").collect();
        cols.extend(["N", "Name"]);
        let ran = grouped_both_ways(&file, "J", &cols, None);
        assert_eq!(ran.filters, [Attempt::Lowered], "{key}");
    }
    // The titanic, each passenger with the name of their class.
    let file = format!(
        "{TABLE}\nsheet Classes\n\n| Pclass | Class  |\n| ------ | ------ |\n| 1      | First  |\n| 2      | Second |\n| 3      | Third  |\n\nsheet Named := Passengers |> select(Name, Pclass) |> join(Classes, Pclass)\n"
    );
    let ran = grouped_both_ways(&file, "Named", &["Name", "Class"], Some("../examples"));
    assert_eq!(ran.filters, [Attempt::Lowered]);
}

/// A sheet of every pairing of `true`, `false` and an empty cell.
const LOGIC: &str = "\
sheet T

A : Boolean
B : Boolean

| A     | B     |
| ----- | ----- |
| false |       |
|       | false |
| true  |       |
|       | true  |
|       |       |
";

#[test]
fn an_empty_cell_in_a_condition_is_an_unknown() {
    // Kleene's rule, which is Polars' and SQL's: an unknown that cannot
    // change the answer does not decide it, so `false and` is false and
    // `true or` is true whichever side the empty cell falls on. It used to
    // be the left side only, by the short-circuit in `eval`, which made
    // `and` read differently each way round.
    assert_eq!(ask(LOGIC, "T[A; 0] and T[B; 0]"), "false");
    assert_eq!(ask(LOGIC, "T[A; 1] and T[B; 1]"), "false");
    assert_eq!(ask(LOGIC, "T[A; 2] or T[B; 2]"), "true");
    assert_eq!(ask(LOGIC, "T[A; 3] or T[B; 3]"), "true");
    // An unknown that could still change it stays unknown, and `not` of one
    // was always this.
    assert_eq!(ask(LOGIC, "T[A; 2] and T[B; 2]"), "empty");
    assert_eq!(ask(LOGIC, "T[A; 0] or T[B; 0]"), "empty");
    assert_eq!(ask(LOGIC, "T[A; 4] and T[B; 4]"), "empty");
    assert_eq!(ask(LOGIC, "not T[A; 4]"), "empty");
}

#[test]
fn the_two_paths_agree_on_an_unknown() {
    // The whole reason for the change: a column of `and` is now one Polars
    // expression, and it answers what the row loop answers.
    let file = format!("{LOGIC}\nBoth := A and B\nEither := A or B\n")
        .replace("| A     | B     |", "| A     | B     | Both | Either |")
        .replace("| ----- | ----- |", "| ----- | ----- | ---- | ------ |")
        .replace("| false |       |", "| false |       | *    | *      |")
        .replace("|       | false |", "|       | false | *    | *      |")
        .replace("| true  |       |", "| true  |       | *    | *      |")
        .replace("|       | true  |", "|       | true  | *    | *      |")
        .replace("|       |       |", "|       |       | *    | *      |");
    assert_eq!(attempt(&file, "Both"), Some(Attempt::Lowered));
    assert_eq!(attempt(&file, "Either"), Some(Attempt::Lowered));
    // `both_ways` is the check: it asks each column both ways and fails if
    // the two do not show the same thing.
    for expr in ["T.Both", "T.Either"] {
        both_ways(&file, expr, None);
    }
}

/// Dates chosen for the parts that are easy to get wrong: an ISO week that
/// belongs to the year before, one that belongs to the year after, a leap
/// day, and a date before the epoch, whose day number is negative.
const PARTS: &str = "\
sheet Stamps

Day   : Date
Clock : Time
When  : Datetime

| Day        | Clock    | When                |
| ---------- | -------- | ------------------- |
| 2021-01-01 | 00:00:00 | 2021-01-01T00:00:00 |
| 2020-02-29 | 23:59:59 | 2020-02-29T23:59:59 |
| 2024-12-31 | 12:34:56 | 2024-12-31T12:34:56 |
| 1969-07-20 | 20:17:40 | 1969-07-20T20:17:40 |
| 2025-03-09 | 06:30:00 | 2025-03-09T06:30:00 |
";

#[test]
fn a_part_of_a_date_or_a_time_is_lowered() {
    // Polars counts the days of the week from Monday as `date::weekday`
    // does, and its week is the ISO week the engine works out from each
    // Thursday. The dates above are what says so.
    let parts = [
        ("Y", "year", "Day"),
        ("M", "month", "Day"),
        ("D", "day", "Day"),
        ("W", "weekday", "Day"),
        ("Q", "quarter", "Day"),
        ("Wk", "week", "Day"),
        ("O", "ordinal_day", "Day"),
        ("H", "hour", "Clock"),
        ("Mi", "minute", "Clock"),
        ("S", "second", "Clock"),
        ("ZY", "year", "When"),
        ("ZW", "week", "When"),
        ("ZH", "hour", "When"),
        ("ZS", "second", "When"),
        ("ZD", "date", "When"),
        ("ZT", "time", "When"),
    ];
    let mut file = PARTS.replace(
        "| Day        | Clock    | When                |",
        &format!(
            "| Day        | Clock    | When                |{}",
            parts
                .iter()
                .map(|(n, ..)| format!(" {n} |"))
                .collect::<String>()
        ),
    );
    file = file.replace(
        "| ---------- | -------- | ------------------- |",
        &format!(
            "| ---------- | -------- | ------------------- |{}",
            parts.iter().map(|_| " - |").collect::<String>()
        ),
    );
    let cells: String = parts.iter().map(|_| " * |").collect();
    for row in [
        "| 2021-01-01 | 00:00:00 | 2021-01-01T00:00:00 |",
        "| 2020-02-29 | 23:59:59 | 2020-02-29T23:59:59 |",
        "| 2024-12-31 | 12:34:56 | 2024-12-31T12:34:56 |",
        "| 1969-07-20 | 20:17:40 | 1969-07-20T20:17:40 |",
        "| 2025-03-09 | 06:30:00 | 2025-03-09T06:30:00 |",
    ] {
        file = file.replace(row, &format!("{row}{cells}"));
    }
    for (name, call, column) in parts {
        file.push_str(&format!("{name} := {column}.{call}()\n"));
    }
    for (name, call, column) in parts {
        assert_eq!(
            attempt(&file, name),
            Some(Attempt::Lowered),
            "{column}.{call}()"
        );
    }
    // And each one answers what the row loop answers.
    for (name, ..) in parts {
        both_ways(&file, &format!("Stamps.{name}"), None);
    }
    // A `Date` has no hour and a `Time` no month. The checker refuses the
    // pairing before lowering is asked — "`hour` needs a Time or a
    // Datetime, but this is Date" — so `lower::part` gates on the dtype
    // only because it has to be total on its own, not because anything
    // reaches it.
    // The ISO weeks the dates were picked for: 2021-01-01 is week 53 of the
    // year before and 2024-12-31 is week 1 of the year after.
    assert_eq!(ask(&file, "Stamps.Wk"), "[53, 9, 1, 29, 10]");
    assert_eq!(ask(&file, "Stamps.W"), "[5, 6, 2, 7, 7]");
    // The day and the time of a date-time, which before the epoch is a
    // division that has to floor rather than truncate.
    assert_eq!(
        ask(&file, "Stamps.ZD"),
        "[2021-01-01, 2020-02-29, 2024-12-31, 1969-07-20, 2025-03-09]"
    );
    assert_eq!(
        ask(&file, "Stamps.ZT"),
        "[00:00, 23:59:59, 12:34:56, 20:17:40, 06:30]"
    );
}

#[test]
fn asking_whether_a_cell_is_empty_is_lowered() {
    // The one call that asks about the absence rather than the value, so
    // every dtype answers it and the answer is never itself empty.
    let dir = Some("../examples");
    let ran = both_ways(TABLE, "Passengers[; is_null(Age)].Age.len()", dir);
    assert_eq!(ran.shown, "0");
    assert_eq!(ran.filters, [Attempt::Lowered]);
    let some = both_ways(TABLE, "Passengers[; is_not_null(Age)].Age.len()", dir);
    assert_eq!(some.shown, "887");
    assert_eq!(some.filters, [Attempt::Lowered]);
}

#[test]
fn asking_whether_a_cell_is_a_nan_is_lowered_over_floats_alone() {
    // A `NaN` is a thing only a floating-point column holds. Over any other
    // the engine answers `false` for every value it has and Polars has no
    // answer at all, so it is left alone.
    let file = "\
sheet T

F : Float
N : Int

| F     | N | Bad | Fine |
| ----- | - | --- | ---- |
| 1.5   | 1 | *   | *    |
| 0     | 2 | *   | *    |
|       | 3 | *   | *    |

Bad  := is_nan(N)
Fine := is_nan(F)
";
    assert_eq!(attempt(file, "Fine"), Some(Attempt::Lowered));
    assert_eq!(attempt(file, "Bad"), Some(Attempt::Refused));
    both_ways(file, "T.Fine", None);
    both_ways(file, "T.Bad", None);
}

#[test]
fn two_conditions_written_together_are_lowered() {
    // `and` and `or` are Kleene's on both paths now, so the commonest
    // condition of all is one Polars pass rather than a row walk.
    let dir = Some("../examples");
    for (expr, answer) in [
        (
            "Passengers[; Survived == 1 and Pclass == 1].Age.len()",
            "136",
        ),
        ("Passengers[; Pclass == 1 or Pclass == 2].Age.len()", "400"),
        (
            "Passengers[; Survived == 1 and Pclass == 1 and Sex == \"female\"].Age.len()",
            "91",
        ),
    ] {
        let ran = both_ways(TABLE, expr, dir);
        assert_eq!(ran.shown, answer, "{expr}");
        assert_eq!(ran.filters, [Attempt::Lowered], "{expr}");
    }
}

#[test]
fn a_condition_over_a_selection_already_made_stays_on_the_engine() {
    // The first condition is over the whole sheet and is lowered. The
    // second is over the rows the first kept, which would have to be
    // gathered into a frame of their own first, so it is never offered to
    // the gate at all — one `Lowered` and nothing beside it.
    let dir = Some("../examples");
    let ran = both_ways(
        TABLE,
        "Passengers[; Survived == 1][; Age > 60].Age.len()",
        dir,
    );
    assert_eq!(ran.shown, "5");
    assert_eq!(ran.filters, [Attempt::Lowered]);
}

#[test]
fn a_copy_made_for_a_plan_is_never_read_back_as_the_column() {
    // Text written bare in a cell is Markdown where a file is shown as a
    // document, and no Polars string carries that. So a copy of a column,
    // made only so that something else could be lowered over it, must not
    // become what the column's own cells are read from.
    let file = "\
sheet T

| N | Note       | Same |
| - | ---------- | ---- |
| 1 | **bold**   | *    |
| 2 | plain text | *    |

Same := Note == \"x\"
";
    for lowering in [Lowering::Never, Lowering::Always] {
        let (program, diags) = compile(file, 0);
        assert!(diags.is_empty(), "{diags:?}");
        let engine = Engine::with_options(
            &program,
            Options {
                lowering,
                ..Options::default()
            },
        );
        engine.run();
        // `Same` is lowered over `Note`, so `Note` is copied into a frame.
        if lowering == Lowering::Always {
            assert_eq!(engine.attempts(), [(0, 2, Attempt::Lowered)]);
        }
        let markdown: Vec<bool> = (0..2)
            .map(|r| engine.cell_shown(0, 1, r).is_markdown())
            .collect();
        assert_eq!(markdown, [true, true], "{lowering:?}");
    }
}

#[test]
fn unselected_columns_are_not_read() {
    // A `table`'s columns are a plan each, and nothing of the file is read
    // until a cell or a calculation asks for one.
    let dir = std::path::Path::new("../examples");
    let open = |expr: Option<&str>| {
        let (mut program, mut diags) = compile(TABLE, 0);
        let tables = omasheet_engine::table::read(&mut program, dir, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        let program = program;
        let engine = Engine::with_tables(
            &program,
            Options {
                base: Some(dir.to_path_buf()),
                lowering: Lowering::Always,
                ..Options::default()
            },
            tables,
        );
        engine.run();
        if let Some(expr) = expr {
            let (node, _) = compile_expr(&program, expr, 1).expect("the expression checks");
            engine.eval_top(&node).expect("a value");
        }
        engine.reads()
    };
    // Opened and calculated, with nothing asked of it: the schema and the
    // row count come from the footer, so no column is read at all.
    assert_eq!(open(None), [], "opened and calculated, nothing asked");
    // One column summed: that column, and none of the other seven.
    assert_eq!(open(Some("Passengers.Fare.sum()")), [(0, 5)]);
    // A condition over one column and a count of another: those two of
    // the eight. `Name` is read because a column selection answers with a
    // vector of its values, which `len` then counts — counting the rows
    // kept without reading them is work for another day.
    assert_eq!(
        open(Some("Passengers[; Pclass == 1].Name.len()")),
        [(0, 1), (0, 2)]
    );
}

// ---- the snapshot (D75) ---------------------------------------------------

/// Values as their `Debug`, which is the strictest comparison there is of
/// them: `Value` has no `PartialEq`, and what is shown would hide the very
/// differences worth catching — the Markdown flag on a string, a `Zoned`
/// where a `DateTime` was expected, an `Integer` where a `Decimal` was.
fn debugged(values: &[omasheet_engine::Value]) -> Vec<String> {
    values.iter().map(|v| format!("{v:?}")).collect()
}

/// Every column of every example, walked column-wise and read cell by cell,
/// has to come to the same values: the snapshot is the one place a wrong
/// value would be seen and the engine would not know it (D75, task 7.1).
#[test]
fn a_column_walked_at_once_holds_what_its_cells_hold() {
    let dir = std::path::Path::new("../examples");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .expect("the example corpus is beside the crates")
        .filter_map(|entry| {
            let path = entry.expect("a readable directory entry").path();
            (path.extension()? == "omx").then_some(path)
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no examples found");
    let mut taken = 0;
    for path in &files {
        let text = std::fs::read_to_string(path).expect("a readable example");
        let name = path.file_name().expect("a file name").to_string_lossy();
        for lowering in [Lowering::Never, Lowering::Always] {
            let (mut program, mut diags) = compile(&text, 0);
            if !diags.is_empty() {
                continue;
            }
            let tables = omasheet_engine::table::read(&mut program, dir, &mut diags);
            let program = program;
            let engine = Engine::with_tables(
                &program,
                Options {
                    base: Some(dir.to_path_buf()),
                    lowering,
                    ..Options::default()
                },
                tables,
            );
            engine.run();
            for (t, sheet) in program.sheets.iter().enumerate() {
                for c in 0..sheet.cols.len() {
                    // The column first, so that the walk is reading the
                    // dataframe rather than slots the cells have filled.
                    let Some(walked) = engine.column_shown(t, c) else {
                        continue;
                    };
                    taken += 1;
                    let asked: Vec<_> = (0..sheet.nrows)
                        .map(|r| engine.cell_shown(t, c, r))
                        .collect();
                    assert_eq!(
                        walked.len(),
                        sheet.nrows,
                        "{name}: {}.{} is not {} rows",
                        sheet.name,
                        sheet.cols[c].name,
                        sheet.nrows
                    );
                    assert_eq!(
                        debugged(&walked),
                        debugged(&asked),
                        "{name} {lowering:?}: {}.{}",
                        sheet.name,
                        sheet.cols[c].name
                    );
                }
            }
        }
    }
    // Lest the whole thing pass by checking nothing. Eight of the
    // corpus's columns come from a file, all of `analysis.omx`, and those
    // are taken at once whichever way the rest is calculated, which is 16
    // of the 33. The other seventeen are the columns lowering forced
    // calculates in one step, and 8.3 is where that number is read.
    assert_eq!(taken, 33, "the columns taken at once");
}

/// The other order: the cells first, then the column. A value already read
/// is the value the walk gives back, so a `table`'s column read one cell at
/// a time cannot then be shown as something else.
#[test]
fn a_column_walked_after_its_cells_gives_back_what_they_gave() {
    let (mut program, mut diags) = compile(TABLE, 0);
    assert!(diags.is_empty(), "{diags:?}");
    let dir = std::path::Path::new("../examples");
    let tables = omasheet_engine::table::read(&mut program, dir, &mut diags);
    assert!(diags.is_empty(), "{diags:?}");
    let program = program;
    let engine = Engine::with_tables(&program, Options::default(), tables);
    engine.run();
    let sheet = &program.sheets[0];
    for c in 0..sheet.cols.len() {
        let asked: Vec<_> = (0..sheet.nrows)
            .map(|r| engine.cell_shown(0, c, r))
            .collect();
        let walked = engine.column_shown(0, c).expect("a column of the file");
        assert_eq!(
            debugged(&walked),
            debugged(&asked),
            "{}",
            sheet.cols[c].name
        );
    }
}

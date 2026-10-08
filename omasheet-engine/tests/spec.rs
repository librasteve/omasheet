//! The scenarios of the Phase 1 specs (`openspec/changes/add-omasheet-core`),
//! one test per scenario where it can be checked from the outside.

use omasheet_engine::omx::date::{Order, Style};
use omasheet_engine::{Options, eval, eval_with, lint, view, view_with};

const SALES: &str = "\
table Sales

Month | Region | Revenue | Cost
Jan   | UK     | 100     | 60
Feb   | US     | 120     | 70
Mar   | UK     | 150     | 80
";

/// Evaluate an expression with no sheet.
fn calc(expr: &str) -> String {
    let out = eval(None, "<expression>", expr);
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

/// Evaluate an expression against a sheet.
fn ask(sheet: &str, expr: &str) -> String {
    let out = eval(Some(("test.omx", sheet)), "<expression>", expr);
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

/// The first error from evaluating an expression.
fn calc_err(sheet: Option<&str>, expr: &str) -> String {
    let out = eval(sheet.map(|s| ("test.omx", s)), "<expression>", expr);
    assert!(!out.ok(), "`{expr}` should fail but gave {}", out.output);
    out.errors.join("\n")
}

fn shown(sheet: &str) -> String {
    let out = view("test.omx", sheet);
    assert!(out.ok(), "view failed:\n{}", out.errors.join("\n"));
    out.output
}

fn lint_errors(sheet: &str) -> Vec<String> {
    lint("test.omx", sheet).errors
}

// ---- numerics ------------------------------------------------------------

#[test]
fn large_integers_do_not_overflow() {
    let n = calc("10**1000");
    assert_eq!(n.len(), 1001);
    assert!(n.starts_with('1') && n[1..].chars().all(|c| c == '0'));
}

#[test]
fn large_rationals_stay_exact() {
    assert_eq!(calc("999999999999999999999 / 7"), "999999999999999999999/7");
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
    assert_eq!(calc("1 / 3 * 3"), "1");
    assert_eq!(calc("19.99 * 3 == 5997/100"), "true");
}

#[test]
fn no_silent_loss_of_exactness() {
    assert_eq!(calc("approx(1/3)"), "3.333333333333333e-1");
    assert_eq!(calc("Num(1) / 3"), "3.333333333333333e-1");
    assert_eq!(calc("1/3 + 1/7 + 1/11 + 1/13 + 1/17"), "35881/51051");
    assert!(calc_err(None, "2 ** 0.5").contains("approx"));
}

#[test]
fn display_of_rationals() {
    assert_eq!(calc("175/4"), "43.75");
    assert_eq!(calc("1/3 + 1/3"), "2/3");
    assert_eq!(calc("1/3"), "1/3");
    assert_eq!(calc("100/3 * 3"), "100");
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
    assert_eq!(ask(SALES, "Sales[1; Revenue]"), "120");
    assert_eq!(ask(SALES, "Sales[1].Revenue"), "120");
    assert_eq!(ask(SALES, "Sales[; Revenue]"), "[100, 120, 150]");
    assert_eq!(
        ask(SALES, "Sales[2; ]"),
        "Month | Region | Revenue | Cost\n------|--------|---------|-----\nMar   | UK     |     150 |   80"
    );
    assert_eq!(
        ask(SALES, "Sales[1..2; 2..3]"),
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
    assert_eq!(ask(SALES, "Sales[0..2].count()"), "3");
    assert_eq!(ask(SALES, "Sales[0..^2].count()"), "2");
    let sheet = format!("const Rows = 1..2\nconst Cols = 2..3\n\n{SALES}");
    assert_eq!(
        ask(&sheet, "Sales[Rows; Cols]"),
        ask(SALES, "Sales[1..2; 2..3]")
    );
}

#[test]
fn row_cursor() {
    let sheet = format!(
        "{SALES}\nPrev := Sales[*-1; Revenue]\nGrowth := Revenue / Sales[*-1; Revenue] - 1\nRunning := Sales[0..*; Revenue].sum()\nWindow := Sales[*-1..*; Revenue].avg()\n"
    );
    assert_eq!(ask(&sheet, "Sales.Prev"), "[empty, 100, 120]");
    assert_eq!(ask(&sheet, "Sales.Growth"), "[empty, 0.2, 0.25]");
    assert_eq!(ask(&sheet, "Sales.Running"), "[100, 220, 370]");
    assert_eq!(ask(&sheet, "Sales.Window"), "[100, 110, 135]");
}

#[test]
fn cursor_needs_a_row() {
    let err = calc_err(Some(SALES), "Sales[*-1; Revenue]");
    assert!(err.contains("`*-1` needs a current row"), "{err}");
    let err = calc_err(Some(SALES), "Sales[*; Revenue]");
    assert!(err.contains("`*` needs a current row"), "{err}");
}

#[test]
fn cursor_survives_row_insertion() {
    let sheet = "table T\n\nV\n1\n5\n2\n\nPrev := T[*-1; V]\n";
    assert_eq!(ask(sheet, "T.Prev"), "[empty, 1, 5]");
}

#[test]
fn predicate_selection() {
    assert_eq!(ask(SALES, "Sales[Region == \"UK\"].Revenue.sum()"), "250");
    let dated = "table Sales\n\nDate | Region | Revenue\n2024-12-31 | UK | 1\n2025-03-01 | UK | 10\n2025-06-01 | US | 100\n2026-01-01 | UK | 1000\n";
    let expr = "Sales[\n    Region == \"UK\" and\n    Date >= 2025-01-01 and\n    Date < 2026-01-01\n].Revenue.sum()";
    assert_eq!(ask(dated, expr), "10");
}

#[test]
fn datetime_columns() {
    let sheet = "table Log\n\nAt : DateTime\n\nAt | N\n2025-03-01T09:30 | 1\n2025-03-01T17:45:10 | 10\n2025-03-02T00:00 | 100\n";
    assert_eq!(
        ask(
            sheet,
            "Log[At >= 2025-03-01T12:00 and At < 2025-03-02T00:00].N.sum()"
        ),
        "10"
    );
    assert_eq!(ask(sheet, "Log.At.max()"), "2025-03-02T00:00");
    assert_eq!(ask(sheet, "Log[1; At]"), "2025-03-01T17:45:10");
    // An undeclared column infers DateTime from its cells.
    let inferred = sheet.replace("At : DateTime\n\n", "");
    assert_eq!(ask(&inferred, "Log.At.min()"), "2025-03-01T09:30");
    // A Date is not a DateTime.
    let err = calc_err(Some(sheet), "Log[At >= 2025-03-01].N.sum()");
    assert!(err.contains("cannot compare DateTime and Date"), "{err}");
    let bad = "table Log\n\nAt : DateTime\n\nAt\n2025-03-01\n";
    let errs = lint_errors(bad);
    assert!(errs[0].contains("is not a `DateTime` literal"), "{errs:?}");
}

#[test]
fn time_columns() {
    let sheet = "table Log\n\nAt : Time\n\nAt | N\n09:30 | 1\n17:45:10 | 10\n00:00 | 100\n";
    assert_eq!(ask(sheet, "Log[At >= 09:30 and At < 18:00].N.sum()"), "11");
    assert_eq!(ask(sheet, "Log.At.max()"), "17:45:10");
    assert_eq!(ask(sheet, "Log[0; At]"), "09:30");
    // An undeclared column infers Time from its cells.
    let inferred = sheet.replace("At : Time\n\n", "");
    assert_eq!(ask(&inferred, "Log.At.min()"), "00:00");
    let err = calc_err(Some(sheet), "Log[At >= 2025-03-01].N.sum()");
    assert!(err.contains("cannot compare Time and Date"), "{err}");
    let errs = lint_errors("table Log\n\nAt : Time\n\nAt\n9.30\n");
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
        ("1 - 2025-01-31", "cannot apply `-` to Int and Date"),
        ("2025-01-31 + 1.5", "cannot apply `+` to Date and Rational"),
        ("2025-01-31 - 09:30", "cannot apply `-` to Date and Time"),
        (
            "2025-01-31T09:30 - 2025-01-31",
            "cannot apply `-` to DateTime and Date",
        ),
    ] {
        let err = calc_err(None, expr);
        assert!(err.contains(message), "{expr}: {err}");
    }

    let sheet = "table Jobs\n\nDue : Date\n\nStart | Days | Due\n2025-01-30 | 3 | = Start + Days\n";
    assert_eq!(ask(sheet, "Jobs[0; Due]"), "2025-02-02");
    let bad = sheet.replace("Due : Date", "Due : DateTime");
    assert!(lint_errors(&bad)[0].contains("declared `DateTime` but this is `Date`"));
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
    assert!(calc_err(None, "09:30.year()").contains("`year` needs a Date or a DateTime"));
    assert!(calc_err(None, "2026-10-08.hour()").contains("`hour` needs a Time or a DateTime"));
    assert!(calc_err(None, "year(2026)").contains("but this is Int"));
}

#[test]
fn today_and_now_come_from_the_options() {
    let options = Options {
        now: Some(20_734 * 86_400 + 17 * 3600 + 47 * 60),
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
    let sheet = "const Start = 2025-01-31\n\ntable T\n\nAt\n09:30\n";
    assert_eq!(
        view_with("test.omx", sheet, gb).output,
        "const Start = 31/01/2025\n\ntable T\n\nAt\n-----\n09:30\n"
    );
    assert_eq!(
        shown(sheet),
        "const Start = 2025-01-31\n\ntable T\n\nAt\n-----\n09:30\n"
    );
}

#[test]
fn index_with_no_table_name() {
    // In a table, `[...]` with a `;` or a row cursor is that table.
    let sheet = "table T\n\nV\n1\n5\n2\n\nPrev := [*-1; V]\nRun := [0..*; V].sum()\nSame := [*; V]\nFirst := [0; V]\nAll := [; V].sum()\nRow := [*-1].V // 0\n";
    assert_eq!(ask(sheet, "T.Prev"), "[empty, 1, 5]");
    assert_eq!(ask(sheet, "T.Run"), "[1, 6, 8]");
    assert_eq!(ask(sheet, "T.Same"), "[1, 5, 2]");
    assert_eq!(ask(sheet, "T.First"), "[1, 1, 1]");
    assert_eq!(ask(sheet, "T.All"), "[8, 8, 8]");
    assert_eq!(ask(sheet, "T.Row"), "[0, 1, 5]");
    // `*` as a column is the formula's own column, and counts from it.
    let grid = "table G\n\nA | B | C\n1 | = [*; *-1] + 1 | = [*; *-1] * 10\n5 | = [*-1; *] + [*; A] | = [*-1; *+0]\n";
    assert_eq!(ask(grid, "G.B"), "[2, 7]");
    assert_eq!(ask(grid, "G.C"), "[20, 20]");
    let off = lint_errors("table G\n\nA | B\n= [*; *-1] | 2\n");
    assert!(off[0].contains("no column 1 to the left"), "{off:?}");
    let err = calc_err(Some(grid), "G[0; *]");
    assert!(err.contains("`*` as a column"), "{err}");
    // A vector literal is still a vector.
    assert_eq!(calc("[1, 2, 3].sum()"), "6");
    assert_eq!(calc("[4]"), "[4]");
    // Outside a table there is no table to mean.
    let err = calc_err(Some(sheet), "[0; V]");
    assert!(err.contains("needs a current table"), "{err}");
}

#[test]
fn complex_numbers() {
    // Written with `i`, and built from parts.
    assert_eq!(calc("3+4i"), "3.0+4.0i");
    assert_eq!(calc("(1+2i) * (3-1i)"), "5.0+5.0i");
    assert_eq!(calc("2i * 2i"), "-4.0+0.0i");
    assert_eq!(calc("Complex(3, 4) == 3+4i"), "true");
    assert_eq!(calc("[re(3+4i), im(3+4i), abs(3+4i)]"), "[3e0, 4e0, 5e0]");
    assert_eq!(calc("conj(3+4i)"), "3.0-4.0i");
    assert_eq!(calc("degrees(arg(2i))"), "9e1");
    assert_eq!(calc("sqrt(-4+0i)"), "0.0+2.0i");
    assert_eq!(calc("ln(exp(1+1i))"), "1.0+1.0i");
    // Real numbers have the same parts.
    assert_eq!(calc("[re(5/2), im(7), conj(3)]"), "[2.5, 0, 3]");
    assert!(calc_err(None, "sin(1i)").contains("`sin` cannot be applied to Complex"));
    assert!(calc_err(None, "1i < 2i").contains("cannot compare"));
    // A column of them: literal cells, a real number, and a formula.
    let sheet =
        "table Z\n\nV : Complex\n\nV\n3+4i\n-2i\n5\n-1.5-2i\n= [*-4; V] * 1i\n\nSize := abs(V)\n";
    assert_eq!(
        ask(sheet, "Z.V"),
        "[3.0+4.0i, 0.0-2.0i, 5.0+0.0i, -1.5-2.0i, -4.0+3.0i]"
    );
    assert_eq!(ask(sheet, "Z.Size"), "[5e0, 2e0, 5e0, 2.5e0, 5e0]");
    assert_eq!(ask(sheet, "Z.V.sum()"), "2.5+3.0i");
    // Undeclared, a column of them is still Complex.
    let inferred = sheet.replace("V : Complex\n\n", "");
    assert_eq!(ask(&inferred, "Z[1; V]"), "0.0-2.0i");
    let bad = lint_errors("table Z\n\nV : Complex\n\nV\nfour\n");
    assert!(bad[0].contains("such as `3+4i`"), "{bad:?}");
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
    assert_eq!(calc("round(2.5e0)"), "3e0");
    // The rest give a Num.
    assert_eq!(calc("sqrt(16)"), "4e0");
    assert_eq!(calc("exp(0)"), "1e0");
    assert_eq!(calc("ln(exp(2))"), "2e0");
    assert_eq!(calc("[log10(1000), log2(8)]"), "[3e0, 3e0]");
    assert_eq!(calc("sin(0) + cos(0)"), "1e0");
    assert_eq!(calc("degrees(pi())"), "1.8e2");
    assert_eq!(calc("round(sin(radians(30)) * 1000)"), "5e2");
    assert_eq!(calc("round(degrees(atan(1)))"), "4.5e1");
    // Over a column, and as a method.
    assert_eq!(
        ask(SALES, "Sales.Revenue.sqrt().floor()"),
        "[1e1, 1e1, 1.2e1]"
    );
    // Outside the domain, and on the wrong kind of value.
    assert!(calc_err(None, "sqrt(-1)").contains("`sqrt` is not defined for -1.0"));
    assert!(calc_err(None, "ln(0)").contains("`ln` is not defined"));
    assert!(calc_err(None, "sin(\"x\")").contains("`sin` cannot be applied to Text"));
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
    let with = |options: &Options, sheet: Option<&str>, expr: &str| {
        let sheet = sheet.map(|s| ("test.omx", s));
        let out = eval_with(sheet, "<expression>", expr, options.clone());
        assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
        out.output.trim_end().to_string()
    };
    let at = |expr: &str| with(&london, None, expr);

    // The same instant on other clocks, with daylight saving.
    assert_eq!(
        at("2025-07-15T12:00.to_zone(\"Asia/Tokyo\")"),
        "2025-07-15T20:00+09:00"
    );
    assert_eq!(at("2025-07-15T12:00.utc()"), "2025-07-15T11:00+00:00");
    assert_eq!(at("2025-01-15T12:00.utc()"), "2025-01-15T12:00+00:00");
    assert_eq!(
        at("2025-07-15T12:00.to_zone(\"America/St_Johns\")"),
        "2025-07-15T08:30-02:30"
    );
    assert_eq!(at("2025-07-15T12:00.offset()"), "3600");
    assert_eq!(at("2025-01-15T12:00.offset()"), "0");
    assert_eq!(at("2025-07-15T12:00.zone()"), "Europe/London");
    assert_eq!(at("2025-07-15T12:00.utc().zone()"), "UTC");
    assert_eq!(at("2025-07-15T12:00.utc().offset()"), "0");
    // Back to the sheet's own zone it is an ordinary date-time again.
    assert_eq!(
        at("2025-07-15T12:00.to_zone(\"Asia/Tokyo\").to_zone(\"Europe/London\")"),
        "2025-07-15T12:00"
    );
    assert_eq!(at("2025-07-15T12:00.utc().local()"), "2025-07-15T12:00");

    // The parts are those of the zone it is in.
    assert_eq!(at("2025-07-15T20:00.to_zone(\"Asia/Tokyo\").hour()"), "4");
    assert_eq!(
        at("2025-07-15T20:00.to_zone(\"Asia/Tokyo\").date()"),
        "2025-07-16"
    );
    // Seconds are added on its own clocks.
    assert_eq!(
        at("2025-07-15T12:00.to_zone(\"Asia/Tokyo\") + 3600"),
        "2025-07-15T21:00+09:00"
    );
    // Across zones it is the instant that is compared and subtracted.
    assert_eq!(
        at("2025-07-15T12:00.to_zone(\"Asia/Tokyo\") == 2025-07-15T12:00"),
        "true"
    );
    assert_eq!(
        at("2025-07-15T12:00.utc() < 2025-07-15T12:00:01.to_zone(\"Asia/Tokyo\")"),
        "true"
    );
    assert_eq!(
        at("2025-07-15T13:00.to_zone(\"Asia/Tokyo\") - 2025-07-15T12:00.utc()"),
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
        "2025-07-15T12:00.to_zone(\"Mars/Base\")",
        london.clone(),
    );
    assert!(out.errors[0].contains("there is no time zone `Mars/Base`"));
    assert!(calc_err(None, "2025-07-15.utc()").contains("`utc` needs a DateTime"));
    assert!(calc_err(None, "now().to_zone(3)").contains("needs the name of a time zone"));

    // A sheet names the zone its date-times are in.
    let sheet = "zone America/New_York\n\ntable Calls\n\nAt\n2025-03-03T09:12\n\n\
                 Tokyo := At.to_zone(\"Asia/Tokyo\")\n";
    let ny = |expr: &str| with(&london, Some(sheet), expr);
    assert_eq!(ny("Calls[0; At]"), "2025-03-03T09:12");
    assert_eq!(ny("Calls[0; At].zone()"), "America/New_York");
    assert_eq!(ny("Calls[0; At].offset()"), "-18000");
    assert_eq!(ny("Calls[0; Tokyo]"), "2025-03-03T23:12+09:00");
    assert_eq!(ny("Calls[0; At].local()"), "2025-03-03T14:12+00:00");
    assert_eq!(ny("Calls[0; At].utc() == Calls[0; At]"), "true");
    // `now()` is on the sheet's clocks: five hours behind London in winter.
    assert_eq!(ny("now().zone()"), "America/New_York");
    assert_eq!(ny("now().local() == now()"), "true");
    let fixed = Options {
        now: Some(20_734 * 86_400),
        ..london.clone()
    };
    assert_eq!(with(&fixed, Some(sheet), "now()"), "2026-10-08T00:00");
    assert_eq!(
        with(&fixed, Some(sheet), "now().local()"),
        "2026-10-08T05:00+01:00"
    );

    // Shown on this machine's clocks only when that is asked for.
    let local = Options {
        local: true,
        ..london.clone()
    };
    assert_eq!(
        with(&local, Some(sheet), "Calls[0; At]"),
        "2025-03-03T14:12"
    );
    assert_eq!(with(&local, Some(sheet), "Calls.At"), "[2025-03-03T14:12]");
    assert_eq!(
        with(&local, Some(sheet), "Calls[0; Tokyo]"),
        "2025-03-03T23:12+09:00"
    );
    assert_eq!(with(&local, Some(sheet), "Calls[0; At].hour()"), "9");
    assert_eq!(
        view_with("test.omx", sheet, local).output,
        "table Calls\n\nAt               | Tokyo\n-----------------|-----------------------\n\
         2025-03-03T14:12 | 2025-03-03T23:12+09:00\n"
    );

    // The zone line is checked, and belongs before the first table.
    let errs = lint_errors("zone Mars/Base\n\ntable T\n\nA\n1\n");
    assert!(
        errs[0].contains("test.omx:1:6: error: there is no time zone `Mars/Base`"),
        "{errs:?}"
    );
    assert!(!view("test.omx", "zone Mars/Base\n\ntable T\n\nA\n1\n").ok());
    let errs = lint_errors("zone UTC\nzone Asia/Tokyo\n\ntable T\n\nA\n1\n");
    assert!(
        errs[0].contains("the sheet already has a `zone`"),
        "{errs:?}"
    );
    assert!(lint_errors("zone UTC\n\ntable T\n\nA\n1\n").is_empty());
    assert_eq!(ask("table T\n\nA\nzone UTC\n", "T[0; A]"), "zone UTC");
}

#[test]
fn lookup_from_another_table() {
    let sheet = "\
table Customers

ID | Name
1  | Ada
2  | Grace

table Sales

CustomerID | Amount
2          | 50
7          | 20
1          | 10

Customer := Customers[ID == CustomerID].Name // \"Unknown\"
";
    assert_eq!(
        ask(sheet, "Sales.Customer"),
        "[\"Grace\", \"Unknown\", \"Ada\"]"
    );
}

#[test]
fn operators() {
    let sheet =
        format!("{SALES}\nPrev := Sales[*-1; Revenue] // 0\nHome := Region in [\"UK\", \"IE\"]\n");
    assert_eq!(ask(&sheet, "Sales.Prev"), "[0, 100, 120]");
    assert_eq!(ask(&sheet, "Sales.Home"), "[true, false, true]");
    let err = calc_err(Some(SALES), "Sales[Region = \"UK\"]");
    assert!(err.contains("=="), "{err}");
}

#[test]
fn conditional_expression() {
    let sheet = "table T\n\nRevenue | Region\n2000 | UK\n500 | UK\n\nNet :=\n  if Revenue > 1000 and Region in [\"UK\", \"US\"]\n  then Revenue * 90%\n  else Revenue\n";
    assert_eq!(ask(sheet, "T.Net"), "[1800, 500]");
}

#[test]
fn value_shapes() {
    assert!(ask(SALES, "Sales[Region == \"UK\"]").starts_with("Month | Region"));
    assert_eq!(ask(SALES, "Sales[Region == \"UK\"].Revenue"), "[100, 150]");
    assert_eq!(ask(SALES, "Sales[Region == \"UK\"].Revenue.sum()"), "250");
}

#[test]
fn aggregation_methods() {
    assert_eq!(ask(SALES, "Sales.Revenue.sum()"), "370");
    assert_eq!(ask(SALES, "Sales.Revenue.avg()"), "370/3");
    assert_eq!(ask(SALES, "Sales.Revenue.min()"), "100");
    assert_eq!(ask(SALES, "Sales.Revenue.max()"), "150");
    assert_eq!(ask(SALES, "Sales.Revenue.count()"), "3");
}

#[test]
fn broadcasting() {
    assert_eq!(calc("[100, 200, 300] * 20%"), "[20, 40, 60]");
    assert_eq!(ask(SALES, "Sales.Revenue - Sales.Cost"), "[40, 50, 70]");
    assert_eq!(
        ask(SALES, "Sales[Sales.Revenue > 110].Month"),
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
        ask(SALES, "Sales[Region == \"UK\"].Revenue.sum()")
    );
}

// ---- sheet-format ----------------------------------------------------------

#[test]
fn minimal_table_and_optional_separator() {
    let plain =
        "table Sales\n\nMonth | Revenue | Cost\nJan   | 10000   | 6000\nFeb   | 12000   | 7000\n";
    let ruled = "table Sales\n\nMonth | Revenue | Cost\n------|---------|-----\nJan   | 10000   | 6000\nFeb   | 12000   | 7000\n";
    assert_eq!(shown(plain), shown(ruled));
    assert_eq!(ask(plain, "Sales.count()"), "2");
}

#[test]
fn ragged_row() {
    let errors = lint_errors("table Sales\n\nMonth | Revenue\nJan | 1 | 2\n");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].starts_with("test.omx:4:1:"), "{}", errors[0]);
    assert!(errors[0].contains("table `Sales`"), "{}", errors[0]);
}

#[test]
fn multiple_tables() {
    let sheet = format!("{SALES}\ntable Summary\n\nLabel | Value\nTotal | = Sales.Revenue.sum()\n");
    assert_eq!(ask(&sheet, "Summary[0; Value]"), "370");
    let errors = lint_errors("table Sales\n\nA\n1\n\ntable Sales\n\nA\n2\n");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].starts_with("test.omx:6:"), "{}", errors[0]);
}

#[test]
fn column_schema() {
    let sheet =
        "table Items\n\nQty   : Int\nPrice : Rational\n\nQty | Price\n2   | 19.99\n5   | 0.50\n";
    assert_eq!(ask(sheet, "Items[0; Price] == 1999/100"), "true");
    assert_eq!(ask(sheet, "Items[0; Qty] * Items[0; Price]"), "39.98");
    let errors = lint_errors("table Items\n\nPrise : Rational\n\nQty | Price\n2 | 1\n");
    assert!(errors[0].contains("no column `Prise`"), "{}", errors[0]);
}

#[test]
fn computed_columns() {
    let sheet = format!("{SALES}\nProfit := Revenue - Cost\n");
    assert_eq!(ask(&sheet, "Sales.Profit"), "[40, 50, 70]");
    let errors = lint_errors("table T\n\nA | Profit\n1 | 2\n\nProfit := A\n");
    assert!(
        errors[0].contains("computed column `Profit`"),
        "{}",
        errors[0]
    );
}

#[test]
fn cell_content() {
    // Text in an undeclared column.
    assert_eq!(ask(SALES, "Sales[0; Month]"), "Jan");
    // A marked formula in a typed column.
    let sheet = "const TaxRate = 20%\n\ntable T\n\nTax : Rational\n\nRevenue | Tax\n100 | = Revenue * TaxRate\n";
    assert_eq!(ask(sheet, "T[0; Tax]"), "20");
    // A fraction of two whole numbers is a Rational literal, declared or not.
    let thirds = "table T\n\nA : Rational\n\nA | B\n1/7 | 2/3\n-3/6 | 1/3\n4/2 | x\n";
    assert_eq!(ask(thirds, "T.A"), "[1/7, -0.5, 2]");
    assert_eq!(ask(thirds, "T[0..1; B].sum()"), "1");
    assert!(
        lint_errors("table T\n\nA : Rational\n\nA\n1/0\n")[0]
            .contains("is not a `Rational` literal")
    );
    assert_eq!(ask("table T\n\nA\n1/0\n", "T[0; A]"), "1/0");
    // An unmarked expression in a typed column is an error.
    let errors = lint_errors("table T\n\nTax : Rational\n\nRevenue | Tax\n100 | Revenue * 2\n");
    assert!(
        errors[0].contains("is not a `Rational` literal"),
        "{}",
        errors[0]
    );
    // An unmarked expression in a text column is text.
    let sheet = "table T\n\nNote : Text\n\nRevenue | Note\n100 | Revenue - Cost\n200 | \"= see appendix\"\n";
    assert_eq!(
        ask(sheet, "T.Note"),
        "[\"Revenue - Cost\", \"= see appendix\"]"
    );
    // An empty cell.
    assert_eq!(ask("table T\n\nA | B\n1 |\n", "T[0; B]"), "empty");
}

#[test]
fn constants() {
    let sheet =
        "const TaxRate = 20%\n\ntable Sales\n\nRevenue\n100\n250\n\nTax := Revenue * TaxRate\n";
    assert_eq!(ask(sheet, "Sales.Tax"), "[20, 50]");
    assert_eq!(ask(sheet, "TaxRate == 1/5"), "true");
}

#[test]
fn a1_reference_is_not_special() {
    let errors = lint_errors("table T\n\nA\n1\n\nB := B2\n");
    assert!(errors[0].contains("unknown name `B2`"), "{}", errors[0]);
}

// ---- evaluation ------------------------------------------------------------

#[test]
fn unresolved_name_is_a_compile_error() {
    let errors = lint_errors("table Sales\n\nRevenue | Cost\n1 | 2\n\nProfit := Revnue - Cost\n");
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
    let sheet = "table Sales\n\nMonth | Revenue\nJan | 100\n\nDouble := Revenue * 2\nBad := Revenue + Month\n";
    let out = view("test.omx", sheet);
    assert_eq!(out.errors.len(), 1);
    assert!(
        out.errors[0].contains("cannot apply `+` to Int and Text"),
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
    let sheet = "table T\n\nName | N\nx | 1\n\nA := N + Name\nB := Name * 2\nC := not N\n";
    assert_eq!(lint_errors(sheet).len(), 3);
}

#[test]
fn declaration_order_does_not_matter() {
    let sheet = "table T\n\nRevenue | Cost\n100 | 60\n\nMargin := Profit / Revenue\nProfit := Revenue - Cost\n";
    assert_eq!(ask(sheet, "T.Margin"), "[0.4]");
}

#[test]
fn row_wise_self_reference() {
    let sheet =
        "table Ledger\n\nAmount\n10\n-3\n5\n\nBalance := (Ledger[*-1; Balance] // 0) + Amount\n";
    assert_eq!(ask(sheet, "Ledger.Balance"), "[10, 7, 12]");
}

#[test]
fn cycle_detection() {
    let errors = lint_errors("table T\n\nX\n1\n\nA := B + 1\nB := A + 1\n");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("A → B → A"), "{}", errors[0]);
}

// ---- cli (the parts that do not need a process) ----------------------------

#[test]
fn view_shows_computed_values() {
    let sheet = format!("{SALES}\nProfit := Revenue - Cost\n");
    assert_eq!(
        shown(&sheet),
        "\
table Sales

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
    let errors = lint_errors(&format!("{SALES}\nBad := Revenue + Month\n"));
    assert_eq!(errors.len(), 1);
}

#[test]
fn diagnostics_are_located() {
    let sheet = "table T\n\nName | N\nx | 1\n\n\n\n\n\n\n\n\n\nBad := N + Name\n";
    let errors = lint_errors(sheet);
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

func Margin(revenue, cost) = (revenue - cost) / revenue
func WithTax(x) = x * (1 + Rate)
func Twice(x) = WithTax(WithTax(x))
func Total(t) = t.Revenue.sum()
func NameOf(id) = Customers[ID == id].Name // \"Unknown\"

table Customers

ID | Name
1  | Ada
2  | Grace

table Sales

Month | Revenue | Cost | Who
Jan   | 100     | 60   | 1
Feb   | 120     | 90   | 9

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
fn custom_function_takes_tables_and_vectors() {
    assert_eq!(ask(FUNCS, "Total(Sales)"), "220");
    assert_eq!(ask(FUNCS, "Total(Sales |> filter(Cost > 60))"), "120");
    assert_eq!(ask(FUNCS, "WithTax(Sales.Revenue)"), "[120, 144]");
}

#[test]
fn custom_function_does_not_see_the_callers_row() {
    let sheet = "func Bad(x) = x + Cost\n\ntable T\n\nRevenue | Cost\n1 | 2\n\nA := Bad(Revenue)\n";
    let errors = lint_errors(sheet).join("\n");
    assert!(errors.contains("unknown name `Cost`"), "{errors}");
}

#[test]
fn custom_function_is_checked_for_each_call() {
    let err = calc_err(Some(FUNCS), "WithTax(\"ten\")");
    assert!(
        err.contains("in `WithTax`: cannot apply `*` to Text"),
        "{err}"
    );
    let err = calc_err(Some(FUNCS), "Margin(1)");
    assert!(err.contains("`Margin` takes 2 arguments, found 1"), "{err}");
    assert!(err.contains("Margin(revenue, cost)"), "{err}");
    let err = calc_err(Some(FUNCS), "Margin(0, 1)");
    assert!(err.contains("in `Margin`: division by zero"), "{err}");
}

#[test]
fn custom_function_definitions_are_checked() {
    let errors = lint_errors("func sum(x) = x\n").join("\n");
    assert!(errors.contains("`sum` is a built-in function"), "{errors}");
    let errors = lint_errors("func F(x) = x\nfunc F(y) = y\n").join("\n");
    assert!(
        errors.contains("function `F` is already defined"),
        "{errors}"
    );
    let errors = lint_errors("func F(x, x) = x\n").join("\n");
    assert!(errors.contains("two parameters named `x`"), "{errors}");
    let errors = lint_errors("func F(x) = F(x) + 1\n").join("\n");
    assert!(errors.contains("function `F` calls itself"), "{errors}");
    let errors = lint_errors("func A(x) = B(x)\nfunc B(x) = A(x)\n").join("\n");
    assert!(errors.contains("calls itself"), "{errors}");
    let errors = lint_errors("func F(x) = x + Nope\n").join("\n");
    assert!(errors.contains("unknown name `Nope`"), "{errors}");
    assert!(lint_errors("func F(t, n) = t[n].Revenue + t[Revenue > n].Cost.sum()\n").is_empty());
}

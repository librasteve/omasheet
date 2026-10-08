//! The scenarios of the Phase 1 specs (`openspec/changes/add-omasheet-core`),
//! one test per scenario where it can be checked from the outside.

use omasheet_engine::{eval, lint, view};

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
    assert_eq!(calc("approx(1/3)"), "0.3333333333333333");
    assert_eq!(calc("Num(1) / 3"), "0.3333333333333333");
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
    let sheet = "table Items\n\nQty   : Int\nPrice : Rat\n\nQty | Price\n2   | 19.99\n5   | 0.50\n";
    assert_eq!(ask(sheet, "Items[0; Price] == 1999/100"), "true");
    assert_eq!(ask(sheet, "Items[0; Qty] * Items[0; Price]"), "39.98");
    let errors = lint_errors("table Items\n\nPrise : Rat\n\nQty | Price\n2 | 1\n");
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
    let sheet =
        "const TaxRate = 20%\n\ntable T\n\nTax : Rat\n\nRevenue | Tax\n100 | = Revenue * TaxRate\n";
    assert_eq!(ask(sheet, "T[0; Tax]"), "20");
    // An unmarked expression in a typed column is an error.
    let errors = lint_errors("table T\n\nTax : Rat\n\nRevenue | Tax\n100 | Revenue * 2\n");
    assert!(
        errors[0].contains("is not a `Rat` literal"),
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

// Copyright (c) 2026 Stephen Roe

//! The scenarios of the `csv-interop` and `xlsx-interop` specs.

use calamine::{Data, Reader, open_workbook_auto_from_rs};
use omasheet_engine::Options;
use omasheet_engine::omx::date::MICROS;
use omasheet_interop::{Datum, Exported, csv, json, parquet, xlsx};
use rust_xlsxwriter::{Format, Formula, Workbook};
use std::io::Cursor;

const BUDGET: &str = "\
const TaxRate = 20%

sheet Sales

Month | Revenue | Cost | Profit | Share
Jan   | 10000   | 6000 | *      | *
Feb   | 12000   | 7000 | *      | *

Profit := Revenue - Cost
Share  := Profit / 3

sheet Summary

Item    | Value
Revenue | = Sales.Revenue.sum()
";

fn options() -> Options {
    Options {
        now: Some(1_750_000_000 * MICROS),
        ..Options::default()
    }
}

/// The options for a file read from `dir`, where a `table` finds the
/// Parquet it reads beside it. Exporting one without this cannot find its
/// rows, and says so rather than writing a sheet with none.
fn options_beside(dir: &str) -> Options {
    Options {
        base: Some(std::path::PathBuf::from(dir)),
        ..options()
    }
}

fn csv_of(text: &str, sheet: Option<&str>) -> Exported {
    csv::export("file.omx", text, sheet, options()).expect("the file exports")
}

fn text_of(exported: &Exported, file: usize) -> String {
    String::from_utf8(exported.files[file].bytes.clone()).unwrap()
}

/// The value of `expr` over a file, as `omasheet eval --exact` prints it.
fn eval(file: &str, expr: &str) -> String {
    let options = Options {
        exact: true,
        ..options()
    };
    let out = omasheet_engine::eval_with(Some(("file.omx", file)), "<expr>", expr, options);
    assert!(out.ok(), "{:?}\n{file}", out.errors);
    out.output.trim().to_string()
}

fn lints(source: &str) {
    let out = omasheet_engine::lint("imported.omx", source);
    assert!(out.ok(), "{}\n{source}", out.errors.join("\n"));
}

// ---- CSV export ----------------------------------------------------------

#[test]
fn csv_writes_a_file_for_each_sheet() {
    let out = csv_of(BUDGET, None);
    let sheets: Vec<_> = out.files.iter().map(|f| f.sheet.as_deref()).collect();
    assert_eq!(sheets, [Some("Sales"), Some("Summary")]);
    assert_eq!(text_of(&out, 1), "Item,Value\nRevenue,22000\n");
}

#[test]
fn csv_of_one_sheet_has_values_for_computed_columns() {
    let out = csv_of(BUDGET, Some("Sales"));
    assert_eq!(out.files.len(), 1);
    assert_eq!(out.files[0].sheet, None);
    assert_eq!(
        text_of(&out, 0),
        "Month,Revenue,Cost,Profit,Share\n\
         Jan,10000,6000,4000,1333.33333333333333333333333333\n\
         Feb,12000,7000,5000,1666.66666666666666666666666667\n"
    );
    assert!(out.notices.is_empty(), "{:?}", out.notices);
}

#[test]
fn csv_keeps_exact_what_ends() {
    let file = "sheet T\n\nA\n19.99\n= 2 ** 100\n= 1/3\n= 1/1024\n";
    let out = csv_of(file, None);
    assert_eq!(
        text_of(&out, 0),
        "A\n19.99\n1267650600228229401496703205376\n0.333333333333333333333333333333\n0.0009765625\n"
    );
    // Nothing is rounded to write it: a decimal holds every digit there
    // is of it, so there is nothing to report.
    assert!(out.notices.is_empty(), "{:?}", out.notices);
}

#[test]
fn csv_keeps_percentages() {
    let file = "sheet T\n\nB : Percent\n\nA | B\n20% | = 2/5\n12.5% | = 1/3\n";
    let out = csv_of(file, None);
    let text = text_of(&out, 0);
    assert!(
        text.starts_with("A,B\n20%,40%\n12.5%,33.3333333333333333"),
        "{text}"
    );
    assert!(out.notices.is_empty(), "{:?}", out.notices);
    // And reads them back as percentages.
    let back = csv::import("t", text.as_bytes()).unwrap();
    lints(&back.source);
    assert_eq!(eval(&back.source, "t.A.sum()"), "32.5%");
}

#[test]
fn xlsx_writes_a_percentage_as_its_number() {
    let file = "sheet T\n\nA | B\n25% | 0.5\n";
    let out = xlsx::export("t.omx", file, None, options()).unwrap();
    let rows = cells(&out.files[0].bytes, "T");
    assert_eq!(rows[1][0], Data::Float(0.25));
    assert!(out.notices.is_empty(), "{:?}", out.notices);
}

#[test]
fn csv_quotes_as_rfc_4180() {
    let file = "sheet T\n\nWho : String\n\nWho\n\"Smith, J \\\"Jay\\\"\"\n";
    assert_eq!(
        text_of(&csv_of(file, None), 0),
        "Who\n\"Smith, J \"\"Jay\"\"\"\n"
    );
}

#[test]
fn csv_leaves_a_failed_cell_empty_and_says_so() {
    let file = "sheet T\n\nA | B\n1 | = Inf.Int\n";
    let out = csv_of(file, None);
    assert_eq!(text_of(&out, 0), "A,B\n1,\n");
    assert_eq!(
        out.notices,
        ["cells that could not be calculated were left empty: T.B"]
    );
}

#[test]
fn a_file_that_does_not_compile_is_not_exported() {
    let errors = csv::export("bad.omx", "sheet T\n\nA\n= Nope\n", None, options()).unwrap_err();
    assert!(errors[0].starts_with("bad.omx:4:3: error:"), "{errors:?}");
    let errors = csv::export("s.omx", BUDGET, Some("Costs"), options()).unwrap_err();
    assert!(errors[0].contains("no sheet `Costs`"), "{errors:?}");
}

// ---- CSV import ----------------------------------------------------------

#[test]
fn csv_imports_a_sheet_named_after_the_file() {
    let out = csv::import("sales", b"Month,Revenue,Cost\nJan,100,60.5\nFeb,120,\n").unwrap();
    assert_eq!(
        out.source,
        "sheet sales\n\n| Month | Revenue | Cost |\n| ----- | ------- | ---- |\n| Jan   | 100     | 60.5 |\n| Feb   | 120     |      |\n"
    );
    assert!(out.notices.is_empty(), "{:?}", out.notices);
    lints(&out.source);
    assert_eq!(eval(&out.source, "sales.Revenue.sum()"), "220");
}

#[test]
fn csv_reads_every_digit() {
    let out = csv::import("t", b"A\n0.12345678901234567890123\n").unwrap();
    // Every digit is read and kept; multiplying out brings them back, as
    // far as the thirty-eight a decimal holds.
    assert_eq!(
        eval(&out.source, "t[A; 0] * 1000000000000000"),
        "123456789012345.67890123"
    );
}

#[test]
fn csv_text_that_looks_like_a_formula_stays_text() {
    let out = csv::import("t", b"A,B\n=SUM(A1:A3),x\n").unwrap();
    lints(&out.source);
    assert_eq!(eval(&out.source, "t[A; 0]"), "=SUM(A1:A3)");
}

#[test]
fn csv_types_its_columns() {
    let data = "\u{feff}When,Ok,Mixed,At\n2025-01-31,true,12,09:30\n2025-02-01,false,n/a,17:05\n";
    let out = csv::import("log", data.as_bytes()).unwrap();
    assert!(
        out.source
            .contains("\n\nWhen : Date\nOk   : Boolean\nAt   : Time\n\n"),
        "{}",
        out.source
    );
    assert_eq!(
        out.notices,
        ["`log.Mixed` holds values of more than one type, and has no type of its own"]
    );
    lints(&out.source);
    // Each cell of a mixed column is what it reads as.
    assert_eq!(eval(&out.source, "log[Mixed; 0] + 1"), "13");
    assert_eq!(eval(&out.source, "log[Mixed; 1]"), "n/a");
    assert_eq!(eval(&out.source, "log.When.max() - log.When.min()"), "1");
}

#[test]
fn csv_makes_names_identifiers() {
    let out = csv::import("2024 sales", b"Unit Price,Unit Price,,in\n1,2,3,4\n").unwrap();
    assert!(
        out.source.starts_with("sheet _2024Sales\n"),
        "{}",
        out.source
    );
    assert!(
        out.source
            .contains("| UnitPrice | UnitPrice_2 | Column3 | in_ |\n"),
        "{}",
        out.source
    );
    assert_eq!(
        out.notices,
        [
            "`2024 sales` was renamed `_2024Sales`",
            "`Unit Price` was renamed `UnitPrice`",
            "`Unit Price` was renamed `UnitPrice_2`",
            "a column with no name was named `Column3`",
            "`in` was renamed `in_`",
        ]
    );
    lints(&out.source);
}

#[test]
fn csv_awkward_text_survives() {
    let data = "Note\na|b\n\"say \"\"hi\"\"\"\n# not a comment\nsheet Sales\n---\n padded \n\"two\nlines\"\n";
    let out = csv::import("t", data.as_bytes()).unwrap();
    lints(&out.source);
    let again = csv_of(&out.source, None);
    assert_eq!(text_of(&again, 0), data);
}

#[test]
fn csv_round_trips_the_examples() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "omx") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let first =
            csv::export("file.omx", &text, None, options_beside(dir)).expect("the file exports");
        for file in 0..first.files.len() {
            let name = first.files[file].sheet.as_deref().unwrap_or("T");
            let imported = csv::import(name, &first.files[file].bytes).unwrap();
            lints(&imported.source);
            let second = csv_of(&imported.source, None);
            let fields = |text: &str| -> Vec<String> {
                let mut reader = ::csv::ReaderBuilder::new()
                    .has_headers(false)
                    .from_reader(text.as_bytes());
                let records = reader.records().map(|r| r.unwrap());
                records
                    .flat_map(|r| r.iter().map(String::from).collect::<Vec<_>>())
                    .collect()
            };
            // CSV holds no types, so text that reads as a number is one once
            // it has been through: `1/3`, `20%` and `1.5e3` come back as
            // `0.3333333333333333`, `0.2` and `1500`. Everything else comes
            // back as it went.
            let (was, now) = (fields(&text_of(&first, file)), fields(&text_of(&second, 0)));
            assert_eq!(was.len(), now.len(), "{} sheet {name}", path.display());
            for (a, b) in was.iter().zip(&now) {
                assert!(
                    a == b || matches!(Datum::read(a), Datum::Lit(..)),
                    "{} sheet {name}: `{a}` came back `{b}`\n{}",
                    path.display(),
                    imported.source
                );
            }
            // And a second trip changes nothing.
            let again = csv::import(name, &second.files[0].bytes).unwrap();
            assert_eq!(
                text_of(&csv_of(&again.source, None), 0),
                text_of(&second, 0),
                "{} sheet {name}",
                path.display()
            );
            seen += 1;
        }
    }
    assert!(seen > 5, "only {seen} sheets");
}

// ---- XLSX ----------------------------------------------------------------

fn cells(bytes: &[u8], worksheet: &str) -> Vec<Vec<Data>> {
    let mut book = open_workbook_auto_from_rs(Cursor::new(bytes)).unwrap();
    let range = book.worksheet_range(worksheet).unwrap();
    range.rows().map(|r| r.to_vec()).collect()
}

#[test]
fn xlsx_has_a_worksheet_for_each_sheet() {
    let out = xlsx::export("budget.omx", BUDGET, None, options()).unwrap();
    assert_eq!(out.files.len(), 1);
    let bytes = &out.files[0].bytes;
    let book = open_workbook_auto_from_rs(Cursor::new(bytes)).unwrap();
    assert_eq!(book.sheet_names(), ["Sales", "Summary"]);
    let sales = cells(bytes, "Sales");
    assert_eq!(sales[0][3], Data::String("Profit".into()));
    assert_eq!(sales[1][0], Data::String("Jan".into()));
    assert_eq!(sales[1][3], Data::Float(4000.0));
    assert_eq!(
        out.notices,
        [
            "constants are not exported: TaxRate",
            "5 formulas written",
            "exact rationals were written as floating point: Sales.Share",
        ]
    );
}

/// The formulas of a worksheet, by cell: `B2` and what is written there.
fn formulas(bytes: &[u8], worksheet: &str) -> Vec<(String, String)> {
    let mut book = open_workbook_auto_from_rs(Cursor::new(bytes)).unwrap();
    let found = book.worksheet_formula(worksheet).unwrap();
    let (top, left) = found.start().unwrap_or((0, 0));
    found
        .used_cells()
        .filter(|(_, _, f)| !f.is_empty())
        .map(|(r, c, f)| {
            let column = (b'A' + (c as u32 + left) as u8) as char;
            (format!("{column}{}", r as u32 + top + 1), f.clone())
        })
        .collect()
}

fn formula_at(bytes: &[u8], worksheet: &str, cell: &str) -> String {
    let all = formulas(bytes, worksheet);
    let found = all.iter().find(|(at, _)| at == cell);
    found.map_or(String::new(), |(_, f)| f.clone())
}

#[test]
fn xlsx_writes_formulas() {
    let out = xlsx::export("budget.omx", BUDGET, None, options()).unwrap();
    let bytes = &out.files[0].bytes;
    // A computed column is written on each row, for that row.
    assert_eq!(formula_at(bytes, "Sales", "D2"), "B2-C2");
    assert_eq!(formula_at(bytes, "Sales", "D3"), "B3-C3");
    assert_eq!(formula_at(bytes, "Sales", "E3"), "D3/3");
    // A cell that is not a formula holds its value alone.
    assert_eq!(formula_at(bytes, "Sales", "B2"), "");
    // Another sheet is another worksheet, and a whole column is all its rows.
    assert_eq!(formula_at(bytes, "Summary", "B2"), "SUM('Sales'!$B$2:$B$3)");
    // The value is there too, for a reader that does not work it out.
    assert_eq!(cells(bytes, "Sales")[1][3], Data::Float(4000.0));
    assert_eq!(cells(bytes, "Summary")[1][1], Data::Float(22000.0));
}

#[test]
fn xlsx_formulas_read_rows_and_sheets() {
    let file = "\
const Rate = 20%

sheet Rates

Code | Fee
a    | 5
b    | 7.5

sheet T

On : Date
At : Time

Code | On         | At    | N  | Prev | Run | Fee | Net | First | Next | Late | Hits | Paid | Odd
a    | 2025-01-31 | 09:00 | 10 | *    | *   | *   | *   | *     | *    | *    | *    | *    | *
b    | 2025-02-01 | 17:30 | 20 | *    | *   | *   | *   | *     | *    | *    | *    | *    | *
c    | 2025-02-03 | 08:15 | 30 | *    | *   | *   | *   | *     | *    | *    | *    | *    | *

Prev  := [N; *-1] // 0
Run   := sum([N; 0..*])
Fee   := Rates[Fee; Code == T[Code; *]] // 1
Net   := N * (1 - Rate) + Fee
First := [N; 0] + [N; -1]
Next  := On + 7
Late  := At - 09:00
Hits  := len(Rates[; Code == T[Code; *]])
Paid  := sum(Rates[Fee; Code == T[Code; *]])
Odd   := [N; *] in [10, 30]
";
    let out = xlsx::export("t.omx", file, None, options()).unwrap();
    let bytes = &out.files[0].bytes;
    let at = |cell: &str| formula_at(bytes, "T", cell);
    // The row before the first is nothing, so `//` gives what it falls back to.
    assert_eq!(at("E2"), "0");
    assert_eq!(at("E3"), "IF(ISBLANK(D2),0,D2)");
    // A range from a fixed row to this one.
    assert_eq!(at("F4"), "SUM($D$2:D4)");
    // A row found by a condition, and what to give when there is none.
    assert_eq!(
        at("G2"),
        "IFERROR(INDEX('Rates'!$B$2:$B$3,MATCH(A2,'Rates'!$A$2:$A$3,0)),1)"
    );
    // A constant is written as what it is.
    assert_eq!(at("H2"), "D2*(1-0.2)+G2");
    // Rows by number, from the start and from the end.
    assert_eq!(at("I3"), "$D$2+$D$4");
    // A workbook counts dates in days, as a file does, and times in days
    // where a file counts seconds.
    assert_eq!(at("J2"), "B2+7");
    assert_eq!(at("K3"), "ROUND((C3-TIME(9,0,0))*86400,0)");
    assert_eq!(cells(bytes, "T")[2][10], Data::Float(30_600.0));
    assert_eq!(at("L2"), "COUNTIFS('Rates'!$A$2:$A$3,A2)");
    assert_eq!(at("M2"), "SUMIFS('Rates'!$B$2:$B$3,'Rates'!$A$2:$A$3,A2)");
    // What a workbook cannot say is written as its value, and reported.
    assert_eq!(at("N2"), "");
    assert_eq!(cells(bytes, "T")[1][13], Data::Bool(true));
    assert!(
        out.notices.contains(
            &"values were written in place of formulas that use `in` or a range: T.Odd".to_string()
        ),
        "{:?}",
        out.notices
    );

    // A sheet exported alone cannot read another: its values are written.
    let alone = xlsx::export("t.omx", file, Some("T"), options()).unwrap();
    assert_eq!(formula_at(&alone.files[0].bytes, "T", "G2"), "");
    assert_eq!(formula_at(&alone.files[0].bytes, "T", "J2"), "B2+7");
    assert!(
        alone.notices.iter().any(|n| n.starts_with(
            "values were written in place of formulas that read a sheet that is not exported: T.Fee"
        )),
        "{:?}",
        alone.notices
    );
}

#[test]
fn xlsx_writes_a_number_past_a_double_as_text() {
    // A workbook holds its numbers as doubles, and a number whose point
    // sits past where a double reaches is no double at all. Rather than
    // put an infinity in the cell, the write spells the number out.
    // The same holds at the other end: a point far enough to the right
    // leaves a double with nothing but a zero to offer, and a zero is not
    // the number either.
    let file = "sheet T\n\nA | B | C\n= 10**1000 | = 1/10**2000 | 2.5\n";
    let out = xlsx::export("t.omx", file, None, options()).unwrap();
    let rows = cells(&out.files[0].bytes, "T");
    assert_eq!(rows[1][0], Data::String("1e+1000".to_string()));
    assert_eq!(rows[1][1], Data::String("1e-2000".to_string()));
    assert_eq!(rows[1][2], Data::Float(2.5));
    assert!(
        out.notices.contains(
            &"values a workbook has no type for were written as text: T.A, T.B".to_string()
        ),
        "{:?}",
        out.notices
    );
}

#[test]
fn xlsx_writes_one_third_as_the_nearest_double() {
    let file = "sheet T\n\nA | B | C | D\n= 1/3 | 0.5 | true | 2025-01-31\n";
    let out = xlsx::export("t.omx", file, None, options()).unwrap();
    let rows = cells(&out.files[0].bytes, "T");
    assert_eq!(rows[1][0], Data::Float(0.333_333_333_333_333_3));
    assert_eq!(rows[1][1], Data::Float(0.5));
    assert_eq!(rows[1][2], Data::Bool(true));
    assert!(
        matches!(&rows[1][3], Data::DateTime(t) if t.as_f64() == 45_688.0),
        "{:?}",
        rows[1][3]
    );
    assert_eq!(
        out.notices,
        [
            "1 formula written",
            "exact rationals were written as floating point: T.A"
        ]
    );
}

fn workbook() -> Vec<u8> {
    let mut book = Workbook::new();
    let date = Format::new().set_num_format("yyyy-mm-dd");
    let stamp = Format::new().set_num_format("yyyy-mm-dd hh:mm");
    let sales = book.add_worksheet();
    sales.set_name("Sales").unwrap();
    for (c, name) in ["Month", "Revenue", "Unit Cost", "Closed", "Total"]
        .iter()
        .enumerate()
    {
        sales.write_string(0, c as u16, *name).unwrap();
    }
    sales.write_string(1, 0, "Jan").unwrap();
    sales.write_number(1, 1, 19.99).unwrap();
    sales.write_number(1, 2, 6000.0).unwrap();
    sales
        .write_number_with_format(1, 3, 45_688.0, &date)
        .unwrap();
    sales
        .write_formula(1, 4, Formula::new("=B2+C2").set_result("6019.99"))
        .unwrap();
    sales.write_string(2, 0, "Feb").unwrap();
    sales.write_number(2, 1, 0.1).unwrap();
    // A workbook takes the empty cell for nothing, and OMX leaves it empty.
    sales
        .write_formula(2, 4, Formula::new("=B3+C3").set_result("0.1"))
        .unwrap();
    sales
        .write_number_with_format(2, 3, 45_689.5, &stamp)
        .unwrap();
    book.add_worksheet().set_name("Blank").unwrap();
    let people = book.add_worksheet();
    people.set_name("Customers").unwrap();
    people.write_string(0, 0, "Name").unwrap();
    people.write_string(1, 0, "42").unwrap();
    people.write_boolean(1, 1, true).unwrap();
    book.save_to_buffer().unwrap()
}

#[test]
fn xlsx_imports_a_sheet_for_each_worksheet() {
    let out = xlsx::import(&workbook(), options()).unwrap();
    assert_eq!(
        out.source,
        "\
sheet Sales

Closed : Datetime

| Month | Revenue | UnitCost | Closed           | Total                |
| ----- | ------- | -------- | ---------------- | -------------------- |
| Jan   | 19.99   | 6000     | 2025-01-31T00:00 | = Revenue + UnitCost |
| Feb   | 0.1     |          | 2025-02-01T12:00 | 0.1                  |

sheet Customers

Column2 : Boolean

| Name | Column2 |
| ---- | ------- |
| \"42\" | true    |
"
    );
    assert_eq!(
        out.notices,
        [
            "worksheet `Blank` is empty and was left out",
            "`Unit Cost` was renamed `UnitCost`",
            "a column with no name was named `Column2`",
            "worksheet `Sales`: 1 formula was translated",
            "worksheet `Sales`: values were imported in place of formulas, as its translation \
             gives another value than the workbook holds: E3",
        ]
    );
    assert_eq!(eval(&out.source, "Sales[Total; 0]"), "6019.99");
    lints(&out.source);
    assert_eq!(eval(&out.source, "Sales[Revenue; 0] * 100"), "1999");
    assert_eq!(eval(&out.source, "Sales[Revenue; 1] * 10"), "1");
    assert!(!out.source.contains("B2"));
}

/// A workbook with formulas: a column filled down, a running total, a
/// total under the rows, and a worksheet that reads another.
fn ledger() -> Vec<u8> {
    let mut book = Workbook::new();
    let sales = book.add_worksheet();
    sales.set_name("Sales").unwrap();
    let headers = [
        "Month", "Revenue", "Cost", "Profit", "Running", "Margin", "Note",
    ];
    for (c, name) in headers.iter().enumerate() {
        sales.write_string(0, c as u16, *name).unwrap();
    }
    let rows = [
        ("Jan", 10_000.0_f64, 6_000.0_f64),
        ("Feb", 12_000.0, 7_000.0),
        ("Mar", 13_200.0, 7_500.0),
    ];
    let mut running = 0.0;
    for (i, (month, revenue, cost)) in rows.iter().enumerate() {
        let (r, n) = (i as u32 + 1, i + 2);
        running += revenue - cost;
        let formula = |f: String, result: String| Formula::new(f).set_result(result);
        sales.write_string(r, 0, *month).unwrap();
        sales.write_number(r, 1, *revenue).unwrap();
        sales.write_number(r, 2, *cost).unwrap();
        sales
            .write_formula(
                r,
                3,
                formula(format!("=B{n}-C{n}"), (revenue - cost).to_string()),
            )
            .unwrap();
        sales
            .write_formula(
                r,
                4,
                formula(format!("=SUM(D$2:D{n})"), running.to_string()),
            )
            .unwrap();
        let margin = (revenue - cost) / revenue;
        sales
            .write_formula(
                r,
                5,
                formula(
                    format!("=ROUND(D{n}/B{n},4)"),
                    ((margin * 1e4).round() / 1e4).to_string(),
                ),
            )
            .unwrap();
        sales
            .write_formula(r, 6, formula(format!("=A{n}&\"!\""), format!("{month}!")))
            .unwrap();
    }
    let summary = book.add_worksheet();
    summary.set_name("Year End").unwrap();
    summary.write_string(0, 0, "Item").unwrap();
    summary.write_string(0, 1, "Value").unwrap();
    let lines = [
        ("Revenue", "=SUM(Sales!B2:B4)", "35200"),
        ("Profit", "=SUM(Sales!D:D)", "14700"),
        ("Best", "=MAX(Sales!D2:D4)", "5700"),
        ("First two", "=Sales!D2+Sales!D3", "9000"),
        ("Share", "=B3/B2", "0.4176136363636364"),
        ("Good", "=IF(B3>10000,\"yes\",\"no\")", "yes"),
        ("Looked up", "=VLOOKUP(\"Feb\",Sales!A2:D4,4,FALSE)", "5000"),
        ("Wrong", "=B2*2", "1"),
    ];
    for (i, (item, formula, result)) in lines.iter().enumerate() {
        summary.write_string(i as u32 + 1, 0, *item).unwrap();
        summary
            .write_formula(i as u32 + 1, 1, Formula::new(*formula).set_result(*result))
            .unwrap();
    }
    book.save_to_buffer().unwrap()
}

#[test]
fn xlsx_round_to_a_number_of_places() {
    let file = "\
sheet T

X     | Cents | Whole | Hundreds
1.005 | *     | *     | *
-1250 | *     | *     | *

Cents    := round(X, 2)
Whole    := round(X)
Hundreds := round(X / 100) * 100
";
    let out = xlsx::export("t.omx", file, None, options()).unwrap();
    let bytes = &out.files[0].bytes;
    let at = |cell: &str| formula_at(bytes, "T", cell);
    // A count of places goes out as the count it is.
    assert_eq!(at("B2"), "ROUND(A2,2)");
    assert_eq!(at("C2"), "ROUND(A2,0)");
    // The nearest hundred is arithmetic in OMX, and goes out as arithmetic.
    assert!(at("D2").starts_with("ROUND(A2/100,0)*"), "{}", at("D2"));
    // And back again.
    let back = xlsx::import(bytes, options()).unwrap().source;
    assert!(back.contains("round(X, 2)"), "{back}");
}

#[test]
fn xlsx_imports_formulas() {
    let out = xlsx::import(&ledger(), options()).unwrap();
    assert_eq!(
        out.source,
        "\
sheet Sales

| Month | Revenue | Cost | Profit | Running | Margin | Note |
| ----- | ------- | ---- | ------ | ------- | ------ | ---- |
| Jan   | 10000   | 6000 | *      | *       | *      | Jan! |
| Feb   | 12000   | 7000 | *      | *       | *      | Feb! |
| Mar   | 13200   | 7500 | *      | *       | *      | Mar! |

Profit  := Revenue - Cost
Running := [Profit; 0..*].sum()
Margin  := round(Profit / Revenue, 4)

sheet YearEnd

| Item      | Value                                          |
| --------- | ---------------------------------------------- |
| Revenue   | = Sales.Revenue.sum()                          |
| Profit    | = Sales.Profit.sum()                           |
| Best      | = Sales.Profit.max()                           |
| First two | = Sales[Profit; 0] + Sales[Profit; 1]          |
| Share     | = [Value; *-3] / [Value; *-4]                  |
| Good      | = if [Value; *-4] > 10000 then \"yes\" else \"no\" |
| Looked up | 5000                                           |
| Wrong     | 1                                              |
"
    );
    assert_eq!(
        out.notices,
        [
            "`Year End` was renamed `YearEnd`",
            "worksheet `Sales`: 9 formulas were translated",
            "worksheet `Sales`: values were imported in place of formulas, as it joins text \
             with `&`: G2, G3, G4",
            "worksheet `Year End`: 6 formulas were translated",
            "worksheet `Year End`: values were imported in place of formulas, as `VLOOKUP` has \
             no equivalent: B8",
            "worksheet `Year End`: values were imported in place of formulas, as its \
             translation gives another value than the workbook holds: B9",
        ]
    );
    lints(&out.source);
    // The file works out what the workbook held.
    assert_eq!(eval(&out.source, "Sales.Running"), "[4000, 9000, 14700]");
    assert_eq!(eval(&out.source, "Sales.Margin"), "[0.4, 0.4167, 0.4318]");
    assert_eq!(
        eval(&out.source, "YearEnd[Value; 4]"),
        "0.417613636363636363636363636364"
    );
    assert_eq!(eval(&out.source, "YearEnd[Value; 5]"), "yes");
}

#[test]
fn xlsx_round_trips_values() {
    let out = xlsx::export("budget.omx", BUDGET, None, options()).unwrap();
    let back = xlsx::import(&out.files[0].bytes, options()).unwrap();
    lints(&back.source);
    assert_eq!(eval(&back.source, "Sales.Profit.sum()"), "9000");
    assert_eq!(eval(&back.source, "Summary[Value; 0]"), "22000");
}

#[test]
fn not_a_workbook() {
    assert!(
        xlsx::import(b"hello", options())
            .unwrap_err()
            .starts_with("it is not a workbook")
    );
    assert_eq!(csv::import("t", b"").unwrap_err(), "it has no header row");
}

// ---- JSON ------------------------------------------------------------------

const KINDS: &str = "\
sheet T

N : Float
F : Decimal

Name | Count | Price | Rate | N    | F   | On    | Day        | At    | Gap | Bad
a    | 7     | 19.99 | 20%  | 1e20 | 1/3 | true  | 2025-01-31 | 09:30 |     | = Inf.Int
\"42\" | -3    | 0.125 | -5%  | Inf  | 1/8 | false | 2025-02-01 | 17:05 | x   | 1
";

fn json_of(text: &str, sheet: Option<&str>) -> Exported {
    json::export("file.omx", text, sheet, options()).expect("the file exports")
}

#[test]
fn json_holds_every_sheet_as_rows_by_column_name() {
    let out = json_of(BUDGET, None);
    assert_eq!(out.files.len(), 1);
    assert_eq!(
        text_of(&out, 0),
        "{\n  \"Sales\": [\n    \
         {\"Month\": \"Jan\", \"Revenue\": 10000, \"Cost\": 6000, \"Profit\": 4000, \"Share\": 1333.33333333333333333333333333},\n    \
         {\"Month\": \"Feb\", \"Revenue\": 12000, \"Cost\": 7000, \"Profit\": 5000, \"Share\": 1666.66666666666666666666666667}\n  ],\n  \
         \"Summary\": [\n    {\"Item\": \"Revenue\", \"Value\": 22000}\n  ]\n}\n"
    );
    assert!(out.notices.is_empty(), "{:?}", out.notices);
    // A sheet that is named is its rows alone.
    assert_eq!(
        text_of(&json_of(BUDGET, Some("Summary")), 0),
        "[\n  {\"Item\": \"Revenue\", \"Value\": 22000}\n]\n"
    );
}

#[test]
fn json_writes_each_kind_of_value() {
    let out = json_of(KINDS, Some("T"));
    assert_eq!(
        text_of(&out, 0),
        "[\n  \
         {\"Name\": \"a\", \"Count\": 7, \"Price\": 19.99, \"Rate\": 0.2, \"N\": 1e+20, \"F\": 0.333333333333333333333333333333, \
         \"On\": true, \"Day\": \"2025-01-31\", \"At\": \"09:30\", \"Gap\": null, \"Bad\": null},\n  \
         {\"Name\": \"42\", \"Count\": -3, \"Price\": 0.125, \"Rate\": -0.05, \"N\": \"Inf\", \"F\": 0.125, \
         \"On\": false, \"Day\": \"2025-02-01\", \"At\": \"17:05\", \"Gap\": \"x\", \"Bad\": 1}\n]\n"
    );
    assert_eq!(
        out.notices,
        ["cells that could not be calculated were left empty: T.Bad"]
    );
}

#[test]
fn json_comes_back_as_the_values_it_was_written_from() {
    let written = text_of(&json_of(KINDS, None), 0);
    let back = json::import("kinds", written.as_bytes()).unwrap();
    lints(&back.source);
    assert_eq!(eval(&back.source, "T.Count"), "[7, -3]");
    assert_eq!(eval(&back.source, "T.Price.sum()"), "20.115");
    assert_eq!(eval(&back.source, "T.N"), "[1e+20, Inf]");
    assert_eq!(eval(&back.source, "T.On"), "[true, false]");
    assert_eq!(eval(&back.source, "T[Day; 1] - T[Day; 0]"), "1");
    assert_eq!(eval(&back.source, "T.At"), "[09:30, 17:05]");
    // A number in quotes is text, and null is empty.
    assert_eq!(eval(&back.source, "T.Name"), "[\"a\", \"42\"]");
    assert_eq!(eval(&back.source, "T.Gap"), "[empty, \"x\"]");
}

#[test]
fn json_reads_a_list_of_rows_as_one_sheet() {
    let rows = r#"[
        {"Unit Price": 19.99, "Qty": 2},
        {"Qty": 10000000000000000000000, "Note": "late", "Tags": ["a", 1]},
        ["not", "a", "row"]
    ]"#;
    let why = json::import("orders", rows.as_bytes()).unwrap_err();
    assert_eq!(
        why,
        "row 3 of `orders` is not an object of values by column name"
    );

    let rows = rows.replace(",\n        [\"not\", \"a\", \"row\"]", "");
    let back = json::import("orders", rows.as_bytes()).unwrap();
    lints(&back.source);
    assert!(back.source.starts_with("sheet orders\n"), "{}", back.source);
    // Columns are in the order they are first met; one a row lacks is empty.
    assert_eq!(eval(&back.source, "orders.UnitPrice"), "[19.99, empty]");
    assert_eq!(
        eval(&back.source, "orders[Qty; 1] + 1"),
        "10000000000000000000001"
    );
    assert_eq!(
        eval(&back.source, "orders.Tags"),
        "[empty, \"[\\\"a\\\",1]\"]"
    );
    assert_eq!(
        back.notices,
        [
            "`orders`: lists and objects inside a row were kept as text",
            "`Unit Price` was renamed `UnitPrice`"
        ]
    );

    // Rows as lists: the first is the names.
    let lists = r#"[["Item", "Cost"], ["tea", 1.5e3], ["milk", null]]"#;
    let back = json::import("shop", lists.as_bytes()).unwrap();
    lints(&back.source);
    assert_eq!(eval(&back.source, "shop.Cost"), "[1500, empty]");

    for bad in ["{", "42", "{}", "[]", r#"{"T": 1}"#] {
        assert!(json::import("x", bad.as_bytes()).is_err(), "{bad}");
    }
}

// ---- Parquet ---------------------------------------------------------------

fn parquet_of(text: &str, sheet: Option<&str>) -> Exported {
    parquet::export("file.omx", text, sheet, options()).expect("the file exports")
}

#[test]
fn parquet_writes_a_file_for_each_sheet() {
    let out = parquet_of(BUDGET, None);
    assert_eq!(out.files.len(), 2);
    assert_eq!(out.files[0].sheet.as_deref(), Some("Sales"));
    assert_eq!(out.files[1].sheet.as_deref(), Some("Summary"));
    // Parquet files begin and end with its magic.
    for file in &out.files {
        assert_eq!(&file.bytes[..4], b"PAR1", "not a Parquet file");
        assert_eq!(&file.bytes[file.bytes.len() - 4..], b"PAR1");
    }
}

#[test]
fn parquet_takes_each_column_type_from_the_schema() {
    let file = "sheet T\n\n\
                N : Int\nP : Decimal\nR : Percent\nF : Float\nS : String\n\
                B : Boolean\nD : Date\nA : Time\nW : Datetime\n\n\
                N | P | R | F | S | B | D | A | W\n\
                7 | 19.99 | 20% | 1.5e3 | hi | true | 2025-01-31 | 09:30 | 2025-01-31T09:30\n\
                -3 | 0.125 | 12.5% | Inf | \"42\" | false | 2025-02-01 | 17:05 | 2025-02-01T17:05\n";
    let out = parquet_of(file, None);
    assert_eq!(out.files.len(), 1);
    let back = parquet::import("t", &out.files[0].bytes).expect("imports");
    lints(&back.source);
    // A schema line is written where re-reading the text would give
    // something else: a `Float`, a `Boolean` and the dates and times. A
    // whole number and a decimal need none, since their text reads back as
    // what they are.
    for line in [
        "F : Float",
        "B : Boolean",
        "D : Date",
        "A : Time",
        "W : Datetime",
    ] {
        assert!(
            back.source.contains(line),
            "{line} missing:\n{}",
            back.source
        );
    }
    // And the values with them.
    assert_eq!(eval(&back.source, "t[N; 0]"), "7");
    assert_eq!(eval(&back.source, "t[P; 0]"), "19.99");
    assert_eq!(eval(&back.source, "t[D; 1]"), "2025-02-01");
    assert_eq!(eval(&back.source, "t[A; 0]"), "09:30");
    assert_eq!(eval(&back.source, "t[W; 1]"), "2025-02-01T17:05");
    assert_eq!(eval(&back.source, "t[B; 0]"), "true");
}

#[test]
fn parquet_meta_brings_the_formulas_back() {
    let file = "zone Europe/London\n\n\
                const Rate = 20%\n\
                fn Twice(x) = x * 2\n\n\
                sheet Sales\n\n\
                Revenue : Decimal['.2f]\n\
                Code    : Hex\n\n\
                | Month | Revenue | Code | Tax | Double |\n\
                | ----- | ------- | ---- | --- | ------ |\n\
                | Jan   | 100     | 0xff | *   | *      |\n\
                | Feb   | 120.5   | 0x10 | *   | *      |\n\n\
                Tax    := Revenue * Rate\n\
                Double := Twice(Revenue)\n";
    let out = parquet::export_meta("file.omx", file, None, options()).expect("the file exports");
    let back = parquet::import("t", &out.files[0].bytes).expect("imports");
    lints(&back.source);
    // The computed columns are formulas again, with the constant, the
    // function, the zone and the column types they were written with.
    for line in [
        "zone Europe/London",
        "const Rate = 20%",
        "fn Twice(x) = x * 2",
        "sheet Sales",
        "Revenue : Decimal['.2f]",
        "Code    : Hex",
        "Tax    := Revenue * Rate",
        "Double := Twice(Revenue)",
        "| Feb   | 120.5   | 0x10 | *   | *      |",
    ] {
        assert!(
            back.source.contains(line),
            "{line} missing:\n{}",
            back.source
        );
    }
    assert_eq!(eval(&back.source, "Sales[Tax; 1]"), "24.1");
    assert_eq!(eval(&back.source, "Sales[Code; 0]"), "0xff");
    // Without `--meta` it is the values, as before.
    let plain = parquet::import("t", &parquet_of(file, None).files[0].bytes).expect("imports");
    assert!(!plain.source.contains(":="), "{}", plain.source);
}

#[test]
fn parquet_keeps_a_decimal_exact() {
    // A decimal column is written as a Parquet decimal, so every digit of it
    // survives the trip: this is what CSV cannot do.
    let file = "sheet T\n\nA : Decimal\n\nA\n19.99\n0.0009765625\n= 1/3\n";
    let out = parquet_of(file, None);
    let back = parquet::import("t", &out.files[0].bytes).expect("imports");
    lints(&back.source);
    assert_eq!(eval(&back.source, "t[A; 0]"), "19.99");
    assert_eq!(eval(&back.source, "t[A; 1]"), "0.0009765625");
    // A Parquet column has one scale for all its values, so the widest it
    // can use is the narrowest any of them allows: `19.99` needs two digits
    // before the point, which leaves thirty-six after it. A third is written
    // to that scale rather than its own thirty-eight, and the cell it comes
    // back as carries every one of those digits.
    assert!(
        back.source
            .contains("0.333333333333333333333333333333333333"),
        "{}",
        back.source
    );
    // Shown, it is the guarded form: the last eight of the thirty-eight
    // digits are working room, so thirty of them are read.
    assert_eq!(
        eval(&back.source, "t[A; 2]"),
        "0.333333333333333333333333333333"
    );
}

#[test]
fn parquet_leaves_a_failed_cell_empty() {
    let file = "sheet T\n\nA\n= Inf.Int\n1\n";
    let out = parquet_of(file, None);
    assert_eq!(
        out.notices,
        ["cells that could not be calculated were left empty: T.A"]
    );
}

#[test]
fn parquet_writes_null_for_a_value_wider_than_the_column_type() {
    // A number past the digits Polars can name has nowhere to sit in a
    // `Decimal(38, s)`, so its cell is written null and the write says so.
    // The rest of the column is untouched: `2.5` keeps its point rather
    // than being dragged to the one scale the monster would allow.
    let file = "sheet T\n\nA : Decimal\n\nA\n= 10**1000\n2.5\n";
    let out = parquet_of(file, None);
    assert_eq!(
        out.notices,
        ["values too wide for the column's type were written as null: T.A"]
    );
    let back = parquet::import("t", &out.files[0].bytes).expect("imports");
    lints(&back.source);
    assert_eq!(eval(&back.source, "t[A; 0]"), "empty");
    assert_eq!(eval(&back.source, "t[A; 1]"), "2.5");
}

/// Every kind of text goes out as the text it holds, in each format, and
/// comes back as text: no format but `.omx` says what the text was read as.
#[test]
fn a_kind_of_text_goes_out_as_the_text_it_holds() {
    const UUID: &str = "7b4e2c10-5f8a-4c3e-9d21-0a1b2c3d4e5f";
    let file = format!(
        "sheet T\n\nSite : Url\nContact : Email\nRef : Uuid\n\n\
         Site | Contact | Ref\n\
         https://example.com | name@example.com | {UUID}\n"
    );
    lints(&file);

    assert_eq!(
        text_of(&csv_of(&file, None), 0),
        format!("Site,Contact,Ref\nhttps://example.com,name@example.com,{UUID}\n")
    );

    let book = xlsx::export("t.omx", &file, None, options()).unwrap();
    let back = xlsx::import(&book.files[0].bytes, options()).unwrap();
    lints(&back.source);
    assert_eq!(eval(&back.source, "T[Contact; 0]"), "name@example.com");

    let written = text_of(&json_of(&file, None), 0);
    let back = json::import("t", written.as_bytes()).unwrap();
    lints(&back.source);
    assert_eq!(eval(&back.source, "T[Ref; 0]"), UUID);

    // Parquet holds it as the String column it is, so nothing goes out as
    // text that was not text already and no loss is reported.
    let out = parquet_of(&file, None);
    assert!(out.notices.is_empty(), "{:?}", out.notices);
    let back = parquet::import("t", &out.files[0].bytes).unwrap();
    lints(&back.source);
    assert_eq!(eval(&back.source, "t[Site; 0]"), "https://example.com");
    // And no schema line comes back: text reads back as text, and the
    // narrower reading is not Parquet's to hold.
    assert!(!back.source.contains(": Email"), "{}", back.source);
}

#[test]
fn parquet_round_trips_the_examples() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "omx") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let first = parquet::export("file.omx", &text, None, options_beside(dir))
            .expect("the file exports");
        for file in 0..first.files.len() {
            let name = first.files[file].sheet.as_deref().unwrap_or("T");
            let imported = parquet::import(name, &first.files[file].bytes)
                .unwrap_or_else(|e| panic!("{} sheet {name}: {e}", path.display()));
            lints(&imported.source);
            // A second trip changes nothing: the types are carried, so there
            // is nothing left to read differently.
            let second = parquet_of(&imported.source, None);
            let again = parquet::import(name, &second.files[0].bytes).unwrap();
            assert_eq!(
                imported.source,
                again.source,
                "{} sheet {name} is not a fixed point",
                path.display()
            );
            seen += 1;
        }
    }
    assert!(seen > 5, "only {seen} sheets seen");
}

// The scenarios of the `file-format` spec for a dataframe-backed table: a
// sheet whose rows live in Parquet files rather than in its own text.

/// Three columns and two rows, to be written out as the file a table reads.
const ROWS: &str = "\
sheet Sales

Month | Revenue | Cost
Jan   | 10000   | 6000
Feb   | 12000   | 7000
";

/// A table over those three columns, with a formula over two of them.
const TABLE: &str = "\
table Sales := dataframe[\"sales.parquet\"]

Month   : String
Revenue : Int
Cost    : Int

Profit := Revenue - Cost
";

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `ROWS` as a Parquet file in `dir`, under `name`.
fn parquet_file(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let exported = parquet_of(ROWS, Some("Sales"));
    let path = dir.join(name);
    std::fs::write(&path, &exported.files[0].bytes).unwrap();
    path
}

/// A file read with its tables filled in from `dir`.
fn with_tables(dir: &std::path::Path, text: &str) -> omasheet_engine::Outcome {
    let options = omasheet_engine::Options {
        base: Some(dir.to_path_buf()),
        ..options()
    };
    omasheet_engine::view_with("file.omx", text, options)
}

#[test]
fn a_table_is_declared_and_read() {
    let dir = scratch("table");
    parquet_file(&dir, "sales.parquet");
    let out = with_tables(&dir, TABLE);
    assert!(out.ok(), "{:?}", out.errors);
    // The rows came from the file, and the formula ran over them.
    assert!(out.output.contains("Jan"), "{}", out.output);
    assert!(out.output.contains("4000"), "{}", out.output);
    assert!(out.output.contains("5000"), "{}", out.output);
}

/// D62: every path that builds an engine reads a table's rows, not just
/// `view_with`. The export path built one without them, so a table went
/// out as a sheet with no rows at all.
#[test]
fn a_table_is_exported_with_the_rows_it_reads() {
    let dir = scratch("table_export");
    parquet_file(&dir, "sales.parquet");
    let options = Options {
        base: Some(dir.to_path_buf()),
        ..options()
    };
    let out = csv::export("file.omx", TABLE, None, options).expect("the table exports");
    let text = text_of(&out, 0);
    // The rows the table reads, and the formula over them.
    assert!(text.contains("Month,Revenue,Cost,Profit"), "{text}");
    assert!(text.contains("Jan,10000,6000,4000"), "{text}");
    assert!(text.contains("Feb,12000,7000,5000"), "{text}");
}

#[test]
fn a_sheet_of_formulas_alone_is_as_long_as_the_table_it_reads() {
    let dir = scratch("table_rowless");
    parquet_file(&dir, "sales.parquet");
    // A table's own length is not known until its file is read, so a sheet
    // that holds no rows of its own takes its length from the table only
    // then.
    let text = format!(
        "{TABLE}\nsheet Margin\n\nTake := Sales[Revenue; *] - Sales[Cost; *]\nAll := Sales.Revenue.sum()\n"
    );
    let out = with_tables(&dir, &text);
    assert!(out.ok(), "{:?}", out.errors);
    let margin = out.output.split("sheet Margin").nth(1).unwrap();
    assert_eq!(
        margin, "\n\nTake | All\n-----|------\n4000 | 22000\n5000 | 22000\n",
        "{margin}"
    );
}

#[test]
fn a_computed_column_over_a_table_reads_its_columns() {
    let dir = scratch("table-sum");
    parquet_file(&dir, "sales.parquet");
    // An aggregate reads the whole column, so the row count must be the
    // file's and not the text's.
    let text = format!("{TABLE}\nconst Total = Sales.Revenue.sum()\n");
    let out = with_tables(&dir, &text);
    assert!(out.ok(), "{:?}", out.errors);
    assert!(out.output.contains("22000"), "{}", out.output);
}

#[test]
fn a_missing_source_is_reported() {
    let dir = scratch("table-missing");
    let out = with_tables(&dir, TABLE);
    assert!(!out.ok());
    assert!(
        out.errors.iter().any(|e| e.contains("sales.parquet")),
        "{:?}",
        out.errors
    );
}

#[test]
fn a_changed_source_is_reported() {
    let dir = scratch("table-changed");
    parquet_file(&dir, "sales.parquet");
    // The file has a `Cost` the schema does not mention.
    let text = TABLE
        .replace("Cost    : Int\n", "")
        .replace("Profit := Revenue - Cost\n", "");
    let out = with_tables(&dir, &text);
    assert!(!out.ok());
    assert!(
        out.errors.iter().any(|e| e.contains("Cost")),
        "{:?}",
        out.errors
    );
}

#[test]
fn a_declared_type_the_file_does_not_hold_is_reported() {
    let dir = scratch("table-type");
    parquet_file(&dir, "sales.parquet");
    let text = TABLE.replace("Revenue : Int", "Revenue : String");
    let out = with_tables(&dir, &text);
    assert!(!out.ok());
    assert!(
        out.errors
            .iter()
            .any(|e| e.contains("Revenue") && e.contains("String")),
        "{:?}",
        out.errors
    );
}

#[test]
fn a_table_with_rows_of_its_own_is_reported() {
    let dir = scratch("table-rows");
    parquet_file(&dir, "sales.parquet");
    let text = format!("{TABLE}\nMonth | Revenue | Cost\nMar   | 1       | 2\n");
    let out = with_tables(&dir, &text);
    assert!(!out.ok());
    assert!(
        out.errors.iter().any(|e| e.contains("rows of its own")),
        "{:?}",
        out.errors
    );
}

#[test]
fn a_source_that_is_not_a_dataframe_is_reported() {
    let dir = scratch("table-source");
    let out = with_tables(&dir, "table Sales := 1 + 1\n");
    assert!(!out.ok());
    assert!(
        out.errors
            .iter()
            .any(|e| e.contains("does not say what to read")),
        "{:?}",
        out.errors
    );
}

/// A document over `text`, reading its tables from `dir`, as the app does.
fn document(dir: &std::path::Path, text: &str) -> omasheet_engine::Document {
    let mut doc = omasheet_engine::Document::from_text(text);
    doc.set_base(dir);
    doc
}

#[test]
fn a_document_shows_a_table_and_takes_no_edit() {
    let dir = scratch("table-doc");
    parquet_file(&dir, "sales.parquet");
    let mut doc = document(&dir, TABLE);

    // The rows of the file are there to look at, formula and all.
    let sheet = &doc.snapshot().sheets[0];
    assert_eq!(sheet.source.as_deref(), Some("sales.parquet"));
    assert_eq!(sheet.rows.len(), 2);
    assert_eq!(sheet.rows[0][0].display, "Jan");
    assert_eq!(sheet.rows[0][3].display, "4000");

    // Nothing of it can be edited, and the text is left as it was.
    let before = doc.text().to_string();
    let why =
        "table `Sales` is a read-only view of `sales.parquet`: import that file to make edits";
    assert!(!doc.set_cell(0, 0, 0, "Mar"));
    assert!(!doc.insert_rows(0, 0, 1));
    assert!(!doc.delete_rows(0, 0, 1));
    assert!(!doc.set_column_types(0, &[1], Some("Float")));
    assert!(!doc.set_column_formula(0, 3, "Revenue"));
    assert_eq!(doc.add_column(0, "Extra").unwrap_err(), why);
    assert_eq!(doc.insert_column(0, 0, "Extra").unwrap_err(), why);
    assert_eq!(doc.rename_column(0, 0, "When").unwrap_err(), why);
    assert_eq!(doc.delete_columns(0, 0, 1).unwrap_err(), why);
    assert_eq!(doc.move_rows(0, 0, 1, 1).unwrap_err(), why);
    assert_eq!(doc.move_columns(0, 0, 1, 1).unwrap_err(), why);
    assert_eq!(
        doc.add_computed(0, "Twice", "Revenue * 2").unwrap_err(),
        why
    );
    assert_eq!(doc.text(), before);
}

#[test]
fn a_sheet_of_its_own_rows_is_edited_as_before() {
    let dir = scratch("table-own");
    let mut doc = document(&dir, ROWS);
    assert_eq!(doc.snapshot().sheets[0].source, None);
    assert!(doc.set_cell(0, 0, 0, "Mar"));
    assert!(doc.text().contains("Mar"), "{}", doc.text());
}

/// A Parquet file holding a column under a name no formula could write.
fn odd_named_file(dir: &std::path::Path) -> std::path::PathBuf {
    use polars::prelude::*;
    let mut df = df![
        "Month" => ["Jan", "Feb"],
        "Units/Boxes Sold" => [3i64, 4],
    ]
    .unwrap();
    let path = dir.join("odd.parquet");
    let mut file = std::fs::File::create(&path).unwrap();
    ParquetWriter::new(&mut file).finish(&mut df).unwrap();
    path
}

#[test]
fn a_column_is_read_under_the_name_the_file_holds_it_by() {
    let dir = scratch("table-from");
    odd_named_file(&dir);
    let text = "\
table Sales := dataframe[\"odd.parquet\"]

Month : String
Units : Int from \"Units/Boxes Sold\"

Double := Units * 2

const Total = Sales.Units.sum()
";
    let out = with_tables(&dir, text);
    assert!(out.ok(), "{:?}", out.errors);
    // The column is there under its own name, and the formula reads it.
    assert!(out.output.contains("Units"), "{}", out.output);
    assert!(out.output.contains("const Total = 7"), "{}", out.output);
    assert!(
        out.output.contains("Jan   |     3 |      6"),
        "{}",
        out.output
    );
}

#[test]
fn a_column_the_schema_does_not_name_says_how_to_name_it() {
    let dir = scratch("table-from-missing");
    odd_named_file(&dir);
    let text = "table Sales := dataframe[\"odd.parquet\"]\n\nMonth : String\n";
    let out = with_tables(&dir, text);
    assert!(!out.ok());
    // The help offers the one form that can name it, under a name made
    // from the file's own rather than a placeholder.
    assert!(
        out.errors
            .iter()
            .any(|e| e.contains("UnitsBoxesSold : Integer from \"Units/Boxes Sold\"")),
        "{:?}",
        out.errors
    );
}

#[test]
fn a_from_on_a_sheet_of_its_own_rows_is_reported() {
    let dir = scratch("table-from-sheet");
    let text = "sheet Sales\n\nUnits : Int from \"Units/Boxes Sold\"\n\nUnits\n3\n";
    let out = with_tables(&dir, text);
    assert!(!out.ok());
    assert!(
        out.errors.iter().any(|e| e.contains("reads no file")),
        "{:?}",
        out.errors
    );
}

#[test]
fn parquet_imports_a_null_column_as_empty_cells() {
    // Parquet names a whole column `null` where it has no type of its own.
    // There is no type line to write for it, so the cells are left empty.
    let dir = scratch("import-null");
    let path = null_column_file(&dir);
    let bytes = std::fs::read(path).unwrap();
    let back = parquet::import("t", &bytes).expect("imports");
    lints(&back.source);
    assert!(back.source.contains("| Month | Note |"), "{}", back.source);
    // No type line: there is no type the column could be given.
    assert!(!back.source.contains(" : "), "{}", back.source);
    for row in ["| Jan   |      |", "| Feb   |      |"] {
        assert!(back.source.contains(row), "{row} missing:\n{}", back.source);
    }
}

/// A Parquet file holding a column of nothing but nulls: the dtype Polars
/// gives a column that has no type of its own.
fn null_column_file(dir: &std::path::Path) -> std::path::PathBuf {
    use polars::prelude::*;
    let mut df = DataFrame::new(vec![
        Series::new("Month".into(), ["Jan", "Feb"]).into_column(),
        Series::new_null("Note".into(), 2).into_column(),
    ])
    .unwrap();
    let path = dir.join("null.parquet");
    let mut file = std::fs::File::create(&path).unwrap();
    ParquetWriter::new(&mut file).finish(&mut df).unwrap();
    path
}

#[test]
fn a_column_of_nulls_fits_the_declared_type() {
    let dir = scratch("table-null");
    null_column_file(&dir);
    // The file holds nothing in `Note`, so it contradicts no declaration:
    // the type is the text's to say.
    let text = "\
table Sales := dataframe[\"null.parquet\"]

Month : String
Note  : String
";
    let out = with_tables(&dir, text);
    assert!(out.ok(), "{:?}", out.errors);
    assert!(out.output.contains("Note"), "{}", out.output);
    assert!(out.output.contains("Jan"), "{}", out.output);
}

#[test]
fn a_column_of_nulls_the_schema_does_not_name_is_said_without_a_type() {
    let dir = scratch("table-null-missing");
    null_column_file(&dir);
    let text = "table Sales := dataframe[\"null.parquet\"]\n\nMonth : String\n";
    let out = with_tables(&dir, text);
    assert!(!out.ok());
    // There is no type to offer, and `Any` is not one that can be written.
    assert!(
        out.errors
            .iter()
            .any(|e| e.contains("add `Note`") && e.contains("the type the file will hold")),
        "{:?}",
        out.errors
    );
    assert!(
        !out.errors.iter().any(|e| e.contains("Any")),
        "{:?}",
        out.errors
    );
}

#[test]
fn a_declared_type_the_file_fits_is_read_as_declared() {
    let dir = scratch("table-wider");
    parquet_file(&dir, "sales.parquet");
    // The file holds `Revenue` as an integer, and a `Decimal` holds every
    // integer: the declaration stands, and the column is read as one.
    let text = TABLE.replace("Revenue : Int", "Revenue : Decimal");
    let out = with_tables(&dir, &text);
    assert!(out.ok(), "{:?}", out.errors);
    assert!(out.output.contains("4000"), "{}", out.output);
}

#[test]
fn a_declared_type_narrower_than_the_file_is_reported() {
    let dir = scratch("table-narrower");
    // A decimal is not held by an `Integer`, so the narrower declaration is a
    // mismatch where the wider one was not.
    let exported = parquet_of(
        "sheet Prices\n\nMonth | Price\nJan   | 19.99\n",
        Some("Prices"),
    );
    std::fs::write(dir.join("price.parquet"), &exported.files[0].bytes).unwrap();
    let text = "\
table Prices := dataframe[\"price.parquet\"]

Month : String
Price : Int
";
    let out = with_tables(&dir, text);
    assert!(!out.ok());
    assert!(
        out.errors
            .iter()
            .any(|e| e.contains("Price") && e.contains("Decimal")),
        "{:?}",
        out.errors
    );
}

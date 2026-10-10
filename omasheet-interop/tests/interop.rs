// Copyright (c) 2026 Stephen Roe

//! The scenarios of the `csv-interop` and `xlsx-interop` specs.

use calamine::{Data, Reader, open_workbook_auto_from_rs};
use omasheet_engine::Options;
use omasheet_interop::{Datum, Exported, csv, xlsx};
use rust_xlsxwriter::{Format, Formula, Workbook};
use std::io::Cursor;

const BUDGET: &str = "\
const TaxRate = 20%

table Sales

Month | Revenue | Cost | Profit | Share
Jan   | 10000   | 6000 | *      | *
Feb   | 12000   | 7000 | *      | *

Profit := Revenue - Cost
Share  := Profit / 3

table Summary

Item    | Value
Revenue | = Sales.Revenue.sum()
";

fn options() -> Options {
    Options {
        now: Some(1_750_000_000),
        ..Options::default()
    }
}

fn csv_of(text: &str, table: Option<&str>) -> Exported {
    csv::export("sheet.omx", text, table, options()).expect("the sheet exports")
}

fn text_of(exported: &Exported, file: usize) -> String {
    String::from_utf8(exported.files[file].bytes.clone()).unwrap()
}

/// The value of `expr` over a sheet, as `omasheet eval --exact` prints it.
fn eval(sheet: &str, expr: &str) -> String {
    let options = Options {
        exact: true,
        ..options()
    };
    let out = omasheet_engine::eval_with(Some(("sheet.omx", sheet)), "<expr>", expr, options);
    assert!(out.ok(), "{:?}\n{sheet}", out.errors);
    out.output.trim().to_string()
}

fn lints(source: &str) {
    let out = omasheet_engine::lint("imported.omx", source);
    assert!(out.ok(), "{}\n{source}", out.errors.join("\n"));
}

// ---- CSV export ----------------------------------------------------------

#[test]
fn csv_writes_a_file_for_each_table() {
    let out = csv_of(BUDGET, None);
    let tables: Vec<_> = out.files.iter().map(|f| f.table.as_deref()).collect();
    assert_eq!(tables, [Some("Sales"), Some("Summary")]);
    assert_eq!(text_of(&out, 1), "Item,Value\nRevenue,22000\n");
}

#[test]
fn csv_of_one_table_has_values_for_computed_columns() {
    let out = csv_of(BUDGET, Some("Sales"));
    assert_eq!(out.files.len(), 1);
    assert_eq!(out.files[0].table, None);
    assert_eq!(
        text_of(&out, 0),
        "Month,Revenue,Cost,Profit,Share\n\
         Jan,10000,6000,4000,1333.3333333333333\n\
         Feb,12000,7000,5000,1666.6666666666667\n"
    );
    assert_eq!(
        out.notices,
        ["exact rationals that do not end were written as decimals: Sales.Share"]
    );
}

#[test]
fn csv_keeps_exact_what_ends() {
    let sheet = "table T\n\nA\n19.99\n= 2 ** 100\n= 1/3\n= 1/1024\n";
    let out = csv_of(sheet, None);
    assert_eq!(
        text_of(&out, 0),
        "A\n19.99\n1267650600228229401496703205376\n0.3333333333333333\n0.0009765625\n"
    );
    assert_eq!(out.notices.len(), 1);
}

#[test]
fn csv_quotes_as_rfc_4180() {
    let sheet = "table T\n\nWho : Text\n\nWho\n\"Smith, J \\\"Jay\\\"\"\n";
    assert_eq!(
        text_of(&csv_of(sheet, None), 0),
        "Who\n\"Smith, J \"\"Jay\"\"\"\n"
    );
}

#[test]
fn csv_leaves_a_failed_cell_empty_and_says_so() {
    let sheet = "table T\n\nA | B\n1 | = 1 / 0\n";
    let out = csv_of(sheet, None);
    assert_eq!(text_of(&out, 0), "A,B\n1,\n");
    assert_eq!(
        out.notices,
        ["cells that could not be calculated were left empty: T.B"]
    );
}

#[test]
fn a_sheet_that_does_not_compile_is_not_exported() {
    let errors = csv::export("bad.omx", "table T\n\nA\n= Nope\n", None, options()).unwrap_err();
    assert!(errors[0].starts_with("bad.omx:4:3: error:"), "{errors:?}");
    let errors = csv::export("s.omx", BUDGET, Some("Costs"), options()).unwrap_err();
    assert!(errors[0].contains("no table `Costs`"), "{errors:?}");
}

// ---- CSV import ----------------------------------------------------------

#[test]
fn csv_imports_a_table_named_after_the_file() {
    let out = csv::import("sales", b"Month,Revenue,Cost\nJan,100,60.5\nFeb,120,\n").unwrap();
    assert_eq!(
        out.source,
        "table sales\n\nMonth | Revenue | Cost\nJan   | 100     | 60.5\nFeb   | 120     |\n"
    );
    assert!(out.notices.is_empty(), "{:?}", out.notices);
    lints(&out.source);
    assert_eq!(eval(&out.source, "sales.Revenue.sum()"), "220");
}

#[test]
fn csv_reads_every_digit() {
    let out = csv::import("t", b"A\n0.12345678901234567890123\n").unwrap();
    assert_eq!(
        eval(&out.source, "t[A; 0] * 100000000000000000000000"),
        "12345678901234567890123"
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
            .contains("\n\nWhen : Date\nOk   : Bool\nAt   : Time\n\n"),
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
        out.source.starts_with("table _2024Sales\n"),
        "{}",
        out.source
    );
    assert!(
        out.source
            .contains("UnitPrice | UnitPrice_2 | Column3 | in_\n"),
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
    let data = "Note\na|b\n\"say \"\"hi\"\"\"\n# not a comment\ntable Sales\n---\n padded \n\"two\nlines\"\n";
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
        let first = csv_of(&text, None);
        for file in 0..first.files.len() {
            let name = first.files[file].table.as_deref().unwrap_or("T");
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
            assert_eq!(was.len(), now.len(), "{} table {name}", path.display());
            for (a, b) in was.iter().zip(&now) {
                assert!(
                    a == b || matches!(Datum::read(a), Datum::Lit(..)),
                    "{} table {name}: `{a}` came back `{b}`\n{}",
                    path.display(),
                    imported.source
                );
            }
            // And a second trip changes nothing.
            let again = csv::import(name, &second.files[0].bytes).unwrap();
            assert_eq!(
                text_of(&csv_of(&again.source, None), 0),
                text_of(&second, 0),
                "{} table {name}",
                path.display()
            );
            seen += 1;
        }
    }
    assert!(seen > 5, "only {seen} tables");
}

// ---- XLSX ----------------------------------------------------------------

fn cells(bytes: &[u8], sheet: &str) -> Vec<Vec<Data>> {
    let mut book = open_workbook_auto_from_rs(Cursor::new(bytes)).unwrap();
    let range = book.worksheet_range(sheet).unwrap();
    range.rows().map(|r| r.to_vec()).collect()
}

#[test]
fn xlsx_has_a_worksheet_for_each_table() {
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
            "exact rationals were written as floating point: Sales.Share",
        ]
    );
}

#[test]
fn xlsx_writes_one_third_as_the_nearest_double() {
    let sheet = "table T\n\nA | B | C | D\n= 1/3 | 0.5 | true | 2025-01-31\n";
    let out = xlsx::export("t.omx", sheet, None, options()).unwrap();
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
        ["exact rationals were written as floating point: T.A"]
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
fn xlsx_imports_a_table_for_each_worksheet() {
    let out = xlsx::import(&workbook()).unwrap();
    assert_eq!(
        out.source,
        "\
table Sales

Closed : DateTime

Month | Revenue | UnitCost | Closed           | Total
Jan   | 19.99   | 6000     | 2025-01-31T00:00 | 6019.99
Feb   | 0.1     |          | 2025-02-01T12:00 |

table Customers

Column2 : Bool

Name | Column2
\"42\" | true
"
    );
    assert_eq!(
        out.notices,
        [
            "worksheet `Sales`: formulas were not translated, and their values were imported: E2",
            "worksheet `Blank` is empty and was left out",
            "`Unit Cost` was renamed `UnitCost`",
            "a column with no name was named `Column2`",
        ]
    );
    lints(&out.source);
    assert_eq!(eval(&out.source, "Sales[Revenue; 0] * 100"), "1999");
    assert_eq!(eval(&out.source, "Sales[Revenue; 1] * 10"), "1");
    assert!(!out.source.contains("B2"));
}

#[test]
fn xlsx_round_trips_values() {
    let out = xlsx::export("budget.omx", BUDGET, None, options()).unwrap();
    let back = xlsx::import(&out.files[0].bytes).unwrap();
    lints(&back.source);
    assert_eq!(eval(&back.source, "Sales.Profit.sum()"), "9000");
    assert_eq!(eval(&back.source, "Summary[Value; 0]"), "22000");
}

#[test]
fn not_a_workbook() {
    assert!(
        xlsx::import(b"hello")
            .unwrap_err()
            .starts_with("it is not a workbook")
    );
    assert_eq!(csv::import("t", b"").unwrap_err(), "it has no header row");
}

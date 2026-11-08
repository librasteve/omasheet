// Copyright (c) 2026 Stephen Roe

//! `open-a-big-csv` 6.2: a file opened inline, as `omasheet import` writes
//! it, and the same file opened as a table over its Parquet copy answer the
//! same, for every cell and for every aggregate of every column. The corpus
//! is the files that are awkward to read: quoted newlines, CRLF, a BOM, no
//! final newline, empty cells, text that is an OMX literal; the same rows as
//! JSON and as a sheet of a workbook; and a tall file that the aggregates
//! run over on Polars.

use omasheet_engine::Document;
use omasheet_interop::{csv, json, xlsx};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let root = std::env::temp_dir().join(format!("omasheet-differential-{}", std::process::id()));
    ONCE.call_once(|| {
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("cache")).unwrap();
        // SAFETY: once, before any file is read as a table.
        unsafe { std::env::set_var("OMASHEET_CACHE", root.join("cache")) };
    });
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn shown_rows(doc: &Document, sheet: usize) -> Vec<Vec<String>> {
    let n = doc.row_count(sheet);
    doc.rows(sheet, 0, n)
        .into_iter()
        .map(|row| row.into_iter().map(|c| c.display).collect())
        .collect()
}

/// The aggregates it makes sense to ask of a column of this type.
fn aggregates(ty: &str) -> &'static [&'static str] {
    let numeric = ["Integer", "Float", "Decimal", "Percent", "Fraction", "Int"]
        .iter()
        .any(|n| ty.eq_ignore_ascii_case(n));
    let ordered = ["Date", "Datetime", "Time", "Duration"]
        .iter()
        .any(|n| ty.eq_ignore_ascii_case(n));
    if numeric {
        &["sum", "mean", "min", "max", "count", "len"]
    } else if ordered {
        &["min", "max", "count", "len"]
    } else {
        &["count", "len"]
    }
}

/// `inline` is the source `import` wrote, and `view` the source of the
/// table over the same file, whose base is `dir`. Every cell shown is the
/// same, and so is every aggregate of every column, with one known
/// exception, which is returned: a column of percentages or fractions,
/// which a table holds as text, has no aggregate in common with the
/// import's and is returned as `name.type`.
fn agree_but(label: &str, dir: &Path, inline: &str, view: &str) -> Vec<String> {
    let mut table = Document::from_text(view);
    table.set_base(dir);
    assert!(
        table.snapshot().problems.is_empty(),
        "{label}: table: {:?}",
        table.snapshot().problems
    );
    let mut imp = Document::from_text(inline);
    imp.set_base(dir);
    assert!(
        imp.snapshot().problems.is_empty(),
        "{label}: inline: {:?}",
        imp.snapshot().problems
    );
    let at = 0;
    assert_eq!(imp.row_count(at), table.row_count(0), "{label}: rows");
    // What a formula can name is what the view declares, so the table is
    // declared with the types the import found, and the file has to hold
    // to them. The names and the types are each side's.
    let columns: Vec<(String, String)> = imp.snapshot().sheets[at]
        .columns
        .iter()
        .map(|c| (c.name.clone(), c.ty.clone()))
        .collect();
    assert_eq!(
        table.snapshot().sheets[0]
            .columns
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>(),
        columns.iter().map(|c| c.0.clone()).collect::<Vec<_>>(),
        "{label}: columns"
    );
    let held: Vec<String> = table.snapshot().sheets[0]
        .columns
        .iter()
        .map(|c| c.ty.clone())
        .collect();
    let declared = format!(
        "{view}\n{}\n",
        columns
            .iter()
            .zip(&held)
            .map(|((name, _), ty)| format!("{name} : {ty}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let (inline_rows, table_rows) = (shown_rows(&imp, at), shown_rows(&table, 0));
    assert_eq!(inline_rows, table_rows, "{label}: cells");

    // Every aggregate of every column, as a constant on each side.
    let sheet = imp.snapshot().sheets[at].name.clone();
    let table_name = table.snapshot().sheets[0].name.clone();
    let (mut on_inline, mut on_table, mut names) = (String::new(), String::new(), Vec::new());
    let mut differing = Vec::new();
    for ((name, ty), held_ty) in columns.iter().zip(&held) {
        // A column an import reads as numbers and a table as text (a
        // percentage or a fraction, `20%`, `1/2`) has no aggregate in common.
        if aggregates(ty) != aggregates(held_ty) {
            differing.push(format!("{name}.type"));
            continue;
        }
        for func in aggregates(ty) {
            on_inline.push_str(&format!(
                "const A{} = {sheet}.{name}.{func}()\n",
                names.len()
            ));
            on_table.push_str(&format!(
                "const A{} = {table_name}.{name}.{func}()\n",
                names.len()
            ));
            names.push(format!("{name}.{func}"));
        }
    }
    let run = |consts: &str, source: &str| {
        let mut doc = Document::from_text(&format!("{consts}\n{source}"));
        doc.set_base(dir);
        assert!(
            doc.snapshot().problems.is_empty(),
            "{label}: {:?}",
            doc.snapshot().problems
        );
        doc.snapshot()
            .consts
            .iter()
            .map(|c| c.display.clone())
            .collect::<Vec<_>>()
    };
    let (a, b) = (run(&on_inline, inline), run(&on_table, &declared));
    assert_eq!(a.len(), names.len(), "{label}");
    let mut at = 0;
    for (name, ty) in columns.iter().zip(&held).map(|(c, h)| ((&c.0, &c.1), h)) {
        if aggregates(name.1) != aggregates(ty) {
            continue;
        }
        for func in aggregates(name.1) {
            let (x, y) = (&a[at], &b[at]);
            at += 1;
            if x == y {
                continue;
            }
            panic!("{label}: {}.{func}: inline {x}, table {y}", name.0);
        }
    }
    differing
}

/// The CSVs that are awkward to read, by name.
fn awkward_csvs() -> Vec<(&'static str, Vec<u8>)> {
    let plain = "id,name,qty,price,when\n1,Ann,3,1.50,2024-01-05\n2,Bob,,2.25,2024-02-10\n3,Cy,7,,2024-03-15\n";
    vec![
        ("plain", plain.as_bytes().to_vec()),
        ("crlf", plain.replace('\n', "\r\n").into_bytes()),
        (
            "bom",
            [b"\xEF\xBB\xBF".as_slice(), plain.as_bytes()].concat(),
        ),
        (
            "nofinalnewline",
            plain.trim_end_matches('\n').as_bytes().to_vec(),
        ),
        (
            "quotednewlines",
            "id,note,qty\n1,\"line one\nline two\",4\n2,\"she said \"\"hi\"\"\",5\n3,\"a,b\",6\n"
                .as_bytes()
                .to_vec(),
        ),
        (
            "emptycells",
            "a,b,c\n1,,x\n,2,\n3,4,z\n,,\n".as_bytes().to_vec(),
        ),
        (
            "literals",
            "rate,share,count\n20%,1/2,10\n5%,3/4,20\n7.5%,1/8,30\n"
                .as_bytes()
                .to_vec(),
        ),
        (
            "hex",
            "code,note\n0x1F,a\n0xff,b\n0x10,c\n0xABC,d\n"
                .as_bytes()
                .to_vec(),
        ),
        (
            "mixed",
            "n,flag,text\n1,true,alpha\n2.5,false,beta\n3,true,gamma\n"
                .as_bytes()
                .to_vec(),
        ),
    ]
}

#[test]
fn awkward_csvs_inline_and_as_tables_agree() {
    let dir = scratch("csv");
    for (name, bytes) in awkward_csvs() {
        let file = format!("{name}.csv");
        std::fs::write(dir.join(&file), &bytes).unwrap();
        let inline = csv::import(name, &bytes).unwrap().source;
        let differing = agree_but(
            &file,
            &dir,
            &inline,
            &format!("view T := dataframe['{file}']\n"),
        );
        only_literals(&file, &differing);
    }
}

/// What an import and a table are known to differ in (see [`agree_but`]):
/// percentages and fractions in a CSV (`20%`, `1/2`), which are text in a
/// table, as its reader sniffs numbers and dates and not OMX literals.
fn only_literals(label: &str, differing: &[String]) {
    for d in differing {
        assert!(
            label == "literals.csv" && matches!(d.as_str(), "rate.type" | "share.type"),
            "{label}: {d} is read as another type by a table"
        );
    }
}

/// The same rows as a tall file, to run the aggregates over on Polars.
/// A row of the tall corpus: an id, a quantity, a price and a day, with gaps.
type Row = (i64, Option<i64>, Option<String>, Option<String>);

fn tall_rows(rows: usize) -> Vec<Row> {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    (0..rows)
        .map(|i| {
            let qty = (i % 9 != 0).then(|| (next() % 90) as i64 + 1);
            let price = (i % 5 != 0).then(|| {
                let cents = next() % 50_000;
                format!("{}.{:02}", cents / 100, cents % 100)
            });
            let day = (i % 17 != 0).then(|| {
                let d = next() % 28 + 1;
                let m = next() % 12 + 1;
                format!("2023-{m:02}-{d:02}")
            });
            (i as i64, qty, price, day)
        })
        .collect()
}

#[test]
fn a_tall_csv_inline_and_as_a_table_agree_on_every_aggregate() {
    let dir = scratch("tall-csv");
    let mut text = String::from("id,qty,price,day,region\n");
    for (id, qty, price, day) in tall_rows(3_000) {
        text.push_str(&format!(
            "{id},{},{},{},{}\n",
            qty.map(|q| q.to_string()).unwrap_or_default(),
            price.unwrap_or_default(),
            day.unwrap_or_default(),
            ["North", "South", "East"][id as usize % 3]
        ));
    }
    std::fs::write(dir.join("tall.csv"), &text).unwrap();
    let inline = csv::import("tall", text.as_bytes()).unwrap().source;
    let differing = agree_but(
        "tall.csv",
        &dir,
        &inline,
        "view T := dataframe['tall.csv']\n",
    );
    assert!(differing.is_empty(), "{differing:?}");
}

#[test]
fn json_inline_and_as_a_table_agree() {
    let dir = scratch("json");
    let mut text = String::from("[");
    for (id, qty, price, day) in tall_rows(2_000) {
        if id > 0 {
            text.push(',');
        }
        let field = |v: Option<String>| v.unwrap_or_else(|| "null".to_string());
        text.push_str(&format!(
            "{{\"id\":{id},\"qty\":{},\"price\":{},\"day\":{},\"note\":\"row {id}\\n\\\"q\\\"\"}}",
            field(qty.map(|q| q.to_string())),
            field(price),
            field(day.map(|d| format!("\"{d}\""))),
        ));
    }
    text.push(']');
    std::fs::write(dir.join("rows.json"), &text).unwrap();
    let inline = json::import("rows", text.as_bytes()).unwrap().source;
    let differing = agree_but(
        "rows.json",
        &dir,
        &inline,
        "view T := dataframe['rows.json']\n",
    );
    assert!(differing.is_empty(), "{differing:?}");
}

#[test]
fn a_workbook_sheet_inline_and_as_a_table_agree() {
    use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};
    let dir = scratch("xlsx");
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name("Sales").unwrap();
    let date = Format::new().set_num_format("yyyy-mm-dd");
    for (c, name) in ["id", "qty", "price", "day", "region", "flag"]
        .iter()
        .enumerate()
    {
        sheet.write_string(0, c as u16, *name).unwrap();
    }
    for (id, qty, price, day) in tall_rows(2_000) {
        let r = id as u32 + 1;
        sheet.write_number(r, 0, id as f64).unwrap();
        if let Some(q) = qty {
            sheet.write_number(r, 1, q as f64).unwrap();
        }
        if let Some(p) = price {
            sheet.write_number(r, 2, p.parse::<f64>().unwrap()).unwrap();
        }
        if let Some(d) = day {
            let (y, m, dd) = (
                d[0..4].parse().unwrap(),
                d[5..7].parse().unwrap(),
                d[8..10].parse().unwrap(),
            );
            let day = ExcelDateTime::from_ymd(y, m, dd).unwrap();
            sheet.write_datetime_with_format(r, 3, &day, &date).unwrap();
        }
        sheet
            .write_string(r, 4, ["North", "South", "East"][id as usize % 3])
            .unwrap();
        sheet.write_boolean(r, 5, id % 2 == 0).unwrap();
    }
    let file = dir.join("book.xlsx");
    book.save(&file).unwrap();
    let bytes = std::fs::read(&file).unwrap();
    let imported = xlsx::import(&bytes, omasheet_engine::Options::default()).unwrap();
    let differing = agree_but(
        "book.xlsx#Sales",
        &dir,
        &imported.source,
        "view T := dataframe['book.xlsx#Sales']\n",
    );
    assert!(differing.is_empty(), "{differing:?}");
}

#[test]
fn hex_text_in_a_json_file_is_text_as_it_is_in_an_import() {
    // Only a CSV cell is read as a literal (and a hexadecimal column shown in
    // lower case, above); a JSON string is text and stays as it was written.
    let dir = scratch("hex");
    let json_text =
        r#"[{"code": "0x1F", "n": 1}, {"code": "0xAb", "n": 2}, {"code": "0x10", "n": 3}]"#;
    std::fs::write(dir.join("codes.json"), json_text).unwrap();
    let inline = json::import("codes", json_text.as_bytes()).unwrap().source;
    let differing = agree_but(
        "codes.json",
        &dir,
        &inline,
        "view T := dataframe['codes.json']\n",
    );
    assert!(differing.is_empty(), "{differing:?}");
}

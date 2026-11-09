// Copyright (c) 2026 Stephen Roe

//! A formula column put in a table opened with no schema lines, as the app
//! opens a big file, keeps the file's own columns.

use omasheet_engine::Document;
use omasheet_engine::datefmt::Names;
use omasheet_engine::omx::date::Style;

#[test]
fn a_formula_column_in_an_unnamed_table_keeps_the_files_columns() {
    let dir = std::env::temp_dir().join(format!("omasheet-formula-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: this test is the only one in this binary.
    unsafe { std::env::set_var("OMASHEET_CACHE", dir.join("cache")) };
    let csv = dir.join("t.csv");
    std::fs::write(&csv, "id,qty,unit price\n1,2,1.50\n2,3,2.25\n").unwrap();
    let text = format!(
        "view T := dataframe[{}]\n",
        omasheet_engine::omx::lexer::quoted(&csv.to_string_lossy())
    );
    let mut doc = Document::on_screen(&text, Style::ISO, Names::default());
    omasheet_engine::table::convert(&csv).unwrap();
    assert!(doc.converted(&csv));
    doc.insert_computed(0, 3, "Margin", "unitPrice * qty")
        .unwrap();
    let snap = doc.snapshot();
    let names: Vec<_> = snap.sheets[0]
        .columns
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, ["id", "qty", "unitPrice", "Margin"]);
    assert!(snap.problems.is_empty(), "{:?}", snap.problems);
    let rows = doc.rows(0, 0, 2);
    let shown: Vec<_> = rows.iter().map(|r| r[3].display.clone()).collect();
    assert_eq!(shown, ["3", "6.75"]);
    let _ = std::fs::remove_dir_all(&dir);
}

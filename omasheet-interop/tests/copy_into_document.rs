// Copyright (c) 2026 Stephen Roe

//! `open-a-big-csv` 4.2: a table over a small file is copied into the
//! document, its rows written as an inline sheet of the table's name, which
//! is then edited as any is.

use omasheet_engine::Document;
use std::path::PathBuf;

/// A directory of its own, with the one cache the tests share.
fn scratch(name: &str) -> PathBuf {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let root = std::env::temp_dir().join(format!("omasheet-copy-in-{}", std::process::id()));
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

#[test]
fn a_table_is_copied_into_the_document_and_edited() {
    let dir = scratch("edit");
    let csv = dir.join("t.csv");
    std::fs::write(&csv, "id,qty,unit price\n1,2,1.50\n2,3,2.25\n").unwrap();
    let text = format!(
        "view T := dataframe[{}]\n\nsheet Notes\n\nA\n1\n",
        omasheet_engine::omx::lexer::quoted(&csv.to_string_lossy())
    );
    let mut doc = Document::from_text(&text);
    omasheet_engine::table::convert(&csv).unwrap();
    assert!(doc.converted(&csv));
    assert_eq!(doc.table_file(0).as_deref(), Some(csv.as_path()));
    assert!(doc.table_file(1).is_none());
    // The table takes no edit.
    assert!(!doc.set_cell(0, 0, 1, "9"));

    // What an import writes, under the table's name.
    let bytes = std::fs::read(&csv).unwrap();
    let source = omasheet_interop::csv::import("t", &bytes).unwrap().source;
    let (_, rest) = source.split_once('\n').unwrap();
    doc.copy_table_into_document(0, &format!("sheet T\n{rest}"))
        .unwrap();

    let snap = doc.snapshot();
    assert!(snap.problems.is_empty(), "{:?}", snap.problems);
    assert_eq!(snap.sheets.len(), 2);
    assert_eq!(snap.sheets[0].name, "T");
    assert!(snap.sheets[0].source.is_none());
    assert_eq!(snap.sheets[1].name, "Notes");
    let names: Vec<_> = snap.sheets[0]
        .columns
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, ["id", "qty", "unitPrice"]);
    // And it is the document's own now: a cell takes an edit, and it undoes.
    assert!(doc.set_cell(0, 0, 1, "9"));
    assert_eq!(doc.rows(0, 0, 1)[0][1].display, "9");
    assert!(doc.undo());
    assert_eq!(doc.rows(0, 0, 1)[0][1].display, "2");
    assert!(doc.undo());
    assert!(doc.table_file(0).is_some());
}

#[test]
fn a_table_with_a_formula_column_is_not_copied() {
    let dir = scratch("formula");
    let csv = dir.join("u.csv");
    std::fs::write(&csv, "a,b\n1,2\n3,4\n").unwrap();
    let text = format!(
        "view U := dataframe[{}]\n\na : Integer\nb : Integer\n\nSum := a + b\n",
        omasheet_engine::omx::lexer::quoted(&csv.to_string_lossy())
    );
    let mut doc = Document::from_text(&text);
    omasheet_engine::table::convert(&csv).unwrap();
    assert!(doc.converted(&csv));
    let before = doc.text().to_string();
    let why = doc.copy_table_into_document(0, "sheet U\n\n| a |\n| - |\n| 1 |\n");
    assert!(why.unwrap_err().contains("formula columns"));
    assert_eq!(doc.text(), before);
}

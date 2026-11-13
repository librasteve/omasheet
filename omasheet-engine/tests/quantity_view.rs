// Copyright (c) 2026 Stephen Roe

//! A Parquet file read in place, as a view, with a column of quantities: the
//! numbers it holds (or, for a plain `Quantity`, the text) are read in the
//! unit and the kind that the file's metadata gives the column.

use omasheet_engine::omx::{compile, compile_expr};
use omasheet_engine::{Engine, Lowering, Options, table};
use polars::prelude::*;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a writable scratch directory");
    dir
}

fn file(dir: &std::path::Path) {
    let sigma = StructChunked::from_series(
        "Len".into(),
        3,
        [
            Series::new("value".into(), [Some(5.0), Some(10.5), None]),
            Series::new("sigma".into(), [Some(0.0), Some(0.3), None]),
        ]
        .iter(),
    )
    .expect("a struct")
    .into_series();
    let mut frame = DataFrame::new(vec![
        Series::new("Weight".into(), [Some(5000.0), Some(300.0), Some(5.0)]).into_column(),
        sigma.into_column(),
        Series::new("Any".into(), [Some("3m"), Some("10.5 ± 0.3kg"), None]).into_column(),
    ])
    .expect("a frame");
    let marks = KeyValueMetadata::from_static(vec![
        ("omasheet.kind.Weight".to_string(), "Mass[g]".to_string()),
        ("omasheet.kind.Len".to_string(), "Length".to_string()),
        ("omasheet.kind.Any".to_string(), "Quantity".to_string()),
    ]);
    let mut out = std::fs::File::create(dir.join("q.parquet")).expect("a file");
    ParquetWriter::new(&mut out)
        .with_key_value_metadata(Some(marks))
        .finish(&mut frame)
        .expect("a Parquet");
}

fn shown(dir: &std::path::Path, text: &str, expr: &str, lowering: Lowering) -> String {
    let (mut program, mut diags) = compile(text, 0);
    let tables = table::read(&mut program, dir, &mut diags);
    assert!(diags.is_empty(), "{diags:?}");
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
    let (node, _) = compile_expr(&program, expr, 1).expect("the expression checks");
    let value = engine
        .eval_top(&node)
        .unwrap_or_else(|| panic!("no value for {expr} ({lowering:?}) over {text}"));
    omasheet_engine::value::format_exact(&value, false, &omasheet_engine::omx::date::Style::ISO)
}

#[test]
fn a_view_reads_a_column_of_quantities_in_its_kind_and_unit() {
    let dir = scratch("quantity-view");
    file(&dir);
    let bare = "view Q := dataframe['q.parquet']\n";
    let declared =
        "view Q := dataframe['q.parquet']\n\nWeight : Mass[g]\nLen : Length\nAny : Quantity\n";
    for lowering in [Lowering::Never, Lowering::Always] {
        for text in [bare, declared] {
            let s = |expr: &str| shown(&dir, text, expr, lowering);
            assert_eq!(s("Q[Weight; 0]"), "5000g");
            assert_eq!(s("Q.Weight.sum()"), "5305g");
            assert_eq!(s("Q[Weight; 0] * 2"), "10000g");
            assert_eq!(s("Q[Len; 1]"), "10.5 ± 0.3m");
            assert_eq!(s("Q[Any; 0] * 2"), "6m");
            assert_eq!(s("Q[Any; 1]"), "10.5 ± 0.3kg");
        }
    }
}

// Copyright (c) 2026 Stephen Roe

//! The `omasheet` binary, end to end: the `cli` spec scenarios and the
//! `examples/` corpus compared with its expected output.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn omasheet(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omasheet"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn version() {
    let out = omasheet(&["--version"]);
    assert!(out.status.success());
    assert!(stdout(&out).starts_with("omasheet "));
}

#[test]
fn examples_match_their_expected_output() {
    let dir = root().join("examples");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "omx") {
            continue;
        }
        let before = std::fs::read(&path).unwrap();
        let out = omasheet(&[path.to_str().unwrap()]);
        assert!(out.status.success(), "{}: {}", path.display(), stderr(&out));
        let expected = std::fs::read_to_string(path.with_extension("expected")).unwrap();
        assert_eq!(stdout(&out), expected, "{}", path.display());
        // Viewing a sheet never modifies it.
        assert_eq!(std::fs::read(&path).unwrap(), before);
        seen += 1;
    }
    assert!(seen >= 3);
}

#[test]
fn eval_pure_arithmetic() {
    let out = omasheet(&["eval", "1/3 + 1/3"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out), "0.66667…\n");
    let out = omasheet(&["eval", "--exact", "1/3 + 1/3"]);
    assert_eq!(stdout(&out), "2/3\n");
}

#[test]
fn eval_against_a_sheet() {
    let expr = "Orders[Total; Region == \"US\"].sum()";
    let flag = omasheet(&["eval", "--sheet", "examples/orders.omx", expr]);
    let positional = omasheet(&["eval", "examples/orders.omx", expr]);
    assert_eq!(stdout(&flag), "74.47\n");
    assert_eq!(stdout(&positional), "74.47\n");
}

#[test]
fn eval_from_standard_input() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omasheet"))
        .args(["eval", "--sheet", "examples/budget.omx"])
        .current_dir(root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"Sales.Revenue.avg()\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(stdout(&out), "11733.33333…\n");
}

#[test]
fn lint_clean_sheet() {
    let out = omasheet(&["lint", "examples/budget.omx"]);
    assert!(out.status.success());
    assert!(stdout(&out).is_empty() && stderr(&out).is_empty());
}

#[test]
fn lint_reports_a_located_type_error() {
    let path = std::env::temp_dir().join(format!("omasheet-lint-{}.omx", std::process::id()));
    std::fs::write(
        &path,
        "table Sales\n\nMonth | Revenue | Bad\nJan | 100 | *\n\nBad := Revenue + Month\n",
    )
    .unwrap();
    let out = omasheet(&["lint", path.to_str().unwrap()]);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(
        err.contains(".omx:6:8: error: cannot apply `+` to Int and Text"),
        "{err}"
    );
    assert!(err.contains("6 | Bad := Revenue + Month"), "{err}");
}

#[test]
fn missing_file_is_a_usage_error() {
    let out = omasheet(&["no-such-file.omx"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn now_can_be_given() {
    let out = omasheet(&["eval", "--now", "2026-10-08T17:47", "now() + 60"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "2026-10-08T17:48\n");
    let out = omasheet(&["eval", "--now", "2026-10-08", "today().weekday()"]);
    assert_eq!(stdout(&out), "4\n");
    let out = omasheet(&["eval", "--now", "tomorrow", "today()"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("is not a date-time"));
}

#[test]
fn dates_are_iso_unless_the_locale_is_asked_for() {
    let run = |locale: &str, args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_omasheet"))
            .args(args)
            .env("LC_ALL", locale)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stderr(&out));
        stdout(&out)
    };
    let expr = "2026-10-08 + 17:47";
    assert_eq!(run("en_US.UTF-8", &["eval", expr]), "2026-10-08T17:47\n");
    // The C locale has no style of its own.
    assert_eq!(run("C", &["eval", "--locale", expr]), "2026-10-08T17:47\n");
    // Other locales are only there if the machine has them installed.
    let shown = run("en_GB.UTF-8", &["eval", "--locale", expr]);
    assert!(
        ["08/10/2026 17:47\n", "2026-10-08T17:47\n"].contains(&shown.as_str()),
        "{shown}"
    );
}

#[test]
fn time_zones_can_be_given() {
    let expr = "2025-07-15T12:00.to_zone(\"Asia/Tokyo\")";
    let out = omasheet(&["eval", "--zone", "Europe/London", expr]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "2025-07-15T20:00+09:00\n");
    let out = omasheet(&["eval", "--zone", "America/New_York", expr]);
    assert_eq!(stdout(&out), "2025-07-16T01:00+09:00\n");
    let out = omasheet(&["eval", "--zone", "Mars/Base", "1"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("there is no time zone `Mars/Base`"));
}

// ---- import, export, render ------------------------------------------------

/// A directory of its own for a test to write in.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn path(dir: &std::path::Path, file: &str) -> String {
    dir.join(file).to_string_lossy().into_owned()
}

#[test]
fn export_writes_beside_the_sheet_and_leaves_it_alone() {
    let dir = scratch("export");
    let sheet = path(&dir, "budget.omx");
    std::fs::copy(root().join("examples/budget.omx"), &sheet).unwrap();
    let before = std::fs::read(&sheet).unwrap();

    let out = omasheet(&["export", &sheet, "--xlsx"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(dir.join("budget.xlsx").exists());
    assert!(
        stderr(&out).contains("note: exact rationals were written as floating point: Sales.Margin"),
        "{}",
        stderr(&out)
    );

    let out = omasheet(&["export", &sheet, "--csv"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(dir.join("budget.Sales.csv").exists());
    assert!(dir.join("budget.Summary.csv").exists());

    let out = omasheet(&["export", &sheet, "--csv", "--table", "Sales"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let csv = std::fs::read_to_string(dir.join("budget.csv")).unwrap();
    assert!(
        csv.starts_with(
            "Month,Closed,Revenue,Cost,Profit,Tax,Margin,Growth\nJan,2025-02-03T17:30,10000,"
        ),
        "{csv}"
    );
    assert_eq!(std::fs::read(&sheet).unwrap(), before);
}

#[test]
fn export_needs_a_format_and_a_sheet_that_compiles() {
    let out = omasheet(&["export", "examples/budget.omx"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--xlsx or --csv"), "{}", stderr(&out));

    let out = omasheet(&["export", "examples/budget.omx", "--csv", "--table", "Costs"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("no table `Costs`"),
        "{}",
        stderr(&out)
    );

    let dir = scratch("export-bad");
    let sheet = path(&dir, "bad.omx");
    std::fs::write(&sheet, "table T\n\nA\n= Nope\n").unwrap();
    let out = omasheet(&["export", &sheet, "--xlsx"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("bad.omx:4:3: error:"),
        "{}",
        stderr(&out)
    );
    assert!(!dir.join("bad.xlsx").exists());
}

#[test]
fn import_reads_what_export_wrote() {
    let dir = scratch("import");
    let sheet = path(&dir, "budget.omx");
    std::fs::copy(root().join("examples/budget.omx"), &sheet).unwrap();
    assert!(omasheet(&["export", &sheet, "--xlsx"]).status.success());

    let back = path(&dir, "back.omx");
    let out = omasheet(&["import", &path(&dir, "budget.xlsx"), "-o", &back]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "");
    assert!(omasheet(&["lint", &back]).status.success());
    let total = omasheet(&["eval", &back, "Sales.Profit.sum()"]);
    assert_eq!(stdout(&total), "14700\n");

    let csv = path(&dir, "sales.csv");
    std::fs::write(&csv, "Month,Unit Price\nJan,19.99\n").unwrap();
    let out = omasheet(&["import", &csv]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "table sales\n\nMonth | UnitPrice\nJan   | 19.99\n"
    );
    assert_eq!(stderr(&out), "note: `Unit Price` was renamed `UnitPrice`\n");
}

#[test]
fn import_says_what_it_can_read() {
    let out = omasheet(&["import", "README.md"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("(.xlsx) or a CSV file (.csv)"),
        "{}",
        stderr(&out)
    );

    let dir = scratch("import-bad");
    let book = path(&dir, "not.xlsx");
    std::fs::write(&book, "hello").unwrap();
    let out = omasheet(&["import", &book]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("it is not a workbook"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn render_matches_the_expected_report() {
    let out = omasheet(&["render", "examples/report.md"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let expected = std::fs::read_to_string(root().join("examples/report.expected")).unwrap();
    assert_eq!(stdout(&out), expected);
    assert!(!expected.contains("{{ S"), "{expected}");
}

#[test]
fn render_writes_html() {
    let dir = scratch("render");
    let page = path(&dir, "report.html");
    let out = omasheet(&["render", "examples/report.md", "--html", "-o", &page]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "");
    let html = std::fs::read_to_string(&page).unwrap();
    assert!(html.contains("<title>First quarter</title>"), "{html}");
    assert!(html.contains("Revenue for the quarter was 35200"), "{html}");
    assert!(
        html.contains("<td style=\"text-align: right\">5700</td>"),
        "{html}"
    );
}

#[test]
fn render_reports_where_in_the_document() {
    let dir = scratch("render-bad");
    let doc = path(&dir, "bad.md");
    std::fs::write(&doc, "---\nsheets: [missing.omx]\n---\nHello\n").unwrap();
    let out = omasheet(&["render", &doc]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("bad.md:2:10: error: cannot read `missing.omx`"),
        "{}",
        stderr(&out)
    );

    std::fs::write(&doc, "Total: {{ Sales.Revenue.sum() }}\n").unwrap();
    let out = omasheet(&["render", &doc]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stdout(&out), "");
    assert!(
        stderr(&out).contains("bad.md:1:11: error:"),
        "{}",
        stderr(&out)
    );
}

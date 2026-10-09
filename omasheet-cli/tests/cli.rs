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

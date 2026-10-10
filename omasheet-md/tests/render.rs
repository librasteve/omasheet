// Copyright (c) 2026 Stephen Roe

//! The scenarios of the `markdown-integration` spec.

use omasheet_engine::Options;
use omasheet_md::{Format, render};

const SALES: &str = "\
const TotalRevenue = Sales.Revenue.sum()

table Sales

Month | Revenue
Jan   | 100
Feb   | 120
Mar   | 150
";

fn load(path: &str) -> Result<(String, String), String> {
    match path {
        "Sales.omx" => Ok((path.to_string(), SALES.to_string())),
        "London.omx" => Ok((
            path.to_string(),
            "zone Europe/London\nconst A = 1\n".to_string(),
        )),
        "Tokyo.omx" => Ok((
            path.to_string(),
            "zone Asia/Tokyo\nconst B = 2\n".to_string(),
        )),
        _ => Err("No such file".to_string()),
    }
}

fn markdown(text: &str) -> String {
    let out = render(
        "report.md",
        text,
        &load,
        Format::Markdown,
        Options::default(),
    );
    assert!(out.ok(), "{}", out.errors.join("\n"));
    out.output
}

fn errors(text: &str) -> String {
    let out = render(
        "report.md",
        text,
        &load,
        Format::Markdown,
        Options::default(),
    );
    assert!(!out.ok());
    assert_eq!(out.output, "");
    out.errors.join("\n")
}

const INLINE: &str = "\
# Report

```omx
table Sales

Month | Revenue | Share
Jan   | 100     | *
Feb   | 120     | *
Mar   | 150     | *

Share := Revenue / Sales.Revenue.sum()
```

Total revenue: {{ Sales.Revenue.sum() }}
";

#[test]
fn a_block_is_shown_as_its_tables_and_prose_quotes_them() {
    assert_eq!(
        markdown(INLINE),
        "\
# Report

| Month | Revenue |    Share |
| :---- | ------: | -------: |
| Jan   |     100 | 0.27027… |
| Feb   |     120 | 0.32432… |
| Mar   |     150 | 0.40541… |

Total revenue: 370
"
    );
}

#[test]
fn a_report_over_a_separate_sheet() {
    let text = "---\ntitle: Q1\nsheets: [Sales.omx]\n---\nWe took {{ TotalRevenue }} in {{ Sales.Month.count() }} months.\n";
    assert_eq!(
        markdown(text),
        "---\ntitle: Q1\nsheets: [Sales.omx]\n---\nWe took 370 in 3 months.\n"
    );
    for front in [
        "sheets: Sales.omx",
        "sheets:\n  - \"Sales.omx\"",
        "sheet: 'Sales.omx'",
    ] {
        let text = format!("---\n{front}\n---\n{{{{ TotalRevenue }}}}\n");
        assert!(markdown(&text).ends_with("---\n370\n"), "{front}");
    }
}

#[test]
fn blocks_and_files_are_one_sheet() {
    let text = "\
---
sheets: [Sales.omx]
---
```omx
const Target = 400
```

```omx
table Gap

Label | Short
Q1    | = Target - TotalRevenue
```
";
    assert_eq!(
        markdown(text),
        "---\nsheets: [Sales.omx]\n---\n| Label | Short |\n| :---- | ----: |\n| Q1    |    30 |\n"
    );
}

#[test]
fn several_values_read_as_a_list() {
    let text = "---\nsheets: Sales.omx\n---\n{{ Sales[; Revenue > 100].Month }} beat {{ Sales[; Revenue == 100].Month }}.\n";
    assert!(markdown(text).ends_with("\nFeb, Mar beat Jan.\n"));
}

#[test]
fn code_is_left_alone() {
    let text = "Write `{{ 1 + 1 }}` to get {{ 1 + 1 }}, or \\{{ 1 + 1 }}.\n\n```\n{{ 2 + 2 }}\n```\n\n```python\nx = '{{ 3 }}'\n```\n";
    assert_eq!(
        markdown(text),
        "Write `{{ 1 + 1 }}` to get 2, or \\{{ 1 + 1 }}.\n\n```\n{{ 2 + 2 }}\n```\n\n```python\nx = '{{ 3 }}'\n```\n"
    );
}

#[test]
fn a_table_in_the_text_is_a_table() {
    let text = "---\nsheets: Sales.omx\n---\nThe good months:\n{{ Sales[; Revenue > 100] }}\n";
    assert_eq!(
        markdown(text),
        "---\nsheets: Sales.omx\n---\nThe good months:\n\n| Month | Revenue |\n| :---- | ------: |\n| Feb   |     120 |\n| Mar   |     150 |\n"
    );
}

#[test]
fn an_error_in_an_interpolation_points_into_the_document() {
    let text = "---\nsheets: Sales.omx\n---\nTotal: {{ Sales.Revenu.sum() }}\n";
    let shown = errors(text);
    assert!(shown.starts_with("report.md:4:17: error:"), "{shown}");
    assert!(
        shown.contains("4 | Total: {{ Sales.Revenu.sum() }}\n"),
        "{shown}"
    );
    assert!(shown.contains("                ^^^^^^\n"), "{shown}");
}

#[test]
fn an_error_in_a_block_points_into_the_document() {
    let text = "Intro.\n\n```omx\ntable T\n\nA\n= Nope + 1\n```\n";
    let shown = errors(text);
    assert!(shown.starts_with("report.md:7:3: error:"), "{shown}");
    assert!(shown.contains("7 | = Nope + 1\n"), "{shown}");
}

#[test]
fn an_error_in_a_linked_sheet_points_into_that_sheet() {
    let text = "---\nsheets: [Sales.omx]\n---\n```omx\ntable Sales\n\nA\n1\n```\n";
    let shown = errors(text);
    assert!(
        shown.contains("table `Sales` is already defined"),
        "{shown}"
    );
    let shown = errors("---\nsheets: [Missing.omx]\n---\nHello\n");
    assert!(
        shown.starts_with("report.md:2:10: error: cannot read `Missing.omx`: No such file"),
        "{shown}"
    );
    let shown = errors("{{ 1 / 0 }}\n");
    assert!(shown.starts_with("report.md:1:4: error:"), "{shown}");
}

#[test]
fn sheets_share_a_zone() {
    let text = "---\nsheets: [Sales.omx, London.omx]\n---\n{{ A }}\n";
    assert!(markdown(text).ends_with("\n1\n"));
    let shown = errors("---\nsheets: [London.omx, Tokyo.omx]\n---\n{{ A + B }}\n");
    assert!(
        shown.starts_with(
            "Tokyo.omx:1:6: error: the document is already in the zone `Europe/London`"
        ),
        "{shown}"
    );
}

#[test]
fn html() {
    let out = render("report.md", INLINE, &load, Format::Html, Options::default());
    assert!(out.ok(), "{:?}", out.errors);
    let html = out.output;
    assert!(html.starts_with("<!doctype html>"), "{html}");
    assert!(html.contains("<title>Report</title>"), "{html}");
    assert!(
        html.contains("<th style=\"text-align: right\">Revenue</th>"),
        "{html}"
    );
    assert!(
        html.contains("<td style=\"text-align: right\">100</td>"),
        "{html}"
    );
    assert!(html.contains("<p>Total revenue: 370</p>"), "{html}");
    assert!(!html.contains("{{"), "{html}");
    assert!(!html.contains("table Sales"), "{html}");

    let text = "---\ntitle: Q1 <draft>\n---\n```omx\ntable T\n\nA\n\"*not* <b>bold</b>\"\n```\n\n{{ T[A; 0] }}\n";
    let html = render("r.md", text, &load, Format::Html, Options::default()).output;
    assert!(html.contains("<title>Q1 &lt;draft&gt;</title>"), "{html}");
    assert!(
        html.contains("<p>*not* &lt;b&gt;bold&lt;/b&gt;</p>"),
        "{html}"
    );
    assert!(
        html.contains("<td style=\"text-align: left\">*not* &lt;b&gt;bold&lt;/b&gt;</td>"),
        "{html}"
    );
    assert!(!html.contains("sheets"), "{html}");
}

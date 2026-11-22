// Copyright (c) 2026 Stephen Roe

//! The scenarios of the `file-format` spec for `plot` declarations.

use omasheet_omx::compile;

const SALES: &str = "\
sheet Sales

Month | Revenue | Cost
Jan   | 100     | 60
Feb   | 120     | 70
";

fn plots(text: &str) -> omasheet_omx::check::Program {
    let (program, diags) = compile(text, 0);
    assert!(diags.is_empty(), "{diags:?}");
    program
}

fn errors(text: &str) -> Vec<String> {
    let (_, diags) = compile(text, 0);
    diags
        .iter()
        .map(|d| {
            let help = d.help.as_deref().unwrap_or("");
            format!("{} | {help} | {}", d.message, &text[d.span.start as usize..d.span.end as usize])
        })
        .collect()
}

#[test]
fn a_plot_after_a_sheet_is_not_part_of_it() {
    let program = plots(&format!(
        "{SALES}\nplot Rev\ntype: line\ndata: Sales\nx: Month\ny: Revenue, Cost\ntitle: Money\nwidth: 500\nk: 2\n"
    ));
    assert_eq!(program.sheets.len(), 1);
    assert_eq!(program.sheets[0].cols.len(), 3);
    assert_eq!(program.sheets[0].nrows, 2);
    let p = &program.plots[0];
    assert_eq!((p.name.as_str(), p.kind.as_str()), ("Rev", "line"));
    assert_eq!(p.x.as_deref(), Some("Month"));
    assert_eq!(p.y, ["Revenue", "Cost"]);
    assert_eq!(p.title.as_deref(), Some("Money"));
    assert_eq!((p.width, p.height, p.k), (Some(500), None, Some(2.0)));
}

#[test]
fn a_plot_may_come_before_its_sheet() {
    let program = plots(&format!("plot Rev\ntype: pie\ndata: Sales |> filter(Revenue > 100)\n\n{SALES}"));
    assert_eq!(program.plots.len(), 1);
    assert_eq!(program.sheets[0].nrows, 2);
}

#[test]
fn a_comment_may_stand_in_a_plot() {
    plots(&format!("{SALES}\nplot Rev\n// the lot\ntype: bar\ndata: Sales\n"));
}

#[test]
fn a_mistake_is_reported_at_its_line() {
    let e = errors(&format!("{SALES}\nplot Rev\ntype: radar\ndata: Sales\n"));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].starts_with("`radar` is not a kind of plot | the types are line, bar, pie, candle | radar"), "{e:?}");

    let e = errors(&format!("{SALES}\nplot Rev\ntype: line\ndata: Sales\ncolour: red\n"));
    assert!(e[0].contains("`colour` is not a control") && e[0].ends_with("| colour"), "{e:?}");

    let e = errors(&format!("{SALES}\nplot Rev\ntype: line\ndata: Sales\ny: Cost, Profit\n"));
    assert!(e[0].starts_with("`data` has no column `Profit` | its columns are Month, Revenue, Cost | Profit"), "{e:?}");

    let e = errors(&format!("{SALES}\nplot Rev\ntype: line\ndata: Sales.Revenue.sum()\n"));
    assert!(e[0].contains("a plot needs a sheet"), "{e:?}");

    let e = errors(&format!("{SALES}\nplot Rev\ntype: line\n"));
    assert!(e[0].contains("needs a `data:`"), "{e:?}");

    let e = errors(&format!("{SALES}\nplot Rev\ndata: Sales\n"));
    assert!(e[0].contains("needs a `type:`"), "{e:?}");

    let e = errors(&format!("{SALES}\nplot Rev\ntype: line\ndata: Nope\n"));
    assert!(e[0].contains("Nope"), "{e:?}");

    let e = errors(&format!("{SALES}\nplot Rev\ntype: line\ndata: Sales\nwidth: 12\n"));
    assert!(e[0].contains("100 to 4000"), "{e:?}");

    let e = errors(&format!("{SALES}\nplot Rev\ntype line\ndata: Sales\n"));
    assert!(e.iter().any(|m| m.contains("expected `key: value`")), "{e:?}");
}

#[test]
fn a_plot_name_is_not_a_sheet_a_constant_or_another_plot() {
    for text in [
        format!("{SALES}\nplot Sales\ntype: line\ndata: Sales\n"),
        format!("const Rev = 1\n\n{SALES}\nplot Rev\ntype: line\ndata: Sales\n"),
        format!("{SALES}\nplot A\ntype: line\ndata: Sales\n\nplot A\ntype: bar\ndata: Sales\n"),
    ] {
        let e = errors(&text);
        assert!(e.iter().any(|m| m.contains("is already used")), "{e:?}");
    }
}

// Copyright (c) 2026 Stephen Roe

//! Editing the plots of a file: the scenarios of the `app` spec.

use omasheet_engine::Document;

const FILE: &str = "\
sheet Sales

Month | Revenue | Cost
Jan   | 100     | 60
Feb   | 120     | 70

plot Trend
type: line
data: Sales
";

fn doc() -> Document {
    Document::from_text(FILE)
}

#[test]
fn a_plot_is_listed_with_its_controls_and_the_columns_of_its_data() {
    let doc = doc();
    assert_eq!(doc.plots(), ["Trend"]);
    assert_eq!(
        doc.plot_controls("Trend").unwrap(),
        [("type".to_string(), "line".to_string()), ("data".to_string(), "Sales".to_string())]
    );
    assert_eq!(doc.plot_columns("Trend"), ["Month", "Revenue", "Cost"]);
    assert!(doc.plot_svg("Trend", (400, 300)).unwrap().starts_with("<svg"));
}

#[test]
fn a_plot_is_added_at_the_end_and_undone_in_one_step() {
    let mut doc = doc();
    doc.add_plot("Share", "pie", "Sales").unwrap();
    assert!(doc.text().ends_with("\n\nplot Share\ntype: pie\ndata: Sales\n"), "{}", doc.text());
    assert_eq!(doc.plots(), ["Trend", "Share"]);
    assert!(doc.undo());
    assert_eq!(doc.text(), FILE);
}

#[test]
fn a_plot_is_refused_what_would_break_the_file() {
    let mut doc = doc();
    assert!(doc.add_plot("Sales", "line", "Sales").unwrap_err().contains("already used"));
    assert!(doc.add_plot("Trend", "line", "Sales").unwrap_err().contains("already used"));
    assert!(doc.add_plot("X", "radar", "Sales").unwrap_err().contains("not a kind"));
    assert!(doc.add_plot("X", "line", "Nope").is_err());
    assert!(doc.add_plot("X", "line", "Sales.Revenue.sum()").unwrap_err().contains("needs a sheet"));
    assert!(doc.add_plot("a b", "line", "Sales").is_err());
    assert_eq!(doc.text(), FILE);
    // And a sheet or a constant cannot take a plot's name.
    assert!(doc.add_const("Trend", "1").unwrap_err().contains("already used"));
    assert!(doc.add_sheet("Trend", None).unwrap_err().contains("already used"));
}

#[test]
fn a_control_is_set_added_and_taken_out() {
    let mut doc = doc();
    doc.set_plot_control("Trend", "type", Some("bar")).unwrap();
    doc.set_plot_control("Trend", "y", Some("Revenue, Cost")).unwrap();
    doc.set_plot_control("Trend", "title", Some("Money")).unwrap();
    assert!(doc.text().ends_with("plot Trend\ntype: bar\ndata: Sales\ny: Revenue, Cost\ntitle: Money\n"), "{}", doc.text());
    doc.set_plot_control("Trend", "y", None).unwrap();
    assert!(!doc.text().contains("y: Revenue"), "{}", doc.text());
    assert!(doc.text().ends_with("data: Sales\ntitle: Money\n"));
    // One step each.
    assert!(doc.undo());
    assert!(doc.text().contains("y: Revenue, Cost"));
}

#[test]
fn a_control_is_refused_what_does_not_fit() {
    let mut doc = doc();
    assert!(doc.set_plot_control("Trend", "type", Some("radar")).is_err());
    assert!(doc.set_plot_control("Trend", "type", None).is_err());
    assert!(doc.set_plot_control("Trend", "colour", Some("red")).is_err());
    assert!(doc.set_plot_control("Trend", "y", Some("Profit")).unwrap_err().contains("no column"));
    assert!(doc.set_plot_control("Trend", "data", Some("Nope")).is_err());
    assert!(doc.set_plot_control("Nope", "type", Some("bar")).is_err());
    assert_eq!(doc.text(), FILE);
}

#[test]
fn a_plot_is_renamed_and_deleted() {
    let mut doc = doc();
    doc.rename_plot("Trend", "Money").unwrap();
    assert_eq!(doc.plots(), ["Money"]);
    assert!(doc.rename_plot("Money", "Sales").is_err());
    doc.delete_plot("Money").unwrap();
    assert!(doc.plots().is_empty());
    assert_eq!(doc.text(), "sheet Sales\n\nMonth | Revenue | Cost\nJan   | 100     | 60\nFeb   | 120     | 70\n");
    assert!(doc.undo());
    assert_eq!(doc.plots(), ["Money"]);
}

#[test]
fn renaming_a_sheet_renames_it_in_the_plots_that_read_it() {
    let mut doc = Document::from_text(&format!("{FILE}\nplot Spread\ntype: bar\ndata: Sales |> select(Month, Cost)\n"));
    doc.rename_sheet(0, "Takings").unwrap();
    assert!(doc.text().contains("data: Takings\n"), "{}", doc.text());
    assert!(doc.text().contains("data: Takings |> select(Month, Cost)"), "{}", doc.text());
    assert_eq!(doc.plots().len(), 2);
}

#[test]
fn a_plot_between_sheets_is_deleted_with_its_comment() {
    let mut doc = Document::from_text("sheet A\n\nX\n1\n\n// the lot\nplot P\ntype: line\ndata: A\n\nsheet B\n\nY\n2\n");
    doc.delete_plot("P").unwrap();
    assert_eq!(doc.text(), "sheet A\n\nX\n1\n\nsheet B\n\nY\n2\n");
}

#[test]
fn every_kind_of_plot_draws_from_the_example() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/plots.omx")).unwrap();
    let doc = Document::from_text(&text);
    let names = doc.plots();
    assert_eq!(names.len(), 17);
    for name in names {
        let svg = doc.plot_svg(&name, (640, 360)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(svg.starts_with("<svg") && !svg.contains("\n\n"), "{name}");
    }
    // A scatter needs numbers along `x`.
    let mut doc = Document::from_text("sheet S\n\nMonth | V\nJan | 1\nFeb | 2\n");
    doc.add_plot("P", "scatter", "S").unwrap();
    assert!(doc.plot_svg("P", (400, 300)).unwrap_err().contains("numbers in `x`"));
}

#[test]
fn a_plot_has_the_sheet_its_data_reads_as_its_parent() {
    let mut doc = Document::from_text(
        "sheet A\n\nM | V\na | 1\n\nsheet B\n\nM | V\nb | 2\n\nplot P\ntype: line\ndata: A |> select(M, V)\n",
    );
    doc.add_plot("Q", "bar", "B").unwrap();
    assert_eq!(doc.plot_parents(), ["A", "B"]);
    doc.rename_sheet(0, "Alpha").unwrap();
    assert_eq!(doc.plot_parents(), ["Alpha", "B"]);
}

#[test]
fn a_click_on_a_mark_finds_its_cell() {
    let doc = doc();
    // Somewhere in the middle of a small line plot there is a point; scan
    // for it, and check it is a cell of Sales.
    let mut found = None;
    'scan: for y in (0..300).step_by(4) {
        for x in (0..400).step_by(4) {
            if let Some(cell) = doc.plot_cell_at("Trend", (400, 300), x as f64, y as f64) {
                found = Some(cell);
                break 'scan;
            }
        }
    }
    let (sheet, row, col) = found.expect("a mark");
    assert_eq!(sheet, "Sales");
    assert!(row < 2 && col < 3);
    assert!(doc.plot_cell_at("Trend", (400, 300), 1.0, 1.0).is_none());
}

#[test]
fn a_kind_the_data_does_not_suit_says_why() {
    let doc = doc();
    let fit = doc.plot_fit("Trend");
    assert_eq!(fit.len(), 16);
    let why = |kind: &str| fit.iter().find(|(k, _)| k == kind).unwrap().1.clone();
    assert_eq!(why("line"), "");
    assert_eq!(why("bar"), "");
    // The labels are months, not numbers.
    assert!(why("scatter").contains("numbers in `x`"), "{}", why("scatter"));
    assert!(why("candle").contains("one uncertain column"), "{}", why("candle"));
}

#[test]
fn the_columns_a_plot_shows_are_those_its_kind_draws() {
    let mut doc = doc();
    assert_eq!(doc.plot_drawn_columns("Trend"), ["Revenue", "Cost", "Month"]);
    doc.set_plot_control("Trend", "y", Some("Cost")).unwrap();
    assert_eq!(doc.plot_drawn_columns("Trend"), ["Cost", "Month"]);
    doc.set_plot_control("Trend", "y", None).unwrap();
    doc.set_plot_control("Trend", "type", Some("pie")).unwrap();
    assert_eq!(doc.plot_drawn_columns("Trend"), ["Revenue", "Month"]);
    doc.set_plot_control("Trend", "type", Some("box")).unwrap();
    assert_eq!(doc.plot_drawn_columns("Trend"), ["Revenue", "Cost"]);
}

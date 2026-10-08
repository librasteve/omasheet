//! An editable sheet for interactive front ends.
//!
//! The `.omx` text stays the single source of truth: every edit rewrites the
//! text and the sheet is then re-read and re-evaluated. A [`Snapshot`] is what
//! a grid shows: for each cell, the value to display and the source to edit.
//!
//! Unlike the command line, a document is evaluated even when it has errors,
//! so that one mistyped formula does not blank the rest of the grid; cells
//! that could not be calculated show as errors.

use crate::eval::Engine;
use crate::value::{Value, format_scalar};
use omasheet_omx::sheet::{SheetAst, TableDecl, ident_len, parse_sheet, split_cells};
use omasheet_omx::{ColKind, Diagnostic, S, Sources, Span, compile};

const UNDO_LIMIT: usize = 200;

/// A fresh sheet: one small empty table to type into.
pub const BLANK: &str = "table Sheet1\n\nA | B | C\n  |   |\n  |   |\n  |   |\n  |   |\n  |   |\n";

#[derive(Debug, Default, Clone)]
pub struct Snapshot {
    pub tables: Vec<TableSnap>,
    pub consts: Vec<ConstSnap>,
    pub problems: Vec<Problem>,
}

#[derive(Debug, Clone)]
pub struct TableSnap {
    pub name: String,
    pub columns: Vec<ColumnSnap>,
    pub rows: Vec<Vec<CellSnap>>,
}

#[derive(Debug, Clone)]
pub struct ColumnSnap {
    pub name: String,
    /// The column type, or empty when it is not known.
    pub ty: String,
    /// The `:=` expression of a computed column; `None` for a data column.
    pub formula: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CellSnap {
    /// The calculated value, as shown in the grid.
    pub display: String,
    /// What the cell holds in the file: a literal, or `= expression`. For a
    /// computed column, the column's expression.
    pub source: String,
    pub numeric: bool,
    pub formula: bool,
    /// Why the cell could not be calculated, if it could not.
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConstSnap {
    pub name: String,
    pub source: String,
    pub display: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Problem {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

pub struct Document {
    text: String,
    snapshot: Snapshot,
    undo: Vec<String>,
    redo: Vec<String>,
}

impl Default for Document {
    fn default() -> Self {
        Document::from_text(BLANK)
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    inner.start >= outer.start && inner.start <= outer.end
}

/// Make arbitrary input safe to store as one cell: a single line that does
/// not split into several cells.
fn clean_cell(input: &str) -> String {
    let text: String = input
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = text.trim();
    if text.is_empty() || split_cells(text, 0, 0).len() == 1 {
        return text.to_string();
    }
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn valid_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || ident_len(name) != name.len() {
        return Err(format!(
            "`{name}` is not a valid name: use letters, digits and `_`, starting with a letter"
        ));
    }
    Ok(name)
}

/// The header and rows of a table as text, plus where each row came from.
struct Grid {
    header: Vec<String>,
    /// `(original row index, cells)`; `None` for a new row.
    rows: Vec<(Option<usize>, Vec<String>)>,
}

impl Grid {
    fn of(t: &TableDecl) -> Grid {
        Grid {
            header: t.header.iter().map(|(name, _)| name.clone()).collect(),
            rows: t
                .rows
                .iter()
                .enumerate()
                .map(|(i, row)| (Some(i), row.iter().map(|c| c.text.clone()).collect()))
                .collect(),
        }
    }

    fn blank_row(&self) -> Vec<String> {
        // A lone empty cell would be a blank line, which is not a row.
        let cell = if self.header.len() == 1 { "\"\"" } else { "" };
        vec![cell.to_string(); self.header.len()]
    }

    /// Rewrite the table's lines in `text`, aligned, touching nothing else.
    fn write(&self, t: &TableDecl, text: &str) -> String {
        let n = self.header.len();
        let mut widths: Vec<usize> = self.header.iter().map(|h| h.chars().count()).collect();
        for (_, row) in &self.rows {
            for (k, cell) in row.iter().enumerate().take(n) {
                widths[k] = widths[k].max(cell.chars().count());
            }
        }
        let line = |cells: &[String]| {
            let padded: Vec<String> = cells
                .iter()
                .enumerate()
                .map(|(k, c)| format!("{c:<width$}", width = widths[k]))
                .collect();
            // A trailing empty cell still ends in its separator.
            padded.join(" | ").trim_end().to_string()
        };

        // (start, end, replacement), applied from the end of the text back.
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        let range = |s: Span| (s.start as usize, s.end as usize);
        let (hs, he) = range(t.header_line);
        edits.push((hs, he, line(&self.header)));
        let mut anchor = he;
        if let Some(sep) = t.separator_line {
            let (s, e) = range(sep);
            let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
            edits.push((s, e, rule.join("-|-")));
            anchor = e;
        }
        let kept: Vec<usize> = self.rows.iter().filter_map(|(o, _)| *o).collect();
        for (i, span) in t.row_lines.iter().enumerate() {
            if !kept.contains(&i) {
                // Remove the line and its line ending.
                let (s, e) = range(*span);
                let end = if text[e..].starts_with("\r\n") {
                    e + 2
                } else if text[e..].starts_with('\n') {
                    e + 1
                } else {
                    e
                };
                edits.push((s, end, String::new()));
            }
        }
        // New rows go after the line of the row they follow.
        let mut pending = String::new();
        for (origin, cells) in &self.rows {
            match origin {
                Some(i) => {
                    if !pending.is_empty() {
                        edits.push((anchor, anchor, std::mem::take(&mut pending)));
                    }
                    let (s, e) = range(t.row_lines[*i]);
                    edits.push((s, e, line(cells)));
                    anchor = e;
                }
                None => {
                    pending.push('\n');
                    pending.push_str(&line(cells));
                }
            }
        }
        if !pending.is_empty() {
            edits.push((anchor, anchor, pending));
        }
        let mut out = text.to_string();
        let mut order: Vec<usize> = (0..edits.len()).collect();
        order.sort_by(|&a, &b| edits[b].0.cmp(&edits[a].0).then(b.cmp(&a)));
        for k in order {
            let (s, e, ref replacement) = edits[k];
            out.replace_range(s..e, replacement);
        }
        out
    }
}

impl Document {
    pub fn from_text(text: &str) -> Document {
        let mut doc = Document {
            text: text.to_string(),
            snapshot: Snapshot::default(),
            undo: Vec::new(),
            redo: Vec::new(),
        };
        doc.rebuild();
        doc
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(&mut self.text, previous));
        self.rebuild();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.text, next));
        self.rebuild();
        true
    }

    /// Replace the text as one undoable step. Returns whether it changed.
    fn commit(&mut self, text: String) -> bool {
        if text == self.text {
            return false;
        }
        self.undo.push(std::mem::replace(&mut self.text, text));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.rebuild();
        true
    }

    fn ast(&self) -> SheetAst {
        parse_sheet(&self.text, 0, &mut Vec::new())
    }

    /// The declaration of the snapshot's table `table`.
    fn table_decl<'a>(&self, ast: &'a SheetAst, table: usize) -> Option<&'a TableDecl> {
        let name = &self.snapshot.tables.get(table)?.name;
        ast.tables.iter().find(|t| &t.name == name)
    }

    fn edit_grid(&mut self, table: usize, change: impl FnOnce(&mut Grid)) -> bool {
        let ast = self.ast();
        let Some(decl) = self.table_decl(&ast, table) else {
            return false;
        };
        let mut grid = Grid::of(decl);
        change(&mut grid);
        let text = grid.write(decl, &self.text);
        self.commit(text)
    }

    /// Set cells of one table in a single undoable step. Cells are
    /// `(row, column, text)`; rows past the end are added. Cells in computed
    /// columns are skipped.
    pub fn set_cells(&mut self, table: usize, cells: &[(usize, usize, String)]) -> bool {
        self.edit_grid(table, |grid| {
            for (row, col, text) in cells {
                if *col >= grid.header.len() {
                    continue;
                }
                while grid.rows.len() <= *row {
                    let blank = grid.blank_row();
                    grid.rows.push((None, blank));
                }
                let single = grid.header.len() == 1;
                let cleaned = clean_cell(text);
                grid.rows[*row].1[*col] = if cleaned.is_empty() && single {
                    "\"\"".to_string()
                } else {
                    cleaned
                };
            }
        })
    }

    pub fn set_cell(&mut self, table: usize, row: usize, col: usize, text: &str) -> bool {
        let is_computed = self
            .snapshot
            .tables
            .get(table)
            .and_then(|t| t.columns.get(col))
            .is_some_and(|c| c.formula.is_some());
        if is_computed {
            return self.set_column_formula(table, col, text);
        }
        self.set_cells(table, &[(row, col, text.to_string())])
    }

    /// Insert `count` empty rows before row `at`.
    pub fn insert_rows(&mut self, table: usize, at: usize, count: usize) -> bool {
        self.edit_grid(table, |grid| {
            let at = at.min(grid.rows.len());
            for _ in 0..count {
                let blank = grid.blank_row();
                grid.rows.insert(at, (None, blank));
            }
        })
    }

    pub fn delete_rows(&mut self, table: usize, first: usize, count: usize) -> bool {
        self.edit_grid(table, |grid| {
            let end = (first + count).min(grid.rows.len());
            if first < end {
                grid.rows.drain(first..end);
            }
        })
    }

    /// Add an empty data column at the end of the header row.
    pub fn add_column(&mut self, table: usize, name: &str) -> Result<(), String> {
        let name = valid_name(name)?.to_string();
        let Some(snap) = self.snapshot.tables.get(table) else {
            return Err("there is no such table".into());
        };
        if snap.columns.iter().any(|c| c.name == name) {
            return Err(format!(
                "table `{}` already has a column `{name}`",
                snap.name
            ));
        }
        self.edit_grid(table, |grid| {
            // A lone column stored its empty cells as `""`.
            if grid.header.len() == 1 {
                for (_, row) in &mut grid.rows {
                    if row[0] == "\"\"" {
                        row[0].clear();
                    }
                }
            }
            grid.header.push(name);
            for (_, row) in &mut grid.rows {
                row.push(String::new());
            }
        });
        Ok(())
    }

    /// Add `Name := expression` to a table.
    pub fn add_computed(&mut self, table: usize, name: &str, expr: &str) -> Result<(), String> {
        let name = valid_name(name)?.to_string();
        let expr = expr.trim().trim_start_matches('=').trim();
        if expr.is_empty() {
            return Err("a computed column needs an expression".into());
        }
        let ast = self.ast();
        let Some(decl) = self.table_decl(&ast, table) else {
            return Err("there is no such table".into());
        };
        if self.snapshot.tables[table]
            .columns
            .iter()
            .any(|c| c.name == name)
        {
            return Err(format!(
                "table `{}` already has a column `{name}`",
                decl.name
            ));
        }
        let rows_end = decl
            .row_lines
            .last()
            .or(decl.separator_line.as_ref())
            .map_or(decl.header_line.end, |s| s.end) as usize;
        let computed_end = decl.computed.iter().map(|c| c.expr_span.end as usize).max();
        let (at, gap) = match computed_end {
            Some(end) if end > rows_end => (end, "\n"),
            _ => (rows_end, "\n\n"),
        };
        let mut text = self.text.clone();
        text.insert_str(at, &format!("{gap}{name} := {}", expr.replace('\n', " ")));
        self.commit(text);
        Ok(())
    }

    /// Change the expression of a computed column.
    pub fn set_column_formula(&mut self, table: usize, col: usize, expr: &str) -> bool {
        let expr = expr
            .trim()
            .trim_start_matches('=')
            .trim()
            .replace('\n', " ");
        if expr.is_empty() {
            return false;
        }
        let ast = self.ast();
        let Some(decl) = self.table_decl(&ast, table) else {
            return false;
        };
        let Some(name) = self.snapshot.tables[table]
            .columns
            .get(col)
            .map(|c| &c.name)
        else {
            return false;
        };
        let Some(cd) = decl.computed.iter().find(|c| &c.name == name) else {
            return false;
        };
        let mut text = self.text.clone();
        text.replace_range(
            cd.expr_span.start as usize..cd.expr_span.end as usize,
            &expr,
        );
        self.commit(text)
    }

    pub fn set_const(&mut self, index: usize, expr: &str) -> bool {
        let expr = expr
            .trim()
            .trim_start_matches('=')
            .trim()
            .replace('\n', " ");
        let ast = self.ast();
        let Some(name) = self.snapshot.consts.get(index).map(|c| &c.name) else {
            return false;
        };
        let Some(cd) = ast.consts.iter().find(|c| &c.name == name) else {
            return false;
        };
        if expr.is_empty() {
            return false;
        }
        let mut text = self.text.clone();
        text.replace_range(
            cd.expr_span.start as usize..cd.expr_span.end as usize,
            &expr,
        );
        self.commit(text)
    }

    pub fn add_const(&mut self, name: &str, expr: &str) -> Result<(), String> {
        let name = valid_name(name)?.to_string();
        let expr = expr.trim().trim_start_matches('=').trim();
        if expr.is_empty() {
            return Err("a constant needs an expression".into());
        }
        let taken = self.snapshot.consts.iter().any(|c| c.name == name)
            || self.snapshot.tables.iter().any(|t| t.name == name);
        if taken {
            return Err(format!("the name `{name}` is already used"));
        }
        // Constants go first, before the first table.
        let ast = self.ast();
        let at = ast
            .tables
            .first()
            .map(|t| {
                self.text[..t.span.start as usize]
                    .rfind('\n')
                    .map_or(0, |i| i + 1)
            })
            .unwrap_or(self.text.len());
        let mut text = self.text.clone();
        text.insert_str(
            at,
            &format!("const {name} = {}\n\n", expr.replace('\n', " ")),
        );
        self.commit(text);
        Ok(())
    }

    /// Append a new table with one empty row.
    pub fn add_table(&mut self, name: &str) -> Result<(), String> {
        let name = valid_name(name)?.to_string();
        let taken = self.snapshot.consts.iter().any(|c| c.name == name)
            || self.snapshot.tables.iter().any(|t| t.name == name);
        if taken {
            return Err(format!("the name `{name}` is already used"));
        }
        let mut text = self.text.trim_end().to_string();
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&format!(
            "table {name}\n\nA | B | C\n  |   |\n  |   |\n  |   |\n"
        ));
        self.commit(text);
        Ok(())
    }

    fn rebuild(&mut self) {
        let mut sources = Sources::new();
        let src = sources.add("sheet", self.text.as_str());
        let (program, mut diags) = compile(&self.text, src);
        let engine = Engine::new(&program);
        engine.run();
        diags.extend(engine.take_diags());
        let ast = self.ast();

        let message = |d: &Diagnostic| match &d.help {
            Some(help) => format!("{} ({help})", d.message),
            None => d.message.clone(),
        };
        let within = |span: Span| diags.iter().find(|d| contains(span, d.span)).map(message);
        let show = |v: &Value| match v {
            Value::Table(_) => "<table>".to_string(),
            Value::Row(_) => "<row>".to_string(),
            other => format_scalar(other, false),
        };

        let mut snapshot = Snapshot::default();
        for (t, table) in program.tables.iter().enumerate() {
            let decl = ast.tables.iter().find(|d| d.name == table.name);
            let mut columns = Vec::new();
            let mut column_errors = Vec::new();
            for col in &table.cols {
                let computed = decl
                    .filter(|_| matches!(col.kind, ColKind::Computed(_)))
                    .and_then(|d| d.computed.iter().find(|c| c.name == col.name));
                columns.push(ColumnSnap {
                    name: col.name.clone(),
                    ty: if col.ty == S::Any {
                        String::new()
                    } else {
                        col.ty.to_string()
                    },
                    formula: computed.map(|c| {
                        self.text[c.expr_span.start as usize..c.expr_span.end as usize].to_string()
                    }),
                });
                column_errors.push(computed.and_then(|c| within(c.expr_span)));
            }
            let mut rows = Vec::with_capacity(table.nrows);
            for r in 0..table.nrows {
                let mut row = Vec::with_capacity(table.cols.len());
                for (c, column) in columns.iter().enumerate() {
                    let value = engine.cell_shown(t, c, r);
                    let failed = matches!(value, Value::Error);
                    let src_cell = decl
                        .filter(|d| c < d.header.len())
                        .and_then(|d| d.rows.get(r))
                        .and_then(|row| row.get(c));
                    let (source, error) = match (&column.formula, src_cell) {
                        (Some(formula), _) => (
                            format!("= {formula}"),
                            failed.then(|| column_errors[c].clone()).flatten(),
                        ),
                        (None, Some(cell)) => (cell.text.clone(), within(cell.span)),
                        (None, None) => (String::new(), None),
                    };
                    row.push(CellSnap {
                        display: show(&value),
                        numeric: value.is_numeric(),
                        formula: source.starts_with('='),
                        error: error.or_else(|| {
                            failed.then(|| "this cell could not be calculated".to_string())
                        }),
                        source,
                    });
                }
                rows.push(row);
            }
            snapshot.tables.push(TableSnap {
                name: table.name.clone(),
                columns,
                rows,
            });
        }
        for (i, c) in program.consts.iter().enumerate() {
            let decl = ast.consts.iter().find(|d| d.name == c.name);
            let value = engine.const_value(i).unwrap_or(Value::Error);
            snapshot.consts.push(ConstSnap {
                name: c.name.clone(),
                source: decl.map_or(String::new(), |d| {
                    self.text[d.expr_span.start as usize..d.expr_span.end as usize].to_string()
                }),
                display: match &value {
                    Value::Empty => "empty".to_string(),
                    Value::Vector(_) | Value::Text(_) => format_scalar(&value, true),
                    other => show(other),
                },
                error: decl.and_then(|d| within(d.expr_span)),
            });
        }
        snapshot.problems = diags
            .iter()
            .map(|d| {
                let (line, column) = sources.line_col(d.span.src, d.span.start);
                Problem {
                    line,
                    column,
                    message: message(d),
                }
            })
            .collect();
        self.snapshot = snapshot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALES: &str = "# sales\nconst Rate = 20%\n\ntable Sales\n\nMonth | Revenue | Cost\nJan   | 100     | 60\nFeb   | 120     | 70\n\nProfit := Revenue - Cost\nTax := Profit * Rate\n";

    fn col(doc: &Document, table: usize, col: usize) -> Vec<String> {
        doc.snapshot().tables[table]
            .rows
            .iter()
            .map(|r| r[col].display.clone())
            .collect()
    }

    #[test]
    fn snapshot_shows_values_and_sources() {
        let doc = Document::from_text(SALES);
        let t = &doc.snapshot().tables[0];
        assert_eq!(t.name, "Sales");
        assert_eq!(t.columns.len(), 5);
        assert_eq!(t.columns[3].formula.as_deref(), Some("Revenue - Cost"));
        assert_eq!(col(&doc, 0, 3), ["40", "50"]);
        assert_eq!(t.rows[0][1].source, "100");
        assert_eq!(t.rows[0][3].source, "= Revenue - Cost");
        assert!(t.rows[0][1].numeric && !t.rows[0][0].numeric);
        assert_eq!(doc.snapshot().consts[0].display, "0.2");
        assert!(doc.snapshot().problems.is_empty());
    }

    #[test]
    fn editing_a_cell_changes_one_line() {
        let mut doc = Document::from_text(SALES);
        assert!(doc.set_cell(0, 1, 1, "130"));
        assert_eq!(doc.text(), SALES.replace("120", "130"));
        assert_eq!(col(&doc, 0, 3), ["40", "60"]);
        assert!(doc.undo());
        assert_eq!(doc.text(), SALES);
        assert!(doc.redo());
        assert_eq!(col(&doc, 0, 4), ["8", "12"]);
    }

    #[test]
    fn formula_cells_and_wider_cells() {
        let mut doc = Document::from_text(SALES);
        doc.set_cell(0, 1, 1, "= Sales[*-1; Revenue] * 2");
        assert_eq!(col(&doc, 0, 1), ["100", "200"]);
        assert!(
            doc.text()
                .contains("Jan   | 100                       | 60")
        );
        assert!(doc.text().contains("# sales\nconst Rate = 20%"));
        assert!(
            doc.text()
                .ends_with("Profit := Revenue - Cost\nTax := Profit * Rate\n")
        );
    }

    #[test]
    fn rows_can_be_added_pasted_and_deleted() {
        let mut doc = Document::from_text(SALES);
        doc.set_cells(
            0,
            &[
                (2, 0, "Mar".into()),
                (2, 1, "150".into()),
                (2, 2, "80".into()),
                (3, 0, "Apr".into()),
            ],
        );
        assert_eq!(col(&doc, 0, 0), ["Jan", "Feb", "Mar", "Apr"]);
        assert_eq!(col(&doc, 0, 3), ["40", "50", "70", ""]);
        assert!(
            doc.text().contains(
                "Feb   | 120     | 70\nMar   | 150     | 80\nApr   |         |\n\nProfit"
            )
        );
        doc.insert_rows(0, 0, 1);
        assert_eq!(col(&doc, 0, 0), ["", "Jan", "Feb", "Mar", "Apr"]);
        doc.delete_rows(0, 0, 2);
        assert_eq!(col(&doc, 0, 0), ["Feb", "Mar", "Apr"]);
        assert!(doc.text().contains("Month | Revenue | Cost\nFeb"));
    }

    #[test]
    fn columns_constants_and_tables() {
        let mut doc = Document::from_text(SALES);
        doc.add_column(0, "Note").unwrap();
        assert!(doc.add_column(0, "Note").is_err());
        assert!(doc.add_column(0, "bad name").is_err());
        doc.set_cell(0, 0, 3, "a | b");
        assert_eq!(doc.snapshot().tables[0].rows[0][3].display, "a | b");
        doc.add_computed(0, "Margin", "Profit / Revenue").unwrap();
        let names: Vec<_> = doc.snapshot().tables[0]
            .columns
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(
            names,
            [
                "Month", "Revenue", "Cost", "Note", "Profit", "Tax", "Margin"
            ]
        );
        assert_eq!(col(&doc, 0, 6), ["0.4", "5/12"]);
        doc.set_cell(0, 0, 6, "Profit / Cost");
        assert_eq!(col(&doc, 0, 6), ["2/3", "5/7"]);
        doc.set_const(0, "50%");
        assert_eq!(col(&doc, 0, 5), ["20", "25"]);
        doc.add_const("Extra", "Sales.Revenue.sum()").unwrap();
        assert_eq!(doc.snapshot().consts[1].display, "220");
        doc.add_table("Notes").unwrap();
        assert_eq!(doc.snapshot().tables[1].rows.len(), 3);
        assert!(
            doc.snapshot().problems.is_empty(),
            "{:?}",
            doc.snapshot().problems
        );
    }

    #[test]
    fn errors_do_not_blank_the_grid() {
        let mut doc = Document::from_text(SALES);
        doc.set_cell(0, 0, 1, "= Revnue + 1");
        let t = &doc.snapshot().tables[0];
        assert_eq!(t.rows[0][1].display, "#ERROR");
        assert!(t.rows[0][1].error.as_ref().unwrap().contains("Revnue"));
        assert_eq!(t.rows[1][3].display, "50");
        assert_eq!(doc.snapshot().problems.len(), 1);
        assert_eq!(doc.snapshot().problems[0].line, 7);
    }

    #[test]
    fn blank_document_is_valid() {
        let mut doc = Document::default();
        assert!(doc.snapshot().problems.is_empty());
        assert_eq!(doc.snapshot().tables[0].rows.len(), 5);
        doc.set_cell(0, 0, 0, "1");
        doc.set_cell(0, 0, 1, "= A + 1");
        assert_eq!(doc.snapshot().tables[0].rows[0][1].display, "2");
    }
}

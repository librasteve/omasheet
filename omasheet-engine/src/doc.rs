//! An editable sheet for interactive front ends.
//!
//! The `.omx` text stays the single source of truth: every edit rewrites the
//! text and the sheet is then re-read and re-evaluated. A [`Snapshot`] is what
//! a grid shows: for each cell, the value to display and the source to edit.
//!
//! Unlike the command line, a document is evaluated even when it has errors,
//! so that one mistyped formula does not blank the rest of the grid; cells
//! that could not be calculated show as errors.

use crate::Options;
use crate::eval::Engine;
use crate::value::Value;
use omasheet_omx::date::{self, Style};
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
    style: Style,
    /// Show date-times on the clocks of this machine.
    local: bool,
}

impl Default for Document {
    fn default() -> Self {
        Document::from_text(BLANK)
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    inner.start >= outer.start && inner.start <= outer.end
}

/// A date or time typed the way `style` writes it (`31/1/2025`, `9:30 pm`),
/// as a sheet writes it, for a column of type `ty`. `None` if it is not one.
fn iso_entry(style: &Style, ty: &str, input: &str) -> Option<String> {
    let text = input.trim();
    let date = || style.parse_date(text).map(date::format);
    let time = || style.parse_time(text).map(date::format_time);
    let datetime = || style.parse_datetime(text).map(date::format_datetime);
    match ty {
        "Date" => date(),
        "Time" => time(),
        // A date alone is the start of that day.
        "DateTime" => datetime().or_else(|| Some(format!("{}T00:00", date()?))),
        // With no type to go by, only what cannot be anything else: a date
        // needs its year in full.
        "" => {
            let full_year = text
                .split(|c: char| !c.is_ascii_digit())
                .any(|part| part.len() == 4);
            datetime().or_else(date).filter(|_| full_year).or_else(time)
        }
        _ => None,
    }
}

/// What can be typed into a cell of a column of type `ty`, with an example
/// written the way `style` reads it. Empty when the type is not known.
pub fn entry_hint(style: &Style, ty: &str) -> String {
    // A day past the twelfth, so the order of day and month is plain.
    let day = date::from_ymd(2025, 1, 31).unwrap_or(0);
    let time = date::time_from(17, 5, 0).unwrap_or(0);
    let also_iso = if style.is_iso() { "" } else { " or ISO" };
    match ty {
        "Int" => "Int: a whole number, e.g. 42".into(),
        "Rat" => "Rat: an exact number, e.g. 19.99, 20% or 1/7".into(),
        "Num" => "Num: a floating-point number, e.g. 1.5 or 2e-3".into(),
        "Bool" => "Bool: true or false".into(),
        "Text" => "Text".into(),
        "Date" => format!(
            "Date: {}, e.g. {}{also_iso}",
            style.date_pattern(),
            style.date(day)
        ),
        "Time" => format!("Time: e.g. {}", style.time(time)),
        "DateTime" => format!(
            "DateTime: e.g. {}{also_iso}",
            style.datetime(day as i64 * 86_400 + time as i64)
        ),
        _ => String::new(),
    }
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
    /// A document that shows dates and times as the sheet writes them.
    pub fn from_text(text: &str) -> Document {
        Document::new(text, Style::ISO, false)
    }

    /// A document for a person to look at: dates and times are shown in
    /// `style`, and date-times on the clocks of this machine even when the
    /// sheet is in another time zone. See [`Document::set_style`].
    pub fn with_style(text: &str, style: Style) -> Document {
        Document::new(text, style, true)
    }

    fn new(text: &str, style: Style, local: bool) -> Document {
        let mut doc = Document {
            text: text.to_string(),
            snapshot: Snapshot::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            style,
            local,
        };
        doc.rebuild();
        doc
    }

    /// How dates and times are shown, and how they may be typed into a cell.
    /// The text of the sheet stays ISO whatever the style.
    pub fn set_style(&mut self, style: Style) {
        self.style = style;
        self.rebuild();
    }

    pub fn style(&self) -> &Style {
        &self.style
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
        let style = self.style;
        let types: Vec<String> = self
            .snapshot
            .tables
            .get(table)
            .map(|t| t.columns.iter().map(|c| c.ty.clone()).collect())
            .unwrap_or_default();
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
                let ty = types.get(*col).map_or("", String::as_str);
                let cleaned = iso_entry(&style, ty, text).unwrap_or_else(|| clean_cell(text));
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

    /// Rename a column, and every reference to it, in one undoable step.
    pub fn rename_column(&mut self, table: usize, col: usize, name: &str) -> Result<(), String> {
        let new = valid_name(name)?.to_string();
        let Some(snap) = self.snapshot.tables.get(table) else {
            return Err("there is no such table".into());
        };
        let Some(old) = snap.columns.get(col).map(|c| c.name.clone()) else {
            return Err("there is no such column".into());
        };
        if new == old {
            return Ok(());
        }
        if snap.columns.iter().any(|c| c.name == new) {
            return Err(format!(
                "table `{}` already has a column `{new}`",
                snap.name
            ));
        }
        let taken = self.snapshot.consts.iter().any(|c| c.name == new)
            || self.snapshot.tables.iter().any(|t| t.name == new);
        if taken {
            return Err(format!("`{new}` is already a constant or a table"));
        }
        let ast = self.ast();
        let Some(decl) = self.table_decl(&ast, table) else {
            return Err("there is no such table".into());
        };

        // The declaration: the header cell or `Name :=`, and its type line.
        let header = decl.header.iter().filter(|h| h.0 == old).map(|h| h.1);
        let computed = decl.computed.iter().filter(|c| c.name == old);
        let schema = decl.schema.iter().filter(|l| l.name == old);
        let mut spans: Vec<Span> = header
            .chain(computed.map(|c| c.span))
            .chain(schema.map(|l| l.span))
            .collect();
        // Every place a formula names this column, as the checker resolved it.
        let (program, _) = compile(&self.text, 0);
        let column = program
            .tables
            .iter()
            .position(|t| t.name == decl.name)
            .and_then(|t| {
                Some((
                    t,
                    program.tables[t].cols.iter().position(|c| c.name == old)?,
                ))
            });
        let uses = |p: &omasheet_omx::Program| {
            let mut found: Vec<Span> = Vec::new();
            if let Some((t, c)) = column {
                let refs = p.col_refs.iter().filter(|r| (r.1, r.2) == (t, c));
                for r in refs {
                    if !found.contains(&r.0) {
                        found.push(r.0);
                    }
                }
            }
            found
        };
        let before = uses(&program);
        spans.extend(before.iter().copied());
        let mut starts: Vec<usize> = spans
            .iter()
            .map(|s| s.start as usize)
            .filter(|s| self.text[*s..].starts_with(&old))
            .collect();
        starts.sort_unstable();
        starts.dedup();
        if starts.is_empty() {
            return Err(format!("column `{old}` cannot be renamed"));
        }
        let mut text = self.text.clone();
        let mut touched: Vec<&str> = Vec::new();
        for &s in starts.iter().rev() {
            let owner = ast.tables.iter().rfind(|t| t.span.start as usize <= s);
            if let Some(t) = owner.filter(|t| !touched.contains(&t.name.as_str())) {
                touched.push(&t.name);
            }
            text.replace_range(s..s + old.len(), &new);
        }
        // The new name must not be one that a formula already reads as
        // something else, or that formula would silently change meaning.
        if uses(&compile(&text, 0).0).len() != before.len() {
            return Err(format!(
                "renaming to `{new}` would change what a formula refers to"
            ));
        }
        // Line the changed tables up again.
        for name in touched {
            let ast = parse_sheet(&text, 0, &mut Vec::new());
            if let Some(decl) = ast.tables.iter().find(|t| t.name == name) {
                text = Grid::of(decl).write(decl, &text);
            }
        }
        self.commit(text);
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
        let engine = Engine::with_options(
            &program,
            Options {
                style: self.style,
                local: self.local,
                ..Options::default()
            },
        );
        engine.run();
        diags.extend(engine.take_diags());
        let ast = self.ast();

        // A cell that does not fit its column: say what the column takes,
        // written the way the user types it.
        for table in &program.tables {
            let Some(decl) = ast.tables.iter().find(|d| d.name == table.name) else {
                continue;
            };
            for (cell, col) in decl.rows.iter().flat_map(|row| row.iter().zip(&table.cols)) {
                let hint = entry_hint(&self.style, &col.ty.to_string());
                let misfits = diags
                    .iter_mut()
                    .filter(|d| d.span == cell.span && d.message.ends_with("` literal"));
                for d in misfits.filter(|_| !hint.is_empty()) {
                    d.message = format!("`{}` does not fit column `{}`", cell.text, col.name);
                    d.help = Some(format!(
                        "{hint}; to calculate a value, start the cell with `=`"
                    ));
                }
            }
        }

        let message = |d: &Diagnostic| match &d.help {
            Some(help) => format!("{} ({help})", d.message),
            None => d.message.clone(),
        };
        let within = |span: Span| diags.iter().find(|d| contains(span, d.span)).map(message);
        let show = |v: &Value| match v {
            Value::Table(_) => "<table>".to_string(),
            Value::Row(_) => "<row>".to_string(),
            other => engine.show(other, false),
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
                    Value::Vector(_) | Value::Text(_) => engine.show(&value, true),
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
    fn renaming_a_column_follows_its_references() {
        let text = format!(
            "{SALES}\ntable Top\n\nKind | Value\nSum  | = Sales.Revenue.sum()\nBig  | = Sales[Revenue > 100; Revenue].sum()\n\nRevenue := Value\n"
        );
        let mut doc = Document::from_text(&text);
        let before = col(&doc, 0, 3);
        doc.rename_column(0, 1, "Income").unwrap();
        let renamed = doc.text().to_string();
        assert!(renamed.contains("Month | Income | Cost\nJan   | 100    | 60\n"));
        assert!(renamed.contains("Profit := Income - Cost"));
        assert!(renamed.contains("= Sales.Income.sum()"));
        assert!(renamed.contains("= Sales[Income > 100; Income].sum()"));
        // Another table's own column of the same name is left alone.
        assert!(renamed.contains("Revenue := Value"));
        assert!(
            doc.snapshot().problems.is_empty(),
            "{:?}",
            doc.snapshot().problems
        );
        assert_eq!(col(&doc, 0, 3), before);
        assert_eq!(col(&doc, 1, 1), ["220", "120"]);

        // A computed column.
        doc.rename_column(0, 3, "Gain").unwrap();
        assert!(
            doc.text()
                .contains("Gain := Income - Cost\nTax := Gain * Rate")
        );
        assert!(doc.snapshot().problems.is_empty());

        assert!(doc.rename_column(0, 1, "Cost").is_err());
        assert!(doc.rename_column(0, 1, "Rate").is_err());
        assert!(doc.rename_column(0, 1, "bad name").is_err());
        // `Value` inside `Sales[...]` would be captured by the renamed column.
        let capture =
            format!("{SALES}\ntable Top\n\nValue\n100\n\nN := Sales[Revenue > Value].count()\n");
        let mut other = Document::from_text(&capture);
        assert!(other.rename_column(0, 1, "Value").is_err());
        assert_eq!(other.text(), capture);
        // Each rename is one undo step.
        doc.undo();
        assert_eq!(doc.text(), renamed);
        doc.undo();
        assert_eq!(doc.text(), text);
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
    fn dates_follow_the_style_but_the_text_stays_iso() {
        use omasheet_omx::date::Order;
        let text = "table Log\n\nDay : Date\nAt : Time\nSeen : DateTime\n\n\
                    Day | At | Seen | Note\n2026-10-08 | 17:47 | 2026-10-08T17:47 | a\n";
        let mut doc = Document::from_text(text);
        let row = |doc: &Document, r: usize| -> Vec<String> {
            let cells = &doc.snapshot().tables[0].rows[r];
            cells.iter().map(|c| c.display.clone()).collect()
        };
        assert_eq!(
            row(&doc, 0),
            ["2026-10-08", "17:47", "2026-10-08T17:47", "a"]
        );

        doc.set_style(Style::new(Order::Dmy, '/', false));
        assert_eq!(
            row(&doc, 0),
            ["08/10/2026", "17:47", "08/10/2026 17:47", "a"]
        );
        assert_eq!(doc.snapshot().tables[0].rows[0][0].source, "2026-10-08");
        assert_eq!(doc.text(), text);

        // Typed the British way, stored the ISO way.
        doc.set_cells(
            0,
            &[
                (1, 0, "9/10/26".to_string()),
                (1, 1, "5:05 pm".to_string()),
                (1, 2, "9/10/2026 8:00".to_string()),
                (1, 3, "1/2/3".to_string()),
            ],
        );
        assert!(
            doc.text()
                .contains("2026-10-09 | 17:05 | 2026-10-09T08:00 | 1/2/3")
        );
        assert_eq!(
            row(&doc, 1),
            ["09/10/2026", "17:05", "09/10/2026 08:00", "1/2/3"]
        );
        doc.set_cell(0, 1, 2, "10/10/2026");
        assert!(doc.text().contains("2026-10-10T00:00"));
        assert!(doc.snapshot().problems.is_empty());

        // The same keys mean another day in the United States.
        doc.set_style(Style::new(Order::Mdy, '/', true));
        assert_eq!(row(&doc, 0)[2], "10/08/2026 5:47 PM");
        doc.set_cell(0, 0, 0, "3/4/2026");
        assert!(doc.text().contains("2026-03-04 | 17:47"));

        // A column with no type takes only what must be a date or a time.
        let mut loose = Document::from_text("table T\n\nA | B\n  |\n  |\n");
        loose.set_style(Style::new(Order::Dmy, '/', false));
        loose.set_cells(
            0,
            &[
                (0, 0, "8/10/2026".to_string()),
                (0, 1, "9:30".to_string()),
                (1, 0, "= 1/2".to_string()),
                (1, 1, "10.11.12".to_string()),
            ],
        );
        assert!(loose.text().contains("2026-10-08 | 09:30"));
        assert!(loose.text().contains("= 1/2"));
        assert!(loose.text().contains("10.11.12"));
    }

    #[test]
    fn entry_hints_follow_the_style() {
        let us = Style::new(date::Order::Mdy, '/', true);
        assert_eq!(
            entry_hint(&us, "Date"),
            "Date: MM/DD/YYYY, e.g. 01/31/2025 or ISO"
        );
        assert_eq!(entry_hint(&us, "Time"), "Time: e.g. 5:05 PM");
        assert_eq!(
            entry_hint(&us, "DateTime"),
            "DateTime: e.g. 01/31/2025 5:05 PM or ISO"
        );
        assert_eq!(
            entry_hint(&Style::ISO, "DateTime"),
            "DateTime: e.g. 2025-01-31T17:05"
        );
        assert_eq!(entry_hint(&us, ""), "");
        // A cell that does not fit says what would.
        let mut doc = Document::with_style("table T\n\nOn : Date\n\nOn\n2025-01-31\n", us);
        doc.set_cell(0, 0, 0, "soon");
        assert_eq!(
            doc.snapshot().tables[0].rows[0][0].error.as_deref(),
            Some(
                "`soon` does not fit column `On` (Date: MM/DD/YYYY, e.g. 01/31/2025 or ISO; \
                 to calculate a value, start the cell with `=`)"
            )
        );
        assert!(
            doc.snapshot().problems[0]
                .message
                .contains("does not fit column `On`")
        );
        // Every example is accepted as an entry for its type.
        for ty in ["Date", "Time", "DateTime"] {
            let hint = entry_hint(&us, ty);
            let example = hint
                .split("e.g. ")
                .nth(1)
                .unwrap()
                .trim_end_matches(" or ISO");
            assert!(iso_entry(&us, ty, example).is_some(), "{hint}");
        }
    }

    #[test]
    fn a_sheet_in_another_zone_is_shown_on_local_clocks() {
        // A zone whose clocks this machine does not keep, wherever it is.
        let zone = match crate::zone::Zone::system().from_utc(0) {
            Some(32_400) => "America/New_York",
            _ => "Asia/Tokyo",
        };
        let text = format!("zone {zone}\n\ntable T\n\nAt\n2025-07-15T12:00\n");
        // As the sheet writes it, for tools.
        let plain = Document::from_text(&text);
        assert_eq!(
            plain.snapshot().tables[0].rows[0][0].display,
            "2025-07-15T12:00"
        );
        // For a person, on this machine's clocks; the source is untouched.
        let mut doc = Document::with_style(&text, Style::ISO);
        let cell = doc.snapshot().tables[0].rows[0][0].clone();
        assert_eq!(cell.source, "2025-07-15T12:00");
        assert_ne!(cell.display, "2025-07-15T12:00");
        // What is typed is in the sheet's zone, as the source shows it.
        doc.set_cell(0, 0, 0, "2025-07-15T13:00");
        assert!(doc.text().contains("\n2025-07-15T13:00\n"));

        let bad = Document::from_text("zone Mars/Base\n\ntable T\n\nA\n1\n");
        assert!(bad.snapshot().problems[0].message.contains("no time zone"));
        assert_eq!(bad.snapshot().tables[0].rows[0][0].display, "1");
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

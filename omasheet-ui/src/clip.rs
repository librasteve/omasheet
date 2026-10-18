// Copyright (c) 2026 Stephen Roe

//! Cut, copy and paste over a [`Document`]: the text that goes to the
//! clipboard and comes back from it, and what is remembered of where it
//! came from. Nothing here knows of Qt, so all of it is tested here.

use omasheet_engine::Document;

/// A cell: its row and column.
type Cell = (usize, usize);

/// A block of cells that was copied.
struct Copied {
    /// What went to the clipboard: the cells as they are written.
    text: String,
    /// The same cells as they were shown.
    values: String,
}

/// A block of cells that was cut, to be moved by the next paste.
struct Cut {
    sheet: usize,
    /// The top left cell.
    at: Cell,
    /// What went to the clipboard.
    text: String,
    /// The cells to clear when they are pasted.
    blanks: Vec<(usize, usize, String)>,
    /// The file as the cells were cut: a paste takes them from where they
    /// were only if nothing has changed since.
    before: String,
}

/// What was last cut or copied in this app, beside the clipboard's text.
#[derive(Default)]
pub struct Clip {
    /// The cells last cut, until they are pasted.
    cut: Option<Cut>,
    /// The cells last copied.
    copied: Option<Copied>,
}

/// Tab-separated text as rows of cells.
pub fn parse_block(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_suffix('\n').unwrap_or(text);
    text.split('\n')
        .map(|line| {
            line.trim_end_matches('\r')
                .split('\t')
                .map(str::to_string)
                .collect()
        })
        .collect()
}

/// A block turned about: its rows as columns. Short rows are filled out.
pub fn transposed(block: &[Vec<String>]) -> Vec<Vec<String>> {
    let width = block.iter().map(Vec::len).max().unwrap_or(0);
    (0..width)
        .map(|j| {
            let cell = |line: &Vec<String>| line.get(j).cloned().unwrap_or_default();
            block.iter().map(cell).collect()
        })
        .collect()
}

/// A block of cells as tab-separated text: what they showed if `values`,
/// otherwise what they hold, and a cell of a formula column its formula.
fn block_text(doc: &Document, sheet: usize, from: Cell, to: Cell, values: bool) -> String {
    let Some(t) = doc.snapshot().sheets.get(sheet) else {
        return String::new();
    };
    let mut lines = Vec::new();
    for row in t.rows.iter().take(to.0 + 1).skip(from.0) {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .take(to.1 + 1)
            .skip(from.1)
            // A value in full, where it is shown with fewer digits than it
            // has.
            .map(|(c, cell)| match &t.columns[c].formula {
                _ if values => cell.exact.clone().unwrap_or_else(|| cell.display.clone()),
                Some(formula) => format!("= {formula}"),
                None => cell.source.clone(),
            })
            .collect();
        lines.push(cells.join("\t"));
    }
    lines.join("\n")
}

/// The cells of a block that can be cleared, each to be set to nothing.
fn blanks(doc: &Document, sheet: usize, from: Cell, to: Cell) -> Vec<(usize, usize, String)> {
    let mut cells = Vec::new();
    let Some(t) = doc.snapshot().sheets.get(sheet) else {
        return cells;
    };
    if t.rows.is_empty() {
        return cells;
    }
    for row in from.0..=to.0.min(t.rows.len() - 1) {
        for col in from.1..=to.1 {
            if t.columns.get(col).is_some_and(|c| c.formula.is_none()) {
                cells.push((row, col, String::new()));
            }
        }
    }
    cells
}

/// Why `block` cannot be pasted at a block, if every column it would land
/// on is a formula column: there is nowhere for any of it to go. A block
/// that lands partly on data columns is pasted there, and the rest left.
fn refused(
    doc: &Document,
    sheet: usize,
    from: Cell,
    to: Cell,
    block: &[Vec<String>],
) -> Result<(), String> {
    let Some(t) = doc.snapshot().sheets.get(sheet) else {
        return Ok(());
    };
    let single = block.len() == 1 && block[0].len() == 1;
    let width = block.iter().map(Vec::len).max().unwrap_or(0);
    let last = if single {
        to.1.max(from.1)
    } else {
        from.1 + width.max(1) - 1
    };
    let onto: Vec<_> = t.columns.iter().take(last + 1).skip(from.1).collect();
    match onto.first() {
        Some(first) if onto.iter().all(|c| c.formula.is_some()) => Err(format!(
            "`{}` is a formula column: only a formula can be pasted there, \
             starting with `=`",
            first.name
        )),
        _ => Ok(()),
    }
}

/// The formula columns that one formula pasted at a block would be the
/// formula of: `None` unless it is a formula and lands on them alone.
fn formula_target(
    doc: &Document,
    sheet: usize,
    from: Cell,
    to: Cell,
    block: &[Vec<String>],
) -> Option<Vec<usize>> {
    let [line] = block else { return None };
    let [text] = line.as_slice() else { return None };
    if !text.trim_start().starts_with('=') {
        return None;
    }
    let t = doc.snapshot().sheets.get(sheet)?;
    let cols: Vec<usize> = (from.1..=to.1.max(from.1)).collect();
    let all = cols
        .iter()
        .all(|&c| t.columns.get(c).is_some_and(|c| c.formula.is_some()));
    all.then_some(cols)
}

/// Put a block of cells down at a block; a single cell fills it. Cells
/// that were `cut` are cleared first, in the same step.
fn put_block(
    doc: &mut Document,
    sheet: usize,
    from: Cell,
    to: Cell,
    block: Vec<Vec<String>>,
    cut: Option<Cut>,
) -> bool {
    let single = block.len() == 1 && block[0].len() == 1;
    let computed: Vec<bool> = doc
        .snapshot()
        .sheets
        .get(sheet)
        .map(|t| t.columns.iter().map(|c| c.formula.is_some()).collect())
        .unwrap_or_default();
    let mut cells = Vec::new();
    let mut put = |row: usize, col: usize, text: &str| {
        if computed.get(col) == Some(&false) {
            cells.push((row, col, text.to_string()));
        }
    };
    if single {
        for row in from.0..=to.0.max(from.0) {
            for col in from.1..=to.1.max(from.1) {
                put(row, col, &block[0][0]);
            }
        }
    } else {
        // A copy onto a smaller block is cut to its size; onto one cell,
        // or a larger block, it goes down whole.
        let clip = cut.is_none() && from != to;
        let (rows, cols) = match clip {
            true => (to.0 - from.0 + 1, to.1 - from.1 + 1),
            false => (usize::MAX, usize::MAX),
        };
        for (i, line) in block.iter().enumerate().take(rows) {
            for (j, text) in line.iter().enumerate().take(cols) {
                put(from.0 + i, from.1 + j, text);
            }
        }
    }
    doc.as_one(|doc| {
        if let Some(cut) = &cut {
            doc.set_cells(cut.sheet, &cut.blanks);
        }
        doc.set_cells(sheet, &cells);
    })
}

impl Clip {
    /// A block of cells as the clipboard holds it, with nothing remembered.
    pub fn text(doc: &Document, sheet: usize, from: Cell, to: Cell) -> String {
        block_text(doc, sheet, from, to, false)
    }

    /// Copy a block of cells: as whole columns, if `columns`. Anything cut
    /// is cut no longer.
    pub fn copy(&mut self, doc: &Document, sheet: usize, from: Cell, to: Cell) -> String {
        let text = block_text(doc, sheet, from, to, false);
        let values = block_text(doc, sheet, from, to, true);
        self.cut = None;
        self.copied = Some(Copied {
            text: text.clone(),
            values,
        });
        text
    }

    /// Cut a block of cells. They stay where they are until they are
    /// pasted, which is when they are cleared.
    pub fn cut(&mut self, doc: &Document, sheet: usize, from: Cell, to: Cell) -> String {
        let text = Self::text(doc, sheet, from, to);
        self.cut = Some(Cut {
            sheet,
            at: from,
            text: text.clone(),
            blanks: blanks(doc, sheet, from, to),
            before: doc.text().to_string(),
        });
        text
    }

    /// Leave what was cut where it is.
    pub fn forget_cut(&mut self) {
        self.cut = None;
    }

    /// The cells that were cut, if `block` from the clipboard is still them
    /// and nothing has been done since. They are cut no longer.
    fn take_cut(&mut self, doc: &Document, block: &[Vec<String>]) -> Option<Cut> {
        let cut = self.cut.take()?;
        (parse_block(&cut.text) == block && cut.before == doc.text()).then_some(cut)
    }

    /// What was last copied, if `text` from the clipboard is still that.
    fn copied(&self, text: &str) -> Option<&Copied> {
        let copied = self.copied.as_ref()?;
        (parse_block(&copied.text) == parse_block(text)).then_some(copied)
    }

    /// Paste `text` at a block of cells. Whether anything changed, or why
    /// it could not be pasted: what was cut is then still cut.
    pub fn paste(
        &mut self,
        doc: &mut Document,
        sheet: usize,
        from: Cell,
        to: Cell,
        text: &str,
    ) -> Result<bool, String> {
        let block = parse_block(text);
        // One formula onto formula columns is their formula, and the cell
        // it was cut from is cleared, in one step.
        if let Some(cols) = formula_target(doc, sheet, from, to, &block) {
            let cut = self.take_cut(doc, &block);
            return Ok(doc.as_one(|doc| {
                for &col in &cols {
                    doc.set_column_formula(sheet, col, &block[0][0]);
                }
                if let Some(cut) = &cut {
                    doc.set_cells(cut.sheet, &cut.blanks);
                }
            }));
        }
        refused(doc, sheet, from, to, &block)?;
        // Cells cut from this sheet, with nothing done since, are moved;
        // one cell onto several fills them all.
        let cut = self.take_cut(doc, &block);
        let fills = block.len() == 1 && block[0].len() == 1 && from != to;
        if let Some(cut) = &cut
            && cut.sheet == sheet
            && !fills
        {
            return Ok(doc.as_one(|doc| {
                doc.set_cells(cut.sheet, &cut.blanks);
                doc.paste_cut(cut.sheet, cut.at, from, &block);
            }));
        }
        Ok(put_block(doc, sheet, from, to, block, cut))
    }

    /// Paste what the copied cells showed, not their formulas.
    pub fn paste_values(
        &mut self,
        doc: &mut Document,
        sheet: usize,
        from: Cell,
        to: Cell,
        text: &str,
    ) -> Result<bool, String> {
        let values = self.copied(text).map(|c| c.values.clone());
        let block = parse_block(values.as_deref().unwrap_or(text));
        refused(doc, sheet, from, to, &block)?;
        let cut = self.take_cut(doc, &parse_block(text));
        Ok(put_block(doc, sheet, from, to, block, cut))
    }

    /// Paste `text` turned about: its rows as columns.
    pub fn paste_transposed(
        &mut self,
        doc: &mut Document,
        sheet: usize,
        from: Cell,
        to: Cell,
        text: &str,
    ) -> Result<bool, String> {
        let block = parse_block(text);
        let turned = transposed(&block);
        refused(doc, sheet, from, to, &turned)?;
        let cut = self.take_cut(doc, &block);
        Ok(put_block(doc, sheet, from, to, turned, cut))
    }

    /// Empty a block of cells. Whether anything changed.
    pub fn clear(doc: &mut Document, sheet: usize, from: Cell, to: Cell) -> bool {
        let cells = blanks(doc, sheet, from, to);
        !cells.is_empty() && doc.set_cells(sheet, &cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "sheet T\n\nA  | B  | C     | Sum\n1  | 2  | = A+B | *\n10 | 20 |       | *\n   |    |       | *\n\nSum := A + B\n\nsheet U\n\nX | Y\n  |\n  |\n";

    fn doc() -> Document {
        Document::from_text(TEXT)
    }

    /// What the cells of a sheet hold, row by row, joined by `|`. A cell
    /// of a formula column holds nothing of its own.
    fn held(doc: &Document, sheet: usize) -> Vec<String> {
        let t = &doc.snapshot().sheets[sheet];
        let mut lines = Vec::new();
        for row in &t.rows {
            let mut cells = Vec::new();
            for (c, cell) in row.iter().enumerate() {
                let own = t.columns[c].formula.is_none();
                cells.push(if own { cell.source.as_str() } else { "" });
            }
            lines.push(cells.join("|"));
        }
        lines
    }

    fn names(doc: &Document, sheet: usize) -> Vec<String> {
        let columns = &doc.snapshot().sheets[sheet].columns;
        columns.iter().map(|c| c.name.clone()).collect()
    }

    #[test]
    fn parses_clipboard_blocks() {
        assert_eq!(parse_block("a\tb\r\nc\t\n"), [["a", "b"], ["c", ""]]);
        assert_eq!(parse_block("x"), [["x"]]);
    }

    #[test]
    fn transposes_blocks() {
        let block = parse_block("a\tb\tc\nd");
        assert_eq!(transposed(&block), [["a", "d"], ["b", ""], ["c", ""]]);
        assert_eq!(transposed(&parse_block("x")), [["x"]]);
    }

    #[test]
    fn copied_cells_paste_as_often_as_wanted() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (0, 0), (0, 1));
        assert_eq!(text, "1\t2");
        assert_eq!(clip.paste(&mut doc, 0, (1, 0), (1, 0), &text), Ok(true));
        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 0), &text), Ok(true));
        // The source is as it was, and so is every paste.
        assert_eq!(held(&doc, 0)[0], "1|2|= A+B|");
        assert_eq!(held(&doc, 0)[1], "1|2||");
        assert_eq!(held(&doc, 0)[2], "1|2||");
        // Pasting what is already there changes nothing.
        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 0), &text), Ok(false));
    }

    #[test]
    fn a_formula_is_copied_as_written_and_a_formula_column_as_its_formula() {
        let doc = doc();
        let mut clip = Clip::default();
        assert_eq!(clip.copy(&doc, 0, (0, 2), (0, 3)), "= A+B\t= A + B");
        assert_eq!(Clip::text(&doc, 0, (0, 0), (1, 1)), "1\t2\n10\t20");
    }

    #[test]
    fn cut_cells_stay_until_they_are_pasted() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.cut(&doc, 0, (0, 0), (0, 1));
        assert_eq!(text, "1\t2");
        // Nothing has moved yet, and there is nothing to undo.
        assert_eq!(doc.text(), TEXT);
        assert!(!doc.can_undo());

        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 0), &text), Ok(true));
        // The formula that read them reads them where they went.
        assert_eq!(held(&doc, 0)[0], "||= [A; *+2]+[B; *+2]|");
        assert_eq!(held(&doc, 0)[2], "1|2||");
        assert_eq!(doc.snapshot().sheets[0].rows[0][2].display, "3");
        // The clearing and the pasting are one step.
        assert!(doc.undo());
        assert_eq!(doc.text(), TEXT);
        assert!(!doc.can_undo());
    }

    #[test]
    fn a_single_cell_cut_is_cleared_where_it_was() {
        // To another cell of the sheet.
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.cut(&doc, 0, (1, 0), (1, 0));
        assert_eq!(clip.paste(&mut doc, 0, (2, 1), (2, 1), &text), Ok(true));
        assert_eq!(held(&doc, 0)[1], "|20||");
        assert_eq!(held(&doc, 0)[2], "|10||");
        // Onto a block: it fills every cell of it.
        let mut doc = self::doc();
        let text = clip.cut(&doc, 0, (1, 0), (1, 0));
        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 1), &text), Ok(true));
        assert_eq!(held(&doc, 0)[1], "|20||");
        assert_eq!(held(&doc, 0)[2], "10|10||");
        // To another sheet.
        let mut doc = self::doc();
        let text = clip.cut(&doc, 0, (1, 0), (1, 0));
        assert_eq!(clip.paste(&mut doc, 1, (0, 1), (0, 1), &text), Ok(true));
        assert_eq!(held(&doc, 0)[1], "|20||");
        assert_eq!(held(&doc, 1)[0], "|10");
        // A cell holding a formula.
        let mut doc = self::doc();
        let text = clip.cut(&doc, 0, (0, 2), (0, 2));
        assert_eq!(clip.paste(&mut doc, 0, (1, 2), (1, 2), &text), Ok(true));
        assert_eq!(held(&doc, 0)[0], "1|2||");
        assert_eq!(held(&doc, 0)[1], "10|20|= A+B|");
    }

    #[test]
    fn a_formula_cut_onto_a_formula_column_is_cleared_where_it_was() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.cut(&doc, 0, (0, 2), (0, 2));
        assert_eq!(clip.paste(&mut doc, 0, (1, 3), (1, 3), &text), Ok(true));
        assert!(doc.text().contains("Sum := A+B"));
        assert_eq!(held(&doc, 0)[0], "1|2||");
        assert!(doc.undo());
        assert_eq!(doc.text(), TEXT);
    }

    #[test]
    fn a_copy_onto_a_smaller_block_is_cut_to_its_size() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (0, 0), (1, 1));
        // Onto one row of two cells: only the first row of the copy.
        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 1), &text), Ok(true));
        assert_eq!(held(&doc, 0).len(), 3);
        assert_eq!(held(&doc, 0)[2], "1|2||");
        // Onto one cell: the whole of it, from there.
        assert!(doc.undo());
        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 0), &text), Ok(true));
        assert_eq!(held(&doc, 0)[2], "1|2||");
        assert_eq!(held(&doc, 0)[3], "10|20||");
    }

    #[test]
    fn cut_cells_are_moved_once() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.cut(&doc, 0, (1, 0), (1, 1));
        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 0), &text), Ok(true));
        assert_eq!(held(&doc, 0)[1], "|||");
        // A second paste is of a copy: where they went is not cleared.
        assert_eq!(clip.paste(&mut doc, 0, (1, 0), (1, 0), &text), Ok(true));
        assert_eq!(held(&doc, 0)[1], "10|20||");
        assert_eq!(held(&doc, 0)[2], "10|20||");
    }

    #[test]
    fn a_formula_follows_cells_cut_and_pasted_in_its_sheet() {
        let mut doc = Document::from_text("sheet T\n\nA | B\n1 | = [A; 0] * 2\n  |\n");
        let mut clip = Clip::default();
        let text = clip.cut(&doc, 0, (0, 0), (0, 0));
        assert_eq!(clip.paste(&mut doc, 0, (1, 0), (1, 0), &text), Ok(true));
        assert_eq!(held(&doc, 0), ["|= [A; 1] * 2", "1|"]);
        assert_eq!(doc.snapshot().sheets[0].rows[0][1].display, "2");
    }

    #[test]
    fn cut_cells_go_to_another_sheet() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.cut(&doc, 0, (1, 0), (1, 1));
        assert_eq!(clip.paste(&mut doc, 1, (0, 0), (0, 0), &text), Ok(true));
        assert_eq!(held(&doc, 0)[1], "|||");
        assert_eq!(held(&doc, 1)[0], "10|20");
        assert!(doc.undo());
        assert_eq!(doc.text(), TEXT);
    }

    #[test]
    fn a_cut_is_dropped_by_an_edit_another_copy_or_esc() {
        // An edit since: the cells are pasted, and stay where they were.
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.cut(&doc, 0, (0, 0), (0, 1));
        doc.set_cell(0, 2, 1, "9");
        assert_eq!(clip.paste(&mut doc, 0, (1, 0), (1, 0), &text), Ok(true));
        assert_eq!(held(&doc, 0)[0], "1|2|= A+B|");
        assert_eq!(held(&doc, 0)[1], "1|2||");

        // Something else on the clipboard since.
        let mut doc = self::doc();
        clip.cut(&doc, 0, (0, 0), (0, 1));
        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 0), "x"), Ok(true));
        assert_eq!(held(&doc, 0)[0], "1|2|= A+B|");
        assert_eq!(held(&doc, 0)[2], "x|||");

        // Let go of, as Esc does.
        let mut doc = self::doc();
        let text = clip.cut(&doc, 0, (0, 0), (0, 1));
        clip.forget_cut();
        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 0), &text), Ok(true));
        assert_eq!(held(&doc, 0)[0], "1|2|= A+B|");

        // A copy after a cut.
        let mut doc = self::doc();
        clip.cut(&doc, 0, (0, 0), (0, 1));
        let text = clip.copy(&doc, 0, (0, 0), (0, 1));
        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 0), &text), Ok(true));
        assert_eq!(held(&doc, 0)[0], "1|2|= A+B|");
    }

    #[test]
    fn one_cell_fills_a_block_and_a_block_starts_at_its_corner() {
        let mut doc = doc();
        let mut clip = Clip::default();
        assert_eq!(clip.paste(&mut doc, 0, (1, 0), (2, 1), "7"), Ok(true));
        assert_eq!(held(&doc, 0)[1], "7|7||");
        assert_eq!(held(&doc, 0)[2], "7|7||");
        // A block goes down whole, whatever is selected.
        assert_eq!(
            clip.paste(&mut doc, 0, (1, 0), (1, 0), "a\tb\nc\td"),
            Ok(true)
        );
        assert_eq!(held(&doc, 0)[1], "a|b||");
        assert_eq!(held(&doc, 0)[2], "c|d||");
    }

    #[test]
    fn a_formula_column_is_not_pasted_over() {
        let mut doc = doc();
        let mut clip = Clip::default();
        assert_eq!(clip.paste(&mut doc, 0, (1, 2), (1, 2), "x\ty"), Ok(true));
        assert_eq!(held(&doc, 0)[1], "10|20|x|");
        assert_eq!(names(&doc, 0), ["A", "B", "C", "Sum"]);
        assert!(doc.snapshot().sheets[0].columns[3].formula.is_some());
    }

    #[test]
    fn a_formula_pasted_onto_a_formula_column_is_its_formula() {
        let mut doc = doc();
        let mut clip = Clip::default();
        assert_eq!(clip.paste(&mut doc, 0, (1, 3), (1, 3), "= A * B"), Ok(true));
        assert!(doc.text().contains("Sum := A * B"));
        assert_eq!(doc.snapshot().sheets[0].rows[1][3].display, "200");
        // In one step.
        doc.undo();
        assert_eq!(doc.text(), TEXT);
    }

    #[test]
    fn nothing_is_pasted_onto_a_formula_column_alone() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let why = Err(
            "`Sum` is a formula column: only a formula can be pasted there, starting with `=`"
                .to_string(),
        );
        assert_eq!(clip.paste(&mut doc, 0, (0, 3), (0, 3), "5"), why);
        assert_eq!(clip.paste(&mut doc, 0, (0, 3), (2, 3), "5\n6"), why);
        assert_eq!(clip.paste_values(&mut doc, 0, (0, 3), (0, 3), "5"), why);
        assert_eq!(
            clip.paste_transposed(&mut doc, 0, (0, 3), (0, 3), "5\n6"),
            why
        );
        assert_eq!(doc.text(), TEXT);
        // What was cut is still cut, to be pasted somewhere it can go.
        let text = clip.cut(&doc, 0, (0, 0), (0, 0));
        assert_eq!(clip.paste(&mut doc, 0, (1, 3), (1, 3), &text), why);
        assert_eq!(doc.text(), TEXT);
        assert_eq!(clip.paste(&mut doc, 0, (2, 0), (2, 0), &text), Ok(true));
        assert_eq!(held(&doc, 0)[0], "|2|= [A; *+2]+B|");
        assert_eq!(held(&doc, 0)[2], "1|||");
        // A block that reaches a data column goes there, and no further.
        let mut doc = self::doc();
        assert_eq!(clip.paste(&mut doc, 0, (1, 2), (1, 2), "x\ty"), Ok(true));
        assert_eq!(held(&doc, 0)[1], "10|20|x|");
        // Turned about, two cells in a row go down the one column.
        assert_eq!(
            clip.paste_transposed(&mut doc, 0, (0, 3), (0, 3), "5\t6"),
            why
        );
    }

    #[test]
    fn values_are_pasted_in_place_of_formulas() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (0, 0), (0, 2));
        assert_eq!(text, "1\t2\t= A+B");
        assert_eq!(
            clip.paste_values(&mut doc, 0, (2, 0), (2, 0), &text),
            Ok(true)
        );
        assert_eq!(held(&doc, 0)[2], "1|2|3|");
        // Text from elsewhere is its own value.
        assert_eq!(
            clip.paste_values(&mut doc, 0, (1, 0), (1, 0), "= 1+1"),
            Ok(true)
        );
        assert_eq!(held(&doc, 0)[1], "= 1+1|20||");
    }

    #[test]
    fn a_block_is_pasted_turned_about() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (0, 0), (0, 1));
        assert_eq!(
            clip.paste_transposed(&mut doc, 1, (0, 0), (0, 0), &text),
            Ok(true)
        );
        assert_eq!(held(&doc, 1), ["1|", "2|"]);
        // Cut cells are cleared as they are put down, in one step.
        let text = clip.cut(&doc, 0, (1, 0), (1, 1));
        assert_eq!(
            clip.paste_transposed(&mut doc, 1, (0, 1), (0, 1), &text),
            Ok(true)
        );
        assert_eq!(held(&doc, 1), ["1|10", "2|20"]);
        assert_eq!(held(&doc, 0)[1], "|||");
        assert!(doc.undo());
        assert_eq!(held(&doc, 0)[1], "10|20||");
        assert_eq!(held(&doc, 1), ["1|", "2|"]);
    }

    #[test]
    fn clearing_empties_cells_but_not_a_formula_column() {
        let mut doc = doc();
        assert!(Clip::clear(&mut doc, 0, (0, 0), (2, 3)));
        assert_eq!(held(&doc, 0), ["|||", "|||", "|||"]);
        assert!(doc.snapshot().sheets[0].columns[3].formula.is_some());
        assert!(!Clip::clear(&mut doc, 0, (0, 0), (2, 3)));
        assert!(!Clip::clear(&mut doc, 0, (0, 3), (2, 3)));
    }
}

// Copyright (c) 2026 Stephen Roe

//! Cut, copy and paste over a [`Document`]: the text that goes to the
//! clipboard and comes back from it, and what is remembered of where it
//! came from. Nothing here knows of Qt, so all of it is tested here.

use omasheet_engine::Document;
use omasheet_engine::doc::{CopiedFrom, carried, carried_turned, copied_cell};

/// A cell: its row and column.
type Cell = (usize, usize);

/// A block of cells that was copied.
struct Copied {
    sheet: usize,
    /// The top left cell.
    at: Cell,
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

/// Where the cells last cut or copied came from, for a reference to them
/// (D209). The clipboard keeps only their text, so this is kept beside it.
#[derive(Clone)]
struct Referent {
    /// What went to the clipboard, to tell that it is still these.
    text: String,
    /// The sheet they were on.
    sheet: String,
    /// Their columns, left to right.
    cols: Vec<String>,
    /// Every column of the sheet, for the names a formula reads there.
    sheet_cols: Vec<String>,
    /// How many rows the sheet had, and where the block began on it.
    sheet_rows: usize,
    at: Cell,
    /// The row of the source each came from: for a table sorted in the
    /// grid, not the row it is shown in (D215).
    rows: Vec<usize>,
}

/// What was last cut or copied in this app, beside the clipboard's text.
#[derive(Default)]
pub struct Clip {
    /// The cells last cut, until they are pasted.
    cut: Option<Cut>,
    /// The cells last copied.
    copied: Option<Copied>,
    /// Where the cells last cut or copied came from.
    refer: Option<Referent>,
}

/// What a formula pasted from another sheet needs to be carried over to
/// where it lands (D214).
struct Carry {
    /// Whether the cells were cut, and so move, and not copied (D221).
    cut: bool,
    from: CopiedFrom,
    /// The top left cell of the block, on the sheet it came from.
    origin: Cell,
    /// The columns, and the rows, of the sheet it lands on.
    target_cols: Vec<String>,
    target_rows: usize,
}

impl Carry {
    /// `text`, the cell `(i, j)` of the block, put down at `at`.
    fn cell(&self, text: &str, (i, j): Cell, at: Cell) -> String {
        let was = (self.origin.0 + i, self.origin.1 + j);
        if self.cut {
            carried(
                text,
                was,
                &self.from,
                at,
                &self.target_cols,
                self.target_rows,
            )
        } else {
            copied_cell(text, was, &self.from, at, &self.target_cols)
        }
    }
}

impl Carry {
    /// As `cell`, for a block put down turned about with its first cell at
    /// `origin`: `text` is the cell `(i, j)` of the block, put down at `at`.
    fn turned_cell(&self, text: &str, (i, j): Cell, origin: Cell, at: Cell) -> String {
        let was = (self.origin.0 + i, self.origin.1 + j);
        carried_turned(
            text,
            was,
            &self.from,
            origin,
            at,
            &self.target_cols,
            self.target_rows,
        )
    }
}

/// A reference to one cell, `Sales[Revenue; 3]`, or `[Revenue; 3]` from the
/// sheet itself (D213).
fn cell_ref(same: bool, sheet: &str, col: &str, row: &str) -> String {
    if same {
        format!("[{col}; {row}]")
    } else {
        format!("{sheet}[{col}; {row}]")
    }
}

/// A reference to a column, read row by row: `*` for the row itself, and
/// `*+5` or `*-5` for one that many rows below or above it. The column
/// alone from its own sheet, as `Fare`.
fn column_ref(same: bool, sheet: &str, col: &str, offset: i64) -> String {
    let row = match offset {
        0 if same => return col.to_string(),
        0 => "*".to_string(),
        n if n > 0 => format!("*+{n}"),
        n => format!("*{n}"),
    };
    cell_ref(same, sheet, col, &row)
}

/// Where a block of cells came from, as it is copied.
fn referent(doc: &Document, sheet: usize, from: Cell, to: Cell, text: &str) -> Option<Referent> {
    let t = doc.snapshot().sheets.get(sheet)?;
    let last = to.0.min(t.nrows.checked_sub(1)?);
    if from.0 > last {
        return None;
    }
    let cols: Vec<String> = t
        .columns
        .iter()
        .take(to.1 + 1)
        .skip(from.1)
        .map(|c| c.name.clone())
        .collect();
    (!cols.is_empty()).then(|| Referent {
        text: text.to_string(),
        sheet: t.name.clone(),
        sheet_cols: t.columns.iter().map(|c| c.name.clone()).collect(),
        sheet_rows: t.nrows,
        at: from,
        cols,
        rows: (from.0..=last).map(|r| doc.source_row(sheet, r)).collect(),
    })
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
    // Out of the document rather than the snapshot: a paged sheet's rows
    // are formatted as they are asked for.
    for row in &doc.rows(sheet, from.0, (to.0 + 1).saturating_sub(from.0)) {
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

/// The formulas that cells pasted onto formula columns alone make theirs:
/// `(column, formula)`. Each column of the block is one formula, the same
/// in every row; one column of them fills every column it is pasted on.
/// `None` if they are not all formulas, or land on a data column.
fn formula_target(
    doc: &Document,
    sheet: usize,
    from: Cell,
    to: Cell,
    block: &[Vec<String>],
) -> Result<Option<Vec<(usize, String)>>, String> {
    let width = block.iter().map(Vec::len).max().unwrap_or(0);
    let mut formulas = Vec::new();
    let mut differ = false;
    for j in 0..width {
        let texts: Vec<&str> = block
            .iter()
            .map(|line| line.get(j).map_or("", |t| t.trim()))
            .collect();
        if !texts.iter().all(|t| t.starts_with('=')) {
            return Ok(None);
        }
        differ |= texts.iter().any(|t| *t != texts[0]);
        formulas.push(texts[0].to_string());
    }
    let Some(t) = doc.snapshot().sheets.get(sheet) else {
        return Ok(None);
    };
    let cols: Vec<usize> = match width {
        0 => return Ok(None),
        1 => (from.1..=to.1.max(from.1)).collect(),
        _ => (from.1..from.1 + width).collect(),
    };
    let all = cols
        .iter()
        .all(|&c| t.columns.get(c).is_some_and(|c| c.formula.is_some()));
    if !all {
        return Ok(None);
    }
    if differ {
        return Err("Cannot paste: these cells have different formulas in each row".into());
    }
    let formula = |k: usize| formulas[k.min(width - 1)].clone();
    Ok(Some(
        cols.iter()
            .enumerate()
            .map(|(k, &c)| (c, formula(k)))
            .collect(),
    ))
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
    carry: Option<&Carry>,
) -> bool {
    let single = block.len() == 1 && block[0].len() == 1;
    let computed: Vec<bool> = doc
        .snapshot()
        .sheets
        .get(sheet)
        .map(|t| t.columns.iter().map(|c| c.formula.is_some()).collect())
        .unwrap_or_default();
    let mut cells = Vec::new();
    // `(i, j)` is the cell of the block the text is, which a formula needs
    // to know to be carried over from another sheet.
    let mut put = |row: usize, col: usize, text: &str, of: Cell| {
        if computed.get(col) == Some(&false) {
            let text = match carry {
                Some(carry) => carry.cell(text, of, (row, col)),
                None => text.to_string(),
            };
            cells.push((row, col, text));
        }
    };
    if single {
        for row in from.0..=to.0.max(from.0) {
            for col in from.1..=to.1.max(from.1) {
                put(row, col, &block[0][0], (0, 0));
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
                put(from.0 + i, from.1 + j, text, (i, j));
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
        self.refer = referent(doc, sheet, from, to, &text);
        self.cut = None;
        self.copied = Some(Copied {
            sheet,
            at: from,
            text: text.clone(),
            values,
        });
        text
    }

    /// Cut a block of cells. They stay where they are until they are
    /// pasted, which is when they are cleared.
    pub fn cut(&mut self, doc: &Document, sheet: usize, from: Cell, to: Cell) -> String {
        let text = Self::text(doc, sheet, from, to);
        self.refer = referent(doc, sheet, from, to, &text);
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

    /// What a formula needs to be carried to `sheet` from the sheet the cells
    /// `block` were last cut or copied on, if that is another sheet.
    fn carry(&self, doc: &Document, sheet: usize, block: &[Vec<String>]) -> Option<Carry> {
        let t = doc.snapshot().sheets.get(sheet)?;
        let r = self
            .refer
            .as_ref()
            .filter(|r| parse_block(&r.text) == block)?;
        Some(Carry {
            cut: false,
            from: CopiedFrom {
                sheet: r.sheet.clone(),
                cols: r.sheet_cols.clone(),
                nrows: r.sheet_rows,
                rows: r.at.0..r.at.0 + r.rows.len(),
                across: r.at.1..r.at.1 + r.cols.len(),
            },
            origin: r.at,
            target_cols: t.columns.iter().map(|c| c.name.clone()).collect(),
            target_rows: t.nrows,
        })
    }

    /// `block`, the cells last cut or copied, with the formulas in it made to
    /// read what they read on the sheet they came from, where they are put
    /// down on another one (D214). As it was for anything else. For a formula
    /// column, whose one formula is every row's.
    fn read_where_it_was(
        &self,
        doc: &Document,
        sheet: usize,
        block: &[Vec<String>],
    ) -> Vec<Vec<String>> {
        let here = doc.snapshot().sheets.get(sheet).map(|t| t.name.as_str());
        let Some(r) = self
            .refer
            .as_ref()
            .filter(|r| Some(r.sheet.as_str()) != here && parse_block(&r.text) == block)
        else {
            return block.to_vec();
        };
        block
            .iter()
            .map(|line| {
                line.iter()
                    .map(|cell| omasheet_engine::doc::qualified(cell, &r.sheet, &r.sheet_cols))
                    .collect()
            })
            .collect()
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
        let original = parse_block(text);
        // A formula copied from another sheet is made to read where it
        // read (D214): into a formula column it is the column's formula, and
        // reads the sheet it came from; in cells, what it read inside the
        // block it reads at the copy. What was cut is known by the text it
        // was copied as.
        let block = self.read_where_it_was(doc, sheet, &original);
        let carry = self.carry(doc, sheet, &original);
        // One formula onto formula columns is their formula, and the cell
        // it was cut from is cleared, in one step.
        if let Some(formulas) = formula_target(doc, sheet, from, to, &block)? {
            let cut = self.take_cut(doc, &original);
            return Ok(doc.as_one(|doc| {
                for (col, formula) in &formulas {
                    doc.set_column_formula(sheet, *col, formula);
                }
                if let Some(cut) = &cut {
                    doc.set_cells(cut.sheet, &cut.blanks);
                }
            }));
        }
        refused(doc, sheet, from, to, &block)?;
        // Cells cut from this sheet, with nothing done since, are moved;
        // one cell onto several fills them all.
        let cut = self.take_cut(doc, &original);
        let carry = carry.map(|c| Carry {
            cut: cut.is_some(),
            ..c
        });
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
        Ok(put_block(
            doc,
            sheet,
            from,
            to,
            original,
            cut,
            carry.as_ref(),
        ))
    }

    /// Paste a live reference to the cells last copied or cut here (D210).
    /// Into data cells each gets a formula for the cell it stands for; into
    /// a formula column the column's formula is set, aligned by row
    /// (D212). What was written, or why nothing was.
    pub fn paste_reference(
        &mut self,
        doc: &mut Document,
        sheet: usize,
        from: Cell,
        to: Cell,
        text: &str,
    ) -> Result<Option<String>, String> {
        let Some(r) = self
            .refer
            .clone()
            .filter(|r| parse_block(&r.text) == parse_block(text))
        else {
            return Err("Copy cells here first: a reference is to cells copied in this app".into());
        };
        let Some(t) = doc.snapshot().sheets.get(sheet) else {
            return Ok(None);
        };
        let same = t.name == r.sheet;
        let single = r.rows.len() == 1 && r.cols.len() == 1;
        // One cell fills the block it is pasted on; a block goes down whole
        // from the first cell, cut to the size of a larger one selected.
        let clip = !single && from != to;
        let (rows, cols) = if single {
            (to.0.max(from.0) - from.0 + 1, to.1.max(from.1) - from.1 + 1)
        } else if clip {
            (
                r.rows.len().min(to.0 - from.0 + 1),
                r.cols.len().min(to.1 - from.1 + 1),
            )
        } else {
            (r.rows.len(), r.cols.len())
        };
        let mut formulas = Vec::new();
        let mut cells = Vec::new();
        for j in 0..cols {
            let col = from.1 + j;
            let Some(target) = t.columns.get(col) else {
                break;
            };
            let name = &r.cols[if single { 0 } else { j }];
            if target.formula.is_some() {
                let expr = if r.rows.len() == 1 {
                    // One row has nothing to align with: the cell itself.
                    cell_ref(same, &r.sheet, name, &r.rows[0].to_string())
                } else {
                    if r.rows.windows(2).any(|w| w[1] != w[0] + 1) {
                        return Err("Cannot paste a reference to rows in a sorted order: \
                             copy one cell, or clear the sort"
                            .into());
                    }
                    column_ref(same, &r.sheet, name, r.rows[0] as i64 - from.0 as i64)
                };
                formulas.push((col, expr));
            } else if let Some(why) = doc.read_only_cell(sheet, col) {
                return Err(why);
            } else {
                for i in 0..rows {
                    let row = r.rows[if single { 0 } else { i }];
                    let formula = cell_ref(same, &r.sheet, name, &row.to_string());
                    cells.push((from.0 + i, col, format!("= {formula}")));
                }
            }
        }
        if formulas.is_empty() && cells.is_empty() {
            return Ok(None);
        }
        let said = formulas.first().map(|(_, e)| e.clone()).or_else(|| {
            cells
                .first()
                .map(|(_, _, e)| e.trim_start_matches("= ").to_string())
        });
        let changed = doc.as_one(|doc| {
            for (col, expr) in &formulas {
                doc.set_column_formula(sheet, *col, expr);
            }
            doc.set_cells(sheet, &cells);
        });
        // What was cut stays where it is: a reference moves nothing.
        self.cut = None;
        Ok(changed.then(|| said.map_or_else(String::new, |s| format!("Pasted `{s}`"))))
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
        Ok(put_block(doc, sheet, from, to, block, cut, None))
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
        // Cells of this sheet keep reading what they read, now where it
        // has been turned to: a formula is not the same text once the
        // cells it reads have gone elsewhere.
        let origin = match &cut {
            Some(cut) => Some((cut.sheet, cut.at)),
            None => self.copied_from(doc, &block),
        };
        let Some((_, at)) = origin.filter(|(s, _)| *s == sheet) else {
            // From another sheet, a formula is made to read what it read
            // there (D214), and what it read in the block, where that went.
            let single = turned.len() == 1 && turned[0].len() == 1;
            let carry = self.carry(doc, sheet, &block);
            let turned = match &carry {
                Some(carry) if !single => turned
                    .iter()
                    .enumerate()
                    .map(|(j, line)| {
                        let cell = |(i, text): (usize, &String)| {
                            carry.turned_cell(text, (i, j), from, (from.0 + j, from.1 + i))
                        };
                        line.iter().enumerate().map(cell).collect()
                    })
                    .collect(),
                _ => turned,
            };
            let carry = carry.filter(|_| single);
            return Ok(put_block(doc, sheet, from, to, turned, cut, carry.as_ref()));
        };
        if let Some(cut) = &cut {
            return Ok(doc.as_one(|doc| {
                doc.set_cells(cut.sheet, &cut.blanks);
                doc.paste_cut_turned(sheet, at, from, &block);
            }));
        }
        let turned = doc.turn_block(sheet, at, from, &block);
        Ok(put_block(doc, sheet, from, to, turned, None, None))
    }

    /// Where `block` was copied from, if the cells there are still it.
    fn copied_from(&self, doc: &Document, block: &[Vec<String>]) -> Option<(usize, Cell)> {
        let copied = self.copied.as_ref()?;
        if parse_block(&copied.text) != block {
            return None;
        }
        let width = block.iter().map(Vec::len).max().unwrap_or(0);
        let last = (copied.at.0 + block.len() - 1, copied.at.1 + width - 1);
        let held = block_text(doc, copied.sheet, copied.at, last, false);
        (parse_block(&held) == block).then_some((copied.sheet, copied.at))
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
        // It reads the cells it read, now a row up (D221).
        assert_eq!(held(&doc, 0)[1], "10|20|= [A; *-1]+[B; *-1]|");
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
    fn columns_paste_as_a_block_does() {
        const COLS: &str = "sheet V\n\nA | B | C | D | E\n1 | 2 | 3 | 4 | 5\n6 | 7 | 8 | 9 | 10\n";
        let mut doc = Document::from_text(COLS);
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (0, 0), (1, 1));
        // Onto one column, given as its top cell: both, from there.
        assert_eq!(clip.paste(&mut doc, 0, (0, 2), (0, 2), &text), Ok(true));
        assert_eq!(held(&doc, 0), ["1|2|1|2|5", "6|7|6|7|10"]);
        // Onto fewer columns: cut to them.
        assert!(doc.undo());
        assert_eq!(clip.paste(&mut doc, 0, (0, 2), (1, 2), &text), Ok(true));
        assert_eq!(held(&doc, 0), ["1|2|1|4|5", "6|7|6|9|10"]);
        // Onto as many.
        assert!(doc.undo());
        assert_eq!(clip.paste(&mut doc, 0, (0, 2), (1, 3), &text), Ok(true));
        assert_eq!(held(&doc, 0), ["1|2|1|2|5", "6|7|6|7|10"]);
        // Onto more: whole, and the rest left.
        assert!(doc.undo());
        assert_eq!(clip.paste(&mut doc, 0, (0, 2), (1, 4), &text), Ok(true));
        assert_eq!(held(&doc, 0), ["1|2|1|2|5", "6|7|6|7|10"]);
        // Onto the last column: what goes past it is left.
        assert!(doc.undo());
        assert_eq!(clip.paste(&mut doc, 0, (0, 4), (0, 4), &text), Ok(true));
        assert_eq!(held(&doc, 0), ["1|2|3|4|1", "6|7|8|9|6"]);
    }

    #[test]
    fn formula_cells_are_pasted_onto_formula_columns() {
        let two =
            "sheet T\n\nA | B | S | P\n1 | 2 | * | *\n3 | 4 | * | *\n\nS := A + B\nP := A * B\n";
        let mut doc = Document::from_text(two);
        let mut clip = Clip::default();
        // A block of one formula column onto another: its formula.
        let text = clip.copy(&doc, 0, (0, 2), (1, 2));
        assert_eq!(clip.paste(&mut doc, 0, (0, 3), (1, 3), &text), Ok(true));
        assert!(doc.text().contains("P := A + B"));
        assert!(doc.undo());
        // Both, each onto its own.
        let text = "= A - B\t= A / B\n= A - B\t= A / B";
        assert_eq!(clip.paste(&mut doc, 0, (0, 2), (0, 2), text), Ok(true));
        assert!(doc.text().contains("S := A - B\nP := A / B"));
        assert!(doc.undo());
        // Formulas that differ from row to row are not one formula.
        let why = Err("Cannot paste: these cells have different formulas in each row".to_string());
        assert_eq!(clip.paste(&mut doc, 0, (0, 2), (1, 2), "= A\n= B"), why);
        assert_eq!(doc.text(), two);
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

    #[test]
    fn a_reference_is_pasted_as_formulas_into_data_cells_of_another_sheet() {
        let mut doc = doc();
        let mut clip = Clip::default();
        // T[A; 0..1] copied, and pasted as a reference onto U.
        let text = clip.copy(&doc, 0, (0, 0), (1, 0));
        let said = clip.paste_reference(&mut doc, 1, (0, 0), (0, 0), &text);
        assert_eq!(said, Ok(Some("Pasted `T[A; 0]`".to_string())));
        assert_eq!(held(&doc, 1)[0], "= T[A; 0]|");
        assert_eq!(held(&doc, 1)[1], "= T[A; 1]|");
        // The cells are live: they show what T holds.
        let shown = &doc.snapshot().sheets[1].rows;
        assert_eq!(
            (shown[0][0].display.as_str(), shown[1][0].display.as_str()),
            ("1", "10")
        );
        // Ctrl+V of the same copy is the values, as before.
        let mut other = self::doc();
        assert_eq!(clip.paste(&mut other, 1, (0, 0), (0, 0), &text), Ok(true));
        assert_eq!(held(&other, 1)[0], "1|");
    }

    #[test]
    fn one_cell_as_a_reference_fills_the_block_it_is_pasted_on() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (1, 1), (1, 1));
        assert_eq!(
            clip.paste_reference(&mut doc, 1, (0, 1), (1, 1), &text)
                .map(|s| s.is_some()),
            Ok(true)
        );
        assert_eq!(held(&doc, 1)[0], "|= T[B; 1]");
        assert_eq!(held(&doc, 1)[1], "|= T[B; 1]");
        // From the sheet itself the sheet's name is left out (D213).
        let mut doc = self::doc();
        let text = clip.copy(&doc, 0, (1, 1), (1, 1));
        assert_eq!(
            clip.paste_reference(&mut doc, 0, (2, 0), (2, 0), &text)
                .map(|s| s.is_some()),
            Ok(true)
        );
        assert_eq!(held(&doc, 0)[2], "= [B; 1]|||");
    }

    #[test]
    fn a_column_as_a_reference_sets_a_formula_columns_formula_aligned_by_row() {
        let mut doc = doc();
        let mut clip = Clip::default();
        // A whole column from its own sheet: just its name.
        let text = clip.copy(&doc, 0, (0, 1), (2, 1));
        let said = clip.paste_reference(&mut doc, 0, (0, 3), (0, 3), &text);
        assert_eq!(said, Ok(Some("Pasted `B`".to_string())));
        assert!(doc.text().contains("Sum := B\n"), "{}", doc.text());
        // Rows 1 to 2 pasted from row 0 are read from one row below.
        let text = clip.copy(&doc, 0, (1, 0), (2, 0));
        let said = clip.paste_reference(&mut doc, 0, (0, 3), (0, 3), &text);
        assert_eq!(said, Ok(Some("Pasted `[A; *+1]`".to_string())));
        assert!(doc.text().contains("Sum := [A; *+1]\n"), "{}", doc.text());
        assert_eq!(col_display(&doc, 0, 3), ["10", "", ""]);
        // And from below, the offset is the other way.
        let text = clip.copy(&doc, 0, (0, 0), (0, 0));
        let said = clip.paste_reference(&mut doc, 0, (0, 3), (0, 3), &text);
        assert_eq!(said, Ok(Some("Pasted `[A; 0]`".to_string())));
        let text = clip.copy(&doc, 0, (0, 0), (1, 0));
        let said = clip.paste_reference(&mut doc, 0, (1, 3), (1, 3), &text);
        assert_eq!(said, Ok(Some("Pasted `[A; *-1]`".to_string())));
        assert!(doc.text().contains("Sum := [A; *-1]\n"), "{}", doc.text());
    }

    fn col_display(doc: &Document, sheet: usize, col: usize) -> Vec<String> {
        doc.snapshot().sheets[sheet]
            .rows
            .iter()
            .map(|r| r[col].display.clone())
            .collect()
    }

    #[test]
    fn a_reference_needs_cells_copied_here() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let said = clip.paste_reference(&mut doc, 1, (0, 0), (0, 0), "7");
        assert!(said.is_err());
        // The clipboard having changed since: not these cells any more.
        clip.copy(&doc, 0, (0, 0), (0, 0));
        assert!(
            clip.paste_reference(&mut doc, 1, (0, 0), (0, 0), "somewhere else")
                .is_err()
        );
        assert_eq!(doc.text(), TEXT);
    }

    #[test]
    fn a_cut_as_a_reference_moves_nothing() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.cut(&doc, 0, (0, 0), (0, 0));
        let said = clip.paste_reference(&mut doc, 1, (0, 0), (0, 0), &text);
        assert_eq!(said, Ok(Some("Pasted `T[A; 0]`".to_string())));
        assert_eq!(held(&doc, 0)[0], "1|2|= A+B|");
        // It is not cut any more: a paste of the text copies it.
        assert_eq!(clip.paste(&mut doc, 1, (1, 0), (1, 0), &text), Ok(true));
        assert_eq!(held(&doc, 0)[0], "1|2|= A+B|");
    }

    const VIEW: &str = "view Passengers := dataframe['titanic.parquet']\n\nName : String\nFare : Float\n\nDouble := Fare * 2\n";

    fn view() -> Document {
        let mut doc = Document::from_text(VIEW);
        doc.set_base(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples"));
        doc
    }

    #[test]
    fn a_views_column_as_a_reference_goes_into_its_decorator_and_nothing_else_does() {
        let mut doc = view();
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (0, 1), (886, 1));
        // The decorator takes the column, from the same sheet.
        let said = clip.paste_reference(&mut doc, 0, (0, 2), (0, 2), &text);
        assert_eq!(said, Ok(Some("Pasted `Fare`".to_string())));
        assert!(doc.text().contains("Double := Fare\n"), "{}", doc.text());
        // The file's own columns take no paste, and say why.
        let said = clip.paste_reference(&mut doc, 0, (0, 0), (0, 0), &text);
        assert!(
            said.as_ref().is_err_and(|e| e.contains("read-only")),
            "{said:?}"
        );
    }

    #[test]
    fn a_reference_to_a_sorted_view_is_to_the_file_row() {
        let mut doc = view();
        let mut clip = Clip::default();
        doc.sort_rows(0, 1, true).expect("sorts");
        let file_row = doc.source_row(0, 0);
        assert_ne!(file_row, 0);
        // One cell: the row it has in the file, not the one it is shown in.
        let one = clip.copy(&doc, 0, (0, 1), (0, 1));
        let said = clip.paste_reference(&mut doc, 0, (0, 2), (0, 2), &one);
        assert_eq!(said, Ok(Some(format!("Pasted `[Fare; {file_row}]`"))));
        // Rows in a sorted order are no shift, so they cannot be a column.
        let many = clip.copy(&doc, 0, (0, 1), (3, 1));
        let said = clip.paste_reference(&mut doc, 0, (0, 2), (0, 2), &many);
        assert!(
            said.as_ref().is_err_and(|e| e.contains("sorted order")),
            "{said:?}"
        );
    }

    #[test]
    fn a_reference_by_row_reads_that_place_when_rows_are_inserted_above_it() {
        let mut doc = doc();
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (1, 1), (1, 1));
        assert_eq!(
            clip.paste_reference(&mut doc, 1, (0, 0), (0, 0), &text)
                .map(|s| s.is_some()),
            Ok(true)
        );
        assert_eq!(col_display(&doc, 1, 0)[0], "20");
        // A row put in above the one referred to: a reference to row 1 is
        // to the place, as one written by hand is, so it reads what is now
        // there. A view has no rows to insert, so this is for a sheet of
        // the file's own.
        assert!(doc.insert_rows(0, 0, 1));
        assert_eq!(col_display(&doc, 1, 0)[0], "2", "{}", doc.text());
    }

    /// A sheet of numbers with formulas that read their neighbours.
    const GRID: &str = "sheet T\n\nA | B | C | D\n1 | 2 | = A+B | 0\n3 | 4 | = [A; *-1]+B | 0\n5 | 6 | 7 | 8\n7 | 8 | 9 | 9\n9 | 9 | 9 | 9\n9 | 9 | 9 | 9\n";

    fn shown(doc: &Document, row: usize, col: usize) -> String {
        doc.snapshot().sheets[0].rows[row][col].display.clone()
    }

    #[test]
    fn a_turned_block_keeps_reading_the_cells_it_read() {
        // Copied: row 0's `A+B` is 3, and row 1's `[A; *-1]+B` is 1 + 4.
        let mut doc = Document::from_text(GRID);
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (0, 0), (1, 2));
        assert_eq!(shown(&doc, 0, 2), "3");
        assert_eq!(shown(&doc, 1, 2), "5");
        assert_eq!(
            clip.paste_transposed(&mut doc, 0, (3, 0), (3, 0), &text),
            Ok(true)
        );
        // The copies sit at rows 5 of A and B and read the copies of A, B.
        assert_eq!(
            held(&doc, 0)[5],
            "= [A; *-2]+[A; *-1]|= [A; *-2]+[B; *-1]|9|9"
        );
        assert_eq!(shown(&doc, 5, 0), "3");
        assert_eq!(shown(&doc, 5, 1), "5");
    }

    #[test]
    fn a_turned_cut_is_followed_by_what_read_it() {
        let text = "sheet T\n\nA | B | C | D\n1 | 2 | 0 | 0\n3 | 4 | 0 | 0\n0 | 0 | = [A; 0] + [B; 1] | 0\n";
        let mut doc = Document::from_text(text);
        let mut clip = Clip::default();
        assert_eq!(shown(&doc, 2, 2), "5");
        let cut = clip.cut(&doc, 0, (0, 0), (1, 1));
        assert_eq!(
            clip.paste_transposed(&mut doc, 0, (0, 2), (0, 2), &cut),
            Ok(true)
        );
        // A holds 1 and 3 across now, so [A; 0] is 1 at C0 and [B; 1] is at D0.
        assert_eq!(held(&doc, 0)[0], "||1|3");
        assert_eq!(held(&doc, 0)[1], "||2|4");
        assert_eq!(held(&doc, 0)[2], "0|0|= [C; 0] + [D; 1]|0");
        assert_eq!(shown(&doc, 2, 2), "5");
    }

    #[test]
    fn a_formula_pasted_onto_another_sheet_reads_where_it_is_pasted() {
        let mut doc = doc();
        let mut clip = Clip::default();
        // `= A+B` of T, copied and pasted on U: its A and B are U's (D220).
        let text = clip.copy(&doc, 0, (0, 2), (0, 2));
        assert_eq!(clip.paste(&mut doc, 1, (0, 0), (0, 0), &text), Ok(true));
        assert_eq!(held(&doc, 1)[0], "= A+B|");
        // On its own sheet it is pasted as it was written.
        assert_eq!(clip.paste(&mut doc, 0, (1, 2), (1, 2), &text), Ok(true));
        assert_eq!(held(&doc, 0)[1], "10|20|= A+B|");
        // A cut formula moved to another sheet reads where it read (D221).
        let mut doc = self::doc();
        let text = clip.cut(&doc, 0, (0, 2), (0, 2));
        assert_eq!(clip.paste(&mut doc, 1, (0, 1), (0, 1), &text), Ok(true));
        assert_eq!(held(&doc, 1)[0], "|= T[A; *]+T[B; *]");
        assert_eq!(held(&doc, 0)[0], "1|2||");
    }

    const TWO: &str = "sheet T\n\nA  | B  | C\n1  | 2  | = A+B\n10 | 20 | = [A; *-1] + A\n\nsheet V\n\nP | Q | R | S\n  |   |   |\n  |   |   |\n";

    fn pasted_cells(copy: (Cell, Cell), at: Cell) -> (Vec<String>, Vec<String>) {
        let mut doc = Document::from_text(TWO);
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, copy.0, copy.1);
        assert_eq!(clip.paste(&mut doc, 1, at, at, &text), Ok(true));
        (
            held(&doc, 1),
            col_display(&doc, 1, at.1 + (copy.1.1 - copy.0.1)),
        )
    }

    #[test]
    fn what_a_pasted_formula_read_inside_the_block_it_reads_at_the_copy() {
        // The whole row: `C = A+B` reads the copies of A and B beside it,
        // named as the columns they landed in are.
        let (held, shown) = pasted_cells(((0, 0), (0, 2)), (0, 0));
        assert_eq!(held[0], "1|2|= P+Q|");
        assert_eq!(shown[0], "3");
        // Put down a column along, they are the columns they landed in.
        let (held, _) = pasted_cells(((0, 0), (0, 2)), (0, 1));
        assert_eq!(held[0], "|1|2|= Q+R");
        // And a row counted from is counted again from where it is.
        let (held, _) = pasted_cells(((0, 0), (1, 2)), (0, 0));
        assert_eq!(held[1], "10|20|= [P; *-1] + P|");
    }

    #[test]
    fn what_a_pasted_formula_read_outside_the_block_it_reads_where_it_is_pasted() {
        // Without A: `C = A+B` has B inside, now P, and A outside, a column
        // name that is read on the sheet pasted on (D220).
        let (held, _) = pasted_cells(((0, 1), (0, 2)), (0, 0));
        assert_eq!(held[0], "2|= A+P||");
        // A row above the block is outside it: counted from the formula.
        let (held, _) = pasted_cells(((1, 0), (1, 2)), (1, 0));
        assert_eq!(held[1], "10|20|= [A; *-1] + P|");
        // Put down on another row, nothing is counted again.
        let (held, _) = pasted_cells(((0, 1), (0, 2)), (1, 0));
        assert_eq!(held[1], "2|= A+P||");
    }

    #[test]
    fn a_range_or_a_column_in_a_pasted_formula_reads_the_place() {
        let text =
            "sheet T\n\nA  | B\n1  | = A.sum()\n10 | = T.A.sum()\n\nsheet V\n\nP | Q\n  |\n  |\n";
        let mut doc = Document::from_text(text);
        let mut clip = Clip::default();
        // A whole column is outside whatever block was copied from it.
        let copied = clip.copy(&doc, 0, (0, 0), (1, 1));
        assert_eq!(clip.paste(&mut doc, 1, (0, 0), (0, 0), &copied), Ok(true));
        assert!(
            held(&doc, 1)[1].contains("T.A.sum()"),
            "{:?}",
            held(&doc, 1)
        );
    }

    #[test]
    fn a_block_turned_onto_another_sheet_reads_the_copy() {
        let text = "sheet T\n\nA | B | C\n1 | 2 | = A+B\n3 | 4 | = [A; *-1]+B\n5 | 6 | 7\n\nsheet U\n\nA | B | C | D\n0 | 0 | 0 | 0\n0 | 0 | 0 | 0\n0 | 0 | 0 | 0\n0 | 0 | 0 | 0\n";
        for cutting in [false, true] {
            let mut doc = Document::from_text(text);
            let mut clip = Clip::default();
            let block = if cutting {
                clip.cut(&doc, 0, (0, 0), (1, 2))
            } else {
                clip.copy(&doc, 0, (0, 0), (1, 2))
            };
            assert_eq!(
                clip.paste_transposed(&mut doc, 1, (1, 1), (1, 1), &block),
                Ok(true)
            );
            let u = held(&doc, 1);
            assert_eq!(u[1], "0|1|3|0");
            assert_eq!(u[2], "0|2|4|0");
            // What each read of the block it reads in the copy: 1 + 2, and
            // 1 + 4.
            assert_eq!(u[3], "0|= [B; *-2]+[B; *-1]|= [B; *-2]+[C; *-1]|0");
            assert_eq!(doc.snapshot().sheets[1].rows[3][1].display, "3");
            assert_eq!(doc.snapshot().sheets[1].rows[3][2].display, "5");
        }
    }

    const WIDE: &str = "sheet T\n\nA | B | C | D | E | F\n1 | 2 | = A+B | 0 | 0 | 0\n3 | 4 | = [0; *]+B | 0 | 0 | 0\n5 | 6 | 7 | 0 | 0 | 0\n0 | 0 | 0 | 0 | 0 | 0\n0 | 0 | 0 | 0 | 0 | 0\n0 | 0 | 0 | 0 | 0 | 0\n0 | 0 | 0 | 0 | 0 | 0\n";

    #[test]
    fn a_name_in_a_copied_block_is_the_column_it_landed_in() {
        let mut doc = Document::from_text(WIDE);
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (0, 0), (1, 2));
        assert_eq!(clip.paste(&mut doc, 0, (3, 3), (3, 3), &text), Ok(true));
        // A name is the column it landed in; a column by number is fixed.
        assert_eq!(held(&doc, 0)[3], "0|0|0|1|2|= D+E");
        assert_eq!(held(&doc, 0)[4], "0|0|0|3|4|= [0; *]+E");
    }

    #[test]
    fn a_cut_formula_goes_on_reading_the_cells_it_read() {
        let mut doc = Document::from_text(WIDE);
        let mut clip = Clip::default();
        let text = clip.cut(&doc, 0, (0, 2), (0, 2));
        assert_eq!(clip.paste(&mut doc, 0, (3, 2), (3, 2), &text), Ok(true));
        // Three rows down, it reads the row it did.
        assert_eq!(held(&doc, 0)[3], "0|0|= [A; *-3]+[B; *-3]|0|0|0");
        assert_eq!(held(&doc, 0)[0], "1|2||0|0|0");
    }

    #[test]
    fn what_a_turned_block_reads_outside_it_is_fixed() {
        // Only B and C are copied: `A` is left out of the block.
        let mut doc = Document::from_text(GRID);
        let mut clip = Clip::default();
        let text = clip.copy(&doc, 0, (0, 1), (1, 2));
        assert_eq!(
            clip.paste_transposed(&mut doc, 0, (3, 0), (3, 0), &text),
            Ok(true)
        );
        // Both read the A of row 0, as they did, and each its B.
        assert_eq!(held(&doc, 0)[4], "= [A; 0]+[A; *-1]|= [A; 0]+[B; *-1]|9|9");
    }

    #[test]
    fn what_a_block_turned_onto_another_sheet_reads_outside_it_is_fixed() {
        let text = "sheet T\n\nA | B | C\n1 | 2 | = A+B\n3 | 4 | = [A; *-1]+B\n\nsheet U\n\nA | B | C\n0 | 0 | 0\n0 | 0 | 0\n0 | 0 | 0\n";
        let mut doc = Document::from_text(text);
        let mut clip = Clip::default();
        let block = clip.copy(&doc, 0, (0, 1), (1, 2));
        assert_eq!(
            clip.paste_transposed(&mut doc, 1, (0, 0), (0, 0), &block),
            Ok(true)
        );
        assert_eq!(held(&doc, 1)[1], "= T[A; 0]+[A; *-1]|= T[A; 0]+[B; *-1]|0");
    }

    #[test]
    fn a_fixed_read_in_a_copied_formula_is_left_as_it_is() {
        let text = "sheet T\n\nA | B | C\n1 | 2 | = T[A; 1] + B\n10 | 20 | 0\n\nsheet V\n\nP | Q | R\n  |   |\n  |   |\n";
        let mut doc = Document::from_text(text);
        let mut clip = Clip::default();
        let block = clip.copy(&doc, 0, (0, 1), (0, 2));
        assert_eq!(clip.paste(&mut doc, 1, (0, 0), (0, 0), &block), Ok(true));
        // `B` is in the block, now `P`; the row of T by name is left alone.
        assert_eq!(held(&doc, 1)[0], "2|= T[A; 1] + P|");
    }
}

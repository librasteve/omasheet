// Copyright (c) 2026 Stephen Roe

//! The `Sheet` object the QML window talks to.

use crate::{json, theme};
use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QUrl};
use omasheet_engine::doc::{BLANK, entry_hint};
use omasheet_engine::omx::date::Style;
use omasheet_engine::{Document, Options, locale};
use omasheet_interop::{Exported, Imported};
use std::path::PathBuf;
use std::sync::OnceLock;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, file_name)]
        #[qproperty(QString, file_path)]
        #[qproperty(bool, modified)]
        #[qproperty(i32, revision)]
        #[qproperty(QString, status)]
        #[qproperty(QString, notes)]
        #[qproperty(bool, can_undo)]
        #[qproperty(bool, can_redo)]
        #[qproperty(bool, dark_mode)]
        #[qproperty(QString, theme_background)]
        #[qproperty(QString, theme_foreground)]
        #[qproperty(QString, theme_accent)]
        #[qproperty(QString, theme_selection)]
        #[qproperty(QString, theme_muted)]
        #[qproperty(QString, theme_error)]
        #[qproperty(QString, snapshot_path)]
        type Sheet = super::SheetRust;

        /// The whole sheet as JSON: tables, constants and problems.
        #[qinvokable]
        fn snapshot_json(self: &Sheet) -> QString;
        /// The sheet as it is written: the text that saving would write.
        #[qinvokable]
        fn source_text(self: &Sheet) -> QString;

        #[qinvokable]
        fn new_document(self: Pin<&mut Sheet>);
        #[qinvokable]
        fn open_path(self: Pin<&mut Sheet>, path: &QString) -> bool;
        #[qinvokable]
        fn open_url(self: Pin<&mut Sheet>, url: &QUrl) -> bool;
        /// Save to the current file. False if there is none yet, or on failure.
        #[qinvokable]
        fn save(self: Pin<&mut Sheet>) -> bool;
        #[qinvokable]
        fn save_url(self: Pin<&mut Sheet>, url: &QUrl) -> bool;
        /// Write the calculated sheet as a workbook, or the table named as
        /// CSV if `csv`; a file named `.xlsx` or `.csv` is written as that.
        /// What could not be written exactly is left in `notes`.
        #[qinvokable]
        fn export_url(self: Pin<&mut Sheet>, url: &QUrl, csv: bool, table: &QString) -> bool;
        /// Where an export would go unless told otherwise: beside the sheet
        /// and under its name, or in `folder` if the sheet is not saved yet.
        #[qinvokable]
        fn export_suggestion(self: &Sheet, csv: bool, table: &QString, folder: &QUrl) -> QUrl;

        #[qinvokable]
        fn set_cell(self: Pin<&mut Sheet>, table: i32, row: i32, col: i32, text: &QString);
        /// The sources of a block of cells as tab-separated text.
        #[qinvokable]
        fn copy_cells(
            self: &Sheet,
            table: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
        ) -> QString;
        /// Copy a block of cells as `copy_cells` does, and clear it. Pasted
        /// next in the same table, the cells are moved: formulas go on
        /// reading what they read.
        #[qinvokable]
        fn cut_cells(
            self: Pin<&mut Sheet>,
            table: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
        ) -> QString;
        /// Copy a block of cells as `copy_cells` does, remembering where it
        /// came from: `columns` says that it is whole columns.
        #[qinvokable]
        fn copy_block(
            self: Pin<&mut Sheet>,
            table: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
            columns: bool,
        ) -> QString;
        /// Paste as `paste_cells` does, but what the copied cells showed and
        /// not their formulas.
        #[qinvokable]
        fn paste_values(
            self: Pin<&mut Sheet>,
            table: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
            text: &QString,
        );
        /// Paste as `paste_cells` does, but turned about: each copied row
        /// goes down a column.
        #[qinvokable]
        fn paste_transposed(
            self: Pin<&mut Sheet>,
            table: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
            text: &QString,
        );
        /// Make room for what was copied and put it there: copied columns
        /// become new columns at `col0`, and anything else new rows at
        /// `row0`. Returns an error message, or an empty string.
        #[qinvokable]
        fn insert_copied(
            self: Pin<&mut Sheet>,
            table: i32,
            row0: i32,
            col0: i32,
            text: &QString,
        ) -> QString;
        /// Paste tab-separated text at a block; a single value fills the block.
        #[qinvokable]
        fn paste_cells(
            self: Pin<&mut Sheet>,
            table: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
            text: &QString,
        );
        #[qinvokable]
        fn clear_cells(
            self: Pin<&mut Sheet>,
            table: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
        );
        #[qinvokable]
        fn insert_rows(self: Pin<&mut Sheet>, table: i32, at: i32, count: i32);
        #[qinvokable]
        fn delete_rows(self: Pin<&mut Sheet>, table: i32, first: i32, count: i32);

        // These return an error message, or an empty string on success.
        #[qinvokable]
        fn add_column(self: Pin<&mut Sheet>, table: i32, name: &QString) -> QString;
        #[qinvokable]
        fn add_computed(
            self: Pin<&mut Sheet>,
            table: i32,
            name: &QString,
            expr: &QString,
        ) -> QString;
        /// Add an empty column as column `at`.
        #[qinvokable]
        fn insert_column(self: Pin<&mut Sheet>, table: i32, at: i32, name: &QString) -> QString;
        /// Add a formula column as column `at`.
        #[qinvokable]
        fn insert_computed(
            self: Pin<&mut Sheet>,
            table: i32,
            at: i32,
            name: &QString,
            expr: &QString,
        ) -> QString;
        #[qinvokable]
        fn rename_column(self: Pin<&mut Sheet>, table: i32, col: i32, name: &QString) -> QString;
        /// Delete `count` columns from `first`.
        #[qinvokable]
        fn delete_columns(self: Pin<&mut Sheet>, table: i32, first: i32, count: i32) -> QString;
        /// Move `count` rows from `first` so the first becomes row `to`.
        #[qinvokable]
        fn move_rows(self: Pin<&mut Sheet>, table: i32, first: i32, count: i32, to: i32)
        -> QString;
        /// Move `count` columns from `first` so the first becomes column `to`.
        #[qinvokable]
        fn move_columns(
            self: Pin<&mut Sheet>,
            table: i32,
            first: i32,
            count: i32,
            to: i32,
        ) -> QString;
        #[qinvokable]
        fn add_table(self: Pin<&mut Sheet>, name: &QString) -> QString;
        #[qinvokable]
        fn add_const(self: Pin<&mut Sheet>, name: &QString, expr: &QString) -> QString;
        #[qinvokable]
        fn set_const(self: Pin<&mut Sheet>, index: i32, expr: &QString);

        #[qinvokable]
        fn undo(self: Pin<&mut Sheet>);
        #[qinvokable]
        fn redo(self: Pin<&mut Sheet>);
        #[qinvokable]
        fn reload_theme(self: Pin<&mut Sheet>);

        /// The locale and how it writes a date and a time.
        #[qinvokable]
        fn locale_hint(self: &Sheet) -> QString;
        /// The function directory as JSON.
        #[qinvokable]
        fn functions_json(self: &Sheet) -> QString;
        /// What can be typed into a column of the named type.
        #[qinvokable]
        fn entry_hint(self: &Sheet, ty: &QString) -> QString;
        /// The count, sum and average of a block of cells, for the footer.
        #[qinvokable]
        fn selection_summary(
            self: &Sheet,
            table: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
        ) -> QString;
    }
}

pub struct SheetRust {
    file_name: QString,
    file_path: QString,
    modified: bool,
    revision: i32,
    status: QString,
    /// What the last import or export changed or left behind, a line each.
    notes: QString,
    can_undo: bool,
    can_redo: bool,
    dark_mode: bool,
    theme_background: QString,
    theme_foreground: QString,
    theme_accent: QString,
    theme_selection: QString,
    theme_muted: QString,
    theme_error: QString,
    /// Development aid: where to save a picture of the window, then quit.
    snapshot_path: QString,

    doc: Document,
    path: Option<PathBuf>,
    /// The workbook or CSV file the sheet was imported from, until it is saved.
    origin: Option<PathBuf>,
    /// The text as last opened or saved, to tell whether there are changes.
    saved: String,
    /// The cells last cut, until they are pasted.
    cut: Option<Cut>,
    /// The cells last copied.
    copied: Option<Copied>,
}

/// A block of cells that was copied.
struct Copied {
    table: usize,
    /// What went to the clipboard: the cells as they are written.
    text: String,
    /// The same cells as they were shown.
    values: String,
    /// The names of the columns, if whole columns were copied.
    columns: Option<Vec<String>>,
}

/// A block of cells that was cut, to be moved by the next paste.
struct Cut {
    table: usize,
    /// The top left cell: its row and column.
    at: (usize, usize),
    /// What went to the clipboard.
    text: String,
    /// The sheet once the cells were cleared: a paste moves them only if
    /// nothing else has changed since.
    after: String,
}

impl Default for SheetRust {
    fn default() -> Self {
        let theme = theme::load();
        SheetRust {
            file_name: QString::from("Untitled"),
            file_path: QString::default(),
            modified: false,
            revision: 0,
            status: QString::default(),
            notes: QString::default(),
            can_undo: false,
            can_redo: false,
            dark_mode: theme.dark,
            theme_background: QString::from(&theme.background),
            theme_foreground: QString::from(&theme.foreground),
            theme_accent: QString::from(&theme.accent),
            theme_selection: QString::from(&theme.selection),
            theme_muted: QString::from(&theme.muted),
            theme_error: QString::from(&theme.error),
            snapshot_path: QString::from(std::env::var("OMASHEET_UI_SNAPSHOT").unwrap_or_default()),
            doc: document(BLANK),
            path: None,
            origin: None,
            saved: BLANK.to_string(),
            cut: None,
            copied: None,
        }
    }
}

/// How the user's locale writes dates and times.
fn style() -> Style {
    static STYLE: OnceLock<Style> = OnceLock::new();
    *STYLE.get_or_init(locale::os_style)
}

/// A document that shows dates and times the way the user's locale does.
fn document(text: &str) -> Document {
    Document::with_style(text, style())
}

fn index(n: i32) -> usize {
    n.max(0) as usize
}

/// Tab-separated text as rows of cells.
fn parse_block(text: &str) -> Vec<Vec<String>> {
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
fn transposed(block: &[Vec<String>]) -> Vec<Vec<String>> {
    let width = block.iter().map(Vec::len).max().unwrap_or(0);
    (0..width)
        .map(|j| {
            let cell = |line: &Vec<String>| line.get(j).cloned().unwrap_or_default();
            block.iter().map(cell).collect()
        })
        .collect()
}

impl qobject::Sheet {
    /// Publish the document's state after it changed.
    fn refresh(mut self: Pin<&mut Self>, status: &str) {
        let (modified, can_undo, can_redo) = {
            let rust = self.rust();
            (
                rust.doc.text() != rust.saved,
                rust.doc.can_undo(),
                rust.doc.can_redo(),
            )
        };
        self.as_mut().set_modified(modified);
        self.as_mut().set_can_undo(can_undo);
        self.as_mut().set_can_redo(can_redo);
        self.as_mut().set_status(QString::from(status));
        let next = self.rust().revision.wrapping_add(1);
        self.as_mut().set_revision(next);
    }

    fn set_path(mut self: Pin<&mut Self>, path: Option<PathBuf>) {
        let name = path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or("Untitled".to_string(), |n| n.to_string_lossy().into_owned());
        let full = path
            .as_ref()
            .map_or(String::new(), |p| p.to_string_lossy().into_owned());
        self.as_mut().set_file_name(QString::from(&name));
        self.as_mut().set_file_path(QString::from(&full));
        self.as_mut().rust_mut().path = path;
        self.as_mut().rust_mut().origin = None;
    }

    fn snapshot_json(&self) -> QString {
        QString::from(&json::snapshot(self.rust().doc.snapshot()))
    }

    fn source_text(&self) -> QString {
        QString::from(self.rust().doc.text())
    }

    fn new_document(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.doc = document(BLANK);
            rust.saved = BLANK.to_string();
        }
        self.as_mut().set_notes(QString::default());
        self.as_mut().set_path(None);
        self.refresh("");
    }

    /// Open a workbook or a CSV file as a new sheet, not yet saved.
    fn import_file(mut self: Pin<&mut Self>, path: PathBuf, kind: &str) -> bool {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let imported =
            std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| match kind {
                    "xlsx" => omasheet_interop::xlsx::import(&bytes),
                    _ => {
                        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                        omasheet_interop::csv::import(&stem, &bytes)
                    }
                });
        match imported {
            Ok(Imported { source, notices }) => {
                {
                    let mut rust = self.as_mut().rust_mut();
                    rust.doc = document(&source);
                    // Nothing of it is saved yet.
                    rust.saved = String::new();
                }
                self.as_mut().set_notes(QString::from(&notices.join("\n")));
                self.as_mut().set_path(None);
                self.as_mut().rust_mut().origin = Some(path.clone());
                self.refresh(&format!("Imported {name}"));
                true
            }
            Err(why) => {
                let message = format!("Could not import {name}: {why}");
                self.as_mut().set_status(QString::from(&message));
                false
            }
        }
    }

    fn open_file(mut self: Pin<&mut Self>, path: PathBuf) -> bool {
        let kind = path.extension().unwrap_or_default().to_string_lossy();
        let kind = kind.to_ascii_lowercase();
        if matches!(kind.as_str(), "xlsx" | "csv") {
            return self.import_file(path, &kind);
        }
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                {
                    let mut rust = self.as_mut().rust_mut();
                    rust.doc = document(&text);
                    rust.saved = text;
                }
                self.as_mut().set_notes(QString::default());
                self.as_mut().set_path(Some(path));
                self.refresh("");
                true
            }
            Err(e) => {
                let message = format!("Could not open {}: {e}", path.display());
                self.as_mut().set_status(QString::from(&message));
                false
            }
        }
    }

    fn open_path(self: Pin<&mut Self>, path: &QString) -> bool {
        self.open_file(PathBuf::from(path.to_string()))
    }

    fn open_url(self: Pin<&mut Self>, url: &QUrl) -> bool {
        self.open_file(PathBuf::from(url.to_local_file_or_default().to_string()))
    }

    fn save_file(mut self: Pin<&mut Self>, path: PathBuf) -> bool {
        let text = self.rust().doc.text().to_string();
        match std::fs::write(&path, &text) {
            Ok(()) => {
                self.as_mut().rust_mut().saved = text;
                let message = format!("Saved {}", path.display());
                self.as_mut().set_path(Some(path));
                self.refresh(&message);
                true
            }
            Err(e) => {
                let message = format!("Could not save {}: {e}", path.display());
                self.as_mut().set_status(QString::from(&message));
                false
            }
        }
    }

    fn save(self: Pin<&mut Self>) -> bool {
        match self.rust().path.clone() {
            Some(path) => self.save_file(path),
            None => false,
        }
    }

    fn save_url(self: Pin<&mut Self>, url: &QUrl) -> bool {
        let mut path = PathBuf::from(url.to_local_file_or_default().to_string());
        if path.as_os_str().is_empty() {
            return false;
        }
        if path.extension().is_none() {
            path.set_extension("omx");
        }
        self.save_file(path)
    }

    fn export_url(mut self: Pin<&mut Self>, url: &QUrl, csv: bool, table: &QString) -> bool {
        let mut path = PathBuf::from(url.to_local_file_or_default().to_string());
        if path.as_os_str().is_empty() {
            return false;
        }
        let named = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        let csv = match named.as_deref() {
            Some("csv") => true,
            Some("xlsx") => false,
            _ => {
                path.as_mut_os_string()
                    .push(if csv { ".csv" } else { ".xlsx" });
                csv
            }
        };
        let table = table.to_string();
        if csv && table.is_empty() {
            let message = "CSV holds one table: show the table to export first";
            self.as_mut().set_status(QString::from(message));
            return false;
        }
        let name = self.rust().file_name.to_string();
        let text = self.rust().doc.text().to_string();
        let exported = if csv {
            omasheet_interop::csv::export(&name, &text, Some(&table), Options::default())
        } else {
            omasheet_interop::xlsx::export(&name, &text, None, Options::default())
        };
        let written = exported
            .map_err(|_| "the sheet has problems to put right first".to_string())
            .and_then(|Exported { files, notices }| {
                let bytes = files.first().map_or(&[][..], |f| &f.bytes);
                std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                Ok(notices)
            });
        match written {
            Ok(notices) => {
                self.as_mut().set_notes(QString::from(&notices.join("\n")));
                let message = format!("Exported {}", path.display());
                self.as_mut().set_status(QString::from(&message));
                true
            }
            Err(why) => {
                let message = format!("Could not export {}: {why}", path.display());
                self.as_mut().set_status(QString::from(&message));
                false
            }
        }
    }

    fn export_suggestion(&self, csv: bool, table: &QString, folder: &QUrl) -> QUrl {
        let rust = self.rust();
        let from = rust.path.as_ref().or(rust.origin.as_ref());
        let mut name = from
            .and_then(|p| p.file_stem())
            .map_or("Untitled".to_string(), |s| s.to_string_lossy().into_owned());
        if csv {
            // As the command line names them: budget.Sales.csv, unless the
            // sheet has the one table.
            if rust.doc.snapshot().tables.len() > 1 {
                name.push('.');
                name.push_str(&table.to_string());
            }
            name.push_str(".csv");
        } else {
            name.push_str(".xlsx");
        }
        let folder = from
            .and_then(|p| p.parent())
            .map(PathBuf::from)
            .or_else(|| {
                let folder = folder.to_local_file_or_default().to_string();
                (!folder.is_empty()).then(|| PathBuf::from(folder))
            })
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        QUrl::from_local_file(&QString::from(&*folder.join(name).to_string_lossy()))
    }

    /// Run an edit and publish the result if it changed anything.
    fn edit(mut self: Pin<&mut Self>, change: impl FnOnce(&mut Document) -> bool) {
        let changed = change(&mut self.as_mut().rust_mut().doc);
        if changed {
            self.refresh("");
        }
    }

    /// Run an edit that can be refused, returning the reason.
    fn try_edit(
        mut self: Pin<&mut Self>,
        change: impl FnOnce(&mut Document) -> Result<(), String>,
    ) -> QString {
        let result = change(&mut self.as_mut().rust_mut().doc);
        match result {
            Ok(()) => {
                self.refresh("");
                QString::default()
            }
            Err(message) => QString::from(&message),
        }
    }

    fn set_cell(self: Pin<&mut Self>, table: i32, row: i32, col: i32, text: &QString) {
        let text = text.to_string();
        self.edit(|doc| doc.set_cell(index(table), index(row), index(col), &text));
    }

    /// A block of cells as tab-separated text: what they showed if `values`,
    /// otherwise what they hold.
    fn block_text(
        &self,
        table: i32,
        (row0, col0): (i32, i32),
        (row1, col1): (i32, i32),
        values: bool,
    ) -> String {
        let Some(t) = self.rust().doc.snapshot().tables.get(index(table)) else {
            return String::new();
        };
        let mut lines = Vec::new();
        for row in t.rows.iter().take(index(row1) + 1).skip(index(row0)) {
            let cells: Vec<&str> = row
                .iter()
                .enumerate()
                .take(index(col1) + 1)
                .skip(index(col0))
                // A computed cell has no source of its own, so copy its value:
                // in full, where it is shown with fewer digits than it has.
                .map(|(c, cell)| {
                    if values || t.columns[c].formula.is_some() {
                        cell.exact.as_deref().unwrap_or(&cell.display)
                    } else {
                        cell.source.as_str()
                    }
                })
                .collect();
            lines.push(cells.join("\t"));
        }
        lines.join("\n")
    }

    fn copy_cells(&self, table: i32, row0: i32, col0: i32, row1: i32, col1: i32) -> QString {
        QString::from(&self.block_text(table, (row0, col0), (row1, col1), false))
    }

    fn copy_block(
        mut self: Pin<&mut Self>,
        table: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
        columns: bool,
    ) -> QString {
        let text = self.block_text(table, (row0, col0), (row1, col1), false);
        let values = self.block_text(table, (row0, col0), (row1, col1), true);
        let names: Option<Vec<String>> =
            self.rust()
                .doc
                .snapshot()
                .tables
                .get(index(table))
                .map(|t| {
                    let picked = t.columns.iter().take(index(col1) + 1).skip(index(col0));
                    picked.map(|c| c.name.clone()).collect()
                });
        let mut rust = self.as_mut().rust_mut();
        rust.cut = None;
        rust.copied = Some(Copied {
            table: index(table),
            text: text.clone(),
            values,
            columns: names.filter(|_| columns),
        });
        QString::from(&text)
    }

    /// What was last copied, if `text` from the clipboard is still that.
    fn copied(&self, text: &str) -> Option<&Copied> {
        let copied = self.rust().copied.as_ref()?;
        (parse_block(&copied.text) == parse_block(text)).then_some(copied)
    }

    fn paste_values(
        mut self: Pin<&mut Self>,
        table: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
        text: &QString,
    ) {
        let text = text.to_string();
        let values = self.copied(&text).map(|c| c.values.clone());
        self.as_mut().rust_mut().cut = None;
        let block = parse_block(values.as_deref().unwrap_or(&text));
        self.put_block(table, (index(row0), index(col0)), (row1, col1), block);
    }

    fn paste_transposed(
        mut self: Pin<&mut Self>,
        table: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
        text: &QString,
    ) {
        self.as_mut().rust_mut().cut = None;
        let block = transposed(&parse_block(&text.to_string()));
        self.put_block(table, (index(row0), index(col0)), (row1, col1), block);
    }

    fn insert_copied(
        mut self: Pin<&mut Self>,
        table: i32,
        row0: i32,
        col0: i32,
        text: &QString,
    ) -> QString {
        let text = text.to_string();
        self.as_mut().rust_mut().cut = None;
        // Whole columns copied from this table go in as columns, if they
        // are all still there.
        let columns = self.copied(&text).and_then(|copied| {
            let names = copied.columns.as_ref()?;
            let t = self.rust().doc.snapshot().tables.get(copied.table)?;
            let at = |name: &String| t.columns.iter().position(|c| &c.name == name);
            let cols: Option<Vec<usize>> = names.iter().map(at).collect();
            cols.filter(|_| copied.table == index(table))
        });
        match columns {
            Some(cols) => self.try_edit(|doc| doc.insert_copies(index(table), &cols, index(col0))),
            None => {
                let block = parse_block(&text);
                self.edit(|doc| doc.insert_cells(index(table), (index(row0), index(col0)), &block));
                QString::default()
            }
        }
    }

    fn cut_cells(
        mut self: Pin<&mut Self>,
        table: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
    ) -> QString {
        let text = self.copy_cells(table, row0, col0, row1, col1);
        self.as_mut().clear_cells(table, row0, col0, row1, col1);
        let after = self.rust().doc.text().to_string();
        self.as_mut().rust_mut().cut = Some(Cut {
            table: index(table),
            at: (index(row0), index(col0)),
            text: text.to_string(),
            after,
        });
        text
    }

    fn paste_cells(
        mut self: Pin<&mut Self>,
        table: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
        text: &QString,
    ) {
        let text = text.to_string();
        let block = parse_block(&text);
        let (row0, col0) = (index(row0), index(col0));
        // Cells cut from this table, with nothing done since, are moved.
        let cut = self.as_mut().rust_mut().cut.take();
        if let Some(cut) = cut
            && cut.table == index(table)
            && parse_block(&cut.text) == block
            && cut.after == self.rust().doc.text()
        {
            self.edit(|doc| doc.paste_cut(cut.table, cut.at, (row0, col0), &block));
            return;
        }
        self.put_block(table, (row0, col0), (row1, col1), block);
    }

    /// Put a block of cells down at a block; a single cell fills it.
    fn put_block(
        self: Pin<&mut Self>,
        table: i32,
        (row0, col0): (usize, usize),
        (row1, col1): (i32, i32),
        block: Vec<Vec<String>>,
    ) {
        let single = block.len() == 1 && block[0].len() == 1;
        let computed: Vec<bool> = self
            .rust()
            .doc
            .snapshot()
            .tables
            .get(index(table))
            .map(|t| t.columns.iter().map(|c| c.formula.is_some()).collect())
            .unwrap_or_default();
        let mut cells = Vec::new();
        let mut put = |row: usize, col: usize, text: &str| {
            if computed.get(col) == Some(&false) {
                cells.push((row, col, text.to_string()));
            }
        };
        if single {
            for row in row0..=index(row1).max(row0) {
                for col in col0..=index(col1).max(col0) {
                    put(row, col, &block[0][0]);
                }
            }
        } else {
            for (i, line) in block.iter().enumerate() {
                for (j, text) in line.iter().enumerate() {
                    put(row0 + i, col0 + j, text);
                }
            }
        }
        self.edit(|doc| doc.set_cells(index(table), &cells));
    }

    fn clear_cells(self: Pin<&mut Self>, table: i32, row0: i32, col0: i32, row1: i32, col1: i32) {
        let Some(t) = self.rust().doc.snapshot().tables.get(index(table)) else {
            return;
        };
        let mut cells = Vec::new();
        for row in index(row0)..=index(row1).min(t.rows.len().saturating_sub(1)) {
            for col in index(col0)..=index(col1) {
                if t.columns.get(col).is_some_and(|c| c.formula.is_none()) {
                    cells.push((row, col, String::new()));
                }
            }
        }
        if t.rows.is_empty() {
            return;
        }
        self.edit(|doc| doc.set_cells(index(table), &cells));
    }

    fn insert_rows(self: Pin<&mut Self>, table: i32, at: i32, count: i32) {
        self.edit(|doc| doc.insert_rows(index(table), index(at), index(count)));
    }

    fn delete_rows(self: Pin<&mut Self>, table: i32, first: i32, count: i32) {
        self.edit(|doc| doc.delete_rows(index(table), index(first), index(count)));
    }

    fn add_column(self: Pin<&mut Self>, table: i32, name: &QString) -> QString {
        let name = name.to_string();
        self.try_edit(|doc| doc.add_column(index(table), &name))
    }

    fn insert_column(self: Pin<&mut Self>, table: i32, at: i32, name: &QString) -> QString {
        let name = name.to_string();
        self.try_edit(|doc| doc.insert_column(index(table), index(at), &name))
    }

    fn insert_computed(
        self: Pin<&mut Self>,
        table: i32,
        at: i32,
        name: &QString,
        expr: &QString,
    ) -> QString {
        let (name, expr) = (name.to_string(), expr.to_string());
        self.try_edit(|doc| doc.insert_computed(index(table), index(at), &name, &expr))
    }

    fn add_computed(self: Pin<&mut Self>, table: i32, name: &QString, expr: &QString) -> QString {
        let (name, expr) = (name.to_string(), expr.to_string());
        self.try_edit(|doc| doc.add_computed(index(table), &name, &expr))
    }

    fn rename_column(self: Pin<&mut Self>, table: i32, col: i32, name: &QString) -> QString {
        let name = name.to_string();
        self.try_edit(|doc| doc.rename_column(index(table), index(col), &name))
    }

    fn delete_columns(self: Pin<&mut Self>, table: i32, first: i32, count: i32) -> QString {
        self.try_edit(|doc| doc.delete_columns(index(table), index(first), index(count)))
    }

    fn move_rows(self: Pin<&mut Self>, table: i32, first: i32, count: i32, to: i32) -> QString {
        self.try_edit(|doc| doc.move_rows(index(table), index(first), index(count), index(to)))
    }

    fn move_columns(self: Pin<&mut Self>, table: i32, first: i32, count: i32, to: i32) -> QString {
        self.try_edit(|doc| doc.move_columns(index(table), index(first), index(count), index(to)))
    }

    fn add_table(self: Pin<&mut Self>, name: &QString) -> QString {
        let name = name.to_string();
        self.try_edit(|doc| doc.add_table(&name))
    }

    fn add_const(self: Pin<&mut Self>, name: &QString, expr: &QString) -> QString {
        let (name, expr) = (name.to_string(), expr.to_string());
        self.try_edit(|doc| doc.add_const(&name, &expr))
    }

    fn set_const(self: Pin<&mut Self>, index_: i32, expr: &QString) {
        let expr = expr.to_string();
        self.edit(|doc| doc.set_const(index(index_), &expr));
    }

    fn undo(self: Pin<&mut Self>) {
        self.edit(|doc| doc.undo());
    }

    fn redo(self: Pin<&mut Self>) {
        self.edit(|doc| doc.redo());
    }

    fn locale_hint(&self) -> QString {
        let style = style();
        let name = match locale::os_name() {
            _ if style.is_iso() => "ISO".to_string(),
            name if name.is_empty() => "Locale".to_string(),
            name => name,
        };
        let clock = if style.hour12 { "12h" } else { "24h" };
        QString::from(&format!("{name}: {} {clock}", style.date_pattern()))
    }

    fn functions_json(&self) -> QString {
        QString::from(&json::functions(
            &self.rust().doc.snapshot().funcs,
            omasheet_engine::omx::funcs::FUNCTIONS,
        ))
    }

    fn entry_hint(&self, ty: &QString) -> QString {
        // Only where the way to write a value is not plain from its type.
        let ty = ty.to_string();
        if !matches!(ty.as_str(), "Date" | "Time" | "DateTime" | "Bool") {
            return QString::default();
        }
        QString::from(&entry_hint(&style(), &ty))
    }

    fn selection_summary(&self, table: i32, row0: i32, col0: i32, row1: i32, col1: i32) -> QString {
        let doc = &self.rust().doc;
        let from = (index(row0), index(col0));
        let Some(summary) = doc.summary(index(table), from, (index(row1), index(col1))) else {
            return QString::default();
        };
        let mut text = format!("Count {}", summary.count);
        if let Some((sum, average)) = summary.numbers {
            text.push_str(&format!("   Sum {sum}   Average {average}"));
        }
        QString::from(&text)
    }

    fn reload_theme(mut self: Pin<&mut Self>) {
        let theme = theme::load();
        self.as_mut().set_dark_mode(theme.dark);
        self.as_mut()
            .set_theme_background(QString::from(&theme.background));
        self.as_mut()
            .set_theme_foreground(QString::from(&theme.foreground));
        self.as_mut().set_theme_accent(QString::from(&theme.accent));
        self.as_mut()
            .set_theme_selection(QString::from(&theme.selection));
        self.as_mut().set_theme_muted(QString::from(&theme.muted));
        self.as_mut().set_theme_error(QString::from(&theme.error));
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_block, transposed};

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
}

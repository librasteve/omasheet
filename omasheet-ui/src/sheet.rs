//! The `Sheet` object the QML window talks to.

use crate::{json, theme};
use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QUrl};
use omasheet_engine::Document;
use omasheet_engine::doc::BLANK;
use std::path::PathBuf;

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
    }
}

pub struct SheetRust {
    file_name: QString,
    file_path: QString,
    modified: bool,
    revision: i32,
    status: QString,
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
    /// The text as last opened or saved, to tell whether there are changes.
    saved: String,
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
            doc: Document::default(),
            path: None,
            saved: BLANK.to_string(),
        }
    }
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
    }

    fn snapshot_json(&self) -> QString {
        QString::from(&json::snapshot(self.rust().doc.snapshot()))
    }

    fn new_document(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.doc = Document::default();
            rust.saved = BLANK.to_string();
        }
        self.as_mut().set_path(None);
        self.refresh("");
    }

    fn open_file(mut self: Pin<&mut Self>, path: PathBuf) -> bool {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                {
                    let mut rust = self.as_mut().rust_mut();
                    rust.doc = Document::from_text(&text);
                    rust.saved = text;
                }
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

    fn copy_cells(&self, table: i32, row0: i32, col0: i32, row1: i32, col1: i32) -> QString {
        let Some(t) = self.rust().doc.snapshot().tables.get(index(table)) else {
            return QString::default();
        };
        let mut lines = Vec::new();
        for row in t.rows.iter().take(index(row1) + 1).skip(index(row0)) {
            let cells: Vec<&str> = row
                .iter()
                .enumerate()
                .take(index(col1) + 1)
                .skip(index(col0))
                // A computed cell has no source of its own, so copy its value.
                .map(|(c, cell)| {
                    if t.columns[c].formula.is_some() {
                        cell.display.as_str()
                    } else {
                        cell.source.as_str()
                    }
                })
                .collect();
            lines.push(cells.join("\t"));
        }
        QString::from(&lines.join("\n"))
    }

    fn paste_cells(
        self: Pin<&mut Self>,
        table: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
        text: &QString,
    ) {
        let block = parse_block(&text.to_string());
        let (row0, col0) = (index(row0), index(col0));
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

    fn add_computed(self: Pin<&mut Self>, table: i32, name: &QString, expr: &QString) -> QString {
        let (name, expr) = (name.to_string(), expr.to_string());
        self.try_edit(|doc| doc.add_computed(index(table), &name, &expr))
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
    use super::parse_block;

    #[test]
    fn parses_clipboard_blocks() {
        assert_eq!(parse_block("a\tb\r\nc\t\n"), [["a", "b"], ["c", ""]]);
        assert_eq!(parse_block("x"), [["x"]]);
    }
}

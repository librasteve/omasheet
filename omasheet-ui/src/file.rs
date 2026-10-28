// Copyright (c) 2026 Stephen Roe

//! The `File` object the QML window talks to.

use crate::clip::Clip;
use crate::{json, theme};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};
use omasheet_engine::doc::{BLANK, entry_hint};
use omasheet_engine::omx::date::Style;
use omasheet_engine::{Document, Options, locale};
use omasheet_interop::parquet;
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
        #[qproperty(QString, export_warning)]
        #[qproperty(QString, snapshot_path)]
        type File = super::FileRust;

        /// The whole file as JSON: sheets, constants and problems.
        #[qinvokable]
        fn snapshot_json(self: &File) -> QString;
        /// The file as it is written, for the source view: every sheet's
        /// lines aligned, as editing it there will leave them.
        #[qinvokable]
        fn source_text(self: &File) -> QString;
        /// Take `text` as the file: the source as edited by hand.
        #[qinvokable]
        fn set_source_text(self: Pin<&mut File>, text: &QString);

        #[qinvokable]
        fn new_document(self: Pin<&mut File>);
        #[qinvokable]
        fn open_path(self: Pin<&mut File>, path: &QString) -> bool;
        #[qinvokable]
        fn open_url(self: Pin<&mut File>, url: &QUrl) -> bool;
        /// Save to the current file. False if there is none yet, or on failure.
        #[qinvokable]
        fn save(self: Pin<&mut File>) -> bool;
        #[qinvokable]
        fn save_url(self: Pin<&mut File>, url: &QUrl) -> bool;
        /// Write the calculated file as `kind`: a workbook, `xlsx`, or
        /// `json`, or the sheet named as `csv` or `parquet`. A file named
        /// `.xlsx`, `.csv`, `.json` or `.parquet` is written as that. What
        /// could not be written exactly is left in `notes`.
        ///
        /// An export that would write a number as a null because no Parquet
        /// decimal holds it writes nothing: the warning is left in
        /// `export_warning` for the app to put to the user, who answers it
        /// by asking again with `force`.
        #[qinvokable]
        fn export_url(
            self: Pin<&mut File>,
            url: &QUrl,
            kind: &QString,
            sheet: &QString,
            force: bool,
        ) -> bool;
        /// Where an export would go unless told otherwise: beside the file
        /// and under its name, or in `folder` if the file is not saved yet.
        #[qinvokable]
        fn export_suggestion(self: &File, kind: &QString, sheet: &QString, folder: &QUrl) -> QUrl;

        #[qinvokable]
        fn set_cell(self: Pin<&mut File>, sheet: i32, row: i32, col: i32, text: &QString);
        /// The sources of a block of cells as tab-separated text.
        #[qinvokable]
        fn copy_cells(
            self: &File,
            sheet: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
        ) -> QString;
        /// Copy a block of cells as `copy_cells` does, to be cleared when it
        /// is next pasted. Pasted in the same sheet, the cells are moved:
        /// formulas go on reading what they read.
        #[qinvokable]
        fn cut_cells(
            self: Pin<&mut File>,
            sheet: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
        ) -> QString;
        /// Leave the cells that were cut where they are: a paste copies them.
        #[qinvokable]
        fn forget_cut(self: Pin<&mut File>);
        /// Copy a block of cells as `copy_cells` does, remembering what was
        /// copied.
        #[qinvokable]
        fn copy_block(
            self: Pin<&mut File>,
            sheet: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
        ) -> QString;
        /// Paste as `paste_cells` does, but what the copied cells showed and
        /// not their formulas.
        #[qinvokable]
        fn paste_values(
            self: Pin<&mut File>,
            sheet: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
            text: &QString,
        ) -> QString;
        /// Paste as `paste_cells` does, but turned about: each copied row
        /// goes down a column.
        #[qinvokable]
        fn paste_transposed(
            self: Pin<&mut File>,
            sheet: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
            text: &QString,
        ) -> QString;
        /// Paste tab-separated text at a block; a single value fills the block.
        #[qinvokable]
        fn paste_cells(
            self: Pin<&mut File>,
            sheet: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
            text: &QString,
        ) -> QString;
        #[qinvokable]
        fn clear_cells(
            self: Pin<&mut File>,
            sheet: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
        );
        #[qinvokable]
        fn insert_rows(self: Pin<&mut File>, sheet: i32, at: i32, count: i32);
        #[qinvokable]
        fn delete_rows(self: Pin<&mut File>, sheet: i32, first: i32, count: i32);

        // These return an error message, or an empty string on success.
        #[qinvokable]
        fn add_column(self: Pin<&mut File>, sheet: i32, name: &QString) -> QString;
        #[qinvokable]
        fn add_computed(
            self: Pin<&mut File>,
            sheet: i32,
            name: &QString,
            expr: &QString,
        ) -> QString;
        /// Add an empty column as column `at`.
        #[qinvokable]
        fn insert_column(self: Pin<&mut File>, sheet: i32, at: i32, name: &QString) -> QString;
        /// Add a formula column as column `at`.
        #[qinvokable]
        fn insert_computed(
            self: Pin<&mut File>,
            sheet: i32,
            at: i32,
            name: &QString,
            expr: &QString,
        ) -> QString;
        #[qinvokable]
        fn rename_column(self: Pin<&mut File>, sheet: i32, col: i32, name: &QString) -> QString;
        /// Delete `count` columns from `first`.
        #[qinvokable]
        fn delete_columns(self: Pin<&mut File>, sheet: i32, first: i32, count: i32) -> QString;
        /// Declare the type of `count` columns from `first` as `ty`, or with
        /// no `ty` leave them to take their type from their cells.
        #[qinvokable]
        fn set_column_types(self: Pin<&mut File>, sheet: i32, first: i32, count: i32, ty: &QString);
        /// Move `count` rows from `first` so the first becomes row `to`.
        #[qinvokable]
        fn move_rows(self: Pin<&mut File>, sheet: i32, first: i32, count: i32, to: i32) -> QString;
        /// Move `count` columns from `first` so the first becomes column `to`.
        #[qinvokable]
        fn move_columns(
            self: Pin<&mut File>,
            sheet: i32,
            first: i32,
            count: i32,
            to: i32,
        ) -> QString;
        #[qinvokable]
        fn add_sheet(self: Pin<&mut File>, name: &QString) -> QString;
        /// A new sheet before sheet `at`, or last if there is none there.
        #[qinvokable]
        fn insert_sheet(self: Pin<&mut File>, at: i32, name: &QString) -> QString;
        #[qinvokable]
        fn rename_sheet(self: Pin<&mut File>, sheet: i32, name: &QString) -> QString;
        #[qinvokable]
        fn delete_sheet(self: Pin<&mut File>, sheet: i32) -> QString;
        /// Sheet `from` moved to be sheet `to`.
        #[qinvokable]
        fn move_sheet(self: Pin<&mut File>, from: i32, to: i32) -> QString;
        /// Sheet `to` made a copy of sheet `from`, under its name copied.
        #[qinvokable]
        fn paste_sheet(self: Pin<&mut File>, from: i32, to: i32) -> QString;
        #[qinvokable]
        fn add_const(self: Pin<&mut File>, name: &QString, expr: &QString) -> QString;
        #[qinvokable]
        fn set_const(self: Pin<&mut File>, index: i32, expr: &QString);

        #[qinvokable]
        fn undo(self: Pin<&mut File>);
        /// How many steps there are to undo, and every step since then
        /// made one.
        #[qinvokable]
        fn undo_mark(&self) -> i32;
        #[qinvokable]
        fn join_since(self: Pin<&mut File>, mark: i32);
        #[qinvokable]
        fn redo(self: Pin<&mut File>);
        #[qinvokable]
        fn reload_theme(self: Pin<&mut File>);
        /// Take the theme from now on: Omarchy's as it changes, and without
        /// one the colours for a dark or a light system.
        #[qinvokable]
        fn follow_theme(self: Pin<&mut File>, system_dark: bool);

        /// The locale and how it writes a date and a time.
        #[qinvokable]
        fn locale_hint(self: &File) -> QString;
        /// The keyboard help, from `help.txt`, with the locale and the
        /// version filled in.
        #[qinvokable]
        fn help_text(self: &File) -> QString;
        /// The function directory as JSON.
        #[qinvokable]
        fn functions_json(self: &File) -> QString;
        /// What can be typed into a column of the named type.
        #[qinvokable]
        fn entry_hint(self: &File, ty: &QString) -> QString;
        /// The text size the desktop is set to, in pixels; 0 where the
        /// desktop does not say.
        #[qinvokable]
        fn desktop_font_size(self: &File) -> i32;
        /// The count, sum and average of a block of cells, for the footer.
        #[qinvokable]
        fn selection_summary(
            self: &File,
            sheet: i32,
            row0: i32,
            col0: i32,
            row1: i32,
            col1: i32,
        ) -> QString;
    }

    // The theme is watched from a thread of its own.
    impl cxx_qt::Threading for File {}
}

pub struct FileRust {
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
    /// Whether the system's colour scheme is dark, for where Omarchy has no say.
    system_dark: bool,
    /// Whether the Omarchy theme is being watched.
    watching: bool,
    /// What an export would lose, asked before it is written.
    export_warning: QString,
    /// Development aid: where to save a picture of the window, then quit.
    snapshot_path: QString,

    doc: Document,
    path: Option<PathBuf>,
    /// The workbook or CSV file the file was imported from, until it is saved.
    origin: Option<PathBuf>,
    /// The text as last opened or saved, to tell whether there are changes.
    saved: String,
    /// What was last cut or copied here.
    clip: Clip,
}

impl Default for FileRust {
    fn default() -> Self {
        let theme = theme::load(true);
        FileRust {
            file_name: QString::from("Untitled"),
            file_path: QString::default(),
            modified: false,
            revision: 0,
            status: QString::default(),
            notes: QString::default(),
            export_warning: QString::default(),
            can_undo: false,
            can_redo: false,
            dark_mode: theme.dark,
            theme_background: QString::from(&theme.background),
            theme_foreground: QString::from(&theme.foreground),
            theme_accent: QString::from(&theme.accent),
            theme_selection: QString::from(&theme.selection),
            theme_muted: QString::from(&theme.muted),
            theme_error: QString::from(&theme.error),
            system_dark: true,
            watching: false,
            snapshot_path: QString::from(std::env::var("OMASHEET_UI_SNAPSHOT").unwrap_or_default()),
            doc: document(BLANK),
            path: None,
            origin: None,
            saved: BLANK.to_string(),
            clip: Clip::default(),
        }
    }
}

/// How the user's locale writes dates, times and numbers, with numbers
/// grouped in threes, `12,345.5`.
fn style() -> Style {
    static STYLE: OnceLock<Style> = OnceLock::new();
    *STYLE.get_or_init(|| {
        let (point, sep) = locale::os_numbers();
        locale::os_style().numbers(point, sep)
    })
}

/// A document that shows dates and times the way the user's locale does.
fn document(text: &str) -> Document {
    Document::with_style(text, style())
}

fn index(n: i32) -> usize {
    n.max(0) as usize
}

/// What the app asks before an export writes a null in place of a number
/// held at a scale no Parquet decimal names.
const WIDE_WARNING: &str = "values too large/small for Polars Decimal128, click to export anyway";

impl qobject::File {
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
        // A `table` names its Parquet beside the file, so where the rows
        // are read from changes with the file's own place.
        let dir = match self.rust().path.as_ref().and_then(|p| p.parent()) {
            Some(dir) => dir.to_path_buf(),
            None => PathBuf::new(),
        };
        self.as_mut().rust_mut().doc.set_base(dir);
    }

    fn snapshot_json(&self) -> QString {
        QString::from(&json::snapshot(self.rust().doc.snapshot()))
    }

    fn source_text(&self) -> QString {
        QString::from(&self.rust().doc.aligned_text())
    }

    fn set_source_text(self: Pin<&mut Self>, text: &QString) {
        let text = text.to_string();
        self.edit(|doc| doc.set_text(&text));
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

    /// Open a workbook, a CSV file or JSON as a new file, not yet saved.
    fn import_file(mut self: Pin<&mut Self>, path: PathBuf, kind: &str) -> bool {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let imported = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                match kind {
                    // The app overrides neither the clock nor the zone, so the
                    // machine's own are what a formula is checked under, as
                    // they are for its exports.
                    "xlsx" => omasheet_interop::xlsx::import(&bytes, Options::default()),
                    "json" => omasheet_interop::json::import(&stem, &bytes),
                    "parquet" => omasheet_interop::parquet::import(&stem, &bytes),
                    _ => omasheet_interop::csv::import(&stem, &bytes),
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
        if matches!(kind.as_str(), "xlsx" | "csv" | "json" | "parquet") {
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

    fn export_url(
        mut self: Pin<&mut Self>,
        url: &QUrl,
        kind: &QString,
        sheet: &QString,
        force: bool,
    ) -> bool {
        self.as_mut().set_export_warning(QString::default());
        let mut path = PathBuf::from(url.to_local_file_or_default().to_string());
        if path.as_os_str().is_empty() {
            return false;
        }
        let named = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        let kind = match named {
            Some(named) if matches!(named.as_str(), "xlsx" | "csv" | "json" | "parquet") => named,
            _ => {
                let kind = kind.to_string();
                path.as_mut_os_string().push(format!(".{kind}"));
                kind
            }
        };
        let sheet = sheet.to_string();
        if matches!(kind.as_str(), "csv" | "parquet") && sheet.is_empty() {
            let message = "one sheet to a file: show the sheet to export first";
            self.as_mut().set_status(QString::from(message));
            return false;
        }
        let name = self.rust().file_name.to_string();
        let text = self.rust().doc.text().to_string();
        let exported = match kind.as_str() {
            "csv" => omasheet_interop::csv::export(&name, &text, Some(&sheet), Options::default()),
            "json" => omasheet_interop::json::export(&name, &text, None, Options::default()),
            "parquet" => {
                omasheet_interop::parquet::export(&name, &text, Some(&sheet), Options::default())
            }
            _ => omasheet_interop::xlsx::export(&name, &text, None, Options::default()),
        };
        let written = exported
            .map_err(|_| "the file has problems to put right first".to_string())
            .and_then(|Exported { files, notices }| {
                // Nothing is written until the loss is answered for: the
                // export was run to find it, and is run again to keep it.
                if !force && notices.iter().any(|n| n.starts_with(parquet::WIDE)) {
                    return Ok(None);
                }
                let bytes = files.first().map_or(&[][..], |f| &f.bytes);
                std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                Ok(Some(notices))
            });
        match written {
            Ok(None) => {
                self.as_mut()
                    .set_export_warning(QString::from(WIDE_WARNING));
                false
            }
            Ok(Some(notices)) => {
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

    fn export_suggestion(&self, kind: &QString, sheet: &QString, folder: &QUrl) -> QUrl {
        let rust = self.rust();
        let from = rust.path.as_ref().or(rust.origin.as_ref());
        let mut name = from
            .and_then(|p| p.file_stem())
            .map_or("Untitled".to_string(), |s| s.to_string_lossy().into_owned());
        let kind = kind.to_string();
        // As the command line names them: budget.Sales.csv, unless the
        // file has the one sheet.
        if matches!(kind.as_str(), "csv" | "parquet") && rust.doc.snapshot().sheets.len() > 1 {
            name.push('.');
            name.push_str(&sheet.to_string());
        }
        name.push('.');
        name.push_str(&kind);
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

    /// Edit sheet `sheet`, and say why in the status bar when it takes no
    /// edit at all: a `table` is a read-only view of its Parquet file.
    fn edit_sheet(
        mut self: Pin<&mut Self>,
        sheet: i32,
        change: impl FnOnce(&mut Document) -> bool,
    ) {
        let changed = change(&mut self.as_mut().rust_mut().doc);
        if changed {
            self.refresh("");
            return;
        }
        if let Some(why) = self.rust().doc.read_only(index(sheet)) {
            self.as_mut().set_status(QString::from(&why));
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

    fn set_cell(self: Pin<&mut Self>, sheet: i32, row: i32, col: i32, text: &QString) {
        let text = text.to_string();
        self.edit_sheet(sheet, |doc| {
            doc.set_cell(index(sheet), index(row), index(col), &text)
        });
    }

    /// Cut, copy or paste: with what was last cut or copied, and the document.
    fn clip<T>(mut self: Pin<&mut Self>, act: impl FnOnce(&mut Clip, &mut Document) -> T) -> T {
        let mut rust = self.as_mut().rust_mut();
        let rust = &mut *rust;
        act(&mut rust.clip, &mut rust.doc)
    }

    /// A paste, published if it changed anything: why not, if it was refused.
    fn pasted(
        mut self: Pin<&mut Self>,
        act: impl FnOnce(&mut Clip, &mut Document) -> Result<bool, String>,
    ) -> QString {
        match self.as_mut().clip(act) {
            Ok(changed) => {
                if changed {
                    self.refresh("");
                }
                QString::default()
            }
            Err(message) => QString::from(&message),
        }
    }

    fn copy_cells(&self, sheet: i32, row0: i32, col0: i32, row1: i32, col1: i32) -> QString {
        let (from, to) = ((index(row0), index(col0)), (index(row1), index(col1)));
        QString::from(&Clip::text(&self.rust().doc, index(sheet), from, to))
    }

    fn copy_block(
        self: Pin<&mut Self>,
        sheet: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
    ) -> QString {
        let (from, to) = ((index(row0), index(col0)), (index(row1), index(col1)));
        QString::from(&self.clip(|clip, doc| clip.copy(doc, index(sheet), from, to)))
    }

    fn cut_cells(
        self: Pin<&mut Self>,
        sheet: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
    ) -> QString {
        let (from, to) = ((index(row0), index(col0)), (index(row1), index(col1)));
        QString::from(&self.clip(|clip, doc| clip.cut(doc, index(sheet), from, to)))
    }

    fn forget_cut(self: Pin<&mut Self>) {
        self.clip(|clip, _| clip.forget_cut());
    }

    fn paste_cells(
        self: Pin<&mut Self>,
        sheet: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
        text: &QString,
    ) -> QString {
        let text = text.to_string();
        let (from, to) = ((index(row0), index(col0)), (index(row1), index(col1)));
        self.pasted(|clip, doc| clip.paste(doc, index(sheet), from, to, &text))
    }

    fn paste_values(
        self: Pin<&mut Self>,
        sheet: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
        text: &QString,
    ) -> QString {
        let text = text.to_string();
        let (from, to) = ((index(row0), index(col0)), (index(row1), index(col1)));
        self.pasted(|clip, doc| clip.paste_values(doc, index(sheet), from, to, &text))
    }

    fn paste_transposed(
        self: Pin<&mut Self>,
        sheet: i32,
        row0: i32,
        col0: i32,
        row1: i32,
        col1: i32,
        text: &QString,
    ) -> QString {
        let text = text.to_string();
        let (from, to) = ((index(row0), index(col0)), (index(row1), index(col1)));
        self.pasted(|clip, doc| clip.paste_transposed(doc, index(sheet), from, to, &text))
    }

    fn clear_cells(self: Pin<&mut Self>, sheet: i32, row0: i32, col0: i32, row1: i32, col1: i32) {
        let (from, to) = ((index(row0), index(col0)), (index(row1), index(col1)));
        self.edit_sheet(sheet, |doc| Clip::clear(doc, index(sheet), from, to));
    }

    fn insert_rows(self: Pin<&mut Self>, sheet: i32, at: i32, count: i32) {
        self.edit_sheet(sheet, |doc| {
            doc.insert_rows(index(sheet), index(at), index(count))
        });
    }

    fn delete_rows(self: Pin<&mut Self>, sheet: i32, first: i32, count: i32) {
        self.edit_sheet(sheet, |doc| {
            doc.delete_rows(index(sheet), index(first), index(count))
        });
    }

    fn add_column(self: Pin<&mut Self>, sheet: i32, name: &QString) -> QString {
        let name = name.to_string();
        self.try_edit(|doc| doc.add_column(index(sheet), &name))
    }

    fn insert_column(self: Pin<&mut Self>, sheet: i32, at: i32, name: &QString) -> QString {
        let name = name.to_string();
        self.try_edit(|doc| doc.insert_column(index(sheet), index(at), &name))
    }

    fn insert_computed(
        self: Pin<&mut Self>,
        sheet: i32,
        at: i32,
        name: &QString,
        expr: &QString,
    ) -> QString {
        let (name, expr) = (name.to_string(), expr.to_string());
        self.try_edit(|doc| doc.insert_computed(index(sheet), index(at), &name, &expr))
    }

    fn add_computed(self: Pin<&mut Self>, sheet: i32, name: &QString, expr: &QString) -> QString {
        let (name, expr) = (name.to_string(), expr.to_string());
        self.try_edit(|doc| doc.add_computed(index(sheet), &name, &expr))
    }

    fn rename_column(self: Pin<&mut Self>, sheet: i32, col: i32, name: &QString) -> QString {
        let name = name.to_string();
        self.try_edit(|doc| doc.rename_column(index(sheet), index(col), &name))
    }

    fn delete_columns(self: Pin<&mut Self>, sheet: i32, first: i32, count: i32) -> QString {
        self.try_edit(|doc| doc.delete_columns(index(sheet), index(first), index(count)))
    }

    fn set_column_types(self: Pin<&mut Self>, sheet: i32, first: i32, count: i32, ty: &QString) {
        let ty = ty.to_string();
        let cols: Vec<usize> = (index(first)..index(first) + index(count)).collect();
        self.edit_sheet(sheet, |doc| {
            doc.set_column_types(
                index(sheet),
                &cols,
                Some(ty.as_str()).filter(|t| !t.is_empty()),
            )
        });
    }

    fn move_rows(self: Pin<&mut Self>, sheet: i32, first: i32, count: i32, to: i32) -> QString {
        self.try_edit(|doc| doc.move_rows(index(sheet), index(first), index(count), index(to)))
    }

    fn move_columns(self: Pin<&mut Self>, sheet: i32, first: i32, count: i32, to: i32) -> QString {
        self.try_edit(|doc| doc.move_columns(index(sheet), index(first), index(count), index(to)))
    }

    fn add_sheet(self: Pin<&mut Self>, name: &QString) -> QString {
        let name = name.to_string();
        self.try_edit(|doc| doc.add_sheet(&name))
    }

    fn insert_sheet(self: Pin<&mut Self>, at: i32, name: &QString) -> QString {
        let name = name.to_string();
        self.try_edit(|doc| doc.insert_sheet(index(at), &name))
    }

    fn rename_sheet(self: Pin<&mut Self>, sheet: i32, name: &QString) -> QString {
        let name = name.to_string();
        self.try_edit(|doc| doc.rename_sheet(index(sheet), &name))
    }

    fn delete_sheet(self: Pin<&mut Self>, sheet: i32) -> QString {
        self.try_edit(|doc| doc.delete_sheet(index(sheet)))
    }

    fn move_sheet(self: Pin<&mut Self>, from: i32, to: i32) -> QString {
        self.try_edit(|doc| doc.move_sheet(index(from), index(to)))
    }

    fn paste_sheet(self: Pin<&mut Self>, from: i32, to: i32) -> QString {
        self.try_edit(|doc| doc.paste_sheet(index(from), index(to)).map(|_| ()))
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

    fn undo_mark(&self) -> i32 {
        self.rust().doc.undo_mark() as i32
    }

    fn join_since(mut self: Pin<&mut Self>, mark: i32) {
        // Nothing to show: the text is as it was.
        self.as_mut().rust_mut().doc.join_since(index(mark));
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

    fn help_text(&self) -> QString {
        let text = include_str!("../help.txt")
            .trim_end()
            .replace("{locale}", &self.locale_hint().to_string())
            .replace("{version}", env!("CARGO_PKG_VERSION"));
        QString::from(&text)
    }

    fn functions_json(&self) -> QString {
        QString::from(&json::functions(
            &self.rust().doc.snapshot().funcs,
            omasheet_engine::omx::funcs::FUNCTIONS,
        ))
    }

    fn desktop_font_size(&self) -> i32 {
        crate::theme::base_font_size().map_or(0, |size| size as i32)
    }

    fn entry_hint(&self, ty: &QString) -> QString {
        // Only where the way to write a value is not plain from its type.
        let ty = ty.to_string();
        if !matches!(
            ty.as_str(),
            "Date" | "Time" | "Datetime" | "Boolean" | "Url" | "Email" | "Uuid"
        ) {
            return QString::default();
        }
        // The header shows the type already.
        let hint = entry_hint(&style(), &ty);
        let hint = hint.strip_prefix(&format!("{ty}: ")).unwrap_or(&hint);
        QString::from(hint)
    }

    fn selection_summary(&self, sheet: i32, row0: i32, col0: i32, row1: i32, col1: i32) -> QString {
        let doc = &self.rust().doc;
        let from = (index(row0), index(col0));
        let Some(summary) = doc.summary(index(sheet), from, (index(row1), index(col1))) else {
            return QString::default();
        };
        let mut text = format!("Count {}", summary.count);
        if let Some((sum, mean)) = summary.numbers {
            text.push_str(&format!("   Sum {sum}   Mean {mean}"));
        }
        QString::from(&text)
    }

    fn follow_theme(mut self: Pin<&mut Self>, system_dark: bool) {
        self.as_mut().rust_mut().system_dark = system_dark;
        self.as_mut().reload_theme();
        if !self.watching {
            self.as_mut().rust_mut().watching = true;
            let thread = self.qt_thread();
            theme::watch(move || thread.queue(|file| file.reload_theme()).is_ok());
        }
    }

    fn reload_theme(mut self: Pin<&mut Self>) {
        let theme = theme::load(self.system_dark);
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

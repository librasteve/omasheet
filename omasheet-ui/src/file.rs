// Copyright (c) 2026 Stephen Roe

//! The `File` object the QML window talks to.

use crate::clip::Clip;
use crate::{json, theme};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};
use omasheet_engine::datefmt::Names;
use omasheet_engine::doc::{BLANK, PAGED_ROWS, entry_hint};
use omasheet_engine::omx::date::Style;
use omasheet_engine::{Document, Options, locale};
use omasheet_interop::parquet;
use omasheet_interop::{Exported, Imported};
use std::cell::RefCell;
use std::collections::HashSet;
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
        /// Whether something is being read in the background, for the
        /// spinner in the status bar.
        #[qproperty(bool, working)]
        type File = super::FileRust;

        /// The whole file as JSON: sheets, constants and problems.
        #[qinvokable]
        fn snapshot_json(self: &File) -> QString;
        /// `count` rows of sheet `sheet` from row `from`, as JSON in the
        /// shape of a sheet's `rows`: how the grid reads a paged sheet,
        /// whose rows the snapshot does not hold.
        #[qinvokable]
        fn rows_json(self: &File, sheet: i32, from: i32, count: i32) -> QString;
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
        /// Show the values of `count` columns from `first` by `format`,
        /// `'.2f` or `%d %b %Y`, or with an empty one as before. Empty, or
        /// why not.
        #[qinvokable]
        fn set_column_format(
            self: Pin<&mut File>,
            sheet: i32,
            first: i32,
            count: i32,
            format: &QString,
        ) -> QString;
        /// Move `count` rows from `first` so the first becomes row `to`.
        #[qinvokable]
        fn move_rows(self: Pin<&mut File>, sheet: i32, first: i32, count: i32, to: i32) -> QString;
        /// Move `count` columns from `first` so the first becomes column `to`.
        /// Filter sheet `sheet`, and say how many rows it shows: -1 where
        /// nothing is filtered and every row shows, and -2 where a filter
        /// fails, which [`filter_problem`](Self::filter_problem) then says,
        /// and the filter before stays in force. `filters` is each filtered
        /// column's name and its expression, separated by U+001F, the
        /// columns by U+001E (filter-by-expression D172). The rows are kept
        /// here, for [`row_at`](Self::row_at) and [`pos_of`](Self::pos_of)
        /// to answer from: a big table's would be millions, too many to
        /// hand to QML.
        #[qinvokable]
        fn filter_rows(self: &File, sheet: i32, filters: &QString) -> i32;
        /// Why the last filter failed, as `Column: why`.
        #[qinvokable]
        fn filter_problem(self: &File) -> QString;
        /// What is wrong with `expr` as a filter of column `col` of sheet
        /// `sheet`, or nothing: checked as it is typed, and no row read.
        #[qinvokable]
        fn check_filter(self: &File, sheet: i32, col: i32, expr: &QString) -> QString;
        /// The row shown at place `pos` of the filtered list, or -1 past its
        /// ends; `pos` itself where nothing is filtered.
        #[qinvokable]
        fn row_at(self: &File, pos: i32) -> i32;
        /// The place in the filtered list of row `row`, or -1 where it is
        /// hidden; `row` itself where nothing is filtered.
        #[qinvokable]
        fn pos_of(self: &File, row: i32) -> i32;
        /// Whether any row from `top` to `bottom` is hidden by the filter.
        #[qinvokable]
        fn hides_between(self: &File, top: i32, bottom: i32) -> bool;
        /// The rows of sheet `sheet` rearranged in the order of column
        /// `col`, descending or not.
        #[qinvokable]
        fn sort_rows(self: Pin<&mut File>, sheet: i32, col: i32, descending: bool) -> QString;
        /// A filter worked out on a worker has been: `count` rows shown,
        /// or -2 where it failed (see `filter_problem`).
        #[qsignal]
        fn filtered(self: Pin<&mut File>, count: i32);
        /// A sort done on a worker has been, with what went wrong if
        /// anything.
        #[qsignal]
        fn sorted(self: Pin<&mut File>, col: i32, descending: bool, problem: QString);
        #[qinvokable]
        fn move_columns(
            self: Pin<&mut File>,
            sheet: i32,
            first: i32,
            count: i32,
            to: i32,
        ) -> QString;
        #[qinvokable]
        fn add_sheet(self: Pin<&mut File>, name: &QString, formula: &QString) -> QString;
        /// A new sheet before sheet `at`, or last if there is none there.
        #[qinvokable]
        fn insert_sheet(
            self: Pin<&mut File>,
            at: i32,
            name: &QString,
            formula: &QString,
        ) -> QString;
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
        /// How many edits have been made, undo and redo not being edits.
        #[qinvokable]
        fn edit_count(&self) -> i32;
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
        /// What the function directory says first.
        #[qinvokable]
        fn functions_intro(self: &File) -> QString;
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

/// The rows a filter shows, by number, and the place each row has among
/// them, -1 for one it hides.
struct Shown {
    rows: Vec<usize>,
    pos: Vec<i32>,
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
    working: bool,

    doc: Document,
    path: Option<PathBuf>,
    /// The workbook or CSV file the file was imported from, until it is saved.
    origin: Option<PathBuf>,
    /// The text as last opened or saved, to tell whether there are changes.
    saved: String,
    /// What was last cut or copied here.
    clip: Clip,
    /// Why the last filter failed. See [`qobject::File::filter_problem`].
    filter_problem: RefCell<String>,
    /// The rows the filter in force shows, and each row's place among them;
    /// `None` while every row shows. See [`qobject::File::filter_rows`].
    shown: RefCell<Option<Shown>>,
    /// The tables' files being read whole on a worker (D157).
    reading: HashSet<PathBuf>,
    /// The CSV files being copied to Parquet on a worker (open-a-big-csv
    /// D153), shown from their samples meanwhile.
    converting: HashSet<PathBuf>,
    /// The number of the last filter asked for: one worked out on a worker
    /// and overtaken by another is dropped.
    filter_asked: std::cell::Cell<u64>,
    /// Whether a filter or a sort is being worked out on a worker.
    busy_jobs: std::cell::Cell<usize>,
    /// The files already said to be too big to hold, so that it is said
    /// once rather than with every edit.
    told: HashSet<PathBuf>,
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
            working: false,
            doc: document(BLANK),
            path: None,
            origin: None,
            saved: BLANK.to_string(),
            clip: Clip::default(),
            filter_problem: RefCell::new(String::new()),
            shown: RefCell::new(None),
            reading: HashSet::new(),
            converting: HashSet::new(),
            filter_asked: std::cell::Cell::new(0),
            busy_jobs: std::cell::Cell::new(0),
            told: HashSet::new(),
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
/// The size above which a CSV is opened as a table rather than imported as
/// text (open-a-big-csv D146).
const BIG_CSV: u64 = 4 * 1024 * 1024;

/// The rows above which a filter or a sort is worked out on a worker, so
/// that the window goes on answering the desktop, which takes a window
/// that stops answering for a few seconds to be hung.
const WORKER_ROWS: usize = 200_000;

fn document(text: &str) -> Document {
    static NAMES: OnceLock<Names> = OnceLock::new();
    let names = NAMES.get_or_init(locale::os_names);
    Document::on_screen(text, style(), names.clone())
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
        let copying = self.as_mut().convert_tables();
        let note = self.as_mut().hold_tables();
        let note = if note.is_empty() { copying } else { note };
        let status = if status.is_empty() {
            note.as_str()
        } else {
            status
        };
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

    fn rows_json(&self, sheet: i32, from: i32, count: i32) -> QString {
        let rows = self
            .rust()
            .doc
            .rows(index(sheet), index(from), index(count));
        QString::from(&json::rows(&rows))
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

    /// Open a big Parquet file as a new file, not yet saved, of one `table`
    /// that reads it where it is: copied into the text, as an import does,
    /// a file of millions of rows would never be shown. `false` for a file
    /// of no more rows than a sheet is formatted whole at, which is
    /// imported to be edited, and for one that cannot be read, which the
    /// import then says why of.
    ///
    /// The file is named by its whole path, so that the `.omx` saved from
    /// this reads it wherever it is saved.
    fn open_table(mut self: Pin<&mut Self>, path: &std::path::Path) -> bool {
        let Ok(full) = std::fs::canonicalize(path) else {
            return false;
        };
        let Some(full) = full.to_str() else {
            return false;
        };
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let name = match omasheet_interop::identifier(&stem) {
            empty if empty.is_empty() => "Table".to_string(),
            name => name,
        };
        let doc = document(&format!(
            "table {name} := dataframe[{}]\n",
            omasheet_engine::omx::lexer::quoted(full)
        ));
        let csv = omasheet_engine::table::is_csv(path);
        let rows = doc.row_count(0);
        if !csv && (rows <= PAGED_ROWS || !doc.snapshot().sheets[0].paged) {
            return false;
        }
        // A CSV that cannot be read as one is imported, which says why.
        if csv && !doc.snapshot().problems.is_empty() {
            return false;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.doc = doc;
            rust.saved = String::new();
        }
        let file = path.file_name().unwrap_or_default().to_string_lossy();
        // No notes, and so no dialog: the table is there to be seen. The
        // status bar says what it is; for a CSV, that its first rows are
        // shown while the rest is read, which `convert_tables` says.
        self.as_mut().set_notes(QString::default());
        self.as_mut().set_path(None);
        self.as_mut().rust_mut().origin = Some(path.to_path_buf());
        let status = if csv {
            String::new()
        } else {
            format!("Opened {file} as a read-only table ({rows} rows)")
        };
        self.refresh(&status);
        true
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
        if kind == "parquet" && self.as_mut().open_table(&path) {
            return true;
        }
        // A CSV too big to import as text (open-a-big-csv D146).
        if kind == "csv"
            && std::fs::metadata(&path).is_ok_and(|m| m.len() > BIG_CSV)
            && self.as_mut().open_table(&path)
        {
            return true;
        }
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

    fn set_column_format(
        self: Pin<&mut Self>,
        sheet: i32,
        first: i32,
        count: i32,
        format: &QString,
    ) -> QString {
        let format = format.to_string();
        let format = format.trim();
        let cols: Vec<usize> = (index(first)..index(first) + index(count)).collect();
        self.try_edit(|doc| {
            doc.set_column_format(index(sheet), &cols, Some(format).filter(|f| !f.is_empty()))
        })
    }

    fn move_rows(self: Pin<&mut Self>, sheet: i32, first: i32, count: i32, to: i32) -> QString {
        self.try_edit(|doc| doc.move_rows(index(sheet), index(first), index(count), index(to)))
    }

    fn filter_rows(&self, sheet: i32, filters: &QString) -> i32 {
        let filters: Vec<(String, String)> = filters
            .to_string()
            .split('\u{1e}')
            .filter_map(|column| {
                let (name, expr) = column.split_once('\u{1f}')?;
                Some((name.to_string(), expr.to_string())).filter(|(_, e)| !e.trim().is_empty())
            })
            .collect();
        let rust = self.rust();
        let asked = rust.filter_asked.get() + 1;
        rust.filter_asked.set(asked);
        if filters.is_empty() {
            *rust.shown.borrow_mut() = None;
            return -1;
        }
        let job = match rust.doc.filter_job(index(sheet), &filters) {
            Ok(job) => job,
            Err(why) => {
                *rust.filter_problem.borrow_mut() = why;
                return -2;
            }
        };
        // A big one is run on a worker, so that the window goes on
        // answering the desktop; the rows shown stay as they were until it
        // comes back, and -3 says so.
        if job.rows() > WORKER_ROWS {
            rust.busy_jobs.set(rust.busy_jobs.get() + 1);
            let thread = self.qt_thread();
            std::thread::spawn(move || {
                let rows = job.run();
                let _ = thread.queue(move |file| file.filter_ran(asked, sheet, filters, rows));
            });
            return -3;
        }
        let rows = match job.run() {
            Some(rows) => Ok(rows),
            None => rust.doc.shown_rows(index(sheet), &filters),
        };
        match rows {
            Ok(rows) => self.show_rows(sheet, rows),
            Err(why) => {
                *rust.filter_problem.borrow_mut() = why;
                -2
            }
        }
    }

    /// Keep `rows` as the rows the filter shows, and give their count.
    fn show_rows(&self, sheet: i32, rows: Vec<usize>) -> i32 {
        let rust = self.rust();
        let n = rust.doc.row_count(index(sheet));
        let mut pos = vec![-1; n];
        for (k, &r) in rows.iter().enumerate() {
            if let Some(p) = pos.get_mut(r) {
                *p = k as i32;
            }
        }
        let count = rows.len() as i32;
        *rust.shown.borrow_mut() = Some(Shown { rows, pos });
        count
    }

    fn filter_problem(&self) -> QString {
        QString::from(&*self.rust().filter_problem.borrow())
    }

    fn check_filter(&self, sheet: i32, col: i32, expr: &QString) -> QString {
        match self.rust().doc.check_filter(index(sheet), index(col), &expr.to_string()) {
            Ok(()) => QString::default(),
            Err(why) => QString::from(&why),
        }
    }

    fn row_at(&self, pos: i32) -> i32 {
        match &*self.rust().shown.borrow() {
            None => pos,
            Some(shown) => usize::try_from(pos)
                .ok()
                .and_then(|p| shown.rows.get(p))
                .map_or(-1, |&r| r as i32),
        }
    }

    fn pos_of(&self, row: i32) -> i32 {
        match &*self.rust().shown.borrow() {
            None => row,
            Some(shown) => usize::try_from(row)
                .ok()
                .and_then(|r| shown.pos.get(r))
                .copied()
                .unwrap_or(-1),
        }
    }

    fn hides_between(&self, top: i32, bottom: i32) -> bool {
        match &*self.rust().shown.borrow() {
            None => false,
            Some(shown) => {
                let (top, bottom) = (index(top.min(bottom)), index(top.max(bottom)));
                shown
                    .pos
                    .get(top..=bottom.min(shown.pos.len().saturating_sub(1)))
                    .is_some_and(|block| block.iter().any(|&p| p < 0))
            }
        }
    }

    fn sort_rows(mut self: Pin<&mut Self>, sheet: i32, col: i32, descending: bool) -> QString {
        // A big table is put in order on a worker; `sorted` says when, and
        // the "…" returned now says it is coming.
        let job = self.rust().doc.sort_job(index(sheet), index(col), descending);
        if let Some(job) = job.filter(|_| self.rust().doc.row_count(index(sheet)) > WORKER_ROWS) {
            self.rust().busy_jobs.set(self.rust().busy_jobs.get() + 1);
            self.as_mut().set_working(true);
            let thread = self.qt_thread();
            std::thread::spawn(move || {
                let shown = job.run();
                let _ = thread.queue(move |file| file.sort_ran(job, shown, col, descending));
            });
            return QString::from("\u{2026}");
        }
        self.try_edit(|doc| doc.sort_rows(index(sheet), index(col), descending))
    }

    /// A sort run on a worker has come back: the table is put in that
    /// order, unless it has changed meanwhile and is sorted again here.
    fn sort_ran(
        mut self: Pin<&mut Self>,
        job: omasheet_engine::doc::SortJob,
        shown: Vec<usize>,
        col: i32,
        descending: bool,
    ) {
        self.rust().busy_jobs.set(self.rust().busy_jobs.get().saturating_sub(1));
        let problem = self.as_mut().try_edit(move |doc| doc.finish_sort(job, shown));
        self.as_mut().update_working();
        self.as_mut().sorted(col, descending, problem);
    }

    /// A filter run on a worker has come back: kept, unless another has
    /// been asked for since.
    fn filter_ran(
        mut self: Pin<&mut Self>,
        asked: u64,
        sheet: i32,
        filters: Vec<(String, String)>,
        rows: Option<Vec<usize>>,
    ) {
        self.rust().busy_jobs.set(self.rust().busy_jobs.get().saturating_sub(1));
        self.as_mut().update_working();
        if asked != self.rust().filter_asked.get() {
            return;
        }
        let rows = match rows {
            Some(rows) => Ok(rows),
            None => self.rust().doc.shown_rows(index(sheet), &filters),
        };
        let count = match rows {
            Ok(rows) => self.show_rows(sheet, rows),
            Err(why) => {
                *self.rust().filter_problem.borrow_mut() = why;
                -2
            }
        };
        self.as_mut().filtered(count);
    }

    /// Whether anything is being done on a worker, for the spinner.
    fn update_working(mut self: Pin<&mut Self>) {
        let rust = self.rust();
        let busy = !rust.reading.is_empty() || !rust.converting.is_empty() || rust.busy_jobs.get() > 0;
        self.as_mut().set_working(busy);
    }

    fn move_columns(self: Pin<&mut Self>, sheet: i32, first: i32, count: i32, to: i32) -> QString {
        self.try_edit(|doc| doc.move_columns(index(sheet), index(first), index(count), index(to)))
    }

    fn add_sheet(self: Pin<&mut Self>, name: &QString, formula: &QString) -> QString {
        let formula = formula.to_string();
        let name = name.to_string();
        self.try_edit(|doc| doc.add_sheet(&name, Some(formula.as_str())))
    }

    fn insert_sheet(self: Pin<&mut Self>, at: i32, name: &QString, formula: &QString) -> QString {
        let formula = formula.to_string();
        let name = name.to_string();
        self.try_edit(|doc| doc.insert_sheet(index(at), &name, Some(formula.as_str())))
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

    fn edit_count(&self) -> i32 {
        self.rust().doc.edit_count() as i32
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
            omasheet_engine::omx::funcs::ABOUT,
        ))
    }

    fn functions_intro(&self) -> QString {
        QString::from(omasheet_engine::omx::funcs::INTRO)
    }

    fn desktop_font_size(&self) -> i32 {
        crate::theme::base_font_size().map_or(0, |size| size as i32)
    }

    fn entry_hint(&self, ty: &QString) -> QString {
        // Only where the way to write a value is not plain from its type.
        let ty = ty.to_string();
        if !matches!(
            ty.as_str(),
            "Date" | "Time" | "Datetime" | "Duration" | "Boolean" | "Url" | "Email" | "Uuid"
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

    /// Read whole, on a worker, each paged table that fits in half the memory
    /// there is free, so that it is given from memory once it arrives
    /// (D157); and say of one that does not fit that it is kept on disk,
    /// and why. Returns what to say, if anything.
    fn hold_tables(mut self: Pin<&mut Self>) -> String {
        let wanted = self.rust().doc.to_hold();
        if wanted.is_empty() {
            return String::new();
        }
        let Some(free) = mem_available() else {
            return String::new();
        };
        let mut note = String::new();
        for want in wanted {
            let name = file_name(&self.rust().doc.source_file(&want.path));
            if self.rust().reading.contains(&want.path) {
                continue;
            }
            if want.bytes >= free / 2 {
                if self.as_mut().rust_mut().told.insert(want.path.clone()) {
                    note = format!(
                        "{name} kept on disk: {} to hold, {} free",
                        bytes_text(want.bytes),
                        bytes_text(free)
                    );
                }
                continue;
            }
            self.as_mut().rust_mut().reading.insert(want.path.clone());
            self.as_mut().set_working(true);
            note = format!(
                "Reading {name} into memory ({})\u{2026}",
                bytes_text(want.bytes)
            );
            let thread = self.qt_thread();
            std::thread::spawn(move || {
                let got = omasheet_engine::table::read_whole(&want.path, &want.columns);
                let _ = thread.queue(move |file| file.whole_read(want.path, want.bytes, got));
            });
        }
        note
    }

    /// Copy to Parquet, on a worker each, the CSV files the document's
    /// tables read that have no copy yet (open-a-big-csv D153). Returns
    /// what to say, if anything.
    fn convert_tables(mut self: Pin<&mut Self>) -> String {
        let mut note = String::new();
        for csv in self.rust().doc.to_convert() {
            if !self.as_mut().rust_mut().converting.insert(csv.clone()) {
                continue;
            }
            self.as_mut().set_working(true);
            note = format!(
                "Showing the first {} rows of {}, read-only, while the rest is read\u{2026}",
                omasheet_engine::table::SAMPLE_ROWS,
                file_name(&csv)
            );
            let thread = self.qt_thread();
            std::thread::spawn(move || {
                let got = omasheet_engine::table::convert(&csv).map(|_| ());
                let _ = thread.queue(move |file| file.csv_converted(csv, got));
            });
        }
        note
    }

    /// A CSV's copy has been written, or could not be.
    fn csv_converted(mut self: Pin<&mut Self>, csv: PathBuf, got: Result<(), String>) {
        self.as_mut().rust_mut().converting.remove(&csv);
        self.as_mut().update_working();
        let name = file_name(&csv);
        match got {
            Ok(()) => {
                if self.as_mut().rust_mut().doc.converted(&csv) {
                    let rows = self.rust().doc.snapshot().sheets.first().map_or(0, |t| t.nrows);
                    self.refresh(&format!("{name} read: {rows} rows"));
                }
            }
            Err(why) => self
                .as_mut()
                .set_status(QString::from(&format!("Cannot read all of {name}: {why}"))),
        }
    }

    /// A table's file has been read whole, or could not be. `bytes` is what
    /// it was estimated to take, which is what it takes (measured within 3%
    /// of the memory it adds), where Polars' own `estimated_size` counts
    /// little of a column of text.
    fn whole_read(
        mut self: Pin<&mut Self>,
        path: PathBuf,
        bytes: usize,
        got: Result<omasheet_engine::table::Whole, String>,
    ) {
        self.as_mut().rust_mut().reading.remove(&path);
        self.as_mut().update_working();
        let name = file_name(&self.rust().doc.source_file(&path));
        match got {
            Ok(whole) => {
                let size = bytes_text(bytes);
                if self.as_mut().rust_mut().doc.hold(path, whole) {
                    self.refresh(&format!("{name} held in memory ({size})"));
                }
            }
            Err(why) => self.as_mut().set_status(QString::from(&format!(
                "Cannot read {name} into memory: {why}"
            ))),
        }
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

/// The memory the system says can be had without swapping, in bytes:
/// `MemAvailable` in `/proc/meminfo`. `None` where there is no such file,
/// and then no table is held whole.
fn mem_available() -> Option<usize> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = info.lines().find(|l| l.starts_with("MemAvailable:"))?;
    let kb: usize = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

/// A size as the status bar says it: `501 MB`, `3.2 GB`.
fn bytes_text(bytes: usize) -> String {
    let mb = bytes as f64 / 1e6;
    if mb < 1000.0 {
        format!("{mb:.0} MB")
    } else {
        format!("{:.1} GB", mb / 1000.0)
    }
}

/// The last part of a path, for the status bar.
fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

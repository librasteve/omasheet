// Copyright (c) 2026 Stephen Roe

//! The Omasheet engine: evaluates files compiled by `omasheet-omx`.
//!
//! [`view`], [`lint`] and [`eval`] are the three things the command line
//! does; each returns what to print and the diagnostics, already rendered.

pub mod doc;
pub mod eval;
pub mod locale;
pub mod value;
pub mod zone;

pub use doc::Document;
pub use eval::Engine;
pub mod dtype;
pub mod lower;
pub mod table;

pub use omasheet_omx as omx;
pub use value::Value;

use omasheet_omx::date::Style;
use omasheet_omx::{Diagnostic, Program, Sources, compile, compile_expr};
use zone::Zone;

/// What evaluation takes from its surroundings. The default is the ISO
/// style, and the clock and time zone of this machine.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// The directory a sheet's source is read relative to: the directory of
    /// the `.omx` file, not the one the command was run in. `None` is the
    /// working directory, which is what an expression with no file has.
    pub base: Option<std::path::PathBuf>,
    /// How dates and times are shown. A file's text is always ISO.
    pub style: Style,
    /// What `now()` gives in place of the clock, as microseconds since the
    /// start of 1970-01-01 on the clocks of the file's time zone — the unit
    /// a date-time is held in.
    pub now: Option<i64>,
    /// The time zone of this machine, in place of the one it is set to. It
    /// is also the zone of a file that names none.
    pub zone: Option<Zone>,
    /// Show date-times on the clocks of this machine rather than of the
    /// file's zone.
    pub local: bool,
    /// Show every `Num` in full, `1.4142135623730951`, rather than as the
    /// decimal it is rounded to for display, `1.41421…`. An exact number
    /// is always shown in full: `19.99`, `1/3`.
    pub exact: bool,
    /// Whether a column that can be calculated in one step over the whole
    /// dataframe is (D74).
    pub lowering: Lowering,
}

/// Whether a column is calculated by Polars over the whole column or by the
/// engine row by row, where both can do it.
///
/// The two agree, which is what the differential test over the example
/// corpus checks; this is the lever that test pulls, the bisection tool
/// when a result changes, and the way out for a file that miscalculates in
/// the field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lowering {
    /// By Polars where there are enough rows for it to pay: see
    /// [`ROW_THRESHOLD`].
    #[default]
    Auto,
    /// Always by the engine, row by row, as before there was a dataframe.
    Never,
    /// By Polars wherever it can be, however few the rows, and what was
    /// refused is recorded.
    Always,
}

/// How many rows a sheet needs before a column of it is lowered under
/// [`Lowering::Auto`]. Building a plan, optimising it and collecting it are
/// slower than the row loop for a sheet of ten rows, and a sheet of ten
/// rows is omasheet's usual document. The figure is a starting point and
/// not yet a measurement (D73).
pub const ROW_THRESHOLD: usize = 1024;

/// What a command produced: text for standard output, and rendered
/// diagnostics. Any diagnostic means the command failed.
#[derive(Debug, Default)]
pub struct Outcome {
    pub output: String,
    pub errors: Vec<String>,
}

impl Outcome {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    fn report(&mut self, sources: &Sources, diags: &[Diagnostic]) {
        self.errors.extend(diags.iter().map(|d| sources.render(d)));
    }
}

/// Where a sheet's source is read from: beside the file, or the working
/// directory for an expression that has no file.
fn base_of(options: &Options) -> &std::path::Path {
    options
        .base
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."))
}

/// Parse and statically check a file without evaluating it.
pub fn lint(name: &str, text: &str) -> Outcome {
    let mut sources = Sources::new();
    let src = sources.add(name, text);
    let (program, mut diags) = compile(text, src);
    diags.extend(eval::unknown_zone(&program));
    let mut out = Outcome::default();
    out.report(&sources, &diags);
    out
}

/// Evaluate a file and show its constants and sheets with computed values.
pub fn view(name: &str, text: &str) -> Outcome {
    view_with(name, text, Options::default())
}

pub fn view_with(name: &str, text: &str, options: Options) -> Outcome {
    let mut sources = Sources::new();
    let src = sources.add(name, text);
    let (mut program, mut diags) = compile(text, src);
    let mut out = Outcome::default();
    let mut tables = table::Tables::default();
    if diags.is_empty() {
        tables = table::read(&mut program, base_of(&options), &mut diags);
    }
    if !diags.is_empty() {
        out.report(&sources, &diags);
        return out;
    }
    let program = program;
    let engine = Engine::with_tables(&program, options, tables);
    let zone_diags = engine.take_diags();
    if !zone_diags.is_empty() {
        out.report(&sources, &zone_diags);
        return out;
    }
    engine.run();
    out.output = render_file(&program, &engine);
    out.report(&sources, &engine.take_diags());
    out
}

fn render_file(program: &Program, engine: &Engine) -> String {
    let mut blocks = Vec::new();
    let mut consts = String::new();
    for (i, c) in program.consts.iter().enumerate() {
        let shown = match engine.const_value(i) {
            Ok(Value::Sheet(_)) => "<sheet>".to_string(),
            Ok(Value::Row(_)) => "<row>".to_string(),
            Ok(Value::Null) => "empty".to_string(),
            Ok(v) => engine.show(&v, true),
            Err(_) => engine.show(&Value::Error, false),
        };
        consts.push_str(&format!("const {} = {}\n", c.name, shown));
    }
    if !consts.is_empty() {
        blocks.push(consts);
    }
    for (t, sheet) in program.sheets.iter().enumerate() {
        blocks.push(format!(
            "sheet {}\n\n{}",
            sheet.name,
            engine.render_sheet(t)
        ));
    }
    blocks.join("\n")
}

/// Evaluate one expression, optionally against a file given as
/// `(name, text)`.
pub fn eval(file: Option<(&str, &str)>, expr_name: &str, expr: &str) -> Outcome {
    eval_with(file, expr_name, expr, Options::default())
}

pub fn eval_with(
    file: Option<(&str, &str)>,
    expr_name: &str,
    expr: &str,
    options: Options,
) -> Outcome {
    let mut sources = Sources::new();
    let mut out = Outcome::default();
    let mut tables = table::Tables::default();
    let program = match file {
        Some((name, text)) => {
            let src = sources.add(name, text);
            let (mut program, mut diags) = compile(text, src);
            if diags.is_empty() {
                tables = table::read(&mut program, base_of(&options), &mut diags);
            }
            if !diags.is_empty() {
                out.report(&sources, &diags);
                return out;
            }
            program
        }
        None => Program::default(),
    };
    let src = sources.add(expr_name, expr);
    let (node, _) = match compile_expr(&program, expr, src) {
        Ok(checked) => checked,
        Err(diags) => {
            out.report(&sources, &diags);
            return out;
        }
    };
    // Calculate the file first, in order, so the expression reads finished
    // cells. Failures in cells the expression never reads do not matter.
    let engine = Engine::with_tables(&program, options, tables);
    let zone_diags = engine.take_diags();
    if !zone_diags.is_empty() {
        out.report(&sources, &zone_diags);
        return out;
    }
    engine.run();
    let file_diags = engine.take_diags();
    match engine.eval_top(&node) {
        Some(value) => out.output = engine.render_value(&value),
        None => {
            let own = engine.take_diags();
            out.report(&sources, if own.is_empty() { &file_diags } else { &own });
        }
    }
    out
}

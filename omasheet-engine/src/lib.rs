//! The Omasheet engine: evaluates sheets compiled by `omasheet-omx`.
//!
//! [`view`], [`lint`] and [`eval`] are the three things the command line
//! does; each returns what to print and the diagnostics, already rendered.

pub mod doc;
pub mod eval;
pub mod value;

pub use doc::Document;
pub use eval::Engine;
pub use omasheet_omx as omx;
pub use value::Value;

use omasheet_omx::{Diagnostic, Program, Sources, compile, compile_expr};
use value::format_scalar;

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

/// Parse and statically check a sheet without evaluating it.
pub fn lint(name: &str, text: &str) -> Outcome {
    let mut sources = Sources::new();
    let src = sources.add(name, text);
    let (_, diags) = compile(text, src);
    let mut out = Outcome::default();
    out.report(&sources, &diags);
    out
}

/// Evaluate a sheet and show its constants and tables with computed values.
pub fn view(name: &str, text: &str) -> Outcome {
    let mut sources = Sources::new();
    let src = sources.add(name, text);
    let (program, diags) = compile(text, src);
    let mut out = Outcome::default();
    if !diags.is_empty() {
        out.report(&sources, &diags);
        return out;
    }
    let engine = Engine::new(&program);
    engine.run();
    out.output = render_sheet(&program, &engine);
    out.report(&sources, &engine.take_diags());
    out
}

fn render_sheet(program: &Program, engine: &Engine) -> String {
    let mut blocks = Vec::new();
    let mut consts = String::new();
    for (i, c) in program.consts.iter().enumerate() {
        let shown = match engine.const_value(i) {
            Ok(Value::Table(_)) => "<table>".to_string(),
            Ok(Value::Row(_)) => "<row>".to_string(),
            Ok(Value::Empty) => "empty".to_string(),
            Ok(v) => format_scalar(&v, true),
            Err(_) => format_scalar(&Value::Error, false),
        };
        consts.push_str(&format!("const {} = {}\n", c.name, shown));
    }
    if !consts.is_empty() {
        blocks.push(consts);
    }
    for (t, table) in program.tables.iter().enumerate() {
        blocks.push(format!(
            "table {}\n\n{}",
            table.name,
            engine.render_table(t)
        ));
    }
    blocks.join("\n")
}

/// Evaluate one expression, optionally against a sheet given as
/// `(name, text)`.
pub fn eval(sheet: Option<(&str, &str)>, expr_name: &str, expr: &str) -> Outcome {
    let mut sources = Sources::new();
    let mut out = Outcome::default();
    let program = match sheet {
        Some((name, text)) => {
            let src = sources.add(name, text);
            let (program, diags) = compile(text, src);
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
    // Calculate the sheet first, in order, so the expression reads finished
    // cells. Failures in cells the expression never reads do not matter.
    let engine = Engine::new(&program);
    engine.run();
    let sheet_diags = engine.take_diags();
    match engine.eval_top(&node) {
        Some(value) => out.output = engine.render_value(&value),
        None => {
            let own = engine.take_diags();
            out.report(&sources, if own.is_empty() { &sheet_diags } else { &own });
        }
    }
    out
}

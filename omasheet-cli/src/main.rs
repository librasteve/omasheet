//! The `omasheet` command.

use clap::{Parser, Subcommand};
use omasheet_engine::Outcome;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "omasheet",
    version,
    about = "A text-native spreadsheet: view, evaluate and check .omx sheets",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// A sheet to evaluate and show (read-only)
    file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Evaluate one OMX expression and print its value
    ///
    /// The expression is read from standard input when it is not given.
    Eval {
        /// A sheet whose tables and constants the expression may use
        #[arg(long, value_name = "FILE")]
        sheet: Option<PathBuf>,

        /// [FILE] EXPRESSION
        #[arg(value_name = "ARGS", num_args = 0..=2)]
        args: Vec<String>,
    },
    /// Check a sheet without evaluating it
    Lint {
        /// The sheet to check
        file: PathBuf,
    },
}

fn read(path: &Path) -> Result<String, ExitCode> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("omasheet: cannot read {}: {e}", path.display());
        ExitCode::from(2)
    })
}

fn finish(outcome: Outcome) -> ExitCode {
    print!("{}", outcome.output);
    for (i, error) in outcome.errors.iter().enumerate() {
        if i > 0 {
            eprintln!();
        }
        eprint!("{error}");
    }
    if outcome.ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run(cli: Cli) -> Result<ExitCode, ExitCode> {
    match cli.command {
        Some(Command::Lint { file }) => {
            let text = read(&file)?;
            Ok(finish(omasheet_engine::lint(
                &file.to_string_lossy(),
                &text,
            )))
        }
        Some(Command::Eval { sheet, mut args }) => {
            // `eval FILE EXPR` is the same as `eval --sheet FILE EXPR`.
            let sheet = match (sheet, args.len()) {
                (None, 2) => Some(PathBuf::from(args.remove(0))),
                (Some(_), 2) => {
                    eprintln!(
                        "omasheet: give the sheet once, either as --sheet or before the expression"
                    );
                    return Err(ExitCode::from(2));
                }
                (sheet, _) => sheet,
            };
            let (expr_name, expr) = match args.pop() {
                Some(expr) => ("<expression>", expr),
                None => {
                    let mut buffer = String::new();
                    std::io::stdin().read_to_string(&mut buffer).map_err(|e| {
                        eprintln!("omasheet: cannot read standard input: {e}");
                        ExitCode::from(2)
                    })?;
                    ("<stdin>", buffer)
                }
            };
            let loaded = match &sheet {
                Some(path) => Some((path.to_string_lossy().into_owned(), read(path)?)),
                None => None,
            };
            let sheet_ref = loaded.as_ref().map(|(n, t)| (n.as_str(), t.as_str()));
            Ok(finish(omasheet_engine::eval(sheet_ref, expr_name, &expr)))
        }
        None => {
            let Some(file) = cli.file else {
                eprintln!("omasheet: give a sheet to view, or see `omasheet --help`");
                return Err(ExitCode::from(2));
            };
            let text = read(&file)?;
            Ok(finish(omasheet_engine::view(
                &file.to_string_lossy(),
                &text,
            )))
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) | Err(code) => code,
    }
}

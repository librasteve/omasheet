// Copyright (c) 2026 Stephen Roe

//! The `omasheet` command.

use clap::{Args, Parser, Subcommand};
use omasheet_engine::omx::date::Style;
use omasheet_engine::zone::Zone;
use omasheet_engine::{Options, Outcome, locale};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "omasheet",
    version,
    about = "A text-native spreadsheet: view, evaluate, check, import, export and render .omx sheets",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// A sheet to evaluate and show (read-only)
    file: Option<PathBuf>,

    #[command(flatten)]
    env: Env,
}

#[derive(Args)]
struct Env {
    /// Show dates and times the way this machine's locale writes them,
    /// rather than as the sheet does (2025-01-31, 17:05), and date-times on
    /// this machine's clocks rather than those of the sheet's time zone
    #[arg(long)]
    locale: bool,

    /// What `today()` and `now()` give, in place of the clock
    #[arg(long, value_name = "DATETIME")]
    now: Option<String>,

    /// The time zone of this machine, in place of the one it is set to; it
    /// is also the zone of a sheet that names none
    #[arg(long, value_name = "ZONE")]
    zone: Option<String>,

    /// Show every number in full (1/3), rather than rounded to five
    /// digits after the decimal point (0.33333…)
    #[arg(long)]
    exact: bool,
}

impl Env {
    fn options(&self) -> Result<Options, ExitCode> {
        let mut options = Options {
            exact: self.exact,
            ..Options::default()
        };
        if self.locale {
            options.style = locale::os_style();
            options.local = true;
        }
        if let Some(name) = &self.zone {
            options.zone = Some(Zone::named(name).ok_or_else(|| {
                eprintln!(
                    "omasheet: there is no time zone `{name}`; use a name such as Europe/London"
                );
                ExitCode::from(2)
            })?);
        }
        if let Some(text) = &self.now {
            let iso = Style::ISO;
            let day = || Some(iso.parse_date(text)? as i64 * 86_400);
            options.now = Some(iso.parse_datetime(text).or_else(day).ok_or_else(|| {
                eprintln!("omasheet: `{text}` is not a date-time; write it like 2025-01-31T17:05");
                ExitCode::from(2)
            })?);
        }
        Ok(options)
    }
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

        #[command(flatten)]
        env: Env,
    },
    /// Check a sheet without evaluating it
    Lint {
        /// The sheet to check
        file: PathBuf,
    },
    /// Write a workbook (.xlsx) or a CSV file (.csv) as a sheet
    ///
    /// The sheet is printed, and anything that was changed or left behind
    /// is reported.
    Import {
        /// The workbook or CSV file to read
        file: PathBuf,

        /// Write the sheet to this file rather than printing it
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Write the calculated tables of a sheet as a workbook or as CSV
    ///
    /// The file is written beside the sheet: budget.omx as budget.xlsx or
    /// budget.csv. CSV holds one table, so a sheet with several is written
    /// as budget.Sales.csv, budget.Summary.csv.
    Export {
        /// The sheet to export
        file: PathBuf,

        /// Write a workbook, with a worksheet for each table
        #[arg(long, conflicts_with = "csv")]
        xlsx: bool,

        /// Write CSV, a file for each table
        #[arg(long)]
        csv: bool,

        /// Export this table alone
        #[arg(long, value_name = "NAME")]
        table: Option<String>,

        /// Write to this file rather than beside the sheet
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        #[command(flatten)]
        env: Env,
    },
    /// Render a Markdown document with its sheets calculated
    ///
    /// A fenced `omx` block is shown as its tables, and {{ expression }} as
    /// its value. `sheets: [budget.omx]` in the front matter names sheets
    /// to read as well. The document is printed as Markdown, or as HTML.
    Render {
        /// The Markdown document to render
        file: PathBuf,

        /// Write an HTML page
        #[arg(long)]
        html: bool,

        /// Write to this file rather than printing
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        #[command(flatten)]
        env: Env,
    },
}

fn read(path: &Path) -> Result<String, ExitCode> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("omasheet: cannot read {}: {e}", path.display());
        ExitCode::from(2)
    })
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), ExitCode> {
    std::fs::write(path, bytes).map_err(|e| {
        eprintln!("omasheet: cannot write {}: {e}", path.display());
        ExitCode::from(2)
    })
}

fn usage<T>(message: &str) -> Result<T, ExitCode> {
    eprintln!("omasheet: {message}");
    Err(ExitCode::from(2))
}

fn note(notices: &[String]) {
    for notice in notices {
        eprintln!("note: {notice}");
    }
}

fn extension(path: &Path) -> String {
    let ext = path.extension().unwrap_or_default();
    ext.to_string_lossy().to_ascii_lowercase()
}

fn import(file: &Path, output: Option<&Path>) -> Result<ExitCode, ExitCode> {
    let kind = extension(file);
    if !matches!(kind.as_str(), "xlsx" | "csv") {
        return usage(&format!(
            "cannot import {}: give a workbook (.xlsx) or a CSV file (.csv)",
            file.display()
        ));
    }
    let bytes = std::fs::read(file).map_err(|e| {
        eprintln!("omasheet: cannot read {}: {e}", file.display());
        ExitCode::from(2)
    })?;
    let imported = match kind.as_str() {
        "xlsx" => omasheet_interop::xlsx::import(&bytes),
        _ => {
            let stem = file.file_stem().unwrap_or_default().to_string_lossy();
            omasheet_interop::csv::import(&stem, &bytes)
        }
    };
    let imported = match imported {
        Ok(imported) => imported,
        Err(why) => {
            eprintln!("omasheet: cannot import {}: {why}", file.display());
            return Ok(ExitCode::FAILURE);
        }
    };
    match output {
        Some(path) => write(path, imported.source.as_bytes())?,
        None => print!("{}", imported.source),
    }
    note(&imported.notices);
    Ok(ExitCode::SUCCESS)
}

fn export(
    file: &Path,
    xlsx: bool,
    table: Option<&str>,
    output: Option<&Path>,
    options: Options,
) -> Result<ExitCode, ExitCode> {
    let text = read(file)?;
    let name = file.to_string_lossy();
    let exported = if xlsx {
        omasheet_interop::xlsx::export(&name, &text, table, options)
    } else {
        omasheet_interop::csv::export(&name, &text, table, options)
    };
    let exported = match exported {
        Ok(exported) => exported,
        Err(errors) => {
            return Ok(finish(Outcome {
                errors,
                ..Outcome::default()
            }));
        }
    };
    let ext = if xlsx { "xlsx" } else { "csv" };
    if output.is_some() && exported.files.len() > 1 {
        return usage(
            "the sheet has several tables: give one with --table to write it to --output",
        );
    }
    for out in &exported.files {
        let path = match (output, &out.table) {
            (Some(path), _) => path.to_path_buf(),
            (None, Some(table)) => file.with_extension(format!("{table}.{ext}")),
            (None, None) => file.with_extension(ext),
        };
        write(&path, &out.bytes)?;
        eprintln!("wrote {}", path.display());
    }
    note(&exported.notices);
    Ok(ExitCode::SUCCESS)
}

fn render(
    file: &Path,
    html: bool,
    output: Option<&Path>,
    options: Options,
) -> Result<ExitCode, ExitCode> {
    let text = read(file)?;
    // A sheet is named from where the document is.
    let load = |path: &str| {
        let path = file.parent().unwrap_or(Path::new("")).join(path);
        std::fs::read_to_string(&path)
            .map(|text| (path.to_string_lossy().into_owned(), text))
            .map_err(|e| e.to_string())
    };
    let format = if html {
        omasheet_md::Format::Html
    } else {
        omasheet_md::Format::Markdown
    };
    let mut outcome = omasheet_md::render(&file.to_string_lossy(), &text, &load, format, options);
    if let Some(path) = output.filter(|_| outcome.ok()) {
        write(path, std::mem::take(&mut outcome.output).as_bytes())?;
    }
    Ok(finish(outcome))
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
        Some(Command::Eval {
            sheet,
            mut args,
            env,
        }) => {
            let options = env.options()?;
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
            Ok(finish(omasheet_engine::eval_with(
                sheet_ref, expr_name, &expr, options,
            )))
        }
        Some(Command::Import { file, output }) => import(&file, output.as_deref()),
        Some(Command::Export {
            file,
            xlsx,
            csv,
            table,
            output,
            env,
        }) => {
            if !xlsx && !csv {
                return usage("say what to export as: --xlsx or --csv");
            }
            let options = env.options()?;
            export(&file, xlsx, table.as_deref(), output.as_deref(), options)
        }
        Some(Command::Render {
            file,
            html,
            output,
            env,
        }) => render(&file, html, output.as_deref(), env.options()?),
        None => {
            let Some(file) = cli.file else {
                eprintln!("omasheet: give a sheet to view, or see `omasheet --help`");
                return Err(ExitCode::from(2));
            };
            let options = cli.env.options()?;
            let text = read(&file)?;
            Ok(finish(omasheet_engine::view_with(
                &file.to_string_lossy(),
                &text,
                options,
            )))
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) | Err(code) => code,
    }
}

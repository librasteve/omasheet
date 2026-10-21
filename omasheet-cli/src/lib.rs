// Copyright (c) 2026 Stephen Roe

//! The `omasheet` command: its commands, and what it is given to open in
//! the app when there is none.

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
    about = "A text-native spreadsheet: open a file in the app, or print, evaluate, check, import, export and render .omx files",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// An .omx file, XLSX Workbook, CSV file or Parquet file to open in the
    /// app; a new file if none is given
    file: Option<PathBuf>,
}

#[derive(Args)]
struct Env {
    /// Show dates and times the way this machine's locale writes them,
    /// rather than as the file does (2025-01-31, 17:05), and date-times on
    /// this machine's clocks rather than those of the file's time zone
    #[arg(long)]
    locale: bool,

    /// What `today()` and `now()` give, in place of the clock
    #[arg(long, value_name = "DATETIME")]
    now: Option<String>,

    /// The time zone of this machine, in place of the one it is set to; it
    /// is also the zone of a file that names none
    #[arg(long, value_name = "ZONE")]
    zone: Option<String>,

    /// Show every floating-point number in full (1.4142135623730951),
    /// rather than rounded to five digits after the decimal point
    /// (1.41421…); exact numbers are always in full (19.99, 1/3)
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
    /// Calculate a file and print its sheets
    Print {
        /// The file to print
        file: PathBuf,

        #[command(flatten)]
        env: Env,
    },
    /// Evaluate one OMX expression and print its value
    ///
    /// The expression is read from standard input when it is not given.
    Eval {
        /// A file whose sheets and constants the expression may use
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,

        /// [FILE] EXPRESSION
        #[arg(value_name = "ARGS", num_args = 0..=2)]
        args: Vec<String>,

        #[command(flatten)]
        env: Env,
    },
    /// Check a file without evaluating it
    Lint {
        /// The file to check
        file: PathBuf,
    },
    /// Write an XLSX Workbook, a CSV file, JSON or Parquet as an .omx file
    ///
    /// The file is printed, and anything that was changed or left behind
    /// is reported.
    Import {
        /// The XLSX Workbook, CSV, JSON or Parquet file to read
        file: PathBuf,

        /// Write the file to this file rather than printing it
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Write the calculated sheets of a file as an XLSX Workbook, CSV, JSON
    /// or Parquet
    ///
    /// The file is written beside the file: budget.omx as budget.xlsx,
    /// budget.csv, budget.json or budget.parquet. CSV and Parquet hold one
    /// sheet, so a file with several is written as budget.Sales.csv,
    /// budget.Summary.csv.
    Export {
        /// The file to export
        file: PathBuf,

        /// Write an XLSX Workbook, with a worksheet for each sheet
        #[arg(long, conflicts_with_all = ["csv", "json", "parquet"])]
        xlsx: bool,

        /// Write CSV, a file for each sheet
        #[arg(long, conflicts_with_all = ["json", "parquet"])]
        csv: bool,

        /// Write JSON: the values of each sheet as a list of rows by
        /// column name, and of a sheet given with --sheet as that list alone
        #[arg(long, conflicts_with = "parquet")]
        json: bool,

        /// Write Parquet, a file for each sheet, with each column as the
        /// Parquet type its own type names
        #[arg(long)]
        parquet: bool,

        /// Export this sheet alone
        #[arg(long, value_name = "NAME")]
        sheet: Option<String>,

        /// Write to this file rather than beside the file
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        #[command(flatten)]
        env: Env,
    },
    /// Render a Markdown document with its files calculated
    ///
    /// A fenced `omx` block is shown as its sheets, and {{ expression }} as
    /// its value. `files: [budget.omx]` in the front matter names files
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
    if !matches!(kind.as_str(), "xlsx" | "csv" | "json" | "parquet") {
        return usage(&format!(
            "cannot import {}: give an XLSX Workbook (.xlsx), a CSV file (.csv), JSON (.json) or Parquet (.parquet)",
            file.display()
        ));
    }
    let bytes = std::fs::read(file).map_err(|e| {
        eprintln!("omasheet: cannot read {}: {e}", file.display());
        ExitCode::from(2)
    })?;
    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
    let imported = match kind.as_str() {
        "xlsx" => omasheet_interop::xlsx::import(&bytes),
        "json" => omasheet_interop::json::import(&stem, &bytes),
        "parquet" => omasheet_interop::parquet::import(&stem, &bytes),
        _ => omasheet_interop::csv::import(&stem, &bytes),
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
    ext: &str,
    sheet: Option<&str>,
    output: Option<&Path>,
    options: Options,
) -> Result<ExitCode, ExitCode> {
    let text = read(file)?;
    let name = file.to_string_lossy();
    let exported = match ext {
        "xlsx" => omasheet_interop::xlsx::export(&name, &text, sheet, options),
        "json" => omasheet_interop::json::export(&name, &text, sheet, options),
        "parquet" => omasheet_interop::parquet::export(&name, &text, sheet, options),
        _ => omasheet_interop::csv::export(&name, &text, sheet, options),
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
    if output.is_some() && exported.files.len() > 1 {
        return usage("the file has several sheets: give one with --sheet to write it to --output");
    }
    for out in &exported.files {
        let path = match (output, &out.sheet) {
            (Some(path), _) => path.to_path_buf(),
            (None, Some(sheet)) => file.with_extension(format!("{sheet}.{ext}")),
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
    // A file is named from where the document is.
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

fn run(cli: Cli, open: impl FnOnce(Option<&Path>) -> ExitCode) -> Result<ExitCode, ExitCode> {
    match cli.command {
        Some(Command::Lint { file }) => {
            let text = read(&file)?;
            Ok(finish(omasheet_engine::lint(
                &file.to_string_lossy(),
                &text,
            )))
        }
        Some(Command::Eval {
            file,
            mut args,
            env,
        }) => {
            let options = env.options()?;
            // `eval FILE EXPR` is the same as `eval --file FILE EXPR`.
            let file = match (file, args.len()) {
                (None, 2) => Some(PathBuf::from(args.remove(0))),
                (Some(_), 2) => {
                    eprintln!(
                        "omasheet: give the file once, either as --file or before the expression"
                    );
                    return Err(ExitCode::from(2));
                }
                (file, _) => file,
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
            let loaded = match &file {
                Some(path) => Some((path.to_string_lossy().into_owned(), read(path)?)),
                None => None,
            };
            let file_ref = loaded.as_ref().map(|(n, t)| (n.as_str(), t.as_str()));
            Ok(finish(omasheet_engine::eval_with(
                file_ref, expr_name, &expr, options,
            )))
        }
        Some(Command::Import { file, output }) => import(&file, output.as_deref()),
        Some(Command::Export {
            file,
            xlsx,
            csv,
            json,
            parquet,
            sheet,
            output,
            env,
        }) => {
            let ext = match (xlsx, csv, json, parquet) {
                (true, ..) => "xlsx",
                (_, true, ..) => "csv",
                (_, _, true, _) => "json",
                (.., true) => "parquet",
                _ => {
                    return usage("say what to export as: --xlsx, --csv, --json or --parquet");
                }
            };
            let options = env.options()?;
            export(&file, ext, sheet.as_deref(), output.as_deref(), options)
        }
        Some(Command::Render {
            file,
            html,
            output,
            env,
        }) => render(&file, html, output.as_deref(), env.options()?),
        None => Ok(open(cli.file.as_deref())),
        Some(Command::Print { file, env }) => {
            let options = env.options()?;
            let text = read(&file)?;
            Ok(finish(omasheet_engine::view_with(
                &file.to_string_lossy(),
                &text,
                options,
            )))
        }
    }
}

/// Run the command line this program was started with. With no command,
/// `open` is called to open the app, on the file given if there was one.
pub fn main(open: impl FnOnce(Option<&Path>) -> ExitCode) -> ExitCode {
    match run(Cli::parse(), open) {
        Ok(code) | Err(code) => code,
    }
}

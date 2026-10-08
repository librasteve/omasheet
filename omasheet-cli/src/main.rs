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
    about = "A text-native spreadsheet: view, evaluate and check .omx sheets",
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
}

impl Env {
    fn options(&self) -> Result<Options, ExitCode> {
        let mut options = Options::default();
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

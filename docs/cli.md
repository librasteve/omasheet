# Command-line cheat sheet

Everything the app does is available on the command line. `omasheet` with no command, or with only a file, opens the app; every other command prints or writes and returns.

## Commands

| Command | Does |
|---|---|
| `omasheet` | the app, on a new file |
| `omasheet file.omx` | the app, on a `.omx`, `.csv`, `.json`, `.xlsx` or `.parquet` file |
| `omasheet print file.omx` | calculate a file and print its sheets |
| `omasheet eval 'expr'` | work out one expression |
| `omasheet eval file.omx 'expr'` | work out an expression against a file's sheets and constants |
| `omasheet lint file.omx` | check a file without calculating it |
| `omasheet import file.xlsx` | write an XLSX Workbook, CSV, JSON or Parquet file as an `.omx` |
| `omasheet export file.omx --xlsx` | write the calculated sheets as XLSX, `--csv`, `--json` or `--parquet` |
| `omasheet render report.md` | render a Markdown document with its files calculated |
| `omasheet rates` | say what date the exchange rates are as of; `omasheet rates update` fetches the latest |

## Examples

```sh
omasheet print examples/budget.omx
omasheet eval '1/3 + 1/6'                       # 1/2, exactly
omasheet eval examples/budget.omx 'Sales.Revenue.sum()'
omasheet lint examples/budget.omx
omasheet export examples/budget.omx --xlsx      # or --csv | --json | --parquet
omasheet render examples/report.md --html -o report.html
```

## Options

These work on `print`, `eval`, `import`, `export` and `render`:

| Option | Does |
|---|---|
| `--locale` | show dates and times as this machine's locale writes them, and date-times on this machine's clocks |
| `--now 2025-01-31T09:30` | fix what `today()` and `now()` give |
| `--seed 7` | fix what `rand`, `roll` and `pick` give, so that a run can be repeated |
| `--zone Europe/London` | stand in for this machine's time zone, which is also the zone of a file that names none |
| `--exact` | show every number in full: `18446744073709551616`, not `1.8446744073709551e+19·`, and `125/3%`, not `41.67%` |

## Command by command

- **`eval`** reads the expression from standard input when none is given. `omasheet eval file.omx 'expr'` is the same as `omasheet eval --file file.omx 'expr'`.
- **`export`** writes beside the file: `budget.omx` as `budget.xlsx`, `budget.csv`, `budget.json` or `budget.parquet`. CSV and Parquet hold one sheet, so a file with several is written as `budget.Sales.csv`, and so on. `--sheet NAME` exports one sheet, and `-o FILE` writes it elsewhere. With `--parquet`, `--meta` keeps the file's formulas, constants and functions in the Parquet's metadata so that importing it gives them back, and `--decimal` writes a `Fraction` column as the nearest `Float64`.
- **`import`** prints the file it makes, and reports anything that was changed or left behind. `-o FILE` writes it instead.
- **`render`** prints Markdown, or an HTML page with `--html`. `-o FILE` writes it.
- **`lint`** reports every mistake at once, with its line and, usually, a `help:` line that says what to write.

From the source tree, `omasheet` is `cargo run --`.

## Where next

[The app and the command line](app.md#the-command-line) has the same commands in the guide, [Interchange](interop.md) says how each file format maps, and [Reports on every build](reports-in-builds.md) puts `render` in a script.

# cli

The `import`, `export` and `render` subcommands.

## ADDED Requirements

### Requirement: Import subcommand
`omasheet import <file>` SHALL produce `.omx` source from an XLSX workbook or a
CSV file, chosen by the file's extension, as specified in `xlsx-interop` and
`csv-interop`. The source SHALL be written to standard output, or to the file
given with `-o`; notices SHALL be written to standard error.

#### Scenario: Import a workbook
- **WHEN** `omasheet import budget.xlsx -o budget.omx` is run
- **THEN** `budget.omx` is written with one table per worksheet

#### Scenario: Import a CSV file
- **WHEN** `omasheet import sales.csv` is run
- **THEN** `.omx` source with the table `sales` is printed

#### Scenario: Unknown kind of file
- **WHEN** `omasheet import notes.txt` is run
- **THEN** the command fails, naming the kinds of file it can import

### Requirement: Export subcommand
`omasheet export <file.omx>` SHALL take exactly one of `--xlsx` and `--csv` and
write the evaluated sheet in that format beside the source, or to the path given
with `-o`. `--table <Name>` SHALL export that table alone. A sheet that does not
compile SHALL NOT be exported.

#### Scenario: Export to XLSX
- **WHEN** `omasheet export budget.omx --xlsx` is run
- **THEN** `budget.xlsx` is written

#### Scenario: Export one table to CSV
- **WHEN** `omasheet export budget.omx --csv --table Sales` is run
- **THEN** `budget.csv` is written, holding the `Sales` table

#### Scenario: No format
- **WHEN** `omasheet export budget.omx` is run
- **THEN** the command fails, asking for `--xlsx` or `--csv`

### Requirement: Render subcommand
`omasheet render <file.md>` SHALL render a Markdown document with its Omasheet
content evaluated, as specified in `markdown-integration`: as Markdown, or as
HTML with `--html`, to standard output or to the file given with `-o`. If
anything in the document fails, the command SHALL fail and write nothing.

#### Scenario: Render a report
- **WHEN** `omasheet render report.md` is run
- **THEN** the rendered document is written with every interpolation replaced by its value

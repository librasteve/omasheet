# cli

The `omasheet` command.

## ADDED Requirements

### Requirement: Single binary
Omasheet SHALL be distributed as one self-contained executable named `omasheet`
that needs no language runtime to be installed.

#### Scenario: Runs on a clean system
- **GIVEN** an Omarchy system with only the `omasheet` binary installed
- **WHEN** `omasheet --version` is run
- **THEN** the version is printed and the exit status is zero

### Requirement: View a sheet
`omasheet <file.sheet>` SHALL evaluate the sheet and present its tables with
computed values in a read-only view. It SHALL NOT modify the file.

#### Scenario: Open a sheet
- **GIVEN** `budget.sheet` with a computed column `Profit := Revenue - Cost`
- **WHEN** `omasheet budget.sheet` is run
- **THEN** each table is shown with `Profit` values calculated and formatted
- **AND** `budget.sheet` is unchanged on disk

### Requirement: Evaluate an expression
`omasheet eval '<expr>'` SHALL evaluate one OMX expression and print its value.
With `--sheet <file>` (or a file argument) the expression SHALL be able to refer
to that sheet's tables and constants. The expression SHALL also be readable from
standard input.

#### Scenario: Pure arithmetic
- **WHEN** `omasheet eval '1/3 + 1/6'` is run
- **THEN** the output is `1/2`

#### Scenario: Against a sheet
- **WHEN** `omasheet eval --sheet sales.sheet 'Sales[Region == "UK"; Revenue].sum()'` is run
- **THEN** the total UK revenue is printed

#### Scenario: From standard input
- **WHEN** `echo 'Sales.Revenue.avg()' | omasheet eval --sheet sales.sheet` is run
- **THEN** the average is printed

### Requirement: Lint a sheet
`omasheet lint <file.sheet>` SHALL parse and statically check a sheet without
executing it, print every diagnostic, and exit non-zero if any error was found.

#### Scenario: Clean sheet
- **WHEN** `omasheet lint` is run on a valid sheet
- **THEN** nothing is reported and the exit status is zero

#### Scenario: Unit error
- **WHEN** `omasheet lint` is run on a sheet containing `Revenue + Weight` with incompatible units
- **THEN** the error is printed and the exit status is non-zero

### Requirement: Diagnostics
Every diagnostic SHALL include the file name, line and column, a message, and the
offending source excerpt.

#### Scenario: Located error
- **WHEN** a type error occurs in a cell on line 14
- **THEN** the message begins with `<file>:14:<col>` and shows line 14 with the faulty span marked

### Requirement: Import and export subcommands
`omasheet import <file.xlsx>` SHALL produce `.sheet` source from a workbook, and
`omasheet export <file.sheet> --xlsx` SHALL produce an XLSX workbook, as specified
in `xlsx-interop`.

#### Scenario: Export
- **WHEN** `omasheet export budget.sheet --xlsx` is run
- **THEN** `budget.xlsx` is written

### Requirement: Render subcommand
`omasheet render <file.md>` SHALL render a Markdown document with its Omasheet
content evaluated, as specified in `markdown-integration`.

#### Scenario: Render a report
- **WHEN** `omasheet render report.md` is run
- **THEN** the rendered document is written with every interpolation replaced by its value

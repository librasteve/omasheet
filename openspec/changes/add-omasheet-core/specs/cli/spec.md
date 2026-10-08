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
`omasheet <file.omx>` SHALL evaluate the sheet and present its tables with
computed values in a read-only view. It SHALL NOT modify the file.

#### Scenario: Open a sheet
- **GIVEN** `budget.omx` with a computed column `Profit := Revenue - Cost`
- **WHEN** `omasheet budget.omx` is run
- **THEN** each table is shown with `Profit` values calculated and formatted
- **AND** `budget.omx` is unchanged on disk

### Requirement: Evaluate an expression
`omasheet eval '<expr>'` SHALL evaluate one OMX expression and print its value.
With `--sheet <file>` (or a file argument) the expression SHALL be able to refer
to that sheet's tables and constants. The expression SHALL also be readable from
standard input.

#### Scenario: Pure arithmetic
- **WHEN** `omasheet eval '1/3 + 1/3'` is run
- **THEN** the output is `2/3`

#### Scenario: Against a sheet
- **WHEN** `omasheet eval --sheet sales.omx 'Sales[Region == "UK"; Revenue].sum()'` is run
- **THEN** the total UK revenue is printed

#### Scenario: From standard input
- **WHEN** `echo 'Sales.Revenue.avg()' | omasheet eval --sheet sales.omx` is run
- **THEN** the average is printed

### Requirement: Dates, times and the clock
Dates and times SHALL be printed in their ISO forms unless `--locale` is given,
in which case they SHALL be printed the way the locale of the machine
(`LC_TIME`) writes them, with a four-digit year. `--now <datetime>` SHALL set
what `today()` and `now()` give, in place of the clock. `--zone <name>` SHALL set
the time zone of the machine, in place of the one it is set to. With `--locale`
the date-times of a sheet in another time zone SHALL be printed on the clocks of
the machine. These options SHALL apply to viewing a sheet and to `eval`.

#### Scenario: ISO by default
- **GIVEN** a machine whose locale is British
- **WHEN** `omasheet eval '2026-10-08 + 17:47'` is run
- **THEN** the output is `2026-10-08T17:47`

#### Scenario: Locale on request
- **GIVEN** a machine whose locale is British
- **WHEN** `omasheet eval --locale '2026-10-08 + 17:47'` is run
- **THEN** the output is `08/10/2026 17:47`

#### Scenario: A fixed clock
- **WHEN** `omasheet eval --now 2026-10-08T17:47 'today()'` is run
- **THEN** the output is `2026-10-08`

#### Scenario: A given zone
- **WHEN** `omasheet eval --zone America/New_York '2025-07-15T12:00.utc()'` is run
- **THEN** the output is `2025-07-15T16:00+00:00`

### Requirement: Lint a sheet
`omasheet lint <file.omx>` SHALL parse and statically check a sheet without
executing it, print every diagnostic, and exit non-zero if any error was found.

#### Scenario: Clean sheet
- **WHEN** `omasheet lint` is run on a valid sheet
- **THEN** nothing is reported and the exit status is zero

#### Scenario: Type error
- **WHEN** `omasheet lint` is run on a sheet containing `Revenue + Month` where `Month` is `Text`
- **THEN** the error is printed and the exit status is non-zero

### Requirement: Diagnostics
Every diagnostic SHALL include the file name, line and column, a message, and the
offending source excerpt.

#### Scenario: Located error
- **WHEN** a type error occurs in a cell on line 14
- **THEN** the message begins with `<file>:14:<col>` and shows line 14 with the faulty span marked

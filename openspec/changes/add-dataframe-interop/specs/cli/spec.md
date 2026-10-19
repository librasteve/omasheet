# cli

Parquet in the `import` and `export` subcommands.

## ADDED Requirements

### Requirement: Parquet in import and export

`omasheet import <file.parquet>` SHALL produce `.omx` source from a Parquet
file, and `omasheet export <file.omx> --parquet` SHALL write Parquet, as
specified in `parquet-interop`. `--parquet` SHALL be one of the formats of
which `export` takes exactly one, and `--sheet` and `-o` SHALL apply as they do
to the others.

Parquet SHALL be available in every build. There SHALL NOT be a Cargo feature
that leaves it out (D26).

#### Scenario: Import a Parquet file

- **WHEN** `omasheet import sales.parquet` is run
- **THEN** `.omx` source with the sheet `sales` and a type line for each column is printed
- **AND** each type comes from the file's schema

#### Scenario: Export to Parquet

- **WHEN** `omasheet export budget.omx --parquet --sheet Sales` is run
- **THEN** `budget.parquet` is written, holding the `Sales` sheet

#### Scenario: The format is named in the messages

- **WHEN** `omasheet export budget.omx` is run with no format
- **THEN** the message lists `--parquet` among the formats it takes

### Requirement: `--meta`

`omasheet export <file.omx> --parquet --meta` SHALL additionally write what no
format round-trips today — the file's zone, each column's declared type, the
computed columns' formulas, the constants and the functions — into the Parquet
file's key-value metadata.

On import, such metadata SHALL be used in place of reading types from the
schema alone, so that a file exported with `--meta` and imported again holds
its formulas rather than their results.

A reader that ignores the metadata SHALL still see a usable Parquet file.

#### Scenario: Formulas survive a round trip

- **GIVEN** a file with a computed column
- **WHEN** it is exported with `--parquet --meta` and imported again
- **THEN** the computed column is a formula again, not its values

#### Scenario: Plain Parquet is still plain

- **GIVEN** a file exported with `--parquet --meta`
- **WHEN** it is read by a tool that knows nothing of the metadata
- **THEN** every column reads as its ordinary Parquet type

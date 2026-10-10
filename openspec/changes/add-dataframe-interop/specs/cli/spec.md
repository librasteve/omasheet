# cli

Parquet in the `import` and `export` subcommands.

## ADDED Requirements

### Requirement: Parquet in import and export
`omasheet import <file.parquet>` SHALL produce `.omx` source from a Parquet
file, and `omasheet export <file.omx> --parquet` SHALL write Parquet, as
specified in `parquet-interop`. `--parquet` SHALL be one of the formats of which
`export` takes exactly one, and `--table` and `-o` SHALL apply as they do to
the others.

#### Scenario: Import a Parquet file
- **WHEN** `omasheet import sales.parquet` is run
- **THEN** `.omx` source with the table `sales` and a type line for each column is printed

#### Scenario: Export to Parquet
- **WHEN** `omasheet export budget.omx --parquet --table Sales` is run
- **THEN** `budget.parquet` is written, holding the `Sales` table

#### Scenario: Built without Parquet
- **WHEN** either command is run in a build without the Parquet feature
- **THEN** the command fails, saying this build has no Parquet support

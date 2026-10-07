# cli

The `import`, `export` and `render` subcommands.

## ADDED Requirements

### Requirement: Import and export subcommands
`omasheet import <file.xlsx>` SHALL produce `.omx` source from a workbook, and
`omasheet export <file.omx> --xlsx` SHALL produce an XLSX workbook, as specified
in `xlsx-interop`.

#### Scenario: Export
- **WHEN** `omasheet export budget.omx --xlsx` is run
- **THEN** `budget.xlsx` is written

### Requirement: Render subcommand
`omasheet render <file.md>` SHALL render a Markdown document with its Omasheet
content evaluated, as specified in `markdown-integration`.

#### Scenario: Render a report
- **WHEN** `omasheet render report.md` is run
- **THEN** the rendered document is written with every interpolation replaced by its value

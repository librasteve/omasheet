## ADDED Requirements

### Requirement: Plot declarations

A file MAY declare plots with `plot Name`, followed by `key: value` lines up to the next blank line. The controls are `type` (required: `line`, `bar`, `pie`, `candle`), `data` (required: an expression that gives a sheet), `x`, `y`, `title`, `width`, `height` and `k`. A plot's name SHALL NOT be that of a sheet, a constant or another plot.

#### Scenario: A plot after a sheet
- **WHEN** a `plot Revenue` declaration follows the rows of a sheet
- **THEN** the sheet ends before it, and its lines are not rows, columns or types of the sheet

#### Scenario: A mistake is found by lint
- **WHEN** a plot has an unknown control, a `type` that is not a kind, a `data` that is not a sheet, or an `x` or `y` that is not a column of it
- **THEN** `lint` reports it at that line, with the controls or columns that would do

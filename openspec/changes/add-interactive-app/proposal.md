# Proposal: Interactive app

## Why

Phase 1 gives a command line: edit the `.omx` file in a text editor, run
`omasheet` to see the result. That suits a text-first tool but not how people
expect to use a spreadsheet. The owner asked for an interactive grid in the
style of Omawrite: select cells, type into them, copy and paste, open and save.

This replaces the provisional decision that the first app would be a read-only
viewer (P15 in `add-omasheet-core`).

## What Changes

- **app** — ADDED: a desktop application showing each table as an editable grid,
  with tabs for tables and constants, an entry box for the current cell,
  block selection, copy / cut / paste, undo, open / save, and live recalculation

## Impact

- Depends on `add-omasheet-core`
- New crate `omasheet-ui`: Qt Quick (QML) window driven from Rust through
  `cxx-qt`. Needs Qt 6 (`qt6-base`, `qt6-declarative`) to build and run
- `omasheet-engine` gains `Document`, an editable sheet that rewrites the `.omx`
  text for each edit; `omasheet-omx` records line positions to support it
- The `.omx` text remains the only saved form; the app adds no file format

## Out of Scope

- Renaming or deleting columns, tables and constants; reordering rows
- Changing a column's declared type from the app
- Find and replace, printing, charts, formatting
- Packaging (desktop entry, icon, Arch package)

# Proposal: Units (Phase 4)

## Why

A spreadsheet number has no meaning attached, so `Revenue + Weight` is legal and
a mixed-currency sum is silently wrong. Making units part of a value's type,
declared per column and checked before execution, catches that whole class of
error.

## What Changes

- **units** — ADDED: unit literals, dimensional analysis, currencies and `%` as
  units, column-level units, unit inference for computed columns, user-defined
  units, display units
- **file-format** — MODIFIED: a column type may be a unit
- **evaluation** — MODIFIED: static checking covers units; sheet operations
  preserve units
- **cli** — MODIFIED: `omasheet lint` reports unit errors
- **xlsx-interop** — ADDED: lossy mapping of units on export

## Impact

- Depends on `add-omasheet-core`; the `evaluation` sheet-operations delta
  depends on `add-sheet-operations` and the `xlsx-interop` delta on `add-interop`
- Extends the type system in `omasheet-omx` and the value model in
  `omasheet-engine`; files written for earlier phases stay valid

## Out of Scope

- Uncertainty (`add-uncertainty`)
- Live exchange rates; `convert()` takes an explicit rate
- Formatting syntax for display units (design Q11)

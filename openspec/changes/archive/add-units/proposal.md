# Proposal: Units (Phase 4)

> **Superseded**, and nothing of it was built. The decision of 2026-11-11 replaces its literal grammar (`5 m/s`, `unit Day = 24h`), its exact `Fraction` magnitudes and its unit-as-column-type with `add-si-units`: strict SI cell text (`5kg`, `9.81m.s^-2`), `Scientific` magnitudes, and named quantity types as column types, from the `siquant` crate. Currencies and `%` as units are not carried over. What replaces it: `add-si-units`, then `add-data-prefixes`, `add-angles` and `add-durations-as-time`.

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

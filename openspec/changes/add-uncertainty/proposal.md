# Proposal: Uncertainty (Phase 5)

## Why

Measured values carry an uncertainty, and spreadsheets force users to track it
in a second column and propagate it by hand. Treating uncertainty as part of the
value lets it propagate automatically through arithmetic and aggregation.

## What Changes

- **uncertainty** — ADDED: `±` values, automatic propagation, aggregation,
  `Uncertain<…>` columns, display
- **sheet-format** — MODIFIED: a column type may be `Uncertain<…>`
- **evaluation** — MODIFIED: table operations preserve uncertainties
- **xlsx-interop** — ADDED: lossy mapping of uncertainty on export

## Impact

- Depends on `add-units` (uncertain quantities share the unit of their value);
  the `evaluation` delta depends on `add-table-operations` and the
  `xlsx-interop` delta on `add-interop`
- Extends the value model in `omasheet-engine`

## Out of Scope

- Intervals (`10m .. 12m`) — design Q7
- Probabilistic values (`Normal(...)`, `Uniform(...)`)
- Correlated uncertainties; operands are treated as independent

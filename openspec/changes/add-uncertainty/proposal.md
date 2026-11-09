# Proposal: Uncertainty

## Why

Measured values carry an uncertainty, and spreadsheets force users to track it in a second column and propagate it by hand. Treating uncertainty as part of the value lets it propagate automatically through arithmetic, the standard functions and aggregation, and lets values be written and read in the concise form that physics and chemistry tables use: `7.297 352 5643(11) × 10⁻³`.

## What Changes

- **uncertainty** — ADDED: the `Uncertain` value, the `±` and `(nn)` literals, first-order propagation through arithmetic, `**` and the math functions, `sum` and `avg`, the `Uncertain` column type, display
- **file-format** — MODIFIED: a column type may be `Uncertain`, with an optional display format
- **evaluation** — MODIFIED: sheet operations preserve uncertainties
- **xlsx-interop** — ADDED: lossy mapping of uncertainty on export

## Impact

- Extends the value model in `omasheet-engine` and the lexer, parser, type lattice and function table in `omasheet-omx`
- No longer depends on `add-units`. Uncertain values carry no unit in this change; a unit on an uncertain value is a later extension once units exist
- The `file-format` delta rewrites the column-schema requirement, so whichever of this change and `add-units` is archived second must merge the two type lists

## Out of Scope

- Units on uncertain values (`10 ± 0.1 m`)
- Uncertain complex numbers, now or later: an uncertain value degrades to a `Float` where an operation may give a `Complex`
- Intervals (`10m .. 12m`) and probability distributions (`Normal(...)`, `Uniform(...)`)
- Correlated uncertainties: operands are always independent, even when they are the same cell
- Parsing pasted CODATA text with space-grouped digits and `× 10⁻³` (convert it once, outside omasheet)

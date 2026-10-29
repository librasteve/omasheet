# Tasks: Units (Phase 4)

> **Deferred** until every pipe stage of `add-sheet-operations-polars` is done
> (owner, 2026-10-28; D89 there). Phase 4 is a delta on Phase 3 — units through group and join — so
> starting it first would mean writing those deltas against stages that do not
> exist yet.

## 0. Settle the open questions
- [ ] 0.1 Decide what `%` is: literal, unit, or both (design Q6)

## 1. Units (`units`)
- [ ] 1.1 Unit registry: SI base and common derived units, prefixes
- [ ] 1.2 Exponent-vector dimension algebra
- [ ] 1.3 Unit literals and conversion on addition
- [ ] 1.4 Currency units; `convert()`
- [ ] 1.5 Column unit declarations, inference for computed columns, cell checking
- [ ] 1.6 `unit` declarations
- [ ] 1.7 Display unit separate from stored value

## 2. Existing capabilities
- [ ] 2.1 Unit types in column schemas (`file-format`)
- [ ] 2.2 Unit checking in static analysis and `omasheet lint` (`evaluation`, `cli`)
- [ ] 2.3 Units preserved through group, join and aggregation (`evaluation`)
- [ ] 2.4 Unit mapping on XLSX export (`xlsx-interop`)

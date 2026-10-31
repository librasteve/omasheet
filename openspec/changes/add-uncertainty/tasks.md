# Tasks: Uncertainty (Phase 5)

> **Deferred** until every pipe stage of `add-sheet-operations-polars` is done
> (owner, 2026-10-28; D89 there). Phase 5 builds on Phase 4, which is itself deferred behind
> Phase 3, so this is two changes back in the queue.

## 0. Settle the open questions
- [ ] 0.1 Decide how uncertainty magnitudes are stored: `Num`, or exact variance (design Q5)

## 1. Uncertainty (`uncertainty`)
- [ ] 1.1 `±` / `+-` literal
- [ ] 1.2 Propagation for `+ - * /` and aggregations
- [ ] 1.3 `Uncertain<…>` column type
- [ ] 1.4 Display forms

## 2. Existing capabilities
- [ ] 2.1 `Uncertain<…>` in column schemas (`file-format`)
- [ ] 2.2 Uncertainty preserved through group, join and aggregation (`evaluation`)
- [ ] 2.3 Value and uncertainty columns on XLSX export (`xlsx-interop`)

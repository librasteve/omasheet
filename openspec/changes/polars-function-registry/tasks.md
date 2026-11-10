# Tasks: A Polars function registry

Deferred until `add-units` is archived.

## 0. Settle
- [ ] 0.1 Ratify the naming rules N1–N9, and decide N8 against the pinned Polars docs
- [ ] 0.2 Decide how `by` parses inside a call

## 1. The registry
- [ ] 1.1 The entry type and table in `omasheet-omx`; the N9 test
- [ ] 1.2 `check.rs` takes arguments and result type from the entry
- [ ] 1.3 `lower.rs` builds the Polars expression from the entry
- [ ] 1.4 The plain-sheet path: a one-column temporary frame, run, read back
- [ ] 1.5 Refusal of `Complex` and `Scientific` by name

## 2. The first functions
- [ ] 2.1 `cum_sum cum_min cum_max cum_prod cum_count` with the order key and `by`
- [ ] 2.2 `diff pct_change rank forward_fill backward_fill`
- [ ] 2.3 `rolling_sum rolling_mean rolling_min rolling_max rolling_std`
- [ ] 2.4 `n_unique clip is_between any all`
- [ ] 2.5 Cargo features, family by family

## 3. Checks
- [ ] 3.1 Table path and sheet path agree on every function and every dtype they take
- [ ] 3.2 Docs, skill, an example, `.expected` regenerated
- [ ] 3.3 fmt, clippy -D warnings, workspace tests

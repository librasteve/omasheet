# Proposal: `apply` on groups

**Forthcoming; not scheduled.** It follows `add-units` and sits beside `polars-function-registry`, whose naming rules (N1–N9) it takes an exception to (see D-A1).

## Why

`group_by(keys) |> agg(aggregates)` already gives a row per group, but an aggregate can only be a function of one column (`Revenue.sum()`), or an expression over columns that each reduce to one value. There is no way to hand a whole group to a function of your own, which is what a per-group calculation needs: a weighted average, a group's running balance, the top three of each region, a ranking within the group, a fit. Polars has it as `group_by(...).map_groups(fn)` (called `apply` before 0.19) and pandas as `groupby().apply(fn)`. `fn Name(sheet) = …` already lets a parameter be a whole sheet (`Worth(orders)`), so the pieces exist: the group is a sheet, and the function takes one.

The owner asked for `group_by` and `apply` (2026-11-10). `group_by` with `agg` is already built (`add-sheet-operations-polars`); what is new is `apply`, and `group_by` as a stage that may be followed by it.

## What Changes

- **omx-expressions**: ADDED. `apply(Fn)` as a stage after `group_by(keys)`: `Fn`, a function of your own taking one sheet, is called once for each group with that group's rows as a sheet (the source sheet's columns, key columns included, in the source's order).
- **omx-expressions**: ADDED. The result by what `Fn` gives:
  - a single value: one row per group, the key columns then a column named for the function (`Fn`, or `Name := apply(Fn)`);
  - a sheet: its rows for each group, the key columns prepended and repeated on each row (Polars' `map_groups`); a group for which it gives no rows contributes none;
  - a vector: one row per group with the vector in the column, as `agg` of a list does today.
- **omx-expressions**: ADDED. `apply(Fn, Other)` is not provided: a function of one parameter is the whole of it. Extra constants are read from the file as any function reads them.
- **omx-expressions**: ADDED. Groups are taken in the order of the keys, an empty key first, as `agg` does, so `apply` and `agg` agree on which row is which.
- The result is a sheet, so a stage may follow it (`sort`, `head`, `filter`, `select`), and `sheet Name := Sales |> group_by(Region) |> apply(Top3)` is a sheet calculated from an expression, read-only, as any other.

## Impact

- `omasheet-omx`: `ast.rs`/parser (`apply` as a stage), `check.rs` (the function's parameter and result types), `funcs.rs`
- `omasheet-engine`: `eval.rs` (running the function per group), `doc.rs` (view columns and types of the result)
- `omasheet-interop`: exports of such a sheet are as any calculated sheet (its values)
- `docs/expressions.md`, `docs/format.md`, `skill/omasheet/SKILL.md`, an example `examples/groups.omx`

## Out of Scope

- A function written inline in the call (OMX has no lambdas); only a named `fn`
- `apply` on a single column per element (Polars `map_elements`); `apply` over a whole sheet with no `group_by` (that is calling the function)
- Lowering to Polars: `map_groups` calls arbitrary code, so `apply` is run by the engine, one group at a time, and leaves the rest of the plan to Polars. A `Complex`/`Scientific` value in a group is fine, being the engine's own
- Parallel evaluation of groups

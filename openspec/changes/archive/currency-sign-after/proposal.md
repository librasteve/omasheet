# Proposal: A currency's sign after the amount

> **Built**, with one check by hand outstanding (see its last task); archived.

## Why

`Currency[JPY; $.0f]` could only put a sign before the amount, `¥144`. Some currencies and house styles write it after, `144¥`.

## What Changes

- **file-format**: ADDED. `$>` in a currency format puts the sign after the number, `Currency[JPY; $>.0f]` shows `144¥`, and `_` after it sets it apart by a space, `$>_`, `144 ¥`. A currency with no sign shows its code after the number as `$` does, spaced by `_`
- **interop**: an `.xlsx` export writes the same placement as the cell's number format, `#,##0"¥"`

## Impact

- `omasheet-omx/src/colfmt.rs` (`SciFormat.sign_after`), `omasheet-engine/src/quantity.rs` (`render_money`), `omasheet-interop/src/xlsx.rs`
- `docs/exact-money.md`, `docs/expressions.md`, `omasheet-engine/resources/format_help.txt`, the omasheet skill, `examples/currency.omx`

## Out of Scope

- A per-currency default placement

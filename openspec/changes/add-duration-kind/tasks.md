# Tasks: `Duration` is the kind of quantity for time

## 0. Settle
- [x] 0.1 `d` as a unit against the prefix `d` (deci): no clash, `dm` and `hPa` still read as prefix and unit, `d.` and `h.` are still scales, and `kmin` and `dd` are refused
- [ ] 0.2 What a result is named (`Duration ± Duration` and date differences are `Duration`; others a `Quantity` until declared)
- [x] 0.3 A `Duration` column holds a Polars `Duration(ns)` whatever its unit; the unit is display metadata. Parquet reads and writes it natively; CSV and xlsx read back as `Duration[s]`. A duration with an uncertainty stays on the engine path

## 1. siquant
- [x] 1.1 `Duration` in `quantity_types.yaml`, unit `s`, group SI Base; `min`, `h`, `d`, `wk` in `units.yaml`. The function `min` is also a unit now, which D237 keeps apart by the bracket: `5kg.min()` is the call

## 2. The type

Decision: a column of the `Duration` kind is stored as a Polars `Duration` (microseconds, as it already was) and keeps being lowered; other quantity kinds are unchanged (still engine-calculated). `Value::Duration` and `S::Duration` go as separate types; the literal forms stay as syntax (`Tok::Duration` and `Lit::Duration` become a quantity of the `Duration` kind).
- [x] 2.1 `S::Duration` stays as the Polars-backed storage of the kind, which takes the kind's formats and head unit; the lattice, the casts and the checks follow
- [x] 2.2 An ISO duration and a bare number read as a kind of quantity; years and months stay refused
- [x] 2.3 The signed colon form in the lexer and cell reader; `hh:mm:ss.x` with no sign stays a `Time`
- [x] 2.4 The sign is the operator after an operand; `09:00 + +01:30`

## 3. The engine
- [x] 3.1 Date, time and date-time differences are `Duration`s; the rules for sums, scales and products
- [x] 3.2 A bare `Duration` is `Duration[s]`; `Duration[unit]` as any kind; the formats `%H:%M:%S` and `iso`
- [x] 3.3 (`as_span`, `as_duration` and `from_duration` stay as the private helpers of the arithmetic, since a quantity of time beside a date is still read as a duration)  `.Quantity` and `.Duration` are removed with no compatibility; the examples that use them are adapted
- [x] 3.4 The `total_*` functions, filters, sorts and `cum_sum` of durations

## 4. Interop
- [x] 4.1 Parquet, xlsx and CSV, in and out

## 5. Checks
- [x] 5.1 Tests for the kind, the units, the three literals, the sign and the operator, the rules
- [x] 5.2 (the flight vignette is not in the repository, so `BurnTime : Duration[s; .0f]` is left to its owner) `examples/durations.omx` and any example or doc that uses `.Quantity` or `.Duration` (its `Airtime` and `Elapsed` columns change display, or take `[iso]`) and the flight vignette (`BurnTime : Duration[s; .0f]`); expected output regenerated
- [x] 5.3 `docs/format.md`, `docs/expressions.md`, `skill/omasheet/SKILL.md`, the format help and the popup suggestions

# Tasks: `Duration` is the kind of quantity for time

## 0. Settle
- [ ] 0.1 `d` as a unit against the prefix `d` (deci)
- [ ] 0.2 What a result is named (`Duration ± Duration` and date differences are `Duration`; others a `Quantity` until declared)
- [ ] 0.3 What a Parquet duration column and a CSV or xlsx one hold and read back as

## 1. siquant
- [ ] 1.1 `Duration` in `quantity_types.yaml`, unit `s`, group SI Base; `min`, `h`, `d`, `wk` in `units.yaml`

## 2. The type
- [ ] 2.1 `S::Duration` becomes the kind; the lattice, the casts and the checks follow
- [ ] 2.2 An ISO duration and a bare number read as a kind of quantity; years and months stay refused
- [ ] 2.3 The signed colon form in the lexer and cell reader; `hh:mm:ss.x` with no sign stays a `Time`
- [ ] 2.4 The sign is the operator after an operand; `09:00 + +01:30`

## 3. The engine
- [ ] 3.1 Date, time and date-time differences are `Duration`s; the rules for sums, scales and products
- [ ] 3.2 A bare `Duration` is `Duration[s]`; `Duration[unit]` as any kind; the formats `%H:%M:%S` and `iso`
- [ ] 3.3 `as_span`, `as_duration` and `from_duration` go, and `.Quantity` and `.Duration` are removed with no compatibility; the examples that use them are adapted
- [ ] 3.4 The `total_*` functions, filters, sorts and `cum_sum` of durations

## 4. Interop
- [ ] 4.1 Parquet, xlsx and CSV, in and out

## 5. Checks
- [ ] 5.1 Tests for the kind, the units, the three literals, the sign and the operator, the rules
- [ ] 5.2 `examples/durations.omx` and any example or doc that uses `.Quantity` or `.Duration` (its `Airtime` and `Elapsed` columns change display, or take `[iso]`) and the flight vignette (`BurnTime : Duration[s; .0f]`); expected output regenerated
- [ ] 5.3 `docs/format.md`, `docs/expressions.md`, `skill/omasheet/SKILL.md`, the format help and the popup suggestions

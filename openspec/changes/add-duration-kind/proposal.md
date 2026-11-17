# Proposal: `Duration` is the kind of quantity for time

## Why

Every other dimension has a named kind of `Quantity`, `Mass[kg]`, `Length[m]`, `Speed[m.s^-1]`, but time has none: a time in seconds is a `Quantity[s]`, a different thing from `Duration`, which is a separate type held in whole microseconds with no uncertainty and no unit. Two types for one idea needs special rules, a quantity of time that becomes a `Duration` beside a date, a `Duration` that is read as seconds beside a unit, and `.Quantity` and `.Duration` to cross between them. `quantity_types.yaml` says "types for time arrive with the stage that adds it"; this is that stage.

## What Changes

- **file-format**: MODIFIED. `Duration` is a named kind of `Quantity`, of the dimension of time, with the preferred unit `s`: `Duration[s]`, `Duration[ms]`, `Duration[h]`. It takes every rule of a kind: the unit it is held and shown in, a bare number read in it, the unit in the head, `v` for the unit in each row, an uncertainty. `S::Duration` goes.
- **file-format**: ADDED. The units `min`, `h`, `d` and `wk`, as the non-SI units are (open: `d` against the prefix).
- **file-format**: MODIFIED. A duration is written as an ISO 8601 duration, `PT1H30M`, as a quantity of time, `139s`, `2.3min`, or in the signed colon form `+hh:mm:ss.x`: `+11:30` is 11 hours 30 minutes, `-0:00:30.5` is minus half a minute, `+128:04:15` is 128 hours. The sign is what makes it a `Duration`: `hh:mm:ss.x` with none is a `Time`, as it is now.
- **file-format**: ADDED. A `Duration` format in the style of a time format, `Duration[%H:%M:%S]`, shows the signed colon form with the hours not capped at 24, and reads back as itself.
- **file-format**: MODIFIED. A bare `Duration` column is `Duration[s]`, as a bare `Mass` is `Mass[kg]`: its cells are numbers of seconds and its head says `Duration[s]`. ISO 8601 is a format, `Duration[iso]`, which shows `P1DT10H5M30S`, so the old display is one word away.
- **omx-expressions**: MODIFIED. A date, a time or a date-time minus its own kind is a `Duration`. A `Duration` plus or minus a `Duration` is a `Duration`, times or over a number a `Duration`, and over or times a quantity of something else a quantity of what the product measures. A date, time or date-time plus or minus a `Duration` is as it was, whole days with a date.
- **omx-expressions**: MODIFIED. In a formula the sign is the operator where an operand has gone before it, so a signed duration after an operator takes two signs, `09:00 + +01:30`.
- **omx-expressions**: REMOVED. `.Quantity` and `.Duration`, which crossed between the two types; a duration is a quantity of time, so there is nothing to cross. A file that uses them is an error, and the examples are changed.
- **Breaking**: a `Duration` column that showed `PT1H30M` now shows `5400` under `Duration[s]`; the file and its stored values read as they did, and `Duration[iso]` shows the old form. A formula that used `.Quantity` or `.Duration` is an error; there is no compatibility for files written before, and the examples are adapted.

## Impact

- `vendor/siquant`: `quantity_types.yaml` (`Duration`, dimension `{Time: 1}`, unit `s`, group SI Base), `units.yaml` (`min`, `h`, `d`, `wk`)
- `omasheet-omx`: `types.rs` (`S::Duration` becomes `Quantity(Duration)`), `check.rs` (the rules for dates and quantities), the lexer and `convert.rs` (the signed colon form, and the sign against the operator)
- `omasheet-engine`: `value.rs`, `quantity.rs` (`as_span`, `as_duration`, `from_duration` fall away, and the `.Quantity` and `.Duration` conversions), `eval.rs`, the `total_*` functions
- `omasheet-interop`: Parquet, xlsx and CSV, where a `Duration` column is a Polars duration and, in the file, an ISO text
- `docs/format.md`, `docs/expressions.md`, `skill/omasheet/SKILL.md`, `examples/durations.omx` and its expected output, `examples/showcase/flight15.omx`

## Out of Scope

- Years and months, which are not a fixed length and stay refused
- The kind of every result: only a date difference and the sum, difference and scaling of durations are named `Duration`; any other result is a plain `Quantity` until a column is declared
- Nanosecond durations, which a float does not keep

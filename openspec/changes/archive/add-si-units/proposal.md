# Proposal: SI units (Phase 4, stage 1)

## Why

A spreadsheet number has no meaning attached, so `Revenue + Weight` is legal and `5kg + 300g` has to be converted by hand. Making a unit part of a value, with the dimension declared per column, lets units convert and propagate through arithmetic, and lets a mismatch be reported as an error instead of a wrong number. The units come from the standalone `siquant` crate (a second-generation, much simpler Rust successor to the Raku `Physics::Unit` and `Physics::Measure`), which also carries the uncertainty that `add-uncertainty` already gave `Scientific`.

This replaces `add-units`, whose literal grammar (`5 m/s`, `unit Day = 24h`), exact `Fraction` magnitudes and unit-as-column-type predate the design below. Currencies and `%` are out of scope.

## What Changes

- **units** — ADDED: the `Quantity` value (a `Scientific` magnitude and an SI unit), the cell and literal grammar `5kg`, `10.5(3)e-3kg`, `9.81m.s^-2`, `20°C`; SI prefixes; the scale literal `1k.`; conversion with `.to('g')`; arithmetic, comparison and aggregation; named quantity types (`Length`, `Mass`, `Force`, ...) as column types
- **file-format** — MODIFIED: a column type may be `Quantity` or a named quantity type, with an optional display unit in the format bracket (`Weight : Mass[g]`)
- **xlsx-interop** — ADDED: a quantity is exported as text, the unit not a number format

## Impact

- Adds the `siquant` dependency to `omasheet-omx` and `omasheet-engine`; extends the lexer, parser, type lattice (`S::Quantity`) and function table in `omasheet-omx` and the value model in `omasheet-engine`
- Builds on `add-uncertainty`: the magnitude of a quantity is a `Scientific`, so `10.5(3)e-3kg` carries its uncertainty and arithmetic propagates it
- Stages 2 to 4 are separate changes: `add-data-prefixes`, `add-angles`, `add-durations-as-time`

## Out of Scope

- Non-SI units in cell text (`ft`, `L`, `min`); they are chosen as a column display unit later
- Spaces, `/`, brackets and superscripts in a unit: only `kg.m^2.s^-2`
- Currencies, `%` as a unit, user-defined units (`unit Day = 24h`)
- Time: the `Duration` bridge is Stage 4; a `Quantity` beside a `Duration` is an error until then
- `std` and `var` of quantities

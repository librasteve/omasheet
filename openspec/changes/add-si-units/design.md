# Design: SI units

## Context

Raised by the owner (2026-11-11) with the `siquant` crate (`librasteve/siquant`): strict SI cell text, `Scientific` magnitudes, named quantity types, and the trailing-dot scale. The goal is that every siquant stage is seen working in omasheet. D/P/Q numbers continue from `add-uncertainty` and the later changes (the highest used before this change is D227).

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D228 | Decided | Units come from the `siquant` crate, a dependency of `omasheet-omx` and `omasheet-engine` | omx needs it to read a unit at lex time with the literal's span. A path dependency is used while the work is on `dev`; it is switched to a crates.io or git version before anything is merged to `main` |
| D229 | Decided | Cell and literal grammar is a `Scientific` literal immediately followed by an SI unit: `5kg`, `300cm`, `10.5(3)e-3kg`, `9.81m.s^-2`, `20°C`. Compounds are `.`-separated terms with `^` and a signed integer exponent. No space, no `/`, no brackets, no superscripts, no names or plurals | Supersedes P11 of `add-units`. `5 kg` is two tokens and an error |
| D230 | Decided | The magnitude of a quantity is an `f64` with an `f64` uncertainty, as `Scientific`; there is no exact `Decimal` path | `5m + 300cm` is `8m` in `f64` anyway. Settles Q8 of `add-units` the other way |
| D231 | Decided | A prefix and a trailing `.` is a scale literal that reads as a number: `1k.` is 1000, `1.5k.` is the exact decimal 1500, `5m.` is 0.005 | The dot makes it a prefix and not the metre or the tesla, so `m.` and `T.` do not clash. It is not a `Quantity`, so decimal arithmetic stays exact and the shown number echoes what was read. A stray `5m.` is read as a scale, a known trap |
| D232 | Decided | One generic type `Quantity`, and named types (`Length`, `Mass`, `Force`, ...) that are refinements of it by dimension; a column type is the point of definition. `S::Quantity(QId)` holds a small `Copy` id into siquant's type list, `QId(0)` being the generic `Quantity` | Names are resolved through the registry, so every string-keyed path (`set_column_types`, the snapshot, the menu) keeps working. Several names share a dimension (`Energy`/`Torque`); nothing is inferred from a dimension |
| D233 | Decided | A bare number in a column of a quantity type is read in the column's display unit, by default the preferred unit of the type; a cell with a unit is converted to that unit on entry. The display unit goes in the format bracket: `Weight : Mass[g]` | Matches `add-units`. The display unit is how the value is shown, not what it is |
| D234 | Decided | `x.to('g')` and `to(x, 'g')` convert a quantity to another unit of its dimension | The unit text is read with siquant's wide scope, so units that are not allowed in a cell can be named here later |
| D235 | Decided | A result with no dimension (`6m / 3m`) is a plain number: a `Float`, or a `Scientific` if it has an uncertainty | |
| D236 | Decided | A unit is read from text only in an `.omx` file; CSV, XLSX and JSON import never read a quantity, so `12g` or `10m` in imported data stays text | A switch on `parse_literal` |
| D237 | Decided | After a number, `.` continues a unit only if the next term is not followed by `(` and the whole text reads as an SI unit; otherwise it is a method call or field: `5kg.abs()`, `5m.Float` | The lexer backs off one term at a time. A test checks no function name is also a unit symbol |
| D238 | Decided | `sum`, `mean`, `min`, `max` and `median` of quantities fold from the first element, not from an integer zero. `std` and `var` refuse a quantity | |
| D239 | Decided | An affine unit (`°C`) is read only on its own; arithmetic on it goes through kelvin, so `20°C - 10°C` is `10K` | Delta-temperature units are not modelled |

## Open Questions

None for this stage.

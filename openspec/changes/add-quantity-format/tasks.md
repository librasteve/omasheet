# Tasks: A format for any quantity, and the `s` conversion

## 1. omx
- [x] 1.1 `QuantityFormat.unit` is optional; `Quantity[; .9s]` reads, writes back the same, and is refused with no unit and no `;`
- [x] 1.2 `SciConv::Auto`, the letter `s`; the help text and the error messages name it

## 2. engine
- [x] 2.1 `s` is `g` with the precision as significant digits when there is no uncertainty, and with no precision when there is one
- [x] 2.2 A format with no unit shows each value in its own unit and leaves the column's unit as it was
- [x] 2.3 Tests, `examples/formats.omx` and `examples/units.omx` cover it, and the expected output is regenerated

- [x] 2.4 The flag `_`: a space between the number and the unit
- [x] 2.5 The cut mark goes after the unit wherever it stood in the number

## 2b. constants
- [x] 2.6 `Document::set_const_format`, read by each constant of a type that takes it; `ConstSnap.numeric`
- [x] 2.7 The Value column of the Constants tab: `Set format…` in its menu, F4 to that alone, values set right

## 3. showcase and docs
- [x] 3.0 The format pane offers `Quantity[; ...]` formats, with short descriptions, and is open for a quantity with no unit; the footer says a cut value for the cursor's cell
- [x] 3.1 `examples/showcase/codata.omx` uses `Quantity[; .9s]` for its Checks
- [x] 3.2 `docs/format.md` describes the unit-less quantity format and `s`

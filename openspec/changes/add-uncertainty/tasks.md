# Tasks: Uncertainty

> Scope reduced and no longer dependent on `add-units` (owner, 2026-11-09), and built on `dev` after `add-sheet-operations-polars` was done.

## 1. Storage
- [x] 1.1 Decide how uncertainty magnitudes are stored: an `f64` standard deviation (D90)
- [x] 1.2 `Value::Uncertain(f64, f64)` and its place in the type lattice, above `Float` (D91)

## 2. Literals (`uncertainty`)
- [x] 2.1 `±` / `+-` operator, and the concise `10.5(3)e-3` literal in the lexer

## 3. Propagation (`uncertainty`)
- [x] 3.1 `+ - * /` in quadrature
- [x] 3.2 `**` and the `MathFn` set, to first order
- [x] 3.3 `sum` and `avg`; `min` and `max` keep the chosen element
- [x] 3.4 Where an operation may give a `Complex`, the uncertain value degrades to a `Float`
- [x] 3.5 `uncertain complex not supported` shown as an amber warning in the bottom bar; a warning colour in the theme

## 4. Existing capabilities
- [x] 4.1 `Uncertain` column type, and its `[±]` display format (`file-format`)
- [x] 4.2 Uncertainty preserved through group, join and aggregation (`evaluation`)
- [x] 4.3 Display: `(nn)` by default, `±` by format option
- [x] 4.4 Value and uncertainty columns on XLSX export (`xlsx-interop`)

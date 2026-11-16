# Tasks: widen what lowers to Polars

Forthcoming; follows `polars-function-registry`.

## 0. Settle
- [x] 0.1 Read the engine's uncertainty formulas and its offset-unit rule; settle D-W4 and D-W5 against them (siquant `quantity.rs`/`unit.rs`, `value.rs:402`)
- [x] 0.2 The open items in `design.md`
- [ ] 0.3 D-W6 against `add-group-apply`: rewrite its D-A6 and task 3.2

## 1. Static types only
- [ ] 1.1 `lower.rs` declines a column with no declared type, a `Fraction` and anything typed `Any`, by name
- [ ] 1.2 Correct the module header: what is declined, and why

## 2. Percent
- [ ] 2.1 `Lit::Percent` and a percent `const` as a `Decimal` in a declared column
- [ ] 2.2 The result's dress from `S::wider` through `Column::ty`
- [ ] 2.3 Tests: `N * 5%`, `Rate + 2%`, `5% + 3%`, `Rate * 2`, `Profit * TaxRate` (`examples/budget.omx`); the two paths agree

## 3. Quantity and Scientific
- [ ] 3.1 `Quantity op Quantity`: result unit from the checker, a constant factor for the right side
- [ ] 3.2 `Scientific op` a number or a `Scientific`: a value and an uncertainty expression
- [ ] 3.3 Offset units, scales, a column against a column in another unit, `Any`, and a mixed dimension stay on the engine; tests that they do
- [ ] 3.5 A zero divisor in `Quantity / Quantity` is not believed: the engine calculates the column and gives its error
- [ ] 3.4 Tests: every operator, an empty column, a null, a unit conversion, an uncertainty through a chain; the two paths agree

## 4. Checks
- [ ] 4.1 `docs/expressions.md`: which columns are calculated in Polars
- [ ] 4.2 fmt, clippy -D warnings, workspace tests

# Tasks: Cast to Float for the trigonometric, exponential and logarithmic functions

## 1. Engine (`omx-expressions`)
- [x] 1.1 List the functions covered: `sin cos tan`, their inverse and hyperbolic forms, `exp ln log10 log2`
- [x] 1.2 Cast the argument with the rule `.Float` uses, and keep `exp` and `ln` of a `Complex` as `Complex`
- [x] 1.3 Keep the error naming the function for a value `.Float` refuses
- [x] 1.4 A column goes through the same `math`, a row at a time: a `Boolean` or text column is cast, an empty cell stays empty (no separate Polars path for these)

## 2. Check
- [x] 2.1 `omasheet-omx/src/check.rs`: the lint accepts the same argument types, so no `cannot be applied` is reported for a value that will be cast

## 3. Tests
- [x] 3.1 `sin(true)`, `sin('1')`, `ln('20')`, `exp(0x10)` and `sin(3+0i)` give `Float`s
- [x] 3.2 `sin(2025-01-01)`, `ln(PT1H)` and `sin(3+4i)` are still errors that name the function
- [x] 3.3 `exp(3+0i)` stays `20.085536923187668+0i`
- [x] 3.4 An empty cell through `sin` stays empty (the `Null` arm); the column case is by the same function

## 4. Docs
- [x] 4.1 `docs/expressions.md`: the Maths line, and the Complex section
- [x] 4.2 `skill/omasheet/SKILL.md` does not list the argument types: nothing to change

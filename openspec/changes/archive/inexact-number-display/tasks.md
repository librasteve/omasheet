# Tasks: one display rule for inexact numbers

## 0. Decide
- [x] 0.1 D-I1: the significant-digit bound (15 or 14)
- [x] 0.2 D-I2: the three-place stage
- [x] 0.3 D-I3: zero-sigma `Scientific`
- [x] 0.4 Prototype D-I1 and show before and after for the probe table

## 1. Implement
- [x] 1.1 `to_places` bounded by significant digits
- [x] 1.2 Drop `without_trailing_zeros` in `quantity.rs` if redundant
- [x] 1.3 Whatever 0.2 and 0.3 decide

## 2. Goldens and docs
- [x] 2.1 Show the diff of each golden that moves, then take it
- [x] 2.2 `docs/expressions.md` (cutting paragraph), `docs/format.md`
- [x] 2.3 Workspace tests, `cargo fmt`, `clippy -D warnings`

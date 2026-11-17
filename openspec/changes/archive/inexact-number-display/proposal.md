# Proposal: one display rule for inexact numbers

**Under evaluation; nothing is decided beyond what `design.md` marks settled.** It came out of removing `tidy` from siquant (0.1.3), which showed that a converted quantity is shown with the noise of its arithmetic: `2000.000mg·`, `30.000°·`.

## Why

An inexact number (`Float`, a `Quantity` with no uncertainty, each part of a `Complex`, a `Scientific` with a zero uncertainty) is the result of f64 arithmetic, and the last digit or two of it are noise. The display cleans this in two stages that disagree:

- `to_places` rounds a `Float` to `float_places` (15) *places after the point*, which cleans noise on small numbers (`0.1 + 0.2` is `0.3·`) but finds nothing to clean on a larger one (`100 * 1.1` is `110.000·`);
- `cut_places` then rounds to three places, which leaves one significant digit for `pi / 1000` (`0.003·`) and exempts anything below a thousandth.

The `Quantity` path reuses the `Float` one, and a zero-sigma `Scientific` skips both and shows 16 digits with no dot. The dot itself means two things: pure ulp noise, and digits really dropped.

## What Changes

- **omx-expressions**: MODIFIED. The noise stage of the display is bounded by significant digits as well as places, so a number keeps no more than fifteen significant digits: `110.00000000000001` is `110·`, `2000.0000000000002mg` is `2000mg·`.
- **omx-expressions**: the Quantity-only trailing-zero trim in `quantity.rs` is dropped, if the above makes it unnecessary.
- To be settled in `design.md`: whether the three-place stage becomes three significant digits, and whether a zero-sigma `Scientific` is cut like a `Float`.
- Goldens that show noise change: `examples/units.expected`, the angles golden, and any of `display.expected`/`complex.expected` that move. Each diff is shown before it is taken.

## Impact

- `omasheet-engine`: `value.rs` (`to_places`, possibly `cut_places`), `quantity.rs`
- `docs/expressions.md` (the paragraph on cutting), `docs/format.md`
- Goldens and tests as above; `Decimal` and exact paths are unchanged (`1.000·`, `0.300·` stay).

## Out of Scope

- `Decimal`, `Fraction` and `Percent` display; their trailing zeros are kept on purpose.
- The `.Ns` column format, which stays the opt-in for significant digits.
- The file form and `--exact`, which keep every digit.

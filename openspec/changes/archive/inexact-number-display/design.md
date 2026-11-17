# Design: one display rule for inexact numbers

## Current behaviour (probed on `spike`)

| Expression | Grid | Note |
|---|---|---|
| `0.1.Float + 0.2.Float` | `0.3·` | places stage cleans it |
| `sin(pi)` | `0·` | places stage |
| `100 * 1.1.Float` | `110.000·` | places stage finds nothing; 3-place stage shows zeros |
| `pi / 1000` | `0.003·` | 3-place stage keeps one significant digit |
| `pi / 100000` | `0.000031415926536·` | below 0.001 is exempt |
| `exp(1) * 1e15` | `2.718e+15·` | |
| `exp(i * pi)` | `-1+0i·` | complex part cleaned by places stage |
| `(1/3.Float) ± 0` | `0.3333333333333333` | zero-sigma `Scientific`: never cut, no dot |
| `(2g).to('mg')` | `2000.000mg·` before the Quantity trim | |

## D-I1. Bound the noise stage by significant digits (settled)

`to_places` uses `min(float_places, 14 - floor(log10 |x|))`, never below 0. Noise in the sixteenth significant digit is then cleaned at every magnitude, and `sin(pi)` still shows `0·` because the 15-places limit still applies to small numbers. Decimal and exact paths do not go through it.

**Open:** whether 15 significant digits is the right bound; an f64 holds 15.95, and a conversion can lose a digit or two, so 14 may be the safer one.

## D-I2. The three-place stage (settled)

Three places after the point for a number of 1 or more; for a number below 1, three significant digits (`pi / 1000` is `0.00314·`, `1/3000` is `0.000333·`). The old exemption below a thousandth goes. Below a millionth a number is shown as before, in full or in e notation, so that e notation is not made wider than it is (`1.235e-15·` stays). `Decimal` shares the rule, so exact decimals below 0.1 with more than three significant digits are now shown with them (`0.0725`, not `0.073·`).

## D-I3. Zero-sigma `Scientific` (settled)

Shown as a `Float` is: the same noise stage, the same cut, the dot only for digits really dropped. A format of its own (`.Ns`, `f`, `e`) still sets the digits, and a file or `--exact` keeps every digit. A `Scientific` with an uncertainty is unchanged.

## D-I4. What the dot means (settled)

It marks both noise removed and digits dropped. Noise removed past the fifteenth significant digit is not marked; the dot means digits that were there are not shown.

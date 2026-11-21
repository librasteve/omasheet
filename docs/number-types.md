# Choosing a number type

Omasheet has several kinds of number, and which one a value is decides how exact it is. This chapter is a map: what each is for, how they group, and what happens when they meet. The details of each are in [the file format](format.md#types) and [Expressions](expressions.md#numbers).

| Type | Group | Literal | Representations |
|---|---|---|---|
| `Integer` | ℤ | `1` | `0xff`, `0o17`, `0b101` |
| `Fraction` | ℚ | `<1/7>` | `1/7`, `2 3/4` |
| `Decimal` | ℚ | `1.0` | `14.29%` |
| `Float` | ℝ | `1e0` | - |
| `Scientific` | ℝ | `1.01(01)e0` | `1 ± 0.01` |
| `Complex` | ℂ | `3+4i` | - |

The group is the number system the type belongs to: ℤ the integers, ℚ the rationals, ℝ the reals and ℂ the complex numbers. The type is how the value is held and shown.

## Which to use

- **`Integer`** for counts, identifiers and anything whole. Written in hexadecimal, octal or binary it is still an `Integer`, shown the way it was written.
- **`Fraction`** when the answer has to be exact whatever it is: `1/3 + 1/6` is `1/2`, not `0.5000000000000001`. It is shown as a fraction, and a mixed number such as `2 3/4` is the same value as `11/4`.
- **`Decimal`** for money and measurements written with a decimal point. `0.1 + 0.2 == 0.3` is true, because a `Decimal` is exact and a `Float` is not. A percentage in a `Decimal` column is the number it stands for, so `14.29%` is `0.1429`.
- **`Float`** for what has a wide range, such as `1e-300` or `2 ** 0.5`. It trades exactness for range, and a result is shown cut to three places with a `·` after it, `1.414·`, to say there is more. `--exact` on the command line shows it in full.
- **`Scientific`** for a measurement, where the uncertainty matters: `10 ± 0.1`. See [Constants and uncertainty](constants.md).
- **`Complex`** for `3+4i`, where an answer has a real and an imaginary part.

Other types are not numbers in this sense but sit beside them: `Quantity` and its kinds, a number with an SI unit (see [Units and dimensions](units.md)); `String`; `Boolean`; and the dates and times.

## When types meet

An operation gives the least number system that holds both of its operands, and the more *infectious* type wins. Reading the types in the table from the top, each is more infectious than the one above it:

```text
> 1 + 1/2
1 1/2
> 1.5 + 1/2
2
> 2 ** 0.5
1.414·
> 0xff + 1
256
```

`Integer`, `Fraction` and `Decimal` are the exact types. Once a `Float`, a `Scientific` or a `Complex` is in the calculation, the answer is that type, however exact the rest was. A power with a whole exponent stays exact, and one with any other exponent, such as `2 ** 0.5`, is a `Float`, as are the trigonometric, exponential and logarithmic functions.

Two values shown the same way keep the way: `Hex` plus `Hex` is a `Hex`. Two different ways of showing one type give the plain type.

## Converting

A conversion is a method named for the type you want: `x.Integer`, `x.Decimal`, `x.Fraction`, `x.Float`, `x.Complex`, `x.Percent`. `(1/8).Decimal` is `0.125`. A conversion that loses exactness, such as `x.Float`, is the one place a value leaves the exact types on purpose, so it is the call to look for when a result is not as exact as it should be. [Expressions](expressions.md#converting) has the table.

## In a column

A column's declared type fixes what its cells are. A `Fraction` column keeps every exact number exactly: `1/7` stays `1/7`, and `2.5` is `2 1/2`. A `Decimal` column holds its values to a fixed number of places, so a `1/7` written into one is rounded to them and shown cut, `0.143·`. If a value must not be rounded, declare the column `Fraction`. [The file format](format.md#types) lists what each type reads and how its format, written in brackets after the type, shows it.

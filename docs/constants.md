# Constants and uncertainty

Physical constants are numbers with a unit and, usually, a measured uncertainty. Omasheet has the common ones built in, and a number type that carries uncertainty through your arithmetic.

## Built-in constants

These names are ready in every file, drawing on the 2019 SI and CODATA 2022:

| Name | Is | Value |
|---|---|---|
| `pi`, `tau`, `e` | the mathematical constants | `3.141592653589793`, `6.283185307179586`, `2.718281828459045` |
| `c` | speed of light | `299792458m⋅s⁻¹` |
| `planck` | Planck constant | `6.62607015e-34J⋅s` |
| `hbar` | reduced Planck constant | `1.0545718176461565e-34J⋅s` |
| `kB` | Boltzmann constant | `1.380649e-23J⋅K⁻¹` |
| `NA` | Avogadro constant | `6.02214076e+23mol⁻¹` |
| `R` | molar gas constant | `8.31446261815324J⋅K⁻¹⋅mol⁻¹` |
| `qe` | elementary charge | `1.602176634e-19C` |
| `me` | electron mass | `9.1093837139e-31kg` |
| `G` | gravitational constant | `6.6743e-11m³⋅kg⁻¹⋅s⁻²` |
| `gn` | standard gravity | `9.80665m⋅s⁻²` |
| `au`, `ly`, `ld` | astronomical unit, light year, light day | `149597870700m`, `9.4607304725808e+15m`, `25902068371200m` |

A constant is a quantity, so it works in a formula with other units: `planck * c` is `1.9864458571489286e-25J⋅m`, the units combining as they do on paper. A column, constant or sheet of the same name replaces a built-in, so a file that calls a column `c` is still fine.

Small values are cut for display: at the default precision `planck` is shown as `0J⋅s`. Use `--exact` on the command line, `omasheet eval --exact planck`, to see the value in full.

## Uncertainty

A measurement carries an uncertainty, a standard deviation. `10 ± 0.1` is `10` with an uncertainty of `0.1`. The concise form that physics and chemistry tables use writes the uncertainty in the last digits shown: `10.5(3)` is `10.5 ± 0.3`. An exponent after the brackets scales both, so the Newtonian constant of gravitation, with its uncertainty, is:

```text
> 6.67430(15)e-11m^3.kg^-1.s^-2
6.67430(15)e-11m³⋅kg⁻¹⋅s⁻²
```

The built-in `G` is the value alone. Write the literal when the uncertainty matters.

An uncertain value carries its uncertainty through `+ - * /`, `**` and the maths functions, to first order and treating operands as independent: a sum combines the absolute uncertainties, and a product or quotient the relative ones, in quadrature.

```text
> (10 ± 1) + (20 ± 2)
30.0(22)
> 10 ± 0.1 * 2
20.00(20)
> (5kg ± 100g) * 2
10.00(20)kg
```

The uncertainty of `(10 ± 1) + (20 ± 2)` is √5, not 3. Operands are independent even when they are the same value, so `x - x` is not exactly `0`: Omasheet does not track which measurements a value came from. Write an identity out before the uncertainty is attached to cancel it.

## In a column

A column of measurements is a `Scientific`, or a quantity with a unit, and its format, in brackets, decides how it is shown. With nothing before the letters the value is shown as `10.5(3)`; with a `±` before them it is `value ± uncertainty`. `Length : Scientific[e-3]` puts the figures in the exponent form. Where the format gives no places, they are worked out for each value from its own uncertainty. [The file format](format.md#types) has the table of format letters, and [Expressions](expressions.md#scientific-numbers) the rules for uncertainty in full.

## Limits

There are no uncertain complex numbers: a `Scientific` beside a `Complex` becomes a `Float`, and the app says so in its bottom bar. A first-order answer is poor where the uncertainty is large beside the distance to the edge of a function's domain, as in `ln(0.1 ± 0.5)`.

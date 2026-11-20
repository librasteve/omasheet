# Expressions (OMX)

See [OMX in brief](omx.md) for the grammar. Try any of these with `omasheet eval [file.omx] '<expression>'`.

## Numbers

Arithmetic is exact: `1/3 + 1/6` is `1/2`, `0.1 + 0.2` is `0.3`, and integers
do not overflow. `x.Float` gives a floating-point `Float`.

No exact number is a division by zero, so that gives a `Float`: `1/0` is `Inf`,
`-1/0` is `-Inf` and `0/0` is `NaN`. So does a function at the
edge of its domain: `ln(0)` is `-Inf`. Past the edge a real argument gives the principal
`Complex` (see Complex numbers): `sqrt(-1)` is `0+1i`. They are written as they are shown,
`Inf`, `-Inf` and `NaN`, in a formula or a cell; the text is in quotes,
`'Inf'` in a formula and `"Inf"` in a cell. A `NaN` is ordered greatest and equal to itself, so `NaN == NaN` is
true, a `NaN` sorts last, and the largest of a column holding one is the
`NaN`. That is a dataframe's rule rather than IEEE's, where a `NaN` equals
nothing at all: it is kept so that a column of floats gives the same answer
whichever works it out. To ask whether a value is one, `is_nan(x)`; an
empty cell is a missing value rather than a `NaN`, so `is_nan` of one is
empty and `is_null` is what answers for it.

A number is shown in at most 23 characters, and a `-` before it. From
`1e-15` up to `1e15` it is plain, `0.015625`; smaller or larger it is in e
notation, `1e-43` and `1e+60`, and so is one too wide to be plain. An exact
number in e notation keeps every digit it can, and where they do not all fit
they are cut and marked: `2 ** 64` is shown as `1.8446744073709551e+19·`.
Only the display is cut, and `omasheet eval --exact` gives it in full.

A `Decimal`, a `Float` and the parts of a `Complex` are shown to three places
after the point, and where they have more the rest is cut and marked with
`·`: `1.0/3` is `0.333·` and `sqrt(2)` is `1.414·`. One with no more than three
places is whole, `19.99`. One below 1 is shown to three significant digits
instead, `pi / 1000` is `0.00314·`, so as not to show as nought or as one
digit; below a millionth it keeps its digits, or is in e notation. A `Float` in e notation has its mantissa cut the same way, `1.234e-12·`. Before that a `Float`, and each part of a `Complex`, is rounded to 15 places after the point, and to no more than 15 significant digits, so that the noise of a calculation is not shown, and not marked either, as it is not digits that were there: `cos(90°)` is `0` and not `6.123e-17·`, `100 * 1.1.Float` is `110` and not `110.00000000000001`, and `1e-20.Float` is `0`. That is the `float_places` of the display style; the value itself, and a file, keep every digit, and `=~=` compares numbers that differ by noise. A quantity with no uncertainty and no format has its number cut the same way, with the `·` after the unit: `arcsin(1)` is `1.571rad·`. A `Scientific` with an uncertainty, and a quantity with one, is not cut: the uncertainty in brackets
sets the digits it shows. One with an uncertainty of nought is an inexact number, and is shown as a `Float` is: `(1/3.Float) ± 0` is `0.333·`.

A `Decimal` is a Polars `Decimal(38, s)`: 38 digits, `s` of them after the
point, and the places it keeps are the column's, written `Decimal[2]`. A bare
`Decimal` is `Decimal[10]`, which holds up to about 10²⁸. Every value of a
column has its scale; the point does not move. A sum is exact at the finer
of the two scales, a product is worked out exactly and rounded once to it,
and a quotient, or a mean, is rounded once to ten places or the finer scale
of its sides, half away from zero. So `1.0/3` is `0.3333333333`, `1.0/3 * 3` is
`0.9999999999`, and `2.0/7 + 3.0/7` is not `5/7`; for those, use a `Fraction`,
which is what a quotient of whole numbers is: `1/3 * 3` is `1`.

A formula's literal has ten places or its own, whichever is more, so
`19.99 * 1.5` is `29.985` and not `29.99`. A column that declares
`Decimal[2]` holds a formula's result rounded to two places, and refuses a
cell written with more (`0.125` is refused, naming `Decimal[3]` or
`Fraction`). A column that declares nothing infers the least scale that holds
its cells: `19.99` and `5` are `Decimal[2]`.

What does not fit is a `Fraction`. A result that needs more than 38 digits at
its scale, `10**100`, or a decimal written with more than eighteen places, is
the exact rational it comes to, never an error: an undeclared column holding
it is a `Fraction` column, and a column that declares a `Decimal` refuses it,
saying so. `10**100 * <1/10>**80` is `100000000000000000000`.

Only the display is cut, and `--exact` gives a decimal in full.

A `Float` is never rounded: it is shown with the fewest digits that give the
same `Float` back, `0.30000000000000004`, and always fits.

A `Percent` is an exact number shown as so many in a hundred: `20%` is `1/5`.
It is rounded to two digits after the point, `41.67%`, and `--exact` gives
every digit it holds.

A quotient of whole numbers is a `Fraction`, exact whether or not it would
end as a decimal: `7 / 4` is `7/4` and `7 / 3` is `2 1/3`. A quotient with a
decimal in it is a `Decimal`, held to ten places: `7.0 / 4` is `1.75` and
`7.0 / 3` is `2.333·`.

Numbers are shown the way Raku shows them:

| Type | Shown |
|---|---|
| `Integer` | `101`; `1e+15` and `1.8446744073709551e+19·` from `1e15` |
| `Decimal` | `1.01`; `1e-43` below `1e-15` |
| `Fraction` | `1/7`, `3/4`, `2 3/4`, `333 1/3`; `3` when it is whole |
| `Percent` | `20%`, `12.5%`, `41.67%` |
| `Float` | `100`, `1.5`, `1.414·`; `1e+21` or `1.5e-16` when very large or small; `Inf`, `-Inf`, `NaN` |
| `Scientific` | the concise form of the physical tables, the uncertainty to two digits in brackets after the value cut at the same place: `7.2973525643(11)e-3`, `11.000(82)`, `1.00(20)e3`; a column format changes it, see below; no uncertainty is a plain number |
| `Complex` | each part as a `Float`: `3+4i`, `1.5-2i`, `0+1.414i·`; both parts cut alike when too wide, and marked once after the `i`; `0+Inf\i` for an infinite imaginary part |

A `Float` that is a whole number looks like an `Integer`, but it is still a `Float`.

A `Decimal` and a `Percent` are the same exact numbers shown in different
ways: `19.99` and `20%`. Adding, subtracting, multiplying and dividing
decimals gives a `Decimal`, and so does an average. A `Fraction` is not a
dress but the exact rational: an integer numerator and denominator of any
size, always reduced, never rounded. It is shown as a fraction of whole
numbers, with a whole part where there is one, `1/3`, `2 3/4`; past a
denominator of 10000 it is shown as a decimal, cut and marked, and in
e notation where it is very large or small. `--exact` writes a fraction as
`n/d`, which is how a file carries one.

`x.Fraction` is the fraction of the number `x` is: `1/8` for `0.125`, and for
a decimal quotient the fraction of what it was rounded to, so `(1.0/7).Fraction`
is `1428571429/10000000000`, where `(1/7).Fraction` is `1/7`. To get a seventh, write one: in a formula a fraction
is written in angle brackets, `<1/7>`, `<-3/4>` or `<2 3/4>`, and is a
`Fraction` as `1/7` is in a cell. A fraction beside a whole number or
another fraction is a fraction, worked out exactly: `<1/7> * 7 == 1` is true,
and a `Fraction` column holding `Yield * 2` shows fractions. Beside a
`Decimal` or a `Percent` it is a `Decimal`, rounded to its places, so
`<1/3> + 0.25` is `0.5833333333`: a `Decimal` is exact only to its places.
Beside a `Float` it is a `Float`. `sqrt`, `ln`, `exp`, the trigonometry and a fractional
power of a fraction are `Float`s, having no rational result. A space goes
before a fraction that follows a `+`, since `+<` is a shift; the error for
`1+<1/10>` says so.

A column of fractions is the engine's: no dataframe has a dtype for a
rational, so it is not lowered (see [Down a column](#down-a-column)), and a
write to Parquet or a workbook puts it in as text or the nearest double.

Two percentages give a percentage: `20% + 5%` is `25%`, and so are their
sum, average, least and greatest. A percentage with any other number gives
the plain number: `100 * 20%` is `20`, `1 + 20%` is `1.2`, and `20% == 0.2`
is true. To show a result as a percentage, declare its column `Percent` or
write `x.Percent`.

`a % b` is what is left over when `a` is divided by `b` in whole steps down,
so it has the sign of `b`, as a workbook's `MOD(a, b)` does: `7 % 3` is `1`,
`-7 % 3` is `2`, `7 % -3` is `-2` and `7.5 % 2` is `1.5`. It binds as `*`
and `/` do. A `%` straight after a number is a percentage (`20%`), so a
remainder is written with spaces round it. A number is divisible by another
when the remainder is zero: `x % 3 == 0`. Exact numbers stay exact, and
`x % 0` is `NaN`, as `0/0` is. A workbook export writes `MOD(a, b)`.

A quotient of whole numbers is a `Fraction`, though `4/2` written out is an
`Integer`. A quotient is taken where a whole number is wanted, `[1, 2, 3][4/2]`,
and reported there if it is not whole, `5/2`.

A power with a whole exponent is exact; with any other, `2 ** 0.5`, it is a
`Float`, as `sqrt(2)` is.

`+ - * / **`, `== != =~= < > <= >=`, `and or not`, `in`, and `a // b` (b when a
is empty). `if c then a else b`.

Comparisons chain, as in Raku: `0 < x <= 10` is `0 < x and x <= 10`, and
`a < b < c < d` tests each pair in turn. `in` takes one side each.

`==` on two `Float` values is exact, so `sin(pi) == 0` is false. `a =~= b` is Raku's approximate equality: true when `a` and `b` differ by less than 1e-15 of the larger of them (or by less than 1e-15 where one is zero), so `sin(pi) =~= 0` and `pi * 1rad =~= 180°` are true. It takes numbers, `Complex` values and quantities (the right side is read in the unit of the left), ignores any uncertainty, and leaves `==` exact: it is the one comparison that has a tolerance.

A string in a formula is in single quotes: `Name // 'Unknown'`, with `\'`,
`\\`, `\n` and `\t` as its escapes. Double quotes are for text in a table
cell, `"2 * 3 * 4"`, so code and text are told apart at a glance; a formula
with a double-quoted string is an error that shows the single-quoted one.
Everything outside a table cell is code: a column's formula, a `const`, a
cell's `= …`, `dataframe['sales.parquet']`, `from 'Units/Boxes Sold'`, and
`{{ }}` and `omx` blocks in Markdown. In a number format, the `'` that
groups digits is escaped: `'%\'.2f'`.

An empty cell in a comparison or a sum gives an empty answer: anything done
with a missing value is missing. `and` and `or` are the exception, and they
read an empty cell as an unknown rather than as a gap: an unknown that
cannot change the answer does not decide it. So `false and` is false and
`true or` is true whatever is on the other side, empty cell included, while
`true and` and `false or` an empty cell are empty, as is `not` of one. This
is the rule a dataframe and SQL both use, and it is kept for the reason the
`NaN` ordering above is kept — so that a column gives the same answer
whichever works it out. Where the answer should be false rather than empty,
`a // false` says so.

### Bitwise

On whole numbers, written as Raku writes them: `a +& b`, `a +| b` and
`a +^ b` are and, or and xor of their bits, `+^a` is the complement, and
`a +< n` and `a +> n` shift `a` left and right by `n` bits. `and`, `or` and
`not` stay Boolean. `+&`, `+<` and `+>` bind as `*` does and `+|` and `+^`
as `+` does, so a flag test needs no brackets: `Mode +& 0o100 == 0o100`. A
`+` and its symbol are one operator only with nothing between them: `a + ^5`
is still a sum and a range.

The numbers are 64-bit two's complement. `+^0xff` is `0xffffffffffffff00`,
and `+>` fills with the sign, so `-16 +> 2` is `-4`. A shift of less than 0
or more than 63 bits is an error, and so is a `+<` that leaves 64 bits.
Two numbers of one base give that base and a mix an `Integer`:
`0xF0 +& 0x3C` is `0x30` and `0xF0 +& 60` is `48`; a shift keeps the base
of the number it shifts. Anything but a whole number is an error, `true +& 6`
included. In a table, a `+|` in a cell that starts with `=` stays in the
cell. An XLSX Workbook export writes the values of these formulas.

## Converting

The name of a type converts to it, written `x.Integer`, `x.Integer()` or `Integer(x)`. A
vector is converted value by value.

| Written | Gives |
|---|---|
| `x.Integer` | a whole number: `19.99` gives `19`, `-19.99` gives `-19` |
| `x.Decimal` | the same number as a decimal: `(1/8).Decimal` is `0.125`, `(1/3).Decimal` is `0.333·` |
| `x.Percent` | the same number as a percentage: `0.4` gives `40%`, `5/12` gives `41.67%` |
| `x.Float` | a floating-point number; of a `Complex`, its real part, where its imaginary part is zero, and otherwise an error |
| `x.Complex`, `Complex(re, im)` | a complex number from a real one, or from two |
| `x.String` | any value as text, written the way a file writes it |
| `x.Boolean` | `false` for zero, `true` for any other number |
| `x.Date`, `x.Time` | that half of a `Datetime` |
| `x.Datetime` | a `Date` at midnight |

Text is read the way a cell is: `'42'.Integer` is `42`, `'20%'.Decimal` is `1/5`,
`'2025-01-31'.Date` is that date, and `'abc'.Integer` is an error. `true.Integer` is
`1`. To round rather than drop the fraction, use `round(x)`; `round(x, 2)`
rounds to two decimal places. For the nearest hundred, divide, round and
multiply back: `round(x / 100) * 100`.

## Scientific numbers

A measurement carries an uncertainty, a standard deviation. `10 ± 0.1` is `10`
with an uncertainty of `0.1`, and `10.5(3)` is the
concise form physics and chemistry tables use, where the digits in brackets
are the uncertainty in the last places shown: `10.5 ± 0.3`. An exponent after
the brackets scales both, so `7.297_352_5643(11)e-3` is the fine-structure
constant with its uncertainty. The digits in brackets are digits only, with no
space before them. `±` binds more tightly than any other operator, a power
included, so `10 ± 0.1 ** 2` is `(10 ± 0.1) ** 2` and `10 ± 0.1 * 2` is
`20 ± 0.2`; each side is a single plain number, name or bracketed
expression, and the uncertainty is not negative.

An uncertain value carries its uncertainty through `+ - * /`, `**` and the
math functions, to first order and treating the operands as independent: a
sum combines the absolute uncertainties, and a product or quotient the
relative ones, in quadrature. A number with no uncertainty has an uncertainty
of zero.

```
(10 ± 1) + (20 ± 2)        30.0(22)       the uncertainty is √5, not 3
(10 ± 0.1) * 2             20.00(20)
sin(0.5 ± 0.01)            0.4794(88)
ln(100 ± 1)                4.605(10)
(10 ± 1) - (10 ± 1)        0.0(14)        independent, even when they are the same cell
```

Operands are independent even when they are the same value: `x - x` is `0 ± √2σ`, not exactly `0`, and `x / x` is `1 ± √2σ/x`, because Omasheet does not track which measurements a value came from. A cell reference is read as another measurement of the same quantity. To cancel an identity, write it out before the uncertainty is added, as `0 ± 0` or `1`, or use the same value once, as `2 * x` rather than `x + x` (`2 * x` is `2σ`; `x + x` is `√2σ`).

`sum` and `mean` carry the uncertainty; `min` and `max` give the value they
chose with its own; `std`, `var` and the other spreads work on the values
alone. A comparison and a sort use the values. A first-order answer is poor
where the uncertainty is large beside the distance to the edge of a
function's domain, as `ln(0.1 ± 0.5)`.

A column's format, in brackets after its type, is a number format as a `Float` takes (see [the file format](format.md)), with `±` before it for `value ± uncertainty`: `f` for a number of places, `e` for an exponent, `g` for whichever reads better, and the flags `+`, space, `0`, `-`, `'` and a width. `Length : Scientific[e-3]` puts every value of the column at one exponent, `Scientific[.4f]` gives them all four places, so that the brackets stand in line, `Scientific[±]` shows each as `11.000 ± 0.082`, and `Scientific[±.2e]` as `(1.21 ± 0.03)e-2`. What a format does
not name is worked out for each value from its own uncertainty. A value with
no uncertainty keeps every digit it has under a format, with `(0)` for an
uncertainty. `examples/uncertainty.omx` shows them side by side.

There are no uncertain complex numbers. A `Scientific` beside a `Complex`
becomes a `Float` and the calculation goes on with a `Complex`; the app
says `uncertain complex not supported` in amber in its bottom bar while the
cell is selected. A `Scientific` goes to a `Float` with `x.Float`, which keeps
the value alone.

## Units

A number followed at once by an SI unit is a quantity: `5kg`, `300cm`,
`9.81m.s^-2`, `20°C`. The number is any number a `Scientific` is, so
`10.5(3)e-3kg` has its uncertainty, and `5kg ± 100g` is a quantity with one.
A unit of more than one term is joined with `.`, each term with an optional
`^` and a whole exponent: `kg.m^2.s^-2`. There is no space and no `/`; a
negative exponent is `m.s^-1`. Only metric units are written in a cell: the
symbols `g m s A K mol cd`, the named units `Hz N Pa J W C V F Ω S Wb T H Bq Gy
Sv kat` and `°C`, each with an SI prefix from `q` to `Q`, `kg`, `mm`, `μs`,
`GHz`, and the non-SI units the metric system uses: `l` (litre, also `L`), `t` (tonne), `kt` and `Mt` (kilo- and megatonne, as the tonne takes no prefix), `ct` (carat, 200 mg), `ha` (hectare), `are`, `barn`, `bar`, `atm`, `torr`, `eV`, `cal` (4.184 J), `Wh` (3600 J), `mhp` (the metric horsepower, 735.49875 W), `gsm` (a gram per square metre), `Da` (dalton), `min`, `h`, `d`, `wk` and `yr` (the Julian year) for time, `au`, `ly`, `ld`, `pc`, `Å` and `nmi` for length, `kn` (knot), `gon`, `rev` and `rpm` (an angular speed) for angle, `rem`, `Ci` and `R` for radiation, `Jy` (jansky), `tog` and `sverdrup`. All but `t`, `kt`, `Mt`, `ha`, `are`, `ct`, `au`, `ld`, `Å`, `nmi`, `kn`, `atm`, `mhp`, `gsm`, `gon`, `rev`, `rpm`, `tog`, `sverdrup`, `min`, `h`, `d` and `wk` take prefixes: `ml`, `kWh`, `keV`, `kcal`, `kpc`, `Mly`, `Gyr`, `mbarn`, `mCi`, `mJy`. A symbol is case-sensitive, and `ºC`, `µ` and the ohm sign are read as `°C`, `μ` and `Ω`; `u` is read as the prefix `μ`, so `5um` is `5μm`. The imperial and US units are there too, and none of them takes a prefix: `in`, `ft`, `yd`, `mi`, `fur`, `ftm`, `chain`, `rod`, `thou`, `pica` and `point` for length, `oz`, `lb`, `st`, `cwt`, `ton`, `lton`, `gr`, `ozt`, `dwt` and `slug` for mass, `ac` (acre), `gal`, `qt`, `pt`, `cup`, `floz`, `tbsp`, `tsp`, `bbl` (the 42-gallon oil barrel), `impgal`, `imppint` and `cord` for volume, `mph`, `fps`, `lbf`, `ozf`, `psi`, `btu`, `therm`, `hp` (the mechanical horsepower) and the temperature scales `°F` and `°R`. Each is an exact multiple of an SI unit, `1ft` is `0.3048m` and `1gal` is the US gallon, 3.785411784 l, and `212°F` is `100°C`. The unit picker has a button for each sector, `Metric`, `Non-Metric`, `Currency` and `All`, and these units are in `Non-Metric`. A unit other than these is not a unit: `5xyz` is an error and `5 kg` is a number and a name. `PS` is a petasiemens, so the metric horsepower is `mhp`, and the barn is spelled out as `b` is the bit.

The product or quotient of two quantities takes the kind that has the dimension of the result: `Force * Length` is an `Energy`, `Length / Speed` a `Duration`. Where two kinds share a dimension the result is the fixed one of `Energy` (not `Torque`), `Frequency` (not `Radioactivity`), `Momentum` (not `Impulse`) and `SpecificEnergy` (not `Dose`), and `Area`; where no kind has the dimension it is a `Quantity`. Its unit is the one the operands give. A column that declares the other kind, `Torque[N.m]`, or another unit, `Energy[kWh]`, takes the result all the same, as any unit of the dimension fits.

A quantity to a whole power, `3m ** 2` or `2m ** 4`, has its unit to that power. A root is the power `1/2`, `1/3` or `1/4`: `9m^2 ** (1/2)` is `3m`, `8m^3 ** (1/3)` is `2m`, and `sqrt` is the square root. A root needs every exponent of the unit to divide by it, so `5m ** (1/2)` is an error, and a root beyond the fourth is too. The uncertainty goes through the power or root. Where the exponent is written out, the result is the kind of the new dimension: `Area ** (1/2)` is a `Length`, and a root of a kind the dimension does not divide is an error before any value is known.

An angle has the radian `rad`, the steradian `sr` (a radian squared) and the degree `°`, the minute `′` and the second `″` of arc, and the lumen `lm` and the lux `lx` that need it. Degrees, minutes and seconds are written together, `45°30′30″` or `-12°30′`, and `º`, `'` and `"` do for the marks; minutes and seconds are under 60 and only the last part may have a point. `45°` is a degree and `20°C` is Celsius. `sin`, `cos` and `tan` take an angle in any unit (a plain number is in radians), and `arcsin`, `arccos` and `arctan` give one back in `rad`, so `arcsin(0.5).to('°')` is `30°` and `sin(arcsin(0.5)) =~= 0.5` is true; `degrees(x)` takes such an angle too, and gives the number of degrees. An `arcsin` of a number beyond -1 to 1 is a `Complex`, with no unit. `dms` writes an angle out: `45.5°.dms()` is `45°30′0″`, and `dms(x, 2)` keeps two places of seconds.

Data has two units, the bit `b` and the byte `B` (eight bits). Each takes the SI
prefixes, where `kB` is 1000 B, and the binary ones `Ki Mi Gi Ti Pi Ei Zi Yi Ri
Qi`, where `KiB` is 1024 B: `4GiB`, `100Mb`. A rate needs no unit of its own,
`MiB.s^-1` or `kb.s^-1`. A binary prefix and a dot on its own is a scale, as
`k.` is: `1Ki.` is 1024 and `1.5Mi.` is 1572864.

Money is a kind of quantity too, `Currency`, of its own dimension `$`, with the US dollar `USD` for its unit. Every currency in [currency-api](https://latest.currency-api.pages.dev/v1/currencies/usd.json), about 340 of them with the crypto ones (`BTC`, `ETH`), is a unit by its code, `5GBP`, and the signs `$ € £ ¥ ￥ ₹ ₩ ฿ ₽ ₺ ₪ ₱ ₴ ₫ ₦` and `A$ C$ HK$ NZ$ S$` are a code each, written after the number, `5€`, or before it, `€5.50`. `$-1` is still the term before in a sequence, so a negative amount is `-$5`. A sum converts the right side to the unit of the left by the rates in use, and money mixes with the other units, `1GiB / $5` is `0.2GiB.USD^-1`. The rates change about once a day: the program has a snapshot of its own, a copy of the last fetch is kept in its cache directory, `omasheet rates` says what date they are as of and `omasheet rates update` fetches the latest; the app does so itself while a file with money in it is open, and says `rates 2026-11-18 (today)` in its status bar, `(cached)` or `(bundled)` for rates of an earlier day. A column `Price : Currency[EUR]` shows and holds euros, reads a bare number in them, and shows two places, or more for a unit worth more, unless the format gives places, `Currency[EUR; .4f]`; `$` in the format puts each amount down by its sign, `Currency[EUR; $]` is `€4.50`, and by its code where it has no sign, `3.00CHF`.

```
5kg + 300g           5.3kg          the right side is converted to the left's unit
300g + 5kg           5300g
2m * 3m              6m^2           a product or a quotient combines the units
10m / 4s             2.5m.s^-1
2m ** 3              8m^3           a power takes a whole number
4GiB + 512MiB        4.5GiB         data takes the binary prefixes
10MB.s^-1 * 3s       30MB           a rate over a time is an amount
1KiB.to('B')         1024B
sqrt(9m ** 2)        9m             a root, where the units allow it
6m / 3cm             200            no dimension is a plain number
5kg.to('g')          5000g          to another unit of the dimension
sin(30°)             0.5            an angle in any unit
45°30′ == 45.5°      true           degrees, minutes and seconds
(pi * 1rad).to('°') 180°          pi in radians, back to degrees
pi/2 rad            1.571rad       a unit after a constant multiplies the whole term before it
20°C - 10°C          10°C           an interval of two temperatures keeps the unit
5kg + 3m             an error: they measure Mass and Length
```

A sum, a difference and a comparison need the same dimension. The uncertainty
propagates as for a `Scientific`; two quantities are equal when they differ by
no more than their uncertainties together. `abs` and `sqrt` take a
quantity and the other math functions need a number with no unit. `sum`, `mean`,
`median`, `min` and `max` of a column of quantities give one, in the unit of
the first; `std` and `var` do not. `x.to('g')` and `to(x, 'g')` give a
quantity in another unit, and `x.Float` gives the number of a quantity with no
dimension. `x.unit()` gives the unit as text, `5kg.unit()` is `kg`, and
`x.value()` gives the number in that unit with its uncertainty, as a
`Scientific`: `10.5(3)kg.value()` is `10.5(3)`. Together they take a quantity
apart, and `x.to('g').value()` is its number of grams.

A kind of quantity, such as `Length`, `Mass` or `Energy`, and `Duration` are also
functions that give a value of that kind, in a call or as a method. A number, or
text that reads as one, takes the kind's own unit, seconds for a `Duration`, or
the unit given as a second argument: `Length(2.5)` is `2.5m`, `Length(2.5, 'km')`
is `2.5km` and `Length('5km')` is `5km`. A quantity of that kind is converted to
the unit given, `(5km).Length('mi')` is `3.107mi`, and otherwise kept in its own
unit. One of another dimension is an error: `Mass(5km)`. `Quantity` has no unit of
its own, so `Quantity(2.5, 'kg')` needs one. `x.type()` gives the name of the
type of `x` as text: `5.type()` is `Integer`, `Length(3).type()` is `Length`, and
a quantity with no declared kind gives every kind its dimension has, so
`(5J).type()` is `Energy/Torque`.

A formula in a sheet may begin a reference with a `.` to mean that sheet, so
`.Revenue.sum()` is `Sales.Revenue.sum()` written in `Sales`.

A `.` after a unit continues it if what follows is a unit, so `5N.m` is a
newton metre, and ends it if it is a call or a name, so `5kg.abs()` and
`5m.Float` are the methods.

A prefix and a `.` on their own is a scale, and reads as the number it stands
for: `1k.` is 1000, `1.5M.` is 1500000 and `5m.` is 0.005. A scale on an exact
number is exact, so `1.5k.` is the integer 1500. The dot makes it a prefix and
nothing else, which is why `m.` is milli while `5m` is five metres. Mind a
stray dot after `m` or `T`: `5m.` is 0.005, not five metres.

A column may be of a quantity type, which is covered under the types in the
file format.

## Complex numbers

An imaginary number is written with an `i`, `4i`, and `3+4i` is the sum of
`3` and `4i`. `Inf\i` and `NaN\i` are an infinite and a not-a-number imaginary
part. A `Complex` is the most infectious number type, so any number beside
one gives a `Complex`:

```
(1+2i) * (3-1i)        5+5i
abs(3+4i)              5
sqrt(-4+0i)            0+2i
conj(3+4i)             3-4i
degrees(arg(2i))       90
(1+1i) ** 2            0+2i
```

`+ - * /` and `**` follow the rules of complex numbers; a whole exponent is
repeated multiplication, as in Raku. `re(z)` and `im(z)` are the parts,
`conj(z)` the conjugate and `arg(z)` the angle in radians. `abs sqrt exp ln
log10 log2 sin cos tan sinh cosh tanh arcsin arccos arctan arcsinh arccosh
arctanh sign` take a `Complex` and give the principal value (the branch cuts are
C99's, so `arcsin(2+0i)` is `1.5707963267948966+1.3169578969248166i`).
`round floor ceil radians degrees` do not. A real number has the same parts,
with no imaginary part: `im(7)` is `0`.

A real argument outside the function's real domain gives the principal
`Complex` too, not `NaN`: `sqrt(-4)` is `0+2i`, `ln(-1)` is `0+3.141i·`,
`arccos(2)` is `0-1.316i·`, `arccosh(0)` is `0+1.571i·`,
`arctanh(2)` is `0.549+1.571i·`, `(-2) ** 0.5` is `1.414i·` (to a rounding error). The
edge of the domain stays real (`ln(0)` is `-Inf`, `arctanh(1)` is `Inf`), and a
`NaN` stays `NaN`. The type checker still calls the result a `Float`, so the
`Complex` shows in the value: an undeclared column holding one is `Complex`, a
column declared `Float` rejects the cell (`declared Float but this is Complex`),
and `sqrt(-4).Float` is an error. A `Scientific` stays real: `sqrt(-4 ± 0.4)` is
`NaN ± NaN`.

A `Complex` has no order: `1i < 2i` is an error. `=` and `!=` compare both
parts exactly. To get a real number out, use `re(z)` or `abs(z)`, or `z.Float`,
which works only where the imaginary part is exactly zero, and is otherwise
an error: `im(z) = 0` is the test, and `z =~= w` allows for the noise of a
calculation.

## Names

| Written | Means |
|---|---|
| `Sales` | the sheet |
| `Sales.Revenue` | the column, as a vector |
| `Revenue` | inside the sheet: this row's value |
| `TaxRate` | a constant |

## Selecting with `[columns; rows]`

| Written | Means |
|---|---|
| `Sales[Revenue; 1]` | one cell; positions count from 0 |
| `Sales[Revenue; -1]` | the last row |
| `Sales[; 0..2]` | rows 0 to 2, every column; `0..^2` leaves out the end |
| `Sales[; 0^..2]` | rows 1 to 2: `^..` leaves out the start, `^..^` both ends |
| `Sales[; ^2]` | the first two rows: `^n` is `0..^n` |
| `Sales[Revenue]` | the whole column: a missing or empty slot is all of it |
| `Sales[; Revenue > 11000]` | the rows where it is true |
| `Sales[Revenue; *-1]` | the row before this one; `*` is this row |
| `Sales[Revenue; 0..*]` | from the top down to this row |
| `Sales[Revenue; *-1 by Region]` | the row before this one among the rows of this row's region |

Inside its own sheet the name can be dropped: `[Revenue; *-1]`. A row
outside the sheet is empty, so `[Revenue; *-1] // 0` starts a running value.

`by` counts a cursor offset within a partition: the rows whose key columns
hold what this row's do, in the sheet's order. A running total per region
is

```omx
Running := Revenue + ([Running; *-1 by Region] // 0)
```

and `by Region, Month` partitions by both. The rows with an empty key are a
partition of their own. Only an offset of the cursor, `*-1` or `*+2`, takes
`by`; a range does not.

## Sheets from sheets

A sheet goes through stages with `|>`, each giving a sheet:

```omx
Passengers |> filter(Survived == 1) |> sort(Age) |> select(Name, Age) |> head(5)
```

- `filter(condition)`: the rows the condition is true of
- `select(columns)`: those columns, in that order
- `head(n)` and `tail(n)`: the first or last `n` rows
- `sort(columns)`: the rows in the order of the columns, ascending, or
  descending with `desc` as the last word: `sort(Fare, desc)`. Ties keep the
  order they came in, and an empty cell sorts first ascending and last
  descending

`sheet Name := …` makes a sheet of one (see [the format](format.md)). Two
more stages make rows of their own rather than keeping some of a sheet's,
so each is the whole of a sheet's formula, and what comes after it is
written over that sheet by name:

```omx
sheet ByRegion := Sales |> group_by(Region) |> agg(Revenue.sum(), Orders := Item.len())
sheet Best     := ByRegion |> sort(Revenue_sum, desc) |> head(1)
```

- `group_by(keys) |> agg(aggregates)`: a row for each value of the key
  columns, in the order of the keys, an empty key first. Its columns are
  the keys, then one for each aggregate, worked out over the rows of the
  group: in `agg` a column's name is that column over the group, so it
  needs an aggregate, `Revenue.sum()` and not `Revenue`. An aggregate of
  one column is named for it, `Revenue_sum`; `Name := …` names one, and
  an aggregate of anything else must be named
- `group_by(keys) |> apply(Fn)`: calls the function `Fn` once on the rows of each group, as a sheet, in the order `agg` gives the groups. The function takes one argument, the group's rows, and what it gives sets what the group makes. A value (`fn Takings(rows) = (rows.Units * rows.Price).sum()`) makes one row, the keys and a column named for the function (`apply(Total := Takings)` names it). A column or a sheet of the group's own rows (`fn BestTwo(rows) = rows |> sort(Units, desc) |> head(2) |> select(Rep, Units)`) makes a row for each row it gives, the keys repeated, so the top rows of each group and a ranking within it are one function each. A sheet that is another's rows (a lookup in a second sheet by the group's key) is paired with the group in the same way a `join` pairs rows. A function that gives a column a key also names is an error, so `select` it away; a group it gives no rows for has none, and a group it fails on has no rows and says why. See `examples/groups.omx`
- `join(Other, keys)`: a row for every pair of rows of the two sheets whose
  key columns are equal, the left sheet's columns and then the other's.
  `join(Other, keys, left)` keeps a left row that matches nothing, its other
  columns empty. A key that matches twice gives two rows, an empty key
  matches nothing, and a name both sheets have takes `_right` on the other
  sheet's column. The rows come in the left sheet's order, then the other's

## Calculated in Polars

A formula column over a table big enough to be worth it is calculated by Polars, in one step over the whole column, when every part of the formula is one it can do. The answer is the one the engine would give a row at a time, to the digit; a formula with any part Polars cannot do is the engine's, whole. The type of each column a formula reads is the one the checker has for it, written in the schema or worked out from all of the column's cells, so a column with no declared type is no worse off.

What is calculated in Polars:

- whole numbers, floats and decimals: `+ - *`, a quotient to as many places as Polars has (21), and a small whole power;
- a percentage in the formula, `Revenue * 5%` or `Profit * TaxRate` with `const TaxRate = 20%`, and a `Percent` column, as the `Decimal` each is. The result is a percentage where both sides are, and the plain number for any other mix;
- a quantity, in a column that declares a named kind (`Length`, `Mass`, `Area`, ...) or in a file's own column of one: `+ - * /` with another quantity or a number. A sum converts its right side to the left's unit, a product or a quotient is in the product or quotient of the units, and the answer is held in the unit of the column it is written into, so `Area := Width * Height` is in `m^2` whichever units the lengths were written in;
- a `Scientific`, and a quantity with an uncertainty: `+ - * /` with a number or another, the uncertainty carried to first order as the engine does. It is worked out as `sqrt(a² + b²)` where the engine has `hypot`, so it may differ from the engine's in the last place.

A `group_by(keys) |> apply(Fn)` is calculated in Polars, for every group at once, in these cases:

- a function that gives a value: `fn Takings(rows) = (rows.Units * rows.Price).sum()`, an aggregate of a column or of an expression over the group's columns that Polars can do;
- a function that gives rows of the grouped sheet: `sort` (one), `head`, `tail`, a condition on one row at a time and `select`, as in `rows |> sort(Units, desc) |> head(2) |> select(Rep, Units)`. The condition comes before any `head` or `tail`;
- a function that gives rows of another sheet picked by equality with the group's key, `Targets[; Region == rows[Region; 0]] |> select(Target)`, as a join.

Any other function, a group's cell that is empty where the body reads it, or an operation that might not fit, is called a group at a time by the engine, with the same answer. A rank within each group is a `rank(... by Key)` column, not an `apply`.

What is always the engine's: a `Fraction`; a column of mixed types; a quantity in an offset unit (`°C`) or a scale (`k.`), or in a column with no declared kind; a quantity operation whose answer has no dimension, as `Width / Height` has; a sum of two dimensions, which is an error there; a divisor of nought, which is an error there; `%` and `**` of a `Scientific`; and a `Complex`. The answer is the same either way, and only the time differs.

## Down a column

A window function gives each row a value read from the rows around it. It
is a method on a column, and in a computed column it is worked out once for
the whole column and each row takes its own:

```omx
Balance := Amount.cum_sum(Date by Account)
```

| Written | Gives |
|---|---|
| `cum_sum() cum_min() cum_max() cum_prod()` | the running total, smallest, largest or product so far |
| `cum_count()` | how many values so far are not empty |
| `diff()`, `diff(2)` | the change from the row 1 (or `n`) back, empty where there is none |
| `pct_change()`, `pct_change(2)` | that change as a share of the row back, a Float |
| `rank()` | each value's place in order; see below |
| `forward_fill()`, `backward_fill()` | an empty cell takes the last value above it, or the next below |
| `rolling_sum(3)` and `rolling_mean rolling_min rolling_max rolling_std` | over the last `n` rows; empty until `n` rows are there, or where one of them is empty |

An empty cell stays empty in `cum_*` and is skipped by it, so the total goes
on. `rolling_std` is the standard deviation of a sample, and the mean of
whole numbers is a Decimal, as `mean` is.

Without anything more, the rows are taken in the order of the file, which
goes wrong without a word when a row was added out of date order. Name an
**order key**, a column of the same sheet, after the number if there is
one, and the rows go down it (rows with the same key keep the file's order),
each answer going back on the row it came from. `by` after it names the
**partition**: a window for each value of the columns, an empty key being a
partition of its own, as in [`[x; *-1 by Region]`](#selecting-with-columns-rows).

```omx
Balance := Amount.cum_sum(Date by Account)
Change  := Amount.diff(1, Date by Account)
Recent  := Amount.rolling_mean(3, Date)
```

`rank` takes words, not a key: how ties are numbered, `average` (the
default, a Float), `min`, `max`, `dense` or `ordinal`, and `desc` to rank
from the largest. `by` partitions it: `Score.rank(min, desc by Region)`.

`by` can stand alone, `Amount.cum_sum(by Account)` and `Score.rank(by
Region)`, for a partition in the order of the file.

Over a whole column, `Sales.Amount.cum_sum(Sales.Date)`, the answer is a
column. A calculation down a column is written to an XLSX Workbook as its
values.

**Speed.** Over whole numbers, floats and decimals a column of these is
calculated by Polars, in one step over the whole table, and the answer is the
one the engine would give. That is `cum_sum cum_min cum_max cum_count diff
rank forward_fill backward_fill`, with `pct_change`, `rolling_min` and
`rolling_max` for whole numbers and floats, `cum_prod` for floats and
`rolling_sum` for whole numbers; the temporal types and text have `cum_min`,
`cum_max`, `cum_count` and the fills. Anything else is the engine's, which
works every type but a row at a time: a Fraction, a number with units, a
rolling mean or deviation, a float rolling sum (Polars slides a window along
and the last digit differs), a rolling window or a ratio of decimals (Polars
has none), a column with a `NaN`, and a total that might pass the largest
whole number or the digits of a decimal. A table of 3.7 million rows takes a couple of seconds
for a running total by group, against more than a minute.

## Sequences with `...`

A computed column may be written as a sequence, `terms ... limit`. The terms
given start the column and the rest follow a rule.

| Written | Gives |
|---|---|
| `1, 3 ... Inf` | 1, 3, 5, 7, … — a constant difference |
| `1, 2, 4 ... Inf` | 1, 2, 4, 8, … — a constant ratio |
| `5 ... Inf` | 5, 6, 7, … — one term alone steps by one |
| `0, 1, $-1 + $-2 ... Inf` | 0, 1, 1, 2, 3, 5, … — a rule of its own |
| `1, 3 ... 9` | 1, 3, 5, 7, 9, then empty |
| `'A', 'B' ... Inf` | A, B, … Z, AA, AB, … |
| `'Monday', 'Tuesday' ... Inf` | the days of the week, round and round |
| `'Jan', 'Feb' ... Inf` | the months of the year |
| `'Jan-2026', 'Apr-2026' ... Inf` | quarters, carrying into the year |
| `'AY' ... 'BA'` | AY, AZ, BA, then empty |
| `2025-01-01, 2025-01-08 ... Inf` | every seventh day |
| `2025-01-01, 2025-02-01 ... Inf` | the first of each month |
| `2025-01-31, 2025-02-28 ... Inf` | the last day of each month |

- `$-1` is the term before this one, `$-2` the one before that. The last
  term before `...` is the rule when it reads one, and another starting
  term when it does not.
- With no rule the step is taken from the terms: their difference, or their
  ratio where three or more share one. Two terms are read as a difference,
  so a constant ratio needs three.
- `Inf` fills the column. Any other limit stops the sequence there and
  leaves the rows after it empty; a sequence that would step over its limit
  stops before it.
- The days of the week and the months of the year are known by name,
  written in full or in three letters, in any case: they go round, so
  Sunday is followed by Monday and December by January. A month written
  with its year, `Jan-2026` or `January 2026`, carries into the year
  instead. Every term must be written the same way, so `'Mon', 'Tuesday'`
  is not one of these.
- Any other text counts up the way letters and digits do, carrying at `Z`
  and `9`, and only its last run of letters and digits moves: `job-A9` to
  `job-B0`. `succ(x)` is that step on its own, and on a number it adds one.
- Dates a fixed number of days apart step by days, and times and
  date-times by seconds. Dates on the same day of each month step by
  months instead, however many days that is, and the day they are counted
  from is the largest one given: `2025-01-31, 2025-02-28 ... Inf` goes on
  to the 31st of March, not back to its 28th. One date alone steps by a
  day.
- A sequence fills a column: it cannot change how many rows the sheet has,
  and `... Inf` has no meaning anywhere else.

## Durations

Next to a date, a plain number is days, and `n.day`, `n.week`, `n.month`
and `n.year` say which: `Placed + 1.month`, `Due - 2.week`. Only a plain
number takes a unit, so `Placed.month` is still the month of a date.

Months and years keep the day of the month, or the last day of a month too
short for it: `2025-01-31 + 1.month` is `2025-02-28`. That rounding does
not come back, so a sequence of month ends is better written with `month_end`,
which gives the last day of a date's month:
`month_end(2025-01-10), month_end($-1 + 1.month) ... Inf`.

## Functions

Called as `sum(x)`, `x.sum()` or `x |> sum()`.

- Vectors: `sum mean min max count len median product first last`, of one
  vector or several taken together, with single values among them:
  `sum([A; 0..3], [A; 7..9], 5)`. A block of cells counts every cell:
  `sum([0..2; 0..3])`. `count` counts what is not empty and `len` counts
  every cell, or a sheet's rows.
- Spread: `std(x)` and `var(x)`, of a sample; `std(x, 0)` and `var(x, 0)` of
  a whole population; `quantile(x, 0.9)`, the value that far through the
  values in order
- Down a column: `cum_sum cum_min cum_max cum_prod cum_count`, `diff`,
  `pct_change`, `rank`, `forward_fill`, `backward_fill` and
  `rolling_sum rolling_mean rolling_min rolling_max rolling_std`, as
  [below](#down-a-column)
- Sheets: `filter(condition)`, `select(columns)`, `head(n)`, `tail(n)`,
  `sort(columns)`, `group_by(keys) |> agg(aggregates)` and `join(Other,
  keys)`, as [above](#sheets-from-sheets); `shift(n)` — the value `n` rows
  back
- Booleans: `a xor b`, `a nand b` and `a nor b` are the gates `and` and `or`
  have no word for, written between two values. They are worked out in `and`, `or` and `not`, so an
  empty value is treated as there: `xor` and `nand` are empty if either is.
  `or` and `nor` bind loosest, then `xor`, then `and` and `nand`. `x.all()`, `x.any()` and
  `x.none()` say whether every, some or no value of a column is true; as
  `sum` and `count` do, they leave the empty ones out, so a column of three
  squares with one empty is `all` true if the other two are. Test
  `x.count() == 3` too where that matters
- Empty or not: `is_null(x)` and `is_not_null(x)`, of a value or of every
  cell of a column; `fill_null(x, 0)`, which is `x // 0`, and
  `is_in(x, [1, 2])`, which is `x in [1, 2]` — these two take one value, as
  the operators they stand for do
- A `NaN` or not: `is_nan(x)` and `is_not_nan(x)`. A `NaN` is a Float and an
  empty cell is a missing value, so they are different absences:
  `is_nan(0/0)` is true, `is_null(0/0)` is false, and `is_nan` of an empty
  cell is empty. `is_finite(x)` and `is_infinite(x)` likewise; an exact
  number is always finite
- Text: `starts_with('A')`, `ends_with('son')`, `contains('an')` — exactly
  as written — `matches('^A.*n$')`, a regular expression anywhere in the
  text, and `contains_any(['an', 'en'])`. `starts_with('A'..'C')` takes a
  range of first letters, which is `x >= 'A' and x < 'D'`. `len_chars()`
  counts characters, and `to_lowercase()` and `to_uppercase()` change case,
  so `Name.to_lowercase().starts_with('a')` ignores it. Each is empty of an
  empty cell
- Showing a value: `fmt('.2f')` gives a value, or each value of a column, as text in a format written as a column's is, `Price.fmt('.2f')`; see *Number formats* in the file format guide
- Chance, as Raku has it: `rand()` is a `Float` from 0 up to, not including, 1 (Raku's `rand`). `Name.roll()` is one value of a column at random, and `Name.roll(3)` a vector of three, each put back after it is taken, so it may come again, as a die rolls; `(1..6).roll()` is a die. `Name.pick()` is one at random and `Name.pick(3)` three that are all different, none coming twice and no more than the column holds, as a hand is dealt; `Name.pick(Name.len())` shuffles it. Each cell draws afresh, and what is drawn is kept until the file is calculated again. `--seed 7` on the command line seeds them, so that a run can be repeated. `rand()` is `RAND()` in an exported workbook; `roll` and `pick` have no formula there
- Among a column's values: `is_unique()`, `is_duplicated()`,
  `is_first_distinct()` and `is_last_distinct()`, of a column or a vector,
  or of this row's cell, `Name.is_duplicated()`, which is answered among the
  column's. An empty cell is a value of its own here
- Maths: `abs round floor ceil sqrt exp ln log10 log2 sin cos tan arcsinh arccosh arctanh` and more, and the constants `pi tau e c gn planck hbar kB NA R qe G me au ly ld`, which a column, constant or sheet of the same name replaces
  `exp`, `ln`, `log10`, `log2` and the trigonometric functions cast their
  argument to a `Float` as `x.Float` does, so `sin(true)` is `sin(1)`,
  and `ln('20')` is `ln(20)`; a date or a duration is an error. These take a `Complex`
  and stay `Complex`; a real argument past the domain gives one (`sqrt(-4)`)
- Complex: `re im conj arg sign`, with `abs` and every function above except `round floor ceil radians degrees` for a `Complex` too
- Counting up: `succ` — a number plus one, or the next text, `AZ` to `BA`
- Converting: `Integer Decimal Percent Float String Boolean Date Time Datetime
  Duration`,
  each also written `cast(x, Integer)`
- Dates: `today() now() year month day weekday quarter week ordinal_day
  hour minute second millisecond microsecond nanosecond date time
  is_leap_year`
- Durations: `total_days total_hours total_minutes total_seconds`, each an
  exact number: `PT1H30M.total_hours()` is `1.5`
- Months: `add_months(x, n)` — what `x + n.month` is — `month_end(x)` and
  `month_start(x)`
- Time zones: `convert_time_zone('Asia/Tokyo') utc() local()
  base_utc_offset() time_zone()`
- Your own: `fn` in the file

The tests are those Polars has, under Polars' names, so a view's column is
tested by Polars in one pass rather than a row at a time.

The app lists them all under `fn()` (F1).

## Dates and times

- A whole number is days next to a `Date`, seconds next to a `Time` or
  `Datetime`: `2025-01-31 + 1` is `2025-02-01`.
- Two of a kind subtract to a `Duration`: `2025-03-01 - 2024-03-01` is
  `P365D`, and `09:00 - 10:30` is `-PT1H30M`.
- A `Duration` adds to and subtracts from a date or a time, `09:00 + PT1H30M`
  is `10:30`; beside a `Date` only its whole days count. Two add up, and one
  is multiplied or divided by a number, `PT1H * 3` is `PT3H`. `sum`, `mean`
  and `median` of them are durations.
- A quantity of time is a `Duration` beside a date, a time or a `Duration`:
  `17:05:00 + 10s` is `17:05:10`, `10s + 17:05:00` is the same, and `PT1H + 30s`
  is `PT1H30S`. Any other unit is an error, `17:05:00 + 5kg`. A distance over a
  `Duration` is a speed, `8610km / PT11H10M`, in `km.s^-1` until it is `.to('m.s^-1')`.
  A `Duration` is itself a quantity of time, so there is nothing to convert
  between them. Two durations over each other are a number, `PT1H / PT30M` is `2`.
  Two durations multiplied, `PT1M * PT1M`, or one raised to the 2nd, 3rd or 4th
  power, `PT1M ** 2`, are a quantity of seconds to that power, `3600s²`; the
  root takes it back, `(PT1M ** 2) ** (1/2)` is `60s`, and so do `1/3` and `1/4`
  for the cube and the fourth power. A quantity of time works wherever a
  `Duration` does. Another power of a duration is an error.
- A duration may be written with a sign and a colon, `+01:30` or `-0:00:30.5`.
  After an operand a `+` or `-` is the operator, so `09:00 + +01:30` is `10:30`,
  and `09:00 +01:30` is two times added. See `examples/durations.omx`.
- `Date + Time` is a `Datetime`.
- Across time zones, comparison and subtraction use the instant.

---

Copyright (c) 2026 Stephen Roe. MIT licence.

# Expressions (OMX)

See [OMX in brief](omx.md) for the grammar. Try any of these with `omasheet eval [file.omx] '<expression>'`.

## Numbers

Arithmetic is exact: `1/3 + 1/6` is `0.5`, `0.1 + 0.2` is `0.3`, and integers
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
they are cut and marked: `2 ** 64` is shown as `1.8446744073709551·e+19`.
Only the display is cut, and `omasheet eval --exact` gives it in full.

A `Decimal`, a `Float` and the parts of a `Complex` are shown to three places
after the point, and where they have more the rest is cut and marked with
`·`: `1/3` is `0.333·` and `sqrt(2)` is `1.414·`. One with no more than three
places is whole, `19.99`, and one below a thousandth keeps its digits, so as
not to show as nought. A `Float` in e notation has its mantissa cut the same way, `1.225·e-16`. A `Scientific` is not cut: the uncertainty in brackets
sets the digits it shows.

A `Decimal` holds 38 digits, so one that does not end within them is
rounded there: `1/3` is held as 38 threes and shown cut and marked,
`0.333·`. Pointing at it in the app shows the rest, and
`--exact` gives every digit of it.

The 38 digits are significant digits, not a size: the point sits where it
needs to, left of them or right of them. So `10**1000` is a number, and an
exact one, held as the single digit it was given; it is only past where the
point itself reaches, around `1e2147483685`, that a number is too large to
hold and that is reported. A number this size is no Polars decimal, whose
point sits within its digits, so a column of them is calculated on the
engine, and a Parquet or XLSX write puts a null in its place and says so.

The last eight of those 38 digits are working room. They are held, so that a
long calculation has room to be right, but they are not compared and not
shown: 30 digits are. That is what makes two ways of reaching one number
reach one answer, since no fixed-point decimal is closed under division —
held in full, a seventh doubled and two sevenths differ in their last digit.
So `1/3 * 3` is `1`, `1/7 * 7` is `1`, `2/7 + 3/7 == 5/7` is true and
`1/7 + 1/7 == 2/7` is true. Thirty digits is far more than a calculation of
any ordinary length will spend; a long enough one can still drift past the
working room, and then it has a last digit of its own.

A `Float` is never rounded: it is shown with the fewest digits that give the
same `Float` back, `0.30000000000000004`, and always fits.

A `Percent` is an exact number shown as so many in a hundred: `20%` is `1/5`.
It is rounded to two digits after the point, `41.67%`, and `--exact` gives
every digit it holds.

Whole numbers and decimals divided give a `Decimal`, whether or not it
ends: `7 / 4` is `1.75` and `7 / 3` is `2.333·`, held
exactly.

Numbers are shown the way Raku shows them:

| Type | Shown |
|---|---|
| `Integer` | `101`; `1e+15` and `1.8446744073709551·e+19` from `1e15` |
| `Decimal` | `1.01`; `1e-43` below `1e-15` |
| `Fraction` | `1/7`, `3/4`, `2 3/4`, `333 1/3`; `3` when it is whole |
| `Percent` | `20%`, `12.5%`, `41.67%` |
| `Float` | `100`, `1.5`, `1.414·`; `1e+21` or `1.5e-16` when very large or small; `Inf`, `-Inf`, `NaN` |
| `Scientific` | the concise form of the physical tables, the uncertainty to two digits in brackets after the value cut at the same place: `7.2973525643(11)e-3`, `11.000(82)`, `1.00(20)e3`; a column format changes it, see below; no uncertainty is a plain number |
| `Complex` | each part as a `Float`: `3+4i`, `1.5-2i`, `0+1.414i·`; both parts cut alike when too wide, and marked once after the `i`; `0+Inf\i` for an infinite imaginary part |

A `Float` that is a whole number looks like an `Integer`, but it is still a `Float`.

A `Decimal`, a `Fraction` and a `Percent` are the same exact numbers shown in
different ways: `19.99`, `1/7` and `20%`. Adding, subtracting, multiplying
and dividing decimals gives a `Decimal`, and so does an average. Each goes
anywhere the others do: a `Decimal` column shows `1/3` as `0.333·`.

A `Fraction` is an exact number shown as a fraction of whole numbers, with a
whole part where there is one: `x.Fraction` gives `1/8` for `0.125` and
`2 3/4` for `2.75`. The fraction is worked out from the number, not
remembered from where it was written, so `(2/7 + 3/7).Fraction` is `5/7`.
Only a denominator of 10000 or less is looked for; past that the number is
shown as a decimal. Two fractions give a fraction, and a fraction with any
other number gives a `Decimal`: in a `Fraction` column, `Yield + Yield` is
still a fraction, where `Yield * 2` is a `Decimal`.
`--exact` writes a fraction as `n/d`, which is how a file carries one.

In a formula `1/10` is a quotient, a `Decimal`. A fraction is written in angle
brackets, `<1/10>`, `<-3/4>` or `<2 3/4>`, and is a `Fraction` as `1/10` is in a
cell, so `<1/10> + <1/10>` is `1/5`. It is the same as `Fraction(1/10)`. A
fraction with a whole number is a `Decimal`, `<1/10> + 1` being `1.1`, so
where the result should stay a fraction use `<1/1>` for one: `<1/10> + <1/1>`
is `1 1/10`. A space goes before a fraction that follows a `+`, since `+<` is
a shift; the error for `1+<1/10>` says so.

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

A quotient of whole numbers is a `Decimal`, though `4/2` written out is an
`Integer`. A quotient is taken where a whole number is wanted, `[1, 2, 3][4/2]`,
and reported there if it is not whole, `5/2`.

A power with a whole exponent is exact; with any other, `2 ** 0.5`, it is a
`Float`, as `sqrt(2)` is.

`+ - * / **`, `== != =~= < > <= >=`, `and or not`, `in`, and `a // b` (b when a
is empty). `if c then a else b`.

Comparisons chain, as in Raku: `0 < x <= 10` is `0 < x and x <= 10`, and
`a < b < c < d` tests each pair in turn. `in` takes one side each.

`==` on two `Float` values is exact, so `sin(pi()) == 0` is false. `a =~= b` is Raku's approximate equality: true when `a` and `b` differ by less than 1e-15 of the larger of them (or by less than 1e-15 where one is zero), so `sin(pi()) =~= 0` and `pi() * 1rad =~= 180°` are true. It takes numbers, `Complex` values and quantities (the right side is read in the unit of the left), ignores any uncertainty, and leaves `==` exact: it is the one comparison that has a tolerance, beside the `Complex` to real test.

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
| `x.Float` | a floating-point number; of a `Complex`, its real part, where its imaginary part is zero to within `1e-15`, and otherwise an error |
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
negative exponent is `m.s^-1`. Only SI is written in a cell: the symbols
`g m s A K mol cd`, the named units `Hz N Pa J W C V F Ω S Wb T H Bq Gy Sv kat`
and `°C`, each with an SI prefix from `q` to `Q`, `kg`, `mm`, `μs`, `GHz`. A
symbol is case-sensitive, and `ºC`, `µ` and the ohm sign are read as `°C`,
`μ` and `Ω`. A unit other than these is not a unit: `5ft`, `5L` and `5min` are
errors, and `5 kg` is a number and a name.

An angle has the radian `rad`, the steradian `sr` (a radian squared) and the degree `°`, the minute `′` and the second `″` of arc, and the lumen `lm` and the lux `lx` that need it. Degrees, minutes and seconds are written together, `45°30′30″` or `-12°30′`, and `º`, `'` and `"` do for the marks; minutes and seconds are under 60 and only the last part may have a point. `45°` is a degree and `20°C` is Celsius. `sin`, `cos` and `tan` take an angle in any unit, and `dms` writes one out: `45.5°.dms()` is `45°30′0″`, and `dms(x, 2)` keeps two places of seconds.

Data has two units, the bit `b` and the byte `B` (eight bits). Each takes the SI
prefixes, where `kB` is 1000 B, and the binary ones `Ki Mi Gi Ti Pi Ei Zi Yi Ri
Qi`, where `KiB` is 1024 B: `4GiB`, `100Mb`. A rate needs no unit of its own,
`MiB.s^-1` or `kb.s^-1`. A binary prefix and a dot on its own is a scale, as
`k.` is: `1Ki.` is 1024 and `1.5Mi.` is 1572864.

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
180°.to('rad')       3.14159rad
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
dimension.

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
which works only where the imaginary part is zero to within `1e-15`, Raku's
tolerance, and is otherwise an error. That tolerance is used nowhere else:
`im(z) = 0` is the exact test.

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
- `join(Other, keys)`: a row for every pair of rows of the two sheets whose
  key columns are equal, the left sheet's columns and then the other's.
  `join(Other, keys, left)` keeps a left row that matches nothing, its other
  columns empty. A key that matches twice gives two rows, an empty key
  matches nothing, and a name both sheets have takes `_right` on the other
  sheet's column. The rows come in the left sheet's order, then the other's

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
- Sheets: `filter(condition)`, `select(columns)`, `head(n)`, `tail(n)`,
  `sort(columns)`, `group_by(keys) |> agg(aggregates)` and `join(Other,
  keys)`, as [above](#sheets-from-sheets); `shift(n)` — the value `n` rows
  back
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
- Among a column's values: `is_unique()`, `is_duplicated()`,
  `is_first_distinct()` and `is_last_distinct()`, of a column or a vector,
  or of this row's cell, `Name.is_duplicated()`, which is answered among the
  column's. An empty cell is a value of its own here
- Maths: `abs round floor ceil sqrt exp ln log10 log2 sin cos tan arcsinh arccosh arctanh pi() e()` and more
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
  `d.Quantity` is the quantity of seconds a `Duration` stands for, `PT90M.Quantity`
  is `5400s`, and `q.Duration` is the `Duration` of a quantity of time, `5400s.Duration`
  is `PT1H30M`; any other unit is an error. See `examples/durations.omx`.
- `Date + Time` is a `Datetime`.
- Across time zones, comparison and subtraction use the instant.

---

Copyright (c) 2026 Stephen Roe. MIT licence.

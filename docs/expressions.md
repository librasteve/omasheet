# Expressions (OMX)

Try any of these with `omasheet eval [file.omx] '<expression>'`.

## Numbers

Arithmetic is exact: `1/3 + 1/6` is `0.5`, `0.1 + 0.2` is `0.3`, and integers
do not overflow. `x.Num` gives a floating-point `Num`.

No exact number is a division by zero, so that gives a `Num`: `1/0` is `Inf`,
`-1/0` is `-Inf` and `0/0` is `NaN`. So does a function outside its
domain: `sqrt(-1)` is `NaN` and `ln(0)` is `-Inf`. They are written as they are shown,
`Inf`, `-Inf` and `NaN`, in a formula or a cell; the text is in quotes,
`"Inf"`. A `NaN` is not equal to anything, itself included: of the
comparisons only `!=` is true of one.

A number is shown in at most 23 characters, and a `-` before it. From
`1e-15` up to `1e15` it is plain, `0.015625`; smaller or larger it is in e
notation, `1e-43` and `1e+60`, and so is one too wide to be plain. An exact
number in e notation keeps every digit it can, and where they do not all fit
they are cut and marked: `2 ** 64` is shown as `1.8446744073709551…e+19`.
Only the display is cut, and `omasheet eval --exact` gives it in full.

A `Decimal` that does not end is cut and marked the same way: `1/3` is
shown as `0.33333333333333333333…`. Pointing at it in the app shows more
places, and `--exact` gives it as a fraction, `1/3`.

A `Num` is never rounded: it is shown with the fewest digits that give the
same `Num` back, `0.30000000000000004`, and always fits.

A `Percent` is an exact number shown as so many in a hundred: `20%` is `1/5`.
It is rounded to two digits after the point, `41.67%`, and `--exact` gives it
in full, as a fraction where it does not end, `125/3%`.

A `Fraction` is shown as a fraction where it fits: `1/3`, and one of more
than one as a whole number and a part, `11/4` as `2 3/4`. One too wide is
shown in e notation, cut and marked. `--exact` and `x.Text` give it as it is
written in the file, `11/4`.

Whole numbers and decimals divided give a `Decimal`, whether or not it
ends: `7 / 4` is `1.75` and `7 / 3` is `2.33333333333333333333…`, held
exactly. Anything worked out from a `Fraction` is a `Fraction`:
`x.Fraction * 2`. To show a result as a fraction, declare its column
`Fraction` or write `x.Fraction`; for a decimal, write `x.Decimal`.

Numbers are shown the way Raku shows them:

| Type | Shown |
|---|---|
| `Int` | `101`; `1e+15` and `1.8446744073709551…e+19` from `1e15` |
| `Decimal` | `1.01`; `1e-43` below `1e-15` |
| `Fraction` | `1/3`, `2 3/4`; `1.6129030457865418…e-01` when too wide |
| `Percent` | `20%`, `12.5%`, `41.67%` |
| `Num` | `100`, `1.5`, `1.4142135623730951`; `1e+21` or `1.5e-16` when very large or small; `Inf`, `-Inf`, `NaN` |
| `Complex` | each part as a `Num`: `3+4i`, `1.5-2i`, `0+1.4142135623730951i`; both parts cut alike when too wide |

A `Num` that is a whole number looks like an `Int`, but it is still a `Num`.

A `Decimal`, a `Fraction` and a `Percent` are the same exact numbers shown
in different ways: `19.99`, `2 3/4` and `20%`. Adding, subtracting,
multiplying and dividing decimals gives a `Decimal`, and so does an average.
Each goes anywhere another does: a `Decimal` column shows `1/3` as
`0.333…`. A column of no declared type with a decimal in it that does not
end is a `Fraction` column, and shows every number in it as a fraction.

Percentages add up to a percentage:
`20% + 5%` is `25%`, and so are their sum, average, least and greatest.
Anything else done with one gives the plain number: `100 * 20%` is `20`,
`1 + 20%` is `1.2`, and `20% == 0.2` is true. To show a result as a
percentage, declare its column `Percent` or write `x.Percent`.

`+ - * / **`, `== != < > <= >=`, `and or not`, `in`, and `a // b` (b when a
is empty). `if c then a else b`.

## Converting

The name of a type converts to it, written `x.Int`, `x.Int()` or `Int(x)`. A
vector is converted value by value.

| Written | Gives |
|---|---|
| `x.Int` | a whole number: `19.99` gives `19`, `-19.99` gives `-19` |
| `x.Decimal` | the same number as a decimal: `(1/8).Decimal` is `0.125`, `(1/3).Decimal` is `0.333…` |
| `x.Fraction` | the same number as a fraction: `2.75` gives `2 3/4`; a `Num` as the decimal it is shown as |
| `x.Percent` | the same number as a percentage: `0.4` gives `40%`, `5/12` gives `41.67%` |
| `x.Num` | a floating-point number |
| `x.Complex` | a complex number from a real one |
| `x.Text` | any value as text, written the way a file writes it |
| `x.Bool` | `false` for zero, `true` for any other number |
| `x.Date`, `x.Time` | that half of a `DateTime` |
| `x.DateTime` | a `Date` at midnight |

Text is read the way a cell is: `"42".Int` is `42`, `"20%".Fraction` is `1/5`,
`"2025-01-31".Date` is that date, and `"abc".Int` is an error. `true.Int` is
`1`. To round rather than drop the fraction, use `round(x)`; `round(x, 0.01)`
rounds to the nearest 0.01, and `round(x, 100)` to the nearest hundred.

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

Inside its own sheet the name can be dropped: `[Revenue; *-1]`. A row
outside the sheet is empty, so `[Revenue; *-1] // 0` starts a running value.

## Functions

Called as `sum(x)`, `x.sum()` or `x |> sum()`.

- Vectors: `sum avg min max count`, of one vector or several taken together,
  with single values among them: `sum([A; 0..3], [A; 7..9], 5)`. A block of
  cells counts every cell: `sum([0..2; 0..3])`.
- Sheets: `filter(condition)`, `select(columns)`
- Maths: `abs round floor ceil sqrt exp ln log10 log2 sin cos tan pi() e()` and more
- Converting: `Int Decimal Fraction Percent Num Complex Text Bool Date Time DateTime`
- Dates: `today() now() year month day weekday hour minute second date time`
- Time zones: `to_zone("Asia/Tokyo") utc() local() offset() zone()`
- Your own: `fn` in the file

The app lists them all under `fn()` (F1).

## Dates and times

- A whole number is days next to a `Date`, seconds next to a `Time` or
  `DateTime`: `2025-01-31 + 1` is `2025-02-01`.
- Two of a kind subtract to a number: `2025-03-01 - 2024-03-01` is `365`.
- `Date + Time` is a `DateTime`.
- Across time zones, comparison and subtraction use the instant.

---

Copyright (c) 2026 Stephen Roe. MIT licence.

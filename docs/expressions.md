# Expressions (OMX)

Try any of these with `omasheet eval [sheet.omx] '<expression>'`.

## Numbers

Arithmetic is exact: `1/3 + 1/6` is `0.5`, `0.1 + 0.2` is `0.3`, and integers
do not overflow. `x.Num` gives a floating-point `Num`, shown with an
exponent (`3.333333333333333e-1`).

An exact number is shown as a decimal with up to five digits after the point.
One with more is rounded and marked: `1/3` is shown as `0.33333…`. Only the
display is rounded: `1/3 * 3` is still `1`. `omasheet eval --exact` and
`x.Text` give it in full, `1/3`.

`+ - * / **`, `== != < > <= >=`, `and or not`, `in`, and `a // b` (b when a
is empty). `if c then a else b`.

## Converting

The name of a type converts to it, written `x.Int`, `x.Int()` or `Int(x)`. A
vector is converted value by value.

| Written | Gives |
|---|---|
| `x.Int` | a whole number: `19.99` gives `19`, `-19.99` gives `-19` |
| `x.Ratio` | an exact number: a `Num` as the decimal it is shown as |
| `x.Num` | a floating-point number |
| `x.Complex` | a complex number from a real one |
| `x.Text` | any value as text, written the way a sheet writes it |
| `x.Bool` | `false` for zero, `true` for any other number |
| `x.Date`, `x.Time` | that half of a `DateTime` |
| `x.DateTime` | a `Date` at midnight |

Text is read the way a cell is: `"42".Int` is `42`, `"20%".Ratio` is `0.2`,
`"2025-01-31".Date` is that date, and `"abc".Int` is an error. `true.Int` is
`1`. To round rather than drop the fraction, use `round(x)`.

## Names

| Written | Means |
|---|---|
| `Sales` | the table |
| `Sales.Revenue` | the column, as a vector |
| `Revenue` | inside the table: this row's value |
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

Inside its own table the name can be dropped: `[Revenue; *-1]`. A row
outside the table is empty, so `[Revenue; *-1] // 0` starts a running value.

## Functions

Called as `sum(x)`, `x.sum()` or `x |> sum()`.

- Vectors: `sum avg min max count`
- Tables: `filter(condition)`, `select(columns)`
- Maths: `abs round floor ceil sqrt exp ln log10 log2 sin cos tan pi() e()` and more
- Converting: `Int Ratio Num Complex Text Bool Date Time DateTime`
- Dates: `today() now() year month day weekday hour minute second date time`
- Time zones: `to_zone("Asia/Tokyo") utc() local() offset() zone()`
- Your own: `fn` in the sheet

The app lists them all under `fn()` (F1).

## Dates and times

- A whole number is days next to a `Date`, seconds next to a `Time` or
  `DateTime`: `2025-01-31 + 1` is `2025-02-01`.
- Two of a kind subtract to a number: `2025-03-01 - 2024-03-01` is `365`.
- `Date + Time` is a `DateTime`.
- Across time zones, comparison and subtraction use the instant.

---

Copyright (c) 2026 Stephen Roe. MIT licence.

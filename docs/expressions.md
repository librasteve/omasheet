# Expressions (OMX)

Try any of these with `omasheet eval [sheet.omx] '<expression>'`.

## Numbers

Arithmetic is exact: `1/3 + 1/6` is `0.5`, `0.1 + 0.2` is `0.3`, and integers
do not overflow. `approx(x)` gives a floating-point `Num`, shown with an
exponent (`3.333333333333333e-1`).

`+ - * / **`, `== != < > <= >=`, `and or not`, `in`, and `a // b` (b when a
is empty). `if c then a else b`.

## Names

| Written | Means |
|---|---|
| `Sales` | the table |
| `Sales.Revenue` | the column, as a vector |
| `Revenue` | inside the table: this row's value |
| `TaxRate` | a constant |

## Selecting with `[rows; columns]`

| Written | Means |
|---|---|
| `Sales[1; Revenue]` | one cell; positions count from 0 |
| `Sales[-1; Revenue]` | the last row |
| `Sales[0..2]` | rows 0 to 2; `0..^2` leaves out the end |
| `Sales[; Revenue]` | an empty slot is all of it |
| `Sales[Revenue > 11000]` | the rows where it is true |
| `Sales[*-1; Revenue]` | the row before this one; `*` is this row |
| `Sales[0..*; Revenue]` | from the top down to this row |

Inside its own table the name can be dropped: `[*-1; Revenue]`. A row
outside the table is empty, so `[*-1; Revenue] // 0` starts a running value.

## Functions

Called as `sum(x)`, `x.sum()` or `x |> sum()`.

- Vectors: `sum avg min max count`
- Tables: `filter(condition)`, `select(columns)`
- Maths: `abs round floor ceil sqrt exp ln log10 log2 sin cos tan pi() e()` and more
- Dates: `today() now() year month day weekday hour minute second date time`
- Time zones: `to_zone("Asia/Tokyo") utc() local() offset() zone()`
- Your own: `func` in the sheet

The app lists them all under `f(x)` (F1).

## Dates and times

- A whole number is days next to a `Date`, seconds next to a `Time` or
  `DateTime`: `2025-01-31 + 1` is `2025-02-01`.
- Two of a kind subtract to a number: `2025-03-01 - 2024-03-01` is `365`.
- `Date + Time` is a `DateTime`.
- Across time zones, comparison and subtraction use the instant.

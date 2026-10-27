# Expressions (OMX)

Try any of these with `omasheet eval [file.omx] '<expression>'`.

## Numbers

Arithmetic is exact: `1/3 + 1/6` is `0.5`, `0.1 + 0.2` is `0.3`, and integers
do not overflow. `x.Float` gives a floating-point `Float`.

No exact number is a division by zero, so that gives a `Float`: `1/0` is `Inf`,
`-1/0` is `-Inf` and `0/0` is `NaN`. So does a function outside its
domain: `sqrt(-1)` is `NaN` and `ln(0)` is `-Inf`. They are written as they are shown,
`Inf`, `-Inf` and `NaN`, in a formula or a cell; the text is in quotes,
`"Inf"`. A `NaN` is ordered greatest and equal to itself, so `NaN == NaN` is
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
they are cut and marked: `2 ** 64` is shown as `1.8446744073709551…e+19`.
Only the display is cut, and `omasheet eval --exact` gives it in full.

A `Decimal` holds 38 digits, so one that does not end within them is
rounded there: `1/3` is held as 38 threes and shown cut and marked,
`0.33333333333333333333…`. Pointing at it in the app shows the rest, and
`--exact` gives every digit of it.

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
ends: `7 / 4` is `1.75` and `7 / 3` is `2.33333333333333333333…`, held
exactly.

Numbers are shown the way Raku shows them:

| Type | Shown |
|---|---|
| `Int` | `101`; `1e+15` and `1.8446744073709551…e+19` from `1e15` |
| `Decimal` | `1.01`; `1e-43` below `1e-15` |
| `Fraction` | `1/7`, `3/4`, `2 3/4`, `333 1/3`; `3` when it is whole |
| `Percent` | `20%`, `12.5%`, `41.67%` |
| `Float` | `100`, `1.5`, `1.4142135623730951`; `1e+21` or `1.5e-16` when very large or small; `Inf`, `-Inf`, `NaN` |

A `Float` that is a whole number looks like an `Int`, but it is still a `Float`.

A `Decimal`, a `Fraction` and a `Percent` are the same exact numbers shown in
different ways: `19.99`, `1/7` and `20%`. Adding, subtracting, multiplying
and dividing decimals gives a `Decimal`, and so does an average. Each goes
anywhere the others do: a `Decimal` column shows `1/3` as `0.333…`.

A `Fraction` is an exact number shown as a fraction of whole numbers, with a
whole part where there is one: `x.Fraction` gives `1/8` for `0.125` and
`2 3/4` for `2.75`. The fraction is worked out from the number, not
remembered from where it was written, so `(2/7 + 3/7).Fraction` is `5/7`.
Only a denominator of 10000 or less is looked for; past that the number is
shown as a decimal. Anything done with a fraction gives a fraction, and
`--exact` writes it as `n/d`, which is how a file carries one.

Percentages add up to a percentage:
`20% + 5%` is `25%`, and so are their sum, average, least and greatest.
Anything else done with one gives the plain number: `100 * 20%` is `20`,
`1 + 20%` is `1.2`, and `20% == 0.2` is true. To show a result as a
percentage, declare its column `Percent` or write `x.Percent`.

`+ - * / **`, `== != < > <= >=`, `and or not`, `in`, and `a // b` (b when a
is empty). `if c then a else b`.

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

## Converting

The name of a type converts to it, written `x.Int`, `x.Int()` or `Int(x)`. A
vector is converted value by value.

| Written | Gives |
|---|---|
| `x.Int` | a whole number: `19.99` gives `19`, `-19.99` gives `-19` |
| `x.Decimal` | the same number as a decimal: `(1/8).Decimal` is `0.125`, `(1/3).Decimal` is `0.333…` |
| `x.Percent` | the same number as a percentage: `0.4` gives `40%`, `5/12` gives `41.67%` |
| `x.Float` | a floating-point number |
| `x.String` | any value as text, written the way a file writes it |
| `x.Boolean` | `false` for zero, `true` for any other number |
| `x.Date`, `x.Time` | that half of a `Datetime` |
| `x.Datetime` | a `Date` at midnight |

Text is read the way a cell is: `"42".Int` is `42`, `"20%".Decimal` is `1/5`,
`"2025-01-31".Date` is that date, and `"abc".Int` is an error. `true.Int` is
`1`. To round rather than drop the fraction, use `round(x)`; `round(x, 2)`
rounds to two decimal places. For the nearest hundred, divide, round and
multiply back: `round(x / 100) * 100`.

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
| `"A", "B" ... Inf` | A, B, … Z, AA, AB, … |
| `"Monday", "Tuesday" ... Inf` | the days of the week, round and round |
| `"Jan", "Feb" ... Inf` | the months of the year |
| `"Jan-2026", "Apr-2026" ... Inf` | quarters, carrying into the year |
| `"AY" ... "BA"` | AY, AZ, BA, then empty |
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
  instead. Every term must be written the same way, so `"Mon", "Tuesday"`
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
  `shift(n)` — the value `n` rows back
- Empty or not: `is_null(x)` and `is_not_null(x)`, of a value or of every
  cell of a column; `fill_null(x, 0)`, which is `x // 0`, and
  `is_in(x, [1, 2])`, which is `x in [1, 2]` — these two take one value, as
  the operators they stand for do
- A `NaN` or not: `is_nan(x)` and `is_not_nan(x)`. A `NaN` is a Float and an
  empty cell is a missing value, so they are different absences:
  `is_nan(0/0)` is true, `is_null(0/0)` is false, and `is_nan` of an empty
  cell is empty
- Maths: `abs round floor ceil sqrt exp ln log10 log2 sin cos tan pi() e()` and more
- Counting up: `succ` — a number plus one, or the next text, `AZ` to `BA`
- Converting: `Int Decimal Percent Float String Boolean Date Time Datetime`,
  each also written `cast(x, Int)`
- Dates: `today() now() year month day weekday quarter week ordinal_day
  hour minute second date time`
- Months: `add_months(x, n)` — what `x + n.month` is — `month_end(x)` and
  `month_start(x)`
- Time zones: `convert_time_zone("Asia/Tokyo") utc() local()
  base_utc_offset() time_zone()`
- Your own: `fn` in the file

The app lists them all under `fn()` (F1).

## Dates and times

- A whole number is days next to a `Date`, seconds next to a `Time` or
  `Datetime`: `2025-01-31 + 1` is `2025-02-01`.
- Two of a kind subtract to a number: `2025-03-01 - 2024-03-01` is `365`.
- `Date + Time` is a `Datetime`.
- Across time zones, comparison and subtraction use the instant.

---

Copyright (c) 2026 Stephen Roe. MIT licence.

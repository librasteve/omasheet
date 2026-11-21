# Reference: functions and constants

An index of what the language has, by what it is for. Each group links to where it is explained. A function is called as `sum(x)`, `x.sum()` or `x |> sum()`, and the app lists them all under `fn()` (F1).

## Functions

| For | Functions |
|---|---|
| Vectors | `sum mean min max count len median product first last` |
| Spread | `std var quantile`, of a sample, or of a whole population with a second argument of `0` |
| Down a column | `cum_sum cum_min cum_max cum_prod cum_count diff pct_change rank forward_fill backward_fill rolling_sum rolling_mean rolling_min rolling_max rolling_std`, and `shift`. See [Windowed columns](windowed.md) |
| Sheets | `filter select head tail sort group_by agg apply join`. See [Queries on Polars](queries.md) |
| Booleans | `and or not xor nand nor`, and `all any none` of a column. See [Booleans and logic](booleans.md) |
| Empty or not | `is_null is_not_null fill_null is_in` |
| `NaN` and infinity | `is_nan is_not_nan is_finite is_infinite` |
| Text | `starts_with ends_with contains contains_any matches len_chars to_lowercase to_uppercase` |
| Showing a value | `fmt('.2f')` |
| Chance | `rand roll pick`, repeatable with `--seed` |
| Among a column's values | `is_unique is_duplicated is_first_distinct is_last_distinct` |
| Maths | `abs round floor ceil sqrt exp ln log10 log2 sin cos tan arcsinh arccosh arctanh` and more |
| Complex | `re im conj arg sign` |
| Counting up | `succ` |
| Converting | `Integer Decimal Percent Float String Boolean Date Time Datetime Duration`, each also written `cast(x, Integer)` |
| Dates | `today now year month day weekday quarter week ordinal_day hour minute second millisecond microsecond nanosecond date time is_leap_year` |
| Durations | `total_days total_hours total_minutes total_seconds` |
| Months | `add_months month_end month_start` |
| Time zones | `convert_time_zone utc local base_utc_offset time_zone` |
| Your own | `fn` in the file |

[Expressions](expressions.md#functions) explains each of these, with its rules for empty cells and types.

## Constants

`pi tau e c gn planck hbar kB NA R qe G me au ly ld` are built in, and a column, constant or sheet of the same name replaces one. Their values are in [Constants and uncertainty](constants.md).

## Operators

Operators are in [OMX in brief](omx.md#operators), with the grammar of the language on one page.

## Types

The types and what each reads and shows are in [the file format](format.md#types), and how the number types relate is in [Choosing a number type](number-types.md).

## Units

[Expressions](expressions.md#units) lists every unit, prefix and kind, and [Units and dimensions](units.md) is the tour.

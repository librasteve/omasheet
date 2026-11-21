# Windowed columns and `by`

A window function gives each row a value that is read from the rows around it: a running total, the change since the last row, a rank, a rolling average. It is a method on a column, and in a computed column it is worked out once for the whole column.

```omx
Balance := Amount.cum_sum(Date by Account)
```

## The ledger example

`examples/windows.omx` is a small ledger of six rows, deliberately out of date order:

| Written | Gives |
|---|---|
| `cum_sum() cum_min() cum_max() cum_prod()` | the running total, smallest, largest or product so far |
| `diff()`, `diff(2)` | the change from the row 1 (or `n`) back |
| `pct_change()` | that change as a share of the row back |
| `rank()` | each value's place in order |
| `forward_fill()`, `backward_fill()` | an empty cell takes the last value above it, or the next below |
| `rolling_sum(3)`, `rolling_mean`, `rolling_min`, `rolling_max`, `rolling_std` | over the last `n` rows |

The columns of the example, and what they show:

```omx
InFileOrder := Amount.cum_sum()
Balance     := Amount.cum_sum(Date by Account)
Largest     := Amount.cum_max(Date)
Change      := Amount.diff(1, Date by Account)
Growth      := Amount.pct_change(1, Date by Account).Percent
Place       := Amount.rank(min, desc by Account)
Rolling     := Amount.rolling_mean(2, Date by Account)
Filled      := Price.forward_fill(Date)
```

## Order matters

Without anything more, rows are taken in the order of the file. That goes wrong without a word when a row was added out of date order: in the example, `InFileOrder` is `200, 700, 800, 680, 640, 720` and is wrong as a ledger. Name an **order key**, a column of the same sheet, after the number if there is one, and the rows go down it, each answer going back on the row it came from. `Balance` is the same running total in date order.

## Partitioning with `by`

`by` names the **partition**: a window for each value of the columns. `Amount.cum_sum(Date by Account)` is a separate running total for each account, in date order. `by` can stand alone, `Amount.cum_sum(by Account)`, and `by Region, Month` partitions by both. The rows with an empty key are a partition of their own.

## Rank

`rank` takes words, not a key: how ties are numbered, `average` (the default), `min`, `max`, `dense` or `ordinal`, and `desc` to rank from the largest. `by` partitions it: `Score.rank(min, desc by Region)`.

## Empty cells

An empty cell stays empty in `cum_*` and is skipped by it, so the total goes on. A rolling window is empty until `n` rows are there, or where one of them is empty.

## Speed

Over whole numbers, floats and decimals a window column is calculated by Polars in one step over the whole table, and the answer is the one the engine would give. Anything Polars cannot do, such as a fraction, a number with units, or a rolling mean or deviation, is worked out by the engine a row at a time, with the same answer. A table of 3.7 million rows takes a couple of seconds for a running total by group, against more than a minute. The full rules are in [Expressions](expressions.md#down-a-column).

## Selecting by offset

A neighbouring row is also reachable in a selection: `[Revenue; *-1]` is the row before this one, and `[Revenue; *-1 by Region]` stays inside a region. `shift(n)` gives the value `n` rows back.

# Design: Polars names for OMX

## Context

Source: the design conversation of 2026-10-23. D/P/Q numbers continue from
`add-dataframe-interop`. On the `polars-core` branch, off `polars`.

The decision is one sentence — where OMX and Polars disagree about a name, the
Polars name wins — and almost all of it is mechanical. This document is about
the places it is not: the four names where taking the Polars name changes what
a file computes, and the one rule that decides every future name.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D51 | Decided | A Polars name flattens by dropping its namespace, and takes the namespace's noun as a prefix only where dropping it would collide or mislead: `dt.month_end` → `month_end`, but `str.len_chars` → `len_chars` and `list.len` → `list_len`, not either of them `len` | Owner decision, 2026-10-28, adopting the rule as it stood. Polars disambiguates with namespaces and OMX has one flat namespace, so "the Polars name" is under-determined without a rule. Low stakes in this change, `len` being the only name it touches; settled now because it decides every name the `.str` namespace would later bring, which is the largest block Polars has to offer, and because the three kinds of text just added (`Url`, `Email`, `Uuid`) arrive in a tree with no string function in it at all — so the first `upper` or `contains` written is the moment the rule stops being cheap to change |
| D52 | Decided | A cutover: an old name is gone, and `lower_call`'s existing `_` arm reports it unknown. No rename table, no deprecation, no migration help | Owner decision, 2026-10-23, in those words: "nope just cutover". `closest` filters on edit distance ≤ 2 and will not reach `avg` → `mean` (4), so the message is the plain unknown-function one. Pre-1.0 is the moment |
| D53 | Decided | `count` counts values that are not empty; `len` counts rows, empties included. `Sheet.count()` becomes `Sheet.len()` | Polars asks the two questions separately and OMX answered both with one name, which meant `Sales.count()` and `Sales.Revenue.count()` were different questions spelled the same. The split falls on two arms of `Engine::call` that already exist |
| D54 | Decided | `round(x, n)` is to `n` decimal places, as Polars has it. The nearest-multiple form is withdrawn rather than renamed | Owner decision, 2026-10-23. `round(x, 2)` is legal in both languages and means different things in each, so leaving it alone would ship a trap. Withdrawn and not renamed to `round_to` because the owner chose the smaller surface; `round(x / 100) * 100` says it where it is wanted. `Func::RoundTo` is deleted and `Func::RoundDigits` added, leaving `MathFn::Round` one-argument and `MathFn::ALL` untouched |
| D55 | Decided | Of the type names only `DateTime` moves, to `Datetime`. `Int` → `Int64` and `Float` → `Float64` wait for `add-polars-dtypes` section 5 | Owner decision, 2026-10-23. `Decimal`, `String`, `Boolean`, `Date` and `Time` are already the Polars spellings; `Fraction` and `Percent` name no dtype, so the schema vocabulary stays mixed whatever is done. The width tells a reader nothing until there is more than one width, and then it should be learned once, with its siblings |
| D56 | Decided | A `NaN` is ordered greatest, sorts last, and is equal to itself, as Polars has it. `Inf` and `-Inf` keep the IEEE behaviour they already have | Owner decision, 2026-10-23. Raku's rule — only `!=` is true of a NaN — would otherwise have kept every NaN-bearing Float column off the dataframe for the whole of `add-columnar-core`, and made the two arithmetic paths disagree by design. Costs one cell of one golden; see *What the NaN rule costs* below |
| D57 | Decided | `std` and `var` take `ddof` positionally, defaulting to 1 as Polars does | OMX has no keyword arguments. One positional argument is the whole of what Polars' signature needs here |
| D58 | Decided | `fill_null` and `is_in` are added as the function spellings of the operators `//` and `in`, which already mean exactly that | One `lower_call` arm each, building the existing `Ir::Binary`. The alignment is legible without a second implementation |
| D59 | Decided | `MathFn::ALL` is folded into `FuncDoc` as a `math: Option<MathFn>` field: one name table, not two | The two tables duplicate 21 names and an inline test exists only to assert they agree. Worth collapsing while both are open |
| D60 | Decided | A name enters the function directory in the change that implements it | `omasheet-engine/tests/spec.rs` asserts every listed name is callable, so listing `group_by` here would mean listing it as broken. It arrives with its implementation, in `add-sheet-operations-polars` |

## Why `round` is the hard one

Every other rename is a spelling. `round` is a name both languages have, for
two different operations:

```
round(1.2345, 2)    # omasheet today: nearest multiple of 2        -> 2
round(1.2345, 2)    # polars:         two decimal places           -> 1.23
```

So a Polars reader writing `round(x, 2)` gets an answer rather than an error,
and the answer is wrong. That is the only such case in the set, and it is why
the rename is a change of meaning rather than of text.

The two-argument form with a non-integer second argument — `round(x, 0.01)`,
which is how the form was written in practice and in every example — becomes a
diagnostic naming the new meaning. `round(x, 100)` cannot be caught that way:
it stays legal and changes answer. That is the one silent break in this change
and the proposal says so plainly.

One consolation, in `omasheet-interop/src/formula.rs`: the Excel importer
turns `ROUND(x, 2)` into `round(x, 0.01)` today, and under the new meaning it
emits `round(x, 2)` directly. The bridge gets shorter and more faithful.

## What the NaN rule costs

Measured over the corpus rather than estimated. `NaN` and `Inf` appear in one
example, `examples/types.omx`, and nowhere else.

| Expression | Today | D56 |
|---|---|---|
| `NaN == NaN` | `false` | `true` |
| `NaN != NaN` | `true` | `false` |
| `NaN < 1` | `false` | `false` |
| `1 <= NaN` | `false` | `true` |
| `[1, NaN, 3].min()` | `NaN` | `1` |
| `[1, NaN, 3].max()` | `NaN` | `NaN`, with `propagate_nans` |
| sorting | no order | last |

`Inf` and `-Inf` do not appear in that table because nothing about them
changes: `Inf - Inf` is `NaN`, `1 / Inf` is `0`, `-Inf < 1` is true, in both.

So the whole corpus cost is one cell — the `Same` column of `Limits` in
`examples/types.omx`, where `Same := Limit == Limit` reads `false` in the NaN
row and now reads `true` — plus the comment above that sheet, which says "A
NaN is not equal to anything, itself included" and stops being true.

`max` is the one open thread. Polars' *sort* order makes a NaN greatest, but
its `min`/`max` aggregations propagate a NaN only with the `propagate_nans`
feature; without it `[1, NaN, 3].max()` is `3`. Enabling it keeps `max`
answering as OMX does today and leaves `min` as the only aggregate that
changes. The feature is cheap and the agreement is worth more than the byte,
so it goes in with this change — but it is a user-visible answer selected by a
Cargo feature, which is worth knowing.

## Why the names are not all improvements

Three of the renames make the language slightly worse to write, and are taken
anyway because a vocabulary that is 90% aligned is harder to use than one that
is wholly aligned:

- `to_zone` → `convert_time_zone` is three times as long for the same idea.
- `offset` → `base_utc_offset` is longer, and Polars returns a `Duration`
  where OMX returns seconds as an `Int`. The names agree now and the types
  agree when `add-polars-dtypes` section 5 gives OMX a `Duration`.
- `zone` → `time_zone` is the weakest in the set: Polars has no expression for
  it at all, because a zone lives on the dtype rather than on a value, so
  `time_zone` is taken from the dtype's own field name rather than from a
  function.

## Open questions

- ~~D51, the flattening rule (owner).~~ Settled 2026-10-28.
- Whether `quantile` should expose Polars' `interpolation` argument at all
  before there is a way to write a keyword argument. It defaults to `nearest`
  and the positional form `quantile(x, 0.9, nearest)` would need the
  bare-word convention that `add-sheet-operations-polars` introduces for
  `join`, so it waits for that change.

# Proposal: Sheet operations over the columnar core (Phase 3)

## Why

OMX has five pipe stages — `filter`, `select`, `head`, `tail` and `shift` — and
they compose with every aggregate, so a dataframe-backed table already answers
a question like *the mean fare of the passengers over seventy*:

```
Passengers |> filter(Age > 70) |> select(Name, Age, Fare)
Passengers.Fare.mean()
```

What it cannot answer is *the mean fare **by class***, because there is no
`group_by`. It cannot put an answer in an order of its own, because there is no
`sort`. And it cannot bring two sheets together, because there is no join.
Those three are the whole of this change, with partitioned cursor offsets
behind them.

`add-sheet-operations` planned the same three on a kernel of omasheet's own and
is superseded, nothing of it built. The decision of 2026-10-23 put the
dataframe at the centre and three changes carry it: `align-polars-vocabulary`
settled the names these stages would be written in — deliberately, *"before
`group_by`, `join` and `sort` are written on top of them rather than renamed
afterwards"* — and `add-columnar-core` holds a sheet's data as Polars columns
and lowers to Polars every column it can. This change is Phase 3 on that core,
and the operations are Polars' own.

Two other changes point here rather than carrying it themselves, and this one
is where the overlap is settled: `add-dataframe-interop`'s section 4 moved its
lowering to `add-columnar-core`, and `add-polars-dtypes` 9.12 holds the
exactness rule that a pushed `group_by`, join or `unique` must obey.

## What Changes

- **file-format**, **omx-expressions**, **interactive-app** — ADDED: a sheet
  computed from an expression, `sheet Top := Sales |> head(3)` (D90). It comes
  first, because every pipe stage already gives a sheet and a file has nowhere
  to put one: `filter`, `select`, `head` and `tail` are each invisible in a
  file today, and the three stages below would be too. The app shows it as a
  read-only tab and its **New Sheet** dialog gains the formula field the
  new-computed-column dialog has.
- **omx-expressions** — ADDED: the `group_by` and `agg` pipe stages; `sort`;
  `join`; partitioned cursor offsets `[*±n by Column]`. Each name enters the
  function directory with its implementation, as D60 requires.
- **evaluation** — ADDED: group, join and sort over the columnar core, lowered
  to Polars where the second gate allows and answered on the row path where it
  does not. Exactness is preserved through all three: a value is rounded to
  `SHOWN_PRECISION` before a pushed group or join, or two cells that compare
  equal in the engine land in different groups in Polars
  (`add-polars-dtypes` 9.12).
- **file-format** — ADDED: nothing of the text changes. A grouped or joined
  sheet is a view, as a filtered one is, and is not written back.
- **Examples** — ADDED: a worked example lands **with each stage**, not in a
  batch at the end (owner, 2026-10-28), each with its `.expected` beside it so
  the golden corpus covers the stage from the commit that adds it.

## Non-goals

- **Recalculating only the dependents of a change.** Section 3 of the
  superseded change; it had no consumer then and has none now.
- **A `group_by` that writes rows back into the file.** A view is a view.
- **`add-units`, `add-uncertainty` and `hold-less-of-a-big-file`**, which all
  wait for this change to finish (owner, 2026-10-28). Units through group and
  join is a Phase 4 delta on this one, and `hold-less-of-a-big-file` cannot
  measure what a big Parquet costs through a group until the group exists.

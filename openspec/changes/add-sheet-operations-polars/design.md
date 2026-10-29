# Design: Sheet operations over the columnar core (Phase 3)

## Context

Source: the roadmap's Phase 3 line and the decision of 2026-10-23 that put the
dataframe at the centre. D/P/Q numbers continue from `add-columnar-core`, which
reached D80. On the `polars-core` branch.

The three operations are Polars' own, so most of this change is surface: what
each stage is spelled, what its result holds, and where the answer is computed.
Two of those are genuinely open, because OMX has no keyword arguments and
Polars leans on them for exactly the two places that matter — `sort`'s
descending flag and `join`'s kind. Those are the questions below.

What is *not* open is the vocabulary. `align-polars-vocabulary` settled the
names first on purpose, and D51 settled the rule for any name not yet written:
drop the namespace, and keep its noun as a prefix only where dropping it would
collide or mislead.

## Decisions

| | Status | Decision | Why |
|---|---|---|---|
| D81 | Decided | Grouping is two stages, not one: `Sales \|> group_by(Region) \|> agg(Revenue.sum())` | The roadmap's own success criterion, and Polars' shape. One stage taking both would have to invent a syntax for "these are keys and these are aggregates", which the two-stage form says with a pipe |
| D82 | Decided | A grouped sheet is a sheet, so `agg` yields the key columns first, then one column per aggregate, and anything downstream treats it as any other sheet | A view, as a filtered sheet is. It keeps `filter`, `select`, `head` and the aggregates working after a group with no new rules. Downstream is by the grouped sheet's name, D96 having made `group_by` and `agg` the whole of a sheet's formula |
| D83 | Decided | An aggregate's column is named for its expression as written — `Revenue.sum()` gives `Revenue_sum` — unless it is named with `:=`: `agg(Orders := Item.len())`. An aggregate with no one column to name it by must be named | Owner decision, 2026-10-29. Polars names it `Revenue` and relies on keyword renaming, which OMX has not got; `:=` is already the mark of a name given to a calculation. Two that come to one name are a diagnostic |
| D84 | Decided | `sort` takes its direction as a bare word in the last place: `sort(Fare)` ascending, `sort(Fare, desc)` descending, `asc` accepted for symmetry. More than one key is allowed and they share the one direction. A column of the sheet named `asc` or `desc` is read as a key, the column being looked for before the word | Owner decision, 2026-10-29. `cast(x, Int)` already takes a bare name as an argument, so the form is the language's own rather than a new one, and it is the convention `join(Other, Key, left)` needs next. A second name would have doubled every stage's entry in the directory; `sort(-Fare)` would have read `-Fare` against what it means everywhere else |
| D85 | Question | The join surface and what a multiple match does: Q9 and Q10 of the superseded change, never answered. Candidates for the kind: a bare word, `join(Other, Key, left)`, or a name per kind, `left_join` | Inherited unanswered. The multiple-match half is the sharper one: a left join on a key that matches twice either yields two rows, which changes a sheet's length under the reader, or is a diagnostic. **Owner to settle**, before 3.1 |
| D86 | Decided | A value is rounded to `SHOWN_PRECISION` before a pushed `group_by`, join or `unique` | `add-polars-dtypes` 9.12, held here because this is where the pushing happens: two cells that compare equal in the engine would otherwise land in different groups in Polars, and a group that depends on which path ran is not a group |
| D87 | Decided | These three stages land in this change and nowhere else | `add-dataframe-interop` section 4 moved its lowering to `add-columnar-core`, and `add-polars-dtypes` 9.12 defers the exactness rule to whoever pushes the group. Without this said, two open changes both look as though they own `group_by` |
| D88 | Decided | A worked example lands with each stage, in the commit that adds it, with its `.expected` beside it | Owner decision, 2026-10-28. The golden corpus is run under all three lowerings and is the only test that compares the two paths against a committed answer, so a stage without an example is a stage whose two paths were never compared on anything but a unit test |
| D90 | Decided | A sheet may be computed from an expression that gives a sheet: `sheet Top := Sales \|> head(3)`. `:=` already marks a computed column; this is the same mark one level up, and the three declarations then read as a set — `sheet Name` has rows, `sheet Name := expr` computes them, `table Name := dataframe[…]` reads them from a file | Every pipe stage already yields a sheet and a `.omx` file has nowhere to put one, so `filter`, `select`, `head` and `tail` are *all* invisible in a file today, and `sort`, `group_by` and `join` would be too. This is the surface they are seen through, not an extra. `table` was rejected for it because its own diagnostic says a table reads from a file, and a new keyword was rejected because `:=` already says computed |
| D91 | Decided | It holds the view's columns, each borrowing the name and declared type of the source column it came from, and it is read-only by the same route a `table` is | `Value::Sheet(View)` is already `{ sheet, rows, cols }` — a source sheet with row and column indices — which is exactly what a pipeline evaluates to. Nothing new is stored; what was missing was a way to name one. Read-only needs no new machinery: `read_only` and `clip.rs`'s refusal already cover a table |
| D92 | Decided | Its row count is filled in after checking, as a table's is, because the checker cannot know it: `SheetTy.rows` is `Some(3)` for `head(3)` and `None` for `filter(…)` | A filtered view's length is a runtime fact. A table already has `nrows` "filled in after checking by whoever can read the file", so the phase exists; a formula sheet joins it, its expression evaluated before the snapshot is built. The alternative — refusing any expression without a static length — would have refused `filter`, which is the one people will reach for first |
| D93 | Provisional | The app's **New Sheet** dialog gains a formula field, echoing the one the new-computed-column dialog has: a name, and an optional expression that makes the sheet a view | The dialog already carries a `formulaBox` for a column, checked when the kind is `computed`, so the shape exists and the sheet kind can reuse it. **Its wording needs the owner's say-so** before it is written, as all dialog text does |
| D94 | Decided | This lands before `sort`, `group_by` and `join` | Without it none of those three can be seen in a file or in the app, and task 6.4 — answering the titanic questions in the app — cannot be done at all |
| D95 | Decided | An empty cell is the smallest thing there is, so it sorts first ascending and last descending. A `NaN` is greatest, by D56 | Owner decision, 2026-10-29. It is Polars' own order — nulls first, and `add-columnar-core`'s agreement table already records `null, 1.0, 3.0, NaN, NaN` as what both paths give — so the lowered sort needs no flag and the two paths agree by construction. The spreadsheet convention of blanks last in both directions was the alternative, and would have made the order something other than the order of the values |
| D96 | Decided | `group_by` and `agg` are the whole of a sheet's formula, and the stages after a group are written over that sheet by name | A grouped sheet's rows are new, where a `View` is a selection of a sheet's own, so they need a sheet to be held by and the formula sheet is it. Its keys are a view of each group's first row and its aggregates are columns evaluated once a group, which is all the machinery there is to add. Stages after `agg` in one formula would need a sheet with no name for them to read: hidden sheets the ~50 places that walk a program's sheets would each have to pass over. Owner decision, 2026-10-29; lifting it later removes a diagnostic and breaks nothing written under it |
| D89 | Decided | `add-units`, `add-uncertainty` and `hold-less-of-a-big-file` all wait until every pipe stage here is done | Owner decision, 2026-10-28. Phase 4 is a delta on this change (units through group and join), Phase 5 builds on Phase 4, and `hold-less-of-a-big-file` is the snagging change that measures what a big Parquet costs — which it cannot do through a group that does not exist |

## Why sort is first

It is the only one of the three that changes no shape. A sorted sheet has the
same columns and the same number of rows as its input, so it needs no new rule
about what the result holds, no naming convention, and no answer to what a
multiple match means. Its one open question is the direction (D84), and the
answer to that is the convention `join` needs too.

`group_by` changes a sheet's shape and needs D82 and D83 settled. `join` needs
D85, which has been open since the superseded change. So the order is sort,
group, join — cheapest first, and each one's convention informs the next.

## What the two paths must agree on

`add-columnar-core` left two gates: `Column::elementwise` decides whether a
column *could* be lowered, and `lower::expr` decides whether it *is*. A stage
here is refused by the second gate until it is written, which costs speed and
never correctness — the row path answers instead. That is the safe first
implementation of each stage, and the differential tests
(`a_column_walked_at_once_holds_what_its_cells_hold`, and the corpus run under
`Never` and `Always`) are what hold the two paths to the same answer.

Which means each stage can land in two commits: the stage on the row path with
its example, then the lowering, with the goldens unchanged across the second.

## Open questions

- D85, the join surface and multiple matches (owner), Q9 and Q10 inherited.
  D84 has settled the bare-word convention it needs: `join(Other, Key, left)`.
- Whether `unique` belongs here. 9.12 names it beside `group_by` and join as a
  thing that must round first, but no task has ever asked for it. Left out
  until something wants it.

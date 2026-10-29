# Tasks: Sheet operations over the columnar core (Phase 3)

The last of the four on the `polars-core` branch, after
`align-polars-vocabulary` (names, complete), `add-columnar-core` (the columnar
core, no work left) and alongside what remains of `add-polars-dtypes`.

Three stages, smallest first: `sort` changes no shape, `group_by` changes the
shape, `join` changes the length. Each one's convention informs the next, and
D84's answer — how a stage takes a bare word — is what `join` needs too.

**An example lands with each stage** (D88), in the commit that adds it, with
its `.expected` beside it. The corpus is the only test that compares the row
path and the lowered path against a committed answer, under `Never`, `Auto`
and `Always`, so a stage with no example is a stage whose two paths were never
compared on anything but a unit test.

Each stage may land in two commits: the stage on the row path with its
example, then its lowering, with the goldens unchanged across the second.
`lower::expr` refusing a stage costs speed and never correctness.

Verification on this machine needs `-j 2` on anything workspace-wide, or
`systemd-oomd` kills the terminal rather than the build.

## 0. Settle the open questions

- [x] 0.1 D84: how `sort` takes its direction. A bare word, `sort(Fare, desc)`;
      a second name, `sort_desc(Fare)`; or a signed column, `sort(-Fare)`. The
      bare-word convention is the one D85 needs as well, so the two are
      cheaper settled together than apart

      **The bare word** (owner, 2026-10-29), in the last place, with `asc`
      accepted beside `desc` and more than one key sharing the one
      direction. `cast(x, Int)` already takes a bare name, so nothing new
      is asked of the language, and `join(Other, Key, left)` is what D85
      now has to spell. A column named `asc` or `desc` is read as a key,
      the column being looked for before the word
- [ ] 0.2 D85: the join surface, and what a multiple match does — Q9 and Q10
      of the superseded change, inherited unanswered. A left join on a key
      that matches twice either yields two rows, which changes a sheet's
      length under the reader, or is a diagnostic
- [ ] 0.3 D83: how an aggregate's column is named. Derived from the
      expression, `Revenue.sum()` → `Revenue_sum`, or a name required per
      aggregate

## 1. A sheet from a formula (`omx-expressions`, `evaluation`, `interactive-app`)

Before the three stages, because it is how any of them is seen: every pipe
stage already gives a sheet and a file has nowhere to put one (D90).

- [x] 1.1 `sheet <Name> := <expression>` parsed. The `sheet` branch of
      `parse_file` takes a name and nothing else today; it gains the `:=`
      tail the `table` branch already has, kept apart from `source` so a
      table still means a table
- [x] 1.2 The expression checked to give `Ty::Sheet`, with a diagnostic
      naming what it gave instead. A column, a number and a range are the
      three a reader will try
- [x] 1.3 The sheet's columns from the view: each borrows the name and the
      declared type of the source column it came from (D91), so `select`
      decides which columns a view has and `filter` does not change them
- [x] 1.4 Its row count filled in after checking, as a table's is (D92):
      `head(3)` knows its length statically and `filter` does not, so the
      count comes from the evaluated view and the snapshot is built after it
- [x] 1.5 Read-only, by the route a `table` takes: no cell edit, no paste
      onto it, and the same refusal text
- [x] 1.6 In the dependency graph at sheet level, which `rows_from` and
      `resolve_rows` already do for a rowless sheet. `sheet A := B |> head(1)`
      against `sheet B := A |> head(1)` is a cycle and must be reported as one
- [x] 1.7 Exported as a table is, its calculated rows written
- [x] 1.8 The app shows it as a tab, read-only, with its expression where a
      table shows its path — which is `add-dataframe-interop` 6.2's
      affordance rather than a second one
- [x] 1.9 The app's **New Sheet** dialog gains the formula field the
      new-computed-column dialog has (D93): a name, and an optional
      expression that makes the sheet a view. `promptDialog` already carries
      a `formulaBox` for the `computed` kind, so the sheet kind reuses it.
      **The wording wants the owner's say-so first**
- [x] 1.10 An example and its `.expected`: `slices.omx` gains the sheet its
      own comment currently says a file cannot hold

      **Two things the tests caught.** A view's columns were first made in
      `finish`, so anything *referring* to a view saw a sheet with no
      columns: `sheet One := Top |> head(1)` gave nothing and a
      self-reference gave no diagnostic at all. They are made in
      `ensure_view` now, and a sheet name resolving ensures the view it
      names, which makes `St::Busy` on re-entry the cycle detector.

      And `resolve_views` needed a fixed point: an engine reads the row
      counts it was built with, so one probe settled a view of a view while
      its source was still zero. It iterates as `resolve_rows` does, a fresh
      engine per pass, stopping when nothing moves — which costs an engine
      per nested view and evaluates each expression twice. Cheap for `head`,
      one pass of the condition for `filter`; worth leaving until something
      measures it.

      A view carries the source's *computed* columns too, which `Best` in
      the example shows: its `Share` and `Growth` come with it.

## 2. `sort` (`omx-expressions`, `evaluation`)

- [x] 2.1 The `sort` stage over a sheet and over a column, with its typing:
      the result is the same columns and the same number of rows, so
      `Ty::Sheet` passes through unchanged

      A sort is a **row selector**, `RowSel::Order { keys, desc }`: a
      permutation of every row rather than a selection of some, which is
      what `rows` already answers in and what a `View`'s `rows` already
      holds. Nothing else about a view changes, so `select`, `head` and the
      aggregates after it needed no new code at all. Over a column the
      selector cannot reach the values — it is given the length of what it
      selects from and not its contents — so `index` sorts a `Vector` in
      its own arm, where the two shapes already part
- [x] 2.2 Ties keep the order they came in, so a sort is stable and two sorts
      on different keys compose into what the reader expects
- [x] 2.3 Where a `NaN` and an empty cell sort. D56 already has a `NaN`
      ordered greatest and equal to itself, as Polars has it; an empty cell
      has no such decision yet and needs one here

      **Smallest** (D95, owner, 2026-10-29), so first ascending and last
      descending, which is Polars' own order and needs no flag when 2.6
      pushes the sort down. `value::sort_order` is `compare` with that one
      rule added, every pair of values having an order where `compare`
      answers nothing for an empty one
- [x] 2.4 `sort` in `FUNCTIONS` with its usage (D60)
- [x] 2.5 An example and its `.expected`: the titanic passengers by fare, which
      is a sort over a dataframe-backed table rather than a sheet of its own
      rows

      `analysis.omx` gains `sheet Dearest`, the five dearest fares aboard:
      `Passengers |> sort(Fare, desc) |> select(Name, Fare) |> head(5)`.
      Three of the five tie at 512.3292 and keep their file order, so the
      golden pins the stability of 2.2 as well
- [x] 2.6 Lowered to Polars' own sort, with the goldens unchanged

      `run_sort`, beside `run_filter` and under the same rule — the whole
      sheet in its own order, since gathering a selection already made is a
      copy of it — and recorded in the same `filters` list, a sort being
      the other way a selection answers positions. Two options earn their
      place: `maintain_order`, which Polars has off by default and which
      the stability of 2.2 needs, and **`nulls_last` set to the direction**.
      Polars' `nulls_last` is an independent flag rather than a consequence
      of `descending`, so a descending sort would otherwise have put the
      empty cells first on the lowered path and last on the row path —
      D95 on one path only. `an_empty_cell_sorts_first_on_both_paths` pins
      it. The sort needs no `lower::expr` at all, the keys being column
      indices and not expressions

## 3. `group_by` and `agg` (`omx-expressions`, `evaluation`)

- [ ] 3.1 The `group_by` stage, taking one or more key columns, and the `agg`
      stage after it taking one or more aggregate expressions (D81)
- [ ] 3.2 The result holds the key columns first, then one column per
      aggregate, and is a sheet like any other (D82), so `filter`, `select`,
      `head` and the aggregates all work after it with no new rules
- [ ] 3.3 The aggregate's column name, once 0.3 is settled
- [ ] 3.4 Which aggregates `agg` takes. Every one that works on a column
      today — `count len sum mean min max std var quantile` — or a stated
      subset, and a diagnostic for an expression that is not an aggregate at
      all, which is the mistake a reader will make first
- [ ] 3.5 A key that is a computed column, and a key of each type: text, a
      whole number, a date. A `Float` key is the one to think about, D56
      having made a `NaN` equal to itself
- [ ] 3.6 `group_by` and `agg` in `FUNCTIONS` (D60)
- [ ] 3.7 An example and its `.expected`: the titanic fares by class, which is
      the roadmap's own question and the one this change exists for
- [ ] 3.8 Lowered to Polars' `group_by`, rounding to `SHOWN_PRECISION` first
      (D86, from `add-polars-dtypes` 9.12), with the goldens unchanged
- [ ] 3.9 A differential test over a grouped column: the row path and the
      lowered path must give the same groups, which is what the rounding is
      for

## 4. `join` (`omx-expressions`, `evaluation`)

- [ ] 4.1 Inner and left join on a key column, once 0.2 is settled
- [ ] 4.2 What a multiple match does, as 0.2 decided, and a test that pins it
      either way
- [ ] 4.3 A column name present in both sheets: renamed, qualified or a
      diagnostic. Whatever is chosen, `select` after a join has to be able to
      name every column the join produced
- [ ] 4.4 A key that is missing on one side, which is what a left join is for
- [ ] 4.5 `join` in `FUNCTIONS` (D60)
- [ ] 4.6 An example and its `.expected`: two sheets of the budget corpus
      joined on month, a join between sheets of a file rather than between
      tables
- [ ] 4.7 Lowered to Polars' join, rounding the key first (D86)

## 5. Partitioned cursor offsets (`omx-expressions`)

- [ ] 5.1 `[*±n by Column]`: the cursor offset within a partition rather than
      the whole sheet, so a running total per region is sayable. Its
      dependency kind is the existing `DepKind::Back` or `Fwd` within the
      partition, which the cycle check already understands
- [ ] 5.2 An example and its `.expected`
- [ ] 5.3 Left on the row path if `lower::expr` has no expression for it,
      which costs speed and not correctness

## 6. Documents

- [ ] 6.1 `docs/expressions.md`: the three stages and the partitioned offset,
      in the Sheets section beside `filter` and `select`
- [ ] 6.2 `docs/format.md`: that a grouped or joined sheet is a view and is
      not written back, beside what it says of a filtered one
- [ ] 6.3 `.claude/skills/omasheet/SKILL.md`: the stages, so a sheet written
      by the skill can use them
- [ ] 6.4 `openspec/project.md`: the Phase 3 row marked done, and the
      Dependencies paragraph for what Phase 4 then builds on

## 7. Verify

- [ ] 7.1 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
      -- -D warnings`, `cargo test --workspace -j 2`
- [ ] 7.2 Every `examples/*.expected` byte-identical under `Never`, `Auto` and
      `Always`, the new ones included
- [ ] 7.3 `how_much_of_the_corpus_lowers` re-measured and its figures updated,
      with what moved written down — the new examples add offered columns, and
      each stage's lowering moves some from refused to lowered
- [ ] 7.4 The titanic questions answered by hand, from the CLI and in the app:
      fares by class, passengers by fare, and a join, each checked against
      what a dataframe tool gives for the same file

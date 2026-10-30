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
- [x] 0.2 D85: the join surface, and what a multiple match does — Q9 and Q10
      of the superseded change, inherited unanswered. A left join on a key
      that matches twice either yields two rows, which changes a sheet's
      length under the reader, or is a diagnostic

      **Polars' defaults** (owner, 2026-10-29): a row for every match, an
      empty key matching nothing, and no promise about the order of the
      rows. Both paths give one order nonetheless, the left sheet's and
      then the right's within a key, Polars asked for it with
      `maintain_order` `left_right`, so the goldens and the differentials
      stay byte for byte
- [x] 0.3 D83: how an aggregate's column is named. Derived from the
      expression, `Revenue.sum()` → `Revenue_sum`, or a name required per
      aggregate

      **Derived, with `Name := …` to name one** (owner, 2026-10-29). Only a
      function of one column has a name to derive; any other aggregate,
      `(Price * Qty).sum()`, must be named, and two that come to the same
      name are a diagnostic saying to name one

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

- [x] 3.1 The `group_by` stage, taking one or more key columns, and the `agg`
      stage after it taking one or more aggregate expressions (D81)

      The two are **the whole of a sheet's formula** (D96): `sheet ByRegion
      := Sales |> group_by(Region) |> agg(Revenue.sum())`. A grouped sheet
      is new rows rather than a selection of a sheet's, and a `View` is
      only ever a selection, so the rows need a sheet to belong to; the
      formula sheet is that sheet. Its key columns are `ColKind::View`
      over the first row of each group, so they needed nothing new, and
      each aggregate is `ColKind::Agg`, evaluated once a group with the
      grouped sheet's column names reading that group's rows
      (`Ir::GroupCol`). Either stage anywhere else is a diagnostic that
      says to write the sheet. The groups are in the order of their keys,
      an empty key first (owner, 2026-10-29), each group's rows in the
      order they came in
- [x] 3.2 The result holds the key columns first, then one column per
      aggregate, and is a sheet like any other (D82), so `filter`, `select`,
      `head` and the aggregates all work after it with no new rules

      By its name, `ByRegion |> sort(Revenue_sum, desc)`, rather than by
      more stages after `agg` in the same formula (D96). The app sorts a
      grouped sheet in the grid, as it does a table, there being no end
      of its formula to add a `sort` to
- [x] 3.3 The aggregate's column name, once 0.3 is settled
- [x] 3.4 Which aggregates `agg` takes. Every one that works on a column
      today — `count len sum mean min max std var quantile` — or a stated
      subset, and a diagnostic for an expression that is not an aggregate at
      all, which is the mistake a reader will make first

      Any expression giving one value a group, so every aggregate there is
      and arithmetic over them. A bare column is the diagnostic "in `agg` a
      column is the rows of one group, so it needs an aggregate", checked
      before the name so that it is the one a reader sees
- [x] 3.5 A key that is a computed column, and a key of each type: text, a
      whole number, a date. A `Float` key is the one to think about, D56
      having made a `NaN` equal to itself

      Two keys are one group where `value::sort_order` ties them, so a
      `NaN` is one group and a decimal is grouped as it is compared, to
      `SHOWN_PRECISION` (D86). The spec tests group by text, a date, two
      keys and the computed `Net`; the example by the computed `Family`
- [x] 3.6 `group_by` and `agg` in `FUNCTIONS` (D60)
- [x] 3.7 An example and its `.expected`: the titanic fares by class, which is
      the roadmap's own question and the one this change exists for

      `analysis.omx` gains `FaresByClass`, `ClassAndSex`, `ByFamily` and
      `Luckiest`, the last a sort of a grouped sheet by name
- [x] 3.8 Lowered to Polars' `group_by`, rounding to `SHOWN_PRECISION` first
      (D86, from `add-polars-dtypes` 9.12), with the goldens unchanged

      `run_group`, beside `run_sort` and under the same rule — the whole
      sheet in its own order — and recorded in the same `filters` list. One
      plan, `with_row_index |> group_by(keys) |> agg([#row, …]) |>
      sort_by_exprs(keys)`, gives the groups and every aggregate of one
      column that `lower::aggregate` takes, each with the guards `run_agg`
      asks of a whole column asked of each group instead: a mean as a sum
      and a count divided by the engine (D72), the extremes that bound a
      sum (D72d), and whether a floating-point `min` has a number (D71a).
      `has_room` and `agg_value` are `run_agg`'s own, taken out so that
      the two cannot come to disagree. An aggregate one group refuses is
      left to the engine for every group, and any other aggregate — a
      median, a conversion, arithmetic — is the engine's over the groups
      Polars made. The groups are sorted with nulls first and
      `maintain_order`, which is D95's order, and each group's rows are put
      back in the order they came in, Polars promising none.

      **D86 refuses where it was to round**: a decimal key of more
      significant digits than `SHOWN_PRECISION` sends the grouping back to
      the engine. Keys of fewer digits are one group exactly where they are
      equal, on both paths, so rounding them would change nothing; and
      rounding a decimal to a number of significant digits is not a Polars
      expression this build has
- [x] 3.9 A differential test over a grouped column: the row path and the
      lowered path must give the same groups, which is what the rounding is
      for

      `the_titanic_groups_alike_on_both_paths`, every column of a grouping
      by class and sex, and a computed key with a sheet sorted from it by
      name; `every_kind_of_key_groups_alike_on_both_paths`, keys of text,
      whole numbers, a float column with a `NaN` and a zero of each sign,
      decimals written to different scales, dates, an empty key in each,
      and two keys together. `asked` now settles the calculated sheets'
      row counts first, as a file opened anywhere else does

## 4. `join` (`omx-expressions`, `evaluation`)

- [x] 4.1 Inner and left join on a key column, once 0.2 is settled

      `sheet Both := Sales |> join(Targets, Month, left)`: D84's bare word
      in the last place, inner when it is left out, and one key or more,
      each a name both sheets have. Like a group it is the whole of a
      sheet's formula (D96), its rows being pairs rather than a selection.
      The left sheet's columns are `ColKind::View` over the left row of
      each pair, and the right's are `ColKind::Joined`, read at the right
      row, empty where a left join kept a row with none. The engine sorts
      the right keys once and finds each left key's run in them, so the
      pairs come in the left sheet's order and then the right's
- [x] 4.2 What a multiple match does, as 0.2 decided, and a test that pins it
      either way

      A row for every match: `join_pairs_the_rows_whose_keys_are_equal`
      pins Feb's two, and an empty key on each side matching neither
- [x] 4.3 A column name present in both sheets: renamed, qualified or a
      diagnostic. Whatever is chosen, `select` after a join has to be able to
      name every column the join produced

      Renamed as Polars renames it: the right one takes `_right`, a key
      appearing once, as the left's. A `_right` name the left has already
      is a diagnostic saying to select before the join
- [x] 4.4 A key that is missing on one side, which is what a left join is for

      `a_left_join_keeps_a_row_that_matches_nothing`: Mar has no target and
      the empty key matches nothing, and both are kept with the right's
      columns empty
- [x] 4.5 `join` in `FUNCTIONS` (D60)
- [x] 4.6 An example and its `.expected`: two sheets of the budget corpus
      joined on month, a join between sheets of a file rather than between
      tables

      `budget.omx` gains `Goals` and `Against`, its sales joined to its
      goals on month, left: Feb twice, Mar with no goal, Apr dropped.
      Not `Targets`, which `report.md` defines beside it
- [x] 4.7 Lowered to Polars' join, rounding the key first (D86)

      `run_join`: both sheets whole and in their own order, the keys of the
      same dtype on each side, `JoinArgs` with `maintain_order`
      `LeftRight` for D85's one order and `nulls_equal` left off, and the
      two row indices read back as the pairs. A decimal key of more
      digits than are compared is refused, as `run_group` refuses one.
      `a_join_pairs_alike_on_both_paths` compares every column both ways,
      inner and left, over keys of every kind joined to themselves —
      a `NaN`, both zeros, decimals of different scales — and over the
      titanic joined to the names of its classes

## 5. Partitioned cursor offsets (`omx-expressions`)

- [x] 5.1 `[*±n by Column]`: the cursor offset within a partition rather than
      the whole sheet, so a running total per region is sayable. Its
      dependency kind is the existing `DepKind::Back` or `Fwd` within the
      partition, which the cycle check already understands

      `[Revenue; *-1 by Region]`, and `by Region, Month` for several keys,
      in any row slot, a sheet named before the index or not. The parser
      reads `by` after a slot's expression, where nothing else can follow
      one, and the checker takes only a cursor offset before it, as
      `RowSel::CursorBy { off, keys }`, with the cursor's own kind and a
      whole-column dependency on each key. The engine keeps a sheet's
      partitions once worked out — rows a sort would tie the keys of, an
      empty key one partition of its own, as an empty key is one group —
      so a column of offsets is a lookup a row rather than a pass. An XLSX
      export writes values for it, a partition's row before being a
      different number of rows back on each row
- [x] 5.2 An example and its `.expected`

      `orders.omx` gains `ToDate := Total + ([ToDate; *-1 by CustomerID]
      // 0)`, what each customer has spent so far: Grace Inc's two orders
      run on to 74.47
- [x] 5.3 Left on the row path if `lower::expr` has no expression for it,
      which costs speed and not correctness

      It reads another row, so the first gate never offers it: the corpus
      figures are unchanged by `ToDate`

## 6. Documents

- [x] 6.1 `docs/expressions.md`: the three stages and the partitioned offset,
      in the Sheets section beside `filter` and `select`
- [x] 6.2 `docs/format.md`: that a grouped or joined sheet is a view and is
      not written back, beside what it says of a filtered one
- [x] 6.3 `.claude/skills/omasheet/SKILL.md`: the stages, so a sheet written
      by the skill can use them
- [x] 6.4 `openspec/project.md`: the Phase 3 row marked done, and the
      Dependencies paragraph for what Phase 4 then builds on

      `format.md` had no section for a sheet calculated from an expression
      at all, so it gains one covering all of them, the filtered and the
      sorted as well as the grouped and the joined. The Phase 3 row says
      built, and archived once 7.4 is done

## 7. Verify

- [x] 7.1 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
      -- -D warnings`, `cargo test --workspace -j 2`
- [x] 7.2 Every `examples/*.expected` byte-identical under `Never`, `Auto` and
      `Always`, the new ones included
- [x] 7.3 `how_much_of_the_corpus_lowers` re-measured and its figures updated,
      with what moved written down — the new examples add offered columns, and
      each stage's lowering moves some from refused to lowered

      Nothing moved: 60 offered, 17 lowered, 42 refused, 1 failed, as after
      `analysis2.omx` went. The harness counts computed columns, and none of
      the three stages makes one — an aggregate, a key and a joined column
      each being a kind of their own, taken by `run_group` and `run_join`
      and counted in `filters` and `aggregates` instead — and the one new
      computed column, `ToDate`, reads another row and is never offered
- [ ] 7.4 The titanic questions answered by hand, from the CLI and in the app:
      fares by class, passengers by fare, and a join, each checked against
      what a dataframe tool gives for the same file

      From the CLI, 2026-10-30, against Python Polars 2.1.0 reading
      `titanic.parquet` itself: all three agree. `FaresByClass` gives the
      means 84.15, 20.66 and 13.71 for Polars' 84.154687, 20.662183 and
      13.707707, the medians 60.29, 14.25 and 8.05 for 60.2875, 14.25 and
      8.05, and the same maxima. `Dearest` gives the same five in the same
      order, the three at 512.3292 in the file's order as a stable sort
      keeps them. `ClassNames`, over the join `Named`, gives the six rows
      Polars' own `join` does, their counts, shares and mean ages the same
      once rounded. The app is still to be looked at.

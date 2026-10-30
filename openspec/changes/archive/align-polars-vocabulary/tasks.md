# Tasks: Polars names for OMX

Names only: nothing here lowers anything to Polars. The first of three on the
`polars-core` branch, before `add-columnar-core` and
`add-sheet-operations-polars`, so that the later two are written against the
names they keep.

No parser change is needed anywhere in this change: `name(a)`, `recv.name(a)`
and `recv |> name(a)` already lower to one `ExprKind::Call`, and no function
name is a lexer keyword.

## 0. Settle the open questions

- [x] 0.1 D51: the namespace flattening rule, which decides every name the
      `.str` namespace would later bring. Settled as it stood (owner,
      2026-10-28): drop the namespace, and keep its noun as a prefix only
      where dropping it would collide or mislead

## 1. The renames (`omx-expressions`)

Each is one entry in `FUNCTIONS` — the `name` *and* the `usage`, which an
inline test requires to contain the name — one or more `lower_call` arms, and
the usage and help strings that embed the name.

- [x] 1.1 `avg` → `mean`, and `Func::Avg` → `Func::Mean`
- [x] 1.2 `asin`, `acos`, `atan` → `arcsin`, `arccos`, `arctan`, in both
      `FUNCTIONS` and `MathFn::ALL`
- [x] 1.3 `eom` → `month_end`
- [x] 1.4 `to_zone` → `convert_time_zone`
- [x] 1.5 `offset` → `base_utc_offset`
- [x] 1.6 `zone` → `time_zone`, leaving the file-level `zone` keyword alone —
      it is a different thing with the same spelling and does not move
- [x] 1.7 Confirm the cutover reads as intended (D52): an old name reports
      `unknown function` and `closest` offers nothing misleading in its place

## 2. `count` and `len` (`omx-expressions`)

- [x] 2.1 `Func::Len`, taking the `Value::Sheet` arm that `Func::Count` has
      today — the one O(1) case, which returns the row count
- [x] 2.2 `count` keeps the arm that counts what is not `Null`, for a vector
      and for a block of cells
- [x] 2.3 `len` over a vector counts its elements, empties included
- [x] 2.4 The `Ty::Sheet` arm of the aggregate dispatch accepts `len` and
      refuses `count` with a message naming `len`

## 3. `round` (`omx-expressions`)

- [x] 3.1 `Func::RoundDigits` for `round(x, n)`, to `n` decimal places
- [x] 3.2 `Func::RoundTo` deleted, with the desugaring that re-synthesised a
      call to find its type
- [x] 3.3 A second argument that is not a non-negative integer literal is a
      diagnostic naming the new meaning, which is what a file written with
      `round(x, 0.01)` gets
- [x] 3.4 `MathFn::Round` stays one-argument, so `MathFn::ALL` and the
      `MathFn` dispatch arm are untouched

## 4. The NaN rule (`numerics`)

- [x] 4.1 `compare` orders a `NaN` greatest rather than returning `None` for
      it, making the order total
- [x] 4.2 The "only `!=` is true of a `NaN`" arm of `scalar_op` is deleted, so
      `equal` follows `compare` and `NaN == NaN` is true
- [x] 4.3 The `NaN`-is-the-answer scan in `min`/`max` is deleted; `max` still
      answers `NaN` because a `NaN` is greatest
- [x] 4.4 ~~`propagate_nans` in the Polars features~~: moved to
      `add-columnar-core`. The feature is
      `propagate_nans = ["polars-lazy?/propagate_nans"]`, so it does nothing
      until `lazy` is on and there is a Polars aggregate to agree with
- [x] 4.5 `is_nan`'s doc comment, which states the rule being replaced
- [x] 4.6 `examples/types.omx`: the `Same` column's explanation, and the
      regenerated `examples/types.expected`
- [x] 4.7 The two assertions in `omasheet-engine/tests/spec.rs`: the
      comparison row and the `!=` of a cell with itself

## 5. The additions (`omx-expressions`)

Aggregates, each one `Func` variant and one arm:

- [x] 5.1 `median`
- [x] 5.2 `std(x)` and `std(x, ddof)`, `var(x)` and `var(x, ddof)`, ddof
      defaulting to 1 (D57)
- [x] 5.3 `quantile(x, q)`, interpolation left at Polars' default
- [x] 5.4 `product`
- [x] 5.5 `first`, `last`

Selections, which desugar and need no `Func`:

- [x] 5.6 `head(n)` → `[; ^n]`, following `filter` and `select` (D32)
- [x] 5.7 `tail(n)` → the last `n` rows, reusing the negative positions
      `Engine::rows` already resolves
- [x] 5.8 `shift(n)` → the cursor offset `[; *-n]`, naming the form that
      `add-columnar-core` will lower to Polars' own `shift`

Elementwise:

- [x] 5.9 `is_null`, `is_not_null`, `is_nan` and `is_not_nan`, the last two
      answering a question OMX could not ask at all: `is_nan` of an empty
      cell is empty, as Polars has it, since a `NaN` and a missing value are
      different kinds of absence. `fill_nan` waits for `fill_null` to widen
      from one value to a column, which is a change to `//`
- [x] 5.10 `fill_null(x, v)` and `is_in(x, v)` as the function spellings of
      `//` and `in` (D58)
- [x] 5.11 `cast(x, T)`, beside the `Int(x)` / `x.Int` spellings that stay
- [x] 5.12 `month_start`, `quarter`, `week`, `ordinal_day`

## 6. `DateTime` → `Datetime` (`file-format`)

- [x] 6.1 `S::from_name` and `Display for S`
- [x] 6.2 The `CONVERT` entry, so `x.Datetime`, `x.Datetime()` and
      `Datetime(x)` are the conversion
- [x] 6.3 The hard-coded type list in the schema-line diagnostic
- [x] 6.4 Every schema line and conversion in `examples/` and `docs/`

## 7. The consumers

- [x] 7.1 `omasheet-interop/src/formula.rs`, which *emits* OMX source when it
      imports an Excel formula: `avg`, the three inverse-trigonometric names
      that stop being the lowercase of Excel's, and `ROUND(x, 2)` →
      `round(x, 2)` (shorter than what it emits today)
- [x] 7.2 `omasheet-interop/src/xlsx/write.rs`, the other direction:
      `Func::Mean` → `AVERAGE`, the `Func::Len` split from `Func::Count`, and
      `Func::RoundDigits` → `ROUND`
- [x] 7.3 `omasheet-ui/src/json.rs`, whose test asserts the first entry of the
      directory by name
- [x] 7.4 `omasheet-ui/src/file.rs`: the footer's summary labels, and the
      entry hints that match on type names
- [x] 7.5 The app's column type menu, which is already stale — it offers
      `Num`, `Complex`, `Text` and `Bool`, none of which exist, and omits
      `Float`, `String` and `Boolean`. Brought into agreement with
      `S::from_name` while it is open
- [x] 7.6 `MathFn::ALL` folded into `FuncDoc` (D59), and the inline test that
      asserted the two tables agree removed with it

## 8. The documents

- [x] 8.1 `docs/expressions.md`: the function list, the conversions, the
      aggregation methods, the NaN sentence
- [x] 8.2 `docs/interop.md`: the Excel mapping tables in both directions
- [x] 8.3 `docs/format.md`: the computed-column examples
- [x] 8.4 `.claude/skills/omasheet/SKILL.md`: the function list in prose and
      the worked examples
- [x] 8.5 The 16 `.omx` and `.md` files under `examples/`, including
      `showcase/`, and their regenerated `.expected`
- [x] 8.6 `openspec/project.md`, where the Phase 3 "Done when" names
      `group`

## 9. Verification

- [x] 9.1 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
      -- -D warnings`, `cargo test --workspace`
- [x] 9.2 The golden corpus: every `.expected` byte-identical except
      `types.expected` (the NaN cell) and any example that used the
      nearest-multiple `round`
- [x] 9.3 The directory test that every listed name is callable still passes,
      which is what catches a rename finished in `FUNCTIONS` and not in
      `lower_call`
- [x] 9.4 A file using each old name reports it unknown, and nothing in the
      tree still spells an old name: `grep -rn` over the workspace for each

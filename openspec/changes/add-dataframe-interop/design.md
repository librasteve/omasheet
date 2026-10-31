# Design: Parquet, and tables backed by a dataframe

## Context

Source: the design conversation of 2026-10-19/20. Builds on `add-interop` and
`add-polars-dtypes`; D/P/Q numbers continue from the latter.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D26 | Decided | Polars is an unconditional dependency: no Cargo feature, no `cfg` branches | Owner decision, 2026-10-19. Reverses the feature gate this change assumed on 2026-10-09 |
| D27 | Decided | `table Sales := dataframe["data/sales.parquet"]` declares a dataframe-backed sheet | Owner decision. Reuses `:=`, which already means "defined by an expression". The string names one file: the `*` in the first example was a placeholder for a file name, not a pattern, and a glob is not wanted (owner, 2026-10-21) |
| D28 | Decided | A `table` is a `Sheet` with a source, and its columns a new `ColKind::Frame` | Keeps the dependency graph, the calculation order and the display unchanged |
| D29 | Decided | The schema of a dataframe-backed table is kept in the `.omx` text and checked against the file's footer on load | A mismatch is a diagnostic, not a silent re-read |
| D30 | Decided | Polars evaluates elementwise column work; row-recursive columns stay on the engine | Owner decision |
| D31 | Decided | Row and column selections are pushed down at `Ir::Index`, not at `Engine::cell` | Per-cell access against Arrow would be correct and pathologically slow |
| D32 | Decided | `.head(n)` desugars to the existing `[; ^n]`; no new `Func`, no evaluator change | Follows `filter` and `select`, which desugar the same way |
| D33 | Decided | A column's type comes from the Parquet schema, never inferred from its values | Such a column arrives already declared |
| D50 | Decided | A table's schema line may carry `from "<name>"`: the column's name in the Parquet, where that is not a name omasheet could write | Owner decision, 2026-10-22. Polars column names are arbitrary strings and omasheet names are identifiers, so without this a file like `titanic.parquet` cannot be read as a table at all. Widening the identifier is not open: a space would make `Siblings Spouses` ambiguous with two names juxtaposed, `Parents/Children` is a division, and a `\|` cannot sit in a Markdown header row. Quoting at each reference (SQL, R) was the alternative; the alias costs one clause on a line that has to exist anyway and leaves the expression grammar untouched. The file's own name stays in the text, so the footer check and the pushdown of section 4 still use it. `from` and not `as` because `as` names the new thing on its right (`SELECT x AS y`) and this line is the other way round; the precedent for this direction is the ES-module `import X from "..."`. The name on the left is formed from the file's own by dropping what a name has no room for and capitalising each word that followed it, so `Parents/Children Aboard` becomes `ParentsChildrenAboard`; the diagnostic for a column no schema line names offers that, because a shortened name such as `Parents` would say something the column does not (owner, 2026-10-22) |
| D49 | Decided | A dataframe-backed table is a read-only view of its Parquet file. The app shows its rows and refuses every edit of them, naming the file and pointing at import | Owner decision, 2026-10-22, settling the editability question. Writeback was already out of scope, and a cell of such a sheet has no span in the text to edit. Import already makes an editable `.omx` from a Parquet file, so the way to edit is a way that exists |
| P34 | Decided | `--meta` writes the zone, the column types, the computed-column formulas, the constants and the functions into Parquet key-value metadata | The payload is what `Program` holds and no format carries. Owner, 2026-10-30. Kept as `.omx` source under the one key `omasheet`, the sheet with no rows, so the same parser reads it back and the import puts the file's rows under its header |

## Why `:=` and not `=>`

The sketch was `table => dataframe[*.parquet] name`. Three things made `:=`
better. `:=` already means "this is defined by an expression" for a computed
column, and a dataframe-backed table is exactly that at sheet scale. It needs
no lexer change, where a bare glob does: `*` is the cursor token and
`.parquet` parses as a field access, so `dataframe[*.parquet]` already parses,
with the wrong meaning. And the name comes first, as it does after `sheet`.

The right-hand side parses today as `Index(Name("dataframe"), [Lit::String])`,
so the checker recognises that shape and lowers it to a source rather than a
selection. This does overload `[]`, which Design Principle 2 reserves for
selection; the owner chose the bracket form knowingly.

## Why the dependency graph already knows what Polars cannot do

A computed column can be lowered to a Polars expression only if it is
elementwise over columns of the same sheet. Anything reading another row — a
running balance, `$-1`, a cursor offset — cannot be, without `shift` or a
cumulative function.

Nothing new is needed to tell them apart. `Checker::order` already records how
each reference reaches its column, as `DepKind::Same`, `Back`, `Fwd` or
`Whole`, and `Group::Cols` already carries a `reverse` flag for a group that
reads later rows. The columns Polars cannot express are exactly those whose
dependencies are not all `Same`. The existing analysis is the selector.

## What fights a dataframe-backed sheet

Worth stating plainly, because each one is a place the change can go wrong.

- **The `.omx` text is the single source of truth.** Every edit rewrites the
  text, and the file is then re-read and re-evaluated with a fresh engine. A
  sheet whose rows live in Parquet has no text to rewrite, and would be
  reloaded on each keystroke if nothing changes.
- **`nrows` is fixed at compile time**, and the engine pre-allocates its whole
  memoisation grid from it. So the row count must be known before evaluation —
  cheap from a Parquet footer, but it means "lazily loaded" cannot mean
  "unknown length".
- **Everything materialises.** A whole-column read clones every value, an
  aggregate collects them, and an elementwise operation allocates a fresh
  vector per step. Pushdown at `Ir::Index` is what keeps `.head(5)` from
  reading the whole file.
- **The engine is single-threaded**, with `RefCell` and `Rc` throughout, so
  Polars' parallelism cannot be driven from inside evaluation.
- **Spans tie every cell to byte offsets in the text**, and the whole editing
  API is span rewriting. An externally-backed column has a declaration line
  but no cell spans.

## Risks

- **The binary and the build.** D26 takes the cost deliberately; it should be
  measured before and after, and the install line timed.
- **Two arithmetic paths.** `add-polars-dtypes` leaves a scalar path in
  `value.rs` and this change adds a Polars one. They must agree, which is why
  the scalar path is specified as "what the Polars kernel would do for a
  one-row series" and property-tested against it.
- **`--meta` is a new format of our own**, inside someone else's. A reader that
  ignores it must still see a usable file.

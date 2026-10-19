# Design: Parquet, and tables backed by a dataframe

## Context

Source: the design conversation of 2026-10-19/20. Builds on `add-interop` and
`add-polars-dtypes`; D/P/Q numbers continue from the latter.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D26 | Decided | Polars is an unconditional dependency: no Cargo feature, no `cfg` branches | Owner decision, 2026-10-19. Reverses the feature gate this change assumed on 2026-10-09 |
| D27 | Decided | `table Sales := dataframe["data/*.parquet"]` declares a dataframe-backed sheet | Owner decision. Reuses `:=`, which already means "defined by an expression" |
| D28 | Decided | A `table` is a `Sheet` with a source, and its columns a new `ColKind::Frame` | Keeps the dependency graph, the calculation order and the display unchanged |
| D29 | Decided | The schema of a dataframe-backed table is kept in the `.omx` text and checked against the file's footer on load | A mismatch is a diagnostic, not a silent re-read |
| D30 | Decided | Polars evaluates elementwise column work; row-recursive columns stay on the engine | Owner decision |
| D31 | Decided | Row and column selections are pushed down at `Ir::Index`, not at `Engine::cell` | Per-cell access against Arrow would be correct and pathologically slow |
| D32 | Decided | `.head(n)` desugars to the existing `[; ^n]`; no new `Func`, no evaluator change | Follows `filter` and `select`, which desugar the same way |
| D33 | Decided | A column's type comes from the Parquet schema, never inferred from its values | Such a column arrives already declared |
| P34 | Provisional | `--meta` writes the zone, the column types, the computed-column formulas, the constants and the functions into Parquet key-value metadata | The payload is what `Program` holds and no format carries |

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

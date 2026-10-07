# Tasks: Omasheet foundation

## 0. Settle the grammar-affecting open questions
- [ ] 0.1 Decide the meaning of bare `*` and the "whole dimension" token (design Q1)
- [ ] 0.2 Decide how text and formula cells are distinguished (design Q4)
- [ ] 0.3 Confirm `//` as the fallback operator (design Q3)
- [ ] 0.4 Decide how exact columns coexist with Polars (design Q2)

## 1. Workspace
- [ ] 1.1 Create Cargo workspace with `omasheet-omx`, `omasheet-engine`, `omasheet-cli`
- [ ] 1.2 Set up CI: fmt, clippy, test
- [ ] 1.3 Add a `examples/` corpus of `.sheet` files used as golden tests

## 2. Numerics (`numerics`)
- [ ] 2.1 `Int` on `BigInt`, `Rat` on `BigRational`, `Num` on `f64`
- [ ] 2.2 Literal lexing: integers, decimals, `_` separators, exponent form, `%`
- [ ] 2.3 Arithmetic and coercion rules; `Int / Int → Rat`
- [ ] 2.4 `approx()` / `Num()` conversions; verify no implicit Rat → Num path
- [ ] 2.5 Rat display: decimal when terminating, fraction otherwise

## 3. OMX core (`omx-expressions`)
- [ ] 3.1 Lexer and expression parser (chumsky) with error recovery
- [ ] 3.2 AST and source spans for diagnostics
- [ ] 3.3 Operators: arithmetic, comparison, `and`/`or`/`not`, `in`, `//`
- [ ] 3.4 `if … then … else`
- [ ] 3.5 Ranges `a..b`, `a..^b`
- [ ] 3.6 Indexing `[ ]`, dimension separator `;`, `.Field` access
- [ ] 3.7 Cursor `*`, `*±n`, cursor-bounded ranges
- [ ] 3.8 Predicate selection `T[cond]`
- [ ] 3.9 Aggregation methods
- [ ] 3.10 Broadcasting for scalar⊗vector and vector⊗vector
- [ ] 3.11 Pipe `|>` with `filter`, `select`, `group`, `sum`

## 4. Sheet format (`sheet-format`)
- [ ] 4.1 Parser for `table`, schema lines, header row, data rows
- [ ] 4.2 `const` declarations
- [ ] 4.3 Computed columns `Name := expr`
- [ ] 4.4 Cell typing rule and diagnostics for ragged rows / unknown columns
- [ ] 4.5 Multiple tables per file and cross-table references

## 5. Evaluation (`evaluation`)
- [ ] 5.1 Name resolution and semantic analysis to typed IR
- [ ] 5.2 Shape checking
- [ ] 5.3 Dependency graph, topological order, cycle detection
- [ ] 5.4 Execution over column storage; out-of-range cursor → empty
- [ ] 5.5 Group and join with exactness preserved

## 6. CLI (`cli`)
- [ ] 6.1 `omasheet eval`
- [ ] 6.2 `omasheet lint`
- [ ] 6.3 `omasheet <file>` read-only rendered view
- [ ] 6.4 Diagnostics with file, line, column and source excerpt

## 7. Units (`units`)
- [ ] 7.1 Unit registry: SI base and common derived units, prefixes
- [ ] 7.2 Exponent-vector dimension algebra
- [ ] 7.3 Unit literals and conversion on addition
- [ ] 7.4 Currency units; `convert()`
- [ ] 7.5 Column unit declarations, inference for computed columns, cell checking
- [ ] 7.6 `unit` declarations
- [ ] 7.7 Display unit separate from stored value

## 8. Uncertainty (`uncertainty`)
- [ ] 8.1 `±` / `+-` literal
- [ ] 8.2 Propagation for `+ - * /` and aggregations
- [ ] 8.3 `Uncertain<…>` column type
- [ ] 8.4 Display forms

## 9. XLSX (`xlsx-interop`)
- [ ] 9.1 `omasheet-xlsx` crate; import via calamine
- [ ] 9.2 Export via rust_xlsxwriter with documented lossy mappings
- [ ] 9.3 `omasheet import` / `omasheet export --xlsx`

## 10. Markdown (`markdown-integration`)
- [ ] 10.1 `{{ expr }}` interpolation
- [ ] 10.2 Fenced `sheet` blocks
- [ ] 10.3 `omasheet render`

## Later (not in this change)
- REPL, LSP server, Neovim plugin, TUI viewer, charts, pivots, formatting syntax

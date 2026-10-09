# Tasks: Omasheet core (Phase 1)

## 0. Settle the grammar-affecting open questions
- [x] 0.1 Decide the meaning of bare `*` and the "whole dimension" token — `*` is always the cursor; empty slot is "all" (design D13)
- [x] 0.2 Decide how text and formula cells are distinguished — leading `=` marks a formula (design D14)
- [x] 0.3 Confirm `//` as the fallback operator (design D15)
- [ ] 0.4 Confirm the provisional choices made while building: `|` inside a cell (design P18) and the comment, string and continuation syntax (design P17)

## 1. Workspace
- [x] 1.1 Create Cargo workspace with `omasheet-omx`, `omasheet-engine`, `omasheet-cli`
- [x] 1.2 Set up CI: fmt, clippy, test
- [x] 1.3 Add a `examples/` corpus of `.omx` files used as golden tests

## 2. Numerics (`numerics`)
- [x] 2.1 `Int` on `BigInt`, `Ratio` on `BigRational`, `Num` on `f64`
- [x] 2.2 Literal lexing: integers, decimals, `_` separators, exponent form, `%`
- [x] 2.3 Arithmetic and coercion rules; `Int / Int → Ratio`
- [x] 2.4 `approx()` / `Num()` conversions; verify no implicit Ratio → Num path
- [x] 2.5 Ratio display: decimal when terminating, fraction otherwise

## 3. OMX core (`omx-expressions`)
- [x] 3.1 Lexer and hand-written expression parser; a syntax error in one declaration does not hide errors in the others
- [x] 3.2 AST and source spans for diagnostics
- [x] 3.3 Operators: arithmetic, comparison, `and`/`or`/`not`, `in`, `//`
- [x] 3.4 `if … then … else`
- [x] 3.5 Ranges `a..b`, `a..^b`
- [x] 3.6 Indexing `[ ]`, dimension separator `;`, empty slot, `.Field` access
- [x] 3.7 Cursor `*`, `*±n`, cursor-bounded ranges; error when there is no row context
- [x] 3.8 Predicate selection `T[; cond]`
- [x] 3.9 Aggregation methods
- [x] 3.10 Broadcasting for scalar⊗vector and vector⊗vector
- [x] 3.11 Pipe `|>` with `filter`, `select`, `sum`

## 4. Sheet format (`sheet-format`)
- [x] 4.1 Parser for `table`, schema lines, header row, data rows
- [x] 4.2 `const` declarations
- [x] 4.3 Computed columns `Name := expr`
- [x] 4.4 Cell rule (`=` formula, otherwise literal) and diagnostics for ragged rows / unknown columns
- [x] 4.5 Multiple tables per file and cross-table references
- [x] 4.6 `fn` definitions, checked at each call, and listed in the function directory

## 5. Evaluation (`evaluation`)
- [x] 5.1 Name resolution and semantic analysis to typed IR
- [x] 5.2 Shape checking
- [x] 5.3 Dependency graph, topological order, cycle detection
- [x] 5.4 Execution over column storage; out-of-range cursor → empty

## 6. CLI (`cli`)
- [x] 6.1 `omasheet eval`
- [x] 6.2 `omasheet lint`
- [x] 6.3 `omasheet <file>` read-only rendered view
- [x] 6.4 Diagnostics with file, line, column and source excerpt

## Later
- Phases 2–5: see the roadmap in `project.md`
- Not yet in any change: REPL, LSP server, Neovim plugin, TUI viewer, charts,
  pivots, formatting syntax

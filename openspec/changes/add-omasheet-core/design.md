# Design: Omasheet core (Phase 1)

## Context

Source: `coinvent-omasheet-format-chatgpt.md`. The conversation explored many
alternatives; this document records where it landed, what is still provisional,
and what does not yet hold together, for the Phase 1 core. Decisions and open
questions that belong to a later phase live in that phase's `design.md`. The
D/P/Q numbers are shared across all five changes, so gaps here are intentional.

## Decisions

### Decided (by the project owner)

| # | Decision | Notes |
|---|----------|-------|
| D1 | Product name **Omasheet**, for the Omarchy platform | Follows Omawrite / Omacalc |
| D2 | Relative row references use the cursor form `Sales[*-1]` | Chosen over `@prev` |
| D3 | OMX is Raku-like: ranges, slices, 2-D arrays | Raku-inspired, not embedded Raku |
| D4 | OMX is a fully custom language compiled to a Rust engine | No embedded Lua/JS/Python/SQL |
| D5 | `;` is the dimension separator inside `[]` | As in Raku: `A[2; 3]` |
| D6 | Numeric design lifted from Raku, centred on rationals | |
| D7 | `Int` is arbitrary precision (BigInt); `Rat` is arbitrary precision (FatRat) | No fixed-width integers at language level |
| D8 | Units and uncertainty are in scope | Later phases — see `add-units`, `add-uncertainty` |
| D9 | Types are expressed over an entire column | Column-typed first, cell-typed second; units per column arrive in `add-units` |
| D10 | XLSX via existing crates | Later phase — see `add-interop`. The dataframe-library half of this decision was withdrawn: see D11 in `add-table-operations` |
| D12 | File extension is `.omx` | Owner decision, 2026-10-07. Replaces the provisional `.sheet`; also seen: `.sd`, `.sheetdown`, `.omasheet` |

### Provisional (proposed, not objected to, or chosen here for coherence)

| # | Choice | Alternative(s) seen |
|---|--------|---------------------|
| P2 | No leading `=` on formulas; no A1 references in authored files | Excel-style `=B2-C2` |
| P3 | Equality is `==`; `=` is binding only | Early examples used `Region="UK"` |
| P4 | Bare column name in a predicate or cell means "this row's value" | `.Revenue`, `*.Revenue` |
| P5 | `.Field` and `; Field` are equivalent: `T[r].C` ≡ `T[r; C]` | Only one of the two |
| P6 | Fallback operator is `//` (Raku defined-or) | `??` — see Q3 |
| P7 | Conditional is `if … then … else …` | `c ? a : b` ternary |
| P8 | Aggregations are methods: `.sum() .avg() .min() .max() .count()` | Function form `sum(x)` |
| P9 | Pipe operator `|>` for multi-step table transforms | Method chaining only |
| P10 | Computed column syntax `Name := expr` | Per-cell repeated formulas |
| P15 | First app is a read-only viewer; editing stays in the text editor | Live spreadsheet editor |

## Architecture

```
 .omx source ──┐
                 ▼
   lexer → parser → AST → semantic analysis (types, shapes)
                                 │
                        typed IR → dependency graph
                                 │
                         execution plan → Rust runtime
                                 │
                      exact scalar/array kernel
```

- File syntax and OMX are separate grammars sharing one AST and one type system.
  The OMX parser does not know whether an expression came from a cell, a
  computed column, `omasheet eval`, or a Markdown `{{ }}`.
- References are resolved at compile time: `Sales[*-1; Revenue]` becomes
  `LoadCell { table_id, row_offset: -1, col_id }`, not a runtime name lookup.
- Storage is column-oriented. MVP may use plain `Vec<T>` per column with
  hand-written slicing; views should not copy.
- Later phases extend this pipeline without replacing it: table operations
  beside the kernel (Phase 2), XLSX and Markdown at the edges (Phase 3), units
  and uncertainty in semantic analysis and the value model (Phases 4 and 5).

## Open Questions

**Q1 — `*` is overloaded three ways.** The conversation uses bare `*` as both
"the current row" (`Sales[*].Revenue`, `Sheet[*; *]` = current cell) and "the
whole dimension" (`A[*; 3]` = all rows of column 3, `A[*; *]` = entire array).
Separately, Raku readers will expect `[*-1]` to mean *last element* (Whatever
= end of dimension), whereas D2 makes it *previous row*, with plain `[-1]` for
last. The spec adopts: `*` is the cursor when the expression has a row context;
with no row context a bare `*` means the whole dimension; `*±n` is always
cursor-relative and is an error without a row context. This needs an explicit
owner decision — a distinct "all" token (e.g. empty slot `A[; 3]`, or `**`)
would remove the ambiguity.

**Q3 — Fallback operator spelling.** `//` and `??` were both floated. In Raku
`//` is defined-or and `??` is half of the ternary `?? !!`. `//` is adopted (P6);
note it then cannot mean integer division or begin a comment.

**Q4 — Text vs formula in a cell.** "No `=` prefix" (P2) means `Jan` (text) and
`Revenue - Cost` (formula) must be told apart by context. Provisional rule in
`sheet-format`: the column's declared type decides — a `Text` column's cells are
literal text; any other declared column's cells are OMX expressions; an
undeclared column's type is inferred from its cells, falling back to `Text`.
Computed columns (`:=`) are the recommended way to write formulas. An explicit
marker for one-off formula cells may still be wanted.

**Q8 — Rat growth.** Long chains of exact operations can grow denominators
without bound. Decided: never convert silently. Not decided: whether to warn
(lint) past a size threshold.

**Q12 — Descending ranges and the full operator set.** `10...1` for descending
ranges, `in` / `∈` membership, `%` as modulo vs percent, and `.map/.filter/
.reduce/.sort/.unique` were all listed without detail. Membership (`in`) is
specified because an example depends on it; the rest are deferred.

**Q13 — Comments and literals not discussed.** No comment syntax, string
escaping rules, date literal grammar beyond `2025-01-01`, or boolean/null
literal spelling was settled (`null` appears once).

## Risks

- **Scope.** Exact numerics plus a new language is already a large surface. The
  roadmap in `project.md` keeps units, uncertainty, table operations and
  XLSX out of this phase so that a useful tool (parse, evaluate, print) exists
  first.
- **Ambiguity debt.** Q1 and Q4 affect the grammar; settle them before the
  parser is written, not after.
- **Later phases reopen the type system.** Units and uncertainty will extend
  column types and static checking. Keep the type representation open to
  parameters (`Rat<unit>`) even though Phase 1 has none.

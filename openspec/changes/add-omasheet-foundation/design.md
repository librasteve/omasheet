# Design: Omasheet foundation

## Context

Source: `coinvent-omasheet-format-chatgpt.md`. The conversation explored many
alternatives; this document records where it landed, what is still provisional,
and what does not yet hold together.

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
| D8 | Units and uncertainty are in scope | Raised by owner; design below is provisional |
| D9 | Units/types are expressed over an entire column | Column-typed first, cell-typed second |
| D10 | Use a Rust dataframe library for group/join; XLSX via existing crates | Polars, calamine, rust_xlsxwriter |

### Provisional (proposed, not objected to, or chosen here for coherence)

| # | Choice | Alternative(s) seen |
|---|--------|---------------------|
| P1 | File extension `.sheet` | `.sd`, `.sheetdown`, `.omasheet` |
| P2 | No leading `=` on formulas; no A1 references in authored files | Excel-style `=B2-C2` |
| P3 | Equality is `==`; `=` is binding only | Early examples used `Region="UK"` |
| P4 | Bare column name in a predicate or cell means "this row's value" | `.Revenue`, `*.Revenue` |
| P5 | `.Field` and `; Field` are equivalent: `T[r].C` ≡ `T[r; C]` | Only one of the two |
| P6 | Fallback operator is `//` (Raku defined-or) | `??` — see Q3 |
| P7 | Conditional is `if … then … else …` | `c ? a : b` ternary |
| P8 | Aggregations are methods: `.sum() .avg() .min() .max() .count()` | Function form `sum(x)` |
| P9 | Pipe operator `|>` for multi-step table transforms | Method chaining only |
| P10 | Computed column syntax `Name := expr` | Per-cell repeated formulas |
| P11 | Unit literals are suffix-attached: `10m`, `5kg`, `12.5USD` | Space-separated `10 m` |
| P12 | Uncertainty literal `10 ± 0.1 m`, ASCII spelling `+-` | `10m(1)` concise form |
| P13 | Independent uncertainties combine in quadrature | Linear (worst-case) sum |
| P14 | Markdown model: `.md` = prose, `.sheet` = computation, linked by `{{ expr }}` | Literate `.sheet` with prose; full notebook |
| P15 | First app is a read-only viewer; editing stays in the text editor | Live spreadsheet editor |
| P16 | Partitioned relative reference `Sales[*-1 by Region]` | `Sales{Region}[*-1]`, `Revenue[-1 by Region]` |

## Architecture

```
 .sheet source ──┐                      ┌── XLSX (calamine in, rust_xlsxwriter out)
                 ▼                      │
   lexer → parser → AST → semantic analysis (types, units, shapes)
                                 │
                        typed IR → dependency graph
                                 │
                         execution plan → Rust runtime
                                 │
            exact scalar/array kernel  +  Polars (group/join/sort/pivot)
```

- File syntax and OMX are separate grammars sharing one AST and one type system.
  The OMX parser does not know whether an expression came from a cell, a
  computed column, `omasheet eval`, or a Markdown `{{ }}`.
- References are resolved at compile time: `Sales[*-1; Revenue]` becomes
  `LoadCell { table_id, row_offset: -1, col_id }`, not a runtime name lookup.
- Storage is column-oriented. MVP may use plain `Vec<T>` per column with
  hand-written slicing; views should not copy.
- Units are exponent vectors over base dimensions, so unit algebra is integer
  vector arithmetic. Currencies are additional independent base dimensions.

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

**Q2 — Exact numerics vs Polars.** D7 and D10 conflict. Polars has no
arbitrary-precision integer or rational dtype (its Decimal is fixed-width), so
`Rat` columns cannot be native Polars series. Options: (a) keep exact columns in
Omasheet's own kernel and use Polars only for row-index computation (group keys,
join indices, sort permutation), then gather exact values ourselves; (b)
represent a `Rat` column as a struct of two big-number-encoded columns; (c)
implement group/join natively and drop Polars. The spec's requirements are
written so any of these satisfies them: exactness must survive group and join.
Recommendation: (a).

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

**Q5 — Uncertainty vs exactness.** Quadrature (P13) needs a square root, which
leaves the rationals. Either uncertainty magnitudes are `Num` (approximate
uncertainty on an exact value — arguably fine, uncertainties are estimates), or
the engine keeps a variance (`σ²`, exact) and takes the root only at display.
Recommendation: store variance exactly; root at render.

**Q6 — What is `%`?** Treated both as a literal (`20%` = `1/5`) and as a unit
(`Margin : %`, and `GBP / GBP` yielding `%`). Provisional: `%` is a display unit
of the dimensionless dimension with scale `1/100`; `GBP / GBP` is dimensionless
and is shown as `%` only if the column is declared `%`.

**Q7 — Range vs interval syntax.** `10m .. 12m` was suggested for intervals
("value lies somewhere in here"), but `..` is the range constructor. Intervals
are out of scope here; they need their own syntax.

**Q8 — Rat growth.** Long chains of exact operations can grow denominators
without bound. Decided: never convert silently. Not decided: whether to warn
(lint) past a size threshold.

**Q9 — Joins.** Three shapes were floated: explicit `Sales.join(Customers,
CustomerID)`, pipe `|> join(...)`, and relationship-following indexing
(`Sales[*; CustomerID.Name]`). The lookup form
`Customers[ID == CustomerID].Name // "Unknown"` is specified; the explicit join
form is specified minimally; relationship-following needs a foreign-key
declaration syntax and is left open.

**Q10 — Multiple matches on lookup.** `Customers[ID == x].Name` returns a
vector if several rows match. A uniqueness assertion (`Customers[ID == x]!`) and
a `unique` column constraint were suggested, not settled.

**Q11 — Formatting syntax.** The principle (format ≠ value) is settled; the
syntax for declaring display format (decimal places, display unit, currency
symbol) is not. `@format Revenue currency(USD)` came from the early brainstorm.

**Q12 — Descending ranges and the full operator set.** `10...1` for descending
ranges, `in` / `∈` membership, `%` as modulo vs percent, and `.map/.filter/
.reduce/.sort/.unique` were all listed without detail. Membership (`in`) is
specified because an example depends on it; the rest are deferred.

**Q13 — Comments and literals not discussed.** No comment syntax, string
escaping rules, date literal grammar beyond `2025-01-01`, or boolean/null
literal spelling was settled (`null` appears once).

## Risks

- **Scope.** Exact numerics + units + uncertainty + a dataframe backend + a new
  language is a large surface. `tasks.md` phases it so that a useful tool (parse,
  evaluate, print) exists before units or XLSX.
- **Ambiguity debt.** Q1 and Q4 affect the grammar; settle them before the
  parser is written, not after.
- **Lossy XLSX round trip.** Exporting loses exactness, units and uncertainty;
  importing cannot recover table structure from a free-form grid. Users must
  not treat XLSX as a save format.

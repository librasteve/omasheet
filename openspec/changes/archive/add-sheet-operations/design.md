# Design: Sheet operations (Phase 3)

## Context

Source: `coinvent-omasheet-format-chatgpt.md`. Builds on `add-omasheet-core`;
D/P/Q numbers continue that change's numbering.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D10 | Superseded | Use a Rust dataframe library (Polars) for group/join | Replaced by D11 |
| D11 | Superseded | Polars is out of scope; group, join and sort are implemented natively on the exact column kernel | Owner decision, 2026-10-07. Resolved the former Q2. Superseded by D14 in `add-polars-dtypes`, 2026-10-19: the conflict is settled the other way, by giving up the rational rather than keeping Polars out |
| P16 | Provisional | Partitioned relative reference `Sales[; *-1 by Region]` | Alternatives seen: `Sales{Region}[; *-1]`, `Revenue[-1 by Region]` |

## Why not Polars

> Superseded by D14 in `add-polars-dtypes`. The reasoning below held; the
> conclusion is reversed from the other end, by making an omasheet type a name
> for a Polars dtype and giving up the rational.

D7 (arbitrary-precision `Int` and `Fraction`) and the earlier D10 conflicted: Polars
has no arbitrary-precision integer or rational dtype (its Decimal is
fixed-width), so `Fraction` columns cannot be native Polars series. It could only
have computed row indices (group keys, join indices, sort permutations) while
Omasheet gathered the exact values itself — a large dependency for hashing and
sorting.

## Open Questions

**Q9 — Joins.** Three shapes were floated: explicit `Sales.join(Customers,
CustomerID)`, pipe `|> join(...)`, and relationship-following indexing
(`Sales[CustomerID.Name; *]`). The lookup form
`Customers[; ID == CustomerID].Name // "Unknown"` is specified in the core; the
explicit join form is specified minimally here; relationship-following needs a
foreign-key declaration syntax and is left open.

**Q10 — Multiple matches on lookup.** `Customers[; ID == x].Name` returns a
vector if several rows match. A uniqueness assertion (`Customers[; ID == x]!`) and
a `unique` column constraint were suggested, not settled.

## Risks

- **Native sheet operations are ours to get right.** Group and join are a hash
  on the key plus a gather, which is small, but performance on large sheets and
  later additions (sort, pivot, window) have no library to lean on.
- **Recalculation has no consumer yet.** The CLI evaluates a file once per
  run. Incremental recalculation only pays off with a long-running viewer or
  LSP; it could move to whichever change introduces one.

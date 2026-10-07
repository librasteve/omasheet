# Design: Units (Phase 4)

## Context

Source: `coinvent-omasheet-format-chatgpt.md`. Builds on `add-omasheet-core`;
D/P/Q numbers continue that change's numbering.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D8 | Decided | Units are in scope | Raised by owner; the design here is provisional |
| D9 | Decided | Units are expressed over an entire column | Column-typed first, cell-typed second |
| P11 | Provisional | Unit literals are suffix-attached: `10m`, `5kg`, `12.5USD` | Alternative seen: space-separated `10 m` |

## Architecture

Units are exponent vectors over base dimensions, so unit algebra is integer
vector arithmetic. Currencies are additional independent base dimensions. Unit
checking runs in semantic analysis alongside type and shape checking.

## Open Questions

**Q6 — What is `%`?** Treated both as a literal (`20%` = `1/5`) and as a unit
(`Margin : %`, and `GBP / GBP` yielding `%`). Provisional: `%` is a display unit
of the dimensionless dimension with scale `1/100`; `GBP / GBP` is dimensionless
and is shown as `%` only if the column is declared `%`.

**Q11 — Formatting syntax.** The principle (format ≠ value) is settled; the
syntax for declaring display format (decimal places, display unit, currency
symbol) is not. `@format Revenue currency(USD)` came from the early brainstorm.

## Risks

- **Suffix literals constrain the core lexer.** `10m` must not collide with
  identifiers or exponent literals (`1e3`). Reserve the form in Phase 1 even
  though it is not accepted until this phase.

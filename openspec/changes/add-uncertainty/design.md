# Design: Uncertainty (Phase 5)

## Context

Source: `coinvent-omasheet-format-chatgpt.md`. Builds on `add-units`; D/P/Q
numbers continue the numbering of `add-omasheet-core`.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D8 | Decided | Uncertainty is in scope | Raised by owner; the design here is provisional |
| P12 | Provisional | Uncertainty literal `10 ± 0.1 m`, ASCII spelling `+-` | Alternative seen: `10m(1)` concise form |
| P13 | Provisional | Independent uncertainties combine in quadrature | Alternative seen: linear (worst-case) sum |

## Open Questions

**Q5 — Uncertainty vs exactness.** Quadrature (P13) needs a square root, which
leaves the rationals. Either uncertainty magnitudes are `Num` (approximate
uncertainty on an exact value — arguably fine, uncertainties are estimates), or
the engine keeps a variance (`σ²`, exact) and takes the root only at display.
Recommendation: store variance exactly; root at render.

**Q7 — Range vs interval syntax.** `10m .. 12m` was suggested for intervals
("value lies somewhere in here"), but `..` is the range constructor. Intervals
are out of scope here; they need their own syntax.

# Design: The Polars dtypes become the representation

## Context

Source: the design conversation of 2026-10-19/20. Builds on
`add-omasheet-core`; D/P/Q numbers continue that change's numbering, and D12
and D13 are taken by `add-interop` and `add-packaging`.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D11 | Superseded | Polars is out of scope; no dataframe library has an arbitrary-precision rational dtype | Replaced by D14. The reasoning held; the conclusion is reversed by giving up the rational, not by bridging to it |
| D14 | Decided | An omasheet type is a name for a Polars dtype; `num-bigint` and `num-rational` are removed | Owner decision, 2026-10-19 |
| D15 | Decided | `Int` is `Int64`; `Decimal` and `Percent` are `Decimal(38, s)`, an `i128` of unscaled digits and a scale | 38 digits is what a `Decimal128` carries |
| D16 | Decided | Exactness narrows from rational to decimal. A result that will not fit says so; it does not wrap | `0.1 + 0.2` is still exactly `0.3`; `(1/3) * 3` is no longer `1` |
| D17 | Decided | The `Fraction` type goes. `1/7` and `2 3/4` stay as literal syntax and are read as exact `Decimal`s | Keeps existing cells compiling; only the display changes |
| D18 | Decided | The `Complex` type goes, with imaginary literals and `re`, `im`, `conj`, `arg` | No Polars dtype holds it. May return as a `Struct{re, im}` |
| D19 | Decided | Type names match the dtype names: `Boolean`, `String`, `Float`, `Null` | `Bool`, `Text`, `Num` and the value called empty are gone |
| D20 | Decided | `Null` replaces the empty cell and is SQL-style: `Null + 1` is `Null`, and aggregates pass over it | Almost a rename: the empty cell already behaved this way |
| D21 | Decided | `DateTime` subsumes `Zoned`; the zone rides in the dtype as `Datetime(tu, tz)` | One type, as Polars has it |
| D22 | Decided | `Duration` has no literal. It is inferred from the difference of two dates or times, and read from a schema | Reverses `Date - Date → Int` |
| D23 | Decided | Unsigned columns are read and preserved but not namable: there is no `UInt` type | Nothing omasheet writes from scratch is unsigned |
| D24 | Decided | A column list (a vector of columns) is a required concept, for the Polars operations that take several columns | Was raised as "col list" |
| D25 | Decided | A whole number past an `i64` widens to a `Decimal`; past 38 digits it is a diagnostic | Built 2026-10-20. Keeps `2 ** 64` exact at `18446744073709551616`, and `10 ** 1000` says the number is too large rather than wrapping. Resolves the former P25 |
| D35 | Decided | `--exact` writes every digit a value holds; there is no `n/d` form to fall back to | Follows from D14: once the rational is gone, 38 digits is all there is of `1/3` |
| D36 | Decided | A number of ordinary size too wide to print is cut and marked; e notation is only for the genuinely small or large | Keeps `0.33333333333333333333…`, which the documents and examples already showed. Whether a value "does not end" is no longer a property it has |

## The type table

| Omasheet type | is Polars dtype | All dtypes |
|---|---|---|
| `Int` | `Int64` | `Int8` / `Int16` / `Int32` / `Int64` / `Int128` |
| | `UInt64` | `UInt8` / `UInt16` / `UInt32` / `UInt64` |
| `Float` | `Float64` | `Float32` / `Float64` |
| `Decimal` | `Decimal(38, s)` | `Decimal(p, s)`, `p` ≤ 38 |
| `Percent` | `Decimal(38, s)` | `Decimal(p, s)` + display flag |
| `Boolean` | `Boolean` | `Boolean` |
| `String` | `String` | `String` |
| `Binary` | `Binary` | `Binary` |
| `Date` | `Date` | `Date` |
| `Time` | `Time` | `Time` (ns) |
| `DateTime` | `Datetime("us", None)` | `Datetime("ms"/"us"/"ns", None \| tz)` |
| `Duration` | `Duration("us")` | `Duration("ms"/"us"/"ns")` — no literal, inferred |
| `Null` | `Null` | `Null` |

Column 2 is what omasheet writes and what an undeclared column defaults to;
column 3 is what it accepts and preserves from an existing schema.

`Error` has no dtype and stays engine-only: a cell whose calculation failed is
written as `Null` with a notice. `Range`, `Sheet` and `Row` are selections
rather than data and stay engine-only too. `Vector` survives as an engine
intermediate, since `[Revenue; 0..*].sum()` builds one, though it is not a
column type.

## Aligning with Polars does not mean depending on it

`Decimal` arithmetic is scale-aligned `i128` work: small enough to own, in
`omasheet-omx/src/decimal.rs`, with no new dependency. So the types can be the
Polars dtypes while the core stays dependency-free, and only Parquet and
frames — `add-dataframe-interop` — need the crate itself.

That leaves two arithmetic paths once that change lands: a scalar one in
`value.rs` for `omasheet eval` and single cells, and Polars expressions for
whole columns. They must agree, so the scalar path is specified as "what the
Polars kernel would do for a one-row series" and tested against it.

## What it costs, measured

Checked against a 38-digit decimal, not assumed:

```
0.1 + 0.2  = 0.3                                        ✓ exact
1/3 + 1/6  = 0.5                                        ✓ the rounding cancels
(1/3) * 3  = 0.99999999999999999999999999999999999999   ✗ omasheet gives 1 today
1/7 * 7    = 0.99999999999999999999999999999999999998   ✗
```

So the README's `omasheet eval '1/3 + 1/6'` → `0.5` survives, by luck of
rounding, while the guarantee behind it does not. `docs/expressions.md` and
`SKILL.md` both tell the reader a fraction in a result is correct and not to
"fix" it; that advice changes.

The fixed-width ceilings, for the same reason:

| Limit | An exact sum spanning |
|---|---|
| `i64` | ~43 distinct denominators |
| `i128` | ~89 |
| `lcm(1..1000)` | 433 digits — unreachable at any fixed width |

## Risks

- **It is irreversible in the files.** Regenerating every `.expected` bakes the
  new display into the repository. The `.omx` sources survive, since `1/7` and
  `2 3/4` still parse, but a reader who wants the old output needs the old
  binary.
- **Two arithmetic paths can drift.** Mitigated by specifying the scalar path
  against the Polars kernel and property-testing the pair.
- **`examples/types.omx` demonstrates arbitrary precision deliberately** — a
  `huge` row of `10_000_000_000_000_000_000`, and `Square` reaching `1e+38`.
  Both are past the new ceiling, so the example loses what it was there to
  show.

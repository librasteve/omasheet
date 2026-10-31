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
| D16 | Decided | Exactness narrows from rational to decimal. A result that will not fit says so; it does not wrap | `0.1 + 0.2` is still exactly `0.3`. `(1/3) * 3` is `1` again under D37, through the guard digits rather than through a rational |
| D17 | Superseded | The `Fraction` type goes. `1/7` and `2 3/4` stay as literal syntax and are read as exact `Decimal`s | Replaced by D38. The reasoning held — no dtype holds a rational — but the type was giving up more than the representation required, since the fraction can be recovered from the decimal |
| D18 | Decided | The `Complex` type goes, with imaginary literals and `re`, `im`, `conj`, `arg` | No Polars dtype holds it. May return as a `Struct{re, im}` |
| D19 | Decided | Type names match the dtype names: `Boolean`, `String`, `Float`, `Null` | `Bool`, `Text`, `Num` and the value called empty are gone |
| D20 | Decided | `Null` replaces the empty cell and is SQL-style: `Null + 1` is `Null`, and aggregates pass over it | Almost a rename: the empty cell already behaved this way |
| D21 | Decided | `DateTime` subsumes `Zoned`; the zone rides in the dtype as `Datetime(tu, tz)` | One type, as Polars has it |
| D22 | Decided | `Duration` is inferred from the difference of two dates or times and read from a schema, **and has an ISO 8601 literal**: `PT30M`, `PT1H30M`, `P1DT6H`, `PT0.5S` | Reverses `Date - Date → Int`. Amended 2026-10-26 (owner): the literal, dropped in the original decision, is restored so a duration can be written in the app and a computed one can be written back to the file. ISO 8601 because it is unambiguous, parseable, and already the form an XLSX duration arrives in (`Data::DurationIso`). It is the shown form too, for now; a selectable display format is later work and not part of this change |
| D23 | Decided | Unsigned columns are read and preserved but not namable: there is no `UInt` type | Nothing omasheet writes from scratch is unsigned |
| D24 | Decided | A column list (a vector of columns) is a required concept, for the Polars operations that take several columns | Was raised as "col list" |
| D25 | Decided | A whole number past an `i64` widens to a `Decimal`; past 38 digits it is a diagnostic | Built 2026-10-20. Keeps `2 ** 64` exact at `18446744073709551616`, and `10 ** 1000` says the number is too large rather than wrapping. Resolves the former P25 |
| D35 | Decided | `--exact` writes every digit a value holds; there is no `n/d` form to fall back to | Follows from D14: once the rational is gone, 38 digits is all there is of `1/3` |
| D36 | Decided | A number of ordinary size too wide to print is cut and marked; e notation is only for the genuinely small or large | Keeps `0.33333333333333333333…`, which the documents and examples already showed |
| D37 | Decided | The last 8 of the 38 digits are guard digits: held, but neither compared nor shown. 30 significant digits are | Owner decision, 2026-10-21. No fixed-point decimal is closed under division, so the last digits of a long one carry the rounding of every operation before it: held in full, `1/7 + 1/7` and `2/7` differ in the 38th digit and `1/7` and `2/7` do not even land on the same scale. Guarding at the boundary rather than in storage is what works — rounding each result to a fixed scale instead leaves the error at the last digit, unguarded, and `2/7 + 3/7 == 5/7` fails. Guarding on significant digits rather than decimal places is what keeps `1e-38`, whose every digit is significant |
| D38 | Decided | `Fraction` returns, as a display of a `Decimal` recovered by continued fractions with a denominator of at most 10000 | Owner decision, 2026-10-21, reversing D17. It costs no dtype: the column is a `Decimal(38, s)` and lowers to Polars like any other, so none of `add-dataframe-interop`'s pushdown is excluded. Recovery beats remembering the literal, since it finds a fraction nobody wrote: `(2/7 + 3/7).Fraction` is `5/7`. It is school-exact, not algebraically exact — the held value behind a displayed `1` may be 38 nines |
| D141 | Decided | The byte-string type is `Bytes` on a schema line, and Polars' `Binary` dtype underneath | Owner, 2026-10-30. `Binary` is the base-2 `Integer` of `add-radix-integers` (D123), which a schema line already names |
| D142 | Decided | A `Bytes` value is written and shown `x"48656c6c6f"`: two hex digits a byte, read in either case and shown in lower case; `x""` is no bytes | Owner, 2026-10-30. SQL's `X'…'` in OMX's quotes. It reads back exactly, and cannot be taken for a `0x` integer |

## The type table

| Omasheet type | is Polars dtype | All dtypes |
|---|---|---|
| `Int` | `Int64` | `Int8` / `Int16` / `Int32` / `Int64` / `Int128` |
| | `UInt64` | `UInt8` / `UInt16` / `UInt32` / `UInt64` |
| `Float` | `Float64` | `Float32` / `Float64` |
| `Decimal` | `Decimal(38, s)` | `Decimal(p, s)`, `p` ≤ 38 |
| `Fraction` | `Decimal(38, s)` | `Decimal(p, s)` + display flag |
| `Percent` | `Decimal(38, s)` | `Decimal(p, s)` + display flag |
| `Boolean` | `Boolean` | `Boolean` |
| `String` | `String` | `String` |
| `Bytes` | `Binary` | `Binary` |
| `Date` | `Date` | `Date` |
| `Time` | `Time` | `Time` (ns) |
| `DateTime` | `Datetime("us", None)` | `Datetime("ms"/"us"/"ns", None \| tz)` |
| `Duration` | `Duration("us")` | `Duration("ms"/"us"/"ns")` — ISO 8601 literal, or inferred |
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

# Design: a columnar core

## Context

Source: the design conversation of 2026-10-23. D/P/Q numbers continue from
`align-polars-vocabulary`. On the `polars-core` branch.

Polars is an unconditional dependency (D26) and earns none of that cost
today: it is a file-format codec at the edge of `omasheet-interop` and
touches no evaluation. `omasheet-interop/src/frames.rs:174-180` reads a whole
Parquet into a `DataFrame`, loops `column.get(r)` into a `Vec<Cell>`, and
drops the frame — every Arrow buffer, validity bitmap and all zero-copy
discarded before `Engine::new` is called. Evaluation is then a scalar row
loop over a memo `Slot` per cell: `Sales.Revenue.sum()` over N rows costs N
`cell()` calls, three N-length allocations and N `Dec::checked_add`s
(`omasheet-engine/src/eval.rs:323`, `841-918`).

The owner's decision of 2026-10-23 is to put the dataframe at the centre:
sheets' data lives as Polars columns, and selections, elementwise columns and
aggregations are lowered to Polars. This supersedes the `add-dataframe-interop`
out-of-scope line that kept group, join and sort on a native kernel, and it
absorbs that change's sections 4 and 5.

## What the spike found

Section 0 of the tasks was a gate: if polars 0.51's `Decimal(38, s)`
arithmetic promoted to Float or was unimplemented, most of the case for
lowering elementwise columns would go with it. It does neither. Measured on
polars 0.51, eagerly, with the features this workspace already has:

| Operation | Input | Polars gives | `Dec` gives |
|---|---|---|---|
| `a + b` | `Decimal(38, 2)` twice | `Decimal(None, 2)` | scale 2 — agrees |
| `a - b` | same | `Decimal(None, 2)` | scale 2 — agrees |
| `a * b` | same | `Decimal(None, 4)` | scales add, so 4 — agrees |
| `a / b` | same | `Decimal(None, 6)` | half away from zero at the widest holdable scale, up to 30 — **differs** |
| `sum` | `Decimal(38, 2)` | `Decimal(38, 2)`, exact | agrees |
| `mean` | `Decimal(38, 2)` | **`f64`** | a `Decimal`, exact — **differs** |
| `max`, `min` | `Decimal(38, 2)` | `Decimal(38, 2)` | agrees |
| `max`, `min` | `Float64` holding a `NaN` | the NaN is **skipped**: `3.0` and `1.0` | `NaN` and `1` — **differs for `max`** |
| `sum`, `mean` | same | `NaN` | `NaN` — agrees |
| sort | same | `[1.0, 3.0, NaN]`, the NaN last | D56 — agrees |
| `NaN == NaN` | same | `true` | D56 — agrees |

Three things follow, and they are the shape of this change rather than
details of it:

1. **Add, subtract and multiply agree exactly**, which is what makes an
   elementwise Decimal column lowerable at all. `Dec` was built to be a
   Polars `Decimal(38, s)` and it is one.
2. **Divide does not, and nor does `mean`.** Polars fixes a quotient at scale
   6 where omasheet holds up to 30 digits, and a Polars `mean` over a Decimal
   column comes back `f64`. So D72 covers both: neither is lowered on an
   exact column. `mean` as `sum / len` is no escape, since it is the same
   division.
3. **D56 was the right call and is now cheap.** Equality and sort order agree
   with Polars by construction. Only `max` over a NaN-bearing Float column
   differs, and `propagate_nans` — which would fix it — is
   `["polars-lazy?/propagate_nans"]`, so it does nothing on the eager path.
   A NaN-bearing Float column therefore keeps `min`/`max` on the engine until
   aggregates are lowered through the lazy API.

One smaller finding to carry: a lowered result's dtype comes back with
precision `None` (`Decimal(None, 4)`), not `Decimal(38, 4)`. Whatever writes
a lowered column back into the frame re-stamps the precision, or a later
Parquet write will disagree with the schema in the text.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D61 | Decided | `omasheet-omx` stays Polars-free; `omasheet-engine` takes the dependency | The omx crate has no dependencies at all today, which is what makes it embeddable and its test loop instant. Its only addition is `Column::elementwise: bool`, a `bool` from an analysis it already performs |
| D62 | Decided | The `ReadSources` inversion is dropped; `frames.rs` moves into `omasheet-engine/src/table.rs`; `omasheet-interop` keeps the conversion formats | Owner decision. The inversion exists only because the engine had no Polars — its own doc comment says so — and D26 closed the Polars-free build it was defending. Keeping it would mean threading Polars columns through a callback whose purpose was to avoid naming Polars types. The `dir` it carries survives as `Options { base }`: a source is read beside the `.omx`, not from the cwd |
| D63 | Decided | Authored cells stay `Vec<Cell>` on `Program`; `ColKind::Frame` becomes a marker and a table's rows arrive as Polars columns | Authored cells carry spans, are what the app's editing API rewrites, and are the only thing that can hold a per-cell formula. Only evaluated data becomes columnar. Dropping the payload is what stops `frames.rs:174-180` existing, and is the largest single performance change here |
| D64 | Decided | `Frames` lives on `Engine`, one `Option<Column>` per sheet column | Not on `Program`, which `omasheet-omx` produces; not rebuilt per cell |
| D65 | Decided | The memo grid is kept for row-recursive columns only, allocated per column on first use | `Engine::new` pre-allocates `nrows × ncols` slots today, which is the cost of merely opening a large table |
| D66 | Decided | `Column::elementwise` carries the dep analysis's verdict, and `lower::expr -> Option<Expr>` is a second gate that is total | `DepKind::Same` is exactly "elementwise", and `Checker::finish` throws it away. `Group::Cols { cols, reverse }` is not a substitute: a `Back` self-reference and a `Whole` read both land in a single-column non-reverse group |
| D67 | Decided | A lowered group succeeds wholly or falls back wholly to the row path; `Value::Error` is never a null | Null propagates silently and aggregates skip it; `Error` is poison with a diagnostic already filed, and Polars has no error element. On `Err` from `collect()`, discard, file nothing, re-run the group row-wise — which does produce per-cell errors, exactly as today |
| D68 | Decided | `Value::Time` is nanoseconds of day and `Value::DateTime` microseconds, as Polars holds them. `+ 1` still means one second | `frames.rs` already divides by `NANOS` and `MICROS` on the way in, truncating any sub-second value Parquet holds — that division is the bug this removes |
| D69 | Decided | `Percent` and `Fraction` stay display-only; the wrapper is re-applied from `Column::ty` after a lowered aggregate | "The sum and the mean of percentages are percentages" infers the wrapper from the values, and a Polars aggregate returns a bare `Decimal` |
| D70 | Decided | A `Zoned` value **is** lowered: Polars' `Datetime(TimeUnit, Option<TimeZone>)` carries a zone, and `dtype-datetime` is already on | Owner decision, 2026-10-23, bringing forward "a `DateTime` with a zone" from `add-polars-dtypes` section 5. The column holds `utc` in µs, the dtype holds the zone, `wall` is derived on read and `home` comes from the file's `zone`. **The zone is on the dtype, so one column is one zone**: a column whose values are in different zones stays on the row path. `convert_time_zone` then is Polars' own |
| D71 | Decided | A `NaN` is ordered greatest and equal to itself | Taken in `align-polars-vocabulary` as D56, and the spike confirms Polars agrees on equality and on sort order |
| D71a | Decided | `min` and `max` over a Float column holding a `NaN` are not lowered | The spike: Polars skips the NaN eagerly, where omasheet answers it. `propagate_nans` is a passthrough to `polars-lazy`, so it cannot help the eager path. Revisit when aggregates go through the lazy API |
| D72 | Decided | Neither `/` nor `mean` is lowered on an exact column; `*` is lowered, its scale having been checked against `Dec`'s | The spike. Division is fixed at scale 6 in Polars against up to 30 digits here, and a Polars `mean` over a Decimal column returns `f64` |
| D72a | Decided | A lowered column's dtype is re-stamped to `Decimal(38, s)` before it is stored | Polars returns `Decimal(None, s)` from arithmetic, and the precision is what a Parquet write and the schema in the text agree on |
| D73 | Open | A row count below which the row path is used even for a lowerable column | Needs a measurement, not a guess. A LazyFrame build, an optimiser pass and a `collect()` are slower than the row loop for a ten-row sheet, and a ten-row sheet is omasheet's typical document. Start at 1024 and measure |
| D74 | Decided | `Options { lowering: Auto \| Never \| Always }` | The differential test's lever, the bisection tool when a result changes, and the workaround for a user whose file miscalculates in the field |
| D75 | Decided | The snapshot goes column-wise, through `Engine::column_shown` | `doc.rs:2366` formats every cell of every sheet per keystroke; per-element Arrow access there would be slower than today |

## Risks

- **The per-keystroke path**, which is the sharpest. `Document::rebuild`
  recompiles, builds a fresh `Engine`, runs it and formats every cell on
  every keystroke. The columnar core makes large sheets fast and risks making
  small ones slower in the one place latency is felt. D73 is the mitigation;
  the measurement comes before the optimisation, and it is worth timing
  `compile()`, `Engine::new`, `run()` and the snapshot separately first. If
  the recompile and the snapshot dominate for interactive sheets, then D75
  and a viewport-driven snapshot matter more for the app than lowering does,
  and lowering is justified by large tables alone.
- **The thread pool.** Polars initialises a global rayon pool on first use.
  A `collect()` fanning out per keystroke on the Qt thread is its own latency
  hazard. Either keep `collect()` strictly above D73's threshold or size the
  pool at startup.
- **`RefCell` discipline.** A `frames.borrow()` held across a `collect()` and
  then upgraded is a panic waiting to happen. `run_lowered` borrows to build
  the expressions, drops the borrow, collects, then borrows mutably to write
  back: three statements, scoped, with a comment saying why.
- **The binary and the build.** `lazy` pulls `polars-lazy`, `polars-plan`,
  `polars-expr`, `polars-mem-engine` and more of `polars-ops`, none of which
  are compiled today. Measured before and after, per task 1.2, which
  `add-dataframe-interop` left open for exactly this.

## Open questions

- D73, the threshold (owner, after the measurement).
- Whether aggregates should go through the lazy API rather than the eager
  one, which is what would let `propagate_nans` settle D71a.
- Whether `Value::Series` is needed at all, which is a measurement rather
  than a decision.

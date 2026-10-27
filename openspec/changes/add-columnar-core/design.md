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
| `nan_max`, `nan_min` | same | `NaN` and `NaN` | `NaN` and `1` — **differs for `min`** |
| `sum`, `mean` | same | `NaN` | `NaN` — agrees |
| sort | same | `null, 1.0, 3.0, NaN, NaN` | D56 — agrees |
| `NaN == NaN` | same | `true`, on both paths | D56 — agrees |
| `is_nan` of a null | same | `null`, not `false` | there is no `is_nan` yet (D71b) |
| `group_by` over a NaN | same | the NaNs are one key | — |

Three things follow, and they are the shape of this change rather than
details of it:

1. **Add, subtract and multiply agree exactly**, which is what makes an
   elementwise Decimal column lowerable at all. `Dec` was built to be a
   Polars `Decimal(38, s)` and it is one.
2. **Divide does not, and nor does `mean`.** Polars fixes a quotient at scale
   6 where omasheet holds up to 30 digits, and a Polars `mean` over a Decimal
   column comes back `f64`. So D72 covers both: neither is lowered on an
   exact column. `mean` as `sum / count` *is* an escape all the same, which
   this said it was not — not because the division is any different, but
   because there is one of it: the two halves that are a pass over the
   column are Polars' and the single division is the engine's, which is
   D72b.
3. **D56 was the right call.** `NaN == NaN` is `true` in polars 0.51 on both
   the eager path and through the expression engine — measured over two
   distinct columns, so it is not a self-comparison shortcut — and a NaN
   sorts last. Polars has moved to total-ordering semantics for float
   equality; IEEE, pandas and older Polars say otherwise, and a table saying
   `False` describes one of those rather than 0.51.

   What does not line up is the pair `min` and `max`, and neither Polars
   setting gives both. Omasheet holds a NaN to be greatest, so its `max` is
   the NaN and its `min` is the least of the numbers. Polars' default `max`
   and `min` skip the NaN; `nan_max` and `nan_min`, which `propagate_nans`
   adds, both answer the NaN. So omasheet's `max` is Polars' `nan_max` and
   omasheet's `min` is Polars' default `min` — except over a column of
   nothing but NaNs, where the default `min` is null and omasheet answers the
   NaN. D71a is refined accordingly rather than refusing both.

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
| D69a | Decided | A sheet's own `Percent` or `Fraction` column is held as the `Decimal(38, s)` its values are, only the matching dress being read into it | D69 all through: if the dress is display and `cell_of` puts it back on from `Column::ty`, there is nothing left for `column_of` to decline. Declining it kept every such column of the text off the dataframe, so no aggregate over one could be taken and no lowered expression could read one — which 7.3's fifth table found as the whole of what a `Summary` still cost. A bare number among the values is still declined, since it would come back wearing a dress the text does not give it |
| D70 | Decided | A `Zoned` value **is** lowered: Polars' `Datetime(TimeUnit, Option<TimeZone>)` carries a zone, and `dtype-datetime` is already on | Owner decision, 2026-10-23, bringing forward "a `DateTime` with a zone" from `add-polars-dtypes` section 5. The column holds `utc` in µs, the dtype holds the zone, `wall` is derived on read and `home` comes from the file's `zone`. **The zone is on the dtype, so one column is one zone**: a column whose values are in different zones stays on the row path. `convert_time_zone` then is Polars' own |
| D71 | Decided | A `NaN` is ordered greatest and equal to itself | Taken in `align-polars-vocabulary` as D56, and the spike confirms Polars agrees on equality and on sort order |
| D71a | Decided | `max` is lowered to Polars' `nan_max`, and `min` to Polars' default `min`. A Float column of nothing but `NaN`s is not lowered for `min` | The spike, measured with `propagate_nans` on. Omasheet holds a NaN greatest, which makes its `max` exactly `nan_max` and its `min` exactly the default `min` — the two differ only when there is no number to be least, where the default gives null and omasheet gives the NaN. That case is one `is_not_nan().any()` to detect and is left on the engine |
| D71b | Decided | `is_nan` and `is_not_nan` are added, and `is_nan` of an empty cell is empty rather than false | Polars has both, and without them OMX can ask whether a cell is missing but not whether it is a `NaN` — half of the distinction the two kinds of absence draw. Polars' `is_nan` returns null for a null, which is the answer a missing value deserves, and `is_null` already answers the other question. `fill_nan` waits: `fill_null` is the function spelling of `//` and takes one value, so the pair should widen together |
| D72 | Decided | Neither `/` nor `mean` is lowered on an exact column; `*` is lowered, its scale having been checked against `Dec`'s | The spike. Division is fixed at scale 6 in Polars against up to 30 digits here, and a Polars `mean` over a Decimal column returns `f64` |
| D72a | Decided | A lowered column's dtype is re-stamped to `Decimal(38, s)` before it is stored | Polars returns `Decimal(None, s)` from arithmetic, and the precision is what a Parquet write and the schema in the text agree on |
| D72b | Decided | `mean` over an exact column is lowered as a `sum` and a `count` taken together, and the one division between them is the engine's `Dec` | D72 refuses the division, not the column walk. The walk is what costs O(n) and the division is one scalar operation, so taking the first and keeping the second gives the row path's answer to the digit. A floating-point column is still refused, its `sum` being the half that addition order matters to |
| D72c | Decided | `min` and `max` of a `Date`, a `Time` or a `Datetime` are lowered | An extreme of a temporal column is the extreme of the whole number it is held as, which is the order `value::compare` puts two of them in. The gate asked for an exact scale or a float and so declined these by omission rather than for a reason |
| D72d | Decided | A lowered `sum` is refused where the total has no room: the plan takes the two extremes as well, and `rows × max\|value\|` is checked against `Dec`'s 38 digits before the sum is believed | Polars adds decimals in an `i128` and **wraps on overflow without saying so**. A hundred thousand quotients held at `Decimal(38, 34)` summed to `6481.74` where the engine folds `40509.98`, which this change would have shipped. The bound is one extra pass in the same plan and refuses a shade early rather than ever wrapping |
| D73 | Open | A row count below which the row path is used even for a lowerable column | Needs a measurement, not a guess. A LazyFrame build, an optimiser pass and a `collect()` are slower than the row loop for a ten-row sheet, and a ten-row sheet is omasheet's typical document. Start at 1024 and measure |
| D74 | Decided | `Options { lowering: Auto \| Never \| Always }` | The differential test's lever, the bisection tool when a result changes, and the workaround for a user whose file miscalculates in the field |
| D75 | Decided | The snapshot goes column-wise, through `Engine::column_shown` | `doc.rs:2366` formats every cell of every sheet per keystroke; per-element Arrow access there would be slower than today |
| D76 | Decided | `and` and `or` take Kleene's rule, as Polars and SQL have them: an empty cell is an unknown, and one that cannot change the answer does not decide it. Both are then lowered | The rule they had was not the language's "empty in, empty out" but a left-biased half of Kleene's — the short-circuit in `eval` made `false and null` false and `null and false` null, so `and` read differently each way round. Completing it removes that asymmetry rather than adding an exception, lets the commonest condition of all be one Polars pass, and is the same argument as D71's `NaN` ordering: a column should give the same answer whichever path works it out. Only two cells of the truth table move, and both move from empty to a definite answer |
| D77 | Decided | A call is lowered where Polars has the same answer: `is_null`, `is_nan` over a floating-point column, and the parts of a date or a time — `year`, `month`, `day`, `weekday`, `quarter`, `week`, `ordinal_day`, `hour`, `minute`, `second`, `date`, `time` | 8.3's table says a call is where the refusals are. These are the ones that are total and provably Polars': its weekday counts from Monday as `date::weekday` does, its week is the ISO week, and its cast of a date-time floors before the epoch as `split_datetime` does — each pinned by a test over a leap day, an ISO week belonging to the year either side, and a date in 1969. A conversion, a rounding and a zone stay out for being omasheet's own rules; `abs`, `sign`, `round`, `floor`, `ceil`, `exp`, `log` and the trigonometric functions stay out for being behind polars features this build does not enable, which is 1.3's question |

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
- Whether `fill_null` and `is_in` — and so `fill_nan` — should widen from one
  value to a whole column, which is a change to the operators `//` and `in`
  rather than to the functions that spell them.
- Whether `Value::Series` is needed at all, which is a measurement rather
  than a decision.

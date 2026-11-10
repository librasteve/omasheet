# Design: A Polars function registry

Deferred until `add-units` is archived. D numbers are assigned when the change starts; the rules are N1–N9 until then. They build on D51 (flatten the namespace), D57 (no keyword arguments, positional) and D60 (a name enters the directory with its implementation) of `align-polars-vocabulary`, and D84 (`desc` as a bare word).

## Naming rules

| # | Status | Rule | Example |
|---|--------|------|---------|
| N1 | Proposed | The OMX name is the Polars `Expr` method name, unchanged, as currently documented for the pinned Polars version, snake_case | `cum_sum`, `rolling_mean`, `pct_change`, `n_unique` |
| N2 | Proposed | A deprecated Polars name is not used; the name is the current one | `cum_sum`, not `cumsum` |
| N3 | Proposed | A namespace is dropped (D51): `str.*` and `dt.*` flatten, and the noun is a prefix only where dropping it collides or misleads | `str.replace_all` is `replace_all`; `str.len_chars` is `len_chars`; `list.len` is `list_len` |
| N4 | Proposed | The expression the method is called on is the receiver, in both spellings the language already has: `Amount.cum_sum()` and `cum_sum(Amount)` | |
| N5 | Proposed | Arguments are positional, in Polars' order, each optional one defaulting to Polars' default; a keyword-only Polars argument becomes the next positional one, and a flag a bare word (D84) | `rolling_mean(3)` is `window_size=3`; `rank(desc)`; `diff(2)` is `n=2` |
| N6 | Proposed | `over(partition_by=…, order_by=…)` is not a function but two clauses: the order key is a bare column, first inside the call, and `by Key` is the partition, as already in `[x; *-1 by Region]` | `Amount.cum_sum(Date by Account)` is `pl.col("Amount").cum_sum().over("Account", order_by="Date")` |
| N7 | Proposed | Where OMX already has a name meaning something else, the existing meaning stands and the Polars one takes a prefix or a different spelling, recorded with the reason (as D54 did for `round`) | `pow` is the operator `**`, not a function |
| N8 | Proposed | Where Polars expresses a function as an argument of another (`fill_null(strategy="forward")`), OMX uses the separate name the Polars docs list for it, if it still lists one; otherwise the bare word | `forward_fill` pending the check in N9 |
| N9 | Proposed | The registry entry carries the Polars method name as a field, and a test asserts that every entry's OMX name equals it, apart from an exceptions list in which each line gives the reason | The mapping is data, not a convention to be remembered |

## Stages in the registry

The registry has two kinds of entry: a **function** (a method on a column, above) and a **stage** (a step after `|>`, a function of a sheet that gives a sheet). A stage entry has the same fields, with `args` also naming the second sheet, its keys and the clauses it takes. The existing stages (`filter select head tail sort group_by agg join`) stay as they are and move onto the registry later (out of scope here); the new ones start on it.

| # | Status | Rule | Example |
|---|--------|------|---------|
| N10 | Proposed | A stage is named as the Polars `DataFrame` method is (N1), takes the other sheet first, then the keys, the way `join(Other, keys)` does (D84) | `join_asof`, `join_where` |
| N11 | Proposed | `join_asof(Other, left_key, right_key)` takes the two keys as bare columns, or one when both sheets name it alike; `strategy` is a bare word (`backward`, `forward`, `nearest`), `backward` by default; `by Key` partitions both sheets (N6); `tolerance` is the next positional | `Sales |> join_asof(Rates, Sales_Amount, Sales_Target)`, `... , forward`, `... by Region` |
| N12 | Proposed | `join_where(Other, condition)` reads the condition with the left sheet's columns bare and the other's under its sheet name | `Orders |> join_where(Bands, Amount >= Bands.Low and Amount < Bands.High)` |
| N13 | Proposed | `join`'s `how` takes Polars' words `inner` (default), `left`, `full`, `semi`, `anti`, `cross` | `Orders |> join(Customers, ID, anti)` |
| N14 | Proposed | Both sheets of an as-of join must be sorted by their key; OMX sorts them first, as Polars' `check_sortedness` would otherwise refuse, so the file's row order does not matter | the commission table need not be kept in order |

## Order for the positional reference

| # | Status | Rule | Example |
|---|--------|------|---------|
| N15 | Proposed | `shift(n)` is Polars' `shift`: positive `n` looks back, negative forward, the rows with nothing there empty (so `// 0` as now); it takes the order key and `by` as N6 does | `Balance.shift(1, Date by Member) // 0` |
| N16 | Proposed | The positional reference takes an `order Key` clause after `by`; it sorts the (partitioned) rows by `Key`, stable, applies the offset or range there and puts the result back on the row it came from. No clause means file order, so every existing file reads as it did | `[Balance; *-1 by Member order Date]`, `[Amount; 0..* order Date]` |
| N17 | Proposed | A prefix range (`0..*`) under `sum min max count mean` is one running pass over the ordered rows, the same value as the per-row evaluation, which stays as the definition and the test oracle | `[Amount; 0..*].sum()` |

N16 is the same thing as N6 spelled for the reference form (`cum_sum(Date by Account)` and `[Amount; 0..* order Date by Account]`); both lower to one window, and the registry test asserts the two agree.

The commission example, as a stage and as a column:

```omx
sheet Paid := Sales |> join_asof(Commission_Rates, Sales_Amount, Sales_Target)
```

and, once the registry has the clauses, on a column: `Rate := Sales_Amount.lookup_asof(Commission_Rates, Sales_Target, Commission_Rate)`, a column spelling of the same join that gives one value per row. Decided yes (below).

## Registry entry

```
name        the OMX name (N1)
polars      the Polars method name, for the test of N9
args        the kinds of its arguments: window length, ddof, flag, order key, partition
result      the result-type rule: same as input, Float, Integer, Boolean, ...
refuses     the dtypes it will not take: Complex, Scientific, ... (an error naming the function)
features    the Cargo feature the Polars function needs
```

## Open

- **N8**: Polars may deprecate `forward_fill` and `backward_fill` in favour of `fill_null(strategy=…)`. Check the pinned version's docs before choosing; if deprecated, N2 says the name is `fill_null(forward)`, which clashes with the `//` meaning of `fill_null` (D58). Decide at the start of the change.
- Whether `by` inside a call needs a parser change or can reuse the cursor `by` parser.
- `Decimal` support per function: some Polars windowed functions are not defined for decimals and would need a cast, as `sqrt` casts to `Float`.

- `lookup_asof` as a column function over an as-of stage: **decided yes** (a column of a sheet cannot be added to by a stage, whose sheet is read-only), as the spreadsheet user's form of N11, backed by the same entry. It replaces `Rates[Rate; Start <= Date].last()`, which is right only for a `Rates` already in order.
- The spelling of `order` in a selection (N16) against the cursor and `by` parsers, and whether `*-1` with `order` counts rows or key values (rows, as `*-1` does now).
- N17's one-pass evaluation for `Decimal` (exact) against Polars' `cum_sum`, which may not take every decimal scale; the exact pass is the fallback.
- `join_where` needs an expression language over two sheets' columns at once; how a name clash is spelled (N12) is to be settled with the checker.

## Risks

- Polars changes its names between versions; N1 pins them to the version in `Cargo.toml`, and N9's test fails on a drift.
- The lint of `Tx[Amount; Date <= Date]` cannot tell a deliberate whole-column comparison from a slip; it warns, never errors.
- A scalar-path call on a temporary frame costs a conversion; a cell-at-a-time caller would be quadratic, so the registry functions are whole-column only and are computed once per column.

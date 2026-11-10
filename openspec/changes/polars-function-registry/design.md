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

## Risks

- Polars changes its names between versions; N1 pins them to the version in `Cargo.toml`, and N9's test fails on a drift.
- A scalar-path call on a temporary frame costs a conversion; a cell-at-a-time caller would be quadratic, so the registry functions are whole-column only and are computed once per column.

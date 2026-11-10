# Design: `apply` on groups

Forthcoming. D numbers are assigned when the change starts; `D-A#` until then.

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D-A1 | Proposed | The stage is spelled `apply`, as the owner asked, though the current Polars name is `map_groups` (`apply` is its name before 0.19) | An exception to N1/N2 of `polars-function-registry`, to be recorded in that change's exceptions list (N9) with this reason: `apply` is the name pandas, Grist and older Polars share, and it reads as what it does. A `map_groups` alias is not added: one name |
| D-A2 | Proposed | The function is called once per group, with the group as a sheet whose columns are the source's, key columns included | So `Worth(orders)` of the existing example works unchanged. Dropping the keys would mean a function could not use them, and passing them separately would need a second parameter |
| D-A3 | Proposed | A function that gives a single value or a vector makes one row per group; one that gives a sheet makes its rows, keys prepended | The two Polars cases. The shape is known from the function's result type, which the checker works out once, not per group |
| D-A4 | Proposed | Every group must give the same shape: a function that gives a sheet for one group must give a sheet with the same columns for the others, or it is a cell-level error naming the group | Polars requires one schema. A group with no rows is allowed |
| D-A5 | Proposed | The result's column types are the join of the groups' results, as an undeclared column's are (D191), and a value of a type that cannot join gives an error naming the group | Consistent with how `Complex` widens a column; no new rule |
| D-A6 | Proposed | Groups run in key order, an empty key first, one at a time, in the engine; the rest of a pipeline before and after is lowered as usual | Matches `agg`. `apply` cannot lower, so it is the boundary where the plan is collected. Cost is the number of groups, stated in the docs |
| D-A7 | Proposed | A function may read the whole file (constants, other sheets, other functions) but not the row it is called from, as every `fn` (format.md) | No change to what a function sees |
| D-A8 | Proposed | An error inside the function for one group is an error in that group's row(s) and is reported with the group's key, and the other groups are still worked out | Same as a failing cell: it does not stop the sheet |
| D-A9 | Proposed | `group_by` is a stage that may be followed by `agg` or `apply`, and by nothing else | As now for `agg`: it has no rows of its own |

## Example

```omx
fn Worth(orders) = (orders.Qty * orders.Price).sum()
fn Top3(rows)    = rows |> sort(Revenue, desc) |> head(3)

sheet RegionWorth := Orders |> group_by(Region) |> apply(Worth)
sheet BestThree   := Sales  |> group_by(Region) |> apply(Top3)
```

`RegionWorth` has `Region` and `Worth`. `BestThree` has `Region` and the columns of `Sales`, the best three rows of each region.

## Open

- Whether the single-value column takes the function's name or `Name := apply(Fn)` is the only way to name it (as an aggregate of anything else must be named); recommended: it takes the function's name, and `Name := apply(Fn)` renames.
- Behaviour when the function returns a sheet with a column the keys also name: the key wins and the clash is an error, to be confirmed in the spec.
- Whether a later change opens `apply` to a `fn` with extra arguments by partial application.

## Risks

- Per-group calls are one engine evaluation each: a million groups is a million calls. The docs say so and point to `agg` and the registry's window functions where they are enough.
- A function written for `apply` that returns a sheet is easy to get shape-wrong; D-A4's error names the group and the difference.

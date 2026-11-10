# omx-expressions

## ADDED Requirements

### Requirement: Applying a function to each group

`apply(Fn)` after `group_by(keys)` SHALL call the function `Fn`, a function of your own with one parameter, once for each group, with the rows of the group as a sheet, and SHALL give a sheet: one row per group when the function gives a value or a vector, and the function's rows with the key columns prepended when it gives a sheet. The groups SHALL be in the order of the keys, an empty key first.

#### Scenario: A value per group

- **GIVEN** `fn Worth(orders) = (orders.Qty * orders.Price).sum()` and `sheet RegionWorth := Orders |> group_by(Region) |> apply(Worth)`
- **WHEN** it is calculated
- **THEN** the sheet has a row for each `Region` with the columns `Region` and `Worth`

#### Scenario: Rows per group

- **GIVEN** `fn Top3(rows) = rows |> sort(Revenue, desc) |> head(3)` and `Sales |> group_by(Region) |> apply(Top3)`
- **WHEN** it is calculated
- **THEN** each region has its three best rows, with `Region` first

#### Scenario: A group that fails

- **GIVEN** a function that is an error for one group
- **WHEN** the sheet is calculated
- **THEN** that group's row is an error naming the group's key, and every other group is worked out

#### Scenario: Apply with nothing to apply it to

- **GIVEN** `Orders |> apply(Worth)`
- **WHEN** it is checked
- **THEN** it is an error saying `apply` follows `group_by`

## ADDED Requirements

### Requirement: Kinds of plot

A plot's `type` is one of `line`, `bar`, `pie`, `candle`, `scatter`, `area`, `stacked`, `step`, `histogram`, `box`, `heatmap`, `waterfall`, `bubble` and `donut`.

#### Scenario: A scatter plot of numbers with uncertainties
- **WHEN** a plot of `type: scatter` reads a sheet whose `x` is numeric and whose `y` holds `Scientific` values
- **THEN** each value is a point at its `x`, with an error bar of one standard deviation

#### Scenario: A scatter plot of labels
- **WHEN** a scatter or bubble plot's `x` holds text
- **THEN** the plot is not drawn, and says it needs numbers in `x`

#### Scenario: A histogram
- **WHEN** a plot is `type: histogram` with `bins: 14`
- **THEN** each `y` column's values are counted in 14 bins shared by the columns

#### Scenario: A bad `bins`
- **WHEN** `bins` is not a whole number from 1 to 200
- **THEN** `omasheet lint` reports it at its value

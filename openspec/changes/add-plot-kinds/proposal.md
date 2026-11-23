# Proposal: More kinds of plot

> **Built**, except the look of the new kinds in the pane, which is checked by hand (task 4).

## Why

Four kinds (line, bar, pie, candle) do not reach the plots of `plotly.rs/examples`, nor those that suit scientific data: points with error bars, distributions, grids of values.

## What Changes

- **file-format** — ADDED: ten kinds of `type`: `scatter`, `area`, `stacked`, `step`, `histogram`, `box`, `heatmap`, `waterfall`, `bubble` and `donut`; and a control `bins` for `histogram`.
- **file-format** — MODIFIED: where every `x` is a number, a `line`, `step` or `area` is drawn at them on a numeric axis; `scatter` and `bubble` need them.
- **plot** — `omasheet-plot` draws each: scatter, area, stacked, step, histogram and box and waterfall and bubble on the same axes as before, heatmap and donut as raw SVG as the pie is.
- **app** — the type list and the `+ Plot` prompt offer the new kinds.
- Example: `examples/plots2.omx` (12 plots, with a `Scientific` column for error bars and a candle) and `examples/plots2-report.md`.

## Impact

- `omasheet-plot/src/lib.rs`, `omasheet-omx/src/file.rs` (`PLOT_KINDS`, `PLOT_KEYS`), `check.rs` (`Plot.bins`), `omasheet-engine/src/plot.rs` (`Table.xs`, `bins`)
- `omasheet-ui/qml/Main.qml` (the kind lists), `docs/format.md`, `docs/app.md`

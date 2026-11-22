# Proposal: Plots, declared in the file

> **Built**, except the look of the pane in the app, which is checked by hand (task 7). Replaces the unreleased `add-plot-blocks`, which put plots in Markdown only.

## Why

A report can quote values, but has no way to show a trend, a split or an uncertainty except as a table, and the desktop app, which opens `.omx` files and not Markdown, has nowhere to show one. A plot has to be defined in the `.omx` file so that the file, a report and the app all read the same definition.

## What Changes

- **file-format** — ADDED: a top-level `plot Name` declaration, followed by `key: value` lines to the next blank line: `type` (`line`, `bar`, `pie`, `candle`), `data` (an expression that gives a sheet), `x`, `y`, `title`, `width`, `height`, `k`. A plot shares its namespace with sheets and constants. `lint` reports a bad control, an unknown column or a `data` that is not a sheet.
- **engine** — ADDED: `plot_svg`, which evaluates a plot's `data`, picks its columns and draws it with `omasheet-plot`. A report and the app both use it.
- **markdown-integration** — ADDED: a plot declared in an `omx` block or a file named in the front matter is drawn, in place of its block, as an inline SVG; `{{ Name }}` for the name of a plot draws it where written. REMOVED: the fenced `plot` block, which never shipped.
- **app** — ADDED: a plot pane beside the grid, with a picker for the file's plots, drawn in the theme's colours and redrawn after each edit. Toggled by a footer button and F7, and remembered.
- **new crate** — `omasheet-plot`: the drawing. It uses `plotters` for the marks and writes the text itself, so it needs no system font and its ink can follow the theme.

## Impact

- `omasheet-omx/src/file.rs`, `check.rs` (`FileAst`, `Program`, `ensure_plot`)
- `omasheet-engine` (`plot.rs`, the snapshot), `omasheet-md/src/lib.rs`
- `omasheet-ui/src/file.rs`, `omasheet-ui/qml/Main.qml`, `omasheet-ui/help.txt`, `pkgbuild/PKGBUILD`
- `docs/format.md`, `docs/reports.md`, `docs/keyboard.md`, `docs/app.md`; `examples/plots.omx`, `examples/plots.md`

## Out of Scope

- Making or editing a plot from the app's grid: it is written in the source view (F3)
- Scatter, area, histogram, a second axis, stacked bars, a time axis (dates are labels in row order)
- `omasheet print` listing plots
- Saving a plot as SVG or PNG from the app

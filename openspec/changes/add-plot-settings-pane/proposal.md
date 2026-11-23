# Proposal: Plot settings in a pane, and clicking a mark

> **Built**, except the look in the app, checked by hand (task 4).

## Why

The controls were a row of buttons over the plot, each opening a popup. They deserve a pane: the kind chosen from pictures, the columns ticked as chips, and the size and the options of a kind in fields. And a plot should point back at its data: a click on a mark finds the row it stands for.

## What Changes

- **app** — ADDED: a settings pane to the right of the plot pane (the gear in the plot pane puts it away, and it is kept): name and parent sheet, `type` as tiles each with a picture, `data`, `x` and `y` as chips, `title`, `width`, `height`, and `k` or `bins` where the kind has one. A refused change is said at its top.
- **app** — MODIFIED: the plot pane keeps only the plot chips; the plot takes half of what the settings leave.
- **app** — ADDED: a click on a line or a bar lights up its row in the sheet; on a heatmap, its cell.
- **plot / engine** — ADDED: `render_hits` gives each mark's rectangle and row, and `Document::plot_cell_at` finds the cell under a pixel.

## Impact

`omasheet-plot/src/lib.rs`, `omasheet-engine/src/plot.rs`, `doc.rs`, `omasheet-ui/src/file.rs`, `Main.qml`

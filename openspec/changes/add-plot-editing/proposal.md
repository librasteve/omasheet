# Proposal: Making and changing plots in the app

> **Built**, except the look of the pane in the app, which is checked by hand (task 5).

## Why

`add-plot-declarations` put a plot in the file and a pane in the app, but a plot could only be written in the source view. A plot should be made, and its type, data and columns chosen, as a sheet is: from the window, with every change one step to undo.

## What Changes

- **app** — ADDED: `+ Plot` in the footer and in the pane asks for a name, a kind and the data (the sheet on view to begin with). The pane has a button for each control of the plot it is on: `type`, `data`, `x`, `y` (ticked columns) and `title`, and a menu to rename or delete it. A refused change is said in the pane.
- **app** — MODIFIED: the footer's plot button is three drawn bars in place of the word. F7 is the pane's only key.
- **app** — FIXED: the plot's SVG names the app's own font, not `sans-serif`, which Qt looks up as a family that does not exist and pays for in the first draw.
- **engine** — ADDED: `Document::add_plot`, `set_plot_control`, `rename_plot`, `delete_plot`, `plot_controls` and `plot_columns`. A plot shares its name with sheets and constants, and renaming a sheet renames it in the `data` of the plots that read it.
- **file-format** — the parser records where a plot ends.

## Impact

- `omasheet-omx/src/file.rs` (`PlotDecl.end`), `omasheet-engine/src/doc.rs`, `plot.rs`
- `omasheet-ui/src/file.rs`, `json.rs`, `omasheet-ui/qml/Main.qml` (`promptDialog`, `plotPane`, `FooterButton`)
- `docs/app.md`

## Out of Scope

- A key for `+ Plot`
- `width`, `height` and `k` in the pane: they are written in the source view
- Plots in the tab strip, and moving a plot among the others

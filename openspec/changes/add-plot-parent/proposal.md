# Proposal: The pane follows the sheet

> **Built**, except the look in the app, checked by hand (task 3).

## Why

A file has sheets and plots, and a plot belongs with the sheet it reads. The format already says so in `data`, so no new syntax: the parent is worked out from it.

## What Changes

- **engine** — ADDED: `Plot.sheet`, the sheet its `data` is a view of, and `Document::plot_parents`. It follows a sheet's rename, as `data` does.
- **app** — MODIFIED: the pane lists the plots that read the sheet on view; an `all` chip shows every plot, and `+ Plot` shows the new one wherever it is from. Changing sheet goes to that sheet's first plot.

## Impact

`omasheet-omx/src/check.rs`, `omasheet-engine/src/doc.rs`, `omasheet-ui/src/file.rs`, `Main.qml`

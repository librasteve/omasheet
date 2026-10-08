# Design: Interactive app

## Context

Modelled on Omawrite (`github.com/omacom/omawrite`): a single Qt Quick window,
Material controls, keyboard-first, colours from the current Omarchy theme.
Omawrite is C++ and QMake; the owner prefers Rust, so the window's logic is Rust.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D16 | Decided | The app is an interactive grid, not a read-only viewer | Owner decision, 2026-10-08. Supersedes P15 |
| D17 | Decided | Written in Rust where possible | Owner decision, 2026-10-08. QML for the window, Rust (`cxx-qt`) for everything behind it |
| D18 | Decided | The app and the command line stay two commands for now: `omasheet-ui` and `omasheet` | Owner decision, 2026-10-08. One binary was the alternative |
| P20 | Provisional | The `.omx` text is the model: each edit rewrites the text and the sheet is re-read and re-evaluated | Keeps one source of truth and makes undo a stack of texts |
| P21 | Provisional | Unlike the command line, the app evaluates a sheet that has errors, and shows `#ERROR` only in cells that could not be calculated | One mistyped formula should not blank the grid |
| P22 | Provisional | Editing any cell of a formula column edits that column's formula | A formula column has one expression for all rows |
| P23 | Provisional | Copy takes cell sources (so formulas paste as formulas); for a formula column it takes the values | |

## Architecture

```
 Main.qml  ──calls──▶  Sheet (Rust QObject, cxx-qt)  ──▶  Document (omasheet-engine)
    ▲                        │                                 │ rewrites .omx text
    └── snapshot as JSON ◀───┘                                 ▼ compile + evaluate
```

The window holds no sheet state of its own beyond the cursor and selection. After
each edit `Sheet` bumps a revision number and the window re-reads the snapshot.

## Limitations

- **Whole-sheet snapshot.** Every edit re-evaluates the sheet and sends all of it
  to the window as JSON. Fine for sheets of thousands of cells; a large sheet
  needs a real table model (task 3.4).
- **Normalised layout.** Editing a table re-aligns its header and rows with
  single-space padding and ` | ` separators. A row whose cells do not change
  width stays a one-line diff; a hand-formatted table is normalised on first edit.
- **Single-column tables.** An empty cell in a one-column table would be a blank
  line, which is not a row, so the app writes `""` there. This is a gap in the
  format rather than in the app.
- **Clipboard.** QML has no clipboard object, so copy and paste go through a
  hidden text control.

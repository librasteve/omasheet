# Proposal: GUI: scroll the tab bar while a sheet tab is dragged

## Why

Sheet tabs are dragged along the tab bar to a new place, but the bar scrolls when there are more tabs than room. A tab cannot be dragged to a place that is scrolled out of view: the drag has to be dropped, the bar scrolled by hand or by its arrows, and the drag begun again. Dragging columns scrolls the grid at its edges (`gui-drag-columns`); the tabs should do the same.

## What Changes

- **app** — ADDED: while a sheet tab is dragged and the bar scrolls, holding the pointer near either end of the bar scrolls it that way, faster the nearer the end, and the landing mark follows the gap under the pointer, which moves as the tabs do

## Impact

- `omasheet-ui/qml/Main.qml`: a `Timer` in `tabRow`, and the pointer's place along the bar recorded by the tab's `DragHandler`
- `docs/app.md` if it describes dragging sheets

## Out of Scope

- Scrolling while a row, or a cell range, is dragged
- Changing the arrows at the ends of the bar

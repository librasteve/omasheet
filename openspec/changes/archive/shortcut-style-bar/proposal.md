# Proposal: Keys for the style bar and the column settings

> **Built**, with one check by hand outstanding (see its last task); archived.

## Why

The style bar (undo, bold, italic, heading level, summary, alignment, width, wrap, hide) needed the pointer, the filter had a key of its own that F4's cycle of column settings made redundant, and F4 skipped the format box for any column declared as a type but a quantity, so a `Decimal[.1f; ppm]` column was passed by. The shortcut list was kept twice, in `help.txt` and in `docs/keyboard.md`, and the two had drifted.

## What Changes

- **app** — ADDED: F6 moves the keyboard to the style bar and back; Shift+F6 starts at the far end. ← → (and Home, End) move over the buttons that can be used, Enter, Space or ↓ use one, which opens its menu where it has one, and Esc leaves. The button the keyboard is on has a ring, and the one last used is where F6 returns
- **app** — MODIFIED: F4 and Shift+F4 go round filter, type, unit and format, the filter first. The format box is passed by only for a column with no type yet
- **app** — REMOVED: Ctrl+Shift+F as the filter's key. It is Paste format alone
- **app** — MODIFIED: the helper panes (Formats, Operators, Prefixes) have a handle on the bottom edge to stretch the list; the Formats list sizes its format column to the longest format and cuts a description at the edge, never wrapping it
- **app** — MODIFIED: a column head's type line is cut at the column's edge where a width set for the column is narrower than it
- **docs** — MODIFIED: `omasheet-ui/help.txt` is the one list of shortcuts, and `scripts/sync-keyboard.py` writes the key table of `docs/keyboard.md` from it (`--check` for CI)

## Impact

- `omasheet-ui/qml/Main.qml`: `styleRow`, `metaAvailable`, `allFormattable`, the help panes, `formatPopup`, the head's type line
- `omasheet-ui/help.txt`, `docs/keyboard.md`, `docs/app.md`, `scripts/sync-keyboard.py`
- `omasheet-engine/src/doc.rs`: the format suggestions, shortened to fit a row

## Out of Scope

- Keys inside the style bar's menus beyond what Qt's menu gives
- Keys for the other popups

# Proposal: Keys for the filter box

## Why

The column popup holds the sort entries, the filter field, Clear filter and four buttons, and only the field takes the keyboard. Getting to a button, or opening the popup at all, needs the pointer. Two of its buttons also do the same thing: OK applies and closes, and Cancel closes, which after Apply has already applied is the same, so the popup offers a choice that is not one. Enter applies and closes in one press, so a user cannot apply, look at the grid and then close without reaching for the mouse.

## What Changes

- **app** — MODIFIED: the arrow keys move around the popup: Up and Down between the sort entries, the field, Clear filter and the buttons, Left and Right between the buttons. In the field they move the caret, as now, and leave it only at the top or bottom of the text
- **app** — ADDED: a keyboard shortcut opens the filter popup of the selected column. Ctrl+Shift+F, shown in the shortcut list (Ctrl+?)
- **app** — MODIFIED: OK and Cancel are replaced by one **Close** button, which closes the popup and leaves the filter as last applied. Apply stays
- **app** — MODIFIED: Enter applies a valid expression that differs from the filter in force and keeps the popup open; Enter again, with nothing left to apply, closes it. Enter on an error does nothing

## Impact

- `omasheet-ui/qml/Main.qml`: `columnPopup` (its `accept`, `apply`, the button row, key handling) and the key table
- `docs/app.md`: the Sorting and filtering section and the Keys table
- `omasheet-ui/help.txt`
- `openspec/changes/archive/filter-by-expression/specs/app/spec.md` is the requirement modified

## Out of Scope

- Keys in the other popups and menus
- Changing what a filter is, or how it is checked as it is typed
- A shortcut to clear all filters

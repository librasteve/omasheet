# Tasks: Interactive app

## 1. Editable document (`omasheet-engine`)
- [x] 1.1 Record header, separator and row line positions in the sheet parser
- [x] 1.2 `Document`: snapshot of tables, constants and problems for display
- [x] 1.3 Cell edits that rewrite only the lines they touch, keeping columns aligned
- [x] 1.4 Insert and delete rows; add data columns, formula columns, tables, constants
- [x] 1.5 Undo and redo
- [x] 1.6 Evaluate with errors present, marking only the cells that failed

## 2. Window (`omasheet-ui`)
- [x] 2.1 Rust `Sheet` object exposed to QML through `cxx-qt`
- [x] 2.2 Grid with column headers (name, type or formula) and row numbers
- [x] 2.3 Tabs for tables, and constants as a tab of their own
- [x] 2.4 Entry box and in-cell editing
- [x] 2.5 Block selection by keyboard and mouse; copy, cut, paste, clear
- [x] 2.6 Open, save, save as, new; unsaved-changes prompt
- [x] 2.7 Problems list and per-cell error tooltips
- [x] 2.8 Follow the Omarchy theme colours

## 3. Not yet done
- [ ] 3.1 Try every keyboard shortcut and mouse gesture by hand (only the functions behind them were exercised headlessly)
- [ ] 3.2 Build the window in CI (needs Qt 6 on the runner)
- [ ] 3.3 Desktop entry, icon and package
- [ ] 3.4 A table model that does not send the whole sheet to the window on every edit

# Tasks: Interactive app

## 1. Editable document (`omasheet-engine`)
- [x] 1.1 Record header, separator and row line positions in the file parser
- [x] 1.2 `Document`: snapshot of sheets, constants and problems for display
- [x] 1.3 Cell edits that rewrite only the lines they touch, keeping columns aligned
- [x] 1.4 Insert and delete rows; add data columns, formula columns, sheets, constants
- [x] 1.5 Undo and redo
- [x] 1.6 Evaluate with errors present, marking only the cells that failed

## 2. Window (`omasheet-ui`)
- [x] 2.1 Rust `File` object exposed to QML through `cxx-qt`
- [x] 2.2 Grid with column headers (name, type or formula) and row numbers
- [x] 2.3 Tabs for sheets, and constants as a tab of their own
- [x] 2.4 Entry box and in-cell editing
- [x] 2.5 Block selection by keyboard and mouse; copy, cut, paste, clear
- [x] 2.6 Open, save, save as, new; unsaved-changes prompt
- [x] 2.7 Problems list and per-cell error tooltips
- [x] 2.8 Follow the Omarchy theme colours

## 3. Not yet done
- [ ] 3.1 Try every keyboard shortcut and mouse gesture by hand (only the functions behind them were exercised headlessly)
- [x] 3.2 Build the window in CI (needs Qt 6 on the runner): `ci.yml`
      runs on `archlinux:base-devel` with `qt6-base` and
      `qt6-declarative`, and builds and tests the whole workspace, the
      window included. It passed at 0.1.4; from 0.1.5 it failed at
      `cargo fmt --check`, before building, which `cargo fmt --all` on
      `radix-integers` puts right
- [x] 3.3 Desktop entry, icon and package: `pkgbuild/omasheet.desktop`,
      `omasheet.svg` and the MIME type `omasheet.xml`, installed by
      `pkgbuild/PKGBUILD` (`add-packaging`)
- [ ] 3.4 A sheet model that does not send the whole file to the window on every edit

# Tasks: XLSX, CSV and Markdown interop (Phase 2)

## 0. Groundwork
- [x] 0.1 Notices: what an import or export could not carry, reported without failing the command
- [x] 0.2 `omasheet-interop` crate; a grid of cells to `.omx` source (names to identifiers, column types, cell quoting)
- [x] 0.3 Compile several sources as one sheet (for linked `.omx` files): joined in `omasheet-md`, with diagnostics placed in the file they came from

## 1. CSV (`csv-interop`)
- [x] 1.1 Export via the `csv` crate, one file per table, exact where the value terminates
- [x] 1.2 Import, reading fields as text
- [x] 1.3 Round-trip test over the `examples/` corpus

## 2. XLSX (`xlsx-interop`)
- [x] 2.1 Import via calamine
- [x] 2.2 Export via rust_xlsxwriter with documented lossy mappings

## 3. CLI (`cli`)
- [x] 3.1 `omasheet import` for `.xlsx` and `.csv`
- [x] 3.2 `omasheet export --xlsx | --csv`, with `--table` and `-o`

## 4. Markdown (`markdown-integration`)
- [x] 4.1 `{{ expr }}` interpolation
- [x] 4.2 Fenced `omx` blocks
- [x] 4.3 Decide and implement the syntax for linking an `.omx` file — front matter `sheets:` (design D26)
- [x] 4.4 `omasheet render` with HTML and terminal output

## 5. Finish
- [x] 5.1 `docs/interop.md`, README, an `examples/report.md`
- [x] 5.2 Import and export in the app: Open reads `.xlsx` and `.csv`, Ctrl+E exports
- [ ] 5.3 Archive the change

# Design

- **D1. A declaration, not a fence.** The app opens `.omx` files, so a plot is part of the language. Markdown shows a plot the way it shows a sheet.
- **D2. Controls are `key: value` lines to the next blank line.** The parser takes them itself: left to the sheet rules, `x: Month` would read as a column's type line. `data:` is one line.
- **D3. What `compile` can check, it checks.** Keys, `type`, the numbers, and `data` as an expression that gives a sheet, so `lint` finds them. Whether `x` and `y` name columns of the result is known only once `data` is, and is checked at the checker's end where the sheet's columns are.
- **D4. One drawing path.** `omasheet_engine::plot_svg` builds the table from a plot and hands it to `omasheet-plot`; the report and the app both call it.
- **D5. The drawing is text-free of fonts.** `omasheet-plot` writes `<text>` itself (plotters measures text with a system font), with `currentColor` for ink and no background; the app substitutes the theme's foreground colour, since Qt's `Image` does not resolve `currentColor`.
- **D6. A plot is no row or cell.** Edits to sheets never touch a plot; text-splice edits (`doc.rs`) leave a plot's lines as they are, provided the parser closes the open sheet at `plot`.

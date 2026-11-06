# Design: cut, copy and paste of a block behave as a spreadsheet's do

## Context

Source: the conversation of 2026-11-06. Builds on `paste-as-reference` (D210 to D218) and reverses its D214 for a copy. D numbers continue from there.

## Terms

- A **block** is the cells selected when copying or cutting.
- An **inside read** is a read, by a formula in the block, of another cell of the block; an **outside read** is a read of any other cell. A range or a whole column reads the place, and is outside.
- A **relative read** is a bare column name (the cell of my row), or an offset `[A; *-1]`. A **fixed read** is a row by number or a sheet by name, `[A; 5]` or `T[A; 5]`.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D219 | Decided | A copy keeps relative reads relative; fixed reads are left as they are | Owner, 2026-11-06, for one rule that a spreadsheet user already holds. A block pasted whole keeps its shape, so a relative inside read reads the copy without being rewritten, and a relative outside read shifts with the formula |
| D220 | Decided | A copy is the same on a different sheet as on the same one. This reverses D214 for a copy | A relative read is resolved where it is pasted, as a spreadsheet does. D214 sent every outside read back to the source, which made a copy behave as a move. A column name is a column, not a distance: an outside name is kept, and read on the sheet it is pasted on; an inside name is the column it landed in |
| D221 | Decided | A cut is a move: each read of a formula cut keeps reading the same cell, and the formulas elsewhere that read a cell cut follow it. From another sheet the sheet is named | D214 stands for a cut, which is what it was modelled on. An outside read on another sheet is written as an offset from the formula's new row, so it reads the cell it did |
| D222 | Decided | A transposed paste, of a copy or a cut, reads inside the block at the turned copy, and fixes an outside read to the cell it read, as a reference. Fixed reads are unchanged | Owner, 2026-11-06. An offset has no meaning once rows are columns, so it cannot be kept. A reference to a place is what Paste Reference writes, so the same form is used. It breaks when rows are inserted before it, as any reference by place does |
| D223 | Decided | A copy onto the same sheet maps an inside column name to the column it landed in | Today the text is pasted as written, so a name does not follow the copy on one sheet while it does on another (D214) |
| D224 | Decided | Values and References do not change: Values has no formulas to rewrite, and References writes a fixed read of the source cell (D211, D212) | |
| D225 | Decided | The status bar says nothing about a paste that mixes reads at the copy with reads of the original | Owner, 2026-11-06 |

## Holding

- A fixed read is a place and not a row's identity: if rows are inserted above it, it reads what is there now. The docs say so for a reference already.
- A column name kept for an outside read has no column to read if the sheet pasted on lacks it. The cell shows the usual error for an unknown column; nothing is rewritten to hide it.

## Risks

- D214 was decided the day before it is reversed. A file pasted under it holds `Sales[A; *]`-style reads that still work: a fixed read is left as it is.
- The cut rule and the copy rule differ on another sheet, so the same block pasted by Ctrl+C and by Ctrl+X reads differently. That is the point of the two keys, and the docs say so with the two results side by side.

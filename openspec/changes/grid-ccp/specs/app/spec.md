# app

## MODIFIED Requirements

### Requirement: A copied formula keeps what it read

A copy SHALL keep a formula's relative reads relative, and SHALL leave its fixed reads as they are, on the same sheet and on a different one. What it read inside the copied block SHALL be read at the copy, in the column it landed in. What it read outside the block SHALL be read from where it is pasted: a column name is kept and read on the sheet pasted on, and an offset counts from the formula's new row. A range or a whole column SHALL count as outside.

A cut SHALL be a move. Each read of a formula cut SHALL go on reading the same cell, and a formula elsewhere that read a cell cut SHALL read it where it has gone. A read of another sheet SHALL name that sheet. Pasted into a formula column a formula SHALL be the column's formula, as before.

#### Scenario: A block copied whole

- **GIVEN** `A`, `B` and `C = A + B` of `Sales` copied together
- **WHEN** they are pasted onto columns `P`, `Q` and `R` of another sheet, or onto `D`, `E` and `F` of the same one
- **THEN** the pasted `C` is `= P+Q`, or `= D+E`

#### Scenario: A copy without what it read

- **GIVEN** `B` and `C = A + B` of `Sales` copied, without `A`
- **WHEN** they are pasted onto `P` and `Q` of another sheet
- **THEN** `Q` is `= A+P`, reading the column `A` of the sheet pasted on

#### Scenario: A row above the block

- **GIVEN** `C = [A; *-1] + A` in the second row of `Sales`, copied with `A` and `B` of that row only
- **WHEN** they are pasted onto another sheet
- **THEN** the formula is `= [A; *-1] + P`, reading the row above on the sheet pasted on

#### Scenario: A fixed read

- **GIVEN** a formula `= Sales[A; 5] + B` copied with `B`
- **WHEN** it is pasted onto another sheet
- **THEN** it reads `Sales[A; 5]` as before

#### Scenario: A cut formula on its own sheet

- **GIVEN** `C = A + B` in the first row of `Sales`, cut without `A` and `B`
- **WHEN** it is pasted three rows down
- **THEN** it is `= [A; *-3] + [B; *-3]`
- **AND** it reads the cells it read

#### Scenario: A cut formula on another sheet

- **GIVEN** the same cell cut
- **WHEN** it is pasted onto another sheet at the same row
- **THEN** it is `= Sales[A; *] + Sales[B; *]`

#### Scenario: What read a cell cut

- **GIVEN** a formula that reads the cell `[A; 3]`, and that cell cut and pasted two rows down
- **THEN** the formula reads `[A; 5]`

## ADDED Requirements

### Requirement: Paste Transposed fixes what a formula read outside the block

A transposed paste, of a copy or a cut, SHALL read what a formula read inside the block at the turned copy, and SHALL fix what it read outside the block to the cell it read, as a reference: the row by number, and a column counted from its own by name, with the source sheet named where that is another sheet. Fixed reads SHALL be left as they are. A range or a whole column SHALL count as outside and SHALL be left as it is. A cut SHALL also point the formulas elsewhere that read the cells cut at where each went. No notice SHALL be given for a paste that has both kinds of read.

#### Scenario: A block copied whole

- **GIVEN** `A`, `B` and `C = A + B` of rows 0 and 1 copied, with `C` of row 1 being `[A; *-1] + B`
- **WHEN** they are pasted transposed onto the same sheet at row 3
- **THEN** the formulas read the turned cells: `= [A; *-2]+[A; *-1]` and `= [A; *-2]+[B; *-1]`
- **AND** their values are those the originals had

#### Scenario: Without a column it read

- **GIVEN** `B` and `C` of rows 0 and 1 copied, without `A`
- **WHEN** they are pasted transposed onto the same sheet at row 3, column A
- **THEN** the formulas are `= [A; 0]+[A; *-1]` and `= [A; 0]+[B; *-1]`
- **AND** the first read of `A` is the cell it read, not the cell in the row it is now in

#### Scenario: Onto another sheet

- **GIVEN** the same copy
- **WHEN** it is pasted transposed onto another sheet at its first cell
- **THEN** the formulas are `= T[A; 0]+[A; *-1]` and `= T[A; 0]+[B; *-1]`

#### Scenario: A cut

- **GIVEN** cells cut and a formula elsewhere that reads one of them
- **WHEN** they are pasted transposed
- **THEN** that formula reads the cell where it went

### Requirement: A column name inside a copied block is relative to the block

Inside a copied block a column name SHALL be read in the column it landed in. A column given by number SHALL be fixed and SHALL be left as it is. Outside the block a column name SHALL be kept and read on the sheet pasted on.

#### Scenario: A name inside the block

- **GIVEN** `A`, `B` and `C = A + B` copied together
- **WHEN** they are pasted onto `D`, `E` and `F` of the same sheet
- **THEN** `F` is `= D+E`

#### Scenario: A numbered column inside the block

- **GIVEN** `A`, `B` and `C = [0; *] + B` copied together
- **WHEN** they are pasted onto `D`, `E` and `F`
- **THEN** `F` is `= [0; *]+E`, still reading the first column

### Requirement: A formula may read a fixed row of its own column

A formula SHALL be reported as circular only if a cell reads itself, directly or through the cells it reads. A fixed row of its own column that is another cell, and does not read back, SHALL NOT be circular, whatever the column. The report SHALL name the cell.

#### Scenario: A seed in the first row

- **GIVEN** a column whose first row is `100` and whose other rows are `= [B; 0] * A`
- **THEN** no cell is circular

#### Scenario: A cell that reads itself

- **GIVEN** the third row of a column is `= [B; 2] + 1`
- **THEN** that cell is reported as circular, naming its row

#### Scenario: A frozen read

- **GIVEN** a transposed paste puts `= T[A; 3]` into column `A`
- **THEN** the cell is not circular unless row 3 of `A` reads it back

### Requirement: Paste Values and Paste Reference are not changed by the rules above

Paste Values SHALL paste what the cells showed, with no formula to rewrite. Paste Reference SHALL write a fixed read of each cell it was made from, as before.

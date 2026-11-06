# app

## ADDED Requirements

### Requirement: A read-only sheet takes no edit

A sheet whose rows are a Parquet file's, or that is calculated from an expression, SHALL take no edit. An attempt to start one, by typing, Enter or a double-click, SHALL NOT open the editor, and SHALL put the reason in the status bar. Clear, delete and paste onto such a sheet SHALL do the same, and SHALL change nothing. A cut from one SHALL be a copy, and the status bar SHALL say so.

#### Scenario: Typing on a view

- **GIVEN** a `view` over a Parquet file with a cell selected
- **WHEN** a key is typed
- **THEN** no editor opens
- **AND** the status bar says that the view is read-only and names its file

#### Scenario: Cutting from a view

- **GIVEN** cells of a view selected
- **WHEN** they are cut
- **THEN** they are copied, and are not cleared
- **AND** the status bar says that they were copied, as a view cannot be cut from

### Requirement: Paste as Reference

A copy SHALL remember the sheet, the columns and the rows it came from. **Paste as Reference** SHALL paste a live reference to the copied cells, where Ctrl+V pastes their values or contents as before.

Into data cells it SHALL write a formula in each, such as `= Passengers[Fare; 3]`. Into a formula column it SHALL set the column's formula, aligned by row: `Passengers[Fare; *]` for a whole column; `Passengers[Fare; *+5]` where the rows copied start five below the rows pasted onto, and `*-5` where above; and `Passengers[Fare; 3]` for one cell. A block pasted onto several formula columns SHALL give each its own formula, in order. The name of the sheet SHALL be left out when the copy is from the same sheet. A reference to a row SHALL be to its place in the source, not its place in a sorted grid.

The status bar SHALL say what was written. What is put on the system clipboard SHALL remain tab-separated text.

#### Scenario: A view's column into a decorator

- **GIVEN** a column copied from the view `Passengers`, and a formula column on another sheet
- **WHEN** Paste as Reference is used on the formula column
- **THEN** its formula is `Passengers[Fare; *]`

#### Scenario: Rows with an offset

- **GIVEN** `Passengers[Fare]` rows 5 to 9 copied, and a formula column with its first row selected
- **WHEN** Paste as Reference is used
- **THEN** its formula is `Passengers[Fare; *+5]`
- **AND** a row past the end of the source is empty

#### Scenario: One cell

- **GIVEN** the cell in row 3 of `Passengers.Fare` copied
- **WHEN** Paste as Reference is used on data cells
- **THEN** each takes the formula `= Passengers[Fare; 3]`

#### Scenario: A decorator on the same view

- **GIVEN** a column of a view copied, and a decorator column on that view
- **WHEN** Paste as Reference is used on the decorator
- **THEN** its formula is `Fare`, with no sheet name

#### Scenario: A sorted view

- **GIVEN** a view sorted in the grid, with the cell in grid row 0 copied
- **WHEN** Paste as Reference is used
- **THEN** the reference is to the row that cell has in the source

#### Scenario: Ctrl+V is unchanged

- **GIVEN** the same copy
- **WHEN** Ctrl+V is used on data cells
- **THEN** the values are pasted, as before

### Requirement: A copied formula keeps what it read

*Superseded for a copy by `grid-ccp` (D220): a copy now reads relative to where it is pasted. This holds for a cut.*

A formula copied or cut from one sheet and pasted onto a different one SHALL read what it read. What it read inside the copied block SHALL be read at the copy, on the sheet it is on now, in the column it landed in. What it read outside the block SHALL be read where it was, with the source sheet named. A range or a whole column SHALL count as outside. Pasted into a formula column it SHALL be the column's formula, and SHALL read the sheet it came from.

#### Scenario: A block copied whole

- **GIVEN** `A`, `B` and `C = A + B` of `Sales` copied together
- **WHEN** they are pasted onto columns `P`, `Q` and `R` of another sheet
- **THEN** `R` is `= P+Q`

#### Scenario: A formula copied without what it read

- **GIVEN** `B` and `C = A + B` of `Sales` copied, without `A`
- **WHEN** they are pasted onto `P` and `Q` of another sheet
- **THEN** `Q` is `= Sales[A; *]+P`

#### Scenario: A row above the block

- **GIVEN** `C = [A; *-1] + A` in the second row of `Sales`, copied with `A` and `B` of that row only
- **WHEN** they are pasted onto another sheet
- **THEN** the formula is `= Sales[A; *-1] + P`

#### Scenario: Its own sheet

- **GIVEN** a formula copied
- **WHEN** it is pasted onto the sheet it came from
- **THEN** it is pasted as written

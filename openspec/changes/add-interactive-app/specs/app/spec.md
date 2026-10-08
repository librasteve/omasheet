# app

The interactive Omasheet window.

## ADDED Requirements

### Requirement: Grid of the current table
The app SHALL show one table at a time as a grid with one row per table row and
one column per table column, including computed columns. Each cell SHALL show its
calculated value. Column headers SHALL show the column name and its type, or for
a computed column its expression. Rows SHALL be numbered from zero.

#### Scenario: Open a sheet
- **GIVEN** `budget.omx` with a table `Sales` and a computed column `Profit := Revenue - Cost`
- **WHEN** it is opened in the app
- **THEN** the grid shows the `Sales` rows with a `Profit` column of calculated values
- **AND** the `Profit` header shows `:= Revenue - Cost`

### Requirement: Tabs
When a sheet has more than one table the app SHALL offer a tab for each, and
SHALL show the sheet's constants as a further tab listing each constant's name,
expression and value.

#### Scenario: Switch table
- **GIVEN** a sheet with tables `Sales` and `Summary`
- **WHEN** the `Summary` tab is chosen
- **THEN** the grid shows `Summary`

### Requirement: Entry box
The app SHALL provide an entry box showing the source of the current cell: its
literal text, or its formula including the leading `=`. Committing text in the
entry box, or in the cell itself, SHALL replace the cell's source.

#### Scenario: Edit a value
- **GIVEN** the current cell holds `12000`
- **WHEN** `12500` is typed and committed
- **THEN** the cell shows `12500` and every value that depends on it is recalculated

#### Scenario: Enter a formula
- **WHEN** `= Sales[*-1; Revenue] * 1.1` is committed into a cell
- **THEN** the cell shows the calculated value and the entry box shows the formula

#### Scenario: Edit a computed column
- **GIVEN** the current cell is in the computed column `Profit`
- **WHEN** `Revenue - Cost - Tax` is committed
- **THEN** the column's expression changes and every row of `Profit` is recalculated

### Requirement: Selection and clipboard
The app SHALL allow a rectangular block of cells to be selected with the keyboard
and with the mouse, and SHALL support copy, cut, paste and clear on the
selection. Copied cells SHALL be placed on the system clipboard as tab-separated
text. Pasting tab-separated text SHALL fill cells starting at the selection,
adding rows if the text extends past the last row. Pasting a single value into a
larger selection SHALL fill the selection.

#### Scenario: Copy and paste a block
- **GIVEN** a 2 × 3 block is selected and copied
- **WHEN** it is pasted with the cursor two rows below the last row
- **THEN** the table gains rows and the block's sources appear there

#### Scenario: Paste from another program
- **GIVEN** the clipboard holds `Apr<TAB>14000<NEWLINE>May<TAB>15000`
- **WHEN** it is pasted at the first cell of a new row
- **THEN** two rows are filled

### Requirement: Rows, columns, tables and constants
The app SHALL be able to insert and delete rows, add a data column, add a
computed column from a name and an expression, add a table, and add a constant.
It SHALL refuse a name that is not valid or is already in use, saying why.

#### Scenario: Duplicate column
- **WHEN** a column named `Revenue` is added to a table that has one
- **THEN** nothing changes and the app says the name is already used

### Requirement: The file is the sheet
The app SHALL open and save `.omx` files and SHALL NOT keep any state outside
that text. An edit SHALL change only the lines of the table it touches, leaving
comments, constants, other tables and line order as they were. The app SHALL warn
before discarding unsaved changes.

#### Scenario: One-line diff
- **GIVEN** a saved sheet under version control
- **WHEN** one cell is changed to a value of the same width and the sheet is saved
- **THEN** the diff is one changed line

#### Scenario: Comments survive
- **GIVEN** a sheet with `#` comments between declarations
- **WHEN** a cell is edited and the sheet is saved
- **THEN** every comment is still present

### Requirement: Undo
Every edit SHALL be undoable and redoable, one edit per step, where a paste or a
clear of a block is one edit.

#### Scenario: Undo a paste
- **WHEN** a block is pasted and undo is used once
- **THEN** the sheet is exactly as it was before the paste

### Requirement: Errors do not blank the grid
When a sheet has errors the app SHALL still calculate and show every cell that
can be calculated, SHALL mark each cell that cannot, and SHALL list the problems
with their line numbers.

#### Scenario: One mistyped formula
- **GIVEN** a sheet with ten valid computed columns
- **WHEN** a cell is given the formula `= Revnue * 2`
- **THEN** that cell and the cells that read it show as errors, with the message for the unknown name
- **AND** cells that do not depend on it keep their values

### Requirement: Platform look
The app SHALL take its colours from the current Omarchy theme, and fall back to
built-in colours on other systems.

#### Scenario: Theme change
- **WHEN** the Omarchy theme changes while the app is open
- **THEN** the app uses the new colours the next time its window is activated

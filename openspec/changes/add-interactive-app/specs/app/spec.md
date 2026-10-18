# app

The interactive Omasheet window.

## ADDED Requirements

### Requirement: Grid of the current sheet
The app SHALL show one sheet at a time as a grid with one row per sheet row and
one column per sheet column, including computed columns. Each cell SHALL show its
calculated value. Column headers SHALL show the column name and its type, or for
a computed column its expression. Rows SHALL be numbered from zero.

#### Scenario: Open a file
- **GIVEN** `budget.omx` with a sheet `Sales` and a computed column `Profit := Revenue - Cost`
- **WHEN** it is opened in the app
- **THEN** the grid shows the `Sales` rows with a `Profit` column of calculated values
- **AND** the `Profit` header shows `:= Revenue - Cost`

### Requirement: Tabs
When a file has more than one sheet the app SHALL offer a tab for each, and
SHALL show the file's constants as a further tab listing each constant's name,
expression and value.

#### Scenario: Switch sheet
- **GIVEN** a file with sheets `Sales` and `Summary`
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
- **WHEN** `= Sales[Revenue; *-1] * 1.1` is committed into a cell
- **THEN** the cell shows the calculated value and the entry box shows the formula

#### Scenario: Edit a computed column
- **GIVEN** the current cell is in the computed column `Profit`
- **WHEN** `Revenue - Cost - Tax` is committed
- **THEN** the column's expression changes and every row of `Profit` is recalculated

### Requirement: Dates and times follow the locale
The grid SHALL show `Date`, `Time` and `DateTime` values the way the locale of
the operating system writes them, with a four-digit year. A date or time
committed into a cell of such a column, or of a column with no type yet, SHALL
be accepted in the locale's form as well as the ISO form, and SHALL be stored in
the file in the ISO form. The entry box SHALL show the stored ISO form.

#### Scenario: A British date
- **GIVEN** a British locale and a cell whose source is `2026-10-08`
- **THEN** the grid shows `08/10/2026`

#### Scenario: Typing a date the local way
- **GIVEN** a British locale and a column declared `Date`
- **WHEN** `9/10/2026` is committed into a cell of it
- **THEN** the file holds `2026-10-09` and the grid shows `09/10/2026`

#### Scenario: The same keys elsewhere
- **GIVEN** a United States locale and a column declared `Date`
- **WHEN** `9/10/2026` is committed into a cell of it
- **THEN** the file holds `2026-09-10`

### Requirement: Date-times on local clocks
The grid SHALL show the `DateTime` values of a file in another time zone on the
clocks of the machine. A `DateTime` moved to a zone with `.to_zone()` or `.utc()`
SHALL be shown in that zone with its offset. The entry box SHALL show, and what
is committed SHALL be taken in, the file's zone; the file SHALL NOT change
because of where it is opened.

#### Scenario: A New York file in London
- **GIVEN** a file that begins `zone America/New_York`, opened in London
- **WHEN** a cell holds `2025-03-03T09:12`
- **THEN** the grid shows 14:12 and the entry box shows `2025-03-03T09:12`

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
- **THEN** the sheet gains rows and the block's sources appear there

#### Scenario: Paste from another program
- **GIVEN** the clipboard holds `Apr<TAB>14000<NEWLINE>May<TAB>15000`
- **WHEN** it is pasted at the first cell of a new row
- **THEN** two rows are filled

### Requirement: Keys by selection
The app SHALL give each kind of selection the keys below; a blank is a key that
does nothing there. A formula cell is one cell of a formula column, which stands
for the whole column.

|                  | cell             | formula cell        | cell block       | row              | col              | sheet            |
|------------------|------------------|---------------------|------------------|------------------|------------------|------------------|
| move             | ← → ↑ ↓          |                     |                  | ↑ ↓              | ← →              | Ctrl + Alt + ← → |
| edit             | Enter            | Enter → formula box |                  |                  |                  |                  |
| edit (F2)        | formula box      | rename              |                  |                  | rename           | rename in tab    |
| multiselect      | Shift + ← → ↑ ↓  | Shift + ← → ↑ ↓     |                  | Shift + ↑ ↓      | Shift + ← →      |                  |
| deselect         | Esc              | Esc                 | Esc              | Esc              | Esc              | Esc              |
| cut              | Ctrl + X         |                     | Ctrl + X         | Ctrl + X         | Ctrl + X         | Ctrl + X         |
| copy             | Ctrl + C         | Ctrl + C            | Ctrl + C         | Ctrl + C         | Ctrl + C         | Ctrl + C         |
| paste            | Ctrl + V         | Ctrl + V            | Ctrl + V         | Ctrl + V         | Ctrl + V         | Ctrl + V         |
| paste values     | Ctrl + Shift + V |                     | Ctrl + Shift + V | Ctrl + Shift + V | Ctrl + Shift + V | Ctrl + Shift + V |
| paste transposed | Ctrl + Alt + V   |                     | Ctrl + Alt + V   |                  |                  |                  |
| clear            | Delete           |                     | Delete           | Delete           | Delete           | Delete           |
| delete           |                  |                     |                  | Ctrl + Delete    | Ctrl + Delete    | Ctrl + Delete    |

#### Scenario: Enter on a block
- **GIVEN** a 2 × 3 block of cells is selected
- **WHEN** Enter is pressed
- **THEN** nothing is edited and the selection stays as it is

### Requirement: Picking references in a formula
While a formula is typed in a cell, the app SHALL let cells be picked into it as
references, a single cell, a range or a list, with the keys and clicks below.
A reference is relative to the edited cell, or absolute by row number with Alt.

| In a formula              | Writes                                                |
|---------------------------|-------------------------------------------------------|
| Arrow                     | one cell, relative: `[*; *-1]`                        |
| Shift + Arrow             | a range, relative: `[*; *-3..*-1]`                    |
| Ctrl + Arrow              | the first or last cell that way                       |
| Ctrl + Shift + Arrow      | a range to the first or last cell                     |
| Click                     | one cell, relative                                    |
| Drag                      | a range, relative                                     |
| Ctrl + Click / Drag       | one more in a list, after a comma: `sum([*; *-3], [*+1; *-1])` |
| Alt with any of the above | absolute, by row number: `[Revenue; 3]`, `[Revenue; 0..2]` |

The cells picked SHALL be highlighted as the pick grows.

#### Scenario: Sum a range by keyboard
- **GIVEN** `= sum(` is typed in a cell three rows down a column
- **WHEN** Shift + ↑ is pressed three times
- **THEN** the formula reads `= sum([*; *-3..*-1]` and those three cells are highlighted

#### Scenario: Sum a list by mouse
- **GIVEN** `= sum(` is typed in a cell
- **WHEN** one cell is clicked and another is clicked with Ctrl
- **THEN** both references are written, separated by a comma

### Requirement: Rows, columns, sheets and constants
The app SHALL be able to insert and delete rows, add a data column, add a
computed column from a name and an expression, add a sheet, and add a constant.
It SHALL refuse a name that is not valid or is already in use, saying why.

#### Scenario: Duplicate column
- **WHEN** a column named `Revenue` is added to a sheet that has one
- **THEN** nothing changes and the app says the name is already used

### Requirement: The text is the file
The app SHALL open and save `.omx` files and SHALL NOT keep any state outside
that text. An edit SHALL change only the lines of the sheet it touches, leaving
comments, constants, other sheets and line order as they were. The app SHALL warn
before discarding unsaved changes.

#### Scenario: One-line diff
- **GIVEN** a saved file under version control
- **WHEN** one cell is changed to a value of the same width and the file is saved
- **THEN** the diff is one changed line

#### Scenario: Comments survive
- **GIVEN** a file with `#` comments between declarations
- **WHEN** a cell is edited and the file is saved
- **THEN** every comment is still present

### Requirement: Undo
Every edit SHALL be undoable and redoable, one edit per step, where a paste or a
clear of a block is one edit.

#### Scenario: Undo a paste
- **WHEN** a block is pasted and undo is used once
- **THEN** the file is exactly as it was before the paste

### Requirement: Errors do not blank the grid
When a file has errors the app SHALL still calculate and show every cell that
can be calculated, SHALL mark each cell that cannot, and SHALL list the problems
with their line numbers.

#### Scenario: One mistyped formula
- **GIVEN** a file with ten valid computed columns
- **WHEN** a cell is given the formula `= Revnue * 2`
- **THEN** that cell and the cells that read it show as errors, with the message for the unknown name
- **AND** cells that do not depend on it keep their values

### Requirement: Platform look
The app SHALL take its colours from the current Omarchy theme, and fall back to
built-in colours on other systems.

#### Scenario: Theme change
- **WHEN** the Omarchy theme changes while the app is open
- **THEN** the app uses the new colours the next time its window is activated

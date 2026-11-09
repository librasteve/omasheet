# app

## ADDED Requirements

### Requirement: Editing a formula lights its precedents

While a formula is edited in a cell, whether opened with F2 or typed, the app SHALL give each reference it holds a colour, apply that colour to the reference in the formula's text, and highlight the cells or columns the reference names on the grid in the same colour. Different references SHALL have different colours, repeating only when the palette is used up. The colours SHALL follow the text as it is changed, SHALL be drawn legibly in both themes, and SHALL go when editing ends or is cancelled. Nothing SHALL be written to the file for them.

#### Scenario: F2 on a formula

- **GIVEN** a cell holding `= [A; *-3] + [B; *]`
- **WHEN** F2 is pressed on it
- **THEN** `[A; *-3]` and the cell it names are in one colour, and `[B; *]` and its cell in another

#### Scenario: Editing the formula

- **GIVEN** the cell is being edited
- **WHEN** `*-3` is changed to `*-2`
- **THEN** the highlighted cell moves up to the one it now names

#### Scenario: Escape

- **WHEN** Esc is pressed
- **THEN** the colours are gone and the formula is as it was

### Requirement: Columns are moved by dragging their headers

A column SHALL be dragged by its header to a new place, with a mark where it will land, as the sheet tabs are. Several selected columns SHALL move as a block. The drop SHALL make the same edit as moving the columns any other way, and SHALL be one undo step. A drop that the sheet does not allow SHALL move nothing and say why.

#### Scenario: Drag a column

- **GIVEN** columns `A B C D`
- **WHEN** `A`'s header is dragged between `C` and `D` and dropped
- **THEN** the columns are `B C A D`, and one undo restores `A B C D`

#### Scenario: A block

- **GIVEN** columns `A B C D` with `A` and `B` selected
- **WHEN** `B`'s header is dragged after `D`
- **THEN** the columns are `C D A B`

#### Scenario: Refused

- **GIVEN** a sheet whose columns cannot be reordered
- **WHEN** a header is dragged and dropped
- **THEN** nothing moves and a notice says why

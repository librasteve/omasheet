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

### Requirement: A reference is moved by dragging its cells

While a formula is edited, a press on a cell that a `[cols; rows]` reference reads, without Ctrl or Alt, SHALL take hold of that reference, and dragging SHALL move it, keeping its shape, by rewriting it in the formula where it stands. The cells it read before SHALL no longer be marked, and the reference SHALL keep its colour. A click on a cell with the caret inside a written reference SHALL replace that reference with the cell's.

#### Scenario: Drag the blue cell

- **GIVEN** a cell holding `= [Revenue; *-1] + [Cost; *-2]` being edited, the first reference blue and the second red
- **WHEN** the blue cell is pressed and dragged one row down
- **THEN** the formula reads `= [Revenue; *] + [Cost; *-2]`
- **AND** the cell it left is no longer blue, and the one it landed on is

#### Scenario: A range keeps its shape

- **GIVEN** `= sum([Revenue; *-3..*-1])` being edited
- **WHEN** one of its three cells is dragged up one row
- **THEN** the formula reads `= sum([Revenue; *-4..*-2])`

#### Scenario: Click to replace

- **GIVEN** the caret is inside `[Revenue; *-1]`
- **WHEN** another cell is clicked
- **THEN** that reference is replaced by the one for the cell clicked, and the old one is gone

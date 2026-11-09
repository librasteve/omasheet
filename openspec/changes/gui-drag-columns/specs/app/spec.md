# app

## ADDED Requirements

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

# app

A column's filter.

## MODIFIED Requirements

### Requirement: A column is filtered by an expression typed in its popup

A column's popup SHALL hold its sort entries, a field for the filter expression showing the filter in force with the hint `e.g. * >= 3` while it is empty, Clear filter, Apply and Close. Opening it SHALL read no row of the column. The expression SHALL be checked as it is typed. Apply SHALL be enabled only when the expression has no error and differs from the filter in force, SHALL then be drawn filled, and SHALL apply it and keep the popup open; Clear filter SHALL remove the filter and keep the popup open. Close and Esc SHALL close the popup and leave the filter as last applied. Enter SHALL apply a valid expression that differs from the filter in force and keep the popup open; Enter with nothing left to apply SHALL close it; Enter with an error SHALL do nothing. The arrow keys SHALL move between the sort entries, the field, Clear filter and the buttons. Filters on several columns SHALL all apply.

#### Scenario: Apply, look, close

- **WHEN** `* > 10` is typed and Enter is pressed
- **THEN** the grid shows the rows over 10 and the popup stays open
- **WHEN** Enter is pressed again
- **THEN** the popup closes and the filter stays in force

#### Scenario: Enter on an error

- **WHEN** `0 < * <=` is typed and Enter is pressed
- **THEN** nothing is applied and the popup stays open

#### Scenario: Close leaves the filter

- **GIVEN** `* > 10` applied, and `* > 20` typed and not applied
- **WHEN** Close is pressed
- **THEN** the popup closes with `* > 10` in force

#### Scenario: Moving by arrow keys

- **GIVEN** the popup is open with the field focused
- **WHEN** Down is pressed on a one-line expression
- **THEN** focus moves to Clear filter, and then to the buttons, where Left and Right move between them

## ADDED Requirements

### Requirement: A shortcut opens the filter

Ctrl+Shift+F SHALL open the filter popup of the selected column with its field focused, and SHALL be listed with the other shortcuts.

#### Scenario: From the grid

- **GIVEN** a cell in column `Fare` is selected
- **WHEN** Ctrl+Shift+F is pressed
- **THEN** the popup of `Fare` opens with its filter, if any, selected for replacing

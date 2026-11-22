## ADDED Requirements

### Requirement: F6 moves the keyboard to the style bar

F6 SHALL put the keyboard on the style bar, on the button last used, and F6 or Esc SHALL return it to the grid. Shift+F6 SHALL start at the last button. The arrow keys SHALL move over the buttons that can be used, and Enter, Space or Down SHALL use the one the keyboard is on.

#### Scenario: Toggle bold from the keyboard

- **GIVEN** whole columns are selected
- **WHEN** F6, Right and Enter are pressed on the first button, then Right, Right, Enter
- **THEN** the selection is bold and the keyboard is still on the bar

## MODIFIED Requirements

### Requirement: F4 goes round the column settings

F4 SHALL open the filter, type, unit and format boxes in turn, the filter first, and Shift+F4 the other way. A box that does not suit the selection SHALL be passed by: the filter for several columns, the format for a column with no type.

## REMOVED Requirements

### Requirement: A shortcut opens the filter

Ctrl+Shift+F no longer opens the filter popup; F4 does. Ctrl+Shift+F is Paste format.

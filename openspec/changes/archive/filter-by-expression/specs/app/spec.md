# app

A column's filter.

## ADDED Requirements

### Requirement: A column is filtered by an expression typed in its popup

A column's popup SHALL hold its sort entries, a field for the filter
expression showing the filter in force with the hint `e.g. * >= 3` while
it is empty, Clear filter, Apply, OK and Cancel. Opening it SHALL read no
row of the column. The expression SHALL be checked as it is typed. Apply
SHALL be enabled only when the expression has no error and differs from
the filter in force, SHALL then be drawn filled (OK otherwise), and SHALL apply it and keep the popup open; Clear filter
SHALL remove the filter and keep the popup open; OK and
Enter SHALL apply it and close the popup, and only when it has no error. Esc or Cancel SHALL leave the
filter as it was. Filters on several columns SHALL all apply.

#### Scenario: Opening the popup on a big table

- **GIVEN** a table of millions of rows
- **WHEN** a column's popup is opened
- **THEN** it opens with no read of the column

#### Scenario: An error as it is typed

- **WHEN** `0 < * <=` is typed
- **THEN** the field is outlined in red, its tooltip says what is wrong,
  and Apply and OK are disabled

#### Scenario: Trying filters in turn

- **WHEN** `* > 10` is typed and Apply is pressed, then `* > 20`
- **THEN** the grid shows the rows over 10, the popup stays open, and
  Apply is enabled again for `* > 20`

#### Scenario: Applying

- **WHEN** `* > 10` is typed and Enter is pressed
- **THEN** the grid shows the rows whose value is over 10
- **AND** the column's telltale shows it is filtered

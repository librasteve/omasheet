# markdown-integration

`{{ }}` by a date or time column's format.

## MODIFIED Requirements

### Requirement: Inline values
A `{{ }}` value that is a cell of a date or time column with a format SHALL
be rendered by that format.

#### Scenario: A formatted date in Markdown
- **GIVEN** a column `Due : Date[%e %B %Y]` and a cell `2025-01-05`
- **WHEN** `{{ Invoices.Due[0] }}` is rendered
- **THEN** it is rendered as ` 5 January 2025`

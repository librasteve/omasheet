# markdown-integration

`{{ }}` by a column's format.

## MODIFIED Requirements

### Requirement: Inline values
A `{{ }}` value that is a cell of a column with a format SHALL be rendered
by that format.

#### Scenario: A formatted cell in Markdown
- **GIVEN** a column `Total : Decimal['.2f]` and a cell `12345.678`
- **WHEN** `{{ Sheet.Total[0] }}` is rendered in a locale that writes `12,345.6`
- **THEN** it is rendered as `12,345.68`

## ADDED Requirements

### Requirement: A currency's sign may follow the amount

`$>` in a `Currency` format SHALL show each amount with its sign after the number, and `$>_` with a space between. A currency with no sign SHALL show its code after the number either way.

#### Scenario: Yen

- **GIVEN** `Price : Currency[JPY; $>.0f]` holding `144¥`
- **WHEN** the sheet is shown
- **THEN** the cell reads `144¥`, and with `$>_` it reads `144 ¥`

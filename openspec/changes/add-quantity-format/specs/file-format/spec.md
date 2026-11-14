# file-format

## ADDED Requirements

### Requirement: A quantity format with no unit
The unit part of a `Quantity` column's format MAY be left out when a number format follows the `;`: `Quantity[; .9s]`, `Mass[; .9s]`. A value SHALL then be shown in its own unit, and a named kind SHALL keep its own unit for a bare number. A format with neither a unit nor a number format SHALL be an error.

#### Scenario: A mixed column
- **WHEN** a `Quantity[; .9s]` column holds `1.054571817e-34J.s` and `96485.33212331C`
- **THEN** each is shown in its own unit, to nine significant digits

#### Scenario: Nothing at all
- **WHEN** a column is declared `Quantity[]`
- **THEN** it is an error that the format needs a unit

### Requirement: A space before the unit
The flag `_` SHALL be accepted first in a `Scientific` or `Quantity` number format, after any `±` or `~`, and SHALL set a quantity's unit apart from its number by one space. It SHALL be accepted alone, `Quantity[; _]`.

#### Scenario: A gap
- **WHEN** `6.02214076e23mol^-1` is shown under `Quantity[; _]`
- **THEN** it is `6.022e+23 mol^-1·`

### Requirement: The `s` conversion
The conversion `s` SHALL be accepted by a `Scientific` or a `Quantity` format. A value with an uncertainty SHALL be shown as it is with no precision, its digits set by the uncertainty. A value with none SHALL be shown as `g` shows it, with the precision as its number of significant digits, and without `(0)` for a `Quantity`.

#### Scenario: Exact and measured in one column
- **WHEN** `1.0545718176461565e-34J.s` and `6.674_30(15)e-11m` are shown under `Quantity[; .9s]`
- **THEN** they are `1.05457182e-34J.s` and `6.674 30(15)e-11m`

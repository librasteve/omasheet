# omx-expressions

The expression language: its literals, its operators and its functions.

## MODIFIED Requirements

### Requirement: The conversion functions

The conversions SHALL be `Int`, `Decimal`, `Percent`, `Float`, `String`,
`Binary`, `Boolean`, `Date`, `Time` and `DateTime`, each written `x.Name`,
`x.Name()` or `Name(x)`.

#### Scenario: Converting to a floating-point number

- **GIVEN** the expression `(1/3).Float`
- **WHEN** it is evaluated
- **THEN** the result is `0.3333333333333333`
- **AND** it is a `Float`

## ADDED Requirements

### Requirement: A column list

A list of columns SHALL be expressible as a value, so that an operation taking
several columns — a selection, a group key, a join key — can be given one.

#### Scenario: Several columns are selected by name

- **GIVEN** a sheet with columns `Month`, `Revenue` and `Cost`
- **WHEN** `Revenue` and `Cost` are selected together
- **THEN** the result is those two columns
- **AND** the selection is one value, not two expressions

## REMOVED Requirements

### Requirement: Imaginary literals

`4i`, `2.5i` and `Inf\i` are no longer numbers, and `3+4i` is no longer a
complex number. The lexer does not produce an imaginary token.

### Requirement: The complex functions

`re`, `im`, `conj` and `arg` are no longer functions, and `Complex(re, im)` is
no longer a constructor. `abs`, `sqrt`, `exp` and `ln` keep their real
behaviour and no longer have a complex one.

### Requirement: The fraction conversions

`x.Fraction` is no longer a conversion. A fraction written in a cell is read as
an exact `Decimal`, so `x.Decimal` covers what it did.

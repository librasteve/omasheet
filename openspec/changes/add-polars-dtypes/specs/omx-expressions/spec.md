# omx-expressions

The expression language: its literals, its operators and its functions.

## MODIFIED Requirements

### Requirement: The conversion functions

The conversions SHALL be `Int`, `Decimal`, `Fraction`, `Percent`, `Float`,
`String`, `Binary`, `Boolean`, `Date`, `Time` and `DateTime`, each written
`x.Name`, `x.Name()` or `Name(x)`.

#### Scenario: Converting to a floating-point number

- **GIVEN** the expression `(1/3).Float`
- **WHEN** it is evaluated
- **THEN** the result is `0.3333333333333333`
- **AND** it is a `Float`

### Requirement: The fraction conversion

`x.Fraction` SHALL convert to the `Fraction` display of an exact number, the
fraction recovered from the number rather than held, so that a fraction nobody
wrote is found too. It is no longer a conversion to a rational
representation, since there is none.

#### Scenario: A fraction is recovered from the number

- **GIVEN** the expressions `(1/8).Fraction` and `(2/7 + 3/7).Fraction`
- **WHEN** each is evaluated
- **THEN** they are shown as `1/8` and `5/7`

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

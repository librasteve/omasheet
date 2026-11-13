# units

## ADDED Requirements

### Requirement: Angle units
The radian `rad` SHALL be the unit of a dimension of its own, the steradian `sr` a radian squared, and the degree `°`, the minute of arc `′` and the second of arc `″` SHALL be `π/180` radians, a sixtieth of a degree and a sixtieth of a minute. The radian and the steradian SHALL take the SI prefixes; the degree, the minute and the second SHALL NOT. The lumen `lm` SHALL be `cd.sr` and the lux `lx` `cd.sr.m^-2`. A degree SHALL NOT be read as degrees Celsius, `°C`.

#### Scenario: Degrees and radians
- **WHEN** `1000mrad == 1rad` and `45°30′ == 45.5°` are evaluated
- **THEN** each is `true`

#### Scenario: Not a frequency
- **WHEN** `2rad.s^-1 + 2Hz` is evaluated
- **THEN** it is an error that the dimensions differ

### Requirement: Degrees, minutes and seconds
A number whose degree sign is followed by a digit SHALL be read as an angle in degrees, minutes and seconds: `45°30′30″`, `-12°30′`. The marks `º`, `'` and `"` SHALL be read as `°`, `′` and `″`. Minutes and seconds SHALL be under 60 and only the last part written may have a decimal point.

#### Scenario: The ASCII form
- **WHEN** `45º30'30" == 45°30′30″` is evaluated
- **THEN** the result is `true`

#### Scenario: A bare degree
- **WHEN** `45°` and `20°C` are evaluated
- **THEN** they are `45°` and `20°C`

### Requirement: Trigonometry of an angle
`sin`, `cos` and `tan` SHALL take an angle in any unit, work in radians, and carry the uncertainty through the slope of the function. A quantity that is not an angle SHALL be an error.

#### Scenario: Sine
- **WHEN** `sin(90°)` is evaluated
- **THEN** the result is `1`

#### Scenario: Not an angle
- **WHEN** `sin(5m)` is evaluated
- **THEN** it is an error that `sin` takes an angle

### Requirement: Written as degrees, minutes and seconds
`dms(x)` and `x.dms(places)` SHALL write an angle as text, `45°30′30″`, with the seconds to the places given (none if left off), carrying into the minute and the degree where rounding reaches 60.

#### Scenario: Written out
- **WHEN** `45.5°.dms()` is evaluated
- **THEN** the result is `45°30′0″`

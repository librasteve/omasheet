# file-format

## ADDED Requirements

### Requirement: Duration is a kind of quantity
`Duration` SHALL be a kind of `Quantity` of the dimension of time, held and shown in the unit the column says and otherwise in `s`. A column of it SHALL have the rules of every kind: a bare number is read in its unit, its head shows the unit as `Duration[s]`, `v` puts the unit in each row, and a value may carry an uncertainty.

#### Scenario: A column of durations
- **WHEN** `Run : Duration[min]` holds `90s` and `PT2H`
- **THEN** the cells are `1.5` and `120`
- **AND** the head is `Duration[min]`

#### Scenario: An uncertainty
- **WHEN** a `Duration[s]` column holds `10.5(3)s`
- **THEN** it is shown as `10.5(3)`

#### Scenario: A different dimension
- **WHEN** a `Duration` column holds `5kg`
- **THEN** it is an error that a mass is not a duration

### Requirement: Units of time
The units `min`, `h`, `d` and `wk` SHALL be units of time, of 60 s, 3600 s, 86400 s and 604800 s, beside `s` and its prefixes.

#### Scenario: A column in hours
- **WHEN** `Shift : Duration[h]` holds `7200s`
- **THEN** it is shown as `2`

### Requirement: A duration is written three ways
A duration SHALL be written as an ISO 8601 duration, `PT1H30M`, as a quantity of time, `139s`, or in the signed colon form `±hh:mm:ss.x`: an explicit `+` or `-`, hours of any number, minutes and seconds below 60, and a fraction of a second after the seconds. The seconds MAY be left out, `+11:30`, and are then the 11 hours and 30 minutes; the sign is what says it is a duration. A time of day with no sign SHALL remain a `Time`.

#### Scenario: Signed
- **WHEN** a `Duration` column holds `+11:30`, `-0:00:30.5` and `+128:04:15`
- **THEN** they are 11 hours 30 minutes, minus 30.5 seconds, and 128 hours 4 minutes 15 seconds

#### Scenario: Not signed
- **WHEN** a cell holds `11:30`
- **THEN** it is a `Time`, and in a `Duration` column an error saying to write `+11:30`

#### Scenario: Out of range
- **WHEN** a cell holds `+11:75`
- **THEN** it is an error that minutes are below 60

### Requirement: A format for a duration
A `Duration` column MAY take, in place of a unit, a format in the style of a time format, `Duration[%H:%M:%S]`, whose hours are not capped at 24 and which shows a sign, or `Duration[iso]`, which shows an ISO 8601 duration. What either shows SHALL be read back as the same duration. A bare `Duration` column SHALL be `Duration[s]`.

#### Scenario: Hours past a day
- **WHEN** `128H4M15S`, that is `PT128H4M15S`, is shown under `Duration[%H:%M:%S]`
- **THEN** it is `+128:04:15`

#### Scenario: Bare
- **WHEN** a bare `Duration` column holds `PT1H30M`
- **THEN** its head is `Duration[s]` and the cell is `5400`

#### Scenario: ISO
- **WHEN** the column is `Duration[iso]`
- **THEN** the cell is `PT1H30M`

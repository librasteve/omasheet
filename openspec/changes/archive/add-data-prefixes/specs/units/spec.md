# units

## ADDED Requirements

### Requirement: Data units
The bit `b` and the byte `B` SHALL be units of a dimension of their own, with `1B` equal to `8b`. Each SHALL take the SI prefixes (`kB` is 1000 B) and the binary prefixes `Ki`, `Mi`, `Gi`, `Ti`, `Pi`, `Ei`, `Zi`, `Yi`, `Ri` and `Qi` (`KiB` is 1024 B). A binary prefix SHALL NOT be read on any other unit. A rate SHALL be written by combination: `MiB.s^-1`.

#### Scenario: Binary and decimal
- **WHEN** `1kB + 1KiB` is evaluated
- **THEN** the result is `2.024kB`

#### Scenario: Mixed prefixes
- **WHEN** `4GiB + 512MiB` is evaluated
- **THEN** the result is `4.5GiB`

#### Scenario: Bits and bytes
- **WHEN** `8b == 1B` is evaluated
- **THEN** the result is `true`

#### Scenario: A rate over a time
- **WHEN** `10MB.s^-1 * 3s` is evaluated
- **THEN** the result is `30MB`

#### Scenario: Not a length
- **WHEN** `5B + 3m` is evaluated
- **THEN** an error is reported: they measure Data and Length

### Requirement: Binary scales
A binary prefix and a `.` on their own SHALL be a scale, a power of two: `1Ki.` is 1024.

#### Scenario: A binary scale
- **WHEN** `1.5Mi.` is evaluated
- **THEN** the result is `1572864`

### Requirement: Data quantity types
`Data`, `DataRate` and `DataDensity` (data over an area, `b.m^-2`, as of a disk drive) SHALL be quantity types; a column of `Data` holds `B` unless it names another unit, `Data[GiB]`; a `DataRate` holds `b.s^-1`.

#### Scenario: A column in gibibytes
- **WHEN** a cell `512MiB` is in a column `Data[GiB]`
- **THEN** it is shown as `0.5GiB`

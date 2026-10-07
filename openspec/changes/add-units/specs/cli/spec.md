# cli

Unit errors in `omasheet lint`.

## MODIFIED Requirements

### Requirement: Lint a sheet
`omasheet lint <file.omx>` SHALL parse and statically check a sheet, including
its units, without executing it, print every diagnostic, and exit non-zero if any
error was found.

#### Scenario: Clean sheet
- **WHEN** `omasheet lint` is run on a valid sheet
- **THEN** nothing is reported and the exit status is zero

#### Scenario: Type error
- **WHEN** `omasheet lint` is run on a sheet containing `Revenue + Month` where `Month` is `Text`
- **THEN** the error is printed and the exit status is non-zero

#### Scenario: Unit error
- **WHEN** `omasheet lint` is run on a sheet containing `Revenue + Weight` with incompatible units
- **THEN** the error is printed and the exit status is non-zero

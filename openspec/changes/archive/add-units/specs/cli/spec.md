# cli

Unit errors in `omasheet lint`.

## MODIFIED Requirements

### Requirement: Lint a file
`omasheet lint <file.omx>` SHALL parse and statically check a file, including
its units, without executing it, print every diagnostic, and exit non-zero if any
error was found.

#### Scenario: Clean file
- **WHEN** `omasheet lint` is run on a valid file
- **THEN** nothing is reported and the exit status is zero

#### Scenario: Type error
- **WHEN** `omasheet lint` is run on a file containing `Revenue + Month` where `Month` is `Text`
- **THEN** the error is printed and the exit status is non-zero

#### Scenario: Unit error
- **WHEN** `omasheet lint` is run on a file containing `Revenue + Weight` with incompatible units
- **THEN** the error is printed and the exit status is non-zero

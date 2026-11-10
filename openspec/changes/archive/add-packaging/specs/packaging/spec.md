# packaging

How Omasheet is installed on Omarchy and Arch Linux.

## ADDED Requirements

### Requirement: One package
The package `omasheet` SHALL install the one `omasheet` binary, its licence, a
desktop entry and an icon, for `x86_64` and `aarch64`. It SHALL depend on Qt 6
from the distribution's repositories and bundle none of it.

#### Scenario: Installed
- **WHEN** the package is installed
- **THEN** `omasheet` is on the path, and Omasheet is in the launcher

#### Scenario: A file opens in the app
- **GIVEN** the package is installed
- **WHEN** a `.omx` file is opened from a file manager
- **THEN** it opens in Omasheet

### Requirement: A package with each release
Publishing a GitHub Release tagged `v<version>` SHALL build the package for each
architecture and attach it to the release, with a `SHA256SUMS` file. The build
SHALL fail if the tag, `Cargo.toml` and the `PKGBUILD` do not give the same
version.

#### Scenario: Release
- **WHEN** the release `v0.1.0` is published
- **THEN** it carries `omasheet-0.1.0-1-x86_64.pkg.tar.zst`, `omasheet-0.1.0-1-aarch64.pkg.tar.zst` and `SHA256SUMS`
- **AND** the same packages under names without the version, for the installer

#### Scenario: Versions disagree
- **GIVEN** `Cargo.toml` says `0.1.0`
- **WHEN** the release `v0.2.0` is published
- **THEN** the build fails and nothing is attached

### Requirement: Installer
`install.sh` SHALL install the package of the latest release for this machine's
architecture with `pacman`, after checking it against `SHA256SUMS`. Run again,
it SHALL bring the installation up to date. Where there is no `pacman` it SHALL
say how to build from source, and change nothing.

#### Scenario: Install
- **GIVEN** Omarchy on `x86_64`
- **WHEN** `curl -fsSL …/install.sh | bash` is run
- **THEN** the latest `omasheet` package is installed

#### Scenario: A package that does not match
- **GIVEN** a downloaded package whose checksum is not the one in `SHA256SUMS`
- **WHEN** the installer checks it
- **THEN** it stops, and nothing is installed

#### Scenario: Not Arch
- **GIVEN** a machine with no `pacman`
- **WHEN** the installer is run
- **THEN** it says to use `cargo install`, and exits non-zero

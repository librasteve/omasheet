# packaging

## ADDED Requirements

### Requirement: Listed in OmaStore
The repository SHALL carry an `omastore.toml` at the root of its default
branch, declaring the display name, the freedesktop categories, the icon and,
per architecture, the release asset to install. The asset SHALL be declared as
a pattern over the version, never a fixed version, so that it keeps matching
later releases. OmaStore SHALL be able to install Omasheet on `x86_64` and
`aarch64` from the latest stable release.

#### Scenario: The store lists the app
- **WHEN** OmaStore indexes `librasteve/omasheet`
- **THEN** it shows Omasheet under `Office`, with the icon and the screenshots from the repository
- **AND** reports it as installable

#### Scenario: One package per architecture is chosen
- **GIVEN** a release carrying both `omasheet-<version>-1-<arch>.pkg.tar.zst` and the unversioned `omasheet-<arch>.pkg.tar.zst`
- **WHEN** the store picks the asset for this machine's architecture
- **THEN** it takes the versioned package, and does not guess between the two

#### Scenario: A later release needs no change
- **GIVEN** `omastore.toml` as published for `v0.1.5`
- **WHEN** `v0.2.0` is released with the same asset naming
- **THEN** the store installs `v0.2.0` with the manifest unchanged

#### Scenario: Not on the default branch
- **GIVEN** `omastore.toml` committed only to a feature branch
- **WHEN** the store indexes the repository
- **THEN** the repository is ignored

## MODIFIED Requirements

### Requirement: A package with each release
Publishing a GitHub Release tagged `v<version>` SHALL build the package for each
architecture and attach it to the release, with a `SHA256SUMS` file. The build
SHALL fail if the tag, `Cargo.toml` and the `PKGBUILD` do not give the same
version. The workflow SHALL attest the build provenance of the packages it
attaches, from this repository's own workflow, so that the attestation can be
verified against the files the release carries.

#### Scenario: Release
- **WHEN** the release `v0.1.0` is published
- **THEN** it carries `omasheet-0.1.0-1-x86_64.pkg.tar.zst`, `omasheet-0.1.0-1-aarch64.pkg.tar.zst` and `SHA256SUMS`
- **AND** the same packages under names without the version, for the installer

#### Scenario: Versions disagree
- **GIVEN** `Cargo.toml` says `0.1.0`
- **WHEN** the release `v0.2.0` is published
- **THEN** the build fails and nothing is attached

#### Scenario: Provenance is verifiable
- **GIVEN** a package attached to a published release
- **WHEN** its attestation is checked against this repository
- **THEN** it verifies as built by this repository's GitHub Actions
- **AND** a user who requires provenance can install Omasheet from OmaStore

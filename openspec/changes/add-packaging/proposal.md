# Proposal: Packaging and install

## Why

Omasheet is built from source today: `cargo install`, with Qt 6 and a Rust
toolchain to hand. An Omarchy user should get it as they get Omawrite, with
`pacman`, an entry in the launcher and `.omx` files that open in it.

The goal is the Omarchy Package Repository. Whether a package is taken there is
for its maintainers to say, so there is an interim route that does not wait on
them: a package built here and attached to each GitHub release (owner decision,
2026-10-11).

## The two routes

| | Omarchy repository (goal) | Release package (interim) |
|---|---|---|
| Who builds | Omarchy's CI, from our tagged source, for x86_64 and aarch64 | Our GitHub Action, on each release |
| What we provide | a tag `v0.1.0`, and a `PKGBUILD` recipe sent as a PR to `omacom-io/omarchy-pkgs` | a workflow that builds the `.pkg.tar.zst` and attaches it, and an `install.sh` |
| How users install | `pacman -S omasheet` | `curl … install.sh \| bash`, which runs `pacman -U` on the release asset |
| Updates | a bot watches our releases and proposes each new version | run the installer again |

Both need the same things in this repository: a version tag `v<version>` with a
GitHub Release, a desktop entry, an icon and a `PKGBUILD`. Only the interim
route needs a build on GitHub.

## What Changes

- **packaging** — ADDED: an Arch package of the one `omasheet` binary with its
  desktop entry, icon and file type; a release workflow that builds it for
  x86_64 and aarch64 and attaches it to the release; an installer script
- CI builds and tests the whole workspace, the app included, with Qt 6

## Impact

- `pkgbuild/`: `PKGBUILD`, `omasheet.desktop`, `omasheet.svg`, `omasheet.xml`
  (the `.omx` file type), `omasheet.install`, and `build`, which makes the
  package in a container
- `.github/workflows/package.yml`, `install.sh`, and Qt 6 in
  `.github/workflows/ci.yml`
- A release is made by setting the version in `Cargo.toml` and
  `pkgbuild/PKGBUILD`, tagging `v<version>` and publishing a GitHub Release

## Out of Scope

- The PR to `omacom-io/omarchy-pkgs`: it follows once the interim route has
  made a release, with a recipe that takes the tagged source archive
- An `omasheet update` command, and telling the user that a new version is out
- Other distributions and macOS: `cargo install` stays the way there

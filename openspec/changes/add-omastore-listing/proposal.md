# Proposal: List Omasheet in OmaStore

## Why

`add-packaging` gave Omasheet two ways in: the Omarchy Package Repository (the
goal, not ours to grant) and a package attached to each GitHub Release, fetched
by `install.sh`. Both ask the user to already know Omasheet exists and to run a
command.

OmaStore is a community app store for Omarchy: it indexes GitHub repositories
that carry an `omastore.toml`, and installs the release binary into the user's
home with no `sudo` and no `pacman`. It costs this repository a manifest and a
build attestation, and it is the only route where a user *finds* Omasheet
rather than being sent to it (owner decision, 2026-10-25).

## The third route

| | Omarchy repository (goal) | Release package (interim) | OmaStore |
|---|---|---|---|
| Who builds | Omarchy's CI, from our tagged source | our GitHub Action, on each release | nobody: it installs the same release asset |
| What we provide | a tag and a `PKGBUILD` PR | the workflow and `install.sh` | `omastore.toml`, and provenance on the release |
| How users install | `pacman -S omasheet` | `curl … install.sh \| bash` | browse the store, or `omastore install librasteve/omasheet` |
| Where it lands | `/usr` | `/usr`, via `pacman -U` | `~/.local/share/omastore/apps/…` |
| Updates | a bot watches our releases | run the installer again | the store updates it on its own |

The store never runs anything from the package: it reads the manifest, checks
the asset's digest, verifies the build attestation with Sigstore, and unpacks.

## What Changes

- **packaging** — ADDED: an `omastore.toml` at the root of the default branch
  that declares the display name, categories, icon and the release asset per
  architecture, so OmaStore lists and installs Omasheet
- **packaging** — MODIFIED: the release workflow attests the build provenance
  of the packages it attaches, which OmaStore verifies and some users require

## Impact

- `omastore.toml` at the repository root — it is read from the default branch,
  so it has to be on `main`, not only on a feature branch
- `.github/workflows/package.yml`: an `actions/attest-build-provenance` step in
  the `attach` job, with `id-token: write` and `attestations: write`; the
  workflow's top-level permission narrows to `contents: read`
- The `omarchy` topic on the repository, so discovery does not depend on a
  GitHub token
- `README.md`: an `omastore://librasteve/omasheet` link
- No change to the binary, the package, or `install.sh`

## Out of Scope

- Leaving the Omarchy Package Repository route: OmaStore does not replace it,
  and a listing is not a review
- Changing where the package puts files. OmaStore unpacks into `$HOME`, so
  anything Omasheet resolves at an absolute `/usr` path is wrong under this
  route; today the binary, the desktop entry and the icon are all the package
  holds, and the store writes its own desktop entry from the manifest's
  categories
- A `[services.<id>]` declaration: Omasheet has no background service

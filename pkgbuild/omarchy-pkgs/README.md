# The omarchy-pkgs recipe

`omasheet/` is the recipe for [omacom-io/omarchy-pkgs](https://github.com/omacom-io/omarchy-pkgs), kept here so it is versioned beside the code it builds. It is copied, not linked: to propose it, fork that repository and put the directory at `pkgbuilds/omasheet/`, then follow its README (`bin/build --package omasheet`, then a PR with the one package in it).

It differs from `../PKGBUILD` in where it gets its source: the tagged archive of a release, checked by `sha256sums`, rather than the checkout it sits in. Everything else is the same, and the two should stay so.

When `pkgver` moves, run `updpkgsums` in the recipe directory to fill the checksum, and reset `pkgrel` to 1. The `.omarchy/package.json` watches this repository's GitHub releases for a tag like `v0.1.7`, so omarchy-pkgs proposes the new version itself once the recipe is merged. The `demo-data` pre-release does not match the pattern and is passed over.

# Tasks: Packaging and install

## 1. CI
- [x] 1.1 Build, lint and test the whole workspace with Qt 6, in an Arch container

## 2. Package (`packaging`)
- [x] 2.1 Desktop entry, icon and the `.omx` file type
- [x] 2.2 `pkgbuild/PKGBUILD`, building from the checkout it is in
- [x] 2.3 `pkgbuild/build`: make the package in an Arch or Arch Linux ARM container

## 3. Release (interim route)
- [x] 3.1 `package.yml`: build for x86_64 and aarch64 on a release, attach the packages and `SHA256SUMS`
- [x] 3.2 `install.sh`: fetch the package of the latest release, check it, install it with `pacman`
- [x] 3.3 Run `package.yml` by hand on GitHub, and put right what the first run finds
- [x] 3.4 Publish a release and install from it on a clean machine: releases run to v0.1.6; `install.sh` installed it in an Omarchy VM, and the CLI, launcher entry, `.omx` association, reinstall and uninstall all worked (2026-11-04)

## 4. Omarchy repository (goal)
- [x] 4.1 Recipe for `omacom-io/omarchy-pkgs` from the tagged source archive, with a `package.json` that watches the releases. Written 2026-11-04 in `pkgbuild/omarchy-pkgs/omasheet/` against the v0.1.6 archive, whose checksum it carries. Checked so far by `bash -n` and by reading it against their `omacalc` recipe; not yet built, and their `helpers/upstream-watch.py check` and `tests/pinned-sources.sh` are not yet run, which 4.2 does
- [x] 4.2 Build it with their `bin/build`, and open the PR

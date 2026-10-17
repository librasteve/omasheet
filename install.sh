#!/usr/bin/env bash
# Omasheet installer, for Omarchy and Arch Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/librasteve/omasheet/main/install.sh | bash
#
# Installs the pacman package of the latest release; what it depends on comes
# from the distribution's repositories. Run it again to update.
#
# Environment:
#   OMASHEET_REF  the release tag to install, such as v0.1.0 (default: the latest)

set -euo pipefail

REPO="librasteve/omasheet"
REF="${OMASHEET_REF:-}"

info() { printf '  %s\n' "$*"; }
die() { printf '\e[31mError:\e[0m %s\n' "$*" >&2; exit 1; }

if ! command -v pacman >/dev/null; then
  # --locked: calamine pulls in zip ^8.6, and the whole 8.x line of zip is
  # yanked, so a fresh resolve fails; the repo's Cargo.lock still pins a
  # (yanked but fetchable) 8.x release that works.
  die "this installs a pacman package, and there is no pacman here. Build from source instead:
  cargo install --git https://github.com/$REPO --locked omasheet-ui"
fi

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64 | aarch64) ;;
  *) die "there is no package for $ARCH: only x86_64 and aarch64" ;;
esac

PACKAGE="omasheet-$ARCH.pkg.tar.zst"
if [[ -n $REF ]]; then
  BASE="https://github.com/$REPO/releases/download/$REF"
else
  BASE="https://github.com/$REPO/releases/latest/download"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

printf '\e[1m%s\e[0m\n' "Installing Omasheet (${REF:-latest release}, $ARCH)"

info "Fetching $PACKAGE"
curl -fsSL -o "$TMP/$PACKAGE" "$BASE/$PACKAGE" \
  || die "could not fetch $BASE/$PACKAGE"
curl -fsSL -o "$TMP/SHA256SUMS" "$BASE/SHA256SUMS" \
  || die "could not fetch $BASE/SHA256SUMS"

info "Checking it"
(cd "$TMP" && grep " $PACKAGE\$" SHA256SUMS | sha256sum --check --quiet --strict) \
  || die "$PACKAGE is not what SHA256SUMS says it should be; nothing was installed"

info "Installing it with pacman"
# Piped from curl, this script is on standard input: ask on the terminal.
# It needs at least the Qt it was built with, which pacman may not know of
# until the system is updated.
STALE="pacman did not install it. If it could not satisfy a dependency on qt6, update the system (sudo pacman -Syu) and run this again"
if [[ -r /dev/tty ]]; then
  sudo pacman -U "$TMP/$PACKAGE" </dev/tty || die "$STALE"
else
  sudo pacman -U --noconfirm "$TMP/$PACKAGE" || die "$STALE"
fi

info "Done: $(pacman -Q omasheet)"
info "Open it from the launcher, or run: omasheet"

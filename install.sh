#!/usr/bin/env bash
# Omasheet installer, for macOS and for Omarchy and Arch Linux (Windows has
# install.ps1).
#
#   curl -fsSL https://raw.githubusercontent.com/librasteve/omasheet/main/install.sh | bash
#
# On Arch it installs the pacman package of the latest release; what it
# depends on comes from the distribution's repositories. On macOS it puts
# Omasheet.app in /Applications and links the omasheet command into
# /usr/local/bin. Run it again to update.
#
# Environment:
#   OMASHEET_REF  the release tag to install, such as v0.1.0 (default: the latest)

set -euo pipefail

REPO="librasteve/omasheet"
REF="${OMASHEET_REF:-}"

info() { printf '  %s\n' "$*"; }
die() { printf '\e[31mError:\e[0m %s\n' "$*" >&2; exit 1; }

OS="$(uname -s)"

if [[ $OS == Linux ]] && ! command -v pacman >/dev/null; then
  # --locked: calamine pulls in zip ^8.6, and the whole 8.x line of zip is
  # yanked, so a fresh resolve fails; the repo's Cargo.lock still pins a
  # (yanked but fetchable) 8.x release that works.
  die "this installs a pacman package, and there is no pacman here. Build from source instead:
  cargo install --git https://github.com/$REPO --locked omasheet-ui"
fi

ARCH="$(uname -m)"
case "$OS-$ARCH" in
  Linux-x86_64 | Linux-aarch64) PACKAGE="omasheet-$ARCH.pkg.tar.zst" ;;
  Darwin-arm64 | Darwin-x86_64) PACKAGE="omasheet-macos-$ARCH.zip" ;;
  *) die "there is no package for $OS $ARCH: Linux on x86_64 or aarch64, and macOS" ;;
esac

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
if command -v sha256sum >/dev/null; then SUM=(sha256sum); else SUM=(shasum -a 256); fi
(cd "$TMP" && grep " $PACKAGE\$" SHA256SUMS | "${SUM[@]}" --check --quiet) \
  || die "$PACKAGE is not what SHA256SUMS says it should be; nothing was installed"

if [[ $OS == Darwin ]]; then
  APP="/Applications/Omasheet.app"
  info "Putting Omasheet.app in /Applications"
  rm -rf "$TMP/app" && mkdir "$TMP/app"
  ditto -x -k "$TMP/$PACKAGE" "$TMP/app"
  if [[ -w /Applications ]]; then
    rm -rf "$APP" && ditto "$TMP/app/Omasheet.app" "$APP"
  else
    sudo rm -rf "$APP" && sudo ditto "$TMP/app/Omasheet.app" "$APP"
  fi
  # Fetched by curl, it carries no quarantine mark; clear one if it has.
  xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

  BIN="/usr/local/bin"
  if [[ -w $BIN ]] || { [[ ! -e $BIN ]] && [[ -w /usr/local ]]; }; then
    mkdir -p "$BIN" && ln -sfn "$APP/Contents/MacOS/omasheet" "$BIN/omasheet"
  else
    sudo mkdir -p "$BIN" && sudo ln -sfn "$APP/Contents/MacOS/omasheet" "$BIN/omasheet"
  fi
  info "Done: $APP"
  info "Open it from Launchpad, or run: omasheet"
  exit 0
fi

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

#!/bin/sh
# Skarn installer.
#
#   curl -fsSL https://raw.githubusercontent.com/Rani367/Skarn/main/install.sh | sh
#
# Downloads the latest release binary for your OS/arch into ~/.local/bin (or
# $SKARN_INSTALL_DIR), or falls back to `cargo install skarn` (published on
# crates.io).

set -eu

REPO="Rani367/Skarn"
BIN="skarn"
INSTALL_DIR="${SKARN_INSTALL_DIR:-$HOME/.local/bin}"

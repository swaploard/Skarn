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

say() { printf '\033[1;34mskarn\033[0m %s\n' "$1"; }
err() { printf '\033[1;31merror\033[0m %s\n' "$1" >&2; exit 1; }

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux)  os_part="unknown-linux-gnu" ;;
    Darwin) os_part="apple-darwin" ;;
    *) err "unsupported OS: $os (try: cargo install skarn)" ;;
  esac
  case "$arch" in
    x86_64|amd64)  arch_part="x86_64" ;;
    arm64|aarch64) arch_part="aarch64" ;;
    *) err "unsupported architecture: $arch" ;;
  esac
  echo "${arch_part}-${os_part}"
}

main() {
  if ! command -v curl >/dev/null 2>&1; then
    err "curl is required"
  fi

  target="$(detect_target)"
  say "detected target: $target"

  latest="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \

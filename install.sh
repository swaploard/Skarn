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
    | grep -o '"tag_name": *"[^"]*"' | head -1 | sed 's/.*"\([^"]*\)"$/\1/')"
  if [ -z "${latest:-}" ]; then
    say "no published release found; falling back to cargo"
    command -v cargo >/dev/null 2>&1 || err "cargo not found either"
    exec cargo install skarn
  fi

  url="https://github.com/${REPO}/releases/download/${latest}/skarn-${target}.tar.gz"
  say "downloading ${latest} from ${url}"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  curl -fsSL "$url" -o "$tmp/skarn.tar.gz" || err "download failed (try: cargo install skarn)"
  tar -xzf "$tmp/skarn.tar.gz" -C "$tmp"

  mkdir -p "$INSTALL_DIR"
  found="$(find "$tmp" -name "$BIN" -type f | head -1)"
  [ -n "$found" ] || err "binary not found in archive"
  install -m 0755 "$found" "$INSTALL_DIR/$BIN"

  say "installed to $INSTALL_DIR/$BIN"
  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) say "add $INSTALL_DIR to your PATH to use 'skarn'." ;;
  esac
  "$INSTALL_DIR/$BIN" --version || true
}

main "$@"

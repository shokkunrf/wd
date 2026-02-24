#!/bin/sh
set -eu

# wd installer
# Usage: curl -fsSL https://github.com/shokkunrf/wd/releases/latest/download/install.sh | sh

INSTALL_DIR="${WD_INSTALL_DIR:-/usr/local/bin}"

die() {
  echo "install: error: $*" >&2
  exit 1
}

fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$1"
  else
    die "curl or wget is required"
  fi
}

fetch_to_file() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$2" "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" "$1"
  else
    die "curl or wget is required"
  fi
}

verify_checksum() {
  _file="$1"
  _expected="$2"
  _actual=""
  if command -v sha256sum >/dev/null 2>&1; then
    _actual=$(sha256sum "$_file" | cut -d' ' -f1)
  elif command -v shasum >/dev/null 2>&1; then
    _actual=$(shasum -a 256 "$_file" | cut -d' ' -f1)
  else
    echo "warning: sha256sum/shasum not found, skipping checksum verification" >&2
    return 0
  fi
  if [ "$_actual" != "$_expected" ]; then
    die "Checksum mismatch: expected $_expected, got $_actual"
  fi
}

main() {
  echo "Installing wd..."

  # Get latest release tag
  _latest=$(fetch "https://api.github.com/repos/shokkunrf/wd/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p')
  if [ -z "$_latest" ]; then
    die "Failed to detect latest release"
  fi
  echo "Latest version: $_latest"

  _base_url="https://github.com/shokkunrf/wd/releases/download/$_latest"
  _tmpdir=$(mktemp -d)
  trap 'rm -rf "$_tmpdir"' EXIT

  # Download wd and checksums
  fetch_to_file "$_base_url/wd" "$_tmpdir/wd"
  fetch_to_file "$_base_url/wd.sha256" "$_tmpdir/wd.sha256"

  # Verify checksum
  _expected=$(sed -n 's/  *wd$//p' "$_tmpdir/wd.sha256")
  if [ -n "$_expected" ]; then
    verify_checksum "$_tmpdir/wd" "$_expected"
    echo "Checksum verified"
  fi

  # Install
  chmod +x "$_tmpdir/wd"
  if [ -w "$INSTALL_DIR" ]; then
    cp "$_tmpdir/wd" "$INSTALL_DIR/wd"
  else
    echo "Installing to $INSTALL_DIR (requires sudo)"
    sudo cp "$_tmpdir/wd" "$INSTALL_DIR/wd"
  fi

  echo "wd $_latest installed to $INSTALL_DIR/wd"
  "$INSTALL_DIR/wd" --version
}

main

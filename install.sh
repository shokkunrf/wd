#!/bin/sh
# Usage: curl -fsSL https://github.com/shokkunrf/wd/releases/download/v<version>/install.sh | sh
# releases/latest/download/install.sh installs the latest release that is not a pre-release.
set -eu

VERSION=""
INSTALL_DIR="${WD_INSTALL_DIR:-/usr/local/bin}"
TARGET=x86_64-unknown-linux-musl

die() {
  echo "install: error: $*" >&2
  exit 1
}

fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$2" "$1" || die "cannot download $1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" "$1" || die "cannot download $1"
  else
    die "curl or wget is required"
  fi
}

# Everything runs from main so that a truncated download runs nothing.
main() {
  [ -n "$VERSION" ] || die "use the install.sh attached to a release"
  platform="$(uname -s)-$(uname -m)"
  [ "$platform" = Linux-x86_64 ] || die "only Linux x86_64 is supported: $platform"
  for cmd in sha256sum tar awk mktemp install; do
    command -v "$cmd" >/dev/null 2>&1 || die "$cmd is required"
  done
  [ -d "$INSTALL_DIR" ] || die "install directory does not exist: $INSTALL_DIR"

  name="wd-$VERSION-$TARGET"
  url="https://github.com/shokkunrf/wd/releases/download/v$VERSION"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  fetch "$url/$name.tar.gz" "$tmp/$name.tar.gz"
  fetch "$url/SHA256SUMS" "$tmp/SHA256SUMS"
  sum="$(awk -v file="$name.tar.gz" '$2 == file' "$tmp/SHA256SUMS")"
  [ -n "$sum" ] || die "SHA256SUMS has no entry for $name.tar.gz"
  (cd "$tmp" && echo "$sum" | sha256sum -c - >/dev/null) || die "checksum mismatch for $name.tar.gz"
  tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
  if [ -w "$INSTALL_DIR" ]; then
    install -m 0755 "$tmp/$name/wd" "$INSTALL_DIR/wd"
  elif command -v sudo >/dev/null 2>&1; then
    sudo install -m 0755 "$tmp/$name/wd" "$INSTALL_DIR/wd"
  else
    die "cannot write to $INSTALL_DIR and sudo is not available"
  fi
  "$INSTALL_DIR/wd" --version
}

main "$@"

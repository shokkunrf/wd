#!/bin/sh
# 使い方: curl -fsSL https://github.com/shokkunrf/wd/releases/latest/download/install.sh | sh
set -eu

VERSION=""
INSTALL_DIR="${WD_INSTALL_DIR:-/usr/local/bin}"
TARGET=x86_64-unknown-linux-musl

die() {
  echo "install: $*" >&2
  exit 1
}

fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$2" "$1" || die "取れない: $1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" "$1" || die "取れない: $1"
  else
    die "curlかwgetが要る"
  fi
}

[ -n "$VERSION" ] || die "Releaseに上げたinstall.shを使う"
platform="$(uname -s)-$(uname -m)"
[ "$platform" = Linux-x86_64 ] || die "Linuxのx86_64にだけ入れられる: $platform"
command -v sha256sum >/dev/null 2>&1 || die "sha256sumが要る"
[ -d "$INSTALL_DIR" ] || die "入れる先が無い: $INSTALL_DIR"

name="wd-$VERSION-$TARGET"
url="https://github.com/shokkunrf/wd/releases/download/v$VERSION"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fetch "$url/$name.tar.gz" "$tmp/$name.tar.gz"
fetch "$url/SHA256SUMS" "$tmp/SHA256SUMS"
sum="$(awk -v file="$name.tar.gz" '$2 == file' "$tmp/SHA256SUMS")"
[ -n "$sum" ] || die "SHA256SUMSに$name.tar.gzが無い"
(cd "$tmp" && echo "$sum" | sha256sum -c - >/dev/null) || die "SHA256SUMSと合わない"
tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
if [ -w "$INSTALL_DIR" ]; then
  install -m 0755 "$tmp/$name/wd" "$INSTALL_DIR/wd"
elif command -v sudo >/dev/null 2>&1; then
  sudo install -m 0755 "$tmp/$name/wd" "$INSTALL_DIR/wd"
else
  die "$INSTALL_DIRに書けず、sudoも無い"
fi
"$INSTALL_DIR/wd" --version

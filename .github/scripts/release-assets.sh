#!/bin/sh
set -eu

: "${WD_VERSION:?焼き込むバージョンをWD_VERSIONで渡す}"
export WD_VERSION
mkdir -p "$1"
out="$(cd "$1" && pwd)"
root="$(cd "$(dirname "$0")/../.." && pwd)"
target=x86_64-unknown-linux-musl
bin="$root/app/target/$target/release/wd"
images="
debian:trixie-slim@sha256:f6b08b09a05725f9431502d3b5714f5c4576e82c1a84fa1debe0c24a7e0c3ec4
alpine:3.24.3@sha256:be2656061b536c1883c4034790e21d0a09c5aaaf51968b89368a3c0371ea2c9a
"

fail() {
  echo "::error::$1" >&2
  exit 1
}

cd "$root/app"
cargo build --locked --release --target "$target"

got="$("$bin" --version)"
[ "$got" = "$WD_VERSION" ] || fail "--versionが$WD_VERSIONでない: $got"

program_headers="$(readelf -lW "$bin")"
dynamic_section="$(readelf -dW "$bin")"
case "$program_headers $dynamic_section" in
  *INTERP* | *NEEDED*) fail "動的リンクが残っている: $bin" ;;
esac

for image in $images; do
  got="$(docker run --rm --network none -v "$bin:/usr/local/bin/wd:ro" "$image" wd --version)"
  [ "$got" = "$WD_VERSION" ] || fail "$image で--versionが違う: $got"
done

name="wd-$WD_VERSION-$target"
dir="$out/$name"
mkdir -p "$dir"
install -m 0755 "$bin" "$dir/wd"
cp "$root/LICENSE" "$dir/LICENSE"
cargo about generate --locked --fail about.hbs -o "$dir/THIRD-PARTY-NOTICES.txt"
cp "$(rustc --print sysroot)/share/doc/rust/COPYRIGHT-library.html" "$dir/COPYRIGHT-rust-library.html"
cp /usr/share/doc/musl/copyright "$dir/COPYRIGHT-musl"
tar -C "$out" --sort=name --owner=0 --group=0 --numeric-owner -czf "$out/$name.tar.gz" "$name"
(cd "$out" && sha256sum "$name.tar.gz" >SHA256SUMS)

sed "s/^VERSION=\"\"\$/VERSION=\"$WD_VERSION\"/" "$root/install.sh" >"$out/install.sh"
grep -qx "VERSION=\"$WD_VERSION\"" "$out/install.sh" || fail "install.shにバージョンを焼き込めない"

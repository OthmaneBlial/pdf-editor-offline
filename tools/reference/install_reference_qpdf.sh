#!/usr/bin/env bash
# Independent test reference only; never linked or bundled with the application.
set -euo pipefail

if [[ $# != 1 || $1 == --help || $1 == -h ]]; then
  printf 'Usage: bash tools/reference/install_reference_qpdf.sh NEW_ABSOLUTE_PREFIX\n'
  printf 'Installs official checksum-pinned qpdf 12.4.2 binary/libraries for Linux or macOS.\n'
  if [[ $# == 1 && ( $1 == --help || $1 == -h ) ]]; then exit 0; fi
  exit 2
fi
prefix=$1
case "$prefix" in
  /*) ;;
  *) printf 'Prefix must be absolute.\n' >&2; exit 2 ;;
esac
if [[ -e $prefix || -L $prefix ]]; then
  printf 'Refusing existing prefix: %s\n' "$prefix" >&2; exit 2
fi
for reference_tool in curl unzip awk; do
  command -v "$reference_tool" >/dev/null || { printf 'Missing prerequisite: %s\n' "$reference_tool" >&2; exit 2; }
done
platform="$(uname -s)-$(uname -m)"
case "$platform" in
  Linux-x86_64)
    package=linux-x86_64
    archive_sha256=db367d897829f22c4198ce1094143c9d467bd6ee7dfabc44ba6f02056b24f8b1 ;;
  Linux-aarch64)
    package=linux-aarch64
    archive_sha256=8fd9d009eb0838398180a603f2cf76531e5c68a4ecc8847598e0a1ccb1d3b600 ;;
  Darwin-arm64)
    package=macos-arm64
    archive_sha256=62e46987a30ea167cbc530ccb22690aec3d8c812ed09979a941bcee92e504b79 ;;
  Darwin-x86_64)
    package=macos-x86_64
    archive_sha256=dd3b01f4414d198529bb0f13bba59c2f016a328bb3506695087a96fb80fc1481 ;;
  *) printf 'Unsupported reference platform: %s\n' "$platform" >&2; exit 2 ;;
esac
sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | awk '{print $1}';
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}
work=$(mktemp -d "${TMPDIR:-/tmp}/qpdf-reference.XXXXXX")
trap 'rm -rf "$work"' EXIT
archive="$work/qpdf.zip"
url="https://github.com/qpdf/qpdf/releases/download/v12.4.2/qpdf-12.4.2-bin-$package.zip"
curl --fail --location --silent --show-error --connect-timeout 10 --max-time 120 "$url" -o "$archive"
if [[ $(sha256 "$archive") != "$archive_sha256" ]]; then
  printf 'qpdf archive checksum mismatch.\n' >&2; exit 1
fi
while IFS= read -r member; do
  case "$member" in
    /*|../*|*/../*|*\\*) printf 'Unsafe archive member.\n' >&2; exit 1 ;;
  esac
done < <(unzip -Z1 "$archive")
# Preserve the vendor's relative library layout; omit unrelated helper programs.
unzip -q "$archive" 'bin/qpdf' 'lib/*' -d "$work/install"
version=$("$work/install/bin/qpdf" --version)
if [[ ${version%%$'\n'*} != 'qpdf version 12.4.2' ]]; then
  printf 'Unexpected qpdf reference version.\n' >&2; exit 1
fi
mkdir -p "$work/install/share/licenses/qpdf"
"$work/install/bin/qpdf" --copyright > "$work/install/share/licenses/qpdf/COPYRIGHT.txt"
printf '{"version":"12.4.2","platform":"%s","archive_sha256":"%s","source":"%s"}\n' \
  "$platform" "$archive_sha256" "$url" > "$work/install/reference.json"
mkdir -p "$(dirname "$prefix")"
# mkdir reserves a new path atomically instead of replacing another installation.
mkdir -m 700 "$prefix"
cp -R "$work/install/." "$prefix/"
"$prefix/bin/qpdf" --version

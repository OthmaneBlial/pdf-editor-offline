#!/usr/bin/env bash
# External GPL reference tool for tests; never linked into the Rust application.
set -euo pipefail

version=26.10.0
archive_sha256=6792cb7c69205007ad87d2e936cecc5b3a31fac29ab54ffc3175fdb6b2a6ce35
signer_fingerprint=CA262C6C83DE4D2FB28A332A3A6A4DB839EAA6D7

usage() {
  cat <<'EOF'
Usage: bash scripts/install_reference_poppler.sh EMPTY_ABSOLUTE_PREFIX
       bash scripts/install_reference_poppler.sh --verify-only

Builds pinned Poppler 26.10.0 pdftotext, pdfinfo and pdftoppm into a private prefix.
Ubuntu 24.04 prerequisites (install separately):
  build-essential cmake pkg-config curl xz-utils gnupg poppler-data
  libfreetype-dev libfontconfig1-dev zlib1g-dev libbrotli-dev libharfbuzz-dev libjpeg-dev liblcms2-dev
--verify-only checks the official archive checksum and signature without compiling.
EOF
}

if [[ $# != 1 ]]; then usage >&2; exit 2; fi
if [[ $1 == --help || $1 == -h ]]; then usage; exit 0; fi
verify_only=false
if [[ $1 == --verify-only ]]; then
  verify_only=true
else
  prefix=$1
  case "$prefix" in
    /*) ;;
    *) printf 'Prefix must be absolute.\n' >&2; exit 2 ;;
  esac
  if [[ -e $prefix ]] && { [[ ! -d $prefix ]] || [[ -n $(ls -A "$prefix") ]]; }; then
    printf 'Refusing existing nonempty prefix: %s\n' "$prefix" >&2
    exit 2
  fi
fi

for tool in curl gpg awk; do
  command -v "$tool" >/dev/null || { printf 'Missing prerequisite: %s\n' "$tool" >&2; exit 2; }
done
if ! $verify_only; then
  for tool in cmake pkg-config tar; do
    command -v "$tool" >/dev/null || { printf 'Missing prerequisite: %s\n' "$tool" >&2; exit 2; }
  done
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/reference-poppler.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -m 700 "$work/gnupg"
archive="$work/poppler-$version.tar.xz"
base=https://poppler.freedesktop.org
download() {
  curl --fail --silent --show-error --location --retry 3 \
    --connect-timeout 20 --max-time 120 --output "$2" "$1"
}
download "$base/poppler-$version.tar.xz" "$archive"
download "$base/poppler-$version.tar.xz.sig" "$archive.sig"
if command -v sha256sum >/dev/null; then
  actual_sha256=$(sha256sum "$archive" | awk '{print $1}')
else
  actual_sha256=$(shasum -a 256 "$archive" | awk '{print $1}')
fi
if [[ $actual_sha256 != "$archive_sha256" ]]; then
  printf 'Poppler archive SHA-256 mismatch.\n' >&2; exit 1
fi
# Trust the fingerprint published by Poppler, never the keyserver's identity claim.
download "https://keyserver.ubuntu.com/pks/lookup?op=get&search=0x$signer_fingerprint" "$work/signer.asc"
gpg --homedir "$work/gnupg" --batch --import "$work/signer.asc" >/dev/null 2>&1
actual_fingerprint=$(gpg --homedir "$work/gnupg" --batch --with-colons --fingerprint \
  "$signer_fingerprint" | awk -F: '$1 == "fpr" {print $10; exit}')
if [[ $actual_fingerprint != "$signer_fingerprint" ]]; then
  printf 'Poppler signing-key fingerprint mismatch.\n' >&2; exit 1
fi
gpg --homedir "$work/gnupg" --batch --status-fd 1 \
  --verify "$archive.sig" "$archive" >"$work/signature-status"
awk -v fingerprint="$signer_fingerprint" \
  '$2 == "VALIDSIG" && ($3 == fingerprint || $NF == fingerprint) {valid=1} END {exit !valid}' \
  "$work/signature-status"
printf 'Verified official Poppler %s source: %s\n' "$version" "$archive_sha256"
if $verify_only; then exit 0; fi

tar -xf "$archive" -C "$work"
cmake -S "$work/poppler-$version" -B "$work/build" \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" \
  -DBUILD_SHARED_LIBS=OFF -DENABLE_UTILS=ON \
  -DENABLE_CPP=OFF -DENABLE_GLIB=OFF -DENABLE_QT5=OFF -DENABLE_QT6=OFF \
  -DENABLE_GOBJECT_INTROSPECTION=OFF -DENABLE_GTK_DOC=OFF \
  -DBUILD_MANUAL_TESTS=OFF -DBUILD_GTK_TESTS=OFF -DBUILD_CPP_TESTS=OFF \
  -DBUILD_QT5_TESTS=OFF -DBUILD_QT6_TESTS=OFF -DENABLE_BOOST=OFF \
  -DENABLE_LIBCURL=OFF -DENABLE_NSS3=OFF -DENABLE_GPGME=OFF \
  -DENABLE_LIBOPENJPEG=OFF -DENABLE_LIBJPEG=ON -DENABLE_LCMS=ON \
  -DENABLE_LIBTIFF=OFF -DENABLE_BROTLI=ON -DENABLE_HARFBUZZ=ON
cmake --build "$work/build" --target pdftotext pdfinfo pdftoppm --parallel 2
mkdir -p "$prefix/bin" "$prefix/share/licenses/poppler"
for reference_tool in pdftotext pdfinfo pdftoppm; do
  install -m 755 "$work/build/utils/$reference_tool" "$prefix/bin/$reference_tool"
done
install -m 644 "$work/poppler-$version/COPYING" "$prefix/share/licenses/poppler/COPYING"
"$prefix/bin/pdftotext" -v

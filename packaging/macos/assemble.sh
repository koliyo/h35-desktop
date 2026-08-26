#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
  echo "usage: assemble.sh <binary> <dest-app> <version> [bundle-version]" >&2
  exit 2
fi

for name in APP_NAME BUNDLE_ID EXECUTABLE; do
  if [[ -z ${!name:-} ]]; then
    echo "assemble.sh: $name is required" >&2
    exit 1
  fi
done

binary=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
dest=$2
version=$3
bundle_version=${4:-$version}

if [[ ! -f $binary ]]; then
  echo "assemble.sh: binary not found: $binary" >&2
  exit 1
fi

here=$(cd "$(dirname "$0")" && pwd)
template=${INFO_PLIST:-$here/Info.plist}
if [[ ! -f $template ]]; then
  echo "assemble.sh: missing $template" >&2
  exit 1
fi

rm -rf "$dest"
macos=$dest/Contents/MacOS
resources=$dest/Contents/Resources
mkdir -p "$macos" "$resources"

cp "$binary" "$macos/$EXECUTABLE"
chmod 755 "$macos/$EXECUTABLE"

sed -e "s/@APP_NAME@/${APP_NAME}/g" \
    -e "s/@EXECUTABLE@/${EXECUTABLE}/g" \
    -e "s/@BUNDLE_ID@/${BUNDLE_ID}/g" \
    -e "s/@VERSION@/${version}/g" \
    -e "s/@BUNDLE_VERSION@/${bundle_version}/g" \
    -e "s|@SU_FEED_URL@|${SU_FEED_URL:-}|g" \
    -e "s|@SU_PUBLIC_ED_KEY@|${SU_PUBLIC_ED_KEY:-}|g" \
    "$template" >"$dest/Contents/Info.plist"

printf 'APPL????' >"$dest/Contents/PkgInfo"

if [[ -n ${APP_ICON:-} && -f $APP_ICON ]]; then
  cp "$APP_ICON" "$resources/AppIcon.icns"
fi

if [[ -n ${SPARKLE_FRAMEWORK:-} ]]; then
  if [[ ! -d $SPARKLE_FRAMEWORK ]]; then
    echo "assemble.sh: SPARKLE_FRAMEWORK is not a directory: $SPARKLE_FRAMEWORK" >&2
    exit 1
  fi
  frameworks=$dest/Contents/Frameworks
  mkdir -p "$frameworks"
  rm -rf "$frameworks/Sparkle.framework"
  cp -R "$SPARKLE_FRAMEWORK" "$frameworks/Sparkle.framework"
  if [[ $(uname -s) == Darwin ]] && /usr/bin/file -b "$macos/$EXECUTABLE" | grep -q 'Mach-O'; then
    install_name_tool -add_rpath '@executable_path/../Frameworks' "$macos/$EXECUTABLE"
  fi
fi

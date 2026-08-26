#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
product_root=${PRODUCT_ROOT:-$(pwd)}
crate=${CRATE:-}
executable=${EXECUTABLE:-}
app_name=${APP_NAME:-}

if [[ -z $crate || -z $executable || -z $app_name || -z ${BUNDLE_ID:-} ]]; then
  echo "usage: APP_NAME=… BUNDLE_ID=… EXECUTABLE=… CRATE=… [PRODUCT_ROOT=…] package.sh" >&2
  exit 2
fi

cd "$product_root"
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
if [[ -z $version ]]; then
  echo "package.sh: could not read package.version from Cargo.toml" >&2
  exit 1
fi

cargo build --release -p "$crate"

if [[ $(uname -s) == Darwin ]]; then
  SPARKLE_FRAMEWORK=$("$here/fetch-sparkle.sh")
  export SPARKLE_FRAMEWORK
fi

export APP_NAME=$app_name
export EXECUTABLE=$executable
bundle_version=${BUNDLE_VERSION:-$version}

"$here/assemble.sh" \
  "$product_root/target/release/$executable" \
  "$product_root/dist/${app_name}.app" \
  "$version" \
  "$bundle_version"

if [[ $(uname -s) == Darwin ]]; then
  codesign --force --deep --sign - "$product_root/dist/${app_name}.app"
fi

echo "$product_root/dist/${app_name}.app"

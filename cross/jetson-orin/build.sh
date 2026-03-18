#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /absolute/path/to/jetson-sysroot" >&2
  exit 2
fi

SYSROOT="$1"
TARGET="aarch64-unknown-linux-gnu"
PKG_CONFIG_LIBDIRS=(
  "$SYSROOT/usr/lib/aarch64-linux-gnu/pkgconfig"
  "$SYSROOT/usr/lib/pkgconfig"
  "$SYSROOT/usr/share/pkgconfig"
)

if [[ ! -d "$SYSROOT" ]]; then
  echo "error: sysroot directory not found: $SYSROOT" >&2
  exit 1
fi

export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"
export PKG_CONFIG_DIR=
export PKG_CONFIG_PATH=
export PKG_CONFIG_LIBDIR="$(IFS=:; echo "${PKG_CONFIG_LIBDIRS[*]}")"

cargo build --release --target "$TARGET" -p gst-plugin-eevideo -p eedeviced

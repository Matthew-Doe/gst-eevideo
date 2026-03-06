#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /absolute/path/to/jetson-sysroot" >&2
  exit 2
fi

SYSROOT="$1"
TARGET="aarch64-unknown-linux-gnu"

if [[ ! -d "$SYSROOT" ]]; then
  echo "error: sysroot directory not found: $SYSROOT" >&2
  exit 1
fi

export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"
export PKG_CONFIG_PATH="$SYSROOT/usr/lib/aarch64-linux-gnu/pkgconfig:$SYSROOT/usr/lib/pkgconfig:$SYSROOT/usr/share/pkgconfig"
export RUSTFLAGS="--sysroot=$SYSROOT"

cargo build --release --target "$TARGET" -p gst-plugin-eevideo


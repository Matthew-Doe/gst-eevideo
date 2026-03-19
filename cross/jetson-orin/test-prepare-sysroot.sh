#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SYSROOT_DIR="$(mktemp -d /tmp/eevideo-test-sysroot-XXXXXX)"
LOG_FILE="$(mktemp /tmp/eevideo-test-prepare-sysroot-log-XXXXXX)"
trap 'rm -rf "$SYSROOT_DIR" "$LOG_FILE"' EXIT

if ! bash "$ROOT_DIR/cross/jetson-orin/prepare-sysroot.sh" "$SYSROOT_DIR" >"$LOG_FILE" 2>&1; then
  cat "$LOG_FILE" >&2
  exit 1
fi

assert_file() {
  local path="$1"
  if [[ ! -f "$path" ]]; then
    echo "expected file to exist: $path" >&2
    exit 1
  fi
}

assert_file "$SYSROOT_DIR/usr/lib/aarch64-linux-gnu/pkgconfig/glib-2.0.pc"
assert_file "$SYSROOT_DIR/usr/lib/aarch64-linux-gnu/pkgconfig/gstreamer-1.0.pc"
assert_file "$SYSROOT_DIR/usr/lib/aarch64-linux-gnu/pkgconfig/gstreamer-base-1.0.pc"

echo "sysroot preparation test passed"

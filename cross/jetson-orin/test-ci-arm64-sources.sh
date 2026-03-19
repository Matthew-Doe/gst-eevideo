#!/usr/bin/env bash
set -euo pipefail

WORKFLOW_FILE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/.github/workflows/ci.yml"
workflow_contents="$(cat "$WORKFLOW_FILE")"

assert_contains() {
  local haystack="$1"
  local needle="$2"
  if [[ "$haystack" != *"$needle"* ]]; then
    echo "expected to find: $needle" >&2
    exit 1
  fi
}

assert_not_contains() {
  local haystack="$1"
  local needle="$2"
  if [[ "$haystack" == *"$needle"* ]]; then
    echo "did not expect to find: $needle" >&2
    exit 1
  fi
}

assert_contains "$workflow_contents" "- name: Prepare arm64 sysroot"
assert_contains "$workflow_contents" "bash cross/jetson-orin/prepare-sysroot.sh /tmp/jetson-sysroot"
assert_contains "$workflow_contents" "run: cross/jetson-orin/build.sh /tmp/jetson-sysroot"

assert_not_contains "$workflow_contents" "sudo dpkg --add-architecture arm64"
assert_not_contains "$workflow_contents" "sourceparts=\"$(mktemp -d)\""
assert_not_contains "$workflow_contents" "Install arm64 sysroot dependencies from Ubuntu Ports"
assert_not_contains "$workflow_contents" "libglib2.0-dev:arm64"

echo "ci arm64 source configuration looks correct"

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

assert_contains "$workflow_contents" "sudo install -d -m 0755 /etc/apt/arm64-ports.sources.list.d"
assert_contains "$workflow_contents" "sudo tee /etc/apt/arm64-ports.sources.list.d/ubuntu-ports.sources"
assert_contains "$workflow_contents" "Architectures: arm64"
assert_contains "$workflow_contents" "-o Dir::Etc::sourceparts=/etc/apt/arm64-ports.sources.list.d"

assert_not_contains "$workflow_contents" "sourceparts=\"$(mktemp -d)\""
assert_not_contains "$workflow_contents" "for src in /etc/apt/sources.list.d/*.sources; do"
assert_not_contains "$workflow_contents" "Architectures: amd64"

echo "ci arm64 source configuration looks correct"

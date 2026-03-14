#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LAUNCHER="$SCRIPT_DIR/eedeviced-launch.sh"

assert_contains() {
  local haystack="$1"
  local needle="$2"
  if [[ "$haystack" != *"$needle"* ]]; then
    echo "expected to find: $needle" >&2
    echo "actual: $haystack" >&2
    exit 1
  fi
}

assert_not_contains() {
  local haystack="$1"
  local needle="$2"
  if [[ "$haystack" == *"$needle"* ]]; then
    echo "did not expect to find: $needle" >&2
    echo "actual: $haystack" >&2
    exit 1
  fi
}

common_env() {
  export EEVIDEO_BIND=0.0.0.0:5683
  export EEVIDEO_ADVERTISE_ADDRESS=192.168.1.50
  export EEVIDEO_IFACE=eth0
  export EEVIDEO_PIXEL_FORMAT=uyvy
  export EEVIDEO_WIDTH=1280
  export EEVIDEO_HEIGHT=720
  export EEVIDEO_FPS=30
  export EEVIDEO_MTU=1200
}

probe_command() {
  EEVIDEO_PRINT_COMMAND=1 "$LAUNCHER"
}

common_env
export EEVIDEO_INPUT=synthetic
synthetic_output="$(probe_command)"
assert_contains "$synthetic_output" "--input"
assert_contains "$synthetic_output" "synthetic"
assert_not_contains "$synthetic_output" "--sensor-id"
assert_not_contains "$synthetic_output" "--device"
assert_not_contains "$synthetic_output" "--pipeline"

common_env
export EEVIDEO_INPUT=argus
export EEVIDEO_SENSOR_ID=3
argus_output="$(probe_command)"
assert_contains "$argus_output" "--sensor-id"
assert_contains "$argus_output" "3"
assert_not_contains "$argus_output" "--device"
assert_not_contains "$argus_output" "--pipeline"

common_env
export EEVIDEO_INPUT=v4l2
export EEVIDEO_DEVICE=/dev/video2
v4l2_output="$(probe_command)"
assert_contains "$v4l2_output" "--device"
assert_contains "$v4l2_output" "/dev/video2"
assert_not_contains "$v4l2_output" "--sensor-id"
assert_not_contains "$v4l2_output" "--pipeline"

common_env
export EEVIDEO_INPUT=pipeline
export EEVIDEO_PIPELINE="nvarguscamerasrc sensor-id=0 ! appsink name=framesink"
pipeline_output="$(probe_command)"
assert_contains "$pipeline_output" "--pipeline"
assert_contains "$pipeline_output" "nvarguscamerasrc sensor-id=0 ! appsink name=framesink"
assert_not_contains "$pipeline_output" "--sensor-id"
assert_not_contains "$pipeline_output" "--device"

common_env
export EEVIDEO_INPUT=pipeline
export EEVIDEO_PIPELINE=
if probe_command >/tmp/test-eedeviced-launch.stderr 2>&1; then
  echo "expected empty pipeline command to fail" >&2
  exit 1
fi
assert_contains "$(cat /tmp/test-eedeviced-launch.stderr)" "EEVIDEO_PIPELINE"

echo "launcher tests passed"

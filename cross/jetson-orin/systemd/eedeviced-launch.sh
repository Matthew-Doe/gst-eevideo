#!/usr/bin/env bash
set -euo pipefail

require_env() {
  local name="$1"
  local value="${!name:-}"
  if [[ -z "$value" ]]; then
    echo "$name must be set" >&2
    exit 1
  fi
}

build_command() {
  local -n out="$1"

  require_env EEVIDEO_BIND
  require_env EEVIDEO_INPUT
  require_env EEVIDEO_PIXEL_FORMAT
  require_env EEVIDEO_WIDTH
  require_env EEVIDEO_HEIGHT
  require_env EEVIDEO_FPS
  require_env EEVIDEO_MTU

  out=(
    /opt/eevideo/eedeviced
    --bind "$EEVIDEO_BIND"
    --input "$EEVIDEO_INPUT"
    --pixel-format "$EEVIDEO_PIXEL_FORMAT"
    --width "$EEVIDEO_WIDTH"
    --height "$EEVIDEO_HEIGHT"
    --fps "$EEVIDEO_FPS"
    --mtu "$EEVIDEO_MTU"
  )

  if [[ -n "${EEVIDEO_ADVERTISE_ADDRESS:-}" ]]; then
    out+=(--advertise-address "$EEVIDEO_ADVERTISE_ADDRESS")
  fi

  if [[ -n "${EEVIDEO_IFACE:-}" ]]; then
    out+=(--iface "$EEVIDEO_IFACE")
  fi

  case "$EEVIDEO_INPUT" in
    argus)
      require_env EEVIDEO_SENSOR_ID
      out+=(--sensor-id "$EEVIDEO_SENSOR_ID")
      ;;
    v4l2)
      require_env EEVIDEO_DEVICE
      out+=(--device "$EEVIDEO_DEVICE")
      ;;
    pipeline)
      require_env EEVIDEO_PIPELINE
      out+=(--pipeline "$EEVIDEO_PIPELINE")
      ;;
    synthetic)
      ;;
    *)
      echo "unsupported EEVIDEO_INPUT: $EEVIDEO_INPUT" >&2
      exit 1
      ;;
  esac

}

main() {
  local -a cmd=()
  build_command cmd

  if [[ "${EEVIDEO_PRINT_COMMAND:-0}" == "1" ]]; then
    printf '%s\n' "${cmd[@]}"
    return 0
  fi

  exec "${cmd[@]}"
}

main "$@"

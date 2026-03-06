# EEVideo Rust Plugin Implementation Profile

This repository implements a practical EEVideo v1 interoperability profile for
the current public source tree under `original_source_code/`.

## Scope

- `eevideosrc` receives the same compatibility leader/payload/trailer stream shape
  currently parsed by `goeevideo`.
- `eevideosink` emits that same compatibility format for host-to-host and local
  loopback use.
- Supported raw formats are limited to the formats already mapped by the Go
  viewer path: `GRAY8`, `GRAY16_LE`, `video/x-bayer` (`grbg`, `rggb`, `gbrg`,
  `bggr`), `RGB`, and `UYVY`.

## Explicit Non-Goals For v1

- No claim of conformance to the unfinished native EEVideo stream packet
  specification in `original_source_code/spec-main/spec-main/modules/ROOT/pages/stream.adoc`.
- No public CoAP/register-control API on the GStreamer elements.
- No JPEG transport.
- No resend, PTP/NTP timing profile, multicast tuning, FEC, or security profile.
- No dynamic mid-stream caps renegotiation.

## Source Of Truth

The wire-level behavior in this repo is intentionally aligned to the current Go
implementation, especially:

- the upstream capture module under `original_source_code/goeevideo-main/goeevideo-main/`
- `original_source_code/goeevideo-main/goeevideo-main/capPxlFmt.go`
- `original_source_code/eeview-main/eeview-main/gst/gst.go`

Where the prose spec and the public code differ, this repository follows the
public code.

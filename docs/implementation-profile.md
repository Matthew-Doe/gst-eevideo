# EEVideo Rust Plugin Implementation Profile

This repository implements a practical EEVideo v1 interoperability profile for
the current public EEVideo ecosystem and host-side Go behavior.

The normative packet model for this repository is documented in
[compatibility-stream-profile.md](compatibility-stream-profile.md).

## Scope

- `eevideosrc` receives the same compatibility leader/payload/trailer stream shape
  currently parsed by the public `goeevideo` receiver path.
- `eevideosink` emits that same compatibility format for host-to-host and local
  loopback use.
- Supported raw formats are limited to the formats already mapped by the public
  Go viewer path: `GRAY8`, `GRAY16_LE`, `video/x-bayer` (`grbg`, `rggb`,
  `gbrg`, `bggr`), `RGB`, and `UYVY`.

## Explicit Non-Goals For v1

- No claim of conformance to the unfinished native EEVideo stream packet
  specification published by the upstream EEVideo spec project.
- No public CoAP/register-control API on the GStreamer elements.
- No JPEG transport.
- No resend, PTP/NTP timing profile, multicast tuning, FEC, or security profile.
- No dynamic mid-stream caps renegotiation.

## Source Of Truth

The wire-level behavior in this repo is intentionally aligned to the current Go
implementation, especially:

- the public `goeevideo` capture path
- the public `goeevideo` pixel-format mapping
- the public `eeview` GStreamer bridge

Where the prose spec and the public code differ, this repository follows the
public code.

## Active Profile Rules

The active profile is the named `EEVideo Stream Compatibility Profile v1`.

In addition to the scope above, the profile fixes these v1 interoperability
rules:

- leader, payload, and trailer are the only packet classes
- width, height, payload type, and pixel format are fixed after the first
  complete frame
- zero-length payload packets are ignored as anomalies
- payload packets may arrive out of order within a frame and are buffered until
  gaps are filled, bounded overflow is detected, or timeout occurs
- the trailer closes the packet-id range for the frame, but does not complete
  the frame unless all prior payload packets have arrived and the final byte
  count matches the expected image size
- duplicate payload packets are ignored as anomalies
- packets beyond the declared trailer boundary drop the frame

See [compatibility-stream-profile.md](compatibility-stream-profile.md) for the
full receiver and sender rules.

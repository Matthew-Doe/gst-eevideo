# EEVideo Stream Compatibility Profile v1

This document defines the active wire-level interoperability contract implemented
by this repository.

It is intentionally narrower than the broader EEVideo specification work. The
goal is to make current host-side behavior explicit enough that independent
implementations can match it without guessing.

## Profile Identity

- Profile name: `EEVideo Stream Compatibility Profile v1`
- Packet family: compatibility `Leader` / `Payload` / `Trailer`
- Transport assumption: UDP datagrams carrying exactly one compatibility packet
- Scope: host-to-host transport and current public compatibility-path
  interoperability

## Fixed Stream Parameters

After the first completed frame, these fields are fixed for the lifetime of a
running stream:

- `width`
- `height`
- `payload_type`
- `pixel_format`

Any mid-stream change to those values is treated as a stream error by
`eevideosrc`. The stream must be restarted rather than renegotiated in place.

## Packet Model

The compatibility stream uses exactly three packet classes:

- `Leader`
  - starts a frame
  - carries `frame_id`, `packet_id`, `timestamp`, `payload_type`,
    `pixel_format`, `width`, and `height`
- `Payload`
  - carries `frame_id`, `packet_id`, and contiguous image bytes
- `Trailer`
  - terminates a frame
  - carries `frame_id` and the packet id immediately following the final payload

`frame_id` is the frame assembly key. `packet_id` is monotonic within a frame.

`eevideosink` emits:

- leader `packet_id = 0`
- payload packets starting at `1`
- trailer `packet_id = last_payload_packet_id + 1`

Receivers must not assume localhost-style in-order arrival. Payload packets may
arrive out of order, and the trailer may arrive before one or more missing
payload packets.

## Receiver Conformance Rules

`eevideosrc` and `eevideo-proto::FrameAssembler` implement these rules:

- a frame becomes active only after a valid leader arrives
- payload packets received without an active leader are dropped
- trailer packets received without an active leader are dropped
- a duplicate leader for an active `frame_id` drops the previous partial frame
  and restarts assembly from the new leader
- duplicate payload packets are ignored as anomalies
- out-of-order payload packets are buffered by `packet_id`
- a trailer closes the packet-id range for the frame, but the frame is not
  emitted until all missing payload packets up to that trailer have arrived
- payload packets at or beyond the declared trailer boundary drop the frame
- a frame is emitted only when the trailer has been seen and the received image
  byte count exactly matches the expected payload length for the negotiated
  format
- a frame whose packet range is complete but whose byte count is short is
  dropped immediately
- a frame with unresolved gaps is dropped on timeout

This profile still does not provide resend, FEC, or repair traffic. The
hardening here is limited to deterministic handling of loss, duplication, and
reordering.

## Sender Rules

Senders following this profile must:

- emit exactly one leader and one trailer per frame
- preserve image byte order across payload packets
- split image bytes into contiguous payload chunks
- use an MTU greater than or equal to the leader size

The current Rust sender enforces the minimum MTU by requiring at least
`44` bytes, which is the compatibility leader size.

## Pixel Format Registry

The active v1 profile supports these formats:

- `Mono8` -> `video/x-raw,format=GRAY8`
- `Mono16` -> `video/x-raw,format=GRAY16_LE`
- `BayerGR8` -> `video/x-bayer,format=grbg`
- `BayerRG8` -> `video/x-bayer,format=rggb`
- `BayerGB8` -> `video/x-bayer,format=gbrg`
- `BayerBG8` -> `video/x-bayer,format=bggr`
- `RGB8` -> `video/x-raw,format=RGB`
- `YUV422_8_UYVY` -> `video/x-raw,format=UYVY`

These mappings are part of the profile contract, not only implementation
examples.

## Timestamp Semantics

The leader timestamp is treated as the frame timestamp for the profile.
`eevideosrc` forwards that value to GStreamer buffer PTS, optionally offset by
its configured `latency-ms`.

This repository still assumes only stream-local consistency for timestamp units.
It does not currently claim a stronger wall-clock or device-clock interpretation
than that.

## Control-Plane Boundary

The v1 profile is transport-only. No public device control API is exposed by the
elements yet.

The sink implementation now routes stream lifecycle intent through an internal
control-session abstraction so a future CoAP/register control plane can be added
without rewriting packetization or socket I/O paths. The default backend remains
no-op in v1.

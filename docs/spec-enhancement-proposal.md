# EEVideo Streaming Spec Enhancement Proposal

## Title

Functional Clarifications and Interoperability Enhancements for the EEVideo v1 Stream Profile

## Status

Proposed

## Authors

Repository-local implementation proposal based on the current public EEVideo
specification and the imported upstream Go reference code.

## Motivation

The current public EEVideo specification defines a promising native stream model
and a stronger management plane, but it does not yet provide enough normative
detail to implement an interoperable host-side GStreamer source and sink without
making additional decisions.

To produce a functional Rust plugin, several gaps had to be resolved locally.
These decisions should be promoted into an explicit spec enhancement proposal so
future host and device implementations converge on the same behavior instead of
repeating incompatible assumptions.

This proposal is intentionally conservative. It does not attempt to redesign the
protocol. It captures the minimum additions and clarifications required to make
the current stream profile implementable.

## Problem Statement

The current public stream specification in
`original_source_code/spec-main/spec-main/modules/ROOT/pages/stream.adoc`
describes packet shapes, but leaves important interoperability behavior
unspecified. The public Go code also shows that the shipping host path still
uses a compatibility leader/payload/trailer format with PFNC-style pixel IDs.

That combination creates four practical problems:

1. A receiver cannot know whether to implement the prose EEVideo packet format
   or the currently deployed compatibility stream path.
2. A sender and receiver cannot reliably agree on pixel semantics from the
   current draft pixel format model alone.
3. Mid-stream behavior, frame completion rules, and loss handling are not
   specified tightly enough for consistent host implementations.
4. A GStreamer integration needs explicit caps mapping and timestamp rules that
   are not currently normative in the spec.

## Proposed Enhancements

### 1. Define an Explicit v1 Interoperability Profile

The specification should define a named stream interoperability profile for the
current public implementation state.

Proposed profile name:

- `EEVideo Stream Compatibility Profile v1`

This profile should state that:

- The wire behavior is aligned to the currently deployed compatibility
  leader/payload/trailer framing.
- The profile is valid for host-to-host streaming and current public device
  interoperability.
- The native EEVideo SoF/Data/EoF packet family remains a separate future
  profile until public host and device implementations exist.

This avoids ambiguity between the aspirational native packet design and the
actual interoperable stream path.

### 2. Standardize the v1 Packet Model

The compatibility profile should make the following packet behaviors normative:

- One leader packet starts a frame.
- Zero or more payload packets carry contiguous image bytes.
- One trailer packet terminates the frame.
- `frame_id` is the frame key for assembly.
- `packet_id` is strictly monotonic within a frame.
- The leader packet carries width, height, payload type, pixel format, and
  timestamp.
- Payload packets carry only image bytes after the fixed header.
- Trailer packets carry no additional image metadata.

The current plugin had to assume all of the above because the public Go receiver
already behaves this way.

### 3. Define a Normative v1 Pixel Format Registry

The draft EEVideo pixel format field is too coarse to support interoperable host
pipelines. The implementation had to promote the de facto PFNC-style identifiers
already used by the Go code into the functional source of truth.

The spec should add a v1 registry that explicitly includes at least:

- `Mono8`
- `Mono16`
- `BayerGR8`
- `BayerRG8`
- `BayerGB8`
- `BayerBG8`
- `RGB8`
- `YUV422_8_UYVY`

For each format, the spec should define:

- Numeric identifier
- Packing/layout
- Bytes per pixel or packing rule
- Expected host memory layout
- Canonical GStreamer caps mapping

Without this, a sender and receiver may agree on “mono” or “RGB” in principle
but still disagree on the actual byte layout.

### 4. Define Canonical Caps Mapping for Host Pipelines

The spec currently does not define how EEVideo pixel formats map into host media
frameworks. The plugin had to make these mappings explicit.

Proposed normative GStreamer mappings:

- `Mono8` -> `video/x-raw,format=GRAY8`
- `Mono16` -> `video/x-raw,format=GRAY16_LE`
- `BayerGR8` -> `video/x-bayer,format=grbg`
- `BayerRG8` -> `video/x-bayer,format=rggb`
- `BayerGB8` -> `video/x-bayer,format=gbrg`
- `BayerBG8` -> `video/x-bayer,format=bggr`
- `RGB8` -> `video/x-raw,format=RGB`
- `YUV422_8_UYVY` -> `video/x-raw,format=UYVY`

The spec should explicitly say that these mappings are part of the host
interoperability contract, not merely implementation examples.

### 5. Define Frame Completion and Rejection Rules

The receiver implementation had to define exact frame assembly behavior.
Equivalent rules should be promoted into the specification.

Proposed normative receiver rules:

- A frame begins only after a valid leader packet is received.
- A duplicate leader for an active `frame_id` invalidates the in-progress frame.
- A payload packet received before the leader is discarded.
- A payload packet with a non-consecutive `packet_id` causes the frame to be
  dropped.
- A payload packet that would overflow the expected frame buffer causes the
  frame to be dropped.
- A trailer without an active frame is discarded.
- A frame is complete only when the trailer is received.
- Incomplete frames may be dropped on timeout.

These rules are necessary for interoperable host behavior under packet loss,
duplication, and reordering.

### 6. Define Fixed-Format Stream Behavior

The draft does not specify whether width, height, or pixel format may change
mid-stream. The plugin had to reject such changes after the first completed
frame because GStreamer caps renegotiation is not a safe default for a v1
network source.

Proposed v1 rule:

- Width, height, payload type, and pixel format are fixed for the lifetime of a
  started stream.
- Any mid-stream change to those fields is a stream error and requires the
  stream to be stopped and restarted.

This should remain normative until a future EEVideo profile defines explicit
renegotiation behavior.

### 7. Define Timestamp Semantics for v1

The current draft identifies timestamps but does not fully define how hosts
should interpret them in a media pipeline.

The plugin had to assume:

- The timestamp in the leader packet is the frame timestamp.
- That timestamp is propagated to the host buffer PTS.
- Timestamp units are treated consistently within one stream.

The spec should explicitly define:

- Timestamp width
- Timestamp unit
- Epoch or monotonicity requirement
- Whether timestamps represent acquisition time, transmit time, or another
  device-defined event

Without this, timing-aware receivers cannot behave consistently.

### 8. Define Minimum MTU and Packetization Constraints

To make a sender functional, the implementation had to define a minimum packet
size large enough to carry the leader packet and a deterministic payload split
rule.

Proposed v1 rule:

- The configured maximum packet size must be at least the size of the leader
  packet.
- Image payload data is split into contiguous chunks of
  `max_packet_size - payload_header_size`.
- The sender must preserve byte order and image continuity across payload
  packets.

This removes ambiguity in packet sizing and makes sender behavior predictable.

### 9. Define the Relationship Between EEVideo Control and Stream Profiles

The public code already demonstrates that the management plane is ahead of the
stream plane. The spec should acknowledge this explicitly.

Proposed clarification:

- The management/register model is independently useful and may configure a
  stream profile that is not yet the native EEVideo packet family.
- A stream profile identifier should be exposed by the device so the host knows
  whether the configured stream is:
  - native EEVideo stream framing
  - the compatibility stream profile
  - another future profile

This removes guesswork from hosts and allows a cleaner migration path toward a
fully native stream protocol.

## Recommended Normative Text Additions

The following concepts should be added to the specification:

- A “Stream Compatibility Profile v1” section under the stream chapter
- A “Pixel Format Registry v1” section with numeric IDs and host mappings
- A “Receiver Conformance Rules” section covering duplicates, gaps, overflow,
  timeout, and trailer handling
- A “Fixed Stream Parameters” section defining restart-required changes
- A “Timestamp Semantics” section
- A “Stream Profile Identification” register or feature definition

## Backward Compatibility

This proposal is backward-compatible with the currently imported public Go
implementation because it mostly codifies behavior already required for that
code to work.

It does not block future native EEVideo packet formats. Instead, it cleanly
separates:

- the currently interoperable profile
- the future native profile

## Reference Implementation Impact

These enhancements were necessary to make the Rust plugin functional in this
repository:

- [eevideo-proto/compat_stream.rs](c:/devel/eevideo/crates/eevideo-proto/src/compat_stream.rs)
- [eevideo-proto/pixel_format.rs](c:/devel/eevideo/crates/eevideo-proto/src/pixel_format.rs)
- [eevideo-proto/assembler.rs](c:/devel/eevideo/crates/eevideo-proto/src/assembler.rs)
- [gst-plugin-eevideo/eevideosrc](c:/devel/eevideo/crates/gst-plugin-eevideo/src/eevideosrc/imp.rs)
- [gst-plugin-eevideo/eevideosink](c:/devel/eevideo/crates/gst-plugin-eevideo/src/eevideosink/imp.rs)

## Conclusion

The EEVideo specification does not need a major redesign to become more useful
for host implementations. It needs a tighter, named interoperability profile and
several currently implicit behaviors promoted into normative text.

The most important enhancement is to stop leaving the stream profile ambiguous.
Once the compatibility profile, pixel registry, fixed-format rule, and receiver
assembly rules are explicit, independent host and device implementations can be
built with much less risk of accidental divergence.

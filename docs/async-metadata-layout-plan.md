# EEVideo Async Metadata Packet Layout Plan

## Status

Design plan only. No implementation is proposed in this document.

## Goal

Define a next-generation EEVideo packet layout that handles metadata better
than the current compatibility leader/payload/trailer stream.

The new layout should support:

- multiple metadata blocks per frame
- self-description of what each metadata block contains
- asynchronous metadata that is not forced to arrive only as a trailer appendage
- metadata that can target a single frame, a frame range, a timestamp, a time range, or the stream as a whole
- small fixed fields such as frame counter and timestamp
- larger structured blobs such as sensor state, calibration, and vendor extensions

## Problem Summary

The current compatibility layout only has a small fixed leader and an untyped
image payload stream. That works for raw video transport, but it is a poor fit
for modern machine-vision metadata because:

- the metadata model is effectively a single opaque appendage
- the receiver has weak information about schema and field meaning
- metadata cannot be cleanly split into multiple typed chunks
- asynchronous device events do not map naturally to a frame trailer
- large metadata objects compete with image payload ordering and loss behavior

## Design Criteria

The packet layout should be evaluated against these criteria:

1. Can it carry multiple metadata objects in one frame or event?
2. Can it describe what each metadata object is without out-of-band tribal knowledge?
3. Can it send metadata independently of image payload timing?
4. Can a receiver partially decode useful metadata even if some packets are lost?
5. Can the layout grow without breaking old receivers?
6. Can a GStreamer source expose metadata without blocking frame delivery?
7. Can a sender prioritize video and metadata differently when needed?

## Survey Of Relevant Packet/Layout Patterns

### 1. RTP Base Header Extensions

Relevant source:

- RFC 3550: https://www.rfc-editor.org/rfc/rfc3550

Relevant observation:

- RTP base header extensions only allow a single extension block per packet.

Implication for EEVideo:

- This is too constrained as the primary metadata architecture.
- It works for a few small per-packet hints, but not for rich frame metadata.

### 2. RTP Generalized Header Extensions

Relevant source:

- RFC 8285: https://www.rfc-editor.org/rfc/rfc8285

Relevant observation:

- RTP can carry multiple extension elements in a packet, each with local identifiers and lengths.
- The RFC explicitly frames these as optional metadata and recommends sending them only when needed.
- It also warns that loss handling and repetition need to be designed deliberately.

Implication for EEVideo:

- This is a strong pattern for compact metadata items that are useful but not mandatory for decoding.
- It is still a bad fit for large or schema-rich metadata blobs.

### 3. KLV Metadata Streams

Relevant sources:

- RFC 6597: https://www.rfc-editor.org/rfc/rfc6597.html
- RFC 8088: https://www.rfc-editor.org/rfc/rfc8088.html

Relevant observation:

- KLV gives typed `key-length-value` metadata items and can carry arbitrary metadata.
- RFC 6597 allows multiple metadata items and sets to travel in RTP as their own logical units.
- RFC 8088 notes that KLV is useful for generic metadata, but fragmented metadata needs careful robustness treatment.

Implication for EEVideo:

- KLV is a strong model for self-describing, independently timed metadata.
- Its main weakness for EEVideo is implementation complexity and fragmentation sensitivity if used as the only metadata path.

### 4. GenDC Component/Part Containers

Relevant sources:

- EMVA GenICam introduction: https://www.emva.org/standards-technology/genicam/introduction-new/
- EMVA GenICam downloads: https://www.emva.org/standards-technology/genicam/genicam-downloads/
- GenDC 1.1 PDF: https://www.emva.org/wp-content/uploads/GenICam_GenDC_v1_1.pdf

Relevant observation:

- GenDC is transport-independent and explicitly supports image data and metadata in one container model.
- It has container headers, component headers, part headers, metadata-specific part fields, layout identifiers, and multiple flows.
- It supports multiple metadata components and group scoping.

Implication for EEVideo:

- This is the closest match to what EEVideo needs.
- The most valuable ideas to borrow are:
  - descriptor-driven containers
  - multiple typed parts
  - metadata layout identifiers
  - flow separation between descriptor, image parts, and metadata parts

### 5. CBOR For Structured Metadata Payloads

Relevant sources:

- RFC 8949: https://www.rfc-editor.org/rfc/rfc8949.html
- RFC 8610: https://www.rfc-editor.org/rfc/rfc8610

Relevant observation:

- CBOR is compact, extensible, and designed for binary protocols.
- CDDL can define metadata schemas cleanly for interoperable structured payloads.

Implication for EEVideo:

- CBOR is a strong default encoding for structured metadata values once the packet layout identifies the metadata object and schema.
- It should be used inside metadata payloads, not as the entire transport model.

## Options Considered

### Option A: Extend The Existing Frame Trailer

Description:

- Keep the current packet model and append a typed metadata directory plus chunks near the end of the frame.

Advantages:

- Smallest wire-format change
- Easy transition from the current compatibility stream

Disadvantages:

- Metadata is still frame-bound
- Async device events remain awkward
- Large metadata still arrives late
- Trailer loss makes metadata brittle

Assessment:

- Not recommended as the long-term design.

### Option B: Add A Dedicated Metadata Packet Family

Description:

- Keep the current video packets and introduce separate metadata packets carrying typed metadata objects.

Advantages:

- Async metadata becomes natural
- Easier than a full container redesign
- Good separation of image and metadata timing

Disadvantages:

- Relationship between image and metadata becomes ad hoc unless carefully modeled
- Harder to describe multi-part frame bundles cleanly
- Eventually grows into a container system anyway

Assessment:

- Viable as a transitional profile, but incomplete as the final architecture.

### Option C: Introduce A Descriptor + Parts + Flows Container Model

Description:

- Replace the current flat frame stream with a container model inspired by GenDC and informed by RTP/KLV separation ideas.

Advantages:

- Multiple metadata items per frame
- Natural support for async metadata
- Strong schema and layout signaling
- Clean separation of descriptor, video, and metadata delivery paths
- Better fit for future non-image payloads

Disadvantages:

- More design and implementation work
- Requires clear receiver assembly rules
- Needs careful versioning and migration planning

Assessment:

- Recommended.

## Recommended Packet Layout Direction

### Working Name

Working name:

- `EEVideo Multi-Part Transport Profile`

### Core Model

One logical transmission unit is a `Container`.

A container may represent:

- one video frame
- one burst
- one async metadata event
- one stream-level state update

Each container contains a descriptor plus zero or more parts.

### Packet Classes

The plan recommends these packet classes:

1. `Descriptor`
2. `DataPart`
3. `ContainerEnd`
4. `StreamEvent`

`Descriptor` and `StreamEvent` are control-like packets. `DataPart` carries
video or metadata bytes. `ContainerEnd` provides explicit completion.

### Common Header Fields

Each packet should carry:

- `version`
- `packet_kind`
- `stream_id`
- `flow_id`
- `container_id`
- `part_id`
- `sequence`
- `flags`
- `send_timestamp`

This keeps packet parsing uniform even when the payload kind differs.

### Container Descriptor Model

The descriptor should describe all parts that belong to the container.

Each part descriptor should include:

- `part_id`
- `part_kind`
- `group_id`
- `format_id`
- `encoding_id`
- `schema_id`
- `relation_kind`
- `target_ref`
- `declared_size`
- `flow_id`
- `priority`

### Part Kinds

At minimum:

- `image`
- `metadata_structured`
- `metadata_binary`
- `xml_or_schema`
- `calibration`
- `event`
- `vendor_private`

### Relation Kinds

Metadata relation must be explicit. Recommended values:

- `exact_frame`
- `frame_range`
- `timestamp_point`
- `timestamp_range`
- `stream_state`
- `device_event`
- `applies_to_group`

This is the key improvement over a simple chunk blob.

### Metadata Payload Strategy

### Small Fixed Metadata

Examples:

- frame counter
- sensor timestamp
- exposure time
- analog gain
- rolling-shutter status

Recommendation:

- allow compact fixed-layout metadata parts for very common fields
- reserve stable IDs for a small standard registry

### Structured Metadata

Examples:

- sensor operating mode
- sequencer state
- ISP statistics
- thermal and power telemetry
- vendor diagnostics

Recommendation:

- use CBOR as the default structured metadata encoding
- define schemas with CDDL
- identify schemas using `schema_id` plus an optional `schema_uri`

### Binary Opaque Metadata

Examples:

- vendor blobs
- calibration blobs
- compressed side-data

Recommendation:

- allow opaque payloads with explicit `encoding_id`
- require a `schema_id` or vendor type ID even for opaque binary data

### Flow Model

The plan should assume at least three logical flows:

- `flow 0`: descriptors and stream events
- `flow 1`: image data
- `flow 2`: metadata data

Optional:

- `flow 3+`: extra metadata classes, burst data, auxiliary image planes

Why this matters:

- descriptors can be prioritized and repeated
- metadata can arrive independently of image payload
- receivers can choose to consume image only, metadata only, or both

### Async Metadata Model

The layout must treat metadata as first-class data, not just frame adornment.

Recommended rules:

1. Metadata may be sent in the same container as a frame.
2. Metadata may be sent in a separate metadata-only container.
3. Metadata may target a frame by `container_id` or `frame_id`.
4. Metadata may target a time point or time range.
5. Late metadata is valid if it references an already delivered frame or timestamp.
6. Receivers should be able to emit video immediately and attach metadata later.

This directly addresses the use case of timestamps, frame count, sensor state,
and event data arriving on different cadences.

### Receiver Behavior Plan

The future receiver design should support:

- immediate frame delivery when image parts are complete
- deferred metadata association when metadata arrives later
- partial metadata decoding if some metadata parts are lost
- per-part drop accounting instead of whole-frame invalidation for all metadata errors

Recommended receiver state:

- `container table`
- `layout table`
- `metadata schema cache`
- `frame-to-metadata index`
- `timestamp-to-metadata index`

### Sender Behavior Plan

The future sender design should support:

- predeclared metadata layouts that only change on schema updates
- repeated descriptors when layout changes
- metadata-only containers for async events
- optional metadata throttling or rate control
- different pacing for image and metadata flows

### Why This Is Better Than A Single Chunk Blob

This design improves on the current chunk-style approach because it gives EEVideo:

- more than one metadata unit per frame
- typed metadata instead of one opaque appendage
- explicit schema and layout signaling
- multiple logical flows
- async metadata without blocking image delivery
- room for future multi-component payloads beyond 2D video

### Compatibility Strategy

The plan should preserve a migration path:

### Phase 0

- keep the current compatibility stream as-is
- formalize it as the legacy transport profile

### Phase 1

- introduce an internal abstraction for multiple packet profiles
- keep plugin APIs stable

### Phase 2

- define the new metadata-aware container profile on paper
- build test vectors before runtime code

### Phase 3

- implement sender and receiver behind a new experimental profile switch

### Phase 4

- add GStreamer metadata mapping and async side-channel exposure

### Proposed Work Plan

### Workstream 1: Requirements

- enumerate metadata classes that must be supported in v1 of the new profile
- separate frame-bound metadata from truly async metadata
- define latency and loss-tolerance expectations per metadata class

Deliverable:

- a requirements matrix

### Workstream 2: Wire Format Draft

- define the common packet header
- define descriptor semantics
- define part kinds and relation kinds
- define flow behavior and sequencing rules
- define loss, timeout, and late-arrival behavior

Deliverable:

- an EEVideo transport draft document

### Workstream 3: Metadata Registry

- define a small standard metadata registry
- choose CBOR for structured metadata
- define `schema_id` and `schema_uri` conventions
- define vendor extension ranges

Deliverable:

- a metadata registry draft

### Workstream 4: GStreamer Mapping Plan

- decide how `eevideosrc` exposes metadata
- decide how `eevideosink` accepts metadata
- define when metadata becomes buffer meta versus side-stream messages

Deliverable:

- a GStreamer integration design note

### Workstream 5: Validation Plan

- build golden packet fixtures
- test multiple metadata parts per frame
- test metadata-only containers
- test late metadata arrival
- test descriptor repetition and layout changes
- test loss of metadata without loss of video

Deliverable:

- an interoperability and robustness test plan

### Recommended Initial Scope For The New Design

To keep the design manageable, the first draft should standardize only:

- one image part per frame container
- multiple metadata parts per container
- metadata-only containers
- three flows: descriptor, image, metadata
- CBOR as the default structured metadata encoding
- fixed IDs for timestamp, frame counter, exposure, gain, and temperature

What should be deferred:

- reliability/repair for metadata
- security profile
- compression of metadata payloads
- nested containers
- multiple independent image components in the first shipping version

## Recommendation

Do not evolve the current trailer model further.

The best long-term direction is a descriptor-driven multi-part container
profile, borrowing:

- the container/component/part thinking from GenDC
- the optional, repeated, small-extension mindset from RTP header extensions
- the typed metadata object thinking from KLV
- the compact structured encoding approach of CBOR

That combination is the strongest fit for EEVideo’s need to carry both image
data and asynchronous machine-vision metadata without forcing every metadata
problem into a single opaque chunk.

## Sources

- RFC 3550, RTP: https://www.rfc-editor.org/rfc/rfc3550
- RFC 8285, RTP Header Extensions: https://www.rfc-editor.org/rfc/rfc8285
- RFC 6597, RTP payload for SMPTE ST 336 KLV: https://www.rfc-editor.org/rfc/rfc6597.html
- RFC 8088, RTP payload format guidance: https://www.rfc-editor.org/rfc/rfc8088.html
- RFC 8949, CBOR: https://www.rfc-editor.org/rfc/rfc8949.html
- RFC 8610, CDDL: https://www.rfc-editor.org/rfc/rfc8610
- EMVA GenICam introduction: https://www.emva.org/standards-technology/genicam/introduction-new/
- EMVA GenICam downloads: https://www.emva.org/standards-technology/genicam/genicam-downloads/
- EMVA GenDC 1.1 PDF: https://www.emva.org/wp-content/uploads/GenICam_GenDC_v1_1.pdf

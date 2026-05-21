---
source_file: "crates/eevideo-proto/src/assembler.rs"
type: "code"
community: "Frame Assembly"
location: "L1"
tags:
  - graphify/code
  - graphify/EXTRACTED
  - community/Frame_Assembly
---

# assembler.rs

## Connections
- [[AssembleError]] - `contains` [EXTRACTED]
- [[FrameAssembler]] - `contains` [EXTRACTED]
- [[FrameDropReason]] - `contains` [EXTRACTED]
- [[FrameEvent]] - `contains` [EXTRACTED]
- [[FrameKey]] - `contains` [EXTRACTED]
- [[FrameProgress]] - `contains` [EXTRACTED]
- [[PartialFrame]] - `contains` [EXTRACTED]
- [[append_payload_bytes()]] - `contains` [EXTRACTED]
- [[assembles_a_complete_frame()]] - `contains` [EXTRACTED]
- [[assembles_frame_with_reordered_payloads_and_early_trailer()]] - `contains` [EXTRACTED]
- [[buffered_payloads_overflow()]] - `contains` [EXTRACTED]
- [[build_partial_frame()]] - `contains` [EXTRACTED]
- [[drops_frame_when_buffered_reordered_payloads_exceed_remaining_capacity()]] - `contains` [EXTRACTED]
- [[drops_short_frame_when_trailer_closes_packet_range()]] - `contains` [EXTRACTED]
- [[duplicate_leader_restarts_frame_assembly()]] - `contains` [EXTRACTED]
- [[flush_pending_payloads()]] - `contains` [EXTRACTED]
- [[ignores_zero_length_payload_without_advancing_frame()]] - `contains` [EXTRACTED]
- [[keeps_missing_payload_gap_open_until_timeout()]] - `contains` [EXTRACTED]
- [[pending_payload_bytes()]] - `contains` [EXTRACTED]
- [[progress_frame()]] - `contains` [EXTRACTED]

#graphify/code #graphify/EXTRACTED #community/Frame_Assembly
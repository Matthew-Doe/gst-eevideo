---
type: community
cohesion: 0.15
members: 27
---

# Frame Assembly

**Cohesion:** 0.15 - loosely connected
**Members:** 27 nodes

## Members
- [[.fmt()_2]] - code - crates/eevideo-proto/src/assembler.rs
- [[.ingest()]] - code - crates/eevideo-proto/src/assembler.rs
- [[.ingest_view()]] - code - crates/eevideo-proto/src/assembler.rs
- [[.new()_3]] - code - crates/eevideo-proto/src/assembler.rs
- [[.reap_timeouts()]] - code - crates/eevideo-proto/src/assembler.rs
- [[.reconcile_frame()]] - code - crates/eevideo-proto/src/assembler.rs
- [[AssembleError]] - code - crates/eevideo-proto/src/assembler.rs
- [[FrameAssembler]] - code - crates/eevideo-proto/src/assembler.rs
- [[FrameDropReason]] - code - crates/eevideo-proto/src/assembler.rs
- [[FrameEvent]] - code - crates/eevideo-proto/src/assembler.rs
- [[FrameKey]] - code - crates/eevideo-proto/src/assembler.rs
- [[FrameProgress]] - code - crates/eevideo-proto/src/assembler.rs
- [[PartialFrame]] - code - crates/eevideo-proto/src/assembler.rs
- [[append_payload_bytes()]] - code - crates/eevideo-proto/src/assembler.rs
- [[assembler.rs]] - code - crates/eevideo-proto/src/assembler.rs
- [[assembles_a_complete_frame()]] - code - crates/eevideo-proto/src/assembler.rs
- [[assembles_frame_with_reordered_payloads_and_early_trailer()]] - code - crates/eevideo-proto/src/assembler.rs
- [[buffered_payloads_overflow()]] - code - crates/eevideo-proto/src/assembler.rs
- [[build_partial_frame()]] - code - crates/eevideo-proto/src/assembler.rs
- [[drops_frame_when_buffered_reordered_payloads_exceed_remaining_capacity()]] - code - crates/eevideo-proto/src/assembler.rs
- [[drops_short_frame_when_trailer_closes_packet_range()]] - code - crates/eevideo-proto/src/assembler.rs
- [[duplicate_leader_restarts_frame_assembly()]] - code - crates/eevideo-proto/src/assembler.rs
- [[flush_pending_payloads()]] - code - crates/eevideo-proto/src/assembler.rs
- [[ignores_zero_length_payload_without_advancing_frame()]] - code - crates/eevideo-proto/src/assembler.rs
- [[keeps_missing_payload_gap_open_until_timeout()]] - code - crates/eevideo-proto/src/assembler.rs
- [[pending_payload_bytes()]] - code - crates/eevideo-proto/src/assembler.rs
- [[progress_frame()]] - code - crates/eevideo-proto/src/assembler.rs

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/Frame_Assembly
SORT file.name ASC
```

---
type: community
cohesion: 0.08
members: 28
---

# Viewer CLI

**Cohesion:** 0.08 - loosely connected
**Members:** 28 nodes

## Members
- [[EncoderKind]] - code - crates/eeview/src/lib.rs
- [[EncoderSpec]] - code - crates/eeview/src/lib.rs
- [[ManagedTransportSettings]] - code - crates/eeview/src/lib.rs
- [[SourceStats]] - code - crates/eeview/src/lib.rs
- [[TerminalReason]] - code - crates/eeview/src/lib.rs
- [[advertised_stream_mode()]] - code - crates/eeview/src/lib.rs
- [[advertised_stream_overlay_text()]] - code - crates/eeview/src/lib.rs
- [[builds_overlay_text_from_advertised_stream_mode()]] - code - crates/eeview/src/lib.rs
- [[finalize_run_result()]] - code - crates/eeview/src/lib.rs
- [[finalize_run_result_keeps_primary_error_and_stop_error()]] - code - crates/eeview/src/lib.rs
- [[format_final_reason()]] - code - crates/eeview/src/lib.rs
- [[format_mode_overlay_text()]] - code - crates/eeview/src/lib.rs
- [[format_source_anomaly_breakdown()]] - code - crates/eeview/src/lib.rs
- [[format_source_stats()]] - code - crates/eeview/src/lib.rs
- [[formats_packet_anomaly_breakdown_when_present()]] - code - crates/eeview/src/lib.rs
- [[formats_source_stats_as_a_single_line()]] - code - crates/eeview/src/lib.rs
- [[formats_terminal_reason_for_ctrl_c()]] - code - crates/eeview/src/lib.rs
- [[formats_terminal_reason_for_eos()]] - code - crates/eeview/src/lib.rs
- [[formats_terminal_reason_for_pipeline_error()]] - code - crates/eeview/src/lib.rs
- [[formats_terminal_reason_for_stop_error()]] - code - crates/eeview/src/lib.rs
- [[lib.rs_5]] - code - crates/eeview/src/lib.rs
- [[overlay_is_enabled_by_default()]] - code - crates/eeview/src/lib.rs
- [[parses_no_overlay_flag()]] - code - crates/eeview/src/lib.rs
- [[read_source_stats()]] - code - crates/eeview/src/lib.rs
- [[read_u64_property()]] - code - crates/eeview/src/lib.rs
- [[selects_stable_managed_transport_defaults()]] - code - crates/eeview/src/lib.rs
- [[suggested_record_extensions_match_encoder_kind()]] - code - crates/eeview/src/lib.rs
- [[suggested_record_path()]] - code - crates/eeview/src/lib.rs

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/Viewer_CLI
SORT file.name ASC
```

## Connections to other communities
- 13 edges to [[_COMMUNITY_Pipeline Builder]]
- 8 edges to [[_COMMUNITY_Control CLI]]
- 1 edge to [[_COMMUNITY_Fake Device]]
- 1 edge to [[_COMMUNITY_Provider Backend]]

## Top bridge nodes
- [[lib.rs_5]] - degree 47, connects to 4 communities
- [[advertised_stream_overlay_text()]] - degree 3, connects to 1 community
- [[read_source_stats()]] - degree 3, connects to 1 community
- [[finalize_run_result()]] - degree 3, connects to 1 community
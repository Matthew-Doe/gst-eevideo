---
type: community
cohesion: 0.25
members: 14
---

# Pipeline Builder

**Cohesion:** 0.25 - loosely connected
**Members:** 14 nodes

## Members
- [[add_display_branch()]] - code - crates/eeview/src/lib.rs
- [[build_pipeline()]] - code - crates/eeview/src/lib.rs
- [[build_pipeline_keeps_fps_overlay_when_mode_text_is_unavailable()]] - code - crates/eeview/src/lib.rs
- [[build_pipeline_keeps_record_branch_overlay_free()]] - code - crates/eeview/src/lib.rs
- [[build_pipeline_omits_overlay_when_disabled()]] - code - crates/eeview/src/lib.rs
- [[build_pipeline_uses_overlay_elements_by_default()]] - code - crates/eeview/src/lib.rs
- [[default_control_backend()]] - code - crates/eevideo-control/src/lib.rs
- [[ensure_elements_available()]] - code - crates/eeview/src/lib.rs
- [[init_gst()_1]] - code - crates/eeview/src/lib.rs
- [[link_into_mux()]] - code - crates/eeview/src/lib.rs
- [[link_tee_branch()]] - code - crates/eeview/src/lib.rs
- [[make_element()]] - code - crates/eeview/src/lib.rs
- [[select_encoder()]] - code - crates/eeview/src/lib.rs
- [[select_managed_transport_settings()]] - code - crates/eeview/src/lib.rs

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/Pipeline_Builder
SORT file.name ASC
```

## Connections to other communities
- 13 edges to [[_COMMUNITY_Viewer CLI]]
- 3 edges to [[_COMMUNITY_Control CLI]]
- 3 edges to [[_COMMUNITY_Control Backend]]
- 2 edges to [[_COMMUNITY_Sink Element]]

## Top bridge nodes
- [[build_pipeline()]] - degree 10, connects to 2 communities
- [[default_control_backend()]] - degree 9, connects to 2 communities
- [[select_managed_transport_settings()]] - degree 6, connects to 2 communities
- [[select_encoder()]] - degree 4, connects to 2 communities
- [[build_pipeline_keeps_record_branch_overlay_free()]] - degree 6, connects to 1 community
---
type: community
cohesion: 0.09
members: 41
---

# Provider Backend

**Cohesion:** 0.09 - loosely connected
**Members:** 41 nodes

## Members
- [[.capture_configuration()]] - code - crates/eedeviced/src/lib.rs
- [[.current_format()_3]] - code - crates/eedeviced/src/providers/mod.rs
- [[.default()_9]] - code - crates/eedeviced/src/lib.rs
- [[.fmt()_9]] - code - crates/eedeviced/src/lib.rs
- [[.local_addr()_2]] - code - crates/eedeviced/src/lib.rs
- [[.next_frame()_3]] - code - crates/eedeviced/src/providers/mod.rs
- [[.runtime_config()_1]] - code - crates/eedeviced/src/lib.rs
- [[.shutdown()_2]] - code - crates/eedeviced/src/lib.rs
- [[.spawn()_3]] - code - crates/eedeviced/src/lib.rs
- [[.start_capture()_3]] - code - crates/eedeviced/src/providers/mod.rs
- [[.stop_capture()_3]] - code - crates/eedeviced/src/providers/mod.rs
- [[.try_from()]] - code - crates/eedeviced/src/lib.rs
- [[.uri()_2]] - code - crates/eedeviced/src/lib.rs
- [[CliPixelFormat]] - code - crates/eedeviced/src/lib.rs
- [[DeviceDaemon]] - code - crates/eedeviced/src/lib.rs
- [[DeviceDaemonConfig]] - code - crates/eedeviced/src/lib.rs
- [[InputKind]] - code - crates/eedeviced/src/lib.rs
- [[ProviderBackend]] - code - crates/eedeviced/src/providers/mod.rs
- [[ProviderConfig]] - code - crates/eedeviced/src/providers/mod.rs
- [[argus_pipeline_description_uses_expected_elements()]] - code - crates/eedeviced/src/lib.rs
- [[argus_rejects_non_uyvy_formats()]] - code - crates/eedeviced/src/lib.rs
- [[build_capture_backend()]] - code - crates/eedeviced/src/providers/mod.rs
- [[caps_mapping_supports_requested_formats()]] - code - crates/eedeviced/src/lib.rs
- [[cli_maps_provider_specific_options()]] - code - crates/eedeviced/src/lib.rs
- [[legacy_input_enum_still_parses_synthetic_and_argus()]] - code - crates/eedeviced/src/lib.rs
- [[lib.rs_7]] - code - crates/eedeviced/src/lib.rs
- [[main_entry()]] - code - crates/eedeviced/src/lib.rs
- [[mod.rs_3]] - code - crates/eedeviced/src/providers/mod.rs
- [[packed_buffer_validation_rejects_mismatches()]] - code - crates/eedeviced/src/lib.rs
- [[parses_pixel_format_aliases()]] - code - crates/eedeviced/src/lib.rs
- [[pipeline_provider_requires_framesink()]] - code - crates/eedeviced/src/lib.rs
- [[receive_frame()]] - code - crates/eedeviced/src/lib.rs
- [[reject_unexpected_option()]] - code - crates/eedeviced/src/lib.rs
- [[reject_unused_cli_options()]] - code - crates/eedeviced/src/lib.rs
- [[rejects_uyvy_odd_width()]] - code - crates/eedeviced/src/lib.rs
- [[require_cli_option()]] - code - crates/eedeviced/src/lib.rs
- [[supports_non_uyvy_odd_width_formats()]] - code - crates/eedeviced/src/lib.rs
- [[synthetic_provider_streams_configured_fixed_formats()]] - code - crates/eedeviced/src/lib.rs
- [[v4l2_pipeline_description_requests_configured_caps()]] - code - crates/eedeviced/src/lib.rs
- [[validate_config()]] - code - crates/eedeviced/src/lib.rs
- [[validate_provider_config()]] - code - crates/eedeviced/src/providers/mod.rs

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/Provider_Backend
SORT file.name ASC
```

## Connections to other communities
- 5 edges to [[_COMMUNITY_Control CLI]]
- 4 edges to [[_COMMUNITY_Fake Device]]
- 1 edge to [[_COMMUNITY_Viewer CLI]]

## Top bridge nodes
- [[main_entry()]] - degree 6, connects to 3 communities
- [[lib.rs_7]] - degree 25, connects to 2 communities
- [[.spawn()_3]] - degree 9, connects to 1 community
- [[DeviceDaemon]] - degree 7, connects to 1 community
- [[.shutdown()_2]] - degree 3, connects to 1 community
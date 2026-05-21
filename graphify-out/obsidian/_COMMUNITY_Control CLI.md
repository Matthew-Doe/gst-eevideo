---
type: community
cohesion: 0.10
members: 27
---

# Control CLI

**Cohesion:** 0.10 - loosely connected
**Members:** 27 nodes

## Members
- [[.selector()]] - code - crates/eevid/src/lib.rs
- [[Command]] - code - crates/eevid/src/lib.rs
- [[FieldReadArgs]] - code - crates/eevid/src/lib.rs
- [[FieldWriteArgs]] - code - crates/eevid/src/lib.rs
- [[RegisterSelectorArgs]] - code - crates/eevid/src/lib.rs
- [[RegisterWriteArgs]] - code - crates/eevid/src/lib.rs
- [[StreamArgs]] - code - crates/eevid/src/lib.rs
- [[StreamStopArgs]] - code - crates/eevid/src/lib.rs
- [[build_stream_request()]] - code - crates/eevid/src/lib.rs
- [[controller()]] - code - crates/eevid/src/lib.rs
- [[describe_command_works_against_fake_device()]] - code - crates/eevid/src/lib.rs
- [[format_advertised_stream()]] - code - crates/eevid/src/lib.rs
- [[format_applied_stream()]] - code - crates/eevid/src/lib.rs
- [[format_bus_error()]] - code - crates/eeview/src/lib.rs
- [[format_register_value()]] - code - crates/eevid/src/lib.rs
- [[lib.rs_2]] - code - crates/eevid/src/lib.rs
- [[parse_pixel_format_arg()]] - code - crates/eevid/src/lib.rs
- [[parse_u32_arg()]] - code - crates/eevid/src/lib.rs
- [[parses_decimal_and_hex_numbers()]] - code - crates/eevid/src/lib.rs
- [[parses_pixel_formats()]] - code - crates/eevid/src/lib.rs
- [[pipeline_start_error()]] - code - crates/eeview/src/lib.rs
- [[pixel_format_name()]] - code - crates/eevid/src/lib.rs
- [[profile_name()]] - code - crates/eevid/src/lib.rs
- [[resolve_target()]] - code - crates/eeview/src/lib.rs
- [[run()]] - code - crates/eedeviced/src/lib.rs
- [[stream_start_command_starts_fake_device()]] - code - crates/eevid/src/lib.rs
- [[wait_for_terminal_bus_message()]] - code - crates/eeview/src/lib.rs

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/Control_CLI
SORT file.name ASC
```

## Connections to other communities
- 8 edges to [[_COMMUNITY_Viewer CLI]]
- 5 edges to [[_COMMUNITY_Fake Device]]
- 5 edges to [[_COMMUNITY_Provider Backend]]
- 3 edges to [[_COMMUNITY_Pipeline Builder]]

## Top bridge nodes
- [[run()]] - degree 27, connects to 4 communities
- [[lib.rs_2]] - degree 24, connects to 2 communities
- [[resolve_target()]] - degree 3, connects to 1 community
- [[wait_for_terminal_bus_message()]] - degree 2, connects to 1 community
- [[pipeline_start_error()]] - degree 2, connects to 1 community
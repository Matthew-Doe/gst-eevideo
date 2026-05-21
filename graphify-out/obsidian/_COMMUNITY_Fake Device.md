---
type: community
cohesion: 0.12
members: 25
---

# Fake Device

**Cohesion:** 0.12 - loosely connected
**Members:** 25 nodes

## Members
- [[.capture_config()]] - code - crates/eefakedev/src/lib.rs
- [[.default()]] - code - crates/eefakedev/src/lib.rs
- [[.drain_events()]] - code - crates/eefakedev/src/lib.rs
- [[.drain_events()_2]] - code - crates/eedeviced/src/lib.rs
- [[.effective_transmit_pixel_format()]] - code - crates/eefakedev/src/lib.rs
- [[.from()]] - code - crates/eefakedev/src/lib.rs
- [[.local_addr()]] - code - crates/eefakedev/src/lib.rs
- [[.registers()]] - code - crates/eefakedev/src/lib.rs
- [[.runtime_config()]] - code - crates/eefakedev/src/lib.rs
- [[.shutdown()]] - code - crates/eefakedev/src/lib.rs
- [[.spawn()]] - code - crates/eefakedev/src/lib.rs
- [[.start_count()]] - code - crates/eefakedev/src/lib.rs
- [[.stop_count()]] - code - crates/eefakedev/src/lib.rs
- [[.uri()]] - code - crates/eefakedev/src/lib.rs
- [[.validate()]] - code - crates/eefakedev/src/lib.rs
- [[Cli]] - code - crates/eedeviced/src/lib.rs
- [[FakeDeviceConfig]] - code - crates/eefakedev/src/lib.rs
- [[FakeDeviceServer]] - code - crates/eefakedev/src/lib.rs
- [[enable_bit_transitions_start_and_stop_counts()]] - code - crates/eefakedev/src/lib.rs
- [[lib.rs]] - code - crates/eefakedev/src/lib.rs
- [[parse_pixel_format()]] - code - crates/eefakedev/src/lib.rs
- [[parses_supported_pixel_formats()]] - code - crates/eefakedev/src/lib.rs
- [[print_runtime_events()]] - code - crates/eedeviced/src/lib.rs
- [[wait_until()]] - code - crates/eefakedev/src/lib.rs
- [[write_u32_eventually()]] - code - crates/eefakedev/src/lib.rs

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/Fake_Device
SORT file.name ASC
```

## Connections to other communities
- 5 edges to [[_COMMUNITY_Control CLI]]
- 4 edges to [[_COMMUNITY_Provider Backend]]
- 1 edge to [[_COMMUNITY_Viewer CLI]]

## Top bridge nodes
- [[Cli]] - degree 4, connects to 3 communities
- [[lib.rs]] - degree 11, connects to 2 communities
- [[print_runtime_events()]] - degree 5, connects to 2 communities
- [[.spawn()]] - degree 6, connects to 1 community
- [[.shutdown()]] - degree 2, connects to 1 community
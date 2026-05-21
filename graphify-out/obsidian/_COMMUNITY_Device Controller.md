---
type: community
cohesion: 0.09
members: 47
---

# Device Controller

**Cohesion:** 0.09 - loosely connected
**Members:** 47 nodes

## Members
- [[.address()]] - code - crates/eevideo-control/src/register_map.rs
- [[.backend()]] - code - crates/eevideo-control/src/controller.rs
- [[.client_and_device()]] - code - crates/eevideo-control/src/controller.rs
- [[.configure_stream()]] - code - crates/eevideo-control/src/controller.rs
- [[.describe()_1]] - code - crates/eevideo-control/src/controller.rs
- [[.discover()_1]] - code - crates/eevideo-control/src/controller.rs
- [[.name()]] - code - crates/eevideo-control/src/register_map.rs
- [[.new()_4]] - code - crates/eevideo-control/src/register_map.rs
- [[.new()_9]] - code - crates/eevideo-control/src/controller.rs
- [[.read_field()]] - code - crates/eevideo-control/src/controller.rs
- [[.read_register()]] - code - crates/eevideo-control/src/controller.rs
- [[.shared_backend()]] - code - crates/eevideo-control/src/controller.rs
- [[.start_stream()]] - code - crates/eevideo-control/src/controller.rs
- [[.stop_stream()]] - code - crates/eevideo-control/src/controller.rs
- [[.write_field()]] - code - crates/eevideo-control/src/controller.rs
- [[.write_register()]] - code - crates/eevideo-control/src/controller.rs
- [[DeviceController]] - code - crates/eevideo-control/src/controller.rs
- [[DeviceDescription]] - code - crates/eevideo-control/src/controller.rs
- [[DeviceSummary]] - code - crates/eevideo-control/src/controller.rs
- [[FieldUpdate]] - code - crates/eevideo-control/src/register_map.rs
- [[RegisterSelector]] - code - crates/eevideo-control/src/register_map.rs
- [[RegisterValue]] - code - crates/eevideo-control/src/register_map.rs
- [[controller.rs]] - code - crates/eevideo-control/src/controller.rs
- [[describe_reads_live_stream_mode_when_yaml_cache_exists()]] - code - crates/eevideo-control/src/controller.rs
- [[device()]] - code - crates/eevideo-control/src/register_map.rs
- [[extract_field()]] - code - crates/eevideo-control/src/register_map.rs
- [[extract_field_uses_msb_and_len()]] - code - crates/eevideo-control/src/register_map.rs
- [[field_mask()]] - code - crates/eevideo-control/src/register_map.rs
- [[field_mask_rejects_invalid_lengths()]] - code - crates/eevideo-control/src/register_map.rs
- [[maybe_read_stream_field()]] - code - crates/eevideo-control/src/controller.rs
- [[pixel_format_from_device_bits()_1]] - code - crates/eevideo-control/src/controller.rs
- [[read_advertised_stream_mode()]] - code - crates/eevideo-control/src/controller.rs
- [[read_advertised_streams()]] - code - crates/eevideo-control/src/controller.rs
- [[read_register_field()]] - code - crates/eevideo-control/src/register_map.rs
- [[read_register_value()]] - code - crates/eevideo-control/src/register_map.rs
- [[register_error()]] - code - crates/eevideo-control/src/register_map.rs
- [[register_map.rs]] - code - crates/eevideo-control/src/register_map.rs
- [[resolve_register()]] - code - crates/eevideo-control/src/register_map.rs
- [[resolve_stream_prefix()]] - code - crates/eevideo-control/src/register_map.rs
- [[resolve_stream_prefix_accepts_single_stream_devices()]] - code - crates/eevideo-control/src/register_map.rs
- [[selector_address_helper_is_stable()]] - code - crates/eevideo-control/src/register_map.rs
- [[stream_prefixes()]] - code - crates/eevideo-control/src/register_map.rs
- [[stream_prefixes_collect_unique_names()]] - code - crates/eevideo-control/src/register_map.rs
- [[summary_from_discovered()]] - code - crates/eevideo-control/src/controller.rs
- [[unknown_register()]] - code - crates/eevideo-control/src/register_map.rs
- [[write_register_fields()]] - code - crates/eevideo-control/src/register_map.rs
- [[write_register_u32()]] - code - crates/eevideo-control/src/register_map.rs

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/Device_Controller
SORT file.name ASC
```

## Connections to other communities
- 11 edges to [[_COMMUNITY_CoAP Registers]]

## Top bridge nodes
- [[register_map.rs]] - degree 21, connects to 1 community
- [[write_register_fields()]] - degree 12, connects to 1 community
- [[.client_and_device()]] - degree 10, connects to 1 community
- [[read_register_field()]] - degree 8, connects to 1 community
- [[write_register_u32()]] - degree 6, connects to 1 community
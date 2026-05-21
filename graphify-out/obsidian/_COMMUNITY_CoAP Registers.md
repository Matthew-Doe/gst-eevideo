---
type: community
cohesion: 0.10
members: 52
---

# CoAP Registers

**Cohesion:** 0.10 - loosely connected
**Members:** 52 nodes

## Members
- [[.capabilities()]] - code - crates/eevideo-control/src/backend.rs
- [[.config()]] - code - crates/eevideo-control/src/backend.rs
- [[.configure()]] - code - crates/eevideo-control/src/backend.rs
- [[.connect()]] - code - crates/eevideo-control/src/backend.rs
- [[.connect_endpoint()]] - code - crates/eevideo-control/src/backend.rs
- [[.default()_7]] - code - crates/eevideo-control/src/backend.rs
- [[.describe()]] - code - crates/eevideo-control/src/backend.rs
- [[.disconnect()]] - code - crates/eevideo-control/src/backend.rs
- [[.drop()_1]] - code - crates/eevideo-control/src/backend.rs
- [[.endpoint()]] - code - crates/eevideo-control/src/backend.rs
- [[.ensure_connected()]] - code - crates/eevideo-control/src/backend.rs
- [[.introspect_device()]] - code - crates/eevideo-control/src/backend.rs
- [[.load_or_create_device_config()]] - code - crates/eevideo-control/src/backend.rs
- [[.new()_5]] - code - crates/eevideo-control/src/backend.rs
- [[.register_client()]] - code - crates/eevideo-control/src/backend.rs
- [[.registers()_2]] - code - crates/eevideo-control/src/backend.rs
- [[.resolve_stream_prefix()]] - code - crates/eevideo-control/src/backend.rs
- [[.spawn()_2]] - code - crates/eevideo-control/src/backend.rs
- [[.start()_2]] - code - crates/eevideo-control/src/backend.rs
- [[.stop()_2]] - code - crates/eevideo-control/src/backend.rs
- [[.write_stream_configuration()]] - code - crates/eevideo-control/src/backend.rs
- [[CoapRegisterBackend]] - code - crates/eevideo-control/src/backend.rs
- [[CoapRegisterBackendConfig]] - code - crates/eevideo-control/src/backend.rs
- [[CoapRegisterConnection]] - code - crates/eevideo-control/src/backend.rs
- [[ConfiguredStream]] - code - crates/eevideo-control/src/backend.rs
- [[DeviceEndpoint]] - code - crates/eevideo-control/src/backend.rs
- [[FakeDevice]] - code - crates/eevideo-control/src/backend.rs
- [[FakeDeviceBehavior]] - code - crates/eevideo-control/src/backend.rs
- [[apply_format_registers()]] - code - crates/eevideo-control/src/backend.rs
- [[backend.rs]] - code - crates/eevideo-control/src/backend.rs
- [[build_registers()_1]] - code - crates/eevideo-control/src/backend.rs
- [[configure_reports_applied_value_mismatch()]] - code - crates/eevideo-control/src/backend.rs
- [[configures_starts_and_stops_stream_registers()]] - code - crates/eevideo-control/src/backend.rs
- [[connect_reports_timeout_when_device_never_responds()]] - code - crates/eevideo-control/src/backend.rs
- [[control_target()_1]] - code - crates/eevideo-control/src/backend.rs
- [[discovery_error()]] - code - crates/eevideo-control/src/backend.rs
- [[introspect_device_config()]] - code - crates/eevideo-control/src/backend.rs
- [[local_bind_addr()]] - code - crates/eevideo-control/src/backend.rs
- [[maybe_read_device_config()]] - code - crates/eevideo-control/src/backend.rs
- [[parse_device_endpoint()]] - code - crates/eevideo-control/src/backend.rs
- [[parses_device_uri_without_scheme()]] - code - crates/eevideo-control/src/backend.rs
- [[persist_device_config()]] - code - crates/eevideo-control/src/backend.rs
- [[pixel_format_from_device()]] - code - crates/eevideo-control/src/backend.rs
- [[read_stream_format()]] - code - crates/eevideo-control/src/backend.rs
- [[register_name()]] - code - crates/eevideo-control/src/register_map.rs
- [[request()]] - code - crates/eevideo-control/src/backend.rs
- [[resolve_destination_ip()]] - code - crates/eevideo-control/src/backend.rs
- [[sanitize_filename()]] - code - crates/eevideo-control/src/backend.rs
- [[start_requires_prior_configuration()]] - code - crates/eevideo-control/src/backend.rs
- [[unspecified_host()]] - code - crates/eevideo-control/src/backend.rs
- [[yaml_error()]] - code - crates/eevideo-control/src/backend.rs
- [[yaml_path()]] - code - crates/eevideo-control/src/backend.rs

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/CoAP_Registers
SORT file.name ASC
```

## Connections to other communities
- 11 edges to [[_COMMUNITY_Device Controller]]
- 3 edges to [[_COMMUNITY_YAML Config]]
- 2 edges to [[_COMMUNITY_Discovery]]

## Top bridge nodes
- [[parse_device_endpoint()]] - degree 6, connects to 2 communities
- [[register_name()]] - degree 8, connects to 1 community
- [[CoapRegisterBackend]] - degree 8, connects to 1 community
- [[.write_stream_configuration()]] - degree 8, connects to 1 community
- [[.start()_2]] - degree 8, connects to 1 community
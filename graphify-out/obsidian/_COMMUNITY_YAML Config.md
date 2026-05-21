---
type: community
cohesion: 0.13
members: 20
---

# YAML Config

**Cohesion:** 0.13 - loosely connected
**Members:** 20 nodes

## Members
- [[.fmt()_4]] - code - crates/eevideo-control/src/yaml.rs
- [[DeviceCapabilities]] - code - crates/eevideo-control/src/yaml.rs
- [[DeviceConfig]] - code - crates/eevideo-control/src/yaml.rs
- [[DeviceLocation]] - code - crates/eevideo-control/src/yaml.rs
- [[DeviceMemoryMap]] - code - crates/eevideo-control/src/yaml.rs
- [[DeviceRegisterValue]] - code - crates/eevideo-control/src/yaml.rs
- [[FeatureDefinition]] - code - crates/eevideo-control/src/yaml.rs
- [[FeatureFieldDefinition]] - code - crates/eevideo-control/src/yaml.rs
- [[FeaturePointerDefinition]] - code - crates/eevideo-control/src/yaml.rs
- [[FeatureRegisterDefinition]] - code - crates/eevideo-control/src/yaml.rs
- [[YamlError]] - code - crates/eevideo-control/src/yaml.rs
- [[device_config_round_trips_to_yaml()]] - code - crates/eevideo-control/src/yaml.rs
- [[device_config_to_string()]] - code - crates/eevideo-control/src/yaml.rs
- [[embedded_feature_catalog_contains_stream_definition()]] - code - crates/eevideo-control/src/yaml.rs
- [[load_embedded_feature_catalog()]] - code - crates/eevideo-control/src/yaml.rs
- [[parse_feature_catalog()]] - code - crates/eevideo-control/src/yaml.rs
- [[read_device_config()]] - code - crates/eevideo-control/src/yaml.rs
- [[read_device_config_accepts_single_yaml_in_directory()]] - code - crates/eevideo-control/src/yaml.rs
- [[write_device_config()]] - code - crates/eevideo-control/src/yaml.rs
- [[yaml.rs]] - code - crates/eevideo-control/src/yaml.rs

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/YAML_Config
SORT file.name ASC
```

## Connections to other communities
- 3 edges to [[_COMMUNITY_CoAP Registers]]

## Top bridge nodes
- [[load_embedded_feature_catalog()]] - degree 4, connects to 1 community
- [[read_device_config()]] - degree 3, connects to 1 community
- [[write_device_config()]] - degree 3, connects to 1 community
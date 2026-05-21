# Graph Report - .  (2026-05-21)

## Corpus Check
- Corpus is ~41,772 words - fits in a single context window. You may not need a graph.

## Summary
- 769 nodes · 1322 edges · 45 communities (35 shown, 10 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 47 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Device Runtime|Device Runtime]]
- [[_COMMUNITY_Control Backend|Control Backend]]
- [[_COMMUNITY_Source Element|Source Element]]
- [[_COMMUNITY_CoAP Registers|CoAP Registers]]
- [[_COMMUNITY_Device Controller|Device Controller]]
- [[_COMMUNITY_Docs And CI|Docs And CI]]
- [[_COMMUNITY_Provider Backend|Provider Backend]]
- [[_COMMUNITY_Compat Packets|Compat Packets]]
- [[_COMMUNITY_Sink Element|Sink Element]]
- [[_COMMUNITY_Viewer CLI|Viewer CLI]]
- [[_COMMUNITY_Frame Assembly|Frame Assembly]]
- [[_COMMUNITY_Control CLI|Control CLI]]
- [[_COMMUNITY_Register Client|Register Client]]
- [[_COMMUNITY_Fake Device|Fake Device]]
- [[_COMMUNITY_GStreamer Capture|GStreamer Capture]]
- [[_COMMUNITY_YAML Config|YAML Config]]
- [[_COMMUNITY_CoAP Codec|CoAP Codec]]
- [[_COMMUNITY_Discovery|Discovery]]
- [[_COMMUNITY_Pipeline Builder|Pipeline Builder]]
- [[_COMMUNITY_Pixel Formats|Pixel Formats]]
- [[_COMMUNITY_Stream Stats|Stream Stats]]
- [[_COMMUNITY_Cross Build Env|Cross Build Env]]
- [[_COMMUNITY_Launch Smoke Tests|Launch Smoke Tests]]
- [[_COMMUNITY_Loss Reorder Tests|Loss Reorder Tests]]
- [[_COMMUNITY_Managed Control Tests|Managed Control Tests]]
- [[_COMMUNITY_Profile Identity|Profile Identity]]
- [[_COMMUNITY_Command Entrypoints|Command Entrypoints]]
- [[_COMMUNITY_Plugin Registration|Plugin Registration]]
- [[_COMMUNITY_Multicast Loopback|Multicast Loopback]]
- [[_COMMUNITY_Launch Script|Launch Script]]
- [[_COMMUNITY_Throughput Tests|Throughput Tests]]
- [[_COMMUNITY_Windows Runner|Windows Runner]]
- [[_COMMUNITY_Format Rejection|Format Rejection]]
- [[_COMMUNITY_Video Frame|Video Frame]]
- [[_COMMUNITY_Video Frame Ref|Video Frame Ref]]
- [[_COMMUNITY_Pixel Format Conversion|Pixel Format Conversion]]

## God Nodes (most connected - your core abstractions)
1. `run()` - 27 edges
2. `EeVideoSrc` - 17 edges
3. `EeVideoSink` - 16 edges
4. `ReceiveDiagnostics` - 15 edges
5. `DeviceController` - 14 edges
6. `ControlSession` - 14 edges
7. `configures_starts_and_stops_stream_registers()` - 13 edges
8. `write_register_fields()` - 12 edges
9. `CoapRegisterConnection` - 11 edges
10. `EEVideo Stream Compatibility Profile v1` - 11 edges

## Surprising Connections (you probably didn't know these)
- `EEVideo Stream Compatibility Profile v1` --semantically_similar_to--> `EEVideo Stream Compatibility Profile v1`  [INFERRED] [semantically similar]
  README.md → docs/compatibility-stream-profile.md
- `MIPI Port Feature` --conceptually_related_to--> `pipeline Provider`  [AMBIGUOUS]
  crates/eevideo-control/yaml/EEVideo_Features.yaml → docs/eedeviced-provider-guide.md
- `gst-integration CI Job` --conceptually_related_to--> `eevideosrc`  [INFERRED]
  .github/workflows/ci.yml → README.md
- `gst-integration CI Job` --conceptually_related_to--> `eevideosink`  [INFERRED]
  .github/workflows/ci.yml → README.md
- `Ethernet Interface Feature` --shares_data_with--> `eevid`  [INFERRED]
  crates/eevideo-control/yaml/EEVideo_Features.yaml → README.md

## Hyperedges (group relationships)
- **Compatibility Transport Contract** — compat_profile_v1, compat_leader_payload_trailer, compat_fixed_stream_parameters, compat_frame_assembler_rules, compat_pixel_format_registry [EXTRACTED 1.00]
- **Device Bring-Up Provider Pattern** — readme_eedeviced, provider_synthetic, provider_v4l2, provider_pipeline, linux_setup, jetson_orin_setup, jetson_nano_setup [EXTRACTED 1.00]
- **CI Selective Validation Flow** — ci_changes_job, ci_linux_fast, ci_gst_integration, ci_jetson_cross [EXTRACTED 1.00]

## Communities (45 total, 10 thin omitted)

### Community 0 - "Device Runtime"
Cohesion: 0.07
Nodes (40): advertised_ip(), build_discovery_response(), build_registers(), build_strings(), CaptureBackend, CaptureConfiguration, DeviceRuntime, DeviceRuntimeConfig (+32 more)

### Community 1 - "Control Backend"
Cohesion: 0.07
Nodes (24): AdvertisedStream, AdvertisedStreamMode, AppliedStreamConfiguration, ControlBackend, ControlCapabilities, ControlConnection, ControlError, ControlErrorKind (+16 more)

### Community 2 - "Source Element"
Cohesion: 0.06
Nodes (17): build_stream_configuration(), builds_managed_control_request_from_bound_address(), builds_managed_control_request_with_custom_transport_settings(), create_receiver_socket(), EeVideoSrc, frame_matches_expected_format(), init_gst(), parse_multicast_group() (+9 more)

### Community 3 - "CoAP Registers"
Cohesion: 0.10
Nodes (30): apply_format_registers(), build_registers(), CoapRegisterBackend, CoapRegisterBackendConfig, CoapRegisterConnection, configure_reports_applied_value_mismatch(), ConfiguredStream, configures_starts_and_stops_stream_registers() (+22 more)

### Community 4 - "Device Controller"
Cohesion: 0.09
Nodes (25): describe_reads_live_stream_mode_when_yaml_cache_exists(), DeviceController, DeviceDescription, DeviceSummary, maybe_read_stream_field(), pixel_format_from_device_bits(), read_advertised_stream_mode(), read_advertised_streams() (+17 more)

### Community 5 - "Docs And CI"
Cohesion: 0.07
Nodes (45): CI Changed Path Classifier, gst-integration CI Job, jetson-cross CI Job, linux-fast CI Job, GitHub Actions CI Workflow, Control-Plane Boundary, Fixed Stream Parameters, FrameAssembler Receiver Conformance Rules (+37 more)

### Community 6 - "Provider Backend"
Cohesion: 0.09
Nodes (23): build_capture_backend(), ProviderBackend, ProviderConfig, validate_provider_config(), argus_pipeline_description_uses_expected_elements(), argus_rejects_non_uyvy_formats(), cli_maps_provider_specific_options(), CliPixelFormat (+15 more)

### Community 7 - "Compat Packets"
Cohesion: 0.10
Nodes (21): borrowed_payload_parser_matches_owned_parser(), CompatPacket, CompatPacketEmitError, CompatPacketEmitError<E>, CompatPacketError, CompatPacketizer, CompatPacketView, CompatPacketView<'a> (+13 more)

### Community 8 - "Sink Element"
Cohesion: 0.07
Nodes (11): build_stream_configuration(), EeVideoSink, parse_multicast_iface(), RunningState, Settings, to_stream_format_descriptor(), ManagedControlSettings, FrameFormat (+3 more)

### Community 9 - "Viewer CLI"
Cohesion: 0.08
Nodes (13): advertised_stream_mode(), advertised_stream_overlay_text(), EncoderKind, EncoderSpec, finalize_run_result(), finalize_run_result_keeps_primary_error_and_stop_error(), format_source_anomaly_breakdown(), format_source_stats() (+5 more)

### Community 10 - "Frame Assembly"
Cohesion: 0.15
Nodes (20): append_payload_bytes(), AssembleError, assembles_a_complete_frame(), assembles_frame_with_reordered_payloads_and_early_trailer(), buffered_payloads_overflow(), build_partial_frame(), drops_frame_when_buffered_reordered_payloads_exceed_remaining_capacity(), drops_short_frame_when_trailer_closes_packet_range() (+12 more)

### Community 11 - "Control CLI"
Cohesion: 0.10
Nodes (18): build_stream_request(), Command, controller(), describe_command_works_against_fake_device(), FieldReadArgs, FieldWriteArgs, format_applied_stream(), format_bus_error() (+10 more)

### Community 12 - "Register Client"
Cohesion: 0.15
Nodes (12): build_token(), read_named_register_uses_device_yaml_mapping(), read_u32_retries_after_initial_timeout(), read_u32_round_trips_with_udp_responder(), register_access_option_matches_upstream_packing(), RegisterAccess, RegisterClient, RegisterError (+4 more)

### Community 13 - "Fake Device"
Cohesion: 0.12
Nodes (7): Cli, enable_bit_transitions_start_and_stop_counts(), FakeDeviceConfig, FakeDeviceServer, print_runtime_events(), wait_until(), write_u32_eventually()

### Community 14 - "GStreamer Capture"
Cohesion: 0.17
Nodes (18): build_argus_pipeline_description(), build_pipeline_description(), build_v4l2_pipeline_description(), capture_format_from_caps(), ensure_gstreamer_init(), ensure_gstreamer_init_for_tests(), format_from_sample(), GstreamerCaptureBackend (+10 more)

### Community 15 - "YAML Config"
Cohesion: 0.13
Nodes (18): device_config_round_trips_to_yaml(), device_config_to_string(), DeviceCapabilities, DeviceConfig, DeviceLocation, DeviceMemoryMap, DeviceRegisterValue, embedded_feature_catalog_contains_stream_definition() (+10 more)

### Community 16 - "CoAP Codec"
Cohesion: 0.20
Nodes (9): coap_round_trip_preserves_extended_options(), CoapError, CoapMessage, CoapMessageType, CoapOption, decode_extended(), encode_extended(), encode_rejects_descending_options() (+1 more)

### Community 17 - "Discovery"
Cohesion: 0.19
Nodes (11): build_discovery_request(), discover_devices(), discovery_request_matches_upstream_bytes(), DiscoveryAdvertisement, DiscoveryError, DiscoveryInterface, DiscoveryLink, DiscoveryResponse (+3 more)

### Community 18 - "Pipeline Builder"
Cohesion: 0.25
Nodes (14): add_display_branch(), build_pipeline(), build_pipeline_keeps_fps_overlay_when_mode_text_is_unavailable(), build_pipeline_keeps_record_branch_overlay_free(), build_pipeline_omits_overlay_when_disabled(), build_pipeline_uses_overlay_elements_by_default(), default_control_backend(), ensure_elements_available() (+6 more)

### Community 21 - "Cross Build Env"
Cohesion: 0.22
Nodes (8): CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER, CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS, CC_aarch64_unknown_linux_gnu, PKG_CONFIG_ALLOW_CROSS, PKG_CONFIG_DIR, PKG_CONFIG_LIBDIR, PKG_CONFIG_PATH, PKG_CONFIG_SYSROOT_DIR

### Community 22 - "Launch Smoke Tests"
Cohesion: 0.22
Nodes (4): EEVIDEO_DEVICE, EEVIDEO_INPUT, EEVIDEO_PIPELINE, EEVIDEO_SENSOR_ID

### Community 23 - "Loss Reorder Tests"
Cohesion: 0.42
Nodes (6): drop_random_payload(), FrameDisposition, Lcg, reorder_tail(), source_handles_reordered_frames_and_drops_gapped_frames(), wait_for_terminal_counts()

### Community 24 - "Managed Control Tests"
Cohesion: 0.46
Nodes (6): backend(), control_target(), format_error_message(), source_managed_control_starts_remote_stream_and_stops_cleanly(), source_rejects_frames_that_do_not_match_applied_control_format(), wait_for_frames()

### Community 28 - "Multicast Loopback"
Cohesion: 0.70
Nodes (4): build_receiver_pipeline(), build_sender_pipeline(), sink_multicast_loopback_reaches_multiple_receivers(), source_multicast_loopback_reaches_multiple_receivers()

### Community 29 - "Launch Script"
Cohesion: 0.83
Nodes (3): build_command(), main(), require_env()

### Community 30 - "Throughput Tests"
Cohesion: 0.83
Nodes (3): build_receiver_pipeline(), build_sender_pipeline(), measure_uyvy_720p_profiles()

## Ambiguous Edges - Review These
- `pipeline Provider` → `MIPI Port Feature`  [AMBIGUOUS]
  crates/eevideo-control/yaml/EEVideo_Features.yaml · relation: conceptually_related_to

## Knowledge Gaps
- **86 isolated node(s):** `CC_aarch64_unknown_linux_gnu`, `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER`, `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS`, `PKG_CONFIG_ALLOW_CROSS`, `PKG_CONFIG_SYSROOT_DIR` (+81 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **10 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What is the exact relationship between `pipeline Provider` and `MIPI Port Feature`?**
  _Edge tagged AMBIGUOUS (relation: conceptually_related_to) - confidence is low._
- **Why does `default_control_backend()` connect `Pipeline Builder` to `Sink Element`, `Control Backend`?**
  _High betweenness centrality (0.085) - this node is a cross-community bridge._
- **Why does `run()` connect `Control CLI` to `Viewer CLI`, `Pipeline Builder`, `Fake Device`, `Provider Backend`?**
  _High betweenness centrality (0.047) - this node is a cross-community bridge._
- **Why does `ManagedControlSettings` connect `Sink Element` to `Source Element`?**
  _High betweenness centrality (0.042) - this node is a cross-community bridge._
- **What connects `CC_aarch64_unknown_linux_gnu`, `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER`, `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS` to the rest of the system?**
  _94 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Device Runtime` be split into smaller, more focused modules?**
  _Cohesion score 0.06775956284153005 - nodes in this community are weakly interconnected._
- **Should `Control Backend` be split into smaller, more focused modules?**
  _Cohesion score 0.07017543859649122 - nodes in this community are weakly interconnected._
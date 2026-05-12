#![cfg(feature = "gui")]

use std::path::PathBuf;
use std::time::Duration;

use eeview::gui::{OperatorConsoleState, RecordingForm};
use eeview::session::{
    build_viewer_pipeline, ManagedTransportSettings, RecordingConfig, RecordingEncoder,
    ViewerSessionConfig,
};
use gst::prelude::*;
use gsteevideo::eevideo_control::{default_control_backend, ControlTarget, ControlTransportKind};
use gstreamer as gst;

fn init_gst() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        gst::init().unwrap();
        gsteevideo::register_static().unwrap();
    });
}

fn target() -> ControlTarget {
    ControlTarget {
        device_uri: "coap://127.0.0.1:5683".to_string(),
        transport_kind: ControlTransportKind::CoapRegister,
        auth_scope: None,
    }
}

fn config() -> ViewerSessionConfig {
    ViewerSessionConfig {
        target: target(),
        bind_address: "127.0.0.1".to_string(),
        port: 5000,
        source_timeout: Duration::from_millis(2000),
        latency: Duration::ZERO,
        stream_name: "stream0".to_string(),
        managed_transport: ManagedTransportSettings::default(),
        recording: None,
        overlay_text: None,
    }
}

#[test]
fn operator_console_defaults_require_connection_before_start() {
    let state = OperatorConsoleState::default();

    assert_eq!(state.bind_address, "127.0.0.1");
    assert_eq!(state.stream_name, "stream0");
    assert_eq!(state.port, 5000);
    assert!(!state.can_start());
}

#[test]
fn recording_form_requires_path_when_enabled() {
    let mut form = RecordingForm::default();
    form.enabled = true;

    assert!(form.to_recording_config().is_err());

    form.path = PathBuf::from("capture.webm");
    form.encoder = Some(RecordingEncoder::Vp9);
    assert_eq!(
        form.to_recording_config().unwrap().path,
        PathBuf::from("capture.webm")
    );
    assert_eq!(
        form.to_recording_config().unwrap().encoder,
        Some(RecordingEncoder::Vp9)
    );
}

#[test]
fn gui_pipeline_uses_rgba_appsink_display_branch() {
    init_gst();
    let pipeline = build_viewer_pipeline(&config(), default_control_backend()).unwrap();

    let sink = pipeline.by_name("display-appsink").unwrap();
    assert_eq!(sink.factory().unwrap().name(), "appsink");
    let caps = sink.property::<gst::Caps>("caps");
    let structure = caps.structure(0).unwrap();
    assert_eq!(structure.name(), "video/x-raw");
    assert_eq!(structure.get::<&str>("format").unwrap(), "RGBA");
    assert!(pipeline.by_name("record-queue").is_none());
}

#[test]
fn gui_pipeline_adds_record_branch_only_when_recording_is_configured() {
    init_gst();
    let mut cfg = config();
    cfg.recording = Some(RecordingConfig {
        path: PathBuf::from("capture.webm"),
        encoder: None,
    });

    let pipeline = build_viewer_pipeline(&cfg, default_control_backend()).unwrap();

    assert!(pipeline.by_name("display-appsink").is_some());
    assert!(pipeline.by_name("record-queue").is_some());
    assert!(pipeline.by_name("record-file-sink").is_some());
}

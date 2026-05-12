#![cfg(feature = "gui")]

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use eefakedev::{FakeDeviceConfig, FakeDeviceServer};
use eevideo_proto::PixelFormat;
use eeview::session::{ManagedTransportSettings, ViewerEvent, ViewerPipeline, ViewerSessionConfig};
use gsteevideo::eevideo_control::{
    CoapRegisterBackend, CoapRegisterBackendConfig, ControlTarget, ControlTransportKind,
};

const TEST_PORT: u32 = 5622;

#[test]
fn viewer_pipeline_receives_frame_and_stops_fake_device() {
    gstreamer::init().unwrap();
    gsteevideo::register_static().unwrap();

    let device = FakeDeviceServer::spawn(FakeDeviceConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        width: 32,
        height: 16,
        pixel_format: PixelFormat::Uyvy,
        ..FakeDeviceConfig::default()
    })
    .unwrap();

    let mut pipeline = ViewerPipeline::start(
        ViewerSessionConfig {
            target: ControlTarget {
                device_uri: device.uri(),
                transport_kind: ControlTransportKind::CoapRegister,
                auth_scope: None,
            },
            bind_address: "127.0.0.1".to_string(),
            port: TEST_PORT,
            source_timeout: Duration::from_millis(250),
            latency: Duration::ZERO,
            stream_name: "stream0".to_string(),
            managed_transport: ManagedTransportSettings::default(),
            recording: None,
            overlay_text: None,
        },
        Arc::new(CoapRegisterBackend::new(CoapRegisterBackendConfig {
            interface_name: None,
            bind_address: Some("127.0.0.1".to_string()),
            discovery_timeout: Duration::from_millis(250),
            request_timeout: Duration::from_millis(250),
            yaml_root: None,
            local_port: 0,
        })),
    )
    .unwrap();

    let events = pipeline.take_events().unwrap();
    let received_frame = wait_for_frame(&events, Duration::from_secs(3));
    let stats = pipeline.stop().unwrap();

    assert!(received_frame, "expected at least one GUI frame event");
    assert!(stats.frames_received >= 1);
    assert_eq!(device.start_count(), 1);
    assert!(
        wait_for_stop_count(&device, Duration::from_secs(2)) >= 1,
        "expected stream stop on shutdown"
    );
}

fn wait_for_frame(events: &std::sync::mpsc::Receiver<ViewerEvent>, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(event) = events.recv_timeout(Duration::from_millis(100)) {
            if matches!(event, ViewerEvent::Frame(_)) {
                return true;
            }
        }
    }
    false
}

fn wait_for_stop_count(device: &FakeDeviceServer, timeout: Duration) -> usize {
    let deadline = Instant::now() + timeout;
    loop {
        let count = device.stop_count();
        if count > 0 || Instant::now() >= deadline {
            return count;
        }
        thread::sleep(Duration::from_millis(25));
    }
}

#![cfg(feature = "gst-tests")]

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use eefakedev::{FakeDeviceConfig, FakeDeviceServer};
use eevideo_control::{
    CoapRegisterBackend, CoapRegisterBackendConfig, ControlTarget, ControlTransportKind,
    SharedControlBackend,
};
use eevideo_proto::PixelFormat;
use gst::prelude::*;
use gstreamer as gst;

const TEST_PORT: u32 = 5612;
const FRAME_WIDTH: u32 = 32;
const FRAME_HEIGHT: u32 = 16;

#[test]
fn source_managed_control_starts_remote_stream_and_stops_cleanly() {
    gst::init().unwrap();
    gsteevideo::register_static().unwrap();

    let device = FakeDeviceServer::spawn(FakeDeviceConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        width: FRAME_WIDTH,
        height: FRAME_HEIGHT,
        pixel_format: PixelFormat::Mono8,
        ..FakeDeviceConfig::default()
    })
    .unwrap();

    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("eevideosrc")
        .property("address", "127.0.0.1")
        .property("port", TEST_PORT)
        .property("timeout-ms", 150u64)
        .build()
        .unwrap();
    let sink = gst::ElementFactory::make("fakesink")
        .property("sync", false)
        .build()
        .unwrap();

    gsteevideo::configure_source_control_for_tests(
        &src,
        backend(),
        control_target(device.uri()),
        "stream0",
    )
    .unwrap();

    pipeline.add_many([&src, &sink]).unwrap();
    gst::Element::link_many([&src, &sink]).unwrap();
    pipeline.set_state(gst::State::Playing).unwrap();

    let received = wait_for_frames(&src, 1, Duration::from_secs(3));

    pipeline.set_state(gst::State::Null).unwrap();

    assert!(received >= 1, "expected at least one managed-control frame");
    assert_eq!(device.start_count(), 1);
    assert!(
        wait_for_stop_count(&device, Duration::from_secs(2)) >= 1,
        "expected stream stop on shutdown"
    );
}

#[test]
fn source_rejects_frames_that_do_not_match_applied_control_format() {
    gst::init().unwrap();
    gsteevideo::register_static().unwrap();

    let device = FakeDeviceServer::spawn(FakeDeviceConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        width: FRAME_WIDTH,
        height: FRAME_HEIGHT,
        pixel_format: PixelFormat::Mono8,
        transmit_pixel_format: Some(PixelFormat::Rgb8),
        ..FakeDeviceConfig::default()
    })
    .unwrap();

    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("eevideosrc")
        .property("address", "127.0.0.1")
        .property("port", TEST_PORT + 1)
        .property("timeout-ms", 150u64)
        .build()
        .unwrap();
    let sink = gst::ElementFactory::make("fakesink")
        .property("sync", false)
        .build()
        .unwrap();

    gsteevideo::configure_source_control_for_tests(
        &src,
        backend(),
        control_target(device.uri()),
        "stream0",
    )
    .unwrap();

    pipeline.add_many([&src, &sink]).unwrap();
    gst::Element::link_many([&src, &sink]).unwrap();
    pipeline.set_state(gst::State::Playing).unwrap();

    let bus = pipeline.bus().unwrap();
    let message =
        bus.timed_pop_filtered(gst::ClockTime::from_seconds(3), &[gst::MessageType::Error]);

    pipeline.set_state(gst::State::Null).unwrap();

    assert!(
        message.is_some(),
        "expected managed-control format mismatch to post an error"
    );
    let frames_received: u64 = src.property("frames-received");
    let packet_anomalies: u64 = src.property("packet-anomalies");
    assert_eq!(
        frames_received, 1,
        "the source should reject the first completed frame after detecting the format mismatch"
    );
    assert!(packet_anomalies >= 1);
}

fn backend() -> SharedControlBackend {
    Arc::new(CoapRegisterBackend::new(CoapRegisterBackendConfig {
        request_timeout: Duration::from_millis(250),
        ..CoapRegisterBackendConfig::default()
    }))
}

fn control_target(device_uri: String) -> ControlTarget {
    ControlTarget {
        device_uri,
        transport_kind: ControlTransportKind::CoapRegister,
        auth_scope: None,
    }
}

fn wait_for_frames(src: &gst::Element, minimum: u64, timeout: Duration) -> u64 {
    let deadline = Instant::now() + timeout;
    loop {
        let frames_received: u64 = src.property("frames-received");
        if frames_received >= minimum || Instant::now() >= deadline {
            return frames_received;
        }
        thread::sleep(Duration::from_millis(25));
    }
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

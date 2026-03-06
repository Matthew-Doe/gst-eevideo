#![cfg(feature = "gst-tests")]

use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

use eevideo_proto::{CompatPacketizer, PayloadType, PixelFormat, VideoFrame};
use gst::prelude::*;
use gstreamer as gst;

const MULTICAST_GROUP: &str = "239.255.10.10";
const MULTICAST_PORT: u32 = 5602;

#[test]
fn source_multicast_loopback_reaches_multiple_receivers() {
    gst::init().unwrap();
    gsteevideo::register_static().unwrap();

    let (pipeline_a, src_a) = build_receiver_pipeline();
    let (pipeline_b, src_b) = build_receiver_pipeline();

    pipeline_a.set_state(gst::State::Playing).unwrap();
    pipeline_b.set_state(gst::State::Playing).unwrap();

    thread::spawn(|| {
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.set_multicast_loop_v4(true).unwrap();
        socket.set_multicast_ttl_v4(1).unwrap();

        let frame = VideoFrame {
            frame_id: 1,
            timestamp: 1,
            width: 32,
            height: 32,
            pixel_format: PixelFormat::Mono8,
            payload_type: PayloadType::Image,
            data: vec![0x2a; 32 * 32],
        };

        let packetizer = CompatPacketizer::new(512).unwrap();
        for packet in packetizer.packetize(&frame).unwrap() {
            socket
                .send_to(&packet, (MULTICAST_GROUP, MULTICAST_PORT as u16))
                .unwrap();
        }
    });

    thread::sleep(Duration::from_millis(1200));

    let frames_a: u64 = src_a.property("frames-received");
    let frames_b: u64 = src_b.property("frames-received");

    pipeline_a.set_state(gst::State::Null).unwrap();
    pipeline_b.set_state(gst::State::Null).unwrap();

    assert!(
        frames_a > 0,
        "expected first receiver to get at least one frame"
    );
    assert!(
        frames_b > 0,
        "expected second receiver to get at least one frame"
    );
}

#[test]
fn sink_multicast_loopback_reaches_multiple_receivers() {
    gst::init().unwrap();
    gsteevideo::register_static().unwrap();

    let (pipeline_a, src_a) = build_receiver_pipeline();
    let (pipeline_b, src_b) = build_receiver_pipeline();
    let sender = build_sender_pipeline();

    pipeline_a.set_state(gst::State::Playing).unwrap();
    pipeline_b.set_state(gst::State::Playing).unwrap();
    thread::sleep(Duration::from_millis(200));

    sender.set_state(gst::State::Playing).unwrap();
    let bus = sender.bus().unwrap();
    let message = bus.timed_pop_filtered(
        gst::ClockTime::from_seconds(3),
        &[gst::MessageType::Eos, gst::MessageType::Error],
    );
    sender.set_state(gst::State::Null).unwrap();

    if let Some(message) = message {
        if let gst::MessageView::Error(error) = message.view() {
            panic!(
                "sender pipeline failed: {} ({:?})",
                error.error(),
                error.debug()
            );
        }
    } else {
        panic!("sender pipeline timed out before EOS");
    }

    thread::sleep(Duration::from_millis(300));

    let frames_a: u64 = src_a.property("frames-received");
    let frames_b: u64 = src_b.property("frames-received");

    pipeline_a.set_state(gst::State::Null).unwrap();
    pipeline_b.set_state(gst::State::Null).unwrap();

    assert!(
        frames_a > 0,
        "expected first receiver to get at least one frame from eevideosink"
    );
    assert!(
        frames_b > 0,
        "expected second receiver to get at least one frame from eevideosink"
    );
}

fn build_receiver_pipeline() -> (gst::Pipeline, gst::Element) {
    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("eevideosrc")
        .property("address", "0.0.0.0")
        .property("port", MULTICAST_PORT)
        .property("multicast-group", MULTICAST_GROUP)
        .property("timeout-ms", 500u64)
        .build()
        .unwrap();
    let sink = gst::ElementFactory::make("fakesink").build().unwrap();

    pipeline.add_many([&src, &sink]).unwrap();
    gst::Element::link_many([&src, &sink]).unwrap();

    (pipeline, src)
}

fn build_sender_pipeline() -> gst::Pipeline {
    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("videotestsrc")
        .property("num-buffers", 1i32)
        .build()
        .unwrap();
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            "video/x-raw,format=UYVY,width=32,height=32,framerate=1/1"
                .parse::<gst::Caps>()
                .unwrap(),
        )
        .build()
        .unwrap();
    let sink = gst::ElementFactory::make("eevideosink")
        .property("host", MULTICAST_GROUP)
        .property("port", MULTICAST_PORT)
        .property("multicast-loop", true)
        .property("multicast-ttl", 1u32)
        .build()
        .unwrap();

    pipeline.add_many([&src, &capsfilter, &sink]).unwrap();
    gst::Element::link_many([&src, &capsfilter, &sink]).unwrap();

    pipeline
}

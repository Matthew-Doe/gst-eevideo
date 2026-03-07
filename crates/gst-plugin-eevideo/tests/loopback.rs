#![cfg(feature = "gst-tests")]

use std::thread;
use std::time::Duration;

use eevideo_proto::{CompatPacketizer, PayloadType, PixelFormat, VideoFrame};
use gst::prelude::*;
use gstreamer as gst;

mod support;

#[test]
fn source_receives_udp_packets_and_exposes_frames() {
    gst::init().unwrap();
    gsteevideo::register_static().unwrap();

    let (reservation, port) = support::reserve_udp_port("127.0.0.1");
    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("eevideosrc")
        .property("address", "127.0.0.1")
        .property("port", port)
        .property("timeout-ms", 500u64)
        .build()
        .unwrap();
    let fakesink = gst::ElementFactory::make("fakesink").build().unwrap();

    pipeline.add_many([&src, &fakesink]).unwrap();
    gst::Element::link_many([&src, &fakesink]).unwrap();

    drop(reservation);
    pipeline.set_state(gst::State::Playing).unwrap();
    thread::spawn(move || {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.connect(("127.0.0.1", port as u16)).unwrap();

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
            socket.send(&packet).unwrap();
        }
    });

    thread::sleep(Duration::from_millis(1200));

    let frames: u64 = src.property("frames-received");
    pipeline.set_state(gst::State::Null).unwrap();

    assert!(frames > 0, "expected at least one received frame");
}

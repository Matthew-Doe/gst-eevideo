#![cfg(feature = "gst-tests")]

use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

use eevideo_proto::{CompatPacketizer, PayloadType, PixelFormat, VideoFrame};
use gst::prelude::*;
use gstreamer as gst;

mod support;

#[test]
fn source_rejects_mid_stream_format_change() {
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
    let sink = gst::ElementFactory::make("fakesink").build().unwrap();

    pipeline.add_many([&src, &sink]).unwrap();
    gst::Element::link_many([&src, &sink]).unwrap();
    drop(reservation);
    pipeline.set_state(gst::State::Playing).unwrap();

    thread::spawn(move || {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.connect(("127.0.0.1", port as u16)).unwrap();
        let packetizer = CompatPacketizer::new(512).unwrap();

        for frame in [
            VideoFrame {
                frame_id: 1,
                timestamp: 1,
                width: 4,
                height: 4,
                pixel_format: PixelFormat::Mono8,
                payload_type: PayloadType::Image,
                data: vec![0x10; 16],
            },
            VideoFrame {
                frame_id: 2,
                timestamp: 2,
                width: 4,
                height: 4,
                pixel_format: PixelFormat::Rgb8,
                payload_type: PayloadType::Image,
                data: vec![0x20; 48],
            },
        ] {
            for packet in packetizer.packetize(&frame).unwrap() {
                socket.send(&packet).unwrap();
            }
            thread::sleep(Duration::from_millis(50));
        }
    });

    let bus = pipeline.bus().unwrap();
    let msg = bus.timed_pop_filtered(gst::ClockTime::from_seconds(2), &[gst::MessageType::Error]);

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(msg.is_some(), "expected format-change error on the bus");
}

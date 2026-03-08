#![cfg(feature = "gst-tests")]

use std::thread;
use std::time::{Duration, Instant};

use gst::prelude::*;
use gstreamer as gst;

mod support;

#[test]
#[ignore = "manual throughput measurement harness"]
fn measure_uyvy_720p_profiles() {
    gst::init().unwrap();
    gsteevideo::register_static().unwrap();

    for fps in [30i32, 60i32] {
        let (reservation, port) = support::reserve_udp_port("127.0.0.1");
        let (receiver, src) = build_receiver_pipeline(port);
        let sender = build_sender_pipeline(port, fps);

        drop(reservation);
        receiver.set_state(gst::State::Playing).unwrap();
        thread::sleep(Duration::from_millis(200));

        let start = Instant::now();
        sender.set_state(gst::State::Playing).unwrap();
        let bus = sender.bus().unwrap();
        let message = bus.timed_pop_filtered(
            gst::ClockTime::from_seconds(15),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        );
        let elapsed = start.elapsed();

        sender.set_state(gst::State::Null).unwrap();

        match message {
            Some(message) => {
                if let gst::MessageView::Error(error) = message.view() {
                    panic!(
                        "sender pipeline failed for {} fps: {} ({:?})",
                        fps,
                        error.error(),
                        error.debug()
                    );
                }
            }
            None => panic!("sender pipeline timed out for {} fps", fps),
        }

        thread::sleep(Duration::from_millis(500));

        let frames_received: u64 = src.property("frames-received");
        let frames_dropped: u64 = src.property("frames-dropped");
        let packet_anomalies: u64 = src.property("packet-anomalies");
        let sent = sender
            .by_name("sink")
            .expect("eevideosink element")
            .property::<u64>("frames-sent");

        receiver.set_state(gst::State::Null).unwrap();

        println!(
            "throughput {} fps target: sent={} ({:.2} fps), received={} dropped={} anomalies={} elapsed={:.2}s",
            fps,
            sent,
            sent as f64 / elapsed.as_secs_f64(),
            frames_received,
            frames_dropped,
            packet_anomalies,
            elapsed.as_secs_f64()
        );

        assert!(
            sent > 0,
            "expected sender to transmit frames at {} fps",
            fps
        );
        assert!(
            frames_received + frames_dropped > 0,
            "expected receiver activity at {} fps",
            fps
        );
    }
}

fn build_receiver_pipeline(port: u32) -> (gst::Pipeline, gst::Element) {
    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("eevideosrc")
        .property("address", "127.0.0.1")
        .property("port", port)
        .property("timeout-ms", 250u64)
        .build()
        .unwrap();
    let sink = gst::ElementFactory::make("fakesink")
        .property("sync", false)
        .build()
        .unwrap();

    pipeline.add_many([&src, &sink]).unwrap();
    gst::Element::link_many([&src, &sink]).unwrap();

    (pipeline, src)
}

fn build_sender_pipeline(port: u32, fps: i32) -> gst::Pipeline {
    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("videotestsrc")
        .property("is-live", true)
        .property("num-buffers", 120i32)
        .build()
        .unwrap();
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            format!(
                "video/x-raw,format=UYVY,width=1280,height=720,framerate={}/1",
                fps
            )
            .parse::<gst::Caps>()
            .unwrap(),
        )
        .build()
        .unwrap();
    let sink = gst::ElementFactory::make("eevideosink")
        .name("sink")
        .property("host", "127.0.0.1")
        .property("port", port)
        .property("mtu", 1400u32)
        .build()
        .unwrap();

    pipeline.add_many([&src, &capsfilter, &sink]).unwrap();
    gst::Element::link_many([&src, &capsfilter, &sink]).unwrap();

    pipeline
}

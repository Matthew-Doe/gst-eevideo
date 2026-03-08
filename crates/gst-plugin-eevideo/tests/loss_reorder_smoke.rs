#![cfg(feature = "gst-tests")]

use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

use eevideo_proto::{CompatPacketizer, PayloadType, PixelFormat, VideoFrame};
use gst::prelude::*;
use gstreamer as gst;

mod support;

const FRAME_COUNT: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrameDisposition {
    InOrder,
    Reordered,
    Dropped,
}

#[test]
fn source_handles_reordered_frames_and_drops_gapped_frames() {
    gst::init().unwrap();
    gsteevideo::register_static().unwrap();

    let (reservation, test_port) = support::reserve_udp_port("127.0.0.1");
    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("eevideosrc")
        .property("address", "127.0.0.1")
        .property("port", test_port)
        .property("timeout-ms", 150u64)
        .build()
        .unwrap();
    let sink = gst::ElementFactory::make("fakesink")
        .property("sync", false)
        .build()
        .unwrap();

    pipeline.add_many([&src, &sink]).unwrap();
    gst::Element::link_many([&src, &sink]).unwrap();
    drop(reservation);
    pipeline.set_state(gst::State::Playing).unwrap();

    thread::sleep(Duration::from_millis(200));

    let mut rng = Lcg::new(0x5eed_cafe);
    let mut expected_received = 0u64;
    let mut expected_dropped = 0u64;

    let dispositions = (1..=FRAME_COUNT)
        .map(|frame_id| {
            let disposition = match frame_id {
                1 => FrameDisposition::Reordered,
                2 => FrameDisposition::Dropped,
                _ => match rng.next_u32() % 3 {
                    0 => FrameDisposition::InOrder,
                    1 => FrameDisposition::Reordered,
                    _ => FrameDisposition::Dropped,
                },
            };

            match disposition {
                FrameDisposition::Dropped => expected_dropped += 1,
                FrameDisposition::InOrder | FrameDisposition::Reordered => expected_received += 1,
            }

            disposition
        })
        .collect::<Vec<_>>();

    let sender = thread::spawn(move || {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.connect(("127.0.0.1", test_port as u16)).unwrap();

        let packetizer = CompatPacketizer::new(96).unwrap();
        let mut rng = Lcg::new(0x5eed_cafe);

        for (index, disposition) in dispositions.into_iter().enumerate() {
            let frame_id = index as u32 + 1;
            let frame = VideoFrame {
                frame_id,
                timestamp: frame_id as u64,
                width: 32,
                height: 16,
                pixel_format: PixelFormat::Mono8,
                payload_type: PayloadType::Image,
                data: vec![frame_id as u8; 32 * 16],
            };

            let mut packets = packetizer.packetize(&frame).unwrap();
            match disposition {
                FrameDisposition::InOrder => {}
                FrameDisposition::Reordered => reorder_tail(&mut packets, &mut rng),
                FrameDisposition::Dropped => {
                    drop_random_payload(&mut packets, &mut rng);
                    reorder_tail(&mut packets, &mut rng);
                }
            }

            for packet in packets {
                socket.send(&packet).unwrap();
                thread::sleep(Duration::from_millis(1));
            }

            thread::sleep(Duration::from_millis(15));
        }
    });

    sender.join().unwrap();
    let (frames_received, frames_dropped, packet_anomalies) =
        wait_for_terminal_counts(&src, FRAME_COUNT as u64, Duration::from_secs(3));

    pipeline.set_state(gst::State::Null).unwrap();

    assert_eq!(frames_received, expected_received);
    assert_eq!(frames_dropped, expected_dropped);
    assert!(
        packet_anomalies >= expected_dropped,
        "expected at least one anomaly per dropped frame, got {} anomalies for {} drops",
        packet_anomalies,
        expected_dropped
    );
}

fn wait_for_terminal_counts(
    src: &gst::Element,
    expected_total: u64,
    timeout: Duration,
) -> (u64, u64, u64) {
    let deadline = std::time::Instant::now() + timeout;

    loop {
        let frames_received: u64 = src.property("frames-received");
        let frames_dropped: u64 = src.property("frames-dropped");
        let packet_anomalies: u64 = src.property("packet-anomalies");

        if frames_received + frames_dropped >= expected_total
            || std::time::Instant::now() >= deadline
        {
            return (frames_received, frames_dropped, packet_anomalies);
        }

        thread::sleep(Duration::from_millis(25));
    }
}

fn reorder_tail(packets: &mut [Vec<u8>], rng: &mut Lcg) {
    if packets.len() <= 2 {
        return;
    }

    let tail = &mut packets[1..];
    for i in (1..tail.len()).rev() {
        let j = (rng.next_u32() as usize) % (i + 1);
        tail.swap(i, j);
    }
}

fn drop_random_payload(packets: &mut Vec<Vec<u8>>, rng: &mut Lcg) {
    if packets.len() <= 3 {
        return;
    }

    let payload_count = packets.len() - 2;
    let payload_index = 1 + (rng.next_u32() as usize % payload_count);
    packets.remove(payload_index);
}

#[derive(Clone, Debug)]
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 32) as u32
    }
}

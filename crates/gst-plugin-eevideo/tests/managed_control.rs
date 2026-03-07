#![cfg(feature = "gst-tests")]

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use eevideo_control::{
    CoapRegisterBackend, CoapRegisterBackendConfig, ControlTarget, ControlTransportKind,
    SharedControlBackend,
};
use eevideo_proto::{CompatPacketizer, PayloadType, PixelFormat, VideoFrame};
use gst::prelude::*;
use gstreamer as gst;

const TEST_PORT: u32 = 5612;
const FRAME_WIDTH: u32 = 32;
const FRAME_HEIGHT: u32 = 16;

const CAPABILITIES_ADDR: u32 = 0;
const FEATURE_TABLE_ADDR: u32 = 16;
const STREAM_DESC_ADDR: u32 = 0x0000_0100;
const STREAM_MAX_PACKET_ADDR: u32 = 0x0004_0000;
const STREAM_DELAY_ADDR: u32 = 0x0004_0004;
const STREAM_DEST_MAC_ADDR: u32 = 0x0004_0008;
const STREAM_DEST_IP_ADDR: u32 = 0x0004_0010;
const STREAM_DEST_PORT_ADDR: u32 = 0x0004_0014;
const STREAM_SOURCE_PORT_ADDR: u32 = 0x0004_0018;
const STREAM_WIDTH_ADDR: u32 = 0x0004_001c;
const STREAM_HEIGHT_ADDR: u32 = 0x0004_0020;
const STREAM_PIXEL_FORMAT_ADDR: u32 = 0x0004_0024;
const STREAM_ACQ_ADDR: u32 = 0x0004_0028;
const STREAM_X_OFFSET_ADDR: u32 = 0x0004_002c;
const STREAM_Y_OFFSET_ADDR: u32 = 0x0004_0030;
const STREAM_TEST_PATTERN_ADDR: u32 = 0x0004_0034;

#[test]
fn source_managed_control_starts_remote_stream_and_stops_cleanly() {
    gst::init().unwrap();
    gsteevideo::register_static().unwrap();

    let device = FakeManagedDevice::spawn(FakeDeviceBehavior::default());
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
    assert!(device.stop_count() >= 1, "expected stream stop on shutdown");
}

#[test]
fn source_rejects_frames_that_do_not_match_applied_control_format() {
    gst::init().unwrap();
    gsteevideo::register_static().unwrap();

    let device = FakeManagedDevice::spawn(FakeDeviceBehavior {
        advertised_format: PixelFormat::Mono8,
        sent_format: PixelFormat::Rgb8,
    });
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
    let message = bus.timed_pop_filtered(
        gst::ClockTime::from_seconds(3),
        &[gst::MessageType::Error],
    );

    pipeline.set_state(gst::State::Null).unwrap();

    assert!(message.is_some(), "expected managed-control format mismatch to post an error");
    let frames_received: u64 = src.property("frames-received");
    let packet_anomalies: u64 = src.property("packet-anomalies");
    assert_eq!(frames_received, 0);
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

#[derive(Clone, Copy, Debug)]
struct FakeDeviceBehavior {
    advertised_format: PixelFormat,
    sent_format: PixelFormat,
}

impl Default for FakeDeviceBehavior {
    fn default() -> Self {
        Self {
            advertised_format: PixelFormat::Mono8,
            sent_format: PixelFormat::Mono8,
        }
    }
}

struct FakeManagedDevice {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    start_count: Arc<AtomicUsize>,
    stop_count: Arc<AtomicUsize>,
    join: Option<JoinHandle<()>>,
}

impl FakeManagedDevice {
    fn spawn(behavior: FakeDeviceBehavior) -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        let addr = socket.local_addr().unwrap();
        let registers = Arc::new(Mutex::new(build_registers(behavior.advertised_format)));
        let strings = Arc::new(BTreeMap::from([(
            STREAM_DESC_ADDR,
            b"EEVideo Stream 0\0".to_vec(),
        )]));
        let stop = Arc::new(AtomicBool::new(false));
        let start_count = Arc::new(AtomicUsize::new(0));
        let stop_count = Arc::new(AtomicUsize::new(0));

        let thread_stop = Arc::clone(&stop);
        let thread_registers = Arc::clone(&registers);
        let thread_strings = Arc::clone(&strings);
        let thread_start_count = Arc::clone(&start_count);
        let thread_stop_count = Arc::clone(&stop_count);

        let join = thread::spawn(move || {
            let mut buffer = [0u8; 2048];
            while !thread_stop.load(Ordering::Relaxed) {
                let (size, peer) = match socket.recv_from(&mut buffer) {
                    Ok(value) => value,
                    Err(err)
                        if err.kind() == std::io::ErrorKind::WouldBlock
                            || err.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        continue;
                    }
                    Err(_) => break,
                };

                let request = match eevideo_control::CoapMessage::decode(&buffer[..size]) {
                    Ok(request) => request,
                    Err(_) => continue,
                };
                let address = request
                    .options
                    .iter()
                    .find(|option| option.number == eevideo_control::OPTION_EEV_BINARY_ADDRESS)
                    .map(|option| {
                        u32::from_be_bytes(option.value.clone().try_into().unwrap())
                    })
                    .unwrap();
                let reg_access = request
                    .options
                    .iter()
                    .find(|option| option.number == eevideo_control::OPTION_EEV_REG_ACCESS)
                    .and_then(|option| option.value.first().copied());

                let response = if request.code == eevideo_control::coap::CODE_GET {
                    let payload = if reg_access.map(|value| value >> 5)
                        == Some(eevideo_control::RegisterReadKind::String as u8)
                    {
                        thread_strings.get(&address).cloned().unwrap_or_default()
                    } else {
                        let value = thread_registers
                            .lock()
                            .unwrap()
                            .get(&address)
                            .copied()
                            .unwrap_or_default();
                        value.to_be_bytes().to_vec()
                    };
                    eevideo_control::CoapMessage::new(
                        eevideo_control::CoapMessageType::Acknowledgement,
                        eevideo_control::coap::CODE_CONTENT,
                        request.message_id,
                        request.token,
                        Vec::<eevideo_control::CoapOption>::new(),
                        payload,
                    )
                } else {
                    let mut registers = thread_registers.lock().unwrap();
                    let old_value = registers.get(&address).copied().unwrap_or_default();
                    let value = u32::from_be_bytes(request.payload.clone().try_into().unwrap());
                    registers.insert(address, value);

                    if address == STREAM_MAX_PACKET_ADDR {
                        let old_enabled = old_value & (1 << 16) != 0;
                        let enabled = value & (1 << 16) != 0;
                        if !old_enabled && enabled {
                            thread_start_count.fetch_add(1, Ordering::Relaxed);
                            let registers_snapshot = registers.clone();
                            thread::spawn(move || {
                                send_frame_burst(&registers_snapshot, behavior.sent_format);
                            });
                        } else if old_enabled && !enabled {
                            thread_stop_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    eevideo_control::CoapMessage::new(
                        eevideo_control::CoapMessageType::Acknowledgement,
                        eevideo_control::coap::CODE_CHANGED,
                        request.message_id,
                        request.token,
                        Vec::<eevideo_control::CoapOption>::new(),
                        Vec::new(),
                    )
                };

                let bytes = response.encode().unwrap();
                let _ = socket.send_to(&bytes, peer);
            }
        });

        Self {
            addr,
            stop,
            start_count,
            stop_count,
            join: Some(join),
        }
    }

    fn uri(&self) -> String {
        format!("coap://{}", self.addr)
    }

    fn start_count(&self) -> usize {
        self.start_count.load(Ordering::Relaxed)
    }

    fn stop_count(&self) -> usize {
        self.stop_count.load(Ordering::Relaxed)
    }
}

impl Drop for FakeManagedDevice {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = UdpSocket::bind("127.0.0.1:0")
            .and_then(|socket| socket.send_to(&[0], self.addr))
            .ok();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn send_frame_burst(registers: &BTreeMap<u32, u32>, sent_format: PixelFormat) {
    let dest_ip = Ipv4Addr::from(registers.get(&STREAM_DEST_IP_ADDR).copied().unwrap_or_default());
    let dest_port = registers.get(&STREAM_DEST_PORT_ADDR).copied().unwrap_or_default() as u16;
    if dest_port == 0 {
        return;
    }

    let mtu = (registers.get(&STREAM_MAX_PACKET_ADDR).copied().unwrap_or(1200) & 0xffff) as usize;
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.connect((dest_ip, dest_port)).unwrap();
    let packetizer = CompatPacketizer::new(mtu.max(256)).unwrap();
    let payload_len = sent_format
        .payload_len(FRAME_WIDTH, FRAME_HEIGHT)
        .expect("valid test frame");

    thread::sleep(Duration::from_millis(50));
    for frame_id in 1..=3u32 {
        let frame = VideoFrame {
            frame_id,
            timestamp: frame_id as u64,
            width: FRAME_WIDTH,
            height: FRAME_HEIGHT,
            pixel_format: sent_format,
            payload_type: PayloadType::Image,
            data: vec![frame_id as u8; payload_len],
        };
        for packet in packetizer.packetize(&frame).unwrap() {
            socket.send(&packet).unwrap();
            thread::sleep(Duration::from_millis(1));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn build_registers(advertised_format: PixelFormat) -> BTreeMap<u32, u32> {
    let mut registers = BTreeMap::new();
    registers.insert(CAPABILITIES_ADDR, 0xE71D_8FFF);
    registers.insert(FEATURE_TABLE_ADDR, 0x1030_010E);

    let pointers = [
        STREAM_DESC_ADDR,
        STREAM_MAX_PACKET_ADDR,
        STREAM_DELAY_ADDR,
        STREAM_DEST_MAC_ADDR,
        STREAM_DEST_IP_ADDR,
        STREAM_DEST_PORT_ADDR,
        STREAM_SOURCE_PORT_ADDR,
        STREAM_WIDTH_ADDR,
        STREAM_HEIGHT_ADDR,
        STREAM_PIXEL_FORMAT_ADDR,
        STREAM_ACQ_ADDR,
        STREAM_X_OFFSET_ADDR,
        STREAM_Y_OFFSET_ADDR,
        STREAM_TEST_PATTERN_ADDR,
    ];
    for (index, pointer) in pointers.into_iter().enumerate() {
        registers.insert(FEATURE_TABLE_ADDR + 4 + (index as u32 * 4), pointer);
    }

    let end_addr = FEATURE_TABLE_ADDR + 4 + (pointers.len() as u32 * 4);
    registers.insert(end_addr, 0xFFF0_0103);
    registers.insert(end_addr + 4, 0x0000_03FF);
    registers.insert(end_addr + 8, 0x0004_0000);
    registers.insert(end_addr + 12, 0x0004_FFFF);

    registers.insert(STREAM_MAX_PACKET_ADDR, 1200);
    registers.insert(STREAM_DELAY_ADDR, 0);
    registers.insert(STREAM_DEST_IP_ADDR, 0);
    registers.insert(STREAM_DEST_PORT_ADDR, 0);
    registers.insert(STREAM_SOURCE_PORT_ADDR, 0);
    registers.insert(STREAM_WIDTH_ADDR, FRAME_WIDTH);
    registers.insert(STREAM_HEIGHT_ADDR, FRAME_HEIGHT);
    registers.insert(STREAM_PIXEL_FORMAT_ADDR, advertised_format.pfnc() & 0xffff);
    registers.insert(STREAM_ACQ_ADDR, 0);
    registers.insert(STREAM_X_OFFSET_ADDR, 0);
    registers.insert(STREAM_Y_OFFSET_ADDR, 0);
    registers.insert(STREAM_TEST_PATTERN_ADDR, 0);
    registers
}

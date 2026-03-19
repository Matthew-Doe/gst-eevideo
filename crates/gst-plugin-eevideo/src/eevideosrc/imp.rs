use std::io::{self, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use eevideo_proto::{
    CompatPacketView, FrameAssembler, FrameDropReason, FrameEvent, PayloadType, StreamProfileId,
    StreamStats, VideoFrame, SUPPORTED_CAPS,
};
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;
use gst_base::subclass::prelude::*;
use gstreamer as gst;
use gstreamer_base as gst_base;
use once_cell::sync::Lazy;
use socket2::{Domain, Protocol, Socket, Type};

use crate::common::FrameFormat;
use crate::control::{
    default_control_backend, default_control_target, ControlSession, ControlTarget,
    SharedControlBackend, StreamConfiguration, StreamFormatDescriptor,
};

#[derive(Clone, Debug)]
struct Settings {
    address: String,
    port: u32,
    multicast_group: String,
    multicast_iface: String,
    timeout_ms: u64,
    latency_ms: u64,
    drop_incomplete: bool,
    managed_max_packet_size: u16,
    managed_packet_delay_ns: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 5000,
            multicast_group: String::new(),
            multicast_iface: String::new(),
            timeout_ms: 2000,
            latency_ms: 0,
            drop_incomplete: true,
            managed_max_packet_size: 1200,
            managed_packet_delay_ns: 0,
        }
    }
}

enum ReceiverEvent {
    Frame(VideoFrame),
    Error(String),
}

#[derive(Clone)]
struct ManagedControlSettings {
    enabled: bool,
    backend: SharedControlBackend,
    target: ControlTarget,
    stream_name: String,
}

impl Default for ManagedControlSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: default_control_backend(),
            target: default_control_target("stream0"),
            stream_name: "stream0".to_string(),
        }
    }
}

struct RunningState {
    stop: Arc<AtomicBool>,
    receiver: Arc<Mutex<Receiver<ReceiverEvent>>>,
    join: Option<JoinHandle<()>>,
    negotiated_format: Option<FrameFormat>,
    control_session: Option<ControlSession>,
}

#[derive(Debug, Default)]
struct ReceiveDiagnostics {
    timeout_drops: AtomicU64,
    payload_overflow_drops: AtomicU64,
    short_frame_drops: AtomicU64,
    duplicate_leader_drops: AtomicU64,
    payload_before_leader_drops: AtomicU64,
    trailer_before_leader_drops: AtomicU64,
    packet_after_trailer_drops: AtomicU64,
    parse_failures: AtomicU64,
    expected_format_mismatches: AtomicU64,
    midstream_format_changes: AtomicU64,
}

impl ReceiveDiagnostics {
    fn timeout_drops(&self) -> u64 {
        self.timeout_drops.load(Ordering::Relaxed)
    }

    fn payload_overflow_drops(&self) -> u64 {
        self.payload_overflow_drops.load(Ordering::Relaxed)
    }

    fn short_frame_drops(&self) -> u64 {
        self.short_frame_drops.load(Ordering::Relaxed)
    }

    fn duplicate_leader_drops(&self) -> u64 {
        self.duplicate_leader_drops.load(Ordering::Relaxed)
    }

    fn payload_before_leader_drops(&self) -> u64 {
        self.payload_before_leader_drops.load(Ordering::Relaxed)
    }

    fn trailer_before_leader_drops(&self) -> u64 {
        self.trailer_before_leader_drops.load(Ordering::Relaxed)
    }

    fn packet_after_trailer_drops(&self) -> u64 {
        self.packet_after_trailer_drops.load(Ordering::Relaxed)
    }

    fn parse_failures(&self) -> u64 {
        self.parse_failures.load(Ordering::Relaxed)
    }

    fn expected_format_mismatches(&self) -> u64 {
        self.expected_format_mismatches.load(Ordering::Relaxed)
    }

    fn midstream_format_changes(&self) -> u64 {
        self.midstream_format_changes.load(Ordering::Relaxed)
    }

    fn record_drop_reason(&self, reason: FrameDropReason) {
        let counter = match reason {
            FrameDropReason::MissingPayload | FrameDropReason::Timeout => &self.timeout_drops,
            FrameDropReason::PayloadOverflow => &self.payload_overflow_drops,
            FrameDropReason::ShortFrame => &self.short_frame_drops,
            FrameDropReason::DuplicateLeader => &self.duplicate_leader_drops,
            FrameDropReason::PayloadBeforeLeader => &self.payload_before_leader_drops,
            FrameDropReason::TrailerBeforeLeader => &self.trailer_before_leader_drops,
            FrameDropReason::PacketAfterTrailer => &self.packet_after_trailer_drops,
            FrameDropReason::UnsupportedPayload => &self.parse_failures,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    fn record_parse_failure(&self) {
        self.parse_failures.fetch_add(1, Ordering::Relaxed);
    }

    fn record_expected_format_mismatch(&self) {
        self.expected_format_mismatches
            .fetch_add(1, Ordering::Relaxed);
    }

    fn record_midstream_format_change(&self) {
        self.midstream_format_changes
            .fetch_add(1, Ordering::Relaxed);
    }
}

pub struct EeVideoSrc {
    settings: Mutex<Settings>,
    control: Mutex<ManagedControlSettings>,
    state: Mutex<Option<RunningState>>,
    stats: Arc<StreamStats>,
    diagnostics: Arc<ReceiveDiagnostics>,
    unlocked: AtomicBool,
}

impl Default for EeVideoSrc {
    fn default() -> Self {
        Self {
            settings: Mutex::new(Settings::default()),
            control: Mutex::new(ManagedControlSettings::default()),
            state: Mutex::new(None),
            stats: Arc::new(StreamStats::default()),
            diagnostics: Arc::new(ReceiveDiagnostics::default()),
            unlocked: AtomicBool::new(false),
        }
    }
}

impl EeVideoSrc {
    pub(crate) fn configure_control(
        &self,
        backend: SharedControlBackend,
        target: ControlTarget,
        stream_name: String,
    ) {
        let mut control = self.control.lock().expect("control lock poisoned");
        *control = ManagedControlSettings {
            enabled: true,
            backend,
            target,
            stream_name,
        };
    }
}

#[glib::object_subclass]
impl ObjectSubclass for EeVideoSrc {
    const NAME: &'static str = "GstEeVideoSrc";
    type Type = super::EeVideoSrc;
    type ParentType = gst_base::PushSrc;
}

impl ObjectImpl for EeVideoSrc {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_live(true);
        obj.set_format(gst::Format::Time);
        obj.set_automatic_eos(false);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpecString::builder("address")
                    .nick("Address")
                    .blurb("Local address to bind for UDP reception")
                    .default_value(Some("0.0.0.0"))
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt::builder("port")
                    .nick("Port")
                    .blurb("UDP port to bind for stream reception")
                    .minimum(0)
                    .maximum(u16::MAX as u32)
                    .default_value(5000)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecString::builder("multicast-group")
                    .nick("Multicast Group")
                    .blurb("Optional IPv4 multicast group to join for shared UDP reception")
                    .default_value(None)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecString::builder("multicast-iface")
                    .nick("Multicast Interface")
                    .blurb("Optional local IPv4 interface address used to join the multicast group")
                    .default_value(None)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt64::builder("timeout-ms")
                    .nick("Timeout")
                    .blurb("Timeout in milliseconds before incomplete frames are reaped")
                    .minimum(1)
                    .maximum(u32::MAX as u64)
                    .default_value(2000)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt64::builder("latency-ms")
                    .nick("Latency")
                    .blurb("Configured source latency hint in milliseconds")
                    .minimum(0)
                    .maximum(u32::MAX as u64)
                    .default_value(0)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecBoolean::builder("drop-incomplete")
                    .nick("Drop Incomplete")
                    .blurb("Drop incomplete frames instead of blocking for recovery")
                    .default_value(true)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt::builder("managed-max-packet-size")
                    .nick("Managed Max Packet Size")
                    .blurb("Managed-control max packet size requested from the remote device")
                    .minimum(256)
                    .maximum(u16::MAX as u32)
                    .default_value(1200)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt64::builder("managed-packet-delay-ns")
                    .nick("Managed Packet Delay")
                    .blurb("Managed-control inter-packet delay requested from the remote device in nanoseconds")
                    .minimum(0)
                    .maximum(u32::MAX as u64)
                    .default_value(0)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt64::builder("frames-received")
                    .nick("Frames Received")
                    .blurb("Number of completed frames received")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("frames-dropped")
                    .nick("Frames Dropped")
                    .blurb("Number of frames dropped by the receiver")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("packet-anomalies")
                    .nick("Packet Anomalies")
                    .blurb("Number of packet loss, duplication, overflow, or timeout anomalies")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("timeout-drops")
                    .nick("Timeout Drops")
                    .blurb("Number of frames dropped because payloads never completed in time")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("payload-overflow-drops")
                    .nick("Payload Overflow Drops")
                    .blurb("Number of frames dropped because buffered payload data exceeded the frame size")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("short-frame-drops")
                    .nick("Short Frame Drops")
                    .blurb("Number of frames dropped because the trailer arrived before the frame payload completed")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("duplicate-leader-drops")
                    .nick("Duplicate Leader Drops")
                    .blurb("Number of frames dropped because a second leader replaced an in-flight frame")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("payload-before-leader-drops")
                    .nick("Payload Before Leader Drops")
                    .blurb("Number of payload packets received before their frame leader")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("trailer-before-leader-drops")
                    .nick("Trailer Before Leader Drops")
                    .blurb("Number of trailer packets received before their frame leader")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("packet-after-trailer-drops")
                    .nick("Packet After Trailer Drops")
                    .blurb("Number of frames dropped because payload traffic continued after the trailer")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("parse-failures")
                    .nick("Parse Failures")
                    .blurb("Number of UDP payloads that could not be parsed as compatibility packets")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("expected-format-mismatches")
                    .nick("Expected Format Mismatches")
                    .blurb("Number of completed frames rejected because they did not match the managed-control format")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("midstream-format-changes")
                    .nick("Midstream Format Changes")
                    .blurb("Number of times the receiver rejected a live format change after startup")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().expect("settings lock poisoned");

        match pspec.name() {
            "address" => settings.address = value.get().expect("address type checked upstream"),
            "port" => settings.port = value.get().expect("port type checked upstream"),
            "multicast-group" => {
                settings.multicast_group =
                    value.get().expect("multicast-group type checked upstream")
            }
            "multicast-iface" => {
                settings.multicast_iface =
                    value.get().expect("multicast-iface type checked upstream")
            }
            "timeout-ms" => {
                settings.timeout_ms = value.get().expect("timeout type checked upstream")
            }
            "latency-ms" => {
                settings.latency_ms = value.get().expect("latency type checked upstream")
            }
            "drop-incomplete" => {
                settings.drop_incomplete =
                    value.get().expect("drop-incomplete type checked upstream")
            }
            "managed-max-packet-size" => {
                settings.managed_max_packet_size = value
                    .get::<u32>()
                    .expect("managed-max-packet-size type checked upstream")
                    as u16
            }
            "managed-packet-delay-ns" => {
                settings.managed_packet_delay_ns = value
                    .get()
                    .expect("managed-packet-delay-ns type checked upstream")
            }
            _ => unreachable!("unknown property {}", pspec.name()),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().expect("settings lock poisoned");

        match pspec.name() {
            "address" => settings.address.to_value(),
            "port" => settings.port.to_value(),
            "multicast-group" => settings.multicast_group.to_value(),
            "multicast-iface" => settings.multicast_iface.to_value(),
            "timeout-ms" => settings.timeout_ms.to_value(),
            "latency-ms" => settings.latency_ms.to_value(),
            "drop-incomplete" => settings.drop_incomplete.to_value(),
            "managed-max-packet-size" => u32::from(settings.managed_max_packet_size).to_value(),
            "managed-packet-delay-ns" => settings.managed_packet_delay_ns.to_value(),
            "frames-received" => self.stats.frames().to_value(),
            "frames-dropped" => self.stats.dropped_frames().to_value(),
            "packet-anomalies" => self.stats.packet_anomalies().to_value(),
            "timeout-drops" => self.diagnostics.timeout_drops().to_value(),
            "payload-overflow-drops" => self.diagnostics.payload_overflow_drops().to_value(),
            "short-frame-drops" => self.diagnostics.short_frame_drops().to_value(),
            "duplicate-leader-drops" => self.diagnostics.duplicate_leader_drops().to_value(),
            "payload-before-leader-drops" => {
                self.diagnostics.payload_before_leader_drops().to_value()
            }
            "trailer-before-leader-drops" => {
                self.diagnostics.trailer_before_leader_drops().to_value()
            }
            "packet-after-trailer-drops" => {
                self.diagnostics.packet_after_trailer_drops().to_value()
            }
            "parse-failures" => self.diagnostics.parse_failures().to_value(),
            "expected-format-mismatches" => {
                self.diagnostics.expected_format_mismatches().to_value()
            }
            "midstream-format-changes" => self.diagnostics.midstream_format_changes().to_value(),
            _ => unreachable!("unknown property {}", pspec.name()),
        }
    }
}

impl GstObjectImpl for EeVideoSrc {}

impl ElementImpl for EeVideoSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "EEVideo Source",
                "Source/Video/Network",
                "Receives EEVideo compatibility streams over UDP",
                "Codex",
            )
        });

        Some(&*METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = SUPPORTED_CAPS
                .parse::<gst::Caps>()
                .expect("static source caps must parse");
            let template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .expect("source pad template");
            vec![template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl BaseSrcImpl for EeVideoSrc {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        self.unlocked.store(false, Ordering::Relaxed);

        let settings = self
            .settings
            .lock()
            .expect("settings lock poisoned")
            .clone();
        let control = self.control.lock().expect("control lock poisoned").clone();
        let socket = create_receiver_socket(&settings).map_err(|err| {
            if settings.multicast_group.is_empty() {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    [
                        "failed to bind {}:{}: {}",
                        settings.address,
                        settings.port,
                        err
                    ]
                )
            } else {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    [
                        "failed to bind {}:{} and join multicast group {}: {}",
                        settings.address,
                        settings.port,
                        settings.multicast_group,
                        err
                    ]
                )
            }
        })?;
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::Settings,
                    ["failed to set read timeout: {}", err]
                )
            })?;
        let local_addr = socket.local_addr().map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::OpenRead,
                ["failed to inspect local receive socket address: {}", err]
            )
        })?;

        let mut control_session = None;
        let mut expected_format = None;
        if control.enabled {
            let control_request =
                build_stream_configuration(&settings, local_addr, &control.stream_name).map_err(
                    |err| {
                        gst::error_msg!(
                            gst::ResourceError::Settings,
                            ["failed to build managed control request: {}", err]
                        )
                    },
                )?;
            let mut session = ControlSession::new(
                Arc::clone(&control.backend),
                control.target.clone(),
                control_request.clone(),
            );
            session.describe().map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::Settings,
                    ["failed to describe control target: {}", err]
                )
            })?;
            let applied = session.configure(control_request).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::Settings,
                    ["failed to configure remote stream: {}", err]
                )
            })?;
            if applied.profile != StreamProfileId::CompatibilityV1 {
                return Err(gst::error_msg!(
                    gst::ResourceError::Settings,
                    [
                        "managed control applied unsupported profile {:?}",
                        applied.profile
                    ]
                ));
            }
            expected_format = applied.format.clone();
            session.start().map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::Settings,
                    ["failed to start remote stream: {}", err]
                )
            })?;
            control_session = Some(session);
        }

        let (sender, receiver) = mpsc::sync_channel(8);
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::clone(&self.stats);
        let diagnostics = Arc::clone(&self.diagnostics);
        let join = Some(spawn_receiver_thread(
            socket,
            settings,
            stop.clone(),
            stats,
            diagnostics,
            sender,
            expected_format,
        ));

        let mut state = self.state.lock().expect("state lock poisoned");
        *state = Some(RunningState {
            stop,
            receiver: Arc::new(Mutex::new(receiver)),
            join,
            negotiated_format: None,
            control_session,
        });

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        if let Some(mut state) = self.state.lock().expect("state lock poisoned").take() {
            state.stop.store(true, Ordering::Relaxed);
            if let Some(control_session) = state.control_session.as_mut() {
                let _ = control_session.stop();
                let _ = control_session.disconnect();
            }
            if let Some(join) = state.join.take() {
                let _ = join.join();
            }
        }

        self.unlocked.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_seekable(&self) -> bool {
        false
    }

    fn unlock(&self) -> Result<(), gst::ErrorMessage> {
        self.unlocked.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
        self.unlocked.store(false, Ordering::Relaxed);
        Ok(())
    }
}

impl PushSrcImpl for EeVideoSrc {
    fn create(
        &self,
        _buffer: Option<&mut gst::BufferRef>,
    ) -> Result<CreateSuccess, gst::FlowError> {
        loop {
            if self.unlocked.load(Ordering::Relaxed) {
                return Err(gst::FlowError::Flushing);
            }

            let receiver = {
                let state_guard = self.state.lock().expect("state lock poisoned");
                let state = state_guard.as_ref().ok_or(gst::FlowError::Error)?;
                Arc::clone(&state.receiver)
            };
            let event = {
                let receiver = receiver.lock().expect("receiver lock poisoned");
                receiver.recv_timeout(Duration::from_millis(50))
            };

            match event {
                Ok(ReceiverEvent::Frame(frame)) => {
                    let format = FrameFormat::from_frame(&frame);
                    let mut state_guard = self.state.lock().expect("state lock poisoned");
                    let state = state_guard.as_mut().ok_or(gst::FlowError::Error)?;
                    match state.negotiated_format {
                        Some(existing) if existing != format => {
                            self.stats.record_drop();
                            self.stats.record_packet_anomaly();
                            return Err(gst::FlowError::NotNegotiated);
                        }
                        None => {
                            if self.obj().set_caps(&format.to_caps()).is_err() {
                                return Err(gst::FlowError::NotNegotiated);
                            }
                            state.negotiated_format = Some(format);
                        }
                        Some(_) => {}
                    }

                    drop(state_guard);

                    let latency_ns = self
                        .settings
                        .lock()
                        .expect("settings lock poisoned")
                        .latency_ms
                        .saturating_mul(1_000_000);
                    let pts = frame.timestamp.saturating_add(latency_ns);
                    let mut buffer = gst::Buffer::from_mut_slice(frame.data);
                    if let Some(buffer_ref) = buffer.get_mut() {
                        buffer_ref.set_pts(gst::ClockTime::from_nseconds(pts));
                    }

                    return Ok(CreateSuccess::NewBuffer(buffer));
                }
                Ok(ReceiverEvent::Error(_message)) => return Err(gst::FlowError::Error),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Err(gst::FlowError::Eos),
            }
        }
    }
}

fn spawn_receiver_thread(
    socket: UdpSocket,
    settings: Settings,
    stop: Arc<AtomicBool>,
    stats: Arc<StreamStats>,
    diagnostics: Arc<ReceiveDiagnostics>,
    sender: SyncSender<ReceiverEvent>,
    expected_format: Option<StreamFormatDescriptor>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut assembler = FrameAssembler::new(Duration::from_millis(settings.timeout_ms));
        let mut buf = vec![0u8; 65_536];
        let mut current_format: Option<FrameFormat> = None;

        while !stop.load(Ordering::Relaxed) {
            let now = Instant::now();

            match socket.recv_from(&mut buf) {
                Ok((size, _peer)) => {
                    let packet = match CompatPacketView::parse(&buf[..size]) {
                        Ok(packet) => packet,
                        Err(_) => {
                            stats.record_packet_anomaly();
                            diagnostics.record_parse_failure();
                            continue;
                        }
                    };

                    match assembler.ingest_view(packet, now, &stats) {
                        Ok(Some(FrameEvent::Complete(frame))) => {
                            if let Some(expected) = expected_format.as_ref() {
                                if !frame_matches_expected_format(&frame, expected) {
                                    stats.record_drop();
                                    stats.record_packet_anomaly();
                                    diagnostics.record_expected_format_mismatch();
                                    let _ = sender.try_send(ReceiverEvent::Error(
                                        "managed-control format mismatch rejected".to_string(),
                                    ));
                                    break;
                                }
                            }

                            let format = FrameFormat::from_frame(&frame);
                            if let Some(existing) = current_format {
                                if existing != format {
                                    stats.record_drop();
                                    stats.record_packet_anomaly();
                                    diagnostics.record_midstream_format_change();
                                    let _ = sender.try_send(ReceiverEvent::Error(
                                        "mid-stream format change rejected".to_string(),
                                    ));
                                    break;
                                }
                            } else {
                                current_format = Some(format);
                            }

                            if sender.try_send(ReceiverEvent::Frame(frame)).is_err() {
                                stats.record_drop();
                            }
                        }
                        Ok(Some(FrameEvent::Dropped { reason, .. })) => {
                            diagnostics.record_drop_reason(reason);
                        }
                        Ok(None) => {}
                        Err(_) => {
                            stats.record_drop();
                            stats.record_packet_anomaly();
                            diagnostics.record_parse_failure();
                        }
                    }
                }
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    if !settings.drop_incomplete {
                        continue;
                    }
                }
                Err(err) => {
                    let _ =
                        sender.try_send(ReceiverEvent::Error(format!("udp receive failed: {err}")));
                    break;
                }
            }

            for event in assembler.reap_timeouts(Instant::now(), &stats) {
                if let FrameEvent::Dropped { reason, .. } = event {
                    diagnostics.record_drop_reason(reason);
                }
            }
        }
    })
}

fn create_receiver_socket(settings: &Settings) -> io::Result<UdpSocket> {
    const RECEIVE_SOCKET_BUFFER_BYTES: usize = 4 * 1024 * 1024;

    let multicast_group = parse_multicast_group(&settings.multicast_group)?;
    let multicast_iface = if multicast_group.is_some() {
        parse_multicast_iface(&settings.multicast_iface)?
    } else {
        None
    };
    let bind_addr = resolve_socket_addr(&settings.address, settings.port as u16)?;

    if multicast_group.is_some() && !matches!(bind_addr.ip(), IpAddr::V4(_)) {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "multicast-group requires an IPv4 bind address",
        ));
    }

    let socket = Socket::new(
        Domain::for_address(bind_addr),
        Type::DGRAM,
        Some(Protocol::UDP),
    )?;
    let _ = socket.set_recv_buffer_size(RECEIVE_SOCKET_BUFFER_BYTES);

    if multicast_group.is_some() {
        socket.set_reuse_address(true)?;
    }

    socket.bind(&bind_addr.into())?;

    let socket: UdpSocket = socket.into();
    if let Some(group) = multicast_group {
        let iface = multicast_iface.unwrap_or(Ipv4Addr::UNSPECIFIED);
        socket.join_multicast_v4(&group, &iface)?;
    }

    Ok(socket)
}

fn resolve_socket_addr(address: &str, port: u16) -> io::Result<SocketAddr> {
    (address, port).to_socket_addrs()?.next().ok_or_else(|| {
        io::Error::new(
            ErrorKind::AddrNotAvailable,
            format!("no socket addresses resolved for {address}:{port}"),
        )
    })
}

fn parse_multicast_group(value: &str) -> io::Result<Option<Ipv4Addr>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    let address = value.parse::<Ipv4Addr>().map_err(|err| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid multicast group {value}: {err}"),
        )
    })?;

    if !address.is_multicast() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("multicast-group must be an IPv4 multicast address, got {value}"),
        ));
    }

    Ok(Some(address))
}

fn parse_multicast_iface(value: &str) -> io::Result<Option<Ipv4Addr>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    let address = value.parse::<Ipv4Addr>().map_err(|err| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid multicast interface {value}: {err}"),
        )
    })?;

    Ok(Some(address))
}

fn build_stream_configuration(
    settings: &Settings,
    local_addr: SocketAddr,
    stream_name: &str,
) -> io::Result<StreamConfiguration> {
    let destination_host = match local_addr.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => ip.to_string(),
        IpAddr::V6(ip) if !ip.is_unspecified() => ip.to_string(),
        _ if !settings.multicast_iface.trim().is_empty() => settings.multicast_iface.clone(),
        _ if !settings.address.trim().is_empty() => settings.address.clone(),
        _ => {
            return Err(io::Error::new(
                ErrorKind::AddrNotAvailable,
                "managed control requires a concrete local receive address",
            ))
        }
    };

    if destination_host == "0.0.0.0" || destination_host == "::" {
        return Err(io::Error::new(
            ErrorKind::AddrNotAvailable,
            "managed control cannot advertise an unspecified destination address",
        ));
    }

    Ok(StreamConfiguration {
        stream_name: stream_name.to_string(),
        profile: StreamProfileId::CompatibilityV1,
        destination_host,
        port: local_addr.port(),
        bind_address: settings.address.clone(),
        packet_delay_ns: settings.managed_packet_delay_ns,
        max_packet_size: settings.managed_max_packet_size,
        format: None,
    })
}

fn frame_matches_expected_format(frame: &VideoFrame, expected: &StreamFormatDescriptor) -> bool {
    frame.payload_type == PayloadType::Image
        && frame.payload_type == expected.payload_type
        && frame.pixel_format == expected.pixel_format
        && frame.width == expected.width
        && frame.height == expected.height
}

#[cfg(test)]
mod tests {
    use super::{
        build_stream_configuration, frame_matches_expected_format, parse_multicast_group,
        parse_multicast_iface, Settings,
    };
    use crate as gsteevideo;
    use eevideo_proto::{FrameDropReason, PayloadType, PixelFormat, StreamProfileId, VideoFrame};
    use gstreamer as gst;
    use gstreamer::prelude::ObjectExt;
    use std::net::{Ipv4Addr, SocketAddr};

    fn init_gst() {
        static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        INIT.get_or_init(|| {
            gst::init().unwrap();
            gsteevideo::register_static().unwrap();
        });
    }

    #[test]
    fn accepts_empty_multicast_group() {
        assert_eq!(parse_multicast_group("").unwrap(), None);
    }

    #[test]
    fn accepts_ipv4_multicast_group() {
        assert_eq!(
            parse_multicast_group("239.255.10.10").unwrap(),
            Some(Ipv4Addr::new(239, 255, 10, 10))
        );
    }

    #[test]
    fn rejects_non_multicast_group() {
        assert!(parse_multicast_group("127.0.0.1").is_err());
    }

    #[test]
    fn accepts_empty_multicast_iface() {
        assert_eq!(parse_multicast_iface("").unwrap(), None);
    }

    #[test]
    fn accepts_ipv4_multicast_iface() {
        assert_eq!(
            parse_multicast_iface("192.168.1.20").unwrap(),
            Some(Ipv4Addr::new(192, 168, 1, 20))
        );
    }

    #[test]
    fn builds_managed_control_request_from_bound_address() {
        let settings = Settings {
            address: "127.0.0.1".to_string(),
            ..Settings::default()
        };

        let request = build_stream_configuration(
            &settings,
            SocketAddr::from(([127, 0, 0, 1], 5000)),
            "stream0",
        )
        .unwrap();

        assert_eq!(request.stream_name, "stream0");
        assert_eq!(request.profile, StreamProfileId::CompatibilityV1);
        assert_eq!(request.destination_host, "127.0.0.1");
        assert_eq!(request.port, 5000);
    }

    #[test]
    fn builds_managed_control_request_with_custom_transport_settings() {
        let settings = Settings {
            address: "127.0.0.1".to_string(),
            managed_packet_delay_ns: 24_913,
            managed_max_packet_size: 1400,
            ..Settings::default()
        };

        let request = build_stream_configuration(
            &settings,
            SocketAddr::from(([127, 0, 0, 1], 5000)),
            "stream0",
        )
        .unwrap();

        assert_eq!(request.packet_delay_ns, 24_913);
        assert_eq!(request.max_packet_size, 1400);
    }

    #[test]
    fn detects_managed_control_format_mismatches() {
        let frame = VideoFrame {
            frame_id: 1,
            timestamp: 0,
            width: 64,
            height: 32,
            pixel_format: PixelFormat::Mono8,
            payload_type: PayloadType::Image,
            data: vec![0; 64 * 32],
        };
        let expected = super::StreamFormatDescriptor {
            payload_type: PayloadType::Image,
            pixel_format: PixelFormat::Mono16,
            width: 64,
            height: 32,
        };

        assert!(!frame_matches_expected_format(&frame, &expected));
    }

    #[test]
    fn source_exposes_receive_diagnostic_properties() {
        init_gst();
        let src = gst::ElementFactory::make("eevideosrc").build().unwrap();

        for property_name in [
            "timeout-drops",
            "payload-overflow-drops",
            "short-frame-drops",
            "duplicate-leader-drops",
            "payload-before-leader-drops",
            "trailer-before-leader-drops",
            "packet-after-trailer-drops",
            "parse-failures",
            "expected-format-mismatches",
            "midstream-format-changes",
        ] {
            assert!(
                src.find_property(property_name).is_some(),
                "missing property {property_name}"
            );
            assert_eq!(src.property::<u64>(property_name), 0, "{property_name}");
        }
    }

    #[test]
    fn receive_diagnostics_track_individual_anomalies() {
        let diagnostics = super::ReceiveDiagnostics::default();

        diagnostics.record_drop_reason(FrameDropReason::Timeout);
        diagnostics.record_drop_reason(FrameDropReason::MissingPayload);
        diagnostics.record_drop_reason(FrameDropReason::PayloadOverflow);
        diagnostics.record_drop_reason(FrameDropReason::ShortFrame);
        diagnostics.record_drop_reason(FrameDropReason::DuplicateLeader);
        diagnostics.record_drop_reason(FrameDropReason::PayloadBeforeLeader);
        diagnostics.record_drop_reason(FrameDropReason::TrailerBeforeLeader);
        diagnostics.record_drop_reason(FrameDropReason::PacketAfterTrailer);
        diagnostics.record_parse_failure();
        diagnostics.record_expected_format_mismatch();
        diagnostics.record_midstream_format_change();

        assert_eq!(diagnostics.timeout_drops(), 2);
        assert_eq!(diagnostics.payload_overflow_drops(), 1);
        assert_eq!(diagnostics.short_frame_drops(), 1);
        assert_eq!(diagnostics.duplicate_leader_drops(), 1);
        assert_eq!(diagnostics.payload_before_leader_drops(), 1);
        assert_eq!(diagnostics.trailer_before_leader_drops(), 1);
        assert_eq!(diagnostics.packet_after_trailer_drops(), 1);
        assert_eq!(diagnostics.parse_failures(), 1);
        assert_eq!(diagnostics.expected_format_mismatches(), 1);
        assert_eq!(diagnostics.midstream_format_changes(), 1);
    }
}

use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use eevideo_proto::{
    CompatPacket, FrameAssembler, FrameEvent, StreamStats, VideoFrame, SUPPORTED_CAPS,
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

use crate::common::FrameFormat;

#[derive(Clone, Debug)]
struct Settings {
    address: String,
    port: u32,
    timeout_ms: u64,
    latency_ms: u64,
    drop_incomplete: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 5000,
            timeout_ms: 2000,
            latency_ms: 0,
            drop_incomplete: true,
        }
    }
}

enum ReceiverEvent {
    Frame(VideoFrame),
    Error(String),
}

struct RunningState {
    stop: Arc<AtomicBool>,
    receiver: Mutex<Receiver<ReceiverEvent>>,
    join: Option<JoinHandle<()>>,
    negotiated_format: Option<FrameFormat>,
}

pub struct EeVideoSrc {
    settings: Mutex<Settings>,
    state: Mutex<Option<RunningState>>,
    stats: Arc<StreamStats>,
    unlocked: AtomicBool,
}

impl Default for EeVideoSrc {
    fn default() -> Self {
        Self {
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(None),
            stats: Arc::new(StreamStats::default()),
            unlocked: AtomicBool::new(false),
        }
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
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().expect("settings lock poisoned");

        match pspec.name() {
            "address" => settings.address = value.get().expect("address type checked upstream"),
            "port" => settings.port = value.get().expect("port type checked upstream"),
            "timeout-ms" => settings.timeout_ms = value.get().expect("timeout type checked upstream"),
            "latency-ms" => settings.latency_ms = value.get().expect("latency type checked upstream"),
            "drop-incomplete" => {
                settings.drop_incomplete = value.get().expect("drop-incomplete type checked upstream")
            }
            _ => unreachable!("unknown property {}", pspec.name()),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().expect("settings lock poisoned");

        match pspec.name() {
            "address" => settings.address.to_value(),
            "port" => settings.port.to_value(),
            "timeout-ms" => settings.timeout_ms.to_value(),
            "latency-ms" => settings.latency_ms.to_value(),
            "drop-incomplete" => settings.drop_incomplete.to_value(),
            "frames-received" => self.stats.frames().to_value(),
            "frames-dropped" => self.stats.dropped_frames().to_value(),
            "packet-anomalies" => self.stats.packet_anomalies().to_value(),
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

        let settings = self.settings.lock().expect("settings lock poisoned").clone();
        let socket = UdpSocket::bind((settings.address.as_str(), settings.port as u16)).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::OpenRead,
                ["failed to bind {}:{}: {}", settings.address, settings.port, err]
            )
        })?;
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::Settings,
                    ["failed to set read timeout: {}", err]
                )
            })?;

        let (sender, receiver) = mpsc::sync_channel(8);
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::clone(&self.stats);
        let join = Some(spawn_receiver_thread(socket, settings, stop.clone(), stats, sender));

        let mut state = self.state.lock().expect("state lock poisoned");
        *state = Some(RunningState {
            stop,
            receiver: Mutex::new(receiver),
            join,
            negotiated_format: None,
        });

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        if let Some(mut state) = self.state.lock().expect("state lock poisoned").take() {
            state.stop.store(true, Ordering::Relaxed);
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

            let mut state_guard = self.state.lock().expect("state lock poisoned");
            let state = state_guard.as_mut().ok_or(gst::FlowError::Error)?;
            let event = {
                let receiver = state.receiver.lock().expect("receiver lock poisoned");
                receiver.recv_timeout(Duration::from_millis(50))
            };

            match event {
                Ok(ReceiverEvent::Frame(frame)) => {
                    let format = FrameFormat::from_frame(&frame);
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
    sender: SyncSender<ReceiverEvent>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut assembler = FrameAssembler::new(Duration::from_millis(settings.timeout_ms));
        let mut buf = vec![0u8; 65_536];
        let mut current_format: Option<FrameFormat> = None;

        while !stop.load(Ordering::Relaxed) {
            let now = Instant::now();

            match socket.recv_from(&mut buf) {
                Ok((size, _peer)) => {
                    let packet = match CompatPacket::parse(&buf[..size]) {
                        Ok(packet) => packet,
                        Err(_) => {
                            stats.record_packet_anomaly();
                            continue;
                        }
                    };

                    match assembler.ingest(packet, now, &stats) {
                        Ok(Some(FrameEvent::Complete(frame))) => {
                            let format = FrameFormat::from_frame(&frame);
                            if let Some(existing) = current_format {
                                if existing != format {
                                    stats.record_drop();
                                    stats.record_packet_anomaly();
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
                        Ok(Some(FrameEvent::Dropped { .. })) => {}
                        Ok(None) => {}
                        Err(_) => {
                            stats.record_drop();
                            stats.record_packet_anomaly();
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
                    let _ = sender.try_send(ReceiverEvent::Error(format!("udp receive failed: {err}")));
                    break;
                }
            }

            let _ = assembler.reap_timeouts(Instant::now(), &stats);
        }
    })
}

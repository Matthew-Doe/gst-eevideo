use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eevideo_proto::{
    CompatPacketEmitError, CompatPacketizer, PayloadType, StreamProfileId, StreamStats,
    VideoFrameRef, SUPPORTED_CAPS,
};
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;
use gstreamer as gst;
use gstreamer_base as gst_base;
use once_cell::sync::Lazy;
use socket2::SockRef;

use crate::common::{parse_caps, FrameFormat};
use crate::control::{
    default_control_backend, ControlSession, SharedControlBackend, StreamConfiguration,
    StreamFormatDescriptor,
};

#[derive(Clone, Debug)]
struct Settings {
    host: String,
    port: u32,
    bind_address: String,
    multicast_iface: String,
    mtu: u32,
    packet_delay_ns: u64,
    multicast_loop: bool,
    multicast_ttl: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 5000,
            bind_address: "0.0.0.0".to_string(),
            multicast_iface: String::new(),
            mtu: 1200,
            packet_delay_ns: 0,
            multicast_loop: true,
            multicast_ttl: 1,
        }
    }
}

struct RunningState {
    socket: UdpSocket,
    next_frame_id: u32,
    negotiated_format: Option<FrameFormat>,
    packetizer: CompatPacketizer,
    packet_scratch: Vec<u8>,
    packet_delay_ns: u64,
    control_session: ControlSession,
    control_template: StreamConfiguration,
}

pub struct EeVideoSink {
    settings: Mutex<Settings>,
    state: Mutex<Option<RunningState>>,
    stats: Arc<StreamStats>,
    control: SharedControlBackend,
    unlocked: AtomicBool,
}

impl Default for EeVideoSink {
    fn default() -> Self {
        Self {
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(None),
            stats: Arc::new(StreamStats::default()),
            control: default_control_backend(),
            unlocked: AtomicBool::new(false),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for EeVideoSink {
    const NAME: &'static str = "GstEeVideoSink";
    type Type = super::EeVideoSink;
    type ParentType = gst_base::BaseSink;
}

impl ObjectImpl for EeVideoSink {
    fn constructed(&self) {
        self.parent_constructed();
        self.obj().set_sync(false);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpecString::builder("host")
                    .nick("Host")
                    .blurb("Destination host for UDP streaming")
                    .default_value(Some("127.0.0.1"))
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt::builder("port")
                    .nick("Port")
                    .blurb("Destination UDP port")
                    .minimum(0)
                    .maximum(u16::MAX as u32)
                    .default_value(5000)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecString::builder("bind-address")
                    .nick("Bind Address")
                    .blurb("Local address to bind before connecting the UDP socket")
                    .default_value(Some("0.0.0.0"))
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecString::builder("multicast-iface")
                    .nick("Multicast Interface")
                    .blurb("Optional local IPv4 interface address used for multicast transmit")
                    .default_value(None)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt::builder("mtu")
                    .nick("MTU")
                    .blurb("Maximum packet size including the compatibility-stream UDP payload")
                    .minimum(256)
                    .maximum(65_535)
                    .default_value(1200)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt64::builder("packet-delay-ns")
                    .nick("Packet Delay")
                    .blurb("Delay inserted between transmitted packets in nanoseconds")
                    .minimum(0)
                    .maximum(u32::MAX as u64)
                    .default_value(0)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecBoolean::builder("multicast-loop")
                    .nick("Multicast Loop")
                    .blurb("Whether locally joined multicast receivers should receive transmitted packets")
                    .default_value(true)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt::builder("multicast-ttl")
                    .nick("Multicast TTL")
                    .blurb("IPv4 multicast TTL used when the destination host is a multicast group")
                    .minimum(0)
                    .maximum(255)
                    .default_value(1)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecUInt64::builder("frames-sent")
                    .nick("Frames Sent")
                    .blurb("Number of frames transmitted")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("frames-dropped")
                    .nick("Frames Dropped")
                    .blurb("Number of frames dropped before transmit")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecUInt64::builder("packet-anomalies")
                    .nick("Packet Anomalies")
                    .blurb("Number of transmit-side packetization or socket anomalies")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().expect("settings lock poisoned");

        match pspec.name() {
            "host" => settings.host = value.get().expect("host type checked upstream"),
            "port" => settings.port = value.get().expect("port type checked upstream"),
            "bind-address" => {
                settings.bind_address = value.get().expect("bind-address type checked upstream")
            }
            "multicast-iface" => {
                settings.multicast_iface =
                    value.get().expect("multicast-iface type checked upstream")
            }
            "mtu" => settings.mtu = value.get().expect("mtu type checked upstream"),
            "packet-delay-ns" => {
                settings.packet_delay_ns =
                    value.get().expect("packet-delay-ns type checked upstream")
            }
            "multicast-loop" => {
                settings.multicast_loop = value.get().expect("multicast-loop type checked upstream")
            }
            "multicast-ttl" => {
                settings.multicast_ttl = value.get().expect("multicast-ttl type checked upstream")
            }
            _ => unreachable!("unknown property {}", pspec.name()),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().expect("settings lock poisoned");

        match pspec.name() {
            "host" => settings.host.to_value(),
            "port" => settings.port.to_value(),
            "bind-address" => settings.bind_address.to_value(),
            "multicast-iface" => settings.multicast_iface.to_value(),
            "mtu" => settings.mtu.to_value(),
            "packet-delay-ns" => settings.packet_delay_ns.to_value(),
            "multicast-loop" => settings.multicast_loop.to_value(),
            "multicast-ttl" => settings.multicast_ttl.to_value(),
            "frames-sent" => self.stats.frames().to_value(),
            "frames-dropped" => self.stats.dropped_frames().to_value(),
            "packet-anomalies" => self.stats.packet_anomalies().to_value(),
            _ => unreachable!("unknown property {}", pspec.name()),
        }
    }
}

impl GstObjectImpl for EeVideoSink {}

impl ElementImpl for EeVideoSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "EEVideo Sink",
                "Sink/Video/Network",
                "Transmits EEVideo compatibility streams over UDP",
                "Codex",
            )
        });

        Some(&*METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = SUPPORTED_CAPS
                .parse::<gst::Caps>()
                .expect("static sink caps must parse");
            let template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .expect("sink pad template");
            vec![template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl BaseSinkImpl for EeVideoSink {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        self.unlocked.store(false, Ordering::Relaxed);

        let settings = self
            .settings
            .lock()
            .expect("settings lock poisoned")
            .clone();
        let mtu = settings.mtu as usize;
        let socket = UdpSocket::bind((settings.bind_address.as_str(), 0)).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::OpenWrite,
                ["failed to bind {}:0: {}", settings.bind_address, err]
            )
        })?;

        if let Ok(multicast_addr) = settings.host.parse::<std::net::Ipv4Addr>() {
            if multicast_addr.is_multicast() {
                if let Some(multicast_iface) = parse_multicast_iface(&settings.multicast_iface)
                    .map_err(|err| {
                        gst::error_msg!(
                            gst::ResourceError::Settings,
                            ["failed to parse multicast interface: {}", err]
                        )
                    })?
                {
                    SockRef::from(&socket)
                        .set_multicast_if_v4(&multicast_iface)
                        .map_err(|err| {
                            gst::error_msg!(
                                gst::ResourceError::Settings,
                                ["failed to set multicast interface: {}", err]
                            )
                        })?;
                }
                socket
                    .set_multicast_loop_v4(settings.multicast_loop)
                    .map_err(|err| {
                        gst::error_msg!(
                            gst::ResourceError::Settings,
                            ["failed to set multicast loopback: {}", err]
                        )
                    })?;
                socket
                    .set_multicast_ttl_v4(settings.multicast_ttl)
                    .map_err(|err| {
                        gst::error_msg!(
                            gst::ResourceError::Settings,
                            ["failed to set multicast TTL: {}", err]
                        )
                    })?;
            }
        }

        socket
            .connect((settings.host.as_str(), settings.port as u16))
            .map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenWrite,
                    [
                        "failed to connect {}:{}: {}",
                        settings.host,
                        settings.port,
                        err
                    ]
                )
            })?;

        let control_template = build_stream_configuration(&settings, None);
        let mut control_session =
            ControlSession::new(Arc::clone(&self.control), control_template.clone());
        control_session
            .configure(control_template.clone())
            .map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::Settings,
                    ["failed to configure control session: {}", err]
                )
            })?;
        let packetizer = CompatPacketizer::new(mtu).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::Settings,
                ["failed to configure packetizer: {}", err]
            )
        })?;

        let mut state = self.state.lock().expect("state lock poisoned");
        *state = Some(RunningState {
            socket,
            next_frame_id: 1,
            negotiated_format: None,
            packetizer,
            packet_scratch: Vec::with_capacity(mtu),
            packet_delay_ns: settings.packet_delay_ns,
            control_session,
            control_template,
        });

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        if let Some(mut state) = self.state.lock().expect("state lock poisoned").take() {
            let _ = state.control_session.stop();
        }
        self.unlocked.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let format = parse_caps(caps.as_ref())
            .map_err(|err| gst::loggable_error!(gst::CAT_RUST, "{}", err))?;

        let mut state_guard = self.state.lock().expect("state lock poisoned");
        let state = state_guard
            .as_mut()
            .ok_or_else(|| gst::loggable_error!(gst::CAT_RUST, "sink not started"))?;

        match state.negotiated_format {
            Some(existing) if existing != format => {
                return Err(gst::loggable_error!(
                    gst::CAT_RUST,
                    "mid-stream format change rejected"
                ));
            }
            Some(_) => {}
            None => {
                let mut config = state.control_template.clone();
                config.format = Some(to_stream_format_descriptor(format));
                state
                    .control_session
                    .configure(config)
                    .map_err(|err| gst::loggable_error!(gst::CAT_RUST, "{}", err))?;
                state
                    .control_session
                    .start()
                    .map_err(|err| gst::loggable_error!(gst::CAT_RUST, "{}", err))?;
                state.negotiated_format = Some(format);
            }
        }

        self.parent_set_caps(caps)
    }

    fn render(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        if self.unlocked.load(Ordering::Relaxed) {
            return Err(gst::FlowError::Flushing);
        }

        let mut state_guard = self.state.lock().expect("state lock poisoned");
        let state = state_guard.as_mut().ok_or(gst::FlowError::Error)?;
        let current_format = state.negotiated_format.ok_or(gst::FlowError::NotNegotiated)?;
        let expected_len = current_format.payload_len().map_err(|_| {
            self.stats.record_drop();
            self.stats.record_packet_anomaly();
            gst::FlowError::Error
        })?;

        let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
        if map.as_slice().len() != expected_len {
            self.stats.record_drop();
            self.stats.record_packet_anomaly();
            return Err(gst::FlowError::Error);
        }

        let timestamp = buffer.pts().map(|pts| pts.nseconds()).unwrap_or(0);
        let frame = VideoFrameRef {
            frame_id: state.next_frame_id,
            timestamp,
            width: current_format.width,
            height: current_format.height,
            pixel_format: current_format.pixel_format,
            payload_type: PayloadType::Image,
            data: map.as_slice(),
        };

        let socket = &state.socket;
        let packetizer = &state.packetizer;
        let packet_scratch = &mut state.packet_scratch;
        let packet_delay_ns = state.packet_delay_ns;
        packetizer
            .emit_packets(frame, packet_scratch, |packet| {
                socket.send(packet).map(|_| ())?;
                if packet_delay_ns > 0 {
                    thread::sleep(Duration::from_nanos(packet_delay_ns));
                }
                Ok::<(), std::io::Error>(())
            })
            .map_err(|err| {
                self.stats.record_drop();
                self.stats.record_packet_anomaly();
                match err {
                    CompatPacketEmitError::Packet(_) | CompatPacketEmitError::Emit(_) => {
                        gst::FlowError::Error
                    }
                }
            })?;

        state.next_frame_id = state.next_frame_id.wrapping_add(1);
        self.stats.record_frame();

        Ok(gst::FlowSuccess::Ok)
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

fn parse_multicast_iface(
    value: &str,
) -> Result<Option<std::net::Ipv4Addr>, std::net::AddrParseError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    value.parse::<std::net::Ipv4Addr>().map(Some)
}

fn build_stream_configuration(
    settings: &Settings,
    format: Option<StreamFormatDescriptor>,
) -> StreamConfiguration {
    StreamConfiguration {
        stream_name: format!("eevideo-compat://{}:{}", settings.host, settings.port),
        profile: StreamProfileId::CompatibilityV1,
        destination_host: settings.host.clone(),
        port: u16::try_from(settings.port).expect("port is validated by the property range"),
        bind_address: settings.bind_address.clone(),
        packet_delay_ns: settings.packet_delay_ns,
        max_packet_size: u16::try_from(settings.mtu).expect("mtu is validated by the property range"),
        format,
    }
}

fn to_stream_format_descriptor(format: FrameFormat) -> StreamFormatDescriptor {
    StreamFormatDescriptor {
        payload_type: PayloadType::Image,
        pixel_format: format.pixel_format,
        width: format.width,
        height: format.height,
    }
}

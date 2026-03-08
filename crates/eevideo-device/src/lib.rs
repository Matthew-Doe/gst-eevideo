use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use eevideo_control::coap::{CODE_CHANGED, CODE_CONTENT, CODE_GET, CODE_PUT};
use eevideo_control::discovery::{
    DISCOVERY_MULTICAST_ADDR, DISCOVERY_PORT, DISCOVERY_RESOURCE_TYPE,
};
use eevideo_control::register::RegisterReadKind;
use eevideo_control::{
    CoapMessage, CoapMessageType, CoapOption, OPTION_EEV_BINARY_ADDRESS, OPTION_EEV_REG_ACCESS,
};
use eevideo_proto::{CompatPacketizer, PayloadType, PixelFormat, VideoFrame};
use if_addrs::{get_if_addrs, IfAddr};

pub const CAPABILITIES_ADDR: u32 = 0;
pub const FEATURE_TABLE_ADDR: u32 = 16;
pub const STREAM_DESC_ADDR: u32 = 0x0000_0100;
pub const STREAM_MAX_PACKET_ADDR: u32 = 0x0004_0000;
pub const STREAM_DELAY_ADDR: u32 = 0x0004_0004;
pub const STREAM_DEST_MAC_ADDR: u32 = 0x0004_0008;
pub const STREAM_DEST_IP_ADDR: u32 = 0x0004_0010;
pub const STREAM_DEST_PORT_ADDR: u32 = 0x0004_0014;
pub const STREAM_SOURCE_PORT_ADDR: u32 = 0x0004_0018;
pub const STREAM_WIDTH_ADDR: u32 = 0x0004_001c;
pub const STREAM_HEIGHT_ADDR: u32 = 0x0004_0020;
pub const STREAM_PIXEL_FORMAT_ADDR: u32 = 0x0004_0024;
pub const STREAM_ACQ_ADDR: u32 = 0x0004_0028;
pub const STREAM_X_OFFSET_ADDR: u32 = 0x0004_002c;
pub const STREAM_Y_OFFSET_ADDR: u32 = 0x0004_0030;
pub const STREAM_TEST_PATTERN_ADDR: u32 = 0x0004_0034;

pub const MAX_PACKET_ENABLE_BIT: u32 = 1 << 16;
const MAX_PACKET_MASK: u32 = 0xffff;
const DELAY_MASK: u32 = 0x00ff_ffff;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRuntimeConfig {
    pub bind: SocketAddr,
    pub interface_name: Option<String>,
    pub advertise_address: Option<Ipv4Addr>,
    pub stream_name: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub fps: u32,
    pub mtu: u16,
    pub enforce_fixed_format: bool,
}

impl Default for DeviceRuntimeConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DISCOVERY_PORT),
            interface_name: None,
            advertise_address: None,
            stream_name: "stream0".to_string(),
            width: 1280,
            height: 720,
            pixel_format: PixelFormat::Uyvy,
            fps: 30,
            mtu: 1200,
            enforce_fixed_format: false,
        }
    }
}

impl DeviceRuntimeConfig {
    fn validate(&self) -> Result<()> {
        if self.stream_name.trim().is_empty() {
            bail!("stream name must not be empty");
        }
        if self.width == 0 || self.height == 0 {
            bail!("frame size must be non-zero");
        }
        if self.fps == 0 {
            bail!("fps must be greater than zero");
        }
        CompatPacketizer::new(self.mtu as usize)
            .with_context(|| format!("invalid device mtu {}", self.mtu))?;
        self.pixel_format
            .payload_len(self.width, self.height)
            .context("invalid device frame dimensions")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureConfiguration {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub fps: u32,
}

pub trait CaptureBackend: Send + 'static {
    fn start_capture(&mut self, config: CaptureConfiguration) -> Result<()>;
    fn stop_capture(&mut self) -> Result<()>;
    fn next_frame(&mut self) -> Result<VideoFrame>;
    fn current_format(&self) -> Option<CaptureConfiguration>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyntheticCaptureConfig {
    pub transmit_pixel_format: Option<PixelFormat>,
}

#[derive(Debug)]
struct SyntheticCaptureState {
    started_at: Instant,
    last_frame_at: Option<Instant>,
    next_frame_id: u32,
    requested: CaptureConfiguration,
    transmit_pixel_format: PixelFormat,
}

#[derive(Debug, Default)]
pub struct SyntheticCaptureBackend {
    config: SyntheticCaptureConfig,
    state: Option<SyntheticCaptureState>,
}

impl SyntheticCaptureBackend {
    pub fn new(config: SyntheticCaptureConfig) -> Self {
        Self {
            config,
            state: None,
        }
    }
}

impl CaptureBackend for SyntheticCaptureBackend {
    fn start_capture(&mut self, config: CaptureConfiguration) -> Result<()> {
        let transmit_pixel_format = self
            .config
            .transmit_pixel_format
            .unwrap_or(config.pixel_format);
        transmit_pixel_format
            .payload_len(config.width, config.height)
            .context("invalid synthetic capture dimensions")?;
        if transmit_pixel_format == PixelFormat::Uyvy && config.width % 2 != 0 {
            bail!("UYVY synthetic capture width must be even");
        }

        self.state = Some(SyntheticCaptureState {
            started_at: Instant::now(),
            last_frame_at: None,
            next_frame_id: 1,
            requested: config,
            transmit_pixel_format,
        });
        Ok(())
    }

    fn stop_capture(&mut self) -> Result<()> {
        self.state = None;
        Ok(())
    }

    fn next_frame(&mut self) -> Result<VideoFrame> {
        let state = self
            .state
            .as_mut()
            .ok_or_else(|| anyhow!("synthetic capture is not running"))?;
        let frame_interval =
            Duration::from_nanos(1_000_000_000u64 / u64::from(state.requested.fps));
        if let Some(last_frame_at) = state.last_frame_at {
            let elapsed = last_frame_at.elapsed();
            if elapsed < frame_interval {
                thread::sleep(frame_interval - elapsed);
            }
        }

        let frame_id = state.next_frame_id;
        state.next_frame_id = state.next_frame_id.wrapping_add(1).max(1);
        state.last_frame_at = Some(Instant::now());
        let width = state.requested.width;
        let height = state.requested.height;
        let pixel_format = state.transmit_pixel_format;

        Ok(VideoFrame {
            frame_id,
            timestamp: state
                .started_at
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
            width,
            height,
            pixel_format,
            payload_type: PayloadType::Image,
            data: generate_pattern_data(frame_id, width, height, pixel_format)?,
        })
    }

    fn current_format(&self) -> Option<CaptureConfiguration> {
        self.state.as_ref().map(|state| CaptureConfiguration {
            width: state.requested.width,
            height: state.requested.height,
            pixel_format: state.transmit_pixel_format,
            fps: state.requested.fps,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SelectedInterface {
    name: String,
    address: Ipv4Addr,
}

#[derive(Clone, Debug)]
struct DeviceState {
    registers: BTreeMap<u32, u32>,
    strings: BTreeMap<u32, Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SenderSettings {
    destination_ip: Ipv4Addr,
    destination_port: u16,
    mtu: usize,
    packet_delay_ns: u32,
    capture: CaptureConfiguration,
}

pub struct DeviceRuntime {
    local_addr: SocketAddr,
    uri: String,
    stop: Arc<AtomicBool>,
    start_count: Arc<AtomicUsize>,
    stop_count: Arc<AtomicUsize>,
    state: Arc<(Mutex<DeviceState>, Condvar)>,
    control_join: Option<JoinHandle<()>>,
    sender_join: Option<JoinHandle<()>>,
}

impl DeviceRuntime {
    pub fn spawn<C>(config: DeviceRuntimeConfig, capture: C) -> Result<Self>
    where
        C: CaptureBackend,
    {
        config.validate()?;
        let selected_interface = select_interface(&config)?;
        let socket = UdpSocket::bind(config.bind)
            .with_context(|| format!("failed to bind device socket at {}", config.bind))?;
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .context("failed to configure device socket timeout")?;
        maybe_join_discovery_multicast(&socket, &selected_interface)?;

        let local_addr = socket
            .local_addr()
            .context("failed to read device local address")?;
        let state = Arc::new((
            Mutex::new(DeviceState {
                registers: build_registers(&config),
                strings: build_strings(&config),
            }),
            Condvar::new(),
        ));
        let stop = Arc::new(AtomicBool::new(false));
        let start_count = Arc::new(AtomicUsize::new(0));
        let stop_count = Arc::new(AtomicUsize::new(0));

        let control_join = Some(spawn_control_loop(
            socket,
            config.clone(),
            selected_interface.clone(),
            Arc::clone(&state),
            Arc::clone(&stop),
            Arc::clone(&start_count),
            Arc::clone(&stop_count),
        ));
        let uri = format!(
            "coap://{}",
            SocketAddr::new(
                IpAddr::V4(advertised_ip(&config, &local_addr)?),
                local_addr.port()
            )
        );
        let sender_join = Some(spawn_sender_loop(
            config,
            selected_interface,
            Arc::clone(&state),
            Arc::clone(&stop),
            capture,
        ));

        Ok(Self {
            local_addr,
            uri,
            stop,
            start_count,
            stop_count,
            state,
            control_join,
            sender_join,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn uri(&self) -> String {
        self.uri.clone()
    }

    pub fn start_count(&self) -> usize {
        self.start_count.load(Ordering::Relaxed)
    }

    pub fn stop_count(&self) -> usize {
        self.stop_count.load(Ordering::Relaxed)
    }

    pub fn registers(&self) -> BTreeMap<u32, u32> {
        let (state, _) = &*self.state;
        state
            .lock()
            .expect("device state lock poisoned")
            .registers
            .clone()
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let (_, cvar) = &*self.state;
        cvar.notify_all();
        let _ = UdpSocket::bind(SocketAddr::new(self.local_addr.ip(), 0))
            .or_else(|_| UdpSocket::bind("127.0.0.1:0"))
            .and_then(|socket| socket.send_to(&[0], self.local_addr));

        if let Some(join) = self.control_join.take() {
            let _ = join.join();
        }
        if let Some(join) = self.sender_join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for DeviceRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn build_strings(config: &DeviceRuntimeConfig) -> BTreeMap<u32, Vec<u8>> {
    BTreeMap::from([(STREAM_DESC_ADDR, {
        let mut value = format!("EEVideo {}", config.stream_name).into_bytes();
        value.push(0);
        value
    })])
}

fn build_registers(config: &DeviceRuntimeConfig) -> BTreeMap<u32, u32> {
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

    registers.insert(STREAM_MAX_PACKET_ADDR, u32::from(config.mtu));
    registers.insert(STREAM_DELAY_ADDR, 0);
    registers.insert(STREAM_DEST_IP_ADDR, 0);
    registers.insert(STREAM_DEST_PORT_ADDR, 0);
    registers.insert(STREAM_SOURCE_PORT_ADDR, 0);
    registers.insert(STREAM_WIDTH_ADDR, config.width);
    registers.insert(STREAM_HEIGHT_ADDR, config.height);
    registers.insert(
        STREAM_PIXEL_FORMAT_ADDR,
        config.pixel_format.pfnc() & 0xffff,
    );
    registers.insert(STREAM_ACQ_ADDR, 0);
    registers.insert(STREAM_X_OFFSET_ADDR, 0);
    registers.insert(STREAM_Y_OFFSET_ADDR, 0);
    registers.insert(STREAM_TEST_PATTERN_ADDR, 0);
    registers
}

fn maybe_join_discovery_multicast(socket: &UdpSocket, interface: &SelectedInterface) -> Result<()> {
    if interface.address.is_loopback() {
        return Ok(());
    }
    socket
        .join_multicast_v4(&DISCOVERY_MULTICAST_ADDR, &interface.address)
        .with_context(|| {
            format!(
                "failed to join discovery multicast group {} on {}",
                DISCOVERY_MULTICAST_ADDR, interface.address
            )
        })?;
    Ok(())
}

fn select_interface(config: &DeviceRuntimeConfig) -> Result<SelectedInterface> {
    let bind_ip = match config.bind.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip),
        _ => None,
    };

    if let Some(bind_ip) = bind_ip {
        if let Some(interface) = interfaces()?
            .into_iter()
            .find(|interface| interface.address == bind_ip)
        {
            return Ok(interface);
        }
        return Ok(SelectedInterface {
            name: "manual".to_string(),
            address: bind_ip,
        });
    }

    if let Some(interface_name) = config.interface_name.as_deref() {
        return interfaces()?
            .into_iter()
            .find(|interface| interface.name == interface_name)
            .ok_or_else(|| anyhow!("no IPv4 interface named {interface_name}"));
    }

    if let Some(address) = config.advertise_address {
        return interfaces()?
            .into_iter()
            .find(|interface| interface.address == address)
            .ok_or_else(|| anyhow!("no IPv4 interface with address {address}"));
    }

    let interfaces = interfaces()?;
    match interfaces.as_slice() {
        [interface] => Ok(interface.clone()),
        [] => bail!("no non-loopback IPv4 interface found; pass --iface or --advertise-address"),
        _ => bail!(
            "multiple non-loopback IPv4 interfaces found; pass --iface or --advertise-address"
        ),
    }
}

fn interfaces() -> Result<Vec<SelectedInterface>> {
    let mut interfaces = Vec::new();
    for interface in get_if_addrs().context("failed to enumerate local interfaces")? {
        if interface.is_loopback() {
            continue;
        }
        if let IfAddr::V4(address) = interface.addr {
            interfaces.push(SelectedInterface {
                name: interface.name,
                address: address.ip,
            });
        }
    }
    Ok(interfaces)
}

fn advertised_ip(config: &DeviceRuntimeConfig, local_addr: &SocketAddr) -> Result<Ipv4Addr> {
    if let Some(ip) = config.advertise_address {
        return Ok(ip);
    }
    match local_addr.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => Ok(ip),
        IpAddr::V4(_) => select_interface(config).map(|interface| interface.address),
        IpAddr::V6(_) => bail!("device runtime currently only supports IPv4"),
    }
}

fn spawn_control_loop(
    socket: UdpSocket,
    config: DeviceRuntimeConfig,
    interface: SelectedInterface,
    state: Arc<(Mutex<DeviceState>, Condvar)>,
    stop: Arc<AtomicBool>,
    start_count: Arc<AtomicUsize>,
    stop_count: Arc<AtomicUsize>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        while !stop.load(Ordering::Relaxed) {
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

            let request = match CoapMessage::decode(&buffer[..size]) {
                Ok(request) => request,
                Err(_) => continue,
            };

            let response = if is_discovery_request(&request) {
                Some(build_discovery_response(&request, &config, &interface))
            } else {
                handle_register_request(&config, &request, &state, &start_count, &stop_count)
            };

            if let Some(response) = response {
                if let Ok(bytes) = response.encode() {
                    let _ = socket.send_to(&bytes, peer);
                }
            }
        }
    })
}

fn spawn_sender_loop<C>(
    config: DeviceRuntimeConfig,
    interface: SelectedInterface,
    state: Arc<(Mutex<DeviceState>, Condvar)>,
    stop: Arc<AtomicBool>,
    mut capture: C,
) -> JoinHandle<()>
where
    C: CaptureBackend,
{
    thread::spawn(move || {
        let sender_bind = SocketAddr::new(IpAddr::V4(interface.address), 0);
        let socket = UdpSocket::bind(sender_bind)
            .or_else(|_| UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)));
        let Ok(socket) = socket else {
            return;
        };

        let mut active_capture = None;
        let mut active_mtu = None;
        let mut packetizer = None;
        let mut scratch = Vec::new();

        while !stop.load(Ordering::Relaxed) {
            let settings = match wait_for_stream_settings(&state, &stop, &config) {
                Some(settings) => settings,
                None => break,
            };

            if settings.destination_port == 0 {
                if active_capture.is_some() {
                    let _ = capture.stop_capture();
                    active_capture = None;
                }
                thread::sleep(Duration::from_millis(20));
                continue;
            }

            if active_capture.as_ref() != Some(&settings.capture) {
                if active_capture.is_some() {
                    let _ = capture.stop_capture();
                }
                if capture.start_capture(settings.capture.clone()).is_err() {
                    active_capture = None;
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
                active_capture = Some(settings.capture.clone());
            }

            if active_mtu != Some(settings.mtu) {
                match CompatPacketizer::new(settings.mtu) {
                    Ok(new_packetizer) => {
                        packetizer = Some(new_packetizer);
                        active_mtu = Some(settings.mtu);
                    }
                    Err(_) => {
                        thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                }
            }

            let frame = match capture.next_frame() {
                Ok(frame) => frame,
                Err(_) => {
                    let _ = capture.stop_capture();
                    active_capture = None;
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
            };
            let destination = SocketAddrV4::new(settings.destination_ip, settings.destination_port);
            let Some(packetizer) = packetizer.as_ref() else {
                thread::sleep(Duration::from_millis(20));
                continue;
            };
            let _ = packetizer.emit_packets((&frame).into(), &mut scratch, |packet| {
                socket.send_to(packet, destination)?;
                if settings.packet_delay_ns > 0 {
                    thread::sleep(Duration::from_nanos(u64::from(settings.packet_delay_ns)));
                }
                Ok::<(), std::io::Error>(())
            });
        }

        if active_capture.is_some() {
            let _ = capture.stop_capture();
        }
    })
}

fn is_discovery_request(request: &CoapMessage) -> bool {
    if request.code != CODE_GET {
        return false;
    }

    let path = request
        .options
        .iter()
        .filter(|option| option.number == 11)
        .filter_map(|option| std::str::from_utf8(&option.value).ok())
        .collect::<Vec<_>>();
    let query_matches = request.options.iter().any(|option| {
        option.number == 15
            && option.value.as_slice() == format!("rt={DISCOVERY_RESOURCE_TYPE}").as_bytes()
    });

    path == [".well-known", "core"] && query_matches
}

fn build_discovery_response(
    request: &CoapMessage,
    config: &DeviceRuntimeConfig,
    interface: &SelectedInterface,
) -> CoapMessage {
    let payload = format!(
        "</{}>;rt=\"{}\";if=\"{}\"",
        config.stream_name, DISCOVERY_RESOURCE_TYPE, interface.name
    );
    CoapMessage::new(
        CoapMessageType::NonConfirmable,
        CODE_CONTENT,
        request.message_id,
        request.token.clone(),
        Vec::<CoapOption>::new(),
        payload,
    )
}

fn handle_register_request(
    config: &DeviceRuntimeConfig,
    request: &CoapMessage,
    state: &Arc<(Mutex<DeviceState>, Condvar)>,
    start_count: &Arc<AtomicUsize>,
    stop_count: &Arc<AtomicUsize>,
) -> Option<CoapMessage> {
    let address = request
        .options
        .iter()
        .find(|option| option.number == OPTION_EEV_BINARY_ADDRESS)
        .and_then(|option| option.value.clone().try_into().ok())
        .map(u32::from_be_bytes)?;
    let reg_access = request
        .options
        .iter()
        .find(|option| option.number == OPTION_EEV_REG_ACCESS)
        .and_then(|option| option.value.first().copied());

    let (lock, cvar) = &**state;
    let mut state = lock.lock().ok()?;

    if request.code == CODE_GET {
        let payload = if reg_access.map(|value| value >> 5) == Some(RegisterReadKind::String as u8)
        {
            state.strings.get(&address).cloned().unwrap_or_default()
        } else {
            state
                .registers
                .get(&address)
                .copied()
                .unwrap_or_default()
                .to_be_bytes()
                .to_vec()
        };
        return Some(CoapMessage::new(
            CoapMessageType::Acknowledgement,
            CODE_CONTENT,
            request.message_id,
            request.token.clone(),
            Vec::<CoapOption>::new(),
            payload,
        ));
    }

    if request.code != CODE_PUT || request.payload.len() != 4 {
        return None;
    }

    let requested_value = u32::from_be_bytes(request.payload.clone().try_into().ok()?);
    let old_value = state.registers.get(&address).copied().unwrap_or_default();
    let new_value = normalize_write_value(config, address, old_value, requested_value);
    state.registers.insert(address, new_value);

    if address == STREAM_MAX_PACKET_ADDR {
        let old_enabled = old_value & MAX_PACKET_ENABLE_BIT != 0;
        let enabled = new_value & MAX_PACKET_ENABLE_BIT != 0;
        if !old_enabled && enabled {
            start_count.fetch_add(1, Ordering::Relaxed);
            cvar.notify_all();
        } else if old_enabled && !enabled {
            stop_count.fetch_add(1, Ordering::Relaxed);
            cvar.notify_all();
        }
    } else if matches!(
        address,
        STREAM_DELAY_ADDR
            | STREAM_DEST_IP_ADDR
            | STREAM_DEST_PORT_ADDR
            | STREAM_WIDTH_ADDR
            | STREAM_HEIGHT_ADDR
            | STREAM_PIXEL_FORMAT_ADDR
    ) {
        cvar.notify_all();
    }

    Some(CoapMessage::new(
        CoapMessageType::Acknowledgement,
        CODE_CHANGED,
        request.message_id,
        request.token.clone(),
        Vec::<CoapOption>::new(),
        Vec::<u8>::new(),
    ))
}

fn normalize_write_value(
    config: &DeviceRuntimeConfig,
    address: u32,
    old_value: u32,
    requested_value: u32,
) -> u32 {
    if !config.enforce_fixed_format {
        return requested_value;
    }

    match address {
        STREAM_WIDTH_ADDR if requested_value != config.width => old_value,
        STREAM_HEIGHT_ADDR if requested_value != config.height => old_value,
        STREAM_PIXEL_FORMAT_ADDR if requested_value != (config.pixel_format.pfnc() & 0xffff) => {
            old_value
        }
        _ => requested_value,
    }
}

fn wait_for_stream_settings(
    state: &Arc<(Mutex<DeviceState>, Condvar)>,
    stop: &AtomicBool,
    config: &DeviceRuntimeConfig,
) -> Option<SenderSettings> {
    let (lock, cvar) = &**state;
    let mut guard = lock.lock().ok()?;
    while !stop.load(Ordering::Relaxed) {
        if stream_enabled(&guard.registers) {
            let destination_port = (guard
                .registers
                .get(&STREAM_DEST_PORT_ADDR)
                .copied()
                .unwrap_or_default()
                & 0xffff) as u16;
            let destination_ip = Ipv4Addr::from(
                guard
                    .registers
                    .get(&STREAM_DEST_IP_ADDR)
                    .copied()
                    .unwrap_or_default(),
            );
            let mtu = guard
                .registers
                .get(&STREAM_MAX_PACKET_ADDR)
                .copied()
                .map(|value| (value & MAX_PACKET_MASK) as usize)
                .unwrap_or(config.mtu as usize)
                .max(256);
            let packet_delay_ns = guard
                .registers
                .get(&STREAM_DELAY_ADDR)
                .copied()
                .unwrap_or_default()
                & DELAY_MASK;
            let width = register_or_default(&guard.registers, STREAM_WIDTH_ADDR, config.width);
            let height = register_or_default(&guard.registers, STREAM_HEIGHT_ADDR, config.height);
            let advertised_bits = guard
                .registers
                .get(&STREAM_PIXEL_FORMAT_ADDR)
                .copied()
                .unwrap_or(config.pixel_format.pfnc() & 0xffff);
            let pixel_format =
                pixel_format_from_device_bits(advertised_bits).unwrap_or(config.pixel_format);

            return Some(SenderSettings {
                destination_ip,
                destination_port,
                mtu,
                packet_delay_ns,
                capture: CaptureConfiguration {
                    width,
                    height,
                    pixel_format,
                    fps: config.fps,
                },
            });
        }

        guard = match cvar.wait_timeout(guard, Duration::from_millis(50)) {
            Ok((guard, _)) => guard,
            Err(_) => return None,
        };
    }
    None
}

fn stream_enabled(registers: &BTreeMap<u32, u32>) -> bool {
    registers
        .get(&STREAM_MAX_PACKET_ADDR)
        .copied()
        .unwrap_or_default()
        & MAX_PACKET_ENABLE_BIT
        != 0
}

fn register_or_default(registers: &BTreeMap<u32, u32>, address: u32, default: u32) -> u32 {
    let value = registers.get(&address).copied().unwrap_or(default);
    if value == 0 {
        default
    } else {
        value
    }
}

fn pixel_format_from_device_bits(value: u32) -> Option<PixelFormat> {
    PixelFormat::from_pfnc(value).ok().or_else(|| {
        [
            PixelFormat::Mono8,
            PixelFormat::Mono16,
            PixelFormat::BayerGr8,
            PixelFormat::BayerRg8,
            PixelFormat::BayerGb8,
            PixelFormat::BayerBg8,
            PixelFormat::Rgb8,
            PixelFormat::Uyvy,
        ]
        .into_iter()
        .find(|format| format.pfnc() & 0xffff == value)
    })
}

fn generate_pattern_data(
    frame_id: u32,
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
) -> Result<Vec<u8>> {
    let capacity = pixel_format
        .payload_len(width, height)
        .context("invalid test-pattern dimensions")?;
    let mut data = Vec::with_capacity(capacity);
    let frame_phase = (frame_id & 0xff) as u8;

    match pixel_format {
        PixelFormat::Uyvy => {
            for y in 0..height {
                for x in (0..width).step_by(2) {
                    let y0 = (((x + frame_id) * 255) / width.max(1)) as u8;
                    let y1 = ((((x + 1) + frame_id) * 255) / width.max(1)) as u8;
                    let u = (((y + frame_id) * 255) / height.max(1)) as u8;
                    let v = frame_phase.wrapping_add(((x / 2) * 31) as u8);
                    data.extend_from_slice(&[u, y0, v, y1]);
                }
            }
        }
        PixelFormat::Mono8
        | PixelFormat::BayerGr8
        | PixelFormat::BayerRg8
        | PixelFormat::BayerGb8
        | PixelFormat::BayerBg8 => {
            for y in 0..height {
                for x in 0..width {
                    let sample = ((((x + y + frame_id) * 13) + (frame_id * 7)) & 0xff) as u8;
                    data.push(sample);
                }
            }
        }
        PixelFormat::Mono16 => {
            for y in 0..height {
                for x in 0..width {
                    let sample = (((x + y + frame_id) * 257) & 0xffff) as u16;
                    data.extend_from_slice(&sample.to_le_bytes());
                }
            }
        }
        PixelFormat::Rgb8 => {
            for y in 0..height {
                for x in 0..width {
                    let r = (((x + frame_id) * 255) / width.max(1)) as u8;
                    let g = (((y + frame_id) * 255) / height.max(1)) as u8;
                    let b = frame_phase.wrapping_add(((x ^ y) & 0xff) as u8);
                    data.extend_from_slice(&[r, g, b]);
                }
            }
        }
    }

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::{
        build_discovery_response, build_registers, CaptureBackend, CaptureConfiguration,
        DeviceRuntime, DeviceRuntimeConfig, SelectedInterface, SyntheticCaptureBackend,
        SyntheticCaptureConfig, STREAM_HEIGHT_ADDR, STREAM_MAX_PACKET_ADDR,
        STREAM_PIXEL_FORMAT_ADDR, STREAM_WIDTH_ADDR,
    };
    use eevideo_control::discovery::parse_discovery_advertisement;
    use eevideo_control::register::RegisterClient;
    use eevideo_control::{CoapMessage, CoapMessageType, CoapOption};
    use eevideo_proto::{PayloadType, PixelFormat};
    use std::net::Ipv4Addr;
    use std::time::Duration;

    #[test]
    fn register_map_uses_expected_defaults() {
        let registers = build_registers(&DeviceRuntimeConfig::default());
        assert_eq!(registers.get(&STREAM_MAX_PACKET_ADDR).copied(), Some(1200));
    }

    #[test]
    fn discovery_payload_round_trips_with_current_parser() {
        let request = CoapMessage::new(
            CoapMessageType::NonConfirmable,
            1,
            0x2000,
            [0x01],
            vec![
                CoapOption::new(11, b".well-known".to_vec()),
                CoapOption::new(11, b"core".to_vec()),
                CoapOption::new(15, b"rt=eev.cam".to_vec()),
            ],
            Vec::<u8>::new(),
        );
        let response = build_discovery_response(
            &request,
            &DeviceRuntimeConfig::default(),
            &SelectedInterface {
                name: "eth0".to_string(),
                address: Ipv4Addr::new(192, 168, 1, 50),
            },
        );

        let advertisement = parse_discovery_advertisement(&response.payload).unwrap();
        assert_eq!(advertisement.links.len(), 1);
        assert_eq!(advertisement.links[0].target, "/stream0");
        assert_eq!(advertisement.links[0].attributes["rt"], "eev.cam");
        assert_eq!(advertisement.links[0].attributes["if"], "eth0");
    }

    #[test]
    fn synthetic_capture_emits_expected_uyvy_payload_length() {
        let mut capture = SyntheticCaptureBackend::new(SyntheticCaptureConfig::default());
        capture
            .start_capture(CaptureConfiguration {
                width: 1280,
                height: 720,
                pixel_format: PixelFormat::Uyvy,
                fps: 30,
            })
            .unwrap();

        let frame = capture.next_frame().unwrap();
        assert_eq!(frame.payload_type, PayloadType::Image);
        assert_eq!(frame.data.len(), 1280 * 720 * 2);
    }

    #[test]
    fn fixed_format_runtime_rejects_mismatched_format_writes() {
        let device = DeviceRuntime::spawn(
            DeviceRuntimeConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                width: 320,
                height: 240,
                pixel_format: PixelFormat::Mono8,
                enforce_fixed_format: true,
                ..DeviceRuntimeConfig::default()
            },
            SyntheticCaptureBackend::new(SyntheticCaptureConfig::default()),
        )
        .unwrap();
        let client = RegisterClient::new("127.0.0.1:0".parse().unwrap(), device.local_addr())
            .with_timeout(Duration::from_millis(250));

        client.write_u32(STREAM_WIDTH_ADDR, 640).unwrap();
        client.write_u32(STREAM_HEIGHT_ADDR, 480).unwrap();
        client
            .write_u32(STREAM_PIXEL_FORMAT_ADDR, PixelFormat::Rgb8.pfnc() & 0xffff)
            .unwrap();

        let registers = device.registers();
        assert_eq!(registers.get(&STREAM_WIDTH_ADDR).copied(), Some(320));
        assert_eq!(registers.get(&STREAM_HEIGHT_ADDR).copied(), Some(240));
        assert_eq!(
            registers.get(&STREAM_PIXEL_FORMAT_ADDR).copied(),
            Some(PixelFormat::Mono8.pfnc() & 0xffff)
        );
    }
}

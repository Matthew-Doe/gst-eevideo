use std::collections::BTreeMap;
use std::ffi::OsString;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use eevideo_control::coap::{CODE_CHANGED, CODE_CONTENT, CODE_GET, CODE_PUT};
use eevideo_control::discovery::{DISCOVERY_MULTICAST_ADDR, DISCOVERY_PORT, DISCOVERY_RESOURCE_TYPE};
use eevideo_control::register::RegisterReadKind;
use eevideo_control::{
    CoapMessage, CoapMessageType, CoapOption, OPTION_EEV_BINARY_ADDRESS, OPTION_EEV_REG_ACCESS,
};
use eevideo_proto::{CompatPacketizer, PayloadType, PixelFormat, VideoFrame};
use if_addrs::{get_if_addrs, IfAddr};

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

const MAX_PACKET_ENABLE_BIT: u32 = 1 << 16;
const MAX_PACKET_MASK: u32 = 0xffff;
const DELAY_MASK: u32 = 0x00ff_ffff;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeDeviceConfig {
    pub bind: SocketAddr,
    pub interface_name: Option<String>,
    pub advertise_address: Option<Ipv4Addr>,
    pub stream_name: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub transmit_pixel_format: Option<PixelFormat>,
    pub fps: u32,
    pub mtu: u16,
}

impl Default for FakeDeviceConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DISCOVERY_PORT),
            interface_name: None,
            advertise_address: None,
            stream_name: "stream0".to_string(),
            width: 1280,
            height: 720,
            pixel_format: PixelFormat::Uyvy,
            transmit_pixel_format: None,
            fps: 30,
            mtu: 1200,
        }
    }
}

impl FakeDeviceConfig {
    pub fn effective_transmit_pixel_format(&self) -> PixelFormat {
        self.transmit_pixel_format.unwrap_or(self.pixel_format)
    }

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
        if self.effective_transmit_pixel_format() == PixelFormat::Uyvy && self.width % 2 != 0 {
            bail!("UYVY test-pattern width must be even");
        }
        CompatPacketizer::new(self.mtu as usize)
            .with_context(|| format!("invalid fake-device mtu {}", self.mtu))?;
        self.effective_transmit_pixel_format()
            .payload_len(self.width, self.height)
            .context("invalid fake-device frame dimensions")?;
        Ok(())
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
    next_frame_id: u32,
}

#[derive(Clone, Copy, Debug)]
struct SenderSettings {
    destination_ip: Ipv4Addr,
    destination_port: u16,
    mtu: usize,
    packet_delay_ns: u32,
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    frame_id: u32,
}

pub struct FakeDeviceServer {
    local_addr: SocketAddr,
    uri: String,
    stop: Arc<AtomicBool>,
    start_count: Arc<AtomicUsize>,
    stop_count: Arc<AtomicUsize>,
    state: Arc<(Mutex<DeviceState>, Condvar)>,
    control_join: Option<JoinHandle<()>>,
    sender_join: Option<JoinHandle<()>>,
}

#[derive(Debug, Parser)]
#[command(name = "eefakedev", about = "Fake EEVideo device daemon with a pure-Rust test-pattern source")]
struct Cli {
    #[arg(long, default_value = "0.0.0.0:5683")]
    bind: SocketAddr,
    #[arg(long)]
    iface: Option<String>,
    #[arg(long)]
    advertise_address: Option<Ipv4Addr>,
    #[arg(long, default_value = "stream0")]
    stream_name: String,
    #[arg(long, default_value_t = 1280)]
    width: u32,
    #[arg(long, default_value_t = 720)]
    height: u32,
    #[arg(long, default_value = "uyvy", value_parser = parse_pixel_format)]
    pixel_format: PixelFormat,
    #[arg(long, default_value_t = 30)]
    fps: u32,
    #[arg(long, default_value_t = 1200)]
    mtu: u16,
}

impl From<Cli> for FakeDeviceConfig {
    fn from(value: Cli) -> Self {
        Self {
            bind: value.bind,
            interface_name: value.iface,
            advertise_address: value.advertise_address,
            stream_name: value.stream_name,
            width: value.width,
            height: value.height,
            pixel_format: value.pixel_format,
            transmit_pixel_format: None,
            fps: value.fps,
            mtu: value.mtu,
        }
    }
}

fn parse_pixel_format(value: &str) -> Result<PixelFormat, String> {
    match value.to_ascii_lowercase().as_str() {
        "mono8" | "gray8" => Ok(PixelFormat::Mono8),
        "mono16" | "gray16" | "gray16_le" => Ok(PixelFormat::Mono16),
        "rgb" | "rgb8" => Ok(PixelFormat::Rgb8),
        "uyvy" => Ok(PixelFormat::Uyvy),
        "bayergr8" | "grbg" => Ok(PixelFormat::BayerGr8),
        "bayerrg8" | "rggb" => Ok(PixelFormat::BayerRg8),
        "bayergb8" | "gbrg" => Ok(PixelFormat::BayerGb8),
        "bayerbg8" | "bggr" => Ok(PixelFormat::BayerBg8),
        _ => Err(format!("unsupported pixel format {value}")),
    }
}

pub fn main_entry<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    run(cli.into())
}

pub fn run(config: FakeDeviceConfig) -> Result<()> {
    let server = FakeDeviceServer::spawn(config)?;
    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .context("failed to install Ctrl+C handler")?;

    println!(
        "fake EEVideo device listening at {} advertising {}",
        server.local_addr(),
        server.uri()
    );
    println!("press Ctrl+C to stop");

    let _ = rx.recv();
    drop(server);
    Ok(())
}

impl FakeDeviceServer {
    pub fn spawn(config: FakeDeviceConfig) -> Result<Self> {
        config.validate()?;
        let selected_interface = select_interface(&config)?;
        let socket = UdpSocket::bind(config.bind)
            .with_context(|| format!("failed to bind fake-device socket at {}", config.bind))?;
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .context("failed to configure fake-device socket timeout")?;
        maybe_join_discovery_multicast(&socket, &selected_interface)?;

        let local_addr = socket.local_addr().context("failed to read fake-device local address")?;
        let state = Arc::new((
            Mutex::new(DeviceState {
                registers: build_registers(&config),
                strings: build_strings(&config),
                next_frame_id: 1,
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
            SocketAddr::new(IpAddr::V4(advertised_ip(&config, &local_addr)?), local_addr.port())
        );
        let sender_join = Some(spawn_sender_loop(
            config,
            selected_interface,
            Arc::clone(&state),
            Arc::clone(&stop),
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
        state.lock().expect("device state lock poisoned").registers.clone()
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

impl Drop for FakeDeviceServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn build_strings(config: &FakeDeviceConfig) -> BTreeMap<u32, Vec<u8>> {
    BTreeMap::from([(
        STREAM_DESC_ADDR,
        {
            let mut value = format!("EEVideo {}", config.stream_name).into_bytes();
            value.push(0);
            value
        },
    )])
}

fn build_registers(config: &FakeDeviceConfig) -> BTreeMap<u32, u32> {
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
    registers.insert(STREAM_PIXEL_FORMAT_ADDR, config.pixel_format.pfnc() & 0xffff);
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

fn select_interface(config: &FakeDeviceConfig) -> Result<SelectedInterface> {
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

fn advertised_ip(config: &FakeDeviceConfig, local_addr: &SocketAddr) -> Result<Ipv4Addr> {
    if let Some(ip) = config.advertise_address {
        return Ok(ip);
    }
    match local_addr.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => Ok(ip),
        IpAddr::V4(_) => select_interface(config).map(|interface| interface.address),
        IpAddr::V6(_) => bail!("fake device currently only supports IPv4"),
    }
}

fn spawn_control_loop(
    socket: UdpSocket,
    config: FakeDeviceConfig,
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
                handle_register_request(&request, &state, &start_count, &stop_count)
            };

            if let Some(response) = response {
                if let Ok(bytes) = response.encode() {
                    let _ = socket.send_to(&bytes, peer);
                }
            }
        }
    })
}

fn spawn_sender_loop(
    config: FakeDeviceConfig,
    interface: SelectedInterface,
    state: Arc<(Mutex<DeviceState>, Condvar)>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let sender_bind = SocketAddr::new(IpAddr::V4(interface.address), 0);
        let socket = UdpSocket::bind(sender_bind)
            .or_else(|_| UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)));
        let Ok(socket) = socket else {
            return;
        };

        let frame_interval = Duration::from_nanos(1_000_000_000u64 / u64::from(config.fps));
        let mut scratch = Vec::new();
        let start = Instant::now();

        while !stop.load(Ordering::Relaxed) {
            let settings = match wait_for_stream_settings(&state, &stop, &config) {
                Some(settings) => settings,
                None => break,
            };

            if settings.destination_port == 0 {
                thread::sleep(Duration::from_millis(20));
                continue;
            }

            let frame = match build_test_pattern_frame(
                &config,
                settings.frame_id,
                start.elapsed(),
                settings.width,
                settings.height,
                settings.pixel_format,
            ) {
                Ok(frame) => frame,
                Err(_) => {
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
            };
            let packetizer = match CompatPacketizer::new(settings.mtu) {
                Ok(packetizer) => packetizer,
                Err(_) => {
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
            };
            let destination = SocketAddrV4::new(settings.destination_ip, settings.destination_port);
            let _ = packetizer.emit_packets((&frame).into(), &mut scratch, |packet| {
                socket.send_to(packet, destination)?;
                if settings.packet_delay_ns > 0 {
                    thread::sleep(Duration::from_nanos(u64::from(settings.packet_delay_ns)));
                }
                Ok::<(), std::io::Error>(())
            });

            thread::sleep(frame_interval);
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
        option.number == 15 && option.value.as_slice() == format!("rt={DISCOVERY_RESOURCE_TYPE}").as_bytes()
    });

    path == [".well-known", "core"] && query_matches
}

fn build_discovery_response(
    request: &CoapMessage,
    config: &FakeDeviceConfig,
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
        let payload = if reg_access.map(|value| value >> 5) == Some(RegisterReadKind::String as u8) {
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

    let new_value = u32::from_be_bytes(request.payload.clone().try_into().ok()?);
    let old_value = state.registers.get(&address).copied().unwrap_or_default();
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

fn wait_for_stream_settings(
    state: &Arc<(Mutex<DeviceState>, Condvar)>,
    stop: &AtomicBool,
    config: &FakeDeviceConfig,
) -> Option<SenderSettings> {
    let (lock, cvar) = &**state;
    let mut guard = lock.lock().ok()?;
    while !stop.load(Ordering::Relaxed) {
        if stream_enabled(&guard.registers) {
            let destination_port =
                (guard.registers.get(&STREAM_DEST_PORT_ADDR).copied().unwrap_or_default() & 0xffff)
                    as u16;
            let destination_ip =
                Ipv4Addr::from(guard.registers.get(&STREAM_DEST_IP_ADDR).copied().unwrap_or_default());
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
            let pixel_format = config
                .transmit_pixel_format
                .or_else(|| pixel_format_from_device_bits(advertised_bits))
                .unwrap_or(config.pixel_format);
            let frame_id = guard.next_frame_id;
            guard.next_frame_id = guard.next_frame_id.wrapping_add(1).max(1);

            return Some(SenderSettings {
                destination_ip,
                destination_port,
                mtu,
                packet_delay_ns,
                width,
                height,
                pixel_format,
                frame_id,
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
    if value == 0 { default } else { value }
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

fn build_test_pattern_frame(
    config: &FakeDeviceConfig,
    frame_id: u32,
    elapsed: Duration,
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
) -> Result<VideoFrame> {
    Ok(VideoFrame {
        frame_id,
        timestamp: elapsed.as_nanos().min(u128::from(u64::MAX)) as u64,
        width,
        height,
        pixel_format,
        payload_type: PayloadType::Image,
        data: generate_pattern_data(config, frame_id, width, height, pixel_format)?,
    })
}

fn generate_pattern_data(
    _config: &FakeDeviceConfig,
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
        build_discovery_response, build_registers, build_test_pattern_frame, parse_pixel_format,
        FakeDeviceConfig, FakeDeviceServer, SelectedInterface, MAX_PACKET_ENABLE_BIT,
        STREAM_DEST_PORT_ADDR, STREAM_MAX_PACKET_ADDR,
    };
    use eevideo_control::discovery::parse_discovery_advertisement;
    use eevideo_control::register::{RegisterClient, RegisterError};
    use eevideo_control::{CoapMessage, CoapMessageType, CoapOption};
    use eevideo_proto::{PayloadType, PixelFormat};
    use std::net::Ipv4Addr;
    use std::time::{Duration, Instant};

    fn write_u32_eventually(
        client: &RegisterClient,
        address: u32,
        value: u32,
        timeout: Duration,
    ) -> Result<(), RegisterError> {
        let deadline = Instant::now() + timeout;

        loop {
            match client.write_u32(address, value) {
                Ok(()) => return Ok(()),
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(err) => return Err(err),
            }
        }
    }

    fn wait_until(
        timeout: Duration,
        mut predicate: impl FnMut() -> bool,
        description: &str,
    ) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if predicate() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(predicate(), "{description}");
    }

    #[test]
    fn register_map_uses_expected_defaults() {
        let registers = build_registers(&FakeDeviceConfig::default());
        assert_eq!(registers.get(&STREAM_MAX_PACKET_ADDR).copied(), Some(1200));
        assert_eq!(registers.get(&STREAM_DEST_PORT_ADDR).copied(), Some(0));
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
            &FakeDeviceConfig::default(),
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
    fn enable_bit_transitions_start_and_stop_counts() {
        let device = FakeDeviceServer::spawn(FakeDeviceConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            width: 32,
            height: 16,
            pixel_format: PixelFormat::Mono8,
            ..FakeDeviceConfig::default()
        })
        .unwrap();
        let client = RegisterClient::new("127.0.0.1:0".parse().unwrap(), device.local_addr())
            .with_timeout(Duration::from_millis(250));

        write_u32_eventually(
            &client,
            STREAM_MAX_PACKET_ADDR,
            MAX_PACKET_ENABLE_BIT | 1200,
            Duration::from_secs(2),
        )
        .unwrap();
        wait_until(
            Duration::from_secs(1),
            || device.start_count() == 1,
            "fake device never observed the stream start transition",
        );

        write_u32_eventually(&client, STREAM_MAX_PACKET_ADDR, 1200, Duration::from_secs(2))
            .unwrap();
        wait_until(
            Duration::from_secs(1),
            || device.stop_count() == 1,
            "fake device never observed the stream stop transition",
        );

        assert_eq!(device.start_count(), 1);
        assert_eq!(device.stop_count(), 1);
    }

    #[test]
    fn uyvy_test_pattern_has_expected_payload_length() {
        let frame = build_test_pattern_frame(
            &FakeDeviceConfig::default(),
            1,
            Duration::from_millis(10),
            1280,
            720,
            PixelFormat::Uyvy,
        )
        .unwrap();

        assert_eq!(frame.payload_type, PayloadType::Image);
        assert_eq!(frame.data.len(), 1280 * 720 * 2);
    }

    #[test]
    fn parses_supported_pixel_formats() {
        assert_eq!(parse_pixel_format("uyvy").unwrap(), PixelFormat::Uyvy);
        assert_eq!(parse_pixel_format("gray8").unwrap(), PixelFormat::Mono8);
    }
}

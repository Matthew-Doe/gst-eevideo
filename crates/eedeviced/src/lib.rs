use std::ffi::OsString;
use std::net::{Ipv4Addr, SocketAddr};

use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};
use eevideo_device::{DeviceRuntime, DeviceRuntimeConfig};
use eevideo_proto::PixelFormat;

mod providers;

pub use providers::ProviderConfig;
use providers::{build_capture_backend, validate_provider_config};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum InputKind {
    Synthetic,
    Argus,
    V4l2,
    Pipeline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
#[value(rename_all = "kebab-case")]
enum CliPixelFormat {
    #[value(alias = "gray8", alias = "mono")]
    Mono8,
    #[value(alias = "gray16le")]
    Mono16,
    BayerGr8,
    BayerRg8,
    BayerGb8,
    BayerBg8,
    Uyvy,
}

impl From<CliPixelFormat> for PixelFormat {
    fn from(value: CliPixelFormat) -> Self {
        match value {
            CliPixelFormat::Mono8 => PixelFormat::Mono8,
            CliPixelFormat::Mono16 => PixelFormat::Mono16,
            CliPixelFormat::BayerGr8 => PixelFormat::BayerGr8,
            CliPixelFormat::BayerRg8 => PixelFormat::BayerRg8,
            CliPixelFormat::BayerGb8 => PixelFormat::BayerGb8,
            CliPixelFormat::BayerBg8 => PixelFormat::BayerBg8,
            CliPixelFormat::Uyvy => PixelFormat::Uyvy,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceDaemonConfig {
    pub bind: SocketAddr,
    pub interface_name: Option<String>,
    pub advertise_address: Option<Ipv4Addr>,
    pub stream_name: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub fps: u32,
    pub mtu: u16,
    pub provider: ProviderConfig,
}

impl Default for DeviceDaemonConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:5683".parse().expect("static socket address"),
            interface_name: None,
            advertise_address: None,
            stream_name: "stream0".to_string(),
            width: 1280,
            height: 720,
            pixel_format: PixelFormat::Uyvy,
            fps: 30,
            mtu: 1200,
            provider: ProviderConfig::Synthetic,
        }
    }
}

impl DeviceDaemonConfig {
    pub fn runtime_config(&self) -> DeviceRuntimeConfig {
        DeviceRuntimeConfig {
            bind: self.bind,
            interface_name: self.interface_name.clone(),
            advertise_address: self.advertise_address,
            stream_name: self.stream_name.clone(),
            width: self.width,
            height: self.height,
            pixel_format: self.pixel_format,
            fps: self.fps,
            mtu: self.mtu,
            enforce_fixed_format: true,
        }
    }

    #[cfg(test)]
    fn capture_configuration(&self) -> eevideo_device::CaptureConfiguration {
        eevideo_device::CaptureConfiguration {
            width: self.width,
            height: self.height,
            pixel_format: self.pixel_format,
            fps: self.fps,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "eedeviced", about = "Single-stream EEVideo device daemon")]
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
    #[arg(long, default_value = "uyvy")]
    pixel_format: CliPixelFormat,
    #[arg(long, default_value_t = 30)]
    fps: u32,
    #[arg(long, default_value_t = 1200)]
    mtu: u16,
    #[arg(long, default_value = "synthetic")]
    input: InputKind,
    #[arg(long)]
    sensor_id: Option<u32>,
    #[arg(long)]
    device: Option<String>,
    #[arg(long)]
    pipeline: Option<String>,
}

impl TryFrom<Cli> for DeviceDaemonConfig {
    type Error = anyhow::Error;

    fn try_from(value: Cli) -> Result<Self, Self::Error> {
        let provider = match value.input {
            InputKind::Synthetic => {
                reject_unused_cli_options(
                    value.sensor_id,
                    value.device.as_deref(),
                    value.pipeline.as_deref(),
                )?;
                ProviderConfig::Synthetic
            }
            InputKind::Argus => {
                reject_unexpected_option("device", value.device.as_deref())?;
                reject_unexpected_option("pipeline", value.pipeline.as_deref())?;
                ProviderConfig::Argus {
                    sensor_id: value.sensor_id.unwrap_or(0),
                }
            }
            InputKind::V4l2 => {
                reject_unexpected_option(
                    "sensor-id",
                    value.sensor_id.map(|id| id.to_string()).as_deref(),
                )?;
                reject_unexpected_option("pipeline", value.pipeline.as_deref())?;
                ProviderConfig::V4l2 {
                    device: require_cli_option("device", value.device)?,
                }
            }
            InputKind::Pipeline => {
                reject_unexpected_option(
                    "sensor-id",
                    value.sensor_id.map(|id| id.to_string()).as_deref(),
                )?;
                reject_unexpected_option("device", value.device.as_deref())?;
                ProviderConfig::Pipeline {
                    description: require_cli_option("pipeline", value.pipeline)?,
                }
            }
        };

        Ok(Self {
            bind: value.bind,
            interface_name: value.iface,
            advertise_address: value.advertise_address,
            stream_name: value.stream_name,
            width: value.width,
            height: value.height,
            pixel_format: value.pixel_format.into(),
            fps: value.fps,
            mtu: value.mtu,
            provider,
        })
    }
}

pub struct DeviceDaemon {
    runtime: DeviceRuntime,
}

impl std::fmt::Debug for DeviceDaemon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceDaemon")
            .field("local_addr", &self.local_addr())
            .field("uri", &self.uri())
            .finish()
    }
}

impl DeviceDaemon {
    pub fn spawn(config: DeviceDaemonConfig) -> Result<Self> {
        validate_config(&config)?;
        let runtime =
            DeviceRuntime::spawn(config.runtime_config(), build_capture_backend(&config))?;
        Ok(Self { runtime })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.runtime.local_addr()
    }

    pub fn uri(&self) -> String {
        self.runtime.uri()
    }

    pub fn shutdown(&mut self) {
        self.runtime.shutdown();
    }
}

pub fn main_entry<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    run(DeviceDaemonConfig::try_from(cli)?)
}

pub fn run(config: DeviceDaemonConfig) -> Result<()> {
    let device = DeviceDaemon::spawn(config)?;
    let (tx, rx) = std::sync::mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })?;

    println!(
        "EEVideo device listening at {} advertising {}",
        device.local_addr(),
        device.uri()
    );
    println!("press Ctrl+C to stop");

    let _ = rx.recv();
    drop(device);
    Ok(())
}

fn validate_config(config: &DeviceDaemonConfig) -> Result<()> {
    if config.width == 0 || config.height == 0 {
        bail!("frame size must be non-zero");
    }
    if config.fps == 0 {
        bail!("fps must be greater than zero");
    }
    config
        .pixel_format
        .payload_len(config.width, config.height)
        .map_err(anyhow::Error::from)?;
    if config.pixel_format == PixelFormat::Uyvy && config.width % 2 != 0 {
        bail!("UYVY device width must be even");
    }
    validate_provider_config(config)
}

fn reject_unused_cli_options(
    sensor_id: Option<u32>,
    device: Option<&str>,
    pipeline: Option<&str>,
) -> Result<()> {
    reject_unexpected_option("sensor-id", sensor_id.map(|id| id.to_string()).as_deref())?;
    reject_unexpected_option("device", device)?;
    reject_unexpected_option("pipeline", pipeline)?;
    Ok(())
}

fn reject_unexpected_option(name: &str, value: Option<&str>) -> Result<()> {
    if let Some(value) = value {
        if !value.trim().is_empty() {
            bail!("--{name} is only valid for its matching provider");
        }
    }
    Ok(())
}

fn require_cli_option(name: &str, value: Option<String>) -> Result<String> {
    let Some(value) = value else {
        bail!("--{name} is required for the selected provider");
    };
    if value.trim().is_empty() {
        bail!("--{name} must not be empty");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::net::UdpSocket;
    use std::time::{Duration, Instant};

    use clap::Parser;
    use eevideo_control::backend::{CoapRegisterBackend, CoapRegisterBackendConfig};
    use eevideo_control::{
        ControlBackend, ControlTarget, ControlTransportKind, RequestedStreamConfiguration,
        StreamFormatDescriptor,
    };
    use eevideo_proto::{
        CompatPacket, FrameAssembler, FrameEvent, PayloadType, PixelFormat, StreamProfileId,
        StreamStats,
    };

    use eevideo_device::CaptureConfiguration;

    use super::{providers, Cli, DeviceDaemon, DeviceDaemonConfig, ProviderConfig};

    #[test]
    fn parses_pixel_format_aliases() {
        let cli = Cli::try_parse_from(["eedeviced", "--pixel-format", "gray8"]).unwrap();
        let config = DeviceDaemonConfig::try_from(cli).unwrap();
        assert_eq!(config.pixel_format, PixelFormat::Mono8);

        let cli = Cli::try_parse_from(["eedeviced", "--pixel-format", "mono"]).unwrap();
        let config = DeviceDaemonConfig::try_from(cli).unwrap();
        assert_eq!(config.pixel_format, PixelFormat::Mono8);

        let cli = Cli::try_parse_from(["eedeviced", "--pixel-format", "gray16le"]).unwrap();
        let config = DeviceDaemonConfig::try_from(cli).unwrap();
        assert_eq!(config.pixel_format, PixelFormat::Mono16);
    }

    #[test]
    fn argus_pipeline_description_uses_expected_elements() {
        let description = providers::build_argus_pipeline_description(
            2,
            &DeviceDaemonConfig {
                width: 1280,
                height: 720,
                fps: 30,
                pixel_format: PixelFormat::Uyvy,
                provider: ProviderConfig::Argus { sensor_id: 2 },
                ..DeviceDaemonConfig::default()
            }
            .capture_configuration(),
        );

        assert!(description.contains("nvarguscamerasrc sensor-id=2"));
        assert!(
            description.contains("video/x-raw(memory:NVMM),width=1280,height=720,framerate=30/1")
        );
        assert!(description.contains("nvvidconv"));
        assert!(description.contains("video/x-raw,format=UYVY,width=1280,height=720"));
        assert!(description.contains("appsink name=framesink"));
    }

    #[test]
    fn v4l2_pipeline_description_requests_configured_caps() {
        let description = providers::build_v4l2_pipeline_description(
            "/dev/video0",
            &DeviceDaemonConfig {
                width: 640,
                height: 480,
                fps: 60,
                pixel_format: PixelFormat::Mono16,
                provider: ProviderConfig::V4l2 {
                    device: "/dev/video0".to_string(),
                },
                ..DeviceDaemonConfig::default()
            }
            .capture_configuration(),
        );

        assert!(description.contains("v4l2src device=/dev/video0"));
        assert!(description
            .contains("video/x-raw,format=GRAY16_LE,width=640,height=480,framerate=60/1"));
        assert!(description.contains("appsink name=framesink"));
    }

    #[test]
    fn supports_non_uyvy_odd_width_formats() {
        let device = DeviceDaemon::spawn(DeviceDaemonConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            width: 31,
            height: 16,
            pixel_format: PixelFormat::Mono8,
            provider: ProviderConfig::Synthetic,
            ..DeviceDaemonConfig::default()
        })
        .unwrap();

        assert!(device.uri().starts_with("coap://127.0.0.1:"));
    }

    #[test]
    fn rejects_uyvy_odd_width() {
        let error = DeviceDaemon::spawn(DeviceDaemonConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            width: 31,
            height: 16,
            pixel_format: PixelFormat::Uyvy,
            provider: ProviderConfig::Synthetic,
            ..DeviceDaemonConfig::default()
        })
        .unwrap_err();

        assert!(error.to_string().contains("UYVY device width must be even"));
    }

    #[test]
    fn argus_rejects_non_uyvy_formats() {
        let error = DeviceDaemon::spawn(DeviceDaemonConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            width: 32,
            height: 16,
            pixel_format: PixelFormat::Mono8,
            provider: ProviderConfig::Argus { sensor_id: 0 },
            ..DeviceDaemonConfig::default()
        })
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("argus provider only supports UYVY output"));
    }

    #[test]
    fn pipeline_provider_requires_framesink() {
        let mut backend =
            providers::GstreamerCaptureBackend::new(providers::GstreamerProviderConfig::Pipeline {
                description: "videotestsrc num-buffers=1 ! fakesink".to_string(),
            });
        let error = providers::start_backend_for_test(
            &mut backend,
            CaptureConfiguration {
                width: 32,
                height: 16,
                pixel_format: PixelFormat::Mono8,
                fps: 30,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("framesink"));
    }

    #[test]
    fn caps_mapping_supports_requested_formats() {
        providers::ensure_gstreamer_init_for_tests().unwrap();

        let caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", "UYVY")
            .field("width", 32i32)
            .field("height", 16i32)
            .build();
        assert_eq!(
            providers::capture_format_from_caps(caps.as_ref(), 30)
                .unwrap()
                .pixel_format,
            PixelFormat::Uyvy
        );

        let caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", "GRAY8")
            .field("width", 32i32)
            .field("height", 16i32)
            .build();
        assert_eq!(
            providers::capture_format_from_caps(caps.as_ref(), 30)
                .unwrap()
                .pixel_format,
            PixelFormat::Mono8
        );

        let caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", "GRAY16_LE")
            .field("width", 32i32)
            .field("height", 16i32)
            .build();
        assert_eq!(
            providers::capture_format_from_caps(caps.as_ref(), 30)
                .unwrap()
                .pixel_format,
            PixelFormat::Mono16
        );

        let caps = gstreamer::Caps::builder("video/x-bayer")
            .field("format", "bggr")
            .field("width", 32i32)
            .field("height", 16i32)
            .build();
        assert_eq!(
            providers::capture_format_from_caps(caps.as_ref(), 30)
                .unwrap()
                .pixel_format,
            PixelFormat::BayerBg8
        );
    }

    #[test]
    fn packed_buffer_validation_rejects_mismatches() {
        let error = providers::validate_packed_buffer_len(
            &CaptureConfiguration {
                width: 32,
                height: 16,
                pixel_format: PixelFormat::Mono16,
                fps: 30,
            },
            32 * 16,
        )
        .unwrap_err();

        assert!(error.to_string().contains("payload length mismatch"));
    }

    #[test]
    fn synthetic_provider_streams_configured_fixed_formats() {
        for format in [
            PixelFormat::Mono8,
            PixelFormat::Mono16,
            PixelFormat::BayerBg8,
            PixelFormat::Uyvy,
        ] {
            let width = if format == PixelFormat::Uyvy { 32 } else { 31 };
            let height = 16;
            let mut device = DeviceDaemon::spawn(DeviceDaemonConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                width,
                height,
                pixel_format: format,
                provider: ProviderConfig::Synthetic,
                ..DeviceDaemonConfig::default()
            })
            .unwrap();

            let receive = UdpSocket::bind("127.0.0.1:0").unwrap();
            receive
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            let port = receive.local_addr().unwrap().port();

            let backend = CoapRegisterBackend::new(CoapRegisterBackendConfig {
                request_timeout: Duration::from_millis(250),
                ..CoapRegisterBackendConfig::default()
            });
            let target = ControlTarget {
                device_uri: device.uri(),
                transport_kind: ControlTransportKind::CoapRegister,
                auth_scope: None,
            };
            let mut connection = backend.connect(&target).unwrap();
            let applied = connection
                .configure(RequestedStreamConfiguration {
                    stream_name: "stream0".to_string(),
                    profile: StreamProfileId::CompatibilityV1,
                    destination_host: "127.0.0.1".to_string(),
                    port,
                    bind_address: "127.0.0.1".to_string(),
                    packet_delay_ns: 0,
                    max_packet_size: 1200,
                    format: Some(StreamFormatDescriptor {
                        payload_type: PayloadType::Image,
                        pixel_format: format,
                        width,
                        height,
                    }),
                })
                .unwrap();

            assert_eq!(applied.format.unwrap().pixel_format, format);
            connection.start(&applied.stream_id).unwrap();
            let frame = receive_frame(&receive, Duration::from_secs(3));
            assert_eq!(frame.pixel_format, format);
            assert_eq!(frame.width, width);
            assert_eq!(frame.height, height);

            device.shutdown();
        }
    }

    #[test]
    fn cli_maps_provider_specific_options() {
        let cli = Cli::try_parse_from([
            "eedeviced",
            "--input",
            "v4l2",
            "--device",
            "/dev/video0",
            "--pixel-format",
            "gray16le",
        ])
        .unwrap();
        let config = DeviceDaemonConfig::try_from(cli).unwrap();

        assert_eq!(
            config.provider,
            ProviderConfig::V4l2 {
                device: "/dev/video0".to_string()
            }
        );
        assert_eq!(config.pixel_format, PixelFormat::Mono16);
    }

    #[test]
    fn legacy_input_enum_still_parses_synthetic_and_argus() {
        let cli = Cli::try_parse_from(["eedeviced", "--input", "synthetic"]).unwrap();
        let config = DeviceDaemonConfig::try_from(cli).unwrap();
        assert_eq!(config.provider, ProviderConfig::Synthetic);

        let cli = Cli::try_parse_from(["eedeviced", "--input", "argus"]).unwrap();
        let config = DeviceDaemonConfig::try_from(cli).unwrap();
        assert_eq!(config.provider, ProviderConfig::Argus { sensor_id: 0 });
    }

    fn receive_frame(socket: &UdpSocket, timeout: Duration) -> eevideo_proto::VideoFrame {
        let deadline = Instant::now() + timeout;
        let mut assembler = FrameAssembler::new(Duration::from_secs(1));
        let stats = StreamStats::default();
        let mut buffer = [0u8; 2048];

        loop {
            if Instant::now() >= deadline {
                panic!("timed out waiting for a frame");
            }

            let size = match socket.recv(&mut buffer) {
                Ok(size) => size,
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    continue;
                }
                Err(err) => panic!("failed to receive frame packet: {err}"),
            };

            let packet = CompatPacket::parse(&buffer[..size]).unwrap();
            if let Some(FrameEvent::Complete(frame)) =
                assembler.ingest(packet, Instant::now(), &stats).unwrap()
            {
                return frame;
            }
        }
    }
}

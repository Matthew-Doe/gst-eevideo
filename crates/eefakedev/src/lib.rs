use std::collections::BTreeMap;
use std::ffi::OsString;
use std::net::{Ipv4Addr, SocketAddr};

use anyhow::{bail, Result};
use clap::Parser;
use eevideo_device::{
    DeviceRuntime, DeviceRuntimeConfig, SyntheticCaptureBackend, SyntheticCaptureConfig,
};
use eevideo_proto::PixelFormat;

const CLI_AFTER_LONG_HELP: &str = "\
Examples:
  eefakedev --advertise-address 192.168.1.50 --pixel-format gray8 --width 640 --height 480
  eevid describe --device-uri coap://192.168.1.50:5683
";

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
            bind: "0.0.0.0:5683".parse().expect("static socket address"),
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
        Ok(())
    }

    fn runtime_config(&self) -> DeviceRuntimeConfig {
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
            enforce_fixed_format: false,
        }
    }

    fn capture_config(&self) -> SyntheticCaptureConfig {
        SyntheticCaptureConfig {
            transmit_pixel_format: self.transmit_pixel_format,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "eefakedev",
    about = "Fake EEVideo device daemon with a pure-Rust test-pattern source",
    after_long_help = CLI_AFTER_LONG_HELP
)]
struct Cli {
    #[arg(
        long,
        default_value = "0.0.0.0:5683",
        help = "Bind address for discovery and register control."
    )]
    bind: SocketAddr,
    #[arg(
        long,
        help = "Prefer a specific local interface for discovery replies."
    )]
    iface: Option<String>,
    #[arg(
        long,
        help = "IPv4 address advertised to remote hosts instead of the bind address."
    )]
    advertise_address: Option<Ipv4Addr>,
    #[arg(
        long,
        default_value = "stream0",
        help = "Name of the single advertised stream."
    )]
    stream_name: String,
    #[arg(
        long,
        default_value_t = 1280,
        help = "Synthetic frame width in pixels."
    )]
    width: u32,
    #[arg(
        long,
        default_value_t = 720,
        help = "Synthetic frame height in pixels."
    )]
    height: u32,
    #[arg(
        long,
        default_value = "uyvy",
        value_parser = parse_pixel_format,
        help = "Synthetic pixel format to advertise and transmit.",
        long_help = "Synthetic pixel format to advertise and transmit. Supported aliases include gray8, gray16, rgb8, grbg, and bggr."
    )]
    pixel_format: PixelFormat,
    #[arg(
        long,
        default_value_t = 30,
        help = "Synthetic frame rate in frames per second."
    )]
    fps: u32,
    #[arg(
        long,
        default_value_t = 1200,
        help = "Maximum UDP payload size advertised to the host."
    )]
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

pub struct FakeDeviceServer {
    runtime: DeviceRuntime,
}

impl FakeDeviceServer {
    pub fn spawn(config: FakeDeviceConfig) -> Result<Self> {
        config.validate()?;
        let runtime = DeviceRuntime::spawn(
            config.runtime_config(),
            SyntheticCaptureBackend::new(config.capture_config()),
        )?;
        Ok(Self { runtime })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.runtime.local_addr()
    }

    pub fn uri(&self) -> String {
        self.runtime.uri()
    }

    pub fn start_count(&self) -> usize {
        self.runtime.start_count()
    }

    pub fn stop_count(&self) -> usize {
        self.runtime.stop_count()
    }

    pub fn registers(&self) -> BTreeMap<u32, u32> {
        self.runtime.registers()
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
    let cli = Cli::parse_from(normalize_help_flag_punctuation(args));
    run(cli.into())
}

fn normalize_help_flag_punctuation<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    args.into_iter()
        .map(Into::into)
        .map(|arg: OsString| match arg.to_str() {
            Some("--help,") => OsString::from("--help"),
            Some("-h,") => OsString::from("-h"),
            _ => arg,
        })
        .collect()
}

pub fn run(config: FakeDeviceConfig) -> Result<()> {
    let server = FakeDeviceServer::spawn(config)?;
    let (tx, rx) = std::sync::mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })?;

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

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{parse_pixel_format, FakeDeviceConfig, FakeDeviceServer};
    use clap::CommandFactory;
    use eevideo_control::register::{RegisterClient, RegisterError};
    use eevideo_device::{MAX_PACKET_ENABLE_BIT, STREAM_MAX_PACKET_ADDR};
    use eevideo_proto::PixelFormat;
    use std::time::{Duration, Instant};

    fn render_long_help(mut command: clap::Command) -> String {
        let mut output = Vec::new();
        command.write_long_help(&mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

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

    fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool, description: &str) {
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

        write_u32_eventually(
            &client,
            STREAM_MAX_PACKET_ADDR,
            1200,
            Duration::from_secs(2),
        )
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
    fn top_level_help_mentions_examples_and_pixel_aliases() {
        let help = render_long_help(super::Cli::command());

        assert!(help.contains("Examples:"));
        assert!(help.contains("eevid describe"));
        assert!(help.contains("Supported aliases include gray8, gray16, rgb8, grbg, and bggr."));
    }

    #[test]
    fn normalizes_help_flags_with_trailing_commas() {
        let args = super::normalize_help_flag_punctuation([
            OsString::from("eefakedev"),
            OsString::from("--help,"),
            OsString::from("-h,"),
            OsString::from("--other,"),
        ]);

        assert_eq!(args[1], OsString::from("--help"));
        assert_eq!(args[2], OsString::from("-h"));
        assert_eq!(args[3], OsString::from("--other,"));
    }

    #[test]
    fn parses_supported_pixel_formats() {
        assert_eq!(parse_pixel_format("uyvy").unwrap(), PixelFormat::Uyvy);
        assert_eq!(parse_pixel_format("gray8").unwrap(), PixelFormat::Mono8);
    }
}

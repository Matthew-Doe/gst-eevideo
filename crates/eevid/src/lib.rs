use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use clap::{Args, Parser, Subcommand};
use eevideo_control::{
    CoapRegisterBackendConfig, ControlTarget, ControlTransportKind, DeviceController,
    RegisterSelector, RegisterValue, RequestedStreamConfiguration, StreamFormatDescriptor,
};
use eevideo_proto::{PayloadType, PixelFormat, StreamProfileId};

const CLI_AFTER_LONG_HELP: &str = "\
Examples:
  eevid discover
  eevid --device-uri coap://192.168.1.50:5683 describe
  eevid --device-uri coap://192.168.1.50:5683 reg-read --name stream0_MaxPacketSize
  eevid --device-uri coap://192.168.1.50:5683 stream-start --stream-name stream0 --destination-host 192.168.1.20 --port 5000 --bind-address 192.168.1.20 --max-packet-size 1200
";

const REG_READ_AFTER_LONG_HELP: &str = "\
Examples:
  eevid --device-uri coap://192.168.1.50:5683 reg-read --name stream0_MaxPacketSize
  eevid --device-uri coap://192.168.1.50:5683 reg-read --address 0x0018
";

const STREAM_START_AFTER_LONG_HELP: &str = "\
Examples:
  eevid --device-uri coap://192.168.1.50:5683 stream-start --stream-name stream0 --destination-host 192.168.1.20 --port 5000 --bind-address 192.168.1.20 --max-packet-size 1200
";

#[derive(Debug, Parser)]
#[command(
    name = "eevid",
    about = "EEVideo discovery and register control CLI",
    after_long_help = CLI_AFTER_LONG_HELP
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        help = "Target a single device directly and skip discovery."
    )]
    device_uri: Option<String>,
    #[arg(
        long,
        global = true,
        help = "Prefer a specific local interface for discovery and control traffic."
    )]
    iface: Option<String>,
    #[arg(
        long,
        global = true,
        default_value_t = 1000,
        help = "Set the discovery and request timeout in milliseconds."
    )]
    timeout_ms: u64,
    #[arg(
        long,
        global = true,
        default_value_t = 0,
        help = "Bind control traffic to a specific local UDP port."
    )]
    local_port: u16,
    #[arg(
        long,
        global = true,
        help = "Override the YAML metadata root used for symbolic register and field names."
    )]
    yaml_root: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Discover devices that answer the EEVideo CoAP/register control API.")]
    Discover,
    #[command(about = "Describe a single device, including streams, profiles, and registers.")]
    Describe,
    #[command(
        about = "Read a register by symbolic name or numeric address.",
        after_long_help = REG_READ_AFTER_LONG_HELP
    )]
    RegRead(RegisterSelectorArgs),
    #[command(about = "Write a raw register value by symbolic name or numeric address.")]
    RegWrite(RegisterWriteArgs),
    #[command(about = "Read a named bitfield from a register.")]
    FieldRead(FieldReadArgs),
    #[command(about = "Write a named bitfield inside a register.")]
    FieldWrite(FieldWriteArgs),
    #[command(
        about = "Apply stream destination and optional format settings without starting transmission."
    )]
    StreamConfigure(StreamArgs),
    #[command(
        about = "Start a stream toward a host-side receiver.",
        after_long_help = STREAM_START_AFTER_LONG_HELP
    )]
    StreamStart(StreamArgs),
    #[command(about = "Stop a previously configured stream.")]
    StreamStop(StreamStopArgs),
}

#[derive(Debug, Args)]
struct RegisterSelectorArgs {
    #[arg(
        long,
        conflicts_with = "address",
        help = "Use the YAML-backed symbolic register name."
    )]
    name: Option<String>,
    #[arg(
        long,
        value_parser = parse_u32_arg,
        conflicts_with = "name",
        help = "Use the numeric register address (decimal or 0x-prefixed hex)."
    )]
    address: Option<u32>,
}

#[derive(Debug, Args)]
struct RegisterWriteArgs {
    #[command(flatten)]
    selector: RegisterSelectorArgs,
    #[arg(
        value_parser = parse_u32_arg,
        help = "Value to write (decimal or 0x-prefixed hex)."
    )]
    value: u32,
}

#[derive(Debug, Args)]
struct FieldReadArgs {
    #[command(flatten)]
    selector: RegisterSelectorArgs,
    #[arg(long, help = "Bitfield name from the YAML register description.")]
    field: String,
}

#[derive(Debug, Args)]
struct FieldWriteArgs {
    #[command(flatten)]
    selector: RegisterSelectorArgs,
    #[arg(long, help = "Bitfield name from the YAML register description.")]
    field: String,
    #[arg(
        value_parser = parse_u32_arg,
        help = "Bitfield value to write (decimal or 0x-prefixed hex)."
    )]
    value: u32,
}

#[derive(Debug, Args, Clone)]
struct StreamArgs {
    #[arg(
        long,
        default_value = "stream0",
        help = "Advertised stream name to control."
    )]
    stream_name: String,
    #[arg(long, help = "IPv4 address or hostname of the host-side receiver.")]
    destination_host: String,
    #[arg(long, help = "UDP port used by the host-side receiver.")]
    port: u16,
    #[arg(
        long,
        default_value = "0.0.0.0",
        help = "Advertise the local receive address that the device should target."
    )]
    bind_address: String,
    #[arg(
        long,
        default_value_t = 0,
        help = "Insert a delay between transmitted packets in nanoseconds."
    )]
    packet_delay_ns: u64,
    #[arg(
        long,
        default_value_t = 1200,
        help = "Maximum UDP payload size the device should emit."
    )]
    max_packet_size: u16,
    #[arg(long, help = "Optional frame width override for configurable devices.")]
    width: Option<u32>,
    #[arg(
        long,
        help = "Optional frame height override for configurable devices."
    )]
    height: Option<u32>,
    #[arg(
        long,
        value_parser = parse_pixel_format_arg,
        help = "Optional pixel format override for configurable devices."
    )]
    pixel_format: Option<PixelFormat>,
}

#[derive(Debug, Args)]
struct StreamStopArgs {
    #[arg(
        long,
        default_value = "stream0",
        help = "Advertised stream name to stop."
    )]
    stream_name: String,
    #[arg(
        long,
        default_value = "0.0.0.0",
        help = "Receive address associated with the running stream configuration."
    )]
    bind_address: String,
}

pub fn main_entry<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(normalize_help_flag_punctuation(args));
    let output = run(cli)?;
    if !output.is_empty() {
        println!("{output}");
    }
    Ok(())
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

pub fn run(cli: Cli) -> Result<String> {
    let controller = controller(&cli);

    match cli.command {
        Command::Discover => {
            let devices = controller.discover(cli.device_uri.as_deref())?;
            if devices.is_empty() {
                return Ok("no devices found".to_string());
            }

            let mut output = String::new();
            for device in devices {
                writeln!(
                    output,
                    "{} {} {} {}",
                    device.target.device_uri,
                    device.interface_name,
                    device.interface_address,
                    device.device_address
                )
                .expect("string write");
            }
            Ok(output.trim_end().to_string())
        }
        Command::Describe => {
            let target = resolve_target(&controller, cli.device_uri.as_deref())?;
            let description = controller.describe(&target)?;
            let mut output = String::new();
            writeln!(
                output,
                "device-uri: {}",
                description.summary.target.device_uri
            )?;
            writeln!(
                output,
                "device-address: {}",
                description.summary.device_address
            )?;
            writeln!(output, "interface: {}", description.summary.interface_name)?;
            writeln!(
                output,
                "interface-address: {}",
                description.summary.interface_address
            )?;
            writeln!(
                output,
                "streams: {}",
                description
                    .streams
                    .iter()
                    .map(|stream| stream.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
            for stream in &description.streams {
                writeln!(output, "{}", format_advertised_stream(stream))?;
            }
            writeln!(
                output,
                "profiles: {}",
                description
                    .capabilities
                    .supported_profiles
                    .iter()
                    .map(profile_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
            writeln!(
                output,
                "pixel-formats: {}",
                description
                    .capabilities
                    .supported_pixel_formats
                    .iter()
                    .map(pixel_format_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
            for (name, register) in &description.device.registers {
                writeln!(
                    output,
                    "register {} addr=0x{:08x} access={}",
                    name, register.addr, register.access
                )?;
            }
            Ok(output.trim_end().to_string())
        }
        Command::RegRead(args) => {
            let target = resolve_target(&controller, cli.device_uri.as_deref())?;
            let value = controller.read_register(&target, None, &args.selector()?)?;
            Ok(format_register_value(&value))
        }
        Command::RegWrite(args) => {
            let target = resolve_target(&controller, cli.device_uri.as_deref())?;
            let selector = args.selector.selector()?;
            controller.write_register(&target, None, &selector, args.value)?;
            Ok("ok".to_string())
        }
        Command::FieldRead(args) => {
            let target = resolve_target(&controller, cli.device_uri.as_deref())?;
            let selector = args.selector.selector()?;
            let value = controller.read_field(&target, None, &selector, &args.field)?;
            Ok(format!("0x{value:08x} ({value})"))
        }
        Command::FieldWrite(args) => {
            let target = resolve_target(&controller, cli.device_uri.as_deref())?;
            let selector = args.selector.selector()?;
            controller.write_field(&target, None, &selector, &args.field, args.value)?;
            Ok("ok".to_string())
        }
        Command::StreamConfigure(args) => {
            let target = resolve_target(&controller, cli.device_uri.as_deref())?;
            let applied = controller.configure_stream(&target, build_stream_request(args)?)?;
            Ok(format_applied_stream(&applied))
        }
        Command::StreamStart(args) => {
            let target = resolve_target(&controller, cli.device_uri.as_deref())?;
            let running = controller.start_stream(&target, build_stream_request(args)?)?;
            Ok(format!(
                "running stream-id={} profile={} active={}",
                running.stream_id,
                profile_name(&running.profile),
                running.running
            ))
        }
        Command::StreamStop(args) => {
            let target = resolve_target(&controller, cli.device_uri.as_deref())?;
            controller.stop_stream(&target, &args.stream_name, Some(&args.bind_address))?;
            Ok("ok".to_string())
        }
    }
}

fn controller(cli: &Cli) -> DeviceController {
    DeviceController::new(CoapRegisterBackendConfig {
        interface_name: cli.iface.clone(),
        discovery_timeout: Duration::from_millis(cli.timeout_ms),
        request_timeout: Duration::from_millis(cli.timeout_ms),
        yaml_root: cli.yaml_root.clone(),
        local_port: cli.local_port,
    })
}

fn resolve_target(
    controller: &DeviceController,
    device_uri: Option<&str>,
) -> Result<ControlTarget> {
    if let Some(device_uri) = device_uri {
        return Ok(ControlTarget {
            device_uri: device_uri.to_string(),
            transport_kind: ControlTransportKind::CoapRegister,
            auth_scope: None,
        });
    }

    let devices = controller.discover(None)?;
    match devices.as_slice() {
        [device] => Ok(device.target.clone()),
        [] => bail!("no devices found; pass --device-uri explicitly"),
        _ => {
            let candidates = devices
                .iter()
                .map(|device| format!("{} ({})", device.target.device_uri, device.device_address))
                .collect::<Vec<_>>()
                .join(", ");
            bail!("multiple devices found; pass --device-uri explicitly: {candidates}")
        }
    }
}

fn build_stream_request(args: StreamArgs) -> Result<RequestedStreamConfiguration> {
    let format = match (args.width, args.height, args.pixel_format) {
        (Some(width), Some(height), Some(pixel_format)) => Some(StreamFormatDescriptor {
            payload_type: PayloadType::Image,
            pixel_format,
            width,
            height,
        }),
        (None, None, None) => None,
        _ => bail!("width, height, and pixel-format must be provided together"),
    };

    Ok(RequestedStreamConfiguration {
        stream_name: args.stream_name,
        profile: StreamProfileId::CompatibilityV1,
        destination_host: args.destination_host,
        port: args.port,
        bind_address: args.bind_address,
        packet_delay_ns: args.packet_delay_ns,
        max_packet_size: args.max_packet_size,
        format,
    })
}

fn format_applied_stream(applied: &eevideo_control::AppliedStreamConfiguration) -> String {
    let mut output = String::new();
    writeln!(output, "stream-id: {}", applied.stream_id).expect("string write");
    writeln!(output, "stream-name: {}", applied.stream_name).expect("string write");
    writeln!(output, "profile: {}", profile_name(&applied.profile)).expect("string write");
    writeln!(
        output,
        "destination: {}:{}",
        applied.destination_host, applied.port
    )
    .expect("string write");
    writeln!(output, "bind-address: {}", applied.bind_address).expect("string write");
    writeln!(output, "packet-delay-ns: {}", applied.packet_delay_ns).expect("string write");
    writeln!(output, "max-packet-size: {}", applied.max_packet_size).expect("string write");
    if let Some(format) = &applied.format {
        writeln!(
            output,
            "format: {} {}x{}",
            pixel_format_name(&format.pixel_format),
            format.width,
            format.height
        )
        .expect("string write");
    }
    output.trim_end().to_string()
}

fn format_register_value(value: &RegisterValue) -> String {
    match value {
        RegisterValue::U32(value) => format!("0x{value:08x} ({value})"),
        RegisterValue::String(value) => value.clone(),
    }
}

fn format_advertised_stream(stream: &eevideo_control::AdvertisedStream) -> String {
    match &stream.mode {
        Some(mode) => format!(
            "stream {}: {} {}x{} @ {} fps",
            stream.name,
            pixel_format_name(&mode.pixel_format),
            mode.width,
            mode.height,
            mode.fps
        ),
        None => format!("stream {}: mode unavailable", stream.name),
    }
}

fn profile_name(profile: &StreamProfileId) -> &'static str {
    match profile {
        StreamProfileId::CompatibilityV1 => "compatibility-v1",
    }
}

fn pixel_format_name(pixel_format: &PixelFormat) -> &'static str {
    pixel_format.gst_format()
}

fn parse_u32_arg(value: &str) -> Result<u32, String> {
    if let Some(value) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u32::from_str_radix(value, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u32>().map_err(|err| err.to_string())
    }
}

fn parse_pixel_format_arg(value: &str) -> Result<PixelFormat, String> {
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

impl RegisterSelectorArgs {
    fn selector(&self) -> Result<RegisterSelector> {
        match (&self.name, self.address) {
            (Some(name), None) => Ok(RegisterSelector::name(name.clone())),
            (None, Some(address)) => Ok(RegisterSelector::address(address)),
            _ => Err(anyhow!("exactly one of --name or --address is required")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use clap::{CommandFactory, Parser};
    use eefakedev::{FakeDeviceConfig, FakeDeviceServer};
    use eevideo_proto::PixelFormat;

    use super::{normalize_help_flag_punctuation, parse_pixel_format_arg, parse_u32_arg, run, Cli};

    fn render_long_help(mut command: clap::Command) -> String {
        let mut output = Vec::new();
        command.write_long_help(&mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    fn render_subcommand_long_help(name: &str) -> String {
        let mut command = Cli::command();
        let subcommand = command.find_subcommand_mut(name).unwrap();
        render_long_help(subcommand.clone())
    }

    #[test]
    fn parses_decimal_and_hex_numbers() {
        assert_eq!(parse_u32_arg("123").unwrap(), 123);
        assert_eq!(parse_u32_arg("0x10").unwrap(), 16);
    }

    #[test]
    fn parses_pixel_formats() {
        assert_eq!(parse_pixel_format_arg("gray8").unwrap(), PixelFormat::Mono8);
        assert_eq!(parse_pixel_format_arg("UYVY").unwrap(), PixelFormat::Uyvy);
    }

    #[test]
    fn top_level_help_mentions_examples_and_global_flags() {
        let help = render_long_help(Cli::command());

        assert!(help.contains("Examples:"));
        assert!(help.contains("eevid discover"));
        assert!(help.contains("Target a single device directly and skip discovery."));
    }

    #[test]
    fn reg_read_help_explains_selector_usage() {
        let help = render_subcommand_long_help("reg-read");

        assert!(help.contains("Read a register by symbolic name or numeric address."));
        assert!(help.contains(
            "eevid --device-uri coap://192.168.1.50:5683 reg-read --name stream0_MaxPacketSize"
        ));
    }

    #[test]
    fn stream_start_help_includes_receiver_example() {
        let help = render_subcommand_long_help("stream-start");

        assert!(help.contains("Start a stream toward a host-side receiver."));
        assert!(help.contains("eevid --device-uri coap://192.168.1.50:5683 stream-start"));
    }

    #[test]
    fn normalizes_help_flags_with_trailing_commas() {
        let args = normalize_help_flag_punctuation([
            OsString::from("eevid"),
            OsString::from("--help,"),
            OsString::from("-h,"),
            OsString::from("--other,"),
        ]);

        assert_eq!(args[1], OsString::from("--help"));
        assert_eq!(args[2], OsString::from("-h"));
        assert_eq!(args[3], OsString::from("--other,"));
    }

    #[test]
    fn describe_command_works_against_fake_device() {
        let device = FakeDeviceServer::spawn(FakeDeviceConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            width: 32,
            height: 16,
            pixel_format: PixelFormat::Mono8,
            fps: 24,
            ..FakeDeviceConfig::default()
        })
        .unwrap();

        let cli = Cli::parse_from(["eevid", "--device-uri", &device.uri(), "describe"]);
        let output = run(cli).unwrap();

        assert!(output.contains("device-uri:"));
        assert!(output.contains("streams: stream0"));
        assert!(output.contains("stream stream0: GRAY8 32x16 @ 24 fps"));
        assert!(output.contains("register stream0_MaxPacketSize"));
    }

    #[test]
    fn stream_start_command_starts_fake_device() {
        let device = FakeDeviceServer::spawn(FakeDeviceConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            width: 32,
            height: 16,
            pixel_format: PixelFormat::Mono8,
            ..FakeDeviceConfig::default()
        })
        .unwrap();

        let cli = Cli::parse_from([
            "eevid",
            "--device-uri",
            &device.uri(),
            "stream-start",
            "--stream-name",
            "stream0",
            "--destination-host",
            "127.0.0.1",
            "--port",
            "5000",
            "--bind-address",
            "127.0.0.1",
            "--max-packet-size",
            "1200",
        ]);
        let output = run(cli).unwrap();

        assert!(output.contains("running stream-id="));
        assert_eq!(device.start_count(), 1);
    }
}

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

#[derive(Debug, Parser)]
#[command(name = "eevid", about = "EEVideo discovery and register control CLI")]
pub struct Cli {
    #[arg(long, global = true)]
    device_uri: Option<String>,
    #[arg(long, global = true)]
    iface: Option<String>,
    #[arg(long, global = true, default_value_t = 1000)]
    timeout_ms: u64,
    #[arg(long, global = true, default_value_t = 0)]
    local_port: u16,
    #[arg(long, global = true)]
    yaml_root: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Discover,
    Describe,
    RegRead(RegisterSelectorArgs),
    RegWrite(RegisterWriteArgs),
    FieldRead(FieldReadArgs),
    FieldWrite(FieldWriteArgs),
    StreamConfigure(StreamArgs),
    StreamStart(StreamArgs),
    StreamStop(StreamStopArgs),
}

#[derive(Debug, Args)]
struct RegisterSelectorArgs {
    #[arg(long, conflicts_with = "address")]
    name: Option<String>,
    #[arg(long, value_parser = parse_u32_arg, conflicts_with = "name")]
    address: Option<u32>,
}

#[derive(Debug, Args)]
struct RegisterWriteArgs {
    #[command(flatten)]
    selector: RegisterSelectorArgs,
    #[arg(value_parser = parse_u32_arg)]
    value: u32,
}

#[derive(Debug, Args)]
struct FieldReadArgs {
    #[command(flatten)]
    selector: RegisterSelectorArgs,
    #[arg(long)]
    field: String,
}

#[derive(Debug, Args)]
struct FieldWriteArgs {
    #[command(flatten)]
    selector: RegisterSelectorArgs,
    #[arg(long)]
    field: String,
    #[arg(value_parser = parse_u32_arg)]
    value: u32,
}

#[derive(Debug, Args, Clone)]
struct StreamArgs {
    #[arg(long, default_value = "stream0")]
    stream_name: String,
    #[arg(long)]
    destination_host: String,
    #[arg(long)]
    port: u16,
    #[arg(long, default_value = "0.0.0.0")]
    bind_address: String,
    #[arg(long, default_value_t = 0)]
    packet_delay_ns: u64,
    #[arg(long, default_value_t = 1200)]
    max_packet_size: u16,
    #[arg(long)]
    width: Option<u32>,
    #[arg(long)]
    height: Option<u32>,
    #[arg(long, value_parser = parse_pixel_format_arg)]
    pixel_format: Option<PixelFormat>,
}

#[derive(Debug, Args)]
struct StreamStopArgs {
    #[arg(long, default_value = "stream0")]
    stream_name: String,
    #[arg(long, default_value = "0.0.0.0")]
    bind_address: String,
}

pub fn main_entry<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    let output = run(cli)?;
    if !output.is_empty() {
        println!("{output}");
    }
    Ok(())
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
            writeln!(output, "device-uri: {}", description.summary.target.device_uri)?;
            writeln!(output, "device-address: {}", description.summary.device_address)?;
            writeln!(output, "interface: {}", description.summary.interface_name)?;
            writeln!(
                output,
                "interface-address: {}",
                description.summary.interface_address
            )?;
            writeln!(output, "streams: {}", description.streams.join(", "))?;
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

fn resolve_target(controller: &DeviceController, device_uri: Option<&str>) -> Result<ControlTarget> {
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

fn profile_name(profile: &StreamProfileId) -> &'static str {
    match profile {
        StreamProfileId::CompatibilityV1 => "compatibility-v1",
    }
}

fn pixel_format_name(pixel_format: &PixelFormat) -> &'static str {
    pixel_format.gst_format()
}

fn parse_u32_arg(value: &str) -> Result<u32, String> {
    if let Some(value) = value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")) {
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
    use eevideo_proto::PixelFormat;

    use super::{parse_pixel_format_arg, parse_u32_arg};

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
}

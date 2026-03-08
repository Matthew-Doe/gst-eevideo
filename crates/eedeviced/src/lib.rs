use std::ffi::OsString;
use std::net::{Ipv4Addr, SocketAddr};

use anyhow::{bail, Result};
use clap::{Parser, ValueEnum};
use eevideo_device::{
    CaptureBackend, CaptureConfiguration, DeviceRuntime, DeviceRuntimeConfig,
    SyntheticCaptureBackend, SyntheticCaptureConfig,
};
use eevideo_proto::{PixelFormat, VideoFrame};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum InputKind {
    Synthetic,
    Argus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceDaemonConfig {
    pub bind: SocketAddr,
    pub interface_name: Option<String>,
    pub advertise_address: Option<Ipv4Addr>,
    pub stream_name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub mtu: u16,
    pub input: InputKind,
    pub sensor_id: u32,
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
            fps: 30,
            mtu: 1200,
            input: InputKind::Synthetic,
            sensor_id: 0,
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
            pixel_format: PixelFormat::Uyvy,
            fps: self.fps,
            mtu: self.mtu,
            enforce_fixed_format: true,
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
    #[arg(long, default_value_t = 30)]
    fps: u32,
    #[arg(long, default_value_t = 1200)]
    mtu: u16,
    #[arg(long, default_value = "synthetic")]
    input: InputKind,
    #[arg(long, default_value_t = 0)]
    sensor_id: u32,
}

impl From<Cli> for DeviceDaemonConfig {
    fn from(value: Cli) -> Self {
        Self {
            bind: value.bind,
            interface_name: value.iface,
            advertise_address: value.advertise_address,
            stream_name: value.stream_name,
            width: value.width,
            height: value.height,
            fps: value.fps,
            mtu: value.mtu,
            input: value.input,
            sensor_id: value.sensor_id,
        }
    }
}

pub struct DeviceDaemon {
    runtime: DeviceRuntime,
}

impl DeviceDaemon {
    pub fn spawn(config: DeviceDaemonConfig) -> Result<Self> {
        validate_config(&config)?;
        let runtime = match config.input {
            InputKind::Synthetic => DeviceRuntime::spawn(
                config.runtime_config(),
                SyntheticCaptureBackend::new(SyntheticCaptureConfig::default()),
            )?,
            InputKind::Argus => {
                DeviceRuntime::spawn(config.runtime_config(), StubArgusCaptureBackend::new(config.sensor_id))?
            }
        };
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
    run(cli.into())
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
    if config.width % 2 != 0 {
        bail!("UYVY device width must be even");
    }
    Ok(())
}

#[derive(Debug)]
struct StubArgusCaptureBackend {
    sensor_id: u32,
}

impl StubArgusCaptureBackend {
    fn new(sensor_id: u32) -> Self {
        Self { sensor_id }
    }
}

impl CaptureBackend for StubArgusCaptureBackend {
    fn start_capture(&mut self, _config: CaptureConfiguration) -> Result<()> {
        bail!(
            "Argus capture backend for sensor {} is not implemented in this build",
            self.sensor_id
        );
    }

    fn stop_capture(&mut self) -> Result<()> {
        Ok(())
    }

    fn next_frame(&mut self) -> Result<VideoFrame> {
        bail!(
            "Argus capture backend for sensor {} is not implemented in this build",
            self.sensor_id
        );
    }

    fn current_format(&self) -> Option<CaptureConfiguration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceDaemon, DeviceDaemonConfig, InputKind};
    use eevideo_control::backend::{CoapRegisterBackend, CoapRegisterBackendConfig};
    use eevideo_control::{ControlBackend, ControlTarget, ControlTransportKind, RequestedStreamConfiguration};
    use eevideo_proto::{PayloadType, PixelFormat, StreamProfileId};
    use std::time::Duration;

    #[test]
    fn synthetic_device_uses_fixed_uyvy_format() {
        let device = DeviceDaemon::spawn(DeviceDaemonConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            input: InputKind::Synthetic,
            width: 640,
            height: 480,
            ..DeviceDaemonConfig::default()
        })
        .unwrap();

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
                port: 5000,
                bind_address: "127.0.0.1".to_string(),
                packet_delay_ns: 0,
                max_packet_size: 1200,
                format: Some(eevideo_control::StreamFormatDescriptor {
                    payload_type: PayloadType::Image,
                    pixel_format: PixelFormat::Uyvy,
                    width: 640,
                    height: 480,
                }),
            })
            .unwrap();
        assert_eq!(applied.format.unwrap().pixel_format, PixelFormat::Uyvy);

        let error = connection
            .configure(RequestedStreamConfiguration {
                stream_name: "stream0".to_string(),
                profile: StreamProfileId::CompatibilityV1,
                destination_host: "127.0.0.1".to_string(),
                port: 5000,
                bind_address: "127.0.0.1".to_string(),
                packet_delay_ns: 0,
                max_packet_size: 1200,
                format: Some(eevideo_control::StreamFormatDescriptor {
                    payload_type: PayloadType::Image,
                    pixel_format: PixelFormat::Rgb8,
                    width: 640,
                    height: 480,
                }),
            })
            .unwrap_err();
        assert_eq!(error.kind(), eevideo_control::ControlErrorKind::AppliedValueMismatch);
    }
}

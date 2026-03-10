use std::collections::BTreeMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

use eevideo_proto::{PixelFormat, StreamProfileId};

use crate::discovery::{discover_devices, DISCOVERY_PORT};
use crate::register::RegisterClient;
use crate::register_map::{
    read_register_field, register_error, register_name, resolve_stream_prefix,
    write_register_fields, write_register_u32, FieldUpdate, RegisterSelector,
};
use crate::yaml::{
    load_embedded_feature_catalog, read_device_config, write_device_config, DeviceCapabilities,
    DeviceConfig, DeviceLocation, DeviceMemoryMap, DeviceRegisterValue, FeatureCatalog, YamlError,
};
use crate::{
    AppliedStreamConfiguration, ControlBackend, ControlCapabilities, ControlConnection,
    ControlError, ControlErrorKind, ControlTarget, ControlTransportKind, DiscoveredDevice,
    RequestedStreamConfiguration, RunningStream, StreamFormatDescriptor,
};

const CAPABILITIES_ADDR: u32 = 0;
const FEATURE_TABLE_ADDR: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoapRegisterBackendConfig {
    pub interface_name: Option<String>,
    pub discovery_timeout: Duration,
    pub request_timeout: Duration,
    pub yaml_root: Option<PathBuf>,
    pub local_port: u16,
}

impl Default for CoapRegisterBackendConfig {
    fn default() -> Self {
        Self {
            interface_name: None,
            discovery_timeout: Duration::from_secs(3),
            request_timeout: Duration::from_secs(1),
            yaml_root: None,
            local_port: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CoapRegisterBackend {
    config: CoapRegisterBackendConfig,
}

#[derive(Clone, Debug)]
pub(crate) struct DeviceEndpoint {
    pub(crate) addr: SocketAddr,
    pub(crate) host: String,
    pub(crate) uri: String,
}

#[derive(Clone, Debug)]
struct ConfiguredStream {
    applied: AppliedStreamConfiguration,
    register_prefix: String,
}

#[derive(Debug)]
struct CoapRegisterConnection {
    endpoint: DeviceEndpoint,
    config: CoapRegisterBackendConfig,
    device: DeviceConfig,
    disconnected: bool,
    configured: Option<ConfiguredStream>,
    running_stream_id: Option<String>,
}

impl CoapRegisterBackend {
    pub fn new(config: CoapRegisterBackendConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &CoapRegisterBackendConfig {
        &self.config
    }

    fn connect_endpoint(
        &self,
        endpoint: &DeviceEndpoint,
    ) -> Result<CoapRegisterConnection, ControlError> {
        let device = self.load_or_create_device_config(endpoint)?;
        Ok(CoapRegisterConnection {
            endpoint: endpoint.clone(),
            config: self.config.clone(),
            device,
            disconnected: false,
            configured: None,
            running_stream_id: None,
        })
    }

    pub(crate) fn load_or_create_device_config(
        &self,
        endpoint: &DeviceEndpoint,
    ) -> Result<DeviceConfig, ControlError> {
        if let Some(root) = self.config.yaml_root.as_ref() {
            if let Some(device) = maybe_read_device_config(root, endpoint)? {
                return Ok(device);
            }
        }

        let device = self.introspect_device(endpoint)?;
        if let Some(root) = self.config.yaml_root.as_ref() {
            persist_device_config(root, endpoint, &device)?;
        }
        Ok(device)
    }

    fn introspect_device(&self, endpoint: &DeviceEndpoint) -> Result<DeviceConfig, ControlError> {
        let client = RegisterClient::new(
            local_bind_addr(None, self.config.local_port, endpoint.addr),
            endpoint.addr,
        )
        .with_timeout(self.config.request_timeout);
        let features = load_embedded_feature_catalog().map_err(yaml_error)?;
        let location = DeviceLocation {
            interface_name: self.config.interface_name.clone().unwrap_or_default(),
            interface_address: unspecified_host(endpoint.addr).to_string(),
            device_address: endpoint.host.clone(),
        };
        introspect_device_config(&client, location, &features)
    }
}

impl ControlBackend for CoapRegisterBackend {
    fn discover(&self, target: &ControlTarget) -> Result<Vec<DiscoveredDevice>, ControlError> {
        let requested_host = if target.device_uri.trim().is_empty() {
            None
        } else {
            parse_device_endpoint(&target.device_uri)
                .ok()
                .map(|endpoint| endpoint.host)
        };

        let responses = discover_devices(
            self.config.interface_name.as_deref(),
            self.config.discovery_timeout,
        )
        .map_err(discovery_error)?;

        Ok(responses
            .into_iter()
            .filter(|response| {
                requested_host
                    .as_deref()
                    .map_or(true, |host| host == response.device_address.to_string())
            })
            .map(|response| DiscoveredDevice {
                device_uri: format!("coap://{}:{}", response.device_address, DISCOVERY_PORT),
                transport_kind: ControlTransportKind::CoapRegister,
                interface_name: response.interface_name,
                interface_address: response.interface_address.to_string(),
                device_address: response.device_address.to_string(),
                auth_scope: None,
            })
            .collect())
    }

    fn connect(&self, target: &ControlTarget) -> Result<Box<dyn ControlConnection>, ControlError> {
        let endpoint = parse_device_endpoint(&target.device_uri)?;
        Ok(Box::new(self.connect_endpoint(&endpoint)?))
    }
}

impl CoapRegisterConnection {
    fn ensure_connected(&self) -> Result<(), ControlError> {
        if self.disconnected {
            Err(ControlError::new(
                ControlErrorKind::Disconnected,
                "control connection is disconnected",
            ))
        } else {
            Ok(())
        }
    }

    fn register_client(&self, bind_address: Option<&str>) -> RegisterClient {
        let bind_addr = local_bind_addr(bind_address, self.config.local_port, self.endpoint.addr);
        RegisterClient::new(bind_addr, self.endpoint.addr).with_timeout(self.config.request_timeout)
    }

    fn capabilities(&self) -> ControlCapabilities {
        let mut supported_pixel_formats = self
            .device
            .registers
            .iter()
            .filter(|(name, _)| name.ends_with("_PixelFormat"))
            .filter_map(|(_, register)| register.int_value)
            .filter_map(|value| pixel_format_from_device(value as u32))
            .collect::<Vec<_>>();
        supported_pixel_formats.sort_by_key(|format| format.pfnc());
        supported_pixel_formats.dedup();

        ControlCapabilities {
            supported_profiles: vec![StreamProfileId::CompatibilityV1],
            supported_pixel_formats,
            multicast_supported: self.device.capabilities.mult_addr,
            packet_pacing_supported: self
                .device
                .registers
                .keys()
                .any(|name| name.ends_with("_Delay")),
            native_framing_supported: false,
        }
    }

    fn resolve_stream_prefix(&self, requested_stream_name: &str) -> Result<String, ControlError> {
        resolve_stream_prefix(&self.device, requested_stream_name)
    }

    fn write_stream_configuration(
        &self,
        client: &RegisterClient,
        prefix: &str,
        request: &RequestedStreamConfiguration,
    ) -> Result<(), ControlError> {
        if request.profile != StreamProfileId::CompatibilityV1 {
            return Err(ControlError::new(
                ControlErrorKind::UnsupportedProfile,
                format!("unsupported stream profile {:?}", request.profile),
            ));
        }

        let destination_ip = resolve_destination_ip(&request.destination_host)?;
        let delay = u32::try_from(request.packet_delay_ns).map_err(|_| {
            ControlError::new(
                ControlErrorKind::InvalidConfiguration,
                format!(
                    "packet delay {}ns exceeds the 32-bit EEVideo register range",
                    request.packet_delay_ns
                ),
            )
        })?;
        if delay > 0x00FF_FFFF {
            return Err(ControlError::new(
                ControlErrorKind::InvalidConfiguration,
                format!("packet delay {delay} exceeds the 24-bit stream delay field"),
            ));
        }

        write_register_fields(
            client,
            &self.device,
            &RegisterSelector::name(register_name(prefix, "Delay")),
            &[FieldUpdate::new("delay", delay)],
        )?;
        write_register_u32(
            client,
            &self.device,
            &RegisterSelector::name(register_name(prefix, "DestPort")),
            u32::from(request.port),
        )?;
        write_register_u32(
            client,
            &self.device,
            &RegisterSelector::name(register_name(prefix, "DestIPAddr")),
            u32::from(destination_ip),
        )?;
        write_register_fields(
            client,
            &self.device,
            &RegisterSelector::name(register_name(prefix, "MaxPacketSize")),
            &[
                FieldUpdate::new("fireTestPkt", 0),
                FieldUpdate::new("enable", 0),
                FieldUpdate::new("maxPkt", u32::from(request.max_packet_size)),
            ],
        )?;

        if let Some(format) = &request.format {
            apply_format_registers(client, &self.device, prefix, format)?;
        }

        Ok(())
    }
}

impl ControlConnection for CoapRegisterConnection {
    fn describe(&self) -> Result<ControlCapabilities, ControlError> {
        self.ensure_connected()?;
        Ok(self.capabilities())
    }

    fn configure(
        &mut self,
        request: RequestedStreamConfiguration,
    ) -> Result<AppliedStreamConfiguration, ControlError> {
        self.ensure_connected()?;

        let stream_prefix = self.resolve_stream_prefix(&request.stream_name)?;
        let client = self.register_client(Some(&request.bind_address));
        self.write_stream_configuration(&client, &stream_prefix, &request)?;
        let applied_format = if request.format.is_some() {
            request.format.clone()
        } else {
            read_stream_format(&client, &self.device, &stream_prefix)?
        };

        let applied = AppliedStreamConfiguration {
            stream_id: format!("{}#{stream_prefix}", self.endpoint.uri),
            stream_name: request.stream_name.clone(),
            profile: request.profile,
            destination_host: request.destination_host.clone(),
            port: request.port,
            bind_address: request.bind_address.clone(),
            packet_delay_ns: request.packet_delay_ns,
            max_packet_size: request.max_packet_size,
            format: applied_format,
            normalized: false,
        };

        self.configured = Some(ConfiguredStream {
            applied: applied.clone(),
            register_prefix: stream_prefix,
        });
        self.running_stream_id = None;
        Ok(applied)
    }

    fn start(&mut self, stream_id: &str) -> Result<RunningStream, ControlError> {
        self.ensure_connected()?;
        let configured = self.configured.as_ref().ok_or_else(|| {
            ControlError::new(
                ControlErrorKind::InvalidConfiguration,
                "stream must be configured before start",
            )
        })?;
        if configured.applied.stream_id != stream_id {
            return Err(ControlError::new(
                ControlErrorKind::AppliedValueMismatch,
                format!(
                    "start requested for stream {stream_id}, but configured stream is {}",
                    configured.applied.stream_id
                ),
            ));
        }

        if self.running_stream_id.as_deref() == Some(stream_id) {
            return Ok(RunningStream {
                stream_id: stream_id.to_string(),
                profile: configured.applied.profile,
                running: true,
            });
        }

        let client = self.register_client(Some(&configured.applied.bind_address));
        write_register_fields(
            &client,
            &self.device,
            &RegisterSelector::name(register_name(&configured.register_prefix, "MaxPacketSize")),
            &[
                FieldUpdate::new("fireTestPkt", 0),
                FieldUpdate::new("enable", 1),
                FieldUpdate::new("maxPkt", u32::from(configured.applied.max_packet_size)),
            ],
        )?;

        self.running_stream_id = Some(stream_id.to_string());
        Ok(RunningStream {
            stream_id: stream_id.to_string(),
            profile: configured.applied.profile,
            running: true,
        })
    }

    fn stop(&mut self, stream_id: &str) -> Result<(), ControlError> {
        self.ensure_connected()?;
        let Some(configured) = self.configured.as_ref() else {
            return Ok(());
        };
        if configured.applied.stream_id != stream_id {
            return Err(ControlError::new(
                ControlErrorKind::AppliedValueMismatch,
                format!(
                    "stop requested for stream {stream_id}, but configured stream is {}",
                    configured.applied.stream_id
                ),
            ));
        }
        if self.running_stream_id.is_none() {
            return Ok(());
        }

        let client = self.register_client(Some(&configured.applied.bind_address));
        write_register_fields(
            &client,
            &self.device,
            &RegisterSelector::name(register_name(&configured.register_prefix, "MaxPacketSize")),
            &[FieldUpdate::new("enable", 0)],
        )?;
        self.running_stream_id = None;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), ControlError> {
        self.disconnected = true;
        self.running_stream_id = None;
        Ok(())
    }
}

pub(crate) fn parse_device_endpoint(device_uri: &str) -> Result<DeviceEndpoint, ControlError> {
    let trimmed = device_uri.trim();
    if trimmed.is_empty() {
        return Err(ControlError::new(
            ControlErrorKind::Connection,
            "control target does not specify a device URI",
        ));
    }

    let raw = trimmed.strip_prefix("coap://").unwrap_or(trimmed);
    let authority = raw.split('/').next().unwrap_or(raw);
    let endpoint = if authority.starts_with('[') || authority.contains(':') {
        authority.to_string()
    } else {
        format!("{authority}:{}", DISCOVERY_PORT)
    };
    let addr = endpoint
        .to_socket_addrs()
        .map_err(|err| {
            ControlError::new(
                ControlErrorKind::Connection,
                format!("failed to resolve control device URI {trimmed}: {err}"),
            )
        })?
        .next()
        .ok_or_else(|| {
            ControlError::new(
                ControlErrorKind::Connection,
                format!("failed to resolve any socket addresses for control device URI {trimmed}"),
            )
        })?;

    Ok(DeviceEndpoint {
        addr,
        host: addr.ip().to_string(),
        uri: format!("coap://{}", addr),
    })
}

fn maybe_read_device_config(
    yaml_root: &Path,
    endpoint: &DeviceEndpoint,
) -> Result<Option<DeviceConfig>, ControlError> {
    let path = yaml_path(yaml_root, endpoint);
    if !path.exists() {
        return Ok(None);
    }
    read_device_config(path).map(Some).map_err(yaml_error)
}

fn persist_device_config(
    yaml_root: &Path,
    endpoint: &DeviceEndpoint,
    device: &DeviceConfig,
) -> Result<(), ControlError> {
    let path = yaml_path(yaml_root, endpoint);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            ControlError::new(
                ControlErrorKind::Other,
                format!(
                    "failed to create YAML directory {}: {err}",
                    parent.display()
                ),
            )
        })?;
    }
    write_device_config(path, device).map_err(yaml_error)
}

fn yaml_path(yaml_root: &Path, endpoint: &DeviceEndpoint) -> PathBuf {
    let is_file = yaml_root.extension().map_or(false, |ext| {
        ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml")
    });
    if is_file {
        return yaml_root.to_path_buf();
    }
    let sanitized = sanitize_filename(&format!("{}_{}", endpoint.host, endpoint.addr.port()));
    yaml_root.join(format!("{sanitized}.yaml"))
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}

pub(crate) fn local_bind_addr(
    bind_address: Option<&str>,
    port: u16,
    device_addr: SocketAddr,
) -> SocketAddr {
    let ip = bind_address
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<IpAddr>().ok())
        .unwrap_or_else(|| unspecified_host(device_addr));
    SocketAddr::new(ip, port)
}

fn unspecified_host(device_addr: SocketAddr) -> IpAddr {
    match device_addr {
        SocketAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        SocketAddr::V6(_) => IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
    }
}

fn resolve_destination_ip(host: &str) -> Result<Ipv4Addr, ControlError> {
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Ok(ip);
    }

    (host, 0)
        .to_socket_addrs()
        .map_err(|err| {
            ControlError::new(
                ControlErrorKind::InvalidConfiguration,
                format!("failed to resolve destination host {host}: {err}"),
            )
        })?
        .find_map(|addr| match addr.ip() {
            IpAddr::V4(ip) => Some(ip),
            IpAddr::V6(_) => None,
        })
        .ok_or_else(|| {
            ControlError::new(
                ControlErrorKind::InvalidConfiguration,
                format!("destination host {host} did not resolve to an IPv4 address"),
            )
        })
}

fn introspect_device_config(
    client: &RegisterClient,
    location: DeviceLocation,
    features: &FeatureCatalog,
) -> Result<DeviceConfig, ControlError> {
    let capabilities_word = client.read_u32(CAPABILITIES_ADDR).map_err(register_error)?;
    if capabilities_word >> 16 != 0xE71D {
        return Err(ControlError::new(
            ControlErrorKind::Connection,
            format!(
                "device capabilities register does not contain the EEVideo signature: 0x{capabilities_word:08x}"
            ),
        ));
    }

    let capabilities = DeviceCapabilities {
        dec_avail: capabilities_word & 0x8000 != 0,
        mult_addr: capabilities_word & 0x0800 != 0,
        string_rd: capabilities_word & 0x0400 != 0,
        fifo_rd: capabilities_word & 0x0200 != 0,
        read_rst: capabilities_word & 0x0100 != 0,
        mask_wr: capabilities_word & 0x0080 != 0,
        bit_tog: capabilities_word & 0x0040 != 0,
        bit_set: capabilities_word & 0x0020 != 0,
        bit_clear: capabilities_word & 0x0010 != 0,
        static_ip: capabilities_word & 0x0008 != 0,
        link_local_ip: capabilities_word & 0x0004 != 0,
        dhcp_ip: capabilities_word & 0x0002 != 0,
        multi_disc: capabilities_word & 0x0001 != 0,
    };

    let mut word_addr = FEATURE_TABLE_ADDR;
    let mut feature_counts = BTreeMap::<u32, usize>::new();
    let mut registers = BTreeMap::new();
    let mut memory_map = DeviceMemoryMap::default();

    loop {
        let feature_word = client.read_u32(word_addr).map_err(register_error)?;
        word_addr += 4;
        let pointer_count = (feature_word & 0xFF) as usize;
        let feature_id = feature_word >> 8;

        let mut pointers = Vec::with_capacity(pointer_count);
        for _ in 0..pointer_count {
            pointers.push(client.read_u32(word_addr).map_err(register_error)?);
            word_addr += 4;
        }

        if feature_word >> 20 == 0xFFF {
            memory_map.last_static = pointers.first().copied().unwrap_or_default();
            memory_map.first_mutable = pointers.get(1).copied().unwrap_or_default();
            memory_map.last_mutable = pointers.get(2).copied().unwrap_or_default();
            break;
        }

        let definition = features.get(&feature_id).ok_or_else(|| {
            ControlError::new(
                ControlErrorKind::Other,
                format!("device referenced unknown feature id 0x{feature_id:06x}"),
            )
        })?;
        let instance_index = feature_counts.entry(feature_id).or_insert(0usize);
        let instance = *instance_index;
        *instance_index += 1;

        for pointer in &definition.pointers {
            let Some(base_addr) = pointers.get(pointer.index as usize).copied() else {
                continue;
            };
            for register in &pointer.registers {
                let register_name =
                    format!("{}{}_{}", definition.short_name, instance, register.name);
                let register_addr = base_addr.saturating_add(register.offset.saturating_mul(4));
                let mut value = DeviceRegisterValue {
                    addr: register_addr,
                    access: register.access.clone().unwrap_or_else(|| "ro".to_string()),
                    int_value: None,
                    str_value: None,
                    fields: register.fields.clone(),
                };

                if value.access == "string" {
                    value.str_value =
                        Some(client.read_string(register_addr).map_err(register_error)?);
                } else {
                    value.int_value =
                        Some(client.read_u32(register_addr).map_err(register_error)? as u64);
                }
                registers.insert(register_name, value);
            }
        }
    }

    Ok(DeviceConfig {
        location,
        capabilities,
        memory_map,
        registers,
    })
}

fn apply_format_registers(
    client: &RegisterClient,
    device: &DeviceConfig,
    prefix: &str,
    format: &StreamFormatDescriptor,
) -> Result<(), ControlError> {
    write_register_fields(
        client,
        device,
        &RegisterSelector::name(register_name(prefix, "PixelsPerLine")),
        &[FieldUpdate::new("ppl", format.width)],
    )?;
    write_register_fields(
        client,
        device,
        &RegisterSelector::name(register_name(prefix, "LinesPerFrame")),
        &[FieldUpdate::new("lpf", format.height)],
    )?;
    write_register_fields(
        client,
        device,
        &RegisterSelector::name(register_name(prefix, "PixelFormat")),
        &[FieldUpdate::new("bpp", format.pixel_format.pfnc() & 0xffff)],
    )?;
    Ok(())
}

fn read_stream_format(
    client: &RegisterClient,
    device: &DeviceConfig,
    prefix: &str,
) -> Result<Option<StreamFormatDescriptor>, ControlError> {
    let width = read_register_field(
        client,
        device,
        &RegisterSelector::name(register_name(prefix, "PixelsPerLine")),
        "ppl",
    )?;
    let height = read_register_field(
        client,
        device,
        &RegisterSelector::name(register_name(prefix, "LinesPerFrame")),
        "lpf",
    )?;
    let pixel_format_bits = read_register_field(
        client,
        device,
        &RegisterSelector::name(register_name(prefix, "PixelFormat")),
        "bpp",
    )?;

    if width == 0 || height == 0 || pixel_format_bits == 0 {
        return Ok(None);
    }

    let pixel_format = pixel_format_from_device(pixel_format_bits).ok_or_else(|| {
        ControlError::new(
            ControlErrorKind::InvalidConfiguration,
            format!("device reported unsupported pixel format value 0x{pixel_format_bits:08x}"),
        )
    })?;

    Ok(Some(StreamFormatDescriptor {
        payload_type: eevideo_proto::PayloadType::Image,
        pixel_format,
        width,
        height,
    }))
}

fn pixel_format_from_device(value: u32) -> Option<PixelFormat> {
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

pub(crate) fn discovery_error(error: impl std::fmt::Display) -> ControlError {
    ControlError::new(ControlErrorKind::Discovery, error.to_string())
}

pub(crate) fn yaml_error(error: YamlError) -> ControlError {
    ControlError::new(ControlErrorKind::Other, error.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use eevideo_proto::{PayloadType, PixelFormat, StreamProfileId};

    use crate::coap::{
        CoapMessage, CoapMessageType, CoapOption, CODE_CHANGED, CODE_CONTENT, CODE_GET,
        OPTION_EEV_BINARY_ADDRESS, OPTION_EEV_REG_ACCESS,
    };
    use crate::register::RegisterReadKind;
    use crate::{
        ControlBackend, ControlTarget, ControlTransportKind, RequestedStreamConfiguration,
    };

    use super::{
        parse_device_endpoint, CoapRegisterBackend, CoapRegisterBackendConfig, DeviceEndpoint,
        StreamFormatDescriptor, CAPABILITIES_ADDR, FEATURE_TABLE_ADDR,
    };

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

    #[test]
    fn parses_device_uri_without_scheme() {
        let endpoint = parse_device_endpoint("127.0.0.1").unwrap();
        assert_eq!(endpoint.addr.port(), 5683);
        assert_eq!(endpoint.host, "127.0.0.1");
    }

    #[test]
    fn configures_starts_and_stops_stream_registers() {
        let device = FakeDevice::spawn(FakeDeviceBehavior::default());
        let backend = CoapRegisterBackend::new(CoapRegisterBackendConfig {
            request_timeout: Duration::from_millis(250),
            ..CoapRegisterBackendConfig::default()
        });
        let mut connection = backend.connect(&control_target(device.endpoint())).unwrap();

        let capabilities = connection.describe().unwrap();
        assert!(capabilities.multicast_supported);

        let applied = connection.configure(request()).unwrap();
        assert!(applied.stream_id.contains("#stream0"));
        let running = connection.start(&applied.stream_id).unwrap();
        assert!(running.running);

        let registers = device.registers();
        assert_eq!(
            registers.get(&STREAM_DELAY_ADDR).copied().unwrap() & 0x00ff_ffff,
            321
        );
        assert_eq!(
            registers.get(&STREAM_DEST_PORT_ADDR).copied().unwrap() & 0xffff,
            5000
        );
        assert_eq!(
            registers.get(&STREAM_DEST_IP_ADDR).copied().unwrap(),
            u32::from(Ipv4Addr::new(239, 1, 2, 3))
        );
        assert_eq!(
            registers.get(&STREAM_MAX_PACKET_ADDR).copied().unwrap() & 0xffff,
            1400
        );
        assert_ne!(
            registers.get(&STREAM_MAX_PACKET_ADDR).copied().unwrap() & (1 << 16),
            0
        );
        assert_eq!(
            registers.get(&STREAM_WIDTH_ADDR).copied().unwrap() & 0xffff,
            320
        );
        assert_eq!(
            registers.get(&STREAM_HEIGHT_ADDR).copied().unwrap() & 0xffff,
            240
        );
        assert_eq!(
            registers.get(&STREAM_PIXEL_FORMAT_ADDR).copied().unwrap(),
            PixelFormat::Mono8.pfnc() & 0xffff
        );

        connection.stop(&applied.stream_id).unwrap();
        let registers = device.registers();
        assert_eq!(
            registers.get(&STREAM_MAX_PACKET_ADDR).copied().unwrap() & (1 << 16),
            0
        );
    }

    #[test]
    fn configure_reports_applied_value_mismatch() {
        let device = FakeDevice::spawn(FakeDeviceBehavior {
            normalize_dest_port: true,
            silent: false,
        });
        let backend = CoapRegisterBackend::new(CoapRegisterBackendConfig {
            request_timeout: Duration::from_millis(250),
            ..CoapRegisterBackendConfig::default()
        });
        let mut connection = backend.connect(&control_target(device.endpoint())).unwrap();

        let error = connection.configure(request()).unwrap_err();
        assert_eq!(error.kind(), crate::ControlErrorKind::AppliedValueMismatch);
    }

    #[test]
    fn connect_reports_timeout_when_device_never_responds() {
        let device = FakeDevice::spawn(FakeDeviceBehavior {
            normalize_dest_port: false,
            silent: true,
        });
        let backend = CoapRegisterBackend::new(CoapRegisterBackendConfig {
            request_timeout: Duration::from_millis(100),
            ..CoapRegisterBackendConfig::default()
        });

        let error = match backend.connect(&control_target(device.endpoint())) {
            Ok(_) => panic!("silent device should time out"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), crate::ControlErrorKind::Timeout);
    }

    #[test]
    fn start_requires_prior_configuration() {
        let device = FakeDevice::spawn(FakeDeviceBehavior::default());
        let backend = CoapRegisterBackend::new(CoapRegisterBackendConfig {
            request_timeout: Duration::from_millis(250),
            ..CoapRegisterBackendConfig::default()
        });
        let mut connection = backend.connect(&control_target(device.endpoint())).unwrap();

        let error = connection.start("coap://127.0.0.1#stream0").unwrap_err();
        assert_eq!(error.kind(), crate::ControlErrorKind::InvalidConfiguration);
    }

    fn control_target(endpoint: DeviceEndpoint) -> ControlTarget {
        ControlTarget {
            device_uri: endpoint.uri,
            transport_kind: ControlTransportKind::CoapRegister,
            auth_scope: None,
        }
    }

    fn request() -> RequestedStreamConfiguration {
        RequestedStreamConfiguration {
            stream_name: "stream0".to_string(),
            profile: StreamProfileId::CompatibilityV1,
            destination_host: "239.1.2.3".to_string(),
            port: 5000,
            bind_address: "0.0.0.0".to_string(),
            packet_delay_ns: 321,
            max_packet_size: 1400,
            format: Some(StreamFormatDescriptor {
                payload_type: PayloadType::Image,
                pixel_format: PixelFormat::Mono8,
                width: 320,
                height: 240,
            }),
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct FakeDeviceBehavior {
        normalize_dest_port: bool,
        silent: bool,
    }

    struct FakeDevice {
        addr: SocketAddr,
        registers: Arc<Mutex<BTreeMap<u32, u32>>>,
        stop: Arc<AtomicBool>,
        join: Option<JoinHandle<()>>,
    }

    impl FakeDevice {
        fn spawn(behavior: FakeDeviceBehavior) -> Self {
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            socket
                .set_read_timeout(Some(Duration::from_millis(50)))
                .unwrap();
            let addr = socket.local_addr().unwrap();

            let registers = Arc::new(Mutex::new(build_registers()));
            let strings = Arc::new(BTreeMap::from([(
                STREAM_DESC_ADDR,
                b"EEVideo Stream 0\0".to_vec(),
            )]));
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = Arc::clone(&stop);
            let thread_registers = Arc::clone(&registers);
            let thread_strings = Arc::clone(&strings);

            let join = thread::spawn(move || {
                let mut buffer = [0u8; 2048];
                while !thread_stop.load(Ordering::Relaxed) {
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

                    if behavior.silent {
                        continue;
                    }

                    let request = match CoapMessage::decode(&buffer[..size]) {
                        Ok(request) => request,
                        Err(_) => continue,
                    };
                    let address = request
                        .options
                        .iter()
                        .find(|option| option.number == OPTION_EEV_BINARY_ADDRESS)
                        .map(|option| u32::from_be_bytes(option.value.clone().try_into().unwrap()))
                        .unwrap();
                    let reg_access = request
                        .options
                        .iter()
                        .find(|option| option.number == OPTION_EEV_REG_ACCESS)
                        .and_then(|option| option.value.first().copied());

                    let response = if request.code == CODE_GET {
                        let payload = if reg_access.map(|value| value >> 5)
                            == Some(RegisterReadKind::String as u8)
                        {
                            thread_strings.get(&address).cloned().unwrap_or_default()
                        } else {
                            let value = thread_registers
                                .lock()
                                .unwrap()
                                .get(&address)
                                .copied()
                                .unwrap_or_default();
                            value.to_be_bytes().to_vec()
                        };
                        CoapMessage::new(
                            CoapMessageType::Acknowledgement,
                            CODE_CONTENT,
                            request.message_id,
                            request.token,
                            Vec::<CoapOption>::new(),
                            payload,
                        )
                    } else {
                        let mut registers = thread_registers.lock().unwrap();
                        let mut value =
                            u32::from_be_bytes(request.payload.clone().try_into().unwrap());
                        if behavior.normalize_dest_port && address == STREAM_DEST_PORT_ADDR {
                            value = (value & !0xffff) | 5001;
                        }
                        registers.insert(address, value);
                        CoapMessage::new(
                            CoapMessageType::Acknowledgement,
                            CODE_CHANGED,
                            request.message_id,
                            request.token,
                            Vec::<CoapOption>::new(),
                            Vec::new(),
                        )
                    };

                    let bytes = response.encode().unwrap();
                    let _ = socket.send_to(&bytes, peer);
                }
            });

            Self {
                addr,
                registers,
                stop,
                join: Some(join),
            }
        }

        fn endpoint(&self) -> DeviceEndpoint {
            DeviceEndpoint {
                addr: self.addr,
                host: self.addr.ip().to_string(),
                uri: format!("coap://{}", self.addr),
            }
        }

        fn registers(&self) -> BTreeMap<u32, u32> {
            self.registers.lock().unwrap().clone()
        }
    }

    impl Drop for FakeDevice {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = UdpSocket::bind("127.0.0.1:0")
                .and_then(|socket| socket.send_to(&[0], self.addr))
                .ok();
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }

    fn build_registers() -> BTreeMap<u32, u32> {
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

        registers.insert(STREAM_MAX_PACKET_ADDR, 1200);
        registers.insert(STREAM_DELAY_ADDR, 0);
        registers.insert(STREAM_DEST_IP_ADDR, 0);
        registers.insert(STREAM_DEST_PORT_ADDR, 0);
        registers.insert(STREAM_SOURCE_PORT_ADDR, 0);
        registers.insert(STREAM_WIDTH_ADDR, 0);
        registers.insert(STREAM_HEIGHT_ADDR, 0);
        registers.insert(STREAM_PIXEL_FORMAT_ADDR, 0);
        registers.insert(STREAM_ACQ_ADDR, 0);
        registers.insert(STREAM_X_OFFSET_ADDR, 0);
        registers.insert(STREAM_Y_OFFSET_ADDR, 0);
        registers.insert(STREAM_TEST_PATTERN_ADDR, 0);
        registers
    }
}

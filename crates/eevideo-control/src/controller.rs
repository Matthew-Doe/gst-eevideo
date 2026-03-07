use std::sync::Arc;

use crate::backend::{
    local_bind_addr, parse_device_endpoint, CoapRegisterBackend, CoapRegisterBackendConfig,
};
use crate::register::RegisterClient;
use crate::register_map::{
    read_register_field, read_register_value, register_name, resolve_stream_prefix,
    stream_prefixes, write_register_fields, write_register_u32, FieldUpdate, RegisterSelector,
    RegisterValue,
};
use crate::yaml::DeviceConfig;
use crate::{
    AppliedStreamConfiguration, ControlBackend, ControlCapabilities, ControlError, ControlSession,
    ControlTarget, ControlTransportKind, DiscoveredDevice, RequestedStreamConfiguration,
    RunningStream, SharedControlBackend,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceSummary {
    pub target: ControlTarget,
    pub interface_name: String,
    pub interface_address: String,
    pub device_address: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceDescription {
    pub summary: DeviceSummary,
    pub capabilities: ControlCapabilities,
    pub device: DeviceConfig,
    pub streams: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DeviceController {
    backend: CoapRegisterBackend,
}

impl DeviceController {
    pub fn new(config: CoapRegisterBackendConfig) -> Self {
        Self {
            backend: CoapRegisterBackend::new(config),
        }
    }

    pub fn backend(&self) -> &CoapRegisterBackend {
        &self.backend
    }

    pub fn shared_backend(&self) -> SharedControlBackend {
        Arc::new(self.backend.clone())
    }

    pub fn discover(
        &self,
        device_uri_filter: Option<&str>,
    ) -> Result<Vec<DeviceSummary>, ControlError> {
        let target = ControlTarget {
            device_uri: device_uri_filter.unwrap_or_default().to_string(),
            transport_kind: ControlTransportKind::CoapRegister,
            auth_scope: None,
        };
        let devices = self.backend.discover(&target)?;
        Ok(devices.into_iter().map(summary_from_discovered).collect())
    }

    pub fn describe(&self, target: &ControlTarget) -> Result<DeviceDescription, ControlError> {
        let endpoint = parse_device_endpoint(&target.device_uri)?;
        let device = self.backend.load_or_create_device_config(&endpoint)?;
        let capabilities = self.backend.connect(target)?.describe()?;

        Ok(DeviceDescription {
            summary: DeviceSummary {
                target: target.clone(),
                interface_name: device.location.interface_name.clone(),
                interface_address: device.location.interface_address.clone(),
                device_address: device.location.device_address.clone(),
            },
            capabilities,
            streams: stream_prefixes(&device),
            device,
        })
    }

    pub fn read_register(
        &self,
        target: &ControlTarget,
        bind_address: Option<&str>,
        selector: &RegisterSelector,
    ) -> Result<RegisterValue, ControlError> {
        let (client, device) = self.client_and_device(target, bind_address)?;
        read_register_value(&client, &device, selector)
    }

    pub fn write_register(
        &self,
        target: &ControlTarget,
        bind_address: Option<&str>,
        selector: &RegisterSelector,
        value: u32,
    ) -> Result<(), ControlError> {
        let (client, device) = self.client_and_device(target, bind_address)?;
        write_register_u32(&client, &device, selector, value)
    }

    pub fn read_field(
        &self,
        target: &ControlTarget,
        bind_address: Option<&str>,
        selector: &RegisterSelector,
        field_name: &str,
    ) -> Result<u32, ControlError> {
        let (client, device) = self.client_and_device(target, bind_address)?;
        read_register_field(&client, &device, selector, field_name)
    }

    pub fn write_field(
        &self,
        target: &ControlTarget,
        bind_address: Option<&str>,
        selector: &RegisterSelector,
        field_name: &str,
        value: u32,
    ) -> Result<(), ControlError> {
        let (client, device) = self.client_and_device(target, bind_address)?;
        write_register_fields(
            &client,
            &device,
            selector,
            &[FieldUpdate::new(field_name, value)],
        )
    }

    pub fn configure_stream(
        &self,
        target: &ControlTarget,
        request: RequestedStreamConfiguration,
    ) -> Result<AppliedStreamConfiguration, ControlError> {
        let mut session = ControlSession::new(self.shared_backend(), target.clone(), request);
        let requested = session.requested().clone();
        session.configure(requested)
    }

    pub fn start_stream(
        &self,
        target: &ControlTarget,
        request: RequestedStreamConfiguration,
    ) -> Result<RunningStream, ControlError> {
        let mut session = ControlSession::new(self.shared_backend(), target.clone(), request);
        session.start()
    }

    pub fn stop_stream(
        &self,
        target: &ControlTarget,
        stream_name: &str,
        bind_address: Option<&str>,
    ) -> Result<(), ControlError> {
        let (client, device) = self.client_and_device(target, bind_address)?;
        let prefix = resolve_stream_prefix(&device, stream_name)?;
        write_register_fields(
            &client,
            &device,
            &RegisterSelector::name(register_name(&prefix, "MaxPacketSize")),
            &[FieldUpdate::new("enable", 0)],
        )
    }

    fn client_and_device(
        &self,
        target: &ControlTarget,
        bind_address: Option<&str>,
    ) -> Result<(RegisterClient, DeviceConfig), ControlError> {
        let endpoint = parse_device_endpoint(&target.device_uri)?;
        let device = self.backend.load_or_create_device_config(&endpoint)?;
        let client = RegisterClient::new(
            local_bind_addr(bind_address, self.backend.config().local_port, endpoint.addr),
            endpoint.addr,
        )
        .with_timeout(self.backend.config().request_timeout);
        Ok((client, device))
    }
}

fn summary_from_discovered(device: DiscoveredDevice) -> DeviceSummary {
    DeviceSummary {
        target: ControlTarget {
            device_uri: device.device_uri,
            transport_kind: device.transport_kind,
            auth_scope: device.auth_scope,
        },
        interface_name: device.interface_name,
        interface_address: device.interface_address,
        device_address: device.device_address,
    }
}

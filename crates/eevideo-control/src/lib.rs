use std::sync::Arc;

use eevideo_proto::{PayloadType, PixelFormat, StreamProfileId};

pub mod coap;
pub mod backend;
pub mod discovery;
pub mod register;
pub mod yaml;

pub use backend::{CoapRegisterBackend, CoapRegisterBackendConfig};
pub use coap::{
    CoapError, CoapMessage, CoapMessageType, CoapOption, OPTION_EEV_BINARY_ADDRESS,
    OPTION_EEV_REG_ACCESS,
};
pub use discovery::{
    build_discovery_request, discover_devices, parse_discovery_advertisement,
    DiscoveryAdvertisement, DiscoveryError, DiscoveryInterface, DiscoveryLink, DiscoveryResponse,
    DISCOVERY_MULTICAST_ADDR, DISCOVERY_PORT, DISCOVERY_RESOURCE_TYPE,
};
pub use register::{
    RegisterAccess, RegisterClient, RegisterError, RegisterReadKind, RegisterWriteKind,
};
pub use yaml::{
    device_config_to_string, load_embedded_feature_catalog, read_device_config,
    write_device_config, DeviceCapabilities, DeviceConfig, DeviceLocation, DeviceMemoryMap,
    DeviceRegisterValue, FeatureCatalog, FeatureDefinition, FeatureFieldDefinition,
    FeaturePointerDefinition, FeatureRegisterDefinition, YamlError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamFormatDescriptor {
    pub payload_type: PayloadType,
    pub pixel_format: PixelFormat,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlTransportKind {
    Noop,
    CoapRegister,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlTarget {
    pub device_uri: String,
    pub transport_kind: ControlTransportKind,
    pub auth_scope: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredDevice {
    pub device_uri: String,
    pub transport_kind: ControlTransportKind,
    pub interface_name: String,
    pub interface_address: String,
    pub device_address: String,
    pub auth_scope: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlCapabilities {
    pub supported_profiles: Vec<StreamProfileId>,
    pub supported_pixel_formats: Vec<PixelFormat>,
    pub multicast_supported: bool,
    pub packet_pacing_supported: bool,
    pub native_framing_supported: bool,
}

impl Default for ControlCapabilities {
    fn default() -> Self {
        Self {
            supported_profiles: vec![StreamProfileId::CompatibilityV1],
            supported_pixel_formats: Vec::new(),
            multicast_supported: true,
            packet_pacing_supported: true,
            native_framing_supported: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestedStreamConfiguration {
    pub stream_name: String,
    pub profile: StreamProfileId,
    pub destination_host: String,
    pub port: u16,
    pub bind_address: String,
    pub packet_delay_ns: u64,
    pub max_packet_size: u16,
    pub format: Option<StreamFormatDescriptor>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedStreamConfiguration {
    pub stream_id: String,
    pub stream_name: String,
    pub profile: StreamProfileId,
    pub destination_host: String,
    pub port: u16,
    pub bind_address: String,
    pub packet_delay_ns: u64,
    pub max_packet_size: u16,
    pub format: Option<StreamFormatDescriptor>,
    pub normalized: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningStream {
    pub stream_id: String,
    pub profile: StreamProfileId,
    pub running: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlErrorKind {
    Connection,
    Discovery,
    Authentication,
    UnsupportedProfile,
    InvalidConfiguration,
    ConflictingState,
    Timeout,
    AppliedValueMismatch,
    Disconnected,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlError {
    kind: ControlErrorKind,
    message: String,
}

impl ControlError {
    pub fn new(kind: ControlErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> ControlErrorKind {
        self.kind
    }
}

impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ControlError {}

pub trait ControlConnection: Send + 'static {
    fn describe(&self) -> Result<ControlCapabilities, ControlError>;
    fn configure(
        &mut self,
        request: RequestedStreamConfiguration,
    ) -> Result<AppliedStreamConfiguration, ControlError>;
    fn start(&mut self, stream_id: &str) -> Result<RunningStream, ControlError>;
    fn stop(&mut self, stream_id: &str) -> Result<(), ControlError>;
    fn disconnect(&mut self) -> Result<(), ControlError>;
}

pub trait ControlBackend: Send + Sync + 'static {
    fn discover(&self, _target: &ControlTarget) -> Result<Vec<DiscoveredDevice>, ControlError> {
        Ok(Vec::new())
    }

    fn connect(&self, target: &ControlTarget) -> Result<Box<dyn ControlConnection>, ControlError>;
}

pub type SharedControlBackend = Arc<dyn ControlBackend>;

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopControlBackend;

impl ControlBackend for NoopControlBackend {
    fn connect(&self, target: &ControlTarget) -> Result<Box<dyn ControlConnection>, ControlError> {
        Ok(Box::new(NoopControlConnection {
            target: target.clone(),
            capabilities: ControlCapabilities::default(),
            applied: None,
            disconnected: false,
        }))
    }
}

#[derive(Debug)]
struct NoopControlConnection {
    target: ControlTarget,
    capabilities: ControlCapabilities,
    applied: Option<AppliedStreamConfiguration>,
    disconnected: bool,
}

impl NoopControlConnection {
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
}

impl ControlConnection for NoopControlConnection {
    fn describe(&self) -> Result<ControlCapabilities, ControlError> {
        self.ensure_connected()?;
        Ok(self.capabilities.clone())
    }

    fn configure(
        &mut self,
        request: RequestedStreamConfiguration,
    ) -> Result<AppliedStreamConfiguration, ControlError> {
        self.ensure_connected()?;
        let applied = AppliedStreamConfiguration {
            stream_id: format!("{}#{}", self.target.device_uri, request.stream_name),
            stream_name: request.stream_name,
            profile: request.profile,
            destination_host: request.destination_host,
            port: request.port,
            bind_address: request.bind_address,
            packet_delay_ns: request.packet_delay_ns,
            max_packet_size: request.max_packet_size,
            format: request.format,
            normalized: false,
        };
        self.applied = Some(applied.clone());
        Ok(applied)
    }

    fn start(&mut self, stream_id: &str) -> Result<RunningStream, ControlError> {
        self.ensure_connected()?;
        let applied = self.applied.as_ref().ok_or_else(|| {
            ControlError::new(
                ControlErrorKind::InvalidConfiguration,
                "stream must be configured before start",
            )
        })?;

        if applied.stream_id != stream_id {
            return Err(ControlError::new(
                ControlErrorKind::AppliedValueMismatch,
                format!(
                    "start requested for stream {stream_id}, but configured stream is {}",
                    applied.stream_id
                ),
            ));
        }

        Ok(RunningStream {
            stream_id: stream_id.to_string(),
            profile: applied.profile,
            running: true,
        })
    }

    fn stop(&mut self, stream_id: &str) -> Result<(), ControlError> {
        self.ensure_connected()?;
        if let Some(applied) = &self.applied {
            if applied.stream_id != stream_id {
                return Err(ControlError::new(
                    ControlErrorKind::AppliedValueMismatch,
                    format!(
                        "stop requested for stream {stream_id}, but configured stream is {}",
                        applied.stream_id
                    ),
                ));
            }
        }
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), ControlError> {
        self.disconnected = true;
        Ok(())
    }
}

pub fn default_control_backend() -> SharedControlBackend {
    Arc::new(NoopControlBackend)
}

pub struct ControlSession {
    backend: SharedControlBackend,
    target: ControlTarget,
    requested: RequestedStreamConfiguration,
    connection: Option<Box<dyn ControlConnection>>,
    capabilities: Option<ControlCapabilities>,
    applied: Option<AppliedStreamConfiguration>,
    running: Option<RunningStream>,
}

impl Clone for ControlSession {
    fn clone(&self) -> Self {
        Self {
            backend: Arc::clone(&self.backend),
            target: self.target.clone(),
            requested: self.requested.clone(),
            connection: None,
            capabilities: self.capabilities.clone(),
            applied: self.applied.clone(),
            running: self.running.clone(),
        }
    }
}

impl ControlSession {
    pub fn new(
        backend: SharedControlBackend,
        target: ControlTarget,
        requested: RequestedStreamConfiguration,
    ) -> Self {
        Self {
            backend,
            target,
            requested,
            connection: None,
            capabilities: None,
            applied: None,
            running: None,
        }
    }

    pub fn target(&self) -> &ControlTarget {
        &self.target
    }

    pub fn requested(&self) -> &RequestedStreamConfiguration {
        &self.requested
    }

    pub fn applied(&self) -> Option<&AppliedStreamConfiguration> {
        self.applied.as_ref()
    }

    pub fn running(&self) -> Option<&RunningStream> {
        self.running.as_ref()
    }

    pub fn discover(&self) -> Result<Vec<DiscoveredDevice>, ControlError> {
        self.backend.discover(&self.target)
    }

    pub fn describe(&mut self) -> Result<&ControlCapabilities, ControlError> {
        if self.capabilities.is_none() {
            let connection = self.ensure_connection()?;
            self.capabilities = Some(connection.describe()?);
        }
        Ok(self.capabilities.as_ref().expect("capabilities populated"))
    }

    pub fn configure(
        &mut self,
        requested: RequestedStreamConfiguration,
    ) -> Result<AppliedStreamConfiguration, ControlError> {
        if self.applied.as_ref().map(|applied| applied.stream_name.as_str())
            == Some(requested.stream_name.as_str())
            && self.requested == requested
        {
            return Ok(self
                .applied
                .clone()
                .expect("matching request must have applied configuration"));
        }

        let connection = self.ensure_connection()?;
        let applied = connection.configure(requested.clone())?;
        self.requested = requested;
        self.applied = Some(applied.clone());
        self.running = None;
        Ok(applied)
    }

    pub fn start(&mut self) -> Result<RunningStream, ControlError> {
        if let Some(running) = &self.running {
            return Ok(running.clone());
        }

        let stream_id = match &self.applied {
            Some(applied) => applied.stream_id.clone(),
            None => self.configure(self.requested.clone())?.stream_id,
        };

        let connection = self.ensure_connection()?;
        let running = connection.start(&stream_id)?;
        self.running = Some(running.clone());
        Ok(running)
    }

    pub fn stop(&mut self) -> Result<(), ControlError> {
        let Some(stream_id) = self.applied.as_ref().map(|applied| applied.stream_id.clone()) else {
            return Ok(());
        };

        if self.running.is_none() {
            return Ok(());
        }

        let connection = self.ensure_connection()?;
        connection.stop(&stream_id)?;
        self.running = None;
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<(), ControlError> {
        if let Some(mut connection) = self.connection.take() {
            connection.disconnect()?;
        }
        self.running = None;
        Ok(())
    }

    fn ensure_connection(&mut self) -> Result<&mut Box<dyn ControlConnection>, ControlError> {
        if self.connection.is_none() {
            self.connection = Some(self.backend.connect(&self.target)?);
        }
        Ok(self.connection.as_mut().expect("connection initialized"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{
        default_control_backend, AppliedStreamConfiguration, ControlBackend, ControlCapabilities,
        ControlConnection, ControlError, ControlSession, ControlTarget, ControlTransportKind,
        RequestedStreamConfiguration, RunningStream, SharedControlBackend, StreamFormatDescriptor,
    };
    use eevideo_proto::{PayloadType, PixelFormat, StreamProfileId};

    #[derive(Default)]
    struct RecordingBackend {
        events: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingBackend {
        fn shared() -> (SharedControlBackend, Arc<Mutex<Vec<String>>>) {
            let backend = Self::default();
            let events = Arc::clone(&backend.events);
            (Arc::new(backend), events)
        }
    }

    struct RecordingConnection {
        events: Arc<Mutex<Vec<String>>>,
        applied: Option<AppliedStreamConfiguration>,
    }

    impl ControlConnection for RecordingConnection {
        fn describe(&self) -> Result<ControlCapabilities, ControlError> {
            self.events
                .lock()
                .expect("events lock poisoned")
                .push("describe".to_string());
            Ok(ControlCapabilities::default())
        }

        fn configure(
            &mut self,
            request: RequestedStreamConfiguration,
        ) -> Result<AppliedStreamConfiguration, ControlError> {
            self.events
                .lock()
                .expect("events lock poisoned")
                .push(format!("configure:{}", request.stream_name));
            let applied = AppliedStreamConfiguration {
                stream_id: request.stream_name.clone(),
                stream_name: request.stream_name,
                profile: request.profile,
                destination_host: request.destination_host,
                port: request.port,
                bind_address: request.bind_address,
                packet_delay_ns: request.packet_delay_ns,
                max_packet_size: request.max_packet_size,
                format: request.format,
                normalized: false,
            };
            self.applied = Some(applied.clone());
            Ok(applied)
        }

        fn start(&mut self, stream_id: &str) -> Result<RunningStream, ControlError> {
            self.events
                .lock()
                .expect("events lock poisoned")
                .push(format!("start:{stream_id}"));
            Ok(RunningStream {
                stream_id: stream_id.to_string(),
                profile: StreamProfileId::CompatibilityV1,
                running: true,
            })
        }

        fn stop(&mut self, stream_id: &str) -> Result<(), ControlError> {
            self.events
                .lock()
                .expect("events lock poisoned")
                .push(format!("stop:{stream_id}"));
            Ok(())
        }

        fn disconnect(&mut self) -> Result<(), ControlError> {
            self.events
                .lock()
                .expect("events lock poisoned")
                .push("disconnect".to_string());
            Ok(())
        }
    }

    impl ControlBackend for RecordingBackend {
        fn discover(
            &self,
            target: &ControlTarget,
        ) -> Result<Vec<super::DiscoveredDevice>, ControlError> {
            self.events
                .lock()
                .expect("events lock poisoned")
                .push(format!("discover:{}", target.device_uri));
            Ok(Vec::new())
        }

        fn connect(
            &self,
            target: &ControlTarget,
        ) -> Result<Box<dyn ControlConnection>, ControlError> {
            self.events
                .lock()
                .expect("events lock poisoned")
                .push(format!("connect:{}", target.device_uri));
            Ok(Box::new(RecordingConnection {
                events: Arc::clone(&self.events),
                applied: None,
            }))
        }
    }

    fn target() -> ControlTarget {
        ControlTarget {
            device_uri: "eevideo://device/1".to_string(),
            transport_kind: ControlTransportKind::Noop,
            auth_scope: None,
        }
    }

    fn request() -> RequestedStreamConfiguration {
        RequestedStreamConfiguration {
            stream_name: "test-stream".to_string(),
            profile: StreamProfileId::CompatibilityV1,
            destination_host: "127.0.0.1".to_string(),
            port: 5000,
            bind_address: "0.0.0.0".to_string(),
            packet_delay_ns: 0,
            max_packet_size: 1200,
            format: Some(StreamFormatDescriptor {
                payload_type: PayloadType::Image,
                pixel_format: PixelFormat::Mono8,
                width: 320,
                height: 240,
            }),
        }
    }

    #[test]
    fn noop_session_configures_starts_and_stops() {
        let mut session = ControlSession::new(default_control_backend(), target(), request());

        assert_eq!(session.describe().unwrap().supported_profiles.len(), 1);
        let applied = session.configure(request()).unwrap();
        assert_eq!(applied.stream_name, "test-stream");
        let running = session.start().unwrap();
        assert!(running.running);
        session.stop().unwrap();
        session.disconnect().unwrap();
    }

    #[test]
    fn session_uses_connection_once_and_preserves_idempotency() {
        let (backend, events) = RecordingBackend::shared();
        let mut session = ControlSession::new(backend, target(), request());

        session.configure(request()).unwrap();
        session.start().unwrap();
        session.start().unwrap();
        session.stop().unwrap();
        session.stop().unwrap();
        session.disconnect().unwrap();

        assert_eq!(
            *events.lock().expect("events lock poisoned"),
            vec![
                "connect:eevideo://device/1".to_string(),
                "configure:test-stream".to_string(),
                "start:test-stream".to_string(),
                "stop:test-stream".to_string(),
                "disconnect".to_string(),
            ]
        );
    }

    #[test]
    fn discover_delegates_to_backend_without_connecting() {
        let (backend, events) = RecordingBackend::shared();
        let session = ControlSession::new(backend, target(), request());

        session.discover().unwrap();

        assert_eq!(
            *events.lock().expect("events lock poisoned"),
            vec!["discover:eevideo://device/1".to_string()]
        );
    }
}

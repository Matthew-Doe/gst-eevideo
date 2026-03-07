use std::sync::Arc;

use eevideo_proto::{PayloadType, PixelFormat, StreamProfileId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamFormatDescriptor {
    pub payload_type: PayloadType,
    pub pixel_format: PixelFormat,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamConfiguration {
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
pub enum ControlCommand {
    Configure(StreamConfiguration),
    Start { stream_name: String },
    Stop { stream_name: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlError {
    message: String,
}

impl ControlError {
    #[allow(dead_code)]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ControlError {}

pub trait ControlBackend: Send + Sync + 'static {
    fn apply(&self, command: ControlCommand) -> Result<(), ControlError>;
}

pub type SharedControlBackend = Arc<dyn ControlBackend>;

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopControlBackend;

impl ControlBackend for NoopControlBackend {
    fn apply(&self, _command: ControlCommand) -> Result<(), ControlError> {
        Ok(())
    }
}

pub fn default_control_backend() -> SharedControlBackend {
    Arc::new(NoopControlBackend)
}

#[derive(Clone)]
pub struct ControlSession {
    backend: SharedControlBackend,
    config: StreamConfiguration,
    configured: bool,
    started: bool,
}

impl ControlSession {
    pub fn new(backend: SharedControlBackend, config: StreamConfiguration) -> Self {
        Self {
            backend,
            config,
            configured: false,
            started: false,
        }
    }

    pub fn configure(&mut self, config: StreamConfiguration) -> Result<(), ControlError> {
        if self.configured && self.config == config {
            return Ok(());
        }

        self.backend.apply(ControlCommand::Configure(config.clone()))?;
        self.config = config;
        self.configured = true;
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), ControlError> {
        if !self.configured {
            self.configure(self.config.clone())?;
        }

        if self.started {
            return Ok(());
        }

        self.backend.apply(ControlCommand::Start {
            stream_name: self.config.stream_name.clone(),
        })?;
        self.started = true;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), ControlError> {
        if !self.started {
            return Ok(());
        }

        self.backend.apply(ControlCommand::Stop {
            stream_name: self.config.stream_name.clone(),
        })?;
        self.started = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use eevideo_proto::{PayloadType, PixelFormat, StreamProfileId};

    use super::{
        ControlBackend, ControlCommand, ControlSession, SharedControlBackend, StreamConfiguration,
        StreamFormatDescriptor,
    };

    #[derive(Clone, Default)]
    struct RecordingBackend {
        commands: Arc<Mutex<Vec<ControlCommand>>>,
    }

    impl RecordingBackend {
        fn shared() -> (SharedControlBackend, Arc<Mutex<Vec<ControlCommand>>>) {
            let backend = Self::default();
            let commands = Arc::clone(&backend.commands);
            (Arc::new(backend), commands)
        }
    }

    impl ControlBackend for RecordingBackend {
        fn apply(&self, command: ControlCommand) -> Result<(), super::ControlError> {
            self.commands.lock().expect("commands lock poisoned").push(command);
            Ok(())
        }
    }

    fn configuration() -> StreamConfiguration {
        StreamConfiguration {
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
    fn control_session_configures_then_starts_and_stops_once() {
        let (backend, commands) = RecordingBackend::shared();
        let mut session = ControlSession::new(backend, configuration());

        session.start().unwrap();
        session.start().unwrap();
        session.stop().unwrap();
        session.stop().unwrap();

        assert_eq!(
            *commands.lock().expect("commands lock poisoned"),
            vec![
                ControlCommand::Configure(configuration()),
                ControlCommand::Start {
                    stream_name: "test-stream".to_string(),
                },
                ControlCommand::Stop {
                    stream_name: "test-stream".to_string(),
                },
            ]
        );
    }

    #[test]
    fn configure_skips_duplicate_configuration() {
        let (backend, commands) = RecordingBackend::shared();
        let config = configuration();
        let mut session = ControlSession::new(backend, config.clone());

        session.configure(config.clone()).unwrap();
        session.configure(config).unwrap();

        assert_eq!(commands.lock().expect("commands lock poisoned").len(), 1);
    }
}

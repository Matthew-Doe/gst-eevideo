use std::net::IpAddr;

#[derive(Clone, Debug)]
pub struct StreamConfiguration {
    pub stream_name: String,
    pub destination: IpAddr,
    pub port: u16,
    pub delay_ticks: u32,
    pub max_packet_size: u16,
}

pub trait ControlBackend: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn configure_stream(&self, config: &StreamConfiguration) -> Result<(), Self::Error>;
    fn start_stream(&self, stream_name: &str) -> Result<(), Self::Error>;
    fn stop_stream(&self, stream_name: &str) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopControlBackend;

#[derive(Clone, Debug)]
pub struct NoopControlError;

impl std::fmt::Display for NoopControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("control backend not configured for v1")
    }
}

impl std::error::Error for NoopControlError {}

impl ControlBackend for NoopControlBackend {
    type Error = NoopControlError;

    fn configure_stream(&self, _config: &StreamConfiguration) -> Result<(), Self::Error> {
        Ok(())
    }

    fn start_stream(&self, _stream_name: &str) -> Result<(), Self::Error> {
        Ok(())
    }

    fn stop_stream(&self, _stream_name: &str) -> Result<(), Self::Error> {
        Ok(())
    }
}


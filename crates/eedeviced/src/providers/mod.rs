use anyhow::{bail, Result};
use eevideo_device::{
    CaptureBackend, CaptureConfiguration, SyntheticCaptureBackend, SyntheticCaptureConfig,
};
use eevideo_proto::PixelFormat;

use crate::DeviceDaemonConfig;

mod gstreamer;

pub(crate) use gstreamer::{GstreamerCaptureBackend, GstreamerProviderConfig};

#[cfg(test)]
pub(crate) use gstreamer::{
    build_argus_pipeline_description, build_v4l2_pipeline_description, capture_format_from_caps,
    ensure_gstreamer_init_for_tests, start_backend_for_test, validate_packed_buffer_len,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderConfig {
    Synthetic,
    Argus { sensor_id: u32 },
    V4l2 { device: String },
    Pipeline { description: String },
}

#[derive(Debug)]
pub(crate) enum ProviderBackend {
    Synthetic(SyntheticCaptureBackend),
    Gstreamer(GstreamerCaptureBackend),
}

impl CaptureBackend for ProviderBackend {
    fn start_capture(&mut self, config: CaptureConfiguration) -> Result<()> {
        match self {
            Self::Synthetic(backend) => backend.start_capture(config),
            Self::Gstreamer(backend) => backend.start_capture(config),
        }
    }

    fn stop_capture(&mut self) -> Result<()> {
        match self {
            Self::Synthetic(backend) => backend.stop_capture(),
            Self::Gstreamer(backend) => backend.stop_capture(),
        }
    }

    fn next_frame(&mut self) -> Result<eevideo_proto::VideoFrame> {
        match self {
            Self::Synthetic(backend) => backend.next_frame(),
            Self::Gstreamer(backend) => backend.next_frame(),
        }
    }

    fn current_format(&self) -> Option<CaptureConfiguration> {
        match self {
            Self::Synthetic(backend) => backend.current_format(),
            Self::Gstreamer(backend) => backend.current_format(),
        }
    }
}

pub(crate) fn build_capture_backend(config: &DeviceDaemonConfig) -> ProviderBackend {
    match &config.provider {
        ProviderConfig::Synthetic => ProviderBackend::Synthetic(SyntheticCaptureBackend::new(
            SyntheticCaptureConfig::default(),
        )),
        ProviderConfig::Argus { sensor_id } => ProviderBackend::Gstreamer(
            GstreamerCaptureBackend::new(GstreamerProviderConfig::Argus {
                sensor_id: *sensor_id,
            }),
        ),
        ProviderConfig::V4l2 { device } => ProviderBackend::Gstreamer(
            GstreamerCaptureBackend::new(GstreamerProviderConfig::V4l2 {
                device: device.clone(),
            }),
        ),
        ProviderConfig::Pipeline { description } => ProviderBackend::Gstreamer(
            GstreamerCaptureBackend::new(GstreamerProviderConfig::Pipeline {
                description: description.clone(),
            }),
        ),
    }
}

pub(crate) fn validate_provider_config(config: &DeviceDaemonConfig) -> Result<()> {
    match &config.provider {
        ProviderConfig::Synthetic => Ok(()),
        ProviderConfig::Argus { .. } => {
            if config.pixel_format != PixelFormat::Uyvy {
                bail!("argus provider only supports UYVY output");
            }
            Ok(())
        }
        ProviderConfig::V4l2 { device } => {
            if device.trim().is_empty() {
                bail!("v4l2 provider device path must not be empty");
            }
            Ok(())
        }
        ProviderConfig::Pipeline { description } => {
            if description.trim().is_empty() {
                bail!("pipeline provider description must not be empty");
            }
            Ok(())
        }
    }
}

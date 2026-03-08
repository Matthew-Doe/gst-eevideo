use std::sync::OnceLock;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use eevideo_device::{CaptureBackend, CaptureConfiguration};
use eevideo_proto::{PayloadType, PixelFormat, VideoFrame};
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;

const FRAME_WAIT_MS: u64 = 250;
const FRAME_STARTUP_WAIT_MS: u64 = 1_000;
const FRAME_SINK_NAME: &str = "framesink";

static GST_INIT: OnceLock<std::result::Result<(), String>> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GstreamerProviderConfig {
    Argus { sensor_id: u32 },
    V4l2 { device: String },
    Pipeline { description: String },
}

#[derive(Debug)]
struct GstreamerCaptureState {
    pipeline: gst::Pipeline,
    sink: gst_app::AppSink,
    pending_sample: Option<gst::Sample>,
    started_at: Instant,
    next_frame_id: u32,
    current_format: CaptureConfiguration,
}

#[derive(Debug)]
pub(crate) struct GstreamerCaptureBackend {
    provider: GstreamerProviderConfig,
    state: Option<GstreamerCaptureState>,
}

impl GstreamerCaptureBackend {
    pub(crate) fn new(provider: GstreamerProviderConfig) -> Self {
        Self {
            provider,
            state: None,
        }
    }
}

impl CaptureBackend for GstreamerCaptureBackend {
    fn start_capture(&mut self, config: CaptureConfiguration) -> Result<()> {
        ensure_gstreamer_init()?;
        validate_provider_capture_config(&self.provider, &config)?;
        if self.state.is_some() {
            self.stop_capture()?;
        }

        let provider_name = provider_name(&self.provider);
        let description = build_pipeline_description(&self.provider, &config);
        let element = gst::parse::launch(&description)
            .with_context(|| format!("failed to build {provider_name} pipeline: {description}"))?;
        let pipeline = element.downcast::<gst::Pipeline>().map_err(|_| {
            anyhow!("{provider_name} pipeline description did not produce a gst::Pipeline")
        })?;
        let sink = pipeline
            .by_name(FRAME_SINK_NAME)
            .ok_or_else(|| {
                anyhow!("{provider_name} pipeline does not expose {FRAME_SINK_NAME} appsink")
            })?
            .downcast::<gst_app::AppSink>()
            .map_err(|_| anyhow!("{provider_name} {FRAME_SINK_NAME} element is not an appsink"))?;

        pipeline
            .set_state(gst::State::Playing)
            .with_context(|| format!("failed to start {provider_name} pipeline"))?;

        let sample = pull_sample(&sink, FRAME_STARTUP_WAIT_MS, provider_name)?;
        let actual_format = format_from_sample(&sample, config.fps)?;
        validate_expected_format(provider_name, &config, &actual_format)?;
        validate_sample_payload(provider_name, &sample, &actual_format)?;

        self.state = Some(GstreamerCaptureState {
            pipeline,
            sink,
            pending_sample: Some(sample),
            started_at: Instant::now(),
            next_frame_id: 1,
            current_format: actual_format,
        });
        Ok(())
    }

    fn stop_capture(&mut self) -> Result<()> {
        let provider_name = provider_name(&self.provider);
        if let Some(state) = self.state.take() {
            state
                .pipeline
                .set_state(gst::State::Null)
                .with_context(|| format!("failed to stop {provider_name} pipeline"))?;
        }
        Ok(())
    }

    fn next_frame(&mut self) -> Result<VideoFrame> {
        let provider_name = provider_name(&self.provider);
        let state = self
            .state
            .as_mut()
            .ok_or_else(|| anyhow!("{provider_name} capture is not running"))?;
        let sample = if let Some(sample) = state.pending_sample.take() {
            sample
        } else {
            pull_sample(&state.sink, FRAME_WAIT_MS, provider_name)?
        };

        let actual_format = format_from_sample(&sample, state.current_format.fps)?;
        validate_expected_format(provider_name, &state.current_format, &actual_format)?;
        validate_sample_payload(provider_name, &sample, &actual_format)?;
        sample_to_frame(provider_name, state, sample)
    }

    fn current_format(&self) -> Option<CaptureConfiguration> {
        self.state
            .as_ref()
            .map(|state| state.current_format.clone())
    }
}

fn validate_provider_capture_config(
    provider: &GstreamerProviderConfig,
    config: &CaptureConfiguration,
) -> Result<()> {
    if config.pixel_format == PixelFormat::Uyvy && config.width % 2 != 0 {
        bail!("UYVY output width must be even");
    }
    if matches!(provider, GstreamerProviderConfig::Argus { .. })
        && config.pixel_format != PixelFormat::Uyvy
    {
        bail!("argus provider only supports UYVY output");
    }
    Ok(())
}

fn build_pipeline_description(
    provider: &GstreamerProviderConfig,
    config: &CaptureConfiguration,
) -> String {
    match provider {
        GstreamerProviderConfig::Argus { sensor_id } => {
            build_argus_pipeline_description(*sensor_id, config)
        }
        GstreamerProviderConfig::V4l2 { device } => build_v4l2_pipeline_description(device, config),
        GstreamerProviderConfig::Pipeline { description } => description.clone(),
    }
}

pub(crate) fn build_argus_pipeline_description(
    sensor_id: u32,
    config: &CaptureConfiguration,
) -> String {
    format!(
        concat!(
            "nvarguscamerasrc sensor-id={sensor_id} ! ",
            "video/x-raw(memory:NVMM),width={width},height={height},framerate={fps}/1 ! ",
            "nvvidconv ! ",
            "video/x-raw,format=UYVY,width={width},height={height} ! ",
            "appsink name={sink} sync=false max-buffers=1 drop=true"
        ),
        sensor_id = sensor_id,
        width = config.width,
        height = config.height,
        fps = config.fps,
        sink = FRAME_SINK_NAME,
    )
}

pub(crate) fn build_v4l2_pipeline_description(
    device: &str,
    config: &CaptureConfiguration,
) -> String {
    format!(
        concat!(
            "v4l2src device={device} ! ",
            "{media_type},format={format},width={width},height={height},framerate={fps}/1 ! ",
            "appsink name={sink} sync=false max-buffers=1 drop=true"
        ),
        device = device,
        media_type = config.pixel_format.media_type(),
        format = config.pixel_format.gst_format(),
        width = config.width,
        height = config.height,
        fps = config.fps,
        sink = FRAME_SINK_NAME,
    )
}

fn pull_sample(sink: &gst_app::AppSink, wait_ms: u64, provider_name: &str) -> Result<gst::Sample> {
    sink.try_pull_sample(gst::ClockTime::from_mseconds(wait_ms))
        .ok_or_else(|| anyhow!("timed out waiting for a {provider_name} frame"))
}

fn format_from_sample(sample: &gst::Sample, fps: u32) -> Result<CaptureConfiguration> {
    let caps = sample
        .caps()
        .ok_or_else(|| anyhow!("sample did not expose caps"))?;
    capture_format_from_caps(caps, fps)
}

pub(crate) fn capture_format_from_caps(
    caps: &gst::CapsRef,
    fps: u32,
) -> Result<CaptureConfiguration> {
    let structure = caps
        .structure(0)
        .ok_or_else(|| anyhow!("caps did not include a structure"))?;
    let media_type = structure.name();
    let format = structure
        .get::<String>("format")
        .context("caps did not expose a format field")?;
    let width = structure
        .get::<i32>("width")
        .context("caps did not expose a width field")?;
    let height = structure
        .get::<i32>("height")
        .context("caps did not expose a height field")?;
    if width <= 0 || height <= 0 {
        bail!("caps exposed invalid dimensions {width}x{height}");
    }

    Ok(CaptureConfiguration {
        width: width as u32,
        height: height as u32,
        pixel_format: PixelFormat::from_caps(media_type, &format)?,
        fps,
    })
}

fn validate_expected_format(
    provider_name: &str,
    requested: &CaptureConfiguration,
    actual: &CaptureConfiguration,
) -> Result<()> {
    if requested.width != actual.width
        || requested.height != actual.height
        || requested.pixel_format != actual.pixel_format
    {
        bail!(
            "{provider_name} provider negotiated {}x{} {:?}, expected {}x{} {:?}",
            actual.width,
            actual.height,
            actual.pixel_format,
            requested.width,
            requested.height,
            requested.pixel_format
        );
    }
    Ok(())
}

fn validate_sample_payload(
    provider_name: &str,
    sample: &gst::Sample,
    format: &CaptureConfiguration,
) -> Result<()> {
    let buffer = sample
        .buffer()
        .ok_or_else(|| anyhow!("{provider_name} sample did not include a buffer"))?;
    validate_packed_buffer_len(format, buffer.size())
}

pub(crate) fn validate_packed_buffer_len(
    format: &CaptureConfiguration,
    actual_len: usize,
) -> Result<()> {
    let expected_len = format
        .pixel_format
        .payload_len(format.width, format.height)
        .context("invalid packed frame dimensions")?;
    if actual_len != expected_len {
        bail!("payload length mismatch: expected {expected_len}, got {actual_len}");
    }
    Ok(())
}

fn sample_to_frame(
    provider_name: &str,
    state: &mut GstreamerCaptureState,
    sample: gst::Sample,
) -> Result<VideoFrame> {
    let buffer = sample
        .buffer_owned()
        .ok_or_else(|| anyhow!("{provider_name} sample did not include a buffer"))?;
    let map = buffer
        .map_readable()
        .map_err(|_| anyhow!("failed to map {provider_name} sample buffer"))?;
    validate_packed_buffer_len(&state.current_format, map.as_slice().len())?;

    let frame_id = state.next_frame_id;
    state.next_frame_id = state.next_frame_id.wrapping_add(1).max(1);
    let timestamp = buffer.pts().map(|pts| pts.nseconds()).unwrap_or_else(|| {
        state
            .started_at
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64
    });

    Ok(VideoFrame {
        frame_id,
        timestamp,
        width: state.current_format.width,
        height: state.current_format.height,
        pixel_format: state.current_format.pixel_format,
        payload_type: PayloadType::Image,
        data: map.as_slice().to_vec(),
    })
}

fn provider_name(provider: &GstreamerProviderConfig) -> &'static str {
    match provider {
        GstreamerProviderConfig::Argus { .. } => "argus",
        GstreamerProviderConfig::V4l2 { .. } => "v4l2",
        GstreamerProviderConfig::Pipeline { .. } => "pipeline",
    }
}

fn ensure_gstreamer_init() -> Result<()> {
    GST_INIT
        .get_or_init(|| gst::init().map_err(|err| err.to_string()))
        .clone()
        .map_err(anyhow::Error::msg)
}

#[cfg(test)]
pub(crate) fn ensure_gstreamer_init_for_tests() -> Result<()> {
    ensure_gstreamer_init()
}

#[cfg(test)]
pub(crate) fn start_backend_for_test(
    backend: &mut GstreamerCaptureBackend,
    config: CaptureConfiguration,
) -> Result<()> {
    backend.start_capture(config)
}

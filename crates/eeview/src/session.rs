use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use gst::prelude::*;
use gsteevideo::eevideo_control::{ControlTarget, SharedControlBackend};
use gstreamer as gst;
use gstreamer_app as gst_app;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagedTransportSettings {
    pub max_packet_size: u16,
    pub packet_delay_ns: u64,
}

impl Default for ManagedTransportSettings {
    fn default() -> Self {
        Self {
            max_packet_size: 1400,
            packet_delay_ns: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordingEncoder {
    Av1,
    Vp9,
    Theora,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordingConfig {
    pub path: PathBuf,
    pub encoder: Option<RecordingEncoder>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewerSessionConfig {
    pub target: ControlTarget,
    pub bind_address: String,
    pub port: u32,
    pub source_timeout: Duration,
    pub latency: Duration,
    pub stream_name: String,
    pub managed_transport: ManagedTransportSettings,
    pub recording: Option<RecordingConfig>,
    pub overlay_text: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoFrame {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewerStats {
    pub frames_received: u64,
    pub frames_dropped: u64,
    pub packet_anomalies: u64,
    pub timeout_drops: u64,
    pub payload_overflow_drops: u64,
    pub short_frame_drops: u64,
    pub duplicate_leader_drops: u64,
    pub payload_before_leader_drops: u64,
    pub trailer_before_leader_drops: u64,
    pub packet_after_trailer_drops: u64,
    pub parse_failures: u64,
    pub expected_format_mismatches: u64,
    pub midstream_format_changes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerState {
    Starting,
    Playing,
    Stopping,
    Stopped,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewerEvent {
    Frame(VideoFrame),
    Stats(ViewerStats),
    State(ViewerState),
    Error(String),
    Eos,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EncoderSpec {
    kind: RecordingEncoder,
    encoder_factory: &'static str,
    mux_factory: &'static str,
    mux_pad_template: &'static str,
}

const ENCODER_SPECS: &[EncoderSpec] = &[
    EncoderSpec {
        kind: RecordingEncoder::Av1,
        encoder_factory: "rav1enc",
        mux_factory: "matroskamux",
        mux_pad_template: "video_%u",
    },
    EncoderSpec {
        kind: RecordingEncoder::Vp9,
        encoder_factory: "vp9enc",
        mux_factory: "webmmux",
        mux_pad_template: "video_%u",
    },
    EncoderSpec {
        kind: RecordingEncoder::Theora,
        encoder_factory: "theoraenc",
        mux_factory: "oggmux",
        mux_pad_template: "video_%u",
    },
];

pub struct ViewerPipeline {
    pipeline: gst::Pipeline,
    bus_thread: Option<thread::JoinHandle<()>>,
    events: Receiver<ViewerEvent>,
}

impl ViewerPipeline {
    pub fn start(config: ViewerSessionConfig, backend: SharedControlBackend) -> Result<Self> {
        let pipeline = build_viewer_pipeline(&config, backend)?;
        let (event_tx, events) = mpsc::channel();
        configure_appsink_events(&pipeline, event_tx.clone())?;
        let bus = pipeline.bus().context("pipeline did not expose a bus")?;

        event_tx.send(ViewerEvent::State(ViewerState::Starting)).ok();
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|_| anyhow!("failed to start viewer pipeline"))?;
        event_tx.send(ViewerEvent::State(ViewerState::Playing)).ok();

        let bus_thread = thread::spawn(move || {
            while let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(200)) {
                match message.view() {
                    gst::MessageView::Eos(..) => {
                        event_tx.send(ViewerEvent::Eos).ok();
                        break;
                    }
                    gst::MessageView::Error(err) => {
                        event_tx
                            .send(ViewerEvent::Error(format_bus_error(err)))
                            .ok();
                        break;
                    }
                    _ => {}
                }
            }
            event_tx.send(ViewerEvent::State(ViewerState::Stopped)).ok();
        });

        Ok(Self {
            pipeline,
            bus_thread: Some(bus_thread),
            events,
        })
    }

    pub fn stop(mut self) -> Result<ViewerStats> {
        let _ = self.pipeline.send_event(gst::event::Eos::new());
        self.pipeline
            .set_state(gst::State::Null)
            .map(|_| ())
            .context("failed to stop viewer pipeline")?;
        if let Some(handle) = self.bus_thread.take() {
            let _ = handle.join();
        }
        Ok(read_viewer_stats(&self.pipeline))
    }

    pub fn events(&self) -> &Receiver<ViewerEvent> {
        &self.events
    }
}

pub fn build_viewer_pipeline(
    config: &ViewerSessionConfig,
    backend: SharedControlBackend,
) -> Result<gst::Pipeline> {
    validate_session_config(config)?;

    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("eevideosrc")
        .name("eeview-source")
        .property("address", &config.bind_address)
        .property("port", config.port)
        .property("timeout-ms", config.source_timeout.as_millis() as u64)
        .property("latency-ms", config.latency.as_millis() as u64)
        .property(
            "managed-max-packet-size",
            u32::from(config.managed_transport.max_packet_size),
        )
        .property(
            "managed-packet-delay-ns",
            config.managed_transport.packet_delay_ns,
        )
        .build()
        .context("failed to create eevideosrc")?;

    gsteevideo::configure_source_control(
        &src,
        backend,
        config.target.clone(),
        config.stream_name.clone(),
    )
    .context("failed to configure managed source control")?;
    pipeline.add(&src)?;

    if let Some(recording) = &config.recording {
        let spec = select_encoder(recording.encoder)?;
        let tee = make_element("tee", Some("display-record-tee"))?;
        let display_queue = add_appsink_display_branch(&pipeline)?;
        let record_queue = make_element("queue", Some("record-queue"))?;
        let record_convert = make_element("videoconvert", Some("record-convert"))?;
        let encoder = make_element(spec.encoder_factory, Some("record-encoder"))?;
        let mux = make_element(spec.mux_factory, Some("record-mux"))?;
        let file_sink = make_element("filesink", Some("record-file-sink"))?;
        file_sink.set_property(
            "location",
            recording
                .path
                .to_str()
                .ok_or_else(|| anyhow!("record path must be valid UTF-8"))?,
        );

        pipeline.add_many([
            &tee,
            &record_queue,
            &record_convert,
            &encoder,
            &mux,
            &file_sink,
        ])?;
        src.link(&tee)?;
        link_tee_branch(&tee, &display_queue)?;
        link_tee_branch(&tee, &record_queue)?;
        gst::Element::link_many([&record_queue, &record_convert, &encoder])?;
        link_into_mux(&encoder, &mux, spec.mux_pad_template)?;
        mux.link(&file_sink)?;
    } else {
        let display_queue = add_appsink_display_branch(&pipeline)?;
        gst::Element::link_many([&src, &display_queue])?;
    }

    Ok(pipeline)
}

pub fn validate_session_config(config: &ViewerSessionConfig) -> Result<()> {
    if config.target.device_uri.trim().is_empty() {
        bail!("device URI is required");
    }
    if config.bind_address.trim().is_empty() {
        bail!("bind address is required");
    }
    if config.stream_name.trim().is_empty() {
        bail!("stream name is required");
    }
    if let Some(recording) = &config.recording {
        if recording.path.as_os_str().is_empty() {
            bail!("record path is required when recording is enabled");
        }
    }
    Ok(())
}

fn add_appsink_display_branch(pipeline: &gst::Pipeline) -> Result<gst::Element> {
    let queue = make_element("queue", Some("display-queue"))?;
    let convert = make_element("videoconvert", Some("display-convert"))?;
    let capsfilter = make_element("capsfilter", Some("display-rgba-caps"))?;
    let sink = make_element("appsink", Some("display-appsink"))?;
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .build();
    capsfilter.set_property("caps", &caps);
    sink.set_property("caps", &caps);
    sink.set_property("sync", false);
    sink.set_property("max-buffers", 2u32);
    sink.set_property("drop", true);
    pipeline.add_many([&queue, &convert, &capsfilter, &sink])?;
    gst::Element::link_many([&queue, &convert, &capsfilter, &sink])?;
    Ok(queue)
}

fn configure_appsink_events(
    pipeline: &gst::Pipeline,
    event_tx: mpsc::Sender<ViewerEvent>,
) -> Result<()> {
    let sink = pipeline
        .by_name("display-appsink")
        .ok_or_else(|| anyhow!("display appsink is missing"))?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| anyhow!("display-appsink is not an appsink"))?;

    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                match frame_from_sample(&sample) {
                    Ok(frame) => {
                        event_tx.send(ViewerEvent::Frame(frame)).ok();
                    }
                    Err(err) => {
                        event_tx.send(ViewerEvent::Error(format!("{err:#}"))).ok();
                    }
                }
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    Ok(())
}

fn frame_from_sample(sample: &gst::Sample) -> Result<VideoFrame> {
    let caps = sample.caps().context("sample is missing caps")?;
    let structure = caps.structure(0).context("sample caps are empty")?;
    let width = structure.get::<i32>("width")?;
    let height = structure.get::<i32>("height")?;
    let buffer = sample.buffer().context("sample is missing buffer")?;
    let map = buffer.map_readable().context("failed to map video frame")?;

    Ok(VideoFrame {
        width,
        height,
        rgba: map.as_slice().to_vec(),
    })
}

fn select_encoder(requested: Option<RecordingEncoder>) -> Result<EncoderSpec> {
    if let Some(requested) = requested {
        let spec = ENCODER_SPECS
            .iter()
            .copied()
            .find(|spec| spec.kind == requested)
            .expect("requested encoder exists in static table");
        ensure_elements_available(spec)?;
        return Ok(spec);
    }

    for spec in ENCODER_SPECS {
        if ensure_elements_available(*spec).is_ok() {
            return Ok(*spec);
        }
    }
    bail!("no supported open encoder/mux pair is available")
}

fn ensure_elements_available(spec: EncoderSpec) -> Result<()> {
    for factory in [spec.encoder_factory, spec.mux_factory] {
        if gst::ElementFactory::find(factory).is_none() {
            bail!("required GStreamer element {factory} is not available");
        }
    }
    Ok(())
}

fn make_element(factory: &str, name: Option<&str>) -> Result<gst::Element> {
    let builder = gst::ElementFactory::make(factory);
    let builder = if let Some(name) = name {
        builder.name(name)
    } else {
        builder
    };
    builder
        .build()
        .with_context(|| format!("failed to create GStreamer element {factory}"))
}

fn link_tee_branch(tee: &gst::Element, branch_sink: &gst::Element) -> Result<()> {
    let tee_pad = tee
        .request_pad_simple("src_%u")
        .ok_or_else(|| anyhow!("failed to request tee source pad"))?;
    let sink_pad = branch_sink
        .static_pad("sink")
        .ok_or_else(|| anyhow!("branch sink element does not expose a sink pad"))?;
    tee_pad
        .link(&sink_pad)
        .map_err(|err| anyhow!("failed to link tee branch: {err:?}"))?;
    Ok(())
}

fn link_into_mux(upstream: &gst::Element, mux: &gst::Element, pad_template: &str) -> Result<()> {
    let src_pad = upstream
        .static_pad("src")
        .ok_or_else(|| anyhow!("upstream encoder does not expose a src pad"))?;
    let mux_pad = mux
        .request_pad_simple(pad_template)
        .ok_or_else(|| anyhow!("failed to request mux pad {pad_template}"))?;
    src_pad
        .link(&mux_pad)
        .map_err(|err| anyhow!("failed to link encoder into mux: {err:?}"))?;
    Ok(())
}

fn read_viewer_stats(pipeline: &gst::Pipeline) -> ViewerStats {
    let Some(source) = pipeline.by_name("eeview-source") else {
        return ViewerStats::default();
    };

    ViewerStats {
        frames_received: read_u64_property(&source, "frames-received"),
        frames_dropped: read_u64_property(&source, "frames-dropped"),
        packet_anomalies: read_u64_property(&source, "packet-anomalies"),
        timeout_drops: read_u64_property(&source, "timeout-drops"),
        payload_overflow_drops: read_u64_property(&source, "payload-overflow-drops"),
        short_frame_drops: read_u64_property(&source, "short-frame-drops"),
        duplicate_leader_drops: read_u64_property(&source, "duplicate-leader-drops"),
        payload_before_leader_drops: read_u64_property(&source, "payload-before-leader-drops"),
        trailer_before_leader_drops: read_u64_property(&source, "trailer-before-leader-drops"),
        packet_after_trailer_drops: read_u64_property(&source, "packet-after-trailer-drops"),
        parse_failures: read_u64_property(&source, "parse-failures"),
        expected_format_mismatches: read_u64_property(&source, "expected-format-mismatches"),
        midstream_format_changes: read_u64_property(&source, "midstream-format-changes"),
    }
}

fn read_u64_property(element: &gst::Element, property_name: &str) -> u64 {
    if element.find_property(property_name).is_some() {
        element.property(property_name)
    } else {
        0
    }
}

fn format_bus_error(err: &gst::message::Error) -> String {
    let src = err
        .src()
        .map(|src| src.path_string().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let debug = err.debug().unwrap_or_default();
    if debug.is_empty() {
        format!("pipeline error from {src}: {}", err.error())
    } else {
        format!("pipeline error from {src}: {} ({debug})", err.error())
    }
}

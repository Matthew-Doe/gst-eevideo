use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, ValueEnum};
use gst::prelude::*;
use gsteevideo::eevideo_control::{
    AdvertisedStreamMode, CoapRegisterBackendConfig, ControlTarget, ControlTransportKind,
    DeviceController, DeviceDescription,
};
use gstreamer as gst;

#[derive(Debug, Parser)]
#[command(name = "eeview", about = "EEVideo live view and recorder CLI")]
pub struct Cli {
    #[arg(long)]
    device_uri: Option<String>,
    #[arg(long)]
    iface: Option<String>,
    #[arg(long, default_value_t = 1000)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 0)]
    local_port: u16,
    #[arg(long)]
    yaml_root: Option<PathBuf>,
    #[arg(long)]
    bind_address: String,
    #[arg(long, default_value_t = 5000)]
    port: u32,
    #[arg(long, default_value_t = 2000)]
    source_timeout_ms: u64,
    #[arg(long, default_value_t = 0)]
    latency_ms: u64,
    #[arg(long, default_value = "stream0")]
    stream_name: String,
    #[arg(long)]
    max_packet_size: Option<u16>,
    #[arg(long)]
    packet_delay_ns: Option<u64>,
    #[arg(long, default_value = "autovideosink")]
    video_sink: String,
    #[arg(long, default_value_t = false)]
    no_overlay: bool,
    #[arg(long)]
    record: Option<PathBuf>,
    #[arg(long)]
    encoder: Option<EncoderKind>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum EncoderKind {
    Av1,
    Vp9,
    Theora,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EncoderSpec {
    kind: EncoderKind,
    encoder_factory: &'static str,
    mux_factory: &'static str,
    mux_pad_template: &'static str,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SourceStats {
    frames_received: u64,
    frames_dropped: u64,
    packet_anomalies: u64,
    timeout_drops: u64,
    payload_overflow_drops: u64,
    short_frame_drops: u64,
    duplicate_leader_drops: u64,
    payload_before_leader_drops: u64,
    trailer_before_leader_drops: u64,
    packet_after_trailer_drops: u64,
    parse_failures: u64,
    expected_format_mismatches: u64,
    midstream_format_changes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ManagedTransportSettings {
    max_packet_size: u16,
    packet_delay_ns: u64,
}

const ENCODER_SPECS: &[EncoderSpec] = &[
    EncoderSpec {
        kind: EncoderKind::Av1,
        encoder_factory: "rav1enc",
        mux_factory: "matroskamux",
        mux_pad_template: "video_%u",
    },
    EncoderSpec {
        kind: EncoderKind::Vp9,
        encoder_factory: "vp9enc",
        mux_factory: "webmmux",
        mux_pad_template: "video_%u",
    },
    EncoderSpec {
        kind: EncoderKind::Theora,
        encoder_factory: "theoraenc",
        mux_factory: "oggmux",
        mux_pad_template: "video_%u",
    },
];

const DEFAULT_MANAGED_MAX_PACKET_SIZE: u16 = 1400;
const DEFAULT_MANAGED_PACKET_DELAY_NS: u64 = 0;

pub fn main_entry<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    run(cli)
}

pub fn run(cli: Cli) -> Result<()> {
    gst::init()?;
    gsteevideo::register_static().context("failed to register EEVideo plugin")?;

    let controller = DeviceController::new(CoapRegisterBackendConfig {
        interface_name: cli.iface.clone(),
        bind_address: Some(cli.bind_address.clone()),
        discovery_timeout: Duration::from_millis(cli.timeout_ms),
        request_timeout: Duration::from_millis(cli.timeout_ms),
        yaml_root: cli.yaml_root.clone(),
        local_port: cli.local_port,
    });
    let target = resolve_target(&controller, cli.device_uri.as_deref())?;
    let overlay_text = if cli.no_overlay {
        None
    } else {
        controller
            .describe(&target)
            .ok()
            .as_ref()
            .and_then(|description| advertised_stream_overlay_text(description, &cli.stream_name))
    };
    let managed_transport =
        select_managed_transport_settings(cli.max_packet_size, cli.packet_delay_ns);
    let recording_spec = if cli.record.is_some() {
        Some(select_encoder(cli.encoder)?)
    } else {
        None
    };

    let pipeline = build_pipeline(
        &cli,
        controller.shared_backend(),
        target,
        managed_transport,
        recording_spec,
        overlay_text.as_deref(),
    )?;
    let bus = pipeline.bus().context("pipeline did not expose a bus")?;
    let (interrupt_tx, interrupt_rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = interrupt_tx.send(());
    })
    .context("failed to install Ctrl+C handler")?;

    let run_result = (|| -> Result<()> {
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|_| pipeline_start_error(&bus))?;

        loop {
            if interrupt_rx.try_recv().is_ok() {
                let _ = pipeline.send_event(gst::event::Eos::new());
                wait_for_terminal_bus_message(&bus, Duration::from_secs(5))?;
                break;
            }

            if let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(200)) {
                match message.view() {
                    gst::MessageView::Eos(..) => break,
                    gst::MessageView::Error(err) => return Err(anyhow!(format_bus_error(err))),
                    _ => {}
                }
            }
        }

        Ok(())
    })();

    let stop_result = pipeline
        .set_state(gst::State::Null)
        .map(|_| ())
        .context("failed to stop pipeline");
    let source_stats = read_source_stats(&pipeline);
    eprintln!("eeview source stats: {}", format_source_stats(source_stats));

    finalize_run_result(run_result, stop_result, source_stats)
}

fn build_pipeline(
    cli: &Cli,
    backend: gsteevideo::eevideo_control::SharedControlBackend,
    target: ControlTarget,
    managed_transport: ManagedTransportSettings,
    recording_spec: Option<EncoderSpec>,
    overlay_text: Option<&str>,
) -> Result<gst::Pipeline> {
    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("eevideosrc")
        .name("eeview-source")
        .property("address", &cli.bind_address)
        .property("port", cli.port)
        .property("timeout-ms", cli.source_timeout_ms)
        .property("latency-ms", cli.latency_ms)
        .property(
            "managed-max-packet-size",
            u32::from(managed_transport.max_packet_size),
        )
        .property("managed-packet-delay-ns", managed_transport.packet_delay_ns)
        .build()
        .context("failed to create eevideosrc")?;

    gsteevideo::configure_source_control(&src, backend, target, cli.stream_name.clone())
        .context("failed to configure managed source control")?;

    pipeline.add(&src)?;

    if let Some(recording_spec) = recording_spec {
        let tee = make_element("tee", None)?;
        let record_queue = make_element("queue", Some("record-queue"))?;
        let record_convert = make_element("videoconvert", Some("record-convert"))?;
        let encoder = make_element(recording_spec.encoder_factory, Some("record-encoder"))?;
        let mux = make_element(recording_spec.mux_factory, Some("record-mux"))?;
        let file_sink = make_element("filesink", Some("record-file-sink"))?;
        file_sink.set_property(
            "location",
            cli.record
                .as_ref()
                .and_then(|path| path.to_str())
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
        let display_queue = add_display_branch(&pipeline, cli, overlay_text)?;
        gst::Element::link_many([&record_queue, &record_convert, &encoder])?;
        link_tee_branch(&tee, &display_queue)?;
        link_tee_branch(&tee, &record_queue)?;
        link_into_mux(&encoder, &mux, recording_spec.mux_pad_template)?;
        mux.link(&file_sink)?;
    } else {
        let queue = add_display_branch(&pipeline, cli, overlay_text)?;
        gst::Element::link_many([&src, &queue])?;
    }

    Ok(pipeline)
}

fn add_display_branch(
    pipeline: &gst::Pipeline,
    cli: &Cli,
    overlay_text: Option<&str>,
) -> Result<gst::Element> {
    let queue = make_element("queue", Some("display-queue"))?;
    let convert = make_element("videoconvert", Some("display-convert"))?;
    pipeline.add_many([&queue, &convert])?;

    if cli.no_overlay {
        let sink = make_element(&cli.video_sink, Some("display-sink"))?;
        sink.set_property("sync", false);
        pipeline.add(&sink)?;
        gst::Element::link_many([&queue, &convert, &sink])?;
        return Ok(queue);
    }

    let video_sink = make_element(&cli.video_sink, Some("display-video-sink"))?;
    video_sink.set_property("sync", false);
    let fps_sink = make_element("fpsdisplaysink", Some("display-sink"))?;
    fps_sink.set_property("sync", false);
    fps_sink.set_property("text-overlay", true);
    fps_sink.set_property("video-sink", &video_sink);
    pipeline.add(&fps_sink)?;

    if let Some(overlay_text) = overlay_text {
        let overlay = make_element("textoverlay", Some("display-mode-overlay"))?;
        overlay.set_property("text", overlay_text);
        overlay.set_property("shaded-background", true);
        overlay.set_property_from_str("halignment", "left");
        overlay.set_property_from_str("valignment", "top");
        pipeline.add(&overlay)?;
        gst::Element::link_many([&queue, &convert, &overlay, &fps_sink])?;
    } else {
        gst::Element::link_many([&queue, &convert, &fps_sink])?;
    }

    Ok(queue)
}

fn resolve_target(
    controller: &DeviceController,
    device_uri: Option<&str>,
) -> Result<ControlTarget> {
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

fn select_encoder(requested: Option<EncoderKind>) -> Result<EncoderSpec> {
    if let Some(requested) = requested {
        let spec = ENCODER_SPECS
            .iter()
            .copied()
            .find(|spec| spec.kind == requested)
            .expect("requested encoder exists in static table");
        ensure_elements_available(spec)?;
        return Ok(spec);
    }

    let mut tried = Vec::new();
    for spec in ENCODER_SPECS {
        tried.push(spec.encoder_factory);
        if ensure_elements_available(*spec).is_ok() {
            return Ok(*spec);
        }
    }

    bail!(
        "no supported open encoder/mux pair is available; tried {}",
        tried.join(", ")
    )
}

fn ensure_elements_available(spec: EncoderSpec) -> Result<()> {
    for factory in [spec.encoder_factory, spec.mux_factory] {
        if gst::ElementFactory::find(factory).is_none() {
            bail!("required GStreamer element {factory} is not available");
        }
    }
    Ok(())
}

fn advertised_stream_overlay_text(
    description: &DeviceDescription,
    stream_name: &str,
) -> Option<String> {
    advertised_stream_mode(description, stream_name).map(format_mode_overlay_text)
}

fn advertised_stream_mode<'a>(
    description: &'a DeviceDescription,
    stream_name: &str,
) -> Option<&'a AdvertisedStreamMode> {
    description
        .streams
        .iter()
        .find(|stream| stream.name == stream_name)
        .and_then(|stream| stream.mode.as_ref())
}

fn format_mode_overlay_text(mode: &AdvertisedStreamMode) -> String {
    format!(
        "Mode: {} {}x{} @ {} fps",
        mode.pixel_format.gst_format(),
        mode.width,
        mode.height,
        mode.fps
    )
}

fn select_managed_transport_settings(
    max_packet_size_override: Option<u16>,
    packet_delay_override: Option<u64>,
) -> ManagedTransportSettings {
    let max_packet_size = max_packet_size_override.unwrap_or(DEFAULT_MANAGED_MAX_PACKET_SIZE);
    let packet_delay_ns = packet_delay_override.unwrap_or(DEFAULT_MANAGED_PACKET_DELAY_NS);

    ManagedTransportSettings {
        max_packet_size,
        packet_delay_ns,
    }
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

fn wait_for_terminal_bus_message(bus: &gst::Bus, timeout: Duration) -> Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(200)) {
            match message.view() {
                gst::MessageView::Eos(..) => return Ok(()),
                gst::MessageView::Error(err) => {
                    let src = err
                        .src()
                        .map(|src| src.path_string().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    bail!(
                        "pipeline error while finalizing from {src}: {}",
                        err.error()
                    );
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub fn suggested_record_path(kind: EncoderKind, base: &Path) -> PathBuf {
    let extension = match kind {
        EncoderKind::Av1 => "mkv",
        EncoderKind::Vp9 => "webm",
        EncoderKind::Theora => "ogv",
    };
    base.with_extension(extension)
}

fn pipeline_start_error(bus: &gst::Bus) -> anyhow::Error {
    if let Some(message) =
        bus.timed_pop_filtered(gst::ClockTime::from_seconds(1), &[gst::MessageType::Error])
    {
        if let gst::MessageView::Error(err) = message.view() {
            return anyhow!("failed to start pipeline: {}", format_bus_error(err));
        }
    }

    anyhow!("failed to start pipeline: element failed to change its state")
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

fn read_source_stats(pipeline: &gst::Pipeline) -> SourceStats {
    let Some(source) = pipeline.by_name("eeview-source") else {
        return SourceStats::default();
    };

    SourceStats {
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

fn format_source_stats(stats: SourceStats) -> String {
    let mut rendered = format!(
        "frames-received={} frames-dropped={} packet-anomalies={}",
        stats.frames_received, stats.frames_dropped, stats.packet_anomalies
    );

    if let Some(breakdown) = format_source_anomaly_breakdown(stats) {
        rendered.push_str(" anomaly-breakdown=[");
        rendered.push_str(&breakdown);
        rendered.push(']');
    }

    rendered
}

fn read_u64_property(element: &gst::Element, property_name: &str) -> u64 {
    if element.find_property(property_name).is_some() {
        element.property(property_name)
    } else {
        0
    }
}

fn format_source_anomaly_breakdown(stats: SourceStats) -> Option<String> {
    let entries = [
        ("timeout-drops", stats.timeout_drops),
        ("payload-overflow-drops", stats.payload_overflow_drops),
        ("short-frame-drops", stats.short_frame_drops),
        ("duplicate-leader-drops", stats.duplicate_leader_drops),
        (
            "payload-before-leader-drops",
            stats.payload_before_leader_drops,
        ),
        (
            "trailer-before-leader-drops",
            stats.trailer_before_leader_drops,
        ),
        (
            "packet-after-trailer-drops",
            stats.packet_after_trailer_drops,
        ),
        ("parse-failures", stats.parse_failures),
        (
            "expected-format-mismatches",
            stats.expected_format_mismatches,
        ),
        ("midstream-format-changes", stats.midstream_format_changes),
    ];

    let rendered = entries
        .into_iter()
        .filter(|(_, value)| *value > 0)
        .map(|(label, value)| format!("{label}={value}"))
        .collect::<Vec<_>>();

    if rendered.is_empty() {
        None
    } else {
        Some(rendered.join(","))
    }
}

fn finalize_run_result(
    run_result: Result<()>,
    stop_result: Result<(), anyhow::Error>,
    source_stats: SourceStats,
) -> Result<()> {
    match (run_result, stop_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) => Err(err.context(format!(
            "source stats: {}",
            format_source_stats(source_stats)
        ))),
        (Ok(()), Err(stop_err)) => Err(stop_err.context(format!(
            "source stats: {}",
            format_source_stats(source_stats)
        ))),
        (Err(err), Err(stop_err)) => Err(anyhow!(
            "{err:#}\nadditionally failed to stop pipeline: {stop_err:#}\nsource stats: {}",
            format_source_stats(source_stats)
        )),
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use clap::Parser;
    use gst::prelude::*;
    use gsteevideo::eevideo_control::{
        default_control_backend, AdvertisedStream, AdvertisedStreamMode, ControlTarget,
        ControlTransportKind, DeviceDescription, DeviceSummary,
    };
    use gstreamer as gst;
    use std::path::PathBuf;

    use super::{
        advertised_stream_overlay_text, build_pipeline, finalize_run_result, format_source_stats,
        select_managed_transport_settings, suggested_record_path, Cli, EncoderKind,
        ManagedTransportSettings, SourceStats,
    };

    #[test]
    fn formats_source_stats_as_a_single_line() {
        assert_eq!(
            format_source_stats(SourceStats {
                frames_received: 12,
                frames_dropped: 3,
                packet_anomalies: 7,
                timeout_drops: 0,
                payload_overflow_drops: 0,
                short_frame_drops: 0,
                duplicate_leader_drops: 0,
                payload_before_leader_drops: 0,
                trailer_before_leader_drops: 0,
                packet_after_trailer_drops: 0,
                parse_failures: 0,
                expected_format_mismatches: 0,
                midstream_format_changes: 0,
            }),
            "frames-received=12 frames-dropped=3 packet-anomalies=7"
        );
    }

    #[test]
    fn formats_packet_anomaly_breakdown_when_present() {
        assert_eq!(
            format_source_stats(SourceStats {
                frames_received: 12,
                frames_dropped: 3,
                packet_anomalies: 7,
                timeout_drops: 2,
                payload_overflow_drops: 0,
                short_frame_drops: 0,
                duplicate_leader_drops: 0,
                payload_before_leader_drops: 0,
                trailer_before_leader_drops: 0,
                packet_after_trailer_drops: 0,
                parse_failures: 4,
                expected_format_mismatches: 1,
                midstream_format_changes: 0,
            }),
            "frames-received=12 frames-dropped=3 packet-anomalies=7 anomaly-breakdown=[timeout-drops=2,parse-failures=4,expected-format-mismatches=1]"
        );
    }

    #[test]
    fn finalize_run_result_keeps_primary_error_and_stop_error() {
        let err = finalize_run_result(
            Err(anyhow!("pipeline error")),
            Err(anyhow!("failed to stop pipeline")),
            SourceStats {
                frames_received: 4,
                frames_dropped: 2,
                packet_anomalies: 1,
                timeout_drops: 0,
                payload_overflow_drops: 0,
                short_frame_drops: 0,
                duplicate_leader_drops: 0,
                payload_before_leader_drops: 0,
                trailer_before_leader_drops: 0,
                packet_after_trailer_drops: 0,
                parse_failures: 0,
                expected_format_mismatches: 0,
                midstream_format_changes: 0,
            },
        )
        .unwrap_err();

        let rendered = format!("{err:#}");
        assert!(rendered.contains("pipeline error"));
        assert!(rendered.contains("additionally failed to stop pipeline"));
        assert!(rendered.contains("frames-received=4"));
    }

    fn init_gst() {
        static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        INIT.get_or_init(|| {
            gst::init().unwrap();
            gsteevideo::register_static().unwrap();
        });
    }

    #[test]
    fn suggested_record_extensions_match_encoder_kind() {
        assert_eq!(
            suggested_record_path(EncoderKind::Av1, std::path::Path::new("out")).extension(),
            Some(std::ffi::OsStr::new("mkv"))
        );
        assert_eq!(
            suggested_record_path(EncoderKind::Vp9, std::path::Path::new("out")).extension(),
            Some(std::ffi::OsStr::new("webm"))
        );
    }

    #[test]
    fn parses_no_overlay_flag() {
        let cli = Cli::try_parse_from([
            "eeview",
            "--device-uri",
            "coap://127.0.0.1:5683",
            "--bind-address",
            "127.0.0.1",
            "--no-overlay",
        ])
        .unwrap();

        assert!(cli.no_overlay);
    }

    #[test]
    fn overlay_is_enabled_by_default() {
        let cli = Cli::try_parse_from([
            "eeview",
            "--device-uri",
            "coap://127.0.0.1:5683",
            "--bind-address",
            "127.0.0.1",
        ])
        .unwrap();

        assert!(!cli.no_overlay);
    }

    #[test]
    fn builds_overlay_text_from_advertised_stream_mode() {
        let description = DeviceDescription {
            summary: DeviceSummary {
                target: ControlTarget {
                    device_uri: "coap://127.0.0.1:5683".to_string(),
                    transport_kind: ControlTransportKind::CoapRegister,
                    auth_scope: None,
                },
                interface_name: "eth0".to_string(),
                interface_address: "127.0.0.1".to_string(),
                device_address: "127.0.0.1".to_string(),
            },
            capabilities: Default::default(),
            device: gsteevideo::eevideo_control::DeviceConfig::default(),
            streams: vec![AdvertisedStream {
                name: "stream0".to_string(),
                mode: Some(AdvertisedStreamMode {
                    pixel_format: eevideo_proto::PixelFormat::Uyvy,
                    width: 1280,
                    height: 720,
                    fps: 30,
                }),
            }],
        };

        assert_eq!(
            advertised_stream_overlay_text(&description, "stream0").as_deref(),
            Some("Mode: UYVY 1280x720 @ 30 fps")
        );
    }

    #[test]
    fn selects_stable_managed_transport_defaults() {
        assert_eq!(
            select_managed_transport_settings(None, None),
            ManagedTransportSettings {
                max_packet_size: 1400,
                packet_delay_ns: 0,
            }
        );
    }

    #[test]
    fn build_pipeline_uses_overlay_elements_by_default() {
        init_gst();
        let cli = Cli::try_parse_from([
            "eeview",
            "--device-uri",
            "coap://127.0.0.1:5683",
            "--bind-address",
            "127.0.0.1",
            "--video-sink",
            "fakesink",
        ])
        .unwrap();

        let pipeline = build_pipeline(
            &cli,
            default_control_backend(),
            ControlTarget {
                device_uri: "coap://127.0.0.1:5683".to_string(),
                transport_kind: ControlTransportKind::CoapRegister,
                auth_scope: None,
            },
            select_managed_transport_settings(None, None),
            None,
            Some("Mode: UYVY 1280x720 @ 30 fps"),
        )
        .unwrap();

        assert_eq!(
            pipeline
                .by_name("display-sink")
                .unwrap()
                .factory()
                .unwrap()
                .name(),
            "fpsdisplaysink"
        );
        assert!(pipeline.by_name("display-mode-overlay").is_some());
    }

    #[test]
    fn build_pipeline_omits_overlay_when_disabled() {
        init_gst();
        let cli = Cli::try_parse_from([
            "eeview",
            "--device-uri",
            "coap://127.0.0.1:5683",
            "--bind-address",
            "127.0.0.1",
            "--video-sink",
            "fakesink",
            "--no-overlay",
        ])
        .unwrap();

        let pipeline = build_pipeline(
            &cli,
            default_control_backend(),
            ControlTarget {
                device_uri: "coap://127.0.0.1:5683".to_string(),
                transport_kind: ControlTransportKind::CoapRegister,
                auth_scope: None,
            },
            select_managed_transport_settings(None, None),
            None,
            Some("Mode: UYVY 1280x720 @ 30 fps"),
        )
        .unwrap();

        assert_eq!(
            pipeline
                .by_name("display-sink")
                .unwrap()
                .factory()
                .unwrap()
                .name(),
            "fakesink"
        );
        assert!(pipeline.by_name("display-mode-overlay").is_none());
    }

    #[test]
    fn build_pipeline_keeps_fps_overlay_when_mode_text_is_unavailable() {
        init_gst();
        let cli = Cli::try_parse_from([
            "eeview",
            "--device-uri",
            "coap://127.0.0.1:5683",
            "--bind-address",
            "127.0.0.1",
            "--video-sink",
            "fakesink",
        ])
        .unwrap();

        let pipeline = build_pipeline(
            &cli,
            default_control_backend(),
            ControlTarget {
                device_uri: "coap://127.0.0.1:5683".to_string(),
                transport_kind: ControlTransportKind::CoapRegister,
                auth_scope: None,
            },
            select_managed_transport_settings(None, None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            pipeline
                .by_name("display-sink")
                .unwrap()
                .factory()
                .unwrap()
                .name(),
            "fpsdisplaysink"
        );
        assert!(pipeline.by_name("display-mode-overlay").is_none());
    }

    #[test]
    fn build_pipeline_keeps_record_branch_overlay_free() {
        init_gst();
        let cli = Cli {
            device_uri: Some("coap://127.0.0.1:5683".to_string()),
            iface: None,
            timeout_ms: 1000,
            local_port: 0,
            yaml_root: None,
            bind_address: "127.0.0.1".to_string(),
            port: 5000,
            source_timeout_ms: 2000,
            latency_ms: 0,
            stream_name: "stream0".to_string(),
            max_packet_size: None,
            packet_delay_ns: None,
            video_sink: "fakesink".to_string(),
            no_overlay: false,
            record: Some(PathBuf::from("capture.mkv")),
            encoder: None,
        };

        let pipeline = build_pipeline(
            &cli,
            default_control_backend(),
            ControlTarget {
                device_uri: "coap://127.0.0.1:5683".to_string(),
                transport_kind: ControlTransportKind::CoapRegister,
                auth_scope: None,
            },
            select_managed_transport_settings(None, None),
            Some(super::select_encoder(None).unwrap()),
            Some("Mode: UYVY 1280x720 @ 30 fps"),
        )
        .unwrap();

        assert!(pipeline.by_name("record-queue").is_some());
        assert!(pipeline.by_name("display-mode-overlay").is_some());
        assert!(pipeline.by_name("record-convert").is_some());
    }
}

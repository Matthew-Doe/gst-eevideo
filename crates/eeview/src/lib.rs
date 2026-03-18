use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, ValueEnum};
use eevideo_control::{
    AdvertisedStreamMode, CoapRegisterBackendConfig, ControlTarget, ControlTransportKind,
    DeviceController, DeviceDescription,
};
use gst::prelude::*;
use gstreamer as gst;

const CLI_AFTER_LONG_HELP: &str = "\
Examples:
  eeview --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --port 5000
  eeview --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --record capture.mkv --encoder av1
";

#[derive(Debug, Parser)]
#[command(
    name = "eeview",
    about = "EEVideo live view and recorder CLI",
    after_long_help = CLI_AFTER_LONG_HELP
)]
pub struct Cli {
    #[arg(
        long,
        help = "Target a single device directly instead of relying on discovery."
    )]
    device_uri: Option<String>,
    #[arg(
        long,
        help = "Prefer a specific local interface for discovery and control traffic."
    )]
    iface: Option<String>,
    #[arg(
        long,
        default_value_t = 1000,
        help = "Set the discovery and request timeout in milliseconds."
    )]
    timeout_ms: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Bind control traffic to a specific local UDP port."
    )]
    local_port: u16,
    #[arg(
        long,
        help = "Override the YAML metadata root used for symbolic register and field names."
    )]
    yaml_root: Option<PathBuf>,
    #[arg(long, help = "Bind the UDP receiver to a concrete local IPv4 address.")]
    bind_address: String,
    #[arg(
        long,
        default_value_t = 5000,
        help = "UDP port that the managed device should send frames to."
    )]
    port: u32,
    #[arg(
        long,
        default_value_t = 2000,
        help = "Stop waiting for incoming frames after this many milliseconds."
    )]
    source_timeout_ms: u64,
    #[arg(
        long,
        default_value_t = 0,
        help = "Extra receiver latency to buffer before display."
    )]
    latency_ms: u64,
    #[arg(
        long,
        default_value = "stream0",
        help = "Advertised stream name to configure before viewing."
    )]
    stream_name: String,
    #[arg(
        long,
        default_value = "autovideosink",
        help = "GStreamer video sink factory used for local display."
    )]
    video_sink: String,
    #[arg(
        long,
        default_value_t = false,
        help = "Disable the FPS and mode HUD overlay in the viewer window."
    )]
    no_overlay: bool,
    #[arg(
        long,
        help = "Write a recording to this output path while continuing live display."
    )]
    record: Option<PathBuf>,
    #[arg(
        long,
        help = "When --record is set, choose a specific open encoder.",
        long_help = "When --record is set, choose a specific open encoder. If omitted, eeview picks the first available open encoder/mux pair from av1, vp9, then theora."
    )]
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

pub fn main_entry<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(normalize_help_flag_punctuation(args));
    run(cli)
}

fn normalize_help_flag_punctuation<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    args.into_iter()
        .map(Into::into)
        .map(|arg: OsString| match arg.to_str() {
            Some("--help,") => OsString::from("--help"),
            Some("-h,") => OsString::from("-h"),
            _ => arg,
        })
        .collect()
}

pub fn run(cli: Cli) -> Result<()> {
    gst::init()?;
    gsteevideo::register_static().context("failed to register EEVideo plugin")?;

    let controller = DeviceController::new(CoapRegisterBackendConfig {
        interface_name: cli.iface.clone(),
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
            .and_then(|description| advertised_stream_overlay_text(&description, &cli.stream_name))
    };
    let recording_spec = if cli.record.is_some() {
        Some(select_encoder(cli.encoder)?)
    } else {
        None
    };

    let pipeline = build_pipeline(
        &cli,
        controller.shared_backend(),
        target,
        recording_spec,
        overlay_text.as_deref(),
    )?;
    let bus = pipeline.bus().context("pipeline did not expose a bus")?;
    let (interrupt_tx, interrupt_rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = interrupt_tx.send(());
    })
    .context("failed to install Ctrl+C handler")?;

    pipeline
        .set_state(gst::State::Playing)
        .context("failed to start pipeline")?;

    loop {
        if interrupt_rx.try_recv().is_ok() {
            let _ = pipeline.send_event(gst::event::Eos::new());
            wait_for_terminal_bus_message(&bus, Duration::from_secs(5))?;
            break;
        }

        if let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(200)) {
            match message.view() {
                gst::MessageView::Eos(..) => break,
                gst::MessageView::Error(err) => {
                    let src = err
                        .src()
                        .map(|src| src.path_string().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let debug = err.debug().unwrap_or_default();
                    bail!("pipeline error from {src}: {} ({debug})", err.error());
                }
                _ => {}
            }
        }
    }

    pipeline
        .set_state(gst::State::Null)
        .context("failed to stop pipeline")?;
    Ok(())
}

fn build_pipeline(
    cli: &Cli,
    backend: eevideo_control::SharedControlBackend,
    target: ControlTarget,
    recording_spec: Option<EncoderSpec>,
    overlay_text: Option<&str>,
) -> Result<gst::Pipeline> {
    let pipeline = gst::Pipeline::default();
    let src = gst::ElementFactory::make("eevideosrc")
        .property("address", &cli.bind_address)
        .property("port", cli.port)
        .property("timeout-ms", cli.source_timeout_ms)
        .property("latency-ms", cli.latency_ms)
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
    description
        .streams
        .iter()
        .find(|stream| stream.name == stream_name)
        .and_then(|stream| stream.mode.as_ref())
        .map(format_mode_overlay_text)
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

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use clap::{CommandFactory, Parser};
    use eevideo_control::{
        default_control_backend, AdvertisedStream, AdvertisedStreamMode, ControlTarget,
        ControlTransportKind, DeviceDescription, DeviceSummary,
    };
    use gst::prelude::*;
    use gstreamer as gst;
    use std::path::PathBuf;

    use super::{
        advertised_stream_overlay_text, build_pipeline, normalize_help_flag_punctuation,
        suggested_record_path, Cli, EncoderKind,
    };

    fn render_long_help(mut command: clap::Command) -> String {
        let mut output = Vec::new();
        command.write_long_help(&mut output).unwrap();
        String::from_utf8(output).unwrap()
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
    fn top_level_help_mentions_live_view_examples() {
        let help = render_long_help(Cli::command());

        assert!(help.contains("Examples:"));
        assert!(help.contains("--bind-address 192.168.1.20"));
        assert!(help.contains("Bind the UDP receiver to a concrete local IPv4 address."));
        assert!(help.contains("When --record is set, choose a specific open encoder."));
    }

    #[test]
    fn normalizes_help_flags_with_trailing_commas() {
        let args = normalize_help_flag_punctuation([
            OsString::from("eeview"),
            OsString::from("--help,"),
            OsString::from("-h,"),
            OsString::from("--other,"),
        ]);

        assert_eq!(args[1], OsString::from("--help"));
        assert_eq!(args[2], OsString::from("-h"));
        assert_eq!(args[3], OsString::from("--other,"));
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
            device: eevideo_control::DeviceConfig::default(),
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
            Some(super::select_encoder(None).unwrap()),
            Some("Mode: UYVY 1280x720 @ 30 fps"),
        )
        .unwrap();

        assert!(pipeline.by_name("record-queue").is_some());
        assert!(pipeline.by_name("display-mode-overlay").is_some());
        assert!(pipeline.by_name("record-convert").is_some());
    }
}

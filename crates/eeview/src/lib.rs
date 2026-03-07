use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, ValueEnum};
use eevideo_control::{
    CoapRegisterBackendConfig, ControlTarget, ControlTransportKind, DeviceController,
};
use gst::prelude::*;
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
    #[arg(long, default_value = "autovideosink")]
    video_sink: String,
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
    let cli = Cli::parse_from(args);
    run(cli)
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
    let recording_spec = if cli.record.is_some() {
        Some(select_encoder(cli.encoder)?)
    } else {
        None
    };

    let pipeline = build_pipeline(&cli, controller.shared_backend(), target, recording_spec)?;
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
        let display_queue = make_element("queue", Some("display-queue"))?;
        let display_convert = make_element("videoconvert", Some("display-convert"))?;
        let display_sink = make_element(&cli.video_sink, Some("display-sink"))?;
        display_sink.set_property("sync", false);

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
            &display_queue,
            &display_convert,
            &display_sink,
            &record_queue,
            &record_convert,
            &encoder,
            &mux,
            &file_sink,
        ])?;

        src.link(&tee)?;
        gst::Element::link_many([&display_queue, &display_convert, &display_sink])?;
        gst::Element::link_many([&record_queue, &record_convert, &encoder])?;
        link_tee_branch(&tee, &display_queue)?;
        link_tee_branch(&tee, &record_queue)?;
        link_into_mux(&encoder, &mux, recording_spec.mux_pad_template)?;
        mux.link(&file_sink)?;
    } else {
        let queue = make_element("queue", Some("display-queue"))?;
        let convert = make_element("videoconvert", Some("display-convert"))?;
        let sink = make_element(&cli.video_sink, Some("display-sink"))?;
        sink.set_property("sync", false);
        pipeline.add_many([&queue, &convert, &sink])?;
        gst::Element::link_many([&src, &queue, &convert, &sink])?;
    }

    Ok(pipeline)
}

fn resolve_target(controller: &DeviceController, device_uri: Option<&str>) -> Result<ControlTarget> {
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

fn link_into_mux(
    upstream: &gst::Element,
    mux: &gst::Element,
    pad_template: &str,
) -> Result<()> {
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
                    bail!("pipeline error while finalizing from {src}: {}", err.error());
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
    use super::{suggested_record_path, EncoderKind};

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

}

mod common;
mod control;
mod eevideosink;
mod eevideosrc;

use gst::glib;
use gstreamer as gst;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    eevideosrc::register(Some(plugin))?;
    eevideosink::register(Some(plugin))?;
    Ok(())
}

pub fn register_static() -> Result<(), glib::BoolError> {
    eevideosrc::register(None)?;
    eevideosink::register(None)?;
    Ok(())
}

pub fn configure_source_control(
    element: &gst::Element,
    backend: eevideo_control::SharedControlBackend,
    target: eevideo_control::ControlTarget,
    stream_name: impl Into<String>,
) -> Result<(), glib::BoolError> {
    use glib::subclass::types::ObjectSubclassIsExt;
    use gst::prelude::Cast;

    let src = element
        .clone()
        .downcast::<eevideosrc::EeVideoSrc>()
        .map_err(|_| glib::bool_error!("expected an eevideosrc element"))?;
    src.imp()
        .configure_control(backend, target, stream_name.into());
    Ok(())
}

#[cfg(feature = "gst-tests")]
pub fn configure_source_control_for_tests(
    element: &gst::Element,
    backend: eevideo_control::SharedControlBackend,
    target: eevideo_control::ControlTarget,
    stream_name: impl Into<String>,
) -> Result<(), glib::BoolError> {
    configure_source_control(element, backend, target, stream_name)
}

gst::plugin_define!(
    eevideo,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    env!("CARGO_PKG_VERSION"),
    "MIT",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    "2026-03-05"
);

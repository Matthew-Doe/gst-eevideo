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


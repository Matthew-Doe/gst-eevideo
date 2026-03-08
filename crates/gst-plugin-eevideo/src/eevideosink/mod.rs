mod imp;

use glib::types::StaticType;
use gst::glib;
use gstreamer as gst;
use gstreamer_base as gst_base;

glib::wrapper! {
    pub struct EeVideoSink(ObjectSubclass<imp::EeVideoSink>)
        @extends gst_base::BaseSink, gst::Element, gst::Object;
}

pub fn register(plugin: Option<&gst::Plugin>) -> Result<(), glib::BoolError> {
    gst::Element::register(
        plugin,
        "eevideosink",
        gst::Rank::NONE,
        EeVideoSink::static_type(),
    )
}

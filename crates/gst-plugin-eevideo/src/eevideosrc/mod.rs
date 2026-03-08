mod imp;

use glib::types::StaticType;
use gst::glib;
use gstreamer as gst;
use gstreamer_base as gst_base;

glib::wrapper! {
    pub struct EeVideoSrc(ObjectSubclass<imp::EeVideoSrc>)
        @extends gst_base::PushSrc, gst_base::BaseSrc, gst::Element, gst::Object;
}

pub fn register(plugin: Option<&gst::Plugin>) -> Result<(), glib::BoolError> {
    gst::Element::register(
        plugin,
        "eevideosrc",
        gst::Rank::NONE,
        EeVideoSrc::static_type(),
    )
}

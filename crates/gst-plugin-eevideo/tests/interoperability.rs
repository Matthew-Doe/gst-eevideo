#![cfg(feature = "gst-tests")]

use gstreamer as gst;

#[test]
fn source_element_registers_and_exposes_expected_caps() {
    gst::init().unwrap();
    gsteevideo::register_static().unwrap();

    let factory = gst::ElementFactory::find("eevideosrc").expect("eevideosrc factory");
    let caps = factory
        .static_pad_templates()
        .into_iter()
        .find(|template| template.direction() == gst::PadDirection::Src)
        .expect("src pad template")
        .caps();

    assert!(caps.to_string().contains("GRAY8"));
    assert!(caps.to_string().contains("video/x-bayer"));
}

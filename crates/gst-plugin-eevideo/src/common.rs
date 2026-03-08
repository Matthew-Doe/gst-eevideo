use eevideo_proto::{PixelFormat, PixelFormatError, VideoFrame};
use gstreamer as gst;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameFormat {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
}

impl FrameFormat {
    pub fn payload_len(self) -> Result<usize, PixelFormatError> {
        self.pixel_format.payload_len(self.width, self.height)
    }

    pub fn from_frame(frame: &VideoFrame) -> Self {
        Self {
            width: frame.width,
            height: frame.height,
            pixel_format: frame.pixel_format,
        }
    }

    pub fn to_caps(self) -> gst::Caps {
        gst::Caps::builder(self.pixel_format.media_type())
            .field("format", self.pixel_format.gst_format())
            .field("width", self.width as i32)
            .field("height", self.height as i32)
            .field("framerate", gst::Fraction::new(0, 1))
            .build()
    }
}

pub fn parse_caps(caps: &gst::CapsRef) -> Result<FrameFormat, String> {
    let structure = caps
        .structure(0)
        .ok_or_else(|| "caps did not contain a structure".to_string())?;

    let media_type = structure.name();
    let format = structure
        .get::<String>("format")
        .map_err(|_| "caps missing string format field".to_string())?;
    let width = structure
        .get::<i32>("width")
        .map_err(|_| "caps missing integer width field".to_string())?;
    let height = structure
        .get::<i32>("height")
        .map_err(|_| "caps missing integer height field".to_string())?;

    if width <= 0 || height <= 0 {
        return Err(format!(
            "invalid negotiated dimensions {}x{}",
            width, height
        ));
    }

    let pixel_format =
        PixelFormat::from_caps(media_type, &format).map_err(|err| err.to_string())?;

    Ok(FrameFormat {
        width: width as u32,
        height: height as u32,
        pixel_format,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_caps;
    use gstreamer as gst;

    #[test]
    fn parses_supported_caps() {
        gst::init().unwrap();

        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "GRAY8")
            .field("width", 320i32)
            .field("height", 240i32)
            .field("framerate", gst::Fraction::new(0, 1))
            .build();

        let format = parse_caps(caps.as_ref()).unwrap();
        assert_eq!(format.width, 320);
        assert_eq!(format.height, 240);
    }
}

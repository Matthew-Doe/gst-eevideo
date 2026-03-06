use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Mono8,
    Mono16,
    BayerGr8,
    BayerRg8,
    BayerGb8,
    BayerBg8,
    Rgb8,
    Uyvy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PixelFormatError {
    UnsupportedPfnc(u32),
    UnsupportedCaps { media_type: String, format: String },
    InvalidDimensions { width: u32, height: u32 },
}

impl fmt::Display for PixelFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPfnc(value) => write!(f, "unsupported PFNC pixel format 0x{value:08x}"),
            Self::UnsupportedCaps { media_type, format } => {
                write!(f, "unsupported caps {media_type} with format {format}")
            }
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid frame dimensions {width}x{height}")
            }
        }
    }
}

impl std::error::Error for PixelFormatError {}

impl PixelFormat {
    pub const MONO8_PFNC: u32 = 0x0108_0001;
    pub const MONO16_PFNC: u32 = 0x0110_0007;
    pub const BAYER_GR8_PFNC: u32 = 0x0108_0008;
    pub const BAYER_RG8_PFNC: u32 = 0x0108_0009;
    pub const BAYER_GB8_PFNC: u32 = 0x0108_000a;
    pub const BAYER_BG8_PFNC: u32 = 0x0108_000b;
    pub const RGB8_PFNC: u32 = 0x0218_0014;
    pub const UYVY_PFNC: u32 = 0x0210_001f;

    pub fn from_pfnc(value: u32) -> Result<Self, PixelFormatError> {
        match value {
            Self::MONO8_PFNC => Ok(Self::Mono8),
            Self::MONO16_PFNC => Ok(Self::Mono16),
            Self::BAYER_GR8_PFNC => Ok(Self::BayerGr8),
            Self::BAYER_RG8_PFNC => Ok(Self::BayerRg8),
            Self::BAYER_GB8_PFNC => Ok(Self::BayerGb8),
            Self::BAYER_BG8_PFNC => Ok(Self::BayerBg8),
            Self::RGB8_PFNC => Ok(Self::Rgb8),
            Self::UYVY_PFNC => Ok(Self::Uyvy),
            _ => Err(PixelFormatError::UnsupportedPfnc(value)),
        }
    }

    pub fn from_caps(media_type: &str, format: &str) -> Result<Self, PixelFormatError> {
        match (media_type, format) {
            ("video/x-raw", "GRAY8") => Ok(Self::Mono8),
            ("video/x-raw", "GRAY16_LE") => Ok(Self::Mono16),
            ("video/x-raw", "RGB") => Ok(Self::Rgb8),
            ("video/x-raw", "UYVY") => Ok(Self::Uyvy),
            ("video/x-bayer", "grbg") => Ok(Self::BayerGr8),
            ("video/x-bayer", "rggb") => Ok(Self::BayerRg8),
            ("video/x-bayer", "gbrg") => Ok(Self::BayerGb8),
            ("video/x-bayer", "bggr") => Ok(Self::BayerBg8),
            _ => Err(PixelFormatError::UnsupportedCaps {
                media_type: media_type.to_string(),
                format: format.to_string(),
            }),
        }
    }

    pub fn pfnc(self) -> u32 {
        match self {
            Self::Mono8 => Self::MONO8_PFNC,
            Self::Mono16 => Self::MONO16_PFNC,
            Self::BayerGr8 => Self::BAYER_GR8_PFNC,
            Self::BayerRg8 => Self::BAYER_RG8_PFNC,
            Self::BayerGb8 => Self::BAYER_GB8_PFNC,
            Self::BayerBg8 => Self::BAYER_BG8_PFNC,
            Self::Rgb8 => Self::RGB8_PFNC,
            Self::Uyvy => Self::UYVY_PFNC,
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Self::BayerGr8 | Self::BayerRg8 | Self::BayerGb8 | Self::BayerBg8 => "video/x-bayer",
            _ => "video/x-raw",
        }
    }

    pub fn gst_format(self) -> &'static str {
        match self {
            Self::Mono8 => "GRAY8",
            Self::Mono16 => "GRAY16_LE",
            Self::BayerGr8 => "grbg",
            Self::BayerRg8 => "rggb",
            Self::BayerGb8 => "gbrg",
            Self::BayerBg8 => "bggr",
            Self::Rgb8 => "RGB",
            Self::Uyvy => "UYVY",
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Mono8 | Self::BayerGr8 | Self::BayerRg8 | Self::BayerGb8 | Self::BayerBg8 => 1,
            Self::Mono16 | Self::Uyvy => 2,
            Self::Rgb8 => 3,
        }
    }

    pub fn payload_len(self, width: u32, height: u32) -> Result<usize, PixelFormatError> {
        if width == 0 || height == 0 {
            return Err(PixelFormatError::InvalidDimensions { width, height });
        }

        Ok(width as usize * height as usize * self.bytes_per_pixel())
    }
}

pub const SUPPORTED_CAPS: &str = concat!(
    "video/x-raw,format=(string){GRAY8,GRAY16_LE,RGB,UYVY},",
    "width=(int)[1,2147483647],height=(int)[1,2147483647],",
    "framerate=(fraction)[0/1,2147483647/1];",
    "video/x-bayer,format=(string){grbg,rggb,gbrg,bggr},",
    "width=(int)[1,2147483647],height=(int)[1,2147483647],",
    "framerate=(fraction)[0/1,2147483647/1]"
);

#[cfg(test)]
mod tests {
    use super::PixelFormat;

    #[test]
    fn round_trips_pfnc_values() {
        for format in [
            PixelFormat::Mono8,
            PixelFormat::Mono16,
            PixelFormat::BayerGr8,
            PixelFormat::BayerRg8,
            PixelFormat::BayerGb8,
            PixelFormat::BayerBg8,
            PixelFormat::Rgb8,
            PixelFormat::Uyvy,
        ] {
            assert_eq!(PixelFormat::from_pfnc(format.pfnc()).unwrap(), format);
        }
    }

    #[test]
    fn maps_caps_to_formats() {
        assert_eq!(
            PixelFormat::from_caps("video/x-raw", "GRAY16_LE").unwrap(),
            PixelFormat::Mono16
        );
        assert_eq!(
            PixelFormat::from_caps("video/x-bayer", "bggr").unwrap(),
            PixelFormat::BayerBg8
        );
    }
}


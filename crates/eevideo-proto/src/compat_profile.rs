use crate::{
    PacketType, PayloadType, PixelFormat, COMPAT_HEADER_SIZE, COMPAT_LEADER_SIZE,
    COMPAT_TRAILER_SIZE,
};

pub const COMPAT_PROFILE_NAME: &str = "EEVideo Stream Compatibility Profile v1";

pub const COMPAT_PROFILE_FIXED_FIELDS: [&str; 4] =
    ["width", "height", "payload_type", "pixel_format"];

pub const COMPAT_PROFILE_PACKET_TYPES: [PacketType; 3] =
    [PacketType::Leader, PacketType::Payload, PacketType::Trailer];

pub const COMPAT_PROFILE_PAYLOAD_TYPES: [PayloadType; 1] = [PayloadType::Image];

pub const COMPAT_PROFILE_PIXEL_FORMATS: [PixelFormat; 8] = [
    PixelFormat::Mono8,
    PixelFormat::Mono16,
    PixelFormat::BayerGr8,
    PixelFormat::BayerRg8,
    PixelFormat::BayerGb8,
    PixelFormat::BayerBg8,
    PixelFormat::Rgb8,
    PixelFormat::Uyvy,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamProfileId {
    CompatibilityV1,
}

impl StreamProfileId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompatibilityV1 => COMPAT_PROFILE_NAME,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompatStreamProfile {
    pub id: StreamProfileId,
    pub name: &'static str,
    pub header_size: usize,
    pub leader_size: usize,
    pub trailer_size: usize,
    pub fixed_stream_fields: &'static [&'static str],
    pub packet_types: &'static [PacketType],
    pub payload_types: &'static [PayloadType],
    pub pixel_formats: &'static [PixelFormat],
}

impl CompatStreamProfile {
    pub const fn minimum_mtu(self) -> usize {
        self.leader_size
    }
}

pub const COMPAT_STREAM_PROFILE: CompatStreamProfile = CompatStreamProfile {
    id: StreamProfileId::CompatibilityV1,
    name: COMPAT_PROFILE_NAME,
    header_size: COMPAT_HEADER_SIZE,
    leader_size: COMPAT_LEADER_SIZE,
    trailer_size: COMPAT_TRAILER_SIZE,
    fixed_stream_fields: &COMPAT_PROFILE_FIXED_FIELDS,
    packet_types: &COMPAT_PROFILE_PACKET_TYPES,
    payload_types: &COMPAT_PROFILE_PAYLOAD_TYPES,
    pixel_formats: &COMPAT_PROFILE_PIXEL_FORMATS,
};

#[cfg(test)]
mod tests {
    use super::{StreamProfileId, COMPAT_PROFILE_NAME, COMPAT_STREAM_PROFILE};

    #[test]
    fn compatibility_profile_identity_is_stable() {
        assert_eq!(COMPAT_STREAM_PROFILE.name, COMPAT_PROFILE_NAME);
        assert_eq!(
            COMPAT_STREAM_PROFILE.id.as_str(),
            StreamProfileId::CompatibilityV1.as_str()
        );
        assert_eq!(
            COMPAT_STREAM_PROFILE.minimum_mtu(),
            COMPAT_STREAM_PROFILE.leader_size
        );
    }
}

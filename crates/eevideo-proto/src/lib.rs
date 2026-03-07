mod assembler;
mod compat_profile;
mod compat_stream;
mod pixel_format;
mod stats;

pub use assembler::{
    AssembleError, FrameAssembler, FrameDropReason, FrameEvent, FrameKey, PartialFrame,
};
pub use compat_profile::{
    CompatStreamProfile, StreamProfileId, COMPAT_PROFILE_FIXED_FIELDS, COMPAT_PROFILE_NAME,
    COMPAT_PROFILE_PACKET_TYPES, COMPAT_PROFILE_PAYLOAD_TYPES, COMPAT_PROFILE_PIXEL_FORMATS,
    COMPAT_STREAM_PROFILE,
};
pub use compat_stream::{
    CompatPacket, CompatPacketEmitError, CompatPacketError, CompatPacketView, CompatPacketizer,
    PacketType, PayloadType, VideoFrameRef, COMPAT_HEADER_SIZE, COMPAT_LEADER_SIZE,
    COMPAT_TRAILER_SIZE,
};
pub use pixel_format::{PixelFormat, PixelFormatError, SUPPORTED_CAPS};
pub use stats::StreamStats;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoFrame {
    pub frame_id: u32,
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub payload_type: PayloadType,
    pub data: Vec<u8>,
}

impl VideoFrame {
    pub fn expected_len(&self) -> usize {
        self.pixel_format
            .payload_len(self.width, self.height)
            .unwrap_or(self.data.len())
    }
}

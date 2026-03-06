mod assembler;
mod compat_stream;
mod pixel_format;
mod stats;

pub use assembler::{
    AssembleError, FrameAssembler, FrameDropReason, FrameEvent, FrameKey, PartialFrame,
};
pub use compat_stream::{
    CompatPacket, CompatPacketError, CompatPacketizer, PacketType, PayloadType, COMPAT_HEADER_SIZE,
    COMPAT_LEADER_SIZE, COMPAT_TRAILER_SIZE,
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

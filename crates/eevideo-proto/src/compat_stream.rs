use crate::{PixelFormat, PixelFormatError, VideoFrame};
use core::fmt;

pub const COMPAT_HEADER_SIZE: usize = 20;
pub const COMPAT_LEADER_SIZE: usize = 44;
pub const COMPAT_TRAILER_SIZE: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketType {
    Leader = 0x1,
    Trailer = 0x2,
    Payload = 0x3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadType {
    Image = 1,
}

impl PayloadType {
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Image),
            _ => None,
        }
    }

    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompatPacket {
    Leader {
        frame_id: u32,
        packet_id: u32,
        timestamp: u64,
        payload_type: PayloadType,
        pixel_format: PixelFormat,
        width: u32,
        height: u32,
    },
    Payload {
        frame_id: u32,
        packet_id: u32,
        data: Vec<u8>,
    },
    Trailer {
        frame_id: u32,
        packet_id: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompatPacketError {
    PacketTooSmall { len: usize, expected: usize },
    UnknownPacketType(u8),
    UnsupportedPayloadType(u16),
    UnsupportedPixelFormat(PixelFormatError),
    InvalidMtu(usize),
}

impl fmt::Display for CompatPacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PacketTooSmall { len, expected } => {
                write!(f, "packet too small: got {len} bytes, expected at least {expected}")
            }
            Self::UnknownPacketType(value) => write!(f, "unknown compatibility packet type {value}"),
            Self::UnsupportedPayloadType(value) => {
                write!(f, "unsupported compatibility payload type {value}")
            }
            Self::UnsupportedPixelFormat(err) => err.fmt(f),
            Self::InvalidMtu(value) => {
                write!(f, "invalid MTU {value}, needs to be >= {COMPAT_LEADER_SIZE}")
            }
        }
    }
}

impl std::error::Error for CompatPacketError {}

impl CompatPacket {
    pub fn parse(buf: &[u8]) -> Result<Self, CompatPacketError> {
        if buf.len() < COMPAT_HEADER_SIZE {
            return Err(CompatPacketError::PacketTooSmall {
                len: buf.len(),
                expected: COMPAT_HEADER_SIZE,
            });
        }

        let packet_type = buf[4] & 0x0f;
        let frame_id = read_u32(buf, 12);
        let packet_id = read_u32(buf, 16);

        match packet_type {
            x if x == PacketType::Leader as u8 => {
                if buf.len() < COMPAT_LEADER_SIZE {
                    return Err(CompatPacketError::PacketTooSmall {
                        len: buf.len(),
                        expected: COMPAT_LEADER_SIZE,
                    });
                }

                let payload_type_raw = read_u16(buf, 22);
                let payload_type = PayloadType::from_u16(payload_type_raw)
                    .ok_or(CompatPacketError::UnsupportedPayloadType(payload_type_raw))?;
                let timestamp = read_u64(buf, 24);
                let pixel_format = PixelFormat::from_pfnc(read_u32(buf, 32))
                    .map_err(CompatPacketError::UnsupportedPixelFormat)?;
                let width = read_u32(buf, 36);
                let height = read_u32(buf, 40);

                Ok(Self::Leader {
                    frame_id,
                    packet_id,
                    timestamp,
                    payload_type,
                    pixel_format,
                    width,
                    height,
                })
            }
            x if x == PacketType::Payload as u8 => Ok(Self::Payload {
                frame_id,
                packet_id,
                data: buf[COMPAT_HEADER_SIZE..].to_vec(),
            }),
            x if x == PacketType::Trailer as u8 => Ok(Self::Trailer { frame_id, packet_id }),
            x => Err(CompatPacketError::UnknownPacketType(x)),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Leader {
                frame_id,
                packet_id,
                timestamp,
                payload_type,
                pixel_format,
                width,
                height,
            } => {
                let mut buf = vec![0u8; COMPAT_LEADER_SIZE];
                write_header(&mut buf, PacketType::Leader, *frame_id, *packet_id);
                write_u16(&mut buf, 22, payload_type.as_u16());
                write_u64(&mut buf, 24, *timestamp);
                write_u32(&mut buf, 32, pixel_format.pfnc());
                write_u32(&mut buf, 36, *width);
                write_u32(&mut buf, 40, *height);
                buf
            }
            Self::Payload {
                frame_id,
                packet_id,
                data,
            } => {
                let mut buf = vec![0u8; COMPAT_HEADER_SIZE + data.len()];
                write_header(&mut buf, PacketType::Payload, *frame_id, *packet_id);
                buf[COMPAT_HEADER_SIZE..].copy_from_slice(data);
                buf
            }
            Self::Trailer {
                frame_id,
                packet_id,
            } => {
                let mut buf = vec![0u8; COMPAT_TRAILER_SIZE];
                write_header(&mut buf, PacketType::Trailer, *frame_id, *packet_id);
                buf
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompatPacketizer {
    mtu: usize,
}

impl CompatPacketizer {
    pub fn new(mtu: usize) -> Result<Self, CompatPacketError> {
        if mtu < COMPAT_LEADER_SIZE {
            return Err(CompatPacketError::InvalidMtu(mtu));
        }
        Ok(Self { mtu })
    }

    pub fn packetize(&self, frame: &VideoFrame) -> Result<Vec<Vec<u8>>, CompatPacketError> {
        let mut packets = Vec::new();
        packets.push(
            CompatPacket::Leader {
                frame_id: frame.frame_id,
                packet_id: 0,
                timestamp: frame.timestamp,
                payload_type: frame.payload_type,
                pixel_format: frame.pixel_format,
                width: frame.width,
                height: frame.height,
            }
            .to_bytes(),
        );

        let chunk_len = self.mtu - COMPAT_HEADER_SIZE;
        let mut packet_id = 1u32;

        for chunk in frame.data.chunks(chunk_len) {
            packets.push(
                CompatPacket::Payload {
                    frame_id: frame.frame_id,
                    packet_id,
                    data: chunk.to_vec(),
                }
                .to_bytes(),
            );
            packet_id += 1;
        }

        packets.push(
            CompatPacket::Trailer {
                frame_id: frame.frame_id,
                packet_id,
            }
            .to_bytes(),
        );

        Ok(packets)
    }
}

fn read_u16(buf: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([buf[offset], buf[offset + 1]])
}

fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

fn read_u64(buf: &[u8], offset: usize) -> u64 {
    u64::from_be_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
        buf[offset + 4],
        buf[offset + 5],
        buf[offset + 6],
        buf[offset + 7],
    ])
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn write_u64(buf: &mut [u8], offset: usize, value: u64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
}

fn write_header(buf: &mut [u8], packet_type: PacketType, frame_id: u32, packet_id: u32) {
    buf[4] = packet_type as u8;
    write_u32(buf, 12, frame_id);
    write_u32(buf, 16, packet_id);
}

#[cfg(test)]
mod tests {
    use super::{CompatPacket, CompatPacketizer, PayloadType};
    use crate::{PixelFormat, VideoFrame};

    #[test]
    fn serializes_and_parses_leader() {
        let packet = CompatPacket::Leader {
            frame_id: 9,
            packet_id: 0,
            timestamp: 1234,
            payload_type: PayloadType::Image,
            pixel_format: PixelFormat::Mono16,
            width: 640,
            height: 480,
        };

        let bytes = packet.to_bytes();
        assert_eq!(CompatPacket::parse(&bytes).unwrap(), packet);
    }

    #[test]
    fn packetizer_emits_leader_payloads_and_trailer() {
        let frame = VideoFrame {
            frame_id: 12,
            timestamp: 99,
            width: 12,
            height: 4,
            pixel_format: PixelFormat::Mono8,
            payload_type: PayloadType::Image,
            data: vec![1; 48],
        };

        let packets = CompatPacketizer::new(44).unwrap().packetize(&frame).unwrap();
        assert_eq!(packets.len(), 4);
        assert!(matches!(CompatPacket::parse(&packets[0]).unwrap(), CompatPacket::Leader { .. }));
        assert!(matches!(CompatPacket::parse(&packets[3]).unwrap(), CompatPacket::Trailer { .. }));
    }
}

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::{CompatPacket, PayloadType, PixelFormat, StreamStats, VideoFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameKey(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameDropReason {
    MissingPayload,
    PayloadOverflow,
    Timeout,
    DuplicateLeader,
    TrailerBeforeLeader,
    UnsupportedPayload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameEvent {
    Complete(VideoFrame),
    Dropped { frame_id: u32, reason: FrameDropReason },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssembleError {
    UnsupportedPayload(PayloadType),
}

impl std::fmt::Display for AssembleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPayload(payload) => write!(f, "unsupported payload type {:?}", payload),
        }
    }
}

impl std::error::Error for AssembleError {}

#[derive(Clone, Debug)]
pub struct PartialFrame {
    pub frame_id: u32,
    pub packet_id: u32,
    pub timestamp: u64,
    pub payload_type: PayloadType,
    pub pixel_format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub offset: usize,
    pub data: Vec<u8>,
    pub last_update: Instant,
}

pub struct FrameAssembler {
    timeout: Duration,
    frames: HashMap<FrameKey, PartialFrame>,
}

impl FrameAssembler {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            frames: HashMap::new(),
        }
    }

    pub fn ingest(
        &mut self,
        packet: CompatPacket,
        now: Instant,
        stats: &StreamStats,
    ) -> Result<Option<FrameEvent>, AssembleError> {
        stats.record_packet();

        match packet {
            CompatPacket::Leader {
                frame_id,
                packet_id,
                timestamp,
                payload_type,
                pixel_format,
                width,
                height,
            } => {
                let key = FrameKey(frame_id);
                if self.frames.contains_key(&key) {
                    stats.record_packet_anomaly();
                    stats.record_drop();
                    self.frames.remove(&key);
                    return Ok(Some(FrameEvent::Dropped {
                        frame_id,
                        reason: FrameDropReason::DuplicateLeader,
                    }));
                }

                let len = pixel_format
                    .payload_len(width, height)
                    .map_err(|_| AssembleError::UnsupportedPayload(payload_type))?;

                self.frames.insert(
                    key,
                    PartialFrame {
                        frame_id,
                        packet_id,
                        timestamp,
                        payload_type,
                        pixel_format,
                        width,
                        height,
                        offset: 0,
                        data: vec![0; len],
                        last_update: now,
                    },
                );

                Ok(None)
            }
            CompatPacket::Payload {
                frame_id,
                packet_id,
                data,
            } => {
                let key = FrameKey(frame_id);
                let Some(frame) = self.frames.get_mut(&key) else {
                    stats.record_packet_anomaly();
                    stats.record_drop();
                    return Ok(Some(FrameEvent::Dropped {
                        frame_id,
                        reason: FrameDropReason::TrailerBeforeLeader,
                    }));
                };

                if frame.packet_id == packet_id {
                    stats.record_packet_anomaly();
                    return Ok(None);
                }

                if frame.packet_id != packet_id.saturating_sub(1) {
                    stats.record_packet_anomaly();
                    stats.record_drop();
                    self.frames.remove(&key);
                    return Ok(Some(FrameEvent::Dropped {
                        frame_id,
                        reason: FrameDropReason::MissingPayload,
                    }));
                }

                let next_offset = frame.offset + data.len();
                if next_offset > frame.data.len() {
                    stats.record_packet_anomaly();
                    stats.record_drop();
                    self.frames.remove(&key);
                    return Ok(Some(FrameEvent::Dropped {
                        frame_id,
                        reason: FrameDropReason::PayloadOverflow,
                    }));
                }

                frame.data[frame.offset..next_offset].copy_from_slice(&data);
                frame.offset = next_offset;
                frame.packet_id = frame.packet_id.saturating_add(1);
                frame.last_update = now;
                Ok(None)
            }
            CompatPacket::Trailer {
                frame_id,
                packet_id: _,
            } => {
                let key = FrameKey(frame_id);
                let Some(frame) = self.frames.remove(&key) else {
                    stats.record_packet_anomaly();
                    stats.record_drop();
                    return Ok(Some(FrameEvent::Dropped {
                        frame_id,
                        reason: FrameDropReason::TrailerBeforeLeader,
                    }));
                };

                stats.record_frame();
                Ok(Some(FrameEvent::Complete(VideoFrame {
                    frame_id: frame.frame_id,
                    timestamp: frame.timestamp,
                    width: frame.width,
                    height: frame.height,
                    pixel_format: frame.pixel_format,
                    payload_type: frame.payload_type,
                    data: frame.data[..frame.offset].to_vec(),
                })))
            }
        }
    }

    pub fn reap_timeouts(&mut self, now: Instant, stats: &StreamStats) -> Vec<FrameEvent> {
        let mut timed_out = Vec::new();
        self.frames.retain(|_, frame| {
            if now.saturating_duration_since(frame.last_update) >= self.timeout {
                stats.record_drop();
                stats.record_packet_anomaly();
                timed_out.push(FrameEvent::Dropped {
                    frame_id: frame.frame_id,
                    reason: FrameDropReason::Timeout,
                });
                false
            } else {
                true
            }
        });
        timed_out
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::{CompatPacket, PayloadType, PixelFormat, StreamStats};

    use super::{FrameAssembler, FrameDropReason, FrameEvent};

    #[test]
    fn assembles_a_complete_frame() {
        let stats = StreamStats::default();
        let mut assembler = FrameAssembler::new(Duration::from_secs(1));
        let now = Instant::now();

        assert!(assembler
            .ingest(
                CompatPacket::Leader {
                    frame_id: 7,
                    packet_id: 0,
                    timestamp: 10,
                    payload_type: PayloadType::Image,
                    pixel_format: PixelFormat::Mono8,
                    width: 2,
                    height: 2,
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        assert!(assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 7,
                    packet_id: 1,
                    data: vec![1, 2, 3, 4],
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        let event = assembler
            .ingest(
                CompatPacket::Trailer {
                    frame_id: 7,
                    packet_id: 2,
                },
                now,
                &stats,
            )
            .unwrap()
            .unwrap();

        match event {
            FrameEvent::Complete(frame) => assert_eq!(frame.data, vec![1, 2, 3, 4]),
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[test]
    fn drops_frame_on_missing_payload() {
        let stats = StreamStats::default();
        let mut assembler = FrameAssembler::new(Duration::from_secs(1));
        let now = Instant::now();

        assembler
            .ingest(
                CompatPacket::Leader {
                    frame_id: 9,
                    packet_id: 0,
                    timestamp: 0,
                    payload_type: PayloadType::Image,
                    pixel_format: PixelFormat::Mono8,
                    width: 4,
                    height: 1,
                },
                now,
                &stats,
            )
            .unwrap();

        let event = assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 9,
                    packet_id: 3,
                    data: vec![1, 2],
                },
                now,
                &stats,
            )
            .unwrap()
            .unwrap();

        assert_eq!(
            event,
            FrameEvent::Dropped {
                frame_id: 9,
                reason: FrameDropReason::MissingPayload,
            }
        );
    }

    #[test]
    fn times_out_partial_frame() {
        let stats = StreamStats::default();
        let mut assembler = FrameAssembler::new(Duration::from_millis(10));
        let now = Instant::now();

        assembler
            .ingest(
                CompatPacket::Leader {
                    frame_id: 11,
                    packet_id: 0,
                    timestamp: 0,
                    payload_type: PayloadType::Image,
                    pixel_format: PixelFormat::Mono8,
                    width: 2,
                    height: 2,
                },
                now,
                &stats,
            )
            .unwrap();

        let drops = assembler.reap_timeouts(now + Duration::from_millis(20), &stats);
        assert_eq!(
            drops,
            vec![FrameEvent::Dropped {
                frame_id: 11,
                reason: FrameDropReason::Timeout,
            }]
        );
    }
}

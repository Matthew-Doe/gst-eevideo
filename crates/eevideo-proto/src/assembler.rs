use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use crate::{CompatPacket, CompatPacketView, PayloadType, PixelFormat, StreamStats, VideoFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameKey(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameDropReason {
    MissingPayload,
    PayloadOverflow,
    Timeout,
    DuplicateLeader,
    PayloadBeforeLeader,
    TrailerBeforeLeader,
    PacketAfterTrailer,
    ShortFrame,
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
            Self::UnsupportedPayload(payload) => {
                write!(f, "unsupported payload type {:?}", payload)
            }
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
    pub pending_payloads: BTreeMap<u32, Vec<u8>>,
    pub trailer_packet_id: Option<u32>,
    pub last_update: Instant,
}

pub struct FrameAssembler {
    timeout: Duration,
    frames: HashMap<FrameKey, PartialFrame>,
}

enum FrameProgress {
    Pending(PartialFrame),
    Complete(VideoFrame),
    Dropped(FrameDropReason),
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
        self.ingest_view(packet.as_view(), now, stats)
    }

    pub fn ingest_view(
        &mut self,
        packet: CompatPacketView<'_>,
        now: Instant,
        stats: &StreamStats,
    ) -> Result<Option<FrameEvent>, AssembleError> {
        stats.record_packet();

        match packet {
            CompatPacketView::Leader {
                frame_id,
                packet_id,
                timestamp,
                payload_type,
                pixel_format,
                width,
                height,
            } => {
                let key = FrameKey(frame_id);
                let frame = build_partial_frame(
                    frame_id,
                    packet_id,
                    timestamp,
                    payload_type,
                    pixel_format,
                    width,
                    height,
                    now,
                )?;

                if self.frames.insert(key, frame).is_some() {
                    stats.record_packet_anomaly();
                    stats.record_drop();
                    return Ok(Some(FrameEvent::Dropped {
                        frame_id,
                        reason: FrameDropReason::DuplicateLeader,
                    }));
                }

                Ok(None)
            }
            CompatPacketView::Payload {
                frame_id,
                packet_id,
                data,
            } => {
                let key = FrameKey(frame_id);
                let Some(mut frame) = self.frames.remove(&key) else {
                    stats.record_packet_anomaly();
                    stats.record_drop();
                    return Ok(Some(FrameEvent::Dropped {
                        frame_id,
                        reason: FrameDropReason::PayloadBeforeLeader,
                    }));
                };

                if packet_id <= frame.packet_id || frame.pending_payloads.contains_key(&packet_id) {
                    stats.record_packet_anomaly();
                    frame.last_update = now;
                    self.frames.insert(key, frame);
                    return Ok(None);
                }

                if let Some(trailer_packet_id) = frame.trailer_packet_id {
                    if packet_id >= trailer_packet_id {
                        stats.record_packet_anomaly();
                        stats.record_drop();
                        return Ok(Some(FrameEvent::Dropped {
                            frame_id,
                            reason: FrameDropReason::PacketAfterTrailer,
                        }));
                    }
                }

                if data.is_empty() {
                    stats.record_packet_anomaly();
                    frame.last_update = now;
                    self.frames.insert(key, frame);
                    return Ok(None);
                }

                if frame.packet_id.checked_add(1) == Some(packet_id) {
                    if let Err(reason) = append_payload_bytes(&mut frame, packet_id, data) {
                        stats.record_packet_anomaly();
                        stats.record_drop();
                        return Ok(Some(FrameEvent::Dropped { frame_id, reason }));
                    }
                } else {
                    if buffered_payloads_overflow(&frame, data.len()) {
                        stats.record_packet_anomaly();
                        stats.record_drop();
                        return Ok(Some(FrameEvent::Dropped {
                            frame_id,
                            reason: FrameDropReason::PayloadOverflow,
                        }));
                    }
                    frame.pending_payloads.insert(packet_id, data.to_vec());
                }
                frame.last_update = now;
                Ok(self.reconcile_frame(key, frame, stats))
            }
            CompatPacketView::Trailer {
                frame_id,
                packet_id,
            } => {
                let key = FrameKey(frame_id);
                let Some(mut frame) = self.frames.remove(&key) else {
                    stats.record_packet_anomaly();
                    stats.record_drop();
                    return Ok(Some(FrameEvent::Dropped {
                        frame_id,
                        reason: FrameDropReason::TrailerBeforeLeader,
                    }));
                };

                if packet_id <= frame.packet_id {
                    stats.record_packet_anomaly();
                    stats.record_drop();
                    return Ok(Some(FrameEvent::Dropped {
                        frame_id,
                        reason: FrameDropReason::ShortFrame,
                    }));
                }

                match frame.trailer_packet_id {
                    Some(existing) if existing == packet_id => {
                        stats.record_packet_anomaly();
                        frame.last_update = now;
                        self.frames.insert(key, frame);
                        Ok(None)
                    }
                    Some(_) => {
                        stats.record_packet_anomaly();
                        stats.record_drop();
                        Ok(Some(FrameEvent::Dropped {
                            frame_id,
                            reason: FrameDropReason::PacketAfterTrailer,
                        }))
                    }
                    None => {
                        frame.trailer_packet_id = Some(packet_id);
                        frame.last_update = now;
                        Ok(self.reconcile_frame(key, frame, stats))
                    }
                }
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

    fn reconcile_frame(
        &mut self,
        key: FrameKey,
        frame: PartialFrame,
        stats: &StreamStats,
    ) -> Option<FrameEvent> {
        match progress_frame(frame) {
            FrameProgress::Pending(frame) => {
                self.frames.insert(key, frame);
                None
            }
            FrameProgress::Complete(frame) => {
                stats.record_frame();
                Some(FrameEvent::Complete(frame))
            }
            FrameProgress::Dropped(reason) => {
                stats.record_drop();
                stats.record_packet_anomaly();
                Some(FrameEvent::Dropped {
                    frame_id: key.0,
                    reason,
                })
            }
        }
    }
}

fn build_partial_frame(
    frame_id: u32,
    packet_id: u32,
    timestamp: u64,
    payload_type: PayloadType,
    pixel_format: PixelFormat,
    width: u32,
    height: u32,
    now: Instant,
) -> Result<PartialFrame, AssembleError> {
    let len = pixel_format
        .payload_len(width, height)
        .map_err(|_| AssembleError::UnsupportedPayload(payload_type))?;

    Ok(PartialFrame {
        frame_id,
        packet_id,
        timestamp,
        payload_type,
        pixel_format,
        width,
        height,
        offset: 0,
        data: vec![0; len],
        pending_payloads: BTreeMap::new(),
        trailer_packet_id: None,
        last_update: now,
    })
}

fn progress_frame(mut frame: PartialFrame) -> FrameProgress {
    if let Err(reason) = flush_pending_payloads(&mut frame) {
        return FrameProgress::Dropped(reason);
    }

    let Some(trailer_packet_id) = frame.trailer_packet_id else {
        return FrameProgress::Pending(frame);
    };

    if frame.packet_id.checked_add(1) != Some(trailer_packet_id) {
        return FrameProgress::Pending(frame);
    }

    if frame.offset != frame.data.len() {
        return FrameProgress::Dropped(FrameDropReason::ShortFrame);
    }

    FrameProgress::Complete(VideoFrame {
        frame_id: frame.frame_id,
        timestamp: frame.timestamp,
        width: frame.width,
        height: frame.height,
        pixel_format: frame.pixel_format,
        payload_type: frame.payload_type,
        data: frame.data,
    })
}

fn append_payload_bytes(
    frame: &mut PartialFrame,
    packet_id: u32,
    data: &[u8],
) -> Result<(), FrameDropReason> {
    let next_offset = frame.offset + data.len();
    if next_offset > frame.data.len() {
        return Err(FrameDropReason::PayloadOverflow);
    }

    frame.data[frame.offset..next_offset].copy_from_slice(data);
    frame.offset = next_offset;
    frame.packet_id = packet_id;
    Ok(())
}

fn flush_pending_payloads(frame: &mut PartialFrame) -> Result<(), FrameDropReason> {
    loop {
        let Some(next_packet_id) = frame.packet_id.checked_add(1) else {
            return Err(FrameDropReason::PacketAfterTrailer);
        };

        let Some(chunk) = frame.pending_payloads.remove(&next_packet_id) else {
            return Ok(());
        };

        append_payload_bytes(frame, next_packet_id, &chunk)?;
    }
}

fn buffered_payloads_overflow(frame: &PartialFrame, next_payload_len: usize) -> bool {
    let remaining = frame.data.len().saturating_sub(frame.offset);
    pending_payload_bytes(frame)
        .checked_add(next_payload_len)
        .is_none_or(|buffered| buffered > remaining)
}

fn pending_payload_bytes(frame: &PartialFrame) -> usize {
    frame
        .pending_payloads
        .values()
        .fold(0usize, |total, chunk| total.saturating_add(chunk.len()))
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
    fn assembles_frame_with_reordered_payloads_and_early_trailer() {
        let stats = StreamStats::default();
        let mut assembler = FrameAssembler::new(Duration::from_secs(1));
        let now = Instant::now();

        assembler
            .ingest(
                CompatPacket::Leader {
                    frame_id: 9,
                    packet_id: 0,
                    timestamp: 99,
                    payload_type: PayloadType::Image,
                    pixel_format: PixelFormat::Mono8,
                    width: 6,
                    height: 1,
                },
                now,
                &stats,
            )
            .unwrap();

        assert!(assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 9,
                    packet_id: 2,
                    data: vec![3, 4],
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        assert!(assembler
            .ingest(
                CompatPacket::Trailer {
                    frame_id: 9,
                    packet_id: 4,
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        assert!(assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 9,
                    packet_id: 1,
                    data: vec![1, 2],
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        let event = assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 9,
                    packet_id: 3,
                    data: vec![5, 6],
                },
                now,
                &stats,
            )
            .unwrap()
            .unwrap();

        match event {
            FrameEvent::Complete(frame) => assert_eq!(frame.data, vec![1, 2, 3, 4, 5, 6]),
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[test]
    fn keeps_missing_payload_gap_open_until_timeout() {
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
                    width: 4,
                    height: 1,
                },
                now,
                &stats,
            )
            .unwrap();

        assert!(assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 11,
                    packet_id: 2,
                    data: vec![3, 4],
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        assert!(assembler
            .ingest(
                CompatPacket::Trailer {
                    frame_id: 11,
                    packet_id: 3,
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        let drops = assembler.reap_timeouts(now + Duration::from_millis(20), &stats);
        assert_eq!(
            drops,
            vec![FrameEvent::Dropped {
                frame_id: 11,
                reason: FrameDropReason::Timeout,
            }]
        );
    }

    #[test]
    fn drops_frame_when_buffered_reordered_payloads_exceed_remaining_capacity() {
        let stats = StreamStats::default();
        let mut assembler = FrameAssembler::new(Duration::from_secs(1));
        let now = Instant::now();

        assembler
            .ingest(
                CompatPacket::Leader {
                    frame_id: 12,
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

        assert!(assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 12,
                    packet_id: 2,
                    data: vec![1, 2, 3],
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        let event = assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 12,
                    packet_id: 3,
                    data: vec![4, 5],
                },
                now,
                &stats,
            )
            .unwrap()
            .unwrap();

        assert_eq!(
            event,
            FrameEvent::Dropped {
                frame_id: 12,
                reason: FrameDropReason::PayloadOverflow,
            }
        );
    }

    #[test]
    fn ignores_zero_length_payload_without_advancing_frame() {
        let stats = StreamStats::default();
        let mut assembler = FrameAssembler::new(Duration::from_secs(1));
        let now = Instant::now();

        assembler
            .ingest(
                CompatPacket::Leader {
                    frame_id: 14,
                    packet_id: 0,
                    timestamp: 0,
                    payload_type: PayloadType::Image,
                    pixel_format: PixelFormat::Mono8,
                    width: 2,
                    height: 1,
                },
                now,
                &stats,
            )
            .unwrap();

        assert!(assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 14,
                    packet_id: 1,
                    data: vec![],
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        assert!(assembler
            .ingest(
                CompatPacket::Trailer {
                    frame_id: 14,
                    packet_id: 2,
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        let event = assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 14,
                    packet_id: 1,
                    data: vec![9, 8],
                },
                now,
                &stats,
            )
            .unwrap()
            .unwrap();

        match event {
            FrameEvent::Complete(frame) => assert_eq!(frame.data, vec![9, 8]),
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[test]
    fn drops_short_frame_when_trailer_closes_packet_range() {
        let stats = StreamStats::default();
        let mut assembler = FrameAssembler::new(Duration::from_secs(1));
        let now = Instant::now();

        assembler
            .ingest(
                CompatPacket::Leader {
                    frame_id: 13,
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

        assert!(assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 13,
                    packet_id: 1,
                    data: vec![1, 2],
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        let event = assembler
            .ingest(
                CompatPacket::Trailer {
                    frame_id: 13,
                    packet_id: 2,
                },
                now,
                &stats,
            )
            .unwrap()
            .unwrap();

        assert_eq!(
            event,
            FrameEvent::Dropped {
                frame_id: 13,
                reason: FrameDropReason::ShortFrame,
            }
        );
    }

    #[test]
    fn duplicate_leader_restarts_frame_assembly() {
        let stats = StreamStats::default();
        let mut assembler = FrameAssembler::new(Duration::from_secs(1));
        let now = Instant::now();

        assembler
            .ingest(
                CompatPacket::Leader {
                    frame_id: 17,
                    packet_id: 0,
                    timestamp: 1,
                    payload_type: PayloadType::Image,
                    pixel_format: PixelFormat::Mono8,
                    width: 4,
                    height: 1,
                },
                now,
                &stats,
            )
            .unwrap();

        assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 17,
                    packet_id: 1,
                    data: vec![1, 2],
                },
                now,
                &stats,
            )
            .unwrap();

        let event = assembler
            .ingest(
                CompatPacket::Leader {
                    frame_id: 17,
                    packet_id: 0,
                    timestamp: 2,
                    payload_type: PayloadType::Image,
                    pixel_format: PixelFormat::Mono8,
                    width: 4,
                    height: 1,
                },
                now,
                &stats,
            )
            .unwrap()
            .unwrap();

        assert_eq!(
            event,
            FrameEvent::Dropped {
                frame_id: 17,
                reason: FrameDropReason::DuplicateLeader,
            }
        );

        assert!(assembler
            .ingest(
                CompatPacket::Payload {
                    frame_id: 17,
                    packet_id: 1,
                    data: vec![9, 8, 7, 6],
                },
                now,
                &stats,
            )
            .unwrap()
            .is_none());

        let event = assembler
            .ingest(
                CompatPacket::Trailer {
                    frame_id: 17,
                    packet_id: 2,
                },
                now,
                &stats,
            )
            .unwrap()
            .unwrap();

        match event {
            FrameEvent::Complete(frame) => assert_eq!(frame.data, vec![9, 8, 7, 6]),
            other => panic!("unexpected event {other:?}"),
        }
    }
}

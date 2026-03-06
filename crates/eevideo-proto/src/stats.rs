use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct StreamStats {
    frames: AtomicU64,
    dropped_frames: AtomicU64,
    packets: AtomicU64,
    packet_anomalies: AtomicU64,
}

impl StreamStats {
    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }

    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames.load(Ordering::Relaxed)
    }

    pub fn packets(&self) -> u64 {
        self.packets.load(Ordering::Relaxed)
    }

    pub fn packet_anomalies(&self) -> u64 {
        self.packet_anomalies.load(Ordering::Relaxed)
    }

    pub fn record_frame(&self) {
        self.frames.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_drop(&self) {
        self.dropped_frames.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_packet(&self) {
        self.packets.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_packet_anomaly(&self) {
        self.packet_anomalies.fetch_add(1, Ordering::Relaxed);
    }
}


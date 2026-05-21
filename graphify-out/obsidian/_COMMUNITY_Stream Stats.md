---
type: community
cohesion: 0.20
members: 10
---

# Stream Stats

**Cohesion:** 0.20 - loosely connected
**Members:** 10 nodes

## Members
- [[.dropped_frames()]] - code - crates/eevideo-proto/src/stats.rs
- [[.frames()]] - code - crates/eevideo-proto/src/stats.rs
- [[.packet_anomalies()]] - code - crates/eevideo-proto/src/stats.rs
- [[.packets()]] - code - crates/eevideo-proto/src/stats.rs
- [[.record_drop()]] - code - crates/eevideo-proto/src/stats.rs
- [[.record_frame()]] - code - crates/eevideo-proto/src/stats.rs
- [[.record_packet()]] - code - crates/eevideo-proto/src/stats.rs
- [[.record_packet_anomaly()]] - code - crates/eevideo-proto/src/stats.rs
- [[StreamStats]] - code - crates/eevideo-proto/src/stats.rs
- [[stats.rs]] - code - crates/eevideo-proto/src/stats.rs

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/Stream_Stats
SORT file.name ASC
```

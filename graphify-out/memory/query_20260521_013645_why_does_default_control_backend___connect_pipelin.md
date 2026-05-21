---
type: "query"
date: "2026-05-21T01:36:45.388725+00:00"
question: "Why does default_control_backend() connect Pipeline Builder to Sink Element, Control Backend?"
contributor: "graphify"
source_nodes: ["default_control_backend()", "EeVideoSink", "ManagedControlSettings", "ControlSession", "build_pipeline()"]
---

# Q: Why does default_control_backend() connect Pipeline Builder to Sink Element, Control Backend?

## Answer

default_control_backend() is the no-op SharedControlBackend factory in crates/eevideo-control/src/lib.rs. It bridges Control Backend because it returns Arc::new(NoopControlBackend) and is used to create ControlSession instances; it bridges Sink Element because EeVideoSink::default and eevideosrc ManagedControlSettings::default both call it as their default managed-control backend. It bridges Pipeline Builder because eeview pipeline tests inject default_control_backend() into build_pipeline(), so UI pipeline construction can exercise managed-control plumbing without requiring a live control service. The graph marks these as cross-community edges from Pipeline Builder to Sink Element and Control Backend.

## Source Nodes

- default_control_backend()
- EeVideoSink
- ManagedControlSettings
- ControlSession
- build_pipeline()
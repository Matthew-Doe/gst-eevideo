# eevideo

Rust GStreamer plugin workspace for EEVideo-compatible streaming.

This workspace intentionally implements the current public interoperability
profile exposed by the upstream Go projects:

- CoAP/register control remains a future integration seam.
- Streaming compatibility targets the currently shipped public compatibility wire path.
- Native EEVideo SoF/Data/EoF packets remain a follow-on phase.

See [docs/implementation-profile.md](docs/implementation-profile.md) for the
normative scope of this repository and
[docs/interop-smoke.md](docs/interop-smoke.md) for the manual upstream
interoperability check.

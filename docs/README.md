# Documentation Guide

This directory is the working docs set for `eevideo`.

Use this page to find the shortest path to the information you need instead of
guessing between overlapping guides.

## Start Here

- [../README.md](../README.md) for the repository overview, quickstart, and core
  examples
- [developer-guide.md](developer-guide.md) for contributor workflow, build/test
  commands, and where to change code

## Device Setup Guides

- [linux-device-first-time-setup.md](linux-device-first-time-setup.md) for
  general Linux `eedeviced` bring-up with `synthetic`, `v4l2`, or `pipeline`
- [jetson-orin-first-time-setup.md](jetson-orin-first-time-setup.md) for Jetson
  Orin bring-up with the recommended pipeline-backed CSI path
- [jetson-nano-jetpack4-first-time-setup.md](jetson-nano-jetpack4-first-time-setup.md)
  for Jetson Nano on JetPack 4.x

## Reference Docs

- [eedeviced-provider-guide.md](eedeviced-provider-guide.md) for provider
  selection, per-provider constraints, and CLI examples
- [../cross/jetson-orin/README.md](../cross/jetson-orin/README.md) for the
  optional Jetson cross-build fallback path
- [compatibility-stream-profile.md](compatibility-stream-profile.md) for the
  active transport profile rules
- [implementation-profile.md](implementation-profile.md) for the repo's current
  interoperability scope and non-goals
- [interop-smoke.md](interop-smoke.md) for manual upstream Go interoperability
  checks

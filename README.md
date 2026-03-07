# eevideo

`eevideo` is a Rust workspace for an EEVideo-oriented GStreamer plugin.

The repository provides two elements:

- `eevideosrc` receives uncompressed frames over UDP and outputs GStreamer video buffers
- `eevideosink` packetizes uncompressed video buffers and transmits them over UDP

The workspace now also provides two host-side tools:

- `eevid` for CoAP discovery, register access, and stream control
- `eeview` for managed live viewing and optional recording with open codecs

The current focus is a functional host-side MVP built around the existing
public compatibility stream profile rather than a full native EEVideo transport
stack.

The original EEVideo repositories and specification work are published under
the EEVideo GitLab group:

- https://gitlab.com/eevideo

## New Developer Quick Start

If you are new to this repository, start here:

1. Read [docs/developer-guide.md](docs/developer-guide.md).
2. Install the toolchain and GStreamer dependencies for your platform.
3. Run `cargo test --workspace`.
4. Run the localhost smoke test from this README.
5. Read [docs/implementation-profile.md](docs/implementation-profile.md) before changing packet behavior.

## Current Scope

Implemented today:

- a Rust protocol crate for the current compatibility packet layout
- frame assembly, packet anomaly handling, and stream statistics
- `eevideosrc` and `eevideosink`
- unit tests and feature-gated GStreamer integration tests
- Windows and Jetson-oriented build scaffolding

Explicitly out of scope for v1:

- native EEVideo SoF/Data/EoF framing
- public CoAP/register control integration
- JPEG transport
- resend, FEC, or security profiles
- dynamic mid-stream caps renegotiation

## Workspace Layout

- `crates/eevideo-proto`
  - compatibility packet parse/serialize logic
  - pixel-format mapping
  - frame assembly
  - stream statistics
- `crates/eevideo-control`
  - CoAP discovery and register access
  - YAML-backed device/register metadata
  - shared host-side control/session APIs
- `crates/gst-plugin-eevideo`
  - the GStreamer plugin implementation
  - `eevideosrc`
  - `eevideosink`
- `crates/eevid`
  - discovery, describe, register read/write, and stream control CLI
- `crates/eeview`
  - managed live viewer and recorder CLI
- `docs/`
  - implementation profile
  - interoperability smoke procedure
  - spec enhancement proposal
- `cross/jetson-orin`
  - cross-build notes and container assets

## Supported Formats

The plugin currently supports these uncompressed formats:

- `video/x-raw,format=GRAY8`
- `video/x-raw,format=GRAY16_LE`
- `video/x-raw,format=RGB`
- `video/x-raw,format=UYVY`
- `video/x-bayer,format=grbg`
- `video/x-bayer,format=rggb`
- `video/x-bayer,format=gbrg`
- `video/x-bayer,format=bggr`

Anything else should be converted upstream with standard elements such as
`videoconvert` or `bayer2rgb`.

## Compatibility Model

This repository intentionally follows the currently deployed public host-side
behavior.

That means the active stream profile is the named
`EEVideo Stream Compatibility Profile v1`.

Its packet model is:

- leader packet starts a frame
- payload packets carry contiguous image bytes
- trailer packet closes the frame packet range, but completion still waits for
  any missing payload packets
- width, height, payload type, and pixel format are fixed after the first complete frame
- incomplete or malformed frames are dropped
- zero-length payload packets are ignored as anomalies
- out-of-order payload packets are buffered within a frame until they can be
  assembled, dropped for bounded overflow, or timed out

See [docs/compatibility-stream-profile.md](docs/compatibility-stream-profile.md)
for the normative packet rules and
[docs/implementation-profile.md](docs/implementation-profile.md) for the
project scope.

## Prerequisites

### Windows

Recommended environment:

- Rust stable with `x86_64-pc-windows-msvc`
- Visual Studio Build Tools with the C++ workload
- GStreamer MSVC runtime and development packages, `1.26+`
- a Windows-safe `pkg-config`

Typical environment setup:

```powershell
$env:PKG_CONFIG = "C:\ProgramData\chocolatey\bin\pkg-config.exe"
$env:PKG_CONFIG_PATH = "C:\Program Files\gstreamer\1.0\msvc_x86_64\lib\pkgconfig"
$env:Path = "C:\Program Files\gstreamer\1.0\msvc_x86_64\bin;$env:Path"
```

The Chocolatey `pkg-config` binary works with the standard `Program Files`
install path. Some MSYS2 `pkg-config` builds mishandle that path on Windows. If
you are stuck with one of those builds, use a no-space mirror or junction only
as a fallback workaround.

### Linux

Install:

- Rust stable
- `pkg-config`
- GStreamer development packages for `gstreamer-1.0` and `gstreamer-base-1.0`

## Build

Debug build:

```sh
cargo build --workspace
```

Release build:

```sh
cargo build --release --workspace
```

## Test

Run the workspace tests:

```sh
cargo test --workspace
```

Build the host-side CLIs:

```sh
cargo build -p eevid -p eeview
```

`gst-plugin-eevideo` tests load GStreamer at runtime, so the GStreamer runtime
DLL directory must be on `PATH` when you run the test binaries.

Run the feature-gated GStreamer integration tests:

```sh
cargo test -p gst-plugin-eevideo --features gst-tests
```

## Basic Local Smoke Test

Build the release plugin first:

```sh
cargo build --release --workspace
```

Point GStreamer at the built plugin:

```sh
set GST_PLUGIN_PATH=C:\devel\eevideo\target\release
```

Receiver:

```sh
gst-launch-1.0 eevideosrc address=127.0.0.1 port=5000 timeout-ms=2000 ! videoconvert ! autovideosink
```

Sender:

```sh
gst-launch-1.0 videotestsrc ! video/x-raw,format=RGB,width=640,height=480,framerate=30/1 ! eevideosink host=127.0.0.1 port=5000 mtu=4000
```

## Webcam Test

Example Windows sender:

```sh
gst-launch-1.0 ksvideosrc ! videoconvert ! video/x-raw,format=RGB,width=640,height=480,framerate=30/1 ! eevideosink host=127.0.0.1 port=5000 mtu=4000
```

Example receiver:

```sh
gst-launch-1.0 eevideosrc address=127.0.0.1 port=5000 timeout-ms=2000 ! videoconvert ! fpsdisplaysink sync=false video-sink=d3d11videosink text-overlay=true
```

## LAN Throughput Notes

For standard 1500-byte Ethernet, prefer an `mtu` in the `1400` to `1472`
range. That keeps the compatibility UDP payload under a normal Ethernet frame
budget without assuming jumbo frames.

Practical guidance:

- `1280x720@60 RGB` exceeds a 1 Gb link in practice; use `UYVY` when you want a
  720p60 saturation test on gigabit Ethernet
- use `mtu=1400` first for standard LAN testing
- only use larger `mtu` values after jumbo frames are enabled end to end on
  both NICs and the switch

Observed result on a direct Windows-to-Windows Ethernet link with jumbo frames
validated by `ping -f -l 8972`:

- `1280x720@60 UYVY` with `mtu=8900` sustained roughly `44` to `54` fps
- `mtu=1400` remained packet-rate limited and is unusable for higher-resolution,
  higher-framerate operation in the current implementation on that setup

Treat `~38 fps` as a practical minimum bar for the current jumbo-frame LAN smoke
test. `60 fps` remains the target, not the guaranteed floor.

If you want to measure the current sender/receiver stack locally before running
across the LAN, there is a manual feature-gated harness:

```sh
cargo test -p gst-plugin-eevideo --features gst-tests --test throughput_measurement -- --ignored --nocapture
```

## Multiple Receivers On One Port

Multiple `eevideosrc` instances can share the same UDP port when the sender uses
IPv4 multicast.

Start each receiver with the same `multicast-group` and `port`:

```sh
gst-launch-1.0 eevideosrc address=0.0.0.0 multicast-group=239.255.10.11 port=5000 timeout-ms=2000 ! videoconvert ! autovideosink sync=false
```

Send to that multicast group:

```sh
gst-launch-1.0 videotestsrc ! video/x-raw,format=UYVY,width=640,height=480,framerate=30/1 ! eevideosink host=239.255.10.11 port=5000 multicast-loop=true mtu=4000
```

This is different from unicast localhost loopback. Two unicast receivers cannot
both bind `127.0.0.1:5000`, but multiple multicast receivers can share the same
port and receive the same stream.

On multihomed systems, you can set `multicast-iface` on both `eevideosrc` and
`eevideosink` to pin multicast traffic to a specific local IPv4 interface
address.

## Upstream Interoperability

This repository no longer vendors the upstream EEVideo source trees.

Original upstream source code, related projects, and specification documents
are available at:

- https://gitlab.com/eevideo

Use that GitLab group as the canonical upstream starting point for the original
EEVideo repository layout and spec material.

If you want to validate against the public Go tools, clone the relevant
upstream repositories separately from that group and follow
[docs/interop-smoke.md](docs/interop-smoke.md).

## Host Tools

List discoverable devices:

```sh
cargo run -p eevid -- discover
```

Describe a specific device:

```sh
cargo run -p eevid -- --device-uri coap://192.168.1.50:5683 describe
```

Start managed viewing on a specific local receive address:

```sh
cargo run -p eeview -- --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --port 5000
```

Record with open codecs only:

```sh
cargo run -p eeview -- --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --record capture.mkv --encoder av1
```

## Additional Documentation

- [docs/developer-guide.md](docs/developer-guide.md)
- [docs/compatibility-stream-profile.md](docs/compatibility-stream-profile.md)
- [docs/implementation-profile.md](docs/implementation-profile.md)
- [docs/interop-smoke.md](docs/interop-smoke.md)
- [docs/spec-enhancement-proposal.md](docs/spec-enhancement-proposal.md)
- [docs/async-metadata-layout-plan.md](docs/async-metadata-layout-plan.md)

## Roadmap

Near term:

- improve Jetson cross-build validation

Completed on this branch:

- harden behavior under loss and reordering outside localhost
- formalize the compatibility stream profile more clearly
- preserve a clean seam for future control-plane integration

Later:

- native EEVideo framing
- device control integration
- richer timing semantics
- optional transport resilience features

# eevideo

`eevideo` is a Rust workspace for an EEVideo-oriented GStreamer plugin.

The repository provides two elements:

- `eevideosrc` receives uncompressed frames over UDP and outputs GStreamer video buffers
- `eevideosink` packetizes uncompressed video buffers and transmits them over UDP

The workspace now also provides two host-side tools:

- `eevid` for CoAP discovery, register access, and stream control
- `eeview` for managed live viewing and optional recording with open codecs

It also provides two device daemons:

- `eefakedev` for a pure-Rust test-pattern EEVideo device you can run on a second machine
- `eedeviced` for a single-stream EEVideo device daemon with synthetic, V4L2, generic
  GStreamer, and Jetson-oriented providers

The current focus is a functional host-side MVP built around the existing
public compatibility stream profile rather than a full native EEVideo transport
stack.

The original EEVideo repositories and specification work are published under
the EEVideo GitLab group:

- https://gitlab.com/eevideo

## New Developer Quick Start

If you are new to this repository, start here:

1. Read [docs/README.md](docs/README.md).
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
- a reusable device runtime plus a single-stream Jetson-oriented device daemon

Explicitly out of scope for v1:

- native EEVideo SoF/Data/EoF framing
- JPEG transport
- resend, FEC, or security profiles
- dynamic mid-stream caps renegotiation
- multi-stream production device firmware and richer hardware control beyond the current single-stream daemon

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
- `crates/eevideo-device`
  - reusable EEVideo device runtime
  - discovery/control handling for device daemons
- `crates/eefakedev`
  - fake EEVideo device daemon with a pure-Rust test-pattern source
- `crates/eedeviced`
  - single-stream EEVideo device daemon
  - synthetic, V4L2, and pipeline-backed capture paths
  - built-in Argus convenience path available but not currently validated in
    this repo due to lack of matching hardware coverage
- `docs/`
  - implementation profile
  - interoperability smoke procedure
  - spec enhancement proposal
- `cross/jetson-orin`
  - experimental Jetson cross-build notes, systemd assets, and container helpers

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
$env:GSTREAMER_ROOT = "C:\Program Files\gstreamer\1.0\msvc_x86_64"
$env:PKG_CONFIG = "C:\ProgramData\chocolatey\bin\pkg-config.exe"
$env:PKG_CONFIG_PATH = "$env:GSTREAMER_ROOT\lib\pkgconfig"
$env:GSTREAMER_LIB_DIR = "$env:GSTREAMER_ROOT\lib"
$env:GSTREAMER_BIN_DIR = "$env:GSTREAMER_ROOT\bin"
$env:Path = "$env:GSTREAMER_BIN_DIR;$env:Path"
```

The Chocolatey `pkg-config` binary works with the standard `Program Files`
install path. Some MSYS2 `pkg-config` builds mishandle that path on Windows. If
you are stuck with one of those builds, use a no-space mirror or junction only
as a fallback workaround.

The checked-in Windows test runner is launched from `.cargo/config.toml`, so it
does not depend on the shell's current working directory.

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

Build the host-side CLIs and device daemons:

```sh
cargo build -p eevid -p eeview -p eefakedev -p eedeviced
```

`gst-plugin-eevideo` tests load GStreamer at runtime, so the GStreamer runtime
DLL directory must be on `PATH` when you run the test binaries.

On Windows, the checked-in runner will also derive the GStreamer `bin` directory
from `GSTREAMER_BIN_DIR`, `GSTREAMER_LIB_DIR`, or `PKG_CONFIG_PATH` if needed.

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

Each host-side CLI includes built-in help. From a checkout, run
`cargo run -p eevid -- --help`, `cargo run -p eeview -- --help`,
`cargo run -p eefakedev -- --help`, or `cargo run -p eedeviced -- --help`.

List discoverable devices:

```sh
cargo run -p eevid -- discover
```

Describe a specific device:

```sh
cargo run -p eevid -- --device-uri coap://192.168.1.50:5683 describe
```

`eevid describe` now reports each stream's advertised mode, for example:

```text
stream stream0: UYVY 1280x720 @ 30 fps
```

Start managed viewing on a specific local receive address:

```sh
cargo run -p eeview -- --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --port 5000
```

`eeview` now shows a live FPS + stream-mode HUD by default. Pass
`--no-overlay` if you want a clean viewer window.

Record with open codecs only:

```sh
cargo run -p eeview -- --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --record capture.mkv --encoder av1
```

## Two-PC Fake Device Workflow

Run `eefakedev` on `PC1` to emulate a single-stream EEVideo camera with a built-in test pattern:

```sh
cargo run -p eefakedev -- --advertise-address 192.168.1.50
```

On `PC2`, discover the fake device:

```sh
cargo run -p eevid -- discover
```

Describe it directly:

```sh
cargo run -p eevid -- --device-uri coap://192.168.1.50:5683 describe
```

Look for the advertised mode line in the output:

```text
stream stream0: UYVY 1280x720 @ 30 fps
```

Start managed viewing from `PC2` using its concrete receive address:

```sh
cargo run -p eeview -- --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --port 5000
```

The viewer HUD is enabled by default; add `--no-overlay` to disable it.

That flow uses the CoAP/register control path to configure `PC1` and then starts a unicast
compatibility stream carrying the animated test pattern.

## EEVideo Device Providers

For the real single-stream device daemon, use `eedeviced`.

The current providers are:

- `synthetic` for local testing and protocol validation
- `v4l2` for Linux webcams and capture devices
- `pipeline` for arbitrary GStreamer pipelines that expose `appsink name=framesink`
- `argus` for a built-in Jetson CSI convenience path that is not currently
  validated in this repo due to lack of matching hardware coverage

For Jetson targets, the recommended path in this repo is:

- build `eedeviced` directly on the device
- use `--input pipeline` with an explicit `nvarguscamerasrc ... ! appsink` pipeline

The built-in `argus` provider remains available in the CLI, but it is not
currently a tested deployment path in this repo due to lack of matching
hardware coverage. The `cross/jetson-orin` helpers are kept as an
experimental fallback rather than the recommended bring-up flow.

The device stays fixed to one configured width, height, and pixel format per
process start. Unsupported host-side width, height, and pixel-format writes are
rejected by applied-value mismatch.

Synthetic mode is useful on any host:

```sh
cargo run -p eedeviced -- --bind 127.0.0.1:5683 --input synthetic --pixel-format mono8
```

Generic V4L2 mode is the first Linux hardware path:

```sh
./eedeviced --bind 0.0.0.0:5683 --advertise-address 192.168.1.50 --iface eth0 --input v4l2 --device /dev/video0 --pixel-format gray16le --width 640 --height 480 --fps 30 --mtu 1200
```

Generic GStreamer mode is the escape hatch for unusual sources:

```sh
./eedeviced --bind 0.0.0.0:5683 --advertise-address 192.168.1.50 --iface eth0 --input pipeline --pixel-format bayer-bg8 --width 1920 --height 1080 --fps 30 --pipeline "videotestsrc is-live=true ! video/x-bayer,format=bggr,width=1920,height=1080,framerate=30/1 ! appsink name=framesink sync=false max-buffers=1 drop=true"
```

Jetson Nano on JetPack 4.x uses the same `pipeline` provider with an explicit
CSI pipeline:

```sh
./eedeviced --bind 0.0.0.0:5683 --advertise-address 192.168.1.50 --iface eth0 --input pipeline --pixel-format uyvy --width 1280 --height 720 --fps 30 --pipeline "nvarguscamerasrc sensor-id=0 ! video/x-raw(memory:NVMM),format=NV12,width=1280,height=720,framerate=30/1 ! nvvidconv ! video/x-raw,format=UYVY,width=1280,height=720 ! appsink name=framesink sync=false max-buffers=1 drop=true"
```

That pipeline must negotiate the same `UYVY 1280x720@30` mode that
`eedeviced` is configured with. If the appsink caps drift, startup fails early.

The same operator-owned `pipeline` approach is the recommended Jetson path on
Jetson Orin as well.

If you want to experiment with the built-in `argus` provider on Jetson Orin,
the CLI path is:

```sh
./eedeviced --bind 0.0.0.0:5683 --advertise-address 192.168.1.50 --iface eth0 --input argus --sensor-id 0 --pixel-format uyvy --width 1280 --height 720 --fps 30 --mtu 1200
```

Treat that `argus` path as experimental for now. It is untested here because
this repo does not currently have matching hardware coverage, so prefer the
explicit `pipeline` provider in production or first-time bring-up docs.

See [docs/eedeviced-provider-guide.md](docs/eedeviced-provider-guide.md) for the
provider matrix and per-provider constraints.
For end-to-end setup guides, start with [docs/README.md](docs/README.md) or jump
directly to:

- [docs/linux-device-first-time-setup.md](docs/linux-device-first-time-setup.md)
- [docs/jetson-nano-jetpack4-first-time-setup.md](docs/jetson-nano-jetpack4-first-time-setup.md)
- [docs/jetson-orin-first-time-setup.md](docs/jetson-orin-first-time-setup.md)

## Additional Documentation

- [docs/README.md](docs/README.md)
- [docs/developer-guide.md](docs/developer-guide.md)
- [docs/compatibility-stream-profile.md](docs/compatibility-stream-profile.md)
- [docs/eedeviced-provider-guide.md](docs/eedeviced-provider-guide.md)
- [docs/linux-device-first-time-setup.md](docs/linux-device-first-time-setup.md)
- [docs/implementation-profile.md](docs/implementation-profile.md)
- [docs/interop-smoke.md](docs/interop-smoke.md)
- [docs/jetson-nano-jetpack4-first-time-setup.md](docs/jetson-nano-jetpack4-first-time-setup.md)
- [docs/jetson-orin-first-time-setup.md](docs/jetson-orin-first-time-setup.md)

## Roadmap

Near term:

- validate `eedeviced` on more real hardware, especially broader V4L2 devices
  and Jetson Argus coverage once matching hardware is available
- improve packaging and release ergonomics for the host tools and device daemons
- expand interoperability validation beyond local loopback and synthetic-device
  coverage

Later:

- native EEVideo framing beyond the current compatibility profile
- richer device control and metadata transport
- richer timing semantics
- optional transport resilience features
- multi-stream device support

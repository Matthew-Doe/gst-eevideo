# eevideo

`eevideo` is a Rust workspace for an EEVideo-oriented GStreamer plugin.

The repository provides two elements:

- `eevideosrc` receives uncompressed frames over UDP and outputs GStreamer video buffers
- `eevideosink` packetizes uncompressed video buffers and transmits them over UDP

The current focus is a functional host-side MVP built around the existing
public compatibility stream profile rather than a full native EEVideo transport
stack.

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
- `crates/gst-plugin-eevideo`
  - the GStreamer plugin implementation
  - `eevideosrc`
  - `eevideosink`
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

That means the active stream profile is:

- leader packet starts a frame
- payload packets carry contiguous image bytes
- trailer packet completes a frame
- width, height, payload type, and pixel format are fixed after the first complete frame
- incomplete or malformed frames are dropped

See [docs/implementation-profile.md](docs/implementation-profile.md) for the
normative project scope.

## Prerequisites

### Windows

Recommended environment:

- Rust stable with `x86_64-pc-windows-msvc`
- Visual Studio Build Tools with the C++ workload
- GStreamer MSVC runtime and development packages, `1.26+`
- `pkg-config`

Typical environment setup:

```powershell
$env:PKG_CONFIG_PATH = "C:\gstreamer\1.0\msvc_x86_64\lib\pkgconfig"
$env:Path = "C:\gstreamer\1.0\msvc_x86_64\bin;$env:Path"
```

If `pkg-config` has trouble with spaces in `Program Files`, use a no-space
mirror or junction as a local workaround.

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

If you want to validate against the public Go tools, clone those upstream
repositories separately and follow
[docs/interop-smoke.md](docs/interop-smoke.md).

## Additional Documentation

- [docs/developer-guide.md](docs/developer-guide.md)
- [docs/implementation-profile.md](docs/implementation-profile.md)
- [docs/interop-smoke.md](docs/interop-smoke.md)
- [docs/spec-enhancement-proposal.md](docs/spec-enhancement-proposal.md)
- [docs/async-metadata-layout-plan.md](docs/async-metadata-layout-plan.md)

## Roadmap

Near term:

- harden behavior under loss and reordering outside localhost
- formalize the compatibility stream profile more clearly
- improve Jetson cross-build validation
- preserve a clean seam for future control-plane integration

Later:

- native EEVideo framing
- device control integration
- richer timing semantics
- optional transport resilience features

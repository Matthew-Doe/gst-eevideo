# Developer Guide

This document is for engineers who are changing the `eevideo` repository itself.
It focuses on contributor workflow, code ownership hints, and the minimum docs
you should read before changing behavior.

If you are trying to bring up a device rather than modify the repo, start with
[../README.md](../README.md) and [README.md](README.md).

## First-Day Checklist

Use this order:

1. Read [../README.md](../README.md).
2. Read [README.md](README.md) in this directory for the docs map.
3. Read [compatibility-stream-profile.md](compatibility-stream-profile.md).
4. Read [implementation-profile.md](implementation-profile.md).
5. Run `cargo test --workspace`.
6. Run a local sender/receiver smoke test from the repo README.

If you skip the profile docs, it is easy to implement behavior that sounds
reasonable but is outside the current repository scope.

## Toolchain Requirements

### Windows

Required:

- Rust stable with `x86_64-pc-windows-msvc`
- Visual Studio Build Tools with the C++ workload
- GStreamer MSVC runtime and development packages
- a Windows-safe `pkg-config`

Practical note:

- the Chocolatey `pkg-config` binary works with the standard `Program Files`
  GStreamer install path
- some MSYS2 `pkg-config` builds handle `Program Files` poorly; use a no-space
  mirror or junction only as a fallback

### Linux

Required:

- Rust stable
- `pkg-config`
- GStreamer development packages for `gstreamer-1.0` and `gstreamer-base-1.0`

## Environment Setup

### Windows PowerShell

```powershell
$env:GSTREAMER_ROOT = "C:\Program Files\gstreamer\1.0\msvc_x86_64"
$env:PKG_CONFIG = "C:\ProgramData\chocolatey\bin\pkg-config.exe"
$env:PKG_CONFIG_PATH = "$env:GSTREAMER_ROOT\lib\pkgconfig"
$env:GSTREAMER_LIB_DIR = "$env:GSTREAMER_ROOT\lib"
$env:GSTREAMER_BIN_DIR = "$env:GSTREAMER_ROOT\bin"
$env:Path = "$env:GSTREAMER_BIN_DIR;$env:Path"
```

### Windows cmd.exe

```cmd
set GSTREAMER_ROOT=C:\Program Files\gstreamer\1.0\msvc_x86_64
set PKG_CONFIG=C:\ProgramData\chocolatey\bin\pkg-config.exe
set PKG_CONFIG_PATH=%GSTREAMER_ROOT%\lib\pkgconfig
set GSTREAMER_LIB_DIR=%GSTREAMER_ROOT%\lib
set GSTREAMER_BIN_DIR=%GSTREAMER_ROOT%\bin
set PATH=%GSTREAMER_BIN_DIR%;%PATH%
```

## Build And Test Workflow

Fast sanity check:

```sh
cargo test --workspace
```

Release build:

```sh
cargo build --release --workspace
```

GStreamer integration tests:

```sh
cargo test -p gst-plugin-eevideo --features gst-tests
```

`gst-plugin-eevideo` tests load GStreamer at runtime, so the GStreamer runtime
directory must already be on `PATH` before you run them.

## Running The Plugin Locally

Build the release plugin:

```sh
cargo build --release --workspace
```

Set `GST_PLUGIN_PATH` to the release directory, then run a simple receiver and
sender pair:

```sh
gst-launch-1.0 eevideosrc address=127.0.0.1 port=5000 timeout-ms=2000 ! videoconvert ! autovideosink
```

```sh
gst-launch-1.0 videotestsrc ! video/x-raw,format=RGB,width=640,height=480,framerate=30/1 ! eevideosink host=127.0.0.1 port=5000 mtu=4000
```

For throughput-oriented LAN tests, prefer `UYVY` over `RGB`. Standard-MTU
settings like `mtu=1400` are useful for compatibility testing, but they are
packet-rate limited for high-bandwidth workloads.

### Multiple receivers on one port

If you want more than one `eevideosrc` process to receive the same stream on the
same UDP port, use IPv4 multicast.

Each receiver should bind the same `port` and join the same `multicast-group`:

```sh
gst-launch-1.0 eevideosrc address=0.0.0.0 multicast-group=239.255.10.11 port=5000 timeout-ms=2000 ! videoconvert ! autovideosink sync=false
```

The sender must transmit to that multicast destination:

```sh
gst-launch-1.0 videotestsrc ! video/x-raw,format=UYVY,width=640,height=480,framerate=30/1 ! eevideosink host=239.255.10.11 port=5000 multicast-loop=true mtu=4000
```

Two unicast listeners cannot both bind the same local UDP socket. Same-port
fanout in this plugin is multicast-based rather than unicast socket sharing.

## Where To Make Changes

### If you are changing the wire format

Start in:

- [compatibility-stream-profile.md](compatibility-stream-profile.md)
- [implementation-profile.md](implementation-profile.md)
- `crates/eevideo-proto/src/compat_stream.rs`
- `crates/eevideo-proto/src/assembler.rs`

You will usually also need tests under `crates/gst-plugin-eevideo/tests/`.

### If you are changing supported pixel formats

Start in:

- `crates/eevideo-proto/src/pixel_format.rs`
- `crates/gst-plugin-eevideo/src/common.rs`

Then rerun protocol tests, plugin tests, and a manual `gst-launch-1.0` smoke
run.

### If you are changing source behavior

Start in `crates/gst-plugin-eevideo/src/eevideosrc/imp.rs`.

Typical areas:

- receive loop
- timeout behavior
- caps negotiation
- PTS handling

### If you are changing sink behavior

Start in `crates/gst-plugin-eevideo/src/eevideosink/imp.rs`.

Typical areas:

- packetization
- pacing
- frame validation
- transport configuration

## Files New Contributors Should Understand Early

- [../Cargo.toml](../Cargo.toml)
- [../README.md](../README.md)
- [compatibility-stream-profile.md](compatibility-stream-profile.md)
- [implementation-profile.md](implementation-profile.md)
- `crates/eevideo-proto/src/compat_stream.rs`
- `crates/eevideo-proto/src/assembler.rs`
- `crates/gst-plugin-eevideo/src/eevideosrc/imp.rs`
- `crates/gst-plugin-eevideo/src/eevideosink/imp.rs`

## Project Rules That Matter

- the current stream profile is intentionally conservative
- mid-stream format changes are rejected
- the first release is transport-focused, not control-plane complete
- new behavior should be backed by tests before it is treated as stable
- if a change alters wire behavior, the docs need to change with the code

## Common Mistakes

- treating the project as if it already implements the future native EEVideo
  stream format
- expanding pixel-format support without updating caps mapping and tests
- adding metadata or control behavior without defining wire semantics first
- assuming Windows shell commands and PowerShell syntax are interchangeable
- testing only with `videotestsrc` and not with a real camera or receiver timing

## Suggested Development Loop

1. Update or add tests.
2. Change the protocol or plugin code.
3. Run `cargo test --workspace`.
4. Run a local `gst-launch-1.0` smoke test.
5. Update the relevant Markdown docs if behavior changed.

## Troubleshooting

### `pkg-config` cannot find GStreamer

Check:

- `PKG_CONFIG`
- `PKG_CONFIG_PATH`
- whether GStreamer dev packages are installed
- whether the GStreamer runtime `bin` directory is on `PATH`

### `cl.exe` is missing on Windows

Open a Visual Studio developer shell or install the C++ build workload.

### `gst-launch-1.0` cannot see the plugin

Check:

- `GST_PLUGIN_PATH`
- whether you built `target/debug` or `target/release`
- whether the plugin DLL or SO is under the path you exported

### The stream is very slow

Check:

- whether you are using a debug build
- whether the sink is converting formats expensively
- whether the display sink is syncing to timestamps
- whether the configured MTU is too small

## Related Docs

- [interop-smoke.md](interop-smoke.md) for testing against upstream Go tools
- https://gitlab.com/eevideo for original upstream source repositories

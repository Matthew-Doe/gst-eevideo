# Developer Guide

This document is for engineers who are new to the `eevideo` repository and need
to build, test, run, and modify the current project without guessing how the
pieces fit together.

## What This Repository Is

This repository contains a Rust GStreamer plugin workspace with two crates:

- `eevideo-proto`
  - protocol parsing and serialization
  - pixel-format mapping
  - frame assembly
  - stream statistics
- `gst-plugin-eevideo`
  - the GStreamer elements
  - network receive/transmit behavior
  - caps negotiation
  - source and sink integration tests

The project currently targets the existing public EEVideo compatibility stream
profile, not a fully native EEVideo transport specification.

## First-Day Checklist

Use this order:

1. Read [README.md](c:/devel/eevideo/README.md).
2. Read [implementation-profile.md](c:/devel/eevideo/docs/implementation-profile.md).
3. Build and run `cargo test --workspace`.
4. Run the local sender/receiver smoke test from the README.
5. Only after that, start changing code.

If you skip step 2, you can easily implement behavior that looks reasonable but
is outside the current project scope.

## Toolchain Requirements

### Windows

Required:

- Rust stable with `x86_64-pc-windows-msvc`
- Visual Studio Build Tools with the C++ workload
- GStreamer MSVC runtime and development packages
- `pkg-config`

Practical note:

- Some Windows `pkg-config` setups handle `Program Files` poorly. If that
  happens, use a no-space mirror or junction for the GStreamer install and point
  `PKG_CONFIG_PATH` there.

### Linux

Required:

- Rust stable
- `pkg-config`
- GStreamer development packages for `gstreamer-1.0` and `gstreamer-base-1.0`

## Environment Setup

### Windows PowerShell

```powershell
$env:PKG_CONFIG_PATH = "C:\gstreamer\1.0\msvc_x86_64\lib\pkgconfig"
$env:Path = "C:\gstreamer\1.0\msvc_x86_64\bin;$env:Path"
```

### Windows cmd.exe

```cmd
set PKG_CONFIG_PATH=C:\gstreamer\1.0\msvc_x86_64\lib\pkgconfig
set PATH=C:\gstreamer\1.0\msvc_x86_64\bin;%PATH%
```

## Build And Test Workflow

### Fast sanity check

```sh
cargo test --workspace
```

### Release build

```sh
cargo build --release --workspace
```

### GStreamer integration tests

```sh
cargo test -p gst-plugin-eevideo --features gst-tests
```

## Running The Plugin Locally

Build the release plugin:

```sh
cargo build --release --workspace
```

Set:

```cmd
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

## Where To Make Changes

### If you are changing the wire format

Start in:

- `crates/eevideo-proto/src/compat_stream.rs`
- `crates/eevideo-proto/src/assembler.rs`
- `docs/implementation-profile.md`

You will usually also need to touch:

- tests under `crates/gst-plugin-eevideo/tests/`

### If you are changing supported pixel formats

Start in:

- `crates/eevideo-proto/src/pixel_format.rs`
- `crates/gst-plugin-eevideo/src/common.rs`

Then re-run:

- protocol tests
- plugin tests
- a manual `gst-launch-1.0` smoke run

### If you are changing source behavior

Start in:

- `crates/gst-plugin-eevideo/src/eevideosrc/imp.rs`

Typical areas:

- receive loop
- timeout behavior
- caps negotiation
- PTS handling

### If you are changing sink behavior

Start in:

- `crates/gst-plugin-eevideo/src/eevideosink/imp.rs`

Typical areas:

- packetization
- pacing
- frame validation
- transport configuration

## Files New Developers Should Understand Early

- [Cargo.toml](c:/devel/eevideo/Cargo.toml)
- [README.md](c:/devel/eevideo/README.md)
- [implementation-profile.md](c:/devel/eevideo/docs/implementation-profile.md)
- [compat_stream.rs](c:/devel/eevideo/crates/eevideo-proto/src/compat_stream.rs)
- [assembler.rs](c:/devel/eevideo/crates/eevideo-proto/src/assembler.rs)
- [imp.rs](c:/devel/eevideo/crates/gst-plugin-eevideo/src/eevideosrc/imp.rs)
- [imp.rs](c:/devel/eevideo/crates/gst-plugin-eevideo/src/eevideosink/imp.rs)

## Project Rules That Matter

- The current stream profile is intentionally conservative.
- Mid-stream format changes are rejected.
- The first release is transport-focused, not control-plane complete.
- New behavior should be backed by tests before it is treated as stable.
- If a change alters wire behavior, the docs need to change with the code.

## Common Mistakes

- Treating the project as if it already implements the future native EEVideo stream format
- Expanding pixel-format support without updating caps mapping and tests
- Adding metadata or control behavior without defining wire semantics first
- Assuming Windows shell commands and PowerShell syntax are interchangeable
- Testing only with `videotestsrc` and not with a real camera or real receiver timing

## Suggested Development Loop

Use this order for most changes:

1. Update or add tests.
2. Change the protocol or plugin code.
3. Run `cargo test --workspace`.
4. Run a local `gst-launch-1.0` smoke test.
5. Update the relevant Markdown docs if behavior changed.

## Troubleshooting

### `pkg-config` cannot find GStreamer

Check:

- `PKG_CONFIG_PATH`
- whether GStreamer dev packages are installed
- whether your Windows install path has spaces that your `pkg-config` build mishandles

### `cl.exe` is missing on Windows

Open a Visual Studio developer shell or install the C++ build workload.

### `gst-launch-1.0` cannot see the plugin

Check:

- `GST_PLUGIN_PATH`
- whether you built `target/debug` or `target/release`
- whether the plugin DLL/SO is under the path you exported

### The stream is very slow

Check:

- whether you are using a debug build
- whether the sink is converting formats expensively
- whether the display sink is syncing to timestamps
- whether the configured MTU is too small

## When To Read The Other Docs

- Read [interop-smoke.md](c:/devel/eevideo/docs/interop-smoke.md) if you want to test against upstream Go tools.
- Read [spec-enhancement-proposal.md](c:/devel/eevideo/docs/spec-enhancement-proposal.md) if you need the rationale behind current protocol constraints.
- Read [async-metadata-layout-plan.md](c:/devel/eevideo/docs/async-metadata-layout-plan.md) if you are exploring future metadata-aware transport designs.

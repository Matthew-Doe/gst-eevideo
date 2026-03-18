# Jetson Cross Build Assets

This directory contains the Jetson cross-build path for the Rust `eevideo`
plugin and the `eedeviced` device daemon.

Cross-building is not the recommended Jetson bring-up path in this repo.
Prefer building directly on the target Jetson whenever possible. These assets
are kept as an experimental or convenience fallback for cases where a local
Jetson build is not practical.

## Assumptions

- Host build environment is Linux or WSL2.
- Target is a Jetson sysroot that provides the matching GStreamer development
  files for the board you are building for.
- Jetson Orin on JetPack 6.x / Ubuntu 22.04 and Jetson Nano on JetPack 4.x /
  L4T 32.7.x may still require environment-specific fixes when cross-building.
- A Jetson sysroot is available locally and already contains the target
  GStreamer development files.

## Inputs

- `JETSON_SYSROOT`: absolute path to the Jetson sysroot.
- `aarch64-linux-gnu-gcc`: available on the host.
- Rust target `aarch64-unknown-linux-gnu`: installed locally.

## Build

```sh
cross/jetson-orin/build.sh /absolute/path/to/jetson-sysroot
```

The resulting artifacts will be emitted under:

```text
target/aarch64-unknown-linux-gnu/release/
```

Notable outputs:

- `libgsteevideo.so`
- `eedeviced`

Service packaging assets live under:

```text
cross/jetson-orin/systemd/
```

Notable deployment files:

- `eedeviced.service`
- `eedeviced-launch.sh`
- `eedeviced.env.example`

## Recommended Native Build

If you can log in to the target Jetson, prefer a native build there:

```sh
cargo build --release -p eedeviced
```

That produces:

```text
target/release/eedeviced
```

The Jetson setup guides in `docs/` assume this native-build path first.

## Nano Deployment Notes

For Jetson Nano on JetPack 4.x:

- prefer a native on-device build over this cross-build path
- if you still use this cross-build flow, use a Nano sysroot that matches the
  target board exactly
- copy `eedeviced`, `eedeviced.service`, `eedeviced-launch.sh`, and
  `eedeviced.env.example` to the Nano
- set `EEVIDEO_INPUT=pipeline`
- set `EEVIDEO_PIPELINE` to the Nano CSI pipeline described in
  [docs/jetson-nano-jetpack4-first-time-setup.md](../../docs/jetson-nano-jetpack4-first-time-setup.md)

For Jetson Orin, the recommended provider is also `pipeline`. The built-in
`argus` provider remains available in the CLI, but it is not currently a
tested deployment path in this repo due to lack of matching hardware
coverage.

No Rust dependency downgrade is planned for Nano JetPack 4.x. The current
workspace GStreamer sys crates still target system GStreamer `>= 1.14`.

## Optional Container Flow

Build the helper image:

```sh
docker build -f cross/jetson-orin/Dockerfile -t eevideo-jetson-build .
```

Run it with the project and sysroot mounted:

```sh
docker run --rm \
  -v "$PWD:/workspace" \
  -v "/absolute/path/to/jetson-sysroot:/opt/jetson-sysroot:ro" \
  eevideo-jetson-build \
  /workspace/cross/jetson-orin/build.sh /opt/jetson-sysroot
```

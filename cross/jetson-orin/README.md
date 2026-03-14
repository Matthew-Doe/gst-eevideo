# Jetson Cross Build Assets

This directory contains the Jetson cross-build path for the Rust `eevideo`
plugin and the `eedeviced` device daemon.

It is currently validated for Jetson Orin on JetPack 6.x and documented for
Jetson Nano on JetPack 4.x pipeline deployments.

## Assumptions

- Host build environment is Linux or WSL2.
- Target is a Jetson sysroot that provides the matching GStreamer development
  files for the board you are building for.
- Jetson Orin on JetPack 6.x / Ubuntu 22.04 is the currently validated target.
- Jetson Nano on JetPack 4.x / L4T 32.7.x uses the same build flow with a Nano
  sysroot.
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

## Nano Deployment Notes

For Jetson Nano on JetPack 4.x:

- use the same `cross/jetson-orin/build.sh` flow with a Nano sysroot
- copy `eedeviced`, `eedeviced.service`, `eedeviced-launch.sh`, and
  `eedeviced.env.example` to the Nano
- set `EEVIDEO_INPUT=pipeline`
- set `EEVIDEO_PIPELINE` to the Nano CSI pipeline described in
  [docs/jetson-nano-jetpack4-first-time-setup.md](../../docs/jetson-nano-jetpack4-first-time-setup.md)

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

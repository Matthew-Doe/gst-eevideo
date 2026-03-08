# Jetson Orin Cross Build

This directory contains the JetPack 6.x cross-build path for the Rust
`eevideo` plugin and the `eedeviced` device daemon.

## Assumptions

- Host build environment is Linux or WSL2.
- Target is Jetson Orin on JetPack 6.x / Ubuntu 22.04.
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

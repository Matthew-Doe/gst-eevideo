# `eedeviced` Provider Guide

`eedeviced` is the single-stream EEVideo device daemon in this workspace.

It exposes one `CompatibilityV1` stream over the existing CoAP/register control
contract, but the frame source is now selected with `--input`.

If you need a first-time bring-up path for general Linux devices, use
[linux-device-first-time-setup.md](linux-device-first-time-setup.md).
For Jetson Nano on JetPack 4.x, use
[jetson-nano-jetpack4-first-time-setup.md](jetson-nano-jetpack4-first-time-setup.md).
For Jetson Orin, use [jetson-orin-first-time-setup.md](jetson-orin-first-time-setup.md).

For Jetson deployments in this repo, the recommended pattern is:

- build `eedeviced` directly on the Jetson
- use `--input pipeline` with an explicit `nvarguscamerasrc ... ! appsink` pipeline

The built-in `argus` provider remains available in the CLI, but it is not
currently a tested deployment path here due to lack of matching hardware
coverage in this repo. The cross-build helpers are kept as a fallback, not the
recommended bring-up flow.

Use this guide as a provider reference, not a full bring-up walkthrough. The
setup guides above own the end-to-end build, install, verification, and service
steps.

## Fixed-Mode Behavior

`eedeviced` stays fixed to one configured mode per process start:

- `--width`
- `--height`
- `--pixel-format`
- `--fps`

The host can still configure destination address, port, packet delay, and MTU,
but incompatible width, height, and pixel-format writes are rejected by
applied-value mismatch.

## Pixel Formats

Supported `--pixel-format` values are:

- `uyvy`
- `mono8`
- `gray8`
- `mono`
- `mono16`
- `gray16le`
- `bayer-gr8`
- `bayer-rg8`
- `bayer-gb8`
- `bayer-bg8`

`gray8` and `mono` map to `Mono8`.

## Providers

### `synthetic`

Use this for local testing or protocol validation on any host.

Example:

```sh
cargo run -p eedeviced -- \
  --bind 127.0.0.1:5683 \
  --input synthetic \
  --pixel-format mono8 \
  --width 640 \
  --height 480 \
  --fps 30 \
  --mtu 1200
```

Notes:

- supports every transport pixel format already implemented in `eevideo-proto`
- produces a built-in animated test pattern

### `v4l2`

Use this on Linux for webcams, frame grabbers, and other `/dev/video*` devices.
On Jetson, use this provider for cameras or grabbers that are exposed as
`/dev/videoX`. For Jetson CSI paths you manage through `nvarguscamerasrc`, keep
using `pipeline` instead.

Example:

```sh
./eedeviced \
  --bind 0.0.0.0:5683 \
  --advertise-address 192.168.1.50 \
  --iface eth0 \
  --input v4l2 \
  --device /dev/video0 \
  --pixel-format gray16le \
  --width 640 \
  --height 480 \
  --fps 30 \
  --mtu 1200
```

Example Jetson V4L2 path:

```sh
./eedeviced \
  --bind 0.0.0.0:5683 \
  --advertise-address 192.168.1.50 \
  --iface eth0 \
  --input v4l2 \
  --device /dev/video0 \
  --pixel-format uyvy \
  --width 1280 \
  --height 720 \
  --fps 30 \
  --mtu 1200
```

Notes:

- this provider uses `v4l2src`
- it requests the configured caps directly and fails startup if the device
  cannot negotiate them
- Bayer formats use `video/x-bayer`; no color conversion is inserted
- on Jetson, list devices and modes with `v4l2-ctl --list-devices` and
  `v4l2-ctl -d /dev/video0 --list-formats-ext` before picking width, height,
  fps, and pixel format

### `pipeline`

Use this when a source does not fit the built-in providers.

Example `GRAY16_LE` path:

```sh
./eedeviced \
  --bind 0.0.0.0:5683 \
  --advertise-address 192.168.1.50 \
  --iface eth0 \
  --input pipeline \
  --pixel-format gray16le \
  --width 640 \
  --height 480 \
  --fps 30 \
  --pipeline "videotestsrc is-live=true ! video/x-raw,format=GRAY16_LE,width=640,height=480,framerate=30/1 ! appsink name=framesink sync=false max-buffers=1 drop=true"
```

Example Bayer path:

```sh
./eedeviced \
  --bind 0.0.0.0:5683 \
  --advertise-address 192.168.1.50 \
  --iface eth0 \
  --input pipeline \
  --pixel-format bayer-bg8 \
  --width 1920 \
  --height 1080 \
  --fps 30 \
  --pipeline "videotestsrc is-live=true ! video/x-bayer,format=bggr,width=1920,height=1080,framerate=30/1 ! appsink name=framesink sync=false max-buffers=1 drop=true"
```

Example Jetson CSI path for the recommended provider:

```sh
./eedeviced \
  --bind 0.0.0.0:5683 \
  --advertise-address 192.168.1.50 \
  --iface eth0 \
  --input pipeline \
  --pixel-format uyvy \
  --width 1280 \
  --height 720 \
  --fps 30 \
  --mtu 1200 \
  --pipeline "nvarguscamerasrc sensor-id=0 ! video/x-raw(memory:NVMM),format=NV12,width=1280,height=720,framerate=30/1 ! nvvidconv ! video/x-raw,format=UYVY,width=1280,height=720 ! appsink name=framesink sync=false max-buffers=1 drop=true"
```

Notes:

- the full pipeline string is user-owned
- `eedeviced` does not append a sink or rewrite the pipeline
- the pipeline must expose `appsink name=framesink`
- the negotiated appsink caps must match the configured width, height, and pixel
  format
- this is the recommended Jetson provider for `nvarguscamerasrc`-backed CSI
  cameras
- on Jetson, use `v4l2` instead when the camera or capture device appears as
  `/dev/videoX`
- Jetson Nano on JetPack 4.x should use this provider, not the built-in `argus`
  provider
- for the Nano operator flow, use
  [jetson-nano-jetpack4-first-time-setup.md](jetson-nano-jetpack4-first-time-setup.md)

### `argus`

This built-in convenience provider is available in the CLI, but it is not
currently a tested deployment path in this repo due to lack of matching
hardware coverage. Prefer `pipeline` on Jetson, even when the camera source
itself is `nvarguscamerasrc`.

Example:

```sh
./eedeviced \
  --bind 0.0.0.0:5683 \
  --advertise-address 192.168.1.50 \
  --iface eth0 \
  --input argus \
  --sensor-id 0 \
  --pixel-format uyvy \
  --width 1280 \
  --height 720 \
  --fps 30 \
  --mtu 1200
```

Notes:

- this provider is `UYVY` only in the current implementation
- it uses `nvarguscamerasrc ! nvvidconv ! appsink`
- it is not currently a validated Jetson bring-up path in this repo because we
  do not have matching hardware coverage for it here
- prefer `pipeline` so the full CSI path stays operator-owned
- Jetson Nano on JetPack 4.x should use the `pipeline` provider so the full CSI
  pipeline stays operator-owned
- for full Jetson setup, use [jetson-orin-first-time-setup.md](jetson-orin-first-time-setup.md)

## Verification Ownership

Use the setup guides for full end-to-end validation:

- [linux-device-first-time-setup.md](linux-device-first-time-setup.md)
- [jetson-orin-first-time-setup.md](jetson-orin-first-time-setup.md)
- [jetson-nano-jetpack4-first-time-setup.md](jetson-nano-jetpack4-first-time-setup.md)

In general:

- verify discovery and describe with `eevid`
- use `eeview` for `UYVY` or other directly viewable paths
- for Bayer or `Mono16` paths, validate the control plane first and then use a
  receiver that understands the configured format

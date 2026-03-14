# `eedeviced` Provider Guide

`eedeviced` is the single-stream EEVideo device daemon in this workspace.

It exposes one `CompatibilityV1` stream over the existing CoAP/register control
contract, but the frame source is now selected with `--input`.

If you need a first-time bring-up path for non-Jetson devices, use
[non-jetson-device-first-time-setup.md](non-jetson-device-first-time-setup.md).
For Jetson Nano on JetPack 4.x, use
[jetson-nano-jetpack4-first-time-setup.md](jetson-nano-jetpack4-first-time-setup.md).

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

Notes:

- this provider uses `v4l2src`
- it requests the configured caps directly and fails startup if the device
  cannot negotiate them
- Bayer formats use `video/x-bayer`; no color conversion is inserted

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

Example Jetson Nano JetPack 4.x CSI path:

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
- Jetson Nano on JetPack 4.x should use this provider, not the built-in `argus`
  provider
- for the Nano operator flow, use
  [jetson-nano-jetpack4-first-time-setup.md](jetson-nano-jetpack4-first-time-setup.md)

### `argus`

Use this on Jetson Orin running JetPack 6.x for CSI capture through
`nvarguscamerasrc`.

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
- Jetson Nano on JetPack 4.x should use the `pipeline` provider so the full CSI
  pipeline stays operator-owned
- for full Jetson setup, use [jetson-orin-first-time-setup.md](jetson-orin-first-time-setup.md)

## First Verification

Once the device is running, verify it from another machine:

```sh
cargo run -p eevid -- discover
```

Then describe it directly:

```sh
cargo run -p eevid -- --device-uri coap://192.168.1.50:5683 describe
```

For a `UYVY` path, start managed viewing:

```sh
cargo run -p eeview -- --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --port 5000
```

For non-display-oriented formats like `Mono16` or Bayer, start with `eevid`
control-plane verification first, then validate the stream with a receiver that
understands the configured format.

# Non-Jetson First-Time EEVideo Device Setup

This guide is for the first time you turn a non-Jetson machine into an EEVideo
device with `eedeviced`.

Use it when you want a concrete bring-up path for:

- `synthetic` on any host
- `v4l2` on Linux webcams and capture devices
- `pipeline` for arbitrary GStreamer-backed sources

If you need the provider matrix and CLI reference, use
[eedeviced-provider-guide.md](eedeviced-provider-guide.md). If you are bringing
up a Jetson Orin CSI camera, use
[jetson-orin-first-time-setup.md](jetson-orin-first-time-setup.md).

## What You Need

- one Linux or development host that will run `eedeviced`
- one second machine that will run `eevid` or `eeview`
- a reachable network path between them
- this repository checked out on the build machine

For hardware-backed bring-up, also have one of:

- a V4L2 device at `/dev/video0` or similar
- a source that can be expressed as a GStreamer pipeline ending in
  `appsink name=framesink`

Recommended first setup:

- start with `synthetic`
- then move to `v4l2` if you have a Linux camera or frame grabber
- keep `mtu 1200`
- use unicast only
- use one fixed width, height, and pixel format per daemon run

## What You Are Building

The non-Jetson device path in this repo is:

- `eedeviced` on the source machine
- CoAP/register discovery and control on port `5683`
- one stream named `stream0`
- `CompatibilityV1` transport
- a source selected by `--input`

The host-side tools stay the same:

- `eevid` for discovery and stream control
- `eeview` for managed live viewing when the chosen pixel format is viewable

## Step 1: Build The Artifacts

On the machine that has the repo checked out:

```sh
cargo build -p eedeviced -p eevid -p eeview
```

If the device machine is not the build machine, copy `eedeviced` to it after the
build completes.

Typical local development binary:

```text
target/debug/eedeviced
```

## Step 2: Pick The First Provider

Use this order:

1. `synthetic`
2. `v4l2`
3. `pipeline`

Why:

- `synthetic` proves discovery, control, and streaming without hardware
- `v4l2` is the lowest-friction real hardware path on Linux
- `pipeline` is the escape hatch for sources that do not fit the built-in paths

## Step 3: Start With `synthetic`

Run this on the device machine:

```sh
cargo run -p eedeviced -- \
  --bind 0.0.0.0:5683 \
  --advertise-address 192.168.1.50 \
  --iface eth0 \
  --input synthetic \
  --pixel-format mono8 \
  --width 640 \
  --height 480 \
  --fps 30 \
  --mtu 1200
```

What each flag is doing:

- `--bind`: listens for discovery and register control
- `--advertise-address`: the IP address the host should connect to
- `--iface`: the NIC used for discovery and replies
- `--input synthetic`: uses the built-in test pattern generator
- `--pixel-format mono8`: the first easy validation format
- `--width`, `--height`, `--fps`: fixed mode for this daemon run
- `--mtu`: UDP payload limit for the stream

Keep that process running while you validate from the host.

## Step 4: Verify From The Host

First check discovery:

```sh
cargo run -p eevid -- discover
```

If discovery is noisy on your network, use the direct URI:

```sh
cargo run -p eevid -- --device-uri coap://192.168.1.50:5683 describe
```

You should see:

- one device
- one stream named `stream0`
- `compatibility-v1`
- the configured width, height, and pixel format

If you want a control-only smoke first:

```sh
cargo run -p eevid -- --device-uri coap://192.168.1.50:5683 stream-start --stream-name stream0 --destination-host 192.168.1.20 --port 5000 --bind-address 192.168.1.20 --max-packet-size 1200 --width 640 --height 480 --pixel-format mono8
```

Expected result:

- `running stream-id=... active=true`

For `uyvy`, `mono8`, or another path that your receiver stack can display or
consume directly, continue with the appropriate local validation tooling. For
formats like `mono16` or Bayer, validate the control plane first, then validate
the packet stream with a receiver that understands the configured format.

## Step 5: Move To `v4l2`

Once `synthetic` works, switch to a real Linux capture device.

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

Before blaming EEVideo, confirm the device exists:

```sh
ls /dev/video*
```

What to expect:

- `eedeviced` requests the configured caps directly
- startup fails if the device cannot negotiate the chosen mode
- Bayer formats use `video/x-bayer`

Recommended first real modes:

- `mono8 640x480@30`
- `gray16le 640x480@30`
- `uyvy 1280x720@30` if the source supports it

## Step 6: Use `pipeline` For Unusual Sources

Use `pipeline` when:

- the device is not exposed cleanly through `v4l2`
- you need custom preprocessing
- you want a source other than a simple camera

Example `GRAY16_LE` source:

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

Example Bayer source:

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

Important rules:

- the pipeline must expose `appsink name=framesink`
- `eedeviced` does not append or rewrite the sink
- the negotiated caps must match the configured width, height, and pixel format
- only tightly packed sample layouts are supported in the current implementation

## Common First-Time Problems

If `eevid discover` finds nothing:

- confirm `eedeviced` is still running
- confirm `--advertise-address` matches the device machine IP
- confirm the host and device are on the same reachable network
- try `eevid --device-uri coap://DEVICE_IP:5683 describe`

If the device rejects stream settings:

- `eedeviced` intentionally keeps the setup fixed to one mode per daemon run
- use the same width, height, and pixel format on the host that the daemon was
  started with
- keep `mtu 1200` until the path is stable

If `v4l2` startup fails:

- confirm the device path is correct
- try a smaller mode first
- try `mono8` before `mono16` or Bayer
- assume the device refused the requested caps until proven otherwise

If `pipeline` startup fails:

- confirm the pipeline string includes `appsink name=framesink`
- confirm the final caps match the configured width, height, and pixel format
- strip the pipeline down until the source can run in plain `gst-launch-1.0`

If packets arrive but the receiver rejects frames:

- verify the configured pixel format matches what the source actually negotiated
- verify the source is producing tightly packed buffers
- avoid implicit format conversion unless you control the full pipeline

## After The First Successful Stream

Once the first non-Jetson setup works, the next things worth validating are:

- repeated start/stop cycles
- another pixel format on the same source
- one `v4l2` path and one `pipeline` path if you need both
- service packaging on your target machine if it will be a persistent device

# Jetson Nano JetPack 4.x First-Time EEVideo Bring-Up

This guide is for the first time you turn a Jetson Nano running JetPack 4.x
into an EEVideo device with `eedeviced`.

Use it when the Nano OS is already installed and you want the first EEVideo
bring-up on that device. This path uses `--input pipeline`, not the built-in
`argus` provider. If you are bringing up Jetson Orin on JetPack 6.x, use
[jetson-orin-first-time-setup.md](jetson-orin-first-time-setup.md). If you need
the provider matrix, use [eedeviced-provider-guide.md](eedeviced-provider-guide.md).

This guide assumes you already have:

- JetPack 4.x / L4T 32.7.x installed on the Nano
- shell access to the Nano
- a working network connection
- a CSI camera recognized by `nvarguscamerasrc`

This guide does not cover:

- flashing JetPack
- initial Linux account setup
- general Jetson networking setup
- camera-driver installation outside the standard JetPack stack

## What You Need

- a Jetson Nano running JetPack 4.x / L4T 32.7.x
- a CSI camera that works with `nvarguscamerasrc`
- a second machine that will run `eevid` and `eeview`
- a network path between the Nano and the host
- this repository checked out on the build machine

Recommended first EEVideo bring-up:

- one Jetson Nano
- one host PC
- one camera
- one Ethernet link
- unicast only
- `1280x720@30 UYVY`

## What You Are Building

The Nano device path in this repo is:

- `eedeviced` on the Nano
- CoAP/register discovery and control on port `5683`
- one stream named `stream0`
- `CompatibilityV1` transport
- `UYVY` output
- a user-owned GStreamer pipeline ending in `appsink name=framesink`

The host-side tools stay the same:

- `eevid` for discovery and stream control
- `eeview` for managed live viewing

## Step 1: Confirm The Existing Nano Setup

Confirm the board is on JetPack 4.x and that the camera stack is already alive.

On the Nano:

```sh
gst-launch-1.0 nvarguscamerasrc sensor-id=0 ! fakesink
```

If that fails, stop here and fix the Nano camera setup first. The EEVideo
pipeline depends on the same Argus camera service, and this guide assumes that
part is already working.

Decide these values before continuing:

- Nano IP address you want the host to use
- network interface name on the Nano, for example `eth0`
- camera sensor id, usually `0` for the first sensor

## Step 2: Build The EEVideo Artifacts

On the build machine, use the Jetson cross-build path with a Nano JetPack 4.x
sysroot:

```sh
cross/jetson-orin/build.sh /absolute/path/to/jetson-sysroot
```

The outputs land under:

```text
target/aarch64-unknown-linux-gnu/release/
```

For the first EEVideo bring-up, copy these to the Nano:

- `eedeviced`
- `cross/jetson-orin/systemd/eedeviced.service`
- `cross/jetson-orin/systemd/eedeviced-launch.sh`
- `cross/jetson-orin/systemd/eedeviced.env.example`

No Rust dependency downgrade is planned for Nano JetPack 4.x. The current
`gstreamer-sys`, `gstreamer-base-sys`, and `gstreamer-app-sys` crates in this
workspace still target system GStreamer `>= 1.14`, which matches the floor this
project already uses.

## Step 3: Copy EEVideo Files To The Nano

Example:

```sh
ssh nvidia@192.168.1.50 "sudo mkdir -p /opt/eevideo /etc/eevideo && sudo chown \$USER /opt/eevideo /etc/eevideo"
scp target/aarch64-unknown-linux-gnu/release/eedeviced nvidia@192.168.1.50:/opt/eevideo/
scp cross/jetson-orin/systemd/eedeviced.service nvidia@192.168.1.50:/tmp/
scp cross/jetson-orin/systemd/eedeviced-launch.sh nvidia@192.168.1.50:/tmp/
scp cross/jetson-orin/systemd/eedeviced.env.example nvidia@192.168.1.50:/tmp/
ssh nvidia@192.168.1.50 "sudo mv /tmp/eedeviced.service /etc/systemd/system/ && sudo mv /tmp/eedeviced-launch.sh /opt/eevideo/ && sudo mv /tmp/eedeviced.env.example /etc/eevideo/eedeviced.env && sudo chmod +x /opt/eevideo/eedeviced-launch.sh"
```

## Step 4: Validate The CSI Pipeline Locally

Before starting `eedeviced`, prove the Nano can negotiate the intended first
mode:

```sh
gst-launch-1.0 nvarguscamerasrc sensor-id=0 ! \
  'video/x-raw(memory:NVMM),format=NV12,width=1280,height=720,framerate=30/1' ! \
  nvvidconv ! \
  'video/x-raw,format=UYVY,width=1280,height=720' ! \
  fakesink
```

If that fails, do not debug EEVideo yet. Fix the local GStreamer path first.

## Step 5: Start The EEVideo Device Manually First

Do not start with `systemd`. Run it manually once so failures are obvious.

On the Nano:

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

What each flag is doing:

- `--bind`: listens for discovery and register control
- `--advertise-address`: the IP address the host should connect to
- `--iface`: the Nano NIC used for device discovery context
- `--input pipeline`: uses the operator-owned GStreamer path
- `--pixel-format uyvy`: must match the final appsink caps
- `--width`, `--height`, `--fps`: must match the final appsink caps
- `--mtu`: UDP payload limit for the stream
- `--pipeline`: owns the full CSI capture pipeline ending in `appsink name=framesink`

Keep that process running while you validate from the host.

## Step 6: Verify From The Host

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
- `UYVY 1280x720`
- `stream stream0: UYVY 1280x720 @ 30 fps`

## Step 7: Start A Control-Plane Smoke

Before using `eeview`, verify that the Nano accepts stream control:

```sh
cargo run -p eevid -- --device-uri coap://192.168.1.50:5683 stream-start --stream-name stream0 --destination-host 192.168.1.20 --port 5000 --bind-address 192.168.1.20 --max-packet-size 1200 --width 1280 --height 720 --pixel-format uyvy
```

Expected result:

- `running stream-id=... active=true`

## Step 8: Start Managed Viewing

Once control works, start managed viewing from the host:

```sh
cargo run -p eeview -- --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --port 5000
```

That command tells the Nano where to send the stream, then starts the local
receiver/viewer. The viewer HUD shows live FPS + stream mode by default; add
`--no-overlay` if you want the video without the overlay.

## Step 9: Install EEVideo As A Service After Manual Success

Once manual startup is stable, use the packaged service files already copied to
the Nano.

Edit `/etc/eevideo/eedeviced.env` to these first values:

```sh
EEVIDEO_BIND=0.0.0.0:5683
EEVIDEO_ADVERTISE_ADDRESS=192.168.1.50
EEVIDEO_IFACE=eth0
EEVIDEO_INPUT=pipeline
EEVIDEO_PIXEL_FORMAT=uyvy
EEVIDEO_WIDTH=1280
EEVIDEO_HEIGHT=720
EEVIDEO_FPS=30
EEVIDEO_MTU=1200
EEVIDEO_PIPELINE=nvarguscamerasrc sensor-id=0 ! video/x-raw(memory:NVMM),format=NV12,width=1280,height=720,framerate=30/1 ! nvvidconv ! video/x-raw,format=UYVY,width=1280,height=720 ! appsink name=framesink sync=false max-buffers=1 drop=true
```

Then enable the service:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now eedeviced
```

Check status:

```sh
sudo systemctl status eedeviced
journalctl -u eedeviced -f
```

The packaged launcher script passes only the provider-specific flags required by
`EEVIDEO_INPUT`, so Nano pipeline deployments do not inherit Orin-only
`--sensor-id` behavior.

## Common Bring-Up Problems

If `nvarguscamerasrc` fails immediately:

- confirm the camera works in the simple `nvarguscamerasrc ! fakesink` test
- make sure the board is actually on JetPack 4.x with the expected camera stack
- restart the Argus camera service before retrying local pipeline tests

If `nvvidconv` or the full local pipeline fails:

- keep the first mode at `1280x720@30`
- keep the source caps at `NV12`
- keep the final caps at `UYVY`
- prove the same pipeline in plain `gst-launch-1.0` before retrying `eedeviced`

If `eedeviced` exits on startup with a caps mismatch:

- compare `--pixel-format`, `--width`, `--height`, and `--fps` with the final
  `appsink` caps
- confirm the pipeline still ends with `appsink name=framesink`
- remove extra conversion elements until the final caps are stable

If `eevid discover` finds nothing:

- confirm `eedeviced` is still running
- confirm the Nano IP matches `--advertise-address`
- confirm host and Nano are on the same reachable network
- try `eevid --device-uri coap://NANO_IP:5683 describe`

If `eeview` starts but no frames arrive:

- keep `mtu` at `1200`
- use unicast first
- confirm the host `--bind-address` is the host's real NIC address, not `0.0.0.0`
- rerun the local Nano pipeline test before debugging the network path

## After The First Successful EEVideo Stream

Once the first setup works, the next things worth validating are:

- repeated start/stop cycles
- boot-time startup through `systemd`
- a second sensor id if the board has more than one camera
- the shared provider reference in [eedeviced-provider-guide.md](eedeviced-provider-guide.md)

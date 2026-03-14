# Jetson Orin First-Time EEVideo Setup

This guide is for the first time you turn a Jetson Orin into an EEVideo device
with `eedeviced`.

Use it when you want the full setup path from a fresh board to a working first
stream. If you already have binaries on the Jetson and only need the shorter
operator flow, use [jetson-orin-device-bringup.md](jetson-orin-device-bringup.md).
If you need the provider matrix for non-Jetson sources, use
[eedeviced-provider-guide.md](eedeviced-provider-guide.md).
If you are bringing up Jetson Nano on JetPack 4.x, use
[jetson-nano-jetpack4-first-time-setup.md](jetson-nano-jetpack4-first-time-setup.md).

## What You Need

- a Jetson Orin running JetPack 6.x
- a CSI camera that works with `nvarguscamerasrc`
- a second machine that will run `eevid` and `eeview`
- a network path between the Jetson and the host
- this repository checked out on the build machine

Recommended first setup:

- one Jetson
- one host PC
- one camera
- one Ethernet link
- unicast only
- `1280x720@30 UYVY`

## What You Are Building

The first device path in this repo is:

- `eedeviced` on the Jetson
- CoAP/register discovery and control on port `5683`
- one stream named `stream0`
- `CompatibilityV1` transport
- `UYVY` output

The host-side tools stay the same:

- `eevid` for discovery and stream control
- `eeview` for managed live viewing

## Step 1: Prepare The Jetson

Confirm the board is on JetPack 6.x and that the camera stack is alive.

On the Jetson:

```sh
gst-launch-1.0 nvarguscamerasrc sensor-id=0 ! fakesink
```

If that fails, stop here and fix the Jetson camera setup first. `eedeviced`
depends on the same Argus path.

Decide these values before continuing:

- Jetson IP address you want the host to use
- network interface name on the Jetson, for example `eth0`
- camera sensor id, usually `0` for the first sensor

## Step 2: Build The Artifacts

On the build machine, use the Jetson cross-build path:

```sh
cross/jetson-orin/build.sh /absolute/path/to/jetson-sysroot
```

The outputs land under:

```text
target/aarch64-unknown-linux-gnu/release/
```

For first setup, copy these to the Jetson:

- `eedeviced`
- optionally `libgsteevideo.so` if you also want the plugin on the Jetson

## Step 3: Copy Files To The Jetson

Example:

```sh
scp target/aarch64-unknown-linux-gnu/release/eedeviced nvidia@192.168.1.50:/home/nvidia/
```

For a cleaner permanent layout:

```sh
ssh nvidia@192.168.1.50 "sudo mkdir -p /opt/eevideo && sudo chown \$USER /opt/eevideo"
scp target/aarch64-unknown-linux-gnu/release/eedeviced nvidia@192.168.1.50:/opt/eevideo/
```

## Step 4: Start The Device Manually First

Do not start with `systemd`. Run it manually once so failures are obvious.

On the Jetson:

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

What each flag is doing:

- `--bind`: listens for discovery and register control
- `--advertise-address`: the IP address the host should connect to
- `--iface`: the Jetson NIC used for device discovery context
- `--input argus`: uses CSI capture through `nvarguscamerasrc`
- `--sensor-id`: selects the camera
- `--pixel-format uyvy`: the current `argus` provider only supports `UYVY`
- `--width`, `--height`, `--fps`: fixed first stream mode
- `--mtu`: UDP payload limit for the stream

Keep that process running while you validate from the host.

## Step 5: Verify From The Host

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
- `stream stream0: UYVY 1280x720 @ 30 fps`

Then start managed viewing from the host:

```sh
cargo run -p eeview -- --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --port 5000
```

That command tells the Jetson where to send the stream, then starts the local
receiver/viewer. The viewer HUD shows live FPS + stream mode by default; add
`--no-overlay` if you want the video without the overlay.

## Step 6: If You Want A Control-Only Smoke First

Before using `eeview`, you can verify that the Jetson accepts stream control:

```sh
cargo run -p eevid -- --device-uri coap://192.168.1.50:5683 stream-start --stream-name stream0 --destination-host 192.168.1.20 --port 5000 --bind-address 192.168.1.20 --max-packet-size 1200 --width 1280 --height 720 --pixel-format uyvy
```

Expected result:

- `running stream-id=... active=true`

## Step 7: Install As A Service After Manual Success

Once manual startup is stable, use the packaged service files:

- `cross/jetson-orin/systemd/eedeviced.service`
- `cross/jetson-orin/systemd/eedeviced-launch.sh`
- `cross/jetson-orin/systemd/eedeviced.env.example`

Install them on the Jetson:

```sh
sudo mkdir -p /etc/eevideo
sudo cp cross/jetson-orin/systemd/eedeviced.service /etc/systemd/system/
sudo cp cross/jetson-orin/systemd/eedeviced-launch.sh /opt/eevideo/
sudo cp cross/jetson-orin/systemd/eedeviced.env.example /etc/eevideo/eedeviced.env
sudo chmod +x /opt/eevideo/eedeviced-launch.sh
```

Edit `/etc/eevideo/eedeviced.env`, then enable the service:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now eedeviced
```

Keep `EEVIDEO_PIXEL_FORMAT=uyvy` for the current Argus path. The packaged
service now passes pixel format explicitly instead of relying on CLI defaults.

Check status:

```sh
sudo systemctl status eedeviced
journalctl -u eedeviced -f
```

## Common First-Time Problems

If `eevid discover` finds nothing:

- confirm `eedeviced` is still running
- confirm the Jetson IP matches `--advertise-address`
- confirm host and Jetson are on the same reachable network
- try `eevid --device-uri coap://JETSON_IP:5683 describe`

If `eeview` starts but no frames arrive:

- keep `mtu` at `1200`
- use unicast first
- confirm the host `--bind-address` is the host’s real NIC address, not `0.0.0.0`
- verify the Jetson camera works with `gst-launch-1.0 nvarguscamerasrc ...`

If the device rejects stream settings:

- `eedeviced` intentionally keeps the first setup fixed to one `UYVY` mode
- use `1280x720`, `30`, and `uyvy` first
- avoid format changes until the base path is stable

## After The First Successful Stream

Once the first setup works, the next things worth validating are:

- repeated start/stop cycles
- boot-time startup through `systemd`
- a higher `mtu` on a known-good LAN
- the full operator flow in [jetson-orin-device-bringup.md](jetson-orin-device-bringup.md)

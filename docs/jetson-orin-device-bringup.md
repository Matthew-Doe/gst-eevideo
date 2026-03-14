# Jetson Orin Device Bring-Up

This runbook covers the `argus` provider workflow for `eedeviced` on a Jetson
Orin target.

If you need the full first-time setup path from a fresh board, start with
[jetson-orin-first-time-setup.md](jetson-orin-first-time-setup.md).
If you need the provider matrix for non-Jetson sources, use
[eedeviced-provider-guide.md](eedeviced-provider-guide.md).
If you are bringing up Jetson Nano on JetPack 4.x, use
[jetson-nano-jetpack4-first-time-setup.md](jetson-nano-jetpack4-first-time-setup.md).

## Goal

Bring up a single-stream `CompatibilityV1` device that:

- exposes CoAP discovery and register control on the Jetson
- captures from CSI via Argus
- streams `UYVY` to an existing `eevid` or `eeview` host

## Build

On a Linux or WSL2 host with a Jetson sysroot:

```sh
cross/jetson-orin/build.sh /absolute/path/to/jetson-sysroot
```

Outputs are written under:

```text
target/aarch64-unknown-linux-gnu/release/
```

Copy at least these to the Jetson:

- `eedeviced`
- `libgsteevideo.so` if you also want the plugin on-device

## Jetson Prerequisites

- JetPack 6.x
- CSI camera available to `nvarguscamerasrc`
- GStreamer runtime with Jetson camera elements
- reachable network path between the Jetson and the host viewer/controller

Quick camera sanity check on the Jetson:

```sh
gst-launch-1.0 nvarguscamerasrc sensor-id=0 ! fakesink
```

## Start The Device

Example Jetson command:

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

Recommended first mode:

- `1280x720`
- `30 fps`
- `UYVY`
- `mtu 1200`

## Install As A Service

After manual validation, install the packaged service assets:

1. Copy `eedeviced` to `/opt/eevideo/eedeviced`.
2. Copy [cross/jetson-orin/systemd/eedeviced.service](../cross/jetson-orin/systemd/eedeviced.service)
   to `/etc/systemd/system/eedeviced.service`.
3. Copy [cross/jetson-orin/systemd/eedeviced-launch.sh](../cross/jetson-orin/systemd/eedeviced-launch.sh)
   to `/opt/eevideo/eedeviced-launch.sh` and make it executable.
4. Copy [cross/jetson-orin/systemd/eedeviced.env.example](../cross/jetson-orin/systemd/eedeviced.env.example)
   to `/etc/eevideo/eedeviced.env` and edit the network, camera, and
   `EEVIDEO_PIXEL_FORMAT` values.
5. Run:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now eedeviced
```

To inspect startup:

```sh
sudo systemctl status eedeviced
journalctl -u eedeviced -f
```

## Host Verification

From the host machine, verify control first:

```sh
cargo run -p eevid -- discover
```

Describe the device explicitly:

```sh
cargo run -p eevid -- --device-uri coap://192.168.1.50:5683 describe
```

Start a managed viewer:

```sh
cargo run -p eeview -- --device-uri coap://192.168.1.50:5683 --bind-address 192.168.1.20 --port 5000
```

The viewer HUD shows live FPS + stream mode by default; add `--no-overlay` if
you want the video without the overlay.

If you want a control-only smoke first:

```sh
cargo run -p eevid -- --device-uri coap://192.168.1.50:5683 stream-start --stream-name stream0 --destination-host 192.168.1.20 --port 5000 --bind-address 192.168.1.20 --max-packet-size 1200 --width 1280 --height 720 --pixel-format uyvy
```

## Expected Results

- `eevid discover` lists the Jetson device
- `eevid describe` reports one `stream0` register block and its advertised mode
- `eeview` triggers remote configure/start successfully
- live video arrives as `UYVY`

## Troubleshooting

- If `eevid` times out, confirm the Jetson IP/port, firewall state, and the `--advertise-address` value.
- If `nvarguscamerasrc` fails, validate camera access locally on the Jetson before debugging EEVideo control.
- If the host receives no frames after start, begin with unicast on a single NIC and keep `mtu` at `1200`.
- If `eeview` reports a format mismatch, verify the device is still configured for `1280x720 UYVY`; `eedeviced` intentionally rejects incompatible width, height, and pixel-format writes.

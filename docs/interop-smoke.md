# Upstream Interop Smoke Test

This is the manual interoperability check for the imported upstream Go stack in
`original_source_code/`.

## Goal

Confirm that the Rust `eevideosrc` receives the same stream profile currently
consumed by the Go `eeview` viewer and that the Rust `eevideosink` produces a
stream that the current Go path can display.

## Preconditions

- `goeevideo` and `eeview` are built locally from:
  - `original_source_code/goeevideo-main/goeevideo-main`
  - `original_source_code/eeview-main/eeview-main`
- The Rust plugin is built and discoverable through `GST_PLUGIN_PATH`.
- Test format is one of the v1-supported uncompressed formats, preferably
  `GRAY8` or `GRAY16_LE`.

## Rust Source Against Upstream Device Stream

1. Use the upstream tooling to configure and start a device stream, or use the
   existing `eeview viewer --noEEV` path if a raw stream is already present.
2. Run:

   ```sh
   gst-launch-1.0 eevideosrc address=0.0.0.0 port=5000 timeout-ms=2000 ! videoconvert ! autovideosink
   ```

3. Success criteria:
   - Frames render continuously.
   - `frames-received` increases.
   - `packet-anomalies` remains stable or near-zero on a clean localhost or LAN path.

## Rust Sink Against Upstream Go Viewer

1. Start the upstream viewer without issuing EEVideo control writes:

   ```sh
   eeview viewer --noEEV --destIP 127.0.0.1 --destPort 5000 --maxPacket 1200
   ```

2. In another terminal, run:

   ```sh
   gst-launch-1.0 videotestsrc ! video/x-raw,format=GRAY8,width=640,height=480 ! eevideosink host=127.0.0.1 port=5000 mtu=1200
   ```

3. Success criteria:
   - The Go viewer displays the transmitted image stream.
   - The Rust sink reports increasing `frames-sent`.
   - No packetization or caps errors are emitted.


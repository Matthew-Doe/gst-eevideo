# Jetson Demo Video Design

## Goal

Create a short product demo video for a potential robotics or computer-vision
developer.

The video's core message is:

`eevideo` provides simple discovery, control, and live viewing for a networked
video device.

## Audience

- robotics developers
- computer-vision developers
- technical evaluators who want to see value quickly

They are likely comfortable with terminals and device workflows, but they do
not want a long build or setup walkthrough in the video.

## Demo Context

- real device: existing Jetson Nano
- source should already be running before recording starts
- host machine records the terminal and viewer workflow
- target runtime: 30 to 45 seconds

The demo should present the Jetson as a live network video source that is easy
to find, inspect, and view from a workstation.

## Recommended Approach

Use a "find device, start stream, see video" narrative.

Why this approach:

- it reaches the payoff quickly
- it matches the stated value proposition
- it avoids overloading the viewer with implementation detail

## Story Arc

1. Introduce the product promise in one line.
2. Show the host discovering the Jetson on the network.
3. Show a concise description of the device and stream.
4. Launch managed live viewing.
5. Hold on the live feed long enough for the viewer to feel the payoff.

## Shot Plan

### Shot 1: Title card

Duration: 3 to 5 seconds

Suggested caption:

`EEVideo: simple discovery, control, and live viewing for a Jetson camera`

### Shot 2: Discovery

Duration: 5 to 8 seconds

Show a host terminal running:

```sh
eevid discover
```

Desired outcome:

- the Jetson device appears clearly in terminal output
- the device URI is readable enough to reuse in the next command

### Shot 3: Describe

Duration: 6 to 8 seconds

Show a host terminal running:

```sh
eevid --device-uri coap://<jetson-ip>:5683 describe
```

The visible output should make at least these points legible:

- device identity
- one available stream
- supported profile or stream mode summary

Do not linger on register listings if they make the output noisy.

### Shot 4: Live view

Duration: 12 to 15 seconds

Show the host launching:

```sh
eeview --device-uri coap://<jetson-ip>:5683 --bind-address <host-ip> --port 5000
```

Then cut to or reveal the live viewer window.

The live scene should include visible motion so the audience immediately reads
it as a real camera feed.

### Shot 5: Closing hold

Duration: 4 to 6 seconds

Hold on the live stream with a short caption such as:

`Discover. Inspect. View.`

or

`Simple host-side discovery, control, and live viewing.`

## Capture Guidelines

- keep terminal font large
- keep the desktop uncluttered
- pre-stage commands to minimize typing noise
- use a real, visually obvious camera scene
- prefer one clean terminal window plus the viewer window
- avoid showing long setup or build steps

If possible, place an object or hand movement in frame during the live-view
segment so the viewer immediately trusts that the stream is live.

## Messaging Guardrails

Emphasize:

- simple network discovery
- lightweight control and inspection
- live viewing from the host

Avoid emphasizing:

- transport internals
- packet format details
- broad production-readiness claims
- unsupported hardware generalizations
- latency or performance claims unless they are demonstrated and measured

## Success Criteria

The demo is successful if a viewer can understand, within one watch, that:

- a Jetson device is discoverable over the network
- the host can query it with simple commands
- the host can open a live view with minimal friction

## Open Preparation Items

Before recording, confirm:

- the Jetson Nano is already configured and reachable
- the camera path is stable on the device
- the host machine can discover the device reliably
- the host IP and Jetson IP are known in advance
- the live scene looks good on camera

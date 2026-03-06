# eevlark - CLI tool for controlling **Embedded Ethernet Video** devices with Starlark scripts
[![Go Reference](https://pkg.go.dev/badge/gitlab.com/eevideo/goeevideo.svg)](https://pkg.go.dev/gitlab.com/eevideo/goeevideo)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

`eevlark` lets you manage and automate **EEVideo** hardware using lightweight, safe, Python-like *Starlark* scripts. Write simple configuration files or one-liners to send commands, adjust settings, run sequences, and more — all from your terminal.

## Features
- Execute Starlark (.star) scripts to control EEVideo devices
- Inline script execution for quick commands
- Device discovery, listing, and status checks
- Subcommands for common operations (expandable)
- Safe & deterministic scripting with Starlark (subset of Python)
- Cross-platform (Linux, macOS, Windows)

## Installation
- Download the latest build files from Releases
- Via `go install` (recommended for developers):

```
go install gitlab.com/eevideo/eevlark@latest
```

## Quick Start
#### Run StarLark Tests
```
eevlark -s test.py
```
#### Read/Write Test Register
Edit file 'scripts/reg_rw.py' to use your EEV device file name in the init_device function. Then run the script to read and write the test register.<br>
```
eevlark -s reg_rw.py
```

## Requirements
Go ≥ 1.25.1+<br>
UDP access to devices (default port 5683)<br>
Devices must support EEV register protocol<br>

## Authors and Acknowledgment
Tecphos

## License
See LICENSE file.

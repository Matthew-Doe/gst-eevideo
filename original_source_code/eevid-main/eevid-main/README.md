# eevid – **Embedded Ethernet Video** device management CLI

[![Go Reference](https://pkg.go.dev/badge/gitlab.com/eevideo/goeevideo.svg)](https://pkg.go.dev/gitlab.com/eevideo/goeevideo)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://gitlab.com/eevideo/eevid/-/blob/main/LICENSE?ref_type=heads)

Command-line tool to discover **EEVideo** devices on the network and read/write their internal registers.<br>

Built with Go and the [goEEVideo](https://pkg.go.dev/?q=goEEVideo) library.

## Features
- Network discovery of EEVideo Ethernet-connected devices
- Read configuration registers
- Write configuration registers
- Simple, focused CLI interface

## Installation
#### Clone the repository and build
```
git clone https://gitlab.com/eevideo/eevid.git
cd eevid
# Build the binary
go build .
```
#### Or if you prefer installing globally:
```
go install gitlab.com/eevideo/eevid/eevid@latest
```

## Quick Start
### 1. Discover Devices
#### Example
```
.\eevid disc
```
This creates device config files located in subfolder "./deviceCfgs/(DeviceModelName)\_(UserDefinedName)\_(SerialNumber)_(Last3IP).yaml" which store device information such as capabilities, network info, and a register map.

### 2. Interface with an EEV Device
#### Examples
```
── High-level: Name based ──
Read (Model Name):
.\eevid reg -n id0_DeviceModelName
 (or i.e. Stream0 DestPort)
.\eevid reg -n stream0_DestPort

Write:
.\eevid reg -n stream0_DestPort -v 55550

Field-level Write:
.\eevid reg -n stream0_DestPort -f dPort=55550

── Low-level: Address based ──
Read (stream0_DestPort):
.\eevid reg -a 0x40224

Write:
.\eevid reg -a 0x40224 -v 55550
```

## Requirements
Go ≥ 1.25.1+ (recommended)<br>
UDP access to devices (default port 5683)<br>
Devices must support EEV discovery and register protocol<br>

## Authors and Acknowledgment
Tecphos

## License
See LICENSE file.

***

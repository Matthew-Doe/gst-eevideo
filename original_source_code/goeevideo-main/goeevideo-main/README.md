# goEEVideo – Go Client Library for Embedded Ethernet Video (EEV) Devices

[![Go Reference](https://pkg.go.dev/badge/gitlab.com/eevideo/goeevideo.svg)](https://pkg.go.dev/gitlab.com/eevideo/goeevideo)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://gitlab.com/eevideo/goeevideo/-/blob/main/LICENSE?ref_type=heads)

**goEEVideo** is a Go client library for discovering, configuring, and controlling **Embedded Ethernet Video (EEV)** devices over UDP/CoAP.

It enables:
- Network discovery of EEV-compatible hardware
- Generation of device configuration YAML files
- High-level API register read/write access by name
- Low-level API register read/write access by address
- Register field-level configuration
- String register handling

Ideal for machine vision cameras, embedded video encoders, industrial imaging devices, or any hardware implementing the EEV protocol.

## Features
- UDP CoAP multicast discovery
- CoAP client tailored for EEV register access (custom options 65301 & 65305)
- Symbolic register names from device YAML → easier & safer usage
- Individual register field writes using the `WriteRegFields` function
- Configurable UDP source port and CoAP token length
- Verbose logging support
- MIT licensed

## Installation
```
go get gitlab.com/eevideo/goeevideo@latest
```

## Quick Start
### 1. Discover Devices
#### Example
```
import (
	"fmt"
	"os"

	"gitlab.com/eevideo/goeevideo"
)

func main() {
	// "Unknown nic" scans all interfaces; otherwise use OS network interface name
	// e.g. "eth0", "Ethernet", etc.
	deviceCfg, err := eev.DiscDevices("Unknown nic", 2000) // timeout in milliseconds, 3 sec
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error EEV Discovery: %v\n", err)
		os.Exit(1)
	}
	fmt.Println("Discovery complete")
}
```
This facilitate creation of files like "./deviceCfgs/device-<ip>.yaml" which store device information such as capabilities, network info, and a register map.

### 2. Interface with an EEV Device
#### Examples
```
import (
	"fmt"
	"os"

	"gitlab.com/eevideo/goeevideo"
)

err := eev.Init(./deviceCfgs/Your_Device.yaml) // Replace with config file path/name
if err != nil {
	fmt.Fprintf(os.Stderr, "Error EEV Init: %v\n", err)
	os.Exit(1)
}

// ── High-level: Name based ──
// Read
regName := "eth0_IPAddress"  // Replace with an actual register name in config file
rdData, fields, err := eev.Device.ReadReg(regName)
if err != nil {
	fmt.Fprintf(os.Stderr, "Error EEV ReadReg: %v\n", err)
	os.Exit(1)
}
fmt.Printf("Register %16s = 0x%X\n", regName, rdData)
fmt.Println("Fields:")
for fieldName, fieldValue := range fields {
	fmt.Printf("%8s: 0x%X\n", fieldName, fieldValue)
}

// Write
regName = "stream0_DestPort"
uVal := uint32(55550)
err = eev.Device.WriteReg(regName, uVal)
if err != nil {
	fmt.Fprintf(os.Stderr, "Error EEV WriteReg: %v\n", err)
	os.Exit(1)
}
fmt.Printf("Wrote register %s to 0x%x\n", regName, uVal)

// Field-level Write
regName = "stream0_DestPort"
destPort := map[string]uint32{"dport": 55550,}
err = eev.Device.WriteRegFields(regName, destPort)
if err != nil {
	fmt.Fprintf(os.Stderr, "Error EEV WriteRegFields: %v\n", err)
	os.Exit(1)
}
fmt.Printf("Wrote register %s fields\n", regName)


// ── Low-level: Address based ──
// Read
uAddr := uint32(0x40224)
rdData, err := eev.Device.RegReadU32(uAddr)
if err != nil {
	fmt.Fprintf(os.Stderr, "Error EEV RegReadU32: %v\n", err)
	os.Exit(1)
}
fmt.Printf("Read register 0x%x returned 0x%x\n", uAddr, rdData)

// Write
uAddr = uint32(0x40224)
uVal = uint32(55550)
err = eev.Device.RegWriteU32(uAddr, uVal)
if err != nil {
	fmt.Fprintf(os.Stderr, "Error EEV RegWriteU32: %v", err)
	os.Exit(1)
}
fmt.Printf("Wrote register 0x%x to 0x%x\n", uAddr, uVal)
```
#### Global Configuration (optional)
```
eev.SetEevReqUdpPort(54321)   // Custom client UDP source port (0 = auto)
eev.SetTokenLen(4)            // CoAP token length
eev.Verbose = 1               // 0 = silent, 1–3 = increasing debug output
```
## Typical Workflow
Call eev.DiscDevices(...) → creates YAML configs<br>
Initialize: eev.Init(...) (sets up global eev.Device)<br>
Use eev.Device.ReadReg(...), eev.Device.WriteReg(...), etc.<br>
For direct/low-level needs → eev.Device.RegReadU32(...), eev.Device.RegReadU32(...), etc.

## Requirements
Go ≥ 1.18<br>
UDP access to devices (default port 5683)<br>
Devices must support EEV discovery and CoAP register protocol<br>

## Authors and Acknowledgment
Tecphos

## License
See LICENSE file.

***

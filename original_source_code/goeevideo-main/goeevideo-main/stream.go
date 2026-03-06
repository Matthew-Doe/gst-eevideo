// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package eev

import (
	"encoding/binary"
	"errors"
	"fmt"
	"net"
	"os"
	"time"
)

// Sets enable stream bit to start streaming on the provided stream number name (e.g. stream0).
// Also, configures streaming registers DestIPAddr, Delay, DestPort and MaxPacketSize.
// If the provided destPort value is 0 it assigns a random port.
func (Device *DeviceType) StreamStart(streamNum string, destIP string, destPort uint32, delay uint32, maxPkt uint32) (error)  {

	// // Negotiate system supported MaxPacketSize from provided maxPkt
	// maxPktSize, err := Device.StreamMaxPktSize(streamNum, destIP, maxPkt, delay)
	// if err != nil {
	// 	return fmt.Errorf("Error EEV StreamStart: Negotiating MaxPacketSize\n  %w", err)
	// }
	// if maxPktSize != (maxPkt) {
	// 	fmt.Printf("Warning EEV StreamStart: MaxPacketSize size negotiated to %d\n", maxPktSize)
	// }

	// Set the Packet Delay Register
	err := Device.WriteReg(streamNum + "_Delay", delay)
	if err != nil {
		return fmt.Errorf("Error EEV StreamStart: Writing Delay Reg\n  %w", err)
	}

	// Set the Packet Destination Port Register
	err = Device.WriteReg(streamNum + "_DestPort", uint32(destPort))
	if err != nil {
		return fmt.Errorf("Error EEV StreamStart: Writing DestPort Reg\n  %w", err)
	}
	if Verbose>0 {fmt.Printf("Device Stream Destination port set to = %d\n",destPort)}

	// Set the Destination IP address
	ip4Bytes := net.ParseIP(destIP).To4()
	if ip4Bytes == nil {
		return fmt.Errorf("Error EEV StreamStart: Unable to convert StrValue %s\n", destIP)
	}
	ipUint32 := binary.BigEndian.Uint32(ip4Bytes)
	err = Device.WriteReg(streamNum + "_DestIPAddr", ipUint32)
	if err != nil {
		return fmt.Errorf("Error EEV StreamStart: Writing DestIPAddr Reg\n  %w", err)
	}

	// Use the negotiated Packet Size and enable stream
	enStream := map[string]uint32{"enable": 1, "fireTestPkt": 0, "maxPkt": maxPkt} // maxPktSize
	err = Device.WriteRegFields(streamNum + "_MaxPacketSize", enStream)
	if err != nil {
		return fmt.Errorf("Error EEV StreamStart: While calling WriteRegFields\n  %w", err)
	}

	return nil
}

// Clear enable stream bit to stop streaming on the provided stream number name (e.g. stream0)
func (Device *DeviceType) StreamStop(streamNum string) error {
	err := Device.WriteRegFields(streamNum + "_MaxPacketSize", map[string]uint32{"enable": 0})
	if err != nil {
		return fmt.Errorf("Error EEV StreamStop: While calling WriteRegFields\n  %w", err)
	}

	return nil
}


func (Device *DeviceType) StreamMaxPktSize(streamNum string, destIP string, maxPkt uint32, delay uint32) (uint32, error) {

	// Setup UDP connection
	conn, err := SetupStreamListener(destIP, 0, Device.Location.IfIP)
	if err != nil {
		return 0, fmt.Errorf("Error EEV StreamMaxPktSize: Setting up UDP listener\n  %w\n", err)
	}
	defer conn.Close()
	fmt.Printf("conn Port: %d\n", uint32(conn.LocalAddr().(*net.UDPAddr).Port))

	// Buffer for reading UDP packets
	buf := make([]byte, maxPkt+256)	// Small increase to UDP receive buffer for safety

	// Get current MaxPacketSize from device
	_, pktSizeFields, err := Device.ReadReg(streamNum + "_MaxPacketSize")
	if err != nil {
		return 0, fmt.Errorf("Error EEV StreamMaxPktSize: Couldn't read MaxPacketSize register\n  %w", err)
	}
	origPktSize := pktSizeFields["maxPkt"]
	pktSizeFields["fireTestPkt"] = 1

	// Set the Packet Delay Register
	err = Device.WriteReg(streamNum + "_Delay", delay)
	if err != nil {
		return 0, fmt.Errorf("Error EEV StreamMaxPktSize: Writing Delay Reg\n  %w", err)
	}

	// Set the Packet Destination Port Register
	destPort := uint32(conn.LocalAddr().(*net.UDPAddr).Port)
	err = Device.WriteReg(streamNum + "_DestPort", uint32(conn.LocalAddr().(*net.UDPAddr).Port))
	if err != nil {
		return 0, fmt.Errorf("Error EEV StreamMaxPktSize: Writing DestPort Reg\n  %w", err)
	}
	if Verbose > 2 {
		fmt.Printf("Info EEV StreamMaxPktSize: Stream Destination port set to = %d\n",destPort)
	}

	// Set the Destination IP address
	ip4Bytes := net.ParseIP(destIP).To4()
	if ip4Bytes == nil {
		return 0, fmt.Errorf("Error EEV StreamMaxPktSize: Unable to determine IP Address\n")
	}
	ipUint32 := binary.BigEndian.Uint32(ip4Bytes)
	err = Device.WriteReg(streamNum + "_DestIPAddr", ipUint32)
	if err != nil {
		return 0, fmt.Errorf("Error EEV StreamMaxPktSize: Writing DestIPAddr Reg\n  %w", err)
	}

	// Negotiate MaxPacketSize
	var pktSizeNego uint32
	var decDone    bool

	pktSizeNego = maxPkt
	attempts := 0

	for ; pktSizeNego > 200 && attempts < 10; attempts++ {
		// Fire Test Packet
		if Verbose > 2 {
			fmt.Printf("Info EEV StreamMaxPktSize: pktSizeNego = %d\n", pktSizeNego)
		}
		pktSizeFields["maxPkt"] = pktSizeNego
		Device.WriteRegFields(streamNum + "_MaxPacketSize", pktSizeFields)

		for {
			// Set Read Deadline
			deadline := time.Now().Add(500 * time.Millisecond)
			err := conn.SetReadDeadline(deadline)
			if err != nil {
				if Verbose > 0 {
					return 0, fmt.Errorf("Warning EEV StreamMaxPktSize: SetReadDeadline failed\n  %w", err)
				}
			}

			// Read UDP packet
			packetLen, _, err := conn.ReadFromUDP(buf)
			if err != nil {
				if Verbose > 0 {
					fmt.Printf("Warning EEV StreamMaxPktSize: Reading UDP packet: %v\n", err)
				}
				if errors.Is(err, os.ErrDeadlineExceeded) {
					if !decDone {
						if (pktSizeNego > 1000) {
							pktSizeNego -= 1000
						} else {
							pktSizeNego -= 100
						}
					} else {
						// Timed out increasing, return previous value
						return pktSizeNego-100, nil
					}
					break
				}
			}

			if Verbose > 2 {
				fmt.Printf("Info EEV StreamMaxPktSize: packetLen = %d\n", packetLen)
			}

			// Validation of packet size
			if packetLen < int(pktSizeNego-42-150) {  //42 for UDP headers, -150 lower range
				if Verbose > 1 {
					fmt.Println("Warning EEV StreamMaxPktSize: Packet too small ", packetLen)
				}
				break
			} else if packetLen > int(pktSizeNego-42+150) { //42 for UDP headers, +150 upper range
				if Verbose > 1 {
					fmt.Println("Warning EEV StreamMaxPktSize: Packet too large ", packetLen)
				}
				break
			} else {
				if !decDone {
					decDone = true
				}

				if pktSizeNego >= maxPkt {
					return pktSizeNego, nil
				} else {
					pktSizeNego += 100
					break
				}
			}
		}
	}

	// Unable to negotiate a new packet size, set back to original
	fmt.Printf("Warning EEV GvspFindMaxPktSize: Unable to negotiate packet size %d. " +
	"Setting back to original device value %d\n", maxPkt, origPktSize)
	pktSizeFields["maxPkt"] = origPktSize
	pktSizeFields["fireTestPkt"] = 0
	Device.WriteRegFields(streamNum + "_MaxPacketSize", pktSizeFields)

	return 0, fmt.Errorf("Error EEV GvspFindMaxPktSize: Unable to negotiate packet size %d", maxPkt)
}

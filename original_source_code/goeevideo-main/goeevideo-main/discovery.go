// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package eev

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"net"
	"strconv"
	"sync"
	"time"

	"golang.org/x/net/ipv4"
)

// Top level function call for Discovering EEVideo Devices
func DiscDevices(discNicName string, udpTimeout int) ([]DeviceType, error) {

	responses, err := getDiscResponses(discNicName, udpTimeout)
	if err != nil {
		return []DeviceType{}, fmt.Errorf("Error EEV DiscDevices: Error whle discovering devices, %w", err)
	}

	if len(responses) == 0 || responses == nil {
		return []DeviceType{}, fmt.Errorf("Info EEV DiscDevices: No devices discovered")
	}
	deviceList := make([]DeviceType, 0, len(responses))

	for _, resp := range responses {
		cfg := DeviceType{
			Location: LocationType{
				IfName: resp.IfName,
				IfIP:   resp.IfIP,
				DevIP:  resp.DevIP},
		}

		HostUDP, err = net.ResolveUDPAddr("udp", resp.IfIP+":"+strconv.Itoa(EevReqUdpPort))
		if err != nil {
			// return deviceList, fmt.Errorf("ResolveUDPAdddr HostUDP %w", err)
			fmt.Printf("Warning EEV DiscDevices: ResolveUDPAdddr HostUDP %v\n", err)
			continue
		}

		DeviceUDP, err = net.ResolveUDPAddr("udp", resp.DevIP+":"+strconv.Itoa(EevUdpPort))
		if err != nil {
			// return deviceList, fmt.Errorf("ResolveUDPAdddr DeviceUDP %w", err)
			fmt.Printf("Warning EEV DiscDevices: ResolveUDPAdddr DeviceUDP %v\n", err)
			continue
		}

		err = createDeviceConfig(&cfg)
		if err != nil {
			fmt.Printf("Error EEV DiscDevices: Creating device config, %v\n", err)
			continue
		} else {
			deviceList = append(deviceList, cfg)
		}
	}

	if Verbose > 0 {
		fmt.Println("Info EEV DiscDevices: Available Devices:")
		for _, device := range deviceList {
			fmt.Printf("Device IP: %s:%s\n", device.Location.DevIP, strconv.Itoa(EevUdpPort))
		}
	}

	return deviceList, nil
}

// getDiscResponses sends an EEVideo CoAP Discovery message on all system
// Network Interface(s) querying rt=eev.cam and processes the eevUdpResponse(s)
func getDiscResponses(nicName string, udpTimeout int) ([]eevUdpResponse, error) {
	// Get all network interfaces
	ifaces, err := net.Interfaces()
	if err != nil {
		return nil, fmt.Errorf("Error EEV getDiscResponses: Couldn't get network interfaces\n  %w", err)
	}

	// Build eeVid/CoAP Multicast Discovery Request Packet
	var disc_msg bytes.Buffer

	hdr := []byte{0x51, 0x01} // COAP v1, Non-Confirmable; TokenLen 1
	msgID := []byte{0x20, 0x00}
	token := []byte{0x1}
	optUriPathWellKnwn := ".well-known"
	optUriPathCore := "core"
	optUriQueryRT := "rt=eev.cam"
	disc_msg.Write(hdr)
	disc_msg.Write(msgID)
	disc_msg.Write(token)
	disc_msg.Write([]byte{0xBB}) // Uri-Path Option Delta and Length
	disc_msg.WriteString(optUriPathWellKnwn)
	disc_msg.Write([]byte{0x04}) // Uri-Path Option Delta and Length
	disc_msg.WriteString(optUriPathCore)
	disc_msg.Write([]byte{0x4A}) // Uri-Query Option Delta and Length
	disc_msg.WriteString(optUriQueryRT)

	// Initialize variables for sending/receiving Discovery packets
	var wgIface sync.WaitGroup
	respCh := make(chan eevUdpResponse, 50)
	ctx, cancel := context.WithTimeout(context.Background(), time.Duration(udpTimeout)*time.Second)
	defer cancel()

	// Iterate through interfaces, find nicName if provided
	for _, iface := range ifaces {
		if nicName != "Unknown nic" && iface.Name != nicName {
			continue
		}

		// Check for proper interface flags
		if !isSuitableInterface(iface) {
			continue
		}

		// Start handler routine for each interface
		wgIface.Add(1)
		go func(iface net.Interface) {
			defer wgIface.Done()
			// err := handleInterface(iface, disc_msg.Bytes(), respCh, &wgIface, udpTimeout)
			err := handleInterface(ctx, iface, disc_msg.Bytes(), respCh)
			if err != nil {
				fmt.Printf("Warning EEV getDiscResponses: Error in handler for %s\n  %v\n", iface.Name, err)
			}
		}(iface)
	}

	// Collect responses in a separate goroutine so we can select on ctx.Done()
	responses := make([]eevUdpResponse, 0, 20)
	done := make(chan struct{})
	go func() {
		for resp := range respCh {
			responses = append(responses, resp)
		}
		close(done)
	}()

	// Wait for Interface handlers to finish
	wgIface.Wait()
	close(respCh)
	<-done

	// Process collected responses
	if len(responses) == 0 {
		return nil, fmt.Errorf("Error EEV getDiscResponses: No responses received.")
	} else if Verbose > 1 {
		fmt.Println("Info EEV getDiscResponses: Received response(s)")
	}

	return responses, nil
}

// handleInterface captures UDP discovery CoAP messages and stores
// them in a eevUdpResponse structure
func handleInterface(ctx context.Context, iface net.Interface, message []byte, respCh chan<- eevUdpResponse) error {
	// Get interface addresses
	addrs, err := iface.Addrs()
	if err != nil {
		return fmt.Errorf("Error EEV handleInterface: Getting addresses for %s\n  %w", iface.Name, err)
	}

	for _, addr := range addrs {
		ipNet, ok := addr.(*net.IPNet)
		if !ok || ipNet.IP.To4() == nil {
			if Verbose > 3 {
				fmt.Printf("Warning EEV handleInterface: Skipping non-IPv4 address %v on %s\n", addr, iface.Name)
			}
			continue
		}

		// Set up connection on this interface
		localAddr := &net.UDPAddr{
			IP:   ipNet.IP,
			Port: EevReqUdpPort,
		}

		conn, err := net.ListenUDP("udp4", localAddr)
		if err != nil {
			return fmt.Errorf("Error EEV handleInterface: Listening on local address\n  %w", err)
		}
		defer conn.Close()

		// Create a UDP connection for CoAP multicast discovery
		mcAddr, err := net.ResolveUDPAddr("udp4", "224.0.1.187:5683")
		if err != nil {
			return fmt.Errorf("Error EEV handleInterface: Error resolving multicast address\n  %w", err)
		}

		// Use ipv4.NewPacketConn for advanced control
		pc := ipv4.NewPacketConn(conn)
		if err := pc.SetMulticastInterface(&iface); err != nil {
			return fmt.Errorf("Error EEV handleInterface: Setting multicast interface\n  %w", err)
		}

		// Join the multicast group
		if err := pc.JoinGroup(&iface, mcAddr); err != nil {
			fmt.Printf("Error joining multicast group: %v\n", err)
		} else if Verbose >= 2 {
			fmt.Printf("Info EEV handleInterface: Joined multicast group %s on interface %s\n", mcAddr.String(), iface.Name)
		}

		// Set up UDP listener to read incoming packets
		readDone := make(chan struct{})
		go func() {
			defer close(readDone)
			buf := make([]byte, 2048)
			for {
				select {
				case <-ctx.Done():
					return
				default:
					// Short read deadline so we can check ctx frequently
					conn.SetReadDeadline(time.Now().Add(500 * time.Millisecond))
					n, addr, err := conn.ReadFromUDP(buf)
					if err != nil {
						if netErr, ok := err.(net.Error); ok && netErr.Timeout() {
							// Normal timeout
							if Verbose >= 2 {
								fmt.Printf("Info EEV handleInterface: Connection on %s timed out\n", iface.Name)
							}
							continue
						}
						// Handle other errors
						if errors.Is(err, net.ErrClosed) {
							// Connection was closed cleanly (e.g. we called conn.Close() on timeout)
							return
						}
						// Log unexpected read errors
						fmt.Printf("Info EEV handleInterface: Reading UDP on %s\n  %v\n", iface.Name, err)
						continue
					}

					// addr can be nil on some error conditions (though rare after err==nil)
					if addr == nil {
						fmt.Printf("Warning EEV handleInterface: Received a packet with nil remote addr on %s (n=%d)\n", iface.Name, n)
						continue
					}

					respCh <- eevUdpResponse{
						IfName: iface.Name,
						IfIP:   localAddr.IP.String(),
						DevIP:  addr.IP.String(),
						Data:   bytes.Clone(buf[:n]), // safer than append(nil, ...)
					}

					if Verbose >= 1 {
						fmt.Printf("Info EEV handleInterface: Discovery response from %s on %s\n", addr.IP.String(), iface.Name)
					}
				}
			}
		}()

		// Send Discovery message
		_, err = pc.WriteTo(message, nil, mcAddr)
		if err != nil {
			return fmt.Errorf("Error EEV handleInterface: Error writing to multicast group: %w\n", err)
		}
		if Verbose >= 2 {
			fmt.Printf("Info EEV handleInterface: Sent Discovery message to %s (%s)\n", iface.Name, mcAddr.String())
		}

		// Wait for context timeout or explicit cancel
		<-ctx.Done()
		conn.Close() // force read goroutine to exit
		<-readDone   // ensure it's done
	}

	return nil
}

// Helper function to check interface suitability
func isSuitableInterface(iface net.Interface) bool {
	// Info - golang net package interface Flags:
	// type Flags uint
	// const (
	// FlagUp           Flags = 1 << iota // interface is administratively up
	// FlagBroadcast                      // interface supports broadcast access capability
	// FlagLoopback                       // interface is a loopback interface
	// FlagPointToPoint                   // interface belongs to a point-to-point link
	// FlagMulticast                      // interface supports multicast access capability
	// FlagRunning                        // interface is in running state
	// )

	if (iface.Flags & net.FlagUp) == 0 {
		return false
	}
	if (iface.Flags & net.FlagLoopback) != 0 {
		return false
	}
	if (iface.Flags & net.FlagMulticast) == 0 {
		return false
	}
	return true
}

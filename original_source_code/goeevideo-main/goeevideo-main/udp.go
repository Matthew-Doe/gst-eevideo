// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package eev

import (
	"fmt"
	"net"
	// "syscall"

	"golang.org/x/net/ipv4"
)

// SetupStreamListener creates a UDP listener for either multicast or unicast
// and joins the multicast group if needed.
func SetupStreamListener(streamDestIP string, streamDestPort uint32, hostIP string) (*net.UDPConn, error) {
	groupIP := net.ParseIP(streamDestIP)
	if groupIP == nil {
		return nil, fmt.Errorf("Error EEV SetupStreamListener: Invalid stream destination IP: %q", streamDestIP)
	}

	localIP := net.ParseIP(hostIP)
	if localIP == nil {
		return nil, fmt.Errorf("Error EEV SetupStreamListener: Invalid local interface IP: %q", hostIP)
	}

	iface, err := findInterfaceForIP(hostIP)
	if err != nil {
		return nil, err
	}

	var conn *net.UDPConn

	if groupIP.IsMulticast() {
		conn, err = setupMulticastListener(iface, groupIP, int(streamDestPort))
		if err != nil {
			return nil, err
		}
	} else {
		conn, err = setupUnicastListener(localIP, int(streamDestPort))
		if err != nil {
			return nil, err
		}
	}

	// Increase system receive buffer (for large stream packets)
	err = conn.SetReadBuffer(8 * 1024 * 1024) // 8 MiB
	if err != nil {
		fmt.Printf("Warning EEV SetupStreamListener: SetReadBuffer failed, %v (falling back to default)\n", err)
		// continue anyway
	}

	// Verify what actually got set (use as needed, import of syscall required)
	// file, err := conn.File()
	// if err != nil {
	// 	fmt.Printf("Warning EEV SetupStreamListener: Cannot verify buffer\n  %v\n", err)
	// } else {
	// 	fd := file.Fd()
	// 	actual, err := syscall.GetsockoptInt(int(fd), syscall.SOL_SOCKET, syscall.SO_RCVBUF)
	// In Windows, build using this function instead of the one above
	// 	// actual, err :=	syscall.GetsockoptInt(syscall.Handle(fd), syscall.SOL_SOCKET, syscall.SO_RCVBUF)
	// 	file.Close() // important – releases the duplicated handle/fd
	//
	// 	if err != nil {
	// 		fmt.Printf("Warning EEV SetupStreamListener: Cannot read SO_RCVBUF\n  %v\n", err)
	// 	} else if Verbose > 0 {
	// 		fmt.Printf("Info EEV SetupStreamListener: Requested: %d bytes → Actual SO_RCVBUF: %d bytes\n",
	// 			8*1024*1024, actual)
	// 	}
	// }

	if Verbose > 0 {
		fmt.Printf("Info EEV SetupStreamListener: UDP listener ready on %s:%d (interface: %s)\n",
			groupIP.String(), conn.LocalAddr().(*net.UDPAddr).Port, iface.Name)
	}

	return conn, nil
}

// findInterfaceForIP returns the network interface that has the provided IP address
func findInterfaceForIP(targetIPStr string) (*net.Interface, error) {
	targetIP := net.ParseIP(targetIPStr)
	if targetIP == nil {
		return nil, fmt.Errorf("Error EEV findInterfaceForIP: Invalid IP address: %q", targetIPStr)
	}

	ifaces, err := net.Interfaces()
	if err != nil {
		return nil, fmt.Errorf("Error EEV findInterfaceForIP: Cannot list network interfaces\n  %w", err)
	}

	for _, iface := range ifaces {
		if iface.Flags&net.FlagUp == 0 {
			continue
		}
		addrs, err := iface.Addrs()
		if err != nil {
			continue
		}
		for _, addr := range addrs {
			var ip net.IP
			switch v := addr.(type) {
				case *net.IPNet:
					ip = v.IP
				case *net.IPAddr:
					ip = v.IP
			}
			if ip != nil && ip.Equal(targetIP) {
				return &iface, nil
			}
		}
	}

	return nil, fmt.Errorf("Error EEV findInterfaceForIP: No network interface found with IP %s", targetIPStr)
}

// setupMulticastListener returns connection paramters for using a multicast UDP configuration
func setupMulticastListener(iface *net.Interface, groupIP net.IP, desiredPort int) (*net.UDPConn, error) {
	if iface.Flags&net.FlagMulticast == 0 {
		return nil, fmt.Errorf("Error EEV setupMulticastListener: Interface %s does not support multicast", iface.Name)
	}

	listenAddr := &net.UDPAddr{IP: groupIP, Port: desiredPort}
	conn, err := net.ListenUDP("udp4", listenAddr)
	if err != nil {
		return nil, fmt.Errorf("Error EEV setupMulticastListener: Cannot listen on multicast address %s:%d\n  %w", groupIP, desiredPort, err)
	}

	// Join multicast group
	pktConn := ipv4.NewPacketConn(conn)
	if err := pktConn.JoinGroup(iface, &net.UDPAddr{IP: groupIP}); err != nil {
		conn.Close()
		return nil, fmt.Errorf("Error EEV setupMulticastListener: Cannot join multicast group %s on %s\n  %w", groupIP, iface.Name, err)
	}

	if err := pktConn.SetMulticastInterface(iface); err != nil {
		conn.Close()
		return nil, fmt.Errorf("Error EEV setupMulticastListener: Cannot set multicast interface %s\n  %w", iface.Name, err)
	}

	return conn, nil
}

// setupUnicastListener returns connection paramters for using a unicast UDP configuration
func setupUnicastListener(localIP net.IP, desiredPort int) (*net.UDPConn, error) {
	listenAddr := &net.UDPAddr{IP: localIP, Port: desiredPort}
	conn, err := net.ListenUDP("udp4", listenAddr)
	if err != nil {
		return nil, fmt.Errorf("Error EEV setupUnicastListener: Cannot listen on %s:%d\n  %w", localIP, desiredPort, err)
	}

	return conn, nil
}

// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package eev

import (
	"net"
	"sync"
)

// ImageBuffer holds the frame data and info for assembling an image
type ImageBuffer struct {
	Data            []byte
	BlockID         uint32
	PacketID        uint32
	PayloadType     uint16
	Width           uint32
	Height          uint32
	PixelFormat     uint32
	Offset          uint32
}

// RawImgCaptureResult bundles WaitGroup and error channel
type RawImgCaptureResult struct {
	Wg    *sync.WaitGroup
	ErrCh chan error
}

// UDPListenerCfg contains the returned items from the setupUDPListener function
type UDPListenerCfg struct {
	Conn        *net.UDPConn
	LocalPort   int  // The actual port we ended up listening on
	Interface   *net.Interface
	IsMulticast bool
}

// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package eev

import (
	"bytes"
	"context"
	"encoding/binary"
	"fmt"
	"net"
	"time"
)

// Packet Types as defined in GVSP Protocol
const (
	PacketTypeLeader  = 0x1
	PacketTypePayload = 0x3
	PacketTypeTrailer = 0x2
)

// Payload Types
const (
	GvspImage = 1
	GvspJPEG  = 6
)

// GvspFrame collects one complete frame and sends it to dataCh when complete.
// Returns nil on success (frame sent), or error on fatal issues.
// Non-fatal packet errors are logged and continues.
func GvspFrame(ctx context.Context, conn *net.UDPConn, maxPkt int,
							 timeout time.Duration, dataCh  chan<- *ImageBuffer) error {

	// if (Verbose>0) {fmt.Printf("MaxPacket = %d\n",maxPkt)}
	buf := make([]byte, maxPkt*2)
	imageBuffers := make(map[uint32]*ImageBuffer)

	for {
		// Set Read Deadline
		deadline := time.Now().Add(timeout)
		err := conn.SetReadDeadline(deadline)
		if err != nil && Verbose>0 {
			fmt.Printf("Warning EEV GvspFrame: SetReadDeadline failed: %v\n", err)
			continue
		}

		// Quick ctx check before blocking read
		if ctx.Err() != nil {
			return ctx.Err()
		}

		// Read UDP packet
		packetLen, _, err := conn.ReadFromUDP(buf)
		if err != nil {
			if netErr, ok := err.(net.Error); ok && netErr.Timeout() {
				continue // expected idle timeout
			}
			return fmt.Errorf("Error EEV GvspFrame: ReadFromUDP failed: %v/n", err)
		}

		// Early validation of packet size and header
		if packetLen < 28 {
			if Verbose > 1 {
				fmt.Println("Warning EEV GvspFrame: Packet too small ", packetLen)
			}
			continue
		}

		// Parse GVSP header
		packetFormat := buf[4] & 0xF
		if packetFormat != PacketTypeLeader &&
			packetFormat != PacketTypePayload &&
			packetFormat != PacketTypeTrailer {
			if Verbose > 1 {
				fmt.Printf("Warning EEV GvspFrame: Unknown packet format: %d\n", packetFormat)
			}
			continue
		}
		blockID := binary.BigEndian.Uint32(buf[12:16])
		packetID := binary.BigEndian.Uint32(buf[16:20])

		// Cache ImageBuffer
		imgBuf, imgExists := imageBuffers[blockID]

		// Handle different GVSP packet types
		switch packetFormat {
		case PacketTypeLeader:
			if imgExists {
				if Verbose > 0 {
					fmt.Printf("Warning EEV GvspFrame: Duplicate leader for BlockID=%d\n", blockID)
				}
				continue
			}

			payloadType := binary.BigEndian.Uint16(buf[22:24])
			width := binary.BigEndian.Uint32(buf[36:40])
			height := binary.BigEndian.Uint32(buf[40:44])
			var payloadSize uint32
			var pixelFormat uint32

			switch payloadType {
			case GvspJPEG:
				pixelFormat = 0
				payloadSize = (width*height)/2
			default:
				pixelFormat = binary.BigEndian.Uint32(buf[32:36])
				// Preallocate the exact buffer size based on PixelFormat
				switch pixelFormat {
				case MONO8, GR8, RG8, GB8, BG8:
					payloadSize = width*height
				case MONO16, YUV422_8_UYVY:
					payloadSize = width*height*2
				case RGB8:
					payloadSize = width*height*3
				default:
					if Verbose > 0 {
						fmt.Printf("Warning EEV GvspFrame: Unsupported pixel format %d\n", pixelFormat)
					}
					continue
				}
			}

			// Create new ImageBuffer
			imgBuf = &ImageBuffer{
				Data:            make([]byte, payloadSize),
				BlockID:         blockID,
				PacketID:        0,
				PayloadType:     payloadType,
				Width:           width,
				Height:          height,
				PixelFormat:     pixelFormat,
				Offset:          0,
			}
			imageBuffers[blockID] = imgBuf

		case PacketTypePayload:
			if !imgExists {
				// if Verbose > 2 {
				// 	fmt.Printf("Warning EEV GvspFrame: Received Payload packet for frame %d before leader\n", blockID)
				// }
				continue
			}

			if imgBuf.PacketID == packetID {
				continue // duplicate
			}

			if imgBuf.PacketID != packetID-1 {
				if Verbose > 0 {
					fmt.Printf("Warning EEV GvspFrame: Discarding frame BlockID %d. Detected missing Payload.\n", blockID)
				}
				delete(imageBuffers, blockID)
				continue
			}

			// Check for buffer overflow from payload data and set nextOffset
			payloadLen := uint32(packetLen) - 20  // -20 for GVSP Payload packet info fields
			nextOffset := imgBuf.Offset + payloadLen
			if nextOffset > uint32(len(imgBuf.Data)) {
				if Verbose > 0 {
					fmt.Printf("Warning EEV GvspFrame: Payload overflow BlockID=%d, PacketID=%d\n",
						blockID, packetID)
				}
				delete(imageBuffers, blockID)
				continue
			}
			// Copy only frame data to buffer
			copy(imgBuf.Data[imgBuf.Offset:nextOffset], buf[20:packetLen])
			imgBuf.Offset = nextOffset
			imgBuf.PacketID++

		case PacketTypeTrailer:
			if !imgExists {
				// if Verbose > 2 {
				// 	fmt.Printf("Warning EEV GvspFrame: Received Trailer packet for frame %d before leader\n", blockID)
				// }
				continue
			}

			// Resize data buffer to received byte size
			// imgBuf.Data = imgBuf.Data[:imgBuf.Offset]

			if imgBuf.PayloadType == GvspJPEG {
				pos := bytes.LastIndex(imgBuf.Data, []byte{0xFF, 0xD9})
				if pos == -1 {
					if Verbose > 0 {
						fmt.Printf("Warning EEV GvspFrame: Discarding frame BlockID %d, JPEG EOI not found.\n", blockID)
					}
					delete(imageBuffers, blockID)
					continue
				} else {
					imgBuf.Data = imgBuf.Data[:pos]
				}
			}

			// Send completed ImageBuffer to the channel
			select {
			case <-ctx.Done():
				return ctx.Err()
			case dataCh <- imgBuf:
				delete(imageBuffers, blockID)
				return nil
			}
		}
	}
}

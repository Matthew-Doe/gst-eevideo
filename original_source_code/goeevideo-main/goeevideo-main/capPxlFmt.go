// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package eev

import (
	"fmt"
)

// Supported Pixel Formats
const (
	MONO8  = 0x01080001
	MONO16 = 0x01100007
	GR8    = 0x01080008
	RG8    = 0x01080009
	GB8    = 0x0108000A
	BG8    = 0x0108000B
	RGB8   = 0x02180014
	YUV422_8_UYVY = 0x0210001F
)

// pixelFormatMap maps known string names (from device registers) to gvsp format constants
var pixelFormatMap = map[string]uint32{
	"MONO8":  MONO8,
	"MONO16": MONO16,
	"GR8":    GR8,
	"RG8":    RG8,
	"GB8":    GB8,
	"BG8":    BG8,
	"RGB8":   RGB8,
	"YUV422_8_UYVY": YUV422_8_UYVY,
}

// PixelFormatFromString converts a pixel format string to the corresponding pixel format constant.
// If the string is empty or unsupported, return an error.
func PixelFormatFromString(pixFmt string) (uint32, error) {
	if pixFmt == "" {
		return 0, fmt.Errorf("Pixel Format string is empty")
	}

	pixFmtVal, ok := pixelFormatMap[pixFmt]
	if !ok {
		return 0, fmt.Errorf("Unsupported Pixel Format: %q\n", pixFmt)
	}

	return pixFmtVal, nil
}

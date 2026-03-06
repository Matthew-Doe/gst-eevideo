// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package eev

import (
	"encoding/binary"
	"fmt"
)

const (
	OptionEevRegAccess     uint16 = 65301
	OptionEevBinaryAddress uint16 = 65305
)

// CoAPMessage represents a parsed CoAP message
type CoAPMessage struct {
	Version   uint8
	Type      uint8
	TokenLen  uint8
	Code      uint8
	MessageID uint16
	Token     []byte
	Options   []CoAPOption
	Payload   []byte
}

// CoAPOption represents a single CoAP option
type CoAPOption struct {
	Number uint16
	Length uint16
	Value  []byte
}

// CoapRespCode represents the value(s) in one CoapRespCode key
type CoapRespCode struct {
	Desc string
}

// CoAP Message Response Codes (combined Class and Detail fields, c.dd)
var CoapRespCodes = map[uint8]CoapRespCode{
	64: {"2.00 Success"},
	// 65 : {"2.01 Created"},
	// 66 : {"2.02 Deleted"},
	// 67 : {"2.03 Valid"},
	68:  {"2.04 Changed"},
	69:  {"2.05 Content"},
	128: {"4.00 Bad Request"},
	129: {"4.01 Unauthorized"},
	130: {"4.02 Bad Option"},
	131: {"4.03 Forbidden"},
	132: {"4.04 Not Found"},
	// 133 : {"4.05 Method Not Allowed"},
	// 134 : {"4.06 Not Acceptable"},
	// 140 : {"4.12 Precondition Failed"},
	// 141 : {"4.13 Reqest Entity Too Large"},
	// 143 : {"4.15 Unsupported Context Format"},
	// 157 : {"4.29 Too Many Requests"},
	160: {"5.00 Internal Server Error"},
	161: {"5.01 Not Implemented"},
	// 162 : {"5.02 Bad Gateway"},
	// 163 : {"5.03 Service Unavailable"},
	// 164 : {"5.04 Gateway Timeout"},
	// 165 : {"5.05 Proxying Not Supported"},
}

// buildCoAPMessage constructs a CoAP message byte slice
func buildCoAPMessage(
	msgType uint8,
	code uint8,
	msgID uint16,
	token []byte,
	options []CoAPOption,
	payload []byte,
) ([]byte, error) {
	if len(token) > 8 {
		return nil, fmt.Errorf("Error EEV buildCoAPMessage: Token length must not exceed 8 bytes")
	}

	buf := make([]byte, 0, 128)

	// Header byte 0: Version | Type | Token Length
	firstByte := (1 << 6) | (msgType << 4) | uint8(len(token))
	buf = append(buf, firstByte)

	// Code
	buf = append(buf, code)

	// Message ID
	buf = append(buf, byte(msgID>>8), byte(msgID))

	// Token
	if len(token) > 0 {
		buf = append(buf, token...)
	}

	// Options
	prevNumber := int32(0)
	for _, opt := range options {
		delta := int32(opt.Number) - prevNumber
		if delta < 0 {
			return nil, fmt.Errorf("Error EEV buildCoAPMessage: Options must be in ascending order")
		}

		var deltaBytes []byte
		switch {
		case delta < 13:
			deltaBytes = []byte{byte(delta << 4)}
		case delta < 269:
			d := delta - 13
			deltaBytes = []byte{13 << 4, byte(d)}
		default:
			d := delta - 269
			deltaBytes = []byte{14 << 4, byte(d >> 8), byte(d)}
		}

		l := len(opt.Value)
		var lenBytes []byte
		switch {
		case l < 13:
			deltaBytes[0] |= byte(l)
		case l < 269:
			lenBytes = []byte{13, byte(l - 13)}
		default:
			lenBytes = []byte{14, byte((l - 269) >> 8), byte(l - 269)}
		}

		buf = append(buf, deltaBytes...)
		if lenBytes != nil {
			buf = append(buf, lenBytes...)
		}
		buf = append(buf, opt.Value...)

		prevNumber = int32(opt.Number)
	}

	// Payload marker + payload
	if len(payload) > 0 {
		buf = append(buf, 0xFF)
		buf = append(buf, payload...)
	}

	return buf, nil
}

// parseCoAPMessage parses a raw CoAP message from bytes
func parseCoAPMessage(rdUdpChan <-chan []byte) (*CoAPMessage, error) {
	data := <-rdUdpChan
	if data == nil {
		return nil, fmt.Errorf("Error EEV parseCoAPMessage: No response received")
	}
	if len(data) < 4 {
		return nil, fmt.Errorf("Error EEV parseCoAPMessage: Response is too short")
	}

	msg := &CoAPMessage{}

	first := data[0]
	msg.Version = first >> 6
	msg.Type = (first >> 4) & 0x03
	msg.TokenLen = first & 0x0F

	if msg.Version != 1 {
		return nil, fmt.Errorf("Error EEV parseCoAPMessage: Not CoAP version 1")
	}

	msg.Code = data[1]
	msg.MessageID = binary.BigEndian.Uint16(data[2:4])

	pos := 4
	if msg.TokenLen > 0 {
		end := pos + int(msg.TokenLen)
		if end > len(data) {
			return nil, fmt.Errorf("Error EEV parseCoAPMessage: Truncated token")
		}
		msg.Token = data[pos:end]
		pos = end
	}

	// Parse options
	var options []CoAPOption
	deltaAcc := uint16(0)

	for pos < len(data) && data[pos] != 0xFF {
		if pos >= len(data) {
			return nil, fmt.Errorf("Error EEV parseCoAPMessage: Truncated option header")
		}
		header := data[pos]
		pos++

		delta := uint16(header >> 4)
		length := uint16(header & 0x0F)

		// Extended delta
		switch(delta) {
		case 13:
			if pos >= len(data) {
				return nil, fmt.Errorf("Error EEV parseCoAPMessage: Truncated extended delta")
			}
			delta = 13 + uint16(data[pos])
			pos++
		case 14:
			if pos+1 >= len(data) {
				return nil, fmt.Errorf("Error EEV parseCoAPMessage: Truncated 2-byte extended delta")
			}
			delta = 269 + binary.BigEndian.Uint16(data[pos:pos+2])
			pos += 2
		case 15:
			return nil, fmt.Errorf("Error EEV parseCoAPMessage: Invalid delta value 15")
		}

		// Extended length
		switch(length) {
		case 13:
			if pos >= len(data) {
				return nil, fmt.Errorf("Error EEV parseCoAPMessage: Truncated extended length")
			}
			length = 13 + uint16(data[pos])
			pos++
		case 14:
			if pos+1 >= len(data) {
				return nil, fmt.Errorf("Error EEV parseCoAPMessage: Truncated 2-byte extended length")
			}
			length = 269 + binary.BigEndian.Uint16(data[pos:pos+2])
			pos += 2
		case 15:
			return nil, fmt.Errorf("Error EEV parseCoAPMessage: Invalid length value 15")
		}

		end := pos + int(length)
		if end > len(data) {
			return nil, fmt.Errorf("Error EEV parseCoAPMessage: Option value truncated")
		}

		optValue := data[pos:end]
		pos = end

		deltaAcc += delta
		options = append(options, CoAPOption{
			Number: deltaAcc,
			Length: length,
			Value:  optValue,
		})
	}

	msg.Options = options

	// Payload
	if pos < len(data) && data[pos] == 0xFF {
		pos++
		if pos < len(data) {
			msg.Payload = data[pos:]
		}
	}

	return msg, nil
}

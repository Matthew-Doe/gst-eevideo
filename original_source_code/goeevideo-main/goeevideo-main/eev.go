// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package eev

import (
	"bytes"
	"crypto/rand"
	"encoding/binary"
	"fmt"
	"net"
	"strconv"
	"sync"
	"time"
)

var (
	Device        *DeviceType
	EevTokenLen   uint8
	EevReqUdpPort int
	Verbose       int
	HostUDP       *net.UDPAddr
	DeviceUDP     *net.UDPAddr
)

func init() {
	EevTokenLen = 1
	EevReqUdpPort = 0
	Verbose = 0
}

const (
	EevUdpPort int = 5683
	EevTimeOut     = 1000 * time.Millisecond
)

type GetAccType uint8

const (
	GetRegAccTypeReg     GetAccType = 0
	GetRegAccTypeFIFO    GetAccType = 1
	GetRegAccTypeRegIncr GetAccType = 4
	GetRegAccTypeString  GetAccType = 5
)

type PutAccType uint8

const (
	PutRegAccTypeWr         PutAccType = 0
	PutRegAccTypeSet        PutAccType = 1
	PutRegAccTypeClear      PutAccType = 2
	PutRegAccTypeToggle     PutAccType = 3
	PutRegAccTypeWrIncr     PutAccType = 5
	PutRegAccTypeMaskWrIncr PutAccType = 6
	PutRegAccTypeRstAdvIncr PutAccType = 7
)

func (accType GetAccType) IsValid() bool {
	switch accType {
	case GetRegAccTypeReg, GetRegAccTypeFIFO, GetRegAccTypeRegIncr, GetRegAccTypeString:
		return true
	}

	return false
}

func (accType PutAccType) IsValid() bool {
	switch accType {
	case PutRegAccTypeWr, PutRegAccTypeSet, PutRegAccTypeClear, PutRegAccTypeToggle,
		PutRegAccTypeWrIncr, PutRegAccTypeMaskWrIncr, PutRegAccTypeRstAdvIncr:
		return true
	}

	return false
}

// Init initailizes an EEV DeviceType structure
func Init(configFile string) error {
	device, err := ReadDeviceCfgYAML(configFile)
	if err != nil {
		return fmt.Errorf("Error EEV Init: Error opening file %s\n  %w", configFile, err)
	}

	HostUDP, err = net.ResolveUDPAddr("udp", device.Location.IfIP+":"+strconv.Itoa(EevReqUdpPort))
	if err != nil {
		return fmt.Errorf("Error EEV Init: ResolveUDPAdddr HostUDP\n  %w", err)
	}

	DeviceUDP, err = net.ResolveUDPAddr("udp", device.Location.DevIP+":"+strconv.Itoa(EevUdpPort))
	if err != nil {
		return fmt.Errorf("Error EEV Init: ResolveUDPAdddr DeviceUDP\n  %w", err)
	}

	Device = device

	return err
}

// Sets the client side UDP port used for EEVideo commands (Default is 0 for random OS assigned Source Port).
// Typically set to a value in the Dynamic/Private Ports range (49152 - 65535)
func SetEevReqUdpPort(port uint16) {
	EevReqUdpPort = int(port)
}

// Sets the Token Length for EEVideo commands. Default is 1.
func SetTokenLen(tokenLen uint8) error {
	if tokenLen > 8 {
		return fmt.Errorf("Error EEV SetTokenLen:Token length must be a value of 0-8")
	} else {
		EevTokenLen = uint8(tokenLen)
	}

	return nil
}

// ReadReg uses a register name string to read a register containing a uint32
// value. It returns the uint32 value and a map of field and value pairs.
func (Device *DeviceType) ReadReg(regName string) (uint32, map[string]uint32, error) {
	devReg, ok := Device.Registers[regName]
	if !ok {
		return 0, nil, fmt.Errorf("Error EEV ReadReg: Unknown Register %s", regName)
	}

	if devReg.Access == "string" {
		return 0, nil, fmt.Errorf("Error EEV ReadReg: Register %s doesn't contain an integer", regName)
	}

	if devReg.Access == "wo" {
		return 0, nil, fmt.Errorf("Error EEV ReadReg: Register %s is write-only", regName)
	}

	var regVal uint32
	var err error
	// Perform a read, else use already read value
	if devReg.Addr >= 0x40000 {
		regVal, err = Device.RegReadU32(devReg.Addr)
		if err != nil {
			return 0, nil, fmt.Errorf("Error EEV ReadReg: While calling RegReadU32\n  %w", err)
		}
	} else {
		regVal = devReg.IntValue
	}

	fields := map[string]uint32{}
	if Verbose >= 3 {
		fmt.Printf("Info EEV ReadReg: Register %s = 0x%X\n  Fields:\n", regName, regVal)
	}
	for fieldName, field := range devReg.Fields {
		fieldVal := (regVal >> (field.Msb + 1 - field.Len)) & (uint32((1 << field.Len) - 1))
		fields[fieldName] = fieldVal
		if Verbose >= 3 {
			fmt.Printf("%12s (msb: %2d, len: %2d) = 0x%X\n", fieldName, field.Msb, field.Len, fieldVal)
		}
	}

	return regVal, fields, nil
}

// ReadRegString is used to read a register containing a string (using the
// provided register name string for address lookup in the Device).
func (Device *DeviceType) ReadRegString(regName string) (string, error) {
	devReg, ok := Device.Registers[regName]
	if !ok {
		return "", fmt.Errorf("Error EEV ReadStringReg: Unknown Register %s", regName)
	}

	if devReg.Access == "string" {
		return devReg.StrValue, nil
	}

	return "", fmt.Errorf("Error EEV ReadStringReg: Register %s doesn't contain a string value", regName)
}

// WriteReg is used to write a uint32 value to a register using the
// provided register name string for address lookup in the Device.
func (Device *DeviceType) WriteReg(regName string, uVal uint32) error {
	devReg, ok := Device.Registers[regName]
	if !ok {
		return fmt.Errorf("Error EEV WriteReg: Unknown Register %s", regName)
	}

	if devReg.Access != "rw" && devReg.Access != "wo" && devReg.Access != "rowc" {
		return fmt.Errorf("Error EEV WriteReg: Register %s isn't writable", regName)
	}

	// Immuatable register space check
	if devReg.Addr < 0x40000 {
		return fmt.Errorf("Error EEV WriteReg: Immutable register address")
	}

	err := Device.RegWriteU32(devReg.Addr, uVal)
	if err != nil {
		return fmt.Errorf("Error EEV WriteReg: While calling RegWriteU32\n  %w", err)
	}

	return nil
}

// WriteRegFields uses a register name string and writes the provide value(s) to the field(s)
func (Device *DeviceType) WriteRegFields(regName string, fVals map[string]uint32) error {
	if Verbose >= 3 {
		fmt.Printf("Info EEV WriteRegFields: Register %s\n   Fields: %v\n", regName, fVals)
	}
	devReg, ok := Device.Registers[regName]
	if !ok {
		return fmt.Errorf("Error EEV WriteRegFields: Unknown Register %s", regName)
	}

	if devReg.Access != "rw" && devReg.Access != "wo" && devReg.Access != "rowc" {
		return fmt.Errorf("Error EEV WriteRegFields: Register %s isn't writable", regName)
	}

	if len(fVals) == 0 || fVals == nil {
		return fmt.Errorf("Error EEV WriteRegFields: Empty Field/Value map")
	}

	// Immuatable register space check
	if devReg.Addr < 0x40000 {
		return fmt.Errorf("Error EEV WriteRegFields: Immutable register address")
	}
	if Verbose >= 3 {
		fmt.Printf("Info EEV WriteRegFields: Passed the WriteRegFields checks\n")
	}
	// Create mask and write value for provided fields
	var writeValue uint32
	var maskValue uint32
	if Verbose >= 3 {
		fmt.Printf("Info EEV WriteRegFields: Processing fvals %v\n",fVals)
	}
	for fieldName, fieldVal := range fVals {
		field, fieldOK := devReg.Fields[fieldName]
		if !fieldOK {
			return fmt.Errorf("Error EEV WriteRegFields: Field %s not found", fieldName)
		} else {
			writeValue |= (fieldVal & ((1 << field.Len) - 1)) << (field.Msb + 1 - field.Len) // clip fieldVal to len bits and shift
			maskValue  |= ((1<<field.Len)-1) << (field.Msb + 1 - field.Len)
			if Verbose >= 4 {
				fmt.Printf("%8s (msb:%2d, len:%2d) = 0x%X\n", fieldName, field.Msb, field.Len, fieldVal)
				fmt.Printf("Info EEV WriteRegFields: writeValue = 0x%X, maskValue = 0x%X\n", writeValue, maskValue)
			}
		}
	}

	// Perform a read and modify (if necessary), write
	if len(Device.Registers[regName].Fields) != len(fVals) {
		if Verbose >= 3 {
			fmt.Println("Info EEV WriteRegFields: Performing Read-Modify-Write")
		}
		regVal, err := Device.RegReadU32(devReg.Addr)
		if err != nil {
			return fmt.Errorf("Error EEV WriteRegFields: Reading register for Modify\n  %w", err)
		}
		writeValue = (regVal & ^maskValue) | (writeValue & maskValue)
	}

	if Verbose >= 1 {
		fmt.Printf("Info EEV WriteRegFields: Writing %s 0x%X to 0x%X(mask 0x%X)\n",regName, devReg.Addr, writeValue, maskValue)
	}
	err := Device.RegWriteU32(devReg.Addr, writeValue)
	if err != nil {
		return fmt.Errorf("Error EEV WriteRegFields: While calling RegWriteU32\n  %w", err)
	}

	return nil
}

// RegReadU32 is used to read a register containing a uint32 value
func (Device *DeviceType) RegReadU32(uAddr uint32) (uint32, error) {
	regAccOpt := EevRegAccOpt{Insert: false, Count: 1, Type: 0}
	payload, err := Device.eevRegCmd(uAddr, nil, regAccOpt)
	if err != nil {
		return 0, fmt.Errorf("Error EEV RegReadU32: While calling eevRegCmd\n  %w", err)
	}

	if len(payload) < 4 {
		return 0, fmt.Errorf("Error EEV RegReadU32: uint32 requires 4 bytes, got %d", len(payload))
	}
	value := payload[0:4]

	return binary.BigEndian.Uint32(value), nil
}

// RegWriteU32 writes a uint32 value to a register
func (Device *DeviceType) RegWriteU32(uAddr uint32, uVal uint32) error {
	regAccOpt := EevRegAccOpt{Insert: false, Count: 1, Type: 0}
	payload := make([]byte, 4)
	binary.BigEndian.PutUint32(payload, uVal)

	_, err := Device.eevRegCmd(uAddr, payload, regAccOpt)
	if err != nil {
		return fmt.Errorf("Error EEV RegWriteU32: While calling eevRegCmd\n  %w", err)
	}

	return nil
}

// RegString is used to read a register that contains a string
func (Device *DeviceType) RegReadString(uAddr uint32) (string, error) {
	regAccOpt := EevRegAccOpt{Insert: true, Count: 1, Type: 5}
	payload, err := Device.eevRegCmd(uAddr, nil, regAccOpt)
	if err != nil {
		return "<empty string>", fmt.Errorf("Error EEV RegReadString: While calling eevRegCmd\n  %w", err)
	}

	s := string(bytes.TrimRight(payload, "\x00"))
	if s == "" {
		return "<empty string>", nil
	}

	return s, nil
}

// RegReadRegAcc reads a register address using an EevRegAccOpt
func (Device *DeviceType) RegReadRegAcc(uAddr uint32, regAccOpt EevRegAccOpt) ([]byte, error) {

	// Check Access Type Option Fields
	if regAccOpt.Count > 31 {
		return []byte{}, fmt.Errorf("Error EEV RegReadRegAcc: Count must be a value of 0-31")
	}
	if !GetAccType(regAccOpt.Type).IsValid() {
		return []byte{}, fmt.Errorf("Error EEV RegReadRegAcc: Undefined EevRegAccOpt Type")
	}

	payload, err := Device.eevRegCmd(uAddr, nil, regAccOpt)
	if err != nil {
		return []byte{}, fmt.Errorf("Error EEV RegReadRegAcc: While calling eevRegCmd\n  %w", err)
	}

	return payload, nil
}

func (Device *DeviceType) readDeviceRegs(addr uint32, count uint32) ([]uint32, error) {
	wordArray := make([]uint32, int(count))

	for i := range count {
		val, err := Device.RegReadU32(addr + (i << 2))
		if err != nil {
			return nil, fmt.Errorf("Error EEV readDeviceRegs: Reading EEV features reg\n  %w\n", err)
		} else {
			wordArray[i] = val
		}
	}

	return wordArray, nil
}

func (Device *DeviceType) getDeviceField(regName string, fieldName string) (uint32, error) {
	devReg, ok := Device.Registers[regName]
	if !ok {
		return 0, fmt.Errorf("Error EEV getDeviceField: Unknown Register Name")
	}

	field, ok := devReg.Fields[fieldName]
	if !ok {
		return 0, fmt.Errorf("Error EEV getDeviceField: Unrecognized Field Name")
	}

	if field.Len <= 0 || field.Msb  < (field.Len - 1) {
		return 0, fmt.Errorf("Error EEV getDeviceField: Bad field Length or Msb")
	}

	shift := field.Msb - (field.Len - 1)
	mask := uint32((1 << field.Len) - 1)

	return uint32(devReg.IntValue >> shift) & mask, nil
}

// EevRegCmd sends a CoAP GET or PUT request and returns the payload as a []byte
func (Device *DeviceType) eevRegCmd(addr uint32, payload []byte, regAccOpt EevRegAccOpt) ([]byte, error) {

	var err error
	var wgResp sync.WaitGroup

	// ── Build a CoAP message ─────────────────────────────────────────
	var token []byte
	if EevTokenLen > 0 {
		token, err = generateRandBytes(EevTokenLen)
		if err != nil {
			fmt.Printf("%v\n", err)
			fmt.Printf("Warning EEV eevRegCmd: Failed to generate random token, using default")
			token = []byte{0x01, 0x02, 0x03, 0x04}
		}
	}

	// Random Message ID (2 bytes → uint16)
	msgIDBytes, err := generateRandBytes(2)
	if err != nil {
		fmt.Printf("%v\n", err)
		fmt.Printf("Warning EEV eevRegCmd: Failed to generate random msgID, using default")
		msgIDBytes = []byte{0x20, 00}
	}
	msgID := binary.BigEndian.Uint16(msgIDBytes)

	// CoAP Options
	var options []CoAPOption

	// Add EEVideo Register Access Option (optional)
	if regAccOpt.Insert {
		var accessTypeCount uint8
		accessTypeCount = regAccOpt.Count | (uint8(regAccOpt.Type) << 5)
		options = append(options, CoAPOption{
			Number: OptionEevRegAccess,
			Value:  []byte{accessTypeCount},
		})
	}

	// Add EEVideo Binary Address Option (always present)
	binAddr := make([]byte, 4)
	binary.BigEndian.PutUint32(binAddr, addr)
	options = append(options, CoAPOption{
		Number: OptionEevBinaryAddress,
		Value:  binAddr,
	})

	// Payload for PUT
	isPut := payload != nil
	msgType := uint8(0) // Confirmable
	code := uint8(1)    // GET = 0.01
	if isPut {
		code = uint8(3) // PUT = 0.03
	}

	msg, err := buildCoAPMessage(msgType, code, msgID, token, options, payload)
	if err != nil {
		return []byte{}, err
	}

	udpConn, err := net.ListenUDP("udp", HostUDP)
	if err != nil {
		return []byte{}, fmt.Errorf("Error EEV eevRegCmd: Creating HostUDP connection\n  %w", err)
	}
	defer udpConn.Close()

	// Open UDP connection to read EEV device response
	rdUdpChan := make(chan []byte)
	wgResp.Add(1)
	go func() {
		defer wgResp.Done()
		defer close(rdUdpChan)
		readPacket(udpConn, rdUdpChan)
	}()

	// Write CoAP EEVideo request message to UDP connection
	udpConn.SetWriteDeadline(time.Now().Add(EevTimeOut))
	_, err = udpConn.WriteToUDP(msg, DeviceUDP)
	if err != nil {
		return []byte{}, fmt.Errorf("Error EEV eevRegCmd: Error Writing DeviceUDP request\n  %w", err)
	}

	// Parse UDP response packet
	resp, err := parseCoAPMessage(rdUdpChan)
	if err != nil {
		return []byte{}, fmt.Errorf("Error EEV eevRegCmd: Parsing CoAP response\n  %w", err)
	}
	wgResp.Wait()

	// Check for a valid and matching CoAP response
	if resp.Type != 2 {
		return []byte{}, fmt.Errorf("Error EEV eevRegCmd: Response not an ACK (CoAP Type = %d)", resp.Type)
	}
	if !bytes.Equal(resp.Token, token) {
		return []byte{}, fmt.Errorf("Error EEV eevRegCmd: Response Token mismatch")
	}
	if resp.Code != 68 && resp.Code != 69 { // 2.04, 2.05
		respCode, ok := CoapRespCodes[resp.Code]
		if ok {
			return []byte{}, fmt.Errorf("Error EEV eevRegCmd: Received unexpected CoAP Response Code, %s", respCode.Desc)
		}
		return []byte{}, fmt.Errorf("Error EEV eevRegCmd: Received unexpected CoAP Response Code, %d.%02d", resp.Code>>5, resp.Code&0x1F)
	}

	return resp.Payload, nil
}

// Read a UDP packet and place response in channel
func readPacket(conn *net.UDPConn, channel chan<- []byte) {
	buf := make([]byte, 2048)
	err := conn.SetDeadline(time.Now().Add(EevTimeOut))
	if err != nil {
		fmt.Printf("Error EEV readPacket: Unable to SetDeadline\n")
		return
	}
	n, _, err := conn.ReadFromUDP(buf)
	if err != nil {
		if netErr, ok := err.(net.Error); ok && netErr.Timeout() {
			fmt.Printf("Warning EEV readPacket: UDP Read timed out\n")
			return
		}
		fmt.Printf("Error EEV readPacket: UDP Read, %v\n", err)
		return
	}
	channel <- buf[0:n]
	conn.SetDeadline(time.Now()) // cancel deadline
}

// generateRandBytes returns a []byte of random values of a uint8 provided length
func generateRandBytes(length uint8) ([]byte, error) {
	b := make([]byte, length)
	if _, err := rand.Read(b); err != nil {
		return nil, fmt.Errorf("Error EEV generateRandBytes: Failed to generate random bytes\n  %w", err)
	}
	return b, nil
}

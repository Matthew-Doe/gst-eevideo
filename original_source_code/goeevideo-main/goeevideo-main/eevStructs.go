// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package eev

import (
)

//
// EEVideo Main Structs
//

// EevRegAccOpt struct used for creating a Register Access CoAP Option
type EevRegAccOpt struct {
	Insert bool
	Count  uint8
	Type   uint8
}

//
// EEVideo Discovery Structs
//

// EevUdpResponse is used to store UDP response info/data gathered during a
// Discovery CoAP request message
type eevUdpResponse struct {
	IfName string
	IfIP   string
	DevIP  string //  *net.UDPAddr
	Data   []byte
}

//
// EEVideo Feature List Structs
//

type FieldType struct {
	Msb uint32 `yaml:"msb"`
	Len uint32 `yaml:"len"`
}

type EevRegisterType struct {
	Offset uint32               `yaml:"offset"`
	Name   string               `yaml:"name"`
	Access string               `yaml:"acc,omitempty"`
	Fields map[string]FieldType `yaml:"fields,omitempty"`
}

type PointerType struct {
	Index     int               `yaml:"index"`
	Name      string            `yaml:"name"`
	Registers []EevRegisterType `yaml:"registers"`
}

type EevFeatureType struct {
	Name      string           `yaml:"name"`
	ShortName string           `yaml:"sname"`
	Pointers  []PointerType `yaml:"pointers"`
}

// Features is a map from hex ID to Feature
type EevFeaturesStrType map[string]EevFeatureType

type EevFeaturesType map[uint32]EevFeatureType

type CapabilitiesType struct {
	DecAvail  bool `yaml:"decAvail"`
	MultAddr  bool `yaml:"multAddr"`
	StringRd  bool `yaml:"stringRd"`
	FifoRd    bool `yaml:"fifoRd"`
	ReadRst   bool `yaml:"readRst"`
	MaskWr    bool `yaml:"maskWr"`
	BitTog    bool `yaml:"bitTog"`
	BitSet    bool `yaml:"bitSet"`
	BitClear  bool `yaml:"bitClear"`
	StaticIP  bool `yaml:"staticIP"`
	LinkLocIP bool `yaml:"linkLocIP"`
	DhcpIP    bool `yaml:"dhcpIP"`
	MultiDisc bool `yaml:"multiDisc"`
}

type DeviceRegisterAddrType struct { // no name for maps with name key
	Addr     uint32               `yaml:"addr"`
	Access   string               `yaml:"acc"`
	IntValue uint32               `yaml:"intval"`
	StrValue string               `yaml:"strval"`
	Fields   map[string]FieldType `yaml:"fields,omitempty"`
}

type DeviceRegisterType struct {
	Addr     uint32               `yaml:"addr"`
	Name     string               `yaml:"name"`
	Access   string               `yaml:"acc"`
	IntValue uint64               `yaml:"intval"`
	StrValue string               `yaml:"strval"`
	Fields   map[string]FieldType `yaml:"fields,omitempty"`
}

type LocationType struct {
	IfName      string `yaml:"ifName"`
	IfIP        string `yaml:"ifIP"`
	DevIP       string `yaml:"devIP"`
}

type DeviceMapType struct {
	LastStatic   uint32 `yaml:"lastStatic"`
	FirstMutable uint32 `yaml:"firstMutable"`
	LastMutable  uint32 `yaml:"lastMutable"`
}

type DeviceType struct {
	Location     LocationType                      `yaml:"location"`
	Capabilities CapabilitiesType                  `yaml:"capabilities"`
	Map          DeviceMapType                     `yaml:"map"`
	Registers    map[string]DeviceRegisterAddrType `yaml:"features"`
}

// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package builtins

import (
	"fmt"

	"go.starlark.net/starlark"
	"gitlab.com/eevideo/goeevideo"
)

// builtin function to call StreamStart
func StreamStart(thread *starlark.Thread, b *starlark.Builtin, args starlark.Tuple, kwargs []starlark.Tuple) (starlark.Value, error) {
	var streamNum  starlark.String
	var destPortSL starlark.Int
	var delaySL    starlark.Int
	var maxPktSL   starlark.Int

	err := starlark.UnpackArgs("stream_start", args, kwargs,
		"stream_num", &streamNum,
		"dest_port", &destPortSL,
		"delay", &delaySL,
		"max_pkt_size", &maxPktSL)
	if err != nil {
			return nil, err
	}

	state  := getState(thread)
	if state.activeDevice == "" {
		return nil, fmt.Errorf("no device initialized; call init_device first")
	}

	// Process variables
	destPort64, ok := destPortSL.Uint64()
	if !ok {
		return nil, fmt.Errorf("Invalid dest_port: %d\n", destPortSL)
	}
	destPort := uint32(destPort64)

	delay64, ok := delaySL.Uint64()
	if !ok {
		return nil, fmt.Errorf("Invalid delay: %d\n", delaySL)
	}
	delay := uint32(delay64)

	maxPkt64, ok := maxPktSL.Uint64()
	if !ok {
		return nil, fmt.Errorf("Invalid max_pkt_size: %d\n", maxPktSL)
	}
	maxPkt := uint32(maxPkt64)

	// Start stream on provided streamNum
	err = eev.Device.StreamStart(streamNum.GoString(), eev.Device.Location.IfIP, destPort, delay, maxPkt)
	if err != nil {
		return nil, err
	}

	return starlark.None, nil
}

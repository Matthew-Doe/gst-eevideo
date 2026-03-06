// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package builtins

import (
	"fmt"

	"go.starlark.net/starlark"
	"gitlab.com/eevideo/goeevideo"
)

// builtin function to call StreamStop
func StreamStop(thread *starlark.Thread, b *starlark.Builtin, args starlark.Tuple, kwargs []starlark.Tuple) (starlark.Value, error) {
	var streamNum starlark.String

	err := starlark.UnpackArgs("stream_stop", args, kwargs,
		"stream_num", &streamNum)
	if err != nil {
			return nil, err
	}

	state  := getState(thread)
	if state.activeDevice == "" {
		return nil, fmt.Errorf("no device initialized; call init_device first")
	}

	// Stop streaming on provided streamNum
	err = eev.Device.StreamStop(streamNum.GoString())
	if err != nil {
		return nil, err
	}

	return starlark.None, nil
}

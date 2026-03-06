// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package builtins

import (
  "fmt"
  "go.starlark.net/starlark"
  "gitlab.com/eevideo/goeevideo"
)

// initDeviceBuiltin initializes an eevdevice given the path/name of a devicecfg file
func InitDevice(thread *starlark.Thread, _ *starlark.Builtin, args starlark.Tuple, kwargs []starlark.Tuple) (starlark.Value, error) {
    var id      starlark.String
    var verbose starlark.Int
    err := starlark.UnpackArgs("init_device", args, kwargs,
    	    "id",      &id,
            "verbose", &verbose)
    if err != nil {
        return nil, err
    }

    rawID := id.GoString()

    state := getState(thread)
    if state.activeDevice != "" {
        return nil, fmt.Errorf("device already initialized to %s", state.activeDevice)
    }

    state.activeDevice = rawID

    // Real initialization (serial port, connection, etc.)
    fmt.Printf("Initializing device %s\n", rawID)
    err = eev.Init(rawID)
    if err != nil {
      return nil, fmt.Errorf("Bad eev Init: %v\n", err)
    }

    verbGo, _ := verbose.Int64()
    eev.Verbose = int(verbGo)
    // e.g. openConnection(rawID)

    return starlark.None, nil
}

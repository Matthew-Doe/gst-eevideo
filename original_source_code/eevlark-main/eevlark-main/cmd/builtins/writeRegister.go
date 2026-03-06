// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package builtins

import (
  "fmt"
  "go.starlark.net/starlark"
  "gitlab.com/eevideo/goeevideo"
)

func WriteRegister(thread *starlark.Thread, _ *starlark.Builtin, args starlark.Tuple, kwargs []starlark.Tuple) (starlark.Value, error) {

  var (
    regName starlark.String
    fields  *starlark.Dict
  )

  err := starlark.UnpackArgs("write_register", args, kwargs,
           "register_name", &regName,
           "fields",        &fields)
  if err != nil {
    return nil, err
  }

  rawReg := regName.GoString()
  state  := getState(thread)
  if state.activeDevice == "" {
    return nil, fmt.Errorf("no device initialized; call init_device first")
  }

  // Convert dict → Go map[string]unit32
  fieldMap := make(map[string]uint32)
  for _, key := range fields.Keys() {
    keyStr, _ := starlark.AsString(key) // we already validated keys are strings

    val, _, _ := fields.Get(key) // returns value, found(bool), err
    var iVal uint32
    err := starlark.AsInt(val,&iVal)
    if err != nil {
        return nil, fmt.Errorf("field %q must be integer: %w", keyStr, err)
    }
    fieldMap[keyStr] = uint32(iVal)
  }

  // Real hardware write
  fmt.Printf("Writing  register %s: %v\n", rawReg, fieldMap)
	err = eev.Device.WriteRegFields(rawReg, fieldMap)
	if err!=nil {
		return nil, fmt.Errorf("WriteRegFields Error:/n  %w\n",err)
	}

  return starlark.None, nil
}

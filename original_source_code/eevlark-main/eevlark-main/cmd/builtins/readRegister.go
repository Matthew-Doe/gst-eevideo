// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package builtins

import (
  "fmt"
  "go.starlark.net/starlark"
  "go.starlark.net/starlarkstruct"
  "gitlab.com/eevideo/goeevideo"
)

// builtin function to call ReadReg
func ReadRegister(thread *starlark.Thread, b *starlark.Builtin, args starlark.Tuple, kwargs []starlark.Tuple) (starlark.Value, error) {
  var regName starlark.String

  err := starlark.UnpackArgs("read_reg", args, kwargs,
    "reg_name", &regName)
  if err != nil {
      return nil, err
  }

  state  := getState(thread)
  if state.activeDevice == "" {
    return nil, fmt.Errorf("no device initialized; call init_device first")
  }


  val, fields, err := eev.Device.ReadReg(regName.GoString())
  if err != nil {
    return nil, err
  }

  // Convert map[string]uint32 → Starlark dict
  fieldsDict := starlark.NewDict(len(fields))
  for k, v := range fields {
      if err := fieldsDict.SetKey(starlark.String(k), starlark.MakeUint64(uint64(v))); err != nil {
          return nil, err // rare, but possible
      }
  }

// Return struct(ok=true, value=..., fields=...)
  return starlarkstruct.FromKeywords(
    starlark.String("ReadResult"),
    []starlark.Tuple{
        {starlark.String("value"),  starlark.MakeUint64(uint64(val))},
        {starlark.String("fields"), fieldsDict},
        // Optional: add more fields like "timestamp", "reg_name", etc.
    },
  ), nil
}

// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package builtins

import (
  "fmt"
  "go.starlark.net/starlark"
  "gitlab.com/eevideo/goeevideo"
)

func WriteI2C(thread *starlark.Thread, _ *starlark.Builtin, args starlark.Tuple, kwargs []starlark.Tuple) (starlark.Value, error) {
  var (
    portStr starlark.String
    i2cID   starlark.Int
    addr    starlark.Bytes
    data    starlark.Value
  )

  err := starlark.UnpackArgs("write_i2c",
    args, kwargs,
    "port",   &portStr,
    "i2c_id", &i2cID,
    "addr",   &addr,
    "data?",  &data)   // optional, defaults to empty bytes
  if err != nil {
    return starlark.MakeInt(0), fmt.Errorf("%w/n",err)
  }

  // Convert starlark.Int → uint32 safely
  id64, ok := i2cID.Uint64()
  if !ok || id64 > uint64(^uint32(0)) {
    return starlark.MakeInt(0), fmt.Errorf("i2c_id too large for uint32\n")
  }

  // Get port from thread-local state (set by init_device)
  state := getState(thread)
  if state.activeDevice == "" {
    return starlark.MakeInt(0), fmt.Errorf("no device initialized; call init_device first\n")
  }

  addrGo := []byte(addr)

  var dataBytes[]byte
  switch v := data.(type) {
  case starlark.Bytes:    // User passed bytes(...) or b"..." → perfect
    dataBytes = []byte(v)
  case *starlark.List:    // User passed a list [0x01, 0xFF, ...] → convert manually
    dataBytes = make([]byte, v.Len())
    iter := v.Iterate()
    defer iter.Done()
    var idx int
    var elem starlark.Value
    for iter.Next(&elem) {
      var i64 int64
      err := starlark.AsInt(elem,&i64)
      if err != nil {
        return starlark.MakeInt(0), fmt.Errorf("data list element %d is not an integer:\n %w\n", idx, err)
      }
      if i64 < 0 || i64 > 255 {
        return starlark.MakeInt(0), fmt.Errorf("data list element %d out of range (must be 0–255): %d\n", idx, i64)
      }
      dataBytes[idx] = byte(i64)
      idx++
    }
  case starlark.NoneType:   // Optional: treat None as empty data
    dataBytes = nil
  default:
    return starlark.MakeInt(0), fmt.Errorf("data must be bytes, list of integers (0–255), or nil got %s\n", data.Type())
  }
  // Call your library function
  err = eev.WriteI2C(portStr.GoString(), uint32(id64), addrGo, dataBytes)
  if err != nil {
    return starlark.MakeInt(0), fmt.Errorf("write_i2c failed: %w\n", err)
  }

  return starlark.Tuple{
    starlark.MakeInt(len(dataBytes)),
    starlark.String(fmt.Sprintf("0x%X",dataBytes)),
    starlark.String(fmt.Sprintf("%d",dataBytes)),
  }, nil

}

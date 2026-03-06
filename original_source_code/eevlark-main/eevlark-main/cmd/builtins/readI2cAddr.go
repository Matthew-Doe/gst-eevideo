package builtins

import (
  "fmt"
  "go.starlark.net/starlark"
  // "go.starlark.net/starlarkstruct"
  "gitlab.com/eevideo/goeevideo"
)

// builtin function to call ReadI2cAddr
func ReadI2cAddr(thread *starlark.Thread, b *starlark.Builtin, args starlark.Tuple, kwargs []starlark.Tuple) (starlark.Value, error) {

  var (
  	portStr  starlark.String
  	i2cID    starlark.Int
  	addr     starlark.Bytes
  	rdLen    starlark.Int
  )

  err := starlark.UnpackArgs("read_i2c_addr",
    args, kwargs,
    "port",   &portStr,
    "i2c_id", &i2cID,
    "addr",   &addr,
  	"rd_len", &rdLen)
  if err != nil {
      return nil, err
  }

  state  := getState(thread)
  if state.activeDevice == "" {
    return nil, fmt.Errorf("no device initialized; call init_device first")
  }

  goI2cID, ok := i2cID.Uint64()
  if !ok || goI2cID>0x7F {
    return nil, fmt.Errorf("i2c_id out of range: %s\n",i2cID)
  }

  addrGo := []byte(addr)

  goRdLen, ok := rdLen.Int64()
  if !ok || goRdLen<0  {
  	return nil, fmt.Errorf("rd_Len out of range: %s\n",rdLen)
  }

  // Call Library function
	returnBytes, err := eev.ReadI2cAddr(portStr.GoString(),uint32(goI2cID),addrGo,int(goRdLen))
	if err!=nil {
		return nil, fmt.Errorf("eev.ReadI2cAddr error:\n%w",err)
	}

	values := make([]starlark.Value, len(returnBytes))
	for index, byteVal := range returnBytes {
		values[index] = starlark.MakeInt(int(byteVal))
	}

	return starlark.NewList(values), nil

}
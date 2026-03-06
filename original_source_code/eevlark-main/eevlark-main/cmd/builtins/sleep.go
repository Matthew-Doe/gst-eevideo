// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package builtins

import (
	"fmt"
	"time"
	"go.starlark.net/starlark"
)

// sleepBuiltin(seconds: number) → None
// Pauses execution for the given number of seconds (float or int allowed).
func Sleep(thread *starlark.Thread, _ *starlark.Builtin, args starlark.Tuple, kwargs []starlark.Tuple) (starlark.Value, error) {
	var secs starlark.Float // accepts int or float
	err := starlark.UnpackArgs("sleep", args, kwargs,
		       "seconds", &secs)
	if err != nil {
		return nil, err
	}

	if secs < 0 {
		return nil, fmt.Errorf("sleep: negative duration not allowed")
	}

	duration := time.Duration(float64(secs) * float64(time.Second))
	time.Sleep(duration)

	return starlark.None, nil
}

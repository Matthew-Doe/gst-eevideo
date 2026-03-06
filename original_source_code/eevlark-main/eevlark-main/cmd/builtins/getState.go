// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package builtins

import (
  "go.starlark.net/starlark"
)


// Per-script state
type scriptState struct {
    activeDevice string
    // add more fields later if needed (conn, logger, counters, ...)
}

// getState manages persistent objects across multiple calls from the script
func getState(thread *starlark.Thread) *scriptState {
	v := thread.Local("state")
	if v != nil {
		return v.(*scriptState)
	}
	s := &scriptState{}
	thread.SetLocal("state", s)   // ← stores the new state in this thread
	return s
}

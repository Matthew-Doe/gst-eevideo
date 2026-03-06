// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

//go:build darwin

package main

// #cgo LDFLAGS: -Wl,-no_warn_duplicate_libraries
// #cgo pkg-config: gstreamer-1.0
// #include <stdlib.h>
// #include <gst/gst.h>
// #include <TargetConditionals.h>
//
// typedef int (*MainFunc)();
//
// extern int realMain(void);
// extern int macosGMain(void);   // declaration only
import "C"

import (
	"os"
	"eeview/cmd"
)

//export realMain
func realMain() C.int {
    // Ignore argc/argv/user_data since Cobra doesn't need them
    cmd.Execute()
    return 0
}
func main() {
	os.Exit(int(C.macosGMain()))
}

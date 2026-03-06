// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package eev

import (
	"context"
	"errors"
	"fmt"
	"net"
	"os"
	"time"
	"sync"
)

// FrameCaptureStart runs the frame collection routine.
// Returns WaitGroup and an error channel (receives first fatal error or nil on clean exit).
func FrameCaptureStart(ctx     context.Context,
	                     conn    *net.UDPConn,
	                     maxPkt  int,
	                     timeout time.Duration,
	                     dataCh  chan<- *ImageBuffer) struct {
	Wg    *sync.WaitGroup
	ErrCh chan error
} {
	var wg sync.WaitGroup
	errCh := make(chan error, 1)

	wg.Go(func() {
		for {
			select {
			case <-ctx.Done():
				// fmt.Println("[DEBUG-producer] ctx.Done() received → sending err and exiting")
				errCh <- ctx.Err()
				return

			default:
				err := GvspFrame(ctx,conn,maxPkt,timeout,dataCh)
				// fmt.Println("[DEBUG-producer] GvspFrame returned:", err)
				if err == nil {
					continue // success → loop
				}

				// Error path — check if context is the cause
				if ctx.Err() != nil {
					// fmt.Println("[DEBUG-producer] Error path but ctx already canceled → exiting")
					errCh <- ctx.Err()
					return
				}

				// Non-cancel error
				if !errors.Is(err, os.ErrDeadlineExceeded) {
					fmt.Printf("Error EEV FrameCaptureStart: Capturing GVSP frame\n%v\n", err)
					errCh <- err
					return
				}

				// Just timeout → continue looping
				continue
			}
		}
	})

	return struct {
		Wg    *sync.WaitGroup
		ErrCh chan error
	}{&wg, errCh}
}

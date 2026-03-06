// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package builtins

import (
	"context"
	"errors"
	"fmt"
	"net"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"go.starlark.net/starlark"
	"gitlab.com/eevideo/goeevideo"
)

// builtin function to call StreamCapture
func StreamCapture(thread *starlark.Thread, b *starlark.Builtin, args starlark.Tuple, kwargs []starlark.Tuple) (starlark.Value, error) {
	var streamNum  starlark.String
	var destPortSL starlark.Int
	var delaySL    starlark.Int
	var maxPktSL   starlark.Int
	var frameCountSL starlark.Int
	var filePath   starlark.String

	err := starlark.UnpackArgs("stream_capture", args, kwargs,
		"stream_num", &streamNum,
		"dest_port", &destPortSL,
		"delay", &delaySL,
		"max_pkt_size", &maxPktSL,
		"frame_count", &frameCountSL,
		"file_path", &filePath)
	if err != nil {
		return nil, err
	}

	state  := getState(thread)
	if state.activeDevice == "" {
		return nil, fmt.Errorf("no device initialized; call init_device first")
	}

	// Process variables
	destPort64, ok := destPortSL.Uint64()
	if !ok {
		return nil, fmt.Errorf("Invalid dest_port: %d\n", destPortSL)
	}
	destPort := uint32(destPort64)

	delay64, ok := delaySL.Uint64()
	if !ok {
		return nil, fmt.Errorf("Invalid delay: %d\n", delaySL)
	}
	delay := uint32(delay64)

	maxPkt64, ok := maxPktSL.Uint64()
	if !ok {
		return nil, fmt.Errorf("Invalid max_pkt_size: %d\n", maxPktSL)
	}
	maxPkt := uint32(maxPkt64)

	frameCount64, ok := frameCountSL.Uint64()
	if !ok {
		return nil, fmt.Errorf("Invalid frame_count: %d\n", frameCountSL)
	}
	frameCount := uint32(frameCount64)

	// Setup UDP connection
	streamConn, err := eev.SetupStreamListener(eev.Device.Location.IfIP, destPort, eev.Device.Location.IfIP)
	if err != nil {
		fmt.Printf("Error setting up UDP listener\n  %v\n", err)
	}
	defer streamConn.Close()
	destPort = uint32(streamConn.LocalAddr().(*net.UDPAddr).Port)

	// Setup context
	ctx, cancel := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer cancel()

	// Setup WaitGroup and watcher channels
	var wg sync.WaitGroup
	errCh  := make(chan error, 2)
	doneCh := make(chan struct{})

	// Make data channel for Image/Frame data
	dataCh := make(chan *eev.ImageBuffer, 10)

	// Set UDP Buffer size
	var maxPktSize int = 256 // TODO: Figure out if additional UDP buffer increase is really needed
	maxPktSize += int(maxPkt)

	// Write stream registers to start streaming
	err = eev.Device.StreamStart(streamNum.GoString(), eev.Device.Location.IfIP, destPort, delay, maxPkt)
	if err != nil {
		return nil, fmt.Errorf("Error Starting stream\n  %v\n", err)
	}

	// Start goroutine to collect frame/image data from UDP
	fmt.Printf("Listening for EEVideo stream on IP:%s:%d\n", eev.Device.Location.IfIP, destPort)
	frameCapRes := eev.FrameCaptureStart(ctx, streamConn, maxPktSize, 2 * time.Second, dataCh)

	// Start goroutine to save raw images to local location
	rawImgRes := eev.RawImgCaptureStart(ctx, frameCount, filePath.GoString(), dataCh)

	// ── goroutine and context handling ───────────────────────────────
	// Producer watcher
	wg.Go(func() {
		err := <-frameCapRes.ErrCh;
		if err != nil && !errors.Is(err, context.Canceled) {
			errCh <- fmt.Errorf("Frame capture failed\n  %w\n", err)
		}
	})

	// Consumer watcher
	wg.Go(func() {
		err := <-rawImgRes.ErrCh
		if err == nil {
			fmt.Println("Raw capture completed")
			close(doneCh)  // Triggers cancel below
			return
		}
		if !errors.Is(err, context.Canceled) {
			errCh <- fmt.Errorf("Raw capture failed\n  %w\n", err)
		}
	})

	// Shutdown trigger
	wg.Go(func() {
		select {
		case <-ctx.Done():
			return
		case err := <-errCh:
			fmt.Printf("Error detected\n  %v\n", err)
			cancel()
		case <-doneCh:
			cancel()
		}
	})

	// Wait for trigger
	<-ctx.Done()

	// ── Graceful shutdown sequence ───────────────────────────────────
	close(dataCh)

	// Wait for producer, consumer, and trigger goroutines to finish
	wg.Wait()

	// Stop stream
	if err := eev.Device.StreamStop(streamNum.GoString()); err != nil {
		return nil, fmt.Errorf("Warning: StreamStop failed\n  %v\n", err)
	} else {
		fmt.Printf("%s stopped\n", streamNum.GoString())
	}

	return starlark.None, nil
}

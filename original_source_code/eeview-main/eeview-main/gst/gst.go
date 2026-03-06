// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package gst

import (
	"context"
	"errors"
	"fmt"
	"net"
	"path/filepath"
	"runtime"
	"strings"
	"sync"

	// "eeview/capture"

	// "github.com/go-gst/go-glib"
	"github.com/go-gst/go-gst/pkg/gst"
	"github.com/go-gst/go-gst/pkg/gstapp"
	"gitlab.com/eevideo/goeevideo"

)

// ErrGstWindowClosed is a sentinel error returned when the GStreamer output
// window is closed by the user (normal, non-fatal shutdown).
var ErrGstWindowClosed = errors.New("output window was closed")

// GstCaptureImgs is used to hold application image capture arguments
type GstCaptureImgs struct {
	// Raw     bool
	Jpeg    bool
	Num     uint32
	Loc     string
}

// GstRunConfig holds configuration for running the GStreamer pipeline
type GstRunConfig struct {
	Conn       *net.UDPConn
	MaxPktSize int
	DataCh     <-chan *eev.ImageBuffer
	ShowFPS    bool
	CapImg    *GstCaptureImgs
	Verbose    int
}

// GstResult bundles the wait group and error channel from GStreamer
type GstResult struct {
	Wg    *sync.WaitGroup
	ErrCh chan error
}

// GstStart launches the GStreamer processing in a background goroutine.
func GstStart(ctx context.Context, cfg GstRunConfig) GstResult {
	var wg sync.WaitGroup
	errCh := make(chan error, 1)

	wg.Go(func() {
		err := GstRun(ctx, cfg)
		errCh <- err
	})

	return GstResult{Wg: &wg, ErrCh: errCh}
}

// GstRun runs the GStreamer pipeline and blocks until shutdown.
// Returns an error on critical failure or window close (caller can decide to cancel ctx).
func GstRun(ctx context.Context, cfg GstRunConfig) error {
	// Initialize Gstreamer
	gst.Init()

	// ── Create Gstreamer pipeline and elements ──────────────────────
	pipeline := gst.NewPipeline("pipeline").(gst.Pipeline)

	// Create appsrc
	source := gst.ElementFactoryMake("appsrc", "source").(gstapp.AppSrc)

	// Setup appsrc
	var capsString string

	// Get first frame to determine caps (with proper shutdown handling)
	var imgBuf *eev.ImageBuffer
	select {
		case buf, ok := <-cfg.DataCh:
			if !ok {
				fmt.Println("Warning GST: Data channel closed before first frame received")
				return fmt.Errorf("GST: Data channel closed before first frame received")
			}
			imgBuf = buf
		case <-ctx.Done():
			fmt.Println("Warning GST: Cancel before first frame received")
			return fmt.Errorf("GST: Cancel before first frame received: %w", ctx.Err())
	}
	if imgBuf == nil {
		fmt.Println("Error GST: Received nil image buffer")
		return fmt.Errorf("GST: Received nil image buffer")
	}


	if imgBuf.PayloadType == eev.GvspJPEG {
		capsString = "image/jpeg"
	} else {
		switch imgBuf.PixelFormat {
		case eev.MONO8:
			capsString = "video/x-raw,format=GRAY8"
		case eev.MONO16:
			capsString = "video/x-raw,format=GRAY16_LE"
		case eev.GR8:
			capsString = "video/x-bayer,format=grbg"
		case eev.RG8:
			capsString = "video/x-bayer,format=rggb"
		case eev.GB8:
			capsString = "video/x-bayer,format=gbrg"
		case eev.BG8:
			capsString = "video/x-bayer,format=bggr"
		case eev.RGB8:
			capsString = "video/x-raw,format=RGB"
		case eev.YUV422_8_UYVY:
			capsString = "video/x-raw,format=UYVY"
		default:
			return fmt.Errorf("Error Gst: Unsupported pixel format %d", imgBuf.PixelFormat)
		}
	}
	source.SetObjectProperty("format", gst.FormatTime) // =3,  gst.FormatBytes=2
	source.SetObjectProperty("is-live", true)
	source.SetObjectProperty("do-timestamp", true)
	source.SetObjectProperty("stream-type", 0) // app.GST_APP_STREAM_TYPE_STREAM)
	source.SetObjectProperty("caps", gst.CapsFromString(fmt.Sprintf("%s,width=%d,height=%d,framerate=0/1", capsString, imgBuf.Width, imgBuf.Height)))


	// Create videoconvert
	videoconvert := gst.ElementFactoryMake("videoconvert", "videoconvert")
	if videoconvert == nil {
		return fmt.Errorf("GST: Failed to create videoconvert")
	}

	// Create bayer2rgb (situational use)
	var bayer2rgb gst.Element
	isBayer := strings.Contains(capsString, "video/x-bayer")
	if isBayer {
		bayer2rgb = gst.ElementFactoryMake("bayer2rgb", "bayer2rgb")
		if bayer2rgb == nil {
			return fmt.Errorf("GST: Failed to create bayer2rgb")
		}
	}

	// Create jpegdec (situational use)
	var jpegdec gst.Element
	if imgBuf.PayloadType == eev.GvspJPEG {
		jpegdec = gst.ElementFactoryMake("jpegdec", "jpegdec")
		if jpegdec == nil {
			return fmt.Errorf("GST: Failed to create jpegdec")
		}
	}

	// Create sink (final sink used is situational)
	var jpegenc gst.Element
	var sink gst.Element

	if cfg.CapImg.Jpeg {
		if err := eev.CheckDir(cfg.CapImg.Loc); err != nil {
			return fmt.Errorf("GST: Invalid JPEG dir \"%s\"\n  %w", cfg.CapImg.Loc, err)
		}

		jpegenc = gst.ElementFactoryMake("jpegenc", "jpegenc")
		if jpegenc == nil {
			return fmt.Errorf("GST: Failed to create jpegenc")
		}

		sink = gst.ElementFactoryMake("multifilesink", "sink")
		if sink == nil {
			return fmt.Errorf("GST: Failed to create multifilesink")
		}
		sink.SetObjectProperty("location", cfg.CapImg.Loc+"/frame_%05d.jpg") // adjust pattern

	} else {
		var displaySink gst.Element

		if runtime.GOOS == "windows" {
			displaySink = gst.ElementFactoryMake("d3d11videosink", "display-sink")
		} else {
			// displaySink = gst.ElementFactoryMake("xvimagesink", "display-sink")
			displaySink = gst.ElementFactoryMake("autovideosink", "display-sink")
		}
		if displaySink == nil {
			return fmt.Errorf("failed to create display sink")
		}
		displaySink.SetObjectProperty("sync", false)

		if cfg.ShowFPS {
			sink = gst.ElementFactoryMake("fpsdisplaysink", "sink")
			if sink != nil {
				sink.SetObjectProperty("video-sink", displaySink)
				sink.SetObjectProperty("sync", false)
			} else {
				sink = displaySink // fallback
			}
		} else {
			sink = displaySink
		}
	}

	// Add elements to pipeline
	pipeline.AddMany(source, videoconvert, sink)

	if bayer2rgb != nil {
		pipeline.Add(bayer2rgb)
	}
	if jpegdec != nil {
		pipeline.Add(jpegdec)
	}
	if jpegenc != nil {
		pipeline.Add(jpegenc)
	}

	// Link used pipeline elements
	cur := source.(gst.Element)

	if jpegdec != nil {
		cur.Link(jpegdec)
		cur = jpegdec
	}

	if isBayer && bayer2rgb != nil {
		cur.Link(bayer2rgb)
		cur = bayer2rgb
	}

	cur.Link(videoconvert)
	cur = videoconvert

	if jpegenc != nil {
		videoconvert.Link(jpegenc)
		cur = jpegenc
	}

	cur.Link(sink)


	// ── Gstreamer AppSrc NeedDataFunc to use frame/image data ───────
	var gstCaptureCnt uint32 = 1
	source.ConnectNeedData(func(self gstapp.AppSrc, _ uint) {

		select {
		case <-ctx.Done():
			self.EndOfStream()
		case imgBuf, ok := <-cfg.DataCh:
			if !ok {
				self.EndOfStream()
				return
			}

			if cfg.CapImg.Jpeg {
				if gstCaptureCnt > cfg.CapImg.Num {
					self.EndOfStream()
					return
				} else {
					// Update multifilesink location property
					filename := filepath.Join(cfg.CapImg.Loc, fmt.Sprintf("frame_%02d_%dx%d.jpg", imgBuf.BlockID, imgBuf.Width, imgBuf.Height))
					sink.SetObjectProperty("location", filename)
					fmt.Printf("Saved frame %d to %s\n", imgBuf.BlockID, filename)
					gstCaptureCnt++
				}
			}

			// Write image data to the buffer
			gstBuf := gst.NewBufferAllocate(nil, uint(len(imgBuf.Data)), nil)
			mapped, ok := gstBuf.Map(gst.MapWrite)
			if !ok {
				fmt.Println("Error GST: Failed to map buffer")
				return
			}
			_, err := mapped.Write(imgBuf.Data)
			if err != nil {
				fmt.Println("Error GST: Failed to write to buffer")
				return
			}

			mapped.Unmap()

			// Push the buffer to the appsrc
			self.PushBuffer(gstBuf)
		}
	})

	// ── Start Gstreamer pipeline ────────────────────────────────────
	pipeline.SetState(gst.StatePlaying)

	for msg := range pipeline.GetBus().Messages(context.Background()) {
		if msg == nil {
			if ctx.Err() != nil {
				return ctx.Err()
			}
			continue
		}
		switch msg.Type() {
		case gst.MessageEOS:
			// pipeline.SetState(gst.StateNull)
			if cfg.Verbose > 0 {
				fmt.Println("Info GST: EOS received")
			}
			return nil
		case gst.MessageError:
			// debug, gerr := msg.ParseError()
			// errStr := gerr.Error()
			// if "Output window was closed" == errStr || "Output window was closed" == gerr.Message() {
			debugStr, gerr := msg.ParseError()
			if gerr == nil {
				// Should not happen, but guard anyway
				return fmt.Errorf("Error GST: Received error message without error")
			}

			errStr := gerr.Error()
			if errStr == "Output window was closed" {
				if cfg.Verbose > 0 {
					fmt.Println("Info GST: Output window closed by user")
				}
				return ErrGstWindowClosed
			}
			// Real error
			if debugStr != "" {
				fmt.Printf("Error GST: %s | debug: %s\n", gerr.Error(), debugStr)
			} else {
				fmt.Printf("Error GST: %s\n", gerr.Error())
			}
			return gerr
		// case gst.MessageStateChanged:
		// 	// Optional: log state changes if you want
		// 	old, new, _ := msg.ParseStateChanged()
		// 	if old == gst.StateNull && new == gst.StateReady {
		// 		fmt.Println("Pipeline state changed → Ready")
		// 	}
		case gst.MessageWarning:
			warn, _ := msg.ParseWarning()
			if cfg.Verbose > 0 {
				fmt.Printf("Warning GST: %v\n", warn)
			}
		default:
			// fmt.Println(msg)
		}
	}

	// If the bus channel closes unexpectedly (rare)
	return fmt.Errorf("Error GST: Bus channel closed unexpectedly")
}

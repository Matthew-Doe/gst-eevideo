// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package cmd

import (
	"context"
	"errors"
	"fmt"
	"net"
	"net/netip"
	"os"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"eeview/gst"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"gitlab.com/eevideo/goeevideo"
)

// viewerCmd represents the viewer command
var viewerCmd = &cobra.Command{
	Use:   "viewer",
	Short: "EEVideo Stream Viewer",
	Long:  `View an EEVideo Stream in a window`,
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Println("viewer called")

		// ── Establish Arguements and Variables ────────────────────────
		viewerSnum      := viper.GetString("streamNum")
		viewerDestIP    := viper.GetString("destIP")
		viewerDestPort  := viper.GetUint32("destPort")
		viewerDelay     := viper.GetUint32("delay")
		viewerMaxPkt    := viper.GetUint32("maxPacket")
		viewerFPS,_     := cmd.Flags().GetBool("fps")
		viewerStart,_   := cmd.Flags().GetBool("start")
		viewerStop,_    := cmd.Flags().GetBool("stop")
		viewerNoStop,_  := cmd.Flags().GetBool("noStop")
		viewerNoEev,_   := cmd.Flags().GetBool("noEEV")
		// viewerCapRaw,_   := cmd.Flags().GetBool("cap")
		viewerCapJPEG,_ := cmd.Flags().GetBool("capJpeg")
		viewerCapNum    := viper.GetUint32("frameCount")
		viewerCapLoc    := viper.GetString("imagePath")

		// if viewerCapRaw && viewerCapJPEG {
		// 	fmt.Println("Cannot capture JPEG and Raw images at the same time")
		// 	os.Exit(1)
		// }

		// Set EEV lib verbose level
		eev.Verbose = Verbose
		// eev.SetEevReqUdpPort(uint16(54325))

		// Load Device Config file
		device := viper.GetString("devicePath") + "/" + viper.GetString("deviceName")
		// fmt.Println(device)
		err := eev.Init(device)
		if err != nil {
			fmt.Printf("Error Init: %v\n", err)
			os.Exit(1)
		}

		// Variables
		capImgs := &gst.GstCaptureImgs{
			// Raw:     viewerCapRaw,
			Jpeg:    viewerCapJPEG,
			Num:     viewerCapNum,
			Loc:     viewerCapLoc,
		}

		var destIP string

		if viewerDestIP != "" {
			destIP = viewerDestIP
		} else {
			destIP = eev.Device.Location.IfIP
		}

		// Stop streaming and exit
		if viewerStop {
			err = eev.Device.StreamStop(viewerSnum)
			if err != nil {
				fmt.Printf("Error Stopping Stream: %v\n", err)
				os.Exit(1)
			}
			fmt.Printf("%s stopped\n", viewerSnum)
			os.Exit(0)
		}

		// Start streaming and exit
		if viewerStart {
			pktSizeFields := map[string]uint32 {"enable": 1,}
			err = eev.Device.WriteRegFields(viewerSnum + "_MaxPacketSize" , pktSizeFields)
			if err != nil {
				fmt.Printf("Error Starting stream\n  %v\n", err)
				os.Exit(1)
			}
			fmt.Printf("%s started\n", viewerSnum)
			os.Exit(0)
		}

		// Setup UDP connection
		streamConn, err := eev.SetupStreamListener(destIP, viewerDestPort, eev.Device.Location.IfIP)
		if err != nil {
			fmt.Printf("Error setting up UDP listener\n  %v\n", err)
			os.Exit(1)
		}
		defer streamConn.Close()

		destPort := uint32(streamConn.LocalAddr().(*net.UDPAddr).Port)
		if Verbose > 0 {
			fmt.Printf("Local Port = %d\n", destPort)
		}

		// Setup context
		ctx, cancel := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
		defer cancel()

		// Setup WaitGroup and watcher channel
		var captureWg *sync.WaitGroup
		var captureErrCh chan error

		// Make data channel for Image/Frame data
		dataCh := make(chan *eev.ImageBuffer, 10)

		// Used for multicast destination IP address check
		addrMC, err := netip.ParseAddr(destIP)
		if err != nil {
			fmt.Println("DestIP address is not a UDP address")
		}

		// Setup Device Stream Registers
		var maxPkt uint32 = viewerMaxPkt
		var maxPktSize uint32 = 256  //TODO: Find out if 256 increase is necessary for UDP safety

		if !viewerNoEev {
			if destPort == 0 {
				fmt.Println("Error: Stream Destination Port was 0")
				os.Exit(1)
			}

			// Find system supported MaxPacketSize
			var negoErr error

			maxPktDone := make(chan struct{})
			go func() {
				defer close(maxPktDone)
				maxPkt, negoErr = eev.Device.StreamMaxPktSize(viewerSnum, destIP, viewerMaxPkt, viewerDelay)
			}()

			select {
				case <-maxPktDone:
					// finished normally
				case <-ctx.Done():
					fmt.Println("Shutdown requested during max packet negotiation")
					// Unfortunately cannot kill the goroutine → it may leak / hang
					// Best you can do is ignore the result and exit soon
					return
				case <-time.After(10 * time.Second):
					fmt.Println("Max packet negotiation timed out")
					return
			}
			if negoErr != nil {
				fmt.Printf("Error negotiating MaxPacketSize\n  %v\n", negoErr)
				os.Exit(1)
			}
			if maxPkt != (viewerMaxPkt) {
				fmt.Printf("Warning: MaxPacketSize size negotiated to %d\n", maxPkt+14)
				// FIXME: Current UDP buffer size is off by 14 in device
			}

			// Set UDP Buffer size
			maxPktSize += maxPkt

			// Write stream registers to start streaming
			err = eev.Device.StreamStart(viewerSnum, destIP, destPort, viewerDelay, maxPkt)
			if err != nil {
				fmt.Printf("Error Starting stream\n  %v\n", err)
				os.Exit(1)
			}
		}

		if eev.Device.Location.IfIP != destIP && !addrMC.IsMulticast() {
			fmt.Printf("Warning: No interface found for DestIPAddr %s.\n", destIP)
			fmt.Println("Warning: Stream enabled but unable to capture video")
		} else {
			// Start goroutine to collect frame/image data from UDP
			fmt.Printf("Listening for EEVideo stream on IP:%s:%d\n", destIP, destPort)
			frameCapRes := eev.FrameCaptureStart(ctx, streamConn, int(maxPktSize), 2 * time.Second, dataCh)

			// ── Save raw images to local location or start gstreamer ────
			// if capImgs.Raw {
				// Start goroutine to save raw images to local location
				// rawImgRes := eev.RawImgCaptureStart(ctx, viewerCapNum, viewerCapLoc, dataCh)
				// 	captureWg = rawImgRes.Wg
				// 	captureErrCh = rawImgRes.ErrCh
			// } else {
				// Start running gstreamer
				gstRunCfg := gst.GstRunConfig{
					Conn:       streamConn,
					MaxPktSize: int(maxPktSize),
					DataCh:     dataCh,
					ShowFPS:    viewerFPS,
					CapImg:     capImgs,
					Verbose:    Verbose,
				}
				gstRes := gst.GstStart(ctx, gstRunCfg)
				captureWg = gstRes.Wg
				captureErrCh = gstRes.ErrCh
			// }

			// ── Wait for shutdown trigger ──────────────────────────────────────────
			shutdownErr := make(chan error, 1)
			go func() {
				select {
				case <-ctx.Done() :
					shutdownErr <- ctx.Err()
				case err := <-frameCapRes.ErrCh :
					if err != nil && !errors.Is(err, context.Canceled) {
						fmt.Printf("Producer error → canceling context: %v\n", err)
						cancel()
					}
					shutdownErr <- err
				case err := <-captureErrCh :
					if err != nil {
						// if capImgs.Raw {
							// fmt.Printf("Raw capture error → canceling context: %v\n", err)
						// } else if errors.Is(err, gst.ErrGstWindowClosed) {
						if errors.Is(err, gst.ErrGstWindowClosed) {
							if Verbose > 0 {
								fmt.Println("GStreamer window closed → shutting down")
							}
							cancel()
						} else {
							fmt.Printf("GStreamer error → canceling context: %v\n", err)
							cancel()
						}
					} else {
						// Normal completion
						cancel()
					}
					shutdownErr <- err
				}
			}()

			// Block until one of the cases happens
			<-shutdownErr

			// ── Graceful shutdown sequence ──────────────────────────────
			if Verbose > 0 {
				fmt.Println("Shutdown initiated — closing data channel")
			}

			// Give frame/image data collection thread a moment to stop
			time.Sleep(300 * time.Millisecond)

			// Close the data channel first — this wakes up any blocked consumer
			close(dataCh)

			// Wait for both components to finish
			var shutdownWg sync.WaitGroup
			shutdownWg.Add(1)
			go func() {
				defer shutdownWg.Done()
				frameCapRes.Wg.Wait()
				// fmt.Println("[SHUTDOWN] Producer WaitGroup done")
			}()

			if captureWg != nil {
				shutdownWg.Add(1)
				go func() {
					defer shutdownWg.Done()
					captureWg.Wait()
					// fmt.Println("[SHUTDOWN] Consumer WaitGroup done")
				}()
			}

			shutdownWg.Wait()
			if Verbose > 0 {
				fmt.Println("All producer/consumer goroutines finished")
			}
		}

		if !viewerNoStop {
			// Disable/Stop the stream
			pktSizeFields := map[string]uint32 {
				"enable": 0,
				"fireTestPkt": 0,
				"maxPkt": maxPkt,
			}

			err = eev.Device.WriteRegFields(viewerSnum + "_MaxPacketSize", pktSizeFields)
			if err != nil {
				fmt.Println(err)
				os.Exit(1)
			}
			fmt.Printf("Stopped %s\n", viewerSnum)

		}
		os.Exit(0)
	},
}

func init() {
	rootCmd.AddCommand(viewerCmd)

	viewerCmd.Flags().StringP("streamNum", "s", "stream0", "Stream number to control/view for stream commands")
	viewerCmd.Flags().StringP("destIP", "i", "", "Stream destination IP address (default \"\", uses LocalIfIP)")
	viewerCmd.Flags().Uint32P("destPort", "p", 0, "Stream destination port. (default 0, uses an OS assigned port number)")
	viewerCmd.Flags().Uint32P("delay", "d", 10, "Delay clocks between stream packets")
	viewerCmd.Flags().Uint32P("maxPacket", "m", 1000, "Maximum stream packet size")
	viewerCmd.Flags().Bool("fps", false, "Display FPS and other stats on stream window")
	// viewerCmd.Flags().Bool("cap", false, "Capture raw frame data and save them in imgPath")
	viewerCmd.Flags().Bool("capJpeg", false, "Capture frames as JPEG images and save them to imagePath")
	viewerCmd.Flags().StringP("imagePath", "f", "./images", "File system location to save captures")
	viewerCmd.Flags().Uint32P("frameCount", "n", 10, "Number of images or frames to capture")
	viewerCmd.Flags().Bool("start", false, "Start stream and exit, sets enable for streamNum")
	viewerCmd.Flags().Bool("stop", false, "Stop stream and exit, clears enable for streamNum")
	viewerCmd.Flags().Bool("noStop", false, "Don't stop stream when window is closed")
	viewerCmd.Flags().Bool("noEEV", false, `Opens a stream window using destIP, destPort, and maxPacket (set them to match the stream).
	Doesn't send EEVideo stream register configuration commands.`)

	viper.BindPFlag("streamNum",  viewerCmd.Flags().Lookup("streamNum"))
	viper.BindPFlag("destIP",     viewerCmd.Flags().Lookup("destIP"))
	viper.BindPFlag("destPort",   viewerCmd.Flags().Lookup("destPort"))
	viper.BindPFlag("delay",      viewerCmd.Flags().Lookup("delay"))
	viper.BindPFlag("maxPacket",  viewerCmd.Flags().Lookup("maxPacket"))
	viper.BindPFlag("imagePath",  viewerCmd.Flags().Lookup("imagePath"))
	viper.BindPFlag("frameCount", viewerCmd.Flags().Lookup("frameCount"))
}

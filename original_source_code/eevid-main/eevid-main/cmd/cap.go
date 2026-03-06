// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package cmd

import (
	"context"
  // "encoding/binary"
	"errors"
	"fmt"
  "net"
	"os"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"gitlab.com/eevideo/goeevideo"
)

// capCmd represents the cap command
var capCmd = &cobra.Command{
	Use:   "cap",
	Short: "EEVideo capture raw images",
	Long:  `Save raw EEVideo stream frames to files`,
	Run: func(cmd *cobra.Command, args []string) {
		verbose      := viper.GetInt("verbose")
		fmt.Printf("cap called %d\n",verbose)
		devicePath   := viper.GetString("devicePath")
		deviceName   := viper.GetString("deviceName")
		stop, _      := cmd.Flags().GetBool("stop")
		frameCount   := viper.GetUint32("frameCount")
		imagePath    := viper.GetString("imagePath")
    streamNum    := viper.GetString("streamNum")
    destPort     := viper.GetUint32("destPort")
    delay        := viper.GetUint32("delay")
    maxPkt       := viper.GetUint32("maxPacket")

		eev.Verbose = verbose    // Set EEV lib verbose level

	if verbose>0 {
			fmt.Printf("Verbose Lvl     : %d\n",verbose);
			fmt.Printf("Frame Count     : %d\n",frameCount);
			fmt.Printf("FilePath        : %s\n",imagePath);
			fmt.Printf("Stream Num      : %s\n",streamNum);
			fmt.Printf("Dest Port       : %d\n",destPort);
			fmt.Printf("Delay           : %d\n",delay);
			fmt.Printf("Max Packet      : %d\n",maxPkt);
	}

		// Load Device Config file
		err := eev.Init(devicePath + "/" + deviceName)
		if err != nil {
			fmt.Printf("Error during Init\n  %v\n", err)
			os.Exit(1)
		}


		// Call stop/disable stream early and exit
		if stop {
			wrErr := eev.Device.StreamStop(streamNum)
			if wrErr != nil {
				fmt.Printf("Error Stopping Stream\n  %v\n", wrErr)
				os.Exit(1)
			}
			fmt.Printf("%s stopped\n", streamNum)
			os.Exit(0)
		}

		streamConn, err := eev.SetupStreamListener(eev.Device.Location.IfIP,
                                           destPort,
                                           eev.Device.Location.IfIP)
		if err != nil {
			fmt.Printf("Error setting up UDP listener\n  %v\n", err)
			os.Exit(1)
		}
		defer streamConn.Close()

    destPort = uint32(streamConn.LocalAddr().(*net.UDPAddr).Port)

		if verbose>0{fmt.Printf("Local Port = %d\n",destPort)}

		// Setup context
		ctx, cancel := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
		defer cancel()

		// Make data channel for Image/Frame data
		dataCh := make(chan *eev.ImageBuffer, 10)


    var maxPktSize int = 256 // TODO: Figure out if additional UDP buffer increase is really needed
    maxPktSize += int(maxPkt)
		// Setup stream registers and start stream
		err = eev.Device.StreamStart(streamNum,
                                eev.Device.Location.IfIP, // Dest IP
                                destPort,
                                delay,
                                maxPkt)

		if err != nil {
			fmt.Printf("Error Starting stream\n  %v\n", err)
			os.Exit(1)
		}

		// Set UDP Buffer size

		// Start collecting frame/image data from UDP
		fmt.Printf("Listening for EEVideo stream on IP:%s:%d\n", eev.Device.Location.IfIP, destPort)
		frameCapRes := eev.FrameCaptureStart(ctx,
                                         streamConn,
                                         maxPktSize,
                                         2 * time.Second,
                                         dataCh)

		// Start thread to save raw images to local location
		rawImgRes := eev.RawImgCaptureStart(ctx, frameCount, imagePath, dataCh)

		// ── Coordination ────────────────────────────────────────────────
		var wg sync.WaitGroup
		errCh  := make(chan error, 2)
		doneCh := make(chan struct{})

		// Producer watcher
		wg.Go(func() {
			err := <-frameCapRes.ErrCh;
			if err != nil && !errors.Is(err, context.Canceled) {
				errCh <- fmt.Errorf("Frame capture failed\n  %v\n", err)
			}
		})

		// Consumer watcher — signal normal completion
		wg.Go(func() {
			err := <-rawImgRes.ErrCh
			if err == nil {
				fmt.Println("Raw capture completed")
				close(doneCh)  // Triggers cancel below
				return
			}
			if !errors.Is(err, context.Canceled) {
				errCh <- fmt.Errorf("Raw capture failed\n  %v\n", err)
			}
		})

		// Shutdown trigger
		wg.Go(func() {
			select {
			case <-ctx.Done():
				return

			case err := <-errCh:
				if verbose > 0 {
					fmt.Printf("Error detected → canceling\n  %v\n", err)
				}
				cancel()

				case <-doneCh:
				if verbose > 0 {
					fmt.Println("Capture target reached → shutting down")
				}
				cancel()
			}
		})

		// Wait for trigger
		<-ctx.Done()

		// Optional: log exit reason (best-effort — ctx.Err() may be nil by now)
		// if verbose > 0 {
		// 	if ctx.Err() != nil {
		// 		fmt.Printf("Shutdown: context canceled\n", ctx.Err())
		// 	}
		// }

		// Graceful shutdown sequence
		if verbose > 0 {
			fmt.Println("Closing data channel")
		}
		close(dataCh)

		// Wait for producer + consumer + trigger goroutine to finish
		wg.Wait()

		// if verbose > 0 {
		// 	fmt.Println("All routines have terminated")
		// }

		// Stop stream
		if err := eev.Device.StreamStop(streamNum); err != nil {
			fmt.Printf("Warning: StreamStop failed\n  %v\n", err)
		} else if verbose > 0 {
			fmt.Printf("Stream %s stopped\n", streamNum)
		}

		os.Exit(0)
	},
}

func init() {
	rootCmd.AddCommand(capCmd)

	capCmd.Flags().Bool(   "stop",            false,      "Stop stream and exit, stops streaming frame data")
  capCmd.Flags().Uint32P("delay",      "d", 0,          "Delay clocks between stream packets")
  capCmd.Flags().Uint32P("maxPacket",  "m", 1000,       "Maximum Stream Packet Size")
  capCmd.Flags().Uint32("destPort", 0,          "Stream destination port")

  viper.BindPFlag("delay",     capCmd.Flags().Lookup("delay"))
  viper.BindPFlag("maxPacket", capCmd.Flags().Lookup("maxPacket"))
  viper.BindPFlag("destPort",  capCmd.Flags().Lookup("destPort"))
}

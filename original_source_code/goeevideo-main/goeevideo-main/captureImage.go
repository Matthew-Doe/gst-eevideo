// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package eev

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sync"
)

// RawImgCaptureStart runs a background consumer routine that saves N raw frames
// Returns WaitGroup that completes when done or canceled
func RawImgCaptureStart(ctx context.Context, frameCount uint32, filePath string, dataCh <-chan *ImageBuffer) RawImgCaptureResult {
	var wg sync.WaitGroup
	errCh := make(chan error, 1)

	if frameCount == 0 {
		close(errCh)
		fmt.Println("Warning EEV RawImgCaptureStart: Number of images to capture set to 0")
		return RawImgCaptureResult{Wg: &wg, ErrCh: errCh}
	}

	wg.Go(func() {
		// defer fmt.Println("Raw capture stopped")

		// Check directory once
		if err := CheckDir(filePath); err != nil {
			errCh <- fmt.Errorf("Info EEV RawImgCaptureStart: Raw capture failed. Invalid directory\n   %w", err)
			return
		}

		captured := uint32(0)

		for captured < frameCount {
			select {
				case <-ctx.Done():
					if Verbose > 0 {
						fmt.Printf("Info EEV RawImgCaptureStart: Raw capture canceled after %d/%d frames\n", captured, frameCount)
					}
					errCh <- ctx.Err()
					return

				case imgBuf, ok := <-dataCh :
					if !ok {
						errCh <- fmt.Errorf("Error EEV RawImgCaptureStart: Data channel closed during raw capture")
						return
					}

					if err := saveRawImage(filePath, imgBuf); err != nil {
						errCh <- fmt.Errorf("Error EEV RawImgCaptureStart: Failed to save raw image\n  %w", err)
						continue // or return if you prefer
					}

					captured++
					if Verbose > 1 {
						fmt.Printf("Info EEV RawImgCaptureStart: Saved raw frame %d/%d\n", captured, frameCount)
					}
      }
		}

		if Verbose > 0 {
			fmt.Println("Info EEV RawImgCaptureStart: Raw capture completed successfully")
		}
		errCh <- nil // ← success signal: time to shut down
	})

	return RawImgCaptureResult{Wg: &wg, ErrCh: errCh}
}

// CheckDir checks if the provided file path directory exists,
// creates it if not, and also checks if it is writable
func CheckDir(outputDir string) error {
	// Create output directory if it doesn't exist
	if err := os.MkdirAll(outputDir, 0755); err != nil && !os.IsExist(err) {
		return fmt.Errorf("Error EEV CheckDir: Failed to create directory %s\n  %w", outputDir, err)
	}

	// Verify directory is writable
	dirInfo, err := os.Stat(outputDir)
	if err != nil {
		return fmt.Errorf("Error EEV CheckDir: Failed to stat directory %s\n  %w", outputDir, err)
	}
	if !dirInfo.IsDir() {
		return fmt.Errorf("Error EEV CheckDir: %s is not a directory", outputDir)
	}
	if dirInfo.Mode().Perm()&0200 == 0 {
		return fmt.Errorf("Error EEV CheckDir: Directory %s is not writable", outputDir)
	}
	return nil
}

// saveImage writes frame data to a file in RAW format
func saveRawImage(outputDir string, imgBuf *ImageBuffer) error {
	filename := filepath.Join(outputDir, fmt.Sprintf("frame_%02d_%dx%d.raw",
							 imgBuf.BlockID, imgBuf.Width, imgBuf.Height))
	file, err := os.Create(filename)
	if err != nil {
		return fmt.Errorf("Error EEV saveRawImage: Failed to create file %s\n  %w", filename, err)
	}
	defer file.Close()

	_, err = file.Write(imgBuf.Data)
	if err != nil {
		return fmt.Errorf("Error EEV saveRawImage: Failed to write image data to %s\n  %w", filename, err)
	}
	if Verbose > 0 {
		fmt.Printf("Info EEV saveRawImage: Saved frame %d to %s\n", imgBuf.BlockID, filename)
	}
	return nil
}

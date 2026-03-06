// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"gitlab.com/eevideo/goeevideo"
	"gopkg.in/yaml.v3"
)

// discCmd represents the disc command
var discCmd = &cobra.Command{
	Use:   "disc",
	Short: "EEVideo Device Discovery",
	Long:  `Discover available EEVideo devices and generate device configuration files`,
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Println("discovery called")

		verb           := viper.GetInt("verb")

		discNicName, _ := cmd.Flags().GetString("nicName")
		discTimeout, _ := cmd.Flags().GetInt("timeout")

		// Set EEV lib verbose level
		eev.Verbose = verb

		deviceList, err := eev.DiscDevices(discNicName, discTimeout)
		if err != nil {
			fmt.Printf("Error during device discovery\n  %v", err)
		}

		for _, device := range deviceList {
			err = createDeviceCfgYAML(&device)
			if err != nil {
				fmt.Printf("Error creating device configuration yaml\n  %v", err)
			}
		}
	},
}

func init() {
	rootCmd.AddCommand(discCmd)

	discCmd.Flags().StringP("nicName", "n", "Unknown nic", "Network Interface ID")
	discCmd.Flags().IntP("timeout", "t", 2, "Discovery wait timeout in seconds")
}

// createDeviceCfgYAML creates a device configuration YAML file for
// the devices that responded to a discovery CoAP message
func createDeviceCfgYAML(device *eev.DeviceType) error {
	devYaml, err := yaml.Marshal(&device)
	if err != nil {
		fmt.Println("Error createDeviceCfgYAML: Marshaling to YAML\n  %w", err)
	}

	// Create output directory if it doesn't exist
	outputDir := "./deviceCfgs"
	if err := os.MkdirAll(outputDir, 0755); err != nil && !os.IsExist(err) {
		return fmt.Errorf("Error createDeviceCfgYAML: Failed to create directory %s\n  %w", outputDir, err)
	}

	// Verify directory is writable
	dirInfo, err := os.Stat(outputDir)
	if err != nil {
		return fmt.Errorf("Error createDeviceCfgYAML: Failed to stat directory %s\n  %w", outputDir, err)
	}
	if !dirInfo.IsDir() {
		return fmt.Errorf("%s is not a directory", outputDir)
	}
	if dirInfo.Mode().Perm()&0200 == 0 {
		return fmt.Errorf("Error createDeviceCfgYAML: Directory %s is not writable", outputDir)
	}

	// Extract last 3 digits from DevIP
	ipParts := strings.SplitN(device.Location.DevIP, ".", 4)
	// fmt.Println("IP Parts ", ipParts)
	if len(ipParts) != 4 {
		return fmt.Errorf("Error createDeviceCfgYAML: Invalid IP address format: %s", device.Location.DevIP)
	}

	// Use simple filename pattern
	filename := fmt.Sprintf("%s_%s_%s_%s.yaml",
		strings.Join(strings.Fields(device.Registers["id0_DeviceModelName"].StrValue), "_"),
		strings.Join(strings.Fields(device.Registers["id0_UserDefinedName"].StrValue), "_"),
		strings.Join(strings.Fields(device.Registers["id0_SerialNumber"].StrValue), "_"),
		ipParts[3])
	filePath := filepath.Join(outputDir, filename)
	absPath, err := filepath.Abs(filePath)
	if err != nil {
		return fmt.Errorf("Error createDeviceCfgYAML: Failed to resolve absolute path for %s\n  %w", filename, err)
	}

	// Create file
	file, err := os.OpenFile(filePath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0644)
	if err != nil {
		if sysErr, ok := err.(*os.PathError); ok {
			if errno, ok := sysErr.Err.(syscall.Errno); ok {
				return fmt.Errorf("Error createDeviceCfgYAML: Failed to open YAML file %s (absolute path: %s)\n  %w (errno: %d)", filePath, absPath, err, errno)
			}
		}
		return fmt.Errorf("Error createDeviceCfgYAML: Failed to open YAML file %s (absolute path: %s)\n  %w", filePath, absPath, err)
	}
	defer file.Close()

	// Write YAML data
	if _, err := file.Write(devYaml); err != nil {
		return fmt.Errorf("Error createDeviceCfgYAML: Failed to write data to YAML file %s (absolute path: %s)\n  %w", filePath, absPath, err)
	}

	// Ensure data is flushed to disk
	if err := file.Sync(); err != nil {
		return fmt.Errorf("Error createDeviceCfgYAML: Failed to sync YAML file %s (absolute path: %s)\n  %w", filePath, absPath, err)
	}

	fmt.Printf("Info createDeviceCfgYAML: Wrote YAML file %s\n", filePath)

	return nil
}

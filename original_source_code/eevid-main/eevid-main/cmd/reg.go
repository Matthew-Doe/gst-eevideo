// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package cmd

import (
	"fmt"
	"os"
	"strconv"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"gitlab.com/eevideo/goeevideo"
)

// regCmd represents the regCmd command
var regCmd = &cobra.Command{
	Use:   "reg",
	Short: "EEVideo Device Register Access",
	Long: `Read and write the EEVideo device's register space` +
		`over Ethernet using Read and Write commands`,
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Println("reg called")

		// ── Establish Arguements and Variables ────────────────────────
		verb         := viper.GetInt("verb")
		devicePath   := viper.GetString("devicePath")
		deviceName   := viper.GetString("deviceName")
		addr, _      := cmd.Flags().GetString("addr")
		val, _       := cmd.Flags().GetString("val")
		tokLength, _ := cmd.Flags().GetString("tokLen")
		regName, _   := cmd.Flags().GetString("rname")
		fVals, _     := cmd.Flags().GetStringToString("fvals")

		// Set EEV lib verbose level
		eev.Verbose = verb

		if len(fVals) > 0 && regName == "" {
			fmt.Println("Error: Register name required for writing fields")
			os.Exit(1)
		}

		fIntVals := make(map[string]uint32, len(fVals))
		for field, valStr := range fVals {
			val, err := strconv.ParseUint(valStr, 0, 32)
			if err != nil {
				fmt.Printf("Error: Invalid value (%s) for field %s\n", valStr, field)
				os.Exit(1)
			}
			fIntVals[field] = uint32(val)
		}

		if tokLength != "" {
			tokLen64, err := strconv.ParseUint(tokLength, 0, 8)
			if err != nil {
				fmt.Printf("Error: Provided token length %s\n  %v\n", tokLength, err)
				os.Exit(1)
			}
			if tokLen64 > 8 || tokLen64 < 0 {
				fmt.Printf("Error: Provided token length %s, isn't a value of 0-8\n", tokLen64)
				os.Exit(1)
			}
			tokLen := uint8(tokLen64)

			// Set Token Length for Read/Write
			eev.SetTokenLen(tokLen)
		}

		var uAddr uint32
		uAddr64, err := strconv.ParseUint(addr, 0, 32)
		if err != nil {
			fmt.Printf("Error: Provided address %s\n  %v\n", addr, err)
			os.Exit(1)
		}
		uAddr = uint32(uAddr64)

		var uVal uint32
		if val != "" {
			uVal64, err := strconv.ParseUint(val, 0, 32)
			if err != nil {
				fmt.Printf("Error: Provided write value %s\n  %v\n", val, err)
				os.Exit(1)
			}
			uVal = uint32(uVal64)
		}

		err = eev.Init(devicePath + "/" + deviceName)
		if err != nil {
			fmt.Printf("Error: Intializing device\n  %v\n", err)
			os.Exit(1)
		}

		// fmt.Printf("fVals Length: %d\n", len(fVals)
		// for key, value := range fVals {
		//   fmt.Printf("fVals Key: %s, Value: %s\n", key, value)
		// }

		// ── Send a Read or a Write (if val is provided) ───────────────
		if val == "" && len(fIntVals) == 0 { // Perform Read
			if regName != "" {
				if eev.Device.Registers[regName].Access == "string" {
					// fmt.Println("ReadStringReg called")
					regStrVal, err := eev.Device.ReadRegString(regName)
					if err != nil {
						fmt.Printf("Error: While calling ReadRegString\n  %v\n", err)
						os.Exit(1)
					} else {
						fmt.Printf("Info: Register %16s is %s\n", regName, regStrVal)
					}
				} else {
					// fmt.Println("ReadReg called")
					regVal, fields, err := eev.Device.ReadReg(regName)
					if err != nil {
						fmt.Printf("Error: While calling ReadReg\n  %v\n", err)
						os.Exit(1)
					} else {
						fmt.Printf("Register %16s = 0x%X\n", regName, regVal)
						fmt.Println("Fields:")
						for fieldName, fieldValue := range fields {
							fmt.Printf("%8s: 0x%X\n", fieldName, fieldValue)
						}
					}
				}
			} else {
				rdData, err := eev.Device.RegReadU32(uAddr)
				if err != nil {
					fmt.Printf("Error: While calling RegReadU32\n  %v\n", err)
					os.Exit(1)
				} else {
					fmt.Printf("Read register 0x%x returned 0x%x\n", uAddr, rdData)
				}
			}
		} else { // Perform Write
			if regName != "" {
				if len(fIntVals) > 0 {
					err := eev.Device.WriteRegFields(regName, fIntVals)
					if err != nil {
						fmt.Printf("Error: While calling WriteRegFields\n  %v\n", err)
						os.Exit(1)
					} else {
						fmt.Printf("Wrote register %s fields\n", regName)
					}
				} else {
					err := eev.Device.WriteReg(regName, uVal)
					if err != nil {
						fmt.Printf("Error: While calling WriteReg\n  %v\n", err)
						os.Exit(1)
					} else {
						fmt.Printf("Wrote register %s to 0x%x\n", regName, uVal)
					}
				}
			} else {
				// fmt.Println("RegWriteU32 called")
				err := eev.Device.RegWriteU32(uAddr, uVal)
				if err != nil {
					fmt.Printf("Error: While calling RegWriteU32\n  %v\n", err)
					os.Exit(1)
				} else {
					fmt.Printf("Wrote register 0x%x to 0x%x\n", uAddr, uVal)
				}
			}
		}
	},
}

func init() {
	rootCmd.AddCommand(regCmd)
	regCmd.Flags().StringP("addr", "a", "0", "Register address (0x for hex) to read/write")
	regCmd.Flags().StringP("tokLen", "l", "", "Token Length (0-8) byte random Token")
	regCmd.Flags().StringP("val", "v", "", "Value to write (0x for hex). Performs a write when provided.")
	regCmd.Flags().StringP("rname", "n", "", "Register Name")
	regCmd.Flags().StringToStringP("fvals", "f", map[string]string{},
		"Field name-value pairs, repeatable or comma-separated (e.g. For rname test0_reg: byte1=1,byte3=0x12)")

}

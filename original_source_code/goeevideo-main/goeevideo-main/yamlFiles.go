// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package eev

import (
	"embed"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"strconv"
	"gopkg.in/yaml.v3"
)

//go:embed yaml/*.yaml
var YamlFiles embed.FS

// ReadDeviceCfgYAML unmarshals/reads a device configuration YAML file
func ReadDeviceCfgYAML(file_loc string) (*DeviceType, error) {
	var deviceCfg DeviceType
	var yamlFilePath string

	// Check if file_loc is a directory
	fileInfo, err := os.Stat(file_loc)
	if err != nil {
		return nil, fmt.Errorf("Error EEV ReadDeviceCfgYAML: Accessing path %s: %w", file_loc, err)
	}

	if strings.HasSuffix(strings.ToLower(file_loc), ".yaml") {
		// Use the provided file path if it ends with .yaml
		yamlFilePath = file_loc
	} else if fileInfo.IsDir() {
		// Read only the provided directory (no subdirectories)
		entries, err := os.ReadDir(file_loc)
		if err != nil {
			return nil, fmt.Errorf("Error EEV ReadDeviceCfgYAML: Reading directory %s\n  %w", file_loc, err)
		}

		// Collect .yaml files
		yamlFiles := []string{}
		for _, entry := range entries {
			if !entry.IsDir() && strings.HasSuffix(strings.ToLower(entry.Name()), ".yaml") {
				yamlFiles = append(yamlFiles, filepath.Join(file_loc, entry.Name()))
			}
		}

		if len(yamlFiles) == 0 {
			return nil, fmt.Errorf("Error EEV ReadDeviceCfgYAML: No YAML file found in directory %s", file_loc)
		}

		if len(yamlFiles) > 1 {
			for cfgNum, cfgName := range yamlFiles {
				fmt.Printf("%d %s\n",cfgNum,cfgName)
			}
			fmt.Println("Enter config number then <Enter>")
	    b := make([]byte, 5)
      n, err := os.Stdin.Read(b)
	    if err != nil {
		    fmt.Println("Error:", err)
		  }
      str := string(b[:n])
      str = strings.Trim(str, "\x00 \t\r\n")  // remove nulls, space, tab, cr, lf
		  if str == "" {
        return nil, fmt.Errorf("No Configuation File Selected\n")
      }

      num, err := strconv.Atoi(str)
      if err != nil {
        return nil, fmt.Errorf("File Number conversion error\n  %w",err)
      } else if num<0 || num>(len(yamlFiles)-1) {
        return nil, fmt.Errorf("Invalid Configuation File Selection %d max %d\n",num,len(yamlFiles))
      }
  		yamlFilePath = yamlFiles[num]
  	} else {
	  	yamlFilePath = yamlFiles[0]
	  }
	} else {
		return nil, fmt.Errorf("Error EEV ReadDeviceCfgYAML: Provided path %s is not a directory or a .yaml file", file_loc)
	}

	// Open the YAML file
	deviceCfgYaml, err := os.OpenFile(yamlFilePath, os.O_RDONLY, 0600)
	if err != nil {
		return nil, fmt.Errorf("Error EEV ReadDeviceCfgYAML: Error opening file %s: %w", yamlFilePath, err)
	}
	defer deviceCfgYaml.Close()

	// Decode the YAML file into a struct
	dec := yaml.NewDecoder(deviceCfgYaml)
	err = dec.Decode(&deviceCfg)
	if err != nil {
		return nil, fmt.Errorf("Error EEV ReadDeviceCfgYAML: Decoding YAML file %s\n  %w", yamlFilePath, err)
	}

	if Verbose > 0 {
		fmt.Println("Info EEV ReadDeviceCfgYAML: Device Config YAML file read successfully:", yamlFilePath)
	}

	return &deviceCfg, nil
}

// Creates Device and Register List YAML files using the eev Master feature
// list and read data from an eev device
func createDeviceConfig(discDevice *DeviceType) error {
	// Load Master Struct of Features
	eevFeats, err := LoadEevFeaturesFromYAML()
	if err != nil {
		return fmt.Errorf("Error EEV createDeviceConfig: Feature list load error\n  %w", err)
	}

	discDevice.Registers = make(map[string]DeviceRegisterAddrType)
	featID := uint32(0)
	var featPtrs []uint32

	// fmt.Printf("  Reading Config Register  ")
	capReg, err := discDevice.RegReadU32(0)
	if err != nil {
		return fmt.Errorf("Error EEV createDeviceConfig: Error reading EEV Capabilities Register\n  %w", err)
	}
	// fmt.Printf("0x%x\n",capReg)
	if capReg>>16 != 0xE71D {
		return fmt.Errorf("Error EEV createDeviceConfig: Device Config does not begin with 0xE71D")
	} else {
		discDevice.Capabilities.DecAvail = (capReg & 0x8000) != 0
		discDevice.Capabilities.MultAddr = (capReg & 0x0800) != 0
		discDevice.Capabilities.StringRd = (capReg & 0x0400) != 0
		discDevice.Capabilities.FifoRd = (capReg & 0x0200) != 0
		discDevice.Capabilities.ReadRst = (capReg & 0x0100) != 0
		discDevice.Capabilities.MaskWr = (capReg & 0x0080) != 0
		discDevice.Capabilities.BitTog = (capReg & 0x0040) != 0
		discDevice.Capabilities.BitSet = (capReg & 0x0020) != 0
		discDevice.Capabilities.BitClear = (capReg & 0x0010) != 0
		discDevice.Capabilities.StaticIP = (capReg & 0x0008) != 0
		discDevice.Capabilities.LinkLocIP = (capReg & 0x0004) != 0
		discDevice.Capabilities.DhcpIP = (capReg & 0x0002) != 0
		discDevice.Capabilities.MultiDisc = (capReg & 0x0001) != 0
	}

	wordIndex := uint32(16) // Start reading after header

	featCounts := make(map[uint32]int,16)
	// fmt.Printf("IP address coming into DeviceConfig %s\n",discDevice.Location.IfIP)

	for featID>>20 != 0xFFF { // until end of features marker has been processed
		// fmt.Println("    Reading Feature ID")
		featID, err = discDevice.RegReadU32(wordIndex)
		if err != nil {
			return fmt.Errorf("Error EEV createDeviceConfig: Create discDevice RegReadU32\n  %w", err)
		}
		wordIndex += 4
		// fmt.Println("    Reading PointerList")
		featPtrs, err = discDevice.readDeviceRegs(wordIndex, (featID & 0xFF))
		if err != nil {
			return fmt.Errorf("Error EEV createDeviceConfig: Create discDevice readDeviceRegs\n  %w", err)
		}
		wordIndex += (featID & 0xFF) << 2

		if featID>>20 == 0xFFF { // End Of Features Marker
			discDevice.Map.LastStatic = featPtrs[0]
			discDevice.Map.FirstMutable = featPtrs[1]
			discDevice.Map.LastMutable = featPtrs[2]
		} else {
			eevFeat := eevFeats[featID>>8] // remove pointer count

			count, countok := featCounts[featID>>8]
			if countok {
				featCounts[featID>>8] = count + 1 // increment
			} else {
				featCounts[featID>>8] = 0 // initialize
			}

			for eevPtrI, eevPtr := range eevFeat.Pointers {
				var eevAddr uint32
				if eevPtrI < len(featPtrs) {
					eevAddr = featPtrs[eevPtrI]
				} else {
					eevAddr = 0
				}
				for eevRegI, eevReg := range eevPtr.Registers {
					var devRegAddr DeviceRegisterAddrType
					devRegAddr.Addr = eevAddr + uint32(eevRegI<<2) // update address field
					devRegAddr.Access = eevReg.Access
					devRegAddr.Fields = eevReg.Fields
					if eevReg.Access == "string" {
						// devReg.StrValue = readDeviceString(int(devRegAddr.Addr))
						devRegAddr.StrValue, err = discDevice.RegReadString(devRegAddr.Addr)
						if err != nil {
							return fmt.Errorf("Error EEV createDeviceConfig: Unable to read StrValue\n  %w", err)
						}
					} else if (devRegAddr.Addr > 0) && (devRegAddr.Addr < 0x40000) {
						devRegAddr.IntValue, err = discDevice.RegReadU32(devRegAddr.Addr)
						if err != nil {
							return fmt.Errorf("Error EEV createDeviceConfig: Unable to read IntValue\n  %w", err)
						}
					}
					devRegName := fmt.Sprintf("%s%d_%s",eevFeat.ShortName,featCounts[featID>>8],eevReg.Name)
					discDevice.Registers[devRegName] = devRegAddr
				}
			}
		} // end of normal feature
	} // end of all device features

	return nil
}

// Load master EEV features list
func LoadEevFeaturesFromYAML() (EevFeaturesType, error) {
	masterData, err := YamlFiles.ReadFile("yaml/EEVideo_Features.yaml")
	if err != nil {
		return nil, fmt.Errorf("Error EEV LoadEevFeaturesFromYAML: Failed to read EEVideo_Features.yaml\n  %w", err)
	}

	var featureList EevFeaturesStrType
	if err := yaml.Unmarshal(masterData, &featureList); err != nil {
		return nil, fmt.Errorf("Error EEV LoadEevFeaturesFromYAML: Failed to unmarshal YAML\n  %w", err)
	}

	// Reformat 0x1234 format map strings to map integers
	features := EevFeaturesType{}
	for key, f := range featureList {
		var id uint32
		if _, err := fmt.Sscanf(key, "0x%x", &id); err != nil {
			return nil, fmt.Errorf("Error EEV LoadEevFeaturesFromYAML: Invalid feature ID key %q\n  %w", key, err)
		}
		features[uint32(id)] = f
	}

	return features, nil
}

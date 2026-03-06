// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"go.starlark.net/starlark"
 	"go.starlark.net/syntax"
 	"go.starlark.net/starlarkstruct"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"gitlab.com/eevideo/eevlark/cmd/builtins"
)

const Revision = "v0.0.4"

var cfgFile string
var verbose int

// Register custom builtins use in all threads
var predeclared = starlark.StringDict{
// Core essentials
  "len"    : starlark.Universe["len"],
  "dict"   : starlark.Universe["dict"],
  "list"   : starlark.Universe["list"],
  "range"  : starlark.Universe["range"],
  "print"  : starlark.Universe["print"],
  "str"    : starlark.Universe["str"],
  "int"    : starlark.Universe["int"],
  "bool"   : starlark.Universe["bool"],
  "type"   : starlark.Universe["type"],
  "hasattr": starlark.Universe["hasattr"],
  "getattr": starlark.Universe["getattr"],
  "hex"    : starlark.Universe["hex"],   // ← for hex()
  "dir"    : starlark.Universe["dir"],   // ← if you ever want to debug  // "struct": starlark.NewBuiltin("struct", starlark.Universe["struct"]),

  "struct"        : starlark.NewBuiltin("struct",         starlarkstruct.Make),

	"sleep"         : starlark.NewBuiltin("sleep",          builtins.Sleep),
  "init_device"   : starlark.NewBuiltin("init_device",    builtins.InitDevice),
	"read_register" : starlark.NewBuiltin("read_register",  builtins.ReadRegister),
  "write_register": starlark.NewBuiltin("write_register", builtins.WriteRegister),
	"write_i2c"     : starlark.NewBuiltin("write_i2c",      builtins.WriteI2C),
	"read_i2c"      : starlark.NewBuiltin("read_i2c",       builtins.ReadI2C),
	"read_i2c_addr" : starlark.NewBuiltin("read_i2c_addr",  builtins.ReadI2cAddr)}

// Set Options to use in all threads
var fileoptions = 	syntax.FileOptions{
	Set              : true,  // allow references to the 'set' built-in function
	While            : true,  // allow 'while' statements
	TopLevelControl  : true,  // allow if/for/while statements at top-level
	GlobalReassign   : true,  // allow reassignment to top-level names
	Recursion        : true,  // disable recursion check for functions in this file
	LoadBindsGlobally: true}  // load creates global not file-local bindings (deprecated)


// Simple filesystem-based loader for loading libraries
func eevLarkLoader(rootDir string) func(thread *starlark.Thread, module string) (starlark.StringDict, error) {
  cache := make(map[string]*starlark.StringDict) // optional: cache loaded modules

  return func(thread *starlark.Thread, module string) (starlark.StringDict, error) {
    if cached, ok := cache[module]; ok {
        return *cached, nil
    }

    filename := filepath.Join(rootDir, module)

    data, err := os.ReadFile(filename)
    if err != nil {
        if os.IsNotExist(err) {
            return nil, fmt.Errorf("module not found: %s", filename)
        }
        return nil, err
    }

    // Create a sub-thread or reuse – usually a fresh one is safer
    subThread := &starlark.Thread{
        Name: "exec " + module,
        Load: thread.Load, // important: chain the same loader!
        Print: thread.Print,
    }

    globals, err := starlark.ExecFileOptions(
    	&fileoptions,
    	subThread,
    	filename,
    	data,
    	predeclared)
    if err != nil {
        return nil, err
    }

    cache[module] = &globals
    return globals, nil
  }
}

// rootCmd represents the base command when called without any subcommands
var rootCmd = &cobra.Command{
	Use:   "eevlark",
	Short: "Starlark Script Interface to eevideo devices",
	Long: `eevlark accepts a text file containing a Python like Starlark syntax
script and executes selected funtions on a Embedded Ethernet
Video Device (camera).  Before commands are issued to the
device, an init_device command must be run declaring the
interface to the target device. Version ` + Revision,
	// Uncomment the following line if your bare application
	// has an action associated with it:
	Run: func(cmd *cobra.Command, args []string) {
		verbose      = viper.GetInt("verbose")
		scriptName  := viper.GetString("scriptName")
		scriptPath  := viper.GetString("scriptPath")
		devicePath  := viper.GetString("devicePath")
		deviceName  := viper.GetString("deviceName")
		imagePath   := viper.GetString("imagePath")
		frameCount  := viper.GetInt("frameCount")
		streamNum   := viper.GetString("streamNum")
		if verbose>0 {fmt.Printf("V%d eevlark called %s/%s for %s/%s\n",verbose,scriptPath,scriptName,devicePath,deviceName)}
		if filepath.Dir(scriptName)=="." {
			scriptName = scriptPath + "/" + scriptName
		}

		// Register custom builtins
		predeclared := starlark.StringDict{
			"verbose"       : starlark.MakeInt(verbose),
			"device_path"   : starlark.String(devicePath),
			"device_name"   : starlark.String(deviceName),
			"image_path"    : starlark.String(imagePath),
			"frame_count"   : starlark.MakeUint64(uint64(frameCount)),
			"stream_num"    : starlark.String(streamNum),
			"sleep"         : starlark.NewBuiltin("sleep",          builtins.Sleep),
			"init_device"   : starlark.NewBuiltin("init_device",    builtins.InitDevice),
			"read_register" : starlark.NewBuiltin("read_register",  builtins.ReadRegister),
			"write_register": starlark.NewBuiltin("write_register", builtins.WriteRegister),
			"write_i2c"     : starlark.NewBuiltin("write_i2c",      builtins.WriteI2C),
			"read_i2c"      : starlark.NewBuiltin("read_i2c",       builtins.ReadI2C),
			"read_i2c_addr" : starlark.NewBuiltin("read_i2c",       builtins.ReadI2cAddr),
			"stream_capture": starlark.NewBuiltin("stream_capture", builtins.StreamCapture),
			"stream_start"  : starlark.NewBuiltin("stream_start",   builtins.StreamStart),
			"stream_stop"   : starlark.NewBuiltin("stream_stop",    builtins.StreamStop),
	  }

	  // Execute Starlark program in a file.
    thread := &starlark.Thread{
    	Name: "eevlark",
      Load: eevLarkLoader(scriptPath), // ← your directory with .star files
      Print: func(_ *starlark.Thread, msg string) { fmt.Println(msg) }}

    _, err := starlark.ExecFileOptions(
    	          &fileoptions, // ← new: pass options (empty is fine for default behavior)
	  	          thread,
	  	          scriptName,            // filename (also for error messages / Load if used)
	  	          nil,                   // src: nil = read from disk
	  	          predeclared)
	  if err != nil {
      if e, ok := err.(*starlark.EvalError); ok {
          fmt.Printf("Script Error:\n%v\nBacktrace:%s\n", e.Error(), e.Backtrace())
      } else {
          fmt.Printf("Script Exec Error: %v\n", err)
      }
    }
	},
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() {
	err := rootCmd.Execute()
	if err != nil {
		os.Exit(1)
	}
}

func init() {
	cobra.OnInitialize(initConfig)
	rootCmd.PersistentFlags().StringVar(&cfgFile, "config", "", "config file (default is $HOME/tecDev.yaml)")
	rootCmd.PersistentFlags().Int("verbose", 0, "verbose output level")
	rootCmd.Flags().StringP("scriptName", "s", "test.py", "Filename of script to run")
	rootCmd.Flags().StringP("scriptPath", "p", "./scripts", "Path to script directory")
	rootCmd.Flags().String("devicePath", "./devices", "Path to directory containing eev device files")
	rootCmd.Flags().String("deviceName", "", "Name of eev device file to use")
	rootCmd.Flags().StringP("imagePath", "f", "./images", "File system location to save captures")
	rootCmd.Flags().IntP("frameCount", "n", 10, "Number of images or frames to capture")
	rootCmd.Flags().String("streamNum", "stream0", "Stream number to control/view for stream commands")

	viper.BindPFlag("verbose",     rootCmd.PersistentFlags().Lookup("verbose"))
	viper.BindPFlag("scriptName",  rootCmd.Flags().Lookup("scriptName"))
	viper.BindPFlag("scriptPath",  rootCmd.Flags().Lookup("scriptPath"))
	viper.BindPFlag("devicePath",  rootCmd.Flags().Lookup("devicePath"))
	viper.BindPFlag("deviceName",  rootCmd.Flags().Lookup("deviceName"))
	viper.BindPFlag("imagePath",   rootCmd.Flags().Lookup("imagePath"))
	viper.BindPFlag("frameCount",  rootCmd.Flags().Lookup("frameCount"))
	viper.BindPFlag("streamNum",   rootCmd.Flags().Lookup("streamNum"))
}

// initConfig reads in config file and ENV variables if set.
func initConfig() {
	if cfgFile != "" {
		// Use config file from the flag.
		viper.SetConfigFile(cfgFile)
	} else {
		// Find home directory.
		home, err := os.UserHomeDir()
		cobra.CheckErr(err)

		// Search config in home directory with name ".eevbuild" (without extension).
		viper.AddConfigPath(home+"/configs")
		viper.AddConfigPath(".")
		viper.SetConfigType("yaml")
		viper.SetConfigName("eevconfig")
	}

	viper.AutomaticEnv() // read in environment variables that match

	// If a config file is found, read it in.
	err := viper.ReadInConfig()
	if err == nil {
		fmt.Println("Using config file:", viper.ConfigFileUsed())
	} else {
		_, ok := err.(viper.ConfigFileNotFoundError)
		if ok {
			fmt.Fprintln(os.Stderr, "No config file found — using defaults + flags")
		} else {
			fmt.Printf("config error: %v", err)
		}
	}
}

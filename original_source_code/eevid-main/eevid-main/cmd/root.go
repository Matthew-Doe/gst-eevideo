// Copyright © 2026 Tecphos
// Use of this source code is governed by a MIT
// license that can be found in the LICENSE file.

package cmd

import (
	"fmt"
	"os"
  "github.com/spf13/cobra"
	"github.com/spf13/viper"
)

const Revision = "v0.0.4"

var cfgFile string

func ErrProc(err error) {
	  if err != nil {
      panic(err)
  }
}

// rootCmd represents the base command when called without any subcommands
var rootCmd = &cobra.Command{
	Use:   "eevid",
	Short: "EEVideo Application, " + Revision,
	Long: `An application to interface with EEVideo devices`,
	// Uncomment the following line if your bare application
	// has an action associated with it:
	// Run: func(cmd *cobra.Command, args []string) { },
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

	rootCmd.Version = "v0.0.4"
	rootCmd.PersistentFlags().StringVar(&cfgFile, "config", "", "config yaml file with path")
	rootCmd.PersistentFlags().Int("verbose", 0, "verbose output level")
	rootCmd.PersistentFlags().StringP("scriptName", "s", "test.py", "Filename of script to run")
	rootCmd.PersistentFlags().StringP("scriptPath", "p", "./scripts", "Path to script directory")
	rootCmd.PersistentFlags().String("devicePath", "./devices", "Path to directory containing eev device files")
	rootCmd.PersistentFlags().String("deviceName", "", "Name of eev device file to use")
	rootCmd.PersistentFlags().StringP("imagePath", "i", "./images", "File system location to save captures")
	rootCmd.PersistentFlags().IntP("frameCount", "c", 10, "Number of images or frames to capture")
	rootCmd.PersistentFlags().String("streamNum", "stream0", "Stream number to control/view for stream commands")

	viper.BindPFlag("verbose",     rootCmd.PersistentFlags().Lookup("verbose"))
	viper.BindPFlag("scriptName",  rootCmd.PersistentFlags().Lookup("scriptName"))
	viper.BindPFlag("scriptPath",  rootCmd.PersistentFlags().Lookup("scriptPath"))
	viper.BindPFlag("devicePath",  rootCmd.PersistentFlags().Lookup("devicePath"))
	viper.BindPFlag("deviceName",  rootCmd.PersistentFlags().Lookup("deviceName"))
	viper.BindPFlag("imagePath",   rootCmd.PersistentFlags().Lookup("imagePath"))
	viper.BindPFlag("frameCount",  rootCmd.PersistentFlags().Lookup("frameCount"))
	viper.BindPFlag("streamNum",   rootCmd.PersistentFlags().Lookup("streamNum"))

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

		// Search config in home directory with name ".gigev" (without extension).
		viper.AddConfigPath(home+"/configs")
		viper.AddConfigPath(".")
		viper.SetConfigName("eevconfig")
		viper.SetConfigType("yaml")
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

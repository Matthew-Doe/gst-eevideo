// Copyright © 2026 Tecphos
// Use of this source code is governed by the MIT
// license in the LICENSE file.

package cmd

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

const Revision = "v0.0.2"

var cfgFile string
var Verbose int

func ErrProc(err error) {
	if err != nil {
		panic(err)
	}
}

// rootCmd represents the base command when called without any subcommands
var rootCmd = &cobra.Command{
	Use:   "eeview",
	Short: "EEVideo Stream Viewer Application",
	Long:  `An application to interface with EEVideo streaming devices, Version ` + Revision,
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

	rootCmd.CompletionOptions.HiddenDefaultCmd = true

	rootCmd.PersistentFlags().StringVar(&cfgFile, "config", "", "config yaml file with path (i.e. eevconfig.yaml)")
	rootCmd.PersistentFlags().IntVar(&Verbose, "verbose", 0, "Verbose reporting level")
	rootCmd.PersistentFlags().String("devicePath", "", "Path to directory containing EEV device files")
	rootCmd.PersistentFlags().String("deviceName", "", "Name of EEV device file to use")

	viper.BindPFlag("verbose",     rootCmd.PersistentFlags().Lookup("verbose"))
	viper.BindPFlag("devicePath",  rootCmd.PersistentFlags().Lookup("devicePath"))
	viper.BindPFlag("deviceName",  rootCmd.PersistentFlags().Lookup("deviceName"))
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

		// Search config in home directory with name "eeview.yaml"
		// viper.AddConfigPath(home)
		viper.AddConfigPath(home + "/configs")
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

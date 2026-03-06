 eeview – **Embedded Ethernet Video Viewer** Stream viewer and configuration CLI

 [![Go Reference](https://pkg.go.dev/badge/gitlab.com/eevideo/goeevideo.svg)](https://pkg.go.dev/gitlab.com/eevideo/goeevideo)
 [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

 Embedded Ethernet Video stream viewer application that displays stream video
 in a GStreamer window. Written in Go using the goEEVideo library.

 ## Features
 - Live video playback using GStreamer pipelines
 - Configure device streaming related registers
 - Capture stream frames and store them as JPEG images
 - Discover EEV devices on the network and generate device files


 ## Build
 Currently, eeview requires building the application locally due to the
 complexities of GStreamer and Operating System dependencies. The system build
 environment will need [Go](https://go.dev/dl/) installed. In Linux, it can
 optionally be installed using Snap. If building in Windows, follow the
 instructions in the [Windows](#windows) section to install Go.<br><br>
 For every system/OS, this project will need to be cloned using git.
 ```
 git clone https://gitlab.com/eevideo/eeview.git
 ```
 Below are additional instructions to set up the build environments for
 each currently supported OS along with building the eeview application.

 ### Linux
 On a Linux system, the OS repository needs to support GStreamer ≥ 1.26
 (e.g. Ubuntu 25.04 or greater). Building GStreamer from source isn't recommended.
 The system needs to have GStreamer developer libraries and some additional
 plugins installed.
 ```
 sudo apt update
 sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libgstreamer-plugins-good1.0-dev libgstreamer-plugins-bad1.0-dev libgstreamer-plugins-ugly1.0-dev gstreamer1.0-libav gstreamer1.0-tools gstreamer1.0-x gstreamer1.0-alsa gstreamer1.0-gl gstreamer1.0-gtk3 gstreamer1.0-pulseaudio
 ```
 Once these packages have been installed, build the eeview application:
 ```
 cd eeview
 CGO_ENABLED=1 go build .
 ```

 ### MacOS
 On a MacOS system, install the latest GStreamer release using Homebrew:
 ```
 brew install gstreamer
 ```
 Once installed, build the eeview application:
 ```
 cd eeview
 CGO_ENABLED=1 go build .
 ```

 ### Windows
 Building the application on a Windows system requires installing the official
 GStreamer release package and a MinGW build environment.<br>

 If needed:
 - Download and install the latest GStreamer
   [MinGW x86_64](https://gstreamer.freedesktop.org/download/#windows) release.
   Ensure the install location is set to the system root folder "C:\gstreamer\1.0\mingw_x86_64".
 - Install [MSYS2](https://www.msys2.org/) to the system root folder, "C:\msys64".


 Open a MSYS2 MINGW64 terminal (it can typically be found in the Start menu).<br>
 Run the command “pacman -Syu” in the terminal window to get the latest package updates.
 Then install Go and build tools for the MYSYS MINGW64 environment.
 ```
 pacman -Syu
 pacman -S mingw-w64-x86_64-toolchain mingw-w64-x86_64-go
 ```
 Once this completes you should be able to use the following commands to check
 the Go environment variables.
 ```
 echo $GOPATH
 echo $GOROOT
 ```
 The expected $GOPATH is "/mingw64/lib/go" and $GOROOT is "C:\Users\your_user_name\go\"<br>


 After the above have been installed you can build the application. Open a
 MSYS2 MINGW64 terminal window, change to the local cloned directory and
 issue environment configuration commands:
 ```
 cd /c/cloned/repository/location/eeview/
 export PKG_CONFIG_PATH="/c/gstreamer/1.0/mingw_x86_64/lib/pkgconfig"
 pkg-config.exe --cflags --libs gstreamer-1.0
 export CGO_CFLAGS="$(pkg-config --cflags gstreamer-1.0)"
 export CGO_LDFLAGS="$(pkg-config --libs gstreamer-1.0)"
 ```
 Now build the eeview application:
 ```
 CGO_ENABLED=1 go build .
 ```

 ## Running the Application
 The eeview application currently supports EEV discovery and stream viewing. It
 can optionally use a configuration file to pass flags/arguments for the commands.
 This project's eevconfig.yaml configuration file can be placed in the same
 folder as the built eeview application or in a folder named "configs"
 located in the current User's Home folder. The latter option may be preferable
 when using other Tecphos EEVideo applications. Modify the eevconfig.yaml file
 for your device and flags/parameters as needed.<br>
 `In Windows, a Firewall rule for Inbound UDP traffic usually needs to be added
 for the eeview application to receive stream packets.`

 ### 1. Discovery
 Use the application's `disc` command to discover any EEV devices
 ```
 ./eeview disc
 ```
 This creates a device file(s) located in subfolder "./devices/(DeviceModelName)\_(UserDefinedName)\_(SerialNumber)_(Last3IP).yaml" by default,
 which stores device information such as capabilities, network info, and a
 register map for each device found.<br>

 ### 2. Viewer
 Use the application's `viewer` command to configure the EEV device and view the
 stream. Provide the device file you want to use after running the disc command
 using the --devicePath and --deviceName flags/arguments or by updating the
 eevconfig.yaml file. If only one device was discovered, it will use that device
 file by default.
 #### Examples
 Run with viewer default flags/parameters:
 ```
 ./eeview viewer
 ```
 (Note: In Windows, you currently need to press ctrl-c in the terminal to cleanly
   stop the application and stream)<br>

Save 50 captured frames as JPEG files for stream number 2, using a max UDP packet
size of 1200 (by default these will be placed in subfolder images/):
```
./eeview.exe viewer -s stream2 -m 1200 --capJpeg --frameCount 50
```

viewer command help:
```
./eeview viewer -h
View an EEVideo Stream in a window

Usage:
./eeview viewer [flags]

Flags:
      --capJpeg             Capture frames as JPEG images and save them to imagePath
  -d, --delay uint32        Delay clocks between stream packets (default 10)
  -i, --destIP string       Stream destination IP address (default "", uses LocalIfIP)
  -p, --destPort uint32     Stream destination port. (default 0, uses an OS assigned port number)
      --fps                 Display FPS and other stats on stream window
  -n, --frameCount uint32   Number of images or frames to capture (default 10)
  -h, --help                help for viewer
  -f, --imagePath string    File system location to save captures (default "./images")
  -m, --maxPacket uint32    Maximum stream packet size (default 1000)
      --noEEV               Opens a stream window using destIP, destPort, and maxPacket (set them to match the stream).
                                Doesn't send EEVideo stream register configuration commands.
      --noStop              Don't stop stream when window is closed
      --start               Start stream and exit, sets enable for streamNum
      --stop                Stop stream and exit, clears enable for streamNum
  -s, --streamNum string    Stream number to control/view for stream commands (default "stream0")

Global Flags:
      --config string       config yaml file with path (i.e. eevconfig.yaml)
      --deviceName string   Name of EEV device file to use
      --devicePath string   Path to directory containing EEV device files
      --verbose int         Verbose reporting level
```

 ## Requirements
 Go ≥ 1.25.1<br>
 UDP access to devices (default port 5683)<br>
 Devices must support EEV register protocol<br>
 Gstreamer ≥ 1.26. See Build Instructions above.

 ## Authors and Acknowledgment
 Tecphos

 ## License
 See LICENSE file.

 ***

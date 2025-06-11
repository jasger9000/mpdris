[![MIT License](https://img.shields.io/badge/License-MIT-green.svg)](https://github.com/jasger9000/mpdris/?tab=MIT-1-ov-file)
[![build](https://github.com/jasger9000/mpdris/actions/workflows/build.yml/badge.svg)](https://github.com/jasger9000/mpdris/actions/workflows/build.yml)
[![GitHub release](https://img.shields.io/github/release/jasger9000/mpdris/all.svg)](https://github.com/jasger9000/mpdris/releases)
[![Issues](https://img.shields.io/github/issues/jasger9000/mpdris.svg)](https://github.com/jasger9000/mpdris/issues)

# mpdris
A lightweight application that implements the media player D-Bus interface [MPRIS](https://wiki.archlinux.org/title/MPRIS) for the [Music Player Daemon (MPD)](https://musicpd.com).


__Table of Contents:__
* [Dependencies](#dependencies)
* [Installation](#installation)
* [Configuration](#configuration)
* [Roadmap](#roadmap)
* [Contributing](#contributing)
* [Licence](#licence)
* [Authors](#authors)


## Dependencies

### Runtime
- D-Bus
- mpd
- libc
- gcc-libs
- (Optional) systemd-libs
  - To be able to use the `--service` flag you need this dependency.<br />
    If your system is using systemd it will already be installed for you

### Compile time
- cargo
- libc
- gcc-libs

## Installation
To install this application, you can either...
- [Use the AUR package (Arch Linux only)](#use-the-aur-package)
- [Build the application yourself](#build-the-application-yourself)
- [Install the application from a release binary](#install-using-release-binary)

### Use the AUR package
> [!IMPORTANT]
> This only works on systems using pacman

The package is available on the AUR or GitHub (see table below)<br />
You can either build the AUR-package yourself, as detailed below, or use your favourite AUR-helper.

**Either way, it is strongly encouraged to read the respective PKGBUILD first.**

| Type                                                        | AUR                                                                                                            | GitHub                                                               |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| [Default](a "Compile yourself from release source tarball") | [![AUR Version](https://img.shields.io/aur/version/mpdris)](https://aur.archlinux.org/packages/mpdris)         | [GitHub Link](https://github.com/jasger9000/mpdris-aur/tree/master)  |
| [Binary](a "Download prebuilt release binaries")            | [![AUR Version](https://img.shields.io/aur/version/mpdris-bin)](https://aur.archlinux.org/packages/mpdris-bin) | [GitHub Link](https://github.com/jasger9000/mpdris-aur/tree/pkg-bin) |
| [Git](a "Download & compile from git source")               | [![AUR Version](https://img.shields.io/aur/version/mpdris-git)](https://aur.archlinux.org/packages/mpdris-git) | [GitHub Link](https://github.com/jasger9000/mpdris-aur/tree/pkg-git) |

#### Build the AUR-package manually
1. Clone the AUR package repository and cd into the directory:
    ```bash
    git clone https://aur.archlinux.org/mpdris.git
    cd mpdris
    ```
2. Run the build & install command:
    ```bash
    makepkg --install --syncdeps
    # or if you want to remove the downloaded dependencies afterwards:
    makepkg --install --syncdeps --rmdeps
    ```
3. Enable the service to start it with MPD
    ```bash
    systemctl --user enable mpdris.service
    ```

### Build the application yourself
1. Clone this repository
    ```bash
    git clone https://github.com/jasger9000/mpdris.git
    ```
2. Build the project with
    ```bash
    cargo build --release
    ```
3. Copy the resulting file from `target/release/mpdris` to `/usr/local/bin`
4. Copy `resources/mpdris.service` to `/usr/local/lib/systemd/user` (You might have to create that directory first)
5. Enable the service to start it with MPD
    ```bash
    systemctl --user enable mpdris.service
    ```

### Install using release binary
2. Download the correct binary for your architecture
1. Go to the [release tab](https://github.com/jasger9000/mpdris/releases)
    - If you don't know what your architecture is, you can find out by running `lscpu`
3. Copy the file to `/usr/local/bin` and rename it to `mpdris`
4. Add the execute permission to the file with
    ```bash
    chmod +x /usr/local/bin/mpdris
    ```
5. Download and move [mpdris.service](https://github.com/jasger9000/mpdris/blob/main/resources/mpdris.service) to `/usr/local/lib/systemd/user`  (You might have to create that directory first)
6. Enable the service to start it with MPD
    ```bash
    systemctl --user enable mpdris.service
    ```

## Configuration
You can configure mpdris using the configuration file or using command-line arguments.
The config file should either be located in `$XDG_CONFIG_HOME/mpdris/mpdris.conf` or `~/.config/mpdris/mpdris.conf`

> [!NOTE]
> While the paths `$XDG_CONFIG_HOME/mpd/mpDris.conf` and `$HOME/mpd/mpDris.conf` still work, they are
> deprecated and may be removed in a future update.

The config file has the following options:
- addr: The IP address mpdris uses to connect to MPD (default: 127.0.0.1)
- port: The port mpdris uses to connect to MPD (default: 6600)
- retries: Defines the amount of times mpdris retries to establish a connection to MPD (default: 3)
- music_directory: The directory in which MPD searches for Music (default: `~/Music`)
- cover_directory: The dedicated directory to where your covers are stored. (default: `~/Music/covers`)

### Covers - cover_directory & music_directory
mpdris will search the configured cover and music directory for image files that correspond to the currently playing song to display as cover art.

Paths will be searched in the following order, trying every extension before moving to the next one:
— $cover_directory/$song_path/$filename.$ext
— $cover_directory/$song_path/$parent_name.$ext
— $music_directory/$song_path/$filename.$ext
— $music_directory/$song_path/cover.$ext

`$ext` can be one of the following values: jpg, jpeg, png, webp, avif, jxl, bmp, gif, heif and heic.<br />
`$song_path` is the file path in the music directory leading up to the current song.<br />
`$parent_name` is the name of the parent directory of the current song. (`$parent_name` is excluded from $song_path when both are used)<br />

#### Example
Let's say you have a user who stores their music in `~/Music` and sets their cover_directory to be in `~/Pictures/songcovers`.
If they now play the song `Resurrections.mp3` located in `~/Music/Celeste`, mpdris will search in `~/Pictures/songcovers/Celeste/` for a file named Resurrections with one of the above-mentioned file extensions.<br />
If it cannot find it there, it will move to the next path.

If the song is located in a subdirectory of the music directory, you can name a cover file the same name as the directory (denoted as `$parent_name` above), and it will be used for every song in that directory.
So sticking with the example from above, mpdris will search for a file named Celeste in cover_directory with one of the above-listed extensions.
If the song was one level deeper, so, for example, `~/Music/some/long/path/Celeste/Resurrections.mp3`, mpdris would look for the cover with this path: `~/Pictures/songcovers/some/long/path/Celeste.$ext`


## Roadmap
- [x] implement base interface
- [x] implement player interface
- [x] add control functionality
- [x] add a manpage
- [ ] add ffmpeg cover support
- [ ] implement tracklist interface



## Contributing
Contributions are always welcome!

If you feel there's something missing/wrong/something that could be improved please open an [issue](https://github.com/jasger9000/mpdris/issues).<br />
Or if you want to add something yourself, just [open a pull request](https://github.com/jasger9000/mpdris/pulls) and I will have a look at it as soon as I can.


## Licence
The Project is Licensed under the [MIT Licence](https://github.com/jasger9000/mpdris/?tab=MIT-1-ov-file)


## Authors
- [@jasger9000](https://www.github.com/jasger9000)

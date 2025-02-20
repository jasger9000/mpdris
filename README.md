[![MIT License](https://img.shields.io/badge/License-MIT-green.svg)](https://github.com/jasger9000/mpDris/?tab=MIT-1-ov-file)
[![build](https://github.com/jasger9000/mpDris/actions/workflows/build.yml/badge.svg)](https://github.com/jasger9000/mpDris/actions/workflows/build.yml)
[![GitHub release](https://img.shields.io/github/release/jasger9000/mpDris/all.svg)](https://github.com/jasger9000/mpDris/releases)
[![Issues](https://img.shields.io/github/issues/jasger9000/mpDris.svg)](https://github.com/jasger9000/mpDris/issues)

# MpDris
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

The package is available on the [AUR](https://aur.archlinux.org/packages/mpdris) or [GitHub](https://github.com/jasger9000/mpDris-aur)<br />
You can either build the AUR-package yourself, as detailed below, or use your favourite AUR-helper.

**Either way, it is strongly encouraged to read the [PKGBUILD](https://aur.archlinux.org/cgit/aur.git/tree/PKGBUILD?h=mpdris) first.**

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
    git clone https://github.com/jasger9000/mpDris
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
1. Go to the [release tab](https://github.com/jasger9000/mpDris/releases)
2. Download the correct binary for your architecture
    - If you don't know what your architecture is, you can find out by running `lscpu`
3. Copy the file to `/usr/local/bin` and rename it to `mpdris`
4. Add the execute permission to the file with
    ```bash
    chmod +x /usr/local/bin/mpdris
    ```
5. Download and move [mpdris.service](https://github.com/jasger9000/mpDris/blob/main/resources/mpdris.service) to `/usr/local/lib/systemd/user`  (You might have to create that directory first)
6. Enable the service to start it with MPD
    ```bash
    systemctl --user enable mpdris.service
    ```

## Configuration
You can configure mpDris using the configuration located at `~/.config/mpd/mpDris.conf` or using command-line arguments.
The config file has the following options:
- addr: The IP address mpDris uses to connect to MPD (default: 127.0.0.1)
- port: The port mpDris uses to connect to MPD (default: 6600)
- retries: Defines the amount of times mpDris retries to establish a connection to MPD (default: 3)
- music_directory: The directory in which MPD searches for Music (default: `~/Music`)
- cover_directory: The dedicated directory to where your covers are stored. (default: `~/Music/covers`)

### cover_directory
This directory will be searched for image files that correspond to the currently playing song to display as cover art.

#### Example:
Let's say you have a user who stores their Music in `~/Music` and set their `cover_directory` to be in `~/Music/Pictures/songcovers`.
If they now play the song `Resurrections.mp3` located in `~/Music/Celeste`,
mpDris will search in `~/Pictures/songcovers/Celeste/` for a file named Resurrections with one of the following file extensions:
`jpg`, `jpeg`, `png`, `webp`, `avif`, `jxl`, `bmp`, `gif`, `heif` and `heic`.

### music_directory
Like cover_directory, this directory can also be used to find covers.
MpDris will search the following paths for song covers, using the first one it finds:
- `$music_directory/$song_path/$filename.$ext`
- `$music_directory/$song_path/cover.$ext`

where `$song_path` the path up to the song from `$music_directory`, `$filename` the underlying filename of the song and
`$ext` one of the image extensions listed under cover_directory.

#### Example
If you have the song `Resurrections.mp3` in `/home/johndoe/Music/Celeste/`, mpDris will search for a cover like this:
- `/home/johndoe/Music/Celeste/Resurrections.jpg`
- `/home/johndoe/Music/Celeste/Resurrections.png`<br />
...
- `/home/johnode/Music/Celeste/cover.jpg`
- `/home/johnode/Music/Celeste/cover.png`<br />
...
- `/home/johndoe/Music/Celeste/cover.heic`

## Roadmap
- [x] implement base interface
- [x] implement player interface
- [x] add control functionality
- [ ] implement tracklist interface



## Contributing
Contributions are always welcome!

If you feel there's something missing/wrong/something that could be improved please open an [issue](https://github.com/jasger9000/mpDris/issues).
Or if you want to add something yourself, just [open a pull request](https://github.com/jasger9000/mpDris/pulls) and I will have a look at it as soon as I can.


## Licence
The Project is Licensed under the [MIT Licence](https://github.com/jasger9000/mpDris/?tab=MIT-1-ov-file)


## Authors
- [@jasger9000](https://www.github.com/jasger9000)

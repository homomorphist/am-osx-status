# am-osx-status

> [!WARNING]
> Unstable; subject to breaking changes and buggy behavior.

macOS Apple Music state observer and dispatcher.

## Features

### Supported Backends
- Discord Rich Presence
- ListenBrainz
- Last.fm

## Installation

No prebuilt binaries are provided [at this time](https://github.com/homomorphist/am-osx-status/issues/52), but instructions for compilation are included below.

### Building

To compile the application with all features enabled, you need [the Rust toolchain](https://rust-lang.org/tools/install/), an active network connection to download dependencies, and approximately 1GB of free disk space.

```sh
cargo build --release
```

This will place the executable in `./target/release/am-osx-status`.

#### Feature Flags

Undesired components of the application can be removed at compilation to result in a more light-weight application.

```sh
# Only support Discord Rich Presence and the usage of Catbox to host custom track artwork.
cargo build --release --no-default-features --features=discord,catbox
```

<details>

<summary>All feature flags</summary>
<br/>

- [`catbox`](https://catbox.moe/): Free file hosting service, used for hosting custom album artwork for the Discord Rich Presence
- `musicdb` Enhanced local metadata extractor (may cause increased memory usage for a large library)

##### Backends
- `lastfm`: LastFM
- `discord`: Discord Rich Presence
- `listenbrainz`: ListenBrainz
</details>

### Relocation

It's recommended that you move the executable to a directory added to `$PATH`, such as `/usr/local/bin`.

If you plan to run the application in the background as a daemon, ensure you choose a location that won't result in you or the system moving the file elsewhere. The service will silently error if the path to the binary is rendered invalid, meaning the application will no longer function.

## Usage

The application can be run in the foreground with `am-osx-status start`.

The first time this is done, you'll be walked through configuring the application.

### Permission Prompts

To minimize unnecessary network requests and read local track artwork, this application reads on-disk metadata written by the native Apple Music app. The first time these actions are performed, the operating system will display a permission prompt pop-up and the process will suspend itself until it is answered. Rejecting these may result in reduced functionality.

### Daemon

A persistent background service can be installed and managed via `am-osx-status service <action>`.

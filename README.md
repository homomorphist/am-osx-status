# am-osx-status

Work-in-progress macOS Apple Music state observer and recorder.

## Features

- last.fm Scrobbler
- ListenBrainz Client
- Discord Rich Presence (w/ support for custom album art)

Configurable[^1] and relatively lightweight.

## Workspace Crates

- [`brainz`](./crates/brainz): Supercrate for working with [MetaBrainz](https://metabrainz.org/) services; very limited in scope
- [`lastfm`](./crates/lastfm/): [last.fm](https://www.last.fm/) API; very limited in scope
- [`maybe_owned_string`](./crates/maybe_owned_string): Enum for a value that's either a `&str` or a `String`
- [`musicdb`](./crates/musicdb/): Apple `musicdb` format reader; currently just limited to `Library.musicdb`
- [`mzstatic`](./crates/mzstatic/): Abstraction over Apple "mzstatic" URLs, which are used to serve album covers among many other things
- [`osascript`](./crates/osascript/): Reusable [osascript](https://ss64.com/mac/osascript.html) process running a REPL so spawning many processes instead isn't needed
- [`plist`](./crates/plist/): Apple [plist](https://en.wikipedia.org/wiki/Property_list) deserializer which supports deserializing into borrowed strings on [serde](https://https://serde.rs/) structs
- [`xml`](./crates/xml): Simple XML parser; limited in functionality

## TODO (and Potential Future Plans)

- Track music while offline to import in bulk
- Track interesting statistics (polls, uptime)
- (Bug) Ensure time is synchronized after skipping while listening.
- (Bug) Handle Discord closing / opening while open.
- Some sort of TUI?

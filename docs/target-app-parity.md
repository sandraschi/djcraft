# DJCraft Feature Parity Matrix

| Area | Status | Notes |
| :--- | :--- | :--- |
| 4-Deck Audio Engine (`cpal` + `symphonia`) | Implemented | Real-time stereo mixing, WSOLA keylock, Catmull-Rom varispeed, dual-bus PFL |
| 3-Band Kill EQ & Bipolar Color Filter | Implemented | LPF/HPF + full-kill EQ per deck |
| 3-Pack FX Rack (12 algorithms) | Implemented | Delay, reverb, filter, flanger, phaser, bitcrush, etc. |
| 4x8 Sampler Bank | Implemented | One-shot and loop modes, cue bus routing |
| Vinyl Scratch & Motorized Platter Sim | Implemented | Interactive jog wheel + beatgrid detection |
| Camelot Harmonic Mixing & Setlist Planner | Implemented | Key compatibility + clash detection |
| 2-Pane Crates & Gig Prep | Implemented | SQLite store, subcrates, drag-drop ordering, `.crate` & `.vdjfolder` interop, per-file autosaved hot cues (`track_cues`) |
| Booth + Crate Split View (`--booth-crate`) | Implemented | Center vertical RGB waveforms, `78px` platters, hideable FX/Sampler/Video strip, lower crate browser defaulting to prepared gig crate |
| 3-Band RGB Spectral Waveforms | Implemented | `<250 Hz` Red, `250-2500 Hz` Green, `>2500 Hz` Cyan with white-hot transient cores, beatgrids, loop bands, cue flags |
| Lossless WAV/FLAC Mix Recorder | Implemented | Streaming mix capture |
| Transparent Sidecar Stems (`.djcraft-stems/`) | Implemented | 4-stem mixer (vocals, drums, bass, other) |
| MIDI Controller Host (`midir`) | Implemented | Built-in profiles + learnable mappings |
| 6 Skins + User JSON Skins | Implemented | Battle Console, Club Dark, Midnight Booth, Carbon Deck, Shocking Pink, Daylight Booth |
| CLI (`djcraft-cli`) & MCP (`djcraft-mcp`) | Implemented | Shared `Command::dispatch` bus |
| Cross-Platform CI (Linux/macOS/Windows) | Implemented | `cpal` + `midir` + `eframe` (`wayland`/`x11`) verified green on `ubuntu-latest`, `macos-latest`, `windows-latest` |
| Video Output & 3-Monitor Booth | Partial | Frame FX + transitions tested; optional UDP external video bridge |

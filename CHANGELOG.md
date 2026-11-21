# Changelog

All notable changes to DJCraft are documented in this file.

## [0.2.2] - 2026-11-22

### Added
- **DVS Timecode Vinyl (`djcraft-audio/src/dvs.rs`, `docks.rs`)**:
  - Added 100% Safe Rust (`#![forbid(unsafe_code)]`) quadrature stereo timecode decoder (`DvsDecoder` for `1000 Hz` / `2000 Hz` control vinyl) and live `cpal` audio input (`DvsInputStream`, `list_input_devices`).
  - Exposes audio input picker, carrier selector, Deck A/B routing, and live `LOCKED / NO SIGNAL` speed/pitch/dBFS telemetry in the MIDI & DVS dock.
- **Pure-Rust UDP Multicast Beat & Tempo Link (`djcraft-core/src/link.rs`, `main.rs`)**:
  - Added clean-room MIT/Apache-2.0 UDP multicast beat/tempo clock (`224.76.78.75:20808`, zero GPL) with `LinkEngine` and `LinkBeacon`, plus a top-bar `LINK` toggle pill showing peer count.
- **Daylight Booth Black Text & Visible Backdrops (`skins.rs`)**:
  - Set `DaylightBooth` `text_primary` to pure black (`#000000`) and `text_secondary` to `#0F1218`.
  - Upgraded `BrushedMetal` and `CarbonWeave` backdrop contrast/alpha so both textures remain clearly visible behind translucent panels.

## [0.2.1] - 2026-11-21

### Added
- **3-Band RGB Spectral Waveforms (`dsp.rs`, `widgets.rs`)**:
  - Upgraded `WaveformSummary` to 3-band spectral peaks (`<250 Hz` Crimson/Red, `250-2500 Hz` Emerald/Lime, `>2500 Hz` Electric Cyan) with white-hot transient cores at `1.5px` slice density, full-lane beatgrid/bar lines, active loop highlights, and colored hot-cue flags across both horizontal and vertical waveforms.
- **Per-File Autosaved Hot Cues (`crates_db.rs`, `main.rs`)**:
  - Added `track_cues` SQLite table in `%LOCALAPPDATA%\djcraft\crates.db` (`CrateStore::set_track_hot_cue`, `CrateStore::get_track_hot_cues`).
  - Hot cues (slots 1-4) are autosaved per audio track path when set and automatically restored whenever that track is loaded onto any deck.
- **Booth + Crate Split Workspace (`--booth-crate`)**:
  - Added `BOOTH + CRATE` workspace mode with center vertical waveforms, small (`78px`) interactive platters, hideable `FX Rack` / `Sampler` / `Video` strip, and lower 2-pane crate browser defaulting to the prepared gig crate.
- **Windows GUI & Cross-Platform Release (`v0.2.1`)**:
  - Added `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` so Windows release launches cleanly without a console window.
  - Removed sibling path dependencies, added `xtask ci` (`fmt`, `clippy`, `test`, `layers`, `assets`), verified cross-platform CI (`ubuntu-latest`, `macos-latest`, `windows-latest`), scrubbed RFC-0029 naming, gated external bridges opt-in, added AccessKit slider metadata + keyboard nudge on rotary knobs, and published `v0.2.1` (Linux/macOS/Windows binaries, `djcraft-0.2.1-setup.exe`, and `SHA256SUMS.txt`).

## [0.2.0] - 2026-11-21

### Added
- **6 Console Skins, Sparkle Club Underlay, 2/4-Deck Layout, Vinyl Scratching & i18n (`skins.rs`, `widgets.rs`, `i18n.rs`, `main.rs`)**:
  - Added **Shocking Pink Club** skin and fixed **Daylight Booth (Light)** to high-contrast deep ink (`#020617` / `#1E293B`).
  - Added 4 textured/dynamic underlays (`Solid`, `Brushed Metal`, `Carbon Weave`, animated `Sparkle Club`).
  - Added `2-DECK` (with `A/C` & `B/D` layer switchers) / `4-DECK` stacked layout toggle, interactive forward/reverse vinyl platter scratching (`-4x`..`+4x`), and 7-language i18n (`EN`/`DE`/`FR`/`ES`/`JA`/`KO`/`ZH`).
  - Fixed Deck B start/sync race via explicit `seek_gen` and end-of-frame `sync_audio_and_repaint()`.
  - Split README into lean root `README.md` + 4 focused guides in `docs/` (`AUDIO_AND_MIXING.md`, `CRATES_AND_GIG_PREP.md`, `SKINS_MIDI_AND_VIDEO.md`, `AI_FLEET_AND_MCP.md`) and expanded `HELP.md` + 8-tab Help Center.

## [0.1.0] - 2026-11-20

### Added
- **L1 Audio & Universal `Command::dispatch` Bus (`djcraft-audio`, `djcraft-core`)**:
  - Enforced `#![forbid(unsafe_code)]` across all workspace crates and eliminated `.unwrap()` from PCM synthesis.
  - Full 4-deck state (`DeckA`-`DeckD`), 4-slot hot cues, beat loops, demo crate library, 64-step Undo/Redo transaction stack, and Camelot `compatible_camelot_keys`.
  - Live Windows WASAPI 4-deck audio output (`cpal 0.15`), equal-power crossfader, soft-clip `tanh` limiter, Master Loudness slider + `MUTE`, and per-deck Volume faders.
  - Pure-Rust `symphonia 0.5` audio decoding (MP3, WAV, FLAC, OGG, M4A/AAC), Catmull-Rom varispeed resampling, WSOLA keylock time-stretching, onset autocorrelation beatgrid detection + `Phase Sync`, real-time 3-band kill EQ (`Low`/`Mid`/`High`), and 10 ms `WaveformSummary` real-audio waveforms.
  - 22-command `Command::catalog()` shared by GUI, CLI, and MCP stdio.
- **Native Video Mixer & Detachable Projector (`crates/djcraft-app/src/video.rs`)**:
  - External `ffmpeg` RGB24 frame-pipe decoder for `.mp4`/`.mkv`/`.mov`/`.webm` with companion video autoload and audio-reactive procedural visual fallback.
  - Crossfader alpha-blend preview + Brightness/Contrast/Saturation controls + detachable secondary projector viewport (`ctx.show_viewport_immediate`).
- **Dual-Bus Headphone PFL Pre-Cue & Lossless WAV/FLAC Mix Recording (`output.rs`, `recorder.rs`)**:
  - Added per-deck Pre-Fader Listen (`PFL A/B/C/D`), `Command::TogglePflCue`, and `Command::SetHeadphoneMix` (24-command L1 bus).
  - Added independent secondary WASAPI Headphone output stream (`AudioOutput::list_output_devices`, `set_headphone_device`) fed via a lock-light ring buffer, plus single-device Split Mono Cue (`L=Cue`, `R=Master`), Cue Volume, and Cue/PGM blend.
  - Added non-blocking live lossless 16-bit PCM WAV & FLAC (`ffmpeg`) master mix recording (`REC MIX` / `Rec As...`) saving to `%USERPROFILE%\Music\DJCraft_Recordings\`.
- **Dedicated 2-Pane Crates & Gig Prep Workspace (`crate_view.rs`, `crates_db.rs`)**:
  - Top bar workspace switch (`LIVE CONSOLE` | `CRATES & GIG PREP`) with full-width 2-pane file-oriented Crate/Subcrate tree (`+ Root`, `+ Sub`, `Rename`, `Duplicate`, `Delete`).
  - Persistent `Gig Order (#)` (`CrateSortField::ManualOrder`), `[::]` drag-drop row reordering, `^`/`v` nudge buttons, drag tracks onto crates, and Windows Explorer audio file/folder drag-and-drop import.
  - Hierarchical crates & subcrates (`%LOCALAPPDATA%\djcraft\crates.db`), Serato DJ binary `.crate` (UTF-16BE TLV + `%%` subcrates) & VirtualDJ `.vdjfolder`/`.m3u8` import/export, folder scanner, multi-field sorting, BPM & Camelot harmonic filtering, and per-track Symphonia/`dettato` analysis.
  - Transparent co-located `.djcraft-stems/<track>/{vocals,drums,bass,other}.wav` batch stemming + automatic real-time 4-stem WAV WASAPI mixing.
- **Pro DJ Console UI, 5 Skins & Hardware MIDI (`skins.rs`, `widgets.rs`, `midi.rs`)**:
  - 5 switchable skins (`Rane Seventy-Two Battle`, `Pioneer Club Dark`, `Serato Midnight`, `VirtualDJ Carbon`, `Daylight Booth`), stacked Parallel Waveforms banner, GPU-painted rotary knobs, vertical channel & pitch faders, MAG FOUR crossfader, and 8 RGB MPC pads per deck.
  - Pure-Rust `midir 0.10` USB MIDI controller host with auto-detect profiles for **Rane Twelve MKII**, **Rane Seventy-Two MKII**, **Pioneer DDJ-FLX4**, and **Hercules DJControl**, plus persistent in-app MIDI Learn.
- **AI Booth Chat, Workflows & Fleet Hub (`fleet_ai.rs`)**:
  - Local Ollama (`:11434`) / LM Studio (`:1234`) `/v1/chat/completions` booth assistant, instant NL command execution, Camelot harmonic setlist builder, DJ vocal drops (`speechcraft-cli` / Windows SAPI -> Deck C), and live connectors for `sfx-mcp` (`:11120`), `vfx-mcp` (`:11122`), and `butterchurn-mcp` (`:10878`).
- **Offline `kobimusic/dettato` Audio-to-MIDI Sidecar**:
  - Subprocess CLI adapter (`Command::AnalyzeTrackDettato`, `djcraft-cli analyze-dettato`, `djcraft_analyze_dettato`) parsing `<stem>.json` tuning cents, instrument tracks, and drum hit counts.
- **Pure-Rust `mixxxxx` Video OSC 1.0 Bridge**:
  - Zero-dependency UDP OSC client (`127.0.0.1:11119` in / `11118` out) supporting `/mixxxxx/ping` and `/deck/{1..4}/video_{enabled,fullscreen,brightness,contrast,saturation}`.
  - Exposed in GUI (`Commands` menu + inline Mixxxxx Video Dock), `djcraft-cli` (`mixxxxx-ping`, `mixxxxx-video`), and `djcraft-mcp` (`djcraft_mixxxxx_ping`, `djcraft_mixxxxx_video`).
- **Tier 4 `egui` 0.37 SOTA Desktop Workstation (`djcraft-app`)**:
  - Universal 8-menu Bookend bar (`File`, `Edit`, `View`, `Insert`, `Modify`, `Commands`, `Window`, `Help`) with `switch_on_hover`.
  - 10-item Help suite: fuzzy `Command Palette...` (`Ctrl+K`), `Keyboard Shortcuts...` (`F1`), 8-tab Help Center synced with `HELP.md`, `Licenses & Notices...`, and `About DJCraft`.
  - Fleet `slate-950` dark + `slate-50` light themes, bumped typography, vector-painted Sun/Moon toggle, `Ctrl+Wheel` zoom + clickable `100%` pill, and `%LOCALAPPDATA%\djcraft\ui.json` persistence.
  - GPU-painted CDJ platters, scrolling dual-frequency waveforms with beatgrid ticks / loop bands / hot-cue pins, vertical stereo VU meters, 12-hour Camelot harmonic wheel, and Decks C & D bottom strip.
  - Mandatory `tests::glyph_allowlist` Tofu Guard test (44 tests total passing, `cargo clippy --all-targets -- -D warnings` clean).
- **Packaging**:
  - Audited and built standalone release binaries (`djcraft.exe`, `djcraft-cli.exe`, `djcraft-mcp.exe`) and per-user NSIS installer (`dist/djcraft-0.1.0-setup.exe`) with `SHA256SUMS.txt` and `BUILD_LOG.md`.

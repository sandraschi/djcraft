# Changelog

All notable changes to DJCraft are documented in this file.

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
- **Persistent SQLite Crate System & Serato / VirtualDJ Interop (`crates_db.rs`)**:
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
  - Mandatory `tests::glyph_allowlist` Tofu Guard test (26 tests total passing, `cargo clippy --all-targets -- -D warnings` clean).
- **Packaging**:
  - Audited and built standalone release binaries (`djcraft.exe`, `djcraft-cli.exe`, `djcraft-mcp.exe`) and per-user NSIS installer (`dist/djcraft-0.1.0-setup.exe`) with `SHA256SUMS.txt` and `BUILD_LOG.md`.

# DJCraft

![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![Rust 1.90+](https://img.shields.io/badge/rust-1.90%2B-orange?logo=rust)
![MCP stdio](https://img.shields.io/badge/MCP-stdio-purple)
![ArtCraft harness](https://img.shields.io/badge/ArtCraft-harness-green)

**DJCraft** is a local-first, 100% Safe Rust (`#![forbid(unsafe_code)]`) 4-deck audio + video DJ workstation, stem mixer, and Camelot harmonic transition engine. It runs locally with zero cloud dependencies, zero telemetry, real-time WASAPI audio output, native FFmpeg video mixing, and shared L1 `Command::dispatch` parity across its `egui` desktop app, CLI, and MCP stdio server.

## Install

Tell Cursor, Claude, Antigravity, or OpenCode: **Install this.**

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\install-craft.ps1
```

Installs the Windows app and the CLI (Start Menu shortcut, CLI on the user PATH). `-RegisterClients` also registers the MCP exe with the IDEs that are installed. `-Clients cursor,claude-desktop` limits that list. `-Mcpb` installs the Claude Desktop extension bundle.

---

## Feature Matrix

| Capability | Status | Notes |
| :--- | :--- | :--- |
| 4-Deck Audio Engine & Live WASAPI Output (`djcraft-audio`) | **Implemented** | Real-time 4-deck `cpal 0.15` WASAPI mixer, equal-power crossfader, soft-clip limiter, Master Loudness + `MUTE`, per-deck Volume faders, built-in 4-stem PCM synth |
| Real Audio File Decode (`symphonia 0.5`) | **Implemented** | Pure-Rust MP3, WAV, FLAC, OGG, M4A/AAC decoding (`LOAD FILE` / `File -> Open Audio File...`) + precomputed 10 ms real-audio waveforms (`WaveformSummary`) |
| Varispeed, WSOLA Keylock & Auto-Beatgrid | **Implemented** | Catmull-Rom cubic Hermite varispeed, pure-Rust WSOLA Master Tempo keylock (`VARISPEED` / `KEYLOCK: ON`), onset autocorrelation BPM + downbeat detector, `/2` / `x2` BPM, `Grid -10ms`/`+10ms`, `Phase Sync` |
| 3-Band Kill EQ (`Low` / `Mid` / `High`) | **Implemented** | Real-time stereo crossover (<250 Hz, 250-2500 Hz, >2500 Hz), `0%`-`200%` sliders, instant `KILL`/`ON` toggles, `FLAT` reset |
| Native Video Mixer & Detachable Projector (`video.rs`) | **Implemented** | Per-deck `.mp4`/`.mkv`/`.mov`/`.webm` via external `ffmpeg` RGB24 pipe, companion video autoload, audio-reactive procedural fallback, crossfader alpha blend + Brightness/Contrast/Saturation, detachable secondary projector window |
| Camelot Harmonic Wheel & Transition Planner | **Implemented** | `parse_camelot`, `is_camelot_compatible`, `compatible_camelot_keys`, 12-hour visual Camelot wheel HUD, BPM delta + key-clash strategy |
| Universal L1 `Command::dispatch` Bus + Undo/Redo | **Implemented** | 22-command catalog, 64-step Undo/Redo, strict L1 clamping, shared by GUI/CLI/MCP |
| `egui` 0.37 Desktop Workstation (`djcraft-app`) | **Implemented** | 8-menu Bookend bar + `switch_on_hover`, `slate-950`/Light themes, vector Sun/Moon, `Ctrl+Wheel` zoom + `100%` pill, painted waveforms/platters/VU/Camelot wheel, Decks C & D strip, `ui.json` prefs, `glyph_allowlist` Tofu Guard |
| Help Suite & Command Palette | **Implemented** | `Ctrl+K` fuzzy Command Palette, `F1` Shortcuts, 8-tab Help Center synced with [HELP.md](HELP.md) |
| Offline `kobimusic/dettato` Audio-to-MIDI Sidecar | **Implemented** | Optional external CLI adapter (`DETTATO_BIN` / `D:\Dev\tools\dettato\.venv\Scripts\dettato.exe`) parsing `.json` tuning/tracks/drums |
| `mixxxxx` Video OSC 1.0 UDP Bridge | **Implemented** | Pure-Rust UDP OSC (`127.0.0.1:11119` in / `11118` out): `/mixxxxx/ping` + `/deck/{1..4}/video_*` + inline Mixxxxx Video Dock |
| MCP stdio Server (`djcraft-mcp`) | **Implemented** | MCP `2024-11-05` stdio, 7 tools (`djcraft_status`, `djcraft_plan_transition`, `djcraft_camelot_check`, `djcraft_commands`, `djcraft_analyze_dettato`, `djcraft_mixxxxx_ping`, `djcraft_mixxxxx_video`) |
| Standalone Release & NSIS Installer | **Implemented** | Per-user NSIS (`%LOCALAPPDATA%\Programs\DjCraft`), `SHA256SUMS.txt`, [BUILD_LOG.md](BUILD_LOG.md) |
| Dual-Bus Headphone PFL Pre-Cue & Mix Recording | **Planned** | Split Master + Headphone cue routing and live WAV/FLAC recording |
| Persistent SQLite Crate Library (`databasecraft-store`) | **Planned** | Demo crate library is pre-loaded in memory; persistent disk crate DB is planned |
| USB MIDI Controller Mapping (Pioneer / Hercules) | **Planned** | Future hardware controller layer (`midir`) |

---

## Workspace Architecture

1. `crates/djcraft-audio`: 4-deck state, `cpal` WASAPI output stream, `symphonia` decoder, Catmull-Rom varispeed, WSOLA keylock, onset autocorrelation beatgrid detector, 3-band kill EQ, 10 ms `WaveformSummary`, and Camelot wheel rules.
2. `crates/djcraft-core`: `DjOrchestrator`, 64-step Undo/Redo, universal `Command::dispatch` bus (22 commands), `dettato` sidecar runner, pure-Rust `mixxxxx` OSC 1.0 UDP client, fleet i18n.
3. `crates/djcraft-mcp`: Line-delimited JSON-RPC 2.0 MCP stdio server (7 tools) over `Command::dispatch`.
4. `crates/djcraft-app`: Native `eframe`/`egui` 0.37 workstation (`djcraft.exe`) with native FFmpeg video mixer + detachable projector viewport, and JSON CLI (`djcraft-cli.exe`).

---

## Verification & Usage

```powershell
# Lint & test suite (34 tests: 33 core/audio/DSP/EQ/OSC + glyph_allowlist Tofu Guard)
cargo clippy --all-targets -- -D warnings
cargo test

# Headless smoke verification
.\target\debug\djcraft.exe --smoke

# CLI commands (JSON output)
.\target\debug\djcraft-cli.exe status
.\target\debug\djcraft-cli.exe transition
.\target\debug\djcraft-cli.exe camelot 8A 9A
.\target\debug\djcraft-cli.exe commands
.\target\debug\djcraft-cli.exe mixxxxx-ping
.\target\debug\djcraft-cli.exe mixxxxx-video --deck 1 --enabled true --brightness 0.8
.\target\debug\djcraft-cli.exe analyze-dettato "D:\audio\track.mp3" --device cuda
```

See [HELP.md](HELP.md) for the full user guide, [CHANGELOG.md](CHANGELOG.md) for release history, [docs/TIERED_PLAN.md](docs/TIERED_PLAN.md) for the 4-tier roadmap, and [BUILD_LOG.md](BUILD_LOG.md) for release build records.

---

## Lineage - the Storytold craft-app pattern

Storytold ArtCraft ([storytold/artcraft](https://github.com/storytold/artcraft) - "the IDE for artists") was the starting point. Each craft app ships headless-first native Rust binaries sharing one L1 command bus across GUI, CLI, and MCP stdio. The code here is independent; the pattern is shared.

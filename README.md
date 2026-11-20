# DJCraft

![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![Rust 1.90+](https://img.shields.io/badge/rust-1.90%2B-orange?logo=rust)
![MCP stdio](https://img.shields.io/badge/MCP-stdio-purple)
![ArtCraft harness](https://img.shields.io/badge/ArtCraft-harness-green)

**DJCraft Pro** is a local-first, 100% Safe Rust (`#![forbid(unsafe_code)]`) 4-deck audio + video DJ workstation, stem mixer, MIDI controller host, and Camelot harmonic performance studio. It runs locally with zero cloud dependencies, real-time WASAPI audio, native FFmpeg video mixing, transparent sidecar stemming, Serato/VirtualDJ crate interoperability, and shared L1 `Command::dispatch` parity across its `egui` desktop app, CLI, and MCP stdio server.

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
| 4-Deck Live WASAPI Audio Engine (`djcraft-audio`) | **Implemented** | Real-time 4-deck `cpal 0.15` WASAPI mixer, equal-power crossfader, soft-clip limiter, Master Loudness + `MUTE`, per-deck Volume faders, built-in 4-stem PCM synth |
| Real Audio Decode, Varispeed, WSOLA Keylock & Auto-Beatgrid | **Implemented** | Pure-Rust `symphonia 0.5` (MP3/WAV/FLAC/OGG/M4A), Catmull-Rom varispeed, WSOLA Master Tempo keylock, onset autocorrelation BPM + downbeat detector, `/2` / `x2`, `Grid -10ms`/`+10ms`, `Phase Sync`, 10 ms real-audio waveforms |
| 3-Band Kill EQ (`Low` / `Mid` / `High`) | **Implemented** | Real-time stereo crossover (<250 Hz, 250-2500 Hz, >2500 Hz) driven by rotary knobs and performance pad kills |
| Transparent Sidecar Stemming (`.djcraft-stems/`) | **Implemented** | Co-located `<parent>/.djcraft-stems/<stem>/{vocals,drums,bass,other}.wav`, hidden from crate scanners, idempotent background `Batch Stem Folder...` (Demucs), real-time 4-stem WAV mixing via VOC/DRM/BAS/OTH rotary knobs |
| Pro DJ Console UI & 5 Switchable Skins (`djcraft-app`) | **Implemented** | 5 skins (Rane Seventy-Two Battle, Pioneer Club Dark, Serato Midnight, VirtualDJ Carbon, Daylight Booth), stacked Parallel Waveforms banner, GPU-painted rotary knobs, vertical channel/pitch faders, MAG FOUR crossfader, 8 RGB MPC pads, stereo VU meters, 8-menu Bookend bar + `switch_on_hover`, `Ctrl+Wheel` zoom, `ui.json` prefs, `glyph_allowlist` Tofu Guard |
| Hardware MIDI Controllers & MIDI Learn (`midir 0.10`) | **Implemented** | USB MIDI port scanner + auto-detect profiles for **Rane Twelve MKII**, **Rane Seventy-Two MKII**, **Pioneer DDJ-FLX4 / DDJ-400**, **Hercules DJControl**, plus in-app MIDI Learn persisted to `midi_learn.json` |
| Persistent SQLite Crates & Serato / VirtualDJ Import-Export | **Implemented** | `%LOCALAPPDATA%\djcraft\crates.db`, unlimited nested subcrates, Serato `.crate` (UTF-16BE TLV + `%%` subcrates), VirtualDJ `.vdjfolder` / `.m3u8`, folder scan, multi-field sorting, BPM/Camelot harmonic filtering, per-track Symphonia & `dettato` analysis |
| Native Video Mixer & Detachable Projector (`video.rs`) | **Implemented** | Per-deck `.mp4`/`.mkv`/`.mov`/`.webm` via `ffmpeg` RGB24 pipe, companion video autoload, audio-reactive procedural fallback, crossfader blend + Brightness/Contrast/Saturation, detachable secondary projector window |
| AI Booth Chat, Workflows & Fleet Hub (`fleet_ai.rs`) | **Implemented** | Local Ollama (`:11434`) / LM Studio (`:1234`) `/v1/chat/completions` chat, instant NL deck commands, Camelot harmonic setlist builder, DJ Vocal Drops (`speechcraft-cli` / Windows SAPI -> Deck C), live connectors for `sfx-mcp` (`:11120`), `vfx-mcp` (`:11122`), and `butterchurn-mcp` (`:10878`) |
| Offline `kobimusic/dettato` & `mixxxxx` OSC 1.0 Bridges | **Implemented** | `dettato` CLI sidecar (`tuning_cents`, tracks, drums) + pure-Rust UDP OSC (`127.0.0.1:11119` in / `11118` out) for `mixxxxx` video decks |
| MCP stdio Server (`djcraft-mcp`) | **Implemented** | MCP `2024-11-05` stdio, 10 tools (`djcraft_status`, `djcraft_plan_transition`, `djcraft_camelot_check`, `djcraft_commands`, `djcraft_analyze_dettato`, `djcraft_mixxxxx_ping`, `djcraft_mixxxxx_video`, `djcraft_crates_list`, `djcraft_crate_import`, `djcraft_crate_export`) |
| Standalone Release & NSIS Installer | **Implemented** | Per-user NSIS (`%LOCALAPPDATA%\Programs\DjCraft`), `SHA256SUMS.txt`, [BUILD_LOG.md](BUILD_LOG.md) |
| Dual-Bus Headphone PFL Pre-Cue & WAV/FLAC Mix Recording | **Planned** | Split Master + Headphone cue channel routing and live lossless mix recording |

---

## Workspace Architecture

1. `crates/djcraft-audio`: 4-deck state, `cpal` WASAPI output stream (full-mix + real 4-stem WAV mixing), `symphonia` decoder, Catmull-Rom varispeed, WSOLA keylock, onset beatgrid detector, 3-band kill EQ, 10 ms `WaveformSummary`, and Camelot wheel rules.
2. `crates/djcraft-core`: `DjOrchestrator`, 64-step Undo/Redo, universal `Command::dispatch` bus (22 commands), SQLite `CrateStore` + Serato/VDJ codecs + transparent `.djcraft-stems/` batch stemmer, `midir` controller engine (`midi.rs`), `fleet_ai.rs` (Ollama/LM Studio, SpeechCraft, SFX, VFX, Butterchurn), `dettato` sidecar, `mixxxxx` OSC 1.0 UDP client, and fleet i18n.
3. `crates/djcraft-mcp`: Line-delimited JSON-RPC 2.0 MCP stdio server (10 tools).
4. `crates/djcraft-app`: Native `eframe`/`egui` 0.37 Pro DJ Console (`djcraft.exe`) with 5 skins, custom GPU hardware widgets, FFmpeg video mixer + detachable projector viewport, and JSON CLI (`djcraft-cli.exe`).

---

## Verification & Usage

```powershell
# Lint & test suite (41 tests: 40 core/audio/DSP/EQ/Crates/MIDI/Stems/AI + glyph_allowlist)
cargo clippy --all-targets -- -D warnings
cargo test

# Headless smoke verification
.\target\debug\djcraft.exe --smoke

# CLI commands (JSON output)
.\target\debug\djcraft-cli.exe status
.\target\debug\djcraft-cli.exe transition
.\target\debug\djcraft-cli.exe camelot 8A 9A
.\target\debug\djcraft-cli.exe crate-list
.\target\debug\djcraft-cli.exe crate-import-serato "D:\_Serato_\Subcrates\House%%Deep.crate"
.\target\debug\djcraft-cli.exe crate-import-vdj "D:\VirtualDJ\Folders\Club.vdjfolder"
.\target\debug\djcraft-cli.exe mixxxxx-ping
.\target\debug\djcraft-cli.exe analyze-dettato "D:\audio\track.mp3" --device cuda
```

See [HELP.md](HELP.md), [CHANGELOG.md](CHANGELOG.md), [docs/TIERED_PLAN.md](docs/TIERED_PLAN.md), and [BUILD_LOG.md](BUILD_LOG.md).

---

## Lineage - the Storytold craft-app pattern

Storytold ArtCraft ([storytold/artcraft](https://github.com/storytold/artcraft) - "the IDE for artists") was the starting point. Each craft app ships headless-first native Rust binaries sharing one L1 command bus across GUI, CLI, and MCP stdio. The code here is independent; the pattern is shared.

# DJCraft

![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![Rust 1.90+](https://img.shields.io/badge/rust-1.90%2B-orange?logo=rust)
![MCP stdio](https://img.shields.io/badge/MCP-stdio-purple)
![ArtCraft harness](https://img.shields.io/badge/ArtCraft-harness-green)

**DJCraft** is a local-first, 100% Safe Rust (`#![forbid(unsafe_code)]`) 4-deck DJ workstation, stem mixer, and Camelot harmonic transition engine. It runs locally with zero cloud dependencies, zero telemetry, and shared L1 `Command::dispatch` parity across its `egui` desktop app, CLI, and MCP stdio server.

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
| 4-Deck State & PCM Synthesizer (`djcraft-audio`) | **Implemented** | Decks A-D, 16-bit stereo software PCM synthesis, 4-stem gain multipliers, hot cues, beat loops |
| Camelot Harmonic Wheel & Transition Planner | **Implemented** | `parse_camelot`, `is_camelot_compatible`, `compatible_camelot_keys`, BPM delta + key-clash strategy |
| Universal L1 `Command::dispatch` Bus + Undo/Redo | **Implemented** | 19-command catalog, 64-step Undo/Redo, strict L1 clamping, shared by GUI/CLI/MCP |
| `egui` 0.37 Desktop Workstation (`djcraft-app`) | **Implemented** | 8-menu Bookend bar + `switch_on_hover`, `slate-950`/Light themes, vector Sun/Moon, `Ctrl+Wheel` zoom, painted waveforms/platters/VU/Camelot wheel, Decks C & D strip, `ui.json` prefs, `glyph_allowlist` Tofu Guard |
| Help Suite & Command Palette | **Implemented** | `Ctrl+K` fuzzy Command Palette, `F1` Shortcuts, 8-tab Help Center synced with [HELP.md](HELP.md) |
| Offline `kobimusic/dettato` Audio-to-MIDI Sidecar | **Implemented** | Optional external CLI adapter (`DETTATO_BIN` / `D:\Dev\tools\dettato\.venv\Scripts\dettato.exe`) parsing `.json` tuning/tracks/drums |
| `mixxxxx` Video OSC 1.0 UDP Bridge | **Implemented** | Pure-Rust UDP OSC (`127.0.0.1:11119` in / `11118` out): `/mixxxxx/ping` + `/deck/{1..4}/video_*` + inline Mixxxxx Video Dock |
| MCP stdio Server (`djcraft-mcp`) | **Implemented** | MCP `2024-11-05` stdio, 7 tools (`djcraft_status`, `djcraft_plan_transition`, `djcraft_camelot_check`, `djcraft_commands`, `djcraft_analyze_dettato`, `djcraft_mixxxxx_ping`, `djcraft_mixxxxx_video`) |
| Standalone Release & NSIS Installer | **Implemented** | Per-user NSIS (`%LOCALAPPDATA%\Programs\DjCraft`), `SHA256SUMS.txt`, [BUILD_LOG.md](BUILD_LOG.md) |
| Live WASAPI / `cpal` Hardware Audio Output | **Planned** | PCM synthesis is implemented; hardware audio device output stream is next |
| Persistent SQLite Crate Library (`databasecraft-store`) | **Planned** | Demo crate library is pre-loaded in memory; persistent disk crate DB is planned |
| USB MIDI Controller Mapping (Pioneer / Hercules) | **Planned** | Future hardware controller layer |

---

## Workspace Architecture

1. `crates/djcraft-audio`: 4-deck state, software PCM synthesizer, stem gains, Camelot wheel rules.
2. `crates/djcraft-core`: `DjOrchestrator`, 64-step Undo/Redo, universal `Command::dispatch` bus, `dettato` sidecar runner, pure-Rust `mixxxxx` OSC 1.0 UDP client, fleet i18n.
3. `crates/djcraft-mcp`: Line-delimited JSON-RPC 2.0 MCP stdio server (7 tools) over `Command::dispatch`.
4. `crates/djcraft-app`: Native `eframe`/`egui` 0.37 workstation (`djcraft.exe`) and JSON CLI (`djcraft-cli.exe`).

---

## Verification & Usage

```powershell
# Lint & test suite (26 tests: 25 core/audio/OSC + glyph_allowlist Tofu Guard)
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

See [HELP.md](HELP.md) for the full user guide, [docs/TIERED_PLAN.md](docs/TIERED_PLAN.md) for the 4-tier roadmap, and [BUILD_LOG.md](BUILD_LOG.md) for release build records.

---

## Lineage - the Storytold craft-app pattern

Storytold ArtCraft ([storytold/artcraft](https://github.com/storytold/artcraft) - "the IDE for artists") was the starting point. Each craft app ships headless-first native Rust binaries sharing one L1 command bus across GUI, CLI, and MCP stdio. The code here is independent; the pattern is shared.

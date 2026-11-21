# DJCraft

![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)
![Rust 1.90+](https://img.shields.io/badge/rust-1.90%2B-orange?logo=rust)
![MCP stdio](https://img.shields.io/badge/MCP-stdio-purple)
![ArtCraft harness](https://img.shields.io/badge/ArtCraft-harness-green)

**DJCraft Pro** is a local-first, 100% Safe Rust (`#![forbid(unsafe_code)]`) 4-deck audio + video DJ workstation, vinyl scratch engine, 4-stem mixer, MIDI controller host, and Camelot harmonic performance studio. GUI (`djcraft.exe`), CLI (`djcraft-cli.exe`), and MCP (`djcraft-mcp.exe`) all share one L1 `Command::dispatch` bus.

---

## Screenshots

Generate fresh local screenshots of all views (`live-console-rane.png`, `live-console-shocking-pink-4deck.png`, `crates-gig-prep.png`, `daylight-booth.png`) into `docs/screenshots/` on your desktop:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\capture-screenshots.ps1
```

---

## Install

Tell Cursor, Claude, Antigravity, or OpenCode: **Install this.**

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\install-craft.ps1
```

Or run the per-user Windows installer (`dist\djcraft-0.2.0-setup.exe`) or standalone `dist\djcraft.exe`.

---

## Feature Matrix & Sub-Guides

| Area | Status | Guide |
| :--- | :--- | :--- |
| 4-Deck WASAPI, Vinyl Scratching, Color Filter, 3-Pack FX Rack, 4x8 Sampler, Dual-Bus PFL, WAV/FLAC Recording & `.djcraft-stems/` | **Implemented** | [docs/AUDIO_AND_MIXING.md](docs/AUDIO_AND_MIXING.md) |
| Dedicated 2-Pane Crates & Gig Prep UI, Drag-Drop Ordering, Serato `.crate` & VirtualDJ Interop | **Implemented** | [docs/CRATES_AND_GIG_PREP.md](docs/CRATES_AND_GIG_PREP.md) |
| 6 Skins (incl. Shocking Pink & Daylight), Sparkle Underlay, 2/4-Deck + Vertical Waveforms, i18n, MIDI (Rane 12/72) & 3-Monitor Video | **Implemented** | [docs/SKINS_MIDI_AND_VIDEO.md](docs/SKINS_MIDI_AND_VIDEO.md) |
| AI Booth Chat (Ollama/LM Studio), Fleet Hub (Speech/SFX/VFX/Butterchurn), CLI & 10-Tool MCP Server | **Implemented** | [docs/AI_FLEET_AND_MCP.md](docs/AI_FLEET_AND_MCP.md) |
| Standalone Release & Per-User NSIS Installer (`SHA256SUMS.txt`) | **Implemented** | [BUILD_LOG.md](BUILD_LOG.md) |

---

## Quick Verification

```powershell
# Lint & test suite (52 tests: 47 core/audio/FX/Crates/MIDI/PFL/Recorder/Scratch/i18n + 3 app/video + 2 sampler)
cargo clippy --all-targets -- -D warnings
cargo test

# Headless smoke verification
.\target\release\djcraft.exe --smoke
```

See also: [HELP.md](HELP.md) | [CHANGELOG.md](CHANGELOG.md) | [docs/TIERED_PLAN.md](docs/TIERED_PLAN.md) | [BUILD_LOG.md](BUILD_LOG.md).

---

## Lineage - the Storytold craft-app pattern

Storytold ArtCraft ([storytold/artcraft](https://github.com/storytold/artcraft) - "the IDE for artists") was the starting point. Each craft app ships headless-first native Rust binaries sharing one L1 command bus across GUI, CLI, and MCP stdio.

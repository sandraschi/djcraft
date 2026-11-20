# Changelog

All notable changes to DJCraft are documented in this file.

## [0.1.0] - 2026-11-20

### Added
- **L1 Audio & Universal `Command::dispatch` Bus (`djcraft-audio`, `djcraft-core`)**:
  - Enforced `#![forbid(unsafe_code)]` across all workspace crates and eliminated `.unwrap()` from PCM synthesis.
  - Full 4-deck state (`DeckA`-`DeckD`), 4-slot hot cues, beat loops, demo crate library, 64-step Undo/Redo transaction stack, and Camelot `compatible_camelot_keys`.
  - 19-command `Command::catalog()` shared by GUI, CLI, and MCP stdio.
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

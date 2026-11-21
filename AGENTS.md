# Rules for agents in Craft repositories

Source: `standards/app-agents.md` in storytold/craft-contrib.

## This repository

- **Workspace**: `djcraft` (pure-Rust 4-deck audio/video DJ workstation)
- **Crates**:
  - `crates/djcraft-audio` - real-time audio engine (`cpal`, `symphonia`), FX, sampler (UI-free)
  - `crates/djcraft-core` - command bus, crates/subcrates SQLite store, MIDI, Camelot, interop (UI-free)
  - `crates/djcraft-app` - `egui`/`eframe` desktop GUI (`djcraft`) + CLI (`djcraft-cli`)
  - `crates/djcraft-mcp` - MCP stdio server (`djcraft-mcp`)
  - `xtask` - `cargo run -p xtask -- ci` (`fmt`, `clippy`, `test`, `layers`, `assets`)
- **Checks**: Run `cargo run -p xtask -- ci` before committing or opening a PR.
- **Safety**: `#![forbid(unsafe_code)]`, zero `unwrap()`/`expect()`/`panic!()` in non-test code.
- **Naming (RFC-0029)**: User-facing skin names and UI labels must stay generic (`docs/feature-names.md`).

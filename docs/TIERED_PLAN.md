# DJCraft — Tiered Execution Plan

**Parent Standard:** [`RUST_ARTCRAFT_STANDARD.md`](../../mcp-central-docs/standards/RUST_ARTCRAFT_STANDARD.md)  
**Menu Bar Standard:** [`CRAFT_MENU_BAR_STANDARD.md`](../../mcp-central-docs/standards/rust-artcraft/CRAFT_MENU_BAR_STANDARD.md)  
**Architecture Model:** 4-Tier ArtCraft Incremental Delivery  
**Status:** Active  

---

## Architecture Flow

```
Tier 1: Core Engine ──> Tier 2: MCP / CLI ──> Tier 3: Desktop UI ──> Tier 4: Menu Bar, Help & Packaging
(djcraft-audio/core)   (djcraft-mcp)         (djcraft-app)         (Hardened Workstation)
```

---

## Tier Specifications

### Tier 1 — Core Audio & Mixing Engine (`djcraft-audio`, `djcraft-core`)
- [x] L1 `Command::dispatch` universal DJ execution bus.
- [x] 4-deck audio buffer management and playback state machine.
- [x] Transient BPM detection and Camelot harmonic key extraction.
- [x] Real-time 4-stem channel routing and isolation faders.
- [x] Crossfader curve mathematics and beat-synced looping.
- [x] `#![forbid(unsafe_code)]` and zero-panic clippy lints.

### Tier 2 — Headless Driver & FastMCP (`djcraft-mcp`)
- [x] Unified CLI `djcraft-cli` with `--json` output.
- [x] FastMCP stdio server communicating over JSON-RPC 2.0.
- [x] Headless transition simulation over fixture tracks.
- [x] Headless `--smoke` verification without window initialization.

### Tier 3 — Desktop Workstation UI (`djcraft-app`)
- [x] Immediate-mode `egui` 0.37 / `wgpu` workstation.
- [x] Scrolling dual-frequency waveform visualizer with beatgrid markers.
- [x] 4-channel mixer console, hot cue pads, and crossfader slider.

### Tier 4 — Craft Menu Bar Standard, Help Suite & Hardened Verification
- [x] Universal 8-menu bar (`File`, `Edit`, `View`, `Insert`, `Modify`, `Commands`, `Window`, `Help`) with `switch_on_hover` and Ctrl+mouse wheel zoom.
- [x] Mandatory 10-item Help suite (`Command Palette...`, `Keyboard Shortcuts...`, 8-tab Help Center + `HELP.md`, `Licenses & Notices...`, `About DJCraft`).
- [x] Fleet UI/UX Pattern 2 adherence: viewport-centric navigation, top button row mapped to Command::dispatch, panel visibility managed via `View` / `Window`.
- [x] Tofu Guard verification (zero emoji/non-BMP glyphs in UI, high-contrast Light/Dark themes).
- [x] Headless `--smoke` parity validation + L1 `Command::dispatch` bus (22 tests passing, clippy `-D warnings` clean, `#![forbid(unsafe_code)]`).
- [x] Offline `kobimusic/dettato` CLI sidecar wired into L1 `Command::AnalyzeTrackDettato`, `djcraft-cli analyze-dettato`, and `djcraft-mcp` `djcraft_analyze_dettato`.
- [x] Pure-Rust Mixxxxx video-extended Mixxx OSC 1.0 UDP bridge (UDP 11119 in / 11118 out) wired into L1 (`Command::MixxxxxPing`, `Command::MixxxxxVideo`), GUI `Commands` menu, `djcraft-cli`, and `djcraft-mcp` (7 MCP tools, 25 tests passing).
- [x] Standalone release binary packaging and NSIS installer (`dist/djcraft-0.1.0-setup.exe`, audited & logged in `BUILD_LOG.md`).

---

### Post-Phase-4 Backlog (Craft Consolidation Freeze)

Deferred until Craft Fleet Consolidation Plan Phase 4 exits:

- [ ] **Offline track analysis via `kobimusic/dettato` CLI sidecar** (`D:\Dev\tools\dettato\.venv\Scripts\dettato.exe`, Apache-2.0, 28.1M params, CUDA <4 GB VRAM):
  - Invoke headless `dettato <track> -o <cache_dir>` (with optional `DETTATO_WEIGHTS` for offline safetensors) during crate ingest/prep, never on the real-time deck audio thread.
  - Consume `<stem>.json` (`tuning_cents`, `tracks`, `drums.hits`, `drums.model`) + `<stem>.mid` to surface A440 tuning-drift warnings, verify Camelot key from transcribed pitch content, and extract drum-hit transient markers.
  - Smoke-tested on Goliath (RTX 4090, `torch 2.14.1+cu126`, `dettato 2.0.0`) on 2026-11-20 (`lofi_cafe.mp3`: 30 s -> electric piano 59 notes, harp 37 notes, Drums 144 hits, tuning +0.61 cents).
  - Shared reference bridge already implemented in `reaper-mcp` (`reaper_mcp/dettato_bridge.py`).

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
- [ ] Universal 8-menu bar (`File`, `Edit`, `View`, `Insert`, `Modify`, `Commands`, `Window`, `Help`) with `switch_on_hover`.
- [ ] Mandatory 5-dialog Help suite (`Command Palette...`, `Keyboard Shortcuts...`, `Project Files & Docs...`, `Licenses & Notices...`, `About DJCraft`).
- [ ] Fleet UI/UX Pattern 2 adherence: viewport-centric navigation, top button row mapped to View toggles, panel visibility managed via `View` / `Window`.
- [ ] Tofu Guard verification (100% font glyph coverage, zero tofu placeholder boxes).
- [ ] Headless `--smoke` parity validation.
- [ ] Standalone release binary packaging and NSIS installer.

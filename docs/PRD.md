# DJCraft — Product Requirements Document (PRD)

**Document Identifier:** `CRAFT-PRD-2026-DJ-001`  
**Version:** 1.0.0  
**Status:** Active  
**Domain:** Local-First DJ Performance, Real-Time Stem Separation, Harmonic Transitions  
**Standard Port:** 14118  
**Parent Standard:** [`RUST_ARTCRAFT_STANDARD.md`](../../mcp-central-docs/standards/RUST_ARTCRAFT_STANDARD.md)  
**Menu Bar Standard:** [`CRAFT_MENU_BAR_STANDARD.md`](../../mcp-central-docs/standards/rust-artcraft/CRAFT_MENU_BAR_STANDARD.md)  

---

## 1. Executive Summary & Problem Statement

Live performance DJ software (Serato, Traktor, Rekordbox, VirtualDJ) is bogged down by expensive subscriptions, cloud lock-in, and unpredictable update churn.

**The DJCraft Solution:**
DJCraft is a local-first, pure-Rust 4-deck DJ workstation and live performance engine:
- Real-time 4-deck audio playback with low-latency WASAPI output.
- Real-time 4-stem separation (Drums, Bass, Vocals, Instruments) for live mashups.
- Harmonic mixing engine with Camelot wheel key detection and automatic BPM beatmatching.
- Hot cue arrays, seamless beat loops, and crossfader curve shaping.
- Zero cloud dependencies, zero telemetry, 100% offline sovereignty.

---

## 2. Core Functional Requirements

1. **Low-Latency Audio Engine (`djcraft-audio`):**
   - High-performance buffer management and resampling.
   - 4-deck playback with pitch bending, scratch physics, and slip mode.
2. **Harmonic Analysis & Stem Separation (`djcraft-core`):**
   - Transient-based BPM detection and beatgrid alignment.
   - Camelot key extraction with compatible key suggestions.
   - Stem isolation channels with dedicated level faders and kill switches.
3. **Universal Command Bus (`Command::dispatch`):**
   - 100% of CLI, MCP, and UI actions dispatch through `djcraft-core::dispatch`.

---

## 3. UI/UX Architecture & Menu Bar Standard

In accordance with [`CRAFT_MENU_BAR_STANDARD.md`](../../mcp-central-docs/standards/rust-artcraft/CRAFT_MENU_BAR_STANDARD.md):
- **Pattern 2 (Viewport-Centric Live Performance Workstation):** Persistent 4-deck turntable / waveform viewport with dual scrolling beatgrids, mixer faders, and stem isolators. Top quick button row acts as accelerators into the `View` menu. To add a new FX unit or sampler dock:
  1. Register a visibility toggle in the `View` or `Window` menu.
  2. Add an entry in the sidebar Decks / Library toolbox.
  3. Wire the action directly into the universal `Command::dispatch` bus.
- **Top Menu Bar Hierarchy:**
  - `File`: New Session / Setlist (`Ctrl+N`), Open Crate / Audio File (`Ctrl+O`), Open Recent ▸, Save Setlist (`Ctrl+S`), Export Mix Recording (`Ctrl+E`), Close (`Ctrl+W`), Quit (`Ctrl+Q`).
  - `Edit`: Undo (`Ctrl+Z`), Redo (`Ctrl+Y`), Cut, Copy, Paste, Select All, Preferences (`Ctrl+,`).
  - `View`: 4-Deck Waveform Viewport, Mixer Console, Stem Isolator Rack, Crate / Track Browser, Camelot Wheel HUD, Theme (`Dark`/`Light`).
  - `Insert`: Load Track to Deck A/B/C/D, Add Hot Cue, Insert Loop Marker, Add Sampler Pad.
  - `Modify`: Adjust BPM / Pitch, Shift Key, Re-align Beatgrid, Toggle Vinyl / CDJ Mode.
  - `Commands`: Auto-Sync Beatgrid, Trigger Transition Macro, Split Stems, Record Master Output.
  - `Window`: Reset Layout, Waveform Viewport, Mixer Drawer, Library Inspector.
  - `Help` (Mandatory 5-Dialog Suite): `Command Palette...` (`Ctrl+K`), `Keyboard Shortcuts...` (`F1`), `Project Files & Docs...`, `Licenses & Notices...`, `About DJCraft`.
- **Behavioral Mechanics:** Smooth `switch_on_hover` between top-level menus, zero-unsafe immediate-mode rendering, and headless `--smoke` parity.

# DJCraft Help & Reference

DJCraft is a local-first, 100% Safe Rust (`#![forbid(unsafe_code)]`) 4-deck DJ workstation, stem mixer, and Camelot harmonic transition engine.

## 1. Get Started

1. **Launch with Demo State**: Run `djcraft.exe` (or `cargo run -p djcraft-app`). Decks A, B, and C start pre-loaded with demo crate tracks (`8A`, `9A`, `8B`).
2. **Explore Decks & Waveforms**: Click `PLAY` / `PAUSE` or `CUE` on any deck (A/B side panels or C/D bottom strip) and watch the scrolling dual-frequency waveform, beatgrid ticks, and stereo VU meters.
3. **Sync & Mix Harmonically**: Use `Commands -> Auto-Sync Beatgrid` (`S`) to pitch-match Deck B to Deck A, and inspect the 12-hour Camelot Harmonic Wheel HUD in the center mixer.
4. **Isolate Stems, Hot Cues & Mixxxxx Video**: Adjust 4-stem sliders (commits to Undo/Redo on drag release), store hot cues (`HOT CUE`), toggle an 8-bar beat loop (`LOOP 8B`), or send video controls to `mixxxxx` from the inline Mixxxxx Video Dock.
5. **Run Headless / CLI / MCP**: Verify parity anytime with `djcraft.exe --smoke`, `djcraft-cli.exe status`, or connect `djcraft-mcp.exe` over MCP stdio.

## 2. Functions

- **4-Deck Audio Engine (`djcraft-audio`)**: Deck A-D state, 16-bit stereo PCM synthesis, 4-stem gain multipliers (Vocals, Drums, Bass, Other), hot cues, beat loops, and Camelot wheel compatibility (`parse_camelot`, `is_camelot_compatible`, `compatible_camelot_keys`).
- **Core Orchestrator & L1 Command Bus (`djcraft-core`)**: Universal `DjOrchestrator::dispatch(Command)` bus (19 commands) with 64-step Undo/Redo history, transition planner, demo crate library, offline `dettato` audio-to-MIDI sidecar adapter, and pure-Rust `mixxxxx` OSC 1.0 UDP client.
- **Desktop Workstation (`djcraft-app`)**: Immediate-mode `egui` 0.37 / `wgpu` UI with the 8-menu Bookend bar (`File`, `Edit`, `View`, `Insert`, `Modify`, `Commands`, `Window`, `Help`), `switch_on_hover`, `slate-950` dark + `slate-50` light themes, vector Sun/Moon toggle, `Ctrl+Wheel` zoom + clickable `100%` pill, `%LOCALAPPDATA%\djcraft\ui.json` persistence, GPU-painted waveforms/platters/VU/Camelot wheel, Decks C & D strip, Mixxxxx Video Dock, and Command Palette (`Ctrl+K`).
- **Headless CLI & MCP Server (`djcraft-cli`, `djcraft-mcp`)**: JSON-formatted CLI and MCP `2024-11-05` stdio server (7 tools) sharing the exact same L1 dispatch bus.

## 3. Usage (GUI, CLI & MCP)

### Desktop GUI
- Open any top menu and glide across the menu bar (`switch_on_hover` switches open menus without a second click).
- Press `Ctrl+K` (or `Ctrl+Shift+P`) to open the fuzzy **Command Palette** and execute any engine command immediately.
- Hold `Ctrl` and scroll the mouse wheel to zoom the interface (`75%` to `200%`), or click the top-right percentage pill (`Ctrl+0`) to reset to `100%`.

### CLI (`djcraft-cli`)
```powershell
djcraft-cli status
djcraft-cli transition
djcraft-cli camelot 8A 9A
djcraft-cli commands
djcraft-cli analyze-dettato "D:\audio\track.mp3" --device cuda
djcraft-cli mixxxxx-ping
djcraft-cli mixxxxx-video --deck 1 --enabled true --fullscreen false --brightness 0.8
```

### MCP (`djcraft-mcp`)
- Transport: stdio (line-delimited JSON-RPC 2.0, MCP `2024-11-05`).
- Tools (7): `djcraft_status`, `djcraft_plan_transition`, `djcraft_camelot_check`, `djcraft_commands`, `djcraft_analyze_dettato`, `djcraft_mixxxxx_ping`, `djcraft_mixxxxx_video`.

## 4. Tips & Tricks

- **Undo Mistakes Mid-Set**: Every deck mutation and slider drag release commits a snapshot onto the 64-step undo stack (`Ctrl+Z` / `Ctrl+Y`).
- **Key Clash Recovery**: When the Transition Assistant flags a key clash, cut melodic stems (`Vocals` + `Other` to `0.0`) and transition over a 4-bar drum loop.
- **Offline Dettato Analysis**: Install `kobimusic/dettato` into `D:\Dev\tools\dettato\.venv` (or set `DETTATO_BIN` and `DETTATO_WEIGHTS`) to extract A440 tuning drift (`tuning_cents`), instrument tracks, and drum hit counts during offline crate prep.
- **Live Video Mixing with Mixxxxx**: Enable OSC in Mixxxxx Preferences (UDP `11119` in / `11118` out; override with `MIXXXXX_OSC_HOST`, `MIXXXXX_OSC_IN_PORT`, `MIXXXXX_OSC_OUT_PORT`) to drive deck video, fullscreen, and VFX sliders directly from DJCraft.

## 5. Common Pitfalls & Safety

- **Never Run Dettato on the Live Audio Thread**: `dettato` is a batch offline PyTorch sidecar (8 s windows). Run it during crate preparation, never during live playback.
- **Audio Rate for Drums**: `dettato` skips drum transcription for source audio below 40 kHz.
- **UDP 11118 Listener Sharing**: If `mixx-dj-mcp` is already bound to UDP `11118`, `mixxxxx-ping` still dispatches `/mixxxxx/ping` to `11119` and reports that `11118` is held by another listener instead of failing.
- **Zero-Unsafe & Zero-Panic**: All crates enforce `#![forbid(unsafe_code)]`; pitch bend (`-16%..+16%`), stem gains (`0.0..1.0`), crossfader (`-1.0..+1.0`), and video VFX (`0.0..1.0`) are strictly clamped at the L1 boundary.

## 6. Planned Extensions

- Real-time WASAPI / `cpal` audio output stream wiring into `AudioEngine::synthesize_deck_audio`.
- Persistent crate indexing with SQLite / `databasecraft-store`.
- MIDI hardware controller mapping (Pioneer DDJ / Hercules DJControl).

## 7. Other Crafts

| Craft / Repo | Role | Handoff |
| :--- | :--- | :--- |
| `mixxxxx` | Video-extended Mixxx fork (FFmpeg + NDI + OSC) | Controlled directly by DJCraft over UDP OSC `11119` (`/mixxxxx/ping`, `/deck/{1..4}/video_*`) |
| `musiccraft` | Docs-only audio and music index | Links `djcraft`, `soundcraft`, and `speechcraft` |
| `soundcraft` | Upstream DAW / audio production fork | Export stems or rendered mixes for `djcraft` crates (never modify upstream directly) |
| `speechcraft` | Pure-Rust speech synthesis and TTS | Generate vocal drops / DJ tags as WAV clips |
| `reaper-mcp` | REAPER DAW automation + `dettato` bridge | Shares the same local `D:\Dev\tools\dettato\.venv` sidecar for audio-to-MIDI workflows |

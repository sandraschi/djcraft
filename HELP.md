# DJCraft Help & Reference

DJCraft is a local-first, 100% Safe Rust (`#![forbid(unsafe_code)]`) 4-deck DJ workstation, stem mixer, and Camelot harmonic transition engine.

## 1. Get Started

1. **Launch with Demo State**: Run `djcraft.exe` (or `cargo run -p djcraft-app`). Deck A, Deck B, and Deck C start pre-loaded with demo crate tracks (`8A`, `9A`, `8B`).
2. **Explore Decks & Crossfader**: Click `PLAY` / `PAUSE` or `CUE` on Deck A or Deck B, and drag the central crossfader (`-1.0` to `+1.0`).
3. **Sync & Mix Harmonically**: Use `Commands -> Auto-Sync Beatgrid` (`S`) to pitch-match Deck B to Deck A, and inspect the Camelot Harmonic Match card in the center mixer.
4. **Isolate Stems & Hot Cues**: Adjust Vocals, Drums, Bass, and Other stem sliders per deck, store hot cues (`Insert -> Add Hot Cue`), or toggle an 8-bar beat loop (`L`).
5. **Run Headless / CLI / MCP**: Verify parity anytime with `djcraft.exe --smoke`, `djcraft-cli.exe status`, or connect `djcraft-mcp.exe` over MCP stdio.

## 2. Functions

- **4-Deck Audio Engine (`djcraft-audio`)**: Deck A-D state, 16-bit stereo PCM synthesis, 4-stem gain multipliers (Vocals, Drums, Bass, Other), hot cues, beat loops, and Camelot wheel compatibility (`parse_camelot`, `is_camelot_compatible`, `compatible_camelot_keys`).
- **Core Orchestrator & L1 Command Bus (`djcraft-core`)**: Universal `DjOrchestrator::dispatch(Command)` bus with 64-step Undo/Redo history, transition planner, demo crate library, and optional offline `dettato` audio-to-MIDI sidecar integration.
- **Desktop Workstation (`djcraft-app`)**: Immediate-mode `egui` 0.37 / `wgpu` UI with the 8-menu Bookend bar (`File`, `Edit`, `View`, `Insert`, `Modify`, `Commands`, `Window`, `Help`), `switch_on_hover`, Ctrl+mouse wheel zoom, high-contrast light/dark themes, and Command Palette (`Ctrl+K`).
- **Headless CLI & MCP Server (`djcraft-cli`, `djcraft-mcp`)**: JSON-formatted CLI and MCP 2024-11-05 stdio server sharing the exact same L1 dispatch bus.

## 3. Usage (GUI, CLI & MCP)

### Desktop GUI
- Open any top menu and glide across the menu bar (`switch_on_hover` switches open menus without a second click).
- Press `Ctrl+K` (or `Ctrl+Shift+P`) to open the fuzzy **Command Palette** and execute any engine command immediately.
- Hold `Ctrl` and scroll the mouse wheel to zoom the interface (`75%` to `200%`), or use `View -> Reset Zoom (100%)`.

### CLI (`djcraft-cli`)
```powershell
djcraft-cli status
djcraft-cli transition
djcraft-cli camelot 8A 9A
djcraft-cli commands
djcraft-cli analyze-dettato "D:\audio\track.mp3" --device cuda
```

### MCP (`djcraft-mcp`)
- Transport: stdio (line-delimited JSON-RPC 2.0, MCP `2024-11-05`).
- Tools: `djcraft_status`, `djcraft_plan_transition`, `djcraft_camelot_check`, `djcraft_commands`, `djcraft_analyze_dettato`.

## 4. Tips & Tricks

- **Undo Mistakes Mid-Set**: Every deck mutation dispatched through the L1 bus pushes a snapshot onto the 64-step undo stack (`Ctrl+Z` / `Ctrl+Y`).
- **Key Clash Recovery**: When the Transition Assistant flags a key clash, cut melodic stems (`Vocals` + `Other` to `0.0`) and transition over a 4-bar drum loop.
- **Offline Dettato Analysis**: Install `kobimusic/dettato` into `D:\Dev\tools\dettato\.venv` (or set `DETTATO_BIN` and `DETTATO_WEIGHTS`) to extract A440 tuning drift (`tuning_cents`), instrument tracks, and drum hit counts during offline crate prep.

## 5. Common Pitfalls & Safety

- **Never Run Dettato on the Live Audio Thread**: `dettato` is a batch offline PyTorch sidecar (8 s windows). Run it during crate preparation, never during live playback.
- **Audio Rate for Drums**: `dettato` skips drum transcription for source audio below 40 kHz.
- **Zero-Unsafe & Zero-Panic**: All crates enforce `#![forbid(unsafe_code)]`; pitch bend (`-16%..+16%`), stem gains (`0.0..1.0`), and crossfader (`-1.0..+1.0`) are strictly clamped at the L1 boundary.

## 6. Planned Extensions

- Real-time WASAPI / `cpal` audio output stream wiring into `AudioEngine::synthesize_deck_audio`.
- Persistent crate indexing with SQLite / `databasecraft-store`.
- MIDI hardware controller mapping (Pioneer DDJ / Hercules DJControl).
- Standalone NSIS installer build and CUA smoke test.

## 7. Other Crafts

| Craft / Repo | Role | Handoff |
| :--- | :--- | :--- |
| `musiccraft` | Docs-only audio and music index | Links `djcraft`, `soundcraft`, and `speechcraft` |
| `soundcraft` | Upstream DAW / audio production fork | Export stems or rendered mixes for `djcraft` crates (never modify upstream directly) |
| `speechcraft` | Pure-Rust speech synthesis and TTS | Generate vocal drops / DJ tags as WAV clips |
| `reaper-mcp` | REAPER DAW automation + `dettato` bridge | Shares the same local `D:\Dev\tools\dettato\.venv` sidecar for audio-to-MIDI workflows |

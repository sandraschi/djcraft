# DJCraft Help & Reference

DJCraft is a local-first, 100% Safe Rust (`#![forbid(unsafe_code)]`) 4-deck audio + video DJ workstation, stem mixer, and Camelot harmonic transition engine.

## 1. Get Started

1. **Launch & Hear Audio**: Run `djcraft.exe` (or `cargo run -p djcraft-app`). Windows WASAPI audio starts automatically (check the green `WASAPI:` indicator and **Master Loudness** slider in the center mixer). Decks A, B, and C start pre-loaded with demo crate tracks (`8A`, `9A`, `8B`).
2. **Load Your Own Audio & Video**: Click `LOAD FILE` (or `File -> Open Audio File...`) to load any MP3, WAV, FLAC, OGG, or M4A track. DJCraft decodes it via `symphonia`, auto-detects its BPM & downbeat beatgrid, renders its real waveform, and auto-loads any companion `.mp4`/`.mkv`/`.mov`/`.webm` with the same basename.
3. **Tempo, Keylock & Phase Sync**: Drag the per-deck **Tempo** slider (`-16%..+16%`), switch between `VARISPEED` (vinyl pitch shift) and `KEYLOCK: ON` (WSOLA Master Tempo), nudge the beatgrid (`Grid -10ms`/`+10ms`), or click `Phase Sync` to lock Deck B onto Deck A's BPM and beat phase.
4. **Mix Audio & Video**: Cut basslines with the **3-Band Kill EQ** (`Low`/`Mid`/`High` + `KILL` toggles), adjust the 4-stem sliders and **Master Crossfader**, and click **`Detach Projector`** in the Native Video Mixer card to open a secondary monitor / projector output window.
5. **Run Headless / CLI / MCP**: Verify parity anytime with `djcraft.exe --smoke`, `djcraft-cli.exe status`, or connect `djcraft-mcp.exe` over MCP stdio.

## 2. Functions

- **4-Deck Audio Engine & Live WASAPI (`djcraft-audio`)**: Real-time `cpal` WASAPI 4-deck mixer, equal-power crossfader, soft-clip limiter, Master Loudness + Mute, `symphonia` file decoder, Catmull-Rom varispeed, WSOLA keylock time-stretcher, onset autocorrelation beatgrid detector, 3-band stereo kill EQ, 10 ms `WaveformSummary`, and Camelot wheel compatibility.
- **Core Orchestrator & L1 Command Bus (`djcraft-core`)**: Universal `DjOrchestrator::dispatch(Command)` bus (22 commands) with 64-step Undo/Redo history, transition planner, demo crate library, offline `dettato` audio-to-MIDI sidecar adapter, and pure-Rust `mixxxxx` OSC 1.0 UDP client.
- **Desktop Workstation & Native Video Mixer (`djcraft-app`)**: Immediate-mode `egui` 0.37 / `wgpu` UI with the 8-menu Bookend bar (`File`, `Edit`, `View`, `Insert`, `Modify`, `Commands`, `Window`, `Help`), `switch_on_hover`, `slate-950` dark + `slate-50` light themes, vector Sun/Moon toggle, `Ctrl+Wheel` zoom + `100%` pill, `%LOCALAPPDATA%\djcraft\ui.json` persistence, GPU-painted waveforms/platters/VU/Camelot wheel, Decks C & D strip, FFmpeg native video mixer + detachable projector viewport, Mixxxxx Video Dock, and Command Palette (`Ctrl+K`).
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

- **Undo Mistakes Mid-Set**: Every deck mutation, EQ kill, and slider drag release commits a snapshot onto the 64-step undo stack (`Ctrl+Z` / `Ctrl+Y`).
- **Clean Bass Swaps**: Kill `Low` on the incoming track (`KILL`), bring up the crossfader, then swap `Low` kills on bar 16.
- **Companion Video Autoload**: Place `song.mp4` beside `song.mp3` in the same folder; loading `song.mp3` automatically attaches `song.mp4` to that deck's native video player.
- **Offline Dettato Analysis**: Install `kobimusic/dettato` into `D:\Dev\tools\dettato\.venv` (or set `DETTATO_BIN` and `DETTATO_WEIGHTS`) to extract A440 tuning drift (`tuning_cents`), instrument tracks, and drum hit counts during offline crate prep.
- **Live Video Control with Mixxxxx**: Enable OSC in Mixxxxx Preferences (UDP `11119` in / `11118` out) to drive Mixxxxx deck video and VFX from the Mixxxxx Video Dock or MCP.

## 5. Common Pitfalls & Safety

- **FFmpeg on PATH for Native Video**: Native video file decoding pipes raw RGB24 frames from `ffmpeg` on PATH; if no video file is loaded (or `ffmpeg` is absent), DJCraft falls back gracefully to its built-in audio-reactive procedural visuals.
- **Never Run Dettato on the Live Audio Thread**: `dettato` is a batch offline PyTorch sidecar (8 s windows). Run it during crate preparation, never during live playback.
- **Audio Rate for Drums**: `dettato` skips drum transcription for source audio below 40 kHz.
- **Zero-Unsafe & Zero-Panic**: All workspace crates enforce `#![forbid(unsafe_code)]`; pitch bend (`-16%..+16%`), EQ (`0%..200%`), stem gains (`0%..100%`), Master Loudness (`0%..100%`), and crossfader (`-1.0..+1.0`) are strictly clamped at the L1 boundary.

## 6. Planned Extensions

- Dual-bus Master + Headphone PFL pre-cue audio routing and live WAV/FLAC mix recording.
- Persistent crate indexing with SQLite / `databasecraft-store`.
- USB MIDI hardware controller mapping (`midir`, Pioneer DDJ / Hercules DJControl).

## 7. Other Crafts

| Craft / Repo | Role | Handoff |
| :--- | :--- | :--- |
| `mixxxxx` | Video-extended Mixxx fork (FFmpeg + NDI + OSC) | Controlled directly by DJCraft over UDP OSC `11119` (`/mixxxxx/ping`, `/deck/{1..4}/video_*`) |
| `musiccraft` | Docs-only audio and music index | Links `djcraft`, `soundcraft`, and `speechcraft` |
| `soundcraft` | Upstream DAW / audio production fork | Export stems or rendered mixes for `djcraft` crates (never modify upstream directly) |
| `speechcraft` | Pure-Rust speech synthesis and TTS | Generate vocal drops / DJ tags as WAV clips |
| `reaper-mcp` | REAPER DAW automation + `dettato` bridge | Shares the same local `D:\Dev\tools\dettato\.venv` sidecar for audio-to-MIDI workflows |

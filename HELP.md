# DJCraft Pro Help & Reference

DJCraft Pro is a local-first, 100% Safe Rust (`#![forbid(unsafe_code)]`) 4-deck audio + video DJ workstation, stem mixer, MIDI controller host, and Camelot harmonic transition engine.

## 1. Get Started

1. **Launch & Choose Your Skin**: Run `djcraft.exe`. Windows WASAPI audio starts live automatically (see the green `WASAPI:` indicator in the Battle Mixer). Pick any of the 5 skins in the top bar (`Rane Seventy-Two Battle`, `Pioneer Club Dark`, `Serato Midnight`, `VirtualDJ Carbon`, `Daylight Booth`).
2. **Prepare Gigs in the Dedicated Crates Workspace**: Click `CRATES & GIG PREP` in the top bar for a full-width 2-pane file-oriented workspace: build nested crate/subcrate trees (`+ Root`, `+ Sub`, `Rename`, `Duplicate`, `Delete`), drag audio files or folders directly from Windows Explorer onto the window, reorder tracks in persistent `Gig Order (#)` via `[::]` drag handles or `^`/`v`, drag tracks onto any crate in the tree, and import/export **Serato DJ `.crate`** or **VirtualDJ `.vdjfolder` & `.m3u8`**.
3. **Mix Tactile Hardware Controls & Headphone PFL Pre-Cue**: Drag rotary High/Mid/Low EQ knobs, rotary 4-stem knobs (`VOC`/`DRM`/`BAS`/`OTH`), vertical channel & pitch faders, the MAG FOUR crossfader, and the 8 RGB MPC pads (`CUE 1-4`, `LOOP 4`, `LOOP 8`, `KILL B`, `KILL V`). Double-click any knob or fader to snap to neutral. Click `PFL A`/`B`/`C`/`D` to pre-cue any deck pre-fader on a secondary WASAPI headphone device (or toggle `SPLIT: ON` for single-DAC L=Cue / R=Master monitoring), and hit `REC MIX` (`WAV` or `FLAC`) in the Battle Mixer to record live lossless sets.
4. **Transparent Sidecar Stems & Video**: Click `Batch Stem Crate` or `Batch Stem Folder...` to generate `.djcraft-stems/<track>/{vocals,drums,bass,other}.wav` beside your music files - DJCraft loads them automatically for real-time 4-stem knob isolation. Open the **Video Mixer** tab and click `Detach Projector` for a secondary monitor video output.
5. **Hardware MIDI, AI Booth & Fleet Hub**: Plug in a **Rane Twelve MKII**, **Rane Seventy-Two MKII**, Pioneer DDJ-FLX4, or Hercules DJControl in the **MIDI (Rane 12/72)** tab; run natural-language commands or harmonic setlists in **AI Chat & Workflows**; fire DJ vocal drops and Butterchurn/SFX/VFX triggers in **Fleet Hub**.

## 2. Functions

- **Audio, Dual-Bus PFL & Mix Recorder (`djcraft-audio`)**: Live `cpal` WASAPI 4-deck mixer, dedicated secondary WASAPI Headphone PFL Cue output + single-device Split Mono Cue, non-blocking live lossless 16-bit PCM WAV / FLAC (`ffmpeg`) mix recorder (`recorder.rs`), `symphonia` decoder (MP3/WAV/FLAC/OGG/M4A), Catmull-Rom varispeed, WSOLA keylock, onset beatgrid detector, 3-band stereo kill EQ, 10 ms real-audio waveforms, and real-time 4-stem WAV mixing from `.djcraft-stems/`.
- **Crates & Gig Prep (`crates_db.rs`, `crate_view.rs`)**: Persistent SQLite `%LOCALAPPDATA%\djcraft\crates.db` with full Crate/Subcrate CRUD (`+ Root`, `+ Sub`, `Rename` with recursive descendant path rewrite, `Duplicate` preserving manual order, `Delete`), `[::]` drag-drop manual gig ordering, drag-to-crate, OS Explorer file/folder drop, Serato `.crate` (`%%` subcrates) & VirtualDJ `.vdjfolder`/`.m3u8` import/export, sorting, BPM/Camelot filtering, and per-track Symphonia/`dettato` analysis.
- **MIDI Engine (`midi.rs`)**: Pure-Rust `midir` USB MIDI host with auto-detected profiles for Rane Twelve MKII, Rane Seventy-Two MKII, Pioneer DDJ-FLX4, and Hercules DJControl, plus persistent in-app MIDI Learn (`midi_learn.json`).
- **AI & Fleet Hub (`fleet_ai.rs`)**: Local Ollama/LM Studio `/v1/chat/completions` booth assistant, instant local NL command execution, Camelot setlist builder, DJ vocal drops (`speechcraft-cli` / Windows SAPI -> Deck C), and live connectors for `sfx-mcp` (`:11120`), `vfx-mcp` (`:11122`), and `butterchurn-mcp` (`:10878`).

## 3. Usage (GUI, CLI & MCP)

### Desktop GUI
- Use the top **Parallel Waveforms** banner to visually align Deck A and Deck B beatgrid bars.
- Double-click any rotary knob or vertical fader to reset it; press `Ctrl+Z`/`Ctrl+Y` to undo/redo.
- Press `Ctrl+K` for the Command Palette, `F1` for Shortcuts, and `Ctrl+Wheel` (`Ctrl+0` reset) to zoom.

### CLI (`djcraft-cli`)
```powershell
djcraft-cli status
djcraft-cli transition
djcraft-cli camelot 8A 9A
djcraft-cli crate-list
djcraft-cli crate-import-serato "D:\_Serato_\Subcrates\House%%Deep.crate"
djcraft-cli crate-export-serato 1 "D:\export.crate"
djcraft-cli crate-import-vdj "D:\VirtualDJ\Folders\Club.vdjfolder"
djcraft-cli crate-export-vdj 1 "D:\export.vdjfolder"
djcraft-cli analyze-dettato "D:\audio\track.mp3" --device cuda
djcraft-cli mixxxxx-ping
```

### MCP (`djcraft-mcp`)
- Transport: stdio (JSON-RPC 2.0, MCP `2024-11-05`).
- Tools (10): `djcraft_status`, `djcraft_plan_transition`, `djcraft_camelot_check`, `djcraft_commands`, `djcraft_analyze_dettato`, `djcraft_mixxxxx_ping`, `djcraft_mixxxxx_video`, `djcraft_crates_list`, `djcraft_crate_import`, `djcraft_crate_export`.

## 4. Tips & Tricks

- **Transparent Stems**: Stem WAVs in `.djcraft-stems/` are automatically ignored by folder scans so they never clutter your track list, and move portably with your music folders.
- **Instant Vocal Drops**: In **Fleet Hub**, type any DJ tag and click `Speak -> Deck C` — works out of the box via `speechcraft-cli` or Windows SAPI.
- **MIDI Learn**: In the **MIDI (Rane 12/72)** tab, click any target action and twist/press a hardware control to rebind and save immediately.

## 5. Common Pitfalls & Safety

- **FFmpeg on PATH**: Required for native `.mp4`/`.mkv`/`.mov`/`.webm` video decoding (falls back to audio-reactive visuals if absent).
- **Demucs for Batch Stemming**: Set `DEMUCS_BIN` or install `demucs` in `stems-mcp/.venv`; batch stemming runs off the audio thread and skips tracks that already have `.djcraft-stems/<stem>/`.
- **Zero-Unsafe & Clamping**: All crates enforce `#![forbid(unsafe_code)]`; all gains, EQs, pitch bends, and crossfader values are strictly clamped at L1.

## 6. Planned Extensions

- All v0.1.0 core DJ workstation capabilities are implemented (including Dual-Bus Headphone PFL Pre-Cue and live WAV/FLAC Mix Recording).
- Future roadmap ideas: DVS timecode vinyl input, Ableton Link network clock sync, and VST3 FX rack hosting.

## 7. Other Crafts & Fleet Integrations

| Repo | Role | Integration |
| :--- | :--- | :--- |
| `mixxxxx` | Video-extended Mixxx fork | Pure-Rust UDP OSC (`11119` in / `11118` out) |
| `speechcraft` | Pure-Rust TTS (`:14132`) | DJ vocal drops onto Deck C |
| `sfx-mcp` | Sound effects (`:11120`) | One-shot risers, impacts, and sweeps |
| `vfx-mcp` | FFmpeg video FX (`:11122`) | Video effect presets for deck clips |
| `butterchurn-mcp` | MilkDrop visualizer (`:10878`) | Live BPM sync from DJCraft |
| `reaper-mcp` | REAPER DAW automation | Shares local `D:\Dev\tools\dettato\.venv` sidecar |

# DJCraft Pro Help & Reference

DJCraft Pro is a local-first, 100% Safe Rust (`#![forbid(unsafe_code)]`) 4-deck audio + video DJ workstation, stem mixer, MIDI controller host, vinyl scratch engine, and Camelot harmonic performance studio.

---

## 1. Get Started

### Starting Playback on Deck A, B, C, or D
1. **Launch `djcraft.exe`**: Windows WASAPI audio starts live automatically (check the green `WASAPI:` indicator in the Battle Mixer).
2. **Load Audio**: Click `AUDIO` on any deck (MP3, WAV, FLAC, OGG, M4A), or switch to **`CRATES & GIG PREP`** in the top bar and click `A`, `B`, `C`, or `D` on any track row.
3. **Start Playback**: Click the highlighted **`PLAY`** button on the deck (turns green **`PAUSE`** while playing), or press **`Space`** for Deck A and **`Shift+Space`** for Deck B.
4. **Verify Audio Routing**: Make sure the deck's channel volume fader (`CH A` / `CH B`) is raised, `MST` Master Loudness is above 0%, `MUTE` is off, and the MAG FOUR crossfader is centered (`0.0`) or moved toward the playing deck.

### Switching Decks & 2-Deck vs 4-Deck Layout
- **Top Bar Layout Toggle**: Click **`2-DECK`** or **`4-DECK`** right next to `CRATES & GIG PREP`.
- **In `2-DECK` Mode**:
  - Click **`-> C`** in Deck A's header to flip the left side between **Deck A** and **Deck C**.
  - Click **`-> D`** in Deck B's header to flip the right side between **Deck B** and **Deck D**.
  - A compact **Decks C & D** transport strip (`PLAY`, `CUE`, `PFL`) is also docked at the bottom.
- **In `4-DECK` Mode**:
  - Left column stacks full hardware **Deck A** (top) + **Deck C** (bottom).
  - Right column stacks full hardware **Deck B** (top) + **Deck D** (bottom).

### Skins, Textured & Sparkly Underlay, and Language (i18n)
- **6 Built-in Skins** (top-right dropdown):
  - `Rane Seventy-Two Battle`
  - `Pioneer Club Dark`
  - `Serato Midnight`
  - `VirtualDJ Carbon`
  - `Shocking Pink Club` (hot magenta/cyan/gold club theme)
  - `Daylight Booth (Light)` (high-contrast deep ink on crisp light surfaces)
- **Textured & Dynamic Underlay** (`BG` dropdown next to Skin): choose **`Solid`**, **`Brushed Metal`**, **`Carbon Weave`**, or **`Sparkle Club`** (animated audio-reactive diamond sparkles & ambient club bokeh behind the console).
- **Internationalization (i18n)** (language dropdown at far top-right): switch live between **EN**, **DE**, **FR**, **ES**, **JA**, **KO**, and **ZH**. All UI preferences persist automatically in `%LOCALAPPDATA%\djcraft\ui.json`.

---

## 2. Core Functions

### Vinyl Scratching & Beatmatching
- **Interactive Platter Scratch**: Click and drag any deck's motorized jog platter left/right to scratch forward or **in reverse** (`-4x` to `+4x` speed) with Catmull-Rom varispeed audio. Works even when a deck is paused/cued so you can perform classic baby scratches on Beat 1 before hitting `PLAY`. Release the mouse to resume playback seamlessly.
- **Varispeed vs Keylock**: Click `VINYL` / `KEYLOCK` on any deck to toggle between classic pitch-coupled varispeed and WSOLA Master Tempo keylock.
- **Beatgrid & Sync**: Use `/2 BPM`, `x2 BPM`, `-10ms`, `+10ms`, `Auto-Sync`, and `Phase Sync` with the top **Parallel Waveforms** banner.

### Dual-Bus Headphone PFL Pre-Cue & Live Lossless WAV/FLAC Mix Recording
- **Per-Deck PFL**: Click **`PFL`** on any deck (or `PFL A` / `PFL B` in the Battle Mixer) to audition that deck **Pre-Fader** (even when its channel fader or crossfader is completely down).
- **Two Monitoring Modes**:
  1. **Dual-Device WASAPI**: Select your headphones / USB DJ interface channels 3-4 from the Headphone dropdown in the Battle Mixer (click `Scan` to refresh devices).
  2. **Single-Device Split Mono Cue**: Click **`SPLIT: ON`** when using a single laptop/DAC output — **Left ear = Mono PFL Cue**, **Right ear = Mono Master**.
- **Cue Mix Controls**: Adjust `Vol` (headphone level) and `Cue/PGM` (`0%` = 100% PFL Cue, `100%` = 100% Master PGM).
- **Live Mix Recorder**: Choose **`WAV`** (16-bit stereo PCM) or **`FLAC`** (lossless via `ffmpeg`) in the Battle Mixer and click **`REC MIX`** (auto-saves to `%USERPROFILE%\Music\DJCraft_Recordings\`) or **`Rec As...`** for a custom path. Recording runs on a dedicated non-blocking writer thread off the WASAPI audio callback.

### Dedicated 2-Pane Crates & Gig Prep Workspace
- Click **`CRATES & GIG PREP`** in the top bar for the full-width file-oriented library manager (`%LOCALAPPDATA%\djcraft\crates.db`).
- **Left Pane (Crate & Subcrate Tree)**: Unlimited nested subcrates with full CRUD (`+ Root`, `+ Sub`, `Rename` with recursive descendant path update, `Duplicate` preserving manual gig order, `Delete`). Drag tracks from the right table onto any crate in the tree to add them.
- **Right Pane (Setlist & Track Table)**: Persistent **`Gig Order (#)`** with `[::]` drag-and-drop row reordering, `^`/`v` nudge buttons, live search, BPM range, Camelot harmonic filter (`Harmonic (Deck A)`), inline star rating, one-click `A`/`B`/`C`/`D` load, `BPM`/`Det` analysis, and `Rem`/`Del`.
- **OS File Drop & Interop**: Drag audio files or folders directly from Windows Explorer onto the DJCraft window, or use Serato DJ `.crate` (`%%` subcrates) and VirtualDJ `.vdjfolder` / `.m3u8` Import/Export.

### Transparent Sidecar Stems, Video, MIDI & Fleet AI
- **Transparent Stems (`.djcraft-stems/`)**: Click `Batch Stem Crate` or `Batch Stem Folder...` to generate `<parent>/.djcraft-stems/<track>/{vocals,drums,bass,other}.wav`. Hidden from folder scans and auto-loaded into the `VOC`/`DRM`/`BAS`/`OTH` rotary knobs.
- **Video Mixer & Mixxxxx OSC**: Load `.mp4`/`.mkv`/`.mov`/`.webm` via `VIDEO`, click `Detach Projector` for a secondary monitor, or drive `mixxxxx` over pure-Rust UDP OSC (`127.0.0.1:11119`).
- **Hardware MIDI (`midir`)**: Plug in **Rane Twelve MKII**, **Rane Seventy-Two MKII**, Pioneer DDJ-FLX4, or Hercules DJControl, plus persistent MIDI Learn (`midi_learn.json`).
- **AI Booth Chat & Fleet Hub**: Local Ollama (`:11434`) / LM Studio (`:1234`) chat, instant NL commands, Camelot harmonic setlists, DJ vocal drops (`speechcraft-cli` / Windows SAPI -> Deck C), `sfx-mcp` (`:11120`), `vfx-mcp` (`:11122`), and `butterchurn-mcp` (`:10878`).

---

## 3. Usage (GUI, CLI & MCP)

### Keyboard & Mouse Shortcuts
- `Space`: Play / Pause Deck A
- `Shift + Space`: Play / Pause Deck B
- `Esc`: Stop All Decks
- `Ctrl + K`: Command Palette | `F1`: Keyboard Shortcuts
- `Ctrl + Z` / `Ctrl + Y`: Undo / Redo (64-step mixer history)
- `Ctrl + Mouse Wheel` / `Ctrl + 0`: Zoom UI / Reset 100%
- **Double-click** any rotary knob, vertical fader, or crossfader to snap to neutral.

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

---

## 4. Troubleshooting & Safety

- **Deck B loaded but not audible**: Verify `PLAY` is green (`PAUSE`), `CH B` fader is raised, and the crossfader is not pulled full left (`-1.0`).
- **FFmpeg on PATH**: Required for native video playback and FLAC mix transcoding (FLAC recording falls back cleanly to lossless WAV if `ffmpeg` is absent).
- **Demucs for Batch Stemming**: Set `DEMUCS_BIN` or install `demucs` in `stems-mcp/.venv`. Always run batch stemming and `dettato` during crate prep, off the live audio thread.
- **Zero-Unsafe & Clamping**: All workspace crates enforce `#![forbid(unsafe_code)]`; all audio/mixer parameters are strictly clamped at L1.

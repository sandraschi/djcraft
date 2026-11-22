# DJCraft Pro Help & Reference

DJCraft Pro is a local-first, 100% Safe Rust (`#![forbid(unsafe_code)]`) 4-deck audio + video DJ workstation, DVS timecode host, stem mixer, MIDI controller host, vinyl scratch engine, and Camelot harmonic performance studio.

---

## 1. Get Started

### Starting Playback on Deck A, B, C, or D
1. **Launch `djcraft.exe`**: Audio output starts live automatically (check the green `WASAPI:` / device indicator in the Battle Mixer or `Settings -> Audio`).
2. **Load Audio**: Click `AUDIO` / `LOAD` on any deck (MP3, WAV, FLAC, OGG, M4A), drag audio files/folders from your file manager onto the window, or pick a track in **`CRATES & GIG PREP`** / **`BOOTH + CRATE`** and click `A`, `B`, `C`, or `D`.
3. **Start Playback**: Click **`PLAY`** on the deck (turns green **`PAUSE`** while playing), or press **`Space`** for Deck A and **`Shift+Space`** for Deck B.
4. **Verify Audio Routing**: Make sure the deck's channel volume fader (`CH A` / `CH B`) is raised, `MST` Master Loudness is above 0%, `MUTE` is off, and the Magnetic crossfader is centered (`0.0`) or moved toward the playing deck.

### Workspace Modes & Layouts
- **Mode dropdown** (top bar):
  - **`LIVE CONSOLE`**: Full 2-deck or 4-deck hardware console with Battle Mixer, stacked/vertical RGB waveforms, and lower Dock Panel.
  - **`BOOTH + CRATE`** (`--booth-crate`): Performance split layout with compact `78px` platters, center vertical RGB waveforms, a hideable **`Booth Strip`** (`Hide` | `FX Rack` | `Sampler` | `Video`), and the lower 2-pane crate browser defaulting to your prepared gig crate.
  - **`CRATES & GIG PREP`** (`--crates`): Full-width 2-pane library manager, setlist sequencer, stem batching, and `.crate` / `.vdjfolder` interop.
- **Decks dropdown**: **`2-Deck`** (use `-> C` / `-> D` in deck headers to flip layers) or **`4-Deck`** (stacks A+C left, B+D right).
- **`Panels` menu**: Toggle **`Live Video Strip`**, **`Live Sampler Strip (4x8)`**, **`Parallel Waveforms`**, **`Vertical Waveforms`**, and **`Decks C & D Bottom Bar`**.

### Skins, Backdrops, Language & Settings (`Ctrl+,`)
- **6 Built-in Skins** (top-right dropdown): `Battle Console`, `Club Dark`, `Midnight Booth`, `Carbon Deck`, `Shocking Pink Club`, and `Daylight Booth (Light)` (pure black ink on crisp light surfaces).
- **4 Backdrops** (next to Skin): `Solid`, `Brushed Metal`, `Carbon Weave`, and `Sparkle Club` (audio-reactive).
- **7 Languages (i18n)**: `EN`, `DE`, `FR`, `ES`, `JA`, `KO`, `ZH` (saved to `%LOCALAPPDATA%\djcraft\ui.json`).
- **Unified Settings (`Ctrl+,`)**: 5-tab modal (`Audio`, `Decks & Booth`, `MIDI & DVS`, `Auto-DJ & Rec`, `AI & Bridges`).

---

## 2. Core Functions

### 3-Band RGB Spectral & Overview Waveforms
- **3-Band Spectral Coloring**: Low (`<250 Hz`) = Crimson/Red, Mid (`250-2500 Hz`) = Emerald/Lime, High (`>2500 Hz`) = Electric Cyan with white-hot transient cores, beatgrid/bar lines, active loop bands, and colored hot-cue flags.
- **Clickable Full-Track Overview**: Click or drag anywhere on the overview strip below each deck header to needle-drop seek instantly.
- **Quantize & Pitch Range**: Enable beatgrid hot-cue **Quantize** and select **`+-8%`**, **`+-16%`**, or **`+-50%`** pitch fader range in **`Settings -> Decks & Booth`**.

### Vinyl Scratching, Keylock & DVS Timecode Vinyl
- **Interactive Platter Scratch**: Drag any deck's motorized jog platter left/right to scratch forward or in reverse (`-4x` to `+4x` speed) with Catmull-Rom varispeed audio.
- **Keylock (`KEYLOCK` / `VINYL`)**: Toggle between classic pitch-coupled varispeed and WSOLA Master Tempo keylock.
- **DVS Timecode Vinyl** (`Settings -> MIDI & DVS` or `MIDI Controllers` dock): Connect a turntable phono preamp / line-in audio interface, choose `1000 Hz` or `2000 Hz` control vinyl, select Deck A or B, and click `Start DVS` to drive pitch and reverse/forward scratching from real vinyl.

### Prelisten (PFL Headphone Cue) & Lossless Mix Recording
- **Per-Deck PFL**: Click **`PFL`** on any deck to audition it **Pre-Fader** (even when its volume fader or crossfader is down).
- **Two Monitoring Modes**:
  1. **Dual-Device Stereo PFL**: Select your headphone / USB DJ interface output in Battle Mixer or `Settings -> Audio`. Main output stays stereo; headphones receive stereo PFL + `Cue/PGM` blend.
  2. **Single-Device Split Mono Cue**: Leave Headphone Device on `Off / Split Cue` and toggle **`SPLIT: ON`** (Left ear = Mono PFL Cue, Right ear = Mono Master) for a standard DJ splitter cable.
- **Live Mix Recorder**: Select `WAV` (16-bit PCM) or `FLAC` (via `ffmpeg`) and click `REC MIX` (saves to `%USERPROFILE%\Music\DJCraft_Recordings\`) or `Rec As...`.

### Crates, Autosaved Hot Cues & Auto-DJ
- **SQLite Library (`%LOCALAPPDATA%\djcraft\crates.db`)**: Nested crates/subcrates, persistent `Gig Order (#)` with `[::]` drag-drop reordering, BPM/Camelot harmonic filtering, and Serato `.crate` / VirtualDJ `.vdjfolder` / `.m3u8` Import/Export.
- **Per-File Autosaved Hot Cues**: Hot cues (slots 1-4) are automatically saved per audio file path in SQLite (`track_cues`) when set and restored whenever that track is loaded onto any deck.
- **Auto-DJ Automix**: Click **`Auto-DJ`** in the top bar to automatically load the next track from the active crate onto the idle deck and crossfade smoothly near outro (`Settings -> Auto-DJ & Rec` sets fade seconds).
- **LUFS Auto-Gain**: Automatic gentle level normalization on track load (`Settings -> Audio`).

### UDP Multicast Link, MIDI, Stems, Video & Booth Chat
- **UDP Multicast Beat & Tempo Link**: Click **`LINK`** in the top bar to join `224.76.78.75:20808` (clean-room MIT/Apache-2.0 peer tempo/phase sync).
- **Hardware MIDI (`midir`)**: 8 built-in clean-room profiles (Rane Twelve/Seventy-Two MKII, Pioneer DDJ-FLX4, Hercules DJControl, Denon DJ SC6000/Prime, Numark Mixtrack Pro FX, Allen & Heath Xone:K2), persistent MIDI Learn, and **`Import XML Mapper...`** for your own local VirtualDJ or Mixxx `.xml` mappers.
- **Transparent Sidecar Stems (`.djcraft-stems/`)**: `Batch Stem Crate` / `Batch Stem Folder...` writes hidden sidecar WAVs loaded automatically into `VOC`/`DRM`/`BAS`/`OTH` knobs.
- **Booth AI & Crate Assistant (`Ctrl+L`)**: Click **`Chat`** in the top bar for a toggleable side drawer showing live Deck A/B status, 1-click Camelot harmonic crate matches (`-> B`), offline NL commands (`sync`, `auto dj on`, `load compatible`, `kill bass a`), and optional OpenAI-compatible LLM chat.

---

## 3. Usage (Shortcuts, CLI & MCP)

### Keyboard & Mouse Shortcuts
- `Space`: Play / Pause Deck A | `Shift + Space`: Play / Pause Deck B | `Esc`: Stop All Decks
- `Ctrl + K`: Command Palette | `F1`: Keyboard Shortcuts | `Ctrl + ,`: Settings | `Ctrl + L`: Booth Chat Drawer
- `Ctrl + Z` / `Ctrl + Y`: Undo / Redo (64-step mixer history)
- `Ctrl + Mouse Wheel` / `Ctrl + 0`: Zoom UI / Reset 100%
- **Double-click** any knob, fader, or crossfader to reset to neutral; **Arrow keys / Home / 0** nudge a focused rotary knob.

### CLI (`djcraft-cli`) & MCP (`djcraft-mcp`)
- **GUI flags**: `djcraft.exe --smoke | --booth-crate | --crates | --four-deck | --skin <battle|club|midnight|carbon|pink|daylight>`
- **CLI (`djcraft-cli`)** (all output JSON):
  - **Config (`ui.json`)**: `config-show`, `config-set <key> <value>`, `config-reset` (controls `skin`, `backdrop`, `locale`, `workspace`, `four-deck`, `zoom`, `pitch-range`, `quantize`, `auto-gain`, `auto-dj-fade`, `headphone-device`, `split-cue`, `recording-format`, `dvs-device`, `dvs-carrier`, `ai-endpoint`, `ai-model`, `bridges-enabled`)
  - **Audio & Probes**: `audio-list`, `dvs-probe [--device] [--carrier 1k|2k] [--ms]`, `link-probe [--bpm] [--ms]`
  - **MIDI**: `midi-list`, `midi-connect [query]`, `midi-profile <profile>`, `midi-import-xml <path>`, `midi-rules`
  - **Crates & Hot Cues**: `crate-list`, `crate-create <name> [--parent-id]`, `crate-scan <folder> [--crate-id]`, `crate-import-serato`, `crate-export-serato`, `crate-import-vdj`, `crate-export-vdj`, `crate-cues <track>`, `crate-set-cue <track> <slot> <sec|clear>`
  - **Analysis & OSC**: `status`, `transition`, `camelot <a> <b>`, `commands`, `analyze-dettato <audio>`, `mixxxxx-ping`, `mixxxxx-video`
- **MCP (`djcraft-mcp`)**: stdio JSON-RPC 2.0 server exposing **25 tools** with 100% CLI parity. See [docs/AI_FLEET_AND_MCP.md](docs/AI_FLEET_AND_MCP.md) for the complete command & tool reference.

---

## 4. Troubleshooting & Safety
- **Deck B loaded but not audible**: Verify `PLAY` is active (`PAUSE`), `CH B` fader is up, and crossfader is not pulled full left (`-1.0`).
- **FFmpeg on PATH**: Required for native video playback and FLAC mix transcoding (FLAC falls back to WAV if `ffmpeg` is absent).
- **Zero-Unsafe & Clamping**: All workspace crates enforce `#![forbid(unsafe_code)]` and strict L1 parameter clamping.

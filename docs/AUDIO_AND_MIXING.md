# Audio Engine, Vinyl Scratching, Headphone PFL & Mix Recording

Back to [README.md](../README.md)

---

## 1. 4-Deck Real-Time WASAPI Engine (`djcraft-audio`)

- **Audio Output**: Native Windows WASAPI via `cpal 0.15` (`crates/djcraft-audio/src/output.rs`), lock-light callback, equal-power crossfader, soft-clip `tanh` master limiter, Master Loudness + `MUTE`, and per-deck channel volume faders.
- **Pure-Rust Decoding**: `symphonia 0.5` decodes MP3, WAV, FLAC, OGG, and M4A/AAC locally with zero external runtime dependencies.
- **Varispeed & Keylock**:
  - `VINYL` mode: Catmull-Rom 4-point cubic varispeed resampling (`-16%` to `+16%` pitch fader).
  - `KEYLOCK` mode: WSOLA pitch-preserving time-stretching (`Master Tempo`).
- **Auto-Beatgrid & Sync**: Onset-energy autocorrelation BPM + first-beat downbeat detection, `/2 BPM`, `x2 BPM`, `-10ms` / `+10ms` grid nudge, `Auto-Sync`, `Phase Sync`, and 10 ms real-audio `WaveformSummary` rendering.
- **3-Band Stereo Kill EQ**: Real-time crossover (`Low <250 Hz`, `Mid 250-2500 Hz`, `High >2500 Hz`) driven by rotary knobs and MPC kill pads (`KILL B`, `KILL V`).

---

## 2. Live Interactive Vinyl Scratching

- Click and drag any deck's motorized jog platter left/right to scratch **forward or in reverse** (`-4x` to `+4x` speed) using Catmull-Rom varispeed interpolation.
- Works both while playing and while paused/cued (press `CUE` to park on Beat 1, grab the platter for baby scratches, then hit `PLAY`).
- Releasing the platter seamlessly resumes normal deck playback.

---

## 3. Dual-Bus Headphone PFL Pre-Cue

Every deck (`A`, `B`, `C`, `D`) has a Pre-Fader Listen (`PFL`) toggle that taps the deck **post-EQ / post-stem-gains and pre-channel-fader / pre-crossfader**, so you can audition a cued track while its fader is completely down.

Two monitoring modes are supported from the Battle Mixer:
1. **Dual-Device WASAPI**: Choose a second Windows audio output device (USB DJ interface channels 3-4, headphones, Bluetooth) in the Headphone dropdown (click `Scan` to refresh devices). Fed via a bounded lock-light ring buffer.
2. **Single-Device Split Mono Cue**: Toggle `SPLIT: ON` when running on a single DAC/laptop output. Routes **Left ear = Mono PFL Cue** and **Right ear = Mono Master**.

Use `Vol` (headphone gain) and `Cue/PGM` (`0%` = 100% PFL Cue, `100%` = 100% Master PGM) to dial in your cue blend.

---

## 4. Non-Blocking Live Lossless WAV / FLAC Mix Recording

- Implemented in `crates/djcraft-audio/src/recorder.rs`.
- Uses a bounded `try_send` tap off the pre-split stereo Master bus so the WASAPI audio thread **never** blocks on disk I/O.
- Select **`WAV`** (16-bit stereo PCM) or **`FLAC`** (lossless via `ffmpeg`, falling back cleanly to lossless `.wav` if `ffmpeg` is absent).
- Click **`REC MIX`** (auto-saves to `%USERPROFILE%\Music\DJCraft_Recordings\`) or **`Rec As...`** for a custom path. Click **`STOP REC (MM:SS)`** to flush and patch exact RIFF/`data` header lengths.

---

## 5. Live Stem Separation (native) and Sidecar Stems

### 5.1 Native live separation (`crates/djcraft-stems`, since 2026-11-28)

- Loading a track (or pressing **STEMS** on a deck) starts separation in Rust: HTDemucs / Mel-Band RoFormer on ONNX Runtime, ported from ClubCraft. No Python and no `demucs` install.
- **Progressive**: the deck plays the original at once; separation starts at the playhead and publishes 1.5 s chunks as they finish. Stem knobs act wherever stems are ready and fade in about 23 ms before that; unseparated regions play the original.
- **Residual mix**: `out = orig + amt * ((r - 1) * orig + sum(g_k * fx(s_k) - r * s_k))`, `r` = mean stem gain. All knobs at full is the original bit-exact (keylock stays on); all at zero is silence. Stem-scoped FX (`Vocals Only`, `Drums Only`) act on the separated stem.
- **Cache**: `%LOCALAPPDATA%\djcraft\Stems\<content hash>.stems.flac` (8-channel 16-bit FLAC, ClubCraft-compatible). A cached track attaches instantly; a cancelled run keeps its partial chunks and resumes.
- **Models**: download `djcraft-models-v1-onnx.tar` from the `models-v1` release and unpack it into `%LOCALAPPDATA%\djcraft\Models` (or set `DJCRAFT_MODELS`). `djcraft-cli models-status` shows what is found.
- **Speed (Goliath CPU, Fast)**: first stems under the playhead after about 10 s, a full track at about 2x realtime, 8 ONNX threads (more threads are slower). GPU (DirectML) currently fails its accuracy self-check on the RTX 4090 and is skipped automatically; see `docs/CLUBCRAFT_PORT_LOG.md`.
- **CLI / MCP**: `djcraft-cli stems-separate <file> --quality fast|high|ultra|best`, `stems-status <file>`, `models-status`; MCP `djcraft_stems_separate`, `djcraft_stems_status`, `djcraft_models_status`.

### 5.2 Transparent Sidecar Stems (`.djcraft-stems/`, legacy)

Existing sidecar WAVs are still loaded first when present (converted to the same stem buffer). The batch commands below still use Python `demucs` until they move to the native separator.

- Run **`Batch Stem Crate`** or **`Batch Stem Folder...`** (uses `demucs` via `DEMUCS_BIN` or `stems-mcp/.venv`) to separate tracks off the audio thread into:
  ```text
  <music_folder>/.djcraft-stems/<track_stem>/{vocals,drums,bass,other}.wav
  ```
- **Hidden from DJCraft's folder scanners** so stem WAVs never pollute your library, travel portably with your music folders, and load automatically into the `VOC` / `DRM` / `BAS` / `OTH` rotary knobs for real-time 4-stem isolation.

---

## 6. Per-Deck Color Filter & Dual 3-Slot BPM-Synced FX Rack (`fx.rs`)

All FX run preallocated in 100% Safe Rust on the live WASAPI audio thread (zero payola, all 3 packs included free). Each of the 3 slots keeps its own delay, modulation, reverb, and brake memory, so delay effects can be chained:

- **Per-Deck Bipolar Color Filter (`A FLT` / `B FLT`)**: Chamberlin state-variable filter in the Battle Mixer (`-1.0` deep LPF <-> `0.0` transparent flat <-> `+1.0` sharp HPF, double-click snaps to `0.0`).
- **Dual FX Banks (`FX 1` & `FX 2`)**: Assignable to `Deck A`, `Deck B`, `Deck C`, `Deck D`, or `Master`, with 3 chained Multi-FX slots, BPM beat divisions (`1/8`, `1/4`, `1/2`, `1`, `2`, `4`), and Wet/Dry mix.
- **Stem-Targeted FX Routing**: Route any FX bank to **`Full Deck`**, **`Vocals Only`** (e.g. Vocal Dub Wash / Echo Out while drums stay completely dry), or **`Drums Only`**. Vocals and Drums apply only when that deck has a `.djcraft-stems/` sidecar. With no sidecar those scopes stay dry.
- **3 Free Built-In FX Packs (12 Real-Time DSP Effects)**:
  - **Club Core Pack**: `BPM Echo`, `Echo Out (Freeze)`, `Hall Reverb`, `Dub Tape Delay`
  - **Modulation Pack**: `Flanger`, `Phaser`, `Stereo Chorus`, `Auto-Gate / Tremolo`
  - **Battle & Lo-Fi Pack**: `Bitcrusher`, `Tape Overdrive`, `Vinyl Brake`, `Beat Stutter`
- **Custom FX Packs**: Save your own 3-slot chains as JSON presets in `%LOCALAPPDATA%\djcraft\fx_packs\`.

---

## 7. Collapsible 4-Bank x 8-Slot Live Sampler (`sampler.rs`)

Toggle open/collapsed anytime via **`SAMPLER: ON` / `SAMPLER`** in the top bar (right next to `VIDEO: ON`, persisted in `ui.json`):
- **32 Total Slots (`Bank A` / `B` / `C` / `D` x 8 Pads)**: Pad hits are queued to the audio thread (they do not take the mix mutex). The sampler sums into the WASAPI master bus, into live WAV/FLAC recordings, and into the headphone cue side, so a full-CUE mix still hears the pads.
- **8 Synthesized One-Shots on Bank A**: `Air Horn`, `Laser Zap`, `Sub Drop`, `Riser Sweep`, `Club Clap`, `Rim Shot`, `Dub Siren`, `Vinyl Chirp`. These are oscillator hits generated in-process, not recorded club samples, and they are ready with zero external files. A short attack fades the start of every pad hit.
- **Custom Audio Loading**: Click **`Load`** on any of the 32 pads to drop any `.wav`, `.mp3`, `.flac`, `.ogg`, or `.m4a` file via `symphonia`.
- **Per-Pad Trigger / Loop Mode + Global Volume + `STOP ALL`**: Toggle `Trig` (one-shot) vs `Loop` per pad, control global sampler volume, or silence all pads instantly with `STOP ALL`.

---

## 8. Studio Beatmaker, Auto-Slice Chopper, Slip Mode, Beatjump & Smart Hot Cues

- **16-Step BPM-Synced Drum Sequencer (`studio.rs`, `studio_dock.rs`)**: Open **`Dock Panel -> Studio Beatmaker & Chopper`** for 4 pre-synthesized 808/909 drum lanes (`Kick`, `Snare`, `Hi-Hat`, `Clap`) x 16 clickable steps locked to Deck A's BPM with 6 one-click genre patterns (`House`, `Techno`, `Hip-Hop`, `Trap`, `DnB`, `Reggaeton`).
- **Auto-Slice Sample Chopper ("Find Samples")**: Runs RMS onset-flux transient detection on Deck A, Deck B, or any local audio file to slice up to 8 playable hits straight into Sampler Banks `A..D`.
- **Slip Mode & Beatjump**: Located in each deck's **`Grid...`** menu - Slip Mode keeps a silent background playhead advancing while scratching so releasing the platter snaps right back on beat; Beatjump skips `-4`, `-1`, `+1`, or `+4` beats seamlessly.
- **High-Visibility Hot Cue Flags & Right-Click Context Menu**: Overview scrubber strips draw 2.8px colored stems with dark contrast halos and top flag badges. Right-click any **`CUE 1..4`** pad to rename it (`Intro`, `Drop`, `Break`, `Vocal`, `Outro`), pick from 8 vivid colors, choose a trigger behavior (`Standard`, `Gate Hold`, `Auto-Loop 4 Bars`, `Cue & Pause`), or clear the cue - all persisted per track in SQLite (`track_cues` + `track_cue_meta`).

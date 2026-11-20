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

## 5. Transparent Sidecar Stems (`.djcraft-stems/`)

- Run **`Batch Stem Crate`** or **`Batch Stem Folder...`** (uses `demucs` via `DEMUCS_BIN` or `stems-mcp/.venv`) to separate tracks off the audio thread into:
  ```text
  <music_folder>/.djcraft-stems/<track_stem>/{vocals,drums,bass,other}.wav
  ```
- Hidden from DJCraft's folder scanners so stem WAVs never pollute your library, travel portably with your music folders, and load automatically into the `VOC` / `DRM` / `BAS` / `OTH` rotary knobs for real-time 4-stem isolation.

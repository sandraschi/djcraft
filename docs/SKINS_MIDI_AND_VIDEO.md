# Skins, Textured Underlays, 2/4-Deck Layout, MIDI & Video

Back to [README.md](../README.md)

---

## 1. Skins & Textured / Dynamic Sparkly Underlays

Switch skins and backdrops live from the top-right bar (persisted in `%LOCALAPPDATA%\djcraft\ui.json`):

### 6 Built-In Console Skins
1. **Battle Console** (electric cyan + crimson battle dark)
2. **Club Dark** (deep navy + amber club console)
3. **Midnight Booth** (pitch-black contrast + neon cyan/pink)
4. **Carbon Deck** (slate carbon + teal/orange)
5. **Shocking Pink Club** (velvet plum-black + shocking pink, electric cyan, acid lime & hot gold)
6. **Daylight Booth (Light)** (high-contrast near-black ink `#020617` on crisp `#FFFFFF` / `#E2E8F0` surfaces)

### 4 Textured & Dynamic Underlays (`BG` Dropdown)
- **Solid**: Flat studio chassis.
- **Brushed Metal**: Fine horizontal anodized metal grain.
- **Carbon Weave**: Diagonal tinted carbon texture.
- **Sparkle Club**: Animated audio-reactive diamond sparkles and ambient club bokeh behind translucent panels.

---

## 2. 2-Deck / 4-Deck Layout, Vertical Waveforms & Internationalization (i18n)

- **Compact Dropdown Top Bar**: Switch **Mode** (`LIVE CONSOLE` / `CRATES & GIG PREP`), **Decks** (`2-Deck` / `4-Deck`), **`Panels`** (toggle `Live Video Strip`, `Live Sampler Strip`, `Parallel Waveforms`, `Vertical Waveforms (between decks)`, `Decks C & D Bottom Bar`), and **`Sync`** (`Auto-Sync`, `Phase-Sync`, `Command Palette`) from clean dropdowns.
  - **Horizontal vs. Vertical Waveforms**: Toggle **`Vertical Waveforms (between decks)`** in the **`Panels`** menu for classic battle-style top-to-bottom scrolling Deck A / Deck B waveforms right beside the Battle Mixer.
  - **`2-DECK`**: Full-height Decks A & B with **`-> C`** / **`-> D`** layer-swap buttons in each deck header plus optional compact Decks C & D strip at the bottom.
  - **`4-DECK`**: Stacks full hardware **Deck A + Deck C** on the left and **Deck B + Deck D** on the right.
- **i18n (`crates/djcraft-core/src/i18n.rs`)**: Switch languages live in the top-right dropdown across **EN**, **DE**, **FR**, **ES**, **JA**, **KO**, and **ZH**.

---

## 3. Hardware MIDI Controllers & MIDI Learn (`midir 0.10`)

Open the **MIDI (Rane 12/72)** dock tab to scan USB MIDI ports and connect controllers with built-in auto-detect profiles:
- **Rane Twelve MKII** (motorized platter, transport, pitch, hot cues)
- **Rane Seventy-Two MKII** (MAG FOUR crossfader, 3-band EQ, channel faders, MPC pads)
- **Pioneer DDJ-FLX4 / DDJ-400**
- **Hercules DJControl**
- **Custom MIDI Learn**: Click any target action, move a hardware control, and DJCraft saves the binding to `%LOCALAPPDATA%\djcraft\midi_learn.json`.

---

## 4. Native FFmpeg Video Mixer & Mixxxxx OSC 1.0

- **Collapsible 3-Monitor Live Video Strip (`VIDEO` toggle in top bar, `video.rs`)**:
  - **Left Box (Deck A Video)** & **Right Box (Deck B Video)**: Live 16:9 previews with `Load A`/`Load B`, per-deck **Individual Video FX dropdown** (`None`, `RGB Split`, `Invert`, `Posterize`, `CRT Scanlines`, `Mirror Kaleido`, `Club Strobe`, `Neon Edge`), and depth slider.
  - **Center Box (Mixed Master Output)**: Live 16:9 composite with **Transition FX dropdown** (`Crossfade`, `Additive Glow`, `Wipe L -> R`, `Iris Circle`, `Luma Key`, `Zoom Cut`, `Glitch Slice`), `Link XF` toggle (or independent Video Crossfader), and **`Projector`** detachable secondary monitor viewport.
  - Loads `.mp4`/`.mkv`/`.mov`/`.webm` via external `ffmpeg` RGB24 pipe (or autoloads companion videos matching the audio filename) and falls back to audio-reactive visuals if `ffmpeg` is absent.
- **Mixxxxx Video OSC Bridge (`mixxxxx_osc.rs`)**: Zero-dependency pure-Rust UDP OSC 1.0 client (`127.0.0.1:11119` in / `11118` out) supporting `/mixxxxx/ping` and `/deck/{1..4}/video_{enabled,fullscreen,brightness,contrast,saturation}`.

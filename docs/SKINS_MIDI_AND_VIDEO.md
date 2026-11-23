# Skins, Textured Underlays, 2/4-Deck Layout, MIDI & Video

Back to [README.md](../README.md)

---

## 1. Skins & Textured / Dynamic Sparkly Underlays

Switch skins and backdrops live from the top-right bar (persisted in `%LOCALAPPDATA%\djcraft\ui.json`):

### 9 Built-In Console Skins
1. **Battle Console** (electric cyan + crimson battle dark)
2. **Club Dark** (deep navy + amber club console)
3. **Midnight Booth** (pitch-black contrast + neon cyan/pink)
4. **Carbon Deck** (slate carbon + teal/orange)
5. **Shocking Pink Club** (velvet plum-black + shocking pink, electric cyan, acid lime & hot gold)
6. **Daylight Booth (Light)** (high-contrast pure black `#000000` ink on crisp `#FFFFFF` / `#E2E8F0` surfaces)
7. **Star Trek LCARS** (deep space obsidian + LCARS amber-gold, lavender, tricorder cyan & Starfleet brackets)
8. **Baroque Velvet & Gold** (royal burgundy velvet + antique burnished gold double frames & jewel accents)
9. **Synthwave Outrun** (ultraviolet indigo + laser cyan, outrun magenta & neon outer glow)

### 6 Textured & Dynamic Underlays (`BG` Dropdown / Settings)
- **Solid**: Flat studio chassis.
- **Brushed Metal**: Bold horizontal anodized metal grain with accent sheen bands.
- **Carbon Weave**: High-contrast diagonal tinted carbon texture.
- **Sparkle Club**: Vivid animated audio-reactive diamond sparkles and glowing club bokeh behind translucent panels.
- **Starfield Warp (LCARS)**: Subspace tactical grid with radial warp star streaks.
- **Baroque Gold Damask**: Gilded diamond lattice with ornate rosette medallions.

### Configurable Card Decorations & Full-Window Settings (`Ctrl+,`)
Open the dedicated 2-pane **Settings** workspace (`Settings` button, Workspace dropdown, or `Ctrl+,`; `Esc` returns to the booth):
- **5 Card Decoration Styles**: `Clean Flat`, `Bold Drop Shadow`, `Neon Outer Glow`, `Baroque Double Frame` (inner gilded hairline + corner studs), and `LCARS Starfleet Bracket` (left vertical bar + top elbow pill).
- **Live Sliders (persisted in `ui.json`)**: Border Thickness (`1.0..3.8 px`), Card Opacity (`52%..100%`), Color Richness / Accent Boost (`0%..100%`), and Background Pattern Intensity (`20%..200%`).
- **7 Serato / VirtualDJ Layout Presets (`Layouts` top-bar menu & Settings)**: `Classic 2-Deck`, `Serato Vertical`, `Extended 4-Deck`, `Scratch / Battle Focus`, `Performance (Sampler + FX)`, `Library / Gig Prep`, and `Video / VJ Booth`.
- **Waveform Magnification**: `-` / Zoom dropdown (`Show All`, `1x (16s)`, `2x (8s)`, `4x (4s)`, `8x (2s)`, `16x (1s)`) / `+` / `ALL` controls on both horizontal parallel waveforms and vertical waveforms.

---

## 2. Menu-First Navigation, 2-Deck / 4-Deck Layout, Vertical Waveforms & i18n

- **Menu-First Top Bar (Quick Toolbar Hidden by Default)**: All workspace switching (`View -> Workspace`), 7 Booth Layout Presets (`View -> Booth Layouts`), skin/backdrop selection, panel visibility (`Live Video Strip`, `Live Sampler Strip`, `Parallel Waveforms`, `Vertical Waveforms`, `Decks C & D`), Settings (`File -> Settings...` / `Ctrl+,`), and performance toggles (`Commands -> Link Sync / Auto-DJ / Stop All / Booth AI Chat`) live in the standard desktop Menu Bar. The optional Quick Toolbar strip is hidden by default and can be toggled anytime via **`View -> Show Quick Toolbar`** or **`Settings -> Booth Layouts`**.
  - **Horizontal vs. Vertical Waveforms**: Toggle **`View -> Vertical Waveforms (between decks)`** (or pick **`Serato Vertical`** under **`View -> Booth Layouts`**) for classic battle-style top-to-bottom scrolling Deck A / Deck B waveforms right beside the Battle Mixer.
  - **`2-DECK`**: Full-height Decks A & B with **`-> C`** / **`-> D`** layer-swap buttons in each deck header plus optional compact Decks C & D strip at the bottom.
  - **`4-DECK`**: Stacks full hardware **Deck A + Deck C** on the left and **Deck B + Deck D** on the right.
- **i18n (`crates/djcraft-core/src/i18n.rs`)**: Switch languages live in the top-right dropdown or Settings across **EN**, **DE**, **FR**, **ES**, **JA**, **KO**, and **ZH**.

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

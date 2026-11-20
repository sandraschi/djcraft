# DJCraft Build Log

## 2026-11-20 - v0.1.0 Tier 4 Release & NSIS Build

- **Host:** Goliath (Windows 11, x64)
- **Toolchain:** Rust 1.99.0 (Cargo release profile), NSIS `makensis.exe` (`C:\Program Files (x86)\NSIS\makensis.exe`)
- **Phase 1 Audit (`TAURI_PRODUCTION_PITFALLS.md`):**
  - Pure-Rust egui ArtCraft app (no Tauri WebView, no PyInstaller sidecar; sections B-G N/A).
  - Section A / H-J verified: per-user NSIS (`RequestExecutionLevel user`), installs to `%LOCALAPPDATA%\Programs\DjCraft`, bundles `djcraft.exe`, `djcraft-cli.exe`, `README.md`, `HELP.md`, writes Start Menu shortcuts + clean HKCU uninstall keys, no running `djcraft*` processes locked files.
- **Gates:**
  - `cargo clippy --all-targets -- -D warnings`: pass (0 warnings)
  - `cargo test`: **25 passed, 0 failed** (including UDP loopback OSC round-trip for Mixxxxx video bridge)
  - `target\release\djcraft.exe --smoke`: pass (`menu_count: 8`, `help_tabs: 8`, `command_catalog_count: 19`, `dettato_sidecar_available: true`, `mixxxxx_binary_available: true`, `tofu_free: true`)
  - `target\release\djcraft-cli.exe mixxxxx-ping`: pass (`binary_found: true`, sent `/mixxxxx/ping` to `127.0.0.1:11119`)
  - `target\release\djcraft-mcp.exe` `tools/list`: pass (7 MCP tools)
- **Artifacts (`dist/`, gitignored):**
  - `djcraft.exe` (14,409,216 bytes)
  - `djcraft-cli.exe` (1,079,808 bytes)
  - `djcraft-mcp.exe` (665,600 bytes)
  - `djcraft-0.1.0-setup.exe` (4,163,540 bytes, LZMA solid)
  - `SHA256SUMS.txt` generated
- **Warnings / Notes:**
  - NSIS warning 7010 (`CHANGELOG.md` not present; `/nonfatal`, harmless).

## 2026-11-20 - v0.1.0 egui SOTA Polish Pass & NSIS Rebuild

- **UI Upgrades (`crates/djcraft-app/src/{theme,widgets,main}.rs`):**
  - Fleet `slate-950` (`#0a0d12` / `#101a2e` / `#263452`) dark visuals + crisp `slate-50` light visuals + bumped typography (`20px` Heading, `14.5px` Body, `13px` Monospace).
  - Vector-painted Sun/Moon theme toggle, clickable `100%` zoom reset pill, and `%LOCALAPPDATA%\djcraft\ui.json` preference persistence.
  - GPU-painted CDJ platters with strobe dots + BPM readout, scrolling dual-frequency waveforms with beatgrid ticks / loop bands / hot-cue pins, vertical stereo segmented VU meters, and 12-hour Camelot harmonic wheel ring.
  - Full 4-deck UI (bottom Decks C & D strip), inline Mixxxxx Video OSC dock, and slider-to-`Command::dispatch` undo on drag release.
  - Added mandatory `glyph_allowlist` Tofu Guard unit test scanning `main.rs`, `theme.rs`, and `widgets.rs`.
- **Gates:**
  - `cargo clippy --all-targets -- -D warnings`: pass (0 warnings)
  - `cargo test`: **26 passed, 0 failed** (25 core + `tests::glyph_allowlist`)
  - Rebuilt `dist/djcraft.exe` (14,508,032 bytes) and `dist/djcraft-0.1.0-setup.exe` (4,188,716 bytes) + `SHA256SUMS.txt`.

## 2026-11-20 - v0.1.0 Live WASAPI Audio, DSP/Beatgrid, 3-Band Kill EQ & Native Video Mixer

- **Audio & DSP (`4c6c647`, `d9a18ec`, `5738213`):**
  - Added `cpal 0.15.3` live WASAPI 4-deck output (`output.rs`), Master Loudness + Mute, per-deck Volume faders.
  - Added `symphonia 0.5.5` audio file decoding, Catmull-Rom varispeed, WSOLA keylock time-stretching, onset autocorrelation beatgrid detection + `Phase Sync`, real-time 3-band stereo kill EQ, and 10 ms `WaveformSummary` rendering (`dsp.rs`, `widgets.rs`).
  - Added native FFmpeg RGB24 video mixer + audio-reactive fallback + detachable projector viewport (`video.rs`, `main.rs`, `rfd 0.15.4`).
- **Gates:**
  - `cargo clippy --all-targets -- -D warnings`: pass (0 warnings)
  - `cargo test`: **34 passed, 0 failed** (33 core/audio/DSP/EQ/OSC + `tests::glyph_allowlist` scanning `main.rs`, `theme.rs`, `video.rs`, `widgets.rs`)
  - `makensis`: 0 warnings (`CHANGELOG.md` now bundled)
  - Rebuilt `dist/djcraft.exe` (16,624,640 bytes), `dist/djcraft-cli.exe` (1,112,576 bytes), `dist/djcraft-mcp.exe` (698,880 bytes), and `dist/djcraft-0.1.0-setup.exe` (4,812,249 bytes) + `SHA256SUMS.txt`.

## 2026-11-20 - v0.1.0 Crates (Serato/VDJ), Pro Console + Skins, Rane 12/72 MIDI, Transparent Stems, AI & Fleet Hub

- **Features (`dcebdde`, `4c3c046`, `a6d8519`):**
  - Persistent SQLite crates/subcrates (`crates_db.rs`) + Serato `.crate` & VirtualDJ `.vdjfolder`/`.m3u8` import/export (10 MCP tools total).
  - Pro DJ Console overhaul (`skins.rs`, `widgets.rs`, `main.rs`): 5 skins, stacked parallel waveforms, rotary knobs, vertical faders, MAG FOUR crossfader, 8 RGB MPC pads.
  - Hardware MIDI host (`midir 0.10.4`, `midi.rs`) with Rane Twelve MKII, Rane Seventy-Two MKII, Pioneer DDJ-FLX4, Hercules DJControl, and MIDI Learn.
  - Transparent `.djcraft-stems/` batch stemming + real-time 4-stem WAV WASAPI mixing, AI Booth Chat/Workflows (`ureq 2.12.1`, `fleet_ai.rs`), and Fleet Hub (`SpeechCraft`, `sfx-mcp`, `vfx-mcp`, `butterchurn-mcp`).
- **Gates:**
  - `cargo clippy --all-targets -- -D warnings`: pass (0 warnings)
  - `cargo test`: **41 passed, 0 failed** (40 core/audio/DSP/EQ/Crates/MIDI/Stems/AI + `tests::glyph_allowlist`)
  - `makensis`: 0 warnings
  - Rebuilt `dist/djcraft.exe` (20,634,112 bytes), `dist/djcraft-cli.exe` (3,266,560 bytes), `dist/djcraft-mcp.exe` (2,847,744 bytes), and `dist/djcraft-0.1.0-setup.exe` (7,239,756 bytes) + `SHA256SUMS.txt`.

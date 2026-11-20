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

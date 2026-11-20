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

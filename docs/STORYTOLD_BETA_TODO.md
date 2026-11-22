# Storytold Beta Readiness TODO - DJCraft

**Target:** Submit to https://getartcraft.com/beta (Tier A Candidate #2 - highest category visibility)
**Audited:** 2026-11-21 (16.3k LOC, 57 tests)
**Reference:** `mcp-central-docs/integrations/storytold-artcraft/BETA_CANDIDATES.md`

---

## P0 - Hard Submission Blockers (Must Complete Before Beta Form)

- [x] **Remove sibling `path` dependencies (S15)**
  - Removed unused `../databasecraft/crates/databasecraft-core` and `../databasecraft/crates/databasecraft-store` from `Cargo.toml` and `crates/djcraft-core/Cargo.toml`; verified standalone `cargo xtask ci` passes.
- [x] **Cross-platform audio backend**
  - Verified `djcraft-audio` uses `cpal` (no Windows-only audio crate); Linux/macOS/Windows matrix CI configured in `.github/workflows/ci.yml`.
- [x] **Legal & governance files**
  - [x] `LICENSE-MIT` and `LICENSE-APACHE` present.
  - [x] Added `NOTICE` (explicit non-affiliation disclaimer with Storytold / third-party DJ brands).
  - [x] Added `ATTRIBUTION.md` (verified by `cargo xtask assets`).
  - [x] Verify every crate in `Cargo.lock` matches Storytold `standards/licences.md` SPDX allowlist (zero GPL/LGPL/AGPL-only crates; verified via `cargo metadata --format-version 1`).
- [x] **RFC-0029 trademark & naming hygiene (`docs/feature-names.md`)**
  - Scrubbed trademarked skin names (`RaneBattle` -> `BattleConsole`, `PioneerClub` -> `ClubDark`, `SeratoMidnight` -> `MidnightBooth`, `VirtualDjCarbon` -> `CarbonDeck`) in `skins.rs`, `main.rs`, `README.md`, `capture-screenshots.ps1`, `SKINS_MIDI_AND_VIDEO.md`, `docks.rs` Help text, and `i18n.rs` (`MAG FOUR` -> `Magnetic`); documented in `docs/feature-names.md`.
- [x] **Decouple personal fleet hub hooks**
  - Made `FleetEndpoints::enabled` default to `false` (`probe_fleet_health` is a no-op when disabled), gated External Bridges UI behind an opt-in toggle in `docks.rs`, and replaced hardcoded `D:\Dev\...` paths in `fleet_ai.rs` and `command.rs` with env var / `PATH` lookups.

## P1 - Storytold Engineering & CI Bar

- [x] **Add `AGENTS.md`** with `## This repository` section and `xtask ci` commands.
- [x] **Add `xtask` (`cargo xtask ci`)**: `fmt --check`, `clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (57 tests), `layers`, `assets` - passing locally.
- [x] **Add GitHub Actions CI & Release workflows (`.github/workflows/ci.yml`, `release.yml`)**
  - Matrix build + test on `ubuntu-latest`, `macos-latest`, `windows-latest`, plus release workflow publishing native binaries + `SHA256SUMS.txt`.
- [x] **Add `packaging/env.sh`** (app ID, display name, binary names).
- [x] **Add `docs/target-app-parity.md` and `docs/gaps.md`** (honest Implemented / Partial / Planned matrix).

## P2 - Polish, Accessibility & Evidence

- [x] **Real desktop screenshots (dark + light modes)** committed to `docs/images/` and linked from `README.md` (`live-console-battle.png`, `live-console-shocking-pink-4deck.png`, `daylight-booth.png`, `crates-gig-prep.png`, attributed in `ATTRIBUTION.md`).
- [ ] **Accessibility check (RFC-0014)**: keyboard shortcut path for every deck/mixer command, numeric entry alternatives for rotary knobs/faders, high-contrast light mode.
- [x] **Clean fresh-clone verification**: cloned to `%TEMP%\djcraft-fresh-verify`, ran `cargo run -p xtask -- ci` (68 tests + `layers` + `assets`) and `--smoke` (`success: true`, `tofu_free: true`), and verified GitHub Actions CI green across `ubuntu-latest`, `macos-latest`, and `windows-latest` (`44979302899`).
- [ ] **Submit** 4-field form at https://getartcraft.com/beta (App name, Email, Repo URL, Description).

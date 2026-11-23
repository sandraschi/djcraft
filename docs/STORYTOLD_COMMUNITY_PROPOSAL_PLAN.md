# DJCraft -> Storytold Community Craft App Proposal Plan

**Date**: 2026-11-23 (updated from 2026-11-21)  
**Current Release**: `v0.4.0` (`https://github.com/sandraschi/djcraft/releases/tag/v0.4.0`, 68 tests passing, `#![forbid(unsafe_code)]`, 0 clippy warnings)  
**Target Intake**: ArtCraft Beta Program (`https://getartcraft.com/beta`)  
**Upstream Reference**: `storytold/craft-contrib` (`standards/*`), `storytold/craft-libs`, `storytold/craft-i18n`, `storytold/craft-launcher`

---

## 1. Storytold's New Intake Structure (Verified 2026-11-20)

- **Do NOT open a new-app RFC PR in `storytold/craft-contrib`.** On 2026-11-20, Storytold org member `benandez` commented on community app RFC PRs (`#64` NetLabCraft through `#68` ModelCraft):
  > *"RFC-0000 has been superceded for new app additions to the suite, these now enter via the Beta program - https://getartcraft.com/beta. This RFC should be closed."*
- New community Craft apps now enter through the 4-field Beta intake form at **`https://getartcraft.com/beta`** (`App name`, `Email`, `Repo`, `Description` - no separate website required; contact `hello@storyteller.ai`).
- **Technical evaluation still uses `storytold/craft-contrib/standards/`** as the quality bar before an app is listed in `craft-launcher` or transferred into `storytold/*`.

---

## 2. Fit in the Storytold Suite

- Storytold ships **SoundCraft** (a multitrack DAW), **FilmCraft** (NLE), **PhotoCraft**, **VectorCraft**, **FigCraft**, etc., and has a community proposal for **TuneCraft** (music library player, `artcraft#2919`).
- **Gap filled by DJCraft**: There is no live DJ performance workstation in the suite (4-deck real-time mixing, beatmatching/keylock, vinyl platter scratching, Dual-Bus Headphone PFL pre-cue, Serato/VirtualDJ crate prep, hardware MIDI controllers, 4-stem live isolation, and live video mixing).
- **Proposed identity**: Display name **DJCraft**, machine id `djcraft` (binaries `djcraft`, `djcraft-cli`, `djcraft-mcp`), app id `ai.storyteller.djcraft` (pending Storytold approval).

---

## 3. Readiness Gap Audit & Phased Plan (Before Submitting)

### Phase 1: Legal, Naming (RFC-0029) & Clean-Room Hygiene
*Mandatory blockers before submitting to `getartcraft.com/beta`.*

| Task | Current State in `djcraft` v0.4.0 | Required Action |
| :--- | :--- | :--- |
| Dual License files | `Cargo.toml` says `MIT OR Apache-2.0` | Ensure root `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE` (explicit non-affiliation disclaimer), and `ATTRIBUTION.md` (all crates/assets) exist and are complete |
| RFC-0029 Naming Audit (`standards/naming.md`) | **DONE in v0.2.1-v0.4.0**: User-visible skins & controls use ordinary descriptive names (`Battle Console`, `Club Dark`, `Midnight Booth`, `Carbon Deck`, `Shocking Pink Club`, `Daylight Booth`, `Star Trek LCARS`, `Baroque Velvet & Gold`, `Synthwave Outrun`). | Keep factual format/hardware interop strings (`"Import Serato .crate"`, `"Import VirtualDJ .vdjfolder"`, working MIDI device profiles) per `naming.md` sec 3. Record audit in `docs/feature-names.md` |
| SPDX Dependency Audit (`standards/licences.md`) | Not yet automated against Storytold's allowlist | Run `cargo deny check licenses` against the exact `standards/licences.md` allowlist (`MIT`, `Apache-2.0`, `BSD-2/3`, `ISC`, `Unicode-3.0`, `Zlib`, `0BSD`, `Unlicense`, etc.); replace or justify any non-allowlisted transitive crate |
| Clean-Room Boundary (`standards/clean-room.md`) | `ffmpeg` and `demucs` run only as external subprocesses (compliant); UDP multicast Link & DVS timecode are 100% Safe Rust clean-room | Document clean-room provenance in `NOTICE` and crate docs; verify zero GPL/LGPL/AGPL code is linked or bundled |

### Phase 2: Engineering, Testing (RFC-0071) & Accessibility (RFC-0014)
*Brings DJCraft above the T2 50-test floor and matches sibling Craft apps.*

| Task | Current State (v0.4.0) | Required Action |
| :--- | :--- | :--- |
| Never-crash lints | `#![forbid(unsafe_code)]` on all crates; 0 clippy warnings (`-D warnings`) | Optionally add `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` to all production crate roots |
| `cargo xtask ci` | **DONE**: Pure-Rust `xtask` crate checks `fmt`, `clippy`, tests, crate layering, and assets | Keep green on every release |
| Invariant & Hostile-Input Tests (50+ floor) | **DONE**: **68 tests passing** (exceeds T2 50-test floor, including SQLite cue & cue-metadata persistence, DVS, Link, Studio Beatmaker, Auto-Slice Chopper, 12 FX, WAV recorder, XML MIDI importers) | Continue adding regression tests per feature |
| Accessibility (`standards/accessibility.md`) | Keyboard shortcuts exist for core transport, `Ctrl+,` full-window Settings, `Ctrl+K` Command Palette, `Ctrl+L` AI Drawer; rotary knobs support keyboard focus & arrow/Home reset | Add `kittest` keyboard/accessibility tests as follow-up |
| Cross-Platform Build Check | Windows WASAPI verified (`cpal` is cross-platform) | Verify `cargo check` / `cargo test` on Linux (ALSA/Pulse) and macOS (CoreAudio) via GitHub Actions |

### Phase 3: Storytold Shared Ecosystem Alignment (RFC-0040 / RFC-0072 / RFC-0074)
*Optional before initial beta signup, expected before suite/launcher inclusion.*

1. **Shared `AGENTS.md` (RFC-0072)**: Adopt `storytold/craft-contrib/standards/app-agents.md` verbatim at top of `AGENTS.md`, followed by `## This repository`.
2. **Shared Crates (`storytold/craft-libs`)**: Migrate settings persistence (`ui.json`), atomic file writes, and keymaps to `craft-settings`, `craft-atomic`, and `craft-keymap` where applicable.
3. **Shared Translations (`storytold/craft-i18n`)**: Migrate `djcraft-core/src/i18n.rs` to load TSV catalogs via `craft-i18n` and `CRAFT_I18N_DIR`.
4. **Measured Parity & Gaps**: Add `docs/target-app-parity.md` (scored by `cargo xtask parity`) and `docs/gaps.md` documenting honest parity % against reference DJ workflows (audio DSP, crate interop, MIDI, DVS, stems, video, recording).
5. **Packaging & CI**: Add `packaging/env.sh` and `craft-common-actions` workflow callers gated with `if: github.repository_owner == 'storytold'`.

### Phase 4: Visual Proof & Beta Submission (`https://getartcraft.com/beta`)
1. **DONE**: Captured 4 real GPU screenshots into `docs/images/` (`djcraft-battle.png`, `djcraft-lcars.png`, `djcraft-settings.png`, `djcraft-crates.png`) and embedded them in `README.md`.
2. **DONE**: Published full (non-prerelease) `v0.4.0` release on GitHub (`https://github.com/sandraschi/djcraft/releases/tag/v0.4.0`) with 68 passing tests, plain exes, per-user NSIS installer, portable zip, validated `.mcpb`, and `SHA256SUMS.txt`.
3. Submit at **`https://getartcraft.com/beta`** (4 visible fields, no website required; leave hidden honeypot blank):
   - **App name**: `DJCraft`
   - **Email**: *(Sandra's contact email)*
   - **Repo**: `https://github.com/sandraschi/djcraft`
   - **Description**: `Local-first, 100% Safe Rust (#![forbid(unsafe_code)]) 4-deck DJ performance & production workstation (egui/wgpu + cpal + symphonia) with 3-band RGB zoomable waveforms (Show All to 16x), live vinyl platter scratching, WSOLA keylock, 3-band kill EQ + bipolar filter, 12 real-time stereo FX (3 free packs), 16-step BPM-synced Studio Beatmaker + auto-slice sample chopper, dual-bus Headphone PFL pre-cue, lossless WAV/FLAC mix recording, DVS timecode vinyl, clean-room UDP multicast beat Link, 2-pane Crates & Gig Prep (Serato/VirtualDJ interop + SQLite hot cues with custom labels/colors/behaviors), transparent 4-stem mixing, USB MIDI + XML mapper import, 3-monitor live video mixer, Art-Net/sACN DMX lighting, full-window 2-pane Settings with 9 skins & 7 booth layouts, and unified GUI/CLI/MCP parity (v0.4.0, 68 tests passing).`

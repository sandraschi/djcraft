# DJCraft -> Storytold Community Craft App Proposal Plan

**Date**: 2026-11-21  
**Current Release**: `v0.2.0` (`sandraschi/djcraft`, 45 tests passing, `#![forbid(unsafe_code)]`)  
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

| Task | Current State in `djcraft` v0.2.0 | Required Action |
| :--- | :--- | :--- |
| Dual License files | `Cargo.toml` says `MIT OR Apache-2.0` | Ensure root `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE` (explicit non-affiliation disclaimer), and `ATTRIBUTION.md` (all crates/assets) exist and are complete |
| RFC-0029 Naming Audit (`standards/naming.md`) | User-visible skin/control labels use third-party trademarks: `"Rane Seventy-Two Battle"`, `"Pioneer Club Dark"`, `"Serato Midnight"`, `"VirtualDJ Carbon"`, `"MAG FOUR"`, `"MPC Performance Pads"` | Rename user-facing UI labels & skins to ordinary descriptive names (`"Battle Dark"`, `"Club Dark"`, `"Midnight"`, `"Carbon"`, `"Shocking Pink"`, `"Daylight Booth"`, `"Magnetic Crossfader"`, `"Performance Pads"`). Keep factual format/hardware interop strings (`"Import Serato .crate"`, `"Import VirtualDJ .vdjfolder"`, working MIDI device profiles) per `naming.md` sec 3. Record audit in `docs/feature-names.md` |
| SPDX Dependency Audit (`standards/licences.md`) | Not yet automated against Storytold's allowlist | Run `cargo deny check licenses` against the exact `standards/licences.md` allowlist (`MIT`, `Apache-2.0`, `BSD-2/3`, `ISC`, `Unicode-3.0`, `Zlib`, `0BSD`, `Unlicense`, etc.); replace or justify any non-allowlisted transitive crate |
| Clean-Room Boundary (`standards/clean-room.md`) | `ffmpeg` and `demucs` run only as external subprocesses (compliant) | Document clean-room provenance in `NOTICE` and crate docs; verify zero GPL/LGPL/AGPL code is linked or bundled |

### Phase 2: Engineering, Testing (RFC-0071) & Accessibility (RFC-0014)
*Brings DJCraft above the T2 50-test floor and matches sibling Craft apps.*

| Task | Current State | Required Action |
| :--- | :--- | :--- |
| Never-crash lints | `#![forbid(unsafe_code)]` on all 4 crates; `unwrap_used`/`expect_used`/`panic` only on `i18n.rs` | Add `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` to all 4 production crate roots (`djcraft-audio`, `djcraft-core`, `djcraft-mcp`, `djcraft-app`) |
| `cargo xtask ci` | Uses PowerShell build script | Add a pure-Rust `xtask` crate (`cargo xtask ci`) checking `fmt`, `clippy`, tests, crate layering (zero UI deps in `djcraft-audio`/`djcraft-core`), license allowlist, and parity score |
| Invariant & Hostile-Input Tests (50+ floor) | 45 tests passing (below T2 50 floor) | Add 15+ invariant & hostile-input tests (truncated/corrupt `.crate` UTF-16BE, malformed `.vdjfolder` XML, zero-length/corrupt audio files, extreme BPM/pitch/crossfader values, malformed OSC packets) -> 60+ tests |
| Accessibility (`standards/accessibility.md`) | Keyboard shortcuts exist for core transport; rotary knobs/faders are drag-only | Ensure every command has a keyboard path, add numeric entry / keyboard nudge support for rotary knobs & faders, and add `kittest` keyboard/accessibility tests |
| Cross-Platform Build Check | Windows WASAPI verified (`cpal` is cross-platform) | Verify `cargo check` / `cargo test` on Linux (ALSA/Pulse) and macOS (CoreAudio) via GitHub Actions |

### Phase 3: Storytold Shared Ecosystem Alignment (RFC-0040 / RFC-0072 / RFC-0074)
*Optional before initial beta signup, expected before suite/launcher inclusion.*

1. **Shared `AGENTS.md` (RFC-0072)**: Adopt `storytold/craft-contrib/standards/app-agents.md` verbatim at top of `AGENTS.md`, followed by `## This repository`.
2. **Shared Crates (`storytold/craft-libs`)**: Migrate settings persistence (`ui.json`), atomic file writes, and keymaps to `craft-settings`, `craft-atomic`, and `craft-keymap` where applicable.
3. **Shared Translations (`storytold/craft-i18n`)**: Migrate `djcraft-core/src/i18n.rs` to load TSV catalogs via `craft-i18n` and `CRAFT_I18N_DIR`.
4. **Measured Parity & Gaps**: Add `docs/target-app-parity.md` (scored by `cargo xtask parity`) and `docs/gaps.md` documenting honest parity % against reference DJ workflows (audio DSP, crate interop, MIDI, DVS, stems, video, recording).
5. **Packaging & CI**: Add `packaging/env.sh` and `craft-common-actions` workflow callers gated with `if: github.repository_owner == 'storytold'`.

### Phase 4: Visual Proof & Beta Submission (`https://getartcraft.com/beta`)
1. Run `scripts\capture-screenshots.ps1` on desktop to generate real dark + light screenshots in `docs/images/` (`djcraft-dark.png`, `djcraft-light.png`, `djcraft-4deck.png`, `djcraft-crates.png`) and embed them in `README.md`.
2. Cut a full (non-prerelease) `v0.3.0` release once tests >= 60 and Phase 1-2 gates pass.
3. Submit at **`https://getartcraft.com/beta`** (4 fields, no website required):
   - **App name**: `DJCraft`
   - **Email**: *(Sandra's contact email)*
   - **Repo**: `https://github.com/sandraschi/djcraft`
   - **Description**: `Local-first, 100% Safe Rust 4-deck DJ workstation (egui/wgpu + cpal + symphonia) with live vinyl platter scratching, WSOLA keylock, 3-band kill EQ, dual-bus Headphone PFL pre-cue, lossless WAV/FLAC mix recording, 2-pane drag-drop crate prep with Serato/VirtualDJ interop, transparent 4-stem mixing, USB MIDI controllers, native video mixer, and unified GUI/CLI/MCP command bus.`

# Storytold Beta Readiness TODO - DJCraft

**Target:** Submit to https://getartcraft.com/beta (Tier A Candidate #2 - highest category visibility)
**Audited:** 2026-11-21 (16.3k LOC, 57 tests)
**Reference:** `mcp-central-docs/integrations/storytold-artcraft/BETA_CANDIDATES.md`

---

## P0 - Hard Submission Blockers (Must Complete Before Beta Form)

- [ ] **Remove sibling `path` dependencies (S15)**
  - Currently `Cargo.toml` references `../databasecraft/crates/databasecraft-core` and `../databasecraft/crates/databasecraft-store`.
  - Replace with either self-contained SQLite (`rusqlite`) crate storage or pinned git dependencies so `git clone` + `cargo build` succeeds on a clean machine with zero sibling repos.
- [ ] **Cross-platform audio backend**
  - Currently WASAPI-focused; Storytold builds and ships Windows + macOS + Linux.
  - Ensure `cpal` (or CoreAudio/PipeWire/ALSA + WASAPI) compiles and passes smoke tests on Linux and macOS CI runners, with a headless/null audio fallback for CI.
- [ ] **Legal & governance files**
  - [x] `LICENSE-MIT` and `LICENSE-APACHE` present.
  - [ ] Add `NOTICE` (explicit non-affiliation disclaimer with Storytold / third-party DJ brands).
  - [ ] Add `ATTRIBUTION.md` (every dependency and audio/font/icon asset license).
  - [ ] Verify every crate in `Cargo.lock` matches Storytold `standards/licences.md` SPDX allowlist (zero GPL/LGPL/AGPL linked or bundled).
- [ ] **RFC-0029 trademark & naming hygiene (`docs/feature-names.md`)**
  - Audit all UI skin names, menus, tooltips, READMEs, and screenshot filenames.
  - Rename any third-party trademarked skin names (e.g. `live-console-rane.png` / "Rane" skin -> generic name like "Battle Console" or "Classic Turntable").
  - Keep factual file/hardware interoperability statements (`Import Serato .crate`, `VirtualDJ`, MIDI controller profiles) clearly framed as factual interop.
- [ ] **Decouple personal fleet hub hooks**
  - Make local Ollama / Speech / SFX / VFX / Butterchurn fleet ports strictly optional and disabled by default so first-run experience has zero broken localhost connections.

## P1 - Storytold Engineering & CI Bar

- [ ] **Add `AGENTS.md`** based on `storytold/craft-contrib/standards/app-agents.md` (RFC-0072) with `## This repository` section.
- [ ] **Add `xtask` (`cargo xtask ci`)**: `fmt --check`, `clippy --all-targets -- -D warnings` (enforcing `#![forbid(unsafe_code)]`, `unwrap_used`, `expect_used`, `panic`), `cargo test`, crate layering check, and SPDX license allowlist check.
- [ ] **Add GitHub Actions CI & Release workflows (`.github/workflows/ci.yml`, `release.yml`)**
  - Matrix build + test on `ubuntu-latest`, `macos-latest`, `windows-latest`.
  - Release workflow publishes signed/checksummed native binaries + `SHA256SUMS.txt`.
- [ ] **Add `packaging/env.sh`** (app ID, display name, description, binary names).
- [ ] **Add `docs/target-app-parity.md` and `docs/gaps.md`** (honest Implemented / Partial / Planned matrix).

## P2 - Polish, Accessibility & Evidence

- [ ] **Real desktop screenshots (dark + light modes)** committed to `docs/images/` and linked from `README.md` (currently 0 images committed in repo).
- [ ] **Accessibility check (RFC-0014)**: keyboard shortcut path for every deck/mixer command, numeric entry alternatives for rotary knobs/faders, high-contrast light mode.
- [ ] **Clean fresh-clone verification**: clone to a temp directory, run `cargo xtask ci` and `--smoke`, confirm zero warnings and zero stubs on public paths.
- [ ] **Submit** 4-field form at https://getartcraft.com/beta (App name, Email, Repo URL, Description).

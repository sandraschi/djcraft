## [0.2.1] - 2026-11-21

### Highlights
- **Per-File Autosaved Hot Cues (`crates.db`)**: Hot cues (slots 1-4) are now persisted per audio track path in SQLite (`track_cues` table) when set and automatically restored whenever that track is loaded onto any deck.
- **Booth + Crate Split Workspace (`--booth-crate`)**: Upper performance booth with center vertical waveforms, compact `78px` platters, and hideable `FX Rack` / `Sampler` / `Video` strip, paired with a lower 2-pane crate browser that auto-selects the prepared gig crate.
- **Cross-Platform & Storytold Beta Polish**: Verified `xtask ci` green across `ubuntu-latest`, `macos-latest`, and `windows-latest`, scrubbed RFC-0029 UI/i18n naming, gated external bridges opt-in, added AccessKit `Role::Slider` + keyboard nudge on rotary knobs, and fixed light-mode contrast + header truncation.

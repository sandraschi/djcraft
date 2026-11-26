# `djcraft-app` (L3 Native egui 4-Deck DJ Workstation)

Native pure-Rust DJ workstation (`djcraft.exe`) calling `djcraft-core` and `djcraft-audio` in-process:

- 4-deck live console with 3-band RGB & overview waveforms, phrase ribbons, 2-pane Crates & Gig Prep UI, and `BOOTH + CRATE` split view.
- 9 skins (Battle Dark, Star Trek LCARS + Starfield Warp, Baroque Velvet & Gold, Synthwave, Daylight, etc.), 5 card decor styles, 6 backdrops, 7 layout presets, and Full-Window Settings (`Ctrl+,`).
- Multi-provider AI Chat (`Ctrl+L`) and iPad Multi-Touch Remote PWA v2 (live overview waveforms, master VU, SynthCraft pads, stem crossfader, PIN-protected on `:14119`).

```powershell
cargo run -p djcraft-app -- --smoke
```

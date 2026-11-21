## [0.2.2] - 2026-11-22

### Highlights
- **DVS Timecode Vinyl (`djcraft-audio/src/dvs.rs`)**: 100% Safe Rust quadrature stereo decoder (`1000 Hz` / `2000 Hz` control vinyl) + live `cpal` audio input routing speed, direction, and pitch directly to Deck A/B.
- **Pure-Rust UDP Multicast Link (`djcraft-core/src/link.rs`)**: Clean-room beat & tempo sync (`224.76.78.75:20808`, zero GPL) with a top-bar `LINK` toggle pill and peer counter.
- **3-Band RGB Spectral Waveforms (`dsp.rs`, `widgets.rs`)**: `<250 Hz` Crimson/Red, `250-2500 Hz` Emerald/Lime, `>2500 Hz` Electric Cyan with white-hot transient cores, beatgrid lines, loop highlights, and colored cue flags.
- **Daylight Black Text & Visible Backdrops (`skins.rs`)**: Pure black (`#000000`) primary text in Daylight Booth mode and high-contrast `BrushedMetal` / `CarbonWeave` textures.

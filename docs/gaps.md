# DJCraft Known Gaps

1. **Cross-platform CI verification**: `cpal` and `midir` support Linux (ALSA/PipeWire), macOS (CoreAudio/CoreMIDI), and Windows (WASAPI/WinMM), but Linux/macOS CI runs have not yet been verified on GitHub Actions.
2. **Desktop screenshots in repo**: `scripts/capture-screenshots.ps1` generates dark + light UI screenshots locally, but `docs/images/` does not yet contain committed PNGs for `README.md`.
3. **Optional local AI/fleet hooks**: AI Booth Chat and external UDP video/SFX bridges require user-configured local services and are optional.
4. **Accessibility tree & keyboard focus audit**: Needs full RFC-0014 `kittest` keyboard/contrast pass across all deck/mixer controls.

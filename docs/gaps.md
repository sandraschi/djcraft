# DJCraft Known Gaps

1. **Cross-platform CI verification**: `cpal` and `midir` support Linux (ALSA/PipeWire), macOS (CoreAudio/CoreMIDI), and Windows (WASAPI/WinMM); GitHub Actions matrix (`ubuntu-latest`, `macos-latest`, `windows-latest`) configured in `.github/workflows/ci.yml`.
2. **Optional local external bridges**: AI Booth Chat and external Speech/SFX/VFX/Visualizer/OSC bridges are disabled by default and require user-configured local services.
3. **Accessibility tree & keyboard focus audit**: Needs full RFC-0014 `kittest` keyboard/contrast pass across all deck/mixer controls (including light-mode contrast polish on secondary mixer labels).

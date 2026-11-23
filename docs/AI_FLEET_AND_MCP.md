# AI Booth Chat, Fleet Hub, CLI & MCP Server

Back to [README.md](../README.md)

---

## 1. AI Booth Chat & Harmonic Setlist Planner (`fleet_ai.rs`)

Open the **AI Chat & Workflows** dock tab:
- **Instant Local Natural-Language Commands**: Type `play a`, `play b`, `sync`, `phase sync`, `kill bass a`, `center crossfader`, `mute`, `reset stems`, or `stop all` for immediate L1 execution without needing an external LLM.
- **Multi-Provider LLM Chat (Local & Cloud)**: Select your provider from the **Provider** dropdown (**Ollama (Local)**, **LM Studio (Local)**, **OpenAI**, **Anthropic**, **Google Gemini**, **OpenRouter**, or **Custom OpenAI-Compatible**). Click **Refresh** (or switch provider) to auto-populate the **Model** dropdown live from Ollama `/api/tags` or `/v1/models`, enter a custom tag if needed, and supply a password-masked **API Key** (or rely on automatic `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` / `GEMINI_API_KEY` / `OPENROUTER_API_KEY` env-var fallback). Live booth state (loaded tracks, BPM, Camelot keys, crossfader) is injected as system context.
- **Camelot Harmonic Setlist Builder**: Uses a Warnsdorff-style degree heuristic over your SQLite crate store to build dead-end-free harmonic progressions starting from any seed key.

---

## 1.5. iPad / Tablet Multi-Touch Remote (LAN PWA, `touch_remote.rs`)

Control DJCraft over local Wi-Fi from any iPad or tablet browser — zero Apple App Store or Xcode required:
- Open **Settings (`Ctrl+,`) -> AI & External Bridges -> Start iPad Touch Remote** (default port `14119`, disabled by default so standalone runs bind zero LAN ports).
- Open the displayed LAN URL (e.g. `http://192.168.x.x:14119`) in **iPad Safari -> Share -> Add to Home Screen** for fullscreen landscape multi-touch mode.
- **3 Multi-Touch Tabs (`PointerEvent` per finger)**:
  - **MIXER & DECKS**: Deck A/B transport, `LOOP 4`, Beatjump (`-4/-1/+1/+4`), custom-colored Hot Cue pads, Bipolar Color Filter & Bass Kill sliders, vertical `CH A / MST / CH B` faders, full-width Crossfader, `SYNC`, `MUTE`.
  - **SAMPLER & FX**: 4x8 Sampler pads (Bank A..D), Studio Beatmaker toggle, FX 1/2 toggles, and a live **XY Touch Pad** (`X` = FX 1 Wet/Dry, `Y` = Deck A Color Filter).
  - **CRATES**: Searchable live track list from `crates.db` with one-tap `-> A` and `-> B` deck loaders.
- **Using Outside the LAN (Tailscale / Tailscale Funnel)**:
  - **Tailnet Members Only (Recommended)**: With Tailscale installed on both the host PC and iPad, open `http://<host-tailscale-ip>:14119` directly, or run `tailscale serve --bg 14119` for automatic HTTPS (`https://<machine>.<tailnet>.ts.net`) restricted to your tailnet.
  - **Public Internet (Tailscale Funnel)**: Set an optional **PIN** right next to the Port field in Settings before clicking **Start iPad Touch Remote**, then run `tailscale funnel --bg 14119` to expose `https://<machine>.<tailnet>.ts.net` publicly (stop anytime with `tailscale funnel off`). When a PIN is set, `/api/state` and `/api/cmd` return `401 Unauthorized` until unlocked with the PIN in the iPad browser.

---

## 2. Fleet Service Hub

Open the **Fleet Hub** dock tab to probe and drive sibling fleet services:

| Service | Default Port | Integration |
| :--- | :--- | :--- |
| `speechcraft` / Windows SAPI | `:14132` (or local CLI) | Synthesizes DJ vocal drop WAVs and loads directly onto Deck C |
| `sfx-mcp` | `:11120` | Searches and triggers one-shot risers, impacts, and sweeps |
| `vfx-mcp` | `:11122` | Queues FFmpeg video effect presets on deck clips |
| `butterchurn-mcp` | `:10878` | Syncs live master BPM to MilkDrop visualizer presets |
| `mixxxxx` | UDP `11119` / `11118` | Video-extended Mixxx OSC 1.0 bridge |
| `dettato` (`reaper-mcp` shared) | Local CLI | Offline audio-to-MIDI analysis (`tuning_cents`, tracks, drums) |

---

## 3. Universal L1 Command Bus, CLI & MCP Stdio

GUI, CLI (`djcraft-cli.exe`), and MCP (`djcraft-mcp.exe`) all route through the same 24-command `Command::dispatch` bus in `crates/djcraft-core/src/command.rs`.

### CLI (`djcraft-cli`)
```powershell
djcraft.exe --smoke
djcraft-cli status
djcraft-cli transition
djcraft-cli camelot 8A 9A
djcraft-cli crate-list
djcraft-cli mixxxxx-ping
djcraft-cli analyze-dettato "D:\audio\track.mp3" --device cuda
```

### MCP Server (`djcraft-mcp`)
- Transport: line-delimited JSON-RPC 2.0 over stdio (`2024-11-05`).
- **10 Tools**:
  - `djcraft_status`
  - `djcraft_plan_transition`
  - `djcraft_camelot_check`
  - `djcraft_commands`
  - `djcraft_analyze_dettato`
  - `djcraft_mixxxxx_ping`
  - `djcraft_mixxxxx_video`
  - `djcraft_crates_list`
  - `djcraft_crate_import`
  - `djcraft_crate_export`

See [docs/CLI_AND_MCP.md](CLI_AND_MCP.md) for the complete 27-command `djcraft-cli` reference, all `config-set` keys, and the full 25-tool `djcraft-mcp` parity table.

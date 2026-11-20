# AI Booth Chat, Fleet Hub, CLI & MCP Server

Back to [README.md](../README.md)

---

## 1. AI Booth Chat & Harmonic Setlist Planner (`fleet_ai.rs`)

Open the **AI Chat & Workflows** dock tab:
- **Instant Local Natural-Language Commands**: Type `play a`, `play b`, `sync`, `phase sync`, `kill bass a`, `center crossfader`, `mute`, `reset stems`, or `stop all` for immediate L1 execution without needing an external LLM.
- **Local OpenAI-Compatible LLM Chat**: Connects to **Ollama** (`http://127.0.0.1:11434/v1/chat/completions`) or **LM Studio** (`:1234`) with live booth state (loaded tracks, BPM, Camelot keys, crossfader) injected as system context.
- **Camelot Harmonic Setlist Builder**: Uses a Warnsdorff-style degree heuristic over your SQLite crate store to build dead-end-free harmonic progressions starting from any seed key.

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

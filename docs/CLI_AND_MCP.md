# DJCraft CLI (`djcraft-cli`) & MCP (`djcraft-mcp`) Reference

Back to [README.md](../README.md) | [HELP.md](../HELP.md) | [AI_FLEET_AND_MCP.md](AI_FLEET_AND_MCP.md)

GUI (`djcraft.exe`), CLI (`djcraft-cli.exe`), and MCP (`djcraft-mcp.exe`) all share the same L1 `Command::dispatch` bus (`crates/djcraft-core/src/command.rs`), `AppConfig` (`%LOCALAPPDATA%\djcraft\ui.json`), `CrateStore` (`%LOCALAPPDATA%\djcraft\crates.db`), and `MidiControllerManager`.

---

## 1. GUI Startup Flags (`djcraft.exe`)

```powershell
djcraft.exe --smoke                    # Headless smoke verification and exit
djcraft.exe --booth-crate              # Launch directly in BOOTH + CRATE split view
djcraft.exe --crates                   # Launch directly in CRATES & GIG PREP view
djcraft.exe --four-deck                # Launch in 4-deck stacked layout
djcraft.exe --skin daylight            # battle | club | midnight | carbon | pink | daylight
```

---

## 2. Full CLI Reference (`djcraft-cli`)

All `djcraft-cli` subcommands emit structured JSON to `stdout` so they can be scripted from PowerShell, Bash, or CI pipelines.

### A. Status, Harmonic Planning & Offline Analysis
| Command | Arguments / Flags | Description |
| :--- | :--- | :--- |
| `status` | - | Inspect 4-deck states, loaded tracks, crossfader, and `dettato` sidecar readiness |
| `transition` | - | Plan harmonic transition between Deck A and Deck B (BPM delta, Camelot compatibility, bars, EQ strategy) |
| `camelot` | `<key_a> <key_b>` | Query Camelot wheel harmonic compatibility between two keys (e.g. `8A 9A`) |
| `commands` | - | List all commands registered on the L1 `Command::dispatch` bus |
| `analyze-dettato` | `<audio_path> [--output-dir <dir>] [--device <auto\|cuda\|cpu>]` | Run local `dettato` audio-to-MIDI offline analysis (`tuning_cents`, tracks, drum hits) |

```powershell
djcraft-cli status
djcraft-cli transition
djcraft-cli camelot 8A 9A
djcraft-cli commands
djcraft-cli analyze-dettato "D:\audio\track.mp3" --device cuda --output-dir "D:\out"
```

### B. Configuration (`%LOCALAPPDATA%\djcraft\ui.json`)
`djcraft-cli` reads and writes the same `ui.json` loaded by `djcraft.exe` on startup (`AppConfig` in `crates/djcraft-core/src/config.rs`). Unknown fields already present in `ui.json` are preserved.

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `config-show` | - | Print all persisted DJCraft settings from `ui.json` |
| `config-set` | `<key> <value>` | Validate, update, and persist any configuration key |
| `config-reset` | - | Reset all configuration keys in `ui.json` to defaults |

**Supported `config-set` Keys:**

| Key (and alias) | Accepted Values | Default |
| :--- | :--- | :--- |
| `skin` | `battle`, `club`, `midnight`, `carbon`, `pink`, `daylight` | `BattleConsole` |
| `backdrop` | `solid`, `metal`, `weave`, `sparkle` | `SparkleClub` |
| `locale` | `en`, `de`, `fr`, `es`, `ja`, `ko`, `zh` | `En` |
| `workspace` | `live`, `booth-crate`, `crates` | `live` |
| `four-deck` (`four_deck_layout`) | `true`/`false`/`on`/`off`/`1`/`0` | `false` |
| `zoom` (`zoom_factor`) | Float clamped to `0.75..=2.0` | `1.0` |
| `pitch-range` (`pitch_range_pct`) | `8`, `16`, `50` | `16.0` |
| `quantize` (`quantize_default`) | Boolean (`true`/`false`/`on`/`off`) | `false` |
| `auto-gain` (`auto_gain_enabled`) | Boolean | `true` |
| `auto-dj-fade` (`auto_dj_fade_sec`) | Float seconds clamped to `2.0..=20.0` | `8.0` |
| `headphone-device` (`headphone_device`) | Output device name substring/exact name, or `none` | `null` |
| `split-cue` (`split_cue`) | Boolean (Left = Mono Cue, Right = Mono Master) | `false` |
| `recording-format` (`recording_format`) | `wav`, `flac` | `wav` |
| `dvs-device` (`dvs_device`) | Input device name, or `none` | `null` |
| `dvs-carrier` (`dvs_carrier`) | `1k` (`1000`), `2k` (`2000`) | `1k` |
| `ai-endpoint` (`ai_endpoint`) | OpenAI-compatible chat completions URL | `http://127.0.0.1:11434/v1/chat/completions` |
| `ai-model` (`ai_model`) | Model identifier string | `qwen2.5:7b` |
| `bridges-enabled` (`bridges_enabled`) | Boolean | `false` |

```powershell
djcraft-cli config-show
djcraft-cli config-set skin midnight
djcraft-cli config-set workspace booth-crate
djcraft-cli config-set headphone-device "Speakers (Reloop Ready)"
djcraft-cli config-set split-cue off
djcraft-cli config-set pitch-range 8
djcraft-cli config-set quantize on
djcraft-cli config-reset
```

### C. Audio Devices, DVS & Link Probes
| Command | Arguments / Flags | Description |
| :--- | :--- | :--- |
| `audio-list` | - | List all available WASAPI/host audio output devices and DVS input devices |
| `dvs-probe` | `[--device <name>] [--carrier <1k\|2k>] [--ms <50..5000>]` | Open a live DVS timecode input stream briefly and report lock, speed ratio, pitch %, and signal dBFS |
| `link-probe` | `[--bpm <f64>] [--ms <100..5000>]` | Enable UDP multicast Link (`224.76.78.75:20808`) briefly and report BPM, beat phase, and peer count |

```powershell
djcraft-cli audio-list
djcraft-cli dvs-probe --carrier 1k --ms 400
djcraft-cli link-probe --bpm 128 --ms 600
```

### D. MIDI Controllers & XML Mappers
| Command | Arguments | Description |
| :--- | :--- | :--- |
| `midi-list` | - | List connected USB MIDI input ports and auto-detected controller profiles |
| `midi-connect` | `[<query>]` | Connect to a MIDI input port by 0-based index or name substring (defaults to port `0`) |
| `midi-profile` | `<profile>` | Activate a built-in profile: `reloop-ready`, `rane-12`, `rane-72`, `ddj-flx4`, `hercules`, `denon`, `numark`, `xone-k2`, `custom` |
| `midi-import-xml` | `<path>` | Import a local VirtualDJ or Mixxx MIDI `.xml` mapper into `midi_learn.json` |
| `midi-rules` | - | Print active MIDI mapping rules (CC/Note -> `MidiTargetAction`) |

```powershell
djcraft-cli midi-list
djcraft-cli midi-connect reloop
djcraft-cli midi-profile reloop-ready
djcraft-cli midi-import-xml "D:\mappers\reloop_ready.xml"
djcraft-cli midi-rules
```

### E. Crates, Folder Scanning, Interop & Autosaved Hot Cues
| Command | Arguments / Flags | Description |
| :--- | :--- | :--- |
| `crate-list` | - | List all persistent SQLite crates and subcrates |
| `crate-create` | `<name> [--parent-id <id>]` | Create a root crate or nested subcrate in SQLite |
| `crate-scan` | `<folder> [--crate-id <id>]` | Scan a folder of audio files into SQLite (creates crate path if `--crate-id` omitted) |
| `crate-import-serato` | `<path>` | Import a Serato DJ `.crate` file (preserves `%%` subcrate hierarchy) |
| `crate-export-serato` | `<crate_id> <dest>` | Export a crate by id to a Serato DJ `.crate` file |
| `crate-import-vdj` | `<path>` | Import a VirtualDJ `.vdjfolder`, `.m3u`, or `.m3u8` playlist |
| `crate-export-vdj` | `<crate_id> <dest>` | Export a crate by id to a VirtualDJ `.vdjfolder` or `.m3u8` file |
| `crate-cues` | `<track_path>` | Show autosaved hot cues (slots `0..3`) for a track path |
| `crate-set-cue` | `<track_path> <slot> <value>` | Set hot cue slot `0..3` in seconds, or pass `clear` to remove it |

```powershell
djcraft-cli crate-list
djcraft-cli crate-create "Peak Hour Techno"
djcraft-cli crate-scan "D:\Music\Techno" --crate-id 2
djcraft-cli crate-cues "D:\Music\Techno\opener.mp3"
djcraft-cli crate-set-cue "D:\Music\Techno\opener.mp3" 0 32.5
djcraft-cli crate-set-cue "D:\Music\Techno\opener.mp3" 0 clear
```

### F. Mixxxxx Video OSC Bridge
| Command | Arguments / Flags | Description |
| :--- | :--- | :--- |
| `mixxxxx-ping` | - | Ping Mixxxxx OSC bridge on UDP `11119` and report local `mixxx.exe` presence |
| `mixxxxx-video` | `[--deck <1..4>] [--enabled <bool>] [--fullscreen <bool>] [--brightness <f32>] [--contrast <f32>] [--saturation <f32>]` | Send `/deck/[N]/video_*` OSC controls to Mixxxxx |

### G. Live Remote Control of Running `djcraft.exe` (Loopback IPC)
When `djcraft.exe` is running, it binds a loopback TCP listener (`127.0.0.1:<ephemeral_port>`) and writes `%LOCALAPPDATA%\djcraft\live.port`. All commands below act directly on the live GUI, audio engine, and decks in real time:

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `live-status` | - | Query live 4-deck playback, mixer, audio device, MIDI, DVS, and Link state |
| `dispatch` | `<json>` | Dispatch any serialized `Command` JSON object (EQ, stems, FX, cues, loops, beatjump, etc.) |
| `load` | `<deck> <path>` | Decode and load a local audio file onto Deck `a`/`b`/`c`/`d` (`1..4`) |
| `play` | `<deck>` | Toggle Play/Pause on Deck `a`/`b`/`c`/`d` |
| `cue` | `<deck>` | Cue return on Deck `a`/`b`/`c`/`d` |
| `stop-all` | - | Stop all 4 decks |
| `sync` | - | Phase-sync Deck B to Deck A beatgrid |
| `crossfader` | `<value>` | Set crossfader (`-1.0..=1.0`) |
| `volume` | `<deck> <value>` | Set channel volume (`0.0..=1.0`) on Deck `a`/`b`/`c`/`d` |
| `master` | `<value>` | Set master output volume (`0.0..=1.0`) |
| `pfl` | `<deck>` | Toggle headphone PFL cue on Deck `a`/`b`/`c`/`d` |
| `mute` | - | Toggle master mute |
| `load-video` | `<deck> <path>` | Load a video file onto Video Deck `a` or `b` and show Live Video Strip |
| `run-set` | `<path> [--speed <f64>]` | Play back a timed `.djset.json` set script (tracks, FX, transitions, video) in real time |
| `macro-rec` | `<start\|stop\|status> [--save <path>]` | Start/stop/inspect in-app live macro recording and optionally save to `.djset.json` |
| `set-play` | `<start\|stop\|status> [--path <file>] [--speed <f64>]` | Control the non-blocking in-app Set Player (`Set & Two-Room` dock tab) |
| `room2` | `<status\|configure\|play\|pause\|stop\|next> [--device <name\|off>] [--crate-id <id>] [--volume <0..1>] [--shuffle <bool>]` | Control Two-Room Setup Room 2 prepared autoplay (independent output device & crate) |
| `lighting` | `<status\|configure\|on\|off\|blackout> [--protocol <artnet\|sacn>] [--target-ip <ip>] [--universe <u16>] [--mode <auto\|chill\|techno\|laser\|blackout>] [--dimmer <0..1>] [--strobe <bool>]` | Control AI Lighting Assistant (Art-Net 4 / sACN DMX512) |

```powershell
djcraft-cli live-status
djcraft-cli load a "D:\Music\Techno\opener.mp3"
djcraft-cli play a
djcraft-cli volume a 0.9
djcraft-cli crossfader -0.5
djcraft-cli dispatch '{"type":"set_stem_gains","deck":"DeckA","vocals":1.0,"drums":1.0,"bass":0.0,"other":1.0}'
djcraft-cli stop-all
```

---

## 3. MCP Server (`djcraft-mcp`) - 35 Tools (100% CLI & Live Remote Parity)

- **Transport**: line-delimited JSON-RPC 2.0 over stdio (`protocolVersion: "2026-07-28"`).
- Every `djcraft-cli` command has a direct MCP tool equivalent:

| MCP Tool | CLI Equivalent | Key Arguments |
| :--- | :--- | :--- |
| `djcraft_status` | `status` | - |
| `djcraft_plan_transition` | `transition` | - |
| `djcraft_camelot_check` | `camelot` | `key_a`, `key_b` |
| `djcraft_commands` | `commands` | - |
| `djcraft_analyze_dettato` | `analyze-dettato` | `audio_path`, `output_dir?`, `device?` |
| `djcraft_mixxxxx_ping` | `mixxxxx-ping` | - |
| `djcraft_mixxxxx_video` | `mixxxxx-video` | `deck`, `enabled?`, `fullscreen?`, `brightness?`, `contrast?`, `saturation?` |
| `djcraft_audio_list` | `audio-list` | - |
| `djcraft_config_show` | `config-show` | - |
| `djcraft_config_set` | `config-set` | `key`, `value` |
| `djcraft_config_reset` | `config-reset` | - |
| `djcraft_midi_list` | `midi-list` | - |
| `djcraft_midi_connect` | `midi-connect` | `query?` |
| `djcraft_midi_profile` | `midi-profile` | `profile` |
| `djcraft_midi_import_xml` | `midi-import-xml` | `path` |
| `djcraft_midi_rules` | `midi-rules` | - |
| `djcraft_crates_list` | `crate-list` | - |
| `djcraft_crate_create` | `crate-create` | `name`, `parent_id?` |
| `djcraft_crate_scan` | `crate-scan` | `folder`, `crate_id?` |
| `djcraft_crate_import` | `crate-import-serato` / `crate-import-vdj` | `format` (`serato` \| `vdj`), `path` |
| `djcraft_crate_export` | `crate-export-serato` / `crate-export-vdj` | `format` (`serato` \| `vdj`), `crate_id`, `dest` |
| `djcraft_crate_cues` | `crate-cues` | `track_path` |
| `djcraft_crate_set_cue` | `crate-set-cue` | `track_path`, `slot` (`0..3`), `value` (seconds or `"clear"`) |
| `djcraft_dvs_probe` | `dvs-probe` | `device?`, `carrier?` (`1k` \| `2k`), `ms?` |
| `djcraft_link_probe` | `link-probe` | `bpm?`, `ms?` |
| `djcraft_live_status` | `live-status` | - |
| `djcraft_dispatch` | `dispatch` | `command` (any serialized `Command` JSON object) |
| `djcraft_load_file` | `load` | `deck` (`a`/`b`/`c`/`d`), `path` |
| `djcraft_deck_control` | `play` / `cue` / `stop-all` / `sync` / `pfl` / `volume` | `deck`, `action?`, `volume?`, `pitch_bend?` |
| `djcraft_mixer_control` | `crossfader` / `master` / `mute` | `crossfader?`, `master_volume?`, `mute?` |
| `djcraft_run_set` | `run-set` | `path` (`.djset.json`), `speed?` |
| `djcraft_macro_record` | `macro-rec` | `action` (`start` \| `stop` \| `status`), `save_path?` |
| `djcraft_set_playback` | `set-play` | `action` (`start` \| `stop` \| `status`), `path?`, `speed?` |
| `djcraft_room2` | `room2` | `action` (`status` \| `configure` \| `play` \| `pause` \| `stop` \| `next`), `device?`, `crate_id?`, `volume?`, `shuffle?` |
| `djcraft_lighting` | `lighting` | `action` (`status` \| `configure` \| `on` \| `off` \| `blackout`), `protocol?`, `target_ip?`, `universe?`, `mode?`, `dimmer?`, `strobe?` |

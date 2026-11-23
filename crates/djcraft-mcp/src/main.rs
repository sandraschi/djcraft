#![forbid(unsafe_code)]
#![recursion_limit = "256"]
//! DJCraft MCP Server (stdio, MCP 2026-07-28).
//!
//! Line-delimited JSON-RPC: stdout is 100% pure JSON-RPC, every diagnostic
//! goes to stderr. All tools dispatch through `DjOrchestrator::dispatch`.

use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Duration;

use djcraft_audio::output::AudioOutput;
use djcraft_audio::{list_input_devices, DeckId, DvsCarrier, DvsInputStream};
use djcraft_core::{
    ipc::{parse_deck, run_set_script, send_ipc_request, IpcRequest, SetScript},
    AppConfig, Command, ControllerProfileId, CrateStore, DjOrchestrator, LinkEngine,
    MidiControllerManager, MixxxxxVideoControls,
};
use serde_json::{json, Value};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    eprintln!("[djcraft-mcp] Starting MCP stdio loop");
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut orch = DjOrchestrator::new();

    for line in stdin.lock().lines() {
        let raw = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                });
                let _ = writeln!(stdout, "{err_resp}");
                let _ = stdout.flush();
                continue;
            }
        };

        if parsed.get("id").is_none() {
            continue;
        }

        let id = parsed.get("id").cloned().unwrap_or(Value::Null);
        let method = parsed.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = parsed.get("params").cloned().unwrap_or_else(|| json!({}));

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2026-07-28",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "djcraft", "version": VERSION }
                }
            }),
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        {
                            "name": "djcraft_status",
                            "description": "Inspect 4-deck states, loaded tracks, crossfader, and dettato sidecar readiness.",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_plan_transition",
                            "description": "Plan harmonic transition between Deck A and Deck B (BPM delta, Camelot compatibility, bars, EQ strategy).",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_camelot_check",
                            "description": "Query Camelot wheel harmonic compatibility between two musical keys (e.g. 8A, 9A).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "key_a": { "type": "string" },
                                    "key_b": { "type": "string" }
                                },
                                "required": ["key_a", "key_b"]
                            }
                        },
                        {
                            "name": "djcraft_commands",
                            "description": "List all commands registered on the DJCraft L1 Command::dispatch bus.",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_analyze_dettato",
                            "description": "Run local dettato audio-to-MIDI offline track analysis (tuning_cents, instrument tracks, drum hits).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "audio_path": { "type": "string" },
                                    "output_dir": { "type": "string" },
                                    "device": { "type": "string", "enum": ["auto", "cuda", "cpu"] }
                                },
                                "required": ["audio_path"]
                            }
                        },
                        {
                            "name": "djcraft_mixxxxx_ping",
                            "description": "Ping the Mixxxxx video-extended Mixxx OSC bridge on UDP 11119 and report local mixxx.exe presence.",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_mixxxxx_video",
                            "description": "Send video OSC controls (/deck/[N]/video_*) to Mixxxxx for deck 1-4.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "deck": { "type": "integer", "minimum": 1, "maximum": 4 },
                                    "enabled": { "type": "boolean" },
                                    "fullscreen": { "type": "boolean" },
                                    "brightness": { "type": "number" },
                                    "contrast": { "type": "number" },
                                    "saturation": { "type": "number" }
                                },
                                "required": ["deck"]
                            }
                        },
                        {
                            "name": "djcraft_crates_list",
                            "description": "List all persistent SQLite crates and subcrates in DJCraft.",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_crate_import",
                            "description": "Import a Serato DJ (.crate) or VirtualDJ (.vdjfolder / .m3u / .m3u8) crate into DJCraft.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "format": { "type": "string", "enum": ["serato", "vdj"] },
                                    "path": { "type": "string" }
                                },
                                "required": ["format", "path"]
                            }
                        },
                        {
                            "name": "djcraft_crate_export",
                            "description": "Export a DJCraft crate by id to Serato DJ (.crate) or VirtualDJ (.vdjfolder / .m3u8).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "format": { "type": "string", "enum": ["serato", "vdj"] },
                                    "crate_id": { "type": "integer" },
                                    "dest": { "type": "string" }
                                },
                                "required": ["format", "crate_id", "dest"]
                            }
                        },
                        {
                            "name": "djcraft_audio_list",
                            "description": "List available audio output devices and DVS input devices.",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_config_show",
                            "description": "Show all persisted DJCraft settings from ui.json.",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_config_set",
                            "description": "Set any DJCraft configuration key in ui.json (e.g. skin, backdrop, locale, workspace, headphone-device, split-cue, recording-format, dvs-device, dvs-carrier, pitch-range, quantize, ai-endpoint, ai-model).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "key": { "type": "string" },
                                    "value": { "type": "string" }
                                },
                                "required": ["key", "value"]
                            }
                        },
                        {
                            "name": "djcraft_config_reset",
                            "description": "Reset all DJCraft configuration keys in ui.json to defaults.",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_midi_list",
                            "description": "List connected USB MIDI controller input ports and auto-detected profiles.",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_midi_connect",
                            "description": "Connect to a MIDI controller input port by index or name substring (e.g. 'reloop').",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "query": { "type": "string" }
                                }
                            }
                        },
                        {
                            "name": "djcraft_midi_profile",
                            "description": "Set active built-in MIDI controller profile (reloop-ready, rane-12, rane-72, ddj-flx4, hercules, denon, numark, xone-k2, custom).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "profile": { "type": "string" }
                                },
                                "required": ["profile"]
                            }
                        },
                        {
                            "name": "djcraft_midi_import_xml",
                            "description": "Import a VirtualDJ or Mixxx MIDI XML mapper into midi_learn.json.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "path": { "type": "string" }
                                },
                                "required": ["path"]
                            }
                        },
                        {
                            "name": "djcraft_midi_rules",
                            "description": "List active MIDI mapping rules.",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_crate_create",
                            "description": "Create a persistent crate or subcrate in SQLite.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "parent_id": { "type": "integer" }
                                },
                                "required": ["name"]
                            }
                        },
                        {
                            "name": "djcraft_crate_scan",
                            "description": "Scan a local audio folder into SQLite crates.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "folder": { "type": "string" },
                                    "crate_id": { "type": "integer" }
                                },
                                "required": ["folder"]
                            }
                        },
                        {
                            "name": "djcraft_crate_cues",
                            "description": "Show autosaved hot cues (slots 0..3) for a track path.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "track_path": { "type": "string" }
                                },
                                "required": ["track_path"]
                            }
                        },
                        {
                            "name": "djcraft_crate_set_cue",
                            "description": "Set or clear an autosaved hot cue (slot 0..3, value = seconds or 'clear').",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "track_path": { "type": "string" },
                                    "slot": { "type": "integer", "minimum": 0, "maximum": 3 },
                                    "value": { "type": "string" }
                                },
                                "required": ["track_path", "slot", "value"]
                            }
                        },
                        {
                            "name": "djcraft_dvs_probe",
                            "description": "Probe live DVS timecode vinyl input signal.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "device": { "type": "string" },
                                    "carrier": { "type": "string", "enum": ["1k", "2k"] },
                                    "ms": { "type": "integer", "minimum": 50, "maximum": 5000 }
                                }
                            }
                        },
                        {
                            "name": "djcraft_link_probe",
                            "description": "Probe UDP multicast Link peers (224.76.78.75:20808).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "bpm": { "type": "number" },
                                    "ms": { "type": "integer", "minimum": 100, "maximum": 5000 }
                                }
                            }
                        },
                        {
                            "name": "djcraft_live_status",
                            "description": "Query live 4-deck, mixer, audio, MIDI, DVS, and Link state from the running djcraft.exe GUI.",
                            "inputSchema": { "type": "object", "properties": {} }
                        },
                        {
                            "name": "djcraft_dispatch",
                            "description": "Dispatch any serialized L1 Command JSON object to the running djcraft.exe GUI (full remote control).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "command": { "type": "object" }
                                },
                                "required": ["command"]
                            }
                        },
                        {
                            "name": "djcraft_load_file",
                            "description": "Decode and load a local audio file onto a deck (a/b/c/d) in the running djcraft.exe GUI.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "deck": { "type": "string", "enum": ["a", "b", "c", "d"] },
                                    "path": { "type": "string" }
                                },
                                "required": ["deck", "path"]
                            }
                        },
                        {
                            "name": "djcraft_deck_control",
                            "description": "Control deck transport/pitch/PFL/sync in the running djcraft.exe GUI.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "deck": { "type": "string", "enum": ["a", "b", "c", "d"] },
                                    "action": { "type": "string", "enum": ["play_pause", "cue_return", "stop_all", "sync", "pfl", "keylock", "slip"] },
                                    "volume": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                                    "pitch_bend": { "type": "number" }
                                },
                                "required": ["deck"]
                            }
                        },
                        {
                            "name": "djcraft_mixer_control",
                            "description": "Control crossfader, master volume, or master mute in the running djcraft.exe GUI.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "crossfader": { "type": "number", "minimum": -1.0, "maximum": 1.0 },
                                    "master_volume": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                                    "mute": { "type": "boolean" }
                                }
                            }
                        },
                        {
                            "name": "djcraft_run_set",
                            "description": "Play back a timed .djset.json set script (tracks, FX, transitions, video) in the running djcraft.exe GUI.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "path": { "type": "string" },
                                    "speed": { "type": "number" }
                                },
                                "required": ["path"]
                            }
                        },
                        {
                            "name": "djcraft_macro_record",
                            "description": "Control live macro recording in djcraft.exe (start, stop with optional save_path, or status).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "action": { "type": "string", "enum": ["start", "stop", "status"] },
                                    "save_path": { "type": "string" }
                                },
                                "required": ["action"]
                            }
                        },
                        {
                            "name": "djcraft_set_playback",
                            "description": "Control the in-app Set Player in djcraft.exe (start with optional path/speed, stop, or status).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "action": { "type": "string", "enum": ["start", "stop", "status"] },
                                    "path": { "type": "string" },
                                    "speed": { "type": "number" }
                                },
                                "required": ["action"]
                            }
                        },
                        {
                            "name": "djcraft_room2",
                            "description": "Control Two-Room Setup Room 2 prepared autoplay (status, configure, play, pause, stop, next) with independent output device and crate.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "action": { "type": "string", "enum": ["status", "configure", "play", "pause", "stop", "next"] },
                                    "device": { "type": "string" },
                                    "crate_id": { "type": "integer" },
                                    "volume": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                                    "shuffle": { "type": "boolean" }
                                },
                                "required": ["action"]
                            }
                        },
                        {
                            "name": "djcraft_lighting",
                            "description": "Control AI Lighting Assistant (Art-Net 4 / sACN E1.31 DMX512) in djcraft.exe: status, configure, on, off, blackout.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "action": { "type": "string", "enum": ["status", "configure", "on", "off", "blackout"] },
                                    "enabled": { "type": "boolean" },
                                    "protocol": { "type": "string", "enum": ["artnet", "sacn"] },
                                    "target_ip": { "type": "string" },
                                    "universe": { "type": "integer", "minimum": 0, "maximum": 255 },
                                    "mode": { "type": "string", "enum": ["auto", "chill", "techno", "laser", "blackout"] },
                                    "dimmer": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                                    "strobe": { "type": "boolean" }
                                },
                                "required": ["action"]
                            }
                        }
                    ]
                }
            }),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let tool_output = match name {
                    "djcraft_status" => match orch.dispatch(Command::Status) {
                        Ok(out) => out.data,
                        Err(e) => json!({ "error": e.to_string() }),
                    },
                    "djcraft_plan_transition" => match orch.dispatch(Command::PlanTransition) {
                        Ok(out) => out.data,
                        Err(e) => json!({ "error": e.to_string() }),
                    },
                    "djcraft_camelot_check" => {
                        let key_a = args.get("key_a").and_then(|k| k.as_str()).unwrap_or("");
                        let key_b = args.get("key_b").and_then(|k| k.as_str()).unwrap_or("");
                        if key_a.trim().is_empty() || key_b.trim().is_empty() {
                            json!({ "error": "key_a and key_b are required" })
                        } else {
                            match orch.dispatch(Command::CheckCamelot {
                                key_a: key_a.to_string(),
                                key_b: key_b.to_string(),
                            }) {
                                Ok(out) => out.data,
                                Err(e) => json!({ "error": e.to_string() }),
                            }
                        }
                    }
                    "djcraft_commands" => {
                        serde_json::to_value(Command::catalog()).unwrap_or_else(|_| json!([]))
                    }
                    "djcraft_analyze_dettato" => {
                        let audio_path = args
                            .get("audio_path")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if audio_path.trim().is_empty() {
                            json!({ "error": "audio_path is required" })
                        } else {
                            let output_dir = args
                                .get("output_dir")
                                .and_then(|v| v.as_str())
                                .map(ToString::to_string);
                            let device = args
                                .get("device")
                                .and_then(|v| v.as_str())
                                .map(ToString::to_string);
                            match orch.dispatch(Command::AnalyzeTrackDettato {
                                audio_path: audio_path.to_string(),
                                output_dir,
                                device,
                            }) {
                                Ok(out) => out.data,
                                Err(e) => json!({ "error": e.to_string() }),
                            }
                        }
                    }
                    "djcraft_mixxxxx_ping" => match orch.dispatch(Command::MixxxxxPing {
                        timeout_ms: Some(250),
                    }) {
                        Ok(out) => out.data,
                        Err(e) => json!({ "error": e.to_string() }),
                    },
                    "djcraft_mixxxxx_video" => {
                        let deck_num = args.get("deck").and_then(|v| v.as_u64()).unwrap_or(1);
                        let deck = match deck_num {
                            1 => DeckId::DeckA,
                            2 => DeckId::DeckB,
                            3 => DeckId::DeckC,
                            4 => DeckId::DeckD,
                            _ => {
                                let err = json!({ "error": format!("Invalid deck {deck_num}; expected 1..=4") });
                                let resp = json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": { "content": [{ "type": "text", "text": err.to_string() }] }
                                });
                                let _ = writeln!(stdout, "{resp}");
                                let _ = stdout.flush();
                                continue;
                            }
                        };
                        let controls = MixxxxxVideoControls {
                            enabled: args.get("enabled").and_then(|v| v.as_bool()),
                            fullscreen: args.get("fullscreen").and_then(|v| v.as_bool()),
                            brightness: args
                                .get("brightness")
                                .and_then(|v| v.as_f64())
                                .map(|v| v as f32),
                            contrast: args
                                .get("contrast")
                                .and_then(|v| v.as_f64())
                                .map(|v| v as f32),
                            saturation: args
                                .get("saturation")
                                .and_then(|v| v.as_f64())
                                .map(|v| v as f32),
                        };
                        match orch.dispatch(Command::MixxxxxVideo { deck, controls }) {
                            Ok(out) => out.data,
                            Err(e) => json!({ "error": e.to_string() }),
                        }
                    }
                    "djcraft_crates_list" => match CrateStore::open_default() {
                        Ok(store) => {
                            let _ = store.seed_demo_if_empty();
                            match store.list_crates() {
                                Ok(list) => {
                                    serde_json::to_value(list).unwrap_or_else(|_| json!([]))
                                }
                                Err(e) => json!({ "error": e.to_string() }),
                            }
                        }
                        Err(e) => json!({ "error": e.to_string() }),
                    },
                    "djcraft_crate_import" => {
                        let fmt = args.get("format").and_then(|v| v.as_str()).unwrap_or("");
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        match CrateStore::open_default() {
                            Ok(store) => {
                                let res = match fmt {
                                    "serato" => store.import_serato_crate(Path::new(path)),
                                    "vdj" => store.import_vdj_file(Path::new(path)),
                                    _ => Err(djcraft_core::DjError::Deck(format!(
                                        "Unsupported format '{fmt}'. Expected serato or vdj."
                                    ))),
                                };
                                match res {
                                    Ok(sum) => {
                                        serde_json::to_value(sum).unwrap_or_else(|_| json!({}))
                                    }
                                    Err(e) => json!({ "error": e.to_string() }),
                                }
                            }
                            Err(e) => json!({ "error": e.to_string() }),
                        }
                    }
                    "djcraft_crate_export" => {
                        let fmt = args.get("format").and_then(|v| v.as_str()).unwrap_or("");
                        let crate_id = args.get("crate_id").and_then(|v| v.as_i64()).unwrap_or(0);
                        let dest = args.get("dest").and_then(|v| v.as_str()).unwrap_or("");
                        match CrateStore::open_default() {
                            Ok(store) => {
                                let res = match fmt {
                                    "serato" => {
                                        store.export_serato_crate(crate_id, Path::new(dest))
                                    }
                                    "vdj" => store.export_vdj_file(crate_id, Path::new(dest)),
                                    _ => Err(djcraft_core::DjError::Deck(format!(
                                        "Unsupported format '{fmt}'. Expected serato or vdj."
                                    ))),
                                };
                                match res {
                                    Ok(cnt) => json!({
                                        "format": fmt,
                                        "crate_id": crate_id,
                                        "dest": dest,
                                        "exported_tracks": cnt
                                    }),
                                    Err(e) => json!({ "error": e.to_string() }),
                                }
                            }
                            Err(e) => json!({ "error": e.to_string() }),
                        }
                    }
                    "djcraft_audio_list" => json!({
                        "outputs": AudioOutput::list_output_devices(),
                        "inputs": list_input_devices()
                    }),
                    "djcraft_config_show" => {
                        serde_json::to_value(AppConfig::load()).unwrap_or_else(|_| json!({}))
                    }
                    "djcraft_config_set" => {
                        let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
                        let value = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        let mut cfg = AppConfig::load();
                        match cfg
                            .set_key(key, value)
                            .and_then(|msg| cfg.save().map(|p| (msg, p)))
                        {
                            Ok((msg, path)) => json!({ "updated": msg, "path": path }),
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_config_reset" => {
                        let cfg = AppConfig::default();
                        match cfg.save() {
                            Ok(path) => json!({ "reset": true, "path": path }),
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_midi_list" => {
                        let ports = MidiControllerManager::list_input_ports();
                        let items: Vec<_> = ports
                            .iter()
                            .enumerate()
                            .map(|(i, p)| {
                                let prof = ControllerProfileId::auto_detect_from_port_name(p);
                                json!({
                                    "index": i,
                                    "port": p,
                                    "detected_profile": prof.label()
                                })
                            })
                            .collect();
                        json!(items)
                    }
                    "djcraft_midi_connect" => {
                        let ports = MidiControllerManager::list_input_ports();
                        if ports.is_empty() {
                            json!({ "error": "No MIDI input ports detected" })
                        } else {
                            let query = args.get("query").and_then(|v| v.as_str());
                            let idx_res: Result<usize, String> = if let Some(q) = query {
                                if let Ok(n) = q.trim().parse::<usize>() {
                                    if n < ports.len() {
                                        Ok(n)
                                    } else {
                                        Err(format!(
                                            "MIDI port index {n} out of range (0..{})",
                                            ports.len()
                                        ))
                                    }
                                } else {
                                    let ql = q.to_lowercase();
                                    ports
                                        .iter()
                                        .position(|p| p.to_lowercase().contains(&ql))
                                        .ok_or_else(|| {
                                            format!(
                                                "No MIDI port matching '{q}' (available: {ports:?})"
                                            )
                                        })
                                }
                            } else {
                                Ok(0)
                            };
                            match idx_res {
                                Ok(idx) => {
                                    let mut mgr = MidiControllerManager::new();
                                    match mgr.connect_port(idx) {
                                        Ok(port_name) => json!({
                                            "connected": true,
                                            "index": idx,
                                            "port": port_name,
                                            "profile": mgr.active_profile.label(),
                                            "rules_count": mgr.rules.len()
                                        }),
                                        Err(e) => json!({ "error": e.to_string() }),
                                    }
                                }
                                Err(e) => json!({ "error": e }),
                            }
                        }
                    }
                    "djcraft_midi_profile" => {
                        let profile = args.get("profile").and_then(|v| v.as_str()).unwrap_or("");
                        let id_res = match profile.trim().to_lowercase().as_str() {
                            "reloop" | "reloop-ready" | "ready" => {
                                Ok(ControllerProfileId::ReloopReady)
                            }
                            "rane-12" | "rane12" | "twelve" => {
                                Ok(ControllerProfileId::RaneTwelveMk2)
                            }
                            "rane-72" | "rane72" | "seventy-two" => {
                                Ok(ControllerProfileId::RaneSeventyTwoMk2)
                            }
                            "pioneer" | "ddj-flx4" | "flx4" => {
                                Ok(ControllerProfileId::PioneerDdjFlx4)
                            }
                            "hercules" | "djcontrol" => {
                                Ok(ControllerProfileId::HerculesDjControl)
                            }
                            "denon" | "sc6000" => Ok(ControllerProfileId::DenonSc6000),
                            "numark" | "mixtrack" => Ok(ControllerProfileId::NumarkMixtrackProFx),
                            "xone" | "xone-k2" | "k2" => {
                                Ok(ControllerProfileId::AllenHeathXoneK2)
                            }
                            "custom" => Ok(ControllerProfileId::CustomLearn),
                            other => Err(format!(
                                "Unknown profile '{other}' (expected: reloop-ready, rane-12, rane-72, ddj-flx4, hercules, denon, numark, xone-k2, custom)"
                            )),
                        };
                        match id_res {
                            Ok(id) => {
                                let mut mgr = MidiControllerManager::new();
                                mgr.set_profile(id);
                                json!({
                                    "profile": mgr.active_profile.label(),
                                    "rules_count": mgr.rules.len()
                                })
                            }
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_midi_import_xml" => {
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        let mut mgr = MidiControllerManager::new();
                        match mgr.import_xml_mapper_file(Path::new(path)) {
                            Ok(count) => json!({
                                "imported_rules": count,
                                "profile": mgr.active_profile.label()
                            }),
                            Err(e) => json!({ "error": e.to_string() }),
                        }
                    }
                    "djcraft_midi_rules" => {
                        let mgr = MidiControllerManager::new();
                        serde_json::to_value(&mgr.rules).unwrap_or_else(|_| json!([]))
                    }
                    "djcraft_crate_create" => {
                        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let parent_id = args.get("parent_id").and_then(|v| v.as_i64());
                        match CrateStore::open_default() {
                            Ok(store) => match store.create_subcrate(parent_id, name) {
                                Ok(crate_id) => json!({
                                    "crate_id": crate_id,
                                    "name": name,
                                    "parent_id": parent_id
                                }),
                                Err(e) => json!({ "error": e.to_string() }),
                            },
                            Err(e) => json!({ "error": e.to_string() }),
                        }
                    }
                    "djcraft_crate_scan" => {
                        let folder = args.get("folder").and_then(|v| v.as_str()).unwrap_or("");
                        let crate_id = args.get("crate_id").and_then(|v| v.as_i64());
                        match CrateStore::open_default() {
                            Ok(store) => {
                                let cid_res = match crate_id {
                                    Some(id) => Ok(id),
                                    None => store.ensure_crate_path(folder),
                                };
                                match cid_res.and_then(|cid| {
                                    store
                                        .scan_audio_folder(cid, Path::new(folder))
                                        .map(|cnt| (cid, cnt))
                                }) {
                                    Ok((cid, cnt)) => json!({
                                        "crate_id": cid,
                                        "folder": folder,
                                        "imported_tracks": cnt
                                    }),
                                    Err(e) => json!({ "error": e.to_string() }),
                                }
                            }
                            Err(e) => json!({ "error": e.to_string() }),
                        }
                    }
                    "djcraft_crate_cues" => {
                        let track_path = args
                            .get("track_path")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        match CrateStore::open_default() {
                            Ok(store) => match store.get_track_hot_cues(track_path) {
                                Ok(cues) => json!({
                                    "track_path": track_path,
                                    "hot_cues": cues
                                }),
                                Err(e) => json!({ "error": e.to_string() }),
                            },
                            Err(e) => json!({ "error": e.to_string() }),
                        }
                    }
                    "djcraft_crate_set_cue" => {
                        let track_path = args
                            .get("track_path")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let slot = args.get("slot").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        let value = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        let pos_res: Result<Option<f64>, String> =
                            if value.trim().eq_ignore_ascii_case("clear") {
                                Ok(None)
                            } else {
                                value
                                    .trim()
                                    .parse::<f64>()
                                    .map(Some)
                                    .map_err(|_| format!("Invalid seconds '{value}' (or 'clear')"))
                            };
                        match pos_res {
                            Ok(pos) => match CrateStore::open_default() {
                                Ok(store) => match store
                                    .set_track_hot_cue(track_path, slot, pos)
                                    .and_then(|()| store.get_track_hot_cues(track_path))
                                {
                                    Ok(cues) => json!({
                                        "track_path": track_path,
                                        "hot_cues": cues
                                    }),
                                    Err(e) => json!({ "error": e.to_string() }),
                                },
                                Err(e) => json!({ "error": e.to_string() }),
                            },
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_dvs_probe" => {
                        let device = args.get("device").and_then(|v| v.as_str());
                        let carrier = args.get("carrier").and_then(|v| v.as_str()).unwrap_or("1k");
                        let ms = args.get("ms").and_then(|v| v.as_u64()).unwrap_or(400);
                        let c = if carrier.trim().eq_ignore_ascii_case("2k") {
                            DvsCarrier::Control2kHz
                        } else {
                            DvsCarrier::Control1kHz
                        };
                        match DvsInputStream::start(device, c) {
                            Ok(stream) => {
                                std::thread::sleep(Duration::from_millis(ms.clamp(50, 5000)));
                                let tel = stream.telemetry();
                                serde_json::to_value(tel).unwrap_or_else(|_| json!({}))
                            }
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_link_probe" => {
                        let bpm = args.get("bpm").and_then(|v| v.as_f64()).unwrap_or(126.0);
                        let ms = args.get("ms").and_then(|v| v.as_u64()).unwrap_or(600);
                        let mut link = LinkEngine::new(bpm);
                        link.set_enabled(true);
                        let steps = (ms.clamp(100, 5000) / 50).max(1);
                        for _ in 0..steps {
                            let _ = link.tick(0.05);
                            std::thread::sleep(Duration::from_millis(50));
                        }
                        json!({
                            "bpm": link.bpm,
                            "beat_phase": link.beat_phase,
                            "peers": link.peer_count()
                        })
                    }
                    "djcraft_live_status" => match send_ipc_request(&IpcRequest::LiveStatus) {
                        Ok(r) if r.ok => r.data,
                        Ok(r) => {
                            json!({ "error": r.error.unwrap_or_else(|| "IPC error".to_string()) })
                        }
                        Err(e) => json!({ "error": e }),
                    },
                    "djcraft_dispatch" => {
                        let cmd_val = args.get("command").cloned().unwrap_or(Value::Null);
                        match serde_json::from_value::<Command>(cmd_val) {
                            Ok(command) => match send_ipc_request(&IpcRequest::Command { command })
                            {
                                Ok(r) if r.ok => r.data,
                                Ok(r) => {
                                    json!({ "error": r.error.unwrap_or_else(|| "IPC error".to_string()) })
                                }
                                Err(e) => json!({ "error": e }),
                            },
                            Err(e) => json!({ "error": format!("Invalid Command JSON: {e}") }),
                        }
                    }
                    "djcraft_load_file" => {
                        let deck_str = args.get("deck").and_then(|v| v.as_str()).unwrap_or("");
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        match parse_deck(deck_str) {
                            Ok(deck) => match send_ipc_request(&IpcRequest::LoadFile {
                                deck,
                                path: path.to_string(),
                            }) {
                                Ok(r) if r.ok => r.data,
                                Ok(r) => {
                                    json!({ "error": r.error.unwrap_or_else(|| "IPC error".to_string()) })
                                }
                                Err(e) => json!({ "error": e }),
                            },
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_deck_control" => {
                        let deck_str = args.get("deck").and_then(|v| v.as_str()).unwrap_or("");
                        match parse_deck(deck_str) {
                            Ok(deck) => {
                                let mut results = Vec::new();
                                if let Some(action) = args.get("action").and_then(|v| v.as_str()) {
                                    let cmd = match action {
                                        "play_pause" => Some(Command::PlayPause { deck }),
                                        "cue_return" => Some(Command::CueReturn { deck }),
                                        "stop_all" => Some(Command::StopAllDecks),
                                        "sync" => Some(Command::PhaseSyncBeatgrid),
                                        "pfl" => Some(Command::TogglePflCue { deck }),
                                        "keylock" => Some(Command::ToggleKeylock { deck }),
                                        "slip" => Some(Command::ToggleSlipMode { deck }),
                                        _ => None,
                                    };
                                    if let Some(command) = cmd {
                                        match send_ipc_request(&IpcRequest::Command { command }) {
                                            Ok(r) if r.ok => results.push(r.data),
                                            Ok(r) => results.push(json!({ "error": r.error })),
                                            Err(e) => results.push(json!({ "error": e })),
                                        }
                                    } else {
                                        results.push(json!({ "error": format!("Unknown action '{action}'") }));
                                    }
                                }
                                if let Some(vol) = args.get("volume").and_then(|v| v.as_f64()) {
                                    match send_ipc_request(&IpcRequest::Command {
                                        command: Command::SetDeckVolume { deck, volume: vol },
                                    }) {
                                        Ok(r) if r.ok => results.push(r.data),
                                        Ok(r) => results.push(json!({ "error": r.error })),
                                        Err(e) => results.push(json!({ "error": e })),
                                    }
                                }
                                if let Some(pct) = args.get("pitch_bend").and_then(|v| v.as_f64()) {
                                    match send_ipc_request(&IpcRequest::Command {
                                        command: Command::SetPitchBend { deck, pct },
                                    }) {
                                        Ok(r) if r.ok => results.push(r.data),
                                        Ok(r) => results.push(json!({ "error": r.error })),
                                        Err(e) => results.push(json!({ "error": e })),
                                    }
                                }
                                json!({ "results": results })
                            }
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_mixer_control" => {
                        let mut results = Vec::new();
                        if let Some(xf) = args.get("crossfader").and_then(|v| v.as_f64()) {
                            match send_ipc_request(&IpcRequest::Command {
                                command: Command::SetCrossfader { value: xf },
                            }) {
                                Ok(r) if r.ok => results.push(r.data),
                                Ok(r) => results.push(json!({ "error": r.error })),
                                Err(e) => results.push(json!({ "error": e })),
                            }
                        }
                        if let Some(mv) = args.get("master_volume").and_then(|v| v.as_f64()) {
                            match send_ipc_request(&IpcRequest::Command {
                                command: Command::SetMasterVolume { volume: mv },
                            }) {
                                Ok(r) if r.ok => results.push(r.data),
                                Ok(r) => results.push(json!({ "error": r.error })),
                                Err(e) => results.push(json!({ "error": e })),
                            }
                        }
                        if args.get("mute").and_then(|v| v.as_bool()) == Some(true) {
                            match send_ipc_request(&IpcRequest::Command {
                                command: Command::ToggleMasterMute,
                            }) {
                                Ok(r) if r.ok => results.push(r.data),
                                Ok(r) => results.push(json!({ "error": r.error })),
                                Err(e) => results.push(json!({ "error": e })),
                            }
                        }
                        json!({ "results": results })
                    }
                    "djcraft_run_set" => {
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        let speed = args.get("speed").and_then(|v| v.as_f64()).unwrap_or(1.0);
                        match std::fs::read_to_string(path)
                            .map_err(|e| format!("Read set script failed: {e}"))
                            .and_then(|raw| {
                                serde_json::from_str::<SetScript>(&raw)
                                    .map_err(|e| format!("Parse set script failed: {e}"))
                            })
                            .and_then(|script| run_set_script(&script, speed))
                        {
                            Ok(v) => v,
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_macro_record" => {
                        let action = args
                            .get("action")
                            .and_then(|v| v.as_str())
                            .unwrap_or("status")
                            .to_string();
                        let save_path = args
                            .get("save_path")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string);
                        match send_ipc_request(&IpcRequest::MacroControl { action, save_path }) {
                            Ok(r) if r.ok => r.data,
                            Ok(r) => {
                                json!({ "error": r.error.unwrap_or_else(|| "IPC error".to_string()) })
                            }
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_set_playback" => {
                        let action = args
                            .get("action")
                            .and_then(|v| v.as_str())
                            .unwrap_or("status")
                            .to_string();
                        let path = args
                            .get("path")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string);
                        let speed = args.get("speed").and_then(|v| v.as_f64());
                        match send_ipc_request(&IpcRequest::SetPlayback {
                            action,
                            path,
                            speed,
                        }) {
                            Ok(r) if r.ok => r.data,
                            Ok(r) => {
                                json!({ "error": r.error.unwrap_or_else(|| "IPC error".to_string()) })
                            }
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_room2" => {
                        let action = args
                            .get("action")
                            .and_then(|v| v.as_str())
                            .unwrap_or("status")
                            .to_string();
                        let device = args
                            .get("device")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string);
                        let crate_id = args.get("crate_id").and_then(|v| v.as_i64());
                        let volume = args
                            .get("volume")
                            .and_then(|v| v.as_f64())
                            .map(|v| v as f32);
                        let shuffle = args.get("shuffle").and_then(|v| v.as_bool());
                        match send_ipc_request(&IpcRequest::Room2Control {
                            action,
                            device,
                            crate_id,
                            volume,
                            shuffle,
                        }) {
                            Ok(r) if r.ok => r.data,
                            Ok(r) => {
                                json!({ "error": r.error.unwrap_or_else(|| "IPC error".to_string()) })
                            }
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "djcraft_lighting" => {
                        let action = args
                            .get("action")
                            .and_then(|v| v.as_str())
                            .unwrap_or("status")
                            .to_string();
                        let enabled = args.get("enabled").and_then(|v| v.as_bool());
                        let protocol = args
                            .get("protocol")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string);
                        let target_ip = args
                            .get("target_ip")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string);
                        let universe = args
                            .get("universe")
                            .and_then(|v| v.as_u64())
                            .map(|v| v as u16);
                        let mode = args
                            .get("mode")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string);
                        let dimmer = args
                            .get("dimmer")
                            .and_then(|v| v.as_f64())
                            .map(|v| v as f32);
                        let strobe = args.get("strobe").and_then(|v| v.as_bool());
                        match send_ipc_request(&IpcRequest::LightingControl {
                            action,
                            enabled,
                            protocol,
                            target_ip,
                            universe,
                            mode,
                            dimmer,
                            strobe,
                        }) {
                            Ok(r) if r.ok => r.data,
                            Ok(r) => {
                                json!({ "error": r.error.unwrap_or_else(|| "IPC error".to_string()) })
                            }
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    _ => json!({ "error": format!("Unknown tool: {name}") }),
                };

                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": serde_json::to_string_pretty(&tool_output).unwrap_or_else(|_| tool_output.to_string())
                            }
                        ]
                    }
                })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            }),
        };

        let _ = writeln!(stdout, "{response}");
        let _ = stdout.flush();
    }
}

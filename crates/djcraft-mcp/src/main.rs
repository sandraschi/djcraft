#![forbid(unsafe_code)]
//! DJCraft MCP Server (stdio, MCP 2024-11-05).
//!
//! Line-delimited JSON-RPC: stdout is 100% pure JSON-RPC, every diagnostic
//! goes to stderr. All tools dispatch through `DjOrchestrator::dispatch`.

use std::path::Path;

use djcraft_audio::DeckId;
use djcraft_core::{Command, CrateStore, DjOrchestrator, MixxxxxVideoControls};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

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
                    "protocolVersion": "2024-11-05",
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

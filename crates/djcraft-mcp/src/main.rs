#![forbid(unsafe_code)]
//! DJCraft MCP Server (stdio, MCP 2024-11-05).
//!
//! Line-delimited JSON-RPC: stdout is 100% pure JSON-RPC, every diagnostic
//! goes to stderr. All tools dispatch through `DjOrchestrator::dispatch`.

use djcraft_core::{Command, DjOrchestrator};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const VERSION: &str = "0.1.0";

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
                        }
                    ]
                }
            }),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
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
                        let audio_path = args.get("audio_path").and_then(|v| v.as_str()).unwrap_or("");
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

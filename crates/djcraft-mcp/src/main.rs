//! DJCraft MCP Server (stdio, MCP 2024-11-05).
//!
//! Line-delimited JSON-RPC: stdout is 100% pure JSON-RPC, every diagnostic
//! goes to stderr. Spawns cleanly per call (initialize -> request -> exit
//! when stdin closes), so MCP clients can one-shot it.

use djcraft_audio::AudioEngine;
use djcraft_core::DjOrchestrator;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const VERSION: &str = "0.1.0";

fn main() {
    eprintln!("[djcraft-mcp] Starting MCP stdio loop");
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

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

        // JSON-RPC notifications (no "id", e.g. notifications/initialized)
        // get NO response — replying desyncs the client's pending request.
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
                            "description": "Inspect deck states, loaded tracks, and crossfader position.",
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
                        }
                    ]
                }
            }),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let orch = DjOrchestrator::new();
                let tool_output = match name {
                    "djcraft_status" => json!({
                        "deck_a": orch.deck_a,
                        "deck_b": orch.deck_b,
                        "crossfader": orch.crossfader,
                    }),
                    "djcraft_plan_transition" => match orch.plan_transition() {
                        Ok(plan) => serde_json::to_value(plan).unwrap_or_else(|e| {
                            json!({ "error": format!("cannot encode plan: {e}") })
                        }),
                        Err(e) => json!({ "error": e.to_string() }),
                    },
                    "djcraft_camelot_check" => {
                        let key_a = args.get("key_a").and_then(|k| k.as_str()).unwrap_or("");
                        let key_b = args.get("key_b").and_then(|k| k.as_str()).unwrap_or("");
                        if key_a.trim().is_empty() || key_b.trim().is_empty() {
                            json!({ "error": "key_a and key_b are required" })
                        } else {
                            json!({
                                "key_a": key_a,
                                "key_b": key_b,
                                "compatible": AudioEngine::is_camelot_compatible(key_a, key_b),
                            })
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

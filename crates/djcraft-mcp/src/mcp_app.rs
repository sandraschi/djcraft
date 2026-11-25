//! SEP-1865 MCP App support for Antigravity (`ui://djcraft/dashboard.html`).

use djcraft_core::{Command, CrateStore, DjOrchestrator};
use serde_json::{json, Value};

pub const APP_URI: &str = "ui://djcraft/dashboard.html";
pub const APP_MIME: &str = "text/html;profile=mcp-app";
pub const TOOL_NAME: &str = "djcraft_dashboard";

const EMBEDDED_HTML: &str = include_str!("dashboard.html");

pub fn resources_list() -> Value {
    json!({
        "resources": [
            {
                "uri": APP_URI,
                "name": "DJCraft Dashboard",
                "description": "Interactive DJCraft 4-deck console, crates, and Camelot transition dashboard.",
                "mimeType": APP_MIME,
                "_meta": { "ui": { "prefersBorder": false } }
            }
        ]
    })
}

pub fn load_html() -> String {
    if let Ok(path) = std::env::var("DJCRAFT_APP_HTML") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            if let Ok(html) = std::fs::read_to_string(trimmed) {
                return html;
            }
        }
    }
    EMBEDDED_HTML.to_string()
}

pub fn resource_read(uri: &str) -> Result<Value, (i32, String)> {
    if uri == APP_URI {
        Ok(json!({
            "contents": [
                {
                    "uri": APP_URI,
                    "mimeType": APP_MIME,
                    "text": load_html(),
                    "_meta": { "ui": { "prefersBorder": false } }
                }
            ]
        }))
    } else {
        Err((-32002, format!("Resource not found: {uri}")))
    }
}

pub fn tool_spec() -> Value {
    json!({
        "name": TOOL_NAME,
        "title": "DJCraft",
        "description": "Open the interactive DJCraft console dashboard (4-deck state, transition planner, SQLite crates, and Camelot wheel).",
        "inputSchema": { "type": "object", "properties": {} },
        "_meta": {
            "ui": { "resourceUri": APP_URI },
            "com.google.antigravity/ui": {
                "entrypoints": [
                    { "type": "aux_pane", "title": "DJCraft" },
                    { "type": "global", "title": "DJCraft" }
                ]
            }
        }
    })
}

pub fn call_dashboard(orch: &mut DjOrchestrator) -> Value {
    let status = match orch.dispatch(Command::Status) {
        Ok(out) => out.data,
        Err(e) => json!({ "error": e.to_string() }),
    };
    let transition = match orch.dispatch(Command::PlanTransition) {
        Ok(out) => out.data,
        Err(e) => json!({ "error": e.to_string() }),
    };
    let crates = match CrateStore::open_default().and_then(|s| s.list_crates()) {
        Ok(list) => serde_json::to_value(list).unwrap_or_else(|_| json!([])),
        Err(_) => json!([]),
    };

    let crate_count = crates.as_array().map(Vec::len).unwrap_or(0);
    let bpm_delta = transition["bpm_delta"].as_f64().unwrap_or(0.0);
    let text = format!(
        "DJCraft console ready: {crate_count} crates, transition BPM delta {bpm_delta:.1}."
    );

    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": {
            "status": status,
            "transition": transition,
            "crates": crates
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn djcraft_mcp_app_metadata_valid() {
        let spec = tool_spec();
        assert_eq!(spec["name"], TOOL_NAME);
        assert_eq!(spec["_meta"]["ui"]["resourceUri"], APP_URI);
        assert!(resource_read(APP_URI).is_ok());
    }
}

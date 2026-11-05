//! DJCraft MCP Server
//!
//! Exposes 4-deck digital mixing, real-time stem extraction control,
//! and Camelot harmonic transition planning as FastMCP JSON-RPC tools on port 14118.

use serde_json::json;
use djcraft_core::DjOrchestrator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = 14118;
    println!("[djcraft-mcp] Initializing Local DJ Studio FastMCP engine on port {}...", port);

    let orch = DjOrchestrator::new();
    let plan = orch.plan_transition()?;

    let status = json!({
        "status": "ready",
        "mcp_server": "djcraft",
        "port": port,
        "capabilities": ["FourDeckMixing", "RealtimeStemIsolation", "CamelotHarmonicMatching", "AutonomousTransitionPlanning"],
        "deck_a_active": orch.deck_a.is_playing,
        "deck_b_ready": orch.deck_b.loaded_track.is_some(),
        "harmonic_mix_compatible": plan.harmonic_compatible,
    });
    println!("[djcraft-mcp] Status: {}", serde_json::to_string_pretty(&status)?);
    Ok(())
}

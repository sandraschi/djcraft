use clap::{Parser, Subcommand};
use djcraft_core::DjOrchestrator;
use djcraft_audio::AudioEngine;

#[derive(Parser)]
#[command(name = "djcraft-cli", about = "DJCraft Sovereign Performance Mixer CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect deck states and active tracks
    Status,
    /// Plan harmonic transition between Deck A and Deck B
    Transition,
    /// Query Camelot wheel harmonic compatibility
    Camelot {
        key_a: String,
        key_b: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let orch = DjOrchestrator::new();

    match cli.command {
        Commands::Status => {
            let payload = serde_json::json!({
                "deck_a": orch.deck_a,
                "deck_b": orch.deck_b,
                "crossfader": orch.crossfader,
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        Commands::Transition => {
            let plan = orch.plan_transition()?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        Commands::Camelot { key_a, key_b } => {
            let compatible = AudioEngine::is_camelot_compatible(&key_a, &key_b);
            let payload = serde_json::json!({
                "key_a": key_a,
                "key_b": key_b,
                "compatible": compatible,
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
    }
    Ok(())
}

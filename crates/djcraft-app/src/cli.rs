#![forbid(unsafe_code)]
use clap::{Parser, Subcommand};
use djcraft_core::{Command, DjOrchestrator};

#[derive(Parser)]
#[command(name = "djcraft-cli", about = "DJCraft Local Performance Mixer CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect 4-deck states, crossfader, and sidecar readiness
    Status,
    /// Plan harmonic transition between Deck A and Deck B
    Transition,
    /// Query Camelot wheel harmonic compatibility
    Camelot { key_a: String, key_b: String },
    /// List the universal Command::dispatch catalog
    #[command(name = "commands")]
    Catalog,
    /// Run local dettato audio-to-MIDI offline track analysis
    AnalyzeDettato {
        /// Path to local audio file (.mp3, .wav, .flac, .ogg)
        audio_path: String,
        /// Optional output directory for .mid and .json
        #[arg(long)]
        output_dir: Option<String>,
        /// Inference device: auto, cuda, or cpu
        #[arg(long)]
        device: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut orch = DjOrchestrator::new();

    match cli.command {
        Commands::Status => {
            let outcome = orch.dispatch(Command::Status)?;
            println!("{}", serde_json::to_string_pretty(&outcome.data)?);
        }
        Commands::Transition => {
            let outcome = orch.dispatch(Command::PlanTransition)?;
            println!("{}", serde_json::to_string_pretty(&outcome.data)?);
        }
        Commands::Camelot { key_a, key_b } => {
            let outcome = orch.dispatch(Command::CheckCamelot { key_a, key_b })?;
            println!("{}", serde_json::to_string_pretty(&outcome.data)?);
        }
        Commands::Catalog => {
            let catalog = Command::catalog();
            println!("{}", serde_json::to_string_pretty(&catalog)?);
        }
        Commands::AnalyzeDettato {
            audio_path,
            output_dir,
            device,
        } => {
            let outcome = orch.dispatch(Command::AnalyzeTrackDettato {
                audio_path,
                output_dir,
                device,
            })?;
            println!("{}", serde_json::to_string_pretty(&outcome)?);
        }
    }
    Ok(())
}

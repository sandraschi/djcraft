#![forbid(unsafe_code)]
use std::path::Path;

use clap::{Parser, Subcommand};
use djcraft_audio::DeckId;
use djcraft_core::{Command, CrateStore, DjOrchestrator, MixxxxxVideoControls};

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
    /// Ping local Mixxxxx video-extended Mixxx OSC bridge (UDP 11119)
    MixxxxxPing,
    /// Send video OSC controls to Mixxxxx for a deck (1-4)
    MixxxxxVideo {
        /// Deck number (1=A, 2=B, 3=C, 4=D)
        #[arg(long, default_value_t = 1)]
        deck: u8,
        #[arg(long)]
        enabled: Option<bool>,
        #[arg(long)]
        fullscreen: Option<bool>,
        #[arg(long)]
        brightness: Option<f32>,
        #[arg(long)]
        contrast: Option<f32>,
        #[arg(long)]
        saturation: Option<f32>,
    },
    /// List persistent SQLite crates and subcrates
    CrateList,
    /// Import a Serato DJ .crate file (preserves %% subcrate hierarchy)
    CrateImportSerato { path: String },
    /// Export a crate by id to a Serato DJ .crate file
    CrateExportSerato { crate_id: i64, dest: String },
    /// Import a VirtualDJ .vdjfolder or .m3u/.m3u8 playlist
    CrateImportVdj { path: String },
    /// Export a crate by id to a VirtualDJ .vdjfolder or .m3u8 file
    CrateExportVdj { crate_id: i64, dest: String },
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
        Commands::MixxxxxPing => {
            let outcome = orch.dispatch(Command::MixxxxxPing {
                timeout_ms: Some(250),
            })?;
            println!("{}", serde_json::to_string_pretty(&outcome)?);
        }
        Commands::MixxxxxVideo {
            deck,
            enabled,
            fullscreen,
            brightness,
            contrast,
            saturation,
        } => {
            let deck_id = match deck {
                1 => DeckId::DeckA,
                2 => DeckId::DeckB,
                3 => DeckId::DeckC,
                4 => DeckId::DeckD,
                _ => return Err(format!("Invalid deck {deck}; expected 1..=4").into()),
            };
            let outcome = orch.dispatch(Command::MixxxxxVideo {
                deck: deck_id,
                controls: MixxxxxVideoControls {
                    enabled,
                    fullscreen,
                    brightness,
                    contrast,
                    saturation,
                },
            })?;
            println!("{}", serde_json::to_string_pretty(&outcome)?);
        }
        Commands::CrateList => {
            let store = CrateStore::open_default()?;
            store.seed_demo_if_empty()?;
            let crates = store.list_crates()?;
            println!("{}", serde_json::to_string_pretty(&crates)?);
        }
        Commands::CrateImportSerato { path } => {
            let store = CrateStore::open_default()?;
            let summary = store.import_serato_crate(Path::new(&path))?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Commands::CrateExportSerato { crate_id, dest } => {
            let store = CrateStore::open_default()?;
            let count = store.export_serato_crate(crate_id, Path::new(&dest))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "format": "serato",
                    "crate_id": crate_id,
                    "dest": dest,
                    "exported_tracks": count
                }))?
            );
        }
        Commands::CrateImportVdj { path } => {
            let store = CrateStore::open_default()?;
            let summary = store.import_vdj_file(Path::new(&path))?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Commands::CrateExportVdj { crate_id, dest } => {
            let store = CrateStore::open_default()?;
            let count = store.export_vdj_file(crate_id, Path::new(&dest))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "format": "vdj",
                    "crate_id": crate_id,
                    "dest": dest,
                    "exported_tracks": count
                }))?
            );
        }
    }
    Ok(())
}

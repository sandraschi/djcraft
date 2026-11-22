#![forbid(unsafe_code)]
use std::path::Path;

use clap::{Parser, Subcommand};
use djcraft_audio::output::AudioOutput;
use djcraft_audio::{list_input_devices, DeckId};
use djcraft_core::{
    Command, ControllerProfileId, CrateStore, DjOrchestrator, MidiControllerManager,
    MixxxxxVideoControls,
};

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
    /// List connected USB MIDI controller input ports and auto-detected profiles
    MidiList,
    /// Connect to a MIDI controller by index or name substring (e.g. "reloop")
    MidiConnect { query: Option<String> },
    /// List available audio output and DVS input devices
    AudioList,
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
        Commands::MidiList => {
            let ports = MidiControllerManager::list_input_ports();
            let items: Vec<_> = ports
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let prof = ControllerProfileId::auto_detect_from_port_name(p);
                    serde_json::json!({
                        "index": i,
                        "port": p,
                        "detected_profile": prof.label()
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&items)?);
        }
        Commands::MidiConnect { query } => {
            let ports = MidiControllerManager::list_input_ports();
            if ports.is_empty() {
                return Err("No MIDI input ports detected".into());
            }
            let idx = if let Some(q) = query.as_deref() {
                if let Ok(n) = q.trim().parse::<usize>() {
                    if n < ports.len() {
                        n
                    } else {
                        return Err(format!(
                            "MIDI port index {n} out of range (0..{})",
                            ports.len()
                        )
                        .into());
                    }
                } else {
                    let ql = q.to_lowercase();
                    ports
                        .iter()
                        .position(|p| p.to_lowercase().contains(&ql))
                        .ok_or_else(|| {
                            format!("No MIDI port matching '{q}' (available: {ports:?})")
                        })?
                }
            } else {
                0
            };
            let mut mgr = MidiControllerManager::new();
            let name = mgr.connect_port(idx)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "connected": true,
                    "index": idx,
                    "port": name,
                    "profile": mgr.active_profile.label(),
                    "rules_count": mgr.rules.len()
                }))?
            );
        }
        Commands::AudioList => {
            let outputs = AudioOutput::list_output_devices();
            let inputs = list_input_devices();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "outputs": outputs,
                    "inputs": inputs
                }))?
            );
        }
    }
    Ok(())
}

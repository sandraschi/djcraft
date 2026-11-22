#![forbid(unsafe_code)]
mod prefs;

use std::path::Path;
use std::time::Duration;

use clap::{Parser, Subcommand};
use djcraft_audio::output::AudioOutput;
use djcraft_audio::{list_input_devices, DeckId, DvsCarrier, DvsInputStream};
use djcraft_core::{
    Command, ControllerProfileId, CrateStore, DjOrchestrator, LinkEngine, MidiControllerManager,
    MixxxxxVideoControls,
};
use prefs::AppConfig;

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
    /// Show all persisted DJCraft settings (ui.json)
    ConfigShow,
    /// Set any configuration key (e.g. skin, backdrop, locale, workspace, headphone-device, split-cue, pitch-range, quantize, ai-endpoint)
    ConfigSet { key: String, value: String },
    /// Reset all configuration keys in ui.json to defaults
    ConfigReset,
    /// Set active built-in MIDI controller profile (e.g. reloop-ready, rane-12, ddj-flx4)
    MidiProfile { profile: String },
    /// Import a local VirtualDJ or Mixxx MIDI XML mapper into midi_learn.json
    MidiImportXml { path: String },
    /// List active MIDI mapping rules
    MidiRules,
    /// Create a crate or subcrate in SQLite
    CrateCreate {
        name: String,
        #[arg(long)]
        parent_id: Option<i64>,
    },
    /// Scan an audio folder into SQLite (creates/uses a crate)
    CrateScan {
        folder: String,
        #[arg(long)]
        crate_id: Option<i64>,
    },
    /// Show autosaved hot cues (slots 0..3) for a track path
    CrateCues { track_path: String },
    /// Set or clear an autosaved hot cue (slot 0..3, value = seconds or "clear")
    CrateSetCue {
        track_path: String,
        slot: usize,
        value: String,
    },
    /// Probe live DVS timecode vinyl input signal
    DvsProbe {
        #[arg(long)]
        device: Option<String>,
        #[arg(long, default_value = "1k")]
        carrier: String,
        #[arg(long, default_value_t = 400)]
        ms: u64,
    },
    /// Probe UDP multicast Link peers (224.76.78.75:20808)
    LinkProbe {
        #[arg(long, default_value_t = 126.0)]
        bpm: f64,
        #[arg(long, default_value_t = 600)]
        ms: u64,
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
        Commands::ConfigShow => {
            let cfg = AppConfig::load();
            println!("{}", serde_json::to_string_pretty(&cfg)?);
        }
        Commands::ConfigSet { key, value } => {
            let mut cfg = AppConfig::load();
            let msg = cfg.set_key(&key, &value)?;
            let path = cfg.save()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "updated": msg,
                    "path": path
                }))?
            );
        }
        Commands::ConfigReset => {
            let cfg = AppConfig::default();
            let path = cfg.save()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "reset": true,
                    "path": path
                }))?
            );
        }
        Commands::MidiProfile { profile } => {
            let id = match profile.trim().to_lowercase().as_str() {
                "reloop" | "reloop-ready" | "ready" => ControllerProfileId::ReloopReady,
                "rane-12" | "rane12" | "twelve" => ControllerProfileId::RaneTwelveMk2,
                "rane-72" | "rane72" | "seventy-two" => ControllerProfileId::RaneSeventyTwoMk2,
                "pioneer" | "ddj-flx4" | "flx4" => ControllerProfileId::PioneerDdjFlx4,
                "hercules" | "djcontrol" => ControllerProfileId::HerculesDjControl,
                "denon" | "sc6000" => ControllerProfileId::DenonSc6000,
                "numark" | "mixtrack" => ControllerProfileId::NumarkMixtrackProFx,
                "xone" | "xone-k2" | "k2" => ControllerProfileId::AllenHeathXoneK2,
                "custom" => ControllerProfileId::CustomLearn,
                other => {
                    return Err(format!(
                        "Unknown profile '{other}' (expected: reloop-ready, rane-12, rane-72, ddj-flx4, hercules, denon, numark, xone-k2, custom)"
                    )
                    .into())
                }
            };
            let mut mgr = MidiControllerManager::new();
            mgr.set_profile(id);
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "profile": mgr.active_profile.label(),
                    "rules_count": mgr.rules.len()
                }))?
            );
        }
        Commands::MidiImportXml { path } => {
            let mut mgr = MidiControllerManager::new();
            let count = mgr.import_xml_mapper_file(Path::new(&path))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "imported_rules": count,
                    "profile": mgr.active_profile.label()
                }))?
            );
        }
        Commands::MidiRules => {
            let mgr = MidiControllerManager::new();
            println!("{}", serde_json::to_string_pretty(&mgr.rules)?);
        }
        Commands::CrateCreate { name, parent_id } => {
            let store = CrateStore::open_default()?;
            let id = store.create_subcrate(parent_id, &name)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "crate_id": id,
                    "name": name,
                    "parent_id": parent_id
                }))?
            );
        }
        Commands::CrateScan { folder, crate_id } => {
            let store = CrateStore::open_default()?;
            let cid = match crate_id {
                Some(id) => id,
                None => store.ensure_crate_path(&folder)?,
            };
            let count = store.scan_audio_folder(cid, Path::new(&folder))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "crate_id": cid,
                    "folder": folder,
                    "imported_tracks": count
                }))?
            );
        }
        Commands::CrateCues { track_path } => {
            let store = CrateStore::open_default()?;
            let cues = store.get_track_hot_cues(&track_path)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "track_path": track_path,
                    "hot_cues": cues
                }))?
            );
        }
        Commands::CrateSetCue {
            track_path,
            slot,
            value,
        } => {
            let store = CrateStore::open_default()?;
            let pos = if value.trim().eq_ignore_ascii_case("clear") {
                None
            } else {
                Some(
                    value
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid seconds '{value}' (or 'clear')"))?,
                )
            };
            store.set_track_hot_cue(&track_path, slot, pos)?;
            let cues = store.get_track_hot_cues(&track_path)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "track_path": track_path,
                    "hot_cues": cues
                }))?
            );
        }
        Commands::DvsProbe {
            device,
            carrier,
            ms,
        } => {
            let c = if carrier.trim().eq_ignore_ascii_case("2k") {
                DvsCarrier::Control2kHz
            } else {
                DvsCarrier::Control1kHz
            };
            let stream = DvsInputStream::start(device.as_deref(), c)?;
            std::thread::sleep(Duration::from_millis(ms.clamp(50, 5000)));
            let tel = stream.telemetry();
            println!("{}", serde_json::to_string_pretty(&tel)?);
        }
        Commands::LinkProbe { bpm, ms } => {
            let mut link = LinkEngine::new(bpm);
            link.set_enabled(true);
            let steps = (ms.clamp(100, 5000) / 50).max(1);
            for _ in 0..steps {
                let _ = link.tick(0.05);
                std::thread::sleep(Duration::from_millis(50));
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "bpm": link.bpm,
                    "beat_phase": link.beat_phase,
                    "peers": link.peer_count()
                }))?
            );
        }
    }
    Ok(())
}

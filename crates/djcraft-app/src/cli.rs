#![forbid(unsafe_code)]

use std::path::Path;
use std::time::Duration;

use clap::{Parser, Subcommand};
use djcraft_audio::output::AudioOutput;
use djcraft_audio::{list_input_devices, DeckId, DvsCarrier, DvsInputStream};
use djcraft_core::{
    ipc::{parse_deck, run_set_script, send_ipc_request, IpcRequest, SetScript},
    AppConfig, Command, ControllerProfileId, CrateStore, DjOrchestrator, LinkEngine,
    MidiControllerManager, MixxxxxVideoControls,
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
    /// Import an AlphaTheta / Pioneer Rekordbox XML library (tracks, playlists, BPM, Camelot, colored cues)
    CrateImportRekordbox { path: String },
    /// Export a crate (or full library when crate_id is omitted) to Rekordbox XML
    CrateExportRekordbox {
        dest: String,
        #[arg(long)]
        crate_id: Option<i64>,
    },
    /// Import a Native Instruments Traktor Pro 4 collection.nml library (tracks, playlists, BPM, Camelot, CUE_V2)
    CrateImportTraktor { path: String },
    /// Export a crate (or full library when crate_id is omitted) to Traktor Pro 4 collection.nml
    CrateExportTraktor {
        dest: String,
        #[arg(long)]
        crate_id: Option<i64>,
        #[arg(long, default_value = "DJCraft Set")]
        playlist: String,
    },
    /// Dynamically query tracks via Smart Crate rules (BPM range, Camelot harmonic key, genre, rating)
    CrateSmartQuery {
        #[arg(long)]
        min_bpm: Option<f64>,
        #[arg(long)]
        max_bpm: Option<f64>,
        #[arg(long)]
        key: Option<String>,
        #[arg(long)]
        harmonic: bool,
        #[arg(long)]
        genre: Option<String>,
        #[arg(long)]
        min_rating: Option<i64>,
    },
    /// Detect duplicate tracks across the library (matching normalized artist + title)
    CrateDuplicates,
    /// List logged live setlist history entries
    SetlistHistory {
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long, default_value_t = 200)]
        limit: usize,
    },
    /// Export live setlist history to .txt (Mixcloud/YouTube), .cue (CD/DAW), or .csv
    SetlistExport {
        dest: String,
        #[arg(long, default_value = "txt")]
        format: String,
        #[arg(long, default_value = "DJCraft Live Set")]
        title: String,
        #[arg(long)]
        session_id: Option<String>,
    },
    /// Analyze all tracks in a crate (or full library) in parallel across CPU cores (BPM + Camelot Key)
    CrateAnalyze {
        #[arg(long)]
        crate_id: Option<i64>,
    },
    /// Analyze a single local audio file (BPM, downbeat, Camelot key, phrases, LUFS auto-gain)
    AnalyzeAudio { path: String },
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
    /// Read ID3v2 metadata and Serato Markers2 cue points from an audio file
    CrateId3Read {
        track_path: String,
        /// Also import found metadata & cues into SQLite (when no local cues exist)
        #[arg(long)]
        import: bool,
    },
    /// Write DJCraft's saved hot cues & labels for a track into its Serato Markers2 ID3v2 tag
    CrateId3Write { track_path: String },
    /// Batch-read ID3v2 metadata and Serato Markers2 cue points for every track in a crate
    CrateId3BatchRead { crate_id: i64 },
    /// Batch-write DJCraft's saved hot cues & labels to Serato Markers2 ID3v2 tags for a crate
    CrateId3BatchWrite { crate_id: i64 },
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
    /// Query live playback, deck, mixer, audio, and MIDI state from running djcraft.exe
    LiveStatus,
    /// Dispatch any raw Command JSON to the running djcraft.exe GUI
    Dispatch { json: String },
    /// Load a local audio file onto a deck (a/b/c/d) in the running djcraft.exe GUI
    Load { deck: String, path: String },
    /// Toggle play/pause on a deck (a/b/c/d) in the running djcraft.exe GUI
    Play { deck: String },
    /// Cue return on a deck (a/b/c/d) in the running djcraft.exe GUI
    Cue { deck: String },
    /// Stop all 4 decks in the running djcraft.exe GUI
    StopAll,
    /// Phase-sync Deck B to Deck A beatgrid in the running djcraft.exe GUI
    Sync,
    /// Set crossfader (-1.0..=1.0) in the running djcraft.exe GUI
    Crossfader { value: f64 },
    /// Set channel volume (0.0..=1.0) on a deck (a/b/c/d) in the running djcraft.exe GUI
    Volume { deck: String, value: f64 },
    /// Set master output volume (0.0..=1.0) in the running djcraft.exe GUI
    Master { value: f64 },
    /// Toggle headphone PFL cue on a deck (a/b/c/d) in the running djcraft.exe GUI
    Pfl { deck: String },
    /// Toggle master mute in the running djcraft.exe GUI
    Mute,
    /// Load a video file onto Video Deck A or B in the running djcraft.exe GUI
    LoadVideo { deck: String, path: String },
    /// Play back a timed .djset.json set script (tracks, FX, transitions, video) in the running djcraft.exe GUI
    RunSet {
        path: String,
        #[arg(long, default_value_t = 1.0)]
        speed: f64,
    },
    /// Control in-app live macro recording (start, stop [--save <file>], status)
    MacroRec {
        action: String,
        #[arg(long)]
        save: Option<String>,
    },
    /// Control the non-blocking in-app Set Player (start [--path <file>] [--speed <f64>], stop, status)
    SetPlay {
        action: String,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        speed: Option<f64>,
    },
    /// Control Two-Room Setup Room 2 prepared autoplay (status, configure, play, pause, stop, next)
    Room2 {
        action: String,
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        crate_id: Option<i64>,
        #[arg(long)]
        volume: Option<f32>,
        #[arg(long)]
        shuffle: Option<bool>,
    },
    /// Control AI Lighting Assistant (Art-Net 4 / sACN DMX512): status, configure, on, off, blackout
    Lighting {
        action: String,
        #[arg(long)]
        enabled: Option<bool>,
        #[arg(long)]
        protocol: Option<String>,
        #[arg(long)]
        target_ip: Option<String>,
        #[arg(long)]
        universe: Option<u16>,
        #[arg(long)]
        mode: Option<String>,
        #[arg(long)]
        dimmer: Option<f32>,
        #[arg(long)]
        strobe: Option<bool>,
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
        Commands::CrateImportRekordbox { path } => {
            let store = CrateStore::open_default()?;
            let summary = djcraft_core::import_rekordbox_xml_file(&store, Path::new(&path))?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Commands::CrateExportRekordbox { dest, crate_id } => {
            let store = CrateStore::open_default()?;
            let count =
                djcraft_core::export_rekordbox_xml_file(&store, crate_id, Path::new(&dest))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "format": "rekordbox_xml",
                    "crate_id": crate_id,
                    "dest": dest,
                    "exported_tracks": count
                }))?
            );
        }
        Commands::CrateImportTraktor { path } => {
            let store = CrateStore::open_default()?;
            let summary = djcraft_core::import_traktor_nml_file(&store, Path::new(&path))?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Commands::CrateExportTraktor {
            dest,
            crate_id,
            playlist,
        } => {
            let store = CrateStore::open_default()?;
            let count = djcraft_core::export_traktor_nml_file(
                &store,
                crate_id,
                &playlist,
                Path::new(&dest),
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "format": "traktor_nml",
                    "crate_id": crate_id,
                    "playlist": playlist,
                    "dest": dest,
                    "exported_tracks": count
                }))?
            );
        }
        Commands::CrateSmartQuery {
            min_bpm,
            max_bpm,
            key,
            harmonic,
            genre,
            min_rating,
        } => {
            let store = CrateStore::open_default()?;
            let rule = djcraft_core::SmartCrateRule {
                name: "CLI Query".into(),
                min_bpm,
                max_bpm,
                camelot_key: key,
                harmonic_compatible: harmonic,
                genre_contains: genre,
                min_rating,
                text_contains: None,
            };
            let tracks = store.query_smart_crate(&rule)?;
            println!("{}", serde_json::to_string_pretty(&tracks)?);
        }
        Commands::CrateDuplicates => {
            let store = CrateStore::open_default()?;
            let dups = store.find_duplicates()?;
            println!("{}", serde_json::to_string_pretty(&dups)?);
        }
        Commands::SetlistHistory { session_id, limit } => {
            let store = CrateStore::open_default()?;
            let entries = store.list_set_history(session_id.as_deref(), limit)?;
            println!("{}", serde_json::to_string_pretty(&entries)?);
        }
        Commands::SetlistExport {
            dest,
            format,
            title,
            session_id,
        } => {
            let store = CrateStore::open_default()?;
            let entries = store.list_set_history(session_id.as_deref(), 500)?;
            let n = djcraft_core::export_setlist_file(&entries, &format, &title, Path::new(&dest))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "format": format,
                    "dest": dest,
                    "exported_entries": n
                }))?
            );
        }
        Commands::CrateAnalyze { crate_id } => {
            let store = CrateStore::open_default()?;
            let report = store.analyze_crate_parallel(crate_id)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::AnalyzeAudio { path } => {
            let decoded = djcraft_audio::dsp::decode_audio_file(Path::new(&path))
                .map_err(djcraft_core::DjError::Sidecar)?;
            let analysis = djcraft_audio::analyze_track_full(&decoded);
            println!("{}", serde_json::to_string_pretty(&analysis)?);
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
        Commands::CrateId3Read { track_path, import } => {
            let info = djcraft_core::read_id3_track_info(Path::new(&track_path))?;
            let imported = if import {
                let store = CrateStore::open_default()?;
                store.import_id3_into_track(&track_path)?
            } else {
                0
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "track_path": track_path,
                    "title": info.title,
                    "artist": info.artist,
                    "genre": info.genre,
                    "bpm": info.bpm,
                    "musical_key": info.musical_key,
                    "cue_count": info.cue_count,
                    "cues": info.cues,
                    "cue_metas": info.cue_metas,
                    "imported_to_sqlite": imported
                }))?
            );
        }
        Commands::CrateId3Write { track_path } => {
            let store = CrateStore::open_default()?;
            let cues = store.get_track_hot_cues(&track_path)?;
            let metas = store.get_track_hot_cue_metas(&track_path)?;
            let written = djcraft_core::write_id3_hot_cues(Path::new(&track_path), &cues, &metas)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "track_path": track_path,
                    "cues_written": written
                }))?
            );
        }
        Commands::CrateId3BatchRead { crate_id } => {
            let store = CrateStore::open_default()?;
            let summary = store.batch_read_id3_cues_for_crate(crate_id)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Commands::CrateId3BatchWrite { crate_id } => {
            let store = CrateStore::open_default()?;
            let summary = store.batch_write_id3_cues_for_crate(crate_id)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
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
        Commands::LiveStatus => {
            let resp = send_ipc_request(&IpcRequest::LiveStatus)?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Dispatch { json } => {
            let command: Command = serde_json::from_str(&json)?;
            let resp = send_ipc_request(&IpcRequest::Command { command })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Load { deck, path } => {
            let deck_id = parse_deck(&deck)?;
            let resp = send_ipc_request(&IpcRequest::LoadFile {
                deck: deck_id,
                path,
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Play { deck } => {
            let resp = send_ipc_request(&IpcRequest::Command {
                command: Command::PlayPause {
                    deck: parse_deck(&deck)?,
                },
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Cue { deck } => {
            let resp = send_ipc_request(&IpcRequest::Command {
                command: Command::CueReturn {
                    deck: parse_deck(&deck)?,
                },
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::StopAll => {
            let resp = send_ipc_request(&IpcRequest::Command {
                command: Command::StopAllDecks,
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Sync => {
            let resp = send_ipc_request(&IpcRequest::Command {
                command: Command::PhaseSyncBeatgrid,
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Crossfader { value } => {
            let resp = send_ipc_request(&IpcRequest::Command {
                command: Command::SetCrossfader { value },
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Volume { deck, value } => {
            let resp = send_ipc_request(&IpcRequest::Command {
                command: Command::SetDeckVolume {
                    deck: parse_deck(&deck)?,
                    volume: value,
                },
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Master { value } => {
            let resp = send_ipc_request(&IpcRequest::Command {
                command: Command::SetMasterVolume { volume: value },
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Pfl { deck } => {
            let resp = send_ipc_request(&IpcRequest::Command {
                command: Command::TogglePflCue {
                    deck: parse_deck(&deck)?,
                },
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Mute => {
            let resp = send_ipc_request(&IpcRequest::Command {
                command: Command::ToggleMasterMute,
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::LoadVideo { deck, path } => {
            let resp = send_ipc_request(&IpcRequest::LoadVideo {
                deck: parse_deck(&deck)?,
                path,
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::RunSet { path, speed } => {
            let raw = std::fs::read_to_string(&path)?;
            let script: SetScript = serde_json::from_str(&raw)?;
            let summary = run_set_script(&script, speed)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Commands::MacroRec { action, save } => {
            let resp = send_ipc_request(&IpcRequest::MacroControl {
                action,
                save_path: save,
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::SetPlay {
            action,
            path,
            speed,
        } => {
            let resp = send_ipc_request(&IpcRequest::SetPlayback {
                action,
                path,
                speed,
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Room2 {
            action,
            device,
            crate_id,
            volume,
            shuffle,
        } => {
            let resp = send_ipc_request(&IpcRequest::Room2Control {
                action,
                device,
                crate_id,
                volume,
                shuffle,
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
        Commands::Lighting {
            action,
            enabled,
            protocol,
            target_ip,
            universe,
            mode,
            dimmer,
            strobe,
        } => {
            let resp = send_ipc_request(&IpcRequest::LightingControl {
                action,
                enabled,
                protocol,
                target_ip,
                universe,
                mode,
                dimmer,
                strobe,
            })?;
            if !resp.ok {
                return Err(resp.error.unwrap_or_else(|| "IPC error".to_string()).into());
            }
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        }
    }
    Ok(())
}

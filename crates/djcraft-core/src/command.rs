#![forbid(unsafe_code)]
//! Universal L1 Command Bus (`Command::dispatch`) for DJCraft.
//!
//! All GUI menu actions, Command Palette invocations, CLI subcommands, and MCP
//! tools route through `DjOrchestrator::dispatch` so behavior stays identical
//! across headless and desktop modes.

use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use djcraft_audio::{AudioEngine, DeckId, TrackMeta};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::mixxxxx_osc::{
    ping_mixxxxx, resolve_mixxxxx_binary, send_deck_video_controls, MixxxxxConfig,
    MixxxxxVideoControls,
};
use crate::{DjError, DjOrchestrator};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub hotkey: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DettatoTrackSummary {
    pub name: String,
    pub program: Option<u32>,
    pub is_drum: bool,
    pub notes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DettatoAnalysis {
    pub source: String,
    pub midi_path: String,
    pub metadata_path: Option<String>,
    pub seconds: Option<f64>,
    pub tuning_cents: Option<f64>,
    pub drum_hits: u64,
    pub drum_model: Option<String>,
    pub tracks: Vec<DettatoTrackSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    NewSession,
    Undo,
    Redo,
    PlayPause {
        deck: DeckId,
    },
    CueReturn {
        deck: DeckId,
    },
    StopAllDecks,
    LoadTrack {
        deck: DeckId,
        track: TrackMeta,
    },
    LoadDemoCrateTrack {
        deck: DeckId,
        index: usize,
    },
    SetCrossfader {
        value: f64,
    },
    SetMasterVolume {
        volume: f64,
    },
    ToggleMasterMute,
    SetDeckVolume {
        deck: DeckId,
        volume: f64,
    },
    SetPitchBend {
        deck: DeckId,
        pct: f64,
    },
    ToggleKeylock {
        deck: DeckId,
    },
    NudgeBeatgrid {
        deck: DeckId,
        delta_ms: f64,
    },
    ScaleBpm {
        deck: DeckId,
        multiplier: f64,
    },
    PhaseSyncBeatgrid,
    SetStemGains {
        deck: DeckId,
        vocals: f64,
        drums: f64,
        bass: f64,
        other: f64,
    },
    ResetAllStems,
    SetHotCue {
        deck: DeckId,
        slot: usize,
    },
    SetLoopBars {
        deck: DeckId,
        bars: Option<u32>,
    },
    AutoSyncBeatgrid,
    PlanTransition,
    CheckCamelot {
        key_a: String,
        key_b: String,
    },
    Status,
    AnalyzeTrackDettato {
        audio_path: String,
        output_dir: Option<String>,
        device: Option<String>,
    },
    MixxxxxPing {
        timeout_ms: Option<u64>,
    },
    MixxxxxVideo {
        deck: DeckId,
        controls: MixxxxxVideoControls,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchOutcome {
    pub ok: bool,
    pub command: String,
    pub message: String,
    pub data: Value,
}

impl Command {
    /// Canonical catalog of commands exposed in Command Palette, Help, and `--smoke`.
    pub fn catalog() -> Vec<CommandSpec> {
        vec![
            CommandSpec {
                id: "session.new",
                label: "New Session / Reset Decks",
                category: "File",
                hotkey: "Ctrl+N",
            },
            CommandSpec {
                id: "edit.undo",
                label: "Undo Last Deck Change",
                category: "Edit",
                hotkey: "Ctrl+Z",
            },
            CommandSpec {
                id: "edit.redo",
                label: "Redo Deck Change",
                category: "Edit",
                hotkey: "Ctrl+Y",
            },
            CommandSpec {
                id: "deck.play_pause.a",
                label: "Toggle Play / Pause Deck A",
                category: "Transport",
                hotkey: "Space",
            },
            CommandSpec {
                id: "deck.play_pause.b",
                label: "Toggle Play / Pause Deck B",
                category: "Transport",
                hotkey: "Shift+Space",
            },
            CommandSpec {
                id: "deck.cue.a",
                label: "Cue Return Deck A (Beat 1)",
                category: "Transport",
                hotkey: "Q",
            },
            CommandSpec {
                id: "deck.cue.b",
                label: "Cue Return Deck B (Beat 1)",
                category: "Transport",
                hotkey: "W",
            },
            CommandSpec {
                id: "deck.stop_all",
                label: "Stop All Decks",
                category: "Commands",
                hotkey: "Esc",
            },
            CommandSpec {
                id: "insert.load_crate_a",
                label: "Load Next Crate Track to Deck A",
                category: "Insert",
                hotkey: "Ctrl+1",
            },
            CommandSpec {
                id: "insert.load_crate_b",
                label: "Load Next Crate Track to Deck B",
                category: "Insert",
                hotkey: "Ctrl+2",
            },
            CommandSpec {
                id: "insert.hot_cue_a",
                label: "Store Hot Cue 1 on Deck A",
                category: "Insert",
                hotkey: "H",
            },
            CommandSpec {
                id: "insert.loop_8_a",
                label: "Toggle 8-Bar Beat Loop on Deck A",
                category: "Insert",
                hotkey: "L",
            },
            CommandSpec {
                id: "modify.reset_stems",
                label: "Reset All Stem Gains to 100%",
                category: "Modify",
                hotkey: "R",
            },
            CommandSpec {
                id: "modify.center_crossfader",
                label: "Center Master Crossfader (0.0)",
                category: "Modify",
                hotkey: "C",
            },
            CommandSpec {
                id: "modify.toggle_mute",
                label: "Toggle Master Audio Mute",
                category: "Modify",
                hotkey: "M",
            },
            CommandSpec {
                id: "commands.auto_sync",
                label: "Auto-Sync Beatgrid (Match Deck B to Deck A BPM)",
                category: "Commands",
                hotkey: "S",
            },
            CommandSpec {
                id: "commands.phase_sync",
                label: "Phase-Sync Deck B onto Deck A Beatgrid",
                category: "Commands",
                hotkey: "Shift+S",
            },
            CommandSpec {
                id: "modify.keylock_a",
                label: "Toggle Keylock (WSOLA Master Tempo) on Deck A",
                category: "Modify",
                hotkey: "",
            },
            CommandSpec {
                id: "commands.plan_transition",
                label: "Plan Harmonic Transition (Deck A -> Deck B)",
                category: "Commands",
                hotkey: "T",
            },
            CommandSpec {
                id: "commands.dettato_status",
                label: "Check Local Dettato Audio-to-MIDI Sidecar",
                category: "Commands",
                hotkey: "",
            },
            CommandSpec {
                id: "commands.mixxxxx_ping",
                label: "Ping Mixxxxx Video Engine (OSC UDP 11119)",
                category: "Commands",
                hotkey: "",
            },
            CommandSpec {
                id: "commands.mixxxxx_video_a_on",
                label: "Enable Mixxxxx Video Output on Deck A",
                category: "Commands",
                hotkey: "",
            },
        ]
    }
}

pub fn resolve_dettato_binary() -> Option<PathBuf> {
    if let Ok(env_bin) = std::env::var("DETTATO_BIN") {
        let p = PathBuf::from(env_bin);
        if p.is_file() {
            return Some(p);
        }
    }
    let default_win = PathBuf::from(r"D:\Dev\tools\dettato\.venv\Scripts\dettato.exe");
    if default_win.is_file() {
        return Some(default_win);
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in ["dettato.exe", "dettato"] {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

pub fn parse_dettato_json(
    raw_json: &str,
    source_fallback: &str,
    midi_path: &Path,
    metadata_path: Option<&Path>,
) -> Result<DettatoAnalysis, DjError> {
    let val: Value = serde_json::from_str(raw_json)
        .map_err(|e| DjError::Sidecar(format!("Invalid dettato JSON: {e}")))?;

    let source = val
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or(source_fallback)
        .to_string();
    let seconds = val.get("seconds").and_then(Value::as_f64);
    let tuning_cents = val.get("tuning_cents").and_then(Value::as_f64);
    let drum_hits = val
        .get("drums")
        .and_then(|d| d.get("hits"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let drum_model = val
        .get("drums")
        .and_then(|d| d.get("model"))
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let mut tracks = Vec::new();
    if let Some(arr) = val.get("tracks").and_then(Value::as_array) {
        for item in arr {
            tracks.push(DettatoTrackSummary {
                name: item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                program: item
                    .get("program")
                    .and_then(Value::as_u64)
                    .map(|p| p as u32),
                is_drum: item
                    .get("is_drum")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                notes: item.get("notes").and_then(Value::as_u64).unwrap_or(0),
            });
        }
    }

    Ok(DettatoAnalysis {
        source,
        midi_path: midi_path.to_string_lossy().to_string(),
        metadata_path: metadata_path.map(|p| p.to_string_lossy().to_string()),
        seconds,
        tuning_cents,
        drum_hits,
        drum_model,
        tracks,
    })
}

pub fn run_dettato_sidecar(
    audio_path: &str,
    output_dir: Option<&str>,
    device: Option<&str>,
) -> Result<DettatoAnalysis, DjError> {
    let src = Path::new(audio_path);
    if !src.is_file() {
        return Err(DjError::Sidecar(format!(
            "Audio file not found: {}",
            src.display()
        )));
    }
    let bin = resolve_dettato_binary().ok_or_else(|| {
        DjError::Sidecar(
            "dettato CLI not found. Set DETTATO_BIN or install into D:\\Dev\\tools\\dettato\\.venv."
                .to_string(),
        )
    })?;

    let out_dir = match output_dir {
        Some(dir) => PathBuf::from(dir),
        None => src
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("dettato_out"),
    };
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| DjError::Sidecar(format!("Cannot create output dir: {e}")))?;

    let mut cmd = ProcessCommand::new(&bin);
    cmd.arg(src).arg("-o").arg(&out_dir);
    if let Some(dev) = device {
        let trimmed = dev.trim().to_lowercase();
        if trimmed == "cpu" || trimmed == "cuda" {
            cmd.arg("--device").arg(trimmed);
        } else if trimmed != "auto" && !trimmed.is_empty() {
            return Err(DjError::Sidecar(format!(
                "Invalid device '{dev}'. Expected auto, cuda, or cpu."
            )));
        }
    }

    let output = cmd
        .output()
        .map_err(|e| DjError::Sidecar(format!("Failed to launch dettato: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DjError::Sidecar(format!(
            "dettato exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let midi_path = out_dir.join(format!("{stem}.mid"));
    let json_path = out_dir.join(format!("{stem}.json"));
    if !midi_path.is_file() {
        return Err(DjError::Sidecar(format!(
            "dettato succeeded but did not produce {}",
            midi_path.display()
        )));
    }

    if json_path.is_file() {
        let raw = std::fs::read_to_string(&json_path)
            .map_err(|e| DjError::Sidecar(format!("Cannot read dettato JSON: {e}")))?;
        parse_dettato_json(&raw, audio_path, &midi_path, Some(&json_path))
    } else {
        Ok(DettatoAnalysis {
            source: audio_path.to_string(),
            midi_path: midi_path.to_string_lossy().to_string(),
            metadata_path: None,
            seconds: None,
            tuning_cents: None,
            drum_hits: 0,
            drum_model: None,
            tracks: Vec::new(),
        })
    }
}

impl DjOrchestrator {
    /// Execute a command through the universal L1 bus.
    pub fn dispatch(&mut self, cmd: Command) -> Result<DispatchOutcome, DjError> {
        match cmd {
            Command::NewSession => {
                self.push_undo();
                let fresh = Self::new();
                self.deck_a = fresh.deck_a;
                self.deck_b = fresh.deck_b;
                self.deck_c = fresh.deck_c;
                self.deck_d = fresh.deck_d;
                self.crossfader = fresh.crossfader;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "new_session".to_string(),
                    message: "Session reset to default 4-deck layout".to_string(),
                    data: json!({ "crossfader": self.crossfader }),
                })
            }
            Command::Undo => {
                if self.undo() {
                    Ok(DispatchOutcome {
                        ok: true,
                        command: "undo".to_string(),
                        message: "Reverted last deck transaction".to_string(),
                        data: json!({ "undo_depth": self.undo_depth(), "redo_depth": self.redo_depth() }),
                    })
                } else {
                    Ok(DispatchOutcome {
                        ok: false,
                        command: "undo".to_string(),
                        message: "Nothing to undo".to_string(),
                        data: json!({ "undo_depth": 0 }),
                    })
                }
            }
            Command::Redo => {
                if self.redo() {
                    Ok(DispatchOutcome {
                        ok: true,
                        command: "redo".to_string(),
                        message: "Re-applied deck transaction".to_string(),
                        data: json!({ "undo_depth": self.undo_depth(), "redo_depth": self.redo_depth() }),
                    })
                } else {
                    Ok(DispatchOutcome {
                        ok: false,
                        command: "redo".to_string(),
                        message: "Nothing to redo".to_string(),
                        data: json!({ "redo_depth": 0 }),
                    })
                }
            }
            Command::PlayPause { deck } => {
                self.push_undo();
                let state = self.deck_mut(deck);
                state.is_playing = !state.is_playing;
                let playing = state.is_playing;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "play_pause".to_string(),
                    message: format!(
                        "{}: {}",
                        deck.label(),
                        if playing { "PLAYING" } else { "PAUSED" }
                    ),
                    data: json!({ "deck": deck, "is_playing": playing }),
                })
            }
            Command::CueReturn { deck } => {
                self.push_undo();
                let state = self.deck_mut(deck);
                state.is_playing = false;
                state.is_cue_active = true;
                state.position_seconds = 0.0;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "cue_return".to_string(),
                    message: format!("{}: CUE RETURN BEAT 1", deck.label()),
                    data: json!({ "deck": deck, "position_seconds": 0.0 }),
                })
            }
            Command::StopAllDecks => {
                self.push_undo();
                for deck in [DeckId::DeckA, DeckId::DeckB, DeckId::DeckC, DeckId::DeckD] {
                    self.deck_mut(deck).is_playing = false;
                }
                Ok(DispatchOutcome {
                    ok: true,
                    command: "stop_all_decks".to_string(),
                    message: "All 4 decks paused".to_string(),
                    data: json!({ "all_stopped": true }),
                })
            }
            Command::LoadTrack { deck, track } => {
                self.push_undo();
                let title = track.title.clone();
                let state = self.deck_mut(deck);
                state.loaded_track = Some(track);
                state.position_seconds = 0.0;
                state.is_playing = false;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "load_track".to_string(),
                    message: format!("Loaded '{}' to {}", title, deck.label()),
                    data: json!({ "deck": deck, "title": title }),
                })
            }
            Command::LoadDemoCrateTrack { deck, index } => {
                let track = self
                    .crate_tracks
                    .get(index)
                    .cloned()
                    .ok_or_else(|| DjError::Deck(format!("Crate index {index} out of range")))?;
                self.dispatch(Command::LoadTrack { deck, track })
            }
            Command::SetCrossfader { value } => {
                self.push_undo();
                self.crossfader = value.clamp(-1.0, 1.0);
                Ok(DispatchOutcome {
                    ok: true,
                    command: "set_crossfader".to_string(),
                    message: format!("Crossfader set to {:+.2}", self.crossfader),
                    data: json!({ "crossfader": self.crossfader }),
                })
            }
            Command::SetMasterVolume { volume } => {
                self.push_undo();
                self.master_volume = volume.clamp(0.0, 1.0);
                Ok(DispatchOutcome {
                    ok: true,
                    command: "set_master_volume".to_string(),
                    message: format!("Master Loudness set to {:.0}%", self.master_volume * 100.0),
                    data: json!({ "master_volume": self.master_volume }),
                })
            }
            Command::ToggleMasterMute => {
                self.push_undo();
                self.master_muted = !self.master_muted;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "toggle_master_mute".to_string(),
                    message: if self.master_muted {
                        "Master Audio MUTED".to_string()
                    } else {
                        "Master Audio UNMUTED".to_string()
                    },
                    data: json!({ "master_muted": self.master_muted }),
                })
            }
            Command::SetDeckVolume { deck, volume } => {
                self.push_undo();
                let clamped = volume.clamp(0.0, 1.0);
                self.deck_mut(deck).volume_fader = clamped;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "set_deck_volume".to_string(),
                    message: format!("{} volume set to {:.0}%", deck.label(), clamped * 100.0),
                    data: json!({ "deck": deck, "volume_fader": clamped }),
                })
            }
            Command::SetPitchBend { deck, pct } => {
                self.push_undo();
                let clamped = pct.clamp(-16.0, 16.0);
                self.deck_mut(deck).pitch_bend_pct = clamped;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "set_pitch_bend".to_string(),
                    message: format!("{} pitch set to {:+.2}%", deck.label(), clamped),
                    data: json!({ "deck": deck, "pitch_bend_pct": clamped }),
                })
            }
            Command::ToggleKeylock { deck } => {
                self.push_undo();
                let state = self.deck_mut(deck);
                state.keylock = !state.keylock;
                let kl = state.keylock;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "toggle_keylock".to_string(),
                    message: format!(
                        "{}: {}",
                        deck.label(),
                        if kl {
                            "KEYLOCK ON (WSOLA Master Tempo)"
                        } else {
                            "KEYLOCK OFF (Vinyl Varispeed)"
                        }
                    ),
                    data: json!({ "deck": deck, "keylock": kl }),
                })
            }
            Command::NudgeBeatgrid { deck, delta_ms } => {
                self.push_undo();
                let state = self.deck_mut(deck);
                state.beatgrid_offset_sec =
                    (state.beatgrid_offset_sec + delta_ms / 1000.0).clamp(-2.0, 2.0);
                let off = state.beatgrid_offset_sec;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "nudge_beatgrid".to_string(),
                    message: format!("{} beatgrid offset {:+.1} ms", deck.label(), off * 1000.0),
                    data: json!({ "deck": deck, "beatgrid_offset_sec": off }),
                })
            }
            Command::ScaleBpm { deck, multiplier } => {
                if !(0.25..=4.0).contains(&multiplier) {
                    return Err(DjError::Deck(format!(
                        "BPM multiplier {multiplier} out of range"
                    )));
                }
                let state = self.deck_mut(deck);
                let Some(track) = state.loaded_track.as_ref() else {
                    return Err(DjError::Deck(format!("{} has no loaded track", deck.label())));
                };
                let new_bpm = (track.bpm * multiplier).clamp(40.0, 240.0);
                self.push_undo();
                if let Some(track_mut) = self.deck_mut(deck).loaded_track.as_mut() {
                    track_mut.bpm = new_bpm;
                }
                Ok(DispatchOutcome {
                    ok: true,
                    command: "scale_bpm".to_string(),
                    message: format!("{} BPM scaled to {:.1}", deck.label(), new_bpm),
                    data: json!({ "deck": deck, "bpm": new_bpm }),
                })
            }
            Command::PhaseSyncBeatgrid => {
                let Some(track_a) = self.deck_a.loaded_track.as_ref() else {
                    return Err(DjError::Deck("Deck A has no loaded track".to_string()));
                };
                let Some(track_b) = self.deck_b.loaded_track.as_ref() else {
                    return Err(DjError::Deck("Deck B has no loaded track".to_string()));
                };
                let eff_bpm_a = (track_a.bpm * (1.0 + self.deck_a.pitch_bend_pct / 100.0)).max(20.0);
                let beat_sec_a = 60.0 / eff_bpm_a;
                let phase_a = (self.deck_a.position_seconds - self.deck_a.beatgrid_offset_sec)
                    .rem_euclid(beat_sec_a);

                let needed_pct =
                    (((eff_bpm_a / track_b.bpm.max(1.0)) - 1.0) * 100.0).clamp(-16.0, 16.0);
                let current_beats_b =
                    ((self.deck_b.position_seconds - self.deck_b.beatgrid_offset_sec) / beat_sec_a)
                        .floor();
                let aligned_pos_b = (self.deck_b.beatgrid_offset_sec
                    + current_beats_b * beat_sec_a
                    + phase_a)
                    .max(0.0);

                self.push_undo();
                self.deck_b.pitch_bend_pct = needed_pct;
                self.deck_b.position_seconds = aligned_pos_b;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "phase_sync_beatgrid".to_string(),
                    message: format!(
                        "Phase-synced Deck B to {:.2} BPM (pitch {:+.2}%, pos {:.2}s)",
                        eff_bpm_a, needed_pct, aligned_pos_b
                    ),
                    data: json!({
                        "target_bpm": eff_bpm_a,
                        "deck_b_pitch_bend_pct": needed_pct,
                        "deck_b_position_seconds": aligned_pos_b,
                    }),
                })
            }
            Command::SetStemGains {
                deck,
                vocals,
                drums,
                bass,
                other,
            } => {
                self.push_undo();
                let state = self.deck_mut(deck);
                state.stem_vocals_gain = vocals.clamp(0.0, 1.0);
                state.stem_drums_gain = drums.clamp(0.0, 1.0);
                state.stem_bass_gain = bass.clamp(0.0, 1.0);
                state.stem_other_gain = other.clamp(0.0, 1.0);
                Ok(DispatchOutcome {
                    ok: true,
                    command: "set_stem_gains".to_string(),
                    message: format!("Updated stem gains on {}", deck.label()),
                    data: json!({
                        "deck": deck,
                        "vocals": state.stem_vocals_gain,
                        "drums": state.stem_drums_gain,
                        "bass": state.stem_bass_gain,
                        "other": state.stem_other_gain,
                    }),
                })
            }
            Command::ResetAllStems => {
                self.push_undo();
                for deck in [DeckId::DeckA, DeckId::DeckB, DeckId::DeckC, DeckId::DeckD] {
                    let state = self.deck_mut(deck);
                    state.stem_vocals_gain = 1.0;
                    state.stem_drums_gain = 1.0;
                    state.stem_bass_gain = 1.0;
                    state.stem_other_gain = 1.0;
                }
                Ok(DispatchOutcome {
                    ok: true,
                    command: "reset_all_stems".to_string(),
                    message: "Reset all stem gains to 100% across 4 decks".to_string(),
                    data: json!({ "stem_gain": 1.0 }),
                })
            }
            Command::SetHotCue { deck, slot } => {
                if slot >= 4 {
                    return Err(DjError::Deck(format!(
                        "Hot cue slot {slot} out of range (0..4)"
                    )));
                }
                self.push_undo();
                let state = self.deck_mut(deck);
                let pos = state.position_seconds;
                state.hot_cue_seconds[slot] = Some(pos);
                Ok(DispatchOutcome {
                    ok: true,
                    command: "set_hot_cue".to_string(),
                    message: format!(
                        "{}: Stored Hot Cue {} at {:.1}s",
                        deck.label(),
                        slot + 1,
                        pos
                    ),
                    data: json!({ "deck": deck, "slot": slot, "position_seconds": pos }),
                })
            }
            Command::SetLoopBars { deck, bars } => {
                self.push_undo();
                self.deck_mut(deck).loop_bars = bars;
                let msg = match bars {
                    Some(b) => format!("{}: Beat loop set to {} bars", deck.label(), b),
                    None => format!("{}: Beat loop cleared", deck.label()),
                };
                Ok(DispatchOutcome {
                    ok: true,
                    command: "set_loop_bars".to_string(),
                    message: msg,
                    data: json!({ "deck": deck, "loop_bars": bars }),
                })
            }
            Command::AutoSyncBeatgrid => {
                let target_bpm = self
                    .deck_a
                    .loaded_track
                    .as_ref()
                    .map(|t| t.bpm * (1.0 + self.deck_a.pitch_bend_pct / 100.0))
                    .ok_or_else(|| DjError::Deck("Deck A has no loaded track".to_string()))?;
                let deck_b_base_bpm = self
                    .deck_b
                    .loaded_track
                    .as_ref()
                    .map(|t| t.bpm)
                    .ok_or_else(|| DjError::Deck("Deck B has no loaded track".to_string()))?;
                if deck_b_base_bpm <= 0.0 {
                    return Err(DjError::Deck("Deck B BPM must be positive".to_string()));
                }
                self.push_undo();
                let needed_pct = (((target_bpm / deck_b_base_bpm) - 1.0) * 100.0).clamp(-16.0, 16.0);
                self.deck_b.pitch_bend_pct = needed_pct;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "auto_sync_beatgrid".to_string(),
                    message: format!(
                        "Synced Deck B to {:.2} BPM (pitch {:+.2}%)",
                        target_bpm, needed_pct
                    ),
                    data: json!({
                        "target_bpm": target_bpm,
                        "deck_b_pitch_bend_pct": needed_pct,
                    }),
                })
            }
            Command::PlanTransition => {
                let plan = self.plan_transition()?;
                let data = serde_json::to_value(&plan)
                    .map_err(|e| DjError::Deck(format!("Serialize error: {e}")))?;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "plan_transition".to_string(),
                    message: format!(
                        "Transition {} -> {} ({} bars, harmonic={})",
                        plan.from_track,
                        plan.to_track,
                        plan.recommended_transition_bars,
                        plan.harmonic_compatible
                    ),
                    data,
                })
            }
            Command::CheckCamelot { key_a, key_b } => {
                let compatible = AudioEngine::is_camelot_compatible(&key_a, &key_b);
                let suggestions = AudioEngine::compatible_camelot_keys(&key_a);
                Ok(DispatchOutcome {
                    ok: true,
                    command: "check_camelot".to_string(),
                    message: format!(
                        "{} <-> {}: {}",
                        key_a,
                        key_b,
                        if compatible { "COMPATIBLE" } else { "CLASH" }
                    ),
                    data: json!({
                        "key_a": key_a,
                        "key_b": key_b,
                        "compatible": compatible,
                        "compatible_with_a": suggestions,
                    }),
                })
            }
            Command::Status => Ok(DispatchOutcome {
                ok: true,
                command: "status".to_string(),
                message: "4-deck orchestrator status snapshot".to_string(),
                data: json!({
                    "deck_a": self.deck_a,
                    "deck_b": self.deck_b,
                    "deck_c": self.deck_c,
                    "deck_d": self.deck_d,
                    "crossfader": self.crossfader,
                    "crate_count": self.crate_tracks.len(),
                    "undo_depth": self.undo_depth(),
                    "redo_depth": self.redo_depth(),
                    "dettato_available": resolve_dettato_binary().is_some(),
                    "mixxxxx_binary_available": resolve_mixxxxx_binary().is_some(),
                }),
            }),
            Command::AnalyzeTrackDettato {
                audio_path,
                output_dir,
                device,
            } => {
                let analysis = run_dettato_sidecar(
                    &audio_path,
                    output_dir.as_deref(),
                    device.as_deref(),
                )?;
                let data = serde_json::to_value(&analysis)
                    .map_err(|e| DjError::Sidecar(format!("Serialize error: {e}")))?;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "analyze_track_dettato".to_string(),
                    message: format!(
                        "Analyzed {} ({} tracks, {} drum hits)",
                        analysis.source,
                        analysis.tracks.len(),
                        analysis.drum_hits
                    ),
                    data,
                })
            }
            Command::MixxxxxPing { timeout_ms } => {
                let cfg = MixxxxxConfig::from_env();
                let res = ping_mixxxxx(&cfg, timeout_ms.unwrap_or(200))?;
                let data = serde_json::to_value(&res)
                    .map_err(|e| DjError::Sidecar(format!("Serialize error: {e}")))?;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "mixxxxx_ping".to_string(),
                    message: res.note,
                    data,
                })
            }
            Command::MixxxxxVideo { deck, controls } => {
                let cfg = MixxxxxConfig::from_env();
                let sent = send_deck_video_controls(&cfg, deck, &controls)?;
                Ok(DispatchOutcome {
                    ok: true,
                    command: "mixxxxx_video".to_string(),
                    message: format!(
                        "Sent {} Mixxxxx video OSC message(s) for {} to {}",
                        sent.len(),
                        deck.label(),
                        cfg.target_addr()
                    ),
                    data: json!({
                        "deck": deck,
                        "target": cfg.target_addr(),
                        "sent_addresses": sent,
                    }),
                })
            }
        }
    }
}

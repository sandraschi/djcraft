#![forbid(unsafe_code)]
//! DJCraft Core Engine
//!
//! Provides 4-deck orchestration, master crossfader curves,
//! transition planner, undo/redo history, and universal Command::dispatch bus.

pub mod command;
pub mod crates_db;
pub mod i18n;
pub mod midi;
pub mod mixxxxx_osc;

pub use command::{
    parse_dettato_json, resolve_dettato_binary, Command, CommandSpec, DettatoAnalysis,
    DettatoTrackSummary, DispatchOutcome,
};
pub use crates_db::{
    encode_m3u, encode_serato_crate_bytes, encode_vdj_folder_xml, parse_m3u,
    parse_serato_crate_bytes, parse_vdj_folder_xml, CrateNode, CrateQuery, CrateSortField,
    CrateStore, CrateTrack, ImportSummary,
};
pub use midi::{
    action_to_command, builtin_profile_rules, parse_raw_midi, ControllerProfileId, MidiBindingKey,
    MidiControllerManager, MidiMappingRule, MidiMsgKind, MidiTargetAction, RawMidiEvent,
};
pub use mixxxxx_osc::{
    encode_osc_message, ping_mixxxxx, resolve_mixxxxx_binary, send_deck_video_controls,
    MixxxxxConfig, MixxxxxPingResult, MixxxxxVideoControls, OscArg,
};

use djcraft_audio::{AudioEngine, DeckId, DeckState, TrackMeta};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_UNDO_DEPTH: usize = 64;

#[derive(Error, Debug)]
pub enum DjError {
    #[error("Deck error: {0}")]
    Deck(String),
    #[error("Sidecar error: {0}")]
    Sidecar(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionPlan {
    pub from_track: String,
    pub to_track: String,
    pub from_bpm: f64,
    pub to_bpm: f64,
    pub bpm_delta: f64,
    pub harmonic_compatible: bool,
    pub recommended_transition_bars: u32,
    pub suggested_eq_strategy: String,
}

#[derive(Debug, Clone)]
struct MixerSnapshot {
    deck_a: DeckState,
    deck_b: DeckState,
    deck_c: DeckState,
    deck_d: DeckState,
    crossfader: f64,
    master_volume: f64,
    master_muted: bool,
}

pub struct DjOrchestrator {
    pub deck_a: DeckState,
    pub deck_b: DeckState,
    pub deck_c: DeckState,
    pub deck_d: DeckState,
    pub crossfader: f64,    // -1.0 (full Deck A) to +1.0 (full Deck B)
    pub master_volume: f64, // 0.0 to 1.0
    pub master_muted: bool,
    pub crate_tracks: Vec<TrackMeta>,
    undo_stack: Vec<MixerSnapshot>,
    redo_stack: Vec<MixerSnapshot>,
}

impl Default for DjOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl DjOrchestrator {
    pub fn new() -> Self {
        let crate_tracks = vec![
            TrackMeta {
                title: "Subterranean Dub".to_string(),
                artist: "Resonant Swarm".to_string(),
                bpm: 126.0,
                musical_key: "8A".to_string(),
                duration_seconds: 384.0,
            },
            TrackMeta {
                title: "Solaris Flare".to_string(),
                artist: "Antigravity Sound".to_string(),
                bpm: 128.0,
                musical_key: "9A".to_string(),
                duration_seconds: 412.0,
            },
            TrackMeta {
                title: "Danube Midnight".to_string(),
                artist: "Vienna Modular".to_string(),
                bpm: 126.0,
                musical_key: "8B".to_string(),
                duration_seconds: 356.0,
            },
            TrackMeta {
                title: "Velvet Acid Pulse".to_string(),
                artist: "Klangwerk".to_string(),
                bpm: 132.0,
                musical_key: "7A".to_string(),
                duration_seconds: 398.0,
            },
        ];

        let mut orch = Self {
            deck_a: AudioEngine::create_deck(DeckId::DeckA),
            deck_b: AudioEngine::create_deck(DeckId::DeckB),
            deck_c: AudioEngine::create_deck(DeckId::DeckC),
            deck_d: AudioEngine::create_deck(DeckId::DeckD),
            crossfader: 0.0,
            master_volume: 0.75,
            master_muted: false,
            crate_tracks,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };

        orch.deck_a.loaded_track = orch.crate_tracks.first().cloned();
        orch.deck_a.is_playing = true;
        orch.deck_b.loaded_track = orch.crate_tracks.get(1).cloned();
        orch.deck_c.loaded_track = orch.crate_tracks.get(2).cloned();

        orch
    }

    pub fn deck(&self, id: DeckId) -> &DeckState {
        match id {
            DeckId::DeckA => &self.deck_a,
            DeckId::DeckB => &self.deck_b,
            DeckId::DeckC => &self.deck_c,
            DeckId::DeckD => &self.deck_d,
        }
    }

    pub fn deck_mut(&mut self, id: DeckId) -> &mut DeckState {
        match id {
            DeckId::DeckA => &mut self.deck_a,
            DeckId::DeckB => &mut self.deck_b,
            DeckId::DeckC => &mut self.deck_c,
            DeckId::DeckD => &mut self.deck_d,
        }
    }

    fn snapshot(&self) -> MixerSnapshot {
        MixerSnapshot {
            deck_a: self.deck_a.clone(),
            deck_b: self.deck_b.clone(),
            deck_c: self.deck_c.clone(),
            deck_d: self.deck_d.clone(),
            crossfader: self.crossfader,
            master_volume: self.master_volume,
            master_muted: self.master_muted,
        }
    }

    fn restore(&mut self, snap: MixerSnapshot) {
        self.deck_a = snap.deck_a;
        self.deck_b = snap.deck_b;
        self.deck_c = snap.deck_c;
        self.deck_d = snap.deck_d;
        self.crossfader = snap.crossfader;
        self.master_volume = snap.master_volume;
        self.master_muted = snap.master_muted;
    }

    pub(crate) fn push_undo(&mut self) {
        let snap = self.snapshot();
        if self.undo_stack.len() >= MAX_UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(snap);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            let current = self.snapshot();
            self.redo_stack.push(current);
            self.restore(prev);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            let current = self.snapshot();
            self.undo_stack.push(current);
            self.restore(next);
            true
        } else {
            false
        }
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn plan_transition(&self) -> Result<TransitionPlan, DjError> {
        let track_a = self
            .deck_a
            .loaded_track
            .as_ref()
            .ok_or_else(|| DjError::Deck("Deck A empty".to_string()))?;
        let track_b = self
            .deck_b
            .loaded_track
            .as_ref()
            .ok_or_else(|| DjError::Deck("Deck B empty".to_string()))?;

        let eff_bpm_a = track_a.bpm * (1.0 + self.deck_a.pitch_bend_pct / 100.0);
        let eff_bpm_b = track_b.bpm * (1.0 + self.deck_b.pitch_bend_pct / 100.0);
        let harmonic = AudioEngine::is_camelot_compatible(&track_a.musical_key, &track_b.musical_key);
        let bpm_delta = eff_bpm_b - eff_bpm_a;

        let strategy = if harmonic {
            "Swap basslines at bar 16; fade Deck A vocals while isolating Deck B high-hats"
        } else {
            "Key clash detected: use a 4-bar drum-only loop bridge and hard-cut melodic stems"
        };

        Ok(TransitionPlan {
            from_track: format!("{} - {}", track_a.artist, track_a.title),
            to_track: format!("{} - {}", track_b.artist, track_b.title),
            from_bpm: eff_bpm_a,
            to_bpm: eff_bpm_b,
            bpm_delta,
            harmonic_compatible: harmonic,
            recommended_transition_bars: if bpm_delta.abs() < 4.0 { 32 } else { 16 },
            suggested_eq_strategy: strategy.to_string(),
        })
    }
}

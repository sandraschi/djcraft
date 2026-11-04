//! DJCraft Core Engine
//!
//! Provides 4-deck orchestration, master crossfader curves,
//! transition planner, and track library indexing with DatabaseCraft.

pub mod i18n;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use djcraft_audio::{AudioEngine, DeckId, DeckState, TrackMeta};

#[derive(Error, Debug)]
pub enum DjError {
    #[error("Deck error: {0}")]
    Deck(String),
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

pub struct DjOrchestrator {
    pub deck_a: DeckState,
    pub deck_b: DeckState,
    pub crossfader: f64, // -1.0 (full Deck A) to +1.0 (full Deck B)
}

impl DjOrchestrator {
    pub fn new() -> Self {
        let mut orch = Self {
            deck_a: AudioEngine::create_deck(DeckId::DeckA),
            deck_b: AudioEngine::create_deck(DeckId::DeckB),
            crossfader: 0.0,
        };

        orch.deck_a.loaded_track = Some(TrackMeta {
            title: "Subterranean Dub".to_string(),
            artist: "Resonant Swarm".to_string(),
            bpm: 126.0,
            musical_key: "8A".to_string(),
            duration_seconds: 384.0,
        });
        orch.deck_a.is_playing = true;

        orch.deck_b.loaded_track = Some(TrackMeta {
            title: "Solaris Flare".to_string(),
            artist: "Antigravity Sound".to_string(),
            bpm: 128.0,
            musical_key: "9A".to_string(),
            duration_seconds: 412.0,
        });

        orch
    }

    pub fn plan_transition(&self) -> Result<TransitionPlan, DjError> {
        let track_a = self.deck_a.loaded_track.as_ref()
            .ok_or_else(|| DjError::Deck("Deck A empty".to_string()))?;
        let track_b = self.deck_b.loaded_track.as_ref()
            .ok_or_else(|| DjError::Deck("Deck B empty".to_string()))?;

        let harmonic = AudioEngine::is_camelot_compatible(&track_a.musical_key, &track_b.musical_key);
        let bpm_delta = track_b.bpm - track_a.bpm;

        Ok(TransitionPlan {
            from_track: format!("{} - {}", track_a.artist, track_a.title),
            to_track: format!("{} - {}", track_b.artist, track_b.title),
            from_bpm: track_a.bpm,
            to_bpm: track_b.bpm,
            bpm_delta,
            harmonic_compatible: harmonic,
            recommended_transition_bars: if bpm_delta.abs() < 4.0 { 32 } else { 16 },
            suggested_eq_strategy: "Swap basslines at bar 16; fade Deck A vocals while isolating Deck B high-hats".to_string(),
        })
    }
}

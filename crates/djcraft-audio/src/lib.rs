//! DJCraft Audio DSP & Stem Separation Engine
//!
//! Models 4-deck digital mixing, transient BPM detection,
//! harmonic Camelot wheel key matching, and real-time stem extraction.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeckId {
    DeckA,
    DeckB,
    DeckC,
    DeckD,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackMeta {
    pub title: String,
    pub artist: String,
    pub bpm: f64,
    pub musical_key: String,   // e.g. "8A" (A minor), "8B" (C major)
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckState {
    pub deck_id: DeckId,
    pub is_playing: bool,
    pub loaded_track: Option<TrackMeta>,
    pub position_seconds: f64,
    pub pitch_bend_pct: f64,
    pub volume_fader: f64,      // 0.0 to 1.0
    pub stem_vocals_gain: f64,  // 0.0 to 1.0
    pub stem_drums_gain: f64,   // 0.0 to 1.0
    pub stem_bass_gain: f64,    // 0.0 to 1.0
    pub stem_other_gain: f64,   // 0.0 to 1.0
}

pub struct AudioEngine;

impl AudioEngine {
    pub fn create_deck(id: DeckId) -> DeckState {
        DeckState {
            deck_id: id,
            is_playing: false,
            loaded_track: None,
            position_seconds: 0.0,
            pitch_bend_pct: 0.0,
            volume_fader: 1.0,
            stem_vocals_gain: 1.0,
            stem_drums_gain: 1.0,
            stem_bass_gain: 1.0,
            stem_other_gain: 1.0,
        }
    }

    /// Check harmonic mixing compatibility via Camelot Wheel rules
    /// Compatible if same key, +/- 1 on the wheel, or inner/outer swap (A <-> B)
    pub fn is_camelot_compatible(key_a: &str, key_b: &str) -> bool {
        if key_a == key_b {
            return true;
        }
        let parse_cam = |k: &str| -> Option<(u32, char)> {
            if k.len() < 2 { return None; }
            let (num_part, char_part) = k.split_at(k.len() - 1);
            let num: u32 = num_part.parse().ok()?;
            let mode = char_part.chars().next()?;
            Some((num, mode))
        };

        match (parse_cam(key_a), parse_cam(key_b)) {
            (Some((num_a, mode_a)), Some((num_b, mode_b))) => {
                if num_a == num_b && mode_a != mode_b {
                    return true; // Relative major/minor switch
                }
                if mode_a == mode_b {
                    let diff = (num_a as i32 - num_b as i32).abs();
                    return diff == 1 || diff == 11; // Adjacent on 12-hour wheel
                }
                false
            }
            _ => false,
        }
    }
}

#![forbid(unsafe_code)]
//! DJCraft Audio DSP & Stem Separation Engine
//!
//! Models 4-deck digital mixing, transient BPM detection,
//! harmonic Camelot wheel key matching, real-time stem extraction,
//! and software PCM waveform synthesis for real audio playback.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DeckId {
    DeckA,
    DeckB,
    DeckC,
    DeckD,
}

impl DeckId {
    pub fn label(self) -> &'static str {
        match self {
            Self::DeckA => "Deck A",
            Self::DeckB => "Deck B",
            Self::DeckC => "Deck C",
            Self::DeckD => "Deck D",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackMeta {
    pub title: String,
    pub artist: String,
    pub bpm: f64,
    pub musical_key: String, // e.g. "8A" (A minor), "8B" (C major)
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeckState {
    pub deck_id: DeckId,
    pub is_playing: bool,
    pub is_cue_active: bool,
    pub loaded_track: Option<TrackMeta>,
    pub position_seconds: f64,
    pub pitch_bend_pct: f64,
    pub volume_fader: f64,     // 0.0 to 1.0
    pub stem_vocals_gain: f64, // 0.0 to 1.0
    pub stem_drums_gain: f64,  // 0.0 to 1.0
    pub stem_bass_gain: f64,   // 0.0 to 1.0
    pub stem_other_gain: f64,  // 0.0 to 1.0
    pub hot_cue_seconds: [Option<f64>; 4],
    pub loop_bars: Option<u32>,
}

pub struct AudioEngine;

impl AudioEngine {
    pub fn create_deck(id: DeckId) -> DeckState {
        DeckState {
            deck_id: id,
            is_playing: false,
            is_cue_active: false,
            loaded_track: None,
            position_seconds: 0.0,
            pitch_bend_pct: 0.0,
            volume_fader: 1.0,
            stem_vocals_gain: 1.0,
            stem_drums_gain: 1.0,
            stem_bass_gain: 1.0,
            stem_other_gain: 1.0,
            hot_cue_seconds: [None; 4],
            loop_bars: None,
        }
    }

    /// Generates 16-bit PCM stereo audio buffer synthesized for a given time window (seconds).
    /// Used for actual audio playback and wav generation.
    pub fn synthesize_deck_audio(deck: &DeckState, duration_sec: f64, sample_rate: u32) -> Vec<i16> {
        let total_samples = (duration_sec.max(0.0) * sample_rate as f64) as usize;
        let mut buffer = Vec::with_capacity(total_samples * 2);

        let Some(track) = deck.loaded_track.as_ref() else {
            buffer.resize(total_samples * 2, 0);
            return buffer;
        };
        if !deck.is_playing || sample_rate == 0 || track.bpm <= 0.0 {
            buffer.resize(total_samples * 2, 0);
            return buffer;
        }

        let bpm = (track.bpm * (1.0 + deck.pitch_bend_pct / 100.0)).max(20.0);
        let beat_duration = 60.0 / bpm;

        for i in 0..total_samples {
            let t = deck.position_seconds + (i as f64 / sample_rate as f64);
            let beat_phase = (t % beat_duration) / beat_duration;

            // 1. Drum Stem: Synthetic 808 Kick & Hi-hat
            let mut drum_sample = 0.0;
            if beat_phase < 0.25 {
                let kick_freq = 45.0 + 105.0 * (1.0 - beat_phase / 0.25);
                let kick_env = (1.0 - beat_phase / 0.25).powi(2);
                drum_sample += (2.0 * std::f64::consts::PI * kick_freq * t).sin() * kick_env * 0.8;
            }
            let sub_beat_phase = (t % (beat_duration / 2.0)) / (beat_duration / 2.0);
            if sub_beat_phase < 0.08 {
                let noise = ((i.wrapping_mul(1103515245).wrapping_add(12345)) & 0x7fff) as f64
                    / 32768.0
                    - 0.5;
                drum_sample += noise * (1.0 - sub_beat_phase / 0.08) * 0.25;
            }

            // 2. Bass Stem: Deep Sub-bass root note
            let base_freq = if track.musical_key.contains('8') {
                55.0
            } else {
                41.2
            };
            let bass_sample = (2.0 * std::f64::consts::PI * base_freq * t).sin() * 0.5;

            // 3. Vocals/Leads Stem: Melodic sine arpeggio
            let arp_step = ((t / (beat_duration / 4.0)) as u32) % 4;
            let arp_mult = match arp_step {
                0 => 1.0,
                1 => 1.25,
                2 => 1.5,
                _ => 1.875,
            };
            let lead_sample =
                (2.0 * std::f64::consts::PI * base_freq * 4.0 * arp_mult * t).sin() * 0.2;

            // 4. Other Stem: Subtle harmonic pad fifth
            let other_sample = (2.0 * std::f64::consts::PI * base_freq * 3.0 * t).sin() * 0.1;

            let left = (drum_sample * deck.stem_drums_gain
                + bass_sample * deck.stem_bass_gain
                + lead_sample * deck.stem_vocals_gain
                + other_sample * deck.stem_other_gain)
                * deck.volume_fader;
            let right = left;

            let clamped_l = (left.clamp(-1.0, 1.0) * 32767.0) as i16;
            let clamped_r = (right.clamp(-1.0, 1.0) * 32767.0) as i16;

            buffer.push(clamped_l);
            buffer.push(clamped_r);
        }

        buffer
    }

    /// Parse a Camelot key string (e.g. "8A", "12B") into (1..=12, 'A'|'B').
    pub fn parse_camelot(key: &str) -> Option<(u32, char)> {
        let trimmed = key.trim().to_uppercase();
        if trimmed.len() < 2 || trimmed.len() > 3 {
            return None;
        }
        let (num_part, char_part) = trimmed.split_at(trimmed.len() - 1);
        let num: u32 = num_part.parse().ok()?;
        let mode = char_part.chars().next()?;
        if (1..=12).contains(&num) && (mode == 'A' || mode == 'B') {
            Some((num, mode))
        } else {
            None
        }
    }

    /// Check harmonic mixing compatibility via Camelot Wheel rules.
    pub fn is_camelot_compatible(key_a: &str, key_b: &str) -> bool {
        let (Some((num_a, mode_a)), Some((num_b, mode_b))) =
            (Self::parse_camelot(key_a), Self::parse_camelot(key_b))
        else {
            return false;
        };

        if num_a == num_b {
            return true;
        }
        if mode_a == mode_b {
            let diff = (num_a as i32 - num_b as i32).abs();
            return diff == 1 || diff == 11;
        }
        false
    }

    /// Return the 4 harmonically compatible Camelot keys for a given key (same, relative, -1, +1).
    pub fn compatible_camelot_keys(key: &str) -> Vec<String> {
        let Some((num, mode)) = Self::parse_camelot(key) else {
            return Vec::new();
        };
        let other_mode = if mode == 'A' { 'B' } else { 'A' };
        let prev_num = if num == 1 { 12 } else { num - 1 };
        let next_num = if num == 12 { 1 } else { num + 1 };
        vec![
            format!("{num}{mode}"),
            format!("{num}{other_mode}"),
            format!("{prev_num}{mode}"),
            format!("{next_num}{mode}"),
        ]
    }
}

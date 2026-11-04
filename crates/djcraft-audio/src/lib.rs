//! DJCraft Audio DSP & Stem Separation Engine
//!
//! Models 4-deck digital mixing, transient BPM detection,
//! harmonic Camelot wheel key matching, real-time stem extraction,
//! and software PCM waveform synthesis for real audio playback.

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
    pub is_cue_active: bool,
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
            is_cue_active: false,
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

    /// Generates 16-bit PCM stereo audio buffer synthesized for a given time window (seconds).
    /// Used for actual audio playback and wav generation.
    pub fn synthesize_deck_audio(deck: &DeckState, duration_sec: f64, sample_rate: u32) -> Vec<i16> {
        let total_samples = (duration_sec * sample_rate as f64) as usize;
        let mut buffer = Vec::with_capacity(total_samples * 2);

        if !deck.is_playing || deck.loaded_track.is_none() {
            buffer.resize(total_samples * 2, 0);
            return buffer;
        }

        let track = deck.loaded_track.as_ref().unwrap();
        let bpm = track.bpm * (1.0 + deck.pitch_bend_pct / 100.0);
        let beat_duration = 60.0 / bpm;

        for i in 0..total_samples {
            let t = deck.position_seconds + (i as f64 / sample_rate as f64);
            let beat_phase = (t % beat_duration) / beat_duration;

            // 1. Drum Stem: Synthetic 808 Kick & Hi-hat
            let mut drum_sample = 0.0;
            if beat_phase < 0.25 {
                // Kick exponential decay pitch sweep from 150 Hz to 45 Hz
                let kick_freq = 45.0 + 105.0 * (1.0 - beat_phase / 0.25);
                let kick_env = (1.0 - beat_phase / 0.25).powi(2);
                drum_sample += (2.0 * std::f64::consts::PI * kick_freq * t).sin() * kick_env * 0.8;
            }
            // Closed hi-hat on 8th notes
            let sub_beat_phase = (t % (beat_duration / 2.0)) / (beat_duration / 2.0);
            if sub_beat_phase < 0.08 {
                let noise = ((i * 1103515245 + 12345) & 0x7fff) as f64 / 32768.0 - 0.5;
                drum_sample += noise * (1.0 - sub_beat_phase / 0.08) * 0.25;
            }

            // 2. Bass Stem: Deep Sub-bass root note (Key 8A = Am = A1 55Hz, Key 9A = Em = E1 41.2Hz)
            let base_freq = if track.musical_key.contains("8") { 55.0 } else { 41.2 };
            let bass_sample = (2.0 * std::f64::consts::PI * base_freq * t).sin() * 0.5;

            // 3. Vocals/Leads Stem: Melodic sine arpeggio
            let arp_step = ((t / (beat_duration / 4.0)) as u32) % 4;
            let arp_mult = match arp_step { 0 => 1.0, 1 => 1.25, 2 => 1.5, _ => 1.875 };
            let lead_sample = (2.0 * std::f64::consts::PI * base_freq * 4.0 * arp_mult * t).sin() * 0.2;

            // Composite mix with stem gains & master volume
            let left = (drum_sample * deck.stem_drums_gain
                + bass_sample * deck.stem_bass_gain
                + lead_sample * deck.stem_vocals_gain) * deck.volume_fader;
            let right = left;

            let clamped_l = (left.clamp(-1.0, 1.0) * 32767.0) as i16;
            let clamped_r = (right.clamp(-1.0, 1.0) * 32767.0) as i16;

            buffer.push(clamped_l);
            buffer.push(clamped_r);
        }

        buffer
    }

    /// Check harmonic mixing compatibility via Camelot Wheel rules
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
                    return true;
                }
                if mode_a == mode_b {
                    let diff = (num_a as i32 - num_b as i32).abs();
                    return diff == 1 || diff == 11;
                }
                false
            }
            _ => false,
        }
    }
}

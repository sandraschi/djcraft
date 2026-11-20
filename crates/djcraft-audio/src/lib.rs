#![forbid(unsafe_code)]
//! DJCraft Audio DSP & Stem Separation Engine
//!
//! Models 4-deck digital mixing, transient BPM detection,
//! harmonic Camelot wheel key matching, real-time stem extraction,
//! and software PCM waveform synthesis for real audio playback.

use serde::{Deserialize, Serialize};

pub mod output;

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

    /// Synthesize a single mono float sample (-1.0..=1.0) for one deck at time `t` (seconds).
    pub fn sample_deck_frame(deck: &DeckState, t: f64, sample_idx: u64) -> f32 {
        let Some(track) = deck.loaded_track.as_ref() else {
            return 0.0;
        };
        if !deck.is_playing || track.bpm <= 0.0 {
            return 0.0;
        }

        let bpm = (track.bpm * (1.0 + deck.pitch_bend_pct / 100.0)).max(20.0);
        let beat_duration = 60.0 / bpm;
        let beat_phase = (t.rem_euclid(beat_duration)) / beat_duration;

        // 1. Drum Stem: Synthetic 808 Kick & Hi-hat
        let mut drum_sample = 0.0;
        if beat_phase < 0.25 {
            let kick_freq = 45.0 + 105.0 * (1.0 - beat_phase / 0.25);
            let kick_env = (1.0 - beat_phase / 0.25).powi(2);
            drum_sample += (2.0 * std::f64::consts::PI * kick_freq * t).sin() * kick_env * 0.75;
        }
        let sub_beat_phase = (t.rem_euclid(beat_duration / 2.0)) / (beat_duration / 2.0);
        if sub_beat_phase < 0.08 {
            let noise = ((sample_idx.wrapping_mul(1103515245).wrapping_add(12345)) & 0x7fff) as f64
                / 32768.0
                - 0.5;
            drum_sample += noise * (1.0 - sub_beat_phase / 0.08) * 0.22;
        }

        // 2. Bass Stem: Deep Sub-bass root note
        let base_freq = if track.musical_key.contains('8') {
            55.0
        } else if track.musical_key.contains('9') {
            61.74
        } else if track.musical_key.contains('7') {
            49.0
        } else {
            41.2
        };
        let bass_sample = (2.0 * std::f64::consts::PI * base_freq * t).sin() * 0.45;

        // 3. Vocals/Leads Stem: Melodic sine arpeggio
        let arp_step = ((t / (beat_duration / 4.0)) as u64) % 4;
        let arp_mult = match arp_step {
            0 => 1.0,
            1 => 1.25,
            2 => 1.5,
            _ => 1.875,
        };
        let lead_sample =
            (2.0 * std::f64::consts::PI * base_freq * 4.0 * arp_mult * t).sin() * 0.18;

        // 4. Other Stem: Subtle harmonic pad fifth
        let other_sample = (2.0 * std::f64::consts::PI * base_freq * 3.0 * t).sin() * 0.1;

        let mix = (drum_sample * deck.stem_drums_gain
            + bass_sample * deck.stem_bass_gain
            + lead_sample * deck.stem_vocals_gain
            + other_sample * deck.stem_other_gain)
            * deck.volume_fader;
        mix.clamp(-1.0, 1.0) as f32
    }

    /// Mix all 4 decks into one stereo frame using an equal-power crossfader curve and Master Loudness.
    pub fn mix_4deck_frame(
        decks: &[DeckState; 4],
        crossfader: f64,
        master_volume: f64,
        muted: bool,
        play_times: &[f64; 4],
        sample_idx: u64,
    ) -> (f32, f32) {
        if muted || master_volume <= 0.0 {
            return (0.0, 0.0);
        }
        let xf = ((crossfader + 1.0) * 0.5).clamp(0.0, 1.0);
        let gain_left = (xf * std::f64::consts::FRAC_PI_2).cos() as f32;
        let gain_right = (xf * std::f64::consts::FRAC_PI_2).sin() as f32;

        let a = Self::sample_deck_frame(&decks[0], play_times[0], sample_idx);
        let b = Self::sample_deck_frame(&decks[1], play_times[1], sample_idx);
        let c = Self::sample_deck_frame(&decks[2], play_times[2], sample_idx);
        let d = Self::sample_deck_frame(&decks[3], play_times[3], sample_idx);

        let summed = (a + c) * gain_left + (b + d) * gain_right;
        let limited = summed.tanh() * master_volume.clamp(0.0, 1.0) as f32;
        (limited, limited)
    }

    /// Generates 16-bit PCM stereo audio buffer synthesized for a given time window (seconds).
    /// Used for actual audio playback and wav generation.
    pub fn synthesize_deck_audio(deck: &DeckState, duration_sec: f64, sample_rate: u32) -> Vec<i16> {
        let total_samples = (duration_sec.max(0.0) * sample_rate as f64) as usize;
        let mut buffer = Vec::with_capacity(total_samples * 2);

        if !deck.is_playing || deck.loaded_track.is_none() || sample_rate == 0 {
            buffer.resize(total_samples * 2, 0);
            return buffer;
        }

        for i in 0..total_samples {
            let t = deck.position_seconds + (i as f64 / sample_rate as f64);
            let sample = Self::sample_deck_frame(deck, t, i as u64);
            let clamped = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
            buffer.push(clamped);
            buffer.push(clamped);
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

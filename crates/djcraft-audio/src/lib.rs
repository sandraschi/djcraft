#![forbid(unsafe_code)]
//! DJCraft Audio DSP & Stem Separation Engine
//!
//! Models 4-deck digital mixing, transient BPM detection,
//! harmonic Camelot wheel key matching, real-time stem extraction,
//! and software PCM waveform synthesis for real audio playback.

use serde::{Deserialize, Serialize};

pub mod analysis;
pub mod dsp;
pub mod dvs;
pub mod fx;
pub mod output;
pub mod recorder;
pub mod sampler;
pub mod studio;

pub use analysis::{
    analyze_track_full, detect_musical_key, detect_phrases, KeyAnalysis, PhraseKind, PhraseSegment,
    TrackAnalysis,
};

pub use dvs::{list_input_devices, DvsCarrier, DvsDecoder, DvsInputStream, DvsTelemetry};

pub use fx::{
    builtin_fx_presets, fx_packs_dir, load_user_fx_presets_from_dir, save_fx_preset_to_dir,
    BipolarFilter, FxKind, FxPackId, FxPackPreset, FxSlotParams, FxStemScope, FxTarget, FxUnitDsp,
    FxUnitParams,
};
pub use recorder::{RecordingFormat, RecordingStopReport};
pub use sampler::{SamplePlayMode, SamplerEngine, SAMPLER_BANKS, SAMPLER_SLOTS};
pub use studio::{
    auto_slice_audio, DrumGenre, SampleSlice, StepSequencer, LANE_LABELS, STEP_COUNT, STEP_LANES,
};

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
    #[serde(default)]
    pub pfl_cue: bool,
    pub loaded_track: Option<TrackMeta>,
    pub position_seconds: f64,
    pub pitch_bend_pct: f64,
    pub keylock: bool,
    pub beatgrid_offset_sec: f64,
    pub audio_path: Option<String>,
    pub volume_fader: f64,     // 0.0 to 1.0
    pub stem_vocals_gain: f64, // 0.0 to 1.0
    pub stem_drums_gain: f64,  // 0.0 to 1.0
    pub stem_bass_gain: f64,   // 0.0 to 1.0
    pub stem_other_gain: f64,  // 0.0 to 1.0
    pub eq_low: f64,           // 0.0 (kill) to 2.0 (boost), 1.0 = flat
    pub eq_mid: f64,           // 0.0 (kill) to 2.0 (boost), 1.0 = flat
    pub eq_high: f64,          // 0.0 (kill) to 2.0 (boost), 1.0 = flat
    #[serde(default)]
    pub color_filter: f64, // -1.0 (LPF) .. 0.0 (flat) .. +1.0 (HPF)
    pub hot_cue_seconds: [Option<f64>; 4],
    pub loop_bars: Option<u32>,
    #[serde(default)]
    pub seek_gen: u64,
    #[serde(default)]
    pub scratch_velocity: Option<f64>,
    #[serde(default)]
    pub slip_mode: bool,
    #[serde(default)]
    pub quantize: bool,
    #[serde(default = "default_pitch_range")]
    pub pitch_range_pct: f64,
}

fn default_pitch_range() -> f64 {
    16.0
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CrossfaderCurve {
    #[default]
    Smooth,
    Linear,
    ScratchCut,
}

impl CrossfaderCurve {
    pub const ALL: [Self; 3] = [Self::Smooth, Self::Linear, Self::ScratchCut];

    pub fn label(self) -> &'static str {
        match self {
            Self::Smooth => "Smooth (Equal-Power)",
            Self::Linear => "Club Linear",
            Self::ScratchCut => "Battle Scratch Cut",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Smooth => "Smooth",
            Self::Linear => "Linear",
            Self::ScratchCut => "Cut",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "linear" | "club" => Self::Linear,
            "cut" | "scratch" | "scratchcut" | "sharp" => Self::ScratchCut,
            _ => Self::Smooth,
        }
    }
}

pub struct AudioEngine;

impl AudioEngine {
    /// Compute `(gain_a, gain_b)` for crossfader position `x` in `-1.0..=1.0`
    /// using the selected club curve and optional Hamster reverse switch.
    pub fn crossfader_gains_curved(x: f64, curve: CrossfaderCurve, reverse: bool) -> (f64, f64) {
        let mut t = ((x.clamp(-1.0, 1.0) + 1.0) * 0.5).clamp(0.0, 1.0);
        if reverse {
            t = 1.0 - t;
        }
        match curve {
            CrossfaderCurve::Smooth => {
                let angle = t * std::f64::consts::FRAC_PI_2;
                (angle.cos(), angle.sin())
            }
            CrossfaderCurve::Linear => (1.0 - t, t),
            CrossfaderCurve::ScratchCut => {
                // Sharp battle cut: full gain within 10% of the opposite edge
                let ga = if t < 0.90 {
                    1.0
                } else {
                    ((1.0 - t) / 0.10).clamp(0.0, 1.0)
                };
                let gb = if t > 0.10 {
                    1.0
                } else {
                    (t / 0.10).clamp(0.0, 1.0)
                };
                (ga, gb)
            }
        }
    }
    pub fn create_deck(id: DeckId) -> DeckState {
        DeckState {
            deck_id: id,
            is_playing: false,
            is_cue_active: false,
            pfl_cue: false,
            loaded_track: None,
            position_seconds: 0.0,
            pitch_bend_pct: 0.0,
            keylock: false,
            beatgrid_offset_sec: 0.0,
            audio_path: None,
            volume_fader: 1.0,
            stem_vocals_gain: 1.0,
            stem_drums_gain: 1.0,
            stem_bass_gain: 1.0,
            stem_other_gain: 1.0,
            eq_low: 1.0,
            eq_mid: 1.0,
            eq_high: 1.0,
            color_filter: 0.0,
            hot_cue_seconds: [None; 4],
            loop_bars: None,
            seek_gen: 0,
            scratch_velocity: None,
            slip_mode: false,
            quantize: false,
            pitch_range_pct: 16.0,
        }
    }

    /// Synthesize a single mono float sample (-1.0..=1.0) for one deck at time `t` (seconds).
    pub fn sample_deck_frame(deck: &DeckState, t: f64, sample_idx: u64) -> f32 {
        let Some(track) = deck.loaded_track.as_ref() else {
            return 0.0;
        };
        if (!deck.is_playing && deck.scratch_velocity.is_none()) || track.bpm <= 0.0 {
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

    /// Mix all 4 decks into independent Master `(L, R)` and Headphone PFL Cue `(L, R)` buses.
    ///
    /// PFL (Pre-Fader Listen) taps each cued deck BEFORE channel volume fader & crossfader,
    /// then blends PFL with Master according to `cue_mix` (0.0 = 100% PFL, 1.0 = 100% Master).
    /// When `split_cue` is true, returns `(cue_mono, master_mono)` on the master bus for
    /// single-output split-ear monitoring.
    #[allow(clippy::too_many_arguments)]
    pub fn mix_dual_bus_frame(
        decks: &[DeckState; 4],
        crossfader: f64,
        master_volume: f64,
        muted: bool,
        cue_volume: f64,
        cue_mix: f64,
        split_cue: bool,
        play_times: &[f64; 4],
        sample_idx: u64,
    ) -> ((f32, f32), (f32, f32)) {
        let master = Self::mix_4deck_frame(
            decks,
            crossfader,
            master_volume,
            muted,
            play_times,
            sample_idx,
        );

        let mut pfl_sum = 0.0_f32;
        for (i, deck) in decks.iter().enumerate() {
            if deck.pfl_cue {
                let mut pre = deck.clone();
                pre.volume_fader = 1.0;
                pfl_sum += Self::sample_deck_frame(&pre, play_times[i], sample_idx);
            }
        }
        let pfl_lim = pfl_sum.tanh();
        let mix_t = cue_mix.clamp(0.0, 1.0) as f32;
        let cv = cue_volume.clamp(0.0, 1.0) as f32;
        let cue_l = (pfl_lim * (1.0 - mix_t) + master.0 * mix_t).tanh() * cv;
        let cue_r = (pfl_lim * (1.0 - mix_t) + master.1 * mix_t).tanh() * cv;

        if split_cue {
            let cue_mono = (cue_l + cue_r) * 0.5;
            let master_mono = (master.0 + master.1) * 0.5;
            ((cue_mono, master_mono), (cue_l, cue_r))
        } else {
            (master, (cue_l, cue_r))
        }
    }

    /// Generates 16-bit PCM stereo audio buffer synthesized for a given time window (seconds).
    /// Used for actual audio playback and wav generation.
    pub fn synthesize_deck_audio(
        deck: &DeckState,
        duration_sec: f64,
        sample_rate: u32,
    ) -> Vec<i16> {
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

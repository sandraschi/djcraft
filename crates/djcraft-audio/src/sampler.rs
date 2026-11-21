#![forbid(unsafe_code)]
//! Real-time 4-Bank x 8-Slot DJ Sampler Engine (`SamplerEngine`).
//!
//! Ships with 8 instant synthesized club one-shots pre-loaded in Bank A (Air Horn,
//! Laser Zap, Sub Drop, Riser Sweep, Clap Hit, Rim Shot, Dub Siren, Scratch Chirp)
//! and supports loading arbitrary MP3/WAV/FLAC/OGG/M4A files via `symphonia` into
//! any of the 32 slots. Mixes lock-free into the live WASAPI Master bus.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::dsp::DecodedAudio;

pub const SAMPLER_BANKS: usize = 4;
pub const SAMPLER_SLOTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SamplePlayMode {
    #[default]
    Trigger,
    Loop,
}

impl SamplePlayMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Trigger => "Trig",
            Self::Loop => "Loop",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Trigger => Self::Loop,
            Self::Loop => Self::Trigger,
        }
    }
}

#[derive(Clone)]
pub struct SampleSlot {
    pub name: String,
    pub audio: Option<Arc<DecodedAudio>>,
    pub mode: SamplePlayMode,
    pub volume: f32,
    pub playing: bool,
    pub play_pos: f64,
}

impl SampleSlot {
    fn empty(idx: usize) -> Self {
        Self {
            name: format!("Slot {}", idx + 1),
            audio: None,
            mode: SamplePlayMode::Trigger,
            volume: 0.85,
            playing: false,
            play_pos: 0.0,
        }
    }
}

/// Real-time 4-bank x 8-slot stereo sampler mixed directly into the WASAPI Master bus.
pub struct SamplerEngine {
    pub banks: [[SampleSlot; SAMPLER_SLOTS]; SAMPLER_BANKS],
    pub master_volume: f32,
    sample_rate: u32,
}

impl SamplerEngine {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000);
        let mut banks: [[SampleSlot; SAMPLER_SLOTS]; SAMPLER_BANKS] = std::array::from_fn(|_| {
            std::array::from_fn(SampleSlot::empty)
        });

        // Pre-populate Bank A with 8 instant club one-shots
        let builtins: [(&str, Arc<DecodedAudio>); SAMPLER_SLOTS] = [
            ("Air Horn", Arc::new(synth_air_horn(sr))),
            ("Laser Zap", Arc::new(synth_laser_zap(sr))),
            ("Sub Drop", Arc::new(synth_sub_drop(sr))),
            ("Riser Sweep", Arc::new(synth_riser_sweep(sr))),
            ("Club Clap", Arc::new(synth_club_clap(sr))),
            ("Rim Shot", Arc::new(synth_rim_shot(sr))),
            ("Dub Siren", Arc::new(synth_dub_siren(sr))),
            ("Vinyl Chirp", Arc::new(synth_vinyl_chirp(sr))),
        ];
        for (idx, (name, audio)) in builtins.into_iter().enumerate() {
            banks[0][idx].name = name.to_string();
            banks[0][idx].audio = Some(audio);
        }

        Self {
            banks,
            master_volume: 0.85,
            sample_rate: sr,
        }
    }

    pub fn trigger(&mut self, bank: usize, slot: usize) {
        if bank >= SAMPLER_BANKS || slot >= SAMPLER_SLOTS {
            return;
        }
        let s = &mut self.banks[bank][slot];
        if s.audio.is_none() {
            return;
        }
        if s.mode == SamplePlayMode::Loop && s.playing {
            s.playing = false;
            s.play_pos = 0.0;
        } else {
            s.playing = true;
            s.play_pos = 0.0;
        }
    }

    pub fn stop_slot(&mut self, bank: usize, slot: usize) {
        if bank >= SAMPLER_BANKS || slot >= SAMPLER_SLOTS {
            return;
        }
        let s = &mut self.banks[bank][slot];
        s.playing = false;
        s.play_pos = 0.0;
    }

    pub fn stop_all(&mut self) {
        for bank in &mut self.banks {
            for s in bank {
                s.playing = false;
                s.play_pos = 0.0;
            }
        }
    }

    pub fn set_slot_audio(&mut self, bank: usize, slot: usize, name: String, audio: Arc<DecodedAudio>) {
        if bank >= SAMPLER_BANKS || slot >= SAMPLER_SLOTS {
            return;
        }
        let s = &mut self.banks[bank][slot];
        s.name = name;
        s.audio = Some(audio);
        s.playing = false;
        s.play_pos = 0.0;
    }

    pub fn set_slot_config(&mut self, bank: usize, slot: usize, mode: SamplePlayMode, volume: f32) {
        if bank >= SAMPLER_BANKS || slot >= SAMPLER_SLOTS {
            return;
        }
        let s = &mut self.banks[bank][slot];
        s.mode = mode;
        s.volume = volume.clamp(0.0, 1.0);
    }

    pub fn active_mask(&self, bank: usize) -> [bool; SAMPLER_SLOTS] {
        let mut mask = [false; SAMPLER_SLOTS];
        if bank < SAMPLER_BANKS {
            for (i, slot) in self.banks[bank].iter().enumerate() {
                mask[i] = slot.playing;
            }
        }
        mask
    }

    pub fn slot_names(&self, bank: usize) -> [String; SAMPLER_SLOTS] {
        std::array::from_fn(|i| {
            if bank < SAMPLER_BANKS {
                self.banks[bank][i].name.clone()
            } else {
                format!("Slot {}", i + 1)
            }
        })
    }

    /// Mix all currently playing sampler slots into one stereo `(L, R)` frame.
    pub fn next_stereo_frame(&mut self, out_sr: f64) -> (f32, f32) {
        if self.master_volume <= 0.001 {
            return (0.0, 0.0);
        }
        let mut sum_l = 0.0_f32;
        let mut sum_r = 0.0_f32;
        let safe_sr = out_sr.max(8000.0);

        for bank in &mut self.banks {
            for slot in bank {
                if !slot.playing {
                    continue;
                }
                let Some(audio) = slot.audio.as_ref() else {
                    slot.playing = false;
                    continue;
                };
                let len = audio.left.len();
                if len == 0 {
                    slot.playing = false;
                    continue;
                }
                let idx = slot.play_pos as usize;
                if idx >= len {
                    if slot.mode == SamplePlayMode::Loop {
                        slot.play_pos = 0.0;
                    } else {
                        slot.playing = false;
                        slot.play_pos = 0.0;
                        continue;
                    }
                }
                let i = (slot.play_pos as usize).min(len - 1);
                let g = slot.volume;
                sum_l += audio.left[i] * g;
                sum_r += audio.right[i] * g;
                let step = audio.sample_rate as f64 / safe_sr;
                slot.play_pos += step;
                if slot.play_pos as usize >= len {
                    if slot.mode == SamplePlayMode::Loop {
                        slot.play_pos = 0.0;
                    } else {
                        slot.playing = false;
                        slot.play_pos = 0.0;
                    }
                }
            }
        }

        let mv = self.master_volume.clamp(0.0, 1.0);
        let _ = self.sample_rate;
        (sum_l * mv, sum_r * mv)
    }
}

fn make_decoded(sr: u32, left: Vec<f32>, right: Vec<f32>) -> DecodedAudio {
    let duration_seconds = left.len() as f64 / sr.max(1) as f64;
    DecodedAudio {
        sample_rate: sr,
        left,
        right,
        duration_seconds,
    }
}

fn synth_air_horn(sr: u32) -> DecodedAudio {
    let n = (sr as f32 * 0.95) as usize;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let env = (t / 0.015).min(1.0) * (1.0 - (t / 0.95).powf(2.2));
        let pitch_bend = if t < 0.08 { 0.92 + t } else { 1.0 };
        let f1 = 392.0 * pitch_bend;
        let f2 = 493.88 * pitch_bend;
        let f3 = 587.33 * pitch_bend;
        let saw = |f: f32| -> f32 { ((t * f).fract() * 2.0 - 1.0) * 0.35 };
        let raw = (saw(f1) + saw(f2) + saw(f3) + (std::f32::consts::TAU * f1 * 2.0 * t).sin() * 0.25)
            .tanh();
        l.push(raw * env * 0.72);
        r.push(raw * env * 0.72);
    }
    make_decoded(sr, l, r)
}

fn synth_laser_zap(sr: u32) -> DecodedAudio {
    let n = (sr as f32 * 0.45) as usize;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    let mut phase = 0.0_f32;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let freq = 2400.0 * (120.0_f32 / 2400.0).powf(t);
        phase += std::f32::consts::TAU * freq / sr as f32;
        let s = phase.sin() * (1.0 - t) * 0.75;
        l.push(s);
        r.push(s);
    }
    make_decoded(sr, l, r)
}

fn synth_sub_drop(sr: u32) -> DecodedAudio {
    let n = (sr as f32 * 1.1) as usize;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    let mut phase = 0.0_f32;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let freq = 115.0 * (32.0_f32 / 115.0).powf(t.powf(0.7));
        phase += std::f32::consts::TAU * freq / sr as f32;
        let env = (1.0 - t).powf(1.4);
        let s = (phase.sin() * 1.3).tanh() * env * 0.85;
        l.push(s);
        r.push(s);
    }
    make_decoded(sr, l, r)
}

fn synth_riser_sweep(sr: u32) -> DecodedAudio {
    let n = (sr as f32 * 1.6) as usize;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    let mut lp = 0.0_f32;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let noise = ((((i as u64).wrapping_mul(1103515245).wrapping_add(12345)) & 0x7fff) as f32
            / 16384.0)
            - 1.0;
        let alpha = 0.04 + t * t * 0.55;
        lp += alpha * (noise - lp);
        let env = (0.12 + 0.88 * t.powf(1.4)) * (1.0 - (t - 0.96).max(0.0) * 25.0).clamp(0.0, 1.0);
        let s = lp * env * 0.75;
        l.push(s);
        r.push(s);
    }
    make_decoded(sr, l, r)
}

fn synth_club_clap(sr: u32) -> DecodedAudio {
    let n = (sr as f32 * 0.28) as usize;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let noise = ((((i as u64).wrapping_mul(1664525).wrapping_add(1013904223)) & 0x7fff) as f32
            / 16384.0)
            - 1.0;
        let burst = if t < 0.035 {
            ((t * 110.0).sin().abs()).powf(0.5)
        } else {
            (-(t - 0.035) * 16.0).exp()
        };
        let s = noise * burst * 0.7;
        l.push(s);
        r.push(s);
    }
    make_decoded(sr, l, r)
}

fn synth_rim_shot(sr: u32) -> DecodedAudio {
    let n = (sr as f32 * 0.12) as usize;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let env = (-t * 48.0).exp();
        let s = ((std::f32::consts::TAU * 420.0 * t).sin() * 0.6
            + (std::f32::consts::TAU * 1650.0 * t).sin() * 0.4)
            * env
            * 0.8;
        l.push(s);
        r.push(s);
    }
    make_decoded(sr, l, r)
}

fn synth_dub_siren(sr: u32) -> DecodedAudio {
    let n = (sr as f32 * 1.2) as usize;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    let mut phase = 0.0_f32;
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let lfo = (std::f32::consts::TAU * 5.5 * t).sin();
        let freq = 660.0 + lfo * 240.0;
        phase += std::f32::consts::TAU * freq / sr as f32;
        let env = (t / 0.02).min(1.0) * (1.0 - (t / 1.2).powf(2.0));
        let s = (phase.sin() * 1.4).tanh() * env * 0.58;
        l.push(s);
        r.push(s);
    }
    make_decoded(sr, l, r)
}

fn synth_vinyl_chirp(sr: u32) -> DecodedAudio {
    let n = (sr as f32 * 0.26) as usize;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    let mut phase = 0.0_f32;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let arch = (std::f32::consts::PI * t).sin();
        let freq = 320.0 + arch * 1100.0;
        phase += std::f32::consts::TAU * freq / sr as f32;
        let s = (phase.sin() + (phase * 2.0).sin() * 0.4) * arch * 0.6;
        l.push(s);
        r.push(s);
    }
    make_decoded(sr, l, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampler_builtin_8_slots_trigger_and_mix() {
        let mut eng = SamplerEngine::new(44100);
        for slot in 0..SAMPLER_SLOTS {
            eng.stop_all();
            eng.trigger(0, slot);
            assert!(eng.active_mask(0)[slot], "Slot {slot} should be playing");
            let mut energy = 0.0_f32;
            for _ in 0..512 {
                let (l, r) = eng.next_stereo_frame(44100.0);
                assert!(l.is_finite() && r.is_finite());
                energy += l.abs() + r.abs();
            }
            assert!(energy > 1.0, "Built-in sample slot {slot} produced silence");
        }
    }

    #[test]
    fn test_sampler_loop_toggle_and_stop_all() {
        let mut eng = SamplerEngine::new(44100);
        eng.set_slot_config(0, 5, SamplePlayMode::Loop, 0.9);
        eng.trigger(0, 5);
        assert!(eng.active_mask(0)[5]);
        // Triggering again in Loop mode toggles it off
        eng.trigger(0, 5);
        assert!(!eng.active_mask(0)[5]);

        eng.trigger(0, 0);
        eng.trigger(0, 2);
        eng.stop_all();
        assert_eq!(eng.active_mask(0), [false; SAMPLER_SLOTS]);
    }
}

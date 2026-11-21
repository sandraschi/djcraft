#![forbid(unsafe_code)]
//! Studio Beatmaker (16-step BPM-synced drum sequencer with genre patterns)
//! and Auto-Slice Sample Chopper ("Find Samples") for DJCraft.
//!
//! All drum one-shots are pre-synthesized on construction so `next_stereo_frame`
//! performs zero heap allocations on the real-time WASAPI audio callback thread.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::dsp::DecodedAudio;

pub const STEP_LANES: usize = 4;
pub const STEP_COUNT: usize = 16;

pub const LANE_LABELS: [&str; STEP_LANES] = ["Kick", "Snare", "Hi-Hat", "Clap"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DrumGenre {
    #[default]
    House,
    Techno,
    HipHop,
    Trap,
    DnB,
    Reggaeton,
}

impl DrumGenre {
    pub const ALL: [Self; 6] = [
        Self::House,
        Self::Techno,
        Self::HipHop,
        Self::Trap,
        Self::DnB,
        Self::Reggaeton,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::House => "House (4/4)",
            Self::Techno => "Techno (Driving)",
            Self::HipHop => "Hip-Hop (Boom Bap)",
            Self::Trap => "Trap (Half-Time)",
            Self::DnB => "DnB (Breakbeat)",
            Self::Reggaeton => "Reggaeton (Dembow)",
        }
    }

    /// Return a 4x16 step matrix `[Kick, Snare, Hi-Hat, Clap]`.
    pub fn pattern(self) -> [[bool; STEP_COUNT]; STEP_LANES] {
        let from_str = |s: &str| -> [bool; STEP_COUNT] {
            let mut row = [false; STEP_COUNT];
            for (i, ch) in s.chars().take(STEP_COUNT).enumerate() {
                row[i] = ch == 'x' || ch == 'X';
            }
            row
        };
        match self {
            Self::House => [
                from_str("x...x...x...x..."), // Kick
                from_str("....x.......x..."), // Snare
                from_str("..x...x...x...x."), // Offbeat Hi-Hat
                from_str("....x.......x..."), // Clap
            ],
            Self::Techno => [
                from_str("x...x...x...x..."),
                from_str("....x.......x..x"),
                from_str("x.x.x.x.x.x.x.x."),
                from_str("....x.......x..."),
            ],
            Self::HipHop => [
                from_str("x......x..x....."),
                from_str("....x.......x..."),
                from_str("x.x.x.x.x.x.x.x."),
                from_str("............x..."),
            ],
            Self::Trap => [
                from_str("x.....x...x....."),
                from_str("........x......."),
                from_str("x.xxx.x.x.xxx.xx"),
                from_str("........x......."),
            ],
            Self::DnB => [
                from_str("x.........x....."),
                from_str("....x.......x..."),
                from_str("x.x.x.x.x.x.x.x."),
                from_str("....x..........."),
            ],
            Self::Reggaeton => [
                from_str("x...x...x...x..."),
                from_str("...x..x....x..x."),
                from_str("x.x.x.x.x.x.x.x."),
                from_str("...x.......x...."),
            ],
        }
    }
}

/// Real-time 4-lane x 16-step drum sequencer locked to Deck A's master BPM.
pub struct StepSequencer {
    pub enabled: bool,
    pub volume: f32,
    pub genre: DrumGenre,
    pub steps: [[bool; STEP_COUNT]; STEP_LANES],
    pub current_step: usize,
    step_accum_sec: f64,
    first_tick: bool,
    voices: [Vec<f32>; STEP_LANES],
    voice_pos: [Option<usize>; STEP_LANES],
}

impl StepSequencer {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000);
        let genre = DrumGenre::House;
        Self {
            enabled: false,
            volume: 0.78,
            genre,
            steps: genre.pattern(),
            current_step: 0,
            step_accum_sec: 0.0,
            first_tick: true,
            voices: [
                synth_808_kick(sr),
                synth_909_snare(sr),
                synth_closed_hat(sr),
                synth_studio_clap(sr),
            ],
            voice_pos: [None; STEP_LANES],
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.current_step = 0;
            self.step_accum_sec = 0.0;
            self.first_tick = true;
        } else {
            self.voice_pos = [None; STEP_LANES];
        }
    }

    pub fn set_genre(&mut self, genre: DrumGenre) {
        self.genre = genre;
        self.steps = genre.pattern();
    }

    pub fn clear(&mut self) {
        self.steps = [[false; STEP_COUNT]; STEP_LANES];
    }

    pub fn set_step(&mut self, lane: usize, step: usize, active: bool) {
        if lane < STEP_LANES && step < STEP_COUNT {
            self.steps[lane][step] = active;
        }
    }

    /// Advance the sequencer by one audio sample at `out_sr` locked to `bpm` (16th-note grid).
    pub fn next_stereo_frame(&mut self, out_sr: f64, bpm: f64) -> (f32, f32) {
        if !self.enabled || self.volume <= 0.001 {
            return (0.0, 0.0);
        }
        let safe_bpm = bpm.clamp(60.0, 200.0);
        let sixteenth_sec = 60.0 / (safe_bpm * 4.0);
        let dt = 1.0 / out_sr.max(8000.0);

        if self.first_tick {
            self.first_tick = false;
            self.trigger_current_step();
        } else {
            self.step_accum_sec += dt;
            if self.step_accum_sec >= sixteenth_sec {
                self.step_accum_sec -= sixteenth_sec;
                self.current_step = (self.current_step + 1) % STEP_COUNT;
                self.trigger_current_step();
            }
        }

        let mut mix = 0.0_f32;
        for lane in 0..STEP_LANES {
            if let Some(pos) = self.voice_pos[lane] {
                if pos < self.voices[lane].len() {
                    mix += self.voices[lane][pos];
                    self.voice_pos[lane] = Some(pos + 1);
                } else {
                    self.voice_pos[lane] = None;
                }
            }
        }

        let out = mix.tanh() * self.volume.clamp(0.0, 1.0);
        (out, out)
    }

    fn trigger_current_step(&mut self) {
        for lane in 0..STEP_LANES {
            if self.steps[lane][self.current_step] {
                self.voice_pos[lane] = Some(0);
            }
        }
    }
}

/// One auto-chopped sample slice ready to audition or load into a Sampler pad.
#[derive(Clone)]
pub struct SampleSlice {
    pub label: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub audio: Arc<DecodedAudio>,
}

/// Detect the top `slice_count` musical transient/beat chops in `audio` ("Find Samples").
///
/// Uses RMS onset flux in 20 ms frames with a minimum inter-slice spacing so slices
/// span distinct musical hits rather than clustering on a single transient.
pub fn auto_slice_audio(audio: &DecodedAudio, slice_count: usize) -> Vec<SampleSlice> {
    let target = slice_count.clamp(1, 16);
    let total = audio.left.len();
    let sr = audio.sample_rate.max(8000) as usize;
    if total < sr / 4 {
        return Vec::new();
    }

    let hop = (sr / 50).max(128); // 20 ms hop
    let num_frames = total / hop;
    if num_frames < 4 {
        return Vec::new();
    }

    // 1. Compute per-frame RMS energy and positive onset flux
    let mut rms = vec![0.0_f32; num_frames];
    for (f, slot) in rms.iter_mut().enumerate() {
        let start = f * hop;
        let end = (start + hop).min(total);
        let mut acc = 0.0_f32;
        for i in start..end {
            let m = 0.5 * (audio.left[i] + audio.right[i]);
            acc += m * m;
        }
        *slot = (acc / (end - start).max(1) as f32).sqrt();
    }

    let mut peaks: Vec<(usize, f32)> = Vec::with_capacity(num_frames);
    for f in 1..num_frames {
        let flux = (rms[f] - rms[f - 1]).max(0.0);
        if flux > 0.015 {
            peaks.push((f, flux));
        }
    }
    peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // 2. Greedy peak picking with minimum spacing (at least 0.35s apart, or evenly spaced fallback)
    let min_gap_frames = ((sr as f64 * 0.35) / hop as f64).max(4.0) as usize;
    let mut chosen_frames: Vec<usize> = Vec::with_capacity(target);
    for (frame, _) in peaks {
        if chosen_frames.len() >= target {
            break;
        }
        if chosen_frames
            .iter()
            .all(|&c| c.abs_diff(frame) >= min_gap_frames)
        {
            chosen_frames.push(frame);
        }
    }

    // Fill any remaining slices evenly across the track so the DJ always gets `target` pads
    if chosen_frames.len() < target {
        let step = (num_frames / target).max(1);
        for k in 0..target {
            if chosen_frames.len() >= target {
                break;
            }
            let f = (k * step).min(num_frames - 1);
            if chosen_frames.iter().all(|&c| c.abs_diff(f) >= 2) {
                chosen_frames.push(f);
            }
        }
    }
    chosen_frames.sort_unstable();

    // 3. Extract stereo slice buffers (up to 1.5s each or until the next slice)
    let max_slice_samples = (sr as f64 * 1.5) as usize;
    let mut out = Vec::with_capacity(chosen_frames.len());
    for (idx, &start_frame) in chosen_frames.iter().enumerate() {
        let start_samp = (start_frame * hop).min(total - 1);
        let next_samp = chosen_frames
            .get(idx + 1)
            .map(|&nf| (nf * hop).min(total))
            .unwrap_or(total);
        let end_samp = next_samp.min(start_samp + max_slice_samples).min(total);
        if end_samp <= start_samp + 64 {
            continue;
        }
        let left = audio.left[start_samp..end_samp].to_vec();
        let right = audio.right[start_samp..end_samp].to_vec();
        let dur = left.len() as f64 / sr as f64;
        out.push(SampleSlice {
            label: format!("Chop {}", idx + 1),
            start_sec: start_samp as f64 / sr as f64,
            end_sec: end_samp as f64 / sr as f64,
            audio: Arc::new(DecodedAudio {
                sample_rate: audio.sample_rate,
                left,
                right,
                duration_seconds: dur,
            }),
        });
    }
    out
}

fn synth_808_kick(sr: u32) -> Vec<f32> {
    let n = (sr as f32 * 0.28) as usize;
    let mut out = Vec::with_capacity(n);
    let mut phase = 0.0_f32;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let freq = 46.0 + 115.0 * (1.0 - t).powf(4.0);
        phase += std::f32::consts::TAU * freq / sr as f32;
        let env = (1.0 - t).powf(1.8);
        out.push((phase.sin() * 1.4).tanh() * env * 0.92);
    }
    out
}

fn synth_909_snare(sr: u32) -> Vec<f32> {
    let n = (sr as f32 * 0.19) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let body = (std::f32::consts::TAU * 185.0 * t).sin() * (-t * 24.0).exp() * 0.55;
        let noise = ((((i as u64).wrapping_mul(1103515245).wrapping_add(12345)) & 0x7fff) as f32
            / 16384.0)
            - 1.0;
        let snap = noise * (-t * 18.0).exp() * 0.5;
        out.push((body + snap) * 0.85);
    }
    out
}

fn synth_closed_hat(sr: u32) -> Vec<f32> {
    let n = (sr as f32 * 0.06) as usize;
    let mut out = Vec::with_capacity(n);
    let mut prev = 0.0_f32;
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let noise = ((((i as u64).wrapping_mul(1664525).wrapping_add(1013904223)) & 0x7fff) as f32
            / 16384.0)
            - 1.0;
        let hp = noise - prev * 0.82;
        prev = noise;
        out.push(hp * (-t * 58.0).exp() * 0.48);
    }
    out
}

fn synth_studio_clap(sr: u32) -> Vec<f32> {
    let n = (sr as f32 * 0.22) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let noise =
            ((((i as u64).wrapping_mul(22695477).wrapping_add(1)) & 0x7fff) as f32 / 16384.0) - 1.0;
        let env = if t < 0.03 {
            ((t * 120.0).sin().abs()).powf(0.4)
        } else {
            (-(t - 0.03) * 19.0).exp()
        };
        out.push(noise * env * 0.65);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_sequencer_all_6_genres_produce_finite_bpm_locked_audio() {
        for genre in DrumGenre::ALL {
            let mut seq = StepSequencer::new(44100);
            seq.set_genre(genre);
            seq.set_enabled(true);
            let mut energy = 0.0_f32;
            for _ in 0..22_050 {
                let (l, r) = seq.next_stereo_frame(44100.0, 128.0);
                assert!(l.is_finite() && r.is_finite());
                energy += l.abs() + r.abs();
            }
            assert!(energy > 5.0, "Genre {genre:?} produced silent audio");
            assert!(seq.current_step > 0, "Sequencer step did not advance");
        }
    }

    #[test]
    fn test_auto_slice_audio_detects_transients_and_returns_valid_slices() {
        let sr = 44100_u32;
        let total = sr as usize * 4; // 4 seconds
        let mut buf = vec![0.0_f32; total];
        // Place 8 clear transient hits every 0.5s
        for hit in 0..8 {
            let start = hit * (sr as usize / 2);
            for i in 0..4000 {
                if start + i < total {
                    let env = 1.0 - (i as f32 / 4000.0);
                    buf[start + i] = (i as f32 * 0.08).sin() * env * 0.8;
                }
            }
        }
        let decoded = DecodedAudio {
            sample_rate: sr,
            left: buf.clone(),
            right: buf,
            duration_seconds: 4.0,
        };
        let slices = auto_slice_audio(&decoded, 8);
        assert_eq!(slices.len(), 8);
        for s in &slices {
            assert!(s.audio.left.len() > 64);
            assert!(s.end_sec > s.start_sec);
        }
    }
}

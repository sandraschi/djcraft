#![forbid(unsafe_code)]
//! Native Rust Musical Key (Krumhansl-Schmuckler Chroma -> Camelot) & Automatic Phrase Analysis.
//!
//! Zero external DSP dependencies: uses Goertzel pitch-class energy extraction across octaves
//! C2..B6 for musical key detection and 8-bar/16-bar RMS energy + novelty segmentation for
//! club phrase labels (`Intro`, `Buildup`, `Drop`, `Breakdown`, `Outro`).

use serde::{Deserialize, Serialize};

use crate::dsp::{compute_auto_gain_linear, detect_beatgrid, BeatgridInfo, DecodedAudio};

/// Detected musical key and its Camelot wheel code (`1A..12B`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyAnalysis {
    pub musical_key: String,
    pub camelot: String,
    pub is_minor: bool,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhraseKind {
    Intro,
    Buildup,
    Drop,
    Breakdown,
    Outro,
}

impl PhraseKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Intro => "Intro",
            Self::Buildup => "Buildup",
            Self::Drop => "Drop",
            Self::Breakdown => "Breakdown",
            Self::Outro => "Outro",
        }
    }

    pub fn rgb(self) -> [u8; 3] {
        match self {
            Self::Intro => [56, 189, 248],     // Cyan
            Self::Buildup => [245, 158, 11],   // Amber
            Self::Drop => [239, 68, 68],       // Crimson
            Self::Breakdown => [168, 85, 247], // Purple
            Self::Outro => [20, 184, 166],     // Teal
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhraseSegment {
    pub kind: PhraseKind,
    pub start_sec: f64,
    pub end_sec: f64,
    pub energy: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackAnalysis {
    pub beatgrid: BeatgridInfo,
    pub key: KeyAnalysis,
    pub phrases: Vec<PhraseSegment>,
    pub auto_gain_linear: f64,
}

// Krumhansl-Schmuckler pitch-class key profiles (C, C#, D, D#, E, F, F#, G, G#, A, A#, B)
const KS_MAJOR: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const KS_MINOR: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

const MAJOR_NAMES: [(&str, &str); 12] = [
    ("C major", "8B"),
    ("Db major", "3B"),
    ("D major", "10B"),
    ("Eb major", "5B"),
    ("E major", "12B"),
    ("F major", "7B"),
    ("F# major", "2B"),
    ("G major", "9B"),
    ("Ab major", "4B"),
    ("A major", "11B"),
    ("Bb major", "6B"),
    ("B major", "1B"),
];

const MINOR_NAMES: [(&str, &str); 12] = [
    ("C minor", "5A"),
    ("C# minor", "12A"),
    ("D minor", "7A"),
    ("Eb minor", "2A"),
    ("E minor", "9A"),
    ("F minor", "4A"),
    ("F# minor", "11A"),
    ("G minor", "6A"),
    ("Ab minor", "1A"),
    ("A minor", "8A"),
    ("Bb minor", "3A"),
    ("B minor", "10A"),
];

fn goertzel_power(samples: &[f32], freq_hz: f64, sr: f64) -> f64 {
    let n = samples.len();
    if n == 0 || freq_hz <= 0.0 || freq_hz >= sr * 0.48 {
        return 0.0;
    }
    let omega = 2.0 * std::f64::consts::PI * freq_hz / sr;
    let coeff = 2.0 * omega.cos();
    let mut s_prev = 0.0_f64;
    let mut s_prev2 = 0.0_f64;
    for &x in samples {
        let s = (x as f64) + coeff * s_prev - s_prev2;
        s_prev2 = s_prev;
        s_prev = s;
    }
    let power = s_prev2 * s_prev2 + s_prev * s_prev - coeff * s_prev * s_prev2;
    (power / (n as f64 * n as f64)).max(0.0)
}

fn pearson_corr(a: &[f64; 12], b: &[f64; 12], shift: usize) -> f64 {
    let mean_a = a.iter().sum::<f64>() / 12.0;
    let mean_b = b.iter().sum::<f64>() / 12.0;
    let mut num = 0.0;
    let mut da = 0.0;
    let mut db = 0.0;
    for i in 0..12 {
        let xa = a[(i + shift) % 12] - mean_a;
        let xb = b[i] - mean_b;
        num += xa * xb;
        da += xa * xa;
        db += xb * xb;
    }
    let denom = (da * db).sqrt();
    if denom < 1e-12 {
        0.0
    } else {
        num / denom
    }
}

/// Detect musical key and Camelot wheel code (`1A..12B`) from decoded stereo audio.
pub fn detect_musical_key(audio: &DecodedAudio) -> KeyAnalysis {
    let n = audio.len_frames();
    let sr = audio.sample_rate.max(1) as f64;
    if n < 2048 {
        return KeyAnalysis {
            musical_key: "A minor".to_string(),
            camelot: "8A".to_string(),
            is_minor: true,
            confidence: 0.0,
        };
    }

    // Downsample mono by 4x (~11 kHz) over up to 45 seconds from the track body for fast chromagram
    let step = 4usize;
    let eff_sr = sr / step as f64;
    let start_frame = if n > (sr as usize) * 20 {
        (n / 6).min((sr as usize) * 15)
    } else {
        0
    };
    let max_span = ((sr as usize) * 45).min(n.saturating_sub(start_frame));
    let block_len = 2048usize;
    let mut mono = Vec::with_capacity(block_len);
    let mut chroma = [0.0_f64; 12];

    // Sample up to 16 evenly-spaced windows across the span
    let num_windows = 16usize;
    let window_stride = (max_span / num_windows).max(block_len * step);

    for w in 0..num_windows {
        let base = start_frame + w * window_stride;
        if base + block_len * step > n {
            break;
        }
        mono.clear();
        for k in 0..block_len {
            let idx = base + k * step;
            let hann =
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * k as f32 / block_len as f32).cos());
            mono.push(0.5 * (audio.left[idx] + audio.right[idx]) * hann);
        }
        // Sum pitch-class energy across octaves 2..=5 (MIDI 36..83, ~65 Hz .. 1046 Hz)
        for (pc, bin) in chroma.iter_mut().enumerate() {
            let mut pc_energy = 0.0;
            for oct in 2..=5 {
                let midi = (oct + 1) * 12 + pc as i32;
                let freq = 440.0 * 2.0_f64.powf((midi - 69) as f64 / 12.0);
                pc_energy += goertzel_power(&mono, freq, eff_sr).sqrt();
            }
            *bin += pc_energy;
        }
    }

    let mut best_corr = f64::NEG_INFINITY;
    let mut second_corr = f64::NEG_INFINITY;
    let mut best_pc = 9usize; // A
    let mut best_minor = true;

    for pc in 0..12 {
        let c_maj = pearson_corr(&chroma, &KS_MAJOR, pc);
        if c_maj > best_corr {
            second_corr = best_corr;
            best_corr = c_maj;
            best_pc = pc;
            best_minor = false;
        } else if c_maj > second_corr {
            second_corr = c_maj;
        }

        let c_min = pearson_corr(&chroma, &KS_MINOR, pc);
        if c_min > best_corr {
            second_corr = best_corr;
            best_corr = c_min;
            best_pc = pc;
            best_minor = true;
        } else if c_min > second_corr {
            second_corr = c_min;
        }
    }

    let (musical_key, camelot) = if best_minor {
        MINOR_NAMES[best_pc]
    } else {
        MAJOR_NAMES[best_pc]
    };
    let margin = (best_corr - second_corr).clamp(0.0, 1.0);
    let confidence = ((best_corr.clamp(0.0, 1.0) * 0.65 + margin * 1.4).clamp(0.0, 1.0)) as f32;

    KeyAnalysis {
        musical_key: musical_key.to_string(),
        camelot: camelot.to_string(),
        is_minor: best_minor,
        confidence,
    }
}

/// Segment a track into 8-bar club phrases (`Intro`, `Buildup`, `Drop`, `Breakdown`, `Outro`).
pub fn detect_phrases(audio: &DecodedAudio, bpm: f64, first_beat_sec: f64) -> Vec<PhraseSegment> {
    let dur = audio.duration_seconds;
    if dur < 4.0 || audio.len_frames() < 1024 {
        return Vec::new();
    }
    let sr = audio.sample_rate.max(1) as f64;
    let safe_bpm = bpm.clamp(60.0, 200.0);
    let phrase_sec = (60.0 / safe_bpm) * 32.0; // 8 bars = 32 beats
    let phrase_sec = phrase_sec.clamp(4.0, (dur * 0.5).max(4.0));

    let mut raw_blocks: Vec<(f64, f64, f32)> = Vec::new();
    let mut t = first_beat_sec.clamp(0.0, (dur * 0.25).min(4.0));
    if t > 1.0 {
        raw_blocks.push((0.0, t, 0.1));
    }
    while t < dur - 0.5 {
        let end = (t + phrase_sec).min(dur);
        let s0 = ((t * sr) as usize).min(audio.len_frames());
        let s1 = ((end * sr) as usize).min(audio.len_frames());
        if s1 > s0 {
            let stride = ((s1 - s0) / 2048).max(1);
            let mut sum_sq = 0.0_f64;
            let mut count = 0usize;
            let mut i = s0;
            while i < s1 {
                let m = 0.5 * (audio.left[i] + audio.right[i]) as f64;
                sum_sq += m * m;
                count += 1;
                i += stride;
            }
            let rms = (sum_sq / count.max(1) as f64).sqrt() as f32;
            raw_blocks.push((t, end, rms));
        }
        t = end;
    }

    if raw_blocks.is_empty() {
        return Vec::new();
    }

    let max_rms = raw_blocks.iter().map(|b| b.2).fold(1e-6_f32, f32::max);
    let count = raw_blocks.len();
    let mut segments = Vec::with_capacity(count);

    for (idx, (start_sec, end_sec, rms)) in raw_blocks.iter().enumerate() {
        let norm_e = (*rms / max_rms).clamp(0.0, 1.0);
        let next_e = raw_blocks
            .get(idx + 1)
            .map(|b| (b.2 / max_rms).clamp(0.0, 1.0))
            .unwrap_or(norm_e);

        let kind = if idx == 0 && norm_e < 0.72 {
            PhraseKind::Intro
        } else if idx + 1 == count && norm_e < 0.72 {
            PhraseKind::Outro
        } else if norm_e >= 0.74 {
            PhraseKind::Drop
        } else if next_e >= 0.74 && norm_e >= 0.42 {
            PhraseKind::Buildup
        } else {
            PhraseKind::Breakdown
        };

        // Merge consecutive identical phrases for clean overview ribbons
        if let Some(last) = segments.last_mut() {
            let last_seg: &mut PhraseSegment = last;
            if last_seg.kind == kind {
                last_seg.end_sec = *end_sec;
                last_seg.energy = (last_seg.energy + norm_e) * 0.5;
                continue;
            }
        }
        segments.push(PhraseSegment {
            kind,
            start_sec: *start_sec,
            end_sec: *end_sec,
            energy: norm_e,
        });
    }

    segments
}

/// Run full offline/load-time track analysis (Beatgrid + Camelot Key + Phrases + LUFS Auto-Gain).
pub fn analyze_track_full(audio: &DecodedAudio) -> TrackAnalysis {
    let beatgrid = detect_beatgrid(audio);
    let key = detect_musical_key(audio);
    let phrases = detect_phrases(audio, beatgrid.bpm, beatgrid.first_beat_sec);
    let auto_gain_linear = compute_auto_gain_linear(audio);
    TrackAnalysis {
        beatgrid,
        key,
        phrases,
        auto_gain_linear,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_chord(freqs: &[f64], secs: f64, sr: u32) -> DecodedAudio {
        let n = (secs * sr as f64) as usize;
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f64 / sr as f64;
            let mut v = 0.0_f64;
            for &f in freqs {
                v += (2.0 * std::f64::consts::PI * f * t).sin();
            }
            let s = (v / freqs.len().max(1) as f64 * 0.6) as f32;
            left.push(s);
            right.push(s);
        }
        DecodedAudio {
            sample_rate: sr,
            left,
            right,
            duration_seconds: secs,
        }
    }

    #[test]
    fn test_detect_musical_key_identifies_a_minor_and_c_major() {
        // A minor triad (A3=220, C4=261.63, E4=329.63) + A2=110
        let am = synth_chord(&[110.0, 220.0, 261.63, 329.63], 6.0, 44100);
        let res_am = detect_musical_key(&am);
        assert_eq!(
            res_am.camelot, "8A",
            "expected A minor (8A), got {res_am:?}"
        );

        // C major triad (C3=130.81, C4=261.63, E4=329.63, G4=392.00)
        let cmaj = synth_chord(&[130.81, 261.63, 329.63, 392.00], 6.0, 44100);
        let res_c = detect_musical_key(&cmaj);
        assert_eq!(res_c.camelot, "8B", "expected C major (8B), got {res_c:?}");
    }

    #[test]
    fn test_detect_phrases_classifies_intro_drop_and_outro() {
        let sr = 44100u32;
        let bpm = 128.0;
        let phrase_sec = (60.0 / bpm) * 32.0; // 15.0s per 8-bar phrase
        let total_sec = phrase_sec * 3.0;
        let n = (total_sec * sr as f64) as usize;
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f64 / sr as f64;
            let amp = if t < phrase_sec {
                0.20 // Intro
            } else if t < phrase_sec * 2.0 {
                0.90 // Drop
            } else {
                0.18 // Outro
            };
            let s = ((2.0 * std::f64::consts::PI * 110.0 * t).sin() * amp) as f32;
            left.push(s);
            right.push(s);
        }
        let audio = DecodedAudio {
            sample_rate: sr,
            left,
            right,
            duration_seconds: total_sec,
        };
        let phrases = detect_phrases(&audio, bpm, 0.0);
        assert!(phrases.len() >= 3, "expected >=3 phrases, got {phrases:?}");
        assert_eq!(phrases.first().map(|p| p.kind), Some(PhraseKind::Intro));
        assert!(phrases.iter().any(|p| p.kind == PhraseKind::Drop));
        assert_eq!(phrases.last().map(|p| p.kind), Some(PhraseKind::Outro));
    }
}

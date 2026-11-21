#![forbid(unsafe_code)]
//! Pure-Rust DSP: Symphonia file decoding, Catmull-Rom varispeed resampling,
//! WSOLA keylock time-stretching, and autocorrelation beatgrid detection.

use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub sample_rate: u32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub duration_seconds: f64,
}

impl DecodedAudio {
    pub fn len_frames(&self) -> usize {
        self.left.len().min(self.right.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len_frames() == 0
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BeatgridInfo {
    pub bpm: f64,
    pub first_beat_sec: f64,
    pub confidence: f64,
}

/// Decode an MP3, WAV, FLAC, OGG, or M4A/AAC file into stereo f32 PCM in 100% Safe Rust.
pub fn decode_audio_file(path: &Path) -> Result<DecodedAudio, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Unsupported audio format: {e}"))?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| "No decodable audio track found".to_string())?;
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Codec init failed: {e}"))?;

    let mut left: Vec<f32> = Vec::new();
    let mut right: Vec<f32> = Vec::new();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(pkt) => pkt,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) => break,
        };

        let spec = *decoded.spec();
        let channels = spec.channels.count().max(1);
        let frames = decoded.frames();
        let buf = sample_buf.get_or_insert_with(|| SampleBuffer::<f32>::new(frames as u64, spec));
        if buf.capacity() < frames * channels {
            *buf = SampleBuffer::<f32>::new(frames as u64, spec);
        }
        buf.copy_interleaved_ref(decoded);
        let samples = buf.samples();

        left.reserve(frames);
        right.reserve(frames);
        for chunk in samples.chunks_exact(channels) {
            let l = chunk[0];
            let r = if channels > 1 { chunk[1] } else { l };
            left.push(l);
            right.push(r);
        }
    }

    if left.is_empty() {
        return Err("Decoded audio contained 0 samples".to_string());
    }

    let duration_seconds = left.len() as f64 / sample_rate.max(1) as f64;
    Ok(DecodedAudio {
        sample_rate,
        left,
        right,
        duration_seconds,
    })
}

/// 4-point Catmull-Rom cubic Hermite interpolation for smooth vinyl varispeed playback.
pub fn sample_varispeed(audio: &DecodedAudio, playhead_samples: f64) -> (f32, f32) {
    let n = audio.len_frames();
    if n == 0 || playhead_samples < 0.0 || playhead_samples >= n as f64 {
        return (0.0, 0.0);
    }

    let idx = playhead_samples.floor() as isize;
    let frac = (playhead_samples - idx as f64) as f32;

    let get = |buf: &[f32], i: isize| -> f32 {
        let clamped = i.clamp(0, (n - 1) as isize) as usize;
        buf[clamped]
    };

    let catmull = |y0: f32, y1: f32, y2: f32, y3: f32, t: f32| -> f32 {
        let t2 = t * t;
        let t3 = t2 * t;
        0.5 * ((2.0 * y1)
            + (-y0 + y2) * t
            + (2.0 * y0 - 5.0 * y1 + 4.0 * y2 - y3) * t2
            + (-y0 + 3.0 * y1 - 3.0 * y2 + y3) * t3)
    };

    let l = catmull(
        get(&audio.left, idx - 1),
        get(&audio.left, idx),
        get(&audio.left, idx + 1),
        get(&audio.left, idx + 2),
        frac,
    );
    let r = catmull(
        get(&audio.right, idx - 1),
        get(&audio.right, idx),
        get(&audio.right, idx + 1),
        get(&audio.right, idx + 2),
        frac,
    );
    (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0))
}

/// Pure-Rust WSOLA (Waveform Similarity Overlap-Add) time-stretcher for Keylock / Master Tempo.
/// Preserves original pitch while advancing through `DecodedAudio` at `tempo_ratio`.
pub struct WsolaDeckState {
    grain_len: usize,
    hop_out: usize,
    search_radius: usize,
    out_cursor: usize,
    in_pos: f64,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
    tail_l: Vec<f32>,
    tail_r: Vec<f32>,
    window: Vec<f32>,
}

impl Default for WsolaDeckState {
    fn default() -> Self {
        Self::new(44100)
    }
}

impl WsolaDeckState {
    pub fn new(sample_rate: u32) -> Self {
        let grain_len = ((sample_rate as usize) * 40 / 1000).clamp(512, 4096) & !1;
        let hop_out = grain_len / 2;
        let search_radius = ((sample_rate as usize) * 8 / 1000).clamp(64, 512);
        let window: Vec<f32> = (0..grain_len)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / grain_len as f32).cos()))
            .collect();
        Self {
            grain_len,
            hop_out,
            search_radius,
            out_cursor: hop_out,
            in_pos: 0.0,
            out_l: vec![0.0; hop_out],
            out_r: vec![0.0; hop_out],
            tail_l: vec![0.0; hop_out],
            tail_r: vec![0.0; hop_out],
            window,
        }
    }

    pub fn reset(&mut self, start_sample: f64) {
        self.in_pos = start_sample.max(0.0);
        self.out_cursor = self.hop_out;
        self.out_l.fill(0.0);
        self.out_r.fill(0.0);
        self.tail_l.fill(0.0);
        self.tail_r.fill(0.0);
    }

    pub fn current_input_sample(&self) -> f64 {
        self.in_pos
    }

    fn synthesize_next_hop(&mut self, audio: &DecodedAudio, tempo_ratio: f64) {
        let n = audio.len_frames();
        if n <= self.grain_len {
            self.out_l.fill(0.0);
            self.out_r.fill(0.0);
            return;
        }

        let target = self.in_pos.round() as isize;
        let max_start = (n - self.grain_len) as isize;
        let min_s = (target - self.search_radius as isize).clamp(0, max_start);
        let max_s = (target + self.search_radius as isize).clamp(0, max_start);

        // Coarse-stepped cross-correlation against previous overlap tail for phase coherence
        let mut best_start = target.clamp(0, max_start) as usize;
        let mut best_score = f32::NEG_INFINITY;
        let step = 4usize;
        let check_len = (self.hop_out / 2).min(256);

        let mut cand = min_s;
        while cand <= max_s {
            let c = cand as usize;
            let mut dot = 0.0_f32;
            let mut i = 0;
            while i < check_len {
                dot += self.tail_l[i] * audio.left[c + i] + self.tail_r[i] * audio.right[c + i];
                i += 2;
            }
            if dot > best_score {
                best_score = dot;
                best_start = c;
            }
            cand += step as isize;
        }

        // Overlap-add first half with previous tail, store second half as next tail
        for i in 0..self.hop_out {
            let w_in = self.window[i];
            self.out_l[i] = self.tail_l[i] + audio.left[best_start + i] * w_in;
            self.out_r[i] = self.tail_r[i] + audio.right[best_start + i] * w_in;

            let w_out = self.window[self.hop_out + i];
            self.tail_l[i] = audio.left[best_start + self.hop_out + i] * w_out;
            self.tail_r[i] = audio.right[best_start + self.hop_out + i] * w_out;
        }

        self.in_pos += self.hop_out as f64 * tempo_ratio.clamp(0.5, 2.0);
        self.out_cursor = 0;
    }

    pub fn next_sample(&mut self, audio: &DecodedAudio, tempo_ratio: f64) -> (f32, f32) {
        if self.in_pos >= audio.len_frames() as f64 {
            return (0.0, 0.0);
        }
        if self.out_cursor >= self.hop_out {
            self.synthesize_next_hop(audio, tempo_ratio);
        }
        let l = self.out_l[self.out_cursor];
        let r = self.out_r[self.out_cursor];
        self.out_cursor += 1;
        (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0))
    }
}

/// Detect BPM and first downbeat phase from decoded stereo PCM using onset autocorrelation.
pub fn detect_beatgrid(audio: &DecodedAudio) -> BeatgridInfo {
    let sr = audio.sample_rate.max(1) as usize;
    let hop = (sr / 100).max(1); // 10 ms analysis frames (100 Hz envelope)
    let max_frames = (sr * 90).min(audio.len_frames()); // analyze up to first 90s
    if max_frames < sr * 2 {
        return BeatgridInfo {
            bpm: 120.0,
            first_beat_sec: 0.0,
            confidence: 0.0,
        };
    }

    let num_env = max_frames / hop;
    let mut energy = Vec::with_capacity(num_env);
    for f in 0..num_env {
        let start = f * hop;
        let end = (start + hop).min(max_frames);
        let mut sum_sq = 0.0_f64;
        for i in start..end {
            let m = 0.5 * (audio.left[i] as f64 + audio.right[i] as f64);
            sum_sq += m * m;
        }
        energy.push((sum_sq / (end - start).max(1) as f64).sqrt());
    }

    // Half-wave rectified onset strength envelope
    let mut onset = vec![0.0_f64; num_env];
    for i in 1..num_env {
        onset[i] = (energy[i] - energy[i - 1]).max(0.0);
    }

    // Autocorrelation over 70..=175 BPM (lags in 10ms hops)
    let env_rate = sr as f64 / hop as f64;
    let min_lag = ((60.0 / 175.0) * env_rate).floor().max(1.0) as usize;
    let max_lag = ((60.0 / 70.0) * env_rate).ceil() as usize;

    let mut best_lag = ((60.0 / 120.0) * env_rate).round() as usize;
    let mut best_score = 0.0_f64;
    let mut total_energy = 0.0_f64;
    for &o in &onset {
        total_energy += o * o;
    }

    for lag in min_lag..=max_lag.min(num_env / 2) {
        let mut corr = 0.0_f64;
        let len = num_env - lag;
        for i in 0..len {
            corr += onset[i] * onset[i + lag];
        }
        let bpm = 60.0 * env_rate / lag as f64;
        // Gentle DJ tempo prior centered around 125 BPM to reduce half/double octave flips
        let prior = if (95.0..=148.0).contains(&bpm) {
            1.15
        } else {
            1.0
        };
        let score = corr * prior;
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }

    let raw_bpm = (60.0 * env_rate / best_lag.max(1) as f64).clamp(60.0, 200.0);
    let bpm = (raw_bpm * 10.0).round() / 10.0;

    // Phase alignment: find downbeat offset within one beat period maximizing onset energy
    let beat_hops = best_lag.max(1);
    let mut best_phase = 0usize;
    let mut best_phase_sum = -1.0_f64;
    for phase in 0..beat_hops {
        let mut s = 0.0_f64;
        let mut idx = phase;
        while idx < num_env {
            s += onset[idx];
            idx += beat_hops;
        }
        if s > best_phase_sum {
            best_phase_sum = s;
            best_phase = phase;
        }
    }

    let first_beat_sec = best_phase as f64 / env_rate;
    let confidence = if total_energy > 1e-9 {
        (best_score / total_energy).clamp(0.0, 1.0)
    } else {
        0.0
    };

    BeatgridInfo {
        bpm,
        first_beat_sec,
        confidence,
    }
}

/// Precomputed 10 ms peak overview of a decoded audio track for 120 FPS waveform rendering.
#[derive(Debug, Clone)]
pub struct WaveformSummary {
    pub sample_rate: u32,
    pub hop_seconds: f64,
    pub low_peaks: Vec<f32>,
    pub high_peaks: Vec<f32>,
}

impl WaveformSummary {
    pub fn from_decoded(audio: &DecodedAudio) -> Self {
        let sr = audio.sample_rate.max(1) as usize;
        let hop = (sr / 100).max(1); // 10 ms bins
        let n = audio.len_frames();
        let bins = n.div_ceil(hop);
        let mut low_peaks = Vec::with_capacity(bins);
        let mut high_peaks = Vec::with_capacity(bins);

        // Simple 1-pole lowpass (~250 Hz) split to color bass vs highs accurately
        let alpha = (2.0 * std::f32::consts::PI * 250.0 / sr as f32).clamp(0.001, 0.99);
        let mut lp = 0.0_f32;

        for bin in 0..bins {
            let start = bin * hop;
            let end = (start + hop).min(n);
            let mut max_low = 0.0_f32;
            let mut max_high = 0.0_f32;
            for i in start..end {
                let m = 0.5 * (audio.left[i] + audio.right[i]);
                lp += alpha * (m - lp);
                let hi = m - lp;
                max_low = max_low.max(lp.abs());
                max_high = max_high.max(hi.abs());
            }
            low_peaks.push(max_low.clamp(0.0, 1.0));
            high_peaks.push(max_high.clamp(0.0, 1.0));
        }

        Self {
            sample_rate: audio.sample_rate,
            hop_seconds: hop as f64 / sr as f64,
            low_peaks,
            high_peaks,
        }
    }

    pub fn sample_at(&self, time_sec: f64) -> (f32, f32) {
        if time_sec < 0.0 || self.low_peaks.is_empty() || self.hop_seconds <= 0.0 {
            return (0.0, 0.0);
        }
        let idx = (time_sec / self.hop_seconds) as usize;
        if idx >= self.low_peaks.len() {
            return (0.0, 0.0);
        }
        (self.low_peaks[idx], self.high_peaks[idx])
    }
}

/// Real-time stereo 3-band DJ kill EQ (Low < 250 Hz, Mid 250-2500 Hz, High > 2500 Hz).
/// Cascaded 2-stage one-pole crossover gives smooth DJ musical roll-off and full 0.0 kill.
#[derive(Debug, Clone)]
pub struct ThreeBandEq {
    a_low: f32,
    a_high: f32,
    lp_l1: f32,
    lp_l2: f32,
    hp_l1: f32,
    hp_l2: f32,
    lp_r1: f32,
    lp_r2: f32,
    hp_r1: f32,
    hp_r2: f32,
}

impl ThreeBandEq {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000) as f32;
        let a_low = 1.0 - (-2.0 * std::f32::consts::PI * 250.0 / sr).exp();
        let a_high = 1.0 - (-2.0 * std::f32::consts::PI * 2500.0 / sr).exp();
        Self {
            a_low: a_low.clamp(0.0005, 0.99),
            a_high: a_high.clamp(0.0005, 0.99),
            lp_l1: 0.0,
            lp_l2: 0.0,
            hp_l1: 0.0,
            hp_l2: 0.0,
            lp_r1: 0.0,
            lp_r2: 0.0,
            hp_r1: 0.0,
            hp_r2: 0.0,
        }
    }

    pub fn process_stereo(
        &mut self,
        l: f32,
        r: f32,
        gain_low: f32,
        gain_mid: f32,
        gain_high: f32,
    ) -> (f32, f32) {
        // Left channel
        self.lp_l1 += self.a_low * (l - self.lp_l1);
        self.lp_l2 += self.a_low * (self.lp_l1 - self.lp_l2);
        let low_l = self.lp_l2;

        self.hp_l1 += self.a_high * (l - self.hp_l1);
        self.hp_l2 += self.a_high * (self.hp_l1 - self.hp_l2);
        let high_l = l - self.hp_l2;
        let mid_l = l - low_l - high_l;

        // Right channel
        self.lp_r1 += self.a_low * (r - self.lp_r1);
        self.lp_r2 += self.a_low * (self.lp_r1 - self.lp_r2);
        let low_r = self.lp_r2;

        self.hp_r1 += self.a_high * (r - self.hp_r1);
        self.hp_r2 += self.a_high * (self.hp_r1 - self.hp_r2);
        let high_r = r - self.hp_r2;
        let mid_r = r - low_r - high_r;

        let out_l = (low_l * gain_low + mid_l * gain_mid + high_l * gain_high).clamp(-2.0, 2.0);
        let out_r = (low_r * gain_low + mid_r * gain_mid + high_r * gain_high).clamp(-2.0, 2.0);
        (out_l, out_r)
    }
}

pub const STEMS_DIR_NAME: &str = ".djcraft-stems";

/// Pre-separated 4-stem stereo buffers co-located beside an audio file in `.djcraft-stems/<stem>/`.
#[derive(Debug, Clone)]
pub struct DecodedStems {
    pub vocals: DecodedAudio,
    pub drums: DecodedAudio,
    pub bass: DecodedAudio,
    pub other: DecodedAudio,
}

impl DecodedStems {
    pub fn sample_rate(&self) -> u32 {
        self.vocals.sample_rate
    }
}

/// Canonical transparent sidecar stem directory: `<parent>/.djcraft-stems/<track_stem>/`.
pub fn stem_dir_for_audio(audio_path: &Path) -> PathBuf {
    let parent = audio_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = audio_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("track");
    parent.join(STEMS_DIR_NAME).join(stem)
}

/// Returns true when all 4 transparent sidecar stem WAVs exist for `audio_path`.
pub fn has_sidecar_stems(audio_path: &Path) -> bool {
    let dir = stem_dir_for_audio(audio_path);
    ["vocals.wav", "drums.wav", "bass.wav", "other.wav"]
        .iter()
        .all(|name| dir.join(name).is_file())
}

/// Load all 4 sidecar stem WAVs from `<parent>/.djcraft-stems/<stem>/` if present.
pub fn load_sidecar_stems(audio_path: &Path) -> Option<DecodedStems> {
    if !has_sidecar_stems(audio_path) {
        return None;
    }
    let dir = stem_dir_for_audio(audio_path);
    let vocals = decode_audio_file(&dir.join("vocals.wav")).ok()?;
    let drums = decode_audio_file(&dir.join("drums.wav")).ok()?;
    let bass = decode_audio_file(&dir.join("bass.wav")).ok()?;
    let other = decode_audio_file(&dir.join("other.wav")).ok()?;
    Some(DecodedStems {
        vocals,
        drums,
        bass,
        other,
    })
}

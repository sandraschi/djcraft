// Ported from ClubCraft (github.com/beneaze/ClubCraft @ 9afec47, crates/core/src/stems.rs).
// Copyright (c) 2026 The ClubCraft contributors. MIT OR Apache-2.0.
// Changes for DJCraft are listed in docs/CLUBCRAFT_PORT_LOG.md.
//! Shared, progressively filled stem buffer.
//!
//! The separator fills chunks (in any order) while the live engine reads them lock-free from the
//! audio thread. A chunk that is not ready yet simply reads as "no stems here", and the engine
//! falls back to the original audio for that region.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// The four stems produced by the separator, in the model's output order
/// (ported from ClubCraft `crates/core/src/analysis.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StemKind {
    Drums = 0,
    Bass = 1,
    /// "Melody" in the UI (Demucs calls it `other`): synths, keys, guitars, fx.
    Other = 2,
    Vocals = 3,
}

impl StemKind {
    pub const ALL: [StemKind; 4] = [
        StemKind::Drums,
        StemKind::Bass,
        StemKind::Other,
        StemKind::Vocals,
    ];
    pub const COUNT: usize = 4;

    pub fn label(self) -> &'static str {
        match self {
            StemKind::Drums => "Drums",
            StemKind::Bass => "Bass",
            StemKind::Other => "Melody",
            StemKind::Vocals => "Vocals",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// Sample rate of every [`StemTrack`] (the rate the separation models run at).
pub const STEM_SAMPLE_RATE: u32 = 44_100;

/// Frames per chunk (2^16 = 1.49 s at 44.1 kHz).
pub const STEM_CHUNK_FRAMES: usize = 1 << 16;
/// Interleaved channels per frame: 4 stems x stereo, in [`crate::StemKind`] order:
/// drums L, drums R, bass L, bass R, other L, other R, vocals L, vocals R.
pub const STEM_CHANNELS: usize = 8;

/// One chunk of separated stems, stored as 16-bit samples with one scale per stem (half the
/// memory of f32; the quantisation floor is 96 dB below each stem's peak in the chunk).
pub struct StemChunk {
    /// `frames * STEM_CHANNELS` interleaved samples; sample = `data[i] as f32 * scale[stem]`.
    pub data: Vec<i16>,
    pub scale: [f32; 4],
}

impl StemChunk {
    pub fn from_f32(data: &[f32]) -> StemChunk {
        let mut peak = [0f32; 4];
        for f in data.as_chunks::<STEM_CHANNELS>().0 {
            for (c, v) in f.iter().enumerate() {
                peak[c / 2] = peak[c / 2].max(v.abs());
            }
        }
        let scale = peak.map(|p| if p > 0.0 { p / 32767.0 } else { 1.0 / 32767.0 });
        let inv = scale.map(|s| 1.0 / s);
        let data = data
            .iter()
            .enumerate()
            .map(|(i, v)| {
                (v * inv[(i % STEM_CHANNELS) / 2])
                    .round()
                    .clamp(-32767.0, 32767.0) as i16
            })
            .collect();
        StemChunk { data, scale }
    }

    pub fn frames(&self) -> usize {
        self.data.len() / STEM_CHANNELS
    }

    /// The 8 interleaved samples of frame `j` within the chunk.
    #[inline]
    pub fn frame(&self, j: usize) -> [f32; STEM_CHANNELS] {
        let o = j * STEM_CHANNELS;
        let d = &self.data[o..o + STEM_CHANNELS];
        std::array::from_fn(|c| d[c] as f32 * self.scale[c / 2])
    }

    #[inline]
    pub fn sample(&self, i: usize) -> f32 {
        self.data[i] as f32 * self.scale[(i % STEM_CHANNELS) / 2]
    }

    /// Largest absolute sample value.
    pub fn peak(&self) -> f32 {
        self.scale.iter().fold(0f32, |m, s| m.max(s * 32767.0))
    }

    pub fn to_f32(&self) -> Vec<f32> {
        (0..self.data.len()).map(|i| self.sample(i)).collect()
    }
}

pub struct StemTrack {
    pub frames: usize,
    /// Name of the model that produced the stems (e.g. "htdemucs_ft").
    pub model: String,
    chunks: Vec<OnceLock<StemChunk>>,
    ready: AtomicUsize,
}

impl StemTrack {
    pub fn new(frames: usize, model: impl Into<String>) -> Self {
        let n = frames.div_ceil(STEM_CHUNK_FRAMES);
        StemTrack {
            frames,
            model: model.into(),
            chunks: (0..n).map(|_| OnceLock::new()).collect(),
            ready: AtomicUsize::new(0),
        }
    }

    pub fn num_chunks(&self) -> usize {
        self.chunks.len()
    }

    /// Frames covered by chunk `i` (the last chunk may be shorter).
    pub fn chunk_frames(&self, i: usize) -> usize {
        let start = i * STEM_CHUNK_FRAMES;
        (self.frames - start.min(self.frames)).min(STEM_CHUNK_FRAMES)
    }

    /// Publish chunk `i`. `data.len()` must equal `chunk_frames(i) * STEM_CHANNELS`.
    /// Publishing an already published chunk is a no-op.
    pub fn set_chunk(&self, i: usize, data: Vec<f32>) {
        debug_assert_eq!(data.len(), self.chunk_frames(i) * STEM_CHANNELS);
        if self.chunks[i].set(StemChunk::from_f32(&data)).is_ok() {
            self.ready.fetch_add(1, Ordering::Release);
        }
    }

    /// Publish an already quantised chunk.
    pub fn set_stem_chunk(&self, i: usize, chunk: StemChunk) {
        debug_assert_eq!(chunk.frames(), self.chunk_frames(i));
        if self.chunks[i].set(chunk).is_ok() {
            self.ready.fetch_add(1, Ordering::Release);
        }
    }

    pub fn chunk(&self, i: usize) -> Option<&StemChunk> {
        self.chunks.get(i)?.get()
    }

    pub fn is_chunk_ready(&self, i: usize) -> bool {
        self.chunks.get(i).is_some_and(|c| c.get().is_some())
    }

    pub fn ready_fraction(&self) -> f32 {
        if self.chunks.is_empty() {
            return 1.0;
        }
        self.ready.load(Ordering::Acquire) as f32 / self.chunks.len() as f32
    }

    pub fn is_complete(&self) -> bool {
        self.ready.load(Ordering::Acquire) == self.chunks.len()
    }

    /// The 8 interleaved stem samples at `frame`, or `None` if that region is not separated yet.
    #[inline]
    pub fn frame(&self, frame: usize) -> Option<[f32; STEM_CHANNELS]> {
        let c = self.chunk(frame / STEM_CHUNK_FRAMES)?;
        let j = frame % STEM_CHUNK_FRAMES;
        (j < c.frames()).then(|| c.frame(j))
    }

    /// Approximate memory held by the published chunks (bytes).
    pub fn memory_bytes(&self) -> usize {
        (0..self.chunks.len())
            .filter_map(|i| self.chunk(i))
            .map(|c| c.data.len() * 2)
            .sum()
    }

    /// Build a complete track from planar stems: `stems[s] = (left, right)` in StemKind order.
    pub fn from_planar(model: impl Into<String>, stems: &[(Vec<f32>, Vec<f32>); 4]) -> Self {
        let frames = stems[0].0.len();
        let t = StemTrack::new(frames, model);
        for i in 0..t.num_chunks() {
            let start = i * STEM_CHUNK_FRAMES;
            let n = t.chunk_frames(i);
            let mut data = Vec::with_capacity(n * STEM_CHANNELS);
            for f in start..start + n {
                for (l, r) in stems.iter() {
                    data.push(l[f]);
                    data.push(r[f]);
                }
            }
            t.set_chunk(i, data);
        }
        t
    }
}

impl StemTrack {
    /// Convert legacy `.djcraft-stems/` sidecar WAVs (vocals, drums, bass, other) into a complete
    /// track. Demucs writes 44.1 kHz; any other rate is linearly resampled to [`STEM_SAMPLE_RATE`].
    pub fn from_decoded_stems(stems: &crate::dsp::DecodedStems) -> StemTrack {
        let to_rate = |a: &crate::dsp::DecodedAudio| -> (Vec<f32>, Vec<f32>) {
            let n = a.left.len().min(a.right.len());
            if a.sample_rate == STEM_SAMPLE_RATE || a.sample_rate == 0 {
                return (a.left[..n].to_vec(), a.right[..n].to_vec());
            }
            let ratio = a.sample_rate as f64 / STEM_SAMPLE_RATE as f64;
            let out_n = ((n as f64) / ratio).floor() as usize;
            let pick = |ch: &[f32], j: usize| -> f32 {
                let x = j as f64 * ratio;
                let i0 = x.floor() as usize;
                let t = (x - i0 as f64) as f32;
                let a0 = ch.get(i0).copied().unwrap_or(0.0);
                let a1 = ch.get(i0 + 1).copied().unwrap_or(a0);
                a0 + (a1 - a0) * t
            };
            (
                (0..out_n).map(|j| pick(&a.left[..n], j)).collect(),
                (0..out_n).map(|j| pick(&a.right[..n], j)).collect(),
            )
        };
        // StemKind order: drums, bass, other, vocals.
        let mut planar = [
            to_rate(&stems.drums),
            to_rate(&stems.bass),
            to_rate(&stems.other),
            to_rate(&stems.vocals),
        ];
        let frames = planar.iter().map(|(l, _)| l.len()).min().unwrap_or(0);
        for (l, r) in planar.iter_mut() {
            l.truncate(frames);
            r.truncate(frames);
        }
        StemTrack::from_planar("legacy-sidecar", &planar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progressive() {
        let n = STEM_CHUNK_FRAMES + 10;
        let t = StemTrack::new(n, "x");
        assert_eq!(t.num_chunks(), 2);
        assert!(t.frame(5).is_none());
        t.set_chunk(1, vec![0.5; 10 * STEM_CHANNELS]);
        assert!((t.frame(STEM_CHUNK_FRAMES + 3).unwrap()[7] - 0.5).abs() < 1e-4);
        assert!(t.frame(0).is_none());
        assert_eq!(t.ready_fraction(), 0.5);
    }
}

#[cfg(test)]
mod quant_tests {
    use super::*;
    #[test]
    fn per_stem_scale_keeps_quiet_stems() {
        let n = 1000;
        let mut d = vec![0f32; n * STEM_CHANNELS];
        for f in 0..n {
            let x = (f as f32 * 0.05).sin();
            d[f * 8] = 1.7 * x; // loud drums above 0 dBFS
            d[f * 8 + 6] = 0.001 * x; // very quiet vocals
        }
        let c = StemChunk::from_f32(&d);
        for f in 0..n {
            let fr = c.frame(f);
            assert!((fr[0] - d[f * 8]).abs() < 1.7 / 32767.0);
            assert!((fr[6] - d[f * 8 + 6]).abs() < 0.001 / 32767.0 + 1e-9);
        }
    }
}

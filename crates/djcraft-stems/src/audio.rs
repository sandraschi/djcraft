//! DJCraft side of the ClubCraft `clubcraft-audio` items the ported separator uses.
//! `AudioBuffer` mirrors ClubCraft's type (MIT OR Apache-2.0, the ClubCraft contributors);
//! decoding goes through DJCraft's own symphonia decoder.

use anyhow::{anyhow, Result};
use std::path::Path;

pub use crate::resample;

/// Planar stereo audio. Both channels always have the same length.
#[derive(Clone, Debug, Default)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

impl AudioBuffer {
    pub fn new(sample_rate: u32, left: Vec<f32>, right: Vec<f32>) -> Self {
        debug_assert_eq!(left.len(), right.len());
        AudioBuffer {
            sample_rate,
            left,
            right,
        }
    }

    pub fn frames(&self) -> usize {
        self.left.len()
    }

    pub fn duration(&self) -> f64 {
        self.frames() as f64 / self.sample_rate.max(1) as f64
    }

    /// Mono downmix (L+R)/2.
    pub fn mono(&self) -> Vec<f32> {
        self.left
            .iter()
            .zip(&self.right)
            .map(|(l, r)| 0.5 * (l + r))
            .collect()
    }

    /// Interleaved L/R samples.
    pub fn interleaved(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.frames() * 2);
        for (l, r) in self.left.iter().zip(&self.right) {
            out.push(*l);
            out.push(*r);
        }
        out
    }

    pub fn from_interleaved(sample_rate: u32, data: &[f32]) -> Self {
        let n = data.len() / 2;
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);
        for f in data.as_chunks::<2>().0 {
            left.push(f[0]);
            right.push(f[1]);
        }
        AudioBuffer {
            sample_rate,
            left,
            right,
        }
    }

    /// Take over a DJCraft decoded track (no copy of the sample data).
    pub fn from_decoded(d: djcraft_audio::dsp::DecodedAudio) -> Self {
        let n = d.left.len().min(d.right.len());
        let (mut left, mut right) = (d.left, d.right);
        left.truncate(n);
        right.truncate(n);
        AudioBuffer {
            sample_rate: d.sample_rate,
            left,
            right,
        }
    }
}

/// Decode with DJCraft's decoder, then resample to `target_rate`.
pub fn decode_file(path: &Path, target_rate: u32) -> Result<AudioBuffer> {
    let decoded = djcraft_audio::dsp::decode_audio_file(path).map_err(|e| anyhow!(e))?;
    let buf = AudioBuffer::from_decoded(decoded);
    if buf.sample_rate == 0 {
        return Err(anyhow!("unknown sample rate: {}", path.display()));
    }
    if buf.sample_rate == target_rate {
        Ok(buf)
    } else {
        Ok(resample::resample(&buf, target_rate))
    }
}

#![forbid(unsafe_code)]
//! Live lossless WAV / FLAC master mix recorder.
//!
//! Designed for zero-blocking audio callbacks: the real-time WASAPI thread pushes
//! small stereo `(f32, f32)` chunks over a bounded channel via `try_send`, while a
//! dedicated writer thread flushes 16-bit PCM WAV frames to disk and optionally
//! transcodes to lossless FLAC via `ffmpeg` on stop.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingFormat {
    Wav,
    Flac,
}

impl RecordingFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Flac => "flac",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Wav => "WAV (16-bit PCM)",
            Self::Flac => "FLAC (Lossless)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStopReport {
    pub output_path: PathBuf,
    pub format: RecordingFormat,
    pub sample_rate: u32,
    pub frames_written: u64,
    pub duration_seconds: f64,
    pub note: String,
}

/// Non-blocking tap held inside the audio callback state.
pub struct RecordingTap {
    tx: SyncSender<Vec<(f32, f32)>>,
    pending: Vec<(f32, f32)>,
    frames_sent: Arc<AtomicU64>,
}

impl RecordingTap {
    pub fn push_frame(&mut self, l: f32, r: f32) {
        self.pending.push((l, r));
        if self.pending.len() >= 1024 {
            let chunk = std::mem::replace(&mut self.pending, Vec::with_capacity(1024));
            let count = chunk.len() as u64;
            if self.tx.try_send(chunk).is_ok() {
                self.frames_sent.fetch_add(count, Ordering::Relaxed);
            }
        }
    }

    pub fn flush(&mut self) {
        if !self.pending.is_empty() {
            let chunk = std::mem::take(&mut self.pending);
            let count = chunk.len() as u64;
            if self.tx.try_send(chunk).is_ok() {
                self.frames_sent.fetch_add(count, Ordering::Relaxed);
            }
        }
    }
}

/// Active recording session handle owned by `AudioOutput`.
pub struct ActiveRecording {
    pub output_path: PathBuf,
    pub format: RecordingFormat,
    pub sample_rate: u32,
    pub frames_written: Arc<AtomicU64>,
    worker: Option<JoinHandle<Result<RecordingStopReport, String>>>,
}

impl ActiveRecording {
    pub fn start(
        output_path: PathBuf,
        sample_rate: u32,
        format: RecordingFormat,
    ) -> Result<(Self, RecordingTap), String> {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create recording directory: {e}"))?;
        }

        let wav_path = match format {
            RecordingFormat::Wav => output_path.clone(),
            RecordingFormat::Flac => output_path.with_extension("tmp.wav"),
        };

        let file = File::create(&wav_path).map_err(|e| {
            format!(
                "Failed to create recording file {}: {e}",
                wav_path.display()
            )
        })?;
        let mut writer = BufWriter::with_capacity(64 * 1024, file);
        write_wav_header(&mut writer, sample_rate, 0)
            .map_err(|e| format!("Failed to write WAV header: {e}"))?;

        let (tx, rx) = mpsc::sync_channel::<Vec<(f32, f32)>>(64);
        let frames_written = Arc::new(AtomicU64::new(0));
        let frames_counter = Arc::clone(&frames_written);
        let final_path = output_path.clone();

        let worker = thread::spawn(move || -> Result<RecordingStopReport, String> {
            let mut total_frames: u64 = 0;
            while let Ok(chunk) = rx.recv() {
                for (l, r) in chunk {
                    let li = (l.clamp(-1.0, 1.0) * 32767.0) as i16;
                    let ri = (r.clamp(-1.0, 1.0) * 32767.0) as i16;
                    writer
                        .write_all(&li.to_le_bytes())
                        .map_err(|e| format!("WAV write error: {e}"))?;
                    writer
                        .write_all(&ri.to_le_bytes())
                        .map_err(|e| format!("WAV write error: {e}"))?;
                    total_frames += 1;
                }
            }
            writer
                .flush()
                .map_err(|e| format!("WAV flush error: {e}"))?;
            let mut file = writer
                .into_inner()
                .map_err(|e| format!("WAV finalize error: {e}"))?;
            let data_bytes = (total_frames * 4).min(u32::MAX as u64) as u32;
            patch_wav_header(&mut file, data_bytes)
                .map_err(|e| format!("WAV header patch error: {e}"))?;
            drop(file);

            frames_counter.store(total_frames, Ordering::Relaxed);
            let duration_seconds = total_frames as f64 / sample_rate.max(1) as f64;

            match format {
                RecordingFormat::Wav => Ok(RecordingStopReport {
                    output_path: final_path,
                    format,
                    sample_rate,
                    frames_written: total_frames,
                    duration_seconds,
                    note: "Saved lossless 16-bit PCM WAV".to_string(),
                }),
                RecordingFormat::Flac => {
                    let status = Command::new("ffmpeg")
                        .args(["-y", "-loglevel", "error", "-i"])
                        .arg(&wav_path)
                        .args(["-c:a", "flac", "-compression_level", "5"])
                        .arg(&final_path)
                        .status();
                    match status {
                        Ok(st) if st.success() && final_path.is_file() => {
                            let _ = std::fs::remove_file(&wav_path);
                            Ok(RecordingStopReport {
                                output_path: final_path,
                                format: RecordingFormat::Flac,
                                sample_rate,
                                frames_written: total_frames,
                                duration_seconds,
                                note: "Saved lossless FLAC via ffmpeg".to_string(),
                            })
                        }
                        _ => {
                            let fallback_wav = final_path.with_extension("wav");
                            let _ = std::fs::rename(&wav_path, &fallback_wav);
                            Ok(RecordingStopReport {
                                output_path: fallback_wav,
                                format: RecordingFormat::Wav,
                                sample_rate,
                                frames_written: total_frames,
                                duration_seconds,
                                note: "ffmpeg unavailable; preserved lossless WAV instead"
                                    .to_string(),
                            })
                        }
                    }
                }
            }
        });

        let tap = RecordingTap {
            tx,
            pending: Vec::with_capacity(1024),
            frames_sent: Arc::clone(&frames_written),
        };

        Ok((
            Self {
                output_path,
                format,
                sample_rate,
                frames_written,
                worker: Some(worker),
            },
            tap,
        ))
    }

    pub fn recorded_seconds(&self) -> f64 {
        self.frames_written.load(Ordering::Relaxed) as f64 / self.sample_rate.max(1) as f64
    }

    /// Wait for the background writer to finalize the file (call AFTER dropping the `RecordingTap`).
    pub fn finish(mut self) -> Result<RecordingStopReport, String> {
        if let Some(handle) = self.worker.take() {
            handle
                .join()
                .map_err(|_| "Recording writer thread panicked".to_string())?
        } else {
            Err("Recording already finalized".to_string())
        }
    }
}

/// Write a standard 44-byte stereo 16-bit PCM RIFF/WAVE header.
pub fn write_wav_header<W: Write>(
    w: &mut W,
    sample_rate: u32,
    data_bytes: u32,
) -> std::io::Result<()> {
    let channels: u16 = 2;
    let bits_per_sample: u16 = 16;
    let byte_rate: u32 = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align: u16 = channels * (bits_per_sample / 8);
    let riff_size: u32 = 36_u32.saturating_add(data_bytes);

    w.write_all(b"RIFF")?;
    w.write_all(&riff_size.to_le_bytes())?;
    w.write_all(b"WAVE")?;
    w.write_all(b"fmt ")?;
    w.write_all(&16_u32.to_le_bytes())?;
    w.write_all(&1_u16.to_le_bytes())?; // PCM
    w.write_all(&channels.to_le_bytes())?;
    w.write_all(&sample_rate.to_le_bytes())?;
    w.write_all(&byte_rate.to_le_bytes())?;
    w.write_all(&block_align.to_le_bytes())?;
    w.write_all(&bits_per_sample.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data_bytes.to_le_bytes())?;
    Ok(())
}

/// Patch RIFF and `data` chunk sizes in place after streaming PCM samples.
pub fn patch_wav_header<W: Write + Seek>(w: &mut W, data_bytes: u32) -> std::io::Result<()> {
    let riff_size: u32 = 36_u32.saturating_add(data_bytes);
    w.seek(SeekFrom::Start(4))?;
    w.write_all(&riff_size.to_le_bytes())?;
    w.seek(SeekFrom::Start(40))?;
    w.write_all(&data_bytes.to_le_bytes())?;
    w.flush()?;
    Ok(())
}

/// Synchronous helper to write stereo `f32` frames directly to a lossless 16-bit PCM WAV file.
pub fn write_wav_frames(
    path: &Path,
    sample_rate: u32,
    frames: &[(f32, f32)],
) -> Result<usize, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {e}"))?;
    }
    let file = File::create(path).map_err(|e| format!("Failed to create WAV: {e}"))?;
    let mut writer = BufWriter::new(file);
    let data_bytes = (frames.len() * 4).min(u32::MAX as usize) as u32;
    write_wav_header(&mut writer, sample_rate, data_bytes)
        .map_err(|e| format!("WAV header error: {e}"))?;
    for &(l, r) in frames {
        let li = (l.clamp(-1.0, 1.0) * 32767.0) as i16;
        let ri = (r.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer
            .write_all(&li.to_le_bytes())
            .map_err(|e| format!("WAV write error: {e}"))?;
        writer
            .write_all(&ri.to_le_bytes())
            .map_err(|e| format!("WAV write error: {e}"))?;
    }
    writer
        .flush()
        .map_err(|e| format!("WAV flush error: {e}"))?;
    Ok(frames.len())
}

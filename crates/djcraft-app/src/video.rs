#![forbid(unsafe_code)]
//! Native pure-Rust Video Mixer for DJCraft.
//!
//! Decodes per-deck video files (.mp4, .mkv, .mov, .webm) via an external `ffmpeg`
//! raw RGB24 pipe (zero GPL link, zero C++ buildenv), falls back to an audio-reactive
//! procedural visual generator when no file is loaded, and composites Deck A + Deck B
//! across the Master Crossfader with Brightness/Contrast/Saturation into an egui texture.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use egui::ColorImage;

pub const VIDEO_W: usize = 320;
pub const VIDEO_H: usize = 180;
const FRAME_BYTES: usize = VIDEO_W * VIDEO_H * 3;

pub struct VideoDeckPlayer {
    latest_rgb: Arc<Mutex<Vec<u8>>>,
    playing: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    pub source_label: String,
    pub has_file: bool,
}

impl Default for VideoDeckPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoDeckPlayer {
    pub fn new() -> Self {
        Self {
            latest_rgb: Arc::new(Mutex::new(vec![0u8; FRAME_BYTES])),
            playing: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            source_label: "Procedural Reactive Visuals".to_string(),
            has_file: false,
        }
    }

    pub fn set_playing(&self, is_playing: bool) {
        self.playing.store(is_playing, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        self.has_file = false;
        self.source_label = "Procedural Reactive Visuals".to_string();
    }

    /// Spawn a background `ffmpeg` decoder streaming 320x180 RGB24 frames at 24 fps (looping).
    pub fn load_file(&mut self, path: &Path) -> Result<(), String> {
        self.stop_flag.store(true, Ordering::Relaxed);
        let new_stop = Arc::new(AtomicBool::new(false));
        self.stop_flag = Arc::clone(&new_stop);

        let mut child = Command::new("ffmpeg")
            .args([
                "-stream_loop",
                "-1",
                "-re",
                "-i",
                &path.to_string_lossy(),
                "-vf",
                "scale=320:180:force_original_aspect_ratio=decrease,pad=320:180:(ow-iw)/2:(oh-ih)/2",
                "-r",
                "24",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-loglevel",
                "error",
                "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to launch ffmpeg (is ffmpeg on PATH?): {e}"))?;

        let Some(mut stdout) = child.stdout.take() else {
            let _ = child.kill();
            return Err("ffmpeg stdout pipe unavailable".to_string());
        };

        let latest = Arc::clone(&self.latest_rgb);
        let playing = Arc::clone(&self.playing);
        self.has_file = true;
        self.source_label = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("video")
            .to_string();

        thread::spawn(move || {
            let mut buf = vec![0u8; FRAME_BYTES];
            while !new_stop.load(Ordering::Relaxed) {
                if !playing.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(30));
                    continue;
                }
                if stdout.read_exact(&mut buf).is_err() {
                    break;
                }
                if let Ok(mut guard) = latest.lock() {
                    guard.copy_from_slice(&buf);
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        });

        Ok(())
    }

    pub fn snapshot_rgb(&self, fallback_phase: f32, accent_rgb: [f32; 3]) -> Vec<u8> {
        if self.has_file {
            if let Ok(guard) = self.latest_rgb.lock() {
                return guard.clone();
            }
        }
        render_procedural_rgb(fallback_phase, accent_rgb)
    }
}

impl Drop for VideoDeckPlayer {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

/// Find a companion video file next to an audio file (same basename, .mp4/.mkv/.mov/.webm).
pub fn find_companion_video(audio_path: &Path) -> Option<PathBuf> {
    for ext in ["mp4", "mkv", "mov", "webm"] {
        let candidate = audio_path.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Zero-dependency audio-reactive plasma/grid fallback so the video mixer works immediately.
fn render_procedural_rgb(phase: f32, accent: [f32; 3]) -> Vec<u8> {
    let mut rgb = vec![0u8; FRAME_BYTES];
    let cx = VIDEO_W as f32 * 0.5;
    let cy = VIDEO_H as f32 * 0.5;
    for y in 0..VIDEO_H {
        let dy = (y as f32 - cy) / cy;
        for x in 0..VIDEO_W {
            let dx = (x as f32 - cx) / cx;
            let dist = (dx * dx + dy * dy).sqrt();
            let rings = ((dist * 10.0 - phase * 4.0).sin() * 0.5 + 0.5).powf(2.0);
            let wave = ((dx * 4.0 + phase * 2.0).sin() * (dy * 4.0 - phase * 1.5).cos()) * 0.5 + 0.5;
            let energy = (rings * 0.65 + wave * 0.35).clamp(0.0, 1.0);
            let idx = (y * VIDEO_W + x) * 3;
            rgb[idx] = ((12.0 + accent[0] * energy * 240.0).clamp(0.0, 255.0)) as u8;
            rgb[idx + 1] = ((16.0 + accent[1] * energy * 240.0).clamp(0.0, 255.0)) as u8;
            rgb[idx + 2] = ((24.0 + accent[2] * energy * 240.0).clamp(0.0, 255.0)) as u8;
        }
    }
    rgb
}

/// Crossfader-blend Deck A and Deck B RGB24 frames with Brightness, Contrast, and Saturation.
pub fn composite_video_frame(
    rgb_a: &[u8],
    rgb_b: &[u8],
    crossfader: f64,
    brightness: f32,
    contrast: f32,
    saturation: f32,
) -> ColorImage {
    let t = ((crossfader + 1.0) * 0.5).clamp(0.0, 1.0) as f32;
    let inv_t = 1.0 - t;
    let b_shift = (brightness - 0.5) * 0.9; // 0.5 = neutral
    let c_scale = 0.5 + contrast; // 0.5 = 1.0 neutral
    let s_scale = saturation * 2.0; // 0.5 = 1.0 neutral

    let mut rgba = vec![255u8; VIDEO_W * VIDEO_H * 4];
    for px in 0..(VIDEO_W * VIDEO_H) {
        let i3 = px * 3;
        let i4 = px * 4;
        let mut r = (rgb_a[i3] as f32 * inv_t + rgb_b[i3] as f32 * t) / 255.0;
        let mut g = (rgb_a[i3 + 1] as f32 * inv_t + rgb_b[i3 + 1] as f32 * t) / 255.0;
        let mut b = (rgb_a[i3 + 2] as f32 * inv_t + rgb_b[i3 + 2] as f32 * t) / 255.0;

        // Contrast around 0.5 + Brightness
        r = ((r - 0.5) * c_scale + 0.5 + b_shift).clamp(0.0, 1.0);
        g = ((g - 0.5) * c_scale + 0.5 + b_shift).clamp(0.0, 1.0);
        b = ((b - 0.5) * c_scale + 0.5 + b_shift).clamp(0.0, 1.0);

        // Saturation around Rec.709 luma
        let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        r = (luma + (r - luma) * s_scale).clamp(0.0, 1.0);
        g = (luma + (g - luma) * s_scale).clamp(0.0, 1.0);
        b = (luma + (b - luma) * s_scale).clamp(0.0, 1.0);

        rgba[i4] = (r * 255.0) as u8;
        rgba[i4 + 1] = (g * 255.0) as u8;
        rgba[i4 + 2] = (b * 255.0) as u8;
    }

    ColorImage::from_rgba_unmultiplied([VIDEO_W, VIDEO_H], &rgba)
}

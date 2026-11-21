#![forbid(unsafe_code)]
//! Pure-Rust DVS (Digital Vinyl System) Timecode Decoder & Live Audio Input.
//!
//! Decodes quadrature stereo control-vinyl tones (1000 Hz or 2000 Hz) via
//! instantaneous complex phase-derivative (`atan2`), recovering forward/reverse
//! platter direction, varispeed pitch %, and signal level in dBFS.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DvsCarrier {
    #[default]
    Control1kHz,
    Control2kHz,
}

impl DvsCarrier {
    pub const ALL: [Self; 2] = [Self::Control1kHz, Self::Control2kHz];

    pub fn hz(self) -> f64 {
        match self {
            Self::Control1kHz => 1000.0,
            Self::Control2kHz => 2000.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Control1kHz => "1000 Hz Control Vinyl",
            Self::Control2kHz => "2000 Hz Control Vinyl",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct DvsTelemetry {
    pub locked: bool,
    pub speed_ratio: f64,
    pub pitch_pct: f64,
    pub signal_dbfs: f32,
}

#[derive(Debug, Clone)]
pub struct DvsDecoder {
    sample_rate: f64,
    carrier: DvsCarrier,
    prev_l: f64,
    prev_r: f64,
    smoothed_speed: f64,
    last_telemetry: DvsTelemetry,
}

impl DvsDecoder {
    pub fn new(sample_rate: u32, carrier: DvsCarrier) -> Self {
        Self {
            sample_rate: (sample_rate.max(8000)) as f64,
            carrier,
            prev_l: 0.0,
            prev_r: 0.0,
            smoothed_speed: 0.0,
            last_telemetry: DvsTelemetry::default(),
        }
    }

    pub fn set_carrier(&mut self, carrier: DvsCarrier) {
        self.carrier = carrier;
    }

    pub fn telemetry(&self) -> DvsTelemetry {
        self.last_telemetry
    }

    /// Process a stereo buffer of `(left, right)` samples and return updated DVS telemetry.
    pub fn process_stereo(&mut self, left: &[f32], right: &[f32]) -> DvsTelemetry {
        let n = left.len().min(right.len());
        if n == 0 {
            return self.last_telemetry;
        }

        let mut energy_sum = 0.0_f64;
        let mut angle_sum = 0.0_f64;
        let mut valid_samples = 0usize;

        for i in 0..n {
            let l = left[i] as f64;
            let r = right[i] as f64;
            energy_sum += 0.5 * (l * l + r * r);

            let re = l * self.prev_l + r * self.prev_r;
            let im = r * self.prev_l - l * self.prev_r;
            if re * re + im * im > 1e-9 {
                angle_sum += im.atan2(re);
                valid_samples += 1;
            }
            self.prev_l = l;
            self.prev_r = r;
        }

        let rms = (energy_sum / n as f64).sqrt().max(1e-6);
        let dbfs = (20.0 * rms.log10()) as f32;

        if rms < 0.008 || valid_samples == 0 {
            self.smoothed_speed *= 0.7;
            if self.smoothed_speed.abs() < 0.01 {
                self.smoothed_speed = 0.0;
            }
            self.last_telemetry = DvsTelemetry {
                locked: false,
                speed_ratio: self.smoothed_speed,
                pitch_pct: 0.0,
                signal_dbfs: dbfs,
            };
            return self.last_telemetry;
        }

        let avg_dtheta = angle_sum / valid_samples as f64;
        let freq_hz = avg_dtheta * self.sample_rate / std::f64::consts::TAU;
        let raw_speed = (freq_hz / self.carrier.hz()).clamp(-4.0, 4.0);
        self.smoothed_speed = self.smoothed_speed * 0.65 + raw_speed * 0.35;

        let pitch_pct = ((self.smoothed_speed - 1.0) * 100.0).clamp(-50.0, 50.0);
        self.last_telemetry = DvsTelemetry {
            locked: true,
            speed_ratio: self.smoothed_speed,
            pitch_pct,
            signal_dbfs: dbfs,
        };
        self.last_telemetry
    }
}

/// List available OS audio input devices (line-in / phono preamps for DVS).
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };
    devices.filter_map(|d| d.name().ok()).collect()
}

pub struct DvsInputStream {
    _stream: cpal::Stream,
    decoder: Arc<Mutex<DvsDecoder>>,
}

impl DvsInputStream {
    pub fn start(device_name: Option<&str>, carrier: DvsCarrier) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = if let Some(name) = device_name {
            host.input_devices()
                .map_err(|e| format!("Audio input enumeration failed: {e}"))?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .or_else(|| host.default_input_device())
        } else {
            host.default_input_device()
        }
        .ok_or_else(|| "No audio input device available for DVS".to_string())?;

        let cfg = device
            .default_input_config()
            .map_err(|e| format!("Cannot read input config: {e}"))?;
        let sr = cfg.sample_rate().0;
        let channels = cfg.channels().max(1) as usize;
        let decoder = Arc::new(Mutex::new(DvsDecoder::new(sr, carrier)));
        let dec_clone = Arc::clone(&decoder);

        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &cfg.into(),
                move |data: &[f32], _| {
                    feed_interleaved_f32(&dec_clone, data, channels);
                },
                |err| eprintln!("[djcraft-dvs] input stream error: {err}"),
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &cfg.into(),
                move |data: &[i16], _| {
                    let f: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                    feed_interleaved_f32(&dec_clone, &f, channels);
                },
                |err| eprintln!("[djcraft-dvs] input stream error: {err}"),
                None,
            ),
            other => {
                return Err(format!("Unsupported input sample format: {other:?}"));
            }
        }
        .map_err(|e| format!("Failed to open DVS input stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start DVS input stream: {e}"))?;

        Ok(Self {
            _stream: stream,
            decoder,
        })
    }

    pub fn telemetry(&self) -> DvsTelemetry {
        self.decoder
            .lock()
            .map(|d| d.telemetry())
            .unwrap_or_default()
    }
}

fn feed_interleaved_f32(decoder: &Arc<Mutex<DvsDecoder>>, data: &[f32], channels: usize) {
    let frames = data.len() / channels;
    if frames == 0 {
        return;
    }
    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);
    for i in 0..frames {
        let base = i * channels;
        left.push(data[base]);
        right.push(if channels > 1 {
            data[base + 1]
        } else {
            data[base]
        });
    }
    if let Ok(mut guard) = decoder.lock() {
        let _ = guard.process_stereo(&left, &right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_quadrature(
        sr: u32,
        carrier_hz: f64,
        speed: f64,
        frames: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let mut l = Vec::with_capacity(frames);
        let mut r = Vec::with_capacity(frames);
        let freq = carrier_hz * speed;
        for i in 0..frames {
            let phase = std::f64::consts::TAU * freq * (i as f64 / sr as f64);
            l.push((phase.cos() * 0.5) as f32);
            r.push((phase.sin() * 0.5) as f32);
        }
        (l, r)
    }

    #[test]
    fn test_dvs_decoder_tracks_forward_pitch_and_reverse_scratch() {
        let sr = 48_000;
        let mut dec = DvsDecoder::new(sr, DvsCarrier::Control1kHz);

        // +1.08x (+8% pitch forward)
        let (l, r) = synth_quadrature(sr, 1000.0, 1.08, 4800);
        let mut tel = DvsTelemetry::default();
        for chunk in 0..10 {
            let a = chunk * 480;
            let b = a + 480;
            tel = dec.process_stereo(&l[a..b], &r[a..b]);
        }
        assert!(tel.locked);
        assert!(
            (tel.speed_ratio - 1.08).abs() < 0.03,
            "got {}",
            tel.speed_ratio
        );
        assert!((tel.pitch_pct - 8.0).abs() < 3.0, "got {}", tel.pitch_pct);

        // -1.5x reverse scratch
        let (rl, rr) = synth_quadrature(sr, 1000.0, -1.5, 4800);
        for chunk in 0..10 {
            let a = chunk * 480;
            let b = a + 480;
            tel = dec.process_stereo(&rl[a..b], &rr[a..b]);
        }
        assert!(tel.locked);
        assert!(tel.speed_ratio < -1.35, "got {}", tel.speed_ratio);

        // Silence unlocks cleanly
        let z = vec![0.0_f32; 480];
        for _ in 0..12 {
            tel = dec.process_stereo(&z, &z);
        }
        assert!(!tel.locked);
    }
}

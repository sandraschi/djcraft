#![forbid(unsafe_code)]
//! Real-time Windows WASAPI / cross-platform dual-bus audio output via `cpal`.
//!
//! Runs lock-light audio callbacks for:
//! 1. **Master Bus**: 4-deck equal-power crossfader, per-deck volume faders, 3-band kill EQ,
//!    4-stem WAV mixing, Master Loudness + Mute, optional Split Mono Cue, and non-blocking
//!    lossless WAV/FLAC recording tap.
//! 2. **Headphone PFL Cue Bus**: optional second WASAPI output device fed from a bounded ring
//!    buffer with Pre-Fader Listen (PFL) + Cue/PGM blend + Cue Volume.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use crate::dsp::{sample_varispeed, DecodedAudio, DecodedStems, ThreeBandEq, WsolaDeckState};
use crate::fx::{BipolarFilter, FxStemScope, FxTarget, FxUnitDsp, FxUnitParams};
use crate::recorder::{ActiveRecording, RecordingFormat, RecordingStopReport, RecordingTap};
use crate::{AudioEngine, DeckId, DeckState};

const CUE_RING_CAPACITY: usize = 8192;

struct LiveMixState {
    decks: [DeckState; 4],
    decoded: [Option<Arc<DecodedAudio>>; 4],
    decoded_stems: [Option<Arc<DecodedStems>>; 4],
    wsola: [WsolaDeckState; 4],
    eqs: [ThreeBandEq; 4],
    filters: [BipolarFilter; 4],
    fx_units: [FxUnitDsp; 2],
    fx_params: [FxUnitParams; 2],
    crossfader: f64,
    master_volume: f64,
    muted: bool,
    cue_volume: f64,
    cue_mix: f64,
    split_cue: bool,
    has_headphone_stream: bool,
    play_times: [f64; 4],
    sample_counter: u64,
    recorder_tap: Option<RecordingTap>,
}

impl LiveMixState {
    fn new(sample_rate: u32) -> Self {
        Self {
            decks: [
                AudioEngine::create_deck(DeckId::DeckA),
                AudioEngine::create_deck(DeckId::DeckB),
                AudioEngine::create_deck(DeckId::DeckC),
                AudioEngine::create_deck(DeckId::DeckD),
            ],
            decoded: [None, None, None, None],
            decoded_stems: [None, None, None, None],
            wsola: [
                WsolaDeckState::new(sample_rate),
                WsolaDeckState::new(sample_rate),
                WsolaDeckState::new(sample_rate),
                WsolaDeckState::new(sample_rate),
            ],
            eqs: [
                ThreeBandEq::new(sample_rate),
                ThreeBandEq::new(sample_rate),
                ThreeBandEq::new(sample_rate),
                ThreeBandEq::new(sample_rate),
            ],
            filters: [
                BipolarFilter::new(sample_rate),
                BipolarFilter::new(sample_rate),
                BipolarFilter::new(sample_rate),
                BipolarFilter::new(sample_rate),
            ],
            fx_units: [FxUnitDsp::new(sample_rate), FxUnitDsp::new(sample_rate)],
            fx_params: [FxUnitParams::default_unit(0), FxUnitParams::default_unit(1)],
            crossfader: 0.0,
            master_volume: 0.75,
            muted: false,
            cue_volume: 0.85,
            cue_mix: 0.35,
            split_cue: false,
            has_headphone_stream: false,
            play_times: [0.0; 4],
            sample_counter: 0,
            recorder_tap: None,
        }
    }
}

pub struct AudioOutput {
    _stream: Option<Stream>,
    headphone_stream: Option<Stream>,
    state: Arc<Mutex<LiveMixState>>,
    cue_ring: Arc<Mutex<VecDeque<(f32, f32)>>>,
    active_recording: Option<ActiveRecording>,
    pub device_name: String,
    pub headphone_device_name: Option<String>,
    pub sample_rate: u32,
    pub active: bool,
    pub error: Option<String>,
}

impl AudioOutput {
    /// Enumerate available OS audio output device names (WASAPI on Windows).
    pub fn list_output_devices() -> Vec<String> {
        let host = cpal::default_host();
        let Ok(devices) = host.output_devices() else {
            return Vec::new();
        };
        devices.filter_map(|d| d.name().ok()).collect()
    }

    /// Open the default OS audio output device (WASAPI on Windows) and start streaming.
    /// Never panics: if no audio device is present, returns an inactive handle with `error`.
    pub fn start_default() -> Self {
        let state = Arc::new(Mutex::new(LiveMixState::new(44100)));
        let cue_ring = Arc::new(Mutex::new(VecDeque::with_capacity(CUE_RING_CAPACITY)));
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return Self {
                _stream: None,
                headphone_stream: None,
                state,
                cue_ring,
                active_recording: None,
                device_name: "None".to_string(),
                headphone_device_name: None,
                sample_rate: 44100,
                active: false,
                error: Some("No default audio output device found".to_string()),
            };
        };

        let device_name = device
            .name()
            .unwrap_or_else(|_| "Default Audio Device".to_string());
        let supported = match device.default_output_config() {
            Ok(cfg) => cfg,
            Err(err) => {
                return Self {
                    _stream: None,
                    headphone_stream: None,
                    state,
                    cue_ring,
                    active_recording: None,
                    device_name,
                    headphone_device_name: None,
                    sample_rate: 44100,
                    active: false,
                    error: Some(format!("Audio config error: {err}")),
                };
            }
        };

        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels() as usize;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();

        let err_fn = |err| {
            eprintln!("[djcraft-audio] master stream error: {err}");
        };

        let stream_res = match sample_format {
            SampleFormat::F32 => {
                let st = Arc::clone(&state);
                let ring = Arc::clone(&cue_ring);
                device.build_output_stream(
                    &config,
                    move |data: &mut [f32], _| {
                        fill_buffer_f32(data, channels, sample_rate, &st, &ring);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let st = Arc::clone(&state);
                let ring = Arc::clone(&cue_ring);
                device.build_output_stream(
                    &config,
                    move |data: &mut [i16], _| {
                        fill_buffer_i16(data, channels, sample_rate, &st, &ring);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let st = Arc::clone(&state);
                let ring = Arc::clone(&cue_ring);
                device.build_output_stream(
                    &config,
                    move |data: &mut [u16], _| {
                        fill_buffer_u16(data, channels, sample_rate, &st, &ring);
                    },
                    err_fn,
                    None,
                )
            }
            other => {
                return Self {
                    _stream: None,
                    headphone_stream: None,
                    state,
                    cue_ring,
                    active_recording: None,
                    device_name,
                    headphone_device_name: None,
                    sample_rate,
                    active: false,
                    error: Some(format!("Unsupported sample format: {other:?}")),
                };
            }
        };

        let stream = match stream_res {
            Ok(s) => s,
            Err(err) => {
                return Self {
                    _stream: None,
                    headphone_stream: None,
                    state,
                    cue_ring,
                    active_recording: None,
                    device_name,
                    headphone_device_name: None,
                    sample_rate,
                    active: false,
                    error: Some(format!("Failed to build audio stream: {err}")),
                };
            }
        };

        if let Err(err) = stream.play() {
            return Self {
                _stream: None,
                headphone_stream: None,
                state,
                cue_ring,
                active_recording: None,
                device_name,
                headphone_device_name: None,
                sample_rate,
                active: false,
                error: Some(format!("Failed to start audio stream: {err}")),
            };
        }

        Self {
            _stream: Some(stream),
            headphone_stream: None,
            state,
            cue_ring,
            active_recording: None,
            device_name,
            headphone_device_name: None,
            sample_rate,
            active: true,
            error: None,
        }
    }

    /// Open (or close when `None`) a dedicated secondary WASAPI output device for Headphone PFL Cue.
    pub fn set_headphone_device(&mut self, device_name: Option<&str>) -> Result<String, String> {
        self.headphone_stream = None;
        self.headphone_device_name = None;
        if let Ok(mut guard) = self.state.lock() {
            guard.has_headphone_stream = false;
        }
        if let Ok(mut ring) = self.cue_ring.lock() {
            ring.clear();
        }

        let Some(target_name) = device_name.filter(|s| !s.trim().is_empty()) else {
            return Ok("Headphone bus disabled (use Split Cue for single-device monitoring)"
                .to_string());
        };

        let host = cpal::default_host();
        let devices = host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {e}"))?;
        let Some(device) = devices
            .into_iter()
            .find(|d| d.name().ok().as_deref() == Some(target_name))
        else {
            return Err(format!("Headphone device not found: {target_name}"));
        };

        let supported = device
            .default_output_config()
            .map_err(|e| format!("Headphone config error: {e}"))?;
        let channels = supported.channels() as usize;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let ring = Arc::clone(&self.cue_ring);
        let err_fn = |err| {
            eprintln!("[djcraft-audio] headphone stream error: {err}");
        };

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data: &mut [f32], _| fill_cue_f32(data, channels, &ring),
                err_fn,
                None,
            ),
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |data: &mut [i16], _| fill_cue_i16(data, channels, &ring),
                err_fn,
                None,
            ),
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |data: &mut [u16], _| fill_cue_u16(data, channels, &ring),
                err_fn,
                None,
            ),
            other => {
                return Err(format!("Unsupported headphone sample format: {other:?}"));
            }
        }
        .map_err(|e| format!("Failed to build headphone stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start headphone stream: {e}"))?;

        self.headphone_stream = Some(stream);
        self.headphone_device_name = Some(target_name.to_string());
        if let Ok(mut guard) = self.state.lock() {
            guard.has_headphone_stream = true;
        }
        Ok(format!("Headphone PFL bus routed to {target_name}"))
    }

    /// Start live lossless Master recording (WAV or FLAC).
    pub fn start_recording(
        &mut self,
        output_path: PathBuf,
        format: RecordingFormat,
    ) -> Result<PathBuf, String> {
        if self.active_recording.is_some() {
            return Err("Recording already in progress".to_string());
        }
        let (rec, tap) = ActiveRecording::start(output_path.clone(), self.sample_rate, format)?;
        if let Ok(mut guard) = self.state.lock() {
            guard.recorder_tap = Some(tap);
        }
        self.active_recording = Some(rec);
        Ok(output_path)
    }

    /// Stop live Master recording and finalize WAV / transcode FLAC.
    pub fn stop_recording(&mut self) -> Result<RecordingStopReport, String> {
        let Some(rec) = self.active_recording.take() else {
            return Err("No active recording".to_string());
        };
        if let Ok(mut guard) = self.state.lock() {
            if let Some(mut tap) = guard.recorder_tap.take() {
                tap.flush();
            }
        }
        rec.finish()
    }

    pub fn is_recording(&self) -> bool {
        self.active_recording.is_some()
    }

    pub fn recording_seconds(&self) -> f64 {
        self.active_recording
            .as_ref()
            .map(ActiveRecording::recorded_seconds)
            .unwrap_or(0.0)
    }

    pub fn recording_path(&self) -> Option<&PathBuf> {
        self.active_recording.as_ref().map(|r| &r.output_path)
    }

    /// Attach or clear a decoded real audio file buffer for a deck (0..4).
    pub fn set_deck_audio(&self, deck_idx: usize, audio: Option<Arc<DecodedAudio>>) {
        if deck_idx >= 4 {
            return;
        }
        if let Ok(mut guard) = self.state.lock() {
            if let Some(ref a) = audio {
                guard.wsola[deck_idx] = WsolaDeckState::new(a.sample_rate);
            } else {
                guard.wsola[deck_idx].reset(0.0);
            }
            guard.decoded[deck_idx] = audio;
            guard.decoded_stems[deck_idx] = None;
            guard.play_times[deck_idx] = 0.0;
        }
    }

    /// Attach transparent 4-stem WAV buffers for real-time stem knob isolation on a deck (0..4).
    pub fn set_deck_stems(&self, deck_idx: usize, stems: Option<Arc<DecodedStems>>) {
        if deck_idx >= 4 {
            return;
        }
        if let Ok(mut guard) = self.state.lock() {
            guard.decoded_stems[deck_idx] = stems;
        }
    }

    /// Update Headphone PFL Cue volume, Cue/PGM blend, and single-device Split Cue mode.
    pub fn set_headphone_mix(&self, cue_volume: f64, cue_mix: f64, split_cue: bool) {
        if let Ok(mut guard) = self.state.try_lock() {
            guard.cue_volume = cue_volume.clamp(0.0, 1.0);
            guard.cue_mix = cue_mix.clamp(0.0, 1.0);
            guard.split_cue = split_cue;
        }
    }

    /// Push live FX 1 & FX 2 unit parameters into the real-time WASAPI mixer (non-blocking).
    pub fn set_fx_units(&self, units: [FxUnitParams; 2]) {
        if let Ok(mut guard) = self.state.try_lock() {
            guard.fx_params = [units[0].clone().clamped(), units[1].clone().clamped()];
        }
    }

    /// Push the latest mixer and deck parameters into the live audio thread (non-blocking).
    pub fn sync_state(
        &self,
        decks: [DeckState; 4],
        crossfader: f64,
        master_volume: f64,
        muted: bool,
    ) {
        if let Ok(mut guard) = self.state.try_lock() {
            for (idx, new_deck) in decks.iter().enumerate() {
                let track_changed = guard.decks[idx]
                    .loaded_track
                    .as_ref()
                    .map(|t| t.title.as_str())
                    != new_deck.loaded_track.as_ref().map(|t| t.title.as_str());
                let seek_changed = guard.decks[idx].seek_gen != new_deck.seek_gen;
                let keylock_changed = guard.decks[idx].keylock != new_deck.keylock;
                if track_changed || seek_changed {
                    let t0 = new_deck.position_seconds.max(0.0);
                    guard.play_times[idx] = t0;
                    let sr = guard.decoded[idx]
                        .as_ref()
                        .map(|a| a.sample_rate as f64)
                        .unwrap_or(self.sample_rate as f64);
                    guard.wsola[idx].reset(t0 * sr);
                } else if keylock_changed {
                    let sr = guard.decoded[idx]
                        .as_ref()
                        .map(|a| a.sample_rate as f64)
                        .unwrap_or(self.sample_rate as f64);
                    if new_deck.keylock {
                        let start_samp = guard.play_times[idx] * sr;
                        guard.wsola[idx].reset(start_samp);
                    } else {
                        guard.play_times[idx] = guard.wsola[idx].current_input_sample() / sr;
                    }
                }
            }
            guard.decks = decks;
            guard.crossfader = crossfader;
            guard.master_volume = master_volume;
            guard.muted = muted;
        }
    }
}

/// Compute one dual-bus frame: `((master_out_l, master_out_r), (cue_l, cue_r))`.
fn next_dual_bus_frame(guard: &mut LiveMixState, dt: f64) -> ((f32, f32), (f32, f32)) {
    let xf = ((guard.crossfader + 1.0) * 0.5).clamp(0.0, 1.0);
    let gain_left = (xf * std::f64::consts::FRAC_PI_2).cos() as f32;
    let gain_right = (xf * std::f64::consts::FRAC_PI_2).sin() as f32;
    let out_sr = 1.0 / dt.max(1e-9);
    let idx = guard.sample_counter;

    let mut deck_post = [(0.0_f32, 0.0_f32); 4];
    let mut deck_pfl = [(0.0_f32, 0.0_f32); 4];

    let fx_params_snap = guard.fx_params.clone();
    let deck_ids = [DeckId::DeckA, DeckId::DeckB, DeckId::DeckC, DeckId::DeckD];

    for i in 0..4 {
        let deck = &guard.decks[i];
        let scratching = deck.scratch_velocity;
        if (!deck.is_playing && scratching.is_none()) || deck.loaded_track.is_none() {
            continue;
        }
        // Holding the vinyl platter still produces silence (hand brake)
        if let Some(vel) = scratching {
            if vel.abs() < 0.02 {
                continue;
            }
        }
        let deck_bpm = deck
            .loaded_track
            .as_ref()
            .map(|t| (t.bpm * (1.0 + deck.pitch_bend_pct / 100.0)).clamp(60.0, 200.0))
            .unwrap_or(126.0);
        let rate = match scratching {
            Some(vel) => vel.clamp(-4.0, 4.0),
            None => (1.0 + deck.pitch_bend_pct / 100.0).clamp(0.5, 2.0),
        };
        let (mut pre_l, mut pre_r) = if let Some(stems) = guard.decoded_stems[i].as_ref() {
            let sample_pos = (guard.play_times[i] * stems.sample_rate() as f64).max(0.0);
            let (mut vl, mut vr) = sample_varispeed(&stems.vocals, sample_pos);
            let (mut dl, mut dr) = sample_varispeed(&stems.drums, sample_pos);
            let (bl, br) = sample_varispeed(&stems.bass, sample_pos);
            let (ol, or) = sample_varispeed(&stems.other, sample_pos);
            guard.play_times[i] = (guard.play_times[i] + dt * rate).max(0.0);

            // Stem-targeted FX (e.g. Vocal Echo Out while Drums stay dry!)
            for (u_idx, fp) in fx_params_snap.iter().enumerate() {
                if fp.enabled && fp.target.matches_deck(deck_ids[i]) {
                    match fp.stem_scope {
                        FxStemScope::VocalsOnly => {
                            let (wl, wr) =
                                guard.fx_units[u_idx].process_stereo(vl, vr, fp, deck_bpm);
                            vl = wl;
                            vr = wr;
                        }
                        FxStemScope::DrumsOnly => {
                            let (wl, wr) =
                                guard.fx_units[u_idx].process_stereo(dl, dr, fp, deck_bpm);
                            dl = wl;
                            dr = wr;
                        }
                        FxStemScope::FullDeck => {}
                    }
                }
            }

            let gv = deck.stem_vocals_gain as f32;
            let gd = deck.stem_drums_gain as f32;
            let gb = deck.stem_bass_gain as f32;
            let go = deck.stem_other_gain as f32;
            let l = vl * gv + dl * gd + bl * gb + ol * go;
            let r = vr * gv + dr * gd + br * gb + or * go;
            guard.eqs[i].process_stereo(
                l,
                r,
                deck.eq_low as f32,
                deck.eq_mid as f32,
                deck.eq_high as f32,
            )
        } else if let Some(audio) = guard.decoded[i].as_ref() {
            let sr_ratio = audio.sample_rate as f64 / out_sr;
            let (l, r) = if deck.keylock && scratching.is_none() {
                let (wl, wr) = guard.wsola[i].next_sample(audio, rate * sr_ratio);
                guard.play_times[i] =
                    guard.wsola[i].current_input_sample() / audio.sample_rate.max(1) as f64;
                (wl, wr)
            } else {
                let sample_pos = (guard.play_times[i] * audio.sample_rate as f64).max(0.0);
                let lr = sample_varispeed(audio, sample_pos);
                guard.play_times[i] = (guard.play_times[i] + dt * rate).max(0.0);
                lr
            };
            guard.eqs[i].process_stereo(
                l,
                r,
                deck.eq_low as f32,
                deck.eq_mid as f32,
                deck.eq_high as f32,
            )
        } else {
            let mut unit_deck = deck.clone();
            unit_deck.volume_fader = 1.0;
            let s = AudioEngine::sample_deck_frame(&unit_deck, guard.play_times[i], idx);
            guard.play_times[i] = (guard.play_times[i] + dt * rate).max(0.0);
            guard.eqs[i].process_stereo(
                s,
                s,
                deck.eq_low as f32,
                deck.eq_mid as f32,
                deck.eq_high as f32,
            )
        };

        // Per-deck bipolar Color Filter (LPF <-> Flat <-> HPF)
        let color_flt = deck.color_filter;
        let has_stems = guard.decoded_stems[i].is_some();
        let (fl, fr) = guard.filters[i].process_stereo(pre_l, pre_r, color_flt);
        pre_l = fl;
        pre_r = fr;

        // Per-deck FX 1 / FX 2 (FullDeck scope, or fallback when deck has no sidecar stems)
        for (u_idx, fp) in fx_params_snap.iter().enumerate() {
            if fp.enabled
                && fp.target.matches_deck(deck_ids[i])
                && (fp.stem_scope == FxStemScope::FullDeck || !has_stems)
            {
                let (wl, wr) = guard.fx_units[u_idx].process_stereo(pre_l, pre_r, fp, deck_bpm);
                pre_l = wl;
                pre_r = wr;
            }
        }

        let vol = deck.volume_fader as f32;
        deck_post[i] = (pre_l * vol, pre_r * vol);
        if deck.pfl_cue {
            deck_pfl[i] = (pre_l, pre_r);
        }
    }

    guard.sample_counter = guard.sample_counter.wrapping_add(1);

    let (mut master_l, mut master_r) = if guard.muted || guard.master_volume <= 0.0 {
        (0.0, 0.0)
    } else {
        let sum_l = (deck_post[0].0 + deck_post[2].0) * gain_left
            + (deck_post[1].0 + deck_post[3].0) * gain_right;
        let sum_r = (deck_post[0].1 + deck_post[2].1) * gain_left
            + (deck_post[1].1 + deck_post[3].1) * gain_right;
        let mv = guard.master_volume.clamp(0.0, 1.0) as f32;
        (sum_l.tanh() * mv, sum_r.tanh() * mv)
    };

    // Master-targeted FX 1 / FX 2
    let master_bpm = guard.decks[0]
        .loaded_track
        .as_ref()
        .map(|t| t.bpm)
        .unwrap_or(126.0);
    for (u_idx, fp) in fx_params_snap.iter().enumerate() {
        if fp.enabled && fp.target == FxTarget::Master {
            let (wl, wr) = guard.fx_units[u_idx].process_stereo(master_l, master_r, fp, master_bpm);
            master_l = wl.tanh();
            master_r = wr.tanh();
        }
    }

    // Feed live Master mix (pre-split) into the non-blocking WAV/FLAC recorder tap
    if let Some(tap) = guard.recorder_tap.as_mut() {
        tap.push_frame(master_l, master_r);
    }

    // Headphone PFL Cue bus: blend PFL sum with Master PGM
    let pfl_l = (deck_pfl[0].0 + deck_pfl[1].0 + deck_pfl[2].0 + deck_pfl[3].0).tanh();
    let pfl_r = (deck_pfl[0].1 + deck_pfl[1].1 + deck_pfl[2].1 + deck_pfl[3].1).tanh();
    let mix_t = guard.cue_mix.clamp(0.0, 1.0) as f32;
    let cv = guard.cue_volume.clamp(0.0, 1.0) as f32;
    let cue_l = (pfl_l * (1.0 - mix_t) + master_l * mix_t).tanh() * cv;
    let cue_r = (pfl_r * (1.0 - mix_t) + master_r * mix_t).tanh() * cv;

    // Single-device Split Mono Cue mode: Left = Cue Mono, Right = Master Mono
    let master_out = if guard.split_cue && !guard.has_headphone_stream {
        ((cue_l + cue_r) * 0.5, (master_l + master_r) * 0.5)
    } else {
        (master_l, master_r)
    };

    (master_out, (cue_l, cue_r))
}

fn push_cue_frames(
    ring: &Arc<Mutex<VecDeque<(f32, f32)>>>,
    frames: &[(f32, f32)],
) {
    if frames.is_empty() {
        return;
    }
    if let Ok(mut q) = ring.try_lock() {
        for &lr in frames {
            if q.len() >= CUE_RING_CAPACITY {
                q.pop_front();
            }
            q.push_back(lr);
        }
    }
}

fn fill_buffer_f32(
    data: &mut [f32],
    channels: usize,
    sample_rate: u32,
    state: &Arc<Mutex<LiveMixState>>,
    cue_ring: &Arc<Mutex<VecDeque<(f32, f32)>>>,
) {
    let Ok(mut guard) = state.try_lock() else {
        data.fill(0.0);
        return;
    };
    let dt = 1.0 / sample_rate.max(1) as f64;
    let ch = channels.max(1);
    let push_hp = guard.has_headphone_stream;
    let mut cue_batch = if push_hp {
        Vec::with_capacity(data.len() / ch)
    } else {
        Vec::new()
    };

    for frame in data.chunks_mut(ch) {
        let ((l, r), cue) = next_dual_bus_frame(&mut guard, dt);
        if push_hp {
            cue_batch.push(cue);
        }
        if ch == 1 {
            frame[0] = (l + r) * 0.5;
        } else {
            frame[0] = l;
            frame[1] = r;
            for extra in frame.iter_mut().skip(2) {
                *extra = 0.0;
            }
        }
    }
    drop(guard);
    if push_hp {
        push_cue_frames(cue_ring, &cue_batch);
    }
}

fn fill_buffer_i16(
    data: &mut [i16],
    channels: usize,
    sample_rate: u32,
    state: &Arc<Mutex<LiveMixState>>,
    cue_ring: &Arc<Mutex<VecDeque<(f32, f32)>>>,
) {
    let Ok(mut guard) = state.try_lock() else {
        data.fill(0);
        return;
    };
    let dt = 1.0 / sample_rate.max(1) as f64;
    let ch = channels.max(1);
    let push_hp = guard.has_headphone_stream;
    let mut cue_batch = if push_hp {
        Vec::with_capacity(data.len() / ch)
    } else {
        Vec::new()
    };

    for frame in data.chunks_mut(ch) {
        let ((l, r), cue) = next_dual_bus_frame(&mut guard, dt);
        if push_hp {
            cue_batch.push(cue);
        }
        let li = (l.clamp(-1.0, 1.0) * 32767.0) as i16;
        let ri = (r.clamp(-1.0, 1.0) * 32767.0) as i16;
        if ch == 1 {
            frame[0] = ((li as i32 + ri as i32) / 2) as i16;
        } else {
            frame[0] = li;
            frame[1] = ri;
            for extra in frame.iter_mut().skip(2) {
                *extra = 0;
            }
        }
    }
    drop(guard);
    if push_hp {
        push_cue_frames(cue_ring, &cue_batch);
    }
}

fn fill_buffer_u16(
    data: &mut [u16],
    channels: usize,
    sample_rate: u32,
    state: &Arc<Mutex<LiveMixState>>,
    cue_ring: &Arc<Mutex<VecDeque<(f32, f32)>>>,
) {
    let Ok(mut guard) = state.try_lock() else {
        data.fill(u16::MAX / 2);
        return;
    };
    let dt = 1.0 / sample_rate.max(1) as f64;
    let ch = channels.max(1);
    let push_hp = guard.has_headphone_stream;
    let mut cue_batch = if push_hp {
        Vec::with_capacity(data.len() / ch)
    } else {
        Vec::new()
    };

    for frame in data.chunks_mut(ch) {
        let ((l, r), cue) = next_dual_bus_frame(&mut guard, dt);
        if push_hp {
            cue_batch.push(cue);
        }
        let lu = ((l.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
        let ru = ((r.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
        if ch == 1 {
            frame[0] = ((lu as u32 + ru as u32) / 2) as u16;
        } else {
            frame[0] = lu;
            frame[1] = ru;
            for extra in frame.iter_mut().skip(2) {
                *extra = u16::MAX / 2;
            }
        }
    }
    drop(guard);
    if push_hp {
        push_cue_frames(cue_ring, &cue_batch);
    }
}

fn fill_cue_f32(
    data: &mut [f32],
    channels: usize,
    cue_ring: &Arc<Mutex<VecDeque<(f32, f32)>>>,
) {
    let Ok(mut q) = cue_ring.try_lock() else {
        data.fill(0.0);
        return;
    };
    let ch = channels.max(1);
    for frame in data.chunks_mut(ch) {
        let (l, r) = q.pop_front().unwrap_or((0.0, 0.0));
        if ch == 1 {
            frame[0] = (l + r) * 0.5;
        } else {
            frame[0] = l;
            frame[1] = r;
            for extra in frame.iter_mut().skip(2) {
                *extra = 0.0;
            }
        }
    }
}

fn fill_cue_i16(
    data: &mut [i16],
    channels: usize,
    cue_ring: &Arc<Mutex<VecDeque<(f32, f32)>>>,
) {
    let Ok(mut q) = cue_ring.try_lock() else {
        data.fill(0);
        return;
    };
    let ch = channels.max(1);
    for frame in data.chunks_mut(ch) {
        let (l, r) = q.pop_front().unwrap_or((0.0, 0.0));
        let li = (l.clamp(-1.0, 1.0) * 32767.0) as i16;
        let ri = (r.clamp(-1.0, 1.0) * 32767.0) as i16;
        if ch == 1 {
            frame[0] = ((li as i32 + ri as i32) / 2) as i16;
        } else {
            frame[0] = li;
            frame[1] = ri;
            for extra in frame.iter_mut().skip(2) {
                *extra = 0;
            }
        }
    }
}

fn fill_cue_u16(
    data: &mut [u16],
    channels: usize,
    cue_ring: &Arc<Mutex<VecDeque<(f32, f32)>>>,
) {
    let Ok(mut q) = cue_ring.try_lock() else {
        data.fill(u16::MAX / 2);
        return;
    };
    let ch = channels.max(1);
    for frame in data.chunks_mut(ch) {
        let (l, r) = q.pop_front().unwrap_or((0.0, 0.0));
        let lu = ((l.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
        let ru = ((r.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
        if ch == 1 {
            frame[0] = ((lu as u32 + ru as u32) / 2) as u16;
        } else {
            frame[0] = lu;
            frame[1] = ru;
            for extra in frame.iter_mut().skip(2) {
                *extra = u16::MAX / 2;
            }
        }
    }
}

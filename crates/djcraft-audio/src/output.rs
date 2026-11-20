#![forbid(unsafe_code)]
//! Real-time Windows WASAPI / cross-platform audio output via `cpal`.
//!
//! Runs a lock-light audio callback that continuously synthesizes and mixes
//! all 4 decks with equal-power crossfader, per-deck volume faders, 4-stem gains,
//! and Master Loudness + Mute.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use crate::dsp::{sample_varispeed, DecodedAudio, DecodedStems, ThreeBandEq, WsolaDeckState};
use crate::{AudioEngine, DeckId, DeckState};

struct LiveMixState {
    decks: [DeckState; 4],
    decoded: [Option<Arc<DecodedAudio>>; 4],
    decoded_stems: [Option<Arc<DecodedStems>>; 4],
    wsola: [WsolaDeckState; 4],
    eqs: [ThreeBandEq; 4],
    crossfader: f64,
    master_volume: f64,
    muted: bool,
    play_times: [f64; 4],
    sample_counter: u64,
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
            crossfader: 0.0,
            master_volume: 0.75,
            muted: false,
            play_times: [0.0; 4],
            sample_counter: 0,
        }
    }
}

pub struct AudioOutput {
    _stream: Option<Stream>,
    state: Arc<Mutex<LiveMixState>>,
    pub device_name: String,
    pub sample_rate: u32,
    pub active: bool,
    pub error: Option<String>,
}

impl AudioOutput {
    /// Open the default OS audio output device (WASAPI on Windows) and start streaming.
    /// Never panics: if no audio device is present, returns an inactive handle with `error`.
    pub fn start_default() -> Self {
        let state = Arc::new(Mutex::new(LiveMixState::new(44100)));
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return Self {
                _stream: None,
                state,
                device_name: "None".to_string(),
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
                    state,
                    device_name,
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
            eprintln!("[djcraft-audio] stream error: {err}");
        };

        let stream_res = match sample_format {
            SampleFormat::F32 => {
                let st = Arc::clone(&state);
                device.build_output_stream(
                    &config,
                    move |data: &mut [f32], _| {
                        fill_buffer_f32(data, channels, sample_rate, &st);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let st = Arc::clone(&state);
                device.build_output_stream(
                    &config,
                    move |data: &mut [i16], _| {
                        fill_buffer_i16(data, channels, sample_rate, &st);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let st = Arc::clone(&state);
                device.build_output_stream(
                    &config,
                    move |data: &mut [u16], _| {
                        fill_buffer_u16(data, channels, sample_rate, &st);
                    },
                    err_fn,
                    None,
                )
            }
            other => {
                return Self {
                    _stream: None,
                    state,
                    device_name,
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
                    state,
                    device_name,
                    sample_rate,
                    active: false,
                    error: Some(format!("Failed to build audio stream: {err}")),
                };
            }
        };

        if let Err(err) = stream.play() {
            return Self {
                _stream: None,
                state,
                device_name,
                sample_rate,
                active: false,
                error: Some(format!("Failed to start audio stream: {err}")),
            };
        }

        Self {
            _stream: Some(stream),
            state,
            device_name,
            sample_rate,
            active: true,
            error: None,
        }
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
                let keylock_changed = guard.decks[idx].keylock != new_deck.keylock;
                if track_changed || new_deck.position_seconds < 0.02 {
                    guard.play_times[idx] = new_deck.position_seconds;
                    let sr = guard.decoded[idx]
                        .as_ref()
                        .map(|a| a.sample_rate as f64)
                        .unwrap_or(self.sample_rate as f64);
                    guard.wsola[idx].reset(new_deck.position_seconds * sr);
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

fn next_stereo_frame(guard: &mut LiveMixState, dt: f64) -> (f32, f32) {
    if guard.muted || guard.master_volume <= 0.0 {
        return (0.0, 0.0);
    }

    let xf = ((guard.crossfader + 1.0) * 0.5).clamp(0.0, 1.0);
    let gain_left = (xf * std::f64::consts::FRAC_PI_2).cos() as f32;
    let gain_right = (xf * std::f64::consts::FRAC_PI_2).sin() as f32;
    let out_sr = 1.0 / dt.max(1e-9);
    let idx = guard.sample_counter;

    let mut deck_lr = [(0.0_f32, 0.0_f32); 4];
    for (i, slot) in deck_lr.iter_mut().enumerate() {
        let deck = &guard.decks[i];
        if !deck.is_playing || deck.loaded_track.is_none() {
            continue;
        }
        let rate = (1.0 + deck.pitch_bend_pct / 100.0).clamp(0.5, 2.0);
        if let Some(stems) = guard.decoded_stems[i].as_ref() {
            let sample_pos = guard.play_times[i] * stems.sample_rate() as f64;
            let (vl, vr) = sample_varispeed(&stems.vocals, sample_pos);
            let (dl, dr) = sample_varispeed(&stems.drums, sample_pos);
            let (bl, br) = sample_varispeed(&stems.bass, sample_pos);
            let (ol, or) = sample_varispeed(&stems.other, sample_pos);
            guard.play_times[i] += dt * rate;

            let gv = deck.stem_vocals_gain as f32;
            let gd = deck.stem_drums_gain as f32;
            let gb = deck.stem_bass_gain as f32;
            let go = deck.stem_other_gain as f32;
            let vol = deck.volume_fader as f32;
            let l = (vl * gv + dl * gd + bl * gb + ol * go) * vol;
            let r = (vr * gv + dr * gd + br * gb + or * go) * vol;
            let (el, er) = guard.eqs[i].process_stereo(
                l,
                r,
                deck.eq_low as f32,
                deck.eq_mid as f32,
                deck.eq_high as f32,
            );
            *slot = (el, er);
        } else if let Some(audio) = guard.decoded[i].as_ref() {
            let sr_ratio = audio.sample_rate as f64 / out_sr;
            let (l, r) = if deck.keylock {
                let (wl, wr) = guard.wsola[i].next_sample(audio, rate * sr_ratio);
                guard.play_times[i] =
                    guard.wsola[i].current_input_sample() / audio.sample_rate.max(1) as f64;
                (wl, wr)
            } else {
                let sample_pos = guard.play_times[i] * audio.sample_rate as f64;
                let lr = sample_varispeed(audio, sample_pos);
                guard.play_times[i] += dt * rate;
                lr
            };
            let vol = deck.volume_fader as f32;
            let (el, er) = guard.eqs[i].process_stereo(
                l * vol,
                r * vol,
                deck.eq_low as f32,
                deck.eq_mid as f32,
                deck.eq_high as f32,
            );
            *slot = (el, er);
        } else {
            let s = AudioEngine::sample_deck_frame(deck, guard.play_times[i], idx);
            guard.play_times[i] += dt;
            let (el, er) = guard.eqs[i].process_stereo(
                s,
                s,
                deck.eq_low as f32,
                deck.eq_mid as f32,
                deck.eq_high as f32,
            );
            *slot = (el, er);
        }
    }

    guard.sample_counter = guard.sample_counter.wrapping_add(1);
    let sum_l = (deck_lr[0].0 + deck_lr[2].0) * gain_left + (deck_lr[1].0 + deck_lr[3].0) * gain_right;
    let sum_r = (deck_lr[0].1 + deck_lr[2].1) * gain_left + (deck_lr[1].1 + deck_lr[3].1) * gain_right;
    let mv = guard.master_volume.clamp(0.0, 1.0) as f32;
    (sum_l.tanh() * mv, sum_r.tanh() * mv)
}

fn fill_buffer_f32(
    data: &mut [f32],
    channels: usize,
    sample_rate: u32,
    state: &Arc<Mutex<LiveMixState>>,
) {
    let Ok(mut guard) = state.try_lock() else {
        data.fill(0.0);
        return;
    };
    let dt = 1.0 / sample_rate.max(1) as f64;
    let ch = channels.max(1);
    for frame in data.chunks_mut(ch) {
        let (l, r) = next_stereo_frame(&mut guard, dt);
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

fn fill_buffer_i16(
    data: &mut [i16],
    channels: usize,
    sample_rate: u32,
    state: &Arc<Mutex<LiveMixState>>,
) {
    let Ok(mut guard) = state.try_lock() else {
        data.fill(0);
        return;
    };
    let dt = 1.0 / sample_rate.max(1) as f64;
    let ch = channels.max(1);
    for frame in data.chunks_mut(ch) {
        let (l, r) = next_stereo_frame(&mut guard, dt);
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

fn fill_buffer_u16(
    data: &mut [u16],
    channels: usize,
    sample_rate: u32,
    state: &Arc<Mutex<LiveMixState>>,
) {
    let Ok(mut guard) = state.try_lock() else {
        data.fill(u16::MAX / 2);
        return;
    };
    let dt = 1.0 / sample_rate.max(1) as f64;
    let ch = channels.max(1);
    for frame in data.chunks_mut(ch) {
        let (l, r) = next_stereo_frame(&mut guard, dt);
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

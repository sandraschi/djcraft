#![forbid(unsafe_code)]
//! Real-time Windows WASAPI / cross-platform audio output via `cpal`.
//!
//! Runs a lock-light audio callback that continuously synthesizes and mixes
//! all 4 decks with equal-power crossfader, per-deck volume faders, 4-stem gains,
//! and Master Loudness + Mute.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use crate::{AudioEngine, DeckId, DeckState};

struct LiveMixState {
    decks: [DeckState; 4],
    crossfader: f64,
    master_volume: f64,
    muted: bool,
    play_times: [f64; 4],
    sample_counter: u64,
}

impl LiveMixState {
    fn new() -> Self {
        Self {
            decks: [
                AudioEngine::create_deck(DeckId::DeckA),
                AudioEngine::create_deck(DeckId::DeckB),
                AudioEngine::create_deck(DeckId::DeckC),
                AudioEngine::create_deck(DeckId::DeckD),
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
        let state = Arc::new(Mutex::new(LiveMixState::new()));
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
                // If the deck was cued back to beat 1 or switched track, snap its audio phase
                let track_changed = guard.decks[idx]
                    .loaded_track
                    .as_ref()
                    .map(|t| t.title.as_str())
                    != new_deck.loaded_track.as_ref().map(|t| t.title.as_str());
                if track_changed || new_deck.position_seconds < 0.02 {
                    guard.play_times[idx] = new_deck.position_seconds;
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
    let times = guard.play_times;
    let idx = guard.sample_counter;
    let out = AudioEngine::mix_4deck_frame(
        &guard.decks,
        guard.crossfader,
        guard.master_volume,
        guard.muted,
        &times,
        idx,
    );
    for (i, deck) in guard.decks.iter().enumerate() {
        if deck.is_playing && deck.loaded_track.is_some() {
            guard.play_times[i] += dt;
        }
    }
    guard.sample_counter = guard.sample_counter.wrapping_add(1);
    out
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

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
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use crate::dsp::{sample_varispeed, DecodedAudio, DecodedStems, ThreeBandEq, WsolaDeckState};
use crate::fx::{BipolarFilter, FxStemScope, FxTarget, FxUnitDsp, FxUnitParams};
use crate::recorder::{ActiveRecording, RecordingFormat, RecordingStopReport, RecordingTap};
use crate::sampler::{SamplePlayMode, SamplerEngine, SAMPLER_SLOTS};
use crate::studio::{DrumGenre, StepSequencer, STEP_COUNT, STEP_LANES};
use crate::{AudioEngine, DeckId, DeckState};

const CUE_RING_CAPACITY: usize = 8192;

enum SamplerCmd {
    Trigger {
        bank: usize,
        slot: usize,
    },
    Stop {
        bank: usize,
        slot: usize,
    },
    StopAll,
    SetAudio {
        bank: usize,
        slot: usize,
        name: String,
        audio: Arc<DecodedAudio>,
    },
    SetConfig {
        bank: usize,
        slot: usize,
        mode: SamplePlayMode,
        volume: f32,
    },
    SetMasterVolume(f32),
    SeqSetEnabled(bool),
    SeqSetStep {
        lane: usize,
        step: usize,
        active: bool,
    },
    SeqSetGenre(DrumGenre),
    SeqClear,
    SeqSetVolume(f32),
}

struct LiveMixState {
    decks: [DeckState; 4],
    decoded: [Option<Arc<DecodedAudio>>; 4],
    decoded_stems: [Option<Arc<DecodedStems>>; 4],
    wsola: [WsolaDeckState; 4],
    eqs: [ThreeBandEq; 4],
    filters: [BipolarFilter; 4],
    fx_units: [FxUnitDsp; 2],
    fx_params: [FxUnitParams; 2],
    sampler: SamplerEngine,
    sequencer: StepSequencer,
    sampler_rx: Option<Receiver<SamplerCmd>>,
    crossfader: f64,
    crossfader_curve: crate::CrossfaderCurve,
    crossfader_reverse: bool,
    stem_xf_mode: crate::StemCrossfaderMode,
    master_volume: f64,
    muted: bool,
    cue_volume: f64,
    cue_mix: f64,
    split_cue: bool,
    has_headphone_stream: bool,
    play_times: [f64; 4],
    slip_times: [f64; 4],
    sample_counter: u64,
    recorder_tap: Option<RecordingTap>,
    aux_send_gain: f32,
    aux_send: Option<AuxSendTap>,
    aux_return_gain: f32,
    aux_return: Option<AuxReturnTap>,
    limiter_env: f32,
    limiter_gr_db: Arc<AtomicU32>,
}

/// Non-blocking stereo PCM UDP receiver (`DJFX` packets on `127.0.0.1:20811`)
/// mixing SynthCraft's live 3U FX / synth output back into DJCraft's Master bus.
pub struct AuxReturnTap {
    socket: std::net::UdpSocket,
    ring: VecDeque<(f32, f32)>,
    pkt_buf: [u8; 8 + 512 * 8],
    packets: Arc<AtomicU64>,
}

impl AuxReturnTap {
    pub fn bind(port: u16) -> Option<Self> {
        let socket = std::net::UdpSocket::bind(("127.0.0.1", port)).ok()?;
        let _ = socket.set_nonblocking(true);
        Some(Self {
            socket,
            ring: VecDeque::with_capacity(4096),
            pkt_buf: [0u8; 8 + 512 * 8],
            packets: Arc::new(AtomicU64::new(0)),
        })
    }

    pub fn local_port(&self) -> u16 {
        self.socket.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    pub fn packet_count(&self) -> u64 {
        self.packets.load(Ordering::Relaxed)
    }

    pub fn poll_and_pop(&mut self) -> (f32, f32) {
        // Drain any ready UDP packets (capped at 4 per sample call for RT safety)
        for _ in 0..4 {
            match self.socket.recv(&mut self.pkt_buf) {
                Ok(n) if n >= 16 && &self.pkt_buf[0..4] == b"DJFX" => {
                    let frame_count = (n - 8) / 8;
                    for i in 0..frame_count {
                        let off = 8 + i * 8;
                        let l = f32::from_le_bytes([
                            self.pkt_buf[off],
                            self.pkt_buf[off + 1],
                            self.pkt_buf[off + 2],
                            self.pkt_buf[off + 3],
                        ]);
                        let r = f32::from_le_bytes([
                            self.pkt_buf[off + 4],
                            self.pkt_buf[off + 5],
                            self.pkt_buf[off + 6],
                            self.pkt_buf[off + 7],
                        ]);
                        if self.ring.len() >= 4096 {
                            self.ring.pop_front();
                        }
                        if l.is_finite() && r.is_finite() {
                            self.ring
                                .push_back((l.clamp(-2.0, 2.0), r.clamp(-2.0, 2.0)));
                        }
                    }
                    self.packets.fetch_add(1, Ordering::Relaxed);
                }
                _ => break,
            }
        }
        self.ring.pop_front().unwrap_or((0.0, 0.0))
    }
}

/// Allocation-free stereo PCM UDP sender (`DJFX` 128-frame packets -> `127.0.0.1:20810`)
/// feeding DJCraft's post-fader mix into SynthCraft's 3U Modular FX rack.
struct AuxSendTap {
    socket: std::net::UdpSocket,
    buf: [u8; 8 + 128 * 8],
    frames: usize,
    seq: u32,
}

impl AuxSendTap {
    fn new() -> Option<Self> {
        let sock = std::net::UdpSocket::bind("127.0.0.1:0").ok()?;
        let _ = sock.set_nonblocking(true);
        let mut buf = [0u8; 8 + 128 * 8];
        buf[0..4].copy_from_slice(b"DJFX");
        Some(Self {
            socket: sock,
            buf,
            frames: 0,
            seq: 0,
        })
    }

    #[inline]
    fn push_frame(&mut self, l: f32, r: f32) {
        let off = 8 + self.frames * 8;
        self.buf[off..off + 4].copy_from_slice(&l.to_le_bytes());
        self.buf[off + 4..off + 8].copy_from_slice(&r.to_le_bytes());
        self.frames += 1;
        if self.frames >= 128 {
            self.buf[4..8].copy_from_slice(&self.seq.to_le_bytes());
            let _ = self.socket.send_to(&self.buf, "127.0.0.1:20810");
            self.seq = self.seq.wrapping_add(1);
            self.frames = 0;
        }
    }
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
            sampler: SamplerEngine::new(sample_rate),
            sequencer: StepSequencer::new(sample_rate),
            sampler_rx: None,
            crossfader: 0.0,
            crossfader_curve: crate::CrossfaderCurve::Smooth,
            crossfader_reverse: false,
            stem_xf_mode: crate::StemCrossfaderMode::FullMix,
            master_volume: 0.75,
            muted: false,
            cue_volume: 0.85,
            cue_mix: 0.35,
            split_cue: false,
            has_headphone_stream: false,
            play_times: [0.0; 4],
            slip_times: [0.0; 4],
            sample_counter: 0,
            recorder_tap: None,
            aux_send_gain: 0.0,
            aux_send: None,
            aux_return_gain: 0.0,
            aux_return: None,
            limiter_env: 0.0,
            limiter_gr_db: Arc::new(AtomicU32::new(0.0_f32.to_bits())),
        }
    }
}

pub struct AudioOutput {
    _stream: Option<Stream>,
    headphone_stream: Option<Stream>,
    state: Arc<Mutex<LiveMixState>>,
    cue_ring: Arc<Mutex<VecDeque<(f32, f32)>>>,
    sampler_tx: Sender<SamplerCmd>,
    active_recording: Option<ActiveRecording>,
    pub device_name: String,
    pub headphone_device_name: Option<String>,
    pub sample_rate: u32,
    pub active: bool,
    pub error: Option<String>,
}

fn offline_output(
    state: Arc<Mutex<LiveMixState>>,
    cue_ring: Arc<Mutex<VecDeque<(f32, f32)>>>,
    sampler_tx: Sender<SamplerCmd>,
    device_name: String,
    sample_rate: u32,
    error: Option<String>,
) -> AudioOutput {
    AudioOutput {
        _stream: None,
        headphone_stream: None,
        state,
        cue_ring,
        sampler_tx,
        active_recording: None,
        device_name,
        headphone_device_name: None,
        sample_rate,
        active: false,
        error,
    }
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
        let (sampler_tx, sampler_rx) = mpsc::channel();
        let mut mix = LiveMixState::new(44100);
        mix.sampler_rx = Some(sampler_rx);
        let state = Arc::new(Mutex::new(mix));
        let cue_ring = Arc::new(Mutex::new(VecDeque::with_capacity(CUE_RING_CAPACITY)));
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return offline_output(
                state,
                cue_ring,
                sampler_tx,
                "None".to_string(),
                44100,
                Some("No default audio output device found".to_string()),
            );
        };

        let device_name = device
            .name()
            .unwrap_or_else(|_| "Default Audio Device".to_string());
        let supported = match device.default_output_config() {
            Ok(cfg) => cfg,
            Err(err) => {
                return offline_output(
                    state,
                    cue_ring,
                    sampler_tx,
                    device_name,
                    44100,
                    Some(format!("Audio config error: {err}")),
                );
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
                return offline_output(
                    state,
                    cue_ring,
                    sampler_tx,
                    device_name,
                    sample_rate,
                    Some(format!("Unsupported sample format: {other:?}")),
                );
            }
        };

        let stream = match stream_res {
            Ok(s) => s,
            Err(err) => {
                return offline_output(
                    state,
                    cue_ring,
                    sampler_tx,
                    device_name,
                    sample_rate,
                    Some(format!("Failed to build audio stream: {err}")),
                );
            }
        };

        if let Err(err) = stream.play() {
            return offline_output(
                state,
                cue_ring,
                sampler_tx,
                device_name,
                sample_rate,
                Some(format!("Failed to start audio stream: {err}")),
            );
        }

        Self {
            _stream: Some(stream),
            headphone_stream: None,
            state,
            cue_ring,
            sampler_tx,
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
            return Ok(
                "Headphone bus disabled (use Split Cue for single-device monitoring)".to_string(),
            );
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

    /// Set the post-fader stereo Aux Send gain (`0.0..=1.0`) streaming `DJFX` UDP audio packets
    /// to SynthCraft's 3U Modular FX rack (`127.0.0.1:20810`).
    pub fn set_synthcraft_aux_send(&self, gain: f32) {
        if let Ok(mut guard) = self.state.lock() {
            let g = gain.clamp(0.0, 1.0);
            guard.aux_send_gain = g;
            if g > 0.001 && guard.aux_send.is_none() {
                guard.aux_send = AuxSendTap::new();
            } else if g <= 0.001 {
                guard.aux_send = None;
            }
        }
    }

    /// Enable or disable the bidirectional SynthCraft stereo Aux Return receiver (`127.0.0.1:20811`).
    pub fn set_synthcraft_aux_return(&self, gain: f32) {
        if let Ok(mut guard) = self.state.lock() {
            let g = gain.clamp(0.0, 1.5);
            guard.aux_return_gain = g;
            if g > 0.001 && guard.aux_return.is_none() {
                guard.aux_return = AuxReturnTap::bind(20811);
            } else if g <= 0.001 {
                guard.aux_return = None;
            }
        }
    }

    /// Configure crossfader curve (`Smooth`, `Linear`, `ScratchCut`) and Hamster reverse switch.
    pub fn set_crossfader_curve(&self, curve: crate::CrossfaderCurve, reverse: bool) {
        if let Ok(mut guard) = self.state.lock() {
            guard.crossfader_curve = curve;
            guard.crossfader_reverse = reverse;
        }
    }

    /// Configure the stem-aware neural crossfader routing mode (`FullMix`, `VocalsOnly`, `DrumsOnly`, `BassSwap`, `InstrumentalOnly`).
    pub fn set_stem_crossfader_mode(&self, mode: crate::StemCrossfaderMode) {
        if let Ok(mut guard) = self.state.lock() {
            guard.stem_xf_mode = mode;
        }
    }

    /// Latest Master Bus soft-knee limiter gain reduction in dB (`0.0` = clean, `> 0.0` = limiting).
    pub fn master_limiter_gr_db(&self) -> f32 {
        if let Ok(guard) = self.state.lock() {
            f32::from_bits(guard.limiter_gr_db.load(Ordering::Relaxed))
        } else {
            0.0
        }
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

    fn enqueue_sampler(&self, cmd: SamplerCmd) {
        if self.sampler_tx.send(cmd).is_err() {
            return;
        }
        // The live callback drains the queue. Offline (no stream) applies it here
        // so a pad hit still updates the engine without taking the mix lock on the callback.
        if !self.active {
            if let Ok(mut guard) = self.state.try_lock() {
                drain_sampler_queue(&mut guard);
            }
        }
    }

    pub fn trigger_sample(&self, bank: usize, slot: usize) {
        self.enqueue_sampler(SamplerCmd::Trigger { bank, slot });
    }

    pub fn stop_sample(&self, bank: usize, slot: usize) {
        self.enqueue_sampler(SamplerCmd::Stop { bank, slot });
    }

    pub fn stop_all_samples(&self) {
        self.enqueue_sampler(SamplerCmd::StopAll);
    }

    pub fn set_sample_slot_audio(
        &self,
        bank: usize,
        slot: usize,
        name: String,
        audio: Arc<DecodedAudio>,
    ) {
        self.enqueue_sampler(SamplerCmd::SetAudio {
            bank,
            slot,
            name,
            audio,
        });
    }

    pub fn set_sample_slot_config(
        &self,
        bank: usize,
        slot: usize,
        mode: SamplePlayMode,
        volume: f32,
    ) {
        self.enqueue_sampler(SamplerCmd::SetConfig {
            bank,
            slot,
            mode,
            volume,
        });
    }

    pub fn set_sampler_master_volume(&self, volume: f32) {
        self.enqueue_sampler(SamplerCmd::SetMasterVolume(volume));
    }

    pub fn sampler_active_mask(&self, bank: usize) -> [bool; SAMPLER_SLOTS] {
        if let Ok(guard) = self.state.try_lock() {
            guard.sampler.active_mask(bank)
        } else {
            [false; SAMPLER_SLOTS]
        }
    }

    pub fn sampler_slot_names(&self, bank: usize) -> [String; SAMPLER_SLOTS] {
        for _ in 0..200 {
            if let Ok(guard) = self.state.try_lock() {
                return guard.sampler.slot_names(bank);
            }
            std::thread::yield_now();
        }
        std::array::from_fn(|i| format!("Slot {}", i + 1))
    }

    pub fn set_sequencer_enabled(&self, enabled: bool) {
        self.enqueue_sampler(SamplerCmd::SeqSetEnabled(enabled));
    }

    pub fn set_sequencer_step(&self, lane: usize, step: usize, active: bool) {
        self.enqueue_sampler(SamplerCmd::SeqSetStep { lane, step, active });
    }

    pub fn set_sequencer_genre(&self, genre: DrumGenre) {
        self.enqueue_sampler(SamplerCmd::SeqSetGenre(genre));
    }

    pub fn clear_sequencer(&self) {
        self.enqueue_sampler(SamplerCmd::SeqClear);
    }

    pub fn set_sequencer_volume(&self, volume: f32) {
        self.enqueue_sampler(SamplerCmd::SeqSetVolume(volume));
    }

    pub fn sequencer_snapshot(&self) -> (bool, usize, [[bool; STEP_COUNT]; STEP_LANES]) {
        if let Ok(guard) = self.state.try_lock() {
            (
                guard.sequencer.enabled,
                guard.sequencer.current_step,
                guard.sequencer.steps,
            )
        } else {
            (false, 0, [[false; STEP_COUNT]; STEP_LANES])
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
                    guard.slip_times[idx] = t0;
                    let sr = guard.decoded[idx]
                        .as_ref()
                        .map(|a| a.sample_rate as f64)
                        .unwrap_or(self.sample_rate as f64);
                    guard.wsola[idx].reset(t0 * sr);
                } else if new_deck.slip_mode
                    && guard.decks[idx].scratch_velocity.is_some()
                    && new_deck.scratch_velocity.is_none()
                {
                    // Slip Mode release: snap playback back to the silent slip playhead!
                    let t_slip = guard.slip_times[idx].max(0.0);
                    guard.play_times[idx] = t_slip;
                    let sr = guard.decoded[idx]
                        .as_ref()
                        .map(|a| a.sample_rate as f64)
                        .unwrap_or(self.sample_rate as f64);
                    guard.wsola[idx].reset(t_slip * sr);
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

fn drain_sampler_queue(guard: &mut LiveMixState) {
    let Some(rx) = guard.sampler_rx.take() else {
        return;
    };
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            SamplerCmd::Trigger { bank, slot } => guard.sampler.trigger(bank, slot),
            SamplerCmd::Stop { bank, slot } => guard.sampler.stop_slot(bank, slot),
            SamplerCmd::StopAll => guard.sampler.stop_all(),
            SamplerCmd::SetAudio {
                bank,
                slot,
                name,
                audio,
            } => guard.sampler.set_slot_audio(bank, slot, name, audio),
            SamplerCmd::SetConfig {
                bank,
                slot,
                mode,
                volume,
            } => guard.sampler.set_slot_config(bank, slot, mode, volume),
            SamplerCmd::SetMasterVolume(volume) => {
                guard.sampler.master_volume = volume.clamp(0.0, 1.0);
            }
            SamplerCmd::SeqSetEnabled(en) => guard.sequencer.set_enabled(en),
            SamplerCmd::SeqSetStep { lane, step, active } => {
                guard.sequencer.set_step(lane, step, active);
            }
            SamplerCmd::SeqSetGenre(genre) => guard.sequencer.set_genre(genre),
            SamplerCmd::SeqClear => guard.sequencer.clear(),
            SamplerCmd::SeqSetVolume(vol) => {
                guard.sequencer.volume = vol.clamp(0.0, 1.0);
            }
        }
    }
    guard.sampler_rx = Some(rx);
}

/// Compute one dual-bus frame: `((master_out_l, master_out_r), (cue_l, cue_r))`.
fn next_dual_bus_frame(guard: &mut LiveMixState, dt: f64) -> ((f32, f32), (f32, f32)) {
    drain_sampler_queue(guard);
    let (ga, gb) = AudioEngine::crossfader_gains_curved(
        guard.crossfader,
        guard.crossfader_curve,
        guard.crossfader_reverse,
    );
    let gain_left = ga as f32;
    let gain_right = gb as f32;
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

            let xf_deck = if i == 0 || i == 2 {
                gain_left
            } else {
                gain_right
            };
            let (xf_v, xf_d, xf_b, xf_o) = match guard.stem_xf_mode {
                crate::StemCrossfaderMode::FullMix => (1.0, 1.0, 1.0, 1.0),
                crate::StemCrossfaderMode::VocalsOnly => (xf_deck, 1.0, 1.0, 1.0),
                crate::StemCrossfaderMode::DrumsOnly => (1.0, xf_deck, 1.0, 1.0),
                crate::StemCrossfaderMode::BassSwap => (1.0, 1.0, xf_deck, 1.0),
                crate::StemCrossfaderMode::InstrumentalOnly => (1.0, xf_deck, xf_deck, xf_deck),
            };
            let gv = deck.stem_vocals_gain as f32 * xf_v;
            let gd = deck.stem_drums_gain as f32 * xf_d;
            let gb = deck.stem_bass_gain as f32 * xf_b;
            let go = deck.stem_other_gain as f32 * xf_o;
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
        let (fl, fr) = guard.filters[i].process_stereo(pre_l, pre_r, color_flt);
        pre_l = fl;
        pre_r = fr;

        // Full-deck FX only. Vocals/Drums scopes run in the stem branch and stay dry
        // when this deck has no sidecar stems.
        for (u_idx, fp) in fx_params_snap.iter().enumerate() {
            if fp.enabled
                && fp.target.matches_deck(deck_ids[i])
                && fp.stem_scope == FxStemScope::FullDeck
            {
                let (wl, wr) = guard.fx_units[u_idx].process_stereo(pre_l, pre_r, fp, deck_bpm);
                pre_l = wl;
                pre_r = wr;
            }
        }

        let vol = deck.volume_fader as f32;
        if deck.slip_mode && deck.is_playing {
            let norm_rate = (1.0 + deck.pitch_bend_pct / 100.0).clamp(0.5, 2.0);
            guard.slip_times[i] = (guard.slip_times[i] + dt * norm_rate).max(0.0);
        } else {
            guard.slip_times[i] = guard.play_times[i];
        }
        deck_post[i] = (pre_l * vol, pre_r * vol);
        if deck.pfl_cue {
            deck_pfl[i] = (pre_l, pre_r);
        }
    }

    guard.sample_counter = guard.sample_counter.wrapping_add(1);

    let seq_bpm = guard.decks[0]
        .loaded_track
        .as_ref()
        .map(|t| (t.bpm * (1.0 + guard.decks[0].pitch_bend_pct / 100.0)).clamp(60.0, 200.0))
        .unwrap_or(126.0);
    let (samp_l, samp_r) = guard.sampler.next_stereo_frame(out_sr);
    let (seq_l, seq_r) = guard.sequencer.next_stereo_frame(out_sr, seq_bpm);

    let (mut master_l, mut master_r) = if guard.muted || guard.master_volume <= 0.0 {
        (0.0, 0.0)
    } else {
        let stem_split = guard.stem_xf_mode != crate::StemCrossfaderMode::FullMix;
        let g0 = if stem_split && guard.decoded_stems[0].is_some() {
            1.0
        } else {
            gain_left
        };
        let g2 = if stem_split && guard.decoded_stems[2].is_some() {
            1.0
        } else {
            gain_left
        };
        let g1 = if stem_split && guard.decoded_stems[1].is_some() {
            1.0
        } else {
            gain_right
        };
        let g3 = if stem_split && guard.decoded_stems[3].is_some() {
            1.0
        } else {
            gain_right
        };
        let sum_l = deck_post[0].0 * g0
            + deck_post[2].0 * g2
            + deck_post[1].0 * g1
            + deck_post[3].0 * g3
            + samp_l
            + seq_l;
        let sum_r = deck_post[0].1 * g0
            + deck_post[2].1 * g2
            + deck_post[1].1 * g1
            + deck_post[3].1 * g3
            + samp_r
            + seq_r;
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

    // Mix optional SynthCraft stereo Aux Return (`DJFX` UDP 127.0.0.1:20811) into Master bus
    let aux_ret_g = guard.aux_return_gain;
    if aux_ret_g > 0.001 {
        if let Some(ret) = guard.aux_return.as_mut() {
            let (rl, rr) = ret.poll_and_pop();
            master_l += rl * aux_ret_g;
            master_r += rr * aux_ret_g;
        }
    }

    // Soft-knee Club Master Limiter (threshold 0.88, fast attack / smooth release) + live GR meter
    let peak = master_l.abs().max(master_r.abs());
    let coeff = if peak > guard.limiter_env {
        0.35
    } else {
        0.002
    };
    guard.limiter_env += coeff * (peak - guard.limiter_env);
    let thresh = 0.88_f32;
    let lim_gain = if guard.limiter_env > thresh {
        let excess = guard.limiter_env / thresh;
        let compressed = 1.0 + (excess - 1.0) * 0.25;
        (compressed / excess).clamp(0.1, 1.0)
    } else {
        1.0
    };
    master_l = (master_l * lim_gain).tanh();
    master_r = (master_r * lim_gain).tanh();
    let gr_db = if lim_gain < 0.999 {
        (-20.0 * lim_gain.log10()).clamp(0.0, 24.0)
    } else {
        0.0
    };
    guard
        .limiter_gr_db
        .store(gr_db.to_bits(), Ordering::Relaxed);

    // Feed live Master mix (pre-split) into the non-blocking WAV/FLAC recorder tap
    if let Some(tap) = guard.recorder_tap.as_mut() {
        tap.push_frame(master_l, master_r);
    }

    // Stream post-fader stereo Aux Send (`DJFX` UDP 127.0.0.1:20810) into SynthCraft's 3U rack
    let aux_g = guard.aux_send_gain;
    if aux_g > 0.001 {
        if let Some(aux) = guard.aux_send.as_mut() {
            aux.push_frame(master_l * aux_g, master_r * aux_g);
        }
    }

    // Headphone cue side always includes the sampler, so a full-CUE mix still hears pads.
    // At full PGM the cue bus is the master, which already contains the sampler once.
    let pfl_l = (deck_pfl[0].0 + deck_pfl[1].0 + deck_pfl[2].0 + deck_pfl[3].0 + samp_l).tanh();
    let pfl_r = (deck_pfl[0].1 + deck_pfl[1].1 + deck_pfl[2].1 + deck_pfl[3].1 + samp_r).tanh();
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

fn push_cue_frames(ring: &Arc<Mutex<VecDeque<(f32, f32)>>>, frames: &[(f32, f32)]) {
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

fn fill_cue_f32(data: &mut [f32], channels: usize, cue_ring: &Arc<Mutex<VecDeque<(f32, f32)>>>) {
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

fn fill_cue_i16(data: &mut [i16], channels: usize, cue_ring: &Arc<Mutex<VecDeque<(f32, f32)>>>) {
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

fn fill_cue_u16(data: &mut [u16], channels: usize, cue_ring: &Arc<Mutex<VecDeque<(f32, f32)>>>) {
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

struct Room2PlaybackState {
    track: Option<Arc<DecodedAudio>>,
    pos_sec: f64,
    playing: bool,
    finished: bool,
    volume: f32,
    out_sample_rate: u32,
}

impl Default for Room2PlaybackState {
    fn default() -> Self {
        Self {
            track: None,
            pos_sec: 0.0,
            playing: false,
            finished: false,
            volume: 0.85,
            out_sample_rate: 44_100,
        }
    }
}

fn next_room2_frame(st: &mut Room2PlaybackState) -> (f32, f32) {
    if !st.playing {
        return (0.0, 0.0);
    }
    let Some(ref audio) = st.track else {
        return (0.0, 0.0);
    };
    if st.pos_sec >= audio.duration_seconds || audio.left.is_empty() {
        st.playing = false;
        st.finished = true;
        return (0.0, 0.0);
    }
    let (l, r) = sample_varispeed(audio, st.pos_sec);
    st.pos_sec += 1.0 / st.out_sample_rate.max(8_000) as f64;
    if st.pos_sec >= audio.duration_seconds {
        st.playing = false;
        st.finished = true;
    }
    (l * st.volume, r * st.volume)
}

/// Independent secondary audio player for Two-Room setups (Room 2 prepared autoplay).
pub struct Room2AudioOutput {
    stream: Option<Stream>,
    device_name: Option<String>,
    state: Arc<Mutex<Room2PlaybackState>>,
}

impl Default for Room2AudioOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl Room2AudioOutput {
    pub fn new() -> Self {
        Self {
            stream: None,
            device_name: None,
            state: Arc::new(Mutex::new(Room2PlaybackState::default())),
        }
    }

    pub fn device_name(&self) -> Option<&str> {
        self.device_name.as_deref()
    }

    pub fn set_device(&mut self, device_name: Option<&str>) -> Result<String, String> {
        self.stream = None;
        self.device_name = None;

        let Some(target_name) = device_name.filter(|s| !s.trim().is_empty()) else {
            return Ok("Room 2 audio output disabled".to_string());
        };

        let host = cpal::default_host();
        let devices = host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {e}"))?;
        let Some(device) = devices
            .into_iter()
            .find(|d| d.name().ok().as_deref() == Some(target_name))
        else {
            return Err(format!("Room 2 output device not found: {target_name}"));
        };

        let supported = device
            .default_output_config()
            .map_err(|e| format!("Room 2 audio config error: {e}"))?;
        let channels = supported.channels() as usize;
        let sample_format = supported.sample_format();
        let out_sr = supported.sample_rate().0;
        let config: cpal::StreamConfig = supported.into();

        if let Ok(mut guard) = self.state.lock() {
            guard.out_sample_rate = out_sr;
        }
        let state = Arc::clone(&self.state);
        let err_fn = |err| {
            eprintln!("[djcraft-audio] Room 2 stream error: {err}");
        };

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data: &mut [f32], _| {
                    let ch = channels.max(1);
                    let Ok(mut st) = state.try_lock() else {
                        data.fill(0.0);
                        return;
                    };
                    for frame in data.chunks_mut(ch) {
                        let (l, r) = next_room2_frame(&mut st);
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
                },
                err_fn,
                None,
            ),
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |data: &mut [i16], _| {
                    let ch = channels.max(1);
                    let Ok(mut st) = state.try_lock() else {
                        data.fill(0);
                        return;
                    };
                    for frame in data.chunks_mut(ch) {
                        let (l, r) = next_room2_frame(&mut st);
                        let li = (l.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        let ri = (r.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
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
                },
                err_fn,
                None,
            ),
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |data: &mut [u16], _| {
                    let ch = channels.max(1);
                    let Ok(mut st) = state.try_lock() else {
                        data.fill(u16::MAX / 2);
                        return;
                    };
                    for frame in data.chunks_mut(ch) {
                        let (l, r) = next_room2_frame(&mut st);
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
                },
                err_fn,
                None,
            ),
            other => return Err(format!("Unsupported Room 2 sample format: {other:?}")),
        }
        .map_err(|e| format!("Failed to build Room 2 stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start Room 2 stream: {e}"))?;

        self.stream = Some(stream);
        self.device_name = Some(target_name.to_string());
        Ok(format!("Room 2 autoplay routed to {target_name}"))
    }

    pub fn load_and_play(&mut self, audio: Arc<DecodedAudio>) {
        if let Ok(mut st) = self.state.lock() {
            st.track = Some(audio);
            st.pos_sec = 0.0;
            st.finished = false;
            st.playing = true;
        }
    }

    pub fn set_playing(&mut self, playing: bool) {
        if let Ok(mut st) = self.state.lock() {
            st.playing = playing;
        }
    }

    pub fn stop(&mut self) {
        if let Ok(mut st) = self.state.lock() {
            st.playing = false;
            st.pos_sec = 0.0;
            st.finished = false;
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        if let Ok(mut st) = self.state.lock() {
            st.volume = volume.clamp(0.0, 1.0);
        }
    }

    pub fn snapshot(&self) -> (bool, bool, f64, f64) {
        if let Ok(st) = self.state.lock() {
            let dur = st.track.as_ref().map_or(0.0, |t| t.duration_seconds);
            (st.playing, st.finished, st.pos_sec, dur)
        } else {
            (false, false, 0.0, 0.0)
        }
    }
}

#[cfg(test)]
mod mix_fix_tests {
    use super::*;

    #[test]
    fn crossfader_curves_and_hamster_reverse_behave_accurately() {
        let (a_cut, b_cut) =
            AudioEngine::crossfader_gains_curved(-0.5, crate::CrossfaderCurve::ScratchCut, false);
        assert!((a_cut - 1.0).abs() < 1e-6 && (b_cut - 1.0).abs() < 1e-6);

        let (a_rev, b_rev) =
            AudioEngine::crossfader_gains_curved(-1.0, crate::CrossfaderCurve::Linear, true);
        assert!(a_rev < 1e-6 && (b_rev - 1.0).abs() < 1e-6);
    }

    #[test]
    fn aux_return_udp_receives_djfx_stereo_pcm_packet() {
        let mut tap = AuxReturnTap::bind(0).expect("bind ephemeral aux return");
        let port = tap.local_port();
        let tx = std::net::UdpSocket::bind("127.0.0.1:0").expect("tx sock");
        let mut pkt = vec![0u8; 8 + 4 * 8];
        pkt[0..4].copy_from_slice(b"DJFX");
        pkt[4..8].copy_from_slice(&1u32.to_le_bytes());
        pkt[8..12].copy_from_slice(&0.5_f32.to_le_bytes());
        pkt[12..16].copy_from_slice(&(-0.25_f32).to_le_bytes());
        tx.send_to(&pkt, ("127.0.0.1", port)).expect("send djfx");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let (l, r) = tap.poll_and_pop();
        assert!((l - 0.5).abs() < 1e-5 && (r + 0.25).abs() < 1e-5);
        assert_eq!(tap.packet_count(), 1);
    }
    use crate::dsp::DecodedAudio;
    use crate::fx::{FxKind, FxSlotParams, FxStemScope, FxTarget, FxUnitParams};
    use crate::TrackMeta;

    fn sine_deck(scope: FxStemScope, enabled: bool) -> LiveMixState {
        let mut guard = LiveMixState::new(44100);
        let n = 4096;
        let left: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin() * 0.4)
            .collect();
        let right = left.clone();
        guard.decoded[0] = Some(Arc::new(DecodedAudio {
            sample_rate: 44100,
            left,
            right,
            duration_seconds: n as f64 / 44100.0,
        }));
        guard.decks[0].is_playing = true;
        guard.decks[0].volume_fader = 1.0;
        guard.decks[0].loaded_track = Some(TrackMeta {
            title: "sine".to_string(),
            artist: "test".to_string(),
            bpm: 120.0,
            musical_key: "8A".to_string(),
            duration_seconds: 1.0,
        });
        guard.master_volume = 1.0;
        guard.fx_params[0] = FxUnitParams {
            enabled,
            target: FxTarget::DeckA,
            stem_scope: scope,
            beats: 0.5,
            wet_dry: 1.0,
            slots: [
                FxSlotParams {
                    kind: FxKind::Bitcrusher,
                    enabled: true,
                    depth: 1.0,
                },
                FxSlotParams::default(),
                FxSlotParams::default(),
            ],
        };
        guard
    }

    fn master_frames(guard: &mut LiveMixState, n: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let ((l, _), _) = next_dual_bus_frame(guard, 1.0 / 44100.0);
            out.push(l);
        }
        out
    }

    #[test]
    fn vocals_scope_without_stems_stays_dry() {
        let dry = master_frames(&mut sine_deck(FxStemScope::FullDeck, false), 300);
        let vocals = master_frames(&mut sine_deck(FxStemScope::VocalsOnly, true), 300);
        let full = master_frames(&mut sine_deck(FxStemScope::FullDeck, true), 300);
        let max_abs = |a: &[f32], b: &[f32]| -> f32 {
            a.iter()
                .zip(b)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0, f32::max)
        };
        let dry_vs_vocals = max_abs(&dry, &vocals);
        let dry_vs_full = max_abs(&dry, &full);
        assert!(
            dry_vs_vocals < 1e-5,
            "Vocals scope without stems must match the dry deck, diff={dry_vs_vocals}"
        );
        assert!(
            dry_vs_full > 0.02,
            "Full-deck bitcrusher must change the mix, diff={dry_vs_full}"
        );
    }

    #[test]
    fn sampler_reaches_cue_bus_when_master_is_down() {
        let mut guard = LiveMixState::new(44100);
        guard.master_volume = 0.0;
        guard.cue_volume = 1.0;
        guard.cue_mix = 0.0;
        guard.sampler.trigger(0, 0);
        let mut master = 0.0_f32;
        let mut cue = 0.0_f32;
        for _ in 0..3000 {
            let ((ml, mr), (cl, cr)) = next_dual_bus_frame(&mut guard, 1.0 / 44100.0);
            master += ml.abs() + mr.abs();
            cue += cl.abs() + cr.abs();
        }
        assert!(
            master < 1e-4,
            "master fader down must silence the house, e={master}"
        );
        assert!(
            cue > 1.0,
            "full-CUE headphones must still hear the sampler, e={cue}"
        );
    }

    #[test]
    fn sampler_command_queue_applies_on_the_audio_side() {
        let (tx, rx) = mpsc::channel();
        let mut guard = LiveMixState::new(44100);
        guard.sampler_rx = Some(rx);
        guard.master_volume = 1.0;
        guard.cue_volume = 1.0;
        guard.cue_mix = 0.0;
        tx.send(SamplerCmd::Trigger { bank: 0, slot: 2 }).unwrap();
        let mut cue = 0.0_f32;
        for _ in 0..3000 {
            let (_, (cl, cr)) = next_dual_bus_frame(&mut guard, 1.0 / 44100.0);
            cue += cl.abs() + cr.abs();
        }
        assert!(cue > 1.0, "queued pad hit must reach the cue bus, e={cue}");
        assert!(guard.sampler.active_mask(0)[2]);
    }

    #[test]
    fn stem_crossfader_mode_isolates_vocals_while_keeping_drums_at_unity() {
        let sr = 44100;
        let len = 4096;
        let silent = DecodedAudio {
            sample_rate: sr,
            left: vec![0.0; len],
            right: vec![0.0; len],
            duration_seconds: len as f64 / sr as f64,
        };
        let tone = |freq: f32| DecodedAudio {
            sample_rate: sr,
            left: (0..len)
                .map(|i| (std::f32::consts::TAU * freq * i as f32 / sr as f32).sin() * 0.5)
                .collect(),
            right: (0..len)
                .map(|i| (std::f32::consts::TAU * freq * i as f32 / sr as f32).sin() * 0.5)
                .collect(),
            duration_seconds: len as f64 / sr as f64,
        };
        // Deck A has Drums only (no vocals). Push crossfader all the way to Deck B (+1.0).
        let stems_a = Arc::new(DecodedStems {
            vocals: silent.clone(),
            drums: tone(120.0),
            bass: silent.clone(),
            other: silent,
        });
        let mut guard = LiveMixState::new(sr);
        guard.master_volume = 1.0;
        guard.crossfader = 1.0; // full Deck B
        guard.decks[0].is_playing = true;
        guard.decks[0].loaded_track = Some(TrackMeta {
            title: "Stem Test".into(),
            artist: "Test".into(),
            bpm: 126.0,
            musical_key: "8A".into(),
            duration_seconds: 1.0,
        });
        guard.decoded_stems[0] = Some(stems_a);

        // In FullMix, Deck A is silenced when crossfader = +1.0
        guard.stem_xf_mode = crate::StemCrossfaderMode::FullMix;
        let mut full_energy = 0.0_f32;
        for _ in 0..512 {
            let ((l, r), _) = next_dual_bus_frame(&mut guard, 1.0 / sr as f64);
            full_energy += l.abs() + r.abs();
        }
        assert!(full_energy < 1e-3, "FullMix at +1.0 must silence Deck A");

        // In VocalsOnly, Deck A's Drums remain audible at full level even at crossfader = +1.0
        guard.play_times[0] = 0.0;
        guard.stem_xf_mode = crate::StemCrossfaderMode::VocalsOnly;
        let mut voc_mode_energy = 0.0_f32;
        for _ in 0..512 {
            let ((l, r), _) = next_dual_bus_frame(&mut guard, 1.0 / sr as f64);
            voc_mode_energy += l.abs() + r.abs();
        }
        assert!(
            voc_mode_energy > 20.0,
            "VocalsOnly must keep Deck A drums playing, energy={voc_mode_energy}"
        );
    }
}

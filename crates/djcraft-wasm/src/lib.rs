//! Bundler-free WebAssembly (`wasm32-unknown-unknown`) real-time DJ DSP engine for DJCraft.
//!
//! Exports a minimal C ABI over WASM linear memory so any browser Web Audio `AudioWorklet`
//! or `ScriptProcessorNode` can instantiate `djcraft_wasm.wasm` directly via
//! `WebAssembly.instantiate(bytes)` with zero `wasm-bindgen` or bundler dependencies.

use std::f32::consts::TAU;

struct DeckSynth {
    playing: bool,
    bpm: f32,
    volume: f32,
    filter: f32,
    phase_sec: f64,
    lp_l: f32,
    lp_r: f32,
    bp_l: f32,
    bp_r: f32,
    root_hz: f32,
}

impl DeckSynth {
    fn new(bpm: f32, root_hz: f32) -> Self {
        Self {
            playing: false,
            bpm,
            volume: 0.9,
            filter: 0.0,
            phase_sec: 0.0,
            lp_l: 0.0,
            lp_r: 0.0,
            bp_l: 0.0,
            bp_r: 0.0,
            root_hz,
        }
    }

    fn tick(&mut self, sr: f32) -> (f32, f32) {
        if !self.playing {
            return (0.0, 0.0);
        }
        let dt = 1.0 / sr.max(8000.0);
        self.phase_sec += dt as f64;
        let t = self.phase_sec as f32;
        let beat_sec = 60.0 / self.bpm.clamp(60.0, 200.0);
        let beat_pos = (t / beat_sec).fract();
        let beat_idx = (t / beat_sec).floor() as u32;
        let bar_beat = beat_idx % 4;
        let sub16 = (beat_pos * 4.0).fract();

        // Punchy 4/4 club kick
        let kick_env = (-beat_pos * 14.0).exp();
        let kick_freq = 48.0 + 135.0 * (-beat_pos * 28.0).exp();
        let kick = (TAU * kick_freq * beat_pos * beat_sec).sin() * kick_env * 0.65;

        // Rolling club bassline (octave + fifth movement)
        let bass_mult = match beat_idx % 8 {
            0 | 1 | 4 => 1.0,
            2 | 3 => 1.189, // minor third
            5 => 1.335,     // fourth
            6 => 1.498,     // fifth
            _ => 1.189,
        };
        let bass_f = self.root_hz * bass_mult;
        let bass_env = (1.0 - sub16 * 0.65).clamp(0.15, 1.0);
        let bass = ((TAU * bass_f * t).sin() * 0.7 + (TAU * bass_f * 2.0 * t).sin() * 0.25)
            * bass_env
            * 0.32;

        // Off-beat crisp hi-hat & chord stab
        let hat_env = if (beat_pos - 0.5).abs() < 0.14 {
            (1.0 - (beat_pos - 0.5).abs() / 0.14).powi(2)
        } else {
            0.0
        };
        let noise = ((t * 19_349.7).sin() * 43_758.547).fract() * 2.0 - 1.0;
        let hat = noise * hat_env * 0.22;

        let chord_env = if bar_beat == 1 || bar_beat == 3 {
            (-beat_pos * 7.0).exp() * 0.24
        } else {
            0.0
        };
        let c1 = (TAU * self.root_hz * 4.0 * t).sin();
        let c2 = (TAU * self.root_hz * 4.0 * 1.189 * t).sin();
        let c3 = (TAU * self.root_hz * 4.0 * 1.498 * t).sin();
        let chord_l = (c1 + c2 * 0.8) * chord_env;
        let chord_r = (c1 + c3 * 0.8) * chord_env;

        let mut l = (kick + bass + hat + chord_l) * self.volume;
        let mut r = (kick + bass + hat * 0.9 + chord_r) * self.volume;

        // Bipolar Chamberlin SVF Color Filter (-1.0 LPF .. 0.0 Flat .. +1.0 HPF)
        let f_amt = self.filter.clamp(-1.0, 1.0);
        if f_amt.abs() > 0.02 {
            let cutoff = if f_amt < 0.0 {
                180.0 * (12_000.0_f32 / 180.0).powf(1.0 + f_amt)
            } else {
                90.0 * (6_500.0_f32 / 90.0).powf(f_amt)
            };
            let g = (2.0 * (std::f32::consts::PI * cutoff / sr).sin()).clamp(0.01, 0.95);
            let q = 0.72_f32;

            let hp_l = l - self.lp_l - q * self.bp_l;
            self.bp_l += g * hp_l;
            self.lp_l += g * self.bp_l;
            let hp_r = r - self.lp_r - q * self.bp_r;
            self.bp_r += g * hp_r;
            self.lp_r += g * self.bp_r;

            if f_amt < 0.0 {
                let wet = (-f_amt).clamp(0.0, 1.0);
                l = l * (1.0 - wet) + self.lp_l * wet;
                r = r * (1.0 - wet) + self.lp_r * wet;
            } else {
                let wet = f_amt.clamp(0.0, 1.0);
                l = l * (1.0 - wet) + hp_l * wet;
                r = r * (1.0 - wet) + hp_r * wet;
            }
        }

        (l, r)
    }
}

struct SampleHit {
    slot: u32,
    age_sec: f32,
    active: bool,
}

impl SampleHit {
    fn tick(&mut self, sr: f32) -> f32 {
        if !self.active {
            return 0.0;
        }
        let t = self.age_sec;
        self.age_sec += 1.0 / sr.max(8000.0);
        let dur = match self.slot {
            0 => 0.65, // Air Horn
            1 => 0.32, // Laser Zap
            2 => 0.85, // Sub Drop
            3 => 0.90, // Riser Sweep
            4 => 0.22, // Club Clap
            5 => 0.12, // Rim Shot
            6 => 0.75, // Dub Siren
            _ => 0.18, // Vinyl Chirp
        };
        if t >= dur {
            self.active = false;
            return 0.0;
        }
        let env = (1.0 - t / dur).powf(1.5) * (t / 0.004).min(1.0);
        let sig = match self.slot {
            0 => {
                (TAU * 466.0 * t).sin() * 0.5
                    + (TAU * 587.3 * t).sin() * 0.4
                    + (TAU * 932.0 * t).sin() * 0.2
            }
            1 => (TAU * (1800.0 * (-t * 10.0).exp() + 120.0) * t).sin(),
            2 => (TAU * (110.0 * (-t * 2.8).exp() + 36.0) * t).sin(),
            3 => (TAU * (240.0 + t * 1800.0) * t).sin() * 0.7,
            4 => (((t * 17_321.0).sin() * 43_758.5).fract() * 2.0 - 1.0) * (-t * 16.0).exp(),
            5 => (TAU * 920.0 * t).sin() * (-t * 32.0).exp(),
            6 => (TAU * (640.0 + 220.0 * (TAU * 6.5 * t).sin()) * t).sin(),
            _ => (TAU * (520.0 + 980.0 * (t / dur)) * t).sin(),
        };
        sig * env * 0.55
    }
}

/// Opaque WASM DJ Engine handle owning 2 decks, 16-step beatmaker, 8-slot sampler,
/// and pre-allocated stereo linear-memory buffers.
pub struct WasmDjEngine {
    sr: f32,
    deck_a: DeckSynth,
    deck_b: DeckSynth,
    crossfader: f32,
    curve: u32,
    seq_on: bool,
    seq_genre: u32,
    seq_phase: f64,
    last_step: u32,
    seq_kick_age: f32,
    seq_snare_age: f32,
    seq_hat_age: f32,
    samples: [SampleHit; 8],
    left: Vec<f32>,
    right: Vec<f32>,
}

impl WasmDjEngine {
    fn crossfader_gains(&self) -> (f32, f32) {
        let t = ((self.crossfader.clamp(-1.0, 1.0) + 1.0) * 0.5).clamp(0.0, 1.0);
        match self.curve {
            1 => (1.0 - t, t), // Linear
            2 => {
                // Scratch Cut
                let ga = if t < 0.9 {
                    1.0
                } else {
                    ((1.0 - t) / 0.1).clamp(0.0, 1.0)
                };
                let gb = if t > 0.1 {
                    1.0
                } else {
                    (t / 0.1).clamp(0.0, 1.0)
                };
                (ga, gb)
            }
            _ => {
                let a = t * std::f32::consts::FRAC_PI_2;
                (a.cos(), a.sin())
            }
        }
    }

    fn tick_sequencer(&mut self) -> f32 {
        if !self.seq_on {
            return 0.0;
        }
        let dt = 1.0 / self.sr.max(8000.0);
        let bpm = self.deck_a.bpm.clamp(60.0, 200.0);
        let step_sec = 60.0 / bpm / 4.0;
        self.seq_phase += dt as f64;
        let step = ((self.seq_phase as f32 / step_sec).floor() as u32) % 16;
        if step != self.last_step {
            self.last_step = step;
            let (k_mask, s_mask, h_mask): (u16, u16, u16) = match self.seq_genre % 6 {
                0 => (
                    0b0001_0001_0001_0001,
                    0b0001_0000_0001_0000,
                    0b0100_0100_0100_0100,
                ), // House
                1 => (
                    0b0001_0001_0001_0001,
                    0b0000_0000_0001_0000,
                    0b1111_1111_1111_1111,
                ), // Techno
                2 => (
                    0b0000_0100_1000_0001,
                    0b0001_0000_0001_0000,
                    0b0101_0101_0101_0101,
                ), // HipHop
                3 => (
                    0b0000_0100_0010_0001,
                    0b0000_0001_0000_0000,
                    0b1111_0101_1111_0101,
                ), // Trap
                4 => (
                    0b0000_0100_0000_0001,
                    0b0001_0000_0001_0000,
                    0b0101_0101_0101_0101,
                ), // DnB
                _ => (
                    0b0001_0001_0001_0001,
                    0b0100_1000_0100_1000,
                    0b0101_0101_0101_0101,
                ), // Reggaeton
            };
            let bit = 1u16 << step;
            if (k_mask & bit) != 0 {
                self.seq_kick_age = 0.0;
            }
            if (s_mask & bit) != 0 {
                self.seq_snare_age = 0.0;
            }
            if (h_mask & bit) != 0 {
                self.seq_hat_age = 0.0;
            }
        }

        let mut out = 0.0_f32;
        if self.seq_kick_age < 0.25 {
            let t = self.seq_kick_age;
            out += (TAU * (52.0 + 130.0 * (-t * 32.0).exp()) * t).sin() * (-t * 16.0).exp() * 0.55;
            self.seq_kick_age += dt;
        }
        if self.seq_snare_age < 0.18 {
            let t = self.seq_snare_age;
            let n = ((t * 19_123.0).sin() * 43_758.5).fract() * 2.0 - 1.0;
            out += (n * 0.6 + (TAU * 210.0 * t).sin() * 0.4) * (-t * 22.0).exp() * 0.40;
            self.seq_snare_age += dt;
        }
        if self.seq_hat_age < 0.07 {
            let t = self.seq_hat_age;
            let n = ((t * 27_917.0).sin() * 43_758.5).fract() * 2.0 - 1.0;
            out += n * (-t * 48.0).exp() * 0.22;
            self.seq_hat_age += dt;
        }
        out
    }
}

/// Create a new DJCraft WASM engine with pre-allocated stereo buffers.
#[no_mangle]
pub extern "C" fn djw_create(sample_rate: f32, block_frames: u32) -> *mut WasmDjEngine {
    let cap = (block_frames as usize).clamp(16, 4096);
    Box::into_raw(Box::new(WasmDjEngine {
        sr: sample_rate.max(8000.0),
        deck_a: DeckSynth::new(126.0, 55.0),  // A1 (8A)
        deck_b: DeckSynth::new(126.0, 65.41), // C2 (8B)
        crossfader: 0.0,
        curve: 0,
        seq_on: false,
        seq_genre: 0,
        seq_phase: 0.0,
        last_step: 99,
        seq_kick_age: 1.0,
        seq_snare_age: 1.0,
        seq_hat_age: 1.0,
        samples: core::array::from_fn(|i| SampleHit {
            slot: i as u32,
            age_sec: 0.0,
            active: false,
        }),
        left: vec![0.0; cap],
        right: vec![0.0; cap],
    }))
}

/// Free a [`WasmDjEngine`] created by [`djw_create`].
///
/// # Safety
/// `ptr` must be null or a pointer returned by [`djw_create`] that has not been freed yet.
#[no_mangle]
pub unsafe extern "C" fn djw_destroy(ptr: *mut WasmDjEngine) {
    if !ptr.is_null() {
        // SAFETY: Caller guarantees `ptr` came from `Box::into_raw` in `djw_create`.
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

/// Pointer to left-channel render buffer in WASM linear memory.
///
/// # Safety
/// `ptr` must be a valid non-null `WasmDjEngine` handle.
#[no_mangle]
pub unsafe extern "C" fn djw_left_ptr(ptr: *mut WasmDjEngine) -> *const f32 {
    if ptr.is_null() {
        return core::ptr::null();
    }
    // SAFETY: Caller guarantees `ptr` is valid.
    unsafe { (*ptr).left.as_ptr() }
}

/// Pointer to right-channel render buffer in WASM linear memory.
///
/// # Safety
/// `ptr` must be a valid non-null `WasmDjEngine` handle.
#[no_mangle]
pub unsafe extern "C" fn djw_right_ptr(ptr: *mut WasmDjEngine) -> *const f32 {
    if ptr.is_null() {
        return core::ptr::null();
    }
    // SAFETY: Caller guarantees `ptr` is valid.
    unsafe { (*ptr).right.as_ptr() }
}

/// Toggle playback on deck `0` (A) or `1` (B).
///
/// # Safety
/// `ptr` must be a valid non-null `WasmDjEngine` handle.
#[no_mangle]
pub unsafe extern "C" fn djw_set_deck_play(ptr: *mut WasmDjEngine, deck: u32, playing: u32) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: Caller guarantees `ptr` is valid.
    let eng = unsafe { &mut *ptr };
    if deck == 0 {
        eng.deck_a.playing = playing != 0;
    } else {
        eng.deck_b.playing = playing != 0;
    }
}

/// Set deck BPM (`60.0..=200.0`).
///
/// # Safety
/// `ptr` must be a valid non-null `WasmDjEngine` handle.
#[no_mangle]
pub unsafe extern "C" fn djw_set_deck_bpm(ptr: *mut WasmDjEngine, deck: u32, bpm: f32) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: Caller guarantees `ptr` is valid.
    let eng = unsafe { &mut *ptr };
    let b = bpm.clamp(60.0, 200.0);
    if deck == 0 {
        eng.deck_a.bpm = b;
    } else {
        eng.deck_b.bpm = b;
    }
}

/// Set deck volume (`0.0..=1.0`).
///
/// # Safety
/// `ptr` must be a valid non-null `WasmDjEngine` handle.
#[no_mangle]
pub unsafe extern "C" fn djw_set_deck_vol(ptr: *mut WasmDjEngine, deck: u32, vol: f32) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: Caller guarantees `ptr` is valid.
    let eng = unsafe { &mut *ptr };
    let v = vol.clamp(0.0, 1.0);
    if deck == 0 {
        eng.deck_a.volume = v;
    } else {
        eng.deck_b.volume = v;
    }
}

/// Set deck bipolar color filter (`-1.0..=1.0`).
///
/// # Safety
/// `ptr` must be a valid non-null `WasmDjEngine` handle.
#[no_mangle]
pub unsafe extern "C" fn djw_set_deck_filter(ptr: *mut WasmDjEngine, deck: u32, flt: f32) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: Caller guarantees `ptr` is valid.
    let eng = unsafe { &mut *ptr };
    let f = flt.clamp(-1.0, 1.0);
    if deck == 0 {
        eng.deck_a.filter = f;
    } else {
        eng.deck_b.filter = f;
    }
}

/// Set battle crossfader (`-1.0` Deck A .. `+1.0` Deck B) and curve (`0`=Smooth, `1`=Linear, `2`=Cut).
///
/// # Safety
/// `ptr` must be a valid non-null `WasmDjEngine` handle.
#[no_mangle]
pub unsafe extern "C" fn djw_set_crossfader(ptr: *mut WasmDjEngine, val: f32, curve: u32) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: Caller guarantees `ptr` is valid.
    let eng = unsafe { &mut *ptr };
    eng.crossfader = val.clamp(-1.0, 1.0);
    eng.curve = curve.min(2);
}

/// Trigger one of the 8 built-in club sampler pads (`0..=7`).
///
/// # Safety
/// `ptr` must be a valid non-null `WasmDjEngine` handle.
#[no_mangle]
pub unsafe extern "C" fn djw_trigger_sample(ptr: *mut WasmDjEngine, slot: u32) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: Caller guarantees `ptr` is valid.
    let eng = unsafe { &mut *ptr };
    let idx = (slot as usize) % 8;
    eng.samples[idx].age_sec = 0.0;
    eng.samples[idx].active = true;
}

/// Toggle 16-step Studio Beatmaker and set genre (`0..=5`).
///
/// # Safety
/// `ptr` must be a valid non-null `WasmDjEngine` handle.
#[no_mangle]
pub unsafe extern "C" fn djw_set_sequencer(ptr: *mut WasmDjEngine, enabled: u32, genre: u32) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: Caller guarantees `ptr` is valid.
    let eng = unsafe { &mut *ptr };
    eng.seq_on = enabled != 0;
    eng.seq_genre = genre % 6;
}

/// Render `frames` stereo samples into the internal left/right buffers.
///
/// # Safety
/// `ptr` must be a valid non-null `WasmDjEngine` handle.
#[no_mangle]
pub unsafe extern "C" fn djw_process(ptr: *mut WasmDjEngine, frames: u32) -> u32 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: Caller guarantees `ptr` is valid.
    let eng = unsafe { &mut *ptr };
    let n = (frames as usize).min(eng.left.len());
    let (ga, gb) = eng.crossfader_gains();
    let sr = eng.sr;
    for i in 0..n {
        let (al, ar) = eng.deck_a.tick(sr);
        let (bl, br) = eng.deck_b.tick(sr);
        let seq = eng.tick_sequencer();
        let mut samp = 0.0_f32;
        for s in &mut eng.samples {
            samp += s.tick(sr);
        }
        eng.left[i] = (al * ga + bl * gb + seq + samp).tanh();
        eng.right[i] = (ar * ga + br * gb + seq + samp).tanh();
    }
    n as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_dj_engine_renders_finite_stereo_decks_sequencer_and_sampler() {
        let ptr = djw_create(44100.0, 256);
        assert!(!ptr.is_null());
        // SAFETY: `ptr` was just allocated by `djw_create`.
        unsafe {
            djw_set_deck_play(ptr, 0, 1);
            djw_set_deck_bpm(ptr, 0, 128.0);
            djw_set_deck_filter(ptr, 0, -0.35);
            djw_set_crossfader(ptr, 0.0, 2);
            djw_set_sequencer(ptr, 1, 1);
            djw_trigger_sample(ptr, 0);
            let n = djw_process(ptr, 256);
            assert_eq!(n, 256);
            let lp = djw_left_ptr(ptr);
            let rp = djw_right_ptr(ptr);
            let ls = core::slice::from_raw_parts(lp, 256);
            let rs = core::slice::from_raw_parts(rp, 256);
            assert!(ls.iter().all(|v| v.is_finite()));
            assert!(rs.iter().all(|v| v.is_finite()));
            assert!(ls.iter().map(|v| v.abs()).fold(0.0_f32, f32::max) > 0.05);
            djw_destroy(ptr);
        }
    }
}

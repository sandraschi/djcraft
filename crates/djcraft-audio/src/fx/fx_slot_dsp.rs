//! Per-slot realtime state for one Multi-FX slot.
//!
//! Delay, modulation, reverb, crush, brake, and stutter memory live here so
//! chained slots cannot overwrite each other.

use super::FxKind;

pub(super) const DELAY_BUF_LEN: usize = 96_000; // ~2.1s at 44.1 kHz
const MOD_BUF_LEN: usize = 4_096;
const REVERB_COMB_LENS: [usize; 4] = [1557, 1617, 1491, 1422];

pub(super) struct FxSlotRuntime {
    sample_rate: f32,
    delay_l: Vec<f32>,
    delay_r: Vec<f32>,
    delay_pos: usize,
    dub_lp_l: f32,
    dub_lp_r: f32,
    mod_l: Vec<f32>,
    mod_r: Vec<f32>,
    mod_pos: usize,
    lfo_phase: f32,
    comb_l: [Vec<f32>; 4],
    comb_r: [Vec<f32>; 4],
    comb_pos: [usize; 4],
    phaser_ap_l: [f32; 4],
    phaser_ap_r: [f32; 4],
    crush_counter: u32,
    crush_hold: (f32, f32),
    brake_buf: Vec<(f32, f32)>,
    brake_write: usize,
    brake_read: f64,
    brake_speed: f64,
    brake_was_active: bool,
    stutter_pos: usize,
}

impl FxSlotRuntime {
    pub(super) fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate.max(8000) as f32,
            delay_l: vec![0.0; DELAY_BUF_LEN],
            delay_r: vec![0.0; DELAY_BUF_LEN],
            delay_pos: 0,
            dub_lp_l: 0.0,
            dub_lp_r: 0.0,
            mod_l: vec![0.0; MOD_BUF_LEN],
            mod_r: vec![0.0; MOD_BUF_LEN],
            mod_pos: 0,
            lfo_phase: 0.0,
            comb_l: [
                vec![0.0; REVERB_COMB_LENS[0]],
                vec![0.0; REVERB_COMB_LENS[1]],
                vec![0.0; REVERB_COMB_LENS[2]],
                vec![0.0; REVERB_COMB_LENS[3]],
            ],
            comb_r: [
                vec![0.0; REVERB_COMB_LENS[0] + 23],
                vec![0.0; REVERB_COMB_LENS[1] + 23],
                vec![0.0; REVERB_COMB_LENS[2] + 23],
                vec![0.0; REVERB_COMB_LENS[3] + 23],
            ],
            comb_pos: [0; 4],
            phaser_ap_l: [0.0; 4],
            phaser_ap_r: [0.0; 4],
            crush_counter: 0,
            crush_hold: (0.0, 0.0),
            brake_buf: vec![(0.0, 0.0); DELAY_BUF_LEN],
            brake_write: 0,
            brake_read: 0.0,
            brake_speed: 1.0,
            brake_was_active: false,
            stutter_pos: 0,
        }
    }

    pub(super) fn release_brake(&mut self) {
        self.brake_was_active = false;
        self.brake_speed = 1.0;
    }

    /// Process one stereo frame. `fx_sec` is the BPM-locked time base for this unit.
    pub(super) fn apply(
        &mut self,
        kind: FxKind,
        mut l: f32,
        mut r: f32,
        d: f32,
        delay_samples: usize,
        fx_sec: f32,
    ) -> (f32, f32) {
        if kind != FxKind::VinylBrake {
            self.release_brake();
        }
        self.lfo_phase = (self.lfo_phase + 1.0 / (fx_sec * self.sample_rate).max(1.0)) % 1.0;

        match kind {
            FxKind::BpmEcho => {
                let read_idx = (self.delay_pos + DELAY_BUF_LEN - delay_samples) % DELAY_BUF_LEN;
                let dl = self.delay_l[read_idx];
                let dr = self.delay_r[read_idx];
                let fb = 0.25 + 0.55 * d;
                self.delay_l[self.delay_pos] = (l + dl * fb).clamp(-2.0, 2.0);
                self.delay_r[self.delay_pos] = (r + dr * fb).clamp(-2.0, 2.0);
                self.delay_pos = (self.delay_pos + 1) % DELAY_BUF_LEN;
                l += dl * d * 0.85;
                r += dr * d * 0.85;
            }
            FxKind::EchoOut => {
                let read_idx = (self.delay_pos + DELAY_BUF_LEN - delay_samples) % DELAY_BUF_LEN;
                let dl = self.delay_l[read_idx];
                let dr = self.delay_r[read_idx];
                let fb = 0.55 + 0.40 * d;
                let in_gain = (1.0 - d * 0.85).max(0.05);
                self.delay_l[self.delay_pos] = (l * in_gain + dl * fb).clamp(-2.0, 2.0);
                self.delay_r[self.delay_pos] = (r * in_gain + dr * fb).clamp(-2.0, 2.0);
                self.delay_pos = (self.delay_pos + 1) % DELAY_BUF_LEN;
                l = l * in_gain + dl * d;
                r = r * in_gain + dr * d;
            }
            FxKind::DubTapeDelay => {
                let read_idx = (self.delay_pos + DELAY_BUF_LEN - delay_samples) % DELAY_BUF_LEN;
                let dl = self.delay_l[read_idx];
                let dr = self.delay_r[read_idx];
                self.dub_lp_l += 0.18 * (dl - self.dub_lp_l);
                self.dub_lp_r += 0.18 * (dr - self.dub_lp_r);
                let fb = 0.35 + 0.5 * d;
                self.delay_l[self.delay_pos] = (l + (self.dub_lp_r * fb).tanh()).clamp(-2.0, 2.0);
                self.delay_r[self.delay_pos] = (r + (self.dub_lp_l * fb).tanh()).clamp(-2.0, 2.0);
                self.delay_pos = (self.delay_pos + 1) % DELAY_BUF_LEN;
                l += self.dub_lp_l * d * 0.9;
                r += self.dub_lp_r * d * 0.9;
            }
            FxKind::HallReverb => {
                let fb = 0.62 + 0.28 * d;
                let mut acc_l = 0.0_f32;
                let mut acc_r = 0.0_f32;
                for c in 0..4 {
                    let len_l = self.comb_l[c].len();
                    let len_r = self.comb_r[c].len();
                    let pl = self.comb_pos[c] % len_l;
                    let pr = self.comb_pos[c] % len_r;
                    let yl = self.comb_l[c][pl];
                    let yr = self.comb_r[c][pr];
                    self.comb_l[c][pl] = (l + yl * fb).clamp(-2.0, 2.0);
                    self.comb_r[c][pr] = (r + yr * fb).clamp(-2.0, 2.0);
                    self.comb_pos[c] = self.comb_pos[c].wrapping_add(1);
                    acc_l += yl;
                    acc_r += yr;
                }
                l = l * (1.0 - 0.35 * d) + acc_l * 0.25 * d;
                r = r * (1.0 - 0.35 * d) + acc_r * 0.25 * d;
            }
            FxKind::Flanger => {
                self.mod_l[self.mod_pos] = l;
                self.mod_r[self.mod_pos] = r;
                let lfo = (self.lfo_phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                let tap = (12.0 + lfo * 220.0 * d) as usize;
                let idx = (self.mod_pos + MOD_BUF_LEN - tap) % MOD_BUF_LEN;
                let fl = self.mod_l[idx];
                let fr = self.mod_r[idx];
                self.mod_l[self.mod_pos] = (l + fl * 0.45 * d).clamp(-2.0, 2.0);
                self.mod_r[self.mod_pos] = (r + fr * 0.45 * d).clamp(-2.0, 2.0);
                self.mod_pos = (self.mod_pos + 1) % MOD_BUF_LEN;
                l = (l + fl * d * 0.7) * 0.75;
                r = (r + fr * d * 0.7) * 0.75;
            }
            FxKind::Phaser => {
                let lfo = (self.lfo_phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                let a = 0.25 + 0.65 * lfo;
                let mut xl = l;
                let mut xr = r;
                for s in 0..4 {
                    let yl = -a * xl + self.phaser_ap_l[s];
                    self.phaser_ap_l[s] = xl + a * yl;
                    xl = yl;

                    let yr = -a * xr + self.phaser_ap_r[s];
                    self.phaser_ap_r[s] = xr + a * yr;
                    xr = yr;
                }
                l = l * (1.0 - 0.5 * d) + xl * 0.5 * d;
                r = r * (1.0 - 0.5 * d) + xr * 0.5 * d;
            }
            FxKind::Chorus => {
                self.mod_l[self.mod_pos] = l;
                self.mod_r[self.mod_pos] = r;
                let lfo_l = (self.lfo_phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                let lfo_r = ((self.lfo_phase + 0.25) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                let tap_l = (320.0 + lfo_l * 450.0 * d) as usize;
                let tap_r = (320.0 + lfo_r * 450.0 * d) as usize;
                let cl = self.mod_l[(self.mod_pos + MOD_BUF_LEN - tap_l) % MOD_BUF_LEN];
                let cr = self.mod_r[(self.mod_pos + MOD_BUF_LEN - tap_r) % MOD_BUF_LEN];
                self.mod_pos = (self.mod_pos + 1) % MOD_BUF_LEN;
                l = l * (1.0 - 0.35 * d) + cl * 0.65 * d;
                r = r * (1.0 - 0.35 * d) + cr * 0.65 * d;
            }
            FxKind::AutoGate => {
                let gate = if self.lfo_phase < 0.5 { 1.0 } else { 1.0 - d };
                l *= gate;
                r *= gate;
            }
            FxKind::Bitcrusher => {
                let hold_n = (1.0 + d * 18.0) as u32;
                if self.crush_counter == 0 {
                    let levels = (2.0_f32).powf(12.0 - d * 9.0).max(4.0);
                    let ql = (l * levels).round() / levels;
                    let qr = (r * levels).round() / levels;
                    self.crush_hold = (ql, qr);
                }
                self.crush_counter = (self.crush_counter + 1) % hold_n.max(1);
                l = self.crush_hold.0;
                r = self.crush_hold.1;
            }
            FxKind::TapeOverdrive => {
                let drive = 1.0 + d * 8.0;
                l = ((l * drive).tanh()) / drive.sqrt();
                r = ((r * drive).tanh()) / drive.sqrt();
            }
            FxKind::VinylBrake => {
                self.brake_buf[self.brake_write] = (l, r);
                if !self.brake_was_active {
                    self.brake_read = self.brake_write as f64;
                    self.brake_speed = 1.0;
                    self.brake_was_active = true;
                }
                let decel = (1.0 / (fx_sec as f64 * self.sample_rate as f64)).max(1e-6);
                self.brake_speed = (self.brake_speed - decel * (0.5 + d as f64)).max(0.0);
                let idx = (self.brake_read as usize) % DELAY_BUF_LEN;
                let (bl, br) = self.brake_buf[idx];
                self.brake_read = (self.brake_read + self.brake_speed) % DELAY_BUF_LEN as f64;
                self.brake_write = (self.brake_write + 1) % DELAY_BUF_LEN;
                let gain = (self.brake_speed as f32).sqrt();
                l = bl * gain;
                r = br * gain;
            }
            FxKind::BeatStutter => {
                let loop_len = delay_samples.clamp(512, DELAY_BUF_LEN / 2);
                if self.stutter_pos < loop_len {
                    self.delay_l[self.stutter_pos] = l;
                    self.delay_r[self.stutter_pos] = r;
                }
                let read_i = self.stutter_pos % loop_len;
                l = l * (1.0 - d) + self.delay_l[read_i] * d;
                r = r * (1.0 - d) + self.delay_r[read_i] * d;
                self.stutter_pos = (self.stutter_pos + 1) % (loop_len * 4);
            }
        }
        (l, r)
    }
}

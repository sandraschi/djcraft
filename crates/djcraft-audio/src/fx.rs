#![forbid(unsafe_code)]
//! Real-time DJ Color Filter, Dual 3-Slot BPM-Synced Multi-FX Rack, and Free FX Packs.
//!
//! All DSP state is pre-allocated on construction so `process_stereo` performs zero
//! heap allocations on the real-time WASAPI audio callback thread.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::DeckId;

/// Curated free built-in FX packs (zero payola, 100% local Safe Rust DSP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FxPackId {
    ClubCore,
    Modulation,
    BattleLoFi,
}

impl FxPackId {
    pub const ALL: [Self; 3] = [Self::ClubCore, Self::Modulation, Self::BattleLoFi];

    pub fn label(self) -> &'static str {
        match self {
            Self::ClubCore => "Club Core Pack",
            Self::Modulation => "Modulation Pack",
            Self::BattleLoFi => "Battle & Lo-Fi Pack",
        }
    }
}

/// 12 real-time stereo DJ DSP effects across 3 built-in packs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FxKind {
    // Club Core Pack
    #[default]
    BpmEcho,
    EchoOut,
    HallReverb,
    DubTapeDelay,
    // Modulation Pack
    Flanger,
    Phaser,
    Chorus,
    AutoGate,
    // Battle & Lo-Fi Pack
    Bitcrusher,
    TapeOverdrive,
    VinylBrake,
    BeatStutter,
}

impl FxKind {
    pub const ALL: [Self; 12] = [
        Self::BpmEcho,
        Self::EchoOut,
        Self::HallReverb,
        Self::DubTapeDelay,
        Self::Flanger,
        Self::Phaser,
        Self::Chorus,
        Self::AutoGate,
        Self::Bitcrusher,
        Self::TapeOverdrive,
        Self::VinylBrake,
        Self::BeatStutter,
    ];

    pub fn pack(self) -> FxPackId {
        match self {
            Self::BpmEcho | Self::EchoOut | Self::HallReverb | Self::DubTapeDelay => {
                FxPackId::ClubCore
            }
            Self::Flanger | Self::Phaser | Self::Chorus | Self::AutoGate => FxPackId::Modulation,
            Self::Bitcrusher | Self::TapeOverdrive | Self::VinylBrake | Self::BeatStutter => {
                FxPackId::BattleLoFi
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::BpmEcho => "BPM Echo",
            Self::EchoOut => "Echo Out (Freeze)",
            Self::HallReverb => "Hall Reverb",
            Self::DubTapeDelay => "Dub Tape Delay",
            Self::Flanger => "Flanger",
            Self::Phaser => "Phaser",
            Self::Chorus => "Stereo Chorus",
            Self::AutoGate => "Auto-Gate / Tremolo",
            Self::Bitcrusher => "Bitcrusher",
            Self::TapeOverdrive => "Tape Overdrive",
            Self::VinylBrake => "Vinyl Brake",
            Self::BeatStutter => "Beat Stutter",
        }
    }
}

/// Audio routing target for an FX unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FxTarget {
    #[default]
    DeckA,
    DeckB,
    DeckC,
    DeckD,
    Master,
}

impl FxTarget {
    pub const ALL: [Self; 5] = [
        Self::DeckA,
        Self::DeckB,
        Self::DeckC,
        Self::DeckD,
        Self::Master,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::DeckA => "Deck A",
            Self::DeckB => "Deck B",
            Self::DeckC => "Deck C",
            Self::DeckD => "Deck D",
            Self::Master => "Master",
        }
    }

    pub fn matches_deck(self, deck: DeckId) -> bool {
        matches!(
            (self, deck),
            (Self::DeckA, DeckId::DeckA)
                | (Self::DeckB, DeckId::DeckB)
                | (Self::DeckC, DeckId::DeckC)
                | (Self::DeckD, DeckId::DeckD)
        )
    }
}

/// Stem-targeted FX routing (applies FX to full deck or isolated Vocal/Drum stems).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FxStemScope {
    #[default]
    FullDeck,
    VocalsOnly,
    DrumsOnly,
}

impl FxStemScope {
    pub const ALL: [Self; 3] = [Self::FullDeck, Self::VocalsOnly, Self::DrumsOnly];

    pub fn label(self) -> &'static str {
        match self {
            Self::FullDeck => "Full Deck",
            Self::VocalsOnly => "Vocals Only",
            Self::DrumsOnly => "Drums Only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FxSlotParams {
    pub kind: FxKind,
    pub enabled: bool,
    pub depth: f64, // 0.0..=1.0
}

impl Default for FxSlotParams {
    fn default() -> Self {
        Self {
            kind: FxKind::BpmEcho,
            enabled: false,
            depth: 0.55,
        }
    }
}

/// Serializable state for one 3-slot Multi-FX unit (FX 1 or FX 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxUnitParams {
    pub enabled: bool,
    pub target: FxTarget,
    pub stem_scope: FxStemScope,
    pub beats: f64,   // 0.125, 0.25, 0.5, 1.0, 2.0, 4.0
    pub wet_dry: f64, // 0.0..=1.0
    pub slots: [FxSlotParams; 3],
}

impl FxUnitParams {
    pub fn default_unit(idx: usize) -> Self {
        if idx == 0 {
            Self {
                enabled: false,
                target: FxTarget::DeckA,
                stem_scope: FxStemScope::FullDeck,
                beats: 0.5,
                wet_dry: 0.65,
                slots: [
                    FxSlotParams {
                        kind: FxKind::BpmEcho,
                        enabled: true,
                        depth: 0.55,
                    },
                    FxSlotParams {
                        kind: FxKind::HallReverb,
                        enabled: false,
                        depth: 0.45,
                    },
                    FxSlotParams {
                        kind: FxKind::Flanger,
                        enabled: false,
                        depth: 0.5,
                    },
                ],
            }
        } else {
            Self {
                enabled: false,
                target: FxTarget::DeckB,
                stem_scope: FxStemScope::FullDeck,
                beats: 0.25,
                wet_dry: 0.65,
                slots: [
                    FxSlotParams {
                        kind: FxKind::BeatStutter,
                        enabled: true,
                        depth: 0.6,
                    },
                    FxSlotParams {
                        kind: FxKind::Bitcrusher,
                        enabled: false,
                        depth: 0.45,
                    },
                    FxSlotParams {
                        kind: FxKind::VinylBrake,
                        enabled: false,
                        depth: 0.7,
                    },
                ],
            }
        }
    }

    pub fn clamped(mut self) -> Self {
        self.beats = self.beats.clamp(0.0625, 8.0);
        self.wet_dry = self.wet_dry.clamp(0.0, 1.0);
        for slot in &mut self.slots {
            slot.depth = slot.depth.clamp(0.0, 1.0);
        }
        self
    }
}

/// Named 3-slot Multi-FX chain preset (built-in or user-saved JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FxPackPreset {
    pub name: String,
    pub pack: FxPackId,
    pub beats: f64,
    pub wet_dry: f64,
    pub stem_scope: FxStemScope,
    pub slots: [FxSlotParams; 3],
}

/// Built-in free FX Pack presets covering Club, Modulation, and Battle/Lo-Fi workflows.
pub fn builtin_fx_presets() -> Vec<FxPackPreset> {
    vec![
        FxPackPreset {
            name: "Club Echo Out".to_string(),
            pack: FxPackId::ClubCore,
            beats: 0.5,
            wet_dry: 0.75,
            stem_scope: FxStemScope::FullDeck,
            slots: [
                FxSlotParams {
                    kind: FxKind::EchoOut,
                    enabled: true,
                    depth: 0.72,
                },
                FxSlotParams {
                    kind: FxKind::HallReverb,
                    enabled: true,
                    depth: 0.35,
                },
                FxSlotParams {
                    kind: FxKind::DubTapeDelay,
                    enabled: false,
                    depth: 0.4,
                },
            ],
        },
        FxPackPreset {
            name: "Vocal Dub Wash".to_string(),
            pack: FxPackId::ClubCore,
            beats: 0.75,
            wet_dry: 0.8,
            stem_scope: FxStemScope::VocalsOnly,
            slots: [
                FxSlotParams {
                    kind: FxKind::DubTapeDelay,
                    enabled: true,
                    depth: 0.65,
                },
                FxSlotParams {
                    kind: FxKind::HallReverb,
                    enabled: true,
                    depth: 0.55,
                },
                FxSlotParams {
                    kind: FxKind::BpmEcho,
                    enabled: false,
                    depth: 0.4,
                },
            ],
        },
        FxPackPreset {
            name: "Jet Flanger Sweep".to_string(),
            pack: FxPackId::Modulation,
            beats: 4.0,
            wet_dry: 0.7,
            stem_scope: FxStemScope::FullDeck,
            slots: [
                FxSlotParams {
                    kind: FxKind::Flanger,
                    enabled: true,
                    depth: 0.75,
                },
                FxSlotParams {
                    kind: FxKind::Phaser,
                    enabled: true,
                    depth: 0.4,
                },
                FxSlotParams {
                    kind: FxKind::Chorus,
                    enabled: false,
                    depth: 0.35,
                },
            ],
        },
        FxPackPreset {
            name: "Trance Auto-Gate".to_string(),
            pack: FxPackId::Modulation,
            beats: 0.25,
            wet_dry: 0.85,
            stem_scope: FxStemScope::FullDeck,
            slots: [
                FxSlotParams {
                    kind: FxKind::AutoGate,
                    enabled: true,
                    depth: 0.85,
                },
                FxSlotParams {
                    kind: FxKind::Chorus,
                    enabled: true,
                    depth: 0.4,
                },
                FxSlotParams {
                    kind: FxKind::HallReverb,
                    enabled: true,
                    depth: 0.3,
                },
            ],
        },
        FxPackPreset {
            name: "8-Bit Crush & Drive".to_string(),
            pack: FxPackId::BattleLoFi,
            beats: 0.5,
            wet_dry: 0.75,
            stem_scope: FxStemScope::FullDeck,
            slots: [
                FxSlotParams {
                    kind: FxKind::Bitcrusher,
                    enabled: true,
                    depth: 0.65,
                },
                FxSlotParams {
                    kind: FxKind::TapeOverdrive,
                    enabled: true,
                    depth: 0.5,
                },
                FxSlotParams {
                    kind: FxKind::BpmEcho,
                    enabled: false,
                    depth: 0.4,
                },
            ],
        },
        FxPackPreset {
            name: "Battle Brake & Stutter".to_string(),
            pack: FxPackId::BattleLoFi,
            beats: 0.25,
            wet_dry: 0.9,
            stem_scope: FxStemScope::FullDeck,
            slots: [
                FxSlotParams {
                    kind: FxKind::BeatStutter,
                    enabled: true,
                    depth: 0.8,
                },
                FxSlotParams {
                    kind: FxKind::VinylBrake,
                    enabled: false,
                    depth: 0.75,
                },
                FxSlotParams {
                    kind: FxKind::TapeOverdrive,
                    enabled: true,
                    depth: 0.35,
                },
            ],
        },
    ]
}

pub fn fx_packs_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("djcraft")
        .join("fx_packs")
}

pub fn save_fx_preset_to_dir(dir: &std::path::Path, preset: &FxPackPreset) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create fx_packs dir: {e}"))?;
    let safe_name: String = preset
        .name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let file_name = if safe_name.is_empty() {
        "custom_fx.json".to_string()
    } else {
        format!("{safe_name}.json")
    };
    let path = dir.join(file_name);
    let json = serde_json::to_string_pretty(preset)
        .map_err(|e| format!("Failed to serialize FX preset: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write FX preset: {e}"))?;
    Ok(path)
}

pub fn load_user_fx_presets_from_dir(dir: &std::path::Path) -> Vec<FxPackPreset> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Ok(raw) = std::fs::read_to_string(&p) {
                if let Ok(preset) = serde_json::from_str::<FxPackPreset>(&raw) {
                    out.push(preset);
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Per-deck stereo bipolar DJ Color Filter (`-1.0` = deep LPF, `0.0` = flat bypass, `+1.0` = sharp HPF).
/// Uses a clean Chamberlin state-variable filter with gentle club resonance.
#[derive(Debug, Clone)]
pub struct BipolarFilter {
    sample_rate: f32,
    lp_l: f32,
    bp_l: f32,
    lp_r: f32,
    bp_r: f32,
}

impl BipolarFilter {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate.max(8000) as f32,
            lp_l: 0.0,
            bp_l: 0.0,
            lp_r: 0.0,
            bp_r: 0.0,
        }
    }

    pub fn process_stereo(&mut self, l: f32, r: f32, amount: f64) -> (f32, f32) {
        let amt = amount.clamp(-1.0, 1.0) as f32;
        if amt.abs() < 0.015 {
            // Smoothly decay filter state while bypassed
            self.lp_l *= 0.9;
            self.bp_l *= 0.9;
            self.lp_r *= 0.9;
            self.bp_r *= 0.9;
            return (l, r);
        }

        let (cutoff_hz, is_hpf) = if amt < 0.0 {
            // -0.015 -> 16 kHz, -1.0 -> 140 Hz (exponential sweep)
            let t = (-amt).clamp(0.0, 1.0);
            let hz = 16000.0 * (140.0_f32 / 16000.0).powf(t);
            (hz, false)
        } else {
            // +0.015 -> 45 Hz, +1.0 -> 6500 Hz
            let t = amt.clamp(0.0, 1.0);
            let hz = 45.0 * (6500.0_f32 / 45.0).powf(t);
            (hz, true)
        };

        let f = (2.0 * (std::f32::consts::PI * cutoff_hz / self.sample_rate).sin()).clamp(0.01, 0.85);
        // Gentle club resonance (q = 1/Q; 0.55 gives musical peak without self-oscillation)
        let q = 0.62_f32;

        let hp_l = l - self.lp_l - q * self.bp_l;
        self.bp_l = (self.bp_l + f * hp_l).clamp(-4.0, 4.0);
        self.lp_l = (self.lp_l + f * self.bp_l).clamp(-4.0, 4.0);

        let hp_r = r - self.lp_r - q * self.bp_r;
        self.bp_r = (self.bp_r + f * hp_r).clamp(-4.0, 4.0);
        self.lp_r = (self.lp_r + f * self.bp_r).clamp(-4.0, 4.0);

        if is_hpf {
            (hp_l.clamp(-2.0, 2.0), hp_r.clamp(-2.0, 2.0))
        } else {
            (self.lp_l.clamp(-2.0, 2.0), self.lp_r.clamp(-2.0, 2.0))
        }
    }
}

const DELAY_BUF_LEN: usize = 96_000; // ~2.1s at 44.1 kHz
const MOD_BUF_LEN: usize = 4_096;
const REVERB_COMB_LENS: [usize; 4] = [1557, 1617, 1491, 1422];

/// Pre-allocated real-time stereo DSP engine for one 3-slot FX unit (FX 1 or FX 2).
pub struct FxUnitDsp {
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

impl FxUnitDsp {
    pub fn new(sample_rate: u32) -> Self {
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

    /// Process one stereo frame through up to 3 chained FX slots with BPM-locked timing.
    pub fn process_stereo(
        &mut self,
        dry_l: f32,
        dry_r: f32,
        params: &FxUnitParams,
        bpm: f64,
    ) -> (f32, f32) {
        if !params.enabled || params.wet_dry <= 0.001 {
            self.brake_was_active = false;
            self.brake_speed = 1.0;
            return (dry_l, dry_r);
        }

        let safe_bpm = bpm.clamp(60.0, 200.0) as f32;
        let beat_sec = 60.0 / safe_bpm;
        let fx_sec = (beat_sec * params.beats.clamp(0.0625, 8.0) as f32).clamp(0.02, 2.0);
        let delay_samples = ((fx_sec * self.sample_rate) as usize).clamp(32, DELAY_BUF_LEN - 2);

        self.lfo_phase = (self.lfo_phase + 1.0 / (fx_sec * self.sample_rate).max(1.0)) % 1.0;

        let mut l = dry_l;
        let mut r = dry_r;
        let mut brake_active_now = false;

        for slot in &params.slots {
            if !slot.enabled || slot.depth <= 0.001 {
                continue;
            }
            let d = slot.depth.clamp(0.0, 1.0) as f32;
            match slot.kind {
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
                    // High-feedback tail freeze: fades dry input out as depth approaches 1.0
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
                    // Warm low-pass + gentle tape saturation in feedback path
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
                    l = ((l * drive).tanh()) / (drive.sqrt());
                    r = ((r * drive).tanh()) / (drive.sqrt());
                }
                FxKind::VinylBrake => {
                    brake_active_now = true;
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
        }

        if !brake_active_now {
            self.brake_was_active = false;
            self.brake_speed = 1.0;
        }

        let wet = params.wet_dry.clamp(0.0, 1.0) as f32;
        let out_l = (dry_l * (1.0 - wet) + l * wet).clamp(-2.0, 2.0);
        let out_r = (dry_r * (1.0 - wet) + r * wet).clamp(-2.0, 2.0);
        (out_l, out_r)
    }
}

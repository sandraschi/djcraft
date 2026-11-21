#![forbid(unsafe_code)]
//! Real-time DJ Color Filter, Dual 3-Slot BPM-Synced Multi-FX Rack, and Free FX Packs.
//!
//! All DSP state is pre-allocated on construction so `process_stereo` performs zero
//! heap allocations on the real-time WASAPI audio callback thread.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::DeckId;

mod fx_slot_dsp;

use fx_slot_dsp::FxSlotRuntime;

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

pub fn save_fx_preset_to_dir(
    dir: &std::path::Path,
    preset: &FxPackPreset,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create fx_packs dir: {e}"))?;
    let safe_name: String = preset
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
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

        let f =
            (2.0 * (std::f32::consts::PI * cutoff_hz / self.sample_rate).sin()).clamp(0.01, 0.85);
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

/// Pre-allocated real-time stereo DSP engine for one 3-slot FX unit (FX 1 or FX 2).
/// Each slot owns its delay, modulation, reverb, and brake memory.
pub struct FxUnitDsp {
    sample_rate: f32,
    slots: [FxSlotRuntime; 3],
}

impl FxUnitDsp {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000);
        Self {
            sample_rate: sr as f32,
            slots: [
                FxSlotRuntime::new(sr),
                FxSlotRuntime::new(sr),
                FxSlotRuntime::new(sr),
            ],
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
            for slot in &mut self.slots {
                slot.release_brake();
            }
            return (dry_l, dry_r);
        }

        let safe_bpm = bpm.clamp(60.0, 200.0) as f32;
        let beat_sec = 60.0 / safe_bpm;
        let fx_sec = (beat_sec * params.beats.clamp(0.0625, 8.0) as f32).clamp(0.02, 2.0);
        let delay_samples =
            ((fx_sec * self.sample_rate) as usize).clamp(32, fx_slot_dsp::DELAY_BUF_LEN - 2);

        let mut l = dry_l;
        let mut r = dry_r;

        for (i, slot) in params.slots.iter().enumerate() {
            if !slot.enabled || slot.depth <= 0.001 {
                self.slots[i].release_brake();
                continue;
            }
            let d = slot.depth.clamp(0.0, 1.0) as f32;
            let (nl, nr) = self.slots[i].apply(slot.kind, l, r, d, delay_samples, fx_sec);
            l = nl;
            r = nr;
        }

        let wet = params.wet_dry.clamp(0.0, 1.0) as f32;
        let out_l = (dry_l * (1.0 - wet) + l * wet).clamp(-2.0, 2.0);
        let out_r = (dry_r * (1.0 - wet) + r * wet).clamp(-2.0, 2.0);
        (out_l, out_r)
    }
}

#[cfg(test)]
mod chain_tests {
    use super::*;

    fn impulse_windows(slots: [FxSlotParams; 3]) -> (f32, f32) {
        let mut dsp = FxUnitDsp::new(44100);
        let params = FxUnitParams {
            enabled: true,
            target: FxTarget::DeckA,
            stem_scope: FxStemScope::FullDeck,
            beats: 0.5,
            wet_dry: 1.0,
            slots,
        };
        let mut on_time = 0.0_f32;
        let mut half_time = 0.0_f32;
        for i in 0..20_000 {
            let s = if i == 0 { 0.8 } else { 0.0 };
            let (ol, _) = dsp.process_stereo(s, s, &params, 120.0);
            if (10_800..11_250).contains(&i) {
                on_time += ol.abs();
            }
            if (5_300..5_750).contains(&i) {
                half_time += ol.abs();
            }
        }
        (on_time, half_time)
    }

    fn echo_slot(kind: FxKind, enabled: bool) -> FxSlotParams {
        FxSlotParams {
            kind,
            enabled,
            depth: 1.0,
        }
    }

    #[test]
    fn chained_delay_slots_keep_independent_buffers() {
        let echo_only = impulse_windows([
            echo_slot(FxKind::BpmEcho, true),
            FxSlotParams::default(),
            FxSlotParams::default(),
        ]);
        let chained = impulse_windows([
            echo_slot(FxKind::BpmEcho, true),
            echo_slot(FxKind::DubTapeDelay, true),
            FxSlotParams::default(),
        ]);
        assert!(
            echo_only.0 > 0.2 && echo_only.0 > echo_only.1 * 4.0,
            "echo-only peak landed early: on={} half={}",
            echo_only.0,
            echo_only.1
        );
        assert!(
            chained.0 > 0.2 && chained.0 > chained.1 * 4.0,
            "chained echo shared a delay line: on={} half={}",
            chained.0,
            chained.1
        );
    }
}

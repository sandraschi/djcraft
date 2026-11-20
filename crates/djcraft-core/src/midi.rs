#![forbid(unsafe_code)]
//! Pure-Rust Hardware DJ Controller Engine (`midir` + MIDI Learn + Pro Profiles).
//!
//! Includes built-in mappings for:
//! - Rane Twelve MKII (motorized 12" deck controller, selectable Deck 1-4 channels)
//! - Rane Seventy-Two MKII (2-channel battle mixer: MAG FOUR crossfader, EQ, 8 RGB pads/side)
//! - Pioneer DDJ-FLX4 / DDJ-400
//! - Hercules DJControl
//!
//! Plus live in-app MIDI Learn persisted to `%LOCALAPPDATA%\djcraft\midi_learn.json`.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use djcraft_audio::DeckId;
use midir::{Ignore, MidiInput, MidiInputConnection};
use serde::{Deserialize, Serialize};

use crate::{Command, DjError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerProfileId {
    RaneTwelveMk2,
    RaneSeventyTwoMk2,
    PioneerDdjFlx4,
    HerculesDjControl,
    CustomLearn,
}

impl ControllerProfileId {
    pub const ALL: [Self; 5] = [
        Self::RaneTwelveMk2,
        Self::RaneSeventyTwoMk2,
        Self::PioneerDdjFlx4,
        Self::HerculesDjControl,
        Self::CustomLearn,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::RaneTwelveMk2 => "Rane Twelve MKII (Motorized Deck)",
            Self::RaneSeventyTwoMk2 => "Rane Seventy-Two MKII (Battle Mixer)",
            Self::PioneerDdjFlx4 => "Pioneer DDJ-FLX4 / DDJ-400",
            Self::HerculesDjControl => "Hercules DJControl",
            Self::CustomLearn => "Custom MIDI Learn",
        }
    }

    pub fn auto_detect_from_port_name(port_name: &str) -> Self {
        let lc = port_name.to_lowercase();
        if lc.contains("twelve") || lc.contains("rane 12") {
            Self::RaneTwelveMk2
        } else if lc.contains("seventy-two") || lc.contains("seventy two") || lc.contains("rane 72")
        {
            Self::RaneSeventyTwoMk2
        } else if lc.contains("ddj") || lc.contains("pioneer") || lc.contains("flx4") {
            Self::PioneerDdjFlx4
        } else if lc.contains("hercules") || lc.contains("djcontrol") {
            Self::HerculesDjControl
        } else {
            Self::CustomLearn
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MidiMsgKind {
    NoteOn,
    ControlChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MidiBindingKey {
    pub kind: MidiMsgKind,
    pub channel: u8, // 0..=15
    pub number: u8,  // note or CC 0..=127
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiTargetAction {
    PlayPause(DeckId),
    CueReturn(DeckId),
    HotCue(DeckId, u8),
    DeckVolume(DeckId),
    DeckPitch(DeckId),
    DeckEqLow(DeckId),
    DeckEqMid(DeckId),
    DeckEqHigh(DeckId),
    Crossfader,
    MasterVolume,
    AutoSync,
    PhaseSync,
    ToggleKeylock(DeckId),
}

impl MidiTargetAction {
    pub const LEARNABLE: [Self; 14] = [
        Self::PlayPause(DeckId::DeckA),
        Self::PlayPause(DeckId::DeckB),
        Self::CueReturn(DeckId::DeckA),
        Self::CueReturn(DeckId::DeckB),
        Self::DeckVolume(DeckId::DeckA),
        Self::DeckVolume(DeckId::DeckB),
        Self::DeckPitch(DeckId::DeckA),
        Self::DeckPitch(DeckId::DeckB),
        Self::DeckEqLow(DeckId::DeckA),
        Self::DeckEqLow(DeckId::DeckB),
        Self::Crossfader,
        Self::MasterVolume,
        Self::AutoSync,
        Self::PhaseSync,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::PlayPause(DeckId::DeckA) => "Deck A Play/Pause",
            Self::PlayPause(DeckId::DeckB) => "Deck B Play/Pause",
            Self::PlayPause(DeckId::DeckC) => "Deck C Play/Pause",
            Self::PlayPause(DeckId::DeckD) => "Deck D Play/Pause",
            Self::CueReturn(DeckId::DeckA) => "Deck A Cue",
            Self::CueReturn(DeckId::DeckB) => "Deck B Cue",
            Self::CueReturn(DeckId::DeckC) => "Deck C Cue",
            Self::CueReturn(DeckId::DeckD) => "Deck D Cue",
            Self::HotCue(DeckId::DeckA, _) => "Deck A Hot Cue",
            Self::HotCue(DeckId::DeckB, _) => "Deck B Hot Cue",
            Self::HotCue(DeckId::DeckC, _) => "Deck C Hot Cue",
            Self::HotCue(DeckId::DeckD, _) => "Deck D Hot Cue",
            Self::DeckVolume(DeckId::DeckA) => "Deck A Volume Fader",
            Self::DeckVolume(DeckId::DeckB) => "Deck B Volume Fader",
            Self::DeckVolume(DeckId::DeckC) => "Deck C Volume Fader",
            Self::DeckVolume(DeckId::DeckD) => "Deck D Volume Fader",
            Self::DeckPitch(DeckId::DeckA) => "Deck A Tempo/Pitch",
            Self::DeckPitch(DeckId::DeckB) => "Deck B Tempo/Pitch",
            Self::DeckPitch(DeckId::DeckC) => "Deck C Tempo/Pitch",
            Self::DeckPitch(DeckId::DeckD) => "Deck D Tempo/Pitch",
            Self::DeckEqLow(DeckId::DeckA) => "Deck A EQ Low",
            Self::DeckEqLow(DeckId::DeckB) => "Deck B EQ Low",
            Self::DeckEqLow(_) => "Deck EQ Low",
            Self::DeckEqMid(_) => "Deck EQ Mid",
            Self::DeckEqHigh(_) => "Deck EQ High",
            Self::Crossfader => "Master Crossfader",
            Self::MasterVolume => "Master Loudness",
            Self::AutoSync => "Auto-Sync BPM",
            Self::PhaseSync => "Phase Sync",
            Self::ToggleKeylock(_) => "Toggle Keylock",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiMappingRule {
    pub key: MidiBindingKey,
    pub action: MidiTargetAction,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RawMidiEvent {
    pub kind: MidiMsgKind,
    pub channel: u8,
    pub number: u8,
    pub value: u8,
}

/// Parse a standard 3-byte MIDI Note-On or Control-Change message.
pub fn parse_raw_midi(msg: &[u8]) -> Option<RawMidiEvent> {
    if msg.len() < 3 {
        return None;
    }
    let status = msg[0] & 0xF0;
    let channel = msg[0] & 0x0F;
    let number = msg[1] & 0x7F;
    let value = msg[2] & 0x7F;
    match status {
        0x90 if value > 0 => Some(RawMidiEvent {
            kind: MidiMsgKind::NoteOn,
            channel,
            number,
            value,
        }),
        0xB0 => Some(RawMidiEvent {
            kind: MidiMsgKind::ControlChange,
            channel,
            number,
            value,
        }),
        _ => None,
    }
}

/// Build the default mapping table for a hardware controller profile.
pub fn builtin_profile_rules(profile: ControllerProfileId) -> Vec<MidiMappingRule> {
    let mut rules = Vec::new();
    match profile {
        ControllerProfileId::RaneTwelveMk2 => {
            // Rane Twelve MKII front-panel deck select transmits on MIDI channels 1..4 (0..3)
            for (ch, deck) in [
                (0u8, DeckId::DeckA),
                (1u8, DeckId::DeckB),
                (2u8, DeckId::DeckC),
                (3u8, DeckId::DeckD),
            ] {
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::NoteOn,
                        channel: ch,
                        number: 0x00, // Start/Stop button
                    },
                    action: MidiTargetAction::PlayPause(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::NoteOn,
                        channel: ch,
                        number: 0x09, // Cue / Instant Doubles
                    },
                    action: MidiTargetAction::CueReturn(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x09, // Pitch fader MSB
                    },
                    action: MidiTargetAction::DeckPitch(deck),
                });
                for slot in 0u8..4 {
                    rules.push(MidiMappingRule {
                        key: MidiBindingKey {
                            kind: MidiMsgKind::NoteOn,
                            channel: ch,
                            number: 0x01 + slot, // Touch-strip Hot Cues 1-4
                        },
                        action: MidiTargetAction::HotCue(deck, slot),
                    });
                }
            }
        }
        ControllerProfileId::RaneSeventyTwoMk2 => {
            // Rane Seventy-Two MKII 2-channel battle mixer
            rules.push(MidiMappingRule {
                key: MidiBindingKey {
                    kind: MidiMsgKind::ControlChange,
                    channel: 0,
                    number: 0x0B, // MAG FOUR Crossfader
                },
                action: MidiTargetAction::Crossfader,
            });
            rules.push(MidiMappingRule {
                key: MidiBindingKey {
                    kind: MidiMsgKind::ControlChange,
                    channel: 15,
                    number: 0x07, // Master Level
                },
                action: MidiTargetAction::MasterVolume,
            });
            for (ch, pad_ch, deck) in [(0u8, 5u8, DeckId::DeckA), (1u8, 6u8, DeckId::DeckB)] {
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x07, // Channel Volume Fader
                    },
                    action: MidiTargetAction::DeckVolume(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x10, // EQ High
                    },
                    action: MidiTargetAction::DeckEqHigh(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x11, // EQ Mid
                    },
                    action: MidiTargetAction::DeckEqMid(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x12, // EQ Low
                    },
                    action: MidiTargetAction::DeckEqLow(deck),
                });
                for slot in 0u8..4 {
                    rules.push(MidiMappingRule {
                        key: MidiBindingKey {
                            kind: MidiMsgKind::NoteOn,
                            channel: pad_ch,
                            number: 0x24 + slot, // MPC RGB Performance Pads 1-4
                        },
                        action: MidiTargetAction::HotCue(deck, slot),
                    });
                }
            }
        }
        ControllerProfileId::PioneerDdjFlx4 => {
            rules.push(MidiMappingRule {
                key: MidiBindingKey {
                    kind: MidiMsgKind::ControlChange,
                    channel: 6,
                    number: 0x1F,
                },
                action: MidiTargetAction::Crossfader,
            });
            for (ch, deck) in [(0u8, DeckId::DeckA), (1u8, DeckId::DeckB)] {
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::NoteOn,
                        channel: ch,
                        number: 0x0B,
                    },
                    action: MidiTargetAction::PlayPause(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::NoteOn,
                        channel: ch,
                        number: 0x0C,
                    },
                    action: MidiTargetAction::CueReturn(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x13,
                    },
                    action: MidiTargetAction::DeckVolume(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x00,
                    },
                    action: MidiTargetAction::DeckPitch(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x07,
                    },
                    action: MidiTargetAction::DeckEqHigh(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x0B,
                    },
                    action: MidiTargetAction::DeckEqMid(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x0F,
                    },
                    action: MidiTargetAction::DeckEqLow(deck),
                });
            }
        }
        ControllerProfileId::HerculesDjControl => {
            rules.push(MidiMappingRule {
                key: MidiBindingKey {
                    kind: MidiMsgKind::ControlChange,
                    channel: 0,
                    number: 0x36,
                },
                action: MidiTargetAction::Crossfader,
            });
            for (ch, deck) in [(1u8, DeckId::DeckA), (2u8, DeckId::DeckB)] {
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::NoteOn,
                        channel: ch,
                        number: 0x07,
                    },
                    action: MidiTargetAction::PlayPause(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::NoteOn,
                        channel: ch,
                        number: 0x06,
                    },
                    action: MidiTargetAction::CueReturn(deck),
                });
                rules.push(MidiMappingRule {
                    key: MidiBindingKey {
                        kind: MidiMsgKind::ControlChange,
                        channel: ch,
                        number: 0x00,
                    },
                    action: MidiTargetAction::DeckVolume(deck),
                });
            }
        }
        ControllerProfileId::CustomLearn => {}
    }
    rules
}

/// Translate a matched MIDI action + 7-bit value (0..=127) into an L1 `Command`.
pub fn action_to_command(
    action: MidiTargetAction,
    value: u8,
    current_eq: (f64, f64, f64),
) -> Command {
    let norm = f64::from(value) / 127.0;
    match action {
        MidiTargetAction::PlayPause(deck) => Command::PlayPause { deck },
        MidiTargetAction::CueReturn(deck) => Command::CueReturn { deck },
        MidiTargetAction::HotCue(deck, slot) => Command::SetHotCue {
            deck,
            slot: slot as usize,
        },
        MidiTargetAction::DeckVolume(deck) => Command::SetDeckVolume { deck, volume: norm },
        MidiTargetAction::DeckPitch(deck) => Command::SetPitchBend {
            deck,
            pct: (norm * 2.0 - 1.0) * 16.0,
        },
        MidiTargetAction::DeckEqLow(deck) => Command::SetDeckEq {
            deck,
            low: norm * 2.0,
            mid: current_eq.1,
            high: current_eq.2,
        },
        MidiTargetAction::DeckEqMid(deck) => Command::SetDeckEq {
            deck,
            low: current_eq.0,
            mid: norm * 2.0,
            high: current_eq.2,
        },
        MidiTargetAction::DeckEqHigh(deck) => Command::SetDeckEq {
            deck,
            low: current_eq.0,
            mid: current_eq.1,
            high: norm * 2.0,
        },
        MidiTargetAction::Crossfader => Command::SetCrossfader {
            value: norm * 2.0 - 1.0,
        },
        MidiTargetAction::MasterVolume => Command::SetMasterVolume { volume: norm },
        MidiTargetAction::AutoSync => Command::AutoSyncBeatgrid,
        MidiTargetAction::PhaseSync => Command::PhaseSyncBeatgrid,
        MidiTargetAction::ToggleKeylock(deck) => Command::ToggleKeylock { deck },
    }
}

pub struct MidiControllerManager {
    _conn: Option<MidiInputConnection<()>>,
    queue: Arc<Mutex<VecDeque<RawMidiEvent>>>,
    pub connected_port: Option<String>,
    pub active_profile: ControllerProfileId,
    pub rules: Vec<MidiMappingRule>,
    pub learn_target: Option<MidiTargetAction>,
    pub last_event: Option<RawMidiEvent>,
}

impl Default for MidiControllerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiControllerManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            _conn: None,
            queue: Arc::new(Mutex::new(VecDeque::with_capacity(64))),
            connected_port: None,
            active_profile: ControllerProfileId::RaneSeventyTwoMk2,
            rules: builtin_profile_rules(ControllerProfileId::RaneSeventyTwoMk2),
            learn_target: None,
            last_event: None,
        };
        mgr.load_custom_rules();
        mgr
    }

    pub fn list_input_ports() -> Vec<String> {
        let Ok(midi_in) = MidiInput::new("djcraft-probe") else {
            return Vec::new();
        };
        midi_in
            .ports()
            .iter()
            .filter_map(|p| midi_in.port_name(p).ok())
            .collect()
    }

    pub fn set_profile(&mut self, profile: ControllerProfileId) {
        self.active_profile = profile;
        self.rules = builtin_profile_rules(profile);
        if profile == ControllerProfileId::CustomLearn {
            self.load_custom_rules();
        }
    }

    pub fn connect_port(&mut self, port_index: usize) -> Result<String, DjError> {
        let mut midi_in = MidiInput::new("djcraft-midi")
            .map_err(|e| DjError::Sidecar(format!("MIDI init failed: {e}")))?;
        midi_in.ignore(Ignore::None);
        let ports = midi_in.ports();
        let port = ports
            .get(port_index)
            .ok_or_else(|| DjError::Sidecar(format!("MIDI port index {port_index} not found")))?;
        let name = midi_in
            .port_name(port)
            .unwrap_or_else(|_| format!("MIDI Port {port_index}"));

        let q = Arc::clone(&self.queue);
        let conn = midi_in
            .connect(
                port,
                "djcraft-in",
                move |_stamp, msg, _| {
                    if let Some(ev) = parse_raw_midi(msg) {
                        if let Ok(mut guard) = q.lock() {
                            if guard.len() >= 128 {
                                guard.pop_front();
                            }
                            guard.push_back(ev);
                        }
                    }
                },
                (),
            )
            .map_err(|e| DjError::Sidecar(format!("MIDI connect failed: {e}")))?;

        let detected = ControllerProfileId::auto_detect_from_port_name(&name);
        if detected != ControllerProfileId::CustomLearn {
            self.set_profile(detected);
        }
        self._conn = Some(conn);
        self.connected_port = Some(name.clone());
        Ok(name)
    }

    pub fn disconnect(&mut self) {
        self._conn = None;
        self.connected_port = None;
    }

    /// Drain incoming MIDI events, handling MIDI Learn or mapping them to `(MidiTargetAction, value)`.
    pub fn poll_actions(&mut self) -> Vec<(MidiTargetAction, u8)> {
        let mut out = Vec::new();
        let events: Vec<RawMidiEvent> = {
            let Ok(mut guard) = self.queue.lock() else {
                return out;
            };
            guard.drain(..).collect()
        };

        for ev in events {
            self.last_event = Some(ev);
            let key = MidiBindingKey {
                kind: ev.kind,
                channel: ev.channel,
                number: ev.number,
            };
            if let Some(target) = self.learn_target.take() {
                self.rules.retain(|r| r.key != key);
                self.rules.push(MidiMappingRule {
                    key,
                    action: target,
                });
                self.save_custom_rules();
                continue;
            }
            if let Some(rule) = self.rules.iter().find(|r| r.key == key) {
                out.push((rule.action, ev.value));
            }
        }
        out
    }

    fn custom_rules_path() -> Option<PathBuf> {
        let base = std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
            .map(PathBuf::from)?;
        Some(base.join("djcraft").join("midi_learn.json"))
    }

    fn save_custom_rules(&self) {
        let Some(path) = Self::custom_rules_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.rules) {
            let _ = std::fs::write(path, json);
        }
    }

    fn load_custom_rules(&mut self) {
        let Some(path) = Self::custom_rules_path() else {
            return;
        };
        if let Ok(raw) = std::fs::read_to_string(path) {
            if let Ok(saved) = serde_json::from_str::<Vec<MidiMappingRule>>(&raw) {
                for rule in saved {
                    if !self.rules.iter().any(|r| r.key == rule.key) {
                        self.rules.push(rule);
                    }
                }
            }
        }
    }
}

#![forbid(unsafe_code)]
//! AI Lighting Assistant & Art-Net 4 / sACN (E1.31) DMX512 Engine.
//!
//! Pure Safe Rust UDP DMX output (`std::net::UdpSocket`) with beat- and energy-synced
//! scene generation so a solo DJ never needs a second pair of hands for lighting.

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub const ARTNET_PORT: u16 = 6454;
pub const SACN_PORT: u16 = 5568;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LightingProtocol {
    #[default]
    ArtNet,
    SacnE131,
}

impl LightingProtocol {
    pub fn label(self) -> &'static str {
        match self {
            Self::ArtNet => "Art-Net 4 (UDP 6454)",
            Self::SacnE131 => "sACN E1.31 (UDP 5568)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixtureKind {
    /// 4 channels: R, G, B, W
    RgbwPar,
    /// 8 channels: Pan, Tilt, Dimmer, R, G, B, W, Strobe
    MovingHead,
    /// 2 channels: Dimmer, Rate
    Strobe,
    /// 6 channels: Mode, Pattern, Zoom, X-Rot, Y-Rot, Strobe
    DmxLaser,
}

impl FixtureKind {
    pub fn channels(self) -> usize {
        match self {
            Self::RgbwPar => 4,
            Self::MovingHead => 8,
            Self::Strobe => 2,
            Self::DmxLaser => 6,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::RgbwPar => "RGBW Par / Bar (4ch)",
            Self::MovingHead => "Moving Head (8ch)",
            Self::Strobe => "Strobe (2ch)",
            Self::DmxLaser => "DMX Laser (6ch)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchedFixture {
    pub name: String,
    pub kind: FixtureKind,
    /// 1-based DMX start address (`1..=512`).
    pub start_channel: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LightingSceneMode {
    #[default]
    AutoClub,
    ChillWarm,
    TechnoStrobe,
    LaserTunnel,
    Blackout,
}

impl LightingSceneMode {
    pub const ALL: [Self; 5] = [
        Self::AutoClub,
        Self::ChillWarm,
        Self::TechnoStrobe,
        Self::LaserTunnel,
        Self::Blackout,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::AutoClub => "AI Auto Club",
            Self::ChillWarm => "Chill Warm",
            Self::TechnoStrobe => "Techno Strobe",
            Self::LaserTunnel => "Laser Tunnel",
            Self::Blackout => "Blackout",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "auto" | "autoclub" | "auto_club" | "club" => Ok(Self::AutoClub),
            "chill" | "chillwarm" | "chill_warm" | "warm" => Ok(Self::ChillWarm),
            "techno" | "strobe" | "technostrobe" | "techno_strobe" => Ok(Self::TechnoStrobe),
            "laser" | "lasertunnel" | "laser_tunnel" => Ok(Self::LaserTunnel),
            "blackout" | "off" | "dark" => Ok(Self::Blackout),
            other => Err(format!(
                "Unknown lighting mode '{other}' (expected: auto, chill, techno, laser, blackout)"
            )),
        }
    }
}

pub struct LightingEngine {
    pub enabled: bool,
    pub protocol: LightingProtocol,
    pub target_ip: String,
    pub universe: u16,
    pub mode: LightingSceneMode,
    pub master_dimmer: f32,
    pub strobe_enable: bool,
    pub fixtures: Vec<PatchedFixture>,
    pub dmx: [u8; 512],
    pub packets_sent: u64,
    pub last_error: Option<String>,
    sequence: u8,
    phase_acc: f64,
    last_send: Instant,
    socket: Option<UdpSocket>,
}

impl Default for LightingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LightingEngine {
    pub fn new() -> Self {
        let fixtures = vec![
            PatchedFixture {
                name: "Wash Left".to_string(),
                kind: FixtureKind::RgbwPar,
                start_channel: 1,
            },
            PatchedFixture {
                name: "Wash Right".to_string(),
                kind: FixtureKind::RgbwPar,
                start_channel: 5,
            },
            PatchedFixture {
                name: "Mover L".to_string(),
                kind: FixtureKind::MovingHead,
                start_channel: 9,
            },
            PatchedFixture {
                name: "Mover R".to_string(),
                kind: FixtureKind::MovingHead,
                start_channel: 17,
            },
            PatchedFixture {
                name: "Strobe".to_string(),
                kind: FixtureKind::Strobe,
                start_channel: 25,
            },
            PatchedFixture {
                name: "Club Laser".to_string(),
                kind: FixtureKind::DmxLaser,
                start_channel: 27,
            },
        ];
        Self {
            enabled: false,
            protocol: LightingProtocol::ArtNet,
            target_ip: "255.255.255.255".to_string(),
            universe: 0,
            mode: LightingSceneMode::AutoClub,
            master_dimmer: 0.85,
            strobe_enable: true,
            fixtures,
            dmx: [0; 512],
            packets_sent: 0,
            last_error: None,
            sequence: 1,
            phase_acc: 0.0,
            last_send: Instant::now(),
            socket: None,
        }
    }

    /// Generate 512 DMX channels from BPM, beat phase, audio energy, and AI mode.
    pub fn compute_frame(&mut self, bpm: f64, beat_phase: f64, energy: f32) -> [u8; 512] {
        self.dmx.fill(0);
        if self.mode == LightingSceneMode::Blackout || self.master_dimmer <= 0.001 {
            return self.dmx;
        }

        let dim = self.master_dimmer.clamp(0.0, 1.0);
        let beat_pulse = (1.0 - beat_phase.fract() as f32).powf(2.0);
        self.phase_acc = (self.phase_acc + (bpm.max(60.0) / 60.0) * 0.033) % 1000.0;
        let t = self.phase_acc;

        let (r, g, b, w, strobe_lvl) = match self.mode {
            LightingSceneMode::AutoClub => {
                let sweep = (t * 0.6).sin() * 0.5 + 0.5;
                let r = ((1.0 - sweep as f32) * 220.0 + beat_pulse * 35.0) * dim;
                let g = (energy * 140.0 * dim).min(255.0);
                let b = (sweep as f32 * 240.0 + beat_pulse * 15.0) * dim;
                let w = (beat_pulse * energy * 180.0) * dim;
                let strb = if self.strobe_enable && energy > 0.78 && beat_pulse > 0.75 {
                    180
                } else {
                    0
                };
                (r, g, b, w, strb)
            }
            LightingSceneMode::ChillWarm => {
                let breathe = (t * 0.35).sin() as f32 * 0.25 + 0.75;
                (
                    235.0 * dim * breathe,
                    110.0 * dim * breathe,
                    40.0 * dim,
                    80.0 * dim,
                    0,
                )
            }
            LightingSceneMode::TechnoStrobe => {
                let flash = if beat_pulse > 0.55 { 1.0 } else { 0.15 };
                let strb = if self.strobe_enable && beat_pulse > 0.6 {
                    220
                } else {
                    0
                };
                (
                    255.0 * dim * flash,
                    20.0 * dim,
                    255.0 * dim * flash,
                    255.0 * dim * flash,
                    strb,
                )
            }
            LightingSceneMode::LaserTunnel => (10.0 * dim, 220.0 * dim, 255.0 * dim, 0.0, 0),
            LightingSceneMode::Blackout => (0.0, 0.0, 0.0, 0.0, 0),
        };

        let clamp_u8 = |v: f32| v.round().clamp(0.0, 255.0) as u8;

        for (idx, fx) in self.fixtures.iter().enumerate() {
            let base = fx.start_channel.saturating_sub(1) as usize;
            if base + fx.kind.channels() > 512 {
                continue;
            }
            let alt = if idx % 2 == 1 { 1.0 } else { -1.0 };
            match fx.kind {
                FixtureKind::RgbwPar => {
                    self.dmx[base] = clamp_u8(r);
                    self.dmx[base + 1] = clamp_u8(g);
                    self.dmx[base + 2] = clamp_u8(b);
                    self.dmx[base + 3] = clamp_u8(w);
                }
                FixtureKind::MovingHead => {
                    let pan = (128.0 + alt * (t * 1.1).sin() * 75.0).clamp(0.0, 255.0) as u8;
                    let tilt =
                        (110.0 + (t * 1.7 + idx as f64).cos() * 55.0).clamp(0.0, 255.0) as u8;
                    self.dmx[base] = pan;
                    self.dmx[base + 1] = tilt;
                    self.dmx[base + 2] = clamp_u8(255.0 * dim);
                    self.dmx[base + 3] = clamp_u8(r);
                    self.dmx[base + 4] = clamp_u8(g);
                    self.dmx[base + 5] = clamp_u8(b);
                    self.dmx[base + 6] = clamp_u8(w);
                    self.dmx[base + 7] = strobe_lvl;
                }
                FixtureKind::Strobe => {
                    self.dmx[base] = if strobe_lvl > 0 {
                        clamp_u8(255.0 * dim)
                    } else {
                        0
                    };
                    self.dmx[base + 1] = strobe_lvl;
                }
                FixtureKind::DmxLaser => {
                    let active = matches!(
                        self.mode,
                        LightingSceneMode::AutoClub
                            | LightingSceneMode::TechnoStrobe
                            | LightingSceneMode::LaserTunnel
                    );
                    if active {
                        self.dmx[base] = 180; // Auto/DMX pattern mode
                        self.dmx[base + 1] = ((t * 12.0) as u8).wrapping_mul(17);
                        self.dmx[base + 2] = clamp_u8(160.0 * dim);
                        self.dmx[base + 3] = (128.0 + (t * 2.0).sin() * 90.0) as u8;
                        self.dmx[base + 4] = (128.0 + (t * 1.3).cos() * 90.0) as u8;
                        self.dmx[base + 5] = strobe_lvl;
                    }
                }
            }
        }
        self.dmx
    }

    /// Update DMX frame and transmit over UDP (~30 Hz) when enabled.
    pub fn tick_and_send(&mut self, bpm: f64, beat_phase: f64, energy: f32) {
        self.compute_frame(bpm, beat_phase, energy);
        if !self.enabled {
            return;
        }
        if self.last_send.elapsed() < Duration::from_millis(33) {
            return;
        }
        self.last_send = Instant::now();
        let _ = self.send_now();
    }

    pub fn send_now(&mut self) -> Result<(), String> {
        if self.socket.is_none() {
            let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("UDP bind failed: {e}"))?;
            let _ = sock.set_broadcast(true);
            self.socket = Some(sock);
        }
        let Some(ref sock) = self.socket else {
            return Err("UDP socket unavailable".to_string());
        };
        let (port, packet) = match self.protocol {
            LightingProtocol::ArtNet => (
                ARTNET_PORT,
                encode_artnet_dmx(self.universe, self.sequence, &self.dmx),
            ),
            LightingProtocol::SacnE131 => (
                SACN_PORT,
                encode_sacn_e131(self.universe.max(1), self.sequence, &self.dmx),
            ),
        };
        self.sequence = self.sequence.wrapping_add(1).max(1);
        let addr = format!("{}:{port}", self.target_ip.trim());
        match sock.send_to(&packet, &addr) {
            Ok(_) => {
                self.packets_sent += 1;
                self.last_error = None;
                Ok(())
            }
            Err(e) => {
                let msg = format!("Send to {addr} failed: {e}");
                self.last_error = Some(msg.clone());
                Err(msg)
            }
        }
    }
}

/// Encode an Art-Net 4 `ArtDmx` (opcode `0x5000`) UDP packet (530 bytes).
pub fn encode_artnet_dmx(universe: u16, sequence: u8, dmx: &[u8; 512]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(18 + 512);
    pkt.extend_from_slice(b"Art-Net\0");
    pkt.extend_from_slice(&0x5000u16.to_le_bytes()); // OpDmx
    pkt.extend_from_slice(&14u16.to_be_bytes()); // ProtVer 14
    pkt.push(sequence);
    pkt.push(0); // Physical
    pkt.extend_from_slice(&universe.to_le_bytes()); // SubUni + Net
    pkt.extend_from_slice(&512u16.to_be_bytes()); // LengthHi/Lo
    pkt.extend_from_slice(dmx);
    pkt
}

/// Encode an ANSI E1.31 (sACN) Data packet (638 bytes).
pub fn encode_sacn_e131(universe: u16, sequence: u8, dmx: &[u8; 512]) -> Vec<u8> {
    let mut pkt = vec![0u8; 638];
    // Root Layer (bytes 0..38)
    pkt[0..2].copy_from_slice(&0x0010u16.to_be_bytes()); // Preamble size
    pkt[4..16].copy_from_slice(b"ASC-E1.17\0\0\0");
    pkt[16..18].copy_from_slice(&(0x7000u16 | (638 - 16) as u16).to_be_bytes());
    pkt[18..22].copy_from_slice(&0x0000_0004u32.to_be_bytes()); // VECTOR_ROOT_E131_DATA
    pkt[22..38].copy_from_slice(b"DJCRAFT-ARTNET01"); // 16-byte CID
                                                      // Framing Layer (bytes 38..115)
    pkt[38..40].copy_from_slice(&(0x7000u16 | (638 - 38) as u16).to_be_bytes());
    pkt[40..44].copy_from_slice(&0x0000_0002u32.to_be_bytes()); // VECTOR_E131_DATA_PACKET
    pkt[44..57].copy_from_slice(b"DJCraft Light");
    pkt[108] = 100; // Priority
    pkt[111] = sequence;
    pkt[113..115].copy_from_slice(&universe.to_be_bytes());
    // DMP Layer (bytes 115..638)
    pkt[115..117].copy_from_slice(&(0x7000u16 | (638 - 115) as u16).to_be_bytes());
    pkt[117] = 0x02; // VECTOR_DMP_SET_PROPERTY
    pkt[118] = 0xa1; // Address Type & Data Type
    pkt[121..123].copy_from_slice(&0x0001u16.to_be_bytes()); // Address increment
    pkt[123..125].copy_from_slice(&513u16.to_be_bytes()); // Property value count (start code + 512)
    pkt[125] = 0x00; // DMX Start Code
    pkt[126..638].copy_from_slice(dmx);
    pkt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artnet_and_sacn_packet_encoding_and_blackout() {
        let mut eng = LightingEngine::new();
        eng.mode = LightingSceneMode::AutoClub;
        let frame = eng.compute_frame(128.0, 0.1, 0.9);
        assert!(frame[0] > 0 || frame[2] > 0);

        let art = encode_artnet_dmx(0, 7, &frame);
        assert_eq!(art.len(), 530);
        assert_eq!(&art[0..8], b"Art-Net\0");
        assert_eq!(u16::from_le_bytes([art[8], art[9]]), 0x5000);

        let sacn = encode_sacn_e131(1, 7, &frame);
        assert_eq!(sacn.len(), 638);
        assert_eq!(&sacn[4..13], b"ASC-E1.17");
        assert_eq!(sacn[125], 0x00);

        eng.mode = LightingSceneMode::Blackout;
        let off = eng.compute_frame(128.0, 0.1, 0.9);
        assert!(off.iter().all(|&b| b == 0));
    }
}

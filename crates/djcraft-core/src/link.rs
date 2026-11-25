#![forbid(unsafe_code)]
//! Pure-Rust UDP Multicast Beat & Tempo Sync (Ableton Link-compatible topology).
//!
//! Clean-room MIT/Apache-2.0 implementation using `std::net::UdpSocket` on
//! multicast group `224.76.78.75:20808` (zero GPL dependencies, zero `unsafe`).

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

const LINK_GROUP: Ipv4Addr = Ipv4Addr::new(224, 76, 78, 75);
const LINK_PORT: u16 = 20808;
/// Loopback companion port where SynthCraft (and other local Craft apps) listen for Link beacons
/// with zero Windows Firewall / multicast-loopback friction.
pub const SYNTHCRAFT_LOOPBACK_PORT: u16 = 20809;
const MAGIC: &[u8; 4] = b"DJLK";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkBeacon {
    pub node_id: u64,
    pub bpm: f64,
    pub beat_phase: f64,
    pub quantum: f32,
    pub playing: bool,
    pub camelot_key: [u8; 8],
}

impl LinkBeacon {
    pub fn camelot_str(&self) -> &str {
        let len = self
            .camelot_key
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.camelot_key.len());
        std::str::from_utf8(&self.camelot_key[..len]).unwrap_or("")
    }

    pub fn pack_camelot(s: &str) -> [u8; 8] {
        let mut out = [0u8; 8];
        let bytes = s.trim().as_bytes();
        let n = bytes.len().min(out.len());
        out[..n].copy_from_slice(&bytes[..n]);
        out
    }

    pub fn encode(self) -> [u8; 40] {
        let mut buf = [0u8; 40];
        buf[0..4].copy_from_slice(MAGIC);
        buf[4..12].copy_from_slice(&self.node_id.to_le_bytes());
        buf[12..20].copy_from_slice(&self.bpm.to_le_bytes());
        buf[20..28].copy_from_slice(&self.beat_phase.to_le_bytes());
        buf[28] = u8::from(self.playing);
        let len = self
            .camelot_key
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.camelot_key.len());
        buf[29] = len as u8;
        buf[30..38].copy_from_slice(&self.camelot_key);
        buf
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 28 || &bytes[0..4] != MAGIC {
            return None;
        }
        let node_id = u64::from_le_bytes(bytes[4..12].try_into().ok()?);
        let bpm = f64::from_le_bytes(bytes[12..20].try_into().ok()?);
        let beat_phase = f64::from_le_bytes(bytes[20..28].try_into().ok()?);
        if !bpm.is_finite() || !(20.0..=300.0).contains(&bpm) || !beat_phase.is_finite() {
            return None;
        }
        let mut playing = false;
        let mut camelot_key = [0u8; 8];
        if bytes.len() >= 38 {
            playing = bytes[28] != 0;
            let len = usize::from(bytes[29]).min(8);
            camelot_key[..len].copy_from_slice(&bytes[30..30 + len]);
        }
        Some(Self {
            node_id,
            bpm,
            beat_phase: beat_phase.rem_euclid(4.0),
            quantum: 4.0,
            playing,
            camelot_key,
        })
    }
}

pub struct LinkEngine {
    pub enabled: bool,
    pub bpm: f64,
    pub beat_phase: f64,
    pub quantum: f64,
    pub playing: bool,
    pub camelot_key: String,
    node_id: u64,
    socket: Option<UdpSocket>,
    last_tx: Instant,
    peers: HashMap<u64, Instant>,
}

impl Default for LinkEngine {
    fn default() -> Self {
        Self::new(126.0)
    }
}

impl LinkEngine {
    pub fn new(initial_bpm: f64) -> Self {
        let node_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x444A_4352);
        Self {
            enabled: false,
            bpm: initial_bpm.clamp(40.0, 240.0),
            beat_phase: 0.0,
            quantum: 4.0,
            playing: false,
            camelot_key: String::new(),
            node_id,
            socket: None,
            last_tx: Instant::now(),
            peers: HashMap::new(),
        }
    }

    pub fn set_enabled(&mut self, enable: bool) {
        if enable == self.enabled {
            return;
        }
        self.enabled = enable;
        if enable {
            if self.socket.is_none() {
                if let Ok(sock) = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)) {
                    let _ = sock.set_nonblocking(true);
                    let _ = sock.set_multicast_loop_v4(true);
                    let _ = sock.set_multicast_ttl_v4(1);
                    let _ = sock.join_multicast_v4(&LINK_GROUP, &Ipv4Addr::UNSPECIFIED);
                    self.socket = Some(sock);
                }
            }
        } else {
            self.peers.clear();
        }
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Advance local beat phase, broadcast/receive UDP multicast beacons, and return
    /// `Some(new_bpm)` if a peer updated the shared session tempo.
    pub fn tick(&mut self, dt_sec: f64) -> Option<f64> {
        let beats_per_sec = self.bpm / 60.0;
        self.beat_phase =
            (self.beat_phase + dt_sec.max(0.0) * beats_per_sec).rem_euclid(self.quantum);

        if !self.enabled {
            return None;
        }

        let mut peer_bpm = None;
        if let Some(ref sock) = self.socket {
            if self.last_tx.elapsed() >= Duration::from_millis(100) {
                let pkt = LinkBeacon {
                    node_id: self.node_id,
                    bpm: self.bpm,
                    beat_phase: self.beat_phase,
                    quantum: self.quantum as f32,
                    playing: self.playing,
                    camelot_key: LinkBeacon::pack_camelot(&self.camelot_key),
                }
                .encode();
                let _ = sock.send_to(&pkt, SocketAddrV4::new(LINK_GROUP, LINK_PORT));
                let _ = sock.send_to(
                    &pkt,
                    SocketAddrV4::new(Ipv4Addr::LOCALHOST, SYNTHCRAFT_LOOPBACK_PORT),
                );
                self.last_tx = Instant::now();
            }

            let mut buf = [0u8; 64];
            while let Ok((len, _)) = sock.recv_from(&mut buf) {
                if let Some(beacon) = LinkBeacon::decode(&buf[..len]) {
                    if beacon.node_id != self.node_id {
                        self.peers.insert(beacon.node_id, Instant::now());
                        if (beacon.bpm - self.bpm).abs() > 0.05 {
                            self.bpm = beacon.bpm;
                            self.beat_phase = beacon.beat_phase;
                            peer_bpm = Some(beacon.bpm);
                        }
                    }
                }
            }
        }

        let timeout = Duration::from_secs(3);
        self.peers.retain(|_, seen| seen.elapsed() < timeout);
        peer_bpm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_beacon_roundtrip_and_rejection() {
        let beacon = LinkBeacon {
            node_id: 0xDEAD_BEEF_1234_5678,
            bpm: 128.5,
            beat_phase: 2.75,
            quantum: 4.0,
            playing: true,
            camelot_key: LinkBeacon::pack_camelot("8A Am"),
        };
        let bytes = beacon.encode();
        let decoded = LinkBeacon::decode(&bytes).expect("valid beacon");
        assert_eq!(decoded.node_id, beacon.node_id);
        assert!((decoded.bpm - 128.5).abs() < 1e-9);
        assert!((decoded.beat_phase - 2.75).abs() < 1e-9);
        assert!(decoded.playing);
        assert_eq!(decoded.camelot_str(), "8A Am");

        assert!(LinkBeacon::decode(b"BAD!").is_none());
        let mut bad_bpm = bytes;
        bad_bpm[12..20].copy_from_slice(&999.0_f64.to_le_bytes());
        assert!(LinkBeacon::decode(&bad_bpm).is_none());
    }

    #[test]
    fn test_link_engine_beat_phase_advances() {
        let mut link = LinkEngine::new(120.0);
        // 120 BPM = 2 beats/sec -> 0.5s advances phase by 1.0 beat
        assert!(link.tick(0.5).is_none());
        assert!((link.beat_phase - 1.0).abs() < 1e-6);
    }
}

#![forbid(unsafe_code)]
//! Pure-Rust OSC 1.0 UDP bridge for Mixxxxx (video-extended Mixxx fork).
//!
//! Default Mixxxxx OSC ports:
//! - UDP 11119: inbound commands to Mixxxxx (`/mixxxxx/ping`, `/deck/[N]/video_*`)
//! - UDP 11118: outbound status/heartbeat from Mixxxxx (`/mixxxxx/pong`)

use std::net::UdpSocket;
use std::path::PathBuf;
use std::time::Duration;

use djcraft_audio::DeckId;
use serde::{Deserialize, Serialize};

use crate::DjError;

pub const DEFAULT_MIXXXXX_HOST: &str = "127.0.0.1";
pub const DEFAULT_MIXXXXX_IN_PORT: u16 = 11119;
pub const DEFAULT_MIXXXXX_OUT_PORT: u16 = 11118;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixxxxxConfig {
    pub host: String,
    pub in_port: u16,
    pub out_port: u16,
}

impl Default for MixxxxxConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl MixxxxxConfig {
    pub fn from_env() -> Self {
        let host = std::env::var("MIXXXXX_OSC_HOST")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MIXXXXX_HOST.to_string());
        let in_port = std::env::var("MIXXXXX_OSC_IN_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(DEFAULT_MIXXXXX_IN_PORT);
        let out_port = std::env::var("MIXXXXX_OSC_OUT_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(DEFAULT_MIXXXXX_OUT_PORT);
        Self {
            host,
            in_port,
            out_port,
        }
    }

    pub fn target_addr(&self) -> String {
        format!("{}:{}", self.host, self.in_port)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OscArg {
    Int(i32),
    Float(f32),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MixxxxxVideoControls {
    pub enabled: Option<bool>,
    pub fullscreen: Option<bool>,
    pub brightness: Option<f32>,
    pub contrast: Option<f32>,
    pub saturation: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixxxxxPingResult {
    pub target: String,
    pub sent: bool,
    pub pong_received: bool,
    pub binary_found: bool,
    pub binary_path: Option<String>,
    pub note: String,
}

pub fn resolve_mixxxxx_binary() -> Option<PathBuf> {
    if let Ok(env_bin) = std::env::var("MIXXXXX_BIN") {
        let p = PathBuf::from(env_bin);
        if p.is_file() {
            return Some(p);
        }
    }
    let default_win = PathBuf::from(r"D:\Dev\repos\mixxxxx\build\mixxx.exe");
    if default_win.is_file() {
        return Some(default_win);
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in ["mixxx.exe", "mixxx"] {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn push_osc_string(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
}

/// Encode a single OSC 1.0 message with 4-byte alignment and big-endian payloads.
pub fn encode_osc_message(address: &str, args: &[OscArg]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(address.len() + 16 + args.len() * 4);
    push_osc_string(&mut buf, address);

    let mut type_tag = String::with_capacity(1 + args.len());
    type_tag.push(',');
    for arg in args {
        match arg {
            OscArg::Int(_) => type_tag.push('i'),
            OscArg::Float(_) => type_tag.push('f'),
        }
    }
    push_osc_string(&mut buf, &type_tag);

    for arg in args {
        match arg {
            OscArg::Int(v) => buf.extend_from_slice(&v.to_be_bytes()),
            OscArg::Float(v) => buf.extend_from_slice(&v.to_be_bytes()),
        }
    }
    buf
}

pub fn deck_number(deck: DeckId) -> u8 {
    match deck {
        DeckId::DeckA => 1,
        DeckId::DeckB => 2,
        DeckId::DeckC => 3,
        DeckId::DeckD => 4,
    }
}

pub fn send_osc_message(target: &str, address: &str, args: &[OscArg]) -> Result<usize, DjError> {
    let packet = encode_osc_message(address, args);
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| DjError::Sidecar(format!("UDP bind failed: {e}")))?;
    socket
        .send_to(&packet, target)
        .map_err(|e| DjError::Sidecar(format!("UDP send to {target} failed: {e}")))
}

pub fn ping_mixxxxx(cfg: &MixxxxxConfig, timeout_ms: u64) -> Result<MixxxxxPingResult, DjError> {
    let bin = resolve_mixxxxx_binary();
    let target = cfg.target_addr();

    let listener = UdpSocket::bind(("0.0.0.0", cfg.out_port)).ok();
    send_osc_message(&target, "/mixxxxx/ping", &[])?;

    let mut pong_received = false;
    let note = if let Some(sock) = listener {
        let _ = sock.set_read_timeout(Some(Duration::from_millis(timeout_ms.clamp(50, 2000))));
        let mut buf = [0u8; 256];
        match sock.recv_from(&mut buf) {
            Ok((n, _)) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                if text.starts_with("/mixxxxx/pong") {
                    pong_received = true;
                    "Mixxxxx OSC bridge responded with /mixxxxx/pong".to_string()
                } else {
                    format!("Received unexpected UDP packet on {}", cfg.out_port)
                }
            }
            Err(_) => {
                "Sent /mixxxxx/ping; no /mixxxxx/pong within timeout (is Mixxxxx running with OSC enabled?)"
                    .to_string()
            }
        }
    } else {
        format!(
            "Sent /mixxxxx/ping; UDP {} already bound (e.g. by mixx-dj-mcp)",
            cfg.out_port
        )
    };

    Ok(MixxxxxPingResult {
        target,
        sent: true,
        pong_received,
        binary_found: bin.is_some(),
        binary_path: bin.map(|p| p.to_string_lossy().to_string()),
        note,
    })
}

pub fn send_deck_video_controls(
    cfg: &MixxxxxConfig,
    deck: DeckId,
    controls: &MixxxxxVideoControls,
) -> Result<Vec<String>, DjError> {
    let n = deck_number(deck);
    let target = cfg.target_addr();
    let mut sent_addresses = Vec::new();

    if let Some(enabled) = controls.enabled {
        let addr = format!("/deck/{n}/video_enabled");
        send_osc_message(&target, &addr, &[OscArg::Int(i32::from(enabled))])?;
        sent_addresses.push(addr);
    }
    if let Some(fullscreen) = controls.fullscreen {
        let addr = format!("/deck/{n}/video_fullscreen");
        send_osc_message(&target, &addr, &[OscArg::Int(i32::from(fullscreen))])?;
        sent_addresses.push(addr);
    }
    if let Some(brightness) = controls.brightness {
        let addr = format!("/deck/{n}/video_brightness");
        send_osc_message(&target, &addr, &[OscArg::Float(brightness.clamp(0.0, 1.0))])?;
        sent_addresses.push(addr);
    }
    if let Some(contrast) = controls.contrast {
        let addr = format!("/deck/{n}/video_contrast");
        send_osc_message(&target, &addr, &[OscArg::Float(contrast.clamp(0.0, 1.0))])?;
        sent_addresses.push(addr);
    }
    if let Some(saturation) = controls.saturation {
        let addr = format!("/deck/{n}/video_saturation");
        send_osc_message(&target, &addr, &[OscArg::Float(saturation.clamp(0.0, 1.0))])?;
        sent_addresses.push(addr);
    }

    if sent_addresses.is_empty() {
        return Err(DjError::Sidecar(
            "Provide at least one video control field (enabled, fullscreen, brightness, contrast, saturation)"
                .to_string(),
        ));
    }

    Ok(sent_addresses)
}
